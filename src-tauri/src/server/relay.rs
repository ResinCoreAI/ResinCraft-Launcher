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
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use resincraft_relay::agent::{AgentConfig, AgentError, Command, Connection, Credentials, Event};
use resincraft_relay::proto::{self, ErrorCode, Limits, ServiceState, Usage};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::core::fsutil::{read_json, write_json};
use crate::core::paths;
use crate::core::text::{text_of, Text};
use crate::core::types::{RelayInfo, RelayNameCheck, ServerMeta};
use crate::player::accounts::{self, MicrosoftAccount};
use crate::err_tr;

use super::store;

/// Where the relay is.
#[derive(Debug, Clone)]
pub struct Settings {
    /// "tls://rscraft.resincore.tech:7000"
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

fn needs_microsoft() -> anyhow::Error {
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

/// Turns the relay's answer into what the player is told.
pub fn describe(e: &AgentError, name: &str) -> anyhow::Error {
    let Some(code) = e.code() else {
        return match e {
            AgentError::Config(msg) => err_tr!("hostRelay.badConfig", "This build's ResinCraft Relay address doesn't work ({error}).", error = msg),
            other => err_tr!("hostRelay.unreachable", "Couldn't reach ResinCraft Relay ({error}).", error = other.to_string()),
        };
    };
    match code {
        ErrorCode::AuthFailed => err_tr!("hostRelay.authFailed", "ResinCraft Relay couldn't confirm your Minecraft account. Sign in again in Accounts."),
        ErrorCode::NotOwned => err_tr!("hostRelay.notOwned", "This Microsoft account doesn't own Minecraft: Java Edition, which ResinCraft Relay needs."),
        ErrorCode::Banned => err_tr!("hostRelay.banned", "This Minecraft account can't use ResinCraft Relay."),
        ErrorCode::NameInvalid => err_tr!("hostRelay.nameInvalid", "Use 3 to 24 lowercase letters, numbers and dashes, not starting or ending with a dash."),
        ErrorCode::NameReserved => err_tr!("hostRelay.nameReserved", "\"{name}\" is reserved. Pick another name.", name = name),
        ErrorCode::NameTaken => err_tr!("hostRelay.nameTaken", "\"{name}\" belongs to another player. Pick another name.", name = name),
        ErrorCode::TooManyNames => err_tr!("hostRelay.tooManyNames", "Your account already has as many ResinCraft addresses as it may. Reuse the name of another server."),
        ErrorCode::TooManyTunnels => err_tr!("hostRelay.tooManyTunnels", "Your account already has as many servers online through ResinCraft Relay as it may. Stop one first."),
        ErrorCode::OverQuota => err_tr!("hostRelay.overQuota", "This month's ResinCraft Relay traffic for your account is used up. It resets on the 1st."),
        ErrorCode::RateLimited => err_tr!("hostRelay.rateLimited", "Too many attempts. Try again in a few minutes."),
        ErrorCode::Unavailable => err_tr!("hostRelay.mojangDown", "Minecraft's account service didn't answer. Try again in a moment."),
        ErrorCode::Version => err_tr!("hostRelay.outdated", "This ResinCraft Launcher is too old for ResinCraft Relay. Update the launcher."),
        ErrorCode::Replaced => err_tr!("hostRelay.replaced", "{name} came online somewhere else, so it stopped here.", name = name),
        ErrorCode::ShuttingDown => err_tr!("hostRelay.restarting", "ResinCraft Relay is restarting."),
        ErrorCode::Paused => err_tr!("hostRelay.paused", "ResinCraft Relay is paused by its admin: {message}", message = admin_message(e)),
        ErrorCode::ServiceOff => err_tr!("hostRelay.off", "ResinCraft Relay is switched off by its admin: {message}", message = admin_message(e)),
        ErrorCode::SessionEnded => err_tr!("hostRelay.sessionEnded", "The ResinCraft Relay session ended."),
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

/// Connects and signs in: with the saved relay session, else with the account's Minecraft token (refreshed
/// once if the relay doesn't accept it).
async fn connect(s: &Settings, account: &MicrosoftAccount) -> Result<Connection, Failure> {
    let mut conn = Connection::connect(&agent_config(s)).await.map_err(Failure::Relay)?;
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
    Online { address: String, usage: Usage, secs_left: Option<u64> },
    Usage(Usage),
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
    /// Gave up (a ban, the name taken over elsewhere, the monthly cap…).
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
    /// Aborting it closes the tunnel.
    pub task: JoinHandle<()>,
}

/// While paused or off, how often to look whether the relay is back.
const PAUSED_RETRY_SECS: u64 = 60;

/// Continue requests for the running tunnels, by server id.
static COMMANDS: Mutex<Option<HashMap<String, mpsc::UnboundedSender<Command>>>> = Mutex::new(None);

fn paused_of(e: &AgentError) -> Option<(bool, Option<String>)> {
    match e {
        AgentError::Refused(ErrorCode::Paused, msg) => Some((false, Some(msg.clone()).filter(|m| !m.is_empty()))),
        AgentError::Refused(ErrorCode::ServiceOff, msg) => Some((true, Some(msg.clone()).filter(|m| !m.is_empty()))),
        _ => None,
    }
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

/// Brings the name online on `conn`: a Bedrock server gets a UDP port of its own on the relay.
async fn go_online(conn: &mut Connection, name: &str, bedrock: bool) -> Result<resincraft_relay::agent::Online, AgentError> {
    if bedrock {
        conn.online_bedrock(name).await
    } else {
        conn.online(name).await
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
/// `bedrock`: a Bedrock server (UDP), reached on its own port of the relay.
pub async fn open(
    id: &str,
    local_port: u16,
    wait_if_paused: bool,
    bedrock: bool,
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
    let (online, paused) = match go_online(&mut conn, &name, bedrock).await {
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
    let task = tokio::spawn(async move {
        let address = task_address;
        let mut current = online.is_some().then_some(conn);
        let mut wait = PAUSED_RETRY_SECS;
        loop {
            if let Some(conn) = current.take() {
                let status = on_status.clone();
                let ended = conn
                    .serve_with(
                        local,
                        move |e| match e {
                            Event::Usage(u) => status(Status::Usage(u)),
                            Event::Warning { secs_left } => status(Status::Warning { secs_left }),
                            Event::Extended { secs_left, continues } => status(Status::Extended { secs_left, continues }),
                            Event::Service { state: ServiceState::On, .. } => status(Status::Resumed),
                            Event::Service { state, message } => status(Status::Paused { message, off: state == ServiceState::Off, running: true }),
                        },
                        &mut commands,
                    )
                    .await;
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
                    let on = go_online(&mut c, &task_name, bedrock).await.map_err(Failure::Relay)?;
                    Ok::<_, Failure>((c, on))
                };
                match attempt.await {
                    Ok((c, on)) => {
                        on_status(Status::Online { address: shown_address(&on.address, on.bedrock_port), usage: on.usage, secs_left: on.secs_left });
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
    let address = shown_address(&address, opened_online.as_ref().and_then(|o| o.bedrock_port));
    let (limits, usage, secs_left) = opened_online.map(|o| (o.limits, o.usage, o.secs_left)).unwrap_or_default();
    Ok(Opened { name, address, limits, usage, secs_left, paused, task })
}

// ---------- The name picker ----------

fn normalize(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

fn as_text(e: &anyhow::Error) -> Text {
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

/// The address friends type, e.g. for the card: `name.rscraft.resincore.tech`.
pub fn address_of(meta: &ServerMeta) -> Option<String> {
    let s = settings()?;
    meta.relay_name.as_deref().map(|n| proto::address(n, &s.domain))
}

