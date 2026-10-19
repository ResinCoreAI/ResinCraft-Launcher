//! Bedrock tunnels: each Bedrock server online gets a UDP port of its own on the relay (players add
//! `<name>.<base_domain>` with that port). Every player address that starts a RakNet connection gets a stream to
//! the host's launcher, and datagrams travel on it both ways as frames ([`proto::write_datagram`]).

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tracing::{debug, info};

use super::metrics::Metrics;
use super::public::{Meter, Slot};
use super::{Relay, Tunnel};
use crate::proto::{self, DataHeader, ErrorCode};

/// A player address that sends nothing for this long is forgotten (Bedrock clients send something every few
/// seconds while connected, and ping the server list every couple of seconds while it's open).
const IDLE: Duration = Duration::from_secs(30);
/// Packets waiting for one player's stream before more are dropped.
const QUEUE: usize = 256;
/// Player addresses one port keeps track of at once.
const MAX_ADDRESSES: usize = 4096;

/// RakNet's first packets: the unconnected pings of the server list (0x01, 0x02) and the first open-connection
/// request (0x05). Anything else from an unknown address opens nothing (a stray or spoofed packet).
fn starts_session(packet: &[u8]) -> bool {
    matches!(packet.first(), Some(0x01 | 0x02 | 0x05))
}

impl Relay {
    /// A UDP socket for `name`'s Bedrock server: the port it had last time when that's free, else one no other
    /// name had, else any free one in `bedrock_ports`.
    pub(crate) async fn bind_bedrock(&self, name: &str) -> Result<(u16, UdpSocket), ErrorCode> {
        let range = self.cfg.bedrock_range().ok_or(ErrorCode::BedrockUnavailable)?;
        let ip: IpAddr = self.cfg.bedrock_ip.trim().parse().unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
        let (mine, others): (Option<u16>, HashSet<u16>) = self.store.read(|d| {
            let mine = d.names.get(name).and_then(|e| e.bedrock_port);
            (mine, d.names.iter().filter(|(n, _)| n.as_str() != name).filter_map(|(_, e)| e.bedrock_port).collect())
        });
        let busy: HashSet<u16> = self.tunnel_list().into_iter().filter(|(n, _)| n != name).filter_map(|(_, t)| t.bedrock_port).collect();
        let mut order: Vec<u16> = mine.filter(|p| range.contains(p) && !busy.contains(p)).into_iter().collect();
        order.extend(range.clone().filter(|p| Some(*p) != mine && !busy.contains(p) && !others.contains(p)));
        order.extend(range.clone().filter(|p| Some(*p) != mine && !busy.contains(p) && others.contains(p)));
        for port in order {
            // The same name coming back online (a reconnect): its old socket may take a moment to close.
            let tries = if Some(port) == mine { 10 } else { 1 };
            for attempt in 1..=tries {
                match UdpSocket::bind((ip, port)).await {
                    Ok(sock) => return Ok((port, sock)),
                    Err(_) if attempt < tries => tokio::time::sleep(Duration::from_millis(200)).await,
                    Err(_) => {}
                }
            }
        }
        Err(ErrorCode::BedrockUnavailable)
    }

    /// Remembers `name`'s port for next time (taking it from a name that had it before).
    pub(crate) fn remember_bedrock_port(&self, name: &str, port: u16) {
        self.store.write(|d| {
            for (n, e) in d.names.iter_mut() {
                if e.bedrock_port == Some(port) && n != name {
                    e.bedrock_port = None;
                }
            }
            if let Some(e) = d.names.get_mut(name) {
                e.bedrock_port = Some(port);
            }
        });
    }
}

/// Serves `name`'s Bedrock port until the task is aborted (when the tunnel goes offline).
pub(crate) async fn serve(relay: Arc<Relay>, sock: UdpSocket, name: String, tunnel: Tunnel) {
    let sock = Arc::new(sock);
    let mut players: HashMap<SocketAddr, mpsc::Sender<Vec<u8>>> = HashMap::new();
    let (gone_tx, mut gone_rx) = mpsc::unbounded_channel::<SocketAddr>();
    let mut buf = vec![0u8; 65_536];
    loop {
        let (n, peer) = tokio::select! {
            got = sock.recv_from(&mut buf) => match got {
                Ok(got) => got,
                // Windows reports an earlier send's "port unreachable" on the next receive: not our problem.
                Err(e) if matches!(e.kind(), std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionRefused) => continue,
                Err(e) => {
                    debug!(name = %name, error = %e, "Bedrock port receive failed");
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }
            },
            Some(peer) = gone_rx.recv() => {
                if players.get(&peer).is_some_and(|tx| tx.is_closed()) {
                    players.remove(&peer);
                }
                continue;
            }
        };
        let packet = &buf[..n];
        if let Some(tx) = players.get(&peer) {
            match tx.try_send(packet.to_vec()) {
                Ok(()) => continue,
                Err(mpsc::error::TrySendError::Full(_)) => {
                    Metrics::inc(&relay.metrics.bedrock_dropped_total);
                    continue;
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    players.remove(&peer);
                }
            }
        }
        if !starts_session(packet) || players.len() >= MAX_ADDRESSES {
            Metrics::inc(&relay.metrics.bedrock_dropped_total);
            continue;
        }
        let (tx, rx) = mpsc::channel(QUEUE);
        let _ = tx.try_send(packet.to_vec());
        players.insert(peer, tx);
        tokio::spawn(player(relay.clone(), sock.clone(), tunnel.clone(), name.clone(), peer, rx, gone_tx.clone()));
    }
}

/// Tells the port's loop that a player's task ended.
struct Gone(SocketAddr, mpsc::UnboundedSender<SocketAddr>);

impl Drop for Gone {
    fn drop(&mut self) {
        let _ = self.1.send(self.0);
    }
}

/// Swallows a refused address's packets until it goes quiet, so each one doesn't start over.
async fn ignore(mut rx: mpsc::Receiver<Vec<u8>>) {
    while let Ok(Some(_)) = tokio::time::timeout(IDLE, rx.recv()).await {}
}

/// One player address: a stream to the host, datagrams both ways until either side goes quiet or closes.
async fn player(
    relay: Arc<Relay>,
    sock: Arc<UdpSocket>,
    tunnel: Tunnel,
    name: String,
    peer: SocketAddr,
    mut rx: mpsc::Receiver<Vec<u8>>,
    gone: mpsc::UnboundedSender<SocketAddr>,
) {
    let _gone = Gone(peer, gone);
    let m = &relay.metrics;
    Metrics::inc(&m.player_connections_total);
    if relay.is_banned_ip(peer.ip()) {
        Metrics::inc(&m.rejected_banned_total);
        return ignore(rx).await;
    }
    let Some(_ip) = relay.ip_guard(peer.ip(), false) else {
        Metrics::inc(&m.rejected_limit_total);
        return ignore(rx).await;
    };
    if relay.over_quota(&tunnel.owner) {
        Metrics::inc(&m.offline_login_total);
        return ignore(rx).await;
    }
    let slot = Slot::take(&tunnel);
    if slot.0 > relay.limits_now().connections_per_tunnel {
        Metrics::inc(&m.rejected_limit_total);
        return ignore(rx).await;
    }
    let mut control = tunnel.control.clone();
    let stream = match tokio::time::timeout(Duration::from_secs(10), control.open_stream()).await {
        Ok(Ok(s)) => s,
        _ => {
            debug!(name = %name, "the host didn't open a stream");
            return ignore(rx).await;
        }
    };
    let (mut from_host, mut to_host) = tokio::io::split(stream);
    let mut first = serde_json::to_vec(&DataHeader { peer: Some(peer.to_string()), udp: true }).unwrap_or_default();
    first.push(b'\n');
    if to_host.write_all(&first).await.is_err() {
        return;
    }
    Metrics::inc(&m.bedrock_sessions_total);
    Metrics::inc(&m.player_forwarded_total);
    m.player_connections_active.fetch_add(1, Ordering::Relaxed);
    let meter = Meter::new(&relay, &tunnel.owner);
    let mut joining = false;
    let up = async {
        let mut frame = Vec::with_capacity(1600);
        while let Ok(Some(packet)) = tokio::time::timeout(IDLE, rx.recv()).await {
            if !joining && packet.first() == Some(&0x05) {
                joining = true;
                info!(name = %name, "Bedrock player joining");
                debug!(name = %name, %peer, "player address");
                relay.event("join", format!("A Bedrock player is joining {name}"), Some(peer.ip()));
            }
            frame.clear();
            proto::write_datagram(&mut frame, &packet);
            if to_host.write_all(&frame).await.is_err() {
                break;
            }
            let n = packet.len() as u64;
            m.bytes_to_host_total.fetch_add(n, Ordering::Relaxed);
            tunnel.bytes_in.fetch_add(n, Ordering::Relaxed);
            if !meter.add(n) {
                break;
            }
        }
        let _ = to_host.shutdown().await;
    };
    let down = async {
        let mut packet = Vec::with_capacity(1600);
        while let Ok(true) = proto::read_datagram(&mut from_host, &mut packet).await {
            if sock.send_to(&packet, peer).await.is_err() {
                continue;
            }
            let n = packet.len() as u64;
            m.bytes_to_player_total.fetch_add(n, Ordering::Relaxed);
            tunnel.bytes_out.fetch_add(n, Ordering::Relaxed);
            if !meter.add(n) {
                break;
            }
        }
    };
    tokio::select! {
        _ = up => {}
        _ = down => {}
    }
    meter.flush();
    m.player_connections_active.fetch_sub(1, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_raknet_openers_start_a_session() {
        assert!(starts_session(&[0x01, 0, 0]) && starts_session(&[0x02]) && starts_session(&[0x05, 0]));
        assert!(!starts_session(&[]) && !starts_session(&[0x84, 0, 0]) && !starts_session(&[0x07]) && !starts_session(&[0xc0]));
    }
}
