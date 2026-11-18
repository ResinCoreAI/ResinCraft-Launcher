//! An MCP server (Model Context Protocol, Streamable HTTP, JSON answers) so AI assistants on this PC (Claude Code,
//! Claude Desktop, Cursor, VS Code, Codex) can work the launcher: list instances, start and stop games, read the game
//! log, search and install mods, look at Java, and start and stop hosted servers.
//!
//! Off by default (Settings → AI assistants). When on, it listens on 127.0.0.1 only, at `http://127.0.0.1:<port>/mcp`,
//! and every request needs `Authorization: Bearer <token>`: a random token kept in `mcp-token.txt` in the data folder,
//! shown in Settings and replaceable there. The tools call the same handlers as the launcher's own window
//! (`core::router::dispatch`), from a fixed list; nothing else is reachable.

use std::sync::Mutex;

use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

#[cfg(not(test))]
use crate::core::router::{dispatch, Args};
use crate::core::{paths, settings};

pub const DEFAULT_PORT: u16 = 47615;
const PROTOCOL: &str = "2025-06-18";
/// A request body bigger than this isn't an MCP call.
const MAX_BODY: usize = 256 * 1024;

static RUNNING: Mutex<Option<tauri::async_runtime::JoinHandle<()>>> = Mutex::new(None);
static STATE: Mutex<McpStatus> = Mutex::new(McpStatus { state: "off", port: 0, error: None });

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    /// "off" | "on" | "error"
    pub state: &'static str,
    pub port: u16,
    pub error: Option<String>,
}

/// Settings → AI assistants: whether it runs, where, and the token to give the assistant.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpInfo {
    #[serde(flatten)]
    pub status: McpStatus,
    pub url: String,
    pub token: String,
}

fn token_file() -> std::path::PathBuf {
    paths::root().join("mcp-token.txt")
}

fn new_token() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

/// The token in memory once read or made, so it can't change under a running server even if the file can't be written.
static TOKEN: tokio::sync::Mutex<Option<String>> = tokio::sync::Mutex::const_new(None);

async fn save_token(t: &str) {
    let _ = tokio::fs::create_dir_all(paths::root()).await;
    let _ = tokio::fs::write(token_file(), t).await;
}

/// The token, made the first time it's needed.
pub async fn token() -> String {
    let mut cached = TOKEN.lock().await;
    if let Some(t) = cached.as_ref() {
        return t.clone();
    }
    let t = match tokio::fs::read_to_string(token_file()).await {
        Ok(t) if t.trim().len() >= 32 => t.trim().to_string(),
        _ => {
            let t = new_token();
            save_token(&t).await;
            t
        }
    };
    *cached = Some(t.clone());
    t
}

/// A new token; assistants set up with the old one have to be given this one.
pub async fn reset_token() -> Result<McpInfo> {
    {
        let t = new_token();
        save_token(&t).await;
        *TOKEN.lock().await = Some(t);
    }
    // The running server checks the token it read at start; restart it with the new one.
    let s = settings::get().await;
    sync(s.mcp, s.mcp_port);
    info().await
}

pub async fn info() -> Result<McpInfo> {
    let s = settings::get().await;
    let port = if s.mcp_port == 0 { DEFAULT_PORT } else { s.mcp_port };
    let token = token().await;
    let status = STATE.lock().unwrap().clone();
    let port = if status.port != 0 { status.port } else { port };
    Ok(McpInfo { status, url: format!("http://127.0.0.1:{port}/mcp"), token })
}

/// Starts or stops the server to match Settings.
pub fn sync(on: bool, port: u16) {
    let mut running = RUNNING.lock().unwrap();
    if let Some(h) = running.take() {
        h.abort();
    }
    *STATE.lock().unwrap() = McpStatus { state: "off", port: 0, error: None };
    if on {
        let port = if port == 0 { DEFAULT_PORT } else { port };
        *running = Some(tauri::async_runtime::spawn(serve(port)));
    }
}

pub async fn init() {
    let s = settings::get().await;
    sync(s.mcp, s.mcp_port);
}

async fn serve(port: u16) {
    let listener = match TcpListener::bind(("127.0.0.1", port)).await {
        Ok(l) => l,
        Err(e) => {
            *STATE.lock().unwrap() = McpStatus { state: "error", port, error: Some(e.to_string()) };
            return;
        }
    };
    *STATE.lock().unwrap() = McpStatus { state: "on", port, error: None };
    let token = token().await;
    loop {
        let Ok((stream, peer)) = listener.accept().await else { continue };
        if !peer.ip().is_loopback() {
            continue;
        }
        let token = token.clone();
        tokio::spawn(async move {
            let _ = connection(stream, &token).await;
        });
    }
}

/// One HTTP/1.1 connection: requests one after another (keep-alive) until the client closes.
async fn connection(stream: TcpStream, token: &str) -> Result<()> {
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read);
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).await? == 0 {
            return Ok(());
        }
        let mut parts = line.split_whitespace();
        let (method, path) = (parts.next().unwrap_or_default().to_string(), parts.next().unwrap_or_default().to_string());
        let (mut length, mut auth, mut origin) = (0usize, String::new(), None::<String>);
        loop {
            let mut h = String::new();
            if reader.read_line(&mut h).await? == 0 {
                return Ok(());
            }
            let h = h.trim_end();
            if h.is_empty() {
                break;
            }
            if let Some((k, v)) = h.split_once(':') {
                match k.trim().to_ascii_lowercase().as_str() {
                    "content-length" => length = v.trim().parse().unwrap_or(0),
                    "authorization" => auth = v.trim().to_string(),
                    "origin" => origin = Some(v.trim().to_string()),
                    _ => {}
                }
            }
        }
        if length > MAX_BODY {
            reply(&mut write, 413, "application/json", r#"{"error":"too big"}"#).await?;
            return Ok(());
        }
        let mut body = vec![0u8; length];
        reader.read_exact(&mut body).await?;

        // Web pages can reach 127.0.0.1 too: a request a browser sends carries an Origin, which local assistants'
        // own requests don't, so one with a web Origin is turned away (DNS rebinding and cross-site requests).
        if origin.as_deref().is_some_and(|o| !o.starts_with("http://127.0.0.1") && !o.starts_with("http://localhost")) {
            reply(&mut write, 403, "application/json", r#"{"error":"forbidden origin"}"#).await?;
            continue;
        }
        if path.split('?').next() != Some("/mcp") {
            reply(&mut write, 404, "application/json", r#"{"error":"not found"}"#).await?;
            continue;
        }
        if auth.strip_prefix("Bearer ").map(str::trim) != Some(token) {
            reply(&mut write, 401, "application/json", r#"{"error":"missing or wrong token (Settings → AI assistants)"}"#).await?;
            continue;
        }
        if method != "POST" {
            // No server-to-client stream: everything is answered in the POST.
            reply(&mut write, 405, "application/json", r#"{"error":"POST only"}"#).await?;
            continue;
        }
        let msg: Value = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(_) => {
                let e = json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "Parse error" } });
                reply(&mut write, 400, "application/json", &e.to_string()).await?;
                continue;
            }
        };
        match handle(msg).await {
            Some(answer) => reply(&mut write, 200, "application/json", &answer.to_string()).await?,
            None => reply(&mut write, 202, "application/json", "").await?,
        }
    }
}

async fn reply(w: &mut tokio::net::tcp::OwnedWriteHalf, status: u16, kind: &str, body: &str) -> Result<()> {
    let text = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Payload Too Large",
    };
    let head = format!("HTTP/1.1 {status} {text}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n", body.len());
    w.write_all(head.as_bytes()).await?;
    w.write_all(body.as_bytes()).await?;
    Ok(())
}

/// One JSON-RPC message; None for notifications (no answer).
pub async fn handle(msg: Value) -> Option<Value> {
    if msg.is_array() {
        // Batches aren't part of this protocol version.
        return Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32600, "message": "Batches aren't supported" } }));
    }
    let id = msg.get("id").cloned()?;
    let method = msg["method"].as_str().unwrap_or_default();
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "resincraft", "title": "ResinCraft Launcher", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Tools for the ResinCraft Minecraft launcher on this PC. Instance ids come from list_instances; \
                             mod project ids from search_content."
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => {
            let name = msg["params"]["name"].as_str().unwrap_or_default().to_string();
            let args = msg["params"]["arguments"].clone();
            return Some(json!({ "jsonrpc": "2.0", "id": id, "result": call(&name, &args).await }));
        }
        _ => Err((-32601, format!("Unknown method: {method}"))),
    };
    Some(match result {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err((code, message)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
    })
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({ "name": name, "description": description, "inputSchema": { "type": "object", "properties": properties, "required": required } })
}

fn tools() -> Vec<Value> {
    let id = json!({ "type": "string", "description": "An instance id from list_instances" });
    let kind = json!({ "type": "string", "enum": ["mod", "resourcepack", "shader"], "description": "What kind of content (default mod)" });
    vec![
        tool("list_instances", "The player's Minecraft instances: id, name, Minecraft version, mod loader, memory, and whether it's running.", json!({}), &[]),
        tool("launch_instance", "Starts Minecraft in an instance with the signed-in account.", json!({ "instance_id": id }), &["instance_id"]),
        tool("stop_instance", "Stops a running instance (every run of it).", json!({ "instance_id": id }), &["instance_id"]),
        tool(
            "read_game_log",
            "The end of an instance's logs/latest.log (the last game run).",
            json!({ "instance_id": id, "lines": { "type": "integer", "description": "How many lines from the end (default 200, at most 2000)" } }),
            &["instance_id"],
        ),
        tool("list_content", "The mods (or resource packs, shaders) in an instance, with whether each is on.", json!({ "instance_id": id, "kind": kind }), &["instance_id"]),
        tool(
            "search_content",
            "Searches Modrinth and the other sites for mods, resource packs or shaders that fit an instance.",
            json!({ "query": { "type": "string" }, "instance_id": id, "kind": kind }),
            &["query"],
        ),
        tool(
            "install_content",
            "Installs a mod (or resource pack, shader) into an instance, with its required dependencies.",
            json!({ "instance_id": id, "project_id": { "type": "string", "description": "From search_content" }, "kind": kind }),
            &["instance_id", "project_id"],
        ),
        tool("list_java", "The Java installations found on this PC.", json!({}), &[]),
        tool(
            "set_instance_settings",
            "Changes an instance's memory (MB) and/or Java path. Leave a field out to keep it; java_path \"\" goes back to automatic.",
            json!({ "instance_id": id, "memory_mb": { "type": "integer" }, "java_path": { "type": "string" } }),
            &["instance_id"],
        ),
        tool("list_servers", "The Minecraft servers this launcher hosts: id, name, type, version and state.", json!({}), &[]),
        tool("start_server", "Starts a hosted server.", json!({ "server_id": { "type": "string" } }), &["server_id"]),
        tool("stop_server", "Stops a hosted server.", json!({ "server_id": { "type": "string" } }), &["server_id"]),
    ]
}

/// A tool's answer: text content, `isError` when it failed (MCP reports tool failures this way, not as JSON-RPC
/// errors, so the assistant sees why).
async fn call(name: &str, args: &Value) -> Value {
    match run(name, args).await {
        Ok(v) => json!({ "content": [{ "type": "text", "text": if v.is_string() { v.as_str().unwrap_or_default().to_string() } else { serde_json::to_string_pretty(&v).unwrap_or_default() } }] }),
        Err(e) => json!({ "content": [{ "type": "text", "text": format!("{e:#}") }], "isError": true }),
    }
}

fn s<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args[key].as_str().filter(|v| !v.is_empty()).ok_or_else(|| anyhow!("{key} is required"))
}

#[cfg(not(test))]
async fn d(channel: &str, args: Vec<Value>) -> Result<Value> {
    // Boxed: the router's future is big, and unboxed it overflows a worker thread's stack in debug builds.
    Box::pin(dispatch(channel, Args::new(args))).await
}

/// Unit tests don't reach the router: it links the file dialogs (Common Controls v6), and without the app's manifest
/// the test exe wouldn't start. examples/check_mcp.rs runs the real thing.
#[cfg(test)]
async fn d(channel: &str, _args: Vec<Value>) -> Result<Value> {
    bail!("{channel} isn't reachable in unit tests")
}

async fn run(name: &str, a: &Value) -> Result<Value> {
    let kind = a["kind"].as_str().unwrap_or("mod").to_string();
    match name {
        "list_instances" => {
            let list = d("instances:list", vec![]).await?;
            let running: Vec<String> = serde_json::from_value(d("instances:running", vec![]).await?).unwrap_or_default();
            Ok(Value::Array(
                list.as_array()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|i| {
                        let id = i["id"].as_str().unwrap_or_default().to_string();
                        json!({
                            "id": id, "name": i["name"], "minecraft": i["mcVersion"], "loader": i["loader"],
                            "loader_version": i["loaderVersion"], "memory_mb": i["memoryMb"], "running": running.contains(&id)
                        })
                    })
                    .collect(),
            ))
        }
        "launch_instance" => {
            d("instances:launch", vec![json!(s(a, "instance_id")?)]).await?;
            Ok(json!("Starting. Use read_game_log to follow it."))
        }
        "stop_instance" => {
            d("instances:kill", vec![json!(s(a, "instance_id")?)]).await?;
            Ok(json!("Stopped."))
        }
        "read_game_log" => {
            let id = s(a, "instance_id")?;
            crate::core::instances::get(id).await?;
            let lines = a["lines"].as_u64().unwrap_or(200).clamp(1, 2000) as usize;
            let path = paths::game_dir(id).join("logs").join("latest.log");
            let text = tokio::fs::read_to_string(&path).await.map_err(|_| anyhow!("This instance has no game log yet."))?;
            let all: Vec<&str> = text.lines().collect();
            Ok(json!(all[all.len().saturating_sub(lines)..].join("\n")))
        }
        "list_content" => d("content:list", vec![json!(s(a, "instance_id")?), json!(kind)]).await,
        "search_content" => {
            let mut q = json!({ "query": s(a, "query")?, "kind": kind, "limit": 10 });
            if let Some(id) = a["instance_id"].as_str().filter(|v| !v.is_empty()) {
                let inst = crate::core::instances::get(id).await?;
                q["gameVersion"] = json!(inst.mc_version);
                q["loader"] = json!(inst.loader);
            }
            let r = d("content:search", vec![q]).await?;
            Ok(Value::Array(
                r["hits"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|h| json!({ "project_id": h["id"], "title": h["title"], "description": h["description"], "source": h["source"], "downloads": h["downloads"] }))
                    .collect(),
            ))
        }
        "install_content" => {
            d("content:install", vec![json!(s(a, "instance_id")?), json!(s(a, "project_id")?), json!(kind), Value::Null]).await?;
            Ok(json!("Installed."))
        }
        "list_java" => d("java:list", vec![]).await,
        "set_instance_settings" => {
            let id = s(a, "instance_id")?;
            let mut patch = serde_json::Map::new();
            if let Some(mb) = a["memory_mb"].as_u64() {
                if !(512..=65536).contains(&mb) {
                    bail!("memory_mb is between 512 and 65536");
                }
                patch.insert("memoryMb".into(), json!(mb));
            }
            if let Some(java) = a["java_path"].as_str() {
                patch.insert("javaPath".into(), if java.is_empty() { Value::Null } else { json!(java) });
            }
            if patch.is_empty() {
                bail!("Give memory_mb and/or java_path");
            }
            let i = d("instances:update", vec![json!(id), Value::Object(patch)]).await?;
            Ok(json!({ "id": i["id"], "memory_mb": i["memoryMb"], "java_path": i["javaPath"] }))
        }
        "list_servers" => {
            let list = d("server:list", vec![]).await?;
            Ok(Value::Array(
                list.as_array()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|v| json!({ "id": v["id"], "name": v["name"], "type": v["type"], "minecraft": v["mcVersion"], "state": v["state"] }))
                    .collect(),
            ))
        }
        "start_server" => {
            d("server:start", vec![json!(s(a, "server_id")?)]).await?;
            Ok(json!("Starting."))
        }
        "stop_server" => {
            d("server:stop", vec![json!(s(a, "server_id")?)]).await?;
            Ok(json!("Stopping."))
        }
        _ => bail!("Unknown tool: {name}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn speaks_the_protocol() {
        let init = handle(json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} })).await.unwrap();
        assert_eq!(init["result"]["protocolVersion"], PROTOCOL);
        assert_eq!(init["result"]["serverInfo"]["name"], "resincraft");
        assert!(handle(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).await.is_none(), "notifications get no answer");
        let list = handle(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).await.unwrap();
        let names: Vec<&str> = list["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"launch_instance") && names.contains(&"install_content") && names.contains(&"read_game_log"));
        for t in list["result"]["tools"].as_array().unwrap() {
            assert_eq!(t["inputSchema"]["type"], "object", "{}", t["name"]);
        }
        let unknown = handle(json!({ "jsonrpc": "2.0", "id": 3, "method": "nope" })).await.unwrap();
        assert_eq!(unknown["error"]["code"], -32601);
        let bad_tool = handle(json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": { "name": "rm_rf", "arguments": {} } })).await.unwrap();
        assert_eq!(bad_tool["result"]["isError"], true);
        let missing = handle(json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": { "name": "launch_instance", "arguments": {} } })).await.unwrap();
        assert_eq!(missing["result"]["isError"], true);
        assert!(missing["result"]["content"][0]["text"].as_str().unwrap().contains("instance_id"));
    }
}
