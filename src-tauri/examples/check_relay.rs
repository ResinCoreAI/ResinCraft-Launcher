//! Headless end-to-end check of ResinCraft Relay with a real Minecraft server (no window, VPS or DNS needed):
//!   cargo run --example check_relay -- [--mc 1.21.1] [--port 25596] [--keep] [--verbose]
//!
//! 1. Starts the relay itself on 127.0.0.1 (plain TCP, base domain play.localtest.me) with a fake Mojang
//!    profile service, and points the launcher at it with RESINCRAFT_RELAY_URL / RESINCRAFT_RELAY_DOMAIN.
//! 2. Uses its own data folder (RESIN_DATA_DIR, default: <temp>/resincraft-check-relay) with a fake Microsoft
//!    account, so the real accounts and servers are never touched.
//! 3. Creates a vanilla server (offline mode), sets "How friends connect" to ResinCraft Relay, runs the name
//!    picker (check, reserve) and starts the server; the launcher brings the name online.
//! 4. As a headless Minecraft client, through the relay: a server list ping, then a login (Login Start →
//!    Login Success → Login Acknowledged → the first configuration packet).
//! 5. The relay's admin bans the account for an hour with a reason: the launcher shows until when and why,
//!    the name picker says the same, and nothing reconnects. Unbanned, Start again brings it back.
//! 6. Checks the relay route for the server's own domain (a CNAME), stops the server and checks the relay
//!    then answers "This server is offline" by itself.

use std::io::Read;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use resincraft_lib::core::types::{NewServer, ServerKind};
use resincraft_lib::core::{ctx, events, paths};
use resincraft_lib::player::accounts;
use resincraft_lib::server::{domain, properties, relay, runtime, store};
use resincraft_relay::handshake::{client_handshake, packet, read_varint, write_string, write_varint};
use resincraft_relay::server::{Config, Relay, Server};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const BASE: &str = "play.localtest.me";
const TOKEN: &str = "check-relay-token";
const UUID: &str = "5f2b7e7a3c1d4e8f9a0b1c2d3e4f5a6b";
const NAME: &str = "check-relay";

struct Opts {
    mc: String,
    port: u16,
    keep: bool,
    verbose: bool,
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn main() {
    let mut argv: Vec<String> = std::env::args().skip(1).collect();
    let mut value = |name: &str| -> Option<String> {
        let i = argv.iter().position(|a| a == name)?;
        let v = argv.get(i + 1).cloned();
        argv.drain(i..(i + 2).min(argv.len()));
        v
    };
    let mc = value("--mc").unwrap_or_else(|| "1.21.1".into());
    let port = value("--port").and_then(|p| p.parse().ok()).unwrap_or(25596);
    let opts = Opts { mc, port, keep: argv.iter().any(|a| a == "--keep"), verbose: argv.iter().any(|a| a == "--verbose") };

    // The launcher reads the relay's address from these (the development override of the built-in one).
    let (public, agent) = (free_port(), free_port());
    std::env::set_var("RESINCRAFT_RELAY_URL", format!("tcp://127.0.0.1:{agent}"));
    std::env::set_var("RESINCRAFT_RELAY_DOMAIN", BASE);

    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let code = match rt.block_on(run(opts, public, agent)) {
        Ok(()) => {
            println!("PASS check_relay");
            0
        }
        Err(e) => {
            eprintln!("CHECK FAILED: {e:#}");
            1
        }
    };
    std::process::exit(code);
}

/// Answers GET /minecraft/profile like Mojang for TOKEN, 401 for anything else.
async fn fake_mojang() -> Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { continue };
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 2048];
                while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    match sock.read(&mut chunk).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => buf.extend_from_slice(&chunk[..n]),
                    }
                }
                let ok = String::from_utf8_lossy(&buf).to_lowercase().contains(&format!("authorization: bearer {TOKEN}"));
                let body = if ok { format!(r#"{{"id":"{UUID}","name":"RelayTester"}}"#) } else { String::new() };
                let status = if ok { "200 OK" } else { "401 Unauthorized" };
                let reply = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                let _ = sock.write_all(reply.as_bytes()).await;
            });
        }
    });
    Ok(format!("http://{addr}/minecraft/profile"))
}

async fn run(opts: Opts, public_port: u16, agent_port: u16) -> Result<()> {
    let data_dir = std::env::var_os("RESIN_DATA_DIR").map(Into::into).unwrap_or_else(|| std::env::temp_dir().join("resincraft-check-relay"));
    std::fs::create_dir_all(&data_dir)?;
    println!("data folder: {}", data_dir.display());
    ctx::init(data_dir.clone(), None);
    let verbose = opts.verbose;
    events::set_sink(move |channel, payload| {
        if channel == "serverLog" && verbose {
            for l in payload.get("lines").and_then(Value::as_array).cloned().unwrap_or_default() {
                println!("    | {}", l.get("text").and_then(Value::as_str).unwrap_or_default());
            }
        }
    });

    // 1. The relay.
    let relay_dir = data_dir.join("relay");
    let _ = std::fs::remove_dir_all(&relay_dir);
    let mut cfg = Config {
        base_domain: BASE.into(),
        public_listen: vec![format!("127.0.0.1:{public_port}")],
        agent_listen: vec![format!("127.0.0.1:{agent_port}")],
        admin_listen: None,
        data_file: relay_dir.join("relay.json"),
        drain_secs: 1,
        ..Config::default()
    };
    cfg.auth.profile_url = fake_mojang().await?;
    cfg.custom_domains.enabled = false;
    cfg.validate()?;
    let server = Server::bind(cfg).await?;
    let relay_addr: SocketAddr = server.public_addrs()[0];
    let admin = server.relay();
    let (stop_relay, stopped) = tokio::sync::oneshot::channel::<()>();
    let relay_task = tokio::spawn(server.run(async move {
        let _ = stopped.await;
    }));
    println!("relay: players on {relay_addr}, launchers on 127.0.0.1:{agent_port}");

    // 2. A fake signed-in Microsoft account whose Minecraft token is still valid (no refresh needed).
    let account = json!([{
        "id": "check-relay-account",
        "type": "msa",
        "username": "RelayTester",
        "uuid": UUID,
        "active": true,
        "refreshToken": accounts::seal("unused"),
        "mcToken": accounts::seal(TOKEN),
        "mcTokenExpires": 4_102_444_800_000i64,
    }]);
    tokio::fs::write(paths::accounts_file(), serde_json::to_vec_pretty(&account)?).await?;
    let _ = tokio::fs::remove_file(paths::root().join("relay-sessions.json")).await;

    let result = exercise(&opts, relay_addr, &admin).await;
    let _ = stop_relay.send(());
    let _ = tokio::time::timeout(Duration::from_secs(10), relay_task).await;
    result
}

async fn exercise(opts: &Opts, relay_addr: SocketAddr, admin: &Relay) -> Result<()> {
    // 3. A vanilla server shared through ResinCraft Relay.
    let req = NewServer {
        name: "Check Relay".into(),
        kind: Some(ServerKind::Vanilla),
        mc_version: Some(opts.mc.clone()),
        loader_version: None,
        memory_mb: Some(1024),
        port: Some(opts.port),
        source: None,
    };
    let created = resincraft_lib::server::create::create(req).await?;
    let id = created.server.meta.id.clone();
    println!("created {id} (Minecraft {})", created.server.meta.mc_version);
    let result = with_server(&id, opts, relay_addr, admin).await;
    if runtime::is_running(&id) {
        runtime::kill(&id);
        let _ = runtime::wait_state(&id, &["stopped", "crashed"], Duration::from_secs(20)).await;
    }
    if !opts.keep {
        let _ = tokio::fs::remove_dir_all(store::dir(&id)?).await;
    }
    result
}

async fn with_server(id: &str, _opts: &Opts, relay_addr: SocketAddr, admin: &Relay) -> Result<()> {
    store::update(id, json!({ "tunnel": "resin" })).await?;
    store::accept_eula(id).await?;
    properties::set_values(&store::dir(id)?, &[("online-mode", "false".into()), ("motd", "Hello through ResinCraft Relay".into())]).await?;

    // The name picker.
    let info = relay::info(id).await?;
    println!("relay info: available={} domain={} account={:?} suggested={}", info.available, info.domain, info.account, info.suggested);
    if !info.available || info.account.as_deref() != Some("RelayTester") || info.suggested != "check-relay" {
        bail!("unexpected relay info");
    }
    let reserved = relay::check(id, "www").await?;
    if reserved.available || reserved.reason.is_none() {
        bail!("\"www\" should be reserved");
    }
    let free = relay::check(id, NAME).await?;
    println!("check {NAME}: available={} mine={}", free.available, free.mine);
    if !free.available {
        bail!("{NAME} should be free");
    }
    let meta = relay::set_name(id, NAME).await?;
    if meta.relay_name.as_deref() != Some(NAME) {
        bail!("the name wasn't saved");
    }
    if !relay::check(id, NAME).await?.mine {
        bail!("{NAME} should be ours now");
    }

    // Start; the launcher brings the name online once the server says "Done".
    let started = Instant::now();
    runtime::start(id).await?;
    runtime::wait_state(id, &["running"], Duration::from_secs(300)).await.context("the server didn't start")?;
    println!("server running after {}s", started.elapsed().as_secs());
    let net = wait_for(Duration::from_secs(30), || runtime::net(id).filter(|n| n.state != "opening")).await.ok_or_else(|| anyhow!("no internet state"))?;
    println!("internet: state={} via={:?} address={:?} error={:?}", net.state, net.via, net.address, net.error);
    let address = format!("{NAME}.{BASE}");
    if net.via != Some("resin") || net.address.as_deref() != Some(address.as_str()) {
        bail!("expected to be online at {address} through ResinCraft Relay");
    }

    // 4. A Minecraft client, through the relay.
    let status = ping(relay_addr, &address).await?;
    let protocol = status["version"]["protocol"].as_i64().unwrap_or(0) as i32;
    println!("status through the relay: {} (protocol {protocol}), motd {:?}", status["version"]["name"], motd(&status));
    if !motd(&status).contains("Hello through ResinCraft Relay") {
        bail!("the status didn't come from the server");
    }
    let config_packet = login(relay_addr, &address, protocol, "RelayBot").await?;
    println!("login through the relay: Login Success, then configuration packet 0x{config_packet:02x}");

    // 5. Banned by the relay's admin: told until when and why, and no reconnecting.
    admin.ban_uuid(UUID, Some(("Testing bans".into(), Some(resincraft_relay::proto::now_secs() + 3600))));
    let banned = wait_for(Duration::from_secs(10), || runtime::net(id).filter(|n| n.resin.as_ref().is_some_and(|r| r.banned))).await.ok_or_else(|| anyhow!("the launcher didn't notice the ban"))?;
    let text = banned.error_text.clone().ok_or_else(|| anyhow!("no ban text for the UI"))?;
    println!("banned: state={} {:?} (key {}, params {})", banned.state, text.text, text.key, serde_json::to_string(&text.params)?);
    if banned.state != "failed" || text.key != "hostRelay.bannedReason" || !text.text.contains("until ") || !text.text.contains("(in 60 minutes). Reason: Testing bans") {
        bail!("unexpected ban message");
    }
    if text.params["ban"]["params"]["date"]["dateTime"].as_u64().is_none() || text.params["ban"]["params"]["left"]["timeLeft"].as_u64().is_none() {
        bail!("the UI needs the ban's end as a time");
    }
    let picker = relay::check(id, NAME).await.err().map(|e| e.to_string()).unwrap_or_default();
    println!("name picker while banned: {picker:?}");
    if !picker.contains("Reason: Testing bans") {
        bail!("the name picker should say the account is banned");
    }
    tokio::time::sleep(Duration::from_secs(5)).await;
    if runtime::net(id).is_some_and(|n| n.state != "failed") {
        bail!("the launcher reconnected while banned");
    }
    // Unbanned: Start again brings the name back.
    admin.ban_uuid(UUID, None);
    runtime::relay_restart(id).await;
    let back = wait_for(Duration::from_secs(20), || runtime::net(id).filter(|n| n.state == "open" && n.via == Some("resin"))).await;
    println!("after unbanning: {:?}", back.as_ref().map(|n| n.address.clone()));
    if back.is_none() {
        bail!("Start again didn't bring the name back after the ban");
    }

    // 6. The server's own domain: a CNAME to the relay address.
    store::update(id, json!({ "domain": "play.example.test" })).await?;
    let d = domain::info(id).await?;
    let rec = d.records.first().ok_or_else(|| anyhow!("no DNS records for the relay route"))?;
    println!("domain route={} record: {} {} → {} (or SRV → {})", d.route, rec.kind, rec.name, rec.value, d.alt_records.first().map(|r| r.value.as_str()).unwrap_or("-"));
    if d.route != "resin" || rec.kind != "CNAME" || rec.value != address {
        bail!("unexpected DNS records for the relay route");
    }

    // Stopped: the relay answers by itself.
    runtime::stop(id)?;
    runtime::wait_state(id, &["stopped", "crashed"], Duration::from_secs(120)).await?;
    let mut offline = Value::Null;
    for _ in 0..20 {
        offline = ping(relay_addr, &address).await?;
        if motd(&offline).contains("offline") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    println!("after stopping: {:?}", motd(&offline));
    if !motd(&offline).contains("This server is offline") {
        bail!("the relay should say the server is offline");
    }
    Ok(())
}

async fn wait_for<T>(limit: Duration, mut f: impl FnMut() -> Option<T>) -> Option<T> {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if let Some(v) = f() {
            return Some(v);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    None
}

fn motd(status: &Value) -> String {
    let d = &status["description"];
    if let Some(s) = d.as_str() {
        return s.to_string();
    }
    let mut text = d["text"].as_str().unwrap_or_default().to_string();
    for e in d["extra"].as_array().into_iter().flatten() {
        text.push_str(e["text"].as_str().or_else(|| e.as_str()).unwrap_or_default());
    }
    text
}

// ---------- A tiny Minecraft client ----------

async fn read_varint_from(sock: &mut TcpStream) -> Result<i32> {
    let mut bytes = Vec::new();
    loop {
        bytes.push(sock.read_u8().await?);
        if let Some((v, _)) = read_varint(&bytes).map_err(|e| anyhow!("{e}"))? {
            return Ok(v);
        }
    }
}

/// One packet: (id, body). With compression on, packets carry a data length (0: not compressed).
async fn read_one(sock: &mut TcpStream, compressed: bool) -> Result<(i32, Vec<u8>)> {
    let len = read_varint_from(sock).await? as usize;
    let mut buf = vec![0u8; len];
    sock.read_exact(&mut buf).await?;
    let data = if compressed {
        let (data_len, n) = read_varint(&buf).map_err(|e| anyhow!("{e}"))?.ok_or_else(|| anyhow!("short packet"))?;
        if data_len == 0 {
            buf[n..].to_vec()
        } else {
            let mut out = Vec::with_capacity(data_len as usize);
            flate2::read::ZlibDecoder::new(&buf[n..]).read_to_end(&mut out)?;
            out
        }
    } else {
        buf
    };
    let (id, n) = read_varint(&data).map_err(|e| anyhow!("{e}"))?.ok_or_else(|| anyhow!("short packet"))?;
    Ok((id, data[n..].to_vec()))
}

fn frame(id: i32, body: &[u8], compressed: bool) -> Vec<u8> {
    if !compressed {
        return packet(id, body);
    }
    let mut inner = Vec::new();
    write_varint(&mut inner, 0);
    write_varint(&mut inner, id);
    inner.extend_from_slice(body);
    let mut out = Vec::new();
    write_varint(&mut out, inner.len() as i32);
    out.extend_from_slice(&inner);
    out
}

/// Server list ping: the status JSON (after checking the pong).
async fn ping(relay: SocketAddr, host: &str) -> Result<Value> {
    let mut sock = TcpStream::connect(relay).await?;
    let mut out = client_handshake(767, host, 25565, 1);
    out.extend_from_slice(&packet(0x00, &[]));
    sock.write_all(&out).await?;
    let (id, body) = tokio::time::timeout(Duration::from_secs(10), read_one(&mut sock, false)).await.context("no status")??;
    if id != 0 {
        bail!("expected a status response, got packet 0x{id:02x}");
    }
    let (len, n) = read_varint(&body).map_err(|e| anyhow!("{e}"))?.ok_or_else(|| anyhow!("short status"))?;
    let status: Value = serde_json::from_slice(&body[n..n + len as usize])?;
    sock.write_all(&packet(0x01, &7i64.to_be_bytes())).await?;
    let (id, payload) = tokio::time::timeout(Duration::from_secs(10), read_one(&mut sock, false)).await.context("no pong")??;
    if id != 1 || payload != 7i64.to_be_bytes() {
        bail!("bad pong");
    }
    Ok(status)
}

/// Logs in (offline mode) and returns the id of the first configuration-phase packet the server sends.
async fn login(relay: SocketAddr, host: &str, protocol: i32, name: &str) -> Result<i32> {
    let mut sock = TcpStream::connect(relay).await?;
    let uuid = uuid::Uuid::parse_str(&accounts::offline_uuid(name))?;
    let mut start = Vec::new();
    write_string(&mut start, name);
    match protocol {
        764.. => start.extend_from_slice(uuid.as_bytes()),
        761..=763 => {
            start.push(1);
            start.extend_from_slice(uuid.as_bytes());
        }
        // 1.19.1/1.19.2: no signature data, then the UUID.
        760 => {
            start.extend_from_slice(&[0, 1]);
            start.extend_from_slice(uuid.as_bytes());
        }
        759 => start.push(0),
        _ => {}
    }
    let mut out = client_handshake(protocol, host, 25565, 2);
    out.extend_from_slice(&packet(0x00, &start));
    sock.write_all(&out).await?;

    let mut compressed = false;
    loop {
        let (id, body) = tokio::time::timeout(Duration::from_secs(15), read_one(&mut sock, compressed)).await.context("no login reply")??;
        match id {
            0x03 => compressed = true, // Set Compression
            0x02 => break,             // Login Success
            0x00 => bail!("disconnected during login: {}", String::from_utf8_lossy(&body)),
            0x01 => bail!("the server asked for encryption: is online-mode off?"),
            other => bail!("unexpected login packet 0x{other:02x}"),
        }
    }
    if protocol < 764 {
        return Ok(-1);
    }
    // Login Acknowledged, then the server starts the configuration phase.
    sock.write_all(&frame(0x03, &[], compressed)).await?;
    let (id, _) = tokio::time::timeout(Duration::from_secs(15), read_one(&mut sock, compressed)).await.context("no configuration packet")??;
    Ok(id)
}
