//! The relay: a public Minecraft port routed by name, an agent port for launchers, a local admin port, and
//! the rules (Minecraft-account sign-in, one owner per name, per-account limits, bans).

pub mod admin;
mod agents;
pub mod auth;
pub mod config;
pub mod dashboard;
pub mod dns;
pub(crate) mod http;
pub mod metrics;
mod public;
pub mod session;
pub mod store;
mod tls;

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio::sync::{watch, Notify, Semaphore};
use tracing::{info, warn};

pub use config::Config;
use config::LimitsConfig;
use metrics::Metrics;
use store::{BanEntry, NameEntry, SessionEntry, Store};

use crate::proto::{self, check_name, month_of, now_secs, ErrorCode, Limits, ServiceState, Usage};
use session::SessionClock;

/// How a tunnel was told to go away (with the admin's message, for paused/off).
pub(crate) struct Kick {
    notify: Notify,
    reason: Mutex<(ErrorCode, Option<String>)>,
}

impl Kick {
    pub fn new() -> Arc<Kick> {
        Arc::new(Kick { notify: Notify::new(), reason: Mutex::new((ErrorCode::Replaced, None)) })
    }

    pub fn kick(&self, reason: ErrorCode) {
        self.kick_with(reason, None);
    }

    pub fn kick_with(&self, reason: ErrorCode, message: Option<String>) {
        *self.reason.lock().unwrap() = (reason, message);
        self.notify.notify_one();
    }

    pub async fn wait(&self) -> (ErrorCode, Option<String>) {
        self.notify.notified().await;
        self.reason.lock().unwrap().clone()
    }
}

/// How long the relay answers "the host's relay session ended" for a name after its session ran out.
const ENDED_NOTICE_SECS: u64 = 15 * 60;

/// A name that is online: where to open player streams.
#[derive(Clone)]
pub(crate) struct Tunnel {
    pub owner: String,
    pub owner_name: String,
    /// Which agent connection serves it (a newer one replaces it).
    pub agent: u64,
    pub control: tokio_yamux::Control,
    pub conns: Arc<AtomicU32>,
    /// Traffic since it came online: players → host, host → players.
    pub bytes_in: Arc<AtomicU64>,
    pub bytes_out: Arc<AtomicU64>,
    pub kick: Arc<Kick>,
    pub since: u64,
    /// Its session time (Continue / Extend move it).
    pub clock: Arc<SessionClock>,
}

/// One line of the dashboard's events log. Player and launcher IP addresses appear here (and at debug log
/// level), never in the normal info logs.
#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub at: u64,
    /// "online", "offline", "signin", "auth-fail", "join", "kick", "ban", "unban", "name", "admin", "settings"
    pub kind: &'static str,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
}

const EVENTS_KEPT: usize = 500;

/// A signed-in account on an agent connection.
#[derive(Debug, Clone)]
pub(crate) struct Account {
    pub uuid: String,
    pub name: String,
}

pub struct Relay {
    pub cfg: Config,
    pub store: Store,
    pub metrics: Metrics,
    /// The limits in force: relay.toml's, or the dashboard's when changed there.
    limits: RwLock<LimitsConfig>,
    reserved: RwLock<Vec<String>>,
    events: Mutex<VecDeque<Event>>,
    /// Names whose session ran out lately (name → when), for the server list's "session ended" answer.
    recent_ends: Mutex<HashMap<String, u64>>,
    /// The admin's switch, for agents that should hear when it changes.
    service: watch::Sender<(ServiceState, Option<String>)>,
    pub started: u64,
    pub(crate) dashboard: dashboard::State,
    tunnels: Mutex<HashMap<String, Tunnel>>,
    ip_players: Mutex<HashMap<IpAddr, u32>>,
    ip_agents: Mutex<HashMap<IpAddr, u32>>,
    login_attempts: Mutex<HashMap<IpAddr, Vec<u64>>>,
    pending: Arc<Semaphore>,
    http: reqwest::Client,
    dns: dns::Resolver,
    shutdown: watch::Sender<bool>,
    next_id: AtomicU64,
}

/// Counts a connection against its IP address until dropped.
pub(crate) struct IpGuard {
    relay: Arc<Relay>,
    ip: IpAddr,
    agent: bool,
}

impl Drop for IpGuard {
    fn drop(&mut self) {
        let map = if self.agent { &self.relay.ip_agents } else { &self.relay.ip_players };
        let mut m = map.lock().unwrap();
        if let Some(n) = m.get_mut(&self.ip) {
            *n -= 1;
            if *n == 0 {
                m.remove(&self.ip);
            }
        }
    }
}

/// IPv4-mapped IPv6 addresses count as the IPv4 address.
fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
        v4 => v4,
    }
}

/// Whether `ip` is `pattern` (an address) or inside it (CIDR).
pub fn ip_matches(ip: IpAddr, pattern: &str) -> bool {
    let (addr, bits) = match pattern.trim().split_once('/') {
        Some((a, b)) => (a, b.trim().parse::<u32>().ok()),
        None => (pattern.trim(), None),
    };
    let Ok(net) = addr.parse::<IpAddr>() else { return false };
    match (canonical(ip), canonical(net)) {
        (IpAddr::V4(a), IpAddr::V4(n)) => {
            let b = bits.unwrap_or(32).min(32);
            let mask = if b == 0 { 0 } else { u32::MAX << (32 - b) };
            u32::from(a) & mask == u32::from(n) & mask
        }
        (IpAddr::V6(a), IpAddr::V6(n)) => {
            let b = bits.unwrap_or(128).min(128);
            let mask = if b == 0 { 0 } else { u128::MAX << (128 - b) };
            u128::from(a) & mask == u128::from(n) & mask
        }
        _ => false,
    }
}

fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// Constant-time comparison for secrets.
pub(crate) fn same_secret(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

const SESSIONS_PER_ACCOUNT: usize = 20;

impl Relay {
    fn new(cfg: Config, store: Store) -> Result<Relay> {
        let http = reqwest::Client::builder().user_agent(concat!("ResinCraft-Relay/", env!("CARGO_PKG_VERSION"))).build()?;
        let (shutdown, _) = watch::channel(false);
        let (limits, reserved) = store.read(|d| (d.overrides.limits.clone(), d.overrides.reserved_names.clone()));
        let service = store.read(|d| (d.service.state, d.service.message.clone()));
        Ok(Relay {
            recent_ends: Mutex::new(HashMap::new()),
            service: watch::channel(service).0,
            limits: RwLock::new(limits.unwrap_or_else(|| cfg.limits.clone())),
            reserved: RwLock::new(reserved.unwrap_or_else(|| cfg.auth.reserved_names.clone())),
            events: Mutex::new(VecDeque::new()),
            started: now_secs(),
            dashboard: dashboard::State::default(),
            dns: dns::Resolver::new(&cfg.custom_domains.resolvers),
            pending: Arc::new(Semaphore::new(cfg.limits.max_pending_handshakes.max(1))),
            cfg,
            store,
            metrics: Metrics::default(),
            tunnels: Mutex::new(HashMap::new()),
            ip_players: Mutex::new(HashMap::new()),
            ip_agents: Mutex::new(HashMap::new()),
            login_attempts: Mutex::new(HashMap::new()),
            http,
            shutdown,
            next_id: AtomicU64::new(1),
        })
    }

    pub(crate) fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    pub(crate) fn shutdown_rx(&self) -> watch::Receiver<bool> {
        self.shutdown.subscribe()
    }

    /// The limits in force right now.
    pub fn limits_now(&self) -> LimitsConfig {
        self.limits.read().unwrap().clone()
    }

    /// Reserved names on top of the built-in list.
    pub fn reserved_names(&self) -> Vec<String> {
        self.reserved.read().unwrap().clone()
    }

    /// Changes the limits (None: back to relay.toml's) and keeps the change in the data file.
    pub fn set_limits(&self, limits: Option<LimitsConfig>) {
        *self.limits.write().unwrap() = limits.clone().unwrap_or_else(|| self.cfg.limits.clone());
        self.store.write(|d| d.overrides.limits = limits);
    }

    /// Changes the extra reserved names (None: back to relay.toml's).
    pub fn set_reserved(&self, names: Option<Vec<String>>) {
        let names = names.map(|list| {
            let mut v: Vec<String> = list.iter().map(|n| n.trim().to_ascii_lowercase()).filter(|n| !n.is_empty()).collect();
            v.sort();
            v.dedup();
            v
        });
        *self.reserved.write().unwrap() = names.clone().unwrap_or_else(|| self.cfg.auth.reserved_names.clone());
        self.store.write(|d| d.overrides.reserved_names = names);
    }

    /// The admin's switch and message.
    pub fn service(&self) -> (ServiceState, Option<String>) {
        self.service.borrow().clone()
    }

    pub(crate) fn service_rx(&self) -> watch::Receiver<(ServiceState, Option<String>)> {
        self.service.subscribe()
    }

    /// Flips the switch. Off closes every tunnel now; Paused lets running ones go on until their session ends.
    pub fn set_service(&self, state: ServiceState, message: Option<String>) {
        let message = message.map(|m| m.trim().chars().take(200).collect::<String>()).filter(|m| !m.is_empty());
        let now = now_secs();
        self.store.write(|d| d.service = store::ServiceSetting { state, message: message.clone(), since: now });
        self.service.send_replace((state, message.clone()));
        if state == ServiceState::Off {
            for t in self.tunnels.lock().unwrap().values() {
                t.kick.kick_with(ErrorCode::ServiceOff, message.clone());
            }
        }
    }

    /// The text launchers get for a paused or switched-off relay.
    pub fn service_message(&self) -> String {
        self.service().1.unwrap_or_else(|| "Try again later.".into())
    }

    /// Remembers that `name`'s session ran out (the server list says so for a while).
    pub(crate) fn mark_ended(&self, name: &str) {
        let now = now_secs();
        let mut m = self.recent_ends.lock().unwrap();
        m.retain(|_, at| now.saturating_sub(*at) < ENDED_NOTICE_SECS);
        m.insert(name.to_string(), now);
    }

    pub fn recently_ended(&self, name: &str) -> bool {
        self.recent_ends.lock().unwrap().get(name).is_some_and(|at| now_secs().saturating_sub(*at) < ENDED_NOTICE_SECS)
    }

    /// The admin's Extend: another full session for the tunnel online under `name`. Returns the seconds left.
    pub fn admin_extend(&self, name: &str) -> Option<u64> {
        let clock = self.tunnels.lock().unwrap().get(name).map(|t| t.clock.clone())?;
        clock.extend(now_secs(), self.limits_now().session_secs())
    }

    /// The admin's End now: closes the tunnel as if its time ran out.
    pub fn admin_end(&self, name: &str) -> bool {
        let ok = self.kick_name(name, ErrorCode::SessionEnded);
        if ok {
            self.mark_ended(name);
        }
        ok
    }

    /// Adds a line to the events log.
    pub fn event(&self, kind: &'static str, text: impl Into<String>, ip: Option<IpAddr>) {
        let mut ev = self.events.lock().unwrap();
        if ev.len() >= EVENTS_KEPT {
            ev.pop_front();
        }
        ev.push_back(Event { at: now_secs(), kind, text: text.into(), ip: ip.map(|i| canonical(i).to_string()) });
    }

    /// The events log, newest last.
    pub fn events(&self) -> Vec<Event> {
        self.events.lock().unwrap().iter().cloned().collect()
    }

    pub(crate) fn ip_guard(self: &Arc<Self>, ip: IpAddr, agent: bool) -> Option<IpGuard> {
        let ip = canonical(ip);
        let l = self.limits_now();
        let (map, limit) = if agent { (&self.ip_agents, l.agents_per_ip) } else { (&self.ip_players, l.connections_per_ip) };
        let mut m = map.lock().unwrap();
        let n = m.entry(ip).or_insert(0);
        if limit > 0 && *n >= limit {
            return None;
        }
        *n += 1;
        Some(IpGuard { relay: self.clone(), ip, agent })
    }

    pub fn is_banned_ip(&self, ip: IpAddr) -> bool {
        let now = now_secs();
        self.cfg.bans.ips.iter().any(|p| ip_matches(ip, p)) || self.store.read(|d| d.bans.ips.iter().any(|(p, b)| b.active(now) && ip_matches(ip, p)))
    }

    pub fn is_banned_uuid(&self, uuid: &str) -> bool {
        let id = auth::dashed_uuid(uuid).unwrap_or_else(|| uuid.to_string());
        let now = now_secs();
        self.cfg.bans.uuids.iter().any(|u| auth::dashed_uuid(u).as_deref() == Some(id.as_str()))
            || self.store.read(|d| d.bans.uuids.get(&id).is_some_and(|b| b.active(now)))
    }

    /// Bans a Minecraft account (disconnecting its tunnels) or lifts the ban. `expires`: Unix seconds.
    pub fn ban_uuid(&self, uuid: &str, ban: Option<(String, Option<u64>)>) -> Option<String> {
        let id = auth::dashed_uuid(uuid)?;
        match ban {
            Some((reason, expires)) => {
                self.store.write(|d| d.bans.uuids.insert(id.clone(), BanEntry { reason, at: now_secs(), expires }));
                self.kick_account(&id, ErrorCode::Banned);
            }
            None => {
                self.store.write(|d| d.bans.uuids.remove(&id));
            }
        }
        Some(id)
    }

    /// Bans an IP address or CIDR range (players and launchers), or lifts the ban.
    pub fn ban_ip(&self, ip: &str, ban: Option<(String, Option<u64>)>) -> Option<String> {
        let ip = ip.trim().to_string();
        let (addr, bits) = match ip.split_once('/') {
            Some((a, b)) => (a, Some(b)),
            None => (ip.as_str(), None),
        };
        let parsed: IpAddr = addr.parse().ok()?;
        if bits.is_some_and(|b| b.parse::<u8>().map_or(true, |n| n > if parsed.is_ipv4() { 32 } else { 128 })) {
            return None;
        }
        match ban {
            Some((reason, expires)) => self.store.write(|d| d.bans.ips.insert(ip.clone(), BanEntry { reason, at: now_secs(), expires })),
            None => self.store.write(|d| d.bans.ips.remove(&ip)),
        };
        Some(ip)
    }

    /// Sign-ins with a Minecraft token from `ip` in the last hour, counting this one.
    pub(crate) fn login_allowed(&self, ip: IpAddr) -> bool {
        let now = now_secs();
        let mut m = self.login_attempts.lock().unwrap();
        let list = m.entry(canonical(ip)).or_default();
        list.retain(|t| now.saturating_sub(*t) < 3600);
        if list.len() as u32 >= self.limits_now().logins_per_ip_per_hour {
            return false;
        }
        list.push(now);
        true
    }

    pub fn limits(&self) -> Limits {
        let l = self.limits_now();
        Limits {
            tunnels: l.tunnels_per_account,
            names: l.names_per_account,
            connections: l.connections_per_tunnel,
            monthly_bytes: l.monthly_bytes(),
            session_secs: l.session_secs(),
        }
    }

    /// This month's traffic of an account.
    pub fn usage_bytes(&self, uuid: &str) -> u64 {
        let month = month_of(now_secs());
        self.store.read(|d| d.usage.get(uuid).filter(|u| u.month == month).map_or(0, |u| u.bytes))
    }

    pub(crate) fn usage(&self, uuid: &str, connections: u32) -> Usage {
        Usage { month: month_of(now_secs()), bytes: self.usage_bytes(uuid), connections }
    }

    pub fn over_quota(&self, uuid: &str) -> bool {
        let cap = self.limits_now().monthly_bytes();
        cap > 0 && self.usage_bytes(uuid) >= cap
    }

    /// Adds traffic to an account; false once it's over the monthly cap.
    pub(crate) fn add_usage(&self, uuid: &str, bytes: u64) -> bool {
        let month = month_of(now_secs());
        let total = self.store.write(|d| {
            let u = d.usage.entry(uuid.to_string()).or_default();
            if u.month != month {
                u.month = month;
                u.bytes = 0;
            }
            u.bytes += bytes;
            u.bytes
        });
        let cap = self.limits_now().monthly_bytes();
        cap == 0 || total < cap
    }

    pub fn is_reserved(&self, name: &str) -> bool {
        self.store.read(|d| d.names.contains_key(name))
    }

    pub(crate) fn tunnel(&self, name: &str) -> Option<Tunnel> {
        self.tunnels.lock().unwrap().get(name).cloned()
    }

    pub fn tunnels_online(&self) -> usize {
        self.tunnels.lock().unwrap().len()
    }

    /// Is `name` free for this account?
    pub(crate) fn check(&self, uuid: &str, name: &str) -> proto::RelayMsg {
        let status = |available, mine, reason| proto::RelayMsg::NameStatus { name: name.to_string(), available, mine, reason };
        if let Err(code) = check_name(name, &self.reserved_names()) {
            return status(false, false, Some(code));
        }
        match self.store.read(|d| d.names.get(name).map(|e| e.owner.clone())) {
            Some(owner) if owner == uuid => status(true, true, None),
            Some(_) => status(false, false, Some(ErrorCode::NameTaken)),
            None => status(true, false, None),
        }
    }

    /// Reserves `name` for the account (first come, first served), freeing `release` (one of ours) at once.
    pub(crate) fn reserve(&self, acc: &Account, name: &str, release: Option<&str>) -> Result<(), ErrorCode> {
        check_name(name, &self.reserved_names())?;
        let now = now_secs();
        let limit = self.limits_now().names_per_account as usize;
        let released = self.store.write(|d| {
            match d.names.get_mut(name) {
                Some(e) if e.owner != acc.uuid => return Err(ErrorCode::NameTaken),
                Some(e) => {
                    e.owner_name = acc.name.clone();
                    e.last_used = now;
                }
                None => {
                    let freeing = release.is_some_and(|r| r != name && d.names.get(r).is_some_and(|e| e.owner == acc.uuid));
                    let owned = d.names.values().filter(|e| e.owner == acc.uuid).count() - usize::from(freeing);
                    if owned >= limit {
                        return Err(ErrorCode::TooManyNames);
                    }
                    d.names.insert(name.to_string(), NameEntry { owner: acc.uuid.clone(), owner_name: acc.name.clone(), created: now, last_used: now });
                }
            }
            let old = release.filter(|r| *r != name && d.names.get(*r).is_some_and(|e| e.owner == acc.uuid));
            if let Some(old) = old {
                d.names.remove(old);
            }
            Ok(old.map(str::to_string))
        })?;
        if let Some(old) = released {
            self.kick_name(&old, ErrorCode::NameTaken);
        }
        Ok(())
    }

    pub(crate) fn release(&self, acc: &Account, name: &str) -> Result<(), ErrorCode> {
        let removed = self.store.write(|d| match d.names.get(name) {
            Some(e) if e.owner == acc.uuid => d.names.remove(name).is_some(),
            Some(_) => false,
            None => true,
        });
        if !removed {
            return Err(ErrorCode::NameTaken);
        }
        self.kick_name(name, ErrorCode::NameTaken);
        Ok(())
    }

    /// Brings `name` online for `tunnel.owner`; a tunnel already online under that name is replaced.
    pub(crate) fn go_online(&self, acc: &Account, name: &str, tunnel: Tunnel) -> Result<(), ErrorCode> {
        match self.service().0 {
            ServiceState::On => {}
            ServiceState::Paused => return Err(ErrorCode::Paused),
            ServiceState::Off => return Err(ErrorCode::ServiceOff),
        }
        // The limits first, so a refused attempt doesn't leave a new reservation behind.
        check_name(name, &self.reserved_names())?;
        if self.over_quota(&acc.uuid) {
            return Err(ErrorCode::OverQuota);
        }
        let too_many = |tunnels: &HashMap<String, Tunnel>| {
            tunnels.iter().filter(|(n, t)| t.owner == acc.uuid && n.as_str() != name).count() >= self.limits_now().tunnels_per_account as usize
        };
        if too_many(&self.tunnels.lock().unwrap()) {
            return Err(ErrorCode::TooManyTunnels);
        }
        self.reserve(acc, name, None)?;
        let mut tunnels = self.tunnels.lock().unwrap();
        if too_many(&tunnels) {
            return Err(ErrorCode::TooManyTunnels);
        }
        if let Some(old) = tunnels.insert(name.to_string(), tunnel) {
            old.kick.kick(ErrorCode::Replaced);
        }
        Ok(())
    }

    /// Takes `name` offline if `agent` still serves it.
    pub(crate) fn go_offline(&self, name: &str, agent: u64) {
        let mut tunnels = self.tunnels.lock().unwrap();
        if tunnels.get(name).is_some_and(|t| t.agent == agent) {
            tunnels.remove(name);
            drop(tunnels);
            let now = now_secs();
            self.store.write(|d| {
                if let Some(e) = d.names.get_mut(name) {
                    e.last_used = now;
                }
            });
        }
    }

    /// Disconnects the tunnel online under `name`; false when none is.
    pub(crate) fn kick_name(&self, name: &str, reason: ErrorCode) -> bool {
        match self.tunnels.lock().unwrap().get(name) {
            Some(t) => {
                t.kick.kick(reason);
                true
            }
            None => false,
        }
    }

    /// The tunnels online, for the dashboard.
    pub(crate) fn tunnel_list(&self) -> Vec<(String, Tunnel)> {
        let mut v: Vec<(String, Tunnel)> = self.tunnels.lock().unwrap().iter().map(|(n, t)| (n.clone(), t.clone())).collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    /// Frees a name (an admin action); its tunnel, if online, is disconnected.
    pub fn admin_release(&self, name: &str) -> bool {
        let existed = self.store.write(|d| d.names.remove(name).is_some());
        self.kick_name(name, ErrorCode::Released);
        existed
    }

    /// Moves a reservation to another name for the same owner (an admin action).
    pub fn admin_rename(&self, from: &str, to: &str) -> Result<(), &'static str> {
        if check_name(to, &[]).is_err() {
            return Err("the new name isn't valid (3-24 lowercase letters, numbers and dashes)");
        }
        self.store.write(|d| {
            if d.names.contains_key(to) {
                return Err("the new name is taken");
            }
            let entry = d.names.remove(from).ok_or("no such name")?;
            d.names.insert(to.to_string(), entry);
            Ok(())
        })?;
        self.kick_name(from, ErrorCode::Released);
        Ok(())
    }

    /// Disconnects every tunnel of an account (after a ban).
    pub fn kick_account(&self, uuid: &str, reason: ErrorCode) {
        for t in self.tunnels.lock().unwrap().values().filter(|t| t.owner == uuid) {
            t.kick.kick(reason);
        }
    }

    /// A new session token for a freshly verified account (only its hash is kept).
    pub(crate) fn new_session(&self, uuid: &str, username: &str) -> String {
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes).expect("the system's random number generator works");
        use base64::Engine;
        let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        let now = now_secs();
        let expires = now + self.limits_now().session_days * 86_400;
        self.store.write(|d| {
            let mut mine: Vec<(String, u64)> = d.sessions.iter().filter(|(_, s)| s.uuid == uuid).map(|(k, s)| (k.clone(), s.created)).collect();
            if mine.len() >= SESSIONS_PER_ACCOUNT {
                mine.sort_by_key(|(_, created)| *created);
                for (k, _) in mine.iter().take(mine.len() + 1 - SESSIONS_PER_ACCOUNT) {
                    d.sessions.remove(k);
                }
            }
            d.sessions.insert(hash_token(&token), SessionEntry { uuid: uuid.to_string(), username: username.to_string(), created: now, expires });
        });
        token
    }

    /// The account behind a session token (extending it while it's in use).
    pub(crate) fn resume(&self, token: &str) -> Option<Account> {
        let key = hash_token(token);
        let now = now_secs();
        let renew = now + self.limits_now().session_days * 86_400;
        let found = self.store.read(|d| d.sessions.get(&key).filter(|s| s.expires > now).map(|s| (s.uuid.clone(), s.username.clone(), s.expires)))?;
        // Extend at most once a day, so the file isn't rewritten on every reconnect.
        if renew.saturating_sub(found.2) > 86_400 {
            self.store.write(|d| {
                if let Some(s) = d.sessions.get_mut(&key) {
                    s.expires = renew;
                }
            });
        }
        Some(Account { uuid: found.0, name: found.1 })
    }

    /// Drops expired sessions, names nobody used for a long time and old traffic counters.
    pub fn prune(&self) {
        let now = now_secs();
        let month = month_of(now);
        let expiry_days = self.limits_now().name_expiry_days;
        let expiry = expiry_days * 86_400;
        let online: Vec<String> = self.tunnels.lock().unwrap().keys().cloned().collect();
        let mut freed = 0;
        self.store.write(|d| {
            d.sessions.retain(|_, s| s.expires > now);
            let before = d.names.len();
            d.names.retain(|n, e| online.contains(n) || now.saturating_sub(e.last_used) < expiry);
            freed = before - d.names.len();
            d.usage.retain(|_, u| u.month == month);
            d.bans.uuids.retain(|_, b| b.active(now));
            d.bans.ips.retain(|_, b| b.active(now));
        });
        if freed > 0 {
            info!(freed, "freed names unused for {expiry_days} days");
        }
        self.dashboard.prune(now);
        self.login_attempts.lock().unwrap().retain(|_, l| l.iter().any(|t| now.saturating_sub(*t) < 3600));
    }
}

fn bind(addr: &str) -> Result<TcpListener> {
    use socket2::{Domain, Protocol, Socket, Type};
    let addr: SocketAddr = addr.trim().parse().with_context(|| format!("{addr} isn't an address like 0.0.0.0:25565"))?;
    let socket = Socket::new(Domain::for_address(addr), Type::STREAM, Some(Protocol::TCP))?;
    // Lets "0.0.0.0:25565" and "[::]:25565" both be listed.
    if addr.is_ipv6() {
        socket.set_only_v6(true)?;
    }
    #[cfg(unix)]
    socket.set_reuse_address(true)?;
    socket.set_nonblocking(true)?;
    socket.bind(&addr.into()).with_context(|| format!("couldn't listen on {addr}"))?;
    socket.listen(1024)?;
    Ok(TcpListener::from_std(socket.into())?)
}

/// A relay with its ports open, ready to run.
pub struct Server {
    relay: Arc<Relay>,
    public: Vec<TcpListener>,
    agents: Vec<TcpListener>,
    admin: Option<TcpListener>,
    dashboard: Option<TcpListener>,
    tls: Option<(tokio_rustls::TlsAcceptor, Arc<tls::CertFiles>)>,
}

impl Server {
    pub async fn bind(cfg: Config) -> Result<Server> {
        let store = Store::load(&cfg.data_file)?;
        let tls = match &cfg.tls {
            Some(t) => {
                let files = tls::CertFiles::open(t)?;
                Some((tls::acceptor(files.clone())?, files))
            }
            None => None,
        };
        let public = cfg.public_listen.iter().map(|a| bind(a)).collect::<Result<Vec<_>>>()?;
        let agents = cfg.agent_listen.iter().map(|a| bind(a)).collect::<Result<Vec<_>>>()?;
        let admin = cfg.admin_listen.as_deref().map(bind).transpose()?;
        let dashboard = cfg.dashboard.listen.as_deref().map(bind).transpose()?;
        Ok(Server { relay: Arc::new(Relay::new(cfg, store)?), public, agents, admin, dashboard, tls })
    }

    pub fn relay(&self) -> Arc<Relay> {
        self.relay.clone()
    }

    pub fn public_addrs(&self) -> Vec<SocketAddr> {
        self.public.iter().filter_map(|l| l.local_addr().ok()).collect()
    }

    pub fn agent_addrs(&self) -> Vec<SocketAddr> {
        self.agents.iter().filter_map(|l| l.local_addr().ok()).collect()
    }

    pub fn admin_addr(&self) -> Option<SocketAddr> {
        self.admin.as_ref().and_then(|l| l.local_addr().ok())
    }

    pub fn dashboard_addr(&self) -> Option<SocketAddr> {
        self.dashboard.as_ref().and_then(|l| l.local_addr().ok())
    }

    /// Serves until `shutdown` completes, then stops accepting, tells launchers to reconnect shortly, lets
    /// connected players finish for up to `drain_secs` and saves the data.
    pub async fn run(self, shutdown: impl Future<Output = ()>) -> Result<()> {
        let Server { relay, public, agents, admin, dashboard, tls } = self;
        let mut accept = Vec::new();
        let (public_addrs, agent_addrs): (Vec<_>, Vec<_>) =
            (public.iter().filter_map(|l| l.local_addr().ok()).collect(), agents.iter().filter_map(|l| l.local_addr().ok()).collect());
        for l in public {
            let relay = relay.clone();
            accept.push(tokio::spawn(async move {
                loop {
                    match l.accept().await {
                        Ok((sock, peer)) => {
                            tokio::spawn(public::serve(relay.clone(), sock, peer));
                        }
                        Err(e) => {
                            warn!(error = %e, "accepting a player failed");
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    }
                }
            }));
        }
        let acceptor = tls.as_ref().map(|(a, _)| a.clone());
        for l in agents {
            let (relay, acceptor) = (relay.clone(), acceptor.clone());
            accept.push(tokio::spawn(async move {
                loop {
                    match l.accept().await {
                        Ok((sock, peer)) => {
                            tokio::spawn(agents::serve(relay.clone(), sock, peer, acceptor.clone()));
                        }
                        Err(e) => {
                            warn!(error = %e, "accepting a launcher failed");
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    }
                }
            }));
        }
        let admin_addr = admin.as_ref().and_then(|l| l.local_addr().ok());
        if let Some(l) = admin {
            accept.push(tokio::spawn(admin::serve(relay.clone(), l)));
        }
        let dashboard_addr = dashboard.as_ref().and_then(|l| l.local_addr().ok());
        if let Some(l) = dashboard {
            accept.push(tokio::spawn(dashboard::serve(relay.clone(), l, acceptor.clone())));
        }
        let certs = tls.map(|(_, files)| files);
        let upkeep = {
            let relay = relay.clone();
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(Duration::from_secs(30));
                let mut n: u64 = 0;
                loop {
                    tick.tick().await;
                    n += 1;
                    if n % 120 == 1 {
                        relay.prune();
                    }
                    if n % 10 == 0 {
                        if let Some(files) = &certs {
                            match files.reload_if_changed() {
                                Ok(true) => info!("loaded the renewed TLS certificate"),
                                Ok(false) => {}
                                Err(e) => warn!(error = %e, "couldn't load the changed TLS certificate; keeping the old one"),
                            }
                        }
                    }
                    if let Err(e) = relay.store.save().await {
                        warn!(error = %e, "couldn't save the relay data");
                    }
                }
            })
        };
        info!(
            base_domain = %relay.cfg.base_domain,
            public = ?public_addrs,
            agents = ?agent_addrs,
            admin = ?admin_addr,
            dashboard = ?dashboard_addr,
            tls = relay.cfg.tls.is_some(),
            "ResinCraft Relay is running"
        );

        shutdown.await;
        info!("shutting down: no new connections, launchers told to reconnect");
        for t in &accept {
            t.abort();
        }
        let _ = relay.shutdown.send(true);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(relay.cfg.drain_secs);
        loop {
            let players = relay.metrics.player_connections_active.load(Ordering::Relaxed);
            if players <= 0 || tokio::time::Instant::now() >= deadline {
                if players > 0 {
                    info!(players, "closing the remaining player connections");
                }
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        upkeep.abort();
        relay.store.save().await?;
        info!("stopped");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_ip_bans() {
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        assert!(ip_matches(ip("203.0.113.7"), "203.0.113.7"));
        assert!(!ip_matches(ip("203.0.113.8"), "203.0.113.7"));
        assert!(ip_matches(ip("198.51.100.200"), "198.51.100.0/24"));
        assert!(!ip_matches(ip("198.51.101.1"), "198.51.100.0/24"));
        assert!(ip_matches(ip("::ffff:198.51.100.9"), "198.51.100.0/24"));
        assert!(ip_matches(ip("2001:db8::1"), "2001:db8::/32"));
        assert!(!ip_matches(ip("2001:db9::1"), "2001:db8::/32"));
        assert!(ip_matches(ip("10.1.2.3"), "0.0.0.0/0"));
        assert!(!ip_matches(ip("10.1.2.3"), "not an ip"));
        assert!(same_secret("abc", "abc") && !same_secret("abc", "abd") && !same_secret("abc", "abcd"));
    }

    #[test]
    fn bans_expire() {
        let now = now_secs();
        assert!(BanEntry { reason: String::new(), at: now, expires: None }.active(now));
        assert!(BanEntry { reason: String::new(), at: now, expires: Some(now + 60) }.active(now));
        assert!(!BanEntry { reason: String::new(), at: now - 120, expires: Some(now - 60) }.active(now));
    }
}
