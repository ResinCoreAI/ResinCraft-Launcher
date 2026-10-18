//! The player's own domain for a hosted server (e.g. play.example.com instead of a raw address): which DNS
//! records it needs for the server's current route, and whether the public DNS has them. Lookups go to a
//! public resolver (1.1.1.1, else 8.8.8.8) over plain UDP, so the computer's own DNS cache can't hide changes.
//! Optional: keeping the records up to date through Cloudflare (see cloudflare.rs).

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{bail, Result};
use tokio::net::UdpSocket;

use crate::core::fsutil::now_ms;
use crate::core::text::{text_of, Text};
use crate::core::types::{DnsCheck, DnsRecord, DnsRecordCheck, DomainInfo};
use crate::{err_tr, tr};

use super::{cloudflare, playit, runtime, store};

/// Minecraft's default port: no SRV record needed for it.
const DEFAULT_PORT: u16 = 25565;
const SRV_PRIORITY: u16 = 0;
const SRV_WEIGHT: u16 = 5;
const RESOLVERS: [IpAddr; 2] = [IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))];

const TYPE_A: u16 = 1;
const TYPE_CNAME: u16 = 5;
const TYPE_AAAA: u16 = 28;
const TYPE_SRV: u16 = 33;

/// Lowercase ASCII (IDN as punycode), without scheme, path, port or trailing dot.
pub fn normalize(input: &str) -> Result<String> {
    let invalid = || err_tr!("hostDomain.invalid", "\"{domain}\" isn't a domain name like play.example.com", domain = input.trim());
    let mut s = input.trim();
    if let Some((_, rest)) = s.split_once("://") {
        s = rest;
    }
    s = s.split(['/', '?', '#']).next().unwrap_or_default();
    if s.parse::<IpAddr>().is_ok() || s.starts_with('[') {
        return Err(err_tr!("hostDomain.notIp", "Enter a domain name, not an IP address"));
    }
    if let Some((host, port)) = s.rsplit_once(':') {
        if port.parse::<u16>().is_err() {
            return Err(invalid());
        }
        s = host;
    }
    let s = s.trim_end_matches('.');
    if s.parse::<IpAddr>().is_ok() {
        return Err(err_tr!("hostDomain.notIp", "Enter a domain name, not an IP address"));
    }
    let ascii = idna::domain_to_ascii(&s.to_lowercase()).map_err(|_| invalid())?;
    let labels: Vec<&str> = ascii.split('.').collect();
    let label_ok = |l: &&str| {
        (1..=63).contains(&l.len())
            && l.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && !l.starts_with('-')
            && !l.ends_with('-')
    };
    let tld_ok = labels.last().is_some_and(|t| !t.bytes().all(|b| b.is_ascii_digit()));
    if ascii.len() > 253 || labels.len() < 2 || !labels.iter().all(label_ok) || !tld_ok {
        return Err(invalid());
    }
    Ok(ascii)
}

/// How friends reach the server right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// The public IP and port (router opened with UPnP, or forwarded by the player).
    Direct { ip: Ipv4Addr, ipv6: Option<Ipv6Addr>, port: u16 },
    /// ResinCraft Relay: `<name>.rscraft.resincore.tech`, routed by name on port 25565.
    Resin { target: String },
    /// playit.gg: the tunnel's host and port.
    Playit { target: String, port: u16 },
    /// A relay with a port that changes every start: no domain possible.
    Relay,
    /// Not running, or no public address yet.
    None,
}

impl Route {
    fn name(&self) -> &'static str {
        match self {
            Route::Direct { .. } => "direct",
            Route::Resin { .. } => "resin",
            Route::Playit { .. } => "playit",
            Route::Relay => "relay",
            Route::None => "none",
        }
    }
}

fn srv_name(domain: &str) -> String {
    format!("_minecraft._tcp.{domain}")
}

fn srv(domain: &str, target: &str, port: u16) -> DnsRecord {
    DnsRecord { kind: "SRV", name: srv_name(domain), value: target.to_string(), port: Some(port), priority: Some(SRV_PRIORITY), weight: Some(SRV_WEIGHT) }
}

/// The records `domain` needs for `route`.
pub fn expected_records(domain: &str, route: &Route) -> Vec<DnsRecord> {
    match route {
        Route::Direct { ip, ipv6, port } => {
            let mut out = vec![DnsRecord { kind: "A", name: domain.to_string(), value: ip.to_string(), port: None, priority: None, weight: None }];
            if let Some(v6) = ipv6 {
                out.push(DnsRecord { kind: "AAAA", name: domain.to_string(), value: v6.to_string(), port: None, priority: None, weight: None });
            }
            if *port != DEFAULT_PORT {
                out.push(srv(domain, domain, *port));
            }
            out
        }
        // The relay routes by the name in the handshake: it follows a CNAME to <name>.rscraft.resincore.tech.
        Route::Resin { target } => vec![DnsRecord { kind: "CNAME", name: domain.to_string(), value: target.clone(), port: None, priority: None, weight: None }],
        Route::Playit { target, port } => vec![srv(domain, target, *port)],
        Route::Relay | Route::None => Vec::new(),
    }
}

/// Records that work instead of [`expected_records`]: for ResinCraft Relay, an SRV record (a domain without a
/// subdomain can't have a CNAME). Minecraft then puts the SRV target into the handshake itself.
pub fn alt_records(domain: &str, route: &Route) -> Vec<DnsRecord> {
    match route {
        Route::Resin { target } => vec![srv(domain, target, DEFAULT_PORT)],
        _ => Vec::new(),
    }
}

/// An SRV record Minecraft would follow before anything else; when one exists it has to point here too.
fn srv_guard(domain: &str, route: &Route) -> Option<DnsRecord> {
    match route {
        Route::Direct { port: DEFAULT_PORT, .. } => Some(srv(domain, domain, DEFAULT_PORT)),
        Route::Resin { target } => Some(srv(domain, target, DEFAULT_PORT)),
        _ => None,
    }
}

/// Direct route: a hosts-file line, so players can map the domain on their own device without any DNS.
pub fn hosts_line(domain: &str, route: &Route) -> Option<String> {
    match route {
        Route::Direct { ip, .. } => Some(format!("{ip} {domain}")),
        _ => None,
    }
}

// ---------- A small DNS client ----------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    A(Ipv4Addr),
    Aaaa(Ipv6Addr),
    Srv { priority: u16, weight: u16, port: u16, target: String },
    Cname(String),
    Other,
}

fn build_query(id: u16, name: &str, qtype: u16) -> Vec<u8> {
    let mut q = Vec::with_capacity(name.len() + 18);
    q.extend_from_slice(&id.to_be_bytes());
    q.extend_from_slice(&[0x01, 0x00]); // recursion desired
    q.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 0]); // one question
    for label in name.trim_end_matches('.').split('.') {
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.push(0);
    q.extend_from_slice(&qtype.to_be_bytes());
    q.extend_from_slice(&1u16.to_be_bytes()); // class IN
    q
}

/// Reads a (possibly compressed) name at `pos`; returns it and the position after it in the original data.
fn read_name(msg: &[u8], mut pos: usize) -> Result<(String, usize)> {
    let mut labels = Vec::new();
    let mut end = None;
    for _ in 0..64 {
        let len = *msg.get(pos).ok_or_else(|| anyhow::anyhow!("short DNS name"))? as usize;
        if len == 0 {
            return Ok((labels.join("."), end.unwrap_or(pos + 1)));
        }
        if len & 0xC0 == 0xC0 {
            let low = *msg.get(pos + 1).ok_or_else(|| anyhow::anyhow!("short DNS pointer"))? as usize;
            end.get_or_insert(pos + 2);
            pos = ((len & 0x3F) << 8) | low;
            continue;
        }
        let label = msg.get(pos + 1..pos + 1 + len).ok_or_else(|| anyhow::anyhow!("short DNS label"))?;
        labels.push(String::from_utf8_lossy(label).to_lowercase());
        pos += 1 + len;
    }
    bail!("DNS name loops")
}

fn u16_at(msg: &[u8], pos: usize) -> Result<u16> {
    let b = msg.get(pos..pos + 2).ok_or_else(|| anyhow::anyhow!("short DNS message"))?;
    Ok(u16::from_be_bytes([b[0], b[1]]))
}

/// The answers of a response to query `id`. A name that doesn't exist (NXDOMAIN) has no answers.
fn parse_response(msg: &[u8], id: u16) -> Result<Vec<Answer>> {
    if u16_at(msg, 0)? != id || msg[2] & 0x80 == 0 {
        bail!("not our DNS answer");
    }
    let rcode = msg[3] & 0x0F;
    if rcode == 3 {
        return Ok(Vec::new());
    }
    if rcode != 0 {
        bail!("DNS error {rcode}");
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
        let rdata = msg.get(data..data + len).ok_or_else(|| anyhow::anyhow!("short DNS record"))?;
        out.push(match (rtype, len) {
            (TYPE_A, 4) => Answer::A(Ipv4Addr::new(rdata[0], rdata[1], rdata[2], rdata[3])),
            (TYPE_AAAA, 16) => Answer::Aaaa(Ipv6Addr::from(<[u8; 16]>::try_from(rdata)?)),
            (TYPE_SRV, 7..) => Answer::Srv {
                priority: u16_at(msg, data)?,
                weight: u16_at(msg, data + 2)?,
                port: u16_at(msg, data + 4)?,
                target: read_name(msg, data + 6)?.0,
            },
            (TYPE_CNAME, _) => Answer::Cname(read_name(msg, data)?.0),
            _ => Answer::Other,
        });
        pos = data + len;
    }
    Ok(out)
}

async fn query(server: IpAddr, name: &str, qtype: u16) -> Result<Vec<Answer>> {
    let sock = UdpSocket::bind(("0.0.0.0", 0)).await?;
    sock.connect(SocketAddr::new(server, 53)).await?;
    let mut buf = vec![0u8; 4096];
    for attempt in 0..2u16 {
        let id = ((now_ms() as u16) ^ (attempt << 8)).wrapping_add(qtype);
        sock.send(&build_query(id, name, qtype)).await?;
        if let Ok(Ok(n)) = tokio::time::timeout(Duration::from_secs(3), sock.recv(&mut buf)).await {
            return parse_response(&buf[..n], id);
        }
    }
    bail!("no answer from {server}")
}

/// Looks `name` up at 1.1.1.1, falling back to 8.8.8.8; returns the resolver that answered too.
pub async fn lookup(name: &str, qtype: u16) -> Result<(IpAddr, Vec<Answer>)> {
    for server in RESOLVERS {
        if let Ok(answers) = query(server, name, qtype).await {
            return Ok((server, answers));
        }
    }
    Err(err_tr!("hostDomain.noResolver", "Couldn't reach a public DNS resolver (1.1.1.1 or 8.8.8.8). Check the internet connection."))
}

fn shown(a: &Answer) -> Option<String> {
    match a {
        Answer::A(ip) => Some(ip.to_string()),
        Answer::Aaaa(ip) => Some(ip.to_string()),
        Answer::Srv { priority, weight, port, target } => Some(format!("{priority} {weight} {port} {target}")),
        Answer::Cname(target) => Some(target.clone()),
        Answer::Other => None,
    }
}

fn same_host(a: &str, b: &str) -> bool {
    a.trim_end_matches('.').eq_ignore_ascii_case(b.trim_end_matches('.'))
}

/// Compares what the public DNS says with `expected`. On the direct route with the default port (and through
/// ResinCraft Relay), an SRV record would send players somewhere else, so one that doesn't point where it
/// should fails the check too.
pub async fn check(domain: &str, expected: &[DnsRecord], route: &Route) -> DnsCheck {
    let mut records = Vec::new();
    let mut resolver = None;
    let mut error = None;
    let mut wanted: Vec<(DnsRecord, bool)> = expected.iter().cloned().map(|r| (r, false)).collect();
    if let Some(guard) = srv_guard(domain, route).filter(|g| !expected.contains(g)) {
        wanted.push((guard, true));
    }
    for (record, only_if_present) in wanted {
        let qtype = match record.kind {
            "A" => TYPE_A,
            "AAAA" => TYPE_AAAA,
            "CNAME" => TYPE_CNAME,
            _ => TYPE_SRV,
        };
        match lookup(&record.name, qtype).await {
            Ok((server, answers)) => {
                resolver.get_or_insert(server);
                let ok = answers.iter().any(|a| match (a, record.kind) {
                    (Answer::A(ip), "A") => ip.to_string() == record.value,
                    (Answer::Aaaa(ip), "AAAA") => record.value.parse::<Ipv6Addr>().is_ok_and(|v| v == *ip),
                    (Answer::Srv { port, target, .. }, "SRV") => Some(*port) == record.port && same_host(target, &record.value),
                    (Answer::Cname(target), "CNAME") => same_host(target, &record.value),
                    _ => false,
                });
                let found: Vec<String> = answers.iter().filter_map(shown).collect();
                if only_if_present && found.is_empty() {
                    continue;
                }
                records.push(DnsRecordCheck { record, ok, found });
            }
            Err(e) => {
                error = Some(as_text(&e));
                break;
            }
        }
    }
    let ok = error.is_none() && !records.is_empty() && records.iter().all(|r| r.ok);
    DnsCheck { at: now_ms(), ok, resolver: resolver.map(|r| r.to_string()).unwrap_or_default(), records, error }
}

// ---------- Per server ----------

/// An error as translatable text (the plain message when it isn't one).
fn as_text(e: &anyhow::Error) -> Text {
    text_of(e).cloned().unwrap_or_else(|| Text::plain(e.to_string()))
}

static CHECKS: Mutex<Option<HashMap<String, DnsCheck>>> = Mutex::new(None);
static CF_STATUS: Mutex<Option<HashMap<String, Text>>> = Mutex::new(None);

fn remember<T>(map: &Mutex<Option<HashMap<String, T>>>, id: &str, v: Option<T>) {
    let mut m = map.lock().unwrap();
    let m = m.get_or_insert_with(HashMap::new);
    match v {
        Some(v) => m.insert(id.to_string(), v),
        None => m.remove(id),
    };
}

fn recall<T: Clone>(map: &Mutex<Option<HashMap<String, T>>>, id: &str) -> Option<T> {
    map.lock().unwrap().as_ref().and_then(|m| m.get(id).cloned())
}

/// The tunnel's host and port behind a playit.gg address ("name.joinmc.link" uses an SRV record itself).
async fn playit_target(address: &str) -> (String, u16) {
    if let Some((host, port)) = address.rsplit_once(':').and_then(|(h, p)| Some((h, p.parse().ok()?))) {
        return (host.to_string(), port);
    }
    if let Ok((_, answers)) = lookup(&srv_name(address), TYPE_SRV).await {
        if let Some(Answer::Srv { port, target, .. }) = answers.into_iter().find(|a| matches!(a, Answer::Srv { .. })) {
            return (target, port);
        }
    }
    (address.to_string(), DEFAULT_PORT)
}

/// Checks `expected`, then (when that fails) the alternative records, and returns whichever matched.
async fn check_route(domain: &str, route: &Route) -> DnsCheck {
    let result = check(domain, &expected_records(domain, route), route).await;
    let alt = alt_records(domain, route);
    if result.ok || result.error.is_some() || alt.is_empty() {
        return result;
    }
    let other = check(domain, &alt, route).await;
    if other.ok {
        other
    } else {
        result
    }
}

/// The server's current route, from its live internet state.
pub async fn route_of(id: &str) -> Route {
    let Some(net) = runtime::net(id).filter(|n| n.state == "open") else { return Route::None };
    let Some(address) = net.address else { return Route::None };
    match net.via {
        Some("router" | "manual") => match address.parse::<SocketAddr>() {
            Ok(SocketAddr::V4(a)) => Route::Direct { ip: *a.ip(), ipv6: None, port: a.port() },
            _ => Route::None,
        },
        Some("playit") => {
            let (target, port) = playit_target(&address).await;
            Route::Playit { target, port }
        }
        Some("resin") => Route::Resin { target: address },
        Some("relay") => Route::Relay,
        _ => Route::None,
    }
}

/// Everything the Settings tab shows about the server's domain.
pub async fn info(id: &str) -> Result<DomainInfo> {
    let meta = store::get(id).await?;
    let route = route_of(id).await;
    let records = meta.domain.as_deref().map(|d| expected_records(d, &route)).unwrap_or_default();
    let alt = meta.domain.as_deref().map(|d| alt_records(d, &route)).unwrap_or_default();
    // A check made for another route (the address changed since) says nothing about these records.
    let covers = |c: &DnsCheck, wanted: &[DnsRecord]| !wanted.is_empty() && wanted.iter().all(|r| c.records.iter().any(|x| &x.record == r));
    let check = recall(&CHECKS, id).filter(|c| covers(c, &records) || covers(c, &alt));
    Ok(DomainInfo {
        hosts_line: meta.domain.as_deref().and_then(|d| hosts_line(d, &route)),
        domain: meta.domain,
        route: route.name(),
        records,
        alt_records: alt,
        playit_guest: matches!(route, Route::Playit { .. }) && playit::is_guest(),
        check,
        cloudflare_token: cloudflare::has_token(id).await,
        cloudflare_status: recall(&CF_STATUS, id),
    })
}

/// Checks the domain against the public DNS now ("Check DNS").
pub async fn check_now(id: &str) -> Result<DomainInfo> {
    let meta = store::get(id).await?;
    let Some(domain) = meta.domain.as_deref() else { return info(id).await };
    let route = route_of(id).await;
    let expected = expected_records(domain, &route);
    if expected.is_empty() {
        remember(&CHECKS, id, None);
        runtime::set_domain_ok(id, None);
        return info(id).await;
    }
    let result = check_route(domain, &route).await;
    runtime::set_domain_ok(id, Some(result.ok));
    remember(&CHECKS, id, Some(result));
    info(id).await
}

/// Once the server is reachable: update the records through Cloudflare when that's on, then check quietly.
pub async fn on_open(id: String) {
    let Ok(meta) = store::get(&id).await else { return };
    let Some(domain) = meta.domain.clone() else {
        remember(&CHECKS, &id, None);
        return;
    };
    let route = route_of(&id).await;
    let expected = expected_records(&domain, &route);
    if expected.is_empty() {
        return;
    }
    if meta.dns_auto {
        let status = match cloudflare::token(&id).await {
            Some(token) => match cloudflare::sync(&token, &domain, &expected, srv_guard(&domain, &route)).await {
                Ok(0) => tr!("hostDomain.cfUpToDate", "Cloudflare: the records were already up to date."),
                Ok(n) => {
                    // Cloudflare answers from its own servers right away; give it a moment anyway.
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    tr!("hostDomain.cfUpdated", "Cloudflare: records created or updated ({count}).", count = n)
                }
                Err(e) => tr!("hostDomain.cfFailed", "Cloudflare: {error}", error = as_text(&e)),
            },
            None => tr!("hostDomain.cfNoToken", "Cloudflare: no API token saved yet."),
        };
        remember(&CF_STATUS, &id, Some(status));
    }
    let result = check_route(&domain, &route).await;
    if result.error.is_none() {
        runtime::set_domain_ok(&id, Some(result.ok));
    }
    remember(&CHECKS, &id, Some(result));
}

/// Forgets cached results, e.g. when the domain changed.
pub fn forget(id: &str) {
    remember(&CHECKS, id, None);
    remember(&CF_STATUS, id, None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_domains() {
        assert_eq!(normalize("Play.Example.COM").unwrap(), "play.example.com");
        assert_eq!(normalize("  https://play.example.com/join  ").unwrap(), "play.example.com");
        assert_eq!(normalize("play.example.com:25566").unwrap(), "play.example.com");
        assert_eq!(normalize("play.example.com.").unwrap(), "play.example.com");
        assert_eq!(normalize("bücher.example").unwrap(), "xn--bcher-kva.example");
        assert_eq!(normalize("mc.café.fr").unwrap(), "mc.xn--caf-dma.fr");
        for bad in ["", "example", "192.168.1.20", "1.2.3.4:25565", "[::1]", "-bad.example.com", "a_b.example.com", "play..example.com", "play.example.123", "play.example.com:abc"] {
            assert!(normalize(bad).is_err(), "{bad} should be rejected");
        }
        assert!(normalize(&format!("{}.com", "a".repeat(64))).is_err());
    }

    #[test]
    fn builds_expected_records() {
        let ip = Ipv4Addr::new(203, 0, 113, 7);
        let direct = expected_records("play.example.com", &Route::Direct { ip, ipv6: None, port: DEFAULT_PORT });
        assert_eq!(direct.len(), 1);
        assert_eq!((direct[0].kind, direct[0].name.as_str(), direct[0].value.as_str()), ("A", "play.example.com", "203.0.113.7"));

        let other_port = expected_records("play.example.com", &Route::Direct { ip, ipv6: None, port: 25570 });
        assert_eq!(other_port.len(), 2);
        let s = &other_port[1];
        assert_eq!((s.kind, s.name.as_str(), s.value.as_str(), s.port, s.priority, s.weight), ("SRV", "_minecraft._tcp.play.example.com", "play.example.com", Some(25570), Some(0), Some(5)));

        let v6: Ipv6Addr = "2001:db8::7".parse().unwrap();
        let both = expected_records("play.example.com", &Route::Direct { ip, ipv6: Some(v6), port: DEFAULT_PORT });
        assert_eq!(both.iter().map(|r| r.kind).collect::<Vec<_>>(), ["A", "AAAA"]);

        let playit = expected_records("play.example.com", &Route::Playit { target: "abc.ply.gg".into(), port: 12345 });
        assert_eq!(playit.len(), 1);
        assert_eq!((playit[0].kind, playit[0].value.as_str(), playit[0].port), ("SRV", "abc.ply.gg", Some(12345)));

        let resin = Route::Resin { target: "smp.rscraft.resincore.tech".into() };
        let cname = expected_records("play.example.com", &resin);
        assert_eq!(cname.len(), 1);
        assert_eq!((cname[0].kind, cname[0].name.as_str(), cname[0].value.as_str()), ("CNAME", "play.example.com", "smp.rscraft.resincore.tech"));
        let alt = alt_records("example.com", &resin);
        assert_eq!((alt[0].kind, alt[0].name.as_str(), alt[0].value.as_str(), alt[0].port), ("SRV", "_minecraft._tcp.example.com", "smp.rscraft.resincore.tech", Some(25565)));
        assert_eq!(srv_guard("play.example.com", &resin).map(|r| r.value), Some("smp.rscraft.resincore.tech".to_string()));
        assert_eq!(hosts_line("play.example.com", &resin), None);
        assert!(alt_records("play.example.com", &Route::Direct { ip, ipv6: None, port: 25570 }).is_empty());
        assert!(expected_records("play.example.com", &Route::Relay).is_empty());
        assert!(expected_records("play.example.com", &Route::None).is_empty());
        assert_eq!(hosts_line("play.example.com", &Route::Direct { ip, ipv6: None, port: 25570 }).as_deref(), Some("203.0.113.7 play.example.com"));
        assert_eq!(hosts_line("play.example.com", &Route::Relay), None);
    }

    #[test]
    fn parses_dns_answers() {
        // A response to an SRV query for _minecraft._tcp.play.example.com, with the name compressed.
        let id = 0x1234;
        let mut msg = build_query(id, "_minecraft._tcp.play.example.com", TYPE_SRV);
        msg[2] = 0x81;
        msg[3] = 0x80;
        msg[7] = 2; // two answers
        // Answer 1: SRV 0 5 25570 play.example.com (target points into the question name).
        msg.extend_from_slice(&[0xC0, 12, 0, 33, 0, 1, 0, 0, 0, 60]);
        msg.extend_from_slice(&[0, 8, 0, 0, 0, 5, 0x63, 0xE2, 0xC0, 12 + 16]);
        // Answer 2: A 203.0.113.7 for the same name.
        msg.extend_from_slice(&[0xC0, 12, 0, 1, 0, 1, 0, 0, 0, 60, 0, 4, 203, 0, 113, 7]);
        let answers = parse_response(&msg, id).unwrap();
        assert_eq!(answers[0], Answer::Srv { priority: 0, weight: 5, port: 25570, target: "play.example.com".into() });
        assert_eq!(answers[1], Answer::A(Ipv4Addr::new(203, 0, 113, 7)));
        // NXDOMAIN: no answers, not an error. Another id: rejected.
        let mut nx = build_query(id, "nothing.example.com", TYPE_A);
        nx[2] = 0x81;
        nx[3] = 0x83;
        assert!(parse_response(&nx, id).unwrap().is_empty());
        assert!(parse_response(&nx, id + 1).is_err());
    }

    /// Asks the real public resolvers: `cargo test --lib domain -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn live_check_of_a_domain_without_records() {
        let domain = "resincraft-no-records.example.com";
        let route = Route::Direct { ip: Ipv4Addr::new(203, 0, 113, 7), ipv6: None, port: 25570 };
        let result = check(domain, &expected_records(domain, &route), &route).await;
        println!("{}", serde_json::to_string_pretty(&result).unwrap());
        assert!(result.error.is_none());
        assert!(!result.ok);
        assert_eq!(result.records.len(), 2);
        assert!(result.records.iter().all(|r| !r.ok && r.found.is_empty()));

        // A real SRV record (XMPP uses them too), to exercise the parser on a live answer.
        let (_, answers) = lookup("_xmpp-client._tcp.jabber.org", TYPE_SRV).await.unwrap();
        println!("SRV: {answers:?}");
        assert!(answers.iter().any(|a| matches!(a, Answer::Srv { port, .. } if *port > 0)));
        let (_, answers) = lookup("example.com", TYPE_A).await.unwrap();
        assert!(answers.iter().any(|a| matches!(a, Answer::A(_))));
    }
}
