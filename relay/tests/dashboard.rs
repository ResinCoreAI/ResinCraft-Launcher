//! The admin dashboard over plain HTTP on 127.0.0.1 (with [tls] it's the same code behind the same acceptor
//! as the agent port): sign-in, lock-out, CSRF, the allow-list, and that bans, kicks, renames, reserved words
//! and limit changes take effect on live tunnels.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use resincraft_relay::agent::{AgentConfig, AgentError, Command, Connection, Credentials, Event};
use resincraft_relay::handshake::{client_handshake, packet, read_varint};
use resincraft_relay::proto::{ErrorCode, ServiceState};
use resincraft_relay::server::config::{AuthConfig, CustomDomainConfig, DashboardConfig, LimitsConfig};
use tokio::sync::mpsc;
use resincraft_relay::server::{dashboard, Config, Server};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const BASE: &str = "play.localtest.me";
const PASSWORD: &str = "correct horse battery staple";

async fn fake_mojang() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
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
                let ok = String::from_utf8_lossy(&buf).to_lowercase().contains("authorization: bearer token-alice");
                let body = if ok { r#"{"id":"8667ba71b85a4004af54457a9734eed7","name":"Alice"}"# } else { "" };
                let reply = format!(
                    "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    if ok { "200 OK" } else { "401 Unauthorized" },
                    body.len()
                );
                let _ = sock.write_all(reply.as_bytes()).await;
            });
        }
    });
    format!("http://{addr}/minecraft/profile")
}

/// Answers every handshake with a status reply, enough to count as a server.
async fn fake_minecraft() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { continue };
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                if sock.read(&mut buf).await.unwrap_or(0) > 0 {
                    let _ = sock.write_all(&resincraft_relay::handshake::status_response(r#"{"description":{"text":"real server"}}"#)).await;
                }
            });
        }
    });
    addr
}

struct Relay {
    public: SocketAddr,
    agent_url: String,
    dash: SocketAddr,
    _stop: tokio::sync::oneshot::Sender<()>,
}

async fn start(tag: &str, allow_ips: Vec<String>) -> Relay {
    start_limits(tag, allow_ips, LimitsConfig::default()).await
}

async fn start_limits(tag: &str, allow_ips: Vec<String>, limits: LimitsConfig) -> Relay {
    let dir = std::env::temp_dir().join(format!("resincraft-relay-dash-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let defaults = Config::default();
    let mut cfg = Config {
        base_domain: BASE.into(),
        public_listen: vec!["127.0.0.1:0".into()],
        agent_listen: vec!["127.0.0.1:0".into()],
        admin_listen: None,
        data_file: dir.join("relay.json"),
        auth: AuthConfig { profile_url: fake_mojang().await, ..defaults.auth.clone() },
        custom_domains: CustomDomainConfig { enabled: false, ..defaults.custom_domains.clone() },
        dashboard: DashboardConfig { listen: Some("127.0.0.1:0".into()), password_hash: dashboard::hash_password(PASSWORD).unwrap(), allow_ips, session_hours: 12 },
        limits,
        drain_secs: 1,
        ..defaults
    };
    cfg.validate().unwrap();
    let server = Server::bind(cfg).await.unwrap();
    let (public, agent, dash) = (server.public_addrs()[0], server.agent_addrs()[0], server.dashboard_addr().unwrap());
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(server.run(async move {
        let _ = rx.await;
    }));
    Relay { public, agent_url: format!("tcp://{agent}"), dash, _stop: tx }
}

struct Reply {
    status: u16,
    headers: HashMap<String, String>,
    body: Value,
}

async fn call(addr: SocketAddr, method: &str, path: &str, headers: &[(&str, &str)], body: Option<Value>) -> Reply {
    let mut sock = TcpStream::connect(addr).await.unwrap();
    let body = body.map(|b| b.to_string()).unwrap_or_default();
    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Length: {}\r\n", body.len());
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    req.push_str(&body);
    sock.write_all(req.as_bytes()).await.unwrap();
    let mut raw = Vec::new();
    sock.read_to_end(&mut raw).await.unwrap();
    let text = String::from_utf8_lossy(&raw).to_string();
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let mut lines = head.split("\r\n");
    let status = lines.next().unwrap().split(' ').nth(1).unwrap().parse().unwrap();
    let headers = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string())).collect();
    Reply { status, headers, body: serde_json::from_str(body).unwrap_or(Value::String(body.to_string())) }
}

async fn login_token(addr: SocketAddr) -> String {
    call(addr, "GET", "/api/session", &[], None).await.body["loginToken"].as_str().unwrap().to_string()
}

/// Signs in; returns (cookie header value, CSRF token).
async fn sign_in(addr: SocketAddr) -> (String, String) {
    let token = login_token(addr).await;
    let r = call(addr, "POST", "/api/login", &[], Some(json!({ "password": PASSWORD, "token": token }))).await;
    assert_eq!(r.status, 200, "{:?}", r.body);
    let cookie = r.headers["set-cookie"].split(';').next().unwrap().to_string();
    (cookie, r.body["csrf"].as_str().unwrap().to_string())
}

async fn post(addr: SocketAddr, auth: &(String, String), path: &str, body: Value) -> Reply {
    call(addr, "POST", path, &[("Cookie", &auth.0), ("X-CSRF-Token", &auth.1)], Some(body)).await
}

async fn get(addr: SocketAddr, auth: &(String, String), path: &str) -> Value {
    let r = call(addr, "GET", path, &[("Cookie", &auth.0)], None).await;
    assert_eq!(r.status, 200, "{path}: {:?}", r.body);
    r.body
}

/// A launcher signed in as Alice with `name` online; returns its serve task.
async fn alice_online(relay: &Relay, name: &str, mc: SocketAddr) -> tokio::task::JoinHandle<resincraft_relay::agent::AgentError> {
    let mut conn = Connection::connect(&AgentConfig { url: relay.agent_url.clone(), extra_roots_pem: None, client: "test".into() }).await.unwrap();
    conn.auth(Credentials::Minecraft("token-alice".into())).await.unwrap();
    conn.online(name).await.unwrap();
    tokio::spawn(conn.serve(mc, |_| {}))
}

/// Status ping through the relay: Some(first packet id) or None when the relay closes the connection.
async fn ping(relay: SocketAddr, host: &str) -> Option<i32> {
    let mut sock = TcpStream::connect(relay).await.ok()?;
    let mut out = client_handshake(767, host, 25565, 1);
    out.extend_from_slice(&packet(0, &[]));
    sock.write_all(&out).await.ok()?;
    let mut buf = vec![0u8; 4096];
    let n = tokio::time::timeout(Duration::from_secs(5), sock.read(&mut buf)).await.ok()?.ok()?;
    if n == 0 {
        return None;
    }
    let (_, m) = read_varint(&buf[..n]).ok()??;
    read_varint(&buf[m..n]).ok()?.map(|(id, _)| id)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sign_in_csrf_and_sessions() {
    let relay = start("auth", vec![]).await;
    let d = relay.dash;

    // It lives at /admin/: the bare addresses send the browser there.
    for bare in ["/", "/admin"] {
        let moved = call(d, "GET", bare, &[], None).await;
        assert_eq!((moved.status, moved.headers["location"].as_str()), (308, "/admin/"), "{bare}");
    }
    assert_eq!(call(d, "GET", "/admin/app.js", &[], None).await.status, 200);
    assert_eq!(call(d, "GET", "/admin/api/tunnels", &[], None).await.status, 401);
    let page = call(d, "GET", "/admin/", &[], None).await;
    assert_eq!(page.status, 200);
    assert!(page.headers["content-security-policy"].contains("script-src 'self'"));
    assert_eq!(page.headers["x-frame-options"], "DENY");
    assert_eq!(call(d, "GET", "/api/tunnels", &[], None).await.status, 401);

    let s = call(d, "GET", "/api/session", &[], None).await.body;
    assert_eq!((s["loggedIn"].as_bool(), s["passwordSet"].as_bool()), (Some(false), Some(true)));

    // The one-time token is required, and works once.
    assert_eq!(call(d, "POST", "/api/login", &[], Some(json!({ "password": PASSWORD }))).await.status, 403);
    let token = login_token(d).await;
    let wrong = call(d, "POST", "/api/login", &[], Some(json!({ "password": "nope nope nope", "token": token }))).await;
    assert_eq!((wrong.status, wrong.body["attemptsLeft"].as_u64()), (401, Some(4)));
    assert_eq!(call(d, "POST", "/api/login", &[], Some(json!({ "password": PASSWORD, "token": token }))).await.status, 403);
    // A sign-in posted from another site is refused.
    let token = login_token(d).await;
    let cross = call(d, "POST", "/api/login", &[("Origin", "https://evil.example")], Some(json!({ "password": PASSWORD, "token": token }))).await;
    assert_eq!(cross.status, 403);

    let token = login_token(d).await;
    let ok = call(d, "POST", "/api/login", &[("Origin", &format!("http://{d}"))], Some(json!({ "password": PASSWORD, "token": token }))).await;
    assert_eq!(ok.status, 200);
    let set_cookie = &ok.headers["set-cookie"];
    for part in ["rc_session=", "HttpOnly", "Secure", "SameSite=Strict", "Max-Age=43200", "Path=/"] {
        assert!(set_cookie.contains(part), "{set_cookie} lacks {part}");
    }
    let auth = (set_cookie.split(';').next().unwrap().to_string(), ok.body["csrf"].as_str().unwrap().to_string());
    let s = call(d, "GET", "/api/session", &[("Cookie", &auth.0)], None).await.body;
    assert_eq!((s["loggedIn"].as_bool(), s["csrf"].as_str()), (Some(true), Some(auth.1.as_str())));

    // Every POST needs the CSRF token, and a same-origin Origin when one is sent.
    let ban = json!({ "ip": "198.51.100.7" });
    assert_eq!(call(d, "POST", "/api/bans/add", &[("Cookie", &auth.0)], Some(ban.clone())).await.status, 403);
    assert_eq!(call(d, "POST", "/api/bans/add", &[("Cookie", &auth.0), ("X-CSRF-Token", "wrong")], Some(ban.clone())).await.status, 403);
    let evil = call(d, "POST", "/api/bans/add", &[("Cookie", &auth.0), ("X-CSRF-Token", &auth.1), ("Origin", "https://evil.example")], Some(ban.clone())).await;
    assert_eq!(evil.status, 403);
    assert!(get(d, &auth, "/api/bans").await["ips"].as_array().unwrap().is_empty(), "nothing was banned by the refused requests");
    assert_eq!(post(d, &auth, "/api/bans/add", ban).await.status, 200);

    // The events log has the failed sign-in with its address; tokens never appear anywhere.
    let events = get(d, &auth, "/api/events").await;
    let text = events.to_string();
    assert!(events.as_array().unwrap().iter().any(|e| e["kind"] == "auth-fail" && e["ip"] == "127.0.0.1"), "{text}");
    assert!(!text.contains(PASSWORD));

    assert_eq!(post(d, &auth, "/api/logout", json!({})).await.status, 200);
    assert_eq!(call(d, "GET", "/api/overview", &[("Cookie", &auth.0)], None).await.status, 401);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn locks_out_after_wrong_passwords() {
    let relay = start("lock", vec![]).await;
    for left in (0..5).rev() {
        let token = login_token(relay.dash).await;
        let r = call(relay.dash, "POST", "/api/login", &[], Some(json!({ "password": "wrong password", "token": token }))).await;
        assert_eq!((r.status, r.body["attemptsLeft"].as_u64()), (401, Some(left)));
    }
    let token = login_token(relay.dash).await;
    let locked = call(relay.dash, "POST", "/api/login", &[], Some(json!({ "password": PASSWORD, "token": token }))).await;
    assert_eq!(locked.status, 429, "even the right password waits out the lock");
    assert!(locked.body["retryAfter"].as_u64().unwrap() > 800);
    assert!(locked.headers.contains_key("retry-after"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn allow_list_keeps_others_out() {
    let relay = start("allow", vec!["10.0.0.0/8".into()]).await;
    assert_eq!(call(relay.dash, "GET", "/", &[], None).await.status, 403);
    assert_eq!(call(relay.dash, "GET", "/api/session", &[], None).await.status, 403);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn admin_actions_take_effect() {
    let relay = start("actions", vec![]).await;
    let d = relay.dash;
    let mc = fake_minecraft().await;
    let auth = sign_in(d).await;
    let host = format!("alice-smp.{BASE}");

    // Live tunnels, then Kick: the launcher is told and doesn't come back by itself.
    let serving = alice_online(&relay, "alice-smp", mc).await;
    assert_eq!(ping(relay.public, &host).await, Some(0));
    let tunnels = get(d, &auth, "/api/tunnels").await;
    assert_eq!(tunnels[0]["name"], "alice-smp");
    assert_eq!(tunnels[0]["ownerName"], "Alice");
    assert!(tunnels[0]["bytesIn"].as_u64().unwrap() > 0 && tunnels[0]["bytesOut"].as_u64().unwrap() > 0);
    assert_eq!(post(d, &auth, "/api/tunnels/kick", json!({ "name": "alice-smp" })).await.status, 200);
    let ended = tokio::time::timeout(Duration::from_secs(5), serving).await.unwrap().unwrap();
    assert_eq!(ended.code(), Some(ErrorCode::Kicked));
    assert!(!ended.retryable());
    assert_eq!(post(d, &auth, "/api/tunnels/kick", json!({ "name": "alice-smp" })).await.status, 404);

    // Usage counts the ping's traffic for Alice.
    let usage = get(d, &auth, "/api/usage").await;
    assert_eq!(usage["accounts"][0]["name"], "Alice");
    assert!(usage["accounts"][0]["bytes"].as_u64().unwrap() > 0);

    // Banning the account disconnects it and keeps it out; unbanning lets it back.
    let serving = alice_online(&relay, "alice-smp", mc).await;
    let r = post(d, &auth, "/api/bans/add", json!({ "uuid": "8667ba71b85a4004af54457a9734eed7", "reason": "test", "hours": 1 })).await;
    assert_eq!(r.status, 200);
    let ended = tokio::time::timeout(Duration::from_secs(5), serving).await.unwrap().unwrap();
    assert_eq!(ended.code(), Some(ErrorCode::Banned));
    let bans = get(d, &auth, "/api/bans").await;
    assert_eq!(bans["uuids"][0]["reason"], "test");
    assert!(bans["uuids"][0]["expires"].as_u64().is_some());
    let mut conn = Connection::connect(&AgentConfig { url: relay.agent_url.clone(), extra_roots_pem: None, client: "test".into() }).await.unwrap();
    assert_eq!(conn.auth(Credentials::Minecraft("token-alice".into())).await.unwrap_err().code(), Some(ErrorCode::Banned));
    assert_eq!(post(d, &auth, "/api/bans/remove", json!({ "uuid": "8667ba71-b85a-4004-af54-457a9734eed7" })).await.status, 200);
    let serving = alice_online(&relay, "alice-smp", mc).await;

    // An IP ban: the relay drops the player without a word; lifted, the ping works again.
    assert_eq!(post(d, &auth, "/api/bans/add", json!({ "ip": "127.0.0.0/8" })).await.status, 200);
    assert_eq!(ping(relay.public, &host).await, None);
    assert_eq!(post(d, &auth, "/api/bans/add", json!({ "ip": "not-an-ip" })).await.status, 400);
    assert_eq!(post(d, &auth, "/api/bans/remove", json!({ "ip": "127.0.0.0/8" })).await.status, 200);
    assert_eq!(ping(relay.public, &host).await, Some(0));

    // Limits apply live: one server online per account now.
    let s = post(d, &auth, "/api/settings", json!({ "limits": { "tunnels_per_account": 1 } })).await;
    assert_eq!((s.status, s.body["limits"]["tunnels_per_account"].as_u64(), s.body["limitsOverridden"].as_bool()), (200, Some(1), Some(true)));
    let mut second = Connection::connect(&AgentConfig { url: relay.agent_url.clone(), extra_roots_pem: None, client: "test".into() }).await.unwrap();
    second.auth(Credentials::Minecraft("token-alice".into())).await.unwrap();
    assert_eq!(second.online("alice-two").await.unwrap_err().code(), Some(ErrorCode::TooManyTunnels));
    assert_eq!(post(d, &auth, "/api/settings", json!({ "limits": { "tunnels_per_account": 0 } })).await.status, 400);
    assert_eq!(post(d, &auth, "/api/settings", json!({ "limits": { "nonsense": 1 } })).await.status, 400);
    let reset = post(d, &auth, "/api/settings/reset", json!({})).await;
    assert_eq!((reset.body["limits"]["tunnels_per_account"].as_u64(), reset.body["limitsOverridden"].as_bool()), (Some(3), Some(false)));

    // Reserved words, then renaming and releasing a name.
    assert_eq!(post(d, &auth, "/api/reserved/add", json!({ "word": "forbidden" })).await.status, 200);
    assert_eq!(second.check("forbidden").await.unwrap().reason, Some(ErrorCode::NameReserved));
    assert_eq!(post(d, &auth, "/api/reserved/remove", json!({ "word": "forbidden" })).await.status, 200);
    assert!(second.check("forbidden").await.unwrap().available);
    assert_eq!(post(d, &auth, "/api/names/rename", json!({ "name": "alice-smp", "to": "alice-new" })).await.status, 200);
    let ended = tokio::time::timeout(Duration::from_secs(5), serving).await.unwrap().unwrap();
    assert_eq!(ended.code(), Some(ErrorCode::Released));
    let names = get(d, &auth, "/api/names?q=alice").await;
    assert_eq!(names["names"][0]["name"], "alice-new");
    assert_eq!(names["names"][0]["ownerName"], "Alice");
    assert_eq!(post(d, &auth, "/api/names/release", json!({ "name": "alice-new" })).await.status, 200);
    assert_eq!(get(d, &auth, "/api/names?q=alice").await["names"].as_array().unwrap().len(), 0);

    // Every admin action is in the events log.
    let events = get(d, &auth, "/api/events").await.to_string();
    for what in ["disconnected alice-smp", "banned account", "unbanned IP", "changed limits: tunnels_per_account = 1", "renamed alice-smp to alice-new", "released alice-new"] {
        assert!(events.contains(what), "events lack {what}: {events}");
    }
    let overview = get(d, &auth, "/api/overview").await;
    assert_eq!(overview["baseDomain"], BASE);
}

/// A launcher signed in as Alice; `name` online, serving with a command channel and its events collected.
struct Live {
    commands: mpsc::UnboundedSender<Command>,
    events: mpsc::UnboundedReceiver<Event>,
    serving: tokio::task::JoinHandle<AgentError>,
    secs_left: Option<u64>,
}

async fn alice_live(relay: &Relay, name: &str, mc: SocketAddr) -> Result<Live, AgentError> {
    let mut conn = Connection::connect(&AgentConfig { url: relay.agent_url.clone(), extra_roots_pem: None, client: "test".into() }).await?;
    conn.auth(Credentials::Minecraft("token-alice".into())).await?;
    let online = conn.online(name).await?;
    let (commands, mut rx) = mpsc::unbounded_channel();
    let (ev_tx, events) = mpsc::unbounded_channel();
    let serving = tokio::spawn(async move {
        conn.serve_with(mc, move |e| {
            let _ = ev_tx.send(e);
        }, &mut rx)
        .await
    });
    Ok(Live { commands, events, serving, secs_left: online.secs_left })
}

/// The next event that isn't a traffic report.
async fn next_event(live: &mut Live, wait: Duration) -> Event {
    loop {
        match tokio::time::timeout(wait, live.events.recv()).await.expect("an event in time").expect("events open") {
            Event::Usage(_) => continue,
            e => return e,
        }
    }
}

/// The server list text the relay answers for `host`.
async fn motd(relay: SocketAddr, host: &str) -> String {
    let mut sock = TcpStream::connect(relay).await.unwrap();
    let mut out = client_handshake(767, host, 25565, 1);
    out.extend_from_slice(&packet(0, &[]));
    sock.write_all(&out).await.unwrap();
    let mut buf = vec![0u8; 8192];
    let n = tokio::time::timeout(Duration::from_secs(5), sock.read(&mut buf)).await.unwrap().unwrap();
    String::from_utf8_lossy(&buf[..n]).to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn service_switch_refuses_new_tunnels_and_stops_running_ones() {
    let relay = start("service", vec![]).await;
    let d = relay.dash;
    let mc = fake_minecraft().await;
    let auth = sign_in(d).await;
    let mut live = alice_live(&relay, "alice-smp", mc).await.unwrap();

    // Paused: the running tunnel goes on (and hears about it), new ones are refused with the admin's words.
    let r = post(d, &auth, "/api/service", json!({ "state": "paused", "message": "Maintenance until 20:00" })).await;
    assert_eq!((r.status, r.body["state"].as_str()), (200, Some("paused")));
    assert_eq!(next_event(&mut live, Duration::from_secs(5)).await, Event::Service { state: ServiceState::Paused, message: Some("Maintenance until 20:00".into()) });
    assert_eq!(ping(relay.public, &format!("alice-smp.{BASE}")).await, Some(0), "running tunnels keep forwarding");
    let mut other = Connection::connect(&AgentConfig { url: relay.agent_url.clone(), extra_roots_pem: None, client: "test".into() }).await.unwrap();
    other.auth(Credentials::Minecraft("token-alice".into())).await.unwrap();
    assert_eq!(other.online("alice-two").await.unwrap_err(), AgentError::Refused(ErrorCode::Paused, "Maintenance until 20:00".into()));
    other.reserve("alice-old", None).await.unwrap();
    assert!(motd(relay.public, &format!("alice-old.{BASE}")).await.contains("ResinCraft Relay is paused"));
    assert!(get(d, &auth, "/api/overview").await["service"]["state"] == "paused");

    // Off: the running tunnel is closed at once, with the message.
    assert_eq!(post(d, &auth, "/api/service", json!({ "state": "off", "message": "Maintenance until 20:00" })).await.status, 200);
    let ended = tokio::time::timeout(Duration::from_secs(5), live.serving).await.unwrap().unwrap();
    assert_eq!(ended, AgentError::Refused(ErrorCode::ServiceOff, "Maintenance until 20:00".into()));
    assert!(ended.retryable(), "launchers keep trying, so they come back when it's on again");
    assert_eq!(get(d, &auth, "/api/tunnels").await.as_array().unwrap().len(), 0);
    assert_eq!(post(d, &auth, "/api/service", json!({ "state": "sideways" })).await.status, 400);

    // On again: tunnels work.
    assert_eq!(post(d, &auth, "/api/service", json!({ "state": "on" })).await.status, 200);
    let back = alice_live(&relay, "alice-smp", mc).await.unwrap();
    assert_eq!(ping(relay.public, &format!("alice-smp.{BASE}")).await, Some(0));
    back.serving.abort();
    let events = get(d, &auth, "/api/events").await.to_string();
    for what in ["paused the relay", "switched the relay off (1 tunnels closed)", "switched the relay on", "Maintenance until 20:00"] {
        assert!(events.contains(what), "events lack {what}: {events}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn session_limit_warns_continues_and_ends() {
    // A 3-second session, warned 2 seconds before the end.
    let limits = LimitsConfig { session_minutes: 0.05, session_warn_minutes: vec![2.0 / 60.0], ..LimitsConfig::default() };
    let relay = start_limits("session", vec![], limits).await;
    let d = relay.dash;
    let mc = fake_minecraft().await;
    let auth = sign_in(d).await;
    let host = format!("alice-smp.{BASE}");

    let mut live = alice_live(&relay, "alice-smp", mc).await.unwrap();
    assert_eq!(live.secs_left, Some(3));
    let Event::Warning { secs_left } = next_event(&mut live, Duration::from_secs(5)).await else { panic!("expected a warning") };
    assert!(secs_left <= 2, "{secs_left}");
    live.commands.send(Command::Continue).unwrap();
    let Event::Extended { secs_left, continues } = next_event(&mut live, Duration::from_secs(5)).await else { panic!("expected the extension") };
    assert_eq!(continues, 1);
    assert!((3..=5).contains(&secs_left), "another session, capped at one session plus the warning window: {secs_left}");
    let t = get(d, &auth, "/api/tunnels").await;
    assert_eq!(t[0]["continues"], 1);
    assert!(t[0]["secsLeft"].as_u64().is_some());

    // The admin's Extend reaches the launcher too.
    assert_eq!(post(d, &auth, "/api/tunnels/extend", json!({ "name": "alice-smp" })).await.status, 200);
    loop {
        match next_event(&mut live, Duration::from_secs(5)).await {
            Event::Extended { continues, .. } => {
                assert_eq!(continues, 2);
                break;
            }
            Event::Warning { .. } => continue,
            other => panic!("unexpected {other:?}"),
        }
    }

    // Without another Continue, the session ends and the server list says so.
    let ended = tokio::time::timeout(Duration::from_secs(15), live.serving).await.expect("the session ends").unwrap();
    assert_eq!(ended.code(), Some(ErrorCode::SessionEnded));
    assert!(!ended.retryable());
    assert!(motd(relay.public, &host).await.contains("The host's relay session ended"));

    // End now from the dashboard.
    let again = alice_live(&relay, "alice-smp", mc).await.unwrap();
    assert_eq!(post(d, &auth, "/api/tunnels/end", json!({ "name": "alice-smp" })).await.status, 200);
    let ended = tokio::time::timeout(Duration::from_secs(5), again.serving).await.unwrap().unwrap();
    assert_eq!(ended.code(), Some(ErrorCode::SessionEnded));

    let events = get(d, &auth, "/api/events").await.to_string();
    for what in ["continued alice-smp (1 so far)", "extended alice-smp's session", "alice-smp's relay session ended", "ended alice-smp's session"] {
        assert!(events.contains(what), "events lack {what}: {events}");
    }
}
