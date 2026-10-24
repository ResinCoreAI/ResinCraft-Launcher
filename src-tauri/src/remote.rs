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

/// What the console sees of a server: no secrets, no paths on this PC, no Java arguments.
fn public_server(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        for key in ["relaySecret", "jvmArgs", "javaPath", "relayAccount"] {
            o.remove(key);
        }
    }
    v
}

const KINDS: &[&str] = &["vanilla", "paper", "purpur", "folia", "spigot", "leaf", "fabric", "quilt", "forge", "neoforge", "arclight", "bedrock"];
/// The server settings the console may change; the rest (Java, sharing secrets, commands) only change here.
const SAFE_PATCH: &[&str] = &["name", "memoryMb", "port", "public", "joinRequests"];

fn arg(params: &Value, i: usize) -> Value {
    params.get(i).cloned().unwrap_or(Value::Null)
}

fn id_of(params: &Value) -> Result<String> {
    match params.get(0).and_then(Value::as_str) {
        Some(id) if !id.is_empty() => Ok(id.to_string()),
        _ => bail!("Which server?"),
    }
}

/// Every method the console may call (the relay only passes these on, too).
const METHODS: &[&str] = &[
    "servers.list", "server.get", "server.start", "server.stop", "server.kill", "server.restart", "server.command", "server.log",
    "server.players", "server.playerAdd", "server.playerRemove", "server.requests", "server.requestAccept", "server.requestReject",
    "server.backup", "server.update", "server.remove", "server.acceptEula", "create.options", "create.versions", "create.builds",
    "create.searchPacks", "create.packVersions", "server.create",
];

/// Refuses what the console may not do, before anything runs.
fn check(method: &str, params: &Value) -> Result<()> {
    if !METHODS.contains(&method) {
        bail!("The web console can't do that ({method}).");
    }
    // A server is made from a pack or an instance on this PC; never from a file path sent from the web.
    if method == "server.create" {
        match params.get(0).and_then(|r| r["source"]["kind"].as_str()) {
            None | Some("modpack") | Some("instance") => {}
            Some(_) => bail!("The web console can make a server from a modpack or an instance, not from a file."),
        }
    }
    Ok(())
}

/// Runs one console request (after `check`).
async fn call(method: &str, params: Value) -> Result<Value> {
    let params = if params.is_array() { params } else { json!([]) };
    check(method, &params)?;
    let server = |channel: &'static str, n: usize| {
        let a: Vec<Value> = (0..n).map(|i| arg(&params, i)).collect();
        async move { dispatch(channel, Args::new(a)).await }
    };
    let out = match method {
        "servers.list" => {
            let list = dispatch("server:list", Args::new(vec![])).await?;
            Value::Array(list.as_array().cloned().unwrap_or_default().into_iter().map(public_server).collect())
        }
        "server.get" => public_server(server("server:get", 1).await?),
        "server.start" => server("server:start", 1).await?,
        "server.stop" => server("server:stop", 1).await?,
        "server.kill" => server("server:kill", 1).await?,
        "server.restart" => {
            let id = id_of(&params)?;
            if crate::server::runtime::is_running(&id) {
                crate::server::runtime::stop(&id)?;
                crate::server::runtime::wait_state(&id, &["stopped", "crashed"], Duration::from_secs(120)).await?;
            }
            dispatch("server:start", Args::new(vec![json!(id)])).await?
        }
        "server.command" => server("server:command", 2).await?,
        "server.log" => {
            let lines = server("server:log", 1).await?;
            let all = lines.as_array().cloned().unwrap_or_default();
            json!({ "lines": all[all.len().saturating_sub(200)..] })
        }
        "server.players" => server("server:playerList", 2).await?,
        "server.playerAdd" => server("server:playerAdd", 3).await?,
        "server.playerRemove" => server("server:playerRemove", 3).await?,
        "server.requests" => server("server:requests", 1).await?,
        "server.requestAccept" => server("server:requestAccept", 2).await?,
        "server.requestReject" => server("server:requestReject", 2).await?,
        "server.backup" => server("server:backup", 1).await?,
        "server.update" => {
            let id = id_of(&params)?;
            let patch: serde_json::Map<String, Value> =
                arg(&params, 1).as_object().map(|o| o.iter().filter(|(k, _)| SAFE_PATCH.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
            public_server(dispatch("server:update", Args::new(vec![json!(id), Value::Object(patch)])).await?)
        }
        "server.remove" => server("server:remove", 1).await?,
        "server.acceptEula" => server("server:acceptEula", 1).await?,
        "create.options" => {
            let instances: Vec<Value> = crate::core::instances::list()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|i| json!({ "id": i.id, "name": i.name, "mcVersion": i.mc_version, "loader": i.loader }))
                .collect();
            let settings = crate::core::settings::get().await;
            json!({ "kinds": KINDS, "instances": instances, "memoryMb": settings.memory_mb, "totalMemoryMb": crate::core::settings::total_memory_mb() })
        }
        "create.versions" => server("server:versions", 1).await?,
        "create.builds" => server("server:builds", 2).await?,
        "create.searchPacks" => {
            let source = arg(&params, 1);
            let query = json!({ "query": arg(&params, 0).as_str().unwrap_or(""), "kind": "modpack", "source": if source.is_string() { source } else { json!("modrinth") }, "limit": 20 });
            dispatch("content:search", Args::new(vec![query])).await?
        }
        "create.packVersions" => dispatch("content:versions", Args::new(vec![arg(&params, 0)])).await?,
        "server.create" => {
            let req = arg(&params, 0);
            let mut created = dispatch("server:create", Args::new(vec![req])).await?;
            created["server"] = public_server(created["server"].take());
            created
        }
        _ => bail!("The web console can't do that ({method})."),
    };
    // The launcher's own screens follow changes made from the console.
    if !method.starts_with("create.") && !matches!(method, "servers.list" | "server.get" | "server.log" | "server.players" | "server.requests") {
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
    fn the_console_never_sees_secrets_or_paths() {
        let v = public_server(json!({ "id": "smp", "name": "SMP", "relaySecret": "s", "jvmArgs": "-x", "javaPath": "C:/Users/me/java.exe", "relayAccount": "u" }));
        assert_eq!(v, json!({ "id": "smp", "name": "SMP" }));
    }

    #[test]
    fn only_listed_methods_run() {
        assert!(check("app:openExternal", &json!(["https://example.com"])).unwrap_err().to_string().contains("can't do that"));
        assert!(check("server:start", &json!(["smp"])).is_err(), "engine channel names aren't methods");
        let file = json!([{ "name": "x", "source": { "kind": "file", "path": "C:/evil.zip" } }]);
        assert!(check("server.create", &file).unwrap_err().to_string().contains("not from a file"));
        assert!(check("server.create", &json!([{ "name": "x", "source": { "kind": "modpack", "projectId": "p" } }])).is_ok());
        assert!(check("servers.list", &json!([])).is_ok());
    }
}
