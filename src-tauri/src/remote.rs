//! Remote control: the web console (console.<relay domain>) runs this launcher's hosted servers from any browser.
//!
//! While Settings → Remote control is on and a Microsoft account is signed in, ResinCraft keeps one connection to
//! ResinCraft Relay open (protocol 6, `remote`), signed in as that account. The console signs the player in with the
//! same Minecraft account, and the relay passes its requests here as calls. Only the methods in `call` exist, so the
//! console can run, watch and make servers but never reach files, Java arguments, commands or other settings; every
//! call shows in the launcher too. Live updates (server states, console lines, task progress) go to the console
//! only for the topics it watches.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{bail, Result};
use resincraft_relay::agent::{AgentError, RemoteCommand, RemoteEvent};
use serde::Serialize;
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
    sync(crate::core::settings::get().await.remote_control);
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
    let ended = conn
        .remote(&device_name(), env!("CARGO_PKG_VERSION"), |e| match e {
            RemoteEvent::Call { id, method, params } => {
                let tx = tx.clone();
                tauri::async_runtime::spawn(async move {
                    let result = call(&method, params).await.map_err(|e| e.to_string());
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
        let push = |topic: String, data: Value| {
            if watched.lock().unwrap().contains(&topic) {
                let _ = tx.send(RemoteCommand::Push { topic, data });
            }
        };
        match channel.as_str() {
            "server" => {
                let mut d = data;
                d["event"] = json!("state");
                push("servers".into(), d);
            }
            "listChanged" if data == json!("servers") => push("servers".into(), json!({ "event": "list" })),
            "serverLog" => {
                let id = data["serverId"].as_str().unwrap_or_default().to_string();
                let lines = data["lines"].as_array().cloned().unwrap_or_default();
                for chunk in lines.chunks(200) {
                    push(format!("log:{id}"), json!({ "lines": chunk }));
                }
            }
            "task" => push("tasks".into(), data),
            _ => {}
        }
    }
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
    "playerList", "playerAdd", "playerRemove", "requests", "requestAccept", "requestReject", "requestBlock", "requestUnblockAll",
    // Properties, anti-xray, Bedrock players, files to download yourself, client-only mods.
    "properties", "setProperties", "antiXray", "setAntiXray", "bedrockJoin", "setBedrockJoin", "downloadsCheck", "downloadsForget", "clientMods",
    // Plugins and mods from the built-in sources.
    "addonSources", "addons", "addonSearch", "addonVersions", "addonChangeVersion", "addonCheckUpdates", "addonCancelPending", "addonInstall", "addonToggle",
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
    if method == "java.list" {
        // Shows where Java is installed on this PC.
        return if full { Ok(()) } else { Err(needs_full()) };
    }
    bail!("The web console can't do that ({method}).")
}

/// Runs one console request (after `check`).
async fn call(method: &str, params: Value) -> Result<Value> {
    let full = crate::core::settings::get().await.remote_full_control;
    let params = if params.is_array() { params } else { json!([]) };
    check(method, &params, full)?;
    let method = alias(method);
    let all: Vec<Value> = params.as_array().cloned().unwrap_or_default();
    let out = match method {
        "server.list" => {
            let list = dispatch("server:list", Args::new(vec![])).await?;
            Value::Array(list.as_array().cloned().unwrap_or_default().into_iter().map(|s| public_server(s, full)).collect())
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
            let allowed = |k: &str| SAFE_PATCH.contains(&k) || (full && FULL_PATCH.contains(&k));
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
        "content.versions", "java.list", "server.downloadsCheck"];
    if !method.starts_with("create.") && !reads.contains(&method) {
        emit("remoteAction", json!({ "method": method, "serverId": params.get(0).and_then(Value::as_str) }));
        crate::core::events::list_changed("servers");
    }
    Ok(out)
}

pub async fn handle(channel: &str, _args: Args) -> Result<Value> {
    match channel {
        "remote:status" => Ok(serde_json::to_value(status())?),
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
    }
}
