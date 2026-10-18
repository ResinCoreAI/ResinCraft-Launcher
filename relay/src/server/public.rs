//! The public Minecraft port: read the handshake, route by name, then forward the player to the tunnel, or
//! answer the server list ping / login ourselves when there's nothing to forward to.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Notify;
use tokio_yamux::StreamHandle;
use tracing::{debug, info};

use super::metrics::Metrics;
use super::{Relay, Tunnel};
use crate::handshake::{self, Handshake, Parsed};
use crate::proto::{self, now_secs, DataHeader};

/// How much traffic is counted locally before it's added to the account (and the cap checked).
const FLUSH_BYTES: u64 = 256 * 1024;

pub(crate) async fn serve(relay: Arc<Relay>, mut sock: TcpStream, peer: SocketAddr) {
    let m = &relay.metrics;
    Metrics::inc(&m.player_connections_total);
    if relay.is_banned_ip(peer.ip()) {
        Metrics::inc(&m.rejected_banned_total);
        return;
    }
    let Some(_ip) = relay.ip_guard(peer.ip(), false) else {
        Metrics::inc(&m.rejected_limit_total);
        return;
    };
    let Ok(permit) = relay.pending.clone().try_acquire_owned() else {
        Metrics::inc(&m.rejected_limit_total);
        return;
    };
    let _ = sock.set_nodelay(true);
    let mut buf = Vec::with_capacity(512);
    let wait = Duration::from_secs(relay.limits_now().handshake_timeout_secs);
    let parsed = tokio::time::timeout(wait, read_handshake(&mut sock, &mut buf)).await;
    drop(permit);
    let (hs, used) = match parsed {
        Ok(Ok(Parsed::Handshake(h, n))) => (h, n),
        Ok(Ok(Parsed::Legacy)) => {
            Metrics::inc(&m.legacy_pings_total);
            let _ = sock.write_all(&handshake::legacy_kick("Update Minecraft to join this server")).await;
            finish(sock).await;
            return;
        }
        _ => {
            Metrics::inc(&m.bad_handshakes_total);
            debug!(%peer, "no valid Minecraft handshake");
            return;
        }
    };

    let host = handshake::normalize_host(&hs.address);
    let base = relay.cfg.base_domain.as_str();
    let mut name = handshake::route_name(&host, base);
    if name.is_none() && relay.cfg.custom_domains.enabled && handshake::looks_like_domain(&host) && !host.ends_with(base) {
        name = relay.dns.route(&host, base).await;
        if name.is_some() {
            Metrics::inc(&m.custom_domain_lookups_total);
        }
    }
    let Some(name) = name else {
        return answer(&relay, sock, buf, used, &hs, Why::Unknown(host)).await;
    };
    let address = proto::address(&name, base);
    let Some(tunnel) = relay.tunnel(&name) else {
        let why = if relay.is_reserved(&name) { Why::Offline(address) } else { Why::Free(address) };
        return answer(&relay, sock, buf, used, &hs, why).await;
    };
    if relay.over_quota(&tunnel.owner) {
        return answer(&relay, sock, buf, used, &hs, Why::Quota(address)).await;
    }
    let _slot = Slot::take(&tunnel);
    if _slot.0 > relay.limits_now().connections_per_tunnel {
        return answer(&relay, sock, buf, used, &hs, Why::Busy(address)).await;
    }
    let mut control = tunnel.control.clone();
    let mut stream = match tokio::time::timeout(Duration::from_secs(10), control.open_stream()).await {
        Ok(Ok(s)) => s,
        _ => {
            debug!(name = %name, "the host didn't open a stream");
            return answer(&relay, sock, buf, used, &hs, Why::Offline(address)).await;
        }
    };
    let mut first = serde_json::to_vec(&DataHeader { peer: Some(peer.to_string()) }).unwrap_or_default();
    first.push(b'\n');
    first.extend_from_slice(&buf);
    if stream.write_all(&first).await.is_err() {
        return answer(&relay, sock, Vec::new(), 0, &hs, Why::Offline(address)).await;
    }
    Metrics::inc(&m.player_forwarded_total);
    m.player_connections_active.fetch_add(1, Ordering::Relaxed);
    if hs.next_state == 1 {
        debug!(name = %name, "server list ping forwarded");
    } else {
        info!(name = %name, "player joining");
        debug!(name = %name, %peer, "player address");
        relay.event("join", format!("A player is joining {name}"), Some(peer.ip()));
    }
    splice(&relay, sock, stream, &tunnel, buf.len() as u64).await;
    m.player_connections_active.fetch_sub(1, Ordering::Relaxed);
}

/// A player's place in a tunnel's connection count.
struct Slot(u32, Arc<std::sync::atomic::AtomicU32>);

impl Slot {
    fn take(t: &Tunnel) -> Slot {
        Slot(t.conns.fetch_add(1, Ordering::Relaxed) + 1, t.conns.clone())
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.1.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Reads until the buffer holds a complete handshake (or a legacy ping); extra bytes stay in `buf`.
async fn read_handshake(sock: &mut TcpStream, buf: &mut Vec<u8>) -> std::io::Result<Parsed> {
    let mut chunk = [0u8; 1024];
    loop {
        match handshake::parse(buf) {
            Ok(Parsed::Incomplete) => {}
            Ok(done) => return Ok(done),
            Err(e) => return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        }
        let n = sock.read(&mut chunk).await?;
        if n == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

/// Why the relay answers itself.
enum Why {
    /// Not a ResinCraft address at all.
    Unknown(String),
    /// A valid name nobody reserved.
    Free(String),
    /// Reserved, but the host's launcher isn't connected.
    Offline(String),
    Quota(String),
    Busy(String),
}

impl Why {
    fn texts(&self) -> (&'static str, String) {
        match self {
            Why::Unknown(host) => ("Unknown address", format!("{host} isn't a ResinCraft Relay address.")),
            Why::Free(address) => ("No server here yet", format!("Nobody uses {address} on ResinCraft Relay.")),
            Why::Offline(address) => ("This server is offline", format!("{address} · Ask the host to start it in ResinCraft Launcher.")),
            Why::Quota(address) => ("This server reached its relay limit for this month", format!("{address} · The host's ResinCraft Relay traffic resets on the 1st.")),
            Why::Busy(address) => ("This server is busy", format!("{address} · Too many players are connecting right now. Try again in a moment.")),
        }
    }
}

/// Answers a server list ping (status + pong) or turns a login away with a message.
async fn answer(relay: &Relay, mut sock: TcpStream, mut buf: Vec<u8>, used: usize, hs: &Handshake, why: Why) {
    let (title, detail) = why.texts();
    if hs.next_state == 1 {
        Metrics::inc(&relay.metrics.offline_status_total);
        let mut rest = buf.split_off(used.min(buf.len()));
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut status_sent = false;
        let mut chunk = [0u8; 256];
        loop {
            match handshake::read_packet(&rest, handshake::MAX_STATUS_PACKET) {
                Ok(Some((0, body, n))) if body.is_empty() && !status_sent => {
                    let json = handshake::status_json(hs.protocol, title, &detail);
                    if sock.write_all(&handshake::status_response(&json)).await.is_err() {
                        return;
                    }
                    status_sent = true;
                    rest.drain(..n);
                }
                Ok(Some((1, body, _))) if body.len() == 8 => {
                    let reply = handshake::pong(body);
                    let _ = sock.write_all(&reply).await;
                    break;
                }
                Ok(Some(_)) | Err(_) => break,
                Ok(None) => match tokio::time::timeout_at(deadline, sock.read(&mut chunk)).await {
                    Ok(Ok(n)) if n > 0 => rest.extend_from_slice(&chunk[..n]),
                    _ => break,
                },
            }
        }
    } else {
        Metrics::inc(&relay.metrics.offline_login_total);
        let _ = sock.write_all(&handshake::login_disconnect(&handshake::disconnect_text(title, &detail))).await;
    }
    finish(sock).await;
}

/// Closes gracefully: no more writes, then wait briefly for the client to close, so it reads everything.
async fn finish(mut sock: TcpStream) {
    let _ = sock.shutdown().await;
    let mut sink = [0u8; 512];
    let _ = tokio::time::timeout(Duration::from_secs(2), async {
        while let Ok(n) = sock.read(&mut sink).await {
            if n == 0 {
                break;
            }
        }
    })
    .await;
}

/// Counts a connection's traffic against the host's account and watches for idling or the cap.
struct Meter<'a> {
    relay: &'a Relay,
    owner: &'a str,
    pending: AtomicU64,
    last: AtomicU64,
    over: AtomicBool,
    stop: Notify,
}

impl Meter<'_> {
    /// Counts `n` bytes; false once the account is over its cap.
    fn add(&self, n: u64) -> bool {
        self.last.store(now_secs(), Ordering::Relaxed);
        if self.pending.fetch_add(n, Ordering::Relaxed) + n >= FLUSH_BYTES {
            let p = self.pending.swap(0, Ordering::Relaxed);
            if !self.relay.add_usage(self.owner, p) {
                self.over.store(true, Ordering::Relaxed);
                self.stop.notify_one();
            }
        }
        !self.over.load(Ordering::Relaxed)
    }

    fn flush(&self) {
        let p = self.pending.swap(0, Ordering::Relaxed);
        if p > 0 {
            self.relay.add_usage(self.owner, p);
        }
    }

    /// Completes when the connection idled too long or went over the cap.
    async fn watch(&self, idle: Duration) {
        loop {
            tokio::select! {
                _ = self.stop.notified() => return,
                _ = tokio::time::sleep(Duration::from_secs(5)) => {
                    if now_secs().saturating_sub(self.last.load(Ordering::Relaxed)) >= idle.as_secs() {
                        return;
                    }
                }
            }
        }
    }
}

/// Copies one direction; `counters` are the relay's total and the tunnel's own for that direction.
async fn pump<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(r: &mut R, w: &mut W, meter: &Meter<'_>, counters: [&AtomicU64; 2]) {
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        let n = match r.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        if w.write_all(&buf[..n]).await.is_err() {
            break;
        }
        for c in counters {
            c.fetch_add(n as u64, Ordering::Relaxed);
        }
        if !meter.add(n as u64) {
            break;
        }
    }
    let _ = w.shutdown().await;
}

async fn splice(relay: &Relay, player: TcpStream, host: StreamHandle, tunnel: &Tunnel, already_sent: u64) {
    let meter = Meter {
        relay,
        owner: &tunnel.owner,
        pending: AtomicU64::new(0),
        last: AtomicU64::new(now_secs()),
        over: AtomicBool::new(false),
        stop: Notify::new(),
    };
    // The handshake (and whatever came with it) went out before the pumps started.
    relay.metrics.bytes_to_host_total.fetch_add(already_sent, Ordering::Relaxed);
    tunnel.bytes_in.fetch_add(already_sent, Ordering::Relaxed);
    meter.add(already_sent);
    let (mut pr, mut pw) = player.into_split();
    let (mut hr, mut hw) = tokio::io::split(host);
    let idle = Duration::from_secs(relay.limits_now().idle_timeout_secs.max(10));
    tokio::select! {
        _ = async {
            tokio::join!(
                pump(&mut pr, &mut hw, &meter, [&relay.metrics.bytes_to_host_total, &tunnel.bytes_in]),
                pump(&mut hr, &mut pw, &meter, [&relay.metrics.bytes_to_player_total, &tunnel.bytes_out]),
            )
        } => {}
        _ = meter.watch(idle) => {}
    }
    meter.flush();
}
