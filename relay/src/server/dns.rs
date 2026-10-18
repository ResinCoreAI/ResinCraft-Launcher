//! Players' own domains: `play.example.com CNAME smp.rscraft.resincore.tech` makes Minecraft connect to the relay,
//! but the handshake still says play.example.com. For such hosts the relay asks a public resolver for the
//! name's A record and follows the CNAME chain in the answer to a `<name>.<base_domain>` target. Answers are
//! cached (found: 5 minutes, not found: 1 minute). (An SRV record needs none of this: Minecraft then puts the
//! SRV target, already `<name>.<base_domain>`, into the handshake.)

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tokio::net::UdpSocket;

const TYPE_A: u16 = 1;
const TYPE_CNAME: u16 = 5;
const FOUND_TTL: Duration = Duration::from_secs(300);
const MISSING_TTL: Duration = Duration::from_secs(60);
const CACHE_MAX: usize = 10_000;

pub struct Resolver {
    servers: Vec<SocketAddr>,
    cache: Mutex<HashMap<String, (Option<String>, Instant)>>,
}

impl Resolver {
    pub fn new(servers: &[String]) -> Resolver {
        let servers = servers.iter().filter_map(|s| s.parse().ok()).collect();
        Resolver { servers, cache: Mutex::new(HashMap::new()) }
    }

    /// The tunnel name `host` points at through a CNAME chain, if any.
    pub async fn route(&self, host: &str, base_domain: &str) -> Option<String> {
        if let Some((name, at)) = self.cache.lock().unwrap().get(host).cloned() {
            let ttl = if name.is_some() { FOUND_TTL } else { MISSING_TTL };
            if at.elapsed() < ttl {
                return name;
            }
        }
        let targets = self.cname_chain(host).await;
        let name = targets.iter().find_map(|t| crate::handshake::route_name(t, base_domain));
        let mut cache = self.cache.lock().unwrap();
        if cache.len() >= CACHE_MAX {
            cache.clear();
        }
        cache.insert(host.to_string(), (name.clone(), Instant::now()));
        name
    }

    /// Every CNAME target in the answer to an A query for `host` (empty when the lookup fails).
    async fn cname_chain(&self, host: &str) -> Vec<String> {
        for server in &self.servers {
            if let Ok(Ok(answers)) = tokio::time::timeout(Duration::from_secs(3), query(*server, host)).await {
                return answers;
            }
        }
        Vec::new()
    }
}

async fn query(server: SocketAddr, host: &str) -> std::io::Result<Vec<String>> {
    let bind = if server.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" };
    let sock = UdpSocket::bind(bind).await?;
    sock.connect(server).await?;
    let mut id = [0u8; 2];
    getrandom::getrandom(&mut id).map_err(|e| std::io::Error::other(e.to_string()))?;
    let id = u16::from_be_bytes(id);
    sock.send(&build_query(id, host, TYPE_A)).await?;
    let mut buf = vec![0u8; 4096];
    loop {
        let n = sock.recv(&mut buf).await?;
        if let Some(answers) = parse_cnames(&buf[..n], id) {
            return Ok(answers);
        }
    }
}

pub fn build_query(id: u16, name: &str, qtype: u16) -> Vec<u8> {
    let mut q = Vec::with_capacity(name.len() + 18);
    q.extend_from_slice(&id.to_be_bytes());
    q.extend_from_slice(&[0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0]);
    for label in name.trim_end_matches('.').split('.') {
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.push(0);
    q.extend_from_slice(&qtype.to_be_bytes());
    q.extend_from_slice(&1u16.to_be_bytes());
    q
}

fn u16_at(msg: &[u8], pos: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*msg.get(pos)?, *msg.get(pos + 1)?]))
}

/// A (possibly compressed) name at `pos`, and the position after it.
fn read_name(msg: &[u8], mut pos: usize) -> Option<(String, usize)> {
    let mut labels = Vec::new();
    let mut end = None;
    for _ in 0..64 {
        let len = *msg.get(pos)? as usize;
        if len == 0 {
            return Some((labels.join("."), end.unwrap_or(pos + 1)));
        }
        if len & 0xC0 == 0xC0 {
            end.get_or_insert(pos + 2);
            pos = ((len & 0x3F) << 8) | *msg.get(pos + 1)? as usize;
            continue;
        }
        labels.push(String::from_utf8_lossy(msg.get(pos + 1..pos + 1 + len)?).to_ascii_lowercase());
        pos += 1 + len;
    }
    None
}

/// The CNAME targets in a response to query `id` (None: not our answer or unreadable).
pub fn parse_cnames(msg: &[u8], id: u16) -> Option<Vec<String>> {
    if u16_at(msg, 0)? != id || msg.get(2)? & 0x80 == 0 {
        return None;
    }
    let (qd, an) = (u16_at(msg, 4)?, u16_at(msg, 6)?);
    let mut pos = 12;
    for _ in 0..qd {
        pos = read_name(msg, pos)?.1 + 4;
    }
    let mut out = Vec::new();
    for _ in 0..an {
        let (_, after) = read_name(msg, pos)?;
        let rtype = u16_at(msg, after)?;
        let len = u16_at(msg, after + 8)? as usize;
        let data = after + 10;
        msg.get(data..data + len)?;
        if rtype == TYPE_CNAME {
            out.push(read_name(msg, data)?.0);
        }
        pos = data + len;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_cname_chains() {
        let id = 0x4242;
        let mut msg = build_query(id, "play.example.com", TYPE_A);
        msg[2] = 0x81;
        msg[3] = 0x80;
        msg[7] = 2;
        // play.example.com CNAME smp.rscraft.resincore.tech
        msg.extend_from_slice(&[0xC0, 12, 0, 5, 0, 1, 0, 0, 1, 0x2C]);
        let target = [&[3u8][..], b"smp", &[7], b"rscraft", &[9], b"resincore", &[4], b"tech", &[0]].concat();
        msg.extend_from_slice(&(target.len() as u16).to_be_bytes());
        let target_at = msg.len();
        msg.extend_from_slice(&target);
        // smp.rscraft.resincore.tech A 203.0.113.7 (name compressed to the target above)
        msg.extend_from_slice(&[0xC0, target_at as u8, 0, 1, 0, 1, 0, 0, 1, 0x2C, 0, 4, 203, 0, 113, 7]);
        assert_eq!(parse_cnames(&msg, id), Some(vec!["smp.rscraft.resincore.tech".to_string()]));
        assert_eq!(parse_cnames(&msg, id + 1), None);
        assert_eq!(crate::handshake::route_name("smp.rscraft.resincore.tech", "rscraft.resincore.tech").as_deref(), Some("smp"));
        assert_eq!(parse_cnames(&msg[..msg.len() - 3], id), None);
    }
}
