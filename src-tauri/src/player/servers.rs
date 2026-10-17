//! The instance page's Servers tab: servers.dat (the in-game multiplayer list) and Server List Ping.

use std::io::ErrorKind;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use regex::Regex;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::files::{game_subdir, js_len, write_atomic};
use super::nbt::{self, compound, list, num, str, Compound, NbtFile, Tag};
use crate::core::text::Text;
use crate::core::types::{ServerEntry, ServerPlayers, ServerStatus};
use crate::{bail_tr, err_tr, tr};

async fn servers_file(id: &str) -> Result<PathBuf> {
    Ok(game_subdir(id, ".").await?.join("servers.dat"))
}

async fn read_servers_file(file: &Path) -> Result<Option<NbtFile>> {
    match tokio::fs::read(file).await {
        Ok(bytes) => nbt::parse(&bytes).map(Some).map_err(|e| err_tr!("servers.unreadable", "servers.dat could not be read: {error}", error = e.to_string())),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => bail_tr!("servers.unreadable", "servers.dat could not be read: {error}", error = e.to_string()),
    }
}

pub async fn list_servers(id: &str) -> Result<Vec<ServerEntry>> {
    let Some(file) = read_servers_file(&servers_file(id).await?).await? else { return Ok(Vec::new()) };
    Ok(list(file.root.get("servers"))
        .iter()
        .filter_map(|t| {
            let c = compound(Some(t))?;
            let accept = num(c.get("acceptTextures"));
            Some(ServerEntry {
                name: str(c.get("name")).unwrap_or_default().to_string(),
                ip: str(c.get("ip")).unwrap_or_default().to_string(),
                icon: str(c.get("icon")).filter(|s| !s.is_empty()).map(String::from),
                accept_textures: accept.map(|a| a != 0.0),
                hidden: num(c.get("hidden")).filter(|h| *h != 0.0 && !h.is_nan()).map(|_| true),
            })
        })
        .collect())
}

fn base64_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9+/]+=*$").unwrap())
}

fn to_compound(s: &ServerEntry) -> Result<Tag> {
    if s.ip.trim().is_empty() {
        bail_tr!("servers.addressMissing", "Every server needs an address");
    }
    if js_len(&s.name) > 200 || js_len(&s.ip) > 300 {
        bail_tr!("servers.tooLong", "Server name or address is too long");
    }
    let mut c = Compound::new();
    c.set("name", Tag::String(s.name.clone()));
    c.set("ip", Tag::String(s.ip.trim().to_string()));
    if let Some(icon) = s.icon.as_deref().filter(|i| !i.is_empty()) {
        if !base64_re().is_match(icon) || icon.len() > 1_000_000 {
            bail!("Invalid server icon");
        }
        c.set("icon", Tag::String(icon.to_string()));
    }
    if let Some(accept) = s.accept_textures {
        c.set("acceptTextures", Tag::Byte(accept as i8));
    }
    if s.hidden == Some(true) {
        c.set("hidden", Tag::Byte(1));
    }
    Ok(Tag::Compound(c))
}

/// Rewrites servers.dat with this list, keeping any other data the file had.
pub async fn save_servers(id: &str, servers: &[ServerEntry]) -> Result<()> {
    if servers.len() > 1000 {
        bail!("Invalid server list");
    }
    let path = servers_file(id).await?;
    let existing = read_servers_file(&path).await.ok().flatten();
    let items = servers.iter().map(to_compound).collect::<Result<Vec<_>>>()?;
    let had_file = existing.is_some();
    let (name, mut root) = existing.map(|f| (f.name, f.root)).unwrap_or_default();
    root.set("servers", Tag::list(nbt::COMPOUND, items));
    if had_file {
        tokio::fs::copy(&path, format!("{}_old", path.display())).await?;
    }
    write_atomic(&path, &nbt::write(&NbtFile { name, root, compression: nbt::Compression::None })?).await
}

// ---------- Server List Ping ----------

fn named_color(name: &str) -> Option<char> {
    Some(match name {
        "black" => '0',
        "dark_blue" => '1',
        "dark_green" => '2',
        "dark_aqua" => '3',
        "dark_red" => '4',
        "dark_purple" => '5',
        "gold" => '6',
        "gray" => '7',
        "dark_gray" => '8',
        "blue" => '9',
        "green" => 'a',
        "aqua" => 'b',
        "red" => 'c',
        "light_purple" => 'd',
        "yellow" => 'e',
        "white" => 'f',
        _ => return None,
    })
}

const PALETTE: [(char, [i32; 3]); 16] = [
    ('0', [0, 0, 0]),
    ('1', [0, 0, 170]),
    ('2', [0, 170, 0]),
    ('3', [0, 170, 170]),
    ('4', [170, 0, 0]),
    ('5', [170, 0, 170]),
    ('6', [255, 170, 0]),
    ('7', [170, 170, 170]),
    ('8', [85, 85, 85]),
    ('9', [85, 85, 255]),
    ('a', [85, 255, 85]),
    ('b', [85, 255, 255]),
    ('c', [255, 85, 85]),
    ('d', [255, 85, 255]),
    ('e', [255, 255, 85]),
    ('f', [255, 255, 255]),
];

/// Hex colours don't exist in § codes, so use the nearest of the 16 classic ones.
fn color_code(color: &Value) -> Option<char> {
    let color = color.as_str().filter(|c| !c.is_empty())?;
    if let Some(c) = named_color(color) {
        return Some(c);
    }
    let hex = color.strip_prefix('#').filter(|h| h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()))?;
    let rgb: Vec<i32> = (0..3).map(|i| i32::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap_or(0)).collect();
    let mut best = 'f';
    let mut best_d = i32::MAX;
    for (code, c) in PALETTE {
        let d: i32 = c.iter().zip(&rgb).map(|(v, w)| (v - w).pow(2)).sum();
        if d < best_d {
            best = code;
            best_d = d;
        }
    }
    Some(best)
}

/// JavaScript truthiness of a JSON value.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

/// `String(v)` in JavaScript, for the value types chat components realistically contain.
fn js_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Array(a) => a.iter().map(|x| if x.is_null() { String::new() } else { js_string(x) }).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}

const STYLE_KEYS: [&str; 6] = ["color", "bold", "italic", "underlined", "strikethrough", "obfuscated"];

#[derive(Clone, Default)]
struct Style([Value; 6]);

/// Flattens a chat component into a legacy string with § codes, which the UI knows how to colour.
fn chat_to_legacy(chat: &Value, inherited: &Style) -> String {
    match chat {
        Value::String(s) => s.clone(),
        Value::Array(items) => items.iter().map(|c| chat_to_legacy(c, inherited)).collect(),
        Value::Object(obj) => {
            let mut style = inherited.clone();
            for (i, k) in STYLE_KEYS.iter().enumerate() {
                if let Some(v) = obj.get(*k) {
                    style.0[i] = v.clone();
                }
            }
            let mut codes = String::from("§r");
            if let Some(c) = color_code(&style.0[0]) {
                codes.push('§');
                codes.push(c);
            }
            for (i, code) in [(1, "§l"), (2, "§o"), (3, "§n"), (4, "§m"), (5, "§k")] {
                if truthy(&style.0[i]) {
                    codes.push_str(code);
                }
            }
            let text = [obj.get("text"), obj.get("translate")].into_iter().flatten().find(|v| !v.is_null()).cloned().unwrap_or(Value::String(String::new()));
            let mut out = if truthy(&text) { codes + &js_string(&text) } else { String::new() };
            if let Some(Value::Array(extra)) = obj.get("extra") {
                for c in extra {
                    out.push_str(&chat_to_legacy(c, &style));
                }
            }
            out
        }
        _ => String::new(),
    }
}

fn strip_codes(s: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("§.").unwrap()).replace_all(s, "").into_owned()
}

fn varint(n: i32, out: &mut Vec<u8>) {
    let mut v = n as u32;
    loop {
        let mut b = (v & 0x7f) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 {
            break;
        }
    }
}

fn packet(id: i32, body: &[u8]) -> Vec<u8> {
    let mut inner = Vec::new();
    varint(id, &mut inner);
    inner.extend_from_slice(body);
    let mut out = Vec::new();
    varint(inner.len() as i32, &mut out);
    out.extend(inner);
    out
}

fn mc_string(s: &str, out: &mut Vec<u8>) {
    varint(s.len() as i32, out);
    out.extend_from_slice(s.as_bytes());
}

/// Reads a varint at `pos`: Ok(None) if the buffer doesn't hold all of it yet.
fn read_varint(buf: &[u8], pos: usize) -> Result<Option<(i32, usize)>> {
    let mut value: u32 = 0;
    for i in 0..5 {
        let Some(&b) = buf.get(pos + i) else { return Ok(None) };
        value |= ((b & 0x7f) as u32) << (7 * i);
        if b & 0x80 == 0 {
            return Ok(Some((value as i32, pos + i + 1)));
        }
    }
    bail_tr!("servers.badResponse", "Bad response from server")
}

/// Splits "host", "host:port", "[v6]:port" into parts; port is None when not given.
fn parse_address(address: &str) -> Result<(String, Option<u16>)> {
    static V6: OnceLock<Regex> = OnceLock::new();
    static HP: OnceLock<Regex> = OnceLock::new();
    let a = address.trim();
    let mut m = V6.get_or_init(|| Regex::new(r"^\[([^\]]+)\](?::(\d+))?$").unwrap()).captures(a);
    if m.is_none() && a.matches(':').count() == 1 {
        m = HP.get_or_init(|| Regex::new(r"^([^:]+):(\d+)$").unwrap()).captures(a);
    }
    let host = m.as_ref().map(|c| c[1].to_string()).unwrap_or_else(|| a.to_string());
    let port = m.as_ref().and_then(|c| c.get(2)).map(|p| p.as_str().parse::<f64>().unwrap_or(f64::INFINITY));
    if host.is_empty() || host.chars().any(char::is_whitespace) {
        bail_tr!("servers.invalidAddress", "Invalid server address");
    }
    match port {
        Some(p) if !(1.0..=65535.0).contains(&p) => bail_tr!("servers.invalidPort", "Invalid port"),
        Some(p) => Ok((host, Some(p as u16))),
        None => Ok((host, None)),
    }
}

/// The first `_minecraft._tcp` SRV record for `host`, as (target, port).
pub async fn resolve_srv(host: &str) -> Option<(String, u16)> {
    let name = format!("_minecraft._tcp.{host}");
    #[cfg(windows)]
    {
        tokio::task::spawn_blocking(move || srv_windows(&name)).await.ok().flatten()
    }
    #[cfg(not(windows))]
    {
        srv_udp(&name).await
    }
}

#[cfg(windows)]
fn srv_windows(name: &str) -> Option<(String, u16)> {
    use windows::core::PCWSTR;
    use windows::Win32::NetworkManagement::Dns::{DnsFree, DnsFreeRecordList, DnsQuery_W, DNS_QUERY_STANDARD, DNS_RECORDA, DNS_RECORDW, DNS_TYPE_SRV};

    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    let mut results: *mut DNS_RECORDA = std::ptr::null_mut();
    // SAFETY: `wide` is NUL-terminated; the record list is only read while alive and freed once.
    unsafe {
        let status = DnsQuery_W(PCWSTR(wide.as_ptr()), DNS_TYPE_SRV, DNS_QUERY_STANDARD, None, &mut results, None);
        if status.0 != 0 || results.is_null() {
            return None;
        }
        let mut found = None;
        let mut rec = results as *const DNS_RECORDW;
        while !rec.is_null() {
            if (*rec).wType == DNS_TYPE_SRV.0 {
                let srv = (*rec).Data.SRV;
                if let Ok(target) = srv.pNameTarget.to_string() {
                    found = Some((target.trim_end_matches('.').to_string(), srv.wPort));
                    break;
                }
            }
            rec = (*rec).pNext;
        }
        DnsFree(Some(results as *const core::ffi::c_void), DnsFreeRecordList);
        found
    }
}

/// A minimal DNS client for SRV lookups via the system resolver in /etc/resolv.conf.
#[cfg_attr(windows, allow(dead_code))]
async fn srv_udp(name: &str) -> Option<(String, u16)> {
    let conf = tokio::fs::read_to_string("/etc/resolv.conf").await.ok()?;
    let ns: IpAddr = conf.lines().filter_map(|l| l.trim().strip_prefix("nameserver")).filter_map(|s| s.trim().split('%').next()?.parse().ok()).next()?;
    let mut q = vec![0u8; 12];
    let id = (crate::core::fsutil::now_ms() & 0xffff) as u16;
    q[0..2].copy_from_slice(&id.to_be_bytes());
    q[2] = 0x01; // recursion desired
    q[5] = 1; // one question
    for label in name.trim_end_matches('.').split('.') {
        if label.is_empty() || label.len() > 63 {
            return None;
        }
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.extend_from_slice(&[0, 0, 33, 0, 1]);
    let bind = if ns.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" };
    let sock = tokio::net::UdpSocket::bind(bind).await.ok()?;
    sock.send_to(&q, (ns, 53)).await.ok()?;
    let mut buf = vec![0u8; 4096];
    let n = sock.recv(&mut buf).await.ok()?;
    parse_srv_response(&buf[..n], id)
}

#[cfg_attr(windows, allow(dead_code))]
fn parse_srv_response(msg: &[u8], id: u16) -> Option<(String, u16)> {
    fn u16_at(m: &[u8], p: usize) -> Option<u16> {
        Some(u16::from_be_bytes([*m.get(p)?, *m.get(p + 1)?]))
    }
    /// Reads a (possibly compressed) name; returns it and the offset after it in the original position.
    fn read_name(m: &[u8], mut p: usize) -> Option<(String, usize)> {
        let mut labels = Vec::new();
        let mut end = None;
        for _ in 0..128 {
            let len = *m.get(p)? as usize;
            if len == 0 {
                return Some((labels.join("."), end.unwrap_or(p + 1)));
            }
            if len & 0xc0 == 0xc0 {
                end.get_or_insert(p + 2);
                p = ((len & 0x3f) << 8) | *m.get(p + 1)? as usize;
                continue;
            }
            labels.push(String::from_utf8_lossy(m.get(p + 1..p + 1 + len)?).into_owned());
            p += 1 + len;
        }
        None
    }
    if u16_at(msg, 0)? != id || msg.get(3)? & 0x0f != 0 {
        return None;
    }
    let (qd, an) = (u16_at(msg, 4)?, u16_at(msg, 6)?);
    let mut p = 12;
    for _ in 0..qd {
        p = read_name(msg, p)?.1 + 4;
    }
    for _ in 0..an {
        p = read_name(msg, p)?.1;
        let (ty, rdlen) = (u16_at(msg, p)?, u16_at(msg, p + 8)? as usize);
        let rdata = p + 10;
        if ty == 33 {
            let port = u16_at(msg, rdata + 4)?;
            let (target, _) = read_name(msg, rdata + 6)?;
            return Some((target, port));
        }
        p = rdata + rdlen;
    }
    None
}

#[derive(Default)]
struct PingState {
    status: Option<ServerStatus>,
    status_at: u64,
}

/// A connection error for the Servers tab; other errors keep the system's own wording.
fn io_message(e: &std::io::Error) -> Text {
    match e.kind() {
        ErrorKind::ConnectionRefused => tr!("servers.connectionRefused", "Connection refused"),
        _ => Text::plain(e.to_string()),
    }
}

fn parse_status(json: &Value) -> ServerStatus {
    let players = &json["players"];
    let number = |v: &Value| -> i64 {
        let n = match v {
            Value::Number(n) => n.as_f64().unwrap_or(0.0),
            Value::String(s) if s.trim().is_empty() => 0.0,
            Value::String(s) => s.trim().parse().unwrap_or(0.0),
            Value::Bool(b) => *b as i64 as f64,
            _ => 0.0,
        };
        if n.is_finite() {
            n as i64
        } else {
            0
        }
    };
    let sample = match &players["sample"] {
        Value::Array(a) => a
            .iter()
            .map(|p| {
                let name = p.get("name").filter(|n| !n.is_null()).map(js_string).unwrap_or_default();
                strip_codes(&name)
            })
            .take(12)
            .collect(),
        _ => Vec::new(),
    };
    let or_empty = |v: Option<&Value>| v.filter(|v| !v.is_null()).cloned().unwrap_or(Value::String(String::new()));
    ServerStatus {
        motd: chat_to_legacy(&or_empty(json.get("description")), &Style::default()),
        players: ServerPlayers { online: number(&players["online"]), max: number(&players["max"]), sample },
        version: strip_codes(&chat_to_legacy(&or_empty(json.get("version").and_then(|v| v.get("name"))), &Style::default())),
        latency: 0,
        favicon: json["favicon"]
            .as_str()
            .filter(|f| f.starts_with("data:image/png;base64,"))
            .map(|f| f.chars().filter(|c| !c.is_whitespace()).collect()),
    }
}

/// Connects, asks for the status and measures a ping. Fills `st.status` as soon as the status arrives.
async fn exchange(handshake_host: &str, host: &str, port: u16, started: Instant, st: &mut PingState) -> Result<u64> {
    let addrs: Vec<_> = tokio::net::lookup_host((host, port)).await.map_err(|_| err_tr!("servers.unknownHost", "Unknown host"))?.collect();
    if addrs.is_empty() {
        bail_tr!("servers.unknownHost", "Unknown host");
    }
    let mut socket = TcpStream::connect(&addrs[..]).await.map_err(|e| io_message(&e))?;
    let mut hs = Vec::new();
    // Protocol -1 asks "what do you run?"; vanilla answers the status request regardless.
    varint(-1, &mut hs);
    mc_string(handshake_host, &mut hs);
    hs.extend_from_slice(&port.to_be_bytes());
    varint(1, &mut hs);
    let mut first = packet(0x00, &hs);
    first.extend(packet(0x00, &[]));
    socket.write_all(&first).await.map_err(|e| io_message(&e))?;

    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    let mut ping_at = Instant::now();
    loop {
        let n = socket.read(&mut chunk).await.map_err(|e| io_message(&e))?;
        if n == 0 {
            bail_tr!("servers.unreachable", "Can't reach server");
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.len() > 4 * 1024 * 1024 {
            bail_tr!("servers.responseTooLarge", "Response too large");
        }
        loop {
            let Some((len, next)) = read_varint(&buf, 0)? else { break };
            if len < 0 {
                bail_tr!("servers.badResponse", "Bad response from server");
            }
            let end = next + len as usize;
            if buf.len() < end {
                break;
            }
            let body: Vec<u8> = buf[next..end].to_vec();
            buf.drain(..end);
            let Some((id, at)) = read_varint(&body, 0)? else { bail_tr!("servers.badResponse", "Bad response from server") };
            if id == 0x00 && st.status.is_none() {
                st.status_at = started.elapsed().as_millis() as u64;
                let Some((slen, sat)) = read_varint(&body, at)? else { bail_tr!("servers.badResponse", "Bad response from server") };
                let s_end = (sat + slen.max(0) as usize).min(body.len());
                let json: Value = serde_json::from_slice(&body[sat.min(s_end)..s_end])?;
                st.status = Some(parse_status(&json));
                let payload = crate::core::fsutil::now_ms().to_be_bytes();
                ping_at = Instant::now();
                socket.write_all(&packet(0x01, &payload)).await.map_err(|e| io_message(&e))?;
            } else if id == 0x01 && st.status.is_some() {
                return Ok(ping_at.elapsed().as_millis() as u64);
            }
        }
    }
}

/// Asks a server for its MOTD, player count and icon using the Server List Ping protocol.
pub async fn ping_server(address: &str, timeout: Duration) -> Result<ServerStatus> {
    let (parsed_host, parsed_port) = parse_address(address)?;
    let mut host = parsed_host.clone();
    let mut port = parsed_port.unwrap_or(25565);
    // Like the game, honour a _minecraft._tcp SRV record when no port is given.
    if parsed_port.is_none() && host.parse::<IpAddr>().is_err() {
        if let Ok(Some((h, p))) = tokio::time::timeout(Duration::from_secs(2), resolve_srv(&host)).await {
            host = h;
            port = p;
        }
    }
    let mut st = PingState::default();
    let started = Instant::now();
    let outcome = tokio::time::timeout(timeout, exchange(&parsed_host, &host, port, started, &mut st)).await;
    let err = match outcome {
        Ok(Ok(latency)) => {
            let mut s = st.status.take().expect("status before pong");
            s.latency = latency;
            return Ok(s);
        }
        Ok(Err(e)) => e,
        Err(_) => err_tr!("servers.timedOut", "Timed out"),
    };
    // Got the status but not the pong: still a useful answer.
    match st.status {
        Some(mut s) => {
            s.latency = st.status_at;
            Ok(s)
        }
        None => Err(err),
    }
}
