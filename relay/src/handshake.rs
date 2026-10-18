//! The Minecraft: Java Edition bits the relay needs: reading the handshake a player sends first (to route by
//! the server address they typed), and the few packets it answers itself while a tunnel is offline (the
//! server list ping and a login disconnect).
//!
//! Handshake: VarInt length, VarInt packet id 0x00, VarInt protocol version, String server address (VarInt
//! byte length + UTF-8), unsigned short port, VarInt next state (1 status, 2 login, 3 transfer). Forge and
//! NeoForge clients append "\0FML…\0" style markers to the address; a trailing dot comes from fully qualified
//! names. Pre-1.7 clients start their server list ping with 0xFE instead (vanilla servers treat any 0xFE
//! first byte that way too).

use serde_json::{json, Value};

/// Longest handshake packet body accepted. Real ones are under 300 bytes, Forge markers included.
pub const MAX_HANDSHAKE: usize = 1100;
/// Longest server address accepted inside it.
pub const MAX_ADDRESS: usize = 1024;
/// Longest packet accepted while answering a status ping ourselves.
pub const MAX_STATUS_PACKET: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handshake {
    pub protocol: i32,
    /// The server address exactly as sent (see [`normalize_host`]).
    pub address: String,
    pub port: u16,
    /// 1 = status (server list ping), 2 = login, 3 = transfer (1.20.5+).
    pub next_state: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Parsed {
    /// More bytes are needed.
    Incomplete,
    /// A pre-1.7 server list ping (first byte 0xFE).
    Legacy,
    /// The handshake, and how many bytes of the buffer it took (length prefix included).
    Handshake(Handshake, usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadPacket {
    /// The length prefix is over the limit (or zero).
    TooLong,
    /// A VarInt longer than 5 bytes.
    VarInt,
    /// Not a handshake: another packet id, a bad next state, leftover bytes.
    NotHandshake,
    /// The fields don't fit in the packet, or the address isn't UTF-8.
    Malformed,
}

impl std::fmt::Display for BadPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            BadPacket::TooLong => "packet too long",
            BadPacket::VarInt => "bad VarInt",
            BadPacket::NotHandshake => "not a handshake",
            BadPacket::Malformed => "malformed packet",
        })
    }
}

impl std::error::Error for BadPacket {}

/// Reads a VarInt at the start of `buf`: its value and length, or None when `buf` ends inside it.
pub fn read_varint(buf: &[u8]) -> Result<Option<(i32, usize)>, BadPacket> {
    let mut value: u32 = 0;
    for i in 0..5 {
        let Some(&b) = buf.get(i) else { return Ok(None) };
        value |= u32::from(b & 0x7F) << (7 * i);
        if b & 0x80 == 0 {
            return Ok(Some((value as i32, i + 1)));
        }
    }
    Err(BadPacket::VarInt)
}

/// One packet split off a buffer: (packet id, body after the id, total bytes including the length prefix).
pub type RawPacket<'a> = (i32, &'a [u8], usize);

/// Splits one length-prefixed packet off the start of `buf`.
pub fn read_packet(buf: &[u8], max: usize) -> Result<Option<RawPacket<'_>>, BadPacket> {
    let Some((len, n)) = read_varint(buf)? else { return Ok(None) };
    if len <= 0 || len as usize > max {
        return Err(BadPacket::TooLong);
    }
    let total = n + len as usize;
    let Some(packet) = buf.get(n..total) else { return Ok(None) };
    let (id, m) = read_varint(packet)?.ok_or(BadPacket::Malformed)?;
    Ok(Some((id, &packet[m..], total)))
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn varint(&mut self) -> Result<i32, BadPacket> {
        let (v, n) = read_varint(self.0)?.ok_or(BadPacket::Malformed)?;
        self.0 = &self.0[n..];
        Ok(v)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], BadPacket> {
        if self.0.len() < n {
            return Err(BadPacket::Malformed);
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Ok(head)
    }
}

/// Parses the first packet a modern client sends.
pub fn parse(buf: &[u8]) -> Result<Parsed, BadPacket> {
    match buf.first() {
        None => return Ok(Parsed::Incomplete),
        Some(0xFE) => return Ok(Parsed::Legacy),
        _ => {}
    }
    let Some((id, body, total)) = read_packet(buf, MAX_HANDSHAKE)? else { return Ok(Parsed::Incomplete) };
    if id != 0 {
        return Err(BadPacket::NotHandshake);
    }
    let mut r = Reader(body);
    let protocol = r.varint()?;
    let len = r.varint()?;
    if len < 0 || len as usize > MAX_ADDRESS {
        return Err(BadPacket::Malformed);
    }
    let address = std::str::from_utf8(r.take(len as usize)?).map_err(|_| BadPacket::Malformed)?.to_string();
    let port = r.take(2)?;
    let port = u16::from_be_bytes([port[0], port[1]]);
    let next_state = r.varint()?;
    if !(1..=3).contains(&next_state) || !r.0.is_empty() {
        return Err(BadPacket::NotHandshake);
    }
    Ok(Parsed::Handshake(Handshake { protocol, address, port, next_state }, total))
}

/// The host part of a handshake address: cut at the first NUL (Forge/NeoForge markers), without surrounding
/// spaces or a trailing dot, lowercase.
pub fn normalize_host(address: &str) -> String {
    let host = address.split('\0').next().unwrap_or_default();
    host.trim().trim_end_matches('.').to_ascii_lowercase()
}

/// The tunnel name in `host` when it is exactly `<name>.<base_domain>` with a valid name.
pub fn route_name(host: &str, base_domain: &str) -> Option<String> {
    let base = base_domain.trim().trim_end_matches('.');
    let label = host.strip_suffix(base)?.strip_suffix('.')?;
    (!label.contains('.') && crate::proto::name_syntax_ok(label)).then(|| label.to_string())
}

/// True for something that could be a DNS name we might look up (not an IP literal, sane characters).
pub fn looks_like_domain(host: &str) -> bool {
    host.len() <= 253
        && host.contains('.')
        && host.parse::<std::net::IpAddr>().is_err()
        && host.split('.').all(|l| (1..=63).contains(&l.len()) && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
}

// ---------- Packets the relay sends itself ----------

pub fn write_varint(out: &mut Vec<u8>, value: i32) {
    let mut v = value as u32;
    loop {
        if v & !0x7F == 0 {
            out.push(v as u8);
            return;
        }
        out.push((v & 0x7F) as u8 | 0x80);
        v >>= 7;
    }
}

pub fn write_string(out: &mut Vec<u8>, s: &str) {
    write_varint(out, s.len() as i32);
    out.extend_from_slice(s.as_bytes());
}

/// A length-prefixed packet.
pub fn packet(id: i32, body: &[u8]) -> Vec<u8> {
    let mut inner = Vec::with_capacity(body.len() + 5);
    write_varint(&mut inner, id);
    inner.extend_from_slice(body);
    let mut out = Vec::with_capacity(inner.len() + 5);
    write_varint(&mut out, inner.len() as i32);
    out.extend_from_slice(&inner);
    out
}

/// Status response (status state, 0x00) carrying the server list JSON.
pub fn status_response(json: &str) -> Vec<u8> {
    let mut body = Vec::new();
    write_string(&mut body, json);
    packet(0x00, &body)
}

/// Pong (status state, 0x01) echoing the ping's payload, so the client shows a latency.
pub fn pong(payload: &[u8]) -> Vec<u8> {
    packet(0x01, payload)
}

/// Login disconnect (login state, 0x00): a JSON text component, still JSON on every version.
pub fn login_disconnect(text: &Value) -> Vec<u8> {
    let mut body = Vec::new();
    write_string(&mut body, &text.to_string());
    packet(0x00, &body)
}

/// The server list entry shown while there's no server to forward to. `protocol` is the client's own, so
/// the entry doesn't look like a version mismatch.
pub fn status_json(protocol: i32, title: &str, detail: &str) -> String {
    json!({
        "version": { "name": "ResinCraft Relay", "protocol": protocol },
        "players": { "max": 0, "online": 0 },
        "description": { "text": title, "color": "gray", "extra": [{ "text": format!("\n{detail}"), "color": "dark_gray" }] },
        "enforcesSecureChat": false
    })
    .to_string()
}

/// The text component for a login disconnect.
pub fn disconnect_text(title: &str, detail: &str) -> Value {
    json!({ "text": title, "extra": [{ "text": format!("\n\n{detail}"), "color": "gray" }] })
}

/// Reply to a pre-1.7 server list ping: a kick packet (0xFF) with the 1.4+ "§1" format; protocol 127 makes
/// old clients show it as incompatible.
pub fn legacy_kick(motd: &str) -> Vec<u8> {
    let text = ["§1", "127", "ResinCraft Relay", motd, "0", "0"].join("\u{0}");
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut out = Vec::with_capacity(3 + units.len() * 2);
    out.push(0xFF);
    out.extend_from_slice(&(units.len() as u16).to_be_bytes());
    for u in units {
        out.extend_from_slice(&u.to_be_bytes());
    }
    out
}

// ---------- Building client packets (tests, the launcher's checks) ----------

/// A handshake packet as a client sends it.
pub fn client_handshake(protocol: i32, address: &str, port: u16, next_state: i32) -> Vec<u8> {
    let mut body = Vec::new();
    write_varint(&mut body, protocol);
    write_string(&mut body, address);
    body.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut body, next_state);
    packet(0x00, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn done(buf: &[u8]) -> (Handshake, usize) {
        match parse(buf).unwrap() {
            Parsed::Handshake(h, n) => (h, n),
            other => panic!("expected a handshake, got {other:?}"),
        }
    }

    #[test]
    fn varints() {
        for v in [0, 1, 127, 128, 255, 25565, 2_097_151, i32::MAX, -1] {
            let mut out = Vec::new();
            write_varint(&mut out, v);
            assert_eq!(read_varint(&out).unwrap(), Some((v, out.len())), "{v}");
            assert_eq!(read_varint(&out[..out.len() - 1]).unwrap(), None);
        }
        assert_eq!(read_varint(&[0xFF; 6]), Err(BadPacket::VarInt));
    }

    #[test]
    fn parses_a_normal_handshake() {
        let mut buf = client_handshake(767, "Puirkle.RSCraft.ResinCore.tech", 25565, 2);
        let len = buf.len();
        buf.extend_from_slice(&[0x05, 0x00, 1, 2, 3, 4]); // the next packet, already buffered
        let (h, n) = done(&buf);
        assert_eq!(n, len);
        assert_eq!(h, Handshake { protocol: 767, address: "Puirkle.RSCraft.ResinCore.tech".into(), port: 25565, next_state: 2 });
        let host = normalize_host(&h.address);
        assert_eq!(host, "puirkle.rscraft.resincore.tech");
        assert_eq!(route_name(&host, "rscraft.resincore.tech").as_deref(), Some("puirkle"));
    }

    #[test]
    fn strips_forge_markers_and_trailing_dot() {
        for (address, want) in [
            ("smp.rscraft.resincore.tech\0FML\0", "smp.rscraft.resincore.tech"),
            ("smp.rscraft.resincore.tech\0FML2\0", "smp.rscraft.resincore.tech"),
            ("SMP.rscraft.resincore.tech.\0FML3\0", "smp.rscraft.resincore.tech"),
            ("smp.rscraft.resincore.tech.", "smp.rscraft.resincore.tech"),
            (" smp.rscraft.resincore.tech ", "smp.rscraft.resincore.tech"),
        ] {
            let (h, _) = done(&client_handshake(47, address, 25565, 1));
            assert_eq!(h.address, address);
            assert_eq!(normalize_host(&h.address), want);
            assert_eq!(route_name(want, "rscraft.resincore.tech.").as_deref(), Some("smp"));
        }
    }

    #[test]
    fn routes_only_valid_single_labels() {
        let base = "rscraft.resincore.tech";
        assert_eq!(route_name("rscraft.resincore.tech", base), None);
        assert_eq!(route_name("a.b.rscraft.resincore.tech", base), None);
        assert_eq!(route_name("ab.rscraft.resincore.tech", base), None);
        assert_eq!(route_name("bad_name.rscraft.resincore.tech", base), None);
        assert_eq!(route_name("xrscraft.resincore.tech", base), None);
        assert_eq!(route_name("smp.rscraft.resincore.tech.evil.com", base), None);
        assert_eq!(route_name("my-smp.rscraft.resincore.tech", base).as_deref(), Some("my-smp"));
        assert!(looks_like_domain("play.example.com") && !looks_like_domain("203.0.113.7") && !looks_like_domain("localhost"));
    }

    #[test]
    fn waits_for_more_bytes() {
        let buf = client_handshake(767, "smp.rscraft.resincore.tech", 25565, 1);
        for cut in 0..buf.len() {
            assert_eq!(parse(&buf[..cut]).unwrap(), Parsed::Incomplete, "cut at {cut}");
        }
    }

    #[test]
    fn rejects_oversized_and_garbage_input() {
        // A length prefix far over the limit, before the bytes even arrive.
        let mut big = Vec::new();
        write_varint(&mut big, 100_000);
        assert_eq!(parse(&big), Err(BadPacket::TooLong));
        // An address longer than allowed.
        let long = "a".repeat(MAX_ADDRESS + 1);
        let mut body = Vec::new();
        write_varint(&mut body, 767);
        write_string(&mut body, &long);
        body.extend_from_slice(&25565u16.to_be_bytes());
        write_varint(&mut body, 2);
        assert!(parse(&packet(0, &body)).is_err());
        // HTTP, TLS and random bytes.
        assert!(parse(b"GET / HTTP/1.1\r\nHost: smp.rscraft.resincore.tech\r\nUser-Agent: curl/8.0\r\nAccept: */*\r\n\r\n").is_err());
        assert!(matches!(parse(&[0x16, 0x03, 0x01, 0x02, 0x00, 0x01]), Err(_) | Ok(Parsed::Incomplete)));
        assert_eq!(parse(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]), Err(BadPacket::VarInt));
        assert_eq!(parse(&[0x00]), Err(BadPacket::TooLong));
        // Wrong packet id, bad next state, leftover bytes, broken UTF-8, a string running past the packet.
        assert_eq!(parse(&packet(0x01, &[0; 8])), Err(BadPacket::NotHandshake));
        let mut bad_state = client_handshake(767, "a.b", 1, 7);
        assert_eq!(parse(&bad_state), Err(BadPacket::NotHandshake));
        bad_state = client_handshake(767, "a.b", 1, 2);
        bad_state[0] += 1;
        bad_state.push(0);
        assert_eq!(parse(&bad_state), Err(BadPacket::NotHandshake));
        let mut utf8 = Vec::new();
        write_varint(&mut utf8, 767);
        write_varint(&mut utf8, 2);
        utf8.extend_from_slice(&[0xC3, 0x28, 0x63, 0xDD, 1]);
        assert_eq!(parse(&packet(0, &utf8)), Err(BadPacket::Malformed));
        let mut short = Vec::new();
        write_varint(&mut short, 767);
        write_varint(&mut short, 50);
        short.extend_from_slice(b"abc");
        assert_eq!(parse(&packet(0, &short)), Err(BadPacket::Malformed));
    }

    #[test]
    fn legacy_ping() {
        assert_eq!(parse(&[0xFE, 0x01, 0xFA]).unwrap(), Parsed::Legacy);
        let kick = legacy_kick("Offline");
        assert_eq!(kick[0], 0xFF);
        let units = u16::from_be_bytes([kick[1], kick[2]]) as usize;
        assert_eq!(kick.len(), 3 + units * 2);
        let text: Vec<u16> = kick[3..].chunks(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
        assert!(String::from_utf16(&text).unwrap().starts_with("§1\u{0}127\u{0}ResinCraft Relay\u{0}Offline\u{0}"));
    }

    #[test]
    fn builds_status_packets() {
        let json = status_json(767, "This server is offline", "smp.rscraft.resincore.tech");
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["version"]["protocol"], 767);
        let p = status_response(&json);
        let (id, body, total) = read_packet(&p, 1 << 20).unwrap().unwrap();
        assert_eq!((id, total), (0, p.len()));
        let (len, n) = read_varint(body).unwrap().unwrap();
        assert_eq!(&body[n..n + len as usize], json.as_bytes());
        assert_eq!(pong(&[1, 2, 3, 4, 5, 6, 7, 8]), vec![9, 1, 1, 2, 3, 4, 5, 6, 7, 8]);
    }
}
