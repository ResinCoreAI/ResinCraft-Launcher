//! Headless end-to-end check of the proxy config editor in a scratch data folder:
//!   cargo run --no-default-features --features panel --example check_proxyconfig -- <empty folder> [velocity] [bungeecord] [waterfall] [waterdogpe]
//! Makes real proxies (none needs the EULA), edits their config through the engine's calls, starts them and checks
//! they read it: a bare login sent to the proxy arrives at the backend address the
//! try list and the forced host name. Paper is only planted as files, since running it needs the EULA.
//! The test turns the proxy's own online mode off, in the scratch folder, so a login needs no Mojang session.

use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use resincraft_lib::core::router::dispatch;
use resincraft_lib::core::types::ServerKind;
use resincraft_lib::core::{ctx, Args};
use resincraft_lib::server::{proxyconfig, runtime, store};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

async fn call(channel: &str, args: Vec<Value>) -> Result<Value> {
    dispatch(channel, Args::new(args)).await
}

fn expect(what: &str, ok: bool, detail: impl std::fmt::Debug) -> Result<()> {
    println!("{} {what}: {detail:?}", if ok { "ok  " } else { "FAIL" });
    if ok {
        Ok(())
    } else {
        Err(anyhow!("{what}"))
    }
}

async fn make(name: &str, kind: &str, port: u16) -> Result<String> {
    let versions = call("server:versions", vec![json!(kind)]).await?;
    let version = versions.as_array().and_then(|v| v.first()).map(|v| v.get("id").cloned().unwrap_or_else(|| v.clone())).context("no versions")?;
    let made = call("server:create", vec![json!({ "name": name, "type": kind, "mcVersion": version, "port": port, "memoryMb": 512 })]).await?;
    made["server"]["id"].as_str().map(str::to_string).context("no id")
}

/// A Paper server in name only: its files, never run.
async fn plant_paper(like: &str, id: &str, port: u16) -> Result<String> {
    let mut meta = store::get(like).await?;
    meta.id = id.to_string();
    meta.name = format!("Planted {id}");
    meta.kind = ServerKind::Paper;
    meta.mc_version = "1.21.4".into();
    meta.port = port;
    let dir = store::dir(id)?;
    std::fs::create_dir_all(dir.join("config"))?;
    std::fs::write(dir.join("server.properties"), format!("# kept\nonline-mode=true\nserver-port={port}\n"))?;
    std::fs::write(dir.join("config/paper-global.yml"), "# Paper's comment\n_version: 29\nproxies:\n  velocity:\n    enabled: false\n    online-mode: true\n    secret: ''\nmisc:\n  kept: true\n")?;
    std::fs::write(dir.join("spigot.yml"), "# Spigot's comment\nsettings:\n  bungeecord: false\n  debug: false\n")?;
    store::save(&meta).await?;
    Ok(id.to_string())
}

async fn start(id: &str) -> Result<()> {
    call("server:start", vec![json!(id)]).await?;
    let state = runtime::wait_state(id, &["running", "crashed", "stopped"], Duration::from_secs(180)).await?;
    expect("proxy comes up", state == "running", state)
}

async fn stop(id: &str) -> Result<()> {
    runtime::stop(id)?;
    runtime::wait_state(id, &["stopped", "crashed"], Duration::from_secs(60)).await?;
    Ok(())
}

async fn log(id: &str) -> String {
    call("server:log", vec![json!(id)]).await.map(|l| l.to_string()).unwrap_or_default()
}

/// Runs a console command and answers with what the log gained.
async fn console(id: &str, command: &str) -> Result<String> {
    let before = log(id).await.len();
    runtime::command(id, command)?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    Ok(log(id).await.chars().skip(before.saturating_sub(2)).collect())
}

fn varint(mut n: u32) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (n & 0x7f) as u8;
        n >>= 7;
        out.push(if n == 0 { byte } else { byte | 0x80 });
        if n == 0 {
            return out;
        }
    }
}

fn text(s: &str) -> Vec<u8> {
    let mut out = varint(s.len() as u32);
    out.extend(s.as_bytes());
    out
}

fn packet(body: Vec<u8>) -> Vec<u8> {
    let mut out = varint(body.len() as u32);
    out.extend(body);
    out
}

/// A 1.20.1 login (no key, and no acknowledgement to send): enough to make the proxy pick a backend and connect to it.
async fn login(port: u16, host: &str, name: &str) -> Result<TcpStream> {
    // Past the proxy's limit on logins from one address.
    tokio::time::sleep(Duration::from_millis(4500)).await;
    let mut s = TcpStream::connect(("127.0.0.1", port)).await?;
    let mut hello = vec![0];
    hello.extend(varint(763));
    hello.extend(text(host));
    hello.extend(port.to_be_bytes());
    hello.extend(varint(2));
    s.write_all(&packet(hello)).await?;
    let mut start = vec![0];
    start.extend(text(name));
    start.push(0);
    s.write_all(&packet(start)).await?;
    Ok(s)
}

async fn reached(listener: &TcpListener) -> bool {
    tokio::time::timeout(Duration::from_secs(20), listener.accept()).await.is_ok_and(|r| r.is_ok())
}

async fn view(id: &str) -> Result<Value> {
    call("server:proxyConfig", vec![json!(id)]).await
}

fn file(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).unwrap_or_default()
}

/// Logs on disk and in memory never hold the secret.
fn scan_logs(dir: &Path, secret: &str) -> Result<()> {
    for e in walk(&dir.join("logs")) {
        let bytes = std::fs::read(&e)?;
        if String::from_utf8_lossy(&bytes).contains(secret) {
            bail!("the secret is in {}", e.display());
        }
    }
    Ok(())
}

fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(dir).map(|r| r.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect()).unwrap_or_default()
}

async fn velocity() -> Result<()> {
    let id = make("proxy-velocity", "velocity", 25731).await?;
    let dir = store::dir(&id)?;
    println!("made {id} in {}", dir.display());
    expect("no config before the first start", view(&id).await?["missing"] == true, "missing")?;
    expect("saving then is refused", call("server:proxySave", vec![json!(id), json!({ "backends": [], "forwarding": "none" })]).await.is_err(), "refused")?;

    start(&id).await?;
    stop(&id).await?;
    let original = file(&dir, "velocity.toml");
    let comments: Vec<String> = original.lines().filter(|l| l.trim_start().starts_with('#')).map(str::to_string).collect();
    let keys_after: Vec<String> = original.split("[advanced]").nth(1).context("[advanced]")?.lines().map(str::to_string).collect();
    println!("default servers: {}", view(&id).await?["backends"]);

    let paper = plant_paper(&id, "planted-lobby", 25732).await?;
    let lobby = TcpListener::bind("127.0.0.1:25732").await?;
    let survival = TcpListener::bind("127.0.0.1:25733").await?;
    let edit = json!({
        "backends": [{ "name": "lobby", "address": "127.0.0.1:25732", "join": true }, { "name": "survival", "address": "127.0.0.1:25733", "join": false }],
        "forcedHosts": [{ "host": "survival.example.com", "servers": ["survival"] }],
        "forwarding": "modern"
    });
    let saved = call("server:proxySave", vec![json!(id), edit]).await?;
    expect("saved", saved["backends"][0]["name"] == "lobby" && saved["forwarding"] == "modern", &saved["backends"])?;
    expect("the local one is recognised", saved["setup"][0]["localId"] == paper.as_str() && saved["setup"][0]["canApply"] == true, &saved["setup"][0])?;
    expect("the other isn't", saved["setup"][1]["advice"] == "remote", &saved["setup"][1]["advice"])?;

    let edited = file(&dir, "velocity.toml");
    let lost: Vec<&String> = comments.iter().filter(|c| !edited.contains(c.as_str())).collect();
    expect("every comment stays", lost.is_empty(), &lost)?;
    expect("[advanced] stays", keys_after.iter().all(|l| edited.contains(l.as_str())), "same lines")?;

    let secret = call("server:proxySecret", vec![json!(id)]).await?.as_str().context("secret")?.to_string();
    expect("a secret exists", !secret.is_empty() && file(&dir, "forwarding.secret").trim() == secret, "not shown")?;

    let applied = call("server:proxyApply", vec![json!(id), json!(paper)]).await?;
    expect("the page never gets the secret", !applied.to_string().contains(&secret), "clean")?;
    expect("checks pass after setup", applied["setup"][0]["onlineModeOff"] == true && applied["setup"][0]["forwardingOk"] == true, &applied["setup"][0])?;
    let back = store::dir(&paper)?;
    let global = file(&back, "config/paper-global.yml");
    expect("Paper's file edited in place", global.contains("# Paper's comment") && global.contains("kept: true") && global.contains("enabled: true") && global.contains(&format!("secret: '{secret}'")), "edited")?;
    expect("spigot.yml keeps bungeecord off", file(&back, "spigot.yml").contains("bungeecord: false") && file(&back, "spigot.yml").contains("# Spigot's comment"), "spigot")?;
    expect("online mode off", file(&back, "server.properties").contains("online-mode=false") && file(&back, "server.properties").contains("# kept"), "props")?;
    expect("sharing off", !store::get(&paper).await?.public, "public=false")?;
    expect("no eula written", !back.join("eula.txt").exists(), "none")?;

    for (path, text) in [("forwarding.secret", json!({ "text": "" })), ("config/paper-global.yml", json!({ "text": global })), ("copy.txt", json!({ "text": format!("x {secret} y") }))] {
        expect(&format!("web console can't read {path}"), proxyconfig::guard_remote_read(path, &text).await.is_err(), "refused")?;
    }
    expect("other files open", proxyconfig::guard_remote_read("velocity.toml", &json!({ "text": edited })).await.is_ok(), "ok")?;

    // Scratch only: a login needs no Mojang session.
    std::fs::write(dir.join("velocity.toml"), file(&dir, "velocity.toml").replace("online-mode = true", "online-mode = false"))?;
    start(&id).await?;
    expect("no config errors", !log(&id).await.contains("not registered"), "clean")?;
    let _first = login(25731, "play.example.com", "Tester").await?;
    expect("try list reaches the default server", reached(&lobby).await, "127.0.0.1:25732")?;
    let _forced = login(25731, "survival.example.com", "Tester2").await?;
    expect("forced host reaches its server", reached(&survival).await, "127.0.0.1:25733")?;

    // A save while it runs waits for the next start.
    let now = view(&id).await?;
    expect("nothing waiting", now["pending"] == false, &now["pending"])?;
    let mut change = json!({ "backends": now["backends"], "forcedHosts": now["forcedHosts"], "forwarding": now["forwarding"] });
    let order = change["backends"].as_array_mut().context("backends")?;
    order.reverse();
    order[0]["join"] = json!(true);
    order.push(json!({ "name": "extra", "address": "10.0.0.5", "join": false }));
    let saved = call("server:proxySave", vec![json!(id), change]).await?;
    expect("a running proxy shows it's waiting", saved["pending"] == true, &saved["pending"])?;
    expect("address got its port", saved["backends"][2]["address"] == "10.0.0.5:25565", &saved["backends"][2])?;
    let _old = login(25731, "play.example.com", "Tester3").await?;
    expect("the running proxy still has the old order", reached(&lobby).await, "127.0.0.1:25732")?;
    stop(&id).await?;
    start(&id).await?;
    expect("restarted, nothing waits", view(&id).await?["pending"] == false, "applied")?;
    let _new = login(25731, "play.example.com", "Tester4").await?;
    expect("the restarted proxy has the new order", reached(&survival).await, "127.0.0.1:25733")?;
    expect("the console log never had the secret", !log(&id).await.contains(&secret), "clean")?;
    stop(&id).await?;
    scan_logs(&dir, &secret)
}

async fn bungee(kind: &str, port: u16) -> Result<()> {
    let id = make(&format!("proxy-{kind}"), kind, port).await?;
    let dir = store::dir(&id)?;
    println!("made {id} in {}", dir.display());
    let (a, b) = (port + 1, port + 2);
    let _lobby = TcpListener::bind(("127.0.0.1", a)).await?;
    let pvp = TcpListener::bind(("127.0.0.1", b)).await?;
    let paper = plant_paper(&id, &format!("planted-{kind}"), a).await?;

    // Before any start: only ResinCraft's starter config.
    let saved = call("server:proxySave", vec![json!(id), json!({
        "backends": [{ "name": "lobby", "address": format!("127.0.0.1:{a}"), "join": true }, { "name": "pvp", "address": format!("127.0.0.1:{b}"), "join": false }],
        "forcedHosts": [{ "host": "pvp.example.com", "servers": ["pvp"] }],
        "forwarding": "legacy"
    })]).await?;
    expect("saved before the first start", saved["backends"].as_array().is_some_and(|l| l.len() == 2), &saved["backends"])?;
    println!("--- written\n{}---", file(&dir, "config.yml"));
    start(&id).await?;
    println!("glist: {}", console(&id, "glist").await?);
    stop(&id).await?;
    let filled = file(&dir, "config.yml");
    println!("--- after its own start\n{filled}---");
    expect("our settings survive its own save", filled.contains(&format!("address: 127.0.0.1:{a}")) && filled.contains("ip_forward: true") && filled.contains("pvp.example.com"), "kept")?;

    // A comment and an unknown key, then an edit, then a start that reads it.
    let marked = format!("# my own note\nmy_unknown_setting: 7\n{}", filled.replace("online_mode: true", "online_mode: false"));
    std::fs::write(dir.join("config.yml"), &marked)?;
    let now = view(&id).await?;
    let mut change = json!({ "backends": now["backends"], "forcedHosts": now["forcedHosts"], "forwarding": now["forwarding"] });
    let order = change["backends"].as_array_mut().context("backends")?;
    order.reverse();
    order[0]["join"] = json!(true);
    order[1]["join"] = json!(true);
    call("server:proxySave", vec![json!(id), change]).await?;
    let edited = file(&dir, "config.yml");
    expect("comment and unknown key stay", edited.contains("# my own note") && edited.contains("my_unknown_setting: 7"), "kept")?;
    start(&id).await?;
    let _first = login(port, "play.example.com", "Tester").await?;
    expect("the new first server gets the login", reached(&pvp).await, b)?;
    drop(_first);
    let _forced = login(port, "pvp.example.com", "Tester2").await?;
    expect("forced host reaches its server", reached(&pvp).await, b)?;
    stop(&id).await?;

    let applied = call("server:proxyApply", vec![json!(id), json!(paper)]).await?;
    let spigot = file(&store::dir(&paper)?, "spigot.yml");
    expect("setup turns bungeecord on", spigot.contains("bungeecord: true") && applied["setup"].to_string().contains("\"forwardingOk\":true"), &applied["setup"])?;
    Ok(())
}

/// Starts and reads the servers: its config is the same shape, but no Bedrock client is sent at it.
async fn waterdog() -> Result<()> {
    let id = make("proxy-waterdog", "waterdogpe", 25761).await?;
    let dir = store::dir(&id)?;
    let saved = call("server:proxySave", vec![json!(id), json!({ "backends": [{ "name": "lobby", "address": "127.0.0.1:25762", "join": true }], "forcedHosts": [], "forwarding": "none" })]).await?;
    expect("saved", saved["backends"][0]["address"] == "127.0.0.1:25762", &saved["backends"])?;
    start(&id).await?;
    stop(&id).await?;
    let filled = file(&dir, "config.yml");
    println!("--- after its own start\n{filled}---");
    expect("it kept our server", filled.contains("127.0.0.1:25762"), "kept")
}

async fn all(words: &[String]) -> Result<()> {
    let wants = |w: &str| words.len() < 2 || words.iter().any(|x| x == w);
    if wants("velocity") {
        velocity().await?;
    }
    if wants("bungeecord") {
        bungee("bungeecord", 25741).await?;
    }
    if wants("waterfall") {
        bungee("waterfall", 25751).await?;
    }
    if wants("waterdogpe") {
        waterdog().await?;
    }
    Ok(())
}

/// The engine's request handler is one big future: more stack than a thread gets by default.
const STACK: usize = 64 * 1024 * 1024;

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().thread_stack_size(STACK).build().expect("Tokio runtime");
    let main = std::thread::Builder::new().stack_size(STACK).spawn(move || runtime.block_on(check())).expect("main thread");
    let _ = main.join();
}

async fn check() {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let Some(folder) = words.first() else {
        eprintln!("usage: check_proxyconfig <empty folder> [velocity] [bungeecord] [waterfall] [waterdogpe]");
        std::process::exit(2);
    };
    std::fs::create_dir_all(folder).unwrap();
    ctx::init(std::path::PathBuf::from(folder), None);
    let done = all(&words).await;
    runtime::shutdown_all(Duration::from_secs(30)).await;
    if let Err(e) = done {
        eprintln!("CHECK FAILED: {e:#}");
        std::process::exit(1);
    }
    println!("all good");
}
