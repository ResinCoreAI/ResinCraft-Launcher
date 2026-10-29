//! Remote control: the web console (console.<relay domain>) runs this launcher's hosted servers from any browser.
//!
//! While Settings → Remote control is on and a Microsoft account is signed in, ResinCraft keeps one connection to
//! ResinCraft Relay open (protocol 6, `remote`), signed in as that account. The console signs the player in with the
//! same Minecraft account, and the relay passes its requests here as calls. Only the methods in `call` exist, so the
//! console can run, watch and make servers but never reach files, Java arguments, commands or other settings; every
//! call shows in the launcher too. Live updates (server states, console lines, task progress) go to the console
//! only for the topics it watches.
//!
//! The host may also let other Minecraft accounts in (Settings → Remote control → People who can use your servers),
//! each with a role (Viewer, Operator, Manager) and the servers it reaches. The relay lists this launcher for them
//! (protocol 9 `remote_share`) and marks their calls with `from`; everything they may do is decided here
//! ([`check_shared`]), and their live updates only carry the servers they reach.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{bail, Result};
use resincraft_relay::agent::{AgentError, RemoteCommand, RemoteEvent};
use resincraft_relay::proto::ShareAccount;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::core::events::{emit, tap};
use crate::core::router::{dispatch, Args};
use crate::player::accounts;
use crate::server::relay;

/// What Settings → Remote control shows (also sent as the `remote` event).
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStatus {
    /// "off" | "connecting" | "online" | "noAccount" | "unavailable" | "relayTooOld" | "error"
    pub state: &'static str,
    /// Console tabs open on this launcher.
    pub viewers: u32,
    /// Signed in on the console as this player.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    /// The console's address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Why the last try didn't work.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

static STATUS: Mutex<Option<RemoteStatus>> = Mutex::new(None);
static RUNNING: Mutex<Option<tauri::async_runtime::JoinHandle<()>>> = Mutex::new(None);

fn publish(f: impl FnOnce(&mut RemoteStatus)) {
    let s = {
        let mut guard = STATUS.lock().unwrap();
        let s = guard.get_or_insert_with(RemoteStatus::default);
        f(s);
        if s.state.is_empty() {
            s.state = "off";
        }
        s.clone()
    };
    emit("remote", s);
}

pub fn status() -> RemoteStatus {
    let mut s = STATUS.lock().unwrap().clone().unwrap_or_default();
    if s.state.is_empty() {
        s.state = "off";
    }
    s.url = console_url();
    s
}

fn console_url() -> Option<String> {
    relay::settings().map(|s| format!("https://console.{}", s.domain))
}

/// Starts or stops the connection to match the setting (at start and whenever Settings change).
pub fn sync(on: bool) {
    let mut running = RUNNING.lock().unwrap();
    match (on, running.as_ref().is_some_and(|h| !h.inner().is_finished())) {
        (true, false) => *running = Some(tauri::async_runtime::spawn(run())),
        (false, true) => {
            if let Some(h) = running.take() {
                h.abort();
            }
            publish(|s| *s = RemoteStatus { state: "off", ..Default::default() });
        }
        _ => {}
    }
}

pub async fn init() {
    *GRANTS.lock().unwrap() = read_grants().await;
    sync(crate::core::settings::get().await.remote_control);
}

// ---------- Other accounts (protocol 9) ----------

/// Another account the host lets in, with what it may do.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Grant {
    /// Minecraft UUID (with dashes).
    pub uuid: String,
    pub name: String,
    /// "viewer" (look), "operator" (also run the servers, players and requests) or "manager" (also settings,
    /// plugins and mods, modpack; never making, deleting or full control).
    pub role: String,
    /// The servers it reaches (None: all of this PC's servers).
    #[serde(default)]
    pub servers: Option<Vec<String>>,
}

const ROLES: &[&str] = &["viewer", "operator", "manager"];
/// Accounts one PC may let in (the relay's limit too).
const MAX_GRANTS: usize = 64;

/// The grants as last saved (also read by the push filter, which can't wait for the disk).
static GRANTS: Mutex<Vec<Grant>> = Mutex::new(Vec::new());
/// The remote-control connection's sender, for a new list of accounts while it's online.
static SHARE: Mutex<Option<mpsc::UnboundedSender<RemoteCommand>>> = Mutex::new(None);
/// This launcher's device id on the relay (remoteview.rs leaves it out of the list of PCs).
static OWN_DEVICE: Mutex<Option<u64>> = Mutex::new(None);

pub fn own_device() -> Option<u64> {
    *OWN_DEVICE.lock().unwrap()
}

fn grants_file() -> PathBuf {
    crate::core::paths::root().join("remote-access.json")
}

async fn read_grants() -> Vec<Grant> {
    crate::core::fsutil::read_json(&grants_file()).await.unwrap_or_default()
}

pub fn grants() -> Vec<Grant> {
    GRANTS.lock().unwrap().clone()
}

/// A UUID as the relay sends it in `from`: lowercase, no dashes.
fn norm(uuid: &str) -> String {
    uuid.trim().replace('-', "").to_ascii_lowercase()
}

fn grant_of(uuid: &str) -> Option<Grant> {
    let uuid = norm(uuid);
    GRANTS.lock().unwrap().iter().find(|g| norm(&g.uuid) == uuid).cloned()
}

fn in_scope(g: &Grant, server: &str) -> bool {
    g.servers.as_ref().is_none_or(|list| list.iter().any(|s| s == server))
}

fn share_list(grants: &[Grant]) -> Vec<ShareAccount> {
    grants.iter().map(|g| ShareAccount { uuid: norm(&g.uuid), name: g.name.clone() }).collect()
}

/// Saves who may use this PC's servers (checked and tidied first) and tells the relay at once.
pub async fn set_grants(list: Vec<Grant>) -> Result<Vec<Grant>> {
    let mut clean: Vec<Grant> = Vec::new();
    for mut g in list {
        let id = norm(&g.uuid);
        let Ok(parsed) = uuid::Uuid::parse_str(&id) else { bail!("{} isn't a Minecraft account", g.name) };
        if !ROLES.contains(&g.role.as_str()) {
            bail!("Pick Viewer, Operator or Manager for {}", g.name);
        }
        if clean.iter().any(|c| norm(&c.uuid) == id) {
            continue;
        }
        g.uuid = parsed.hyphenated().to_string();
        g.name = g.name.trim().chars().take(32).collect();
        if let Some(servers) = &mut g.servers {
            servers.retain(|s| !s.trim().is_empty());
            servers.dedup();
        }
        clean.push(g);
    }
    if clean.len() > MAX_GRANTS {
        bail!("You can let at most {MAX_GRANTS} people in");
    }
    if let Some(own) = accounts::microsoft_account(None).await {
        if clean.iter().any(|g| norm(&g.uuid) == norm(&own.uuid)) {
            bail!("That's your own account: it can always use your servers");
        }
    }
    crate::core::fsutil::write_json(&grants_file(), &clean).await?;
    *GRANTS.lock().unwrap() = clean.clone();
    if let Some(tx) = SHARE.lock().unwrap().as_ref() {
        let _ = tx.send(RemoteCommand::Share(share_list(&clean)));
    }
    Ok(clean)
}

/// What a Viewer may call (`server.<name>`): looking only.
const VIEWER: &[&str] = &[
    "list", "get", "log", "playerList", "properties", "antiXray", "antiCheat", "bedrockJoin", "addons", "addonSources", "relayInfo", "network", "domain",
    "origin", "clientMods", "requests", "packFlags",
];
/// What an Operator may call on top: running the server, its players and whitelist requests.
const OPERATOR: &[&str] = &[
    "start", "stop", "kill", "restart", "command", "acceptEula", "backup", "playerAdd", "playerRemove", "requestAccept", "requestReject", "requestBlock",
    "requestUnblock", "requestUnblockAll", "relayContinue",
];
/// Never for other accounts, whatever their role: making and deleting servers, and the host's relay names and data.
const OWNER_ONLY: &[&str] = &["create", "remove", "relayName", "relayTopUp"];
/// `server.<name>`s that don't name a server.
const NO_SERVER: &[&str] = &["list", "versions", "builds", "softwareIcons"];
/// Server settings other accounts may change (Managers).
const SHARED_PATCH: &[&str] = &["name", "memoryMb", "port", "public", "joinRequests", "optimizedFlags", "packFlags"];

fn role_allows(role: &str, name: &str) -> bool {
    let viewer = VIEWER.contains(&name);
    let operator = viewer || OPERATOR.contains(&name);
    match role {
        "viewer" => viewer,
        "operator" => operator,
        "manager" => operator || (SERVER.contains(&name) && !OWNER_ONLY.contains(&name)),
        _ => false,
    }
}

fn role_name(role: &str) -> &'static str {
    match role {
        "manager" => "Manager",
        "operator" => "Operator",
        _ => "Viewer",
    }
}

/// Refuses what another account may not do with its grant (on top of [`check`]).
fn check_shared(method: &str, params: &Value, g: &Grant) -> Result<()> {
    let method = alias(method);
    let Some(name) = method.strip_prefix("server.") else {
        // The version list a Manager's modpack switch reads.
        if g.role == "manager" && matches!(method, "versions.list" | "content.versions") {
            return Ok(());
        }
        bail!("The owner of this PC hasn't given you access to that.");
    };
    if !role_allows(&g.role, name) {
        bail!("Your access to this PC ({}) doesn't include that. Ask its owner.", role_name(&g.role));
    }
    if !NO_SERVER.contains(&name) && !in_scope(g, &id_of(params)?) {
        bail!("The owner of this PC hasn't shared that server with you.");
    }
    Ok(())
}

/// Keeps the connection up, waiting a little longer after each failure.
async fn run() {
    if relay::settings().is_none() {
        publish(|s| *s = RemoteStatus { state: "unavailable", ..Default::default() });
        return;
    }
    let mut wait = Duration::from_secs(5);
    loop {
        let Some(account) = accounts::microsoft_account(None).await else {
            publish(|s| *s = RemoteStatus { state: "noAccount", ..Default::default() });
            tokio::time::sleep(Duration::from_secs(30)).await;
            continue;
        };
        publish(|s| *s = RemoteStatus { state: "connecting", account: Some(account.username.clone()), ..Default::default() });
        let ended = match relay::connect_for_remote(&account).await {
            Ok(conn) if !conn.has_remote() => {
                publish(|s| *s = RemoteStatus { state: "relayTooOld", account: Some(account.username.clone()), ..Default::default() });
                tokio::time::sleep(Duration::from_secs(600)).await;
                continue;
            }
            Ok(conn) => {
                wait = Duration::from_secs(5);
                serve(conn, &account.username).await
            }
            Err(e) => e,
        };
        let shown = relay::describe(&ended, "").to_string();
        publish(|s| *s = RemoteStatus { state: "error", account: Some(account.username.clone()), error: Some(shown), ..Default::default() });
        tokio::time::sleep(wait).await;
        wait = (wait * 2).min(Duration::from_secs(120));
    }
}

fn device_name() -> String {
    sysinfo::System::host_name().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "ResinCraft".into())
}

/// One connection: answers calls and sends the watched topics' updates until it ends.
async fn serve(conn: resincraft_relay::agent::Connection, account: &str) -> AgentError {
    let (tx, mut rx) = mpsc::unbounded_channel::<RemoteCommand>();
    let watched: Arc<Mutex<HashSet<String>>> = Arc::default();
    let pushes = tokio::spawn(forward_events(tx.clone(), watched.clone()));
    publish(|s| *s = RemoteStatus { state: "online", account: Some(account.to_string()), ..Default::default() });
    // Who else may come in (the relay ignores it when it's too old to share).
    let _ = tx.send(RemoteCommand::Share(share_list(&grants())));
    *SHARE.lock().unwrap() = Some(tx.clone());
    let ended = conn
        .remote(&device_name(), env!("CARGO_PKG_VERSION"), |e| match e {
            RemoteEvent::Ready(id) => *OWN_DEVICE.lock().unwrap() = id,
            RemoteEvent::Call { id, method, params, from } => {
                let tx = tx.clone();
                tauri::async_runtime::spawn(async move {
                    let result = call(&method, params, from.as_deref()).await.map_err(|e| e.to_string());
                    let _ = tx.send(RemoteCommand::Result { id, result });
                });
            }
            RemoteEvent::Watch { topic, on } => {
                let mut w = watched.lock().unwrap();
                if on {
                    w.insert(topic);
                } else {
                    w.remove(&topic);
                }
            }
            RemoteEvent::Viewers(count) => publish(|s| s.viewers = count),
        }, &mut rx)
        .await;
    pushes.abort();
    *SHARE.lock().unwrap() = None;
    *OWN_DEVICE.lock().unwrap() = None;
    ended
}

/// Server states, list changes, console lines and tasks, for the topics the console watches.
async fn forward_events(tx: mpsc::UnboundedSender<RemoteCommand>, watched: Arc<Mutex<HashSet<String>>>) {
    let mut events = tap();
    loop {
        let (channel, data) = match events.recv().await {
            Ok(e) => e,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(_) => return,
        };
        // The owner's own topics, and other accounts' (`as:<uuid>:<topic>`) for the servers they reach.
        let push = |topic: String, data: Value, server: Option<&str>| {
            for t in route(&watched.lock().unwrap(), &topic, server) {
                let _ = tx.send(RemoteCommand::Push { topic: t, data: data.clone() });
            }
        };
        match channel.as_str() {
            "server" => {
                let mut d = data;
                d["event"] = json!("state");
                let id = d["serverId"].as_str().unwrap_or_default().to_string();
                push("servers".into(), d, Some(&id));
            }
            "listChanged" if data == json!("servers") => push("servers".into(), json!({ "event": "list" }), None),
            "serverLog" => {
                let id = data["serverId"].as_str().unwrap_or_default().to_string();
                let lines = data["lines"].as_array().cloned().unwrap_or_default();
                for chunk in lines.chunks(200) {
                    push(format!("log:{id}"), json!({ "lines": chunk }), Some(&id));
                }
            }
            "task" => {
                // Other accounts only hear tasks on a server they reach; the rest are the owner's alone.
                match data["instanceId"].as_str().map(str::to_string) {
                    Some(id) => push("tasks".into(), data, Some(&id)),
                    None if watched.lock().unwrap().contains("tasks") => {
                        let _ = tx.send(RemoteCommand::Push { topic: "tasks".into(), data });
                    }
                    None => {}
                }
            }
            _ => {}
        }
    }
}

/// The watched topics one update goes out on: `topic` itself when the owner watches it, and `as:<uuid>:<topic>`
/// for each other account watching it that still has a grant reaching `server` (None: no server in particular).
fn route(watched: &HashSet<String>, topic: &str, server: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    for w in watched {
        if w == topic {
            out.push(w.clone());
        } else if let Some((uuid, t)) = w.strip_prefix("as:").and_then(|r| r.split_once(':')) {
            if t == topic && grant_of(uuid).is_some_and(|g| server.is_none_or(|s| in_scope(&g, s))) {
                out.push(w.clone());
            }
        }
    }
    out
}

/// What the console sees of a server: never the relay secret; Java settings only with full control.
fn public_server(mut v: Value, full: bool) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.remove("relaySecret");
        o.remove("relayAccount");
        if !full {
            o.remove("jvmArgs");
            o.remove("javaPath");
        }
    }
    v
}

const KINDS: &[&str] = &["vanilla", "paper", "purpur", "folia", "spigot", "leaf", "fabric", "quilt", "forge", "neoforge", "arclight", "bedrock"];

/// Server-manager actions the console may run as `server.<name>` (the engine's `server:<name>`, same arguments).
const SERVER: &[&str] = &[
    "get", "start", "stop", "kill", "command", "log", "acceptEula", "remove", "backup", "setIcon", "update",
    // Players and whitelist requests.
    "playerList", "playerAdd", "playerRemove", "requests", "requestAccept", "requestReject", "requestBlock", "requestUnblockAll", "requestUnblock",
    // Properties, anti-xray, Bedrock players, files to download yourself, client-only mods.
    "properties", "setProperties", "antiXray", "setAntiXray", "antiCheat", "setAntiCheat", "bedrockJoin", "setBedrockJoin", "downloadsCheck", "downloadsForget", "clientMods",
    // Plugins and mods from the built-in sources.
    "addonSources", "addons", "addonSearch", "addonVersions", "addonChangeVersion", "addonUpdateMany", "addonCheckUpdates", "addonCancelPending", "addonInstall", "addonToggle",
    "addonRemove",
    // Modpack: versions, sync from its instance, reinstall.
    "switchPack", "origin", "syncPreview", "syncFromInstance", "reinstall", "reinstallPack", "packFlags",
    // Sharing: ResinCraft Relay, domain.
    "relayInfo", "relayCheck", "relayName", "relayContinue", "relayTopUp", "relayRestart", "domain", "checkDns", "network",
    // Making servers.
    "versions", "builds", "softwareIcons", "create",
];

/// Only with "Allow full control": these can run programs on this PC (Java and its arguments, any downloaded jar) or
/// hand over secrets, so a stolen console sign-in alone can't use them.
const SERVER_FULL: &[&str] = &["launchArgs", "addonFromLink", "setBotToken", "setDnsToken", "regenerateWorld"];

/// The file manager, on a server's own folder only, with full control.
const FILES_FULL: &[&str] = &["list", "readText", "writeText", "mkdir", "rename", "move", "copy", "trash"];

/// Server settings the console may change; the rest need full control.
const SAFE_PATCH: &[&str] = &["name", "memoryMb", "port", "public", "joinRequests", "optimizedFlags", "packFlags", "tunnel", "domain", "dnsAuto"];
const FULL_PATCH: &[&str] = &["javaPath", "jvmArgs", "relayHost", "relaySecret"];

/// Names from the first console (kept so it keeps working) → the engine's names.
fn alias(method: &str) -> &str {
    match method {
        "servers.list" => "server.list",
        "server.players" => "server.playerList",
        "create.versions" => "server.versions",
        "create.builds" => "server.builds",
        other => other,
    }
}

fn arg(params: &Value, i: usize) -> Value {
    params.get(i).cloned().unwrap_or(Value::Null)
}

fn id_of(params: &Value) -> Result<String> {
    match params.get(0).and_then(Value::as_str) {
        Some(id) if !id.is_empty() => Ok(id.to_string()),
        _ => bail!("Which server?"),
    }
}

fn needs_full() -> anyhow::Error {
    anyhow::anyhow!("That needs full control. Turn on Settings → Remote control → Allow full control in ResinCraft on your PC.")
}

/// Refuses what the console may not do, before anything runs.
fn check(method: &str, params: &Value, full: bool) -> Result<()> {
    let method = alias(method);
    if let Some(name) = method.strip_prefix("server.") {
        if name == "list" || name == "restart" || SERVER.contains(&name) {
            // A server is made from a pack or an instance on this PC; never from a file path sent from the web.
            if name == "create" {
                match params.get(0).and_then(|r| r["source"]["kind"].as_str()) {
                    None | Some("modpack") | Some("instance") => {}
                    Some(_) => bail!("The web console can make a server from a modpack or an instance, not from a file."),
                }
            }
            return Ok(());
        }
        if SERVER_FULL.contains(&name) {
            return if full { Ok(()) } else { Err(needs_full()) };
        }
    }
    if let Some(name) = method.strip_prefix("files.") {
        if FILES_FULL.contains(&name) {
            if !full {
                return Err(needs_full());
            }
            // Only a server's folder: an instance's files are out of reach from the web.
            if params.get(0).map(|r| r["kind"].as_str()) != Some(Some("server")) {
                bail!("The web console can only open a server's own files.");
            }
            return Ok(());
        }
    }
    // What the launcher's own server pages read while making a server (the console is that same UI).
    if matches!(method, "create.options" | "create.searchPacks" | "create.packVersions" | "versions.list" | "content.versions") {
        return Ok(());
    }
    if method == "content.search" {
        // Modpacks only: the console makes servers from them; mods and plugins go through server.addonSearch.
        return match params.get(0).and_then(|q| q["kind"].as_str()) {
            Some("modpack") => Ok(()),
            _ => bail!("The web console can only search modpacks here."),
        };
    }
    // Bringing in a server another launcher keeps on this PC (only ones this PC's own scan finds; see `known_servers`).
    if matches!(method, "migrate.scan" | "migrate.size" | "migrate.import") {
        return Ok(());
    }
    if method == "migrate.scanFolder" {
        // A folder typed on the other side (never the picker, which would open on this PC): it reads any folder here.
        if !full {
            return Err(needs_full());
        }
        return match params.get(0).and_then(Value::as_str) {
            Some(p) if !p.trim().is_empty() => Ok(()),
            _ => bail!("Type the server's folder on this PC."),
        };
    }
    if method == "java.list" {
        // Shows where Java is installed on this PC.
        return if full { Ok(()) } else { Err(needs_full()) };
    }
    bail!("The web console can't do that ({method}).")
}

/// Servers other launchers keep on this PC, as this PC's own scan finds them.
async fn found_servers() -> Result<Vec<crate::migrate::Source>> {
    let mut list = crate::migrate::scan().await?;
    for s in &mut list {
        s.items.retain(|i| i.r.server);
    }
    list.retain(|s| !s.items.is_empty());
    Ok(list)
}

/// Refuses any server to import that isn't in [`found_servers`]: a remote call can't name another folder on this PC.
async fn known_servers(refs: &[crate::migrate::ItemRef]) -> Result<()> {
    let keys: HashSet<String> = found_servers().await?.iter().flat_map(|s| s.items.iter().map(|i| i.r.key())).collect();
    if refs.iter().any(|r| !r.server || !keys.contains(&r.key())) {
        bail!("That server isn't in the list on this PC any more. Look again.");
    }
    Ok(())
}

/// Runs one console request (after `check`). `from`: another account's call (its UUID), checked against its grant.
async fn call(method: &str, params: Value, from: Option<&str>) -> Result<Value> {
    let grant = match from {
        Some(uuid) => Some(grant_of(uuid).ok_or_else(|| anyhow::anyhow!("This PC doesn't share its servers with you any more."))?),
        None => None,
    };
    // Full control is the owner's alone.
    let full = grant.is_none() && crate::core::settings::get().await.remote_full_control;
    let params = if params.is_array() { params } else { json!([]) };
    check(method, &params, full)?;
    if let Some(g) = &grant {
        check_shared(method, &params, g)?;
    }
    let method = alias(method);
    let all: Vec<Value> = params.as_array().cloned().unwrap_or_default();
    let out = match method {
        "server.list" => {
            let list = dispatch("server:list", Args::new(vec![])).await?;
            let reached = |s: &Value| grant.as_ref().is_none_or(|g| in_scope(g, s["id"].as_str().unwrap_or_default()));
            Value::Array(list.as_array().cloned().unwrap_or_default().into_iter().filter(reached).map(|s| public_server(s, full)).collect())
        }
        "server.get" => public_server(dispatch("server:get", Args::new(all)).await?, full),
        "server.restart" => {
            let id = id_of(&params)?;
            if crate::server::runtime::is_running(&id) {
                crate::server::runtime::stop(&id)?;
                crate::server::runtime::wait_state(&id, &["stopped", "crashed"], Duration::from_secs(120)).await?;
            }
            dispatch("server:start", Args::new(vec![json!(id)])).await?
        }
        "server.log" => {
            let lines = dispatch("server:log", Args::new(all)).await?;
            let all = lines.as_array().cloned().unwrap_or_default();
            json!({ "lines": all[all.len().saturating_sub(200)..] })
        }
        "server.update" => {
            let id = id_of(&params)?;
            let allowed = |k: &str| if grant.is_some() { SHARED_PATCH.contains(&k) } else { SAFE_PATCH.contains(&k) || (full && FULL_PATCH.contains(&k)) };
            let patch: serde_json::Map<String, Value> =
                arg(&params, 1).as_object().map(|o| o.iter().filter(|(k, _)| allowed(k)).map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
            public_server(dispatch("server:update", Args::new(vec![json!(id), Value::Object(patch)])).await?, full)
        }
        "server.create" => {
            let mut created = dispatch("server:create", Args::new(all)).await?;
            created["server"] = public_server(created["server"].take(), full);
            created
        }
        "create.options" => {
            let instances: Vec<Value> = crate::core::instances::list()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|i| json!({ "id": i.id, "name": i.name, "mcVersion": i.mc_version, "loader": i.loader }))
                .collect();
            let settings = crate::core::settings::get().await;
            json!({ "kinds": KINDS, "instances": instances, "memoryMb": settings.memory_mb, "totalMemoryMb": crate::core::settings::total_memory_mb(), "fullControl": full })
        }
        "create.searchPacks" => {
            let source = arg(&params, 1);
            let query = json!({ "query": arg(&params, 0).as_str().unwrap_or(""), "kind": "modpack", "source": if source.is_string() { source } else { json!("modrinth") }, "limit": 20 });
            dispatch("content:search", Args::new(vec![query])).await?
        }
        "create.packVersions" => dispatch("content:versions", Args::new(vec![arg(&params, 0)])).await?,
        "migrate.scan" => serde_json::to_value(found_servers().await?)?,
        "migrate.scanFolder" => {
            let mut found = dispatch("migrate:scanFolder", Args::new(vec![json!(arg(&params, 0).as_str().unwrap_or_default().trim())])).await?;
            // Servers only, like the scan.
            if let Some(items) = found.get_mut("items").and_then(Value::as_array_mut) {
                items.retain(|i| i["server"].as_bool() == Some(true));
            }
            found
        }
        "migrate.size" => {
            let r: crate::migrate::ItemRef = serde_json::from_value(arg(&params, 0))?;
            // With full control a typed folder may be one the scan doesn't list.
            if !full {
                known_servers(std::slice::from_ref(&r)).await?;
            }
            dispatch("migrate:size", Args::new(vec![arg(&params, 0)])).await?
        }
        "migrate.import" => {
            let refs: Vec<crate::migrate::ItemRef> = serde_json::from_value(arg(&params, 0))?;
            if refs.iter().any(|r| !r.server) {
                bail!("Only servers can be brought in from another PC.");
            }
            if !full {
                known_servers(&refs).await?;
            }
            // Never removes the originals from the web.
            dispatch("migrate:import", Args::new(vec![arg(&params, 0), json!(false)])).await?
        }
        m => {
            // The rest are the engine's own channels with the same arguments: server.<x> → server:<x>, files.<x> → files:<x>.
            let channel = m.replacen('.', ":", 1);
            dispatch(&channel, Args::new(all)).await?
        }
    };
    // The launcher's own screens follow changes made from the console.
    let reads = ["server.list", "server.get", "server.log", "server.playerList", "server.requests", "server.properties", "server.addons", "server.addonSources",
        "server.addonSearch", "server.addonVersions", "server.addonCheckUpdates", "server.antiXray", "server.bedrockJoin", "server.origin", "server.syncPreview",
        "server.relayInfo", "server.relayCheck", "server.domain", "server.checkDns", "server.network", "server.versions", "server.builds", "server.softwareIcons",
        "server.packFlags", "server.clientMods", "server.launchArgs", "files.list", "files.readText", "versions.list", "content.search",
        "content.versions", "java.list", "server.downloadsCheck", "migrate.scan", "migrate.scanFolder", "migrate.size"];
    if !method.starts_with("create.") && !reads.contains(&method) {
        emit("remoteAction", json!({ "method": method, "serverId": params.get(0).and_then(Value::as_str), "by": grant.as_ref().map(|g| g.name.clone()) }));
        crate::core::events::list_changed("servers");
    }
    Ok(out)
}

pub async fn handle(channel: &str, args: Args) -> Result<Value> {
    match channel {
        "remote:status" => Ok(serde_json::to_value(status())?),
        "remote:grants" => Ok(serde_json::to_value(grants())?),
        "remote:setGrants" => Ok(serde_json::to_value(set_grants(args.get::<Vec<Grant>>(0)?).await?)?),
        "remote:lookupName" => {
            let (uuid, name) = crate::server::properties::lookup(args.get::<String>(0)?.trim()).await?;
            Ok(json!({ "uuid": uuid, "name": name }))
        }
        _ => bail!("Unknown request: {channel}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_console_never_sees_secrets_and_java_only_with_full_control() {
        let s = json!({ "id": "smp", "name": "SMP", "relaySecret": "s", "jvmArgs": "-x", "javaPath": "C:/Users/me/java.exe", "relayAccount": "u" });
        assert_eq!(public_server(s.clone(), false), json!({ "id": "smp", "name": "SMP" }));
        assert_eq!(public_server(s, true), json!({ "id": "smp", "name": "SMP", "jvmArgs": "-x", "javaPath": "C:/Users/me/java.exe" }));
    }

    #[test]
    fn only_listed_methods_run() {
        let no = |m: &str, p: Value, full: bool| check(m, &p, full).unwrap_err().to_string();
        assert!(no("app:openExternal", json!(["https://example.com"]), true).contains("can't do that"));
        assert!(no("server:start", json!(["smp"]), true).contains("can't do that"), "engine channel names aren't methods");
        assert!(no("server.openFolder", json!(["smp"]), true).contains("can't do that"));
        assert!(no("server.play", json!(["smp"]), true).contains("can't do that"));
        let file = json!([{ "name": "x", "source": { "kind": "file", "path": "C:/evil.zip" } }]);
        assert!(no("server.create", file, true).contains("not from a file"));
        // Everyday server management, the first console's names included.
        for m in ["servers.list", "server.players", "server.addonInstall", "server.setProperties", "server.switchPack", "server.restart", "create.versions"] {
            assert!(check(m, &json!(["smp"]), false).is_ok(), "{m}");
        }
        // What can run programs or reveal secrets needs full control.
        assert!(no("server.addonFromLink", json!(["smp", "https://x/a.jar"]), false).contains("full control"));
        assert!(no("files.writeText", json!([{ "kind": "server", "id": "smp" }, "a.txt", "x"]), false).contains("full control"));
        assert!(check("files.writeText", &json!([{ "kind": "server", "id": "smp" }, "a.txt", "x"]), true).is_ok());
        assert!(no("files.list", json!([{ "kind": "instance", "id": "i" }, ""]), true).contains("server's own files"));
        assert!(no("files.import", json!([{ "kind": "server", "id": "smp" }, "", []]), true).contains("can't do that"));
        // The launcher's own server pages read these while making a server.
        assert!(check("versions.list", &json!([]), false).is_ok());
        assert!(check("content.search", &json!([{ "query": "atm", "kind": "modpack" }]), false).is_ok());
        assert!(no("content.search", json!([{ "query": "x", "kind": "mod" }]), true).contains("only search modpacks"));
        assert!(no("java.list", json!([]), false).contains("full control"));
        assert!(no("instances.launch", json!(["i"]), true).contains("can't do that"));
        // Servers from other launchers on this PC; never a folder named from the web.
        assert!(check("migrate.import", &json!([[]]), false).is_ok());
        assert!(no("migrate.scanFolder", json!(["C:/Servers/smp"]), false).contains("full control"));
        assert!(check("migrate.scanFolder", &json!(["C:/Servers/smp"]), true).is_ok());
        assert!(no("migrate.scanFolder", json!([null]), true).contains("Type the server's folder"), "never the picker on this PC");
    }

    #[test]
    fn other_accounts_get_only_their_role_and_servers() {
        let g = |role: &str, servers: Option<Vec<&str>>| Grant {
            uuid: "069a79f4-44e9-4726-a5be-fca90e38aaf5".into(),
            name: "Bob".into(),
            role: role.into(),
            servers: servers.map(|s| s.into_iter().map(String::from).collect()),
        };
        let ok = |m: &str, p: Value, g: &Grant| check_shared(m, &p, g).is_ok();
        let viewer = g("viewer", Some(vec!["smp"]));
        assert!(ok("server.list", json!([]), &viewer) && ok("server.log", json!(["smp"]), &viewer) && ok("servers.list", json!([]), &viewer));
        assert!(!ok("server.start", json!(["smp"]), &viewer), "a Viewer only looks");
        assert!(!ok("server.log", json!(["other"]), &viewer), "only shared servers");
        let operator = g("operator", None);
        assert!(ok("server.start", json!(["any"]), &operator) && ok("server.command", json!(["any", "say hi"]), &operator));
        assert!(!ok("server.addonInstall", json!(["any", {}]), &operator) && !ok("server.setProperties", json!(["any", {}]), &operator));
        let manager = g("manager", Some(vec!["smp"]));
        assert!(ok("server.addonInstall", json!(["smp", {}]), &manager) && ok("server.update", json!(["smp", {}]), &manager));
        assert!(ok("content.versions", json!(["p"]), &manager) && !ok("content.versions", json!(["p"]), &operator));
        for owner_only in ["server.create", "server.remove", "migrate.scan", "migrate.import", "server.relayName", "server.relayTopUp", "server.launchArgs", "server.setBotToken", "files.list", "java.list", "create.options"] {
            assert!(!ok(owner_only, json!(["smp"]), &manager), "{owner_only}");
        }
        assert!(!ok("server.start", json!([]), &operator), "a server has to be named");
    }

    #[test]
    fn updates_reach_other_accounts_only_for_their_servers() {
        *GRANTS.lock().unwrap() = vec![Grant { uuid: "069a79f4-44e9-4726-a5be-fca90e38aaf5".into(), name: "Bob".into(), role: "viewer".into(), servers: Some(vec!["smp".into()]) }];
        let bob = "as:069a79f444e94726a5befca90e38aaf5";
        let watched: HashSet<String> = ["servers".to_string(), format!("{bob}:servers"), format!("{bob}:log:smp"), format!("{bob}:log:other"), "as:ffffffffffffffffffffffffffffffff:servers".into()].into();
        let mut r = route(&watched, "servers", Some("smp"));
        r.sort();
        assert_eq!(r, vec!["as:069a79f444e94726a5befca90e38aaf5:servers".to_string(), "servers".into()], "the owner and Bob; not an account without a grant");
        assert_eq!(route(&watched, "servers", Some("other")), vec!["servers".to_string()], "Bob doesn't hear about other servers");
        assert_eq!(route(&watched, "log:other", Some("other")), Vec::<String>::new(), "nor read their logs");
        assert_eq!(route(&watched, "log:smp", Some("smp")), vec![format!("{bob}:log:smp")]);
        assert_eq!(route(&watched, "servers", None).len(), 2, "a list change goes to both");
        *GRANTS.lock().unwrap() = Vec::new();
    }
}
