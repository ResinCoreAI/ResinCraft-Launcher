//! server.properties (Java properties format, comments and order kept) and the player lists:
//! whitelist.json, ops.json, banned-players.json, banned-ips.json.

use std::path::Path;

use anyhow::{anyhow, bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::core::net::{fetch_json, status_of};
use crate::core::types::{PropEntry, ServerListEntry};
use crate::player::files::write_atomic;

use super::{runtime, store};

// ---------- Properties format ----------

enum Line {
    Other(String),
    Pair(String, String),
}

/// Undoes Java properties escapes: \uXXXX, \t, \n, \r, \f and backslash-escaped characters.
fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    Some(ch) => out.push(ch),
                    None => out.push_str(&hex),
                }
            }
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('f') => out.push('\u{c}'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// Escapes like Java's Properties.store: non-ASCII as \uXXXX, separators and comment characters escaped.
fn escape(s: &str, key: bool) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{c}' => out.push_str("\\f"),
            '=' | ':' | '#' | '!' => {
                out.push('\\');
                out.push(c);
            }
            ' ' if key || i == 0 => out.push_str("\\ "),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{unit:04X}"));
                }
            }
            c => out.push(c),
        }
    }
    out
}

fn parse(text: &str) -> Vec<Line> {
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.trim_start();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            out.push(Line::Other(raw.to_string()));
            continue;
        }
        // The key ends at the first unescaped '=', ':' or whitespace.
        let mut end = line.len();
        let mut escaped = false;
        for (i, c) in line.char_indices() {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '=' || c == ':' || c.is_whitespace() {
                end = i;
                break;
            }
        }
        let key = unescape(&line[..end]);
        let mut rest = line[end..].trim_start();
        if rest.starts_with('=') || rest.starts_with(':') {
            rest = rest[1..].trim_start();
        }
        out.push(Line::Pair(key, unescape(rest)));
    }
    out
}

fn render(lines: &[Line]) -> String {
    let mut s = String::new();
    for l in lines {
        match l {
            Line::Other(t) => s.push_str(t),
            Line::Pair(k, v) => {
                s.push_str(&escape(k, true));
                s.push('=');
                s.push_str(&escape(v, false));
            }
        }
        s.push('\n');
    }
    s
}

async fn load(dir: &Path) -> Vec<Line> {
    parse(&tokio::fs::read_to_string(dir.join("server.properties")).await.unwrap_or_default())
}

pub async fn read(dir: &Path) -> Vec<PropEntry> {
    load(dir)
        .await
        .into_iter()
        .filter_map(|l| match l {
            Line::Pair(key, value) => Some(PropEntry { key, value }),
            Line::Other(_) => None,
        })
        .collect()
}

pub async fn get_value(dir: &Path, key: &str) -> Option<String> {
    read(dir).await.into_iter().find(|e| e.key == key).map(|e| e.value)
}

/// Sets some keys, keeping every other line (and comments) as it is.
pub async fn set_values(dir: &Path, values: &[(&str, String)]) -> Result<()> {
    let mut lines = load(dir).await;
    for (k, v) in values {
        let existing = lines.iter_mut().find_map(|l| match l {
            Line::Pair(key, value) if key == k => Some(value),
            _ => None,
        });
        match existing {
            Some(value) => *value = v.clone(),
            None => lines.push(Line::Pair(k.to_string(), v.clone())),
        }
    }
    write_atomic(&dir.join("server.properties"), render(&lines).as_bytes()).await
}

/// Replaces all properties with `entries` (comments stay at the top); keys not listed are removed.
pub async fn write_all(id: &str, entries: &[PropEntry]) -> Result<()> {
    let dir = store::dir(id)?;
    let mut lines: Vec<Line> = load(&dir).await.into_iter().filter(|l| matches!(l, Line::Other(t) if t.trim_start().starts_with('#'))).collect();
    for e in entries {
        let key = e.key.trim();
        if key.is_empty() {
            continue;
        }
        if key.contains(['\n', '\r']) {
            bail!("Invalid property name: {key}");
        }
        lines.push(Line::Pair(key.to_string(), e.value.clone()));
    }
    write_atomic(&dir.join("server.properties"), render(&lines).as_bytes()).await?;
    // Keep the port in server.json in step (it's written into server.properties on every start).
    if let Some(port) = entries.iter().find(|e| e.key == "server-port").and_then(|e| e.value.trim().parse::<u16>().ok()) {
        let mut meta = store::get(id).await?;
        if meta.port != port && port > 0 {
            meta.port = port;
            store::save(&meta).await?;
        }
    }
    Ok(())
}

// ---------- Player lists ----------

#[derive(Clone, Copy, PartialEq)]
enum ListKind {
    Whitelist,
    Ops,
    Bans,
    IpBans,
}

fn list_kind(kind: &str) -> Result<ListKind> {
    Ok(match kind {
        "whitelist" => ListKind::Whitelist,
        "ops" => ListKind::Ops,
        "bans" => ListKind::Bans,
        "ipBans" => ListKind::IpBans,
        _ => bail!("Unknown list: {kind}"),
    })
}

fn list_file(kind: ListKind) -> &'static str {
    match kind {
        ListKind::Whitelist => "whitelist.json",
        ListKind::Ops => "ops.json",
        ListKind::Bans => "banned-players.json",
        ListKind::IpBans => "banned-ips.json",
    }
}

async fn read_raw(dir: &Path, kind: ListKind) -> Vec<Value> {
    let text = tokio::fs::read_to_string(dir.join(list_file(kind))).await.unwrap_or_default();
    serde_json::from_str(&text).unwrap_or_default()
}

pub async fn list_entries(id: &str, kind: &str) -> Result<Vec<ServerListEntry>> {
    let kind = list_kind(kind)?;
    let dir = store::dir(id)?;
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    Ok(read_raw(&dir, kind)
        .await
        .iter()
        .filter_map(|v| {
            Some(ServerListEntry {
                name: s(v, if kind == ListKind::IpBans { "ip" } else { "name" })?,
                uuid: s(v, "uuid"),
                level: v.get("level").and_then(Value::as_u64).map(|l| l as u8),
                reason: s(v, "reason"),
            })
        })
        .collect())
}

fn valid_name(name: &str) -> bool {
    (1..=16).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The UUID an offline-mode server gives a player: a version-3 UUID of "OfflinePlayer:<name>".
fn offline_uuid(name: &str) -> String {
    use md5::Digest;
    let mut b: [u8; 16] = md5::Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    b[6] = (b[6] & 0x0f) | 0x30;
    b[8] = (b[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(b).hyphenated().to_string()
}

/// Looks up a player's UUID (and the exact spelling of their name) for an online-mode server.
pub(crate) async fn lookup(name: &str) -> Result<(String, String)> {
    #[derive(Deserialize)]
    struct Profile {
        id: String,
        name: String,
    }
    let p: Profile = match fetch_json(&format!("https://api.mojang.com/users/profiles/minecraft/{name}")).await {
        Err(e) if matches!(status_of(&e), Some(204 | 404)) => bail!("No Minecraft account is called {name}"),
        Err(_) => bail!("Couldn't look up {name}. Check your internet connection."),
        Ok(p) => p,
    };
    let id = uuid::Uuid::parse_str(&p.id).map_err(|_| anyhow!("Unexpected answer looking up {name}"))?;
    Ok((id.hyphenated().to_string(), p.name))
}

fn command_for(kind: ListKind, add: bool, name: &str) -> String {
    let verb = match (kind, add) {
        (ListKind::Whitelist, true) => "whitelist add",
        (ListKind::Whitelist, false) => "whitelist remove",
        (ListKind::Ops, true) => "op",
        (ListKind::Ops, false) => "deop",
        (ListKind::Bans, true) => "ban",
        (ListKind::Bans, false) => "pardon",
        (ListKind::IpBans, true) => "ban-ip",
        (ListKind::IpBans, false) => "pardon-ip",
    };
    format!("{verb} {name}")
}

/// Adds a player (or an address, for IP bans). A running server is told with its own command, which also
/// updates the file; a stopped server's file is edited directly.
pub async fn add(id: &str, kind: &str, name: &str) -> Result<()> {
    let kind = list_kind(kind)?;
    let name = name.trim();
    if kind == ListKind::IpBans {
        name.parse::<std::net::IpAddr>().map_err(|_| anyhow!("{name} isn't an IP address"))?;
    } else if !valid_name(name) {
        bail!("{name} isn't a valid player name");
    }
    if runtime::is_running(id) {
        runtime::command(id, &command_for(kind, true, name))?;
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;
        return Ok(());
    }
    let dir = store::dir(id)?;
    let mut list = read_raw(&dir, kind).await;
    let key = if kind == ListKind::IpBans { "ip" } else { "name" };
    if list.iter().any(|v| v.get(key).and_then(Value::as_str).is_some_and(|n| n.eq_ignore_ascii_case(name))) {
        return Ok(());
    }
    let created = chrono::Local::now().format("%Y-%m-%d %H:%M:%S %z").to_string();
    let entry = if kind == ListKind::IpBans {
        json!({ "ip": name, "created": created, "source": "Server", "expires": "forever", "reason": "Banned by an operator." })
    } else {
        let online = get_value(&dir, "online-mode").await.map(|v| v.trim() != "false").unwrap_or(true);
        let (uuid, name) = if online { lookup(name).await? } else { (offline_uuid(name), name.to_string()) };
        match kind {
            ListKind::Ops => json!({ "uuid": uuid, "name": name, "level": 4, "bypassesPlayerLimit": false }),
            ListKind::Bans => json!({ "uuid": uuid, "name": name, "created": created, "source": "Server", "expires": "forever", "reason": "Banned by an operator." }),
            _ => json!({ "uuid": uuid, "name": name }),
        }
    };
    list.push(entry);
    write_atomic(&dir.join(list_file(kind)), serde_json::to_string_pretty(&list)?.as_bytes()).await
}

pub async fn remove(id: &str, kind: &str, name: &str) -> Result<()> {
    let kind = list_kind(kind)?;
    let name = name.trim();
    if runtime::is_running(id) {
        if kind != ListKind::IpBans && !valid_name(name) {
            bail!("{name} isn't a valid player name");
        }
        runtime::command(id, &command_for(kind, false, name))?;
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;
        return Ok(());
    }
    let dir = store::dir(id)?;
    let key = if kind == ListKind::IpBans { "ip" } else { "name" };
    let list: Vec<Value> =
        read_raw(&dir, kind).await.into_iter().filter(|v| !v.get(key).and_then(Value::as_str).is_some_and(|n| n.eq_ignore_ascii_case(name))).collect();
    write_atomic(&dir.join(list_file(kind)), serde_json::to_string_pretty(&list)?.as_bytes()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_properties() {
        let text = "#Minecraft server properties\n#Sat Oct 18\nmotd=A \\u00A7aGreen Server\nmax-players=20\nlevel-seed=\nresource-pack=https\\://example.com/p.zip\n";
        let lines = parse(text);
        let pairs: Vec<(String, String)> = lines
            .iter()
            .filter_map(|l| match l {
                Line::Pair(k, v) => Some((k.clone(), v.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(pairs[0], ("motd".into(), "A \u{a7}aGreen Server".into()));
        assert_eq!(pairs[2], ("level-seed".into(), String::new()));
        assert_eq!(pairs[3].1, "https://example.com/p.zip");
        assert_eq!(render(&lines), text.replace("\\u00A7", "\\u00A7"));
    }

    #[test]
    fn offline_uuids_match_minecraft() {
        // Known value: offline UUID of "Notch".
        assert_eq!(offline_uuid("Notch"), "b50ad385-829d-3141-a216-7e7d7539ba7f");
    }
}
