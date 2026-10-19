//! The admin dashboard: one embedded page (no build step) and a small JSON API on its own port, over HTTPS
//! with the relay's certificate.
//!
//! Sign-in: the admin password's Argon2 hash lives in relay.toml (`set-admin-password` writes it, and it's
//! re-read at every sign-in, so changing it needs no restart). A sign-in gives an HttpOnly, Secure,
//! SameSite=Strict session cookie for `session_hours`. Every POST needs the session's CSRF token in an
//! `X-CSRF-Token` header (and a same-origin `Origin` when the browser sends one); the sign-in itself needs a
//! one-time token from `GET /api/session`. Failed sign-ins lock an address out for 15 minutes after 5 tries,
//! and everyone for 5 minutes after 30 tries from all addresses together. `allow_ips` can limit who may
//! connect at all. Microsoft/Minecraft tokens never reach the relay's storage, so nothing here can show them.

use std::collections::{BTreeMap, HashMap};
use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tracing::{info, warn};

use super::config::LimitsConfig;
use super::http::{read_request, write_response, Request, Response};
use super::{ip_matches, same_secret, Relay};
use crate::proto::{self, month_of, now_secs, ErrorCode, ServiceState};

const COOKIE: &str = "rc_session";
const LOGIN_TOKEN_SECS: u64 = 600;
const FAIL_WINDOW: u64 = 15 * 60;
const FAILS_PER_IP: usize = 5;
const IP_LOCK_SECS: u64 = 15 * 60;
const FAILS_GLOBAL: usize = 30;
const GLOBAL_LOCK_SECS: u64 = 5 * 60;
pub const MIN_PASSWORD_LEN: usize = 10;

const INDEX_HTML: &str = include_str!("dashboard/index.html");
const APP_JS: &str = include_str!("dashboard/app.js");
const APP_CSS: &str = include_str!("dashboard/app.css");
const FAVICON: &str = include_str!("dashboard/favicon.svg");

struct Session {
    csrf: String,
    expires: u64,
}

/// Sessions, one-time sign-in tokens and failed sign-ins (all in memory: a restart signs everyone out).
#[derive(Default)]
pub(crate) struct State {
    sessions: Mutex<HashMap<String, Session>>,
    login_tokens: Mutex<HashMap<String, u64>>,
    failures: Mutex<HashMap<IpAddr, Vec<u64>>>,
    global_failures: Mutex<Vec<u64>>,
}

impl State {
    pub(crate) fn prune(&self, now: u64) {
        self.sessions.lock().unwrap().retain(|_, s| s.expires > now);
        self.login_tokens.lock().unwrap().retain(|_, exp| *exp > now);
        self.failures.lock().unwrap().retain(|_, f| f.iter().any(|t| now.saturating_sub(*t) < IP_LOCK_SECS.max(FAIL_WINDOW)));
        self.global_failures.lock().unwrap().retain(|t| now.saturating_sub(*t) < FAIL_WINDOW.max(GLOBAL_LOCK_SECS));
    }

    /// Seconds until `ip` may try again (0: now).
    fn locked_for(&self, ip: IpAddr, now: u64) -> u64 {
        let ip_wait = self.failures.lock().unwrap().get(&ip).and_then(|f| {
            let recent: Vec<u64> = f.iter().copied().filter(|t| now.saturating_sub(*t) < FAIL_WINDOW).collect();
            (recent.len() >= FAILS_PER_IP).then(|| (recent.iter().max().unwrap() + IP_LOCK_SECS).saturating_sub(now))
        });
        let global = self.global_failures.lock().unwrap();
        let recent: Vec<u64> = global.iter().copied().filter(|t| now.saturating_sub(*t) < FAIL_WINDOW).collect();
        let global_wait = (recent.len() >= FAILS_GLOBAL).then(|| (recent.iter().max().unwrap() + GLOBAL_LOCK_SECS).saturating_sub(now));
        ip_wait.unwrap_or(0).max(global_wait.unwrap_or(0))
    }

    /// Records a failed sign-in; returns how many tries `ip` has left before the lock-out.
    fn fail(&self, ip: IpAddr, now: u64) -> usize {
        self.global_failures.lock().unwrap().push(now);
        let mut f = self.failures.lock().unwrap();
        let list = f.entry(ip).or_default();
        list.retain(|t| now.saturating_sub(*t) < FAIL_WINDOW);
        list.push(now);
        FAILS_PER_IP.saturating_sub(list.len())
    }
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).expect("the system's random number generator works");
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn digest(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

// ---------- Password ----------

/// An Argon2id hash (PHC string) of `password`.
pub fn hash_password(password: &str) -> Result<String> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(anyhow!("the admin password needs at least {MIN_PASSWORD_LEN} characters"));
    }
    let mut raw = [0u8; 16];
    getrandom::getrandom(&mut raw).map_err(|e| anyhow!("no randomness for the salt: {e}"))?;
    let salt = SaltString::encode_b64(&raw).map_err(|e| anyhow!("couldn't make a salt: {e}"))?;
    Ok(Argon2::default().hash_password(password.as_bytes(), &salt).map_err(|e| anyhow!("couldn't hash the password: {e}"))?.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| Argon2::default().verify_password(password.as_bytes(), &h).is_ok())
}

/// Writes the hash into the config's [dashboard] table, keeping everything else (comments too) as it was.
pub fn write_password_hash(config: &Path, hash: &str) -> Result<()> {
    let text = std::fs::read_to_string(config).with_context(|| format!("couldn't read {}", config.display()))?;
    let updated = with_password_hash(&text, hash);
    // Only write what still parses and says exactly this.
    let parsed: toml::Table = updated.parse().with_context(|| format!("{} isn't valid TOML", config.display()))?;
    if parsed.get("dashboard").and_then(|d| d.get("password_hash")).and_then(|h| h.as_str()) != Some(hash) {
        return Err(anyhow!("couldn't place password_hash in {}; add it under [dashboard] by hand", config.display()));
    }
    let tmp = config.with_extension("toml.tmp");
    std::fs::write(&tmp, updated).with_context(|| format!("couldn't write {}", tmp.display()))?;
    std::fs::rename(&tmp, config).with_context(|| format!("couldn't replace {}", config.display()))?;
    Ok(())
}

/// The config text with `password_hash` set in its [dashboard] table (replaced in place, added under the
/// table header, or a new table at the end); every other line stays as it was.
fn with_password_hash(text: &str, hash: &str) -> String {
    let line = format!("password_hash = \"{hash}\"");
    let is_header = |l: &str| l.trim_start().starts_with('[');
    let is_dashboard = |l: &str| l.split('#').next().unwrap_or_default().trim() == "[dashboard]";
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    if let Some(start) = lines.iter().position(|l| is_dashboard(l)) {
        let end = lines.iter().skip(start + 1).position(|l| is_header(l)).map_or(lines.len(), |i| start + 1 + i);
        match (start + 1..end).find(|&i| lines[i].split('=').next().is_some_and(|k| k.trim() == "password_hash")) {
            Some(i) => {
                let comment = lines[i].rfind(" #").filter(|&p| !lines[i][p..].contains('"')).map(|p| lines[i][p..].to_string()).unwrap_or_default();
                lines[i] = format!("{line}{comment}");
            }
            None => lines.insert(start + 1, line),
        }
    } else {
        if lines.last().is_some_and(|l| !l.trim().is_empty()) {
            lines.push(String::new());
        }
        lines.push("[dashboard]".into());
        lines.push(line);
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// The hash in force: re-read from the config file, so `set-admin-password` works without a restart.
fn current_hash(relay: &Relay) -> String {
    let from_file = relay.cfg.path.as_deref().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| t.parse::<toml::Table>().ok()).and_then(|t| {
        t.get("dashboard")?.get("password_hash")?.as_str().map(str::to_string)
    });
    from_file.unwrap_or_else(|| relay.cfg.dashboard.password_hash.clone())
}

// ---------- Serving ----------

pub(crate) async fn serve(relay: Arc<Relay>, listener: TcpListener, tls: Option<tokio_rustls::TlsAcceptor>) {
    loop {
        match listener.accept().await {
            Ok((sock, peer)) => {
                let (relay, tls) = (relay.clone(), tls.clone());
                tokio::spawn(async move {
                    let _ = sock.set_nodelay(true);
                    match tls {
                        Some(acceptor) => {
                            if let Ok(Ok(stream)) = tokio::time::timeout(Duration::from_secs(10), acceptor.accept(sock)).await {
                                handle(relay, stream, peer, true).await;
                            }
                        }
                        None => handle(relay, sock, peer, false).await,
                    }
                });
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
}

async fn handle<S: AsyncRead + AsyncWrite + Unpin>(relay: Arc<Relay>, mut sock: S, peer: SocketAddr, secure: bool) {
    let ip = peer.ip();
    let allowed = relay.cfg.dashboard.allow_ips.is_empty() || relay.cfg.dashboard.allow_ips.iter().any(|p| ip_matches(ip, p));
    let resp = match tokio::time::timeout(Duration::from_secs(10), read_request(&mut sock)).await {
        Ok(Ok(_)) if !allowed => Response::new("403 Forbidden", "text/plain; charset=utf-8", "This address may not open the ResinCraft Relay dashboard."),
        Ok(Ok(req)) => route(&relay, &req, ip, secure).await,
        _ => return,
    };
    let mut resp = resp
        .header(
            "Content-Security-Policy",
            "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
        )
        .header("X-Content-Type-Options", "nosniff")
        .header("X-Frame-Options", "DENY")
        .header("Referrer-Policy", "no-referrer")
        .header("Cross-Origin-Opener-Policy", "same-origin")
        .header("Cache-Control", "no-store");
    if secure {
        resp = resp.header("Strict-Transport-Security", "max-age=31536000");
    }
    let _ = write_response(&mut sock, &resp).await;
}

fn cookie(token: &str, max_age: u64) -> String {
    format!("{COOKIE}={token}; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age={max_age}")
}

/// A same-origin request: no Origin header (older browsers, curl), or one that names this host.
fn same_origin(req: &Request, secure: bool) -> bool {
    if req.header("sec-fetch-site").is_some_and(|s| s != "same-origin" && s != "none") {
        return false;
    }
    match (req.header("origin"), req.header("host")) {
        (None, _) => true,
        (Some(origin), Some(host)) => origin.eq_ignore_ascii_case(&format!("{}://{host}", if secure { "https" } else { "http" })),
        (Some(_), None) => false,
    }
}

/// Where the dashboard lives: https://<base_domain>/admin/ (with the dashboard on port 443).
pub const PREFIX: &str = "/admin";

/// The path inside the dashboard: "/admin/api/tunnels" → "/api/tunnels" (paths without the prefix work too).
fn inner_path(path: &str) -> &str {
    path.strip_prefix(PREFIX).filter(|rest| rest.starts_with('/')).unwrap_or(path)
}

async fn route(relay: &Relay, req: &Request, ip: IpAddr, secure: bool) -> Response {
    let get = req.method == "GET";
    if get && (req.path == "/" || req.path == PREFIX) {
        return Response::new("308 Permanent Redirect", "text/plain; charset=utf-8", "").header("Location", format!("{PREFIX}/"));
    }
    let path = inner_path(&req.path);
    match (req.method.as_str(), path) {
        ("GET", "/") | ("GET", "/index.html") => return Response::new("200 OK", "text/html; charset=utf-8", INDEX_HTML),
        ("GET", "/app.js") => return Response::new("200 OK", "text/javascript; charset=utf-8", APP_JS),
        ("GET", "/app.css") => return Response::new("200 OK", "text/css; charset=utf-8", APP_CSS),
        ("GET", "/favicon.svg") => return Response::new("200 OK", "image/svg+xml", FAVICON),
        ("GET", "/api/session") => return session_info(relay, req),
        ("POST", "/api/login") => {
            if !same_origin(req, secure) {
                return Response::error("403 Forbidden", "cross-site request refused");
            }
            return login(relay, req, ip).await;
        }
        (_, p) if !p.starts_with("/api/") => return Response::error("404 Not Found", "not found"),
        _ => {}
    }
    // Everything else needs a session, and changes need its CSRF token.
    let now = now_secs();
    let token_digest = req.cookie(COOKIE).map(digest);
    let csrf = {
        let sessions = relay.dashboard.sessions.lock().unwrap();
        match token_digest.as_ref().and_then(|d| sessions.get(d)).filter(|s| s.expires > now) {
            Some(s) => s.csrf.clone(),
            None => return Response::error("401 Unauthorized", "sign in first"),
        }
    };
    if !get && (!same_origin(req, secure) || !req.header("x-csrf-token").is_some_and(|t| same_secret(t, &csrf))) {
        return Response::error("403 Forbidden", "missing or wrong CSRF token: reload the page");
    }
    let input = req.json();
    match (req.method.as_str(), path) {
        ("POST", "/api/logout") => {
            if let Some(d) = token_digest {
                relay.dashboard.sessions.lock().unwrap().remove(&d);
            }
            Response::ok(json!({ "ok": true })).header("Set-Cookie", cookie("", 0))
        }
        ("GET", "/api/overview") => overview(relay),
        ("GET", "/api/tunnels") => tunnels(relay),
        ("GET", "/api/names") => names(relay, req.param("q").unwrap_or_default().trim().to_lowercase()),
        ("GET", "/api/bans") => bans(relay),
        ("GET", "/api/usage") => usage(relay),
        ("GET", "/api/settings") => settings(relay),
        ("GET", "/api/events") => Response::ok(json!(relay.events())),
        ("GET", "/api/service") => Response::ok(service_json(relay)),
        ("POST", path) => action(relay, path, &input, ip),
        _ => Response::error("404 Not Found", "not found"),
    }
}

fn session_info(relay: &Relay, req: &Request) -> Response {
    let now = now_secs();
    let csrf = req.cookie(COOKIE).map(digest).and_then(|d| relay.dashboard.sessions.lock().unwrap().get(&d).filter(|s| s.expires > now).map(|s| s.csrf.clone()));
    let base = json!({ "baseDomain": relay.cfg.base_domain, "version": env!("CARGO_PKG_VERSION"), "passwordSet": !current_hash(relay).is_empty() });
    let mut out = base.as_object().cloned().unwrap_or_default();
    match csrf {
        Some(csrf) => {
            out.insert("loggedIn".into(), json!(true));
            out.insert("csrf".into(), json!(csrf));
        }
        None => {
            let token = random_token();
            let mut tokens = relay.dashboard.login_tokens.lock().unwrap();
            if tokens.len() > 1000 {
                tokens.retain(|_, exp| *exp > now);
            }
            tokens.insert(token.clone(), now + LOGIN_TOKEN_SECS);
            out.insert("loggedIn".into(), json!(false));
            out.insert("loginToken".into(), json!(token));
        }
    }
    Response::ok(Value::Object(out))
}

async fn login(relay: &Relay, req: &Request, ip: IpAddr) -> Response {
    let now = now_secs();
    let wait = relay.dashboard.locked_for(ip, now);
    if wait > 0 {
        return Response::json("429 Too Many Requests", json!({ "error": "too many wrong passwords: try again later", "retryAfter": wait }))
            .header("Retry-After", wait.to_string());
    }
    let input = req.json();
    let token = input.get("token").and_then(Value::as_str).unwrap_or_default();
    let token_ok = relay.dashboard.login_tokens.lock().unwrap().remove(token).is_some_and(|exp| exp > now);
    if !token_ok {
        return Response::error("403 Forbidden", "the sign-in form expired: reload the page");
    }
    let hash = current_hash(relay);
    if hash.is_empty() {
        return Response::error("503 Service Unavailable", "no admin password is set: run resincraft-relay set-admin-password on the server");
    }
    let password = input.get("password").and_then(Value::as_str).unwrap_or_default().to_string();
    let ok = tokio::task::spawn_blocking(move || verify_password(&password, &hash)).await.unwrap_or(false);
    if !ok {
        let left = relay.dashboard.fail(ip, now);
        warn!("dashboard sign-in failed");
        relay.event("auth-fail", "Wrong dashboard password", Some(ip));
        return Response::json("401 Unauthorized", json!({ "error": "wrong password", "attemptsLeft": left }));
    }
    relay.dashboard.failures.lock().unwrap().remove(&ip);
    let (session, csrf) = (random_token(), random_token());
    let max_age = relay.cfg.dashboard.session_hours * 3600;
    relay.dashboard.sessions.lock().unwrap().insert(digest(&session), Session { csrf: csrf.clone(), expires: now + max_age });
    info!("admin signed in to the dashboard");
    relay.event("admin", "Admin signed in", Some(ip));
    Response::ok(json!({ "ok": true, "csrf": csrf })).header("Set-Cookie", cookie(&session, max_age))
}

// ---------- Reading ----------

/// Minecraft names for UUIDs, from reservations and sessions.
fn account_names(relay: &Relay) -> HashMap<String, String> {
    relay.store.read(|d| {
        let mut m: HashMap<String, String> = d.sessions.values().map(|s| (s.uuid.clone(), s.username.clone())).collect();
        for e in d.names.values() {
            m.insert(e.owner.clone(), e.owner_name.clone());
        }
        m
    })
}

fn overview(relay: &Relay) -> Response {
    let tunnels = relay.tunnel_list();
    let players: u32 = tunnels.iter().map(|(_, t)| t.conns.load(Ordering::Relaxed)).sum();
    let month = month_of(now_secs());
    let (names, bans, traffic) = relay.store.read(|d| {
        (d.names.len(), d.bans.uuids.len() + d.bans.ips.len(), d.usage.values().filter(|u| u.month == month).map(|u| u.bytes).sum::<u64>())
    });
    let m = &relay.metrics;
    Response::ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "baseDomain": relay.cfg.base_domain,
        "uptimeSecs": now_secs().saturating_sub(relay.started),
        "tunnels": tunnels.len(),
        "players": players,
        "names": names,
        "bans": bans + relay.cfg.bans.uuids.len() + relay.cfg.bans.ips.len(),
        "month": month,
        "trafficMonth": traffic,
        "capPerAccount": relay.limits_now().monthly_bytes(),
        "launchers": m.agent_connections_active.load(Ordering::Relaxed),
        "forwardedTotal": m.player_forwarded_total.load(Ordering::Relaxed),
        "offlineAnswered": m.offline_status_total.load(Ordering::Relaxed) + m.offline_login_total.load(Ordering::Relaxed),
        "signInFailures": m.login_failures_total.load(Ordering::Relaxed),
        "service": service_json(relay),
        "sessionMinutes": relay.limits_now().session_minutes,
    }))
}

fn service_json(relay: &Relay) -> Value {
    let (state, message) = relay.service();
    let since = relay.store.read(|d| d.service.since);
    json!({ "state": state, "message": message, "since": since })
}

fn tunnels(relay: &Relay) -> Response {
    let list: Vec<Value> = relay
        .tunnel_list()
        .into_iter()
        .map(|(name, t)| {
            json!({
                "name": name,
                "address": proto::address(&name, &relay.cfg.base_domain),
                "owner": t.owner,
                "ownerName": t.owner_name,
                "since": t.since,
                "players": t.conns.load(Ordering::Relaxed),
                "bytesIn": t.bytes_in.load(Ordering::Relaxed),
                "bytesOut": t.bytes_out.load(Ordering::Relaxed),
                "secsLeft": t.clock.secs_left(now_secs()).filter(|_| relay.limits_now().session_secs() > 0),
                "continues": t.clock.continues(),
                "bedrockPort": t.bedrock_port,
            })
        })
        .collect();
    Response::ok(json!(list))
}

fn names(relay: &Relay, q: String) -> Response {
    let online: Vec<String> = relay.tunnel_list().into_iter().map(|(n, _)| n).collect();
    let (total, list) = relay.store.read(|d| {
        let list: Vec<Value> = d
            .names
            .iter()
            .filter(|(n, e)| q.is_empty() || n.contains(&q) || e.owner_name.to_lowercase().contains(&q) || e.owner.contains(&q))
            .take(500)
            .map(|(n, e)| json!({ "name": n, "owner": e.owner, "ownerName": e.owner_name, "created": e.created, "lastUsed": e.last_used, "online": online.contains(n) }))
            .collect();
        (d.names.len(), list)
    });
    Response::ok(json!({ "total": total, "names": list, "reserved": relay.reserved_names(), "builtinReserved": proto::RESERVED_NAMES }))
}

fn bans(relay: &Relay) -> Response {
    let who = account_names(relay);
    let now = now_secs();
    let (uuids, ips) = relay.store.read(|d| {
        let uuids: Vec<Value> = d
            .bans
            .uuids
            .iter()
            .filter(|(_, b)| b.active(now))
            .map(|(u, b)| json!({ "uuid": u, "name": who.get(u), "reason": b.reason, "at": b.at, "expires": b.expires }))
            .collect();
        let ips: Vec<Value> = d.bans.ips.iter().filter(|(_, b)| b.active(now)).map(|(ip, b)| json!({ "ip": ip, "reason": b.reason, "at": b.at, "expires": b.expires })).collect();
        (uuids, ips)
    });
    Response::ok(json!({ "uuids": uuids, "ips": ips, "fromConfig": { "uuids": relay.cfg.bans.uuids, "ips": relay.cfg.bans.ips } }))
}

fn usage(relay: &Relay) -> Response {
    let who = account_names(relay);
    let month = month_of(now_secs());
    let cap = relay.limits_now().monthly_bytes();
    let online: HashMap<String, usize> = relay.tunnel_list().into_iter().fold(HashMap::new(), |mut m, (_, t)| {
        *m.entry(t.owner).or_default() += 1;
        m
    });
    let mut accounts: Vec<(String, u64)> = relay.store.read(|d| d.usage.iter().filter(|(_, u)| u.month == month).map(|(k, u)| (k.clone(), u.bytes)).collect());
    accounts.sort_by_key(|a| std::cmp::Reverse(a.1));
    let total: u64 = accounts.iter().map(|(_, b)| b).sum();
    let list: Vec<Value> = accounts
        .iter()
        .map(|(uuid, bytes)| {
            json!({
                "uuid": uuid,
                "name": who.get(uuid),
                "bytes": bytes,
                "percent": if cap > 0 { (*bytes as f64 / cap as f64 * 100.0).min(100.0) } else { 0.0 },
                "online": online.get(uuid).copied().unwrap_or(0),
            })
        })
        .collect();
    Response::ok(json!({ "month": month, "cap": cap, "total": total, "accounts": list }))
}

fn settings(relay: &Relay) -> Response {
    let (limits_overridden, reserved_overridden) = relay.store.read(|d| (d.overrides.limits.is_some(), d.overrides.reserved_names.is_some()));
    Response::ok(json!({
        "limits": relay.limits_now(),
        "fileLimits": relay.cfg.limits,
        "limitsOverridden": limits_overridden,
        "reserved": relay.reserved_names(),
        "fileReserved": relay.cfg.auth.reserved_names,
        "reservedOverridden": reserved_overridden,
        "restartOnly": ["max_pending_handshakes"],
    }))
}

// ---------- Changing ----------

fn text(input: &Value, key: &str) -> Option<String> {
    input.get(key).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

/// Writes an admin action to the log (with a timestamp) and the events log.
fn audit(relay: &Relay, kind: &'static str, what: String, ip: IpAddr) {
    info!(action = kind, "admin: {what}");
    relay.event(kind, format!("Admin: {what}"), Some(ip));
}

fn action(relay: &Relay, path: &str, input: &Value, ip: IpAddr) -> Response {
    match path {
        "/api/service" => {
            let state: ServiceState = match input.get("state").cloned().map(serde_json::from_value) {
                Some(Ok(s)) => s,
                _ => return Response::error("400 Bad Request", "state must be on, paused or off"),
            };
            let message = text(input, "message");
            let tunnels = relay.tunnels_online();
            relay.set_service(state, message.clone());
            let what = match state {
                ServiceState::On => "switched the relay on".to_string(),
                ServiceState::Paused => format!("paused the relay (no new tunnels; {tunnels} running go on until their session ends)"),
                ServiceState::Off => format!("switched the relay off ({tunnels} tunnels closed)"),
            };
            let note = message.map(|m| format!(": \"{m}\"")).unwrap_or_default();
            audit(relay, "service", format!("{what}{note}"), ip);
            Response::ok(service_json(relay))
        }
        "/api/tunnels/extend" => {
            let Some(name) = text(input, "name") else { return Response::error("400 Bad Request", "which name?") };
            match relay.admin_extend(&name) {
                Some(left) => {
                    audit(relay, "session", format!("extended {name}'s session ({} min left)", left / 60), ip);
                    Response::ok(json!({ "ok": true, "secsLeft": left }))
                }
                None => Response::error("404 Not Found", "that name isn't online, or sessions have no time limit"),
            }
        }
        "/api/tunnels/end" => {
            let Some(name) = text(input, "name") else { return Response::error("400 Bad Request", "which name?") };
            if !relay.admin_end(&name) {
                return Response::error("404 Not Found", "that name isn't online");
            }
            audit(relay, "session", format!("ended {name}'s session"), ip);
            Response::ok(json!({ "ok": true }))
        }
        "/api/tunnels/kick" => {
            let Some(name) = text(input, "name") else { return Response::error("400 Bad Request", "which name?") };
            if !relay.kick_name(&name, ErrorCode::Kicked) {
                return Response::error("404 Not Found", "that name isn't online");
            }
            audit(relay, "kick", format!("disconnected {name}"), ip);
            Response::ok(json!({ "ok": true }))
        }
        "/api/names/release" => {
            let Some(name) = text(input, "name") else { return Response::error("400 Bad Request", "which name?") };
            if !relay.admin_release(&name) {
                return Response::error("404 Not Found", "no such name");
            }
            audit(relay, "name", format!("released {name}"), ip);
            Response::ok(json!({ "ok": true }))
        }
        "/api/names/rename" => {
            let (Some(from), Some(to)) = (text(input, "name"), text(input, "to").map(|t| t.to_lowercase())) else {
                return Response::error("400 Bad Request", "send name and to");
            };
            match relay.admin_rename(&from, &to) {
                Ok(()) => {
                    audit(relay, "name", format!("renamed {from} to {to}"), ip);
                    Response::ok(json!({ "ok": true }))
                }
                Err(e) => Response::error("400 Bad Request", e),
            }
        }
        "/api/reserved/add" | "/api/reserved/remove" => {
            let Some(word) = text(input, "word").map(|w| w.to_lowercase()) else { return Response::error("400 Bad Request", "which word?") };
            if !word.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') || word.len() > proto::NAME_MAX {
                return Response::error("400 Bad Request", "reserved words use lowercase letters, numbers and dashes");
            }
            let mut list = relay.reserved_names();
            let adding = path.ends_with("/add");
            if adding {
                list.push(word.clone());
            } else {
                list.retain(|w| *w != word);
            }
            relay.set_reserved(Some(list));
            audit(relay, "settings", format!("{} reserved word {word}", if adding { "added" } else { "removed" }), ip);
            Response::ok(json!({ "ok": true, "reserved": relay.reserved_names() }))
        }
        "/api/bans/add" | "/api/bans/remove" => {
            let adding = path.ends_with("/add");
            let hours = input.get("hours").and_then(Value::as_f64).filter(|h| *h > 0.0);
            let ban = adding.then(|| (text(input, "reason").unwrap_or_default(), hours.map(|h| now_secs() + (h * 3600.0) as u64)));
            let until = hours.map(|h| format!(" for {h} h")).unwrap_or_default();
            if let Some(uuid) = text(input, "uuid") {
                let Some(id) = relay.ban_uuid(&uuid, ban) else { return Response::error("400 Bad Request", "that isn't a Minecraft UUID") };
                audit(relay, if adding { "ban" } else { "unban" }, format!("{} account {id}{}", if adding { "banned" } else { "unbanned" }, if adding { &until } else { "" }), ip);
                Response::ok(json!({ "ok": true, "uuid": id }))
            } else if let Some(addr) = text(input, "ip") {
                let Some(addr) = relay.ban_ip(&addr, ban) else { return Response::error("400 Bad Request", "that isn't an IP address or CIDR range") };
                audit(relay, if adding { "ban" } else { "unban" }, format!("{} IP {addr}{}", if adding { "banned" } else { "unbanned" }, if adding { &until } else { "" }), ip);
                Response::ok(json!({ "ok": true, "ip": addr }))
            } else {
                Response::error("400 Bad Request", "send a uuid or an ip")
            }
        }
        "/api/settings" => {
            let Some(changes) = input.get("limits").and_then(Value::as_object) else { return Response::error("400 Bad Request", "send {\"limits\": {…}}") };
            let mut merged: Map<String, Value> = serde_json::to_value(relay.limits_now()).ok().and_then(|v| v.as_object().cloned()).unwrap_or_default();
            let mut changed = BTreeMap::new();
            for (k, v) in changes {
                if !merged.contains_key(k) {
                    return Response::error("400 Bad Request", "unknown limit");
                }
                if merged.get(k) != Some(v) {
                    changed.insert(k.clone(), v.clone());
                }
                merged.insert(k.clone(), v.clone());
            }
            let limits: LimitsConfig = match serde_json::from_value(Value::Object(merged)) {
                Ok(l) => l,
                Err(_) => return Response::error("400 Bad Request", "a limit has the wrong type (whole numbers; GB may have decimals)"),
            };
            if limits.tunnels_per_account == 0
                || limits.names_per_account == 0
                || limits.session_days == 0
                || limits.monthly_gb_per_account < 0.0
                || limits.session_minutes < 0.0
                || limits.session_warn_minutes.iter().any(|m| *m < 0.0)
            {
                return Response::error("400 Bad Request", "tunnels, names and session days must be at least 1, and the cap can't be negative");
            }
            relay.set_limits(Some(limits));
            let what: Vec<String> = changed.iter().map(|(k, v)| format!("{k} = {v}")).collect();
            audit(relay, "settings", format!("changed limits: {}", if what.is_empty() { "nothing".into() } else { what.join(", ") }), ip);
            settings(relay)
        }
        "/api/settings/reset" => {
            relay.set_limits(None);
            relay.set_reserved(None);
            audit(relay, "settings", "reset limits and reserved words to relay.toml".into(), ip);
            settings(relay)
        }
        _ => Response::error("404 Not Found", "not found"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serves_under_admin() {
        assert_eq!(inner_path("/admin/"), "/");
        assert_eq!(inner_path("/admin/api/tunnels"), "/api/tunnels");
        assert_eq!(inner_path("/api/tunnels"), "/api/tunnels");
        assert_eq!(inner_path("/administrator"), "/administrator");
    }

    #[test]
    fn hashes_and_checks_passwords() {
        assert!(hash_password("short").is_err());
        let hash = hash_password("correct horse battery").unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password("correct horse battery", &hash));
        assert!(!verify_password("correct horse battery!", &hash));
        assert!(!verify_password("anything", "not a hash"));
    }

    #[test]
    fn locks_out_after_repeated_failures() {
        let s = State::default();
        let ip: IpAddr = "203.0.113.9".parse().unwrap();
        let now = 1_000_000;
        for i in 0..FAILS_PER_IP {
            assert_eq!(s.locked_for(ip, now), 0);
            assert_eq!(s.fail(ip, now), FAILS_PER_IP - i - 1);
        }
        assert!(s.locked_for(ip, now) > 0);
        assert_eq!(s.locked_for("203.0.113.10".parse().unwrap(), now), 0);
        assert_eq!(s.locked_for(ip, now + IP_LOCK_SECS + 1), 0);
    }

    #[test]
    fn writes_the_hash_into_the_config_keeping_comments() {
        let dir = std::env::temp_dir().join(format!("resincraft-relay-pw-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.toml");
        std::fs::write(&path, "# my relay\nbase_domain = \"rscraft.resincore.tech\"\n\n[dashboard]\nlisten = \"127.0.0.1:7443\" # local\n").unwrap();
        write_password_hash(&path, "$argon2id$v=19$m=19456,t=2,p=1$abc$def").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# my relay") && text.contains("# local"));
        assert!(text.contains("password_hash = \"$argon2id$v=19$m=19456,t=2,p=1$abc$def\""));
        std::fs::write(&path, "base_domain = \"x.example\"\n").unwrap();
        write_password_hash(&path, "h").unwrap();
        assert!(std::fs::read_to_string(&path).unwrap().contains("[dashboard]"));
        let _ = std::fs::remove_dir_all(&dir);

        // Replaced in place, keeping its comment and the keys around it; other tables untouched.
        let before = "[dashboard]\r\nlisten = \"0.0.0.0:7443\"\r\npassword_hash = \"\"   # written by set-admin-password\r\nallow_ips = []\r\n\r\n[limits]\r\npassword_hash_x = 1\r\n";
        let after = with_password_hash(before, "$argon2id$v=19$abc");
        assert_eq!(
            after,
            "[dashboard]\nlisten = \"0.0.0.0:7443\"\npassword_hash = \"$argon2id$v=19$abc\" # written by set-admin-password\nallow_ips = []\n\n[limits]\npassword_hash_x = 1\n"
        );
        let again = with_password_hash(&after, "$argon2id$v=19$def");
        assert_eq!(again.matches("password_hash =").count(), 1);
        assert!(again.contains("\"$argon2id$v=19$def\" # written by set-admin-password"));
    }
}
