//! ResinCraft Relay from the launcher's side (the relay itself lives in ../relay): friends join
//! `<name>.rscraft.resincore.tech`, and the relay sends them here over one outgoing connection, so nothing has to be
//! opened on the router. It needs a signed-in Microsoft account: the relay checks Minecraft ownership once
//! and hands out its own session token, kept sealed in `<dataDir>/relay-sessions.json`.
//!
//! The relay's address is compiled in (RESINCRAFT_RELAY_URL and RESINCRAFT_RELAY_DOMAIN, see build.rs); the
//! same environment variables at runtime override it, and RESINCRAFT_RELAY_CA adds a root certificate to
//! trust (development and tests).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use resincraft_relay::agent::{AgentConfig, AgentError, Command, Connection, Credentials, Event};
use resincraft_relay::proto::{self, DomainState, ErrorCode, JoinOutcome, Limits, ServiceState, Usage};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::core::fsutil::{read_json, write_json};
use crate::core::paths;
use crate::core::text::{text_of, Text};
use crate::core::types::{DnsRecord, DomainOwnership, DomainTxt, RelayInfo, RelayNameAccount, RelayNameCheck, RelayNameEntry, RelayNameRequest, RelayNameServer, RelayNames, ServerMeta};
use crate::player::accounts::{self, MicrosoftAccount};
use crate::{err_tr, tr};

use super::store;

/// Where the relay is.
#[derive(Debug, Clone)]
pub struct Settings {
    /// "tls://relay.rscraft.resincore.tech:7000"
    pub url: String,
    /// "rscraft.resincore.tech"
    pub domain: String,
}

/// The relay this build uses, if any.
pub fn settings() -> Option<Settings> {
    fn pick(key: &str, built: Option<&'static str>) -> Option<String> {
        std::env::var(key).ok().or_else(|| built.map(str::to_string)).map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
    }
    let url = pick("RESINCRAFT_RELAY_URL", option_env!("RESINCRAFT_RELAY_URL"))?;
    let domain = pick("RESINCRAFT_RELAY_DOMAIN", option_env!("RESINCRAFT_RELAY_DOMAIN"))?;
    Some(Settings { url, domain: domain.trim_matches('.').to_ascii_lowercase() })
}

fn agent_config(s: &Settings) -> AgentConfig {
    let extra_roots_pem = std::env::var_os("RESINCRAFT_RELAY_CA").and_then(|p| std::fs::read(p).ok());
    AgentConfig { url: s.url.clone(), extra_roots_pem, client: format!("ResinCraft Launcher {}", env!("CARGO_PKG_VERSION")) }
}

fn not_configured() -> anyhow::Error {
    err_tr!("hostRelay.notConfigured", "This build of ResinCraft Launcher doesn't include ResinCraft Relay.")
}

pub(crate) fn needs_microsoft() -> anyhow::Error {
    err_tr!(
        "hostRelay.needsMicrosoft",
        "ResinCraft Relay needs a Microsoft account that owns Minecraft. Sign in with Microsoft in Accounts (offline profiles can't use it)."
    )
}

/// Whether Automatic can use the relay: it's built in and a Microsoft account is signed in.
pub async fn usable(meta: &ServerMeta) -> bool {
    settings().is_some() && accounts::microsoft_account(meta.relay_account.as_deref()).await.is_some()
}

/// The relay admin's own words that came with an error.
fn admin_message(e: &AgentError) -> String {
    match e {
        AgentError::Refused(_, msg) if !msg.is_empty() => msg.clone(),
        _ => "-".into(),
    }
}

/// A moment in a `{placeholder}`: the UI shows it as a date and time in the player's language and time zone
/// (the English text in logs: "2026-11-01 18:00 UTC").
struct DateTime(u64);

impl std::fmt::Display for DateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&proto::utc_text(self.0))
    }
}

impl serde::Serialize for DateTime {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        serde_json::json!({ "dateTime": self.0 }).serialize(s)
    }
}

/// The time from now until a moment: the UI shows it like "in 3 days" and keeps it current.
struct TimeLeft(u64);

impl std::fmt::Display for TimeLeft {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let minutes = self.0.saturating_sub(proto::now_secs()).div_ceil(60).max(1);
        match minutes {
            m if m < 120 => write!(f, "in {m} minute{}", if m == 1 { "" } else { "s" }),
            m if m < 48 * 60 => write!(f, "in {} hours", (m + 30) / 60),
            m => write!(f, "in {} days", (m + 720) / 1440),
        }
    }
}

impl serde::Serialize for TimeLeft {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        serde_json::json!({ "timeLeft": self.0 }).serialize(s)
    }
}

/// What a banned player is told: until when (or permanently) and why.
fn banned_error(ban: &proto::Ban) -> anyhow::Error {
    let base = match ban.until {
        Some(t) => tr!("hostRelay.bannedUntil", "You're banned from ResinCraft Relay until {date} ({left}).", date = DateTime(t), left = TimeLeft(t)),
        None => tr!("hostRelay.bannedForever", "You're banned from ResinCraft Relay permanently."),
    };
    match ban.reason.trim() {
        "" => base.into(),
        why => err_tr!("hostRelay.bannedReason", "{ban} Reason: {reason}", ban = base, reason = why.to_string()),
    }
}

/// Whether an error is the relay's ban (with how long and why, or from a relay that doesn't say).
pub fn is_banned(e: &anyhow::Error) -> bool {
    text_of(e).is_some_and(|t| t.key.starts_with("hostRelay.banned"))
}

/// Turns the relay's answer into what the player is told.
pub fn describe(e: &AgentError, name: &str) -> anyhow::Error {
    if let AgentError::Banned(ban) = e {
        return banned_error(ban);
    }
    if let AgentError::ListingBanned(ban) = e {
        let base = match ban.until {
            Some(t) => tr!("hostRelay.listBannedUntil", "You're banned from the ResinCraft server list until {date} ({left}).", date = DateTime(t), left = TimeLeft(t)),
            None => tr!("hostRelay.listBanned", "You're banned from the ResinCraft server list."),
        };
        return match ban.reason.trim() {
            "" => base.into(),
            why => err_tr!("hostRelay.listBannedReason", "{ban} Reason: {reason}", ban = base, reason = why.to_string()),
        };
    }
    let Some(code) = e.code() else {
        return match e {
            AgentError::Config(msg) => err_tr!("hostRelay.badConfig", "This build's ResinCraft Relay address doesn't work ({error}).", error = msg),
            other => err_tr!("hostRelay.unreachable", "Couldn't reach ResinCraft Relay ({error}).", error = other.to_string()),
        };
    };
    match code {
        ErrorCode::AuthFailed => err_tr!("hostRelay.authFailed", "ResinCraft Relay couldn't confirm your Minecraft account. Sign in again in Accounts."),
        ErrorCode::NotOwned => err_tr!("hostRelay.notOwned", "This Microsoft account doesn't own Minecraft: Java Edition, which ResinCraft Relay needs."),
        // A relay from before bans said how long and why.
        ErrorCode::Banned => err_tr!("hostRelay.banned", "This Minecraft account can't use ResinCraft Relay."),
        ErrorCode::NameInvalid => err_tr!("hostRelay.nameInvalid", "Use 3 to 24 lowercase letters, numbers and dashes, not starting or ending with a dash."),
        ErrorCode::NameReserved => err_tr!("hostRelay.nameReserved", "\"{name}\" is reserved. Pick another name.", name = name),
        ErrorCode::NameTaken => err_tr!("hostRelay.nameTaken", "\"{name}\" belongs to another player. Pick another name.", name = name),
        ErrorCode::TooManyNames => err_tr!("hostRelay.tooManyNames", "Your account already has as many ResinCraft addresses as it may. Reuse the name of another server."),
        ErrorCode::NameRequestsOff => err_tr!("hostRelay.nameRequestsOff", "ResinCraft Relay isn't taking requests for more names right now."),
        ErrorCode::NameRequestPending => err_tr!("hostRelay.nameRequestPending", "Your request is already waiting for the relay's admin."),
        ErrorCode::TooManyTunnels => err_tr!("hostRelay.tooManyTunnels", "Your account already has as many servers online through ResinCraft Relay as it may. Stop one first."),
        ErrorCode::OverQuota => err_tr!("hostRelay.overQuota", "This month's ResinCraft Relay traffic for your account is used up. It resets on the 1st."),
        ErrorCode::TopUpNotYet => err_tr!("hostRelay.topUpNotYet", "There's still enough ResinCraft Relay data left this month. You can get more once it runs low."),
        ErrorCode::TopUpLimit => err_tr!(
            "hostRelay.topUpLimit",
            "No more ResinCraft Relay top-ups this month. The relay's admin can add data, or it resets on the 1st."
        ),
        ErrorCode::RateLimited => err_tr!("hostRelay.rateLimited", "Too many attempts. Try again in a few minutes."),
        ErrorCode::Unavailable => err_tr!("hostRelay.mojangDown", "Minecraft's account service didn't answer. Try again in a moment."),
        ErrorCode::Version => err_tr!("hostRelay.outdated", "This ResinCraft Launcher is too old for ResinCraft Relay. Update the launcher."),
        ErrorCode::Replaced => err_tr!("hostRelay.replaced", "{name} came online somewhere else, so it stopped here.", name = name),
        ErrorCode::ShuttingDown => err_tr!("hostRelay.restarting", "ResinCraft Relay is restarting."),
        ErrorCode::Paused => err_tr!("hostRelay.paused", "ResinCraft Relay is paused by its admin: {message}", message = admin_message(e)),
        ErrorCode::ServiceOff => err_tr!("hostRelay.off", "ResinCraft Relay is switched off by its admin: {message}", message = admin_message(e)),
        ErrorCode::SessionEnded => err_tr!("hostRelay.sessionEnded", "The ResinCraft Relay session ended."),
        ErrorCode::DomainInvalid => err_tr!("hostRelay.domainInvalid", "ResinCraft Relay can't take this domain: {error}", error = admin_message(e)),
        ErrorCode::ListingInvalid => err_tr!("hostRelay.listingInvalid", "The server list can't take this: {error}", error = admin_message(e)),
        ErrorCode::ListingLimit => err_tr!(
            "hostRelay.listingLimit",
            "You've already submitted as many servers as the list allows. Remove one first."
        ),
        ErrorCode::PaymentsOff => err_tr!("hostRelay.paymentsOff", "ResinCraft Relay isn't selling anything right now: everything is free."),
        ErrorCode::NotForSale => err_tr!("hostRelay.notForSale", "That isn't for sale, or this account has it already."),
        ErrorCode::CodeInvalid => err_tr!("hostRelay.codeInvalid", "That code doesn't work: {error}", error = admin_message(e)),
        ErrorCode::PaymentFailed => err_tr!("hostRelay.paymentFailed", "The payment didn't go through: {error}", error = admin_message(e)),
        ErrorCode::BedrockUnavailable => err_tr!(
            "hostRelay.noBedrock",
            "ResinCraft Relay can't carry Bedrock servers right now (it has no free Bedrock ports, or hasn't been updated for them yet)."
        ),
        // Rarer ones (the relay's admin kicked the server or released its name…): the relay's own words.
        _ => {
            let why = match e {
                AgentError::Refused(_, msg) if !msg.is_empty() => msg.clone(),
                other => other.to_string(),
            };
            err_tr!("hostRelay.refused", "ResinCraft Relay refused: {error}", error = why)
        }
    }
}

// ---------- Session tokens ----------

fn sessions_file() -> std::path::PathBuf {
    paths::root().join("relay-sessions.json")
}

fn session_key(s: &Settings, uuid: &str) -> String {
    format!("{}|{uuid}", s.url)
}

async fn saved_session(s: &Settings, uuid: &str) -> Option<String> {
    let all: HashMap<String, String> = read_json(&sessions_file()).await.unwrap_or_default();
    accounts::unseal(all.get(&session_key(s, uuid)).map(String::as_str)).ok()
}

async fn save_session(s: &Settings, uuid: &str, token: Option<&str>) {
    let mut all: HashMap<String, String> = read_json(&sessions_file()).await.unwrap_or_default();
    match token {
        Some(t) => all.insert(session_key(s, uuid), accounts::seal(t)),
        None => all.remove(&session_key(s, uuid)),
    };
    if let Err(e) = write_json(&sessions_file(), &all).await {
        eprintln!("[relay] couldn't save the relay session: {e:#}");
    }
}

/// Why connecting failed: the relay (maybe worth retrying) or the account (needs the player).
enum Failure {
    Relay(AgentError),
    Account(anyhow::Error),
}

impl Failure {
    fn retryable(&self) -> bool {
        matches!(self, Failure::Relay(e) if e.retryable())
    }

    fn into_error(self, name: &str) -> anyhow::Error {
        match self {
            Failure::Relay(e) => describe(&e, name),
            Failure::Account(e) => e,
        }
    }
}

/// The relay said (at the last connection) that each name's whitelist page is `https://<name>.<base domain>/`.
static NAME_PAGES: AtomicBool = AtomicBool::new(false);

/// Whether `https://<name>.<base domain>/` answered (and when it was tried): the relay may serve name pages before it
/// says so (an older relay with a wildcard certificate), so the launcher looks for itself.
static NAME_PAGE_PROBES: Mutex<Option<HashMap<String, (bool, std::time::Instant)>>> = Mutex::new(None);

/// The name's own address serves its whitelist page with a working certificate (asked again after 6 hours, or 10
/// minutes when it didn't).
async fn name_page_works(address: &str) -> bool {
    let cached = NAME_PAGE_PROBES.lock().unwrap().as_ref().and_then(|m| m.get(address).copied());
    if let Some((ok, at)) = cached {
        if at.elapsed() < Duration::from_secs(if ok { 6 * 3600 } else { 600 }) {
            return ok;
        }
    }
    let ok = crate::core::ctx::http()
        .get(format!("https://{address}/"))
        .timeout(Duration::from_secs(6))
        .send()
        .await
        .is_ok_and(|r| r.status().is_success());
    NAME_PAGE_PROBES.lock().unwrap().get_or_insert_with(HashMap::new).insert(address.to_string(), (ok, std::time::Instant::now()));
    ok
}

/// The web page where players ask to be whitelisted, at its best address: the player's own domain once the relay
/// verified it and has a certificate for it, else the name's own address once it works (the relay says so, or it
/// answers over HTTPS), else `https://<base domain>/join/<name>` (always covered by the relay's certificate).
pub async fn join_page(meta: &ServerMeta) -> Option<String> {
    let address = address_of(meta)?;
    if let (Some(domain), Some(o)) = (meta.domain.as_deref(), super::domain::ownership(&meta.id)) {
        if o.state == "verified" && o.https {
            return Some(format!("https://{domain}/"));
        }
    }
    if NAME_PAGES.load(Ordering::Relaxed) || name_page_works(&address).await {
        return Some(format!("https://{address}/"));
    }
    let (name, base) = address.split_once('.')?;
    Some(format!("https://{base}/join/{name}"))
}

/// Connects and signs in: with the saved relay session, else with the account's Minecraft token (refreshed
/// once if the relay doesn't accept it).
async fn connect(s: &Settings, account: &MicrosoftAccount) -> Result<Connection, Failure> {
    let mut conn = Connection::connect(&agent_config(s)).await.map_err(Failure::Relay)?;
    NAME_PAGES.store(conn.name_pages, Ordering::Relaxed);
    if let Some(session) = saved_session(s, &account.uuid).await {
        match conn.auth(Credentials::Session(session)).await {
            Ok(_) => return Ok(conn),
            Err(AgentError::Refused(ErrorCode::SessionExpired, _)) => save_session(s, &account.uuid, None).await,
            Err(e) => return Err(Failure::Relay(e)),
        }
    }
    let mut token = accounts::microsoft_token(&account.id, false).await.map_err(Failure::Account)?;
    for attempt in 0..2 {
        match conn.auth(Credentials::Minecraft(token.clone())).await {
            Ok(authed) => {
                if let Some(session) = authed.session {
                    save_session(s, &account.uuid, Some(&session)).await;
                }
                return Ok(conn);
            }
            Err(AgentError::Refused(ErrorCode::AuthFailed, _)) if attempt == 0 => {
                token = accounts::microsoft_token(&account.id, true).await.map_err(Failure::Account)?;
            }
            Err(e) => return Err(Failure::Relay(e)),
        }
    }
    Err(Failure::Relay(AgentError::Refused(ErrorCode::AuthFailed, String::new())))
}

/// Connects and signs in as `account` for remote control (remote.rs), with the same saved session as tunnels.
pub(crate) async fn connect_for_remote(account: &MicrosoftAccount) -> Result<Connection, AgentError> {
    let s = settings().ok_or_else(|| AgentError::Config("no relay in this build".into()))?;
    connect(&s, account).await.map_err(|f| match f {
        Failure::Relay(e) => e,
        Failure::Account(e) => AgentError::Refused(ErrorCode::AuthFailed, e.to_string()),
    })
}

/// The relay session token of `account` (signing in first when there's none yet), for the relay's HTTPS uploads
/// (client packs), and the relay's base domain.
pub(crate) async fn session_token(account: &MicrosoftAccount) -> Result<(String, String)> {
    let Some(s) = settings() else { return Err(describe(&AgentError::Config("no relay in this build".into()), "")) };
    if let Some(t) = saved_session(&s, &account.uuid).await {
        return Ok((t, s.domain.clone()));
    }
    drop(connect(&s, account).await.map_err(|f| f.into_error(""))?);
    let t = saved_session(&s, &account.uuid).await.ok_or_else(|| anyhow::anyhow!("ResinCraft Relay didn't give this account a session"))?;
    Ok((t, s.domain.clone()))
}

/// Connects and signs in as `account` for the server list (listing.rs); errors are worded for players.
pub(crate) async fn connect_for_list(account: &MicrosoftAccount) -> Result<Connection> {
    let Some(s) = settings() else { return Err(describe(&AgentError::Config("no relay in this build".into()), "")) };
    connect(&s, account).await.map_err(|f| f.into_error(""))
}

/// A name for a server that has none yet: from its name, with a short suffix when that's taken.
async fn pick_name(conn: &mut Connection, server_name: &str) -> Result<String, Failure> {
    let base = proto::suggest_name(server_name);
    let mut candidates = vec![base.clone()];
    let stem: String = base.chars().take(proto::NAME_MAX - 5).collect::<String>().trim_end_matches('-').to_string();
    for _ in 0..3 {
        let hex = uuid::Uuid::new_v4().simple().to_string();
        candidates.push(format!("{stem}-{}", &hex[..4]));
    }
    for name in &candidates {
        if conn.check(name).await.map_err(Failure::Relay)?.available {
            return Ok(name.clone());
        }
    }
    Err(Failure::Relay(AgentError::Refused(ErrorCode::NameTaken, String::new())))
}

// ---------- Running a tunnel ----------

/// What the card shows while the tunnel runs.
pub enum Status {
    /// Back online after reconnecting (or after the relay was paused).
    /// Back online. `geyser_address`: where Bedrock players join a Java server with Geyser ("name.domain:port").
    Online { address: String, geyser_address: Option<String>, limits: Limits, usage: Usage, secs_left: Option<u64> },
    Usage(Usage),
    /// The session is within the time the host may continue in (15 minutes by default): remind them once.
    ContinueSoon { secs_left: u64 },
    /// More data: the host's own top-up, or the relay's admin added some (`by_admin`).
    TopUp { added_bytes: u64, by_admin: bool, usage: Usage },
    /// The session ends in `secs_left` seconds unless the host continues.
    Warning { secs_left: u64 },
    /// Continued (by the host, or the relay's admin extended it).
    Extended { secs_left: u64, continues: u32 },
    /// The relay's admin paused it or switched it off. `running`: this tunnel goes on until its session ends;
    /// otherwise the launcher waits and comes back by itself when it's on again.
    Paused { message: Option<String>, off: bool, running: bool },
    /// The admin switched it back on while this tunnel kept running.
    Resumed,
    /// The connection dropped; trying again.
    Reconnecting,
    /// The session time ran out (the host can start it again).
    SessionEnded,
    /// Gave up (a ban, the name taken over elsewhere, the monthly cap…). A ban isn't retried: the relay would
    /// refuse every reconnect until it ends.
    Failed(anyhow::Error),
}

pub struct Opened {
    pub name: String,
    pub address: String,
    pub limits: Limits,
    pub usage: Usage,
    pub secs_left: Option<u64>,
    /// The relay is paused or off right now: (off, admin's message). The task waits for it to come back.
    pub paused: Option<(bool, Option<String>)>,
    /// A Java server with Geyser: where Bedrock players join through the relay ("name.domain:port"); None when the
    /// relay has no port for them or doesn't do this yet (then the router or playit.gg carries Geyser).
    pub geyser_address: Option<String>,
    /// Aborting it closes the tunnel.
    pub task: JoinHandle<()>,
}

/// While paused or off, how often to look whether the relay is back.
const PAUSED_RETRY_SECS: u64 = 60;

/// Continue requests for the running tunnels, by server id.
static COMMANDS: Mutex<Option<HashMap<String, mpsc::UnboundedSender<Command>>>> = Mutex::new(None);
/// Top-ups waiting for the relay's answer, by server id.
static TOPUPS: Mutex<Option<HashMap<String, oneshot::Sender<Result<u64, ErrorCode>>>>> = Mutex::new(None);

/// How early the host may continue when the relay doesn't say (relays before protocol 4).
pub const CONTINUE_SECS: u64 = 15 * 60;

fn paused_of(e: &AgentError) -> Option<(bool, Option<String>)> {
    match e {
        AgentError::Refused(ErrorCode::Paused, msg) => Some((false, Some(msg.clone()).filter(|m| !m.is_empty()))),
        AgentError::Refused(ErrorCode::ServiceOff, msg) => Some((true, Some(msg.clone()).filter(|m| !m.is_empty()))),
        _ => None,
    }
}

/// Sends a command to the server's running tunnel; false when it isn't online through the relay.
fn send(id: &str, cmd: Command) -> bool {
    COMMANDS.lock().unwrap().as_ref().and_then(|m| m.get(id)).is_some_and(|tx| tx.send(cmd).is_ok())
}

/// Tells the relay whether the server takes whitelist requests from its web page (after the tunnel opens, and
/// when the host changes it). Relays without the web page ignore it.
pub async fn send_join_requests(id: &str) {
    let on = store::get(id).await.is_ok_and(|m| m.join_requests.enabled && m.join_requests.web);
    send(id, Command::JoinRequests(on));
}

/// Tells the relay what the host decided on `username`'s server list request (their launcher lets them know).
pub fn send_join_decided(id: &str, username: &str, accepted: bool) {
    send(id, Command::JoinDecided { username: username.to_string(), accepted });
}

/// A request from the relay's web page or the server list: kept like the bot's, and the answer goes back.
fn on_join_request(id: String, request: u64, username: String, listing: bool) {
    use super::requests::{submit, Outcome, Via};
    tokio::spawn(async move {
        let outcome = match submit(&id, &username, Via::Web { relay_id: request, listing }).await {
            Outcome::Received => JoinOutcome::Received,
            Outcome::AlreadyListed => JoinOutcome::AlreadyListed,
            Outcome::AlreadyRequested => JoinOutcome::AlreadyRequested,
            Outcome::Closed | Outcome::Blocked => JoinOutcome::Closed,
            Outcome::Invalid => JoinOutcome::Invalid,
            Outcome::Banned => JoinOutcome::Banned,
        };
        send(&id, Command::JoinAnswer { id: request, outcome });
    });
}

/// Another full relay session for a server whose session is about to end (the Continue button, or `continue`
/// typed in its console).
pub fn continue_session(id: &str) -> Result<()> {
    let sent = COMMANDS.lock().unwrap().as_ref().and_then(|m| m.get(id)).is_some_and(|tx| tx.send(Command::Continue).is_ok());
    if sent {
        Ok(())
    } else {
        Err(err_tr!("hostRelay.notOnline", "This server isn't online through ResinCraft Relay right now."))
    }
}

/// More of this month's relay data for the server's account (the "Get 30 GB more" button): through its running
/// tunnel, or (after the data ran out and the tunnel couldn't start) on a connection of its own. Returns the
/// bytes added.
pub async fn top_up(id: &str) -> Result<u64> {
    let (tx, rx) = oneshot::channel();
    let sent = COMMANDS.lock().unwrap().as_ref().and_then(|m| m.get(id)).is_some_and(|c| {
        TOPUPS.lock().unwrap().get_or_insert_with(HashMap::new).insert(id.to_string(), tx);
        c.send(Command::TopUp).is_ok()
    });
    let refused = |code| describe(&AgentError::Refused(code, String::new()), "");
    if sent {
        return match tokio::time::timeout(Duration::from_secs(20), rx).await {
            Ok(Ok(Ok(added))) => Ok(added),
            Ok(Ok(Err(code))) => Err(refused(code)),
            _ => Err(describe(&AgentError::Network("the relay didn't answer".into()), "")),
        };
    }
    let s = settings().ok_or_else(not_configured)?;
    let meta = store::get(id).await?;
    let account = accounts::microsoft_account(meta.relay_account.as_deref()).await.ok_or_else(needs_microsoft)?;
    let mut conn = connect(&s, &account).await.map_err(|f| f.into_error(""))?;
    conn.top_up().await.map(|t| t.added_bytes).map_err(|e| describe(&e, ""))
}

/// The session end the reminder watches (Unix seconds; None: no limit), and whether it already reminded.
#[derive(Default)]
struct Reminder {
    ends: Mutex<Option<u64>>,
    sent: AtomicBool,
}

impl Reminder {
    fn set(&self, secs_left: Option<u64>) {
        *self.ends.lock().unwrap() = secs_left.map(|s| proto::now_secs() + s);
        self.sent.store(false, Ordering::Relaxed);
    }

    /// Seconds left, once, when the end is within `window`.
    fn due(&self, window: u64) -> Option<u64> {
        let left = self.ends.lock().unwrap().map(|e| e.saturating_sub(proto::now_secs()))?;
        (left > 0 && left <= window && !self.sent.swap(true, Ordering::Relaxed)).then_some(left)
    }
}

/// Brings the name online on `conn`: a Bedrock server gets a UDP port of its own on the relay, and a Java server
/// with Geyser (`geyser`: Geyser's local port) asks for one for its Bedrock players.
async fn go_online(conn: &mut Connection, name: &str, bedrock: bool, geyser: Option<u16>) -> Result<resincraft_relay::agent::Online, AgentError> {
    if bedrock {
        return conn.online_bedrock(name).await;
    }
    let Some(port) = geyser else { return conn.online(name).await };
    let online = conn.online_with_geyser(name).await?;
    if online.bedrock_port.is_some() {
        conn.set_udp_target(SocketAddr::from(([127, 0, 0, 1], port)));
    }
    Ok(online)
}

/// Where Bedrock players join a Java server's Geyser through the relay.
fn geyser_address(bedrock: bool, online: &resincraft_relay::agent::Online) -> Option<String> {
    online.bedrock_port.filter(|_| !bedrock).map(|port| format!("{}:{port}", online.address))
}

/// How early the host may continue: the relay's own window, or 15 minutes for relays that don't say.
pub fn continue_window(limits: &Limits) -> u64 {
    if limits.continue_secs > 0 {
        limits.continue_secs
    } else {
        CONTINUE_SECS
    }
}

/// The address players type: "name.rscraft.resincore.tech", with the port for a Bedrock server.
fn shown_address(address: &str, bedrock_port: Option<u16>) -> String {
    match bedrock_port {
        Some(port) => format!("{address}:{port}"),
        None => address.to_string(),
    }
}

/// Brings the server's name online (choosing one first if it has none) and keeps it online, reconnecting
/// when the connection drops, until the task is aborted. With `wait_if_paused` (the server is set to ResinCraft
/// Relay by hand), a paused or switched-off relay isn't an error: the task waits and comes back by itself.
/// `bedrock`: a Bedrock server (UDP), reached on its own port of the relay. `geyser`: a Java server's Geyser port,
/// so Bedrock players get a port on the relay too.
pub async fn open(
    id: &str,
    local_port: u16,
    wait_if_paused: bool,
    bedrock: bool,
    geyser: Option<u16>,
    on_status: impl Fn(Status) + Send + Sync + 'static,
) -> Result<Opened> {
    let s = settings().ok_or_else(not_configured)?;
    let meta = store::get(id).await?;
    let account = accounts::microsoft_account(meta.relay_account.as_deref()).await.ok_or_else(needs_microsoft)?;
    let wanted = meta.relay_name.clone().unwrap_or_default();
    let mut conn = connect(&s, &account).await.map_err(|f| f.into_error(&wanted))?;
    // A name reserved with an account that isn't signed in any more can't come online with this one.
    let other_account = meta.relay_account.as_deref().is_some_and(|a| a != account.uuid);
    let name = match meta.relay_name.clone().filter(|_| !other_account) {
        Some(n) => n,
        None => pick_name(&mut conn, &meta.name).await.map_err(|f| f.into_error(&wanted))?,
    };
    let address = proto::address(&name, &conn.base_domain);
    let (online, paused) = match go_online(&mut conn, &name, bedrock, geyser).await {
        Ok(on) => (Some(on), None),
        Err(e) => match paused_of(&e).filter(|_| wait_if_paused) {
            Some(p) => {
                // Keep the name while waiting (reserving still works while the relay is paused).
                let _ = conn.reserve(&name, None).await;
                (None, Some(p))
            }
            None => return Err(describe(&e, &address)),
        },
    };
    if meta.relay_name.as_deref() != Some(name.as_str()) || meta.relay_account.as_deref() != Some(account.uuid.as_str()) {
        let mut m = store::get(id).await?;
        m.relay_name = Some(name.clone());
        m.relay_account = Some(account.uuid.clone());
        store::save(&m).await?;
    }
    let (tx, mut commands) = mpsc::unbounded_channel();
    COMMANDS.lock().unwrap().get_or_insert_with(HashMap::new).insert(id.to_string(), tx);
    let on_status = Arc::new(on_status);
    let local = SocketAddr::from(([127, 0, 0, 1], local_port));
    let opened_online = online.clone();
    let task_name = name.clone();
    let task_address = address.clone();
    let task_id = id.to_string();
    let reminder = Arc::new(Reminder::default());
    let mut window = online.as_ref().map_or(CONTINUE_SECS, |o| continue_window(&o.limits));
    reminder.set(online.as_ref().and_then(|o| o.secs_left));
    let task = tokio::spawn(async move {
        let address = task_address;
        let mut current = online.is_some().then_some(conn);
        let mut wait = PAUSED_RETRY_SECS;
        loop {
            if let Some(conn) = current.take() {
                let (status, remind, id) = (on_status.clone(), reminder.clone(), task_id.clone());
                let serving = conn.serve_with(
                    local,
                    move |e| match e {
                        Event::Usage(u) => {
                            if u.unlimited {
                                remind.set(None);
                            }
                            status(Status::Usage(u))
                        }
                        Event::Warning { secs_left } => status(Status::Warning { secs_left }),
                        Event::Extended { secs_left, continues } => {
                            remind.set(Some(secs_left));
                            status(Status::Extended { secs_left, continues })
                        }
                        Event::Service { state: ServiceState::On, .. } => status(Status::Resumed),
                        Event::Service { state, message } => status(Status::Paused { message, off: state == ServiceState::Off, running: true }),
                        Event::TopUp { added_bytes, by_admin, refused, usage } => {
                            if !by_admin {
                                let waiting = TOPUPS.lock().unwrap().as_mut().and_then(|m| m.remove(&id));
                                if let Some(tx) = waiting {
                                    let _ = tx.send(refused.map_or(Ok(added_bytes), Err));
                                }
                            }
                            if refused.is_none() {
                                status(Status::TopUp { added_bytes, by_admin, usage })
                            }
                        }
                        Event::JoinRequest { id: request, username, listing } => on_join_request(id.clone(), request, username, listing),
                    },
                    &mut commands,
                );
                // Queued before serving starts, so the relay hears it as soon as the tunnel is up.
                send_join_requests(&task_id).await;
                tokio::pin!(serving);
                let mut tick = tokio::time::interval(Duration::from_secs(5));
                let ended = loop {
                    tokio::select! {
                        ended = &mut serving => break ended,
                        _ = tick.tick() => {
                            if let Some(secs_left) = reminder.due(window) {
                                on_status(Status::ContinueSoon { secs_left });
                            }
                        }
                    }
                };
                if ended.code() == Some(ErrorCode::SessionEnded) {
                    on_status(Status::SessionEnded);
                    return;
                }
                if let Some((off, message)) = paused_of(&ended) {
                    on_status(Status::Paused { message, off, running: false });
                    wait = PAUSED_RETRY_SECS;
                } else if !ended.retryable() {
                    on_status(Status::Failed(describe(&ended, &address)));
                    return;
                } else {
                    eprintln!("[relay] {address}: {ended}; reconnecting");
                    on_status(Status::Reconnecting);
                    // A restart says when to come back (a few seconds); otherwise back off up to a minute.
                    wait = if ended.code() == Some(ErrorCode::ShuttingDown) { 3 } else { 1 };
                }
            }
            loop {
                tokio::time::sleep(Duration::from_secs(wait)).await;
                let attempt = async {
                    let mut c = connect(&s, &account).await?;
                    let on = go_online(&mut c, &task_name, bedrock, geyser).await.map_err(Failure::Relay)?;
                    Ok::<_, Failure>((c, on))
                };
                match attempt.await {
                    Ok((c, on)) => {
                        window = continue_window(&on.limits);
                        reminder.set(on.secs_left);
                        on_status(Status::Online {
                            address: shown_address(&on.address, on.bedrock_port.filter(|_| bedrock)),
                            geyser_address: geyser_address(bedrock, &on),
                            limits: on.limits,
                            usage: on.usage,
                            secs_left: on.secs_left,
                        });
                        current = Some(c);
                        break;
                    }
                    Err(Failure::Relay(e)) if paused_of(&e).is_some() => {
                        let (off, message) = paused_of(&e).unwrap_or_default();
                        on_status(Status::Paused { message, off, running: false });
                        wait = PAUSED_RETRY_SECS;
                    }
                    Err(f) if f.retryable() => wait = (wait * 2).min(60),
                    Err(f) => {
                        on_status(Status::Failed(f.into_error(&address)));
                        return;
                    }
                }
            }
        }
    });
    let address = shown_address(&address, opened_online.as_ref().and_then(|o| o.bedrock_port).filter(|_| bedrock));
    let geyser_address = opened_online.as_ref().and_then(|o| geyser_address(bedrock, o));
    let (limits, usage, secs_left) = opened_online.map(|o| (o.limits, o.usage, o.secs_left)).unwrap_or_default();
    Ok(Opened { name, address, limits, usage, secs_left, paused, geyser_address, task })
}

// ---------- The name picker ----------

// ---------- Friends' invites ----------

/// What a friends' invite is told about its relay way in.
pub enum FriendRelay {
    /// Connected (again).
    Online,
    /// The relay's session ends in `secs_left` seconds unless the host continues.
    Ending { secs_left: u64 },
    /// The relay way in is gone for this invite; direct connections go on.
    Down(Text),
}

/// A tunnel for an invite (see friends/). Stays until [`close_for_friends`].
pub struct FriendTunnel {
    pub name: String,
    /// Reserved just for this invite, and given back when it ends.
    temporary: bool,
    account: MicrosoftAccount,
    commands: mpsc::UnboundedSender<Command>,
    task: JoinHandle<()>,
}

impl FriendTunnel {
    pub fn continue_session(&self) {
        let _ = self.commands.send(Command::Continue);
    }
}

/// A name for an invite: a new one that's given back afterwards, else one of the account's that nothing uses right now.
async fn friend_name(conn: &mut Connection) -> Result<(String, bool), AgentError> {
    let name = format!("friend-{}", &uuid::Uuid::new_v4().simple().to_string()[..10]);
    match conn.reserve(&name, None).await {
        Ok(name) => Ok((name, true)),
        Err(e) if e.code() == Some(ErrorCode::TooManyNames) => {
            let mine = conn.names_mine().await?;
            let hosting = store::list().await.unwrap_or_default();
            let running = |n: &str| hosting.iter().any(|m| m.relay_name.as_deref() == Some(n) && super::runtime::is_running(&m.id));
            mine.names.into_iter().find(|n| !n.online && !running(&n.name)).map(|n| (n.name, false)).ok_or(e)
        }
        Err(e) => Err(e),
    }
}

/// Puts this computer's `local_port` online under a name for an invite. Needs a signed-in Microsoft account; friends
/// need none for the relay itself. Players' connections reach `local_port` with their Minecraft handshake first.
pub async fn open_for_friends(local_port: u16, on_status: impl Fn(FriendRelay) + Send + Sync + 'static) -> Result<FriendTunnel> {
    let s = settings().ok_or_else(not_configured)?;
    let account = accounts::microsoft_account(None).await.ok_or_else(needs_microsoft)?;
    let mut conn = connect(&s, &account).await.map_err(|f| f.into_error(""))?;
    let (name, temporary) = friend_name(&mut conn).await.map_err(|e| describe(&e, ""))?;
    conn.online(&name).await.map_err(|e| describe(&e, &name))?;
    let (commands, mut rx) = mpsc::unbounded_channel();
    let local = SocketAddr::from(([127, 0, 0, 1], local_port));
    let (task_name, task_account) = (name.clone(), account.clone());
    let task = tokio::spawn(async move {
        let mut current = Some(conn);
        let mut wait = 1;
        loop {
            if let Some(conn) = current.take() {
                let status = &on_status;
                let ended = conn
                    .serve_with(
                        local,
                        move |e| {
                            if let Event::Warning { secs_left } = e {
                                status(FriendRelay::Ending { secs_left });
                            }
                        },
                        &mut rx,
                    )
                    .await;
                if !ended.retryable() {
                    on_status(FriendRelay::Down(as_text(&describe(&ended, &task_name))));
                    return;
                }
                wait = 1;
            }
            tokio::time::sleep(Duration::from_secs(wait)).await;
            let attempt = async {
                let mut c = connect(&s, &task_account).await?;
                c.online(&task_name).await.map_err(Failure::Relay)?;
                Ok::<_, Failure>(c)
            };
            match attempt.await {
                Ok(c) => {
                    on_status(FriendRelay::Online);
                    current = Some(c);
                }
                Err(f) if f.retryable() => wait = (wait * 2).min(60),
                Err(f) => {
                    on_status(FriendRelay::Down(as_text(&f.into_error(&task_name))));
                    return;
                }
            }
        }
    });
    Ok(FriendTunnel { name, temporary, account, commands, task })
}

/// Takes an invite's tunnel offline and gives its temporary name back.
pub async fn close_for_friends(tunnel: FriendTunnel) {
    tunnel.task.abort();
    let _ = tunnel.task.await;
    if !tunnel.temporary {
        return;
    }
    let Some(s) = settings() else { return };
    let release = async {
        let mut conn = connect(&s, &tunnel.account).await.ok()?;
        conn.release(&tunnel.name).await.ok()
    };
    if tokio::time::timeout(Duration::from_secs(8), release).await.ok().flatten().is_none() {
        eprintln!("[relay] couldn't give back {}", tunnel.name);
    }
}

fn normalize(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

pub(crate) fn as_text(e: &anyhow::Error) -> Text {
    text_of(e).cloned().unwrap_or_else(|| Text::plain(e.to_string()))
}

/// Everything the Settings tab needs before asking the relay anything.
pub async fn info(id: &str) -> Result<RelayInfo> {
    let meta = store::get(id).await?;
    let Some(s) = settings() else {
        return Ok(RelayInfo { available: false, domain: String::new(), account: None, name: meta.relay_name, suggested: proto::suggest_name(&meta.name) });
    };
    let account = accounts::microsoft_account(meta.relay_account.as_deref()).await;
    Ok(RelayInfo { available: true, domain: s.domain, account: account.map(|a| a.username), name: meta.relay_name, suggested: proto::suggest_name(&meta.name) })
}

/// Whether `name` is free (or already this account's).
pub async fn check(id: &str, name: &str) -> Result<RelayNameCheck> {
    let name = normalize(name);
    let s = settings().ok_or_else(not_configured)?;
    let result = |available, mine, reason: Option<anyhow::Error>| RelayNameCheck { name: name.clone(), available, mine, reason: reason.as_ref().map(as_text) };
    if let Err(code) = proto::check_name(&name, &[]) {
        return Ok(result(false, false, Some(describe(&AgentError::Refused(code, String::new()), &name))));
    }
    let meta = store::get(id).await?;
    let account = accounts::microsoft_account(meta.relay_account.as_deref()).await.ok_or_else(needs_microsoft)?;
    let mut conn = connect(&s, &account).await.map_err(|f| f.into_error(&name))?;
    let st = conn.check(&name).await.map_err(|e| describe(&e, &name))?;
    let reason = st.reason.map(|code| describe(&AgentError::Refused(code, String::new()), &name));
    Ok(result(st.available, st.mine, reason))
}

/// Reserves `name` for the server (freeing its previous one) and saves it.
pub async fn set_name(id: &str, name: &str) -> Result<ServerMeta> {
    let name = normalize(name);
    let s = settings().ok_or_else(not_configured)?;
    if let Err(code) = proto::check_name(&name, &[]) {
        return Err(describe(&AgentError::Refused(code, String::new()), &name));
    }
    let meta = store::get(id).await?;
    let account = accounts::microsoft_account(meta.relay_account.as_deref()).await.ok_or_else(needs_microsoft)?;
    let mut conn = connect(&s, &account).await.map_err(|f| f.into_error(&name))?;
    // Only free the old name when it belongs to this account and no other server uses it.
    let mut release = meta.relay_name.clone().filter(|old| *old != name && meta.relay_account.as_deref() == Some(account.uuid.as_str()));
    if let Some(old) = &release {
        let shared = store::list().await?.iter().any(|m| m.id != meta.id && m.relay_name.as_deref() == Some(old.as_str()));
        if shared {
            release = None;
        }
    }
    conn.reserve(&name, release.as_deref()).await.map_err(|e| describe(&e, &name))?;
    let mut m = store::get(id).await?;
    m.relay_name = Some(name);
    m.relay_account = Some(account.uuid);
    store::save(&m).await?;
    Ok(m)
}

pub(crate) fn same_account(a: &str, b: &str) -> bool {
    a.replace('-', "").eq_ignore_ascii_case(&b.replace('-', ""))
}

/// This PC's servers that use the account's `name`.
fn servers_with<'a>(metas: &'a [ServerMeta], account: &str, name: &str) -> impl Iterator<Item = &'a ServerMeta> {
    let (account, name) = (account.to_string(), name.to_string());
    metas.iter().filter(move |m| m.relay_name.as_deref() == Some(name.as_str()) && m.relay_account.as_deref().is_none_or(|a| same_account(a, &account)))
}

/// The names reserved with every signed-in Microsoft account, with the servers here that use them. A relay from before
/// protocol 10 can't list them, and an unreachable one can't answer: then the names this PC's servers use are shown.
pub async fn names() -> Result<RelayNames> {
    let Some(s) = settings() else { return Ok(RelayNames { available: false, names: Vec::new(), accounts: Vec::new(), error: None }) };
    let metas = store::list().await?;
    let mut names = Vec::new();
    let mut accounts_out = Vec::new();
    let mut error = None;
    for account in accounts::microsoft_accounts().await {
        let listed = match connect(&s, &account).await {
            Ok(mut conn) if conn.has_names() => conn.names_mine().await.map(Some).map_err(|e| describe(&e, "")),
            Ok(_) => Ok(None),
            Err(f) => Err(f.into_error("")),
        };
        let listed = listed.unwrap_or_else(|e| {
            error.get_or_insert_with(|| as_text(&e));
            None
        });
        let entry = |name: String, address: String, online, assigned| {
            let servers = servers_with(&metas, &account.uuid, &name).map(|m| RelayNameServer { id: m.id.clone(), name: m.name.clone() }).collect();
            RelayNameEntry { name, address, account: account.uuid.clone(), account_name: account.username.clone(), online, assigned, servers }
        };
        match listed {
            Some(mine) => {
                let used = mine.names.iter().filter(|n| !n.assigned).count() as u32;
                accounts_out.push(RelayNameAccount {
                    account: account.uuid.clone(),
                    account_name: account.username.clone(),
                    used,
                    limit: mine.limit,
                    requests_on: mine.requests_on,
                    request: mine.request.map(|r| RelayNameRequest {
                        wanted: r.wanted,
                        state: match r.state {
                            proto::NameRequestState::Pending => "pending",
                            proto::NameRequestState::Approved => "approved",
                            proto::NameRequestState::Rejected => "rejected",
                        },
                        note: r.note,
                    }),
                });
                names.extend(mine.names.into_iter().map(|n| entry(n.name, n.address, n.online, n.assigned)));
            }
            None => {
                let mut local: Vec<String> = metas.iter().filter(|m| m.relay_account.as_deref().is_some_and(|a| same_account(a, &account.uuid))).filter_map(|m| m.relay_name.clone()).collect();
                local.sort();
                local.dedup();
                names.extend(local.into_iter().map(|n| {
                    let address = proto::address(&n, &s.domain);
                    entry(n, address, false, false)
                }));
            }
        }
    }
    Ok(RelayNames { available: true, names, accounts: accounts_out, error })
}

/// The signed-in account a name belongs to, once none of this PC's servers is running under it.
async fn name_owner(account: &str, name: &str, metas: &[ServerMeta]) -> Result<MicrosoftAccount> {
    if let Some(m) = servers_with(metas, account, name).find(|m| super::runtime::is_running(&m.id)) {
        return Err(err_tr!("hostRelay.stopFirst", "Stop {server} before you change its ResinCraft address.", server = m.name));
    }
    accounts::microsoft_accounts().await.into_iter().find(|a| same_account(&a.uuid, account)).ok_or_else(needs_microsoft)
}

/// Asks the relay's admin for more names for the account.
pub async fn request_names(account: &str, wanted: u32, reason: &str) -> Result<()> {
    let s = settings().ok_or_else(not_configured)?;
    let owner = accounts::microsoft_accounts().await.into_iter().find(|a| same_account(&a.uuid, account)).ok_or_else(needs_microsoft)?;
    let mut conn = connect(&s, &owner).await.map_err(|f| f.into_error(""))?;
    conn.name_request(wanted, reason).await.map(|_| ()).map_err(|e| describe(&e, ""))
}

/// Frees a reserved name; this PC's servers that used it get a new one the next time they need one.
pub async fn revoke_name(account: &str, name: &str) -> Result<()> {
    let s = settings().ok_or_else(not_configured)?;
    let metas = store::list().await?;
    let owner = name_owner(account, name, &metas).await?;
    let mut conn = connect(&s, &owner).await.map_err(|f| f.into_error(name))?;
    conn.release(name).await.map_err(|e| describe(&e, name))?;
    for m in servers_with(&metas, account, name) {
        let mut m = m.clone();
        m.relay_name = None;
        store::save(&m).await?;
    }
    Ok(())
}

/// After a server was deleted: frees its name on the relay too, unless another server here still uses it, the name is
/// online right now (on another PC), or the relay's admin gave it (it would be lost for good).
pub async fn release_after_delete(meta: ServerMeta) {
    let Some(name) = meta.relay_name.clone() else { return };
    let Some(s) = settings() else { return };
    let Ok(metas) = store::list().await else { return };
    let account = meta.relay_account.clone();
    let same = |m: &ServerMeta| match (&account, &m.relay_account) {
        (Some(a), Some(b)) => same_account(a, b),
        _ => true,
    };
    if metas.iter().any(|m| m.id != meta.id && m.relay_name.as_deref() == Some(name.as_str()) && same(m)) {
        return;
    }
    let all = accounts::microsoft_accounts().await;
    let owner = match &account {
        Some(a) => all.into_iter().find(|x| same_account(&x.uuid, a)),
        None => all.into_iter().next(),
    };
    let Some(owner) = owner else { return };
    let Ok(mut conn) = connect(&s, &owner).await else { return };
    if conn.has_names() {
        match conn.names_mine().await {
            Ok(mine) => match mine.names.iter().find(|n| n.name == name) {
                Some(n) if !n.online && !n.assigned => {}
                _ => return,
            },
            Err(_) => return,
        }
    }
    if conn.release(&name).await.is_ok() {
        crate::core::events::emit("listChanged", "servers");
    }
}

/// Changes a reserved name, for this PC's servers that use it too.
pub async fn rename_name(account: &str, name: &str, new: &str) -> Result<()> {
    let new = normalize(new);
    let s = settings().ok_or_else(not_configured)?;
    if let Err(code) = proto::check_name(&new, &[]) {
        return Err(describe(&AgentError::Refused(code, String::new()), &new));
    }
    let metas = store::list().await?;
    let owner = name_owner(account, name, &metas).await?;
    let mut conn = connect(&s, &owner).await.map_err(|f| f.into_error(&new))?;
    conn.reserve(&new, Some(name)).await.map_err(|e| describe(&e, &new))?;
    for m in servers_with(&metas, account, name) {
        let mut m = m.clone();
        m.relay_name = Some(new.clone());
        store::save(&m).await?;
    }
    Ok(())
}

/// The address friends type, e.g. for the card: `name.rscraft.resincore.tech`.
pub fn address_of(meta: &ServerMeta) -> Option<String> {
    let s = settings()?;
    meta.relay_name.as_deref().map(|n| proto::address(n, &s.domain))
}


// ---------- The player's own domain (relay protocol 7) ----------

/// Registers the server's domain with ResinCraft Relay (or looks again, which `domain_add` also does) and says whether
/// the relay has seen the TXT record that proves it's the player's. None when there's no domain or relay name.
pub async fn domain_ownership(id: &str) -> Result<Option<DomainOwnership>> {
    let meta = store::get(id).await?;
    let (Some(domain), Some(name)) = (meta.domain.clone(), meta.relay_name.clone()) else { return Ok(None) };
    let Some(s) = settings() else { return Ok(None) };
    // Without an answer: `error` says why (the relay may well verify domains); no error means it doesn't (protocol < 7).
    let unknown = |error: Option<Text>| DomainOwnership {
        supported: error.is_some(),
        state: "unknown",
        txt: meta.domain_txt.as_ref().map(txt_record),
        route_ok: false,
        https: false,
        elsewhere: false,
        checked_at: None,
        message: None,
        error,
    };
    let Some(account) = accounts::microsoft_account(meta.relay_account.as_deref()).await else {
        return Ok(Some(unknown(Some(as_text(&needs_microsoft())))));
    };
    let mut conn = match connect(&s, &account).await {
        Ok(c) => c,
        Err(f) => return Ok(Some(unknown(Some(as_text(&f.into_error(&name)))))),
    };
    if !conn.has_domains() {
        return Ok(Some(unknown(None)));
    }
    let st = match conn.domain_add(&name, &domain).await {
        Ok(st) => st,
        Err(e) => return Ok(Some(unknown(Some(as_text(&describe(&e, &name)))))),
    };
    let txt = DomainTxt { name: st.txt_name.clone(), value: st.txt_value.clone() };
    if meta.domain_txt.as_ref() != Some(&txt) {
        store::update(id, serde_json::json!({ "domainTxt": txt })).await?;
    }
    Ok(Some(DomainOwnership {
        supported: true,
        state: match st.state {
            DomainState::Pending => "pending",
            DomainState::Verified => "verified",
            DomainState::Misconfigured => "misconfigured",
            DomainState::Unknown => "unknown",
        },
        txt: Some(txt_record(&txt)),
        route_ok: st.route_ok,
        https: st.https,
        elsewhere: st.elsewhere,
        checked_at: st.checked_at,
        message: st.message,
        error: None,
    }))
}

/// The TXT record as the Settings tab and the Cloudflare updates use it.
pub fn txt_record(txt: &DomainTxt) -> DnsRecord {
    DnsRecord { kind: "TXT", name: txt.name.clone(), value: txt.value.clone(), port: None, priority: None, weight: None }
}

/// Tells the relay `name` no longer has a domain (the player cleared it). Best effort.
pub async fn domain_detach(id: String, name: String) {
    let Some(s) = settings() else { return };
    let Ok(meta) = store::get(&id).await else { return };
    let Some(account) = accounts::microsoft_account(meta.relay_account.as_deref()).await else { return };
    if let Ok(mut conn) = connect(&s, &account).await {
        let _ = conn.domain_remove(&name).await;
    }
    let _ = store::update(&id, serde_json::json!({ "domainTxt": null })).await;
}