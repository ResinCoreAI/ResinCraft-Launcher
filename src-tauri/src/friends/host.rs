//! The host's end: listens for friends, proves they hold the code, and joins each of them to the Open to LAN port.
//! Friends arrive directly (the listening port, through the router or over the LAN) or from ResinCraft Relay, which
//! puts a Minecraft handshake in front of the tunnel so it can route by name; the relay's way in is a loopback port.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use anyhow::{bail, Result};
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use super::code::{Route, SECRET_LEN};
use super::secure;
use crate::server::network;

/// The first ports tried, so someone forwarding one by hand can keep it.
const FIRST_PORT: u16 = 25580;
const PORT_TRIES: u16 = 10;
/// Handshakes under way at once; every one must finish within `secure::HANDSHAKE_TIMEOUT`.
const MAX_OPENING: usize = 8;
const MAX_MC_HANDSHAKE: usize = 1100;

/// What serves one invite.
pub struct Gate {
    pub secret: [u8; SECRET_LEN],
    pub lan_port: u16,
    /// Cancelled when the invite stops: everyone is disconnected.
    pub stop: CancellationToken,
    /// Cancelled when the code expires: nobody new gets in, those playing stay.
    pub accepting: CancellationToken,
    pub connected: AtomicU32,
    pub opening: Semaphore,
}

impl Gate {
    pub fn new(secret: [u8; SECRET_LEN], lan_port: u16) -> Arc<Gate> {
        let stop = CancellationToken::new();
        let accepting = stop.child_token();
        Arc::new(Gate { secret, lan_port, stop, accepting, connected: AtomicU32::new(0), opening: Semaphore::new(MAX_OPENING) })
    }
}

/// Counts one friend in the tunnel for as long as it lives.
struct Connected(Arc<Gate>);

impl Drop for Connected {
    fn drop(&mut self) {
        self.0.connected.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Takes the tunnel from a connection and joins it to the game.
async fn serve_one(gate: Arc<Gate>, mut io: TcpStream, from_relay: bool) -> Result<()> {
    let keys = {
        let Ok(_permit) = gate.opening.try_acquire() else { bail!("too many at once") };
        tokio::time::timeout(secure::HANDSHAKE_TIMEOUT, async {
            if from_relay {
                skip_mc_handshake(&mut io).await?;
            }
            secure::accept(&mut io, &gate.secret).await
        })
        .await??
    };
    let game = TcpStream::connect((Ipv4Addr::LOCALHOST, gate.lan_port)).await?;
    let _ = game.set_nodelay(true);
    gate.connected.fetch_add(1, Ordering::SeqCst);
    let _count = Connected(gate.clone());
    tokio::select! {
        r = secure::pump(io, game, keys) => r,
        _ = gate.stop.cancelled() => Ok(()),
    }
}

async fn read_varint<R: tokio::io::AsyncRead + Unpin>(r: &mut R) -> Result<u32> {
    let mut value = 0u32;
    for i in 0..5 {
        let b = r.read_u8().await?;
        value |= ((b & 0x7f) as u32) << (7 * i);
        if b & 0x80 == 0 {
            return Ok(value);
        }
    }
    bail!("bad VarInt")
}

/// Reads past the Minecraft handshake packet the relay routed by.
async fn skip_mc_handshake<R: tokio::io::AsyncRead + Unpin>(r: &mut R) -> Result<()> {
    let len = read_varint(r).await? as usize;
    if len == 0 || len > MAX_MC_HANDSHAKE {
        bail!("bad handshake");
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).await?;
    Ok(())
}

/// Accepts on `listener` until the code stops working.
pub async fn serve(gate: Arc<Gate>, listener: TcpListener, from_relay: bool) {
    loop {
        tokio::select! {
            _ = gate.accepting.cancelled() => return,
            got = listener.accept() => match got {
                Ok((io, _)) => {
                    let _ = io.set_nodelay(true);
                    let gate = gate.clone();
                    tokio::spawn(async move {
                        if let Err(e) = serve_one(gate, io, from_relay).await {
                            eprintln!("[friends] a connection ended: {e:#}");
                        }
                    });
                }
                Err(e) => eprintln!("[friends] accept: {e}"),
            },
        }
    }
}

/// The direct listeners on one port: IPv4 and, where the computer has it, IPv6.
pub async fn listen() -> Result<(u16, Vec<TcpListener>)> {
    let mut v4 = None;
    for port in (FIRST_PORT..FIRST_PORT + PORT_TRIES).chain([0]) {
        if let Ok(l) = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)).await {
            v4 = Some(l);
            break;
        }
    }
    let Some(v4) = v4 else { crate::bail_tr!("friends.noPort", "Couldn't open a port on this computer for friends.") };
    let port = v4.local_addr()?.port();
    let mut all = vec![v4];
    if let Ok(v6) = TcpListener::bind((std::net::Ipv6Addr::UNSPECIFIED, port)).await {
        all.push(v6);
    }
    Ok((port, all))
}

/// A way out of this computer: the address used to reach `target`. No packets are sent.
fn source_for(target: &str) -> Option<IpAddr> {
    let sock = UdpSocket::bind(if target.contains('[') { "[::]:0" } else { "0.0.0.0:0" }).ok()?;
    sock.connect(target).ok()?;
    Some(sock.local_addr().ok()?.ip())
}

fn is_global_v6(ip: IpAddr) -> bool {
    matches!(ip, IpAddr::V6(v6) if (v6.segments()[0] & 0xe000) == 0x2000)
}

fn is_cgnat_v4(ip: IpAddr) -> bool {
    matches!(ip, IpAddr::V4(v4) if v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1]))
}

/// The routes to put in the code, best first: the router's mapping, the public address (works if the port was forwarded
/// by hand), IPv6, a Tailscale-style VPN, the LAN.
pub fn routes(port: u16, mapped: Option<SocketAddr>, public: Option<Ipv4Addr>) -> Vec<Route> {
    let mut out: Vec<SocketAddr> = Vec::new();
    let mut add = |a: SocketAddr| {
        if !out.contains(&a) {
            out.push(a);
        }
    };
    if let Some(m) = mapped {
        add(m);
    }
    if let Some(ip) = public {
        add(SocketAddr::new(IpAddr::V4(ip), port));
    }
    if let Some(ip) = source_for("[2001:4860:4860::8888]:80").filter(|ip| is_global_v6(*ip)) {
        add(SocketAddr::new(ip, port));
    }
    if let Some(ip) = source_for("100.100.100.100:80").filter(|ip| is_cgnat_v4(*ip)) {
        add(SocketAddr::new(ip, port));
    }
    if let Some(ip) = network::lan_ip() {
        add(SocketAddr::new(IpAddr::V4(ip), port));
    }
    out.into_iter().map(Route::Direct).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_are_best_first_without_repeats() {
        let mapped: SocketAddr = "203.0.113.9:25580".parse().unwrap();
        let r = routes(25580, Some(mapped), Some(Ipv4Addr::new(203, 0, 113, 9)));
        assert_eq!(r[0], Route::Direct(mapped));
        assert_eq!(r.iter().filter(|x| **x == Route::Direct(mapped)).count(), 1);
    }

    #[tokio::test]
    async fn the_relays_handshake_is_skipped() {
        let mut bytes: &[u8] = &[3, 0, 1, 2, 9];
        skip_mc_handshake(&mut bytes).await.unwrap();
        assert_eq!(bytes, [9]);
        let mut too_long: &[u8] = &[0xff, 0xff, 0xff, 0xff, 0x0f];
        assert!(skip_mc_handshake(&mut too_long).await.is_err());
    }
}
