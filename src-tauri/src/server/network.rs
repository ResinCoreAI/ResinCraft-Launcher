//! Letting players reach a hosted server: the computer's LAN address, and opening the port on the home
//! router automatically (UPnP) so friends outside the home network can join without manual port forwarding.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::Duration;

use anyhow::{anyhow, Result};
use igd_next::{AddPortError, PortMappingProtocol, SearchOptions};

/// This computer's address on the local network (the one used for outgoing traffic). No packets are sent.
pub fn lan_ip() -> Option<Ipv4Addr> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    match sock.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_unspecified() => Some(ip),
        _ => None,
    }
}

/// Addresses the internet can't reach: the router itself sits behind another NAT (carrier-grade NAT or a
/// second router).
fn is_shared(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_private() || ip.is_loopback() || ip.is_link_local() || ip.is_unspecified() || (a == 100 && (64..128).contains(&b))
}

/// How long a mapping lasts before the router drops it; renewed while the server runs.
pub const LEASE_SECS: u32 = 3600;

fn gateway() -> Result<igd_next::Gateway> {
    let opts = SearchOptions { timeout: Some(Duration::from_secs(4)), ..Default::default() };
    igd_next::search_gateway(opts)
        .map_err(|_| anyhow!("Your router didn't answer. Turn on UPnP in the router's settings, or forward the port manually."))
}

/// Opens `port` (TCP) on the router towards this computer; returns the public "ip:port" to share.
pub async fn open_port(port: u16, label: String) -> Result<String> {
    open_mapping(PortMappingProtocol::TCP, port, label).await
}

/// The same for UDP (Bedrock servers, and Geyser on Java servers).
pub async fn open_udp(port: u16, label: String) -> Result<String> {
    open_mapping(PortMappingProtocol::UDP, port, label).await
}

async fn open_mapping(proto: PortMappingProtocol, port: u16, label: String) -> Result<String> {
    tokio::task::spawn_blocking(move || -> Result<String> {
        let local = lan_ip().ok_or_else(|| anyhow!("This computer isn't connected to a network"))?;
        let gw = gateway()?;
        let addr = SocketAddr::new(IpAddr::V4(local), port);
        let desc = format!("ResinCraft: {label}");
        // Some routers only accept permanent mappings (lease 0); those are removed when the server stops.
        let res = match gw.add_port(proto, port, addr, LEASE_SECS, &desc) {
            Err(AddPortError::OnlyPermanentLeasesSupported) => gw.add_port(proto, port, addr, 0, &desc),
            r => r,
        };
        res.map_err(|e| match e {
            AddPortError::PortInUse => anyhow!("Another device on your network already uses port {port}. Pick a different port."),
            AddPortError::ActionNotAuthorized => {
                anyhow!("Your router doesn't let apps open ports. Turn on UPnP in the router's settings, or forward the port manually.")
            }
            e => anyhow!("Your router refused to open the port ({e})."),
        })?;
        let ip = gw.get_external_ip().map_err(|e| anyhow!("The port is open, but the router didn't say its public address ({e})."))?;
        match ip {
            IpAddr::V4(v4) if is_shared(v4) => {
                let _ = gw.remove_port(proto, port);
                Err(anyhow!(
                    "Your internet provider shares one public address between several homes (carrier-grade NAT), so friends can't reach this computer directly."
                ))
            }
            ip => Ok(SocketAddr::new(ip, port).to_string()),
        }
    })
    .await?
}

/// Removes the mapping again (best effort).
pub async fn close_port(port: u16) {
    close_mapping(PortMappingProtocol::TCP, port).await
}

pub async fn close_udp(port: u16) {
    close_mapping(PortMappingProtocol::UDP, port).await
}

async fn close_mapping(proto: PortMappingProtocol, port: u16) {
    let _ = tokio::task::spawn_blocking(move || {
        if let Ok(gw) = gateway() {
            let _ = gw.remove_port(proto, port);
        }
    })
    .await;
}

/// This network's public IPv4 address, as Cloudflare's trace page (also reachable as 1.0.0.1) sees it.
pub async fn public_ipv4() -> Result<Ipv4Addr> {
    for url in ["https://1.1.1.1/cdn-cgi/trace", "https://1.0.0.1/cdn-cgi/trace"] {
        let Ok(text) = crate::core::net::fetch_text(url).await else { continue };
        if let Some(ip) = text.lines().find_map(|l| l.strip_prefix("ip=")).and_then(|ip| ip.trim().parse().ok()) {
            return Ok(ip);
        }
    }
    Err(crate::err_tr!("hostNet.noPublicIp", "Couldn't find this network's public IP address. Check the internet connection"))
}

/// True if nothing on this computer is listening on `port` yet.
pub fn port_free(port: u16) -> bool {
    std::net::TcpListener::bind(("0.0.0.0", port)).is_ok()
}

/// The same for a UDP port (Bedrock).
pub fn udp_port_free(port: u16) -> bool {
    UdpSocket::bind(("0.0.0.0", port)).is_ok()
}
