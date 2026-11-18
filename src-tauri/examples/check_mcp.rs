//! Runs the MCP server on a scratch data folder and talks to it over HTTP the way an AI assistant does:
//!   cargo run --example check_mcp
use resincraft_lib::core::ctx;
use resincraft_lib::mcp;
use serde_json::{json, Value};

const PORT: u16 = 47699;

#[tokio::main]
async fn main() {
    ctx::init(std::env::temp_dir().join(format!("resin-mcp-{}", std::process::id())), None);
    mcp::sync(true, PORT);
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let info = mcp::info().await.unwrap();
    assert_eq!(info.status.state, "on", "{:?}", info.status.error);
    println!("listening at {}", info.url);

    let http = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{PORT}/mcp");
    let post = |body: Value, token: Option<&str>, origin: Option<&str>| {
        let mut r = http.post(&url).json(&body);
        if let Some(t) = token {
            r = r.bearer_auth(t);
        }
        if let Some(o) = origin {
            r = r.header("Origin", o);
        }
        r.send()
    };
    let rpc = |id: u32, method: &str, params: Value| json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
    let t = Some(info.token.as_str());

    let r = post(rpc(1, "initialize", json!({})), None, None).await.unwrap();
    assert_eq!(r.status(), 401, "no token");
    let r = post(rpc(1, "initialize", json!({})), Some("wrong"), None).await.unwrap();
    assert_eq!(r.status(), 401, "wrong token");
    let r = post(rpc(1, "initialize", json!({})), t, Some("https://evil.example")).await.unwrap();
    assert_eq!(r.status(), 403, "web page origin");

    let r = post(rpc(1, "initialize", json!({})), t, None).await.unwrap();
    let status = r.status();
    let r: Value = serde_json::from_str(&r.text().await.unwrap()).unwrap_or_else(|e| panic!("{status}: {e}"));
    assert_eq!(status, 200, "{r}");
    println!("initialize → {}", r["result"]["serverInfo"]);
    let r = post(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }), t, None).await.unwrap();
    assert_eq!(r.status(), 202);
    let r: Value = post(rpc(2, "tools/list", json!({})), t, None).await.unwrap().json().await.unwrap();
    println!("tools: {}", r["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect::<Vec<_>>().join(", "));

    for (name, args) in [
        ("list_instances", json!({})),
        ("list_java", json!({})),
        ("list_servers", json!({})),
        ("search_content", json!({ "query": "sodium" })),
        ("read_game_log", json!({ "instance_id": "nope" })),
    ] {
        let r: Value = post(rpc(3, "tools/call", json!({ "name": name, "arguments": args })), t, None).await.unwrap().json().await.unwrap();
        let text = r["result"]["content"][0]["text"].as_str().unwrap_or_default();
        let short: String = text.chars().take(300).collect();
        println!("{name} (error: {}) → {short}", r["result"]["isError"]);
    }
    mcp::sync(false, PORT);
    println!("ok");
}
