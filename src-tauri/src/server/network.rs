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
    tokio::task::spawn_blocking(move || -> Result<String> {
        let local = lan_ip().ok_or_else(|| anyhow!("This computer isn't connected to a network"))?;
        let gw = gateway()?;
        let addr = SocketAddr::new(IpAddr::V4(local), port);
        let desc = format!("ResinCraft: {label}");
        // Some routers only accept permanent mappings (lease 0); those are removed when the server stops.
        let res = match gw.add_port(PortMappingProtocol::TCP, port, addr, LEASE_SECS, &desc) {
            Err(AddPortError::OnlyPermanentLeasesSupported) => gw.add_port(PortMappingProtocol::TCP, port, addr, 0, &desc),
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
                let _ = gw.remove_port(PortMappingProtocol::TCP, port);
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
    let _ = tokio::task::spawn_blocking(move || {
        if let Ok(gw) = gateway() {
            let _ = gw.remove_port(PortMappingProtocol::TCP, port);
        }
    })
    .await;
}

/// True if nothing on this computer is listening on `port` yet.
pub fn port_free(port: u16) -> bool {
    std::net::TcpListener::bind(("0.0.0.0", port)).is_ok()
}
