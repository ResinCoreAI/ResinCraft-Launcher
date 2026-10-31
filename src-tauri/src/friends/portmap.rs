//! Asking the home router to forward the tunnel's port: PCP (RFC 6887), then NAT-PMP (RFC 6886), then UPnP (through
//! server/network.rs), and STUN (RFC 8489) for the public address when none of them says it. Mappings are timed, so
//! one left behind by a crash goes away by itself unless the router only takes permanent UPnP mappings.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use tokio::net::{lookup_host, UdpSocket};

use crate::server::network;

const GATEWAY_PORT: u16 = 5351;
const TCP: u8 = 6;
pub const LEASE_SECS: u32 = 3600;
const STUN_SERVERS: [&str; 2] = ["stun.cloudflare.com:3478", "stun.l.google.com:19302"];
const STUN_COOKIE: u32 = 0x2112_A442;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Via {
    Pcp,
    Natpmp,
    Upnp,
}

/// An open port on the router, towards this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    pub via: Via,
    pub internal: u16,
    pub external: SocketAddr,
    /// When to ask again; the router's answer can be shorter than what was asked.
    pub lease: u32,
    nonce: [u8; 12],
    gateway: SocketAddr,
}

/// The default gateway, from the system's routing table.
async fn default_gateway() -> Option<Ipv4Addr> {
    #[cfg(target_os = "linux")]
    {
        let table = tokio::fs::read_to_string("/proc/net/route").await.ok()?;
        return parse_proc_route(&table);
    }
    #[cfg(windows)]
    {
        let out = crate::game::java::hidden_command("route").args(["print", "-4", "0.0.0.0"]).output().await.ok()?;
        return parse_route_print(&String::from_utf8_lossy(&out.stdout));
    }
    #[cfg(target_os = "macos")]
    {
        let out = tokio::process::Command::new("route").args(["-n", "get", "default"]).output().await.ok()?;
        return String::from_utf8_lossy(&out.stdout).lines().find_map(|l| l.trim().strip_prefix("gateway:")?.trim().parse().ok());
    }
    #[allow(unreachable_code)]
    None
}

/// The default route's row in Windows' `route print`: "0.0.0.0  0.0.0.0  <gateway>  <interface>  <metric>", the lowest
/// metric first in the player's interest. These columns don't change with the system language.
#[cfg_attr(not(windows), allow(dead_code))]
fn parse_route_print(text: &str) -> Option<Ipv4Addr> {
    let mut best: Option<(u32, Ipv4Addr)> = None;
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if let [dest, mask, gw, _, metric] = f[..] {
            if dest == "0.0.0.0" && mask == "0.0.0.0" {
                if let (Ok(gw), Ok(metric)) = (gw.parse::<Ipv4Addr>(), metric.parse::<u32>()) {
                    if best.is_none_or(|(m, _)| metric < m) {
                        best = Some((metric, gw));
                    }
                }
            }
        }
    }
    best.map(|(_, gw)| gw)
}

/// The default route in Linux's /proc/net/route (the gateway is little-endian hex).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn parse_proc_route(table: &str) -> Option<Ipv4Addr> {
    table.lines().skip(1).find_map(|l| {
        let f: Vec<&str> = l.split_whitespace().collect();
        if f.get(1) != Some(&"00000000") {
            return None;
        }
        Some(Ipv4Addr::from(u32::from_str_radix(f.get(2)?, 16).ok()?.to_le_bytes()))
    })
}

/// Sends `request` and waits for a datagram `check` takes, resending as RFC 6886 says.
async fn exchange<T>(gateway: SocketAddr, request: &[u8], check: impl Fn(&[u8]) -> Option<Result<T>>) -> Result<T> {
    let sock = UdpSocket::bind(if gateway.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" }).await?;
    sock.connect(gateway).await?;
    let mut buf = [0u8; 1100];
    for wait in [250u64, 500, 1000] {
        sock.send(request).await?;
        let deadline = tokio::time::Instant::now() + Duration::from_millis(wait);
        loop {
            match tokio::time::timeout_at(deadline, sock.recv(&mut buf)).await {
                Err(_) => break,
                // Windows reports "port unreachable" here when the router doesn't take these requests.
                Ok(Err(e)) => bail!("no answer ({e})"),
                Ok(Ok(n)) => {
                    if let Some(r) = check(&buf[..n]) {
                        return r;
                    }
                }
            }
        }
    }
    bail!("no answer")
}

fn v4_mapped(ip: Ipv4Addr) -> [u8; 16] {
    ip.to_ipv6_mapped().octets()
}

fn pcp_request(client: Ipv4Addr, nonce: &[u8; 12], internal: u16, external: u16, lifetime: u32) -> [u8; 60] {
    let mut p = [0u8; 60];
    p[0] = 2;
    p[1] = 1; // MAP
    p[4..8].copy_from_slice(&lifetime.to_be_bytes());
    p[8..24].copy_from_slice(&v4_mapped(client));
    p[24..36].copy_from_slice(nonce);
    p[36] = TCP;
    p[40..42].copy_from_slice(&internal.to_be_bytes());
    p[42..44].copy_from_slice(&external.to_be_bytes());
    p[44..60].copy_from_slice(&v4_mapped(Ipv4Addr::UNSPECIFIED));
    p
}

/// The mapping in a PCP answer: (lifetime, external address).
fn pcp_reply(r: &[u8], nonce: &[u8; 12]) -> Option<Result<(u32, SocketAddr)>> {
    if r.len() < 60 || r[0] != 2 || r[1] != 0x81 {
        // A NAT-PMP router answers a PCP request with its own "unsupported version".
        return (r.len() >= 4 && r[0] == 0 && r[1] >= 0x80).then(|| Err(anyhow!("speaks NAT-PMP, not PCP")));
    }
    if r[24..36] != nonce[..] {
        return None;
    }
    if r[3] != 0 {
        return Some(Err(anyhow!("refused (PCP result {})", r[3])));
    }
    let lifetime = u32::from_be_bytes([r[4], r[5], r[6], r[7]]);
    let port = u16::from_be_bytes([r[42], r[43]]);
    let octets: [u8; 16] = r[44..60].try_into().ok()?;
    let v6 = Ipv6Addr::from(octets);
    let ip = v6.to_ipv4_mapped().map_or(IpAddr::V6(v6), IpAddr::V4);
    Some(Ok((lifetime, SocketAddr::new(ip, port))))
}

async fn pcp(gateway: SocketAddr, client: Ipv4Addr, nonce: [u8; 12], internal: u16, external: u16, lifetime: u32) -> Result<(u32, SocketAddr)> {
    exchange(gateway, &pcp_request(client, &nonce, internal, external, lifetime), |r| pcp_reply(r, &nonce)).await
}

fn pmp_map_request(internal: u16, external: u16, lifetime: u32) -> [u8; 12] {
    let mut p = [0u8; 12];
    p[1] = 2; // Map TCP
    p[4..6].copy_from_slice(&internal.to_be_bytes());
    p[6..8].copy_from_slice(&external.to_be_bytes());
    p[8..12].copy_from_slice(&lifetime.to_be_bytes());
    p
}

async fn pmp_external_ip(gateway: SocketAddr) -> Result<Ipv4Addr> {
    exchange(gateway, &[0, 0], |r| {
        if r.len() < 12 || r[0] != 0 || r[1] != 128 {
            return None;
        }
        if r[3] != 0 {
            return Some(Err(anyhow!("refused (NAT-PMP result {})", r[3])));
        }
        Some(Ok(Ipv4Addr::new(r[8], r[9], r[10], r[11])))
    })
    .await
}

async fn pmp_map(gateway: SocketAddr, internal: u16, external: u16, lifetime: u32) -> Result<(u32, u16)> {
    exchange(gateway, &pmp_map_request(internal, external, lifetime), |r| {
        if r.len() < 16 || r[0] != 0 || r[1] != 130 {
            return None;
        }
        if r[3] != 0 {
            return Some(Err(anyhow!("refused (NAT-PMP result {})", r[3])));
        }
        let port = u16::from_be_bytes([r[10], r[11]]);
        Some(Ok((u32::from_be_bytes([r[12], r[13], r[14], r[15]]), port)))
    })
    .await
}

fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => !network::is_shared(v4),
        IpAddr::V6(_) => true,
    }
}

/// Tries PCP and NAT-PMP at `gateway`; a router behind another NAT gives an address friends can't use.
async fn open_at(gateway: SocketAddr, client: Ipv4Addr, port: u16) -> Result<Mapping> {
    let nonce: [u8; 12] = uuid::Uuid::new_v4().as_bytes()[..12].try_into()?;
    let pcp_error = match pcp(gateway, client, nonce, port, port, LEASE_SECS).await {
        Ok((lease, external)) if is_public(external.ip()) => return Ok(Mapping { via: Via::Pcp, internal: port, external, lease, nonce, gateway }),
        Ok((_, external)) => {
            let _ = pcp(gateway, client, nonce, port, 0, 0).await;
            bail!("the router's own address is {} (a second NAT)", external.ip());
        }
        Err(e) => e,
    };
    let (lease, external_port) = pmp_map(gateway, port, port, LEASE_SECS).await.map_err(|e| anyhow!("PCP: {pcp_error}; NAT-PMP: {e}"))?;
    let ip = pmp_external_ip(gateway).await?;
    if !is_public(IpAddr::V4(ip)) {
        let _ = pmp_map(gateway, port, 0, 0).await;
        bail!("the router's own address is {ip} (a second NAT)");
    }
    Ok(Mapping { via: Via::Natpmp, internal: port, external: SocketAddr::new(IpAddr::V4(ip), external_port), lease, nonce, gateway })
}

/// Opens `port` on the router. The error says what each way answered (it goes to the launcher's log).
pub async fn open(port: u16) -> Result<Mapping> {
    let client = network::lan_ip().ok_or_else(|| anyhow!("no network"))?;
    let first = match default_gateway().await {
        Some(ip) => match open_at(SocketAddr::new(IpAddr::V4(ip), GATEWAY_PORT), client, port).await {
            Ok(m) => return Ok(m),
            Err(e) => e.to_string(),
        },
        None => "no default gateway".to_string(),
    };
    let address = network::open_port(port, "friends".into()).await.map_err(|e| anyhow!("{first}; UPnP: {e}"))?;
    let external = address.parse().map_err(|_| anyhow!("the router gave {address}"))?;
    Ok(Mapping { via: Via::Upnp, internal: port, external, lease: network::LEASE_SECS, nonce: [0; 12], gateway: SocketAddr::from(([0, 0, 0, 0], 0)) })
}

/// Asks for more time on a mapping.
pub async fn renew(m: &mut Mapping) -> Result<()> {
    let external = m.external.port();
    match m.via {
        Via::Pcp => {
            let client = network::lan_ip().ok_or_else(|| anyhow!("no network"))?;
            let (lease, addr) = pcp(m.gateway, client, m.nonce, m.internal, external, LEASE_SECS).await?;
            (m.lease, m.external) = (lease, addr);
        }
        Via::Natpmp => {
            let (lease, port) = pmp_map(m.gateway, m.internal, external, LEASE_SECS).await?;
            (m.lease, m.external) = (lease, SocketAddr::new(m.external.ip(), port));
        }
        Via::Upnp => {
            network::open_port(m.internal, "friends".into()).await?;
        }
    }
    Ok(())
}

/// Removes a mapping from the router.
pub async fn close(m: &Mapping) -> Result<()> {
    match m.via {
        Via::Pcp => {
            let client = network::lan_ip().ok_or_else(|| anyhow!("no network"))?;
            pcp(m.gateway, client, m.nonce, m.internal, 0, 0).await?;
        }
        Via::Natpmp => {
            pmp_map(m.gateway, m.internal, 0, 0).await?;
        }
        Via::Upnp => network::close_port(m.internal).await,
    }
    Ok(())
}

// ---------- STUN ----------

fn stun_request(id: &[u8; 12]) -> [u8; 20] {
    let mut p = [0u8; 20];
    p[1] = 1; // Binding request, no attributes.
    p[4..8].copy_from_slice(&STUN_COOKIE.to_be_bytes());
    p[8..20].copy_from_slice(id);
    p
}

/// The IPv4 address in a Binding response's XOR-MAPPED-ADDRESS.
fn stun_reply(r: &[u8], id: &[u8; 12]) -> Option<Ipv4Addr> {
    if r.len() < 20 || r[0] != 1 || r[1] != 1 || r[8..20] != id[..] {
        return None;
    }
    let mut at = 20;
    while at + 4 <= r.len() {
        let kind = u16::from_be_bytes([r[at], r[at + 1]]);
        let len = u16::from_be_bytes([r[at + 2], r[at + 3]]) as usize;
        let value = r.get(at + 4..at + 4 + len)?;
        if kind == 0x0020 && len == 8 && value[1] == 1 {
            let raw = u32::from_be_bytes(value[4..8].try_into().ok()?) ^ STUN_COOKIE;
            return Some(Ipv4Addr::from(raw));
        }
        at += 4 + len.div_ceil(4) * 4;
    }
    None
}

async fn stun_at(server: SocketAddr) -> Result<Ipv4Addr> {
    let id: [u8; 12] = uuid::Uuid::new_v4().as_bytes()[..12].try_into()?;
    exchange(server, &stun_request(&id), |r| stun_reply(r, &id).map(Ok)).await
}

/// This network's public IPv4 address as a STUN server sees it (Cloudflare's trace page when UDP is blocked).
pub async fn public_ipv4() -> Option<Ipv4Addr> {
    for server in STUN_SERVERS {
        let Ok(Ok(mut found)) = tokio::time::timeout(Duration::from_secs(2), lookup_host(server)).await else { continue };
        if let Some(addr) = found.find(SocketAddr::is_ipv4) {
            if let Ok(ip) = stun_at(addr).await {
                return Some(ip);
            }
        }
    }
    network::public_ipv4().await.ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// A router that speaks PCP or NAT-PMP on a local port, and remembers its open mappings.
    async fn router(pcp_too: bool, external: Ipv4Addr) -> (SocketAddr, Arc<Mutex<Vec<u16>>>) {
        let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = sock.local_addr().unwrap();
        let open = Arc::new(Mutex::new(Vec::new()));
        let table = open.clone();
        tokio::spawn(async move {
            let mut b = [0u8; 1100];
            while let Ok((n, from)) = sock.recv_from(&mut b).await {
                let r = &b[..n];
                let set = |port: u16, life: u32| {
                    let mut t = table.lock().unwrap();
                    t.retain(|p| *p != port);
                    if life > 0 {
                        t.push(port);
                    }
                };
                if r[0] == 2 && pcp_too {
                    let mut a = [0u8; 60];
                    a[0] = 2;
                    a[1] = 0x81;
                    a[4..8].copy_from_slice(&r[4..8]);
                    a[24..44].copy_from_slice(&r[24..44]);
                    a[44..60].copy_from_slice(&v4_mapped(external));
                    set(u16::from_be_bytes([r[40], r[41]]), u32::from_be_bytes([r[4], r[5], r[6], r[7]]));
                    sock.send_to(&a, from).await.unwrap();
                } else if r[0] == 2 {
                    sock.send_to(&[0, 0x81, 0, 1, 0, 0, 0, 0], from).await.unwrap();
                } else if r[1] == 0 {
                    let mut a = [0u8; 12];
                    a[1] = 128;
                    a[8..12].copy_from_slice(&external.octets());
                    sock.send_to(&a, from).await.unwrap();
                } else {
                    let mut a = [0u8; 16];
                    a[1] = 130;
                    a[8..10].copy_from_slice(&r[4..6]);
                    a[10..12].copy_from_slice(&r[4..6]);
                    a[12..16].copy_from_slice(&r[8..12]);
                    set(u16::from_be_bytes([r[4], r[5]]), u32::from_be_bytes([r[8], r[9], r[10], r[11]]));
                    sock.send_to(&a, from).await.unwrap();
                }
            }
        });
        (addr, open)
    }

    const PUBLIC: Ipv4Addr = Ipv4Addr::new(203, 0, 113, 9);
    const LAN: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 20);

    #[tokio::test]
    async fn pcp_maps_renews_and_removes() {
        let (gw, table) = router(true, PUBLIC).await;
        let mut m = open_at(gw, LAN, 25580).await.unwrap();
        assert_eq!(m.via, Via::Pcp);
        assert_eq!(m.external, SocketAddr::from((PUBLIC, 25580)));
        assert_eq!(m.lease, LEASE_SECS);
        assert_eq!(*table.lock().unwrap(), [25580]);
        let nonce = m.nonce;
        let (lease, _) = pcp(m.gateway, LAN, nonce, 25580, 25580, LEASE_SECS).await.unwrap();
        m.lease = lease;
        pcp(m.gateway, LAN, nonce, 25580, 0, 0).await.unwrap();
        assert!(table.lock().unwrap().is_empty(), "the mapping stayed on the router");
    }

    #[tokio::test]
    async fn nat_pmp_is_the_fallback_and_removes_too() {
        let (gw, table) = router(false, PUBLIC).await;
        let mut m = open_at(gw, LAN, 25581).await.unwrap();
        assert_eq!(m.via, Via::Natpmp);
        assert_eq!(m.external, SocketAddr::from((PUBLIC, 25581)));
        assert_eq!(*table.lock().unwrap(), [25581]);
        renew(&mut m).await.unwrap();
        assert_eq!(*table.lock().unwrap(), [25581]);
        close(&m).await.unwrap();
        assert!(table.lock().unwrap().is_empty(), "the mapping stayed on the router");
    }

    #[tokio::test]
    async fn a_second_nat_is_refused_and_cleaned_up() {
        for pcp_too in [true, false] {
            let (gw, table) = router(pcp_too, Ipv4Addr::new(100, 64, 3, 4)).await;
            assert!(open_at(gw, LAN, 25582).await.is_err());
            assert!(table.lock().unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn stun_finds_the_address() {
        let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = sock.local_addr().unwrap();
        tokio::spawn(async move {
            let mut b = [0u8; 64];
            let (_, from) = sock.recv_from(&mut b).await.unwrap();
            let mut a = vec![1, 1, 0, 24];
            a.extend_from_slice(&b[4..20]);
            a.extend_from_slice(&[0x80, 0x22, 0, 4, 1, 2, 3, 4]); // SOFTWARE, skipped
            a.extend_from_slice(&[0, 0x20, 0, 8, 0, 1, 0, 0]);
            a.extend_from_slice(&(u32::from(PUBLIC) ^ STUN_COOKIE).to_be_bytes());
            sock.send_to(&a, from).await.unwrap();
        });
        assert_eq!(stun_at(addr).await.unwrap(), PUBLIC);
    }

    #[test]
    fn default_gateway_from_route_tables() {
        let win = "IPv4 Route Table\n  Network Destination  Netmask  Gateway  Interface  Metric\n  0.0.0.0  0.0.0.0  10.0.0.1  10.0.0.5  50\n  0.0.0.0  0.0.0.0  192.168.1.1  192.168.1.20  25\n  127.0.0.0  255.0.0.0  On-link  127.0.0.1  331\n";
        assert_eq!(parse_route_print(win), Some(Ipv4Addr::new(192, 168, 1, 1)));
        let linux = "Iface\tDestination\tGateway\tFlags\neth0\t00000000\t0101A8C0\t0003\n";
        assert_eq!(parse_proc_route(linux), Some(Ipv4Addr::new(192, 168, 1, 1)));
    }
}
