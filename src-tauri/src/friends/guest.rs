//! The friend's end: finds a way to the host, then listens on a loopback port the game joins like any server. Every
//! connection the game makes (a server list ping, the join itself) gets its own tunnel.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use futures::stream::{FuturesUnordered, StreamExt};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

use super::code::{Invite, Route};
use super::secure::{self, Keys};
use crate::err_tr;

const DIRECT_TIMEOUT: Duration = Duration::from_secs(6);
/// Direct routes get this head start, so the relay only carries friends who can't connect otherwise.
const RELAY_DELAY: Duration = Duration::from_millis(2500);
/// The port players reach ResinCraft Relay on.
const RELAY_PORT: u16 = 25565;
/// Any recent protocol number: the relay only routes by the address and never reaches a game with it.
const PROTOCOL: u32 = 767;

/// How a dial went: reached the host but it refused the secret, or never reached it.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Miss {
    Rejected,
    Unreachable,
}

fn write_varint(out: &mut Vec<u8>, mut v: u32) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

/// A Minecraft login handshake for `host`: the relay routes by it and sends the rest through unread.
pub fn mc_handshake(host: &str, port: u16) -> Vec<u8> {
    let mut body = vec![0];
    write_varint(&mut body, PROTOCOL);
    write_varint(&mut body, host.len() as u32);
    body.extend_from_slice(host.as_bytes());
    body.extend_from_slice(&port.to_be_bytes());
    body.push(2);
    let mut out = Vec::with_capacity(body.len() + 2);
    write_varint(&mut out, body.len() as u32);
    out.extend_from_slice(&body);
    out
}

/// Where a route leads: (host, port, the handshake to send first).
fn target(route: &Route, relay_domain: Option<&str>) -> Option<(String, u16, Option<Vec<u8>>)> {
    match route {
        Route::Direct(a) => Some((a.ip().to_string(), a.port(), None)),
        Route::Relay(name) => {
            let host = format!("{name}.{}", relay_domain?);
            let first = mc_handshake(&host, RELAY_PORT);
            Some((host, RELAY_PORT, Some(first)))
        }
    }
}

/// Connects along one route and proves this side holds the code.
async fn dial(route: Route, relay_domain: Option<&str>, secret: [u8; 12]) -> Result<(Route, TcpStream, Keys), Miss> {
    let (host, port, first) = target(&route, relay_domain).ok_or(Miss::Unreachable)?;
    let mut io = match tokio::time::timeout(DIRECT_TIMEOUT, TcpStream::connect((host.as_str(), port))).await {
        Ok(Ok(io)) => io,
        _ => return Err(Miss::Unreachable),
    };
    let _ = io.set_nodelay(true);
    let shaken = tokio::time::timeout(secure::HANDSHAKE_TIMEOUT, async {
        if let Some(first) = first {
            io.write_all(&first).await?;
        }
        secure::connect(&mut io, &secret).await
    })
    .await;
    match shaken {
        Ok(Ok(keys)) => Ok((route, io, keys)),
        // Something answers there, but not with this code: an ended invite, or a stranger's service on that address.
        _ => Err(Miss::Rejected),
    }
}

/// All routes at once; the first to finish the handshake wins.
async fn race(routes: Vec<Route>, relay_domain: Option<String>, secret: [u8; 12]) -> Result<(Route, TcpStream, Keys), Miss> {
    let relay_domain = relay_domain.as_deref();
    let mut dials: FuturesUnordered<_> = routes
        .into_iter()
        .map(|r| async move {
            if matches!(r, Route::Relay(_)) {
                tokio::time::sleep(RELAY_DELAY).await;
            }
            dial(r, relay_domain, secret).await
        })
        .collect();
    let mut worst = Miss::Unreachable;
    while let Some(result) = dials.next().await {
        match result {
            Ok(won) => return Ok(won),
            Err(Miss::Rejected) => worst = Miss::Rejected,
            Err(Miss::Unreachable) => {}
        }
    }
    Err(worst)
}

/// An open way into a friend's world.
pub struct Joined {
    pub local_port: u16,
    pub route: Route,
    pub stop: CancellationToken,
    pub active: Arc<AtomicU32>,
}

fn explain(miss: Miss) -> anyhow::Error {
    match miss {
        Miss::Rejected => err_tr!("friends.rejected", "Your friend's computer didn't accept this code. The invite may have ended, so ask for a new code."),
        Miss::Unreachable => err_tr!(
            "friends.unreachable",
            "Couldn't reach your friend. Their world may be closed, or their router and yours don't allow a direct connection and their invite has no ResinCraft Relay."
        ),
    }
}

/// Connects to the host once to prove the code works, then opens the loopback port.
pub async fn open(invite: Invite, relay_domain: Option<String>) -> Result<Joined> {
    if (invite.expires as i64) * 1000 <= crate::core::fsutil::now_ms() {
        crate::bail_tr!("friends.expired", "This code has expired. Ask your friend for a new one.");
    }
    let secret = invite.secret;
    let (route, _probe, _keys) = race(invite.routes.clone(), relay_domain.clone(), secret).await.map_err(explain)?;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let local_port = listener.local_addr()?.port();
    let stop = CancellationToken::new();
    let active = Arc::new(AtomicU32::new(0));
    let best = Arc::new(Mutex::new(route.clone()));
    let (s, a) = (stop.clone(), active.clone());
    tokio::spawn(async move {
        loop {
            let game = tokio::select! {
                _ = s.cancelled() => return,
                got = listener.accept() => match got {
                    Ok((game, _)) => game,
                    Err(_) => continue,
                },
            };
            let _ = game.set_nodelay(true);
            let (routes, domain, s, a, best) = (invite.routes.clone(), relay_domain.clone(), s.clone(), a.clone(), best.clone());
            tokio::spawn(async move {
                let route = best.lock().unwrap().clone();
                let dialed = match dial(route, domain.as_deref(), secret).await {
                    Ok(d) => Ok(d),
                    // The way that worked is gone (a network change): look again.
                    Err(_) => race(routes, domain, secret).await,
                };
                let Ok((route, io, keys)) = dialed else { return };
                *best.lock().unwrap() = route;
                a.fetch_add(1, Ordering::SeqCst);
                tokio::select! {
                    _ = secure::pump(io, game, keys) => {}
                    _ = s.cancelled() => {}
                }
                a.fetch_sub(1, Ordering::SeqCst);
            });
        }
    });
    Ok(Joined { local_port, route, stop, active })
}

/// What the player sees as the way in.
pub fn describe(route: &Route) -> &'static str {
    match route {
        Route::Relay(_) => "relay",
        Route::Direct(SocketAddr::V4(a)) if a.ip().is_private() || a.ip().is_loopback() => "lan",
        Route::Direct(_) => "direct",
    }
}

#[cfg(test)]
mod tests {
    use super::super::host::{self, Gate};
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    const SECRET: [u8; 12] = [9; 12];

    /// A stand-in for the integrated server: echoes, upper-cased, so a reply proves it went through it.
    async fn game() -> u16 {
        let l = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = l.accept().await {
                tokio::spawn(async move {
                    let mut b = [0u8; 4096];
                    while let Ok(n) = s.read(&mut b).await {
                        if n == 0 || s.write_all(&b[..n].to_ascii_uppercase()).await.is_err() {
                            break;
                        }
                    }
                });
            }
        });
        port
    }

    async fn gate_on(lan_port: u16, from_relay: bool) -> (Arc<Gate>, u16) {
        let gate = Gate::new(SECRET, lan_port);
        let l = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = l.local_addr().unwrap().port();
        tokio::spawn(host::serve(gate.clone(), l, from_relay));
        (gate, port)
    }

    fn invite(port: u16, secret: [u8; 12]) -> Invite {
        Invite { secret, expires: u32::MAX, routes: vec![Route::Direct(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))] }
    }

    async fn talk(port: u16) -> std::io::Result<[u8; 5]> {
        let mut c = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).await?;
        c.write_all(b"hello").await?;
        let mut b = [0u8; 5];
        tokio::time::timeout(Duration::from_secs(3), c.read_exact(&mut b)).await??;
        Ok(b)
    }

    #[tokio::test]
    async fn a_friend_reaches_the_game_through_the_tunnel() {
        let (gate, port) = gate_on(game().await, false).await;
        let joined = open(invite(port, SECRET), None).await.unwrap();
        assert_eq!(&talk(joined.local_port).await.unwrap(), b"HELLO");
        assert_eq!(&talk(joined.local_port).await.unwrap(), b"HELLO", "every connection gets a tunnel");
        joined.stop.cancel();
        gate.stop.cancel();
    }

    #[tokio::test]
    async fn the_wrong_secret_gets_nothing_and_expired_codes_stop_early() {
        let (_gate, port) = gate_on(game().await, false).await;
        let e = open(invite(port, [1; 12]), None).await.err().unwrap();
        assert!(e.to_string().contains("didn't accept"), "{e}");
        let mut old = invite(port, SECRET);
        old.expires = 1000;
        assert!(open(old, None).await.err().unwrap().to_string().contains("expired"));
    }

    #[tokio::test]
    async fn stopping_disconnects_and_closes_the_code() {
        let (gate, port) = gate_on(game().await, false).await;
        let joined = open(invite(port, SECRET), None).await.unwrap();
        let mut c = TcpStream::connect((Ipv4Addr::LOCALHOST, joined.local_port)).await.unwrap();
        c.write_all(b"hello").await.unwrap();
        let mut b = [0u8; 5];
        c.read_exact(&mut b).await.unwrap();
        assert_eq!(gate.connected.load(Ordering::SeqCst), 1);
        gate.stop.cancel();
        let mut rest = [0u8; 1];
        let closed = tokio::time::timeout(Duration::from_secs(3), c.read(&mut rest)).await.unwrap();
        assert!(matches!(closed, Ok(0) | Err(_)), "the game connection must drop");
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(open(invite(port, SECRET), None).await.is_err(), "the code still works");
        assert_eq!(gate.connected.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn expiry_keeps_those_playing_but_lets_nobody_new_in() {
        let (gate, port) = gate_on(game().await, false).await;
        let joined = open(invite(port, SECRET), None).await.unwrap();
        let mut c = TcpStream::connect((Ipv4Addr::LOCALHOST, joined.local_port)).await.unwrap();
        c.write_all(b"hello").await.unwrap();
        let mut b = [0u8; 5];
        c.read_exact(&mut b).await.unwrap();
        gate.accepting.cancel();
        tokio::time::sleep(Duration::from_millis(100)).await;
        c.write_all(b"again").await.unwrap();
        c.read_exact(&mut b).await.unwrap();
        assert_eq!(&b, b"AGAIN");
        assert!(open(invite(port, SECRET), None).await.is_err());
    }

    /// The relay hands over its routing handshake first; the host's loopback way in reads past it.
    #[tokio::test]
    async fn a_relayed_connection_starts_with_the_handshake() {
        let (_gate, port) = gate_on(game().await, true).await;
        let mut io = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).await.unwrap();
        io.write_all(&mc_handshake("friend-abc.rscraft.example", RELAY_PORT)).await.unwrap();
        let keys = secure::connect(&mut io, &SECRET).await.unwrap();
        let (mine, far) = tokio::io::duplex(4096);
        tokio::spawn(secure::pump(io, far, keys));
        let (mut r, mut w) = tokio::io::split(mine);
        w.write_all(b"hello").await.unwrap();
        let mut b = [0u8; 5];
        r.read_exact(&mut b).await.unwrap();
        assert_eq!(&b, b"HELLO");
    }

    /// Reads one length-prefixed Minecraft packet.
    async fn packet(s: &mut TcpStream) -> Option<Vec<u8>> {
        let (mut len, mut shift) = (0usize, 0);
        loop {
            let b = s.read_u8().await.ok()?;
            len |= ((b & 0x7f) as usize) << shift;
            shift += 7;
            if b & 0x80 == 0 {
                break;
            }
        }
        let mut body = vec![0u8; len];
        s.read_exact(&mut body).await.ok()?;
        Some(body)
    }

    /// A stand-in for an Open to LAN world: answers the server list ping, and a login with an encryption request, as an
    /// online-mode world does.
    async fn lan_world() -> u16 {
        let l = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = l.accept().await {
                tokio::spawn(async move {
                    let Some(hello) = packet(&mut s).await else { return };
                    if *hello.last().unwrap() == 2 {
                        let _login_start = packet(&mut s).await;
                        s.write_all(&[2, 1, 0]).await.unwrap();
                        return;
                    }
                    while let Some(p) = packet(&mut s).await {
                        if p[0] == 0 {
                            let json = br#"{"version":{"name":"1.21.1","protocol":767},"players":{"max":8,"online":1},"description":"Steve - New World"}"#;
                            let mut body = vec![0];
                            write_varint(&mut body, json.len() as u32);
                            body.extend_from_slice(json);
                            let mut out = Vec::new();
                            write_varint(&mut out, body.len() as u32);
                            out.extend_from_slice(&body);
                            s.write_all(&out).await.unwrap();
                        } else {
                            let mut out = vec![p.len() as u8];
                            out.extend_from_slice(&p);
                            s.write_all(&out).await.unwrap();
                        }
                    }
                });
            }
        });
        port
    }

    /// The launcher's own server list ping reads the world through the tunnel, and a login reaches its account check.
    #[tokio::test]
    async fn ping_and_login_pass_through_to_the_world() {
        let (gate, port) = gate_on(lan_world().await, false).await;
        let joined = open(invite(port, SECRET), None).await.unwrap();
        let address = format!("127.0.0.1:{}", joined.local_port);
        let status = crate::player::servers::ping_server(&address, Duration::from_secs(5)).await.unwrap();
        assert_eq!(status.version, "1.21.1");
        assert!(status.motd.contains("New World"));

        let mut game = TcpStream::connect((Ipv4Addr::LOCALHOST, joined.local_port)).await.unwrap();
        game.write_all(&mc_handshake("127.0.0.1", joined.local_port)).await.unwrap();
        game.write_all(&[6, 0, 4, b'A', b'l', b'e', b'x']).await.unwrap();
        let reply = tokio::time::timeout(Duration::from_secs(3), packet(&mut game)).await.unwrap().unwrap();
        assert_eq!(reply[0], 1, "the world's encryption request comes back untouched");
        gate.stop.cancel();
    }

    #[test]
    fn the_handshake_matches_the_protocol() {
        let h = mc_handshake("a.b", 25565);
        assert_eq!(h[0] as usize, h.len() - 1, "length prefix");
        assert_eq!(h[1], 0, "packet id");
        assert_eq!(*h.last().unwrap(), 2, "login");
        assert_eq!(&h[4..8], &[3, b'a', b'.', b'b']);
    }
}
