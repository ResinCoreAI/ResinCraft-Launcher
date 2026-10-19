//! End to end on 127.0.0.1: a real relay, the launcher's agent, a fake Mojang profile service and a small
//! fake Minecraft server. Players connect with a made-up base domain; the hostname only has to be in the
//! handshake, so no DNS or hosts entry is needed.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use resincraft_relay::agent::{AgentConfig, AgentError, Connection, Credentials};
use resincraft_relay::handshake::{self, client_handshake, packet, read_packet, read_varint, write_string, write_varint};
use resincraft_relay::proto::ErrorCode;
use resincraft_relay::server::config::{AdminConfig, AuthConfig, CustomDomainConfig, LimitsConfig, TlsConfig};
use resincraft_relay::server::{Config, Server};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const BASE: &str = "play.localtest.me";
const ALICE: &str = "8667ba71b85a4004af54457a9734eed7";
const BOB: &str = "069a79f444e94726a5befca90e38aaf5";

// ---------- Fakes ----------

/// Answers GET /minecraft/profile like Mojang: two known tokens, one account without Minecraft, else 401.
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
                    let Ok(n) = sock.read(&mut chunk).await else { return };
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                }
                let head = String::from_utf8_lossy(&buf).to_lowercase();
                let (status, body) = if head.contains("authorization: bearer token-alice") {
                    ("200 OK", format!(r#"{{"id":"{ALICE}","name":"Alice","skins":[],"capes":[]}}"#))
                } else if head.contains("authorization: bearer token-bob") {
                    ("200 OK", format!(r#"{{"id":"{BOB}","name":"Bob"}}"#))
                } else if head.contains("authorization: bearer token-nogame") {
                    ("404 Not Found", r#"{"error":"NOT_FOUND"}"#.to_string())
                } else {
                    ("401 Unauthorized", String::new())
                };
                let reply = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                let _ = sock.write_all(reply.as_bytes()).await;
            });
        }
    });
    format!("http://{addr}/minecraft/profile")
}

/// Reads one length-prefixed packet: (id, body).
async fn read_one(sock: &mut TcpStream) -> std::io::Result<(i32, Vec<u8>)> {
    let mut len_bytes = Vec::new();
    loop {
        let b = sock.read_u8().await?;
        len_bytes.push(b);
        if let Some((len, _)) = read_varint(&len_bytes).unwrap() {
            let mut body = vec![0u8; len as usize];
            sock.read_exact(&mut body).await?;
            let (id, n) = read_varint(&body).unwrap().unwrap();
            return Ok((id, body[n..].to_vec()));
        }
    }
}

/// A tiny Minecraft server: answers the status ping with "Fake server <tag>" and a login with Login Success.
async fn fake_minecraft(tag: &'static str) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { continue };
            tokio::spawn(async move {
                let Ok((0, body)) = read_one(&mut sock).await else { return };
                let next_state = *body.last().unwrap();
                if next_state == 1 {
                    let Ok((0, _)) = read_one(&mut sock).await else { return };
                    let json = format!(r#"{{"version":{{"name":"1.21.1","protocol":767}},"players":{{"max":20,"online":0}},"description":{{"text":"Fake server {tag}"}}}}"#);
                    let _ = sock.write_all(&handshake::status_response(&json)).await;
                    if let Ok((1, payload)) = read_one(&mut sock).await {
                        let _ = sock.write_all(&handshake::pong(&payload)).await;
                    }
                } else {
                    let Ok((0, login)) = read_one(&mut sock).await else { return };
                    // Login Success: UUID + the name from Login Start (enough for the test).
                    let mut reply = vec![0u8; 16];
                    let (len, n) = read_varint(&login).unwrap().unwrap();
                    write_string(&mut reply, std::str::from_utf8(&login[n..n + len as usize]).unwrap());
                    let _ = sock.write_all(&packet(0x02, &reply)).await;
                    // Echo whatever comes next, to prove the stream keeps flowing both ways.
                    let mut buf = [0u8; 1024];
                    while let Ok(n) = sock.read(&mut buf).await {
                        if n == 0 || sock.write_all(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                }
            });
        }
    });
    addr
}

// ---------- Players ----------

/// Server list ping through `relay` for `host`: the status JSON, after a matching pong.
async fn ping(relay: SocketAddr, host: &str) -> Value {
    let mut sock = TcpStream::connect(relay).await.unwrap();
    let mut out = client_handshake(767, host, 25565, 1);
    out.extend_from_slice(&packet(0x00, &[]));
    sock.write_all(&out).await.unwrap();
    let (id, body) = tokio::time::timeout(Duration::from_secs(10), read_one(&mut sock)).await.expect("status in time").unwrap();
    assert_eq!(id, 0);
    let (len, n) = read_varint(&body).unwrap().unwrap();
    let json: Value = serde_json::from_slice(&body[n..n + len as usize]).unwrap();
    sock.write_all(&packet(0x01, &42i64.to_be_bytes())).await.unwrap();
    let (id, payload) = read_one(&mut sock).await.unwrap();
    assert_eq!((id, payload.as_slice()), (1, &42i64.to_be_bytes()[..]));
    json
}

/// Login Start through `relay`: the first packet back (id, body).
async fn login(relay: SocketAddr, host: &str, name: &str) -> (i32, Vec<u8>, TcpStream) {
    let mut sock = TcpStream::connect(relay).await.unwrap();
    let mut out = client_handshake(767, host, 25565, 2);
    let mut start = Vec::new();
    write_string(&mut start, name);
    start.extend_from_slice(&[0u8; 16]);
    out.extend_from_slice(&packet(0x00, &start));
    sock.write_all(&out).await.unwrap();
    let (id, body) = tokio::time::timeout(Duration::from_secs(10), read_one(&mut sock)).await.expect("reply in time").unwrap();
    (id, body, sock)
}

fn description(status: &Value) -> String {
    let d = &status["description"];
    let mut text = d["text"].as_str().unwrap_or_default().to_string();
    for e in d["extra"].as_array().into_iter().flatten() {
        text.push_str(e["text"].as_str().unwrap_or_default());
    }
    text
}

// ---------- Setup ----------

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("resincraft-relay-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn config(tag: &str) -> Config {
    let defaults = Config::default();
    let mut cfg = Config {
        base_domain: BASE.into(),
        public_listen: vec!["127.0.0.1:0".into()],
        agent_listen: vec!["127.0.0.1:0".into()],
        admin_listen: Some("127.0.0.1:0".into()),
        data_file: temp_dir(tag).join("relay.json"),
        auth: AuthConfig { profile_url: fake_mojang().await, ..defaults.auth.clone() },
        admin: AdminConfig { tokens: vec!["test-admin-token-0123456789abcdef".into()] },
        custom_domains: CustomDomainConfig { enabled: false, ..defaults.custom_domains.clone() },
        limits: LimitsConfig { names_per_account: 2, tunnels_per_account: 2, ..defaults.limits.clone() },
        drain_secs: 1,
        ..defaults
    };
    cfg.validate().unwrap();
    cfg
}

struct Running {
    public: SocketAddr,
    agent_url: String,
    admin: SocketAddr,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    done: tokio::task::JoinHandle<anyhow::Result<()>>,
}

async fn start(cfg: Config) -> Running {
    let tls = cfg.tls.is_some();
    let server = Server::bind(cfg).await.unwrap();
    let public = server.public_addrs()[0];
    let agent = server.agent_addrs()[0];
    let admin = server.admin_addr().unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let done = tokio::spawn(server.run(async move {
        let _ = rx.await;
    }));
    let agent_url = if tls { format!("tls://localhost:{}", agent.port()) } else { format!("tcp://{agent}") };
    Running { public, agent_url, admin, stop: Some(tx), done }
}

fn agent_cfg(url: &str) -> AgentConfig {
    AgentConfig { url: url.to_string(), extra_roots_pem: None, client: "relay-test".into() }
}

async fn http(addr: SocketAddr, method: &str, path: &str, token: Option<&str>, body: &str) -> (u16, String) {
    let mut sock = TcpStream::connect(addr).await.unwrap();
    let auth = token.map(|t| format!("Authorization: Bearer {t}\r\n")).unwrap_or_default();
    let req = format!("{method} {path} HTTP/1.1\r\nHost: x\r\n{auth}Content-Length: {}\r\n\r\n{body}", body.len());
    sock.write_all(req.as_bytes()).await.unwrap();
    let mut out = String::new();
    sock.read_to_string(&mut out).await.unwrap();
    let code = out.split(' ').nth(1).unwrap().parse().unwrap();
    (code, out.split("\r\n\r\n").nth(1).unwrap_or_default().to_string())
}

// ---------- Tests ----------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn players_reach_the_host_through_the_relay() {
    let relay = start(config("flow").await).await;
    let mc = fake_minecraft("A").await;

    // Nothing online yet: the relay answers by itself.
    let free = ping(relay.public, &format!("nobody-here.{BASE}")).await;
    assert!(description(&free).contains("No server here yet"), "{free}");
    assert_eq!(free["version"]["protocol"], 767);
    let unknown = ping(relay.public, "example.org").await;
    assert!(description(&unknown).contains("Unknown address"));

    // Sign in: a bad token, an account without Minecraft, then Alice.
    let mut conn = Connection::connect(&agent_cfg(&relay.agent_url)).await.unwrap();
    assert_eq!(conn.base_domain, BASE);
    assert_eq!(conn.auth(Credentials::Minecraft("wrong".into())).await.unwrap_err().code(), Some(ErrorCode::AuthFailed));
    assert_eq!(conn.auth(Credentials::Minecraft("token-nogame".into())).await.unwrap_err().code(), Some(ErrorCode::NotOwned));
    let authed = conn.auth(Credentials::Minecraft("token-alice".into())).await.unwrap();
    assert_eq!((authed.uuid.as_str(), authed.username.as_str()), ("8667ba71-b85a-4004-af54-457a9734eed7", "Alice"));
    let session = authed.session.expect("a session token");

    // The name picker.
    let st = conn.check("alice-smp").await.unwrap();
    assert!(st.available && !st.mine);
    assert_eq!(conn.check("www").await.unwrap().reason, Some(ErrorCode::NameReserved));
    assert_eq!(conn.check("No_Way").await.unwrap().reason, Some(ErrorCode::NameInvalid));
    assert_eq!(conn.reserve("alice-old", None).await.unwrap(), format!("alice-old.{BASE}"));
    // Reserved but offline.
    let offline = ping(relay.public, &format!("alice-old.{BASE}")).await;
    assert!(description(&offline).contains("This server is offline"), "{offline}");
    let (id, body, _) = login(relay.public, &format!("alice-old.{BASE}"), "Steve").await;
    assert_eq!(id, 0, "login disconnect");
    assert!(String::from_utf8_lossy(&body).contains("This server is offline"));

    // Online: renaming frees the old name in the same step (two names allowed).
    assert_eq!(conn.reserve("alice-smp", Some("alice-old")).await.unwrap(), format!("alice-smp.{BASE}"));
    assert!(conn.check("alice-old").await.unwrap().available);
    let online = conn.online("alice-smp").await.unwrap();
    assert_eq!(online.address, format!("alice-smp.{BASE}"));
    assert_eq!(online.limits.names, 2);
    let serving = tokio::spawn(conn.serve(mc, |_| {}));

    // Status ping and login through the relay, with a Forge marker, a trailing dot and capitals.
    let status = ping(relay.public, &format!("Alice-SMP.{BASE}.\0FML3\0")).await;
    assert_eq!(description(&status), "Fake server A");
    let (id, body, mut sock) = login(relay.public, &format!("alice-smp.{BASE}"), "Steve").await;
    assert_eq!(id, 2, "Login Success from the real server");
    assert!(String::from_utf8_lossy(&body).contains("Steve"));
    sock.write_all(b"after login").await.unwrap();
    let mut echo = [0u8; 11];
    sock.read_exact(&mut echo).await.unwrap();
    assert_eq!(&echo, b"after login");
    drop(sock);

    // Bob can't take Alice's name; a resumed session can't either. Names per account are limited.
    let mut bob = Connection::connect(&agent_cfg(&relay.agent_url)).await.unwrap();
    bob.auth(Credentials::Minecraft("token-bob".into())).await.unwrap();
    assert_eq!(bob.online("alice-smp").await.unwrap_err().code(), Some(ErrorCode::NameTaken));
    assert!(!bob.check("alice-smp").await.unwrap().available);
    bob.reserve("bob-1", None).await.unwrap();
    bob.reserve("bob-2", None).await.unwrap();
    assert_eq!(bob.reserve("bob-3", None).await.unwrap_err().code(), Some(ErrorCode::TooManyNames));
    bob.release("bob-2").await.unwrap();
    bob.reserve("bob-3", None).await.unwrap();

    let mut again = Connection::connect(&agent_cfg(&relay.agent_url)).await.unwrap();
    assert_eq!(again.auth(Credentials::Session("not-a-session".into())).await.unwrap_err().code(), Some(ErrorCode::SessionExpired));
    let resumed = again.auth(Credentials::Session(session.clone())).await.unwrap();
    assert_eq!(resumed.username, "Alice");
    assert!(resumed.session.is_none());
    assert!(again.check("alice-smp").await.unwrap().mine);

    // Bringing the same name online elsewhere replaces the first connection.
    let mc_b = fake_minecraft("B").await;
    again.online("alice-smp").await.unwrap();
    let ended = tokio::time::timeout(Duration::from_secs(5), serving).await.expect("first agent kicked").unwrap();
    assert_eq!(ended.code(), Some(ErrorCode::Replaced));
    let serving = tokio::spawn(again.serve(mc_b, |_| {}));
    assert_eq!(description(&ping(relay.public, &format!("alice-smp.{BASE}")).await), "Fake server B");

    // Legacy ping and garbage.
    let mut old = TcpStream::connect(relay.public).await.unwrap();
    old.write_all(&[0xFE, 0x01]).await.unwrap();
    assert_eq!(old.read_u8().await.unwrap(), 0xFF);
    let mut junk = TcpStream::connect(relay.public).await.unwrap();
    junk.write_all(b"GET / HTTP/1.1\r\nHost: alice-smp.play.localtest.me\r\nAccept: */*\r\nUser-Agent: curl/8\r\n\r\n").await.unwrap();
    let mut rest = Vec::new();
    let n = tokio::time::timeout(Duration::from_secs(10), junk.read_to_end(&mut rest)).await.expect("closed").unwrap_or(0);
    assert_eq!(n, 0, "nothing is sent back to junk");

    // Admin API: health, metrics, auth, ban (kicks the tunnel), release.
    assert_eq!(http(relay.admin, "GET", "/healthz", None, "").await.0, 200);
    let (_, metrics) = http(relay.admin, "GET", "/metrics", None, "").await;
    assert!(metrics.contains("resincraft_relay_tunnels_online 1"), "{metrics}");
    assert!(metrics.contains("resincraft_relay_player_forwarded_total 3"), "{metrics}");
    assert_eq!(http(relay.admin, "GET", "/admin/tunnels", None, "").await.0, 401);
    let token = Some("test-admin-token-0123456789abcdef");
    let (code, tunnels) = http(relay.admin, "GET", "/admin/tunnels", token, "").await;
    assert_eq!(code, 200);
    assert!(tunnels.contains("alice-smp") && tunnels.contains("Alice"), "{tunnels}");
    let (code, _) = http(relay.admin, "POST", "/admin/ban", token, &format!(r#"{{"uuid":"{ALICE}","reason":"test"}}"#)).await;
    assert_eq!(code, 200);
    let ended = tokio::time::timeout(Duration::from_secs(5), serving).await.expect("banned agent kicked").unwrap();
    assert_eq!(ended.code(), Some(ErrorCode::Banned));
    let mut banned = Connection::connect(&agent_cfg(&relay.agent_url)).await.unwrap();
    assert_eq!(banned.auth(Credentials::Session(session.clone())).await.unwrap_err().code(), Some(ErrorCode::Banned));
    assert_eq!(http(relay.admin, "POST", "/admin/unban", token, &format!(r#"{{"uuid":"{ALICE}"}}"#)).await.0, 200);
    assert_eq!(http(relay.admin, "POST", "/admin/release", token, r#"{"name":"bob-1"}"#).await.0, 200);
    assert!(bob.check("bob-1").await.unwrap().available && !bob.check("bob-1").await.unwrap().mine);

    // Shutdown tells connected launchers to come back, then saves the reservations.
    let mut back = Connection::connect(&agent_cfg(&relay.agent_url)).await.unwrap();
    back.auth(Credentials::Session(session)).await.unwrap();
    back.online("alice-smp").await.unwrap();
    let serving = tokio::spawn(back.serve(mc, |_| {}));
    let Running { stop, done, .. } = relay;
    stop.unwrap().send(()).unwrap();
    let ended = tokio::time::timeout(Duration::from_secs(5), serving).await.expect("told to reconnect").unwrap();
    assert_eq!(ended, AgentError::Refused(ErrorCode::ShuttingDown, "the relay is restarting".into()));
    tokio::time::timeout(Duration::from_secs(10), done).await.expect("stopped").unwrap().unwrap();
    let saved = std::fs::read_to_string(temp_dir_path("flow")).unwrap();
    assert!(saved.contains("alice-smp") && saved.contains("bob-3") && !saved.contains("token-alice"), "{saved}");
}

fn temp_dir_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("resincraft-relay-test-{tag}-{}", std::process::id())).join("relay.json")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn monthly_traffic_cap() {
    let mut cfg = config("quota").await;
    cfg.limits.monthly_gb_per_account = 0.000_000_1; // 100 bytes
    let relay = start(cfg).await;
    let mc = fake_minecraft("Q").await;
    let mut conn = Connection::connect(&agent_cfg(&relay.agent_url)).await.unwrap();
    conn.auth(Credentials::Minecraft("token-alice".into())).await.unwrap();
    conn.online("capped").await.unwrap();
    let _serving = tokio::spawn(conn.serve(mc, |_| {}));
    // The first ping goes through and uses up the 100 bytes; the next one is answered by the relay.
    assert_eq!(description(&ping(relay.public, &format!("capped.{BASE}")).await), "Fake server Q");
    tokio::time::sleep(Duration::from_millis(200)).await;
    let capped = ping(relay.public, &format!("capped.{BASE}")).await;
    assert!(description(&capped).contains("relay limit"), "{capped}");
    let mut other = Connection::connect(&agent_cfg(&relay.agent_url)).await.unwrap();
    other.auth(Credentials::Minecraft("token-alice".into())).await.unwrap();
    assert_eq!(other.online("capped-2").await.unwrap_err().code(), Some(ErrorCode::OverQuota));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agent_port_over_tls() {
    let dir = temp_dir("tls");
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    std::fs::write(dir.join("cert.pem"), cert.cert.pem()).unwrap();
    std::fs::write(dir.join("key.pem"), cert.key_pair.serialize_pem()).unwrap();
    let mut cfg = config("tls-data").await;
    cfg.tls = Some(TlsConfig { cert: dir.join("cert.pem"), key: dir.join("key.pem") });
    let relay = start(cfg).await;
    let mc = fake_minecraft("T").await;

    // Without trusting the test certificate, the launcher refuses to talk to it.
    assert!(matches!(Connection::connect(&agent_cfg(&relay.agent_url)).await, Err(AgentError::Network(_))));

    let mut trusted = agent_cfg(&relay.agent_url);
    trusted.extra_roots_pem = Some(cert.cert.pem().into_bytes());
    let mut conn = Connection::connect(&trusted).await.unwrap();
    conn.auth(Credentials::Minecraft("token-bob".into())).await.unwrap();
    conn.online("bob-tls").await.unwrap();
    let _serving = tokio::spawn(conn.serve(mc, |_| {}));
    assert_eq!(description(&ping(relay.public, &format!("bob-tls.{BASE}")).await), "Fake server T");
}

#[test]
fn varint_helpers_round_trip() {
    let mut v = Vec::new();
    write_varint(&mut v, 300);
    assert_eq!(read_packet(&packet(5, &v), 64).unwrap().unwrap().0, 5);
}

/// A Bedrock server stand-in: answers every datagram with "echo:" + it, from its own UDP port.
async fn fake_bedrock() -> SocketAddr {
    let sock = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = sock.local_addr().unwrap();
    tokio::spawn(async move {
        let mut buf = [0u8; 2048];
        loop {
            let Ok((n, from)) = sock.recv_from(&mut buf).await else { continue };
            let mut reply = b"echo:".to_vec();
            reply.extend_from_slice(&buf[..n]);
            reply.extend_from_slice(format!("|{}", from.port()).as_bytes());
            let _ = sock.send_to(&reply, from).await;
        }
    });
    addr
}

/// Sends `packet` from `sock` to the relay's Bedrock port; the answer, if one comes within a second.
async fn bedrock_call(sock: &tokio::net::UdpSocket, port: u16, packet: &[u8]) -> Option<String> {
    sock.send_to(packet, ("127.0.0.1", port)).await.unwrap();
    let mut buf = [0u8; 2048];
    let (n, _) = tokio::time::timeout(Duration::from_secs(2), sock.recv_from(&mut buf)).await.ok()?.ok()?;
    Some(String::from_utf8_lossy(&buf[..n]).into_owned())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bedrock_players_reach_the_host_over_udp() {
    let mut cfg = config("bedrock").await;
    cfg.bedrock_ip = "127.0.0.1".into();
    cfg.bedrock_ports = "39251-39254".into();
    let relay = start(cfg).await;
    let server = fake_bedrock().await;

    let mut conn = Connection::connect(&agent_cfg(&relay.agent_url)).await.unwrap();
    assert!(conn.relay_version >= 3);
    conn.auth(Credentials::Minecraft("token-alice".into())).await.unwrap();
    let online = conn.online_bedrock("alice-be").await.unwrap();
    let port = online.bedrock_port.expect("a Bedrock port");
    assert!((39251..=39254).contains(&port), "{port}");
    assert_eq!(online.address, format!("alice-be.{BASE}"));
    let serving = tokio::spawn(conn.serve(server, |_| {}));

    // A Java player typing the name is told it's a Bedrock server.
    let java = ping(relay.public, &format!("alice-be.{BASE}")).await;
    assert!(description(&java).contains("Bedrock"), "{java}");

    // Two players: an unconnected ping and an open-connection request each reach the server, from different
    // local ports (so the server tells them apart), and the answers come back to the right player.
    let p1 = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let p2 = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let a = bedrock_call(&p1, port, b"\x01ping-one").await.expect("an answer for player 1");
    let b = bedrock_call(&p2, port, b"\x05open-two").await.expect("an answer for player 2");
    assert!(a.starts_with("echo:\x01ping-one|") && b.starts_with("echo:\x05open-two|"), "{a} / {b}");
    assert_ne!(a.rsplit('|').next(), b.rsplit('|').next());
    // Later packets from player 1 go the same way (any packet type once the address is known).
    let again = bedrock_call(&p1, port, b"\x84data").await.unwrap();
    assert_eq!(again.rsplit('|').next(), a.rsplit('|').next());
    // A stray packet from an unknown address opens nothing.
    let p3 = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    assert_eq!(bedrock_call(&p3, port, b"\x84stray").await, None);

    // Coming back online on a new connection keeps the same port.
    serving.abort();
    let mut conn = Connection::connect(&agent_cfg(&relay.agent_url)).await.unwrap();
    conn.auth(Credentials::Minecraft("token-alice".into())).await.unwrap();
    assert_eq!(conn.online_bedrock("alice-be").await.unwrap().bedrock_port, Some(port));
    // Another name gets another port.
    let mut bob = Connection::connect(&agent_cfg(&relay.agent_url)).await.unwrap();
    bob.auth(Credentials::Minecraft("token-bob".into())).await.unwrap();
    let other = bob.online_bedrock("bob-be").await.unwrap().bedrock_port.unwrap();
    assert_ne!(other, port);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bedrock_off_when_no_ports_are_configured() {
    let mut cfg = config("bedrock-off").await;
    cfg.bedrock_ports = String::new();
    let relay = start(cfg).await;
    let mut conn = Connection::connect(&agent_cfg(&relay.agent_url)).await.unwrap();
    conn.auth(Credentials::Minecraft("token-alice".into())).await.unwrap();
    assert_eq!(conn.online_bedrock("no-ports").await.unwrap_err().code(), Some(ErrorCode::BedrockUnavailable));
    // Java tunnels still work.
    conn.online("no-ports").await.unwrap();
}
