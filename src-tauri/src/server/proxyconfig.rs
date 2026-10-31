//! A proxy's backend servers: velocity.toml (Velocity) and config.yml (BungeeCord, Waterfall, Travertine, WaterdogPE).
//! The files are edited in place, line by line, so comments and every setting ResinCraft doesn't know stay as they
//! are; each result is read back before it's written. Proxies read their config once, so a save counts from the next
//! start. It also sets up a local Paper-family backend for its proxy (online-mode off, forwarding).
//!
//! The forwarding secret never leaves this PC: `server:proxy*` isn't in the web console's call list, nothing here logs
//! it or puts it in an error, and `guard_remote_read` keeps the console's file manager away from files holding it.

use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use std::sync::{LazyLock, Mutex};

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::core::fsutil::safe_join;
use crate::core::router::json;
use crate::core::types::{ServerKind, ServerMeta};
use crate::core::Args;
use crate::player::files::write_atomic;

use super::{properties, store};

const UNSUPPORTED: &str = "This config file is written in a way ResinCraft can't edit safely. Use the Files tab.";
const DEFAULT_SECRET_FILE: &str = "forwarding.secret";

/// One server behind the proxy. `join`: players are sent here on joining, in list order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Backend {
    pub name: String,
    pub address: String,
    #[serde(default)]
    pub join: bool,
}

/// An address players connect with that goes to its own server first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForcedHost {
    pub host: String,
    pub servers: Vec<String>,
}

/// What the page edits. `forwarding`: none, legacy (BungeeCord style), modern or bungeeguard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyEdit {
    pub backends: Vec<Backend>,
    #[serde(default)]
    pub forced_hosts: Vec<ForcedHost>,
    pub forwarding: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalChoice {
    id: String,
    name: String,
    kind: ServerKind,
    address: String,
}

/// What one backend needs. `None`: ResinCraft can't see it (it's on another machine, or isn't Paper).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BackendSetup {
    name: String,
    local_id: Option<String>,
    local_name: Option<String>,
    /// Which instructions fit: paper, spigot, modded, bedrock, other or remote.
    advice: &'static str,
    can_apply: bool,
    too_old: bool,
    online_mode_off: Option<bool>,
    forwarding_ok: Option<bool>,
    /// Sharing is on, so it may be reachable without the proxy.
    exposed: bool,
    pending: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProxyView {
    file: &'static str,
    /// velocity.toml doesn't exist until the first start.
    missing: bool,
    #[serde(flatten)]
    edit: ProxyEdit,
    has_secret: bool,
    online_mode: bool,
    pending: bool,
    choices: Vec<LocalChoice>,
    setup: Vec<BackendSetup>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Flavor {
    Velocity,
    Bungee,
    Waterdog,
}

impl Flavor {
    fn of(kind: ServerKind) -> Option<Self> {
        match kind {
            ServerKind::Velocity => Some(Self::Velocity),
            ServerKind::Bungeecord | ServerKind::Waterfall | ServerKind::Travertine => Some(Self::Bungee),
            ServerKind::Waterdogpe => Some(Self::Waterdog),
            _ => None,
        }
    }
    fn file(self) -> &'static str {
        if self == Self::Velocity {
            "velocity.toml"
        } else {
            "config.yml"
        }
    }
    fn parse(self, text: &str) -> Result<ProxyEdit> {
        match self {
            Self::Velocity => Ok(parse_velocity(text)),
            Self::Bungee => parse_bungee(text, false),
            Self::Waterdog => parse_bungee(text, true),
        }
    }
    fn write(self, text: &str, edit: &ProxyEdit) -> Result<String> {
        let out = match self {
            Self::Velocity => write_velocity(text, edit),
            Self::Bungee => write_bungee(text, edit, false)?,
            Self::Waterdog => write_bungee(text, edit, true)?,
        };
        // The file is the proxy's only copy: nothing is written that doesn't read back as what was asked for.
        if self.parse(&out)? != *edit {
            bail!(UNSUPPORTED);
        }
        Ok(out)
    }
    /// A proxy that hasn't started yet: what it would write itself, as far as ResinCraft can stand in for it.
    fn starter(self, port: u16) -> Option<String> {
        match self {
            Self::Velocity => None,
            Self::Bungee => Some(format!("listeners:\n- host: 0.0.0.0:{port}\n")),
            Self::Waterdog => Some(format!("listener:\n  host: 0.0.0.0:{port}\n")),
        }
    }
}

fn ordered(servers: Vec<(String, String)>, join: Vec<String>) -> Vec<Backend> {
    let mut out: Vec<Backend> = Vec::new();
    for name in join {
        if let Some((n, a)) = servers.iter().find(|(n, _)| *n == name) {
            if !out.iter().any(|b| b.name == *n) {
                out.push(Backend { name: n.clone(), address: a.clone(), join: true });
            }
        }
    }
    for (name, address) in servers {
        if !out.iter().any(|b| b.name == name) {
            out.push(Backend { name, address, join: false });
        }
    }
    out
}

fn join_list(edit: &ProxyEdit) -> Vec<String> {
    edit.backends.iter().filter(|b| b.join).map(|b| b.name.clone()).collect()
}

/// `true` when `version` (1.21.4, or a year-numbered 26.1) is at least Minecraft 1.`minor`.
fn mc_at_least(version: &str, minor: u32) -> bool {
    let mut parts = version.split('.').map(|p| p.parse::<u32>());
    match (parts.next(), parts.next()) {
        (Some(Ok(1)), Some(Ok(m))) => m >= minor,
        _ => true,
    }
}

// ---------- TOML, by byte ranges ----------

#[derive(Debug)]
struct TomlItem {
    section: String,
    key: String,
    /// Its whole lines, line ending included.
    lines: Range<usize>,
    value: Range<usize>,
}

#[derive(Debug, Default)]
struct TomlDoc {
    items: Vec<TomlItem>,
    tables: Vec<(String, Range<usize>)>,
}

fn after_line(b: &[u8], from: usize) -> usize {
    b[from.min(b.len())..].iter().position(|&c| c == b'\n').map_or(b.len(), |p| from + p + 1)
}

fn toml_unquote(raw: &str) -> String {
    let raw = raw.trim();
    if raw.len() >= 2 && raw.starts_with('\'') && raw.ends_with('\'') {
        return raw[1..raw.len() - 1].to_string();
    }
    if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
        return raw[1..raw.len() - 1].replace("\\\"", "\"").replace("\\\\", "\\");
    }
    raw.to_string()
}

/// Moves `i` past a value and returns where the value ends.
fn toml_value_end(b: &[u8], i: &mut usize) -> usize {
    let mut depth = 0;
    let mut end = *i;
    while *i < b.len() {
        match b[*i] {
            q @ (b'"' | b'\'') => {
                let triple = [q, q, q];
                if b[*i..].starts_with(&triple) {
                    *i += 3;
                    while *i < b.len() && !b[*i..].starts_with(&triple) {
                        *i += if q == b'"' && b[*i] == b'\\' { 2 } else { 1 };
                    }
                    *i += 3;
                } else {
                    *i += 1;
                    while *i < b.len() && b[*i] != q && b[*i] != b'\n' {
                        *i += if q == b'"' && b[*i] == b'\\' { 2 } else { 1 };
                    }
                    *i += 1;
                }
                *i = (*i).min(b.len());
                end = *i;
                continue;
            }
            b'[' | b'{' => depth += 1,
            b']' | b'}' => depth -= 1,
            b'#' if depth <= 0 => break,
            b'#' => {
                *i = after_line(b, *i);
                continue;
            }
            b'\n' if depth <= 0 => break,
            b'\n' | b' ' | b'\t' | b'\r' => {
                *i += 1;
                continue;
            }
            _ => {}
        }
        *i += 1;
        end = *i;
    }
    end
}

fn scan_toml(text: &str) -> TomlDoc {
    let b = text.as_bytes();
    let mut doc = TomlDoc::default();
    let mut section = String::new();
    let mut i = 0;
    while i < b.len() {
        let start = i;
        while i < b.len() && matches!(b[i], b' ' | b'\t') {
            i += 1;
        }
        match b.get(i) {
            None => break,
            Some(b'#' | b'\n' | b'\r') => i = after_line(b, i),
            Some(b'[') => {
                let end = after_line(b, i);
                let head = text[i..end].split('#').next().unwrap_or_default();
                section = toml_unquote(head.trim().trim_matches(['[', ']']));
                doc.tables.push((section.clone(), start..end));
                i = end;
            }
            Some(_) => {
                let key_start = i;
                let mut quote = None;
                while i < b.len() && b[i] != b'\n' {
                    match quote {
                        Some(q) if b[i] == b'\\' && q == b'"' => i += 1,
                        Some(q) if b[i] == q => quote = None,
                        Some(_) => {}
                        None if matches!(b[i], b'"' | b'\'') => quote = Some(b[i]),
                        None if b[i] == b'=' => break,
                        None => {}
                    }
                    i += 1;
                }
                if b.get(i) != Some(&b'=') {
                    i = after_line(b, i);
                    continue;
                }
                let key = toml_unquote(&text[key_start..i]);
                i += 1;
                while i < b.len() && matches!(b[i], b' ' | b'\t') {
                    i += 1;
                }
                let value_start = i;
                let value_end = toml_value_end(b, &mut i);
                let line_end = after_line(b, i);
                doc.items.push(TomlItem { section: section.clone(), key, lines: start..line_end, value: value_start..value_end });
                i = line_end;
            }
        }
    }
    doc
}

/// The strings in a value: one for a string, any number for an array.
fn toml_strings(raw: &str) -> Vec<String> {
    let b = raw.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'#' => i = after_line(b, i),
            q @ (b'"' | b'\'') => {
                let start = i;
                i += 1;
                while i < b.len() && b[i] != q {
                    i += if q == b'"' && b[i] == b'\\' { 2 } else { 1 };
                }
                i = (i + 1).min(b.len());
                out.push(toml_unquote(&raw[start..i]));
            }
            _ => i += 1,
        }
    }
    out
}

fn toml_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn toml_key(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        s.to_string()
    } else {
        toml_quote(s)
    }
}

#[derive(Clone)]
enum TomlValue {
    Text(String),
    List(Vec<String>),
}

impl TomlValue {
    fn render(&self, eol: &str) -> String {
        match self {
            Self::Text(s) => toml_quote(s),
            Self::List(l) if l.is_empty() => "[]".to_string(),
            Self::List(l) => format!("[{eol}{}{eol}]", l.iter().map(|s| format!("    {}", toml_quote(s))).collect::<Vec<_>>().join(&format!(",{eol}"))),
        }
    }
    fn same_as(&self, raw: &str) -> bool {
        match self {
            Self::Text(s) => toml_strings(raw) == [s.clone()] && !raw.trim_start().starts_with('['),
            Self::List(l) => toml_strings(raw) == *l,
        }
    }
}

fn eol_of(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn with_eol(mut s: String, eol: &str) -> String {
    if !s.ends_with('\n') {
        s.push_str(eol);
    }
    s
}

fn splice(text: &str, mut edits: Vec<(Range<usize>, String)>) -> String {
    edits.sort_by_key(|(r, _)| std::cmp::Reverse(r.start));
    let mut out = text.to_string();
    for (range, with) in edits {
        out.replace_range(range, &with);
    }
    out
}

/// Makes the keys of `[section]` (other than `skip`) exactly `wanted`, in that order. A kept key keeps its line and
/// its trailing comment; comment lines stay where they are.
fn toml_set_table(text: &str, section: &str, wanted: &[(String, TomlValue)], skip: &str) -> String {
    let eol = eol_of(text);
    let doc = scan_toml(text);
    let mine: Vec<&TomlItem> = doc.items.iter().filter(|i| i.section == section && i.key != skip).collect();
    let line = |key: &str, v: &TomlValue| -> String {
        match mine.iter().find(|i| i.key == key) {
            Some(i) if v.same_as(&text[i.value.clone()]) => with_eol(text[i.lines.clone()].to_string(), eol),
            Some(i) => with_eol(format!("{}{}{}", &text[i.lines.start..i.value.start], v.render(eol), &text[i.value.end..i.lines.end]), eol),
            None => format!("{} = {}{eol}", toml_key(key), v.render(eol)),
        }
    };
    let extra: String = wanted.iter().skip(mine.len()).map(|(k, v)| line(k, v)).collect();
    if let Some(last) = mine.last() {
        let mut edits: Vec<(Range<usize>, String)> = Vec::new();
        for (i, item) in mine.iter().enumerate() {
            let mut with = wanted.get(i).map(|(k, v)| line(k, v)).unwrap_or_default();
            if item.lines.start == last.lines.start {
                with.push_str(&extra);
            }
            edits.push((item.lines.clone(), with));
        }
        return splice(text, edits);
    }
    if wanted.is_empty() {
        return text.to_string();
    }
    match doc.tables.iter().find(|(name, _)| name == section) {
        Some((_, head)) => {
            // After the comments under the heading.
            let mut at = head.end;
            while text[at..].trim_start_matches([' ', '\t']).starts_with('#') {
                at = after_line(text.as_bytes(), at);
            }
            splice(text, vec![(at..at, extra)])
        }
        None => format!("{}{eol}[{section}]{eol}{extra}", with_eol(text.to_string(), eol)),
    }
}

/// Sets one key (`section` "" is the top of the file), adding it after the section's last key if it's new.
fn toml_set(text: &str, section: &str, key: &str, value: &TomlValue) -> String {
    let eol = eol_of(text);
    let doc = scan_toml(text);
    if let Some(item) = doc.items.iter().find(|i| i.section == section && i.key == key) {
        if value.same_as(&text[item.value.clone()]) {
            return text.to_string();
        }
        return splice(text, vec![(item.value.clone(), value.render(eol))]);
    }
    let last = doc.items.iter().filter(|i| i.section == section).next_back().map(|i| i.lines.end);
    let at = last.or(doc.tables.iter().find(|(n, _)| n == section).map(|(_, r)| r.end)).unwrap_or(0);
    let lead = if at > 0 && !text[..at].ends_with('\n') { eol } else { "" };
    splice(text, vec![(at..at, format!("{lead}{} = {}{eol}", toml_key(key), value.render(eol)))])
}

fn parse_velocity(text: &str) -> ProxyEdit {
    let doc = scan_toml(text);
    let value = |i: &TomlItem| &text[i.value.clone()];
    let servers = doc.items.iter().filter(|i| i.section == "servers" && i.key != "try").map(|i| (i.key.clone(), toml_strings(value(i)).into_iter().next().unwrap_or_default())).collect();
    let join = doc.items.iter().find(|i| i.section == "servers" && i.key == "try").map(|i| toml_strings(value(i))).unwrap_or_default();
    let forced_hosts = doc.items.iter().filter(|i| i.section == "forced-hosts").map(|i| ForcedHost { host: i.key.clone(), servers: toml_strings(value(i)) }).collect();
    let forwarding = doc.items.iter().find(|i| i.section.is_empty() && i.key == "player-info-forwarding-mode").and_then(|i| toml_strings(value(i)).into_iter().next());
    ProxyEdit { backends: ordered(servers, join), forced_hosts, forwarding: forwarding.unwrap_or_else(|| "none".into()).to_lowercase() }
}

fn write_velocity(text: &str, edit: &ProxyEdit) -> String {
    let servers: Vec<_> = edit.backends.iter().map(|b| (b.name.clone(), TomlValue::Text(b.address.clone()))).collect();
    let hosts: Vec<_> = edit.forced_hosts.iter().map(|h| (h.host.clone(), TomlValue::List(h.servers.clone()))).collect();
    let out = toml_set_table(text, "servers", &servers, "try");
    let out = toml_set(&out, "servers", "try", &TomlValue::List(join_list(edit)));
    let out = toml_set_table(&out, "forced-hosts", &hosts, "");
    if parse_velocity(&out).forwarding == edit.forwarding {
        return out;
    }
    toml_set(&out, "", "player-info-forwarding-mode", &TomlValue::Text(edit.forwarding.to_uppercase()))
}

// ---------- YAML, block style, by lines ----------

#[derive(Debug)]
struct YamlFile {
    lines: Vec<String>,
    eol: &'static str,
    bom: bool,
    final_eol: bool,
}

impl YamlFile {
    fn read(text: &str) -> Self {
        let bom = text.starts_with('\u{feff}');
        let body = text.trim_start_matches('\u{feff}');
        Self { lines: body.lines().map(str::to_string).collect(), eol: eol_of(body), bom, final_eol: body.is_empty() || body.ends_with('\n') }
    }
    fn text(&self) -> String {
        let mut s = String::from(if self.bom { "\u{feff}" } else { "" });
        s.push_str(&self.lines.join(self.eol));
        if self.final_eol {
            s.push_str(self.eol);
        }
        s
    }
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn noise(line: &str) -> bool {
    let t = line.trim();
    t.is_empty() || t.starts_with('#')
}

/// Splits what follows a key or a dash into the value and its trailing comment (with the spaces before it).
fn split_comment(rest: &str) -> (&str, &str) {
    let mut quote = None;
    let mut before = ' ';
    for (i, c) in rest.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '\'' || c == '"' => quote = Some(c),
            None if c == '#' && before.is_whitespace() => {
                let value = rest[..i].trim_end();
                return (value.trim_start(), &rest[value.len()..]);
            }
            None => {}
        }
        before = c;
    }
    (rest.trim(), "")
}

fn unquote(raw: &str) -> String {
    let raw = raw.trim();
    if raw.len() >= 2 && raw.starts_with('\'') && raw.ends_with('\'') {
        return raw[1..raw.len() - 1].replace("''", "'");
    }
    toml_unquote(raw)
}

/// A value as YAML: plain when that can't mean anything but text, like `lobby` or `127.0.0.1:25565`.
fn yaml_scalar(v: &str) -> String {
    let word = v.chars().next().is_some_and(char::is_alphabetic) && v.chars().all(|c| c.is_alphanumeric() || "._-:/".contains(c)) && !v.ends_with(':');
    let boolish = ["y", "n", "yes", "no", "on", "off", "true", "false", "null"].contains(&v.to_lowercase().as_str());
    let ipv4 = v.split(':').next().is_some_and(|h| h.split('.').count() == 4) && v.contains(':') && v.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ':');
    if (word && !boolish) || ipv4 {
        v.to_string()
    } else {
        format!("'{}'", v.replace('\'', "''"))
    }
}

#[derive(Debug)]
struct Entry {
    col: usize,
    key: String,
    colon: usize,
    value: String,
    tail: String,
}

/// `key: value` on a line, also after a list's dash.
fn entry(line: &str) -> Option<Entry> {
    if noise(line) {
        return None;
    }
    let mut col = indent_of(line);
    while line[col..].starts_with("- ") {
        col += 2;
        col += line[col..].len() - line[col..].trim_start_matches(' ').len();
    }
    let s = &line[col..];
    let (key, at) = match s.chars().next() {
        Some(q @ ('\'' | '"')) => {
            let close = s[1..].find(q)? + 1;
            let colon = close + 1 + s[close + 1..].find(':')?;
            (unquote(&s[..=close]), colon)
        }
        _ => {
            let colon = s.char_indices().find(|&(i, c)| c == ':' && s[i + 1..].chars().next().is_none_or(char::is_whitespace))?.0;
            (s[..colon].trim_end().to_string(), colon)
        }
    };
    let colon = col + at;
    let (value, tail) = split_comment(&line[colon + 1..]);
    Some(Entry { col, key, colon, value: value.to_string(), tail: tail.to_string() })
}

/// The line after the block under the key at line `k`, column `col`. Trailing comments belong to what follows.
fn block_end(lines: &[String], k: usize, col: usize) -> usize {
    let mut last = k + 1;
    for (i, line) in lines.iter().enumerate().skip(k + 1) {
        if noise(line) {
            continue;
        }
        let indent = indent_of(line);
        if indent > col || (indent == col && line.trim_start().starts_with("- ")) {
            last = i + 1;
        } else {
            break;
        }
    }
    last
}

fn find(lines: &[String], range: Range<usize>, col: usize, key: &str) -> Option<usize> {
    range.into_iter().find(|&i| entry(&lines[i]).is_some_and(|e| e.col == col && e.key == key))
}

fn child_col(lines: &[String], range: Range<usize>) -> Option<usize> {
    range.into_iter().find(|&i| !noise(&lines[i])).map(|i| entry(&lines[i]).map_or(indent_of(&lines[i]), |e| e.col))
}

fn with_value(line: &str, value: &str) -> Option<String> {
    let e = entry(line)?;
    Some(format!("{} {value}{}", &line[..=e.colon], e.tail))
}

/// The line with its value gone, ready for a block under it (`empty` when there's nothing to put there).
fn head_of(line: &str, empty: &str) -> Option<String> {
    let e = entry(line)?;
    let gap = if empty.is_empty() { "" } else { " " };
    Some(format!("{}{gap}{empty}{}", &line[..=e.colon], e.tail))
}

/// Puts `rendered` in the slot lines, removing spare ones and adding any more after the last (or after `after`).
fn fill(lines: &mut Vec<String>, slots: &[usize], after: usize, rendered: Vec<String>) {
    let anchor = slots.last().copied().unwrap_or(after);
    let extra: Vec<String> = rendered.iter().skip(slots.len()).cloned().collect();
    lines.splice(anchor + 1..anchor + 1, extra);
    for (i, &slot) in slots.iter().enumerate().rev() {
        match rendered.get(i) {
            Some(line) => lines[slot] = line.clone(),
            None => {
                lines.remove(slot);
            }
        }
    }
}

fn list_slots(lines: &[String], k: usize, col: usize) -> Vec<usize> {
    (k + 1..block_end(lines, k, col)).filter(|&i| !noise(&lines[i]) && lines[i].trim_start().starts_with("- ")).collect()
}

fn read_list(lines: &[String], k: usize, col: usize) -> Vec<String> {
    if let Some(e) = entry(&lines[k]) {
        if let Some(flow) = e.value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
            return flow.split(',').map(unquote).filter(|s| !s.is_empty()).collect();
        }
    }
    list_slots(lines, k, col).into_iter().map(|i| unquote(split_comment(lines[i].trim_start().trim_start_matches('-')).0)).collect()
}

fn write_list(lines: &mut Vec<String>, k: usize, col: usize, items: &[String]) -> Result<()> {
    if read_list(lines, k, col) == items {
        return Ok(());
    }
    let slots = list_slots(lines, k, col);
    let pad = " ".repeat(slots.first().map_or(col, |&i| indent_of(&lines[i])));
    lines[k] = head_of(&lines[k], if items.is_empty() { "[]" } else { "" }).ok_or_else(|| anyhow!(UNSUPPORTED))?;
    fill(lines, &slots, k, items.iter().map(|s| format!("{pad}- {}", yaml_scalar(s))).collect());
    Ok(())
}

/// The first listener: the lines it spans, and the column of its keys.
fn listener(lines: &[String], waterdog: bool) -> Result<Option<(Range<usize>, usize)>> {
    let Some(k) = find(lines, 0..lines.len(), 0, if waterdog { "listener" } else { "listeners" }) else { return Ok(None) };
    if entry(&lines[k]).is_none_or(|e| !e.value.is_empty()) {
        bail!(UNSUPPORTED);
    }
    let end = block_end(lines, k, 0);
    let col = child_col(lines, k + 1..end).ok_or_else(|| anyhow!(UNSUPPORTED))?;
    if waterdog {
        return Ok(Some((k + 1..end, col)));
    }
    let first = (k + 1..end).find(|&i| !noise(&lines[i])).ok_or_else(|| anyhow!(UNSUPPORTED))?;
    let dash = indent_of(&lines[first]);
    if !lines[first].trim_start().starts_with("- ") {
        bail!(UNSUPPORTED);
    }
    let mut item_end = (first + 1..end).find(|&i| !noise(&lines[i]) && indent_of(&lines[i]) == dash && lines[i].trim_start().starts_with("- ")).unwrap_or(end);
    while noise(&lines[item_end - 1]) {
        item_end -= 1;
    }
    Ok(Some((first..item_end, col)))
}

#[derive(Debug)]
struct Child {
    name: String,
    /// Comments above it, which travel with it.
    start: usize,
    head: usize,
    end: usize,
}

fn children(lines: &[String], k: usize, col: usize) -> Result<(Vec<Child>, usize)> {
    let e = entry(&lines[k]).ok_or_else(|| anyhow!(UNSUPPORTED))?;
    let end = block_end(lines, k, col);
    if !e.value.is_empty() && e.value != "{}" {
        bail!(UNSUPPORTED);
    }
    let mut out = Vec::new();
    let Some(inner) = child_col(lines, k + 1..end) else { return Ok((out, col + 2)) };
    let (mut prev, mut i) = (k + 1, k + 1);
    while i < end {
        if noise(&lines[i]) {
            i += 1;
            continue;
        }
        let c = entry(&lines[i]).filter(|c| c.col == inner).ok_or_else(|| anyhow!(UNSUPPORTED))?;
        let stop = block_end(lines, i, inner);
        out.push(Child { name: c.key, start: prev, head: i, end: stop });
        prev = stop;
        i = stop;
    }
    Ok((out, inner))
}

fn parse_bungee(text: &str, waterdog: bool) -> Result<ProxyEdit> {
    let lines = YamlFile::read(text).lines;
    let mut servers = Vec::new();
    if let Some(k) = find(&lines, 0..lines.len(), 0, "servers") {
        let (list, inner) = children(&lines, k, 0)?;
        for c in list {
            let col = child_col(&lines, c.head + 1..c.end).unwrap_or(inner + 2);
            let address = find(&lines, c.head + 1..c.end, col, "address").and_then(|i| entry(&lines[i])).map(|e| unquote(&e.value));
            if entry(&lines[c.head]).is_some_and(|e| !e.value.is_empty()) {
                bail!(UNSUPPORTED);
            }
            servers.push((c.name, address.unwrap_or_default()));
        }
    }
    let mut join = Vec::new();
    let mut forced_hosts = Vec::new();
    if let Some((range, col)) = listener(&lines, waterdog)? {
        if let Some(k) = find(&lines, range.clone(), col, "priorities") {
            join = read_list(&lines, k, col);
        }
        if let Some(k) = find(&lines, range, col, "forced_hosts") {
            for c in children(&lines, k, col)?.0 {
                let value = entry(&lines[c.head]).map(|e| unquote(&e.value)).unwrap_or_default();
                forced_hosts.push(ForcedHost { host: c.name, servers: vec![value] });
            }
        }
    }
    let forwarded = !waterdog && find(&lines, 0..lines.len(), 0, "ip_forward").and_then(|i| entry(&lines[i])).is_some_and(|e| e.value == "true");
    Ok(ProxyEdit { backends: ordered(servers, join), forced_hosts, forwarding: if forwarded { "legacy" } else { "none" }.into() })
}

fn server_block(pad: &str, b: &Backend, waterdog: bool) -> Vec<String> {
    let mut out = vec![format!("{pad}{}:", yaml_scalar(&b.name)), format!("{pad}  address: {}", yaml_scalar(&b.address))];
    if !waterdog {
        out.push(format!("{pad}  restricted: false"));
    }
    out
}

fn write_servers(lines: &mut Vec<String>, edit: &ProxyEdit, waterdog: bool) -> Result<()> {
    let Some(k) = find(lines, 0..lines.len(), 0, "servers") else {
        lines.push("servers:".to_string());
        lines.extend(edit.backends.iter().flat_map(|b| server_block("  ", b, waterdog)));
        return Ok(());
    };
    let (list, inner) = children(lines, k, 0)?;
    let end = block_end(lines, k, 0);
    let pad = " ".repeat(inner);
    let mut out = Vec::new();
    for b in &edit.backends {
        let Some(c) = list.iter().find(|c| c.name == b.name) else {
            out.extend(server_block(&pad, b, waterdog));
            continue;
        };
        let mut block = lines[c.start..c.end].to_vec();
        let col = child_col(lines, c.head + 1..c.end).unwrap_or(inner + 2);
        let (head, stop) = (c.head - c.start, c.end - c.start);
        match find(&block, head + 1..stop, col, "address") {
            Some(i) if entry(&block[i]).is_some_and(|e| unquote(&e.value) == b.address) => {}
            Some(i) => block[i] = with_value(&block[i], &yaml_scalar(&b.address)).ok_or_else(|| anyhow!(UNSUPPORTED))?,
            None => block.insert(head + 1, format!("{}address: {}", " ".repeat(col), yaml_scalar(&b.address))),
        }
        out.extend(block);
    }
    lines[k] = head_of(&lines[k], if edit.backends.is_empty() { "{}" } else { "" }).ok_or_else(|| anyhow!(UNSUPPORTED))?;
    lines.splice(k + 1..end, out);
    Ok(())
}

fn write_forced(lines: &mut Vec<String>, range: Range<usize>, col: usize, hosts: &[ForcedHost]) -> Result<()> {
    let rendered = |pad: &str| -> Vec<String> { hosts.iter().map(|h| format!("{pad}{}: {}", yaml_scalar(&h.host), yaml_scalar(h.servers.first().map_or("", String::as_str)))).collect() };
    let Some(k) = find(lines, range.clone(), col, "forced_hosts") else {
        if !hosts.is_empty() {
            let mut add = vec![format!("{}forced_hosts:", " ".repeat(col))];
            add.extend(rendered(&" ".repeat(col + 2)));
            lines.splice(range.end..range.end, add);
        }
        return Ok(());
    };
    let (list, inner) = children(lines, k, col)?;
    let current: Vec<(String, String)> = list.iter().map(|c| (c.name.clone(), entry(&lines[c.head]).map(|e| unquote(&e.value)).unwrap_or_default())).collect();
    let wanted: Vec<(String, String)> = hosts.iter().map(|h| (h.host.clone(), h.servers.first().cloned().unwrap_or_default())).collect();
    if current == wanted {
        return Ok(());
    }
    let slots: Vec<usize> = list.iter().map(|c| c.head).collect();
    lines[k] = head_of(&lines[k], if hosts.is_empty() { "{}" } else { "" }).ok_or_else(|| anyhow!(UNSUPPORTED))?;
    fill(lines, &slots, k, rendered(&" ".repeat(inner)));
    Ok(())
}

fn write_bungee(text: &str, edit: &ProxyEdit, waterdog: bool) -> Result<String> {
    let mut file = YamlFile::read(text);
    let lines = &mut file.lines;
    write_servers(lines, edit, waterdog)?;
    let (range, col) = listener(lines, waterdog)?.ok_or_else(|| anyhow!(UNSUPPORTED))?;
    let join = join_list(edit);
    match find(lines, range.clone(), col, "priorities") {
        Some(k) => write_list(lines, k, col, &join)?,
        None => {
            let pad = " ".repeat(col);
            let mut add = vec![format!("{pad}priorities:")];
            add.extend(join.iter().map(|s| format!("{pad}- {}", yaml_scalar(s))));
            lines.splice(range.end..range.end, add);
        }
    }
    let (range, col) = listener(lines, waterdog)?.ok_or_else(|| anyhow!(UNSUPPORTED))?;
    write_forced(lines, range, col, &edit.forced_hosts)?;
    if !waterdog {
        yaml_set(lines, &["ip_forward"], if edit.forwarding == "none" { "false" } else { "true" })?;
    }
    Ok(file.text())
}

/// Sets a value by its path of keys, making the parents it needs.
fn yaml_set(lines: &mut Vec<String>, path: &[&str], value: &str) -> Result<()> {
    let mut range = 0..lines.len();
    let mut col = 0;
    for (depth, key) in path.iter().enumerate() {
        let Some(k) = find(lines, range.clone(), col, key) else {
            let mut add = Vec::new();
            for (i, part) in path[depth..].iter().enumerate() {
                let pad = " ".repeat(col + i * 2);
                add.push(if depth + i + 1 == path.len() { format!("{pad}{part}: {value}") } else { format!("{pad}{part}:") });
            }
            lines.splice(range.end..range.end, add);
            return Ok(());
        };
        let e = entry(&lines[k]).ok_or_else(|| anyhow!(UNSUPPORTED))?;
        if depth + 1 == path.len() {
            if e.value != value {
                lines[k] = with_value(&lines[k], value).ok_or_else(|| anyhow!(UNSUPPORTED))?;
            }
            return Ok(());
        }
        if !e.value.is_empty() && e.value != "{}" {
            bail!(UNSUPPORTED);
        }
        if !e.value.is_empty() {
            lines[k] = head_of(&lines[k], "").ok_or_else(|| anyhow!(UNSUPPORTED))?;
        }
        let end = block_end(lines, k, col);
        range = k + 1..end;
        col = child_col(lines, k + 1..end).unwrap_or(col + 2);
    }
    Ok(())
}

fn yaml_get(text: &str, path: &[&str]) -> Option<String> {
    let lines = YamlFile::read(text).lines;
    let (mut range, mut col, mut found) = (0..lines.len(), 0, None);
    for key in path {
        let k = find(&lines, range.clone(), col, key)?;
        found = entry(&lines[k]).map(|e| unquote(&e.value));
        let end = block_end(&lines, k, col);
        range = k + 1..end;
        col = child_col(&lines, k + 1..end).unwrap_or(col + 2);
    }
    found
}

// ---------- What a save may contain ----------

fn valid_name(name: &str) -> bool {
    (1..=64).contains(&name.chars().count()) && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') && name != "try"
}

/// A `host:port`, with the usual port added when it's left out.
fn clean_address(address: &str, default_port: u16) -> Result<String> {
    let a = address.trim();
    let bad = || anyhow!("{a} isn't an address. Use a form like 127.0.0.1:25566.");
    let (host, port) = match a.strip_prefix('[') {
        Some(v6) => {
            let (host, rest) = v6.split_once(']').ok_or_else(bad)?;
            host.parse::<std::net::Ipv6Addr>().map_err(|_| bad())?;
            (format!("[{host}]"), rest.strip_prefix(':').unwrap_or_default())
        }
        None => {
            let (host, port) = a.rsplit_once(':').unwrap_or((a, ""));
            if host.is_empty() || !host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_') {
                return Err(bad());
            }
            (host.to_string(), port)
        }
    };
    let port = if port.is_empty() && !a.ends_with(':') { default_port } else { port.parse::<u16>().ok().filter(|p| *p > 0).ok_or_else(bad)? };
    Ok(format!("{host}:{port}"))
}

/// Checks and tidies what the page sent. Servers players join come first, since the file can't say otherwise.
fn clean(edit: ProxyEdit, flavor: Flavor, own_port: u16) -> Result<ProxyEdit> {
    let default_port = if flavor == Flavor::Waterdog { 19132 } else { 25565 };
    let mut backends: Vec<Backend> = Vec::new();
    for b in edit.backends {
        let name = b.name.trim().to_string();
        if !valid_name(&name) {
            bail!("\"{name}\" can't be a server name. Use letters, numbers, - and _.");
        }
        if backends.iter().any(|o| o.name.eq_ignore_ascii_case(&name)) {
            bail!("Two servers are called {name}.");
        }
        let address = clean_address(&b.address, default_port)?;
        if port_of_local(&address) == Some(own_port) {
            bail!("{name} points at this proxy itself.");
        }
        backends.push(Backend { name, address, join: b.join });
    }
    if !backends.is_empty() && !backends.iter().any(|b| b.join) {
        bail!("Choose at least one server for players to join.");
    }
    if backends.is_empty() && flavor != Flavor::Velocity {
        bail!("This proxy needs at least one server.");
    }
    backends.sort_by_key(|b| !b.join);
    let mut forced_hosts: Vec<ForcedHost> = Vec::new();
    for h in edit.forced_hosts {
        let host = h.host.trim().trim_end_matches('.').to_lowercase();
        if host.is_empty() || !host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_') {
            bail!("\"{host}\" isn't an address players can connect with.");
        }
        if forced_hosts.iter().any(|o| o.host == host) {
            bail!("{host} is listed twice.");
        }
        if h.servers.is_empty() {
            bail!("Choose a server for {host}.");
        }
        if let Some(gone) = h.servers.iter().find(|s| !backends.iter().any(|b| b.name == **s)) {
            bail!("{host} goes to {gone}, which isn't in the list.");
        }
        let servers = if flavor == Flavor::Velocity { h.servers } else { h.servers.into_iter().take(1).collect() };
        forced_hosts.push(ForcedHost { host, servers });
    }
    let forwarding = edit.forwarding.to_lowercase();
    let known: &[&str] = match flavor {
        Flavor::Velocity => &["none", "legacy", "modern", "bungeeguard"],
        Flavor::Bungee => &["none", "legacy"],
        Flavor::Waterdog => &["none"],
    };
    if !known.contains(&forwarding.as_str()) {
        bail!("Unknown forwarding mode");
    }
    Ok(ProxyEdit { backends, forced_hosts, forwarding })
}

fn host_port(address: &str) -> Option<(&str, u16)> {
    let (host, port) = address.rsplit_once(':')?;
    Some((host, port.parse().ok()?))
}

fn port_of_local(address: &str) -> Option<u16> {
    host_port(address).filter(|(h, _)| matches!(h.to_lowercase().as_str(), "127.0.0.1" | "localhost" | "0.0.0.0" | "[::1]")).map(|(_, p)| p)
}

// ---------- Files ----------

fn paper_family(kind: ServerKind) -> bool {
    matches!(kind, ServerKind::Paper | ServerKind::Purpur | ServerKind::Folia | ServerKind::Leaf | ServerKind::Pufferfish)
}

/// Where a Paper server keeps its Velocity settings: per version, unless one of the files already exists.
fn paper_settings(dir: &Path, mc: &str) -> (std::path::PathBuf, [&'static str; 2]) {
    let (new, old) = (dir.join("config").join("paper-global.yml"), dir.join("paper.yml"));
    if new.exists() || (!old.exists() && mc_at_least(mc, 19)) {
        (new, ["proxies", "velocity"])
    } else {
        (old, ["settings", "velocity-support"])
    }
}

#[derive(Default)]
struct PaperState {
    velocity: bool,
    secret: String,
    bungeecord: bool,
}

async fn paper_state(dir: &Path, mc: &str) -> PaperState {
    let (file, [a, b]) = paper_settings(dir, mc);
    let text = tokio::fs::read_to_string(file).await.unwrap_or_default();
    let spigot = tokio::fs::read_to_string(dir.join("spigot.yml")).await.unwrap_or_default();
    PaperState {
        velocity: yaml_get(&text, &[a, b, "enabled"]).as_deref() == Some("true"),
        secret: yaml_get(&text, &[a, b, "secret"]).unwrap_or_default(),
        bungeecord: yaml_get(&spigot, &["settings", "bungeecord"]).as_deref() == Some("true"),
    }
}

fn secret_path(dir: &Path, text: &str) -> Result<std::path::PathBuf> {
    let doc = scan_toml(text);
    let name = doc.items.iter().find(|i| i.section.is_empty() && i.key == "forwarding-secret-file").and_then(|i| toml_strings(&text[i.value.clone()]).into_iter().next());
    safe_join(dir, name.as_deref().filter(|n| !n.is_empty()).unwrap_or(DEFAULT_SECRET_FILE))
}

async fn read_secret(dir: &Path, text: &str) -> Option<String> {
    if let Ok(path) = secret_path(dir, text) {
        if let Ok(s) = tokio::fs::read_to_string(path).await {
            if !s.trim().is_empty() {
                return Some(s.trim().to_string());
            }
        }
    }
    // Configs from before the secret had its own file.
    let doc = scan_toml(text);
    doc.items.iter().find(|i| i.section.is_empty() && i.key == "forwarding-secret").and_then(|i| toml_strings(&text[i.value.clone()]).into_iter().next()).filter(|s| !s.is_empty())
}

/// The proxy's secret, making one if it has none yet (Velocity would, on start).
async fn ensure_secret(dir: &Path, text: &str) -> Result<String> {
    if let Some(s) = read_secret(dir, text).await {
        return Ok(s);
    }
    let secret = uuid::Uuid::new_v4().simple().to_string();
    let path = secret_path(dir, text)?;
    write_atomic(&path, secret.as_bytes()).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).await?;
    }
    Ok(secret)
}

async fn proxy_text(meta: &ServerMeta, flavor: Flavor) -> Result<(String, bool)> {
    match tokio::fs::read_to_string(store::dir(&meta.id)?.join(flavor.file())).await {
        Ok(t) if !t.trim().is_empty() => Ok((t, false)),
        _ => Ok((flavor.starter(meta.port).unwrap_or_default(), true)),
    }
}

fn needs_secret(forwarding: &str) -> bool {
    matches!(forwarding, "modern" | "bungeeguard")
}

/// Saved changes count from the next start: the start time each save saw. The launcher's servers end with it, so
/// nothing here needs to outlive it.
static PENDING: LazyLock<Mutex<HashMap<String, Option<i64>>>> = LazyLock::new(Default::default);

fn mark_pending(meta: &ServerMeta) {
    PENDING.lock().unwrap().insert(meta.id.clone(), meta.last_started);
}

fn is_pending(meta: &ServerMeta) -> bool {
    PENDING.lock().unwrap().get(&meta.id) == Some(&meta.last_started)
}

// ---------- The page ----------

async fn setup_of(b: &Backend, proxy: &ServerMeta, flavor: Flavor, forwarding: &str, secret: Option<&str>, servers: &[ServerMeta]) -> BackendSetup {
    let local = port_of_local(&b.address).and_then(|port| servers.iter().find(|m| m.id != proxy.id && !m.kind.proxy() && m.port == port && m.kind.bedrock() == (flavor == Flavor::Waterdog)));
    let mut out = BackendSetup { name: b.name.clone(), local_id: None, local_name: None, advice: "remote", can_apply: false, too_old: false, online_mode_off: None, forwarding_ok: None, exposed: false, pending: false };
    let Some(meta) = local else {
        if flavor == Flavor::Waterdog {
            out.advice = "bedrock";
        }
        return out;
    };
    out.local_id = Some(meta.id.clone());
    out.local_name = Some(meta.name.clone());
    out.exposed = meta.public;
    out.pending = is_pending(meta);
    out.advice = match meta.kind {
        k if k.bedrock() => "bedrock",
        k if paper_family(k) => "paper",
        k if k.bukkit() => "spigot",
        k if k.modded() => "modded",
        _ => "other",
    };
    let Ok(dir) = store::dir(&meta.id) else { return out };
    if meta.kind.uses_properties() && !meta.kind.bedrock() {
        out.online_mode_off = Some(properties::get_value(&dir, "online-mode").await.is_some_and(|v| v.trim() == "false"));
    }
    if paper_family(meta.kind) {
        out.too_old = forwarding == "modern" && !mc_at_least(&meta.mc_version, 13);
        out.can_apply = !out.too_old && forwarding != "bungeeguard";
        let p = paper_state(&dir, &meta.mc_version).await;
        out.forwarding_ok = Some(match forwarding {
            "modern" => p.velocity && !p.bungeecord && secret.is_some_and(|s| s == p.secret),
            "legacy" => p.bungeecord && !p.velocity,
            _ => false,
        });
    }
    out
}

async fn view(meta: &ServerMeta, flavor: Flavor) -> Result<ProxyView> {
    let dir = store::dir(&meta.id)?;
    let (text, unwritten) = proxy_text(meta, flavor).await?;
    let edit = flavor.parse(&text)?;
    let servers = store::list().await?;
    let choices = servers
        .iter()
        .filter(|m| m.id != meta.id && !m.kind.proxy() && m.kind.bedrock() == (flavor == Flavor::Waterdog))
        .map(|m| LocalChoice { id: m.id.clone(), name: m.name.clone(), kind: m.kind, address: format!("127.0.0.1:{}", m.port) })
        .collect();
    let secret = if flavor == Flavor::Velocity { read_secret(&dir, &text).await } else { None };
    let mut setup = Vec::new();
    for b in &edit.backends {
        setup.push(setup_of(b, meta, flavor, &edit.forwarding, secret.as_deref(), &servers).await);
    }
    let online_mode = match flavor {
        Flavor::Velocity => {
            let doc = scan_toml(&text);
            doc.items.iter().find(|i| i.section.is_empty() && i.key == "online-mode").is_none_or(|i| text[i.value.clone()].trim() != "false")
        }
        _ => yaml_get(&text, &["online_mode"]).as_deref() != Some("false"),
    };
    Ok(ProxyView { file: flavor.file(), missing: unwritten && flavor == Flavor::Velocity, edit, has_secret: secret.is_some(), online_mode, pending: is_pending(meta), choices, setup })
}

async fn save(meta: &ServerMeta, flavor: Flavor, edit: ProxyEdit) -> Result<()> {
    let edit = clean(edit, flavor, meta.port)?;
    let (text, unwritten) = proxy_text(meta, flavor).await?;
    if unwritten && flavor == Flavor::Velocity {
        bail!("Start this proxy once first, so it writes {}.", flavor.file());
    }
    let out = flavor.write(&text, &edit)?;
    let dir = store::dir(&meta.id)?;
    if flavor == Flavor::Velocity && needs_secret(&edit.forwarding) {
        ensure_secret(&dir, &out).await?;
    }
    if out != text {
        write_atomic(&dir.join(flavor.file()), out.as_bytes()).await?;
        mark_pending(meta);
    }
    Ok(())
}

/// The forwarding mode to set up a backend of this Minecraft version with, given the proxy's.
fn mode_for(flavor: Flavor, current: &str, mc: &str) -> Result<&'static str> {
    let modern_ok = mc_at_least(mc, 13);
    Ok(match (flavor, current) {
        (Flavor::Waterdog, _) => bail!("WaterdogPE has no forwarding to set up."),
        (Flavor::Bungee, _) => "legacy",
        (Flavor::Velocity, "bungeeguard") => bail!("BungeeGuard is set up with its own plugin."),
        (Flavor::Velocity, "legacy") => "legacy",
        (Flavor::Velocity, "modern") if !modern_ok => bail!("Minecraft 1.12 and older can't use modern forwarding. Choose legacy forwarding for this proxy."),
        (Flavor::Velocity, "modern") => "modern",
        (Flavor::Velocity, _) if modern_ok => "modern",
        (Flavor::Velocity, _) => "legacy",
    })
}

/// A backend's Paper and Spigot config after setup. `secret` set: modern forwarding.
fn paper_config(paper: &str, path: [&str; 2], spigot: &str, secret: Option<&str>) -> Result<(String, String)> {
    let mut p = YamlFile::read(paper);
    let [a, b] = path;
    yaml_set(&mut p.lines, &[a, b, "enabled"], if secret.is_some() { "true" } else { "false" })?;
    if let Some(s) = secret {
        yaml_set(&mut p.lines, &[a, b, "online-mode"], "true")?;
        yaml_set(&mut p.lines, &[a, b, "secret"], &format!("'{}'", s.replace('\'', "''")))?;
    }
    let mut g = YamlFile::read(spigot);
    yaml_set(&mut g.lines, &["settings", "bungeecord"], if secret.is_some() { "false" } else { "true" })?;
    Ok((p.text(), g.text()))
}

async fn apply(meta: &ServerMeta, flavor: Flavor, backend_id: &str) -> Result<()> {
    let target = store::get(backend_id).await?;
    if !paper_family(target.kind) || target.id == meta.id {
        bail!("ResinCraft can only set up Paper-family servers. Follow the steps shown for this one.");
    }
    let (text, unwritten) = proxy_text(meta, flavor).await?;
    if unwritten && flavor == Flavor::Velocity {
        bail!("Start this proxy once first, so it writes {}.", flavor.file());
    }
    let mut edit = flavor.parse(&text)?;
    let mode = mode_for(flavor, &edit.forwarding, &target.mc_version)?;
    edit.forwarding = mode.to_string();
    let out = flavor.write(&text, &edit)?;

    let dir = store::dir(&meta.id)?;
    let secret = if mode == "modern" { Some(ensure_secret(&dir, &out).await?) } else { None };
    let backend = store::dir(backend_id)?;
    let (paper_file, path) = paper_settings(&backend, &target.mc_version);
    let spigot_file = backend.join("spigot.yml");
    let paper = tokio::fs::read_to_string(&paper_file).await.unwrap_or_default();
    let spigot = tokio::fs::read_to_string(&spigot_file).await.unwrap_or_default();
    let (paper, spigot) = paper_config(&paper, path, &spigot, secret.as_deref())?;

    // Everything is worked out: only now is anything written.
    if out != text {
        write_atomic(&dir.join(flavor.file()), out.as_bytes()).await?;
        mark_pending(meta);
    }
    write_atomic(&paper_file, paper.as_bytes()).await?;
    write_atomic(&spigot_file, spigot.as_bytes()).await?;
    properties::set_values(&backend, &[("online-mode", "false".to_string())]).await?;
    if target.public {
        store::update(backend_id, json!({ "public": false })).await?;
    }
    mark_pending(&target);
    Ok(())
}

pub async fn handle(channel: &str, args: Args) -> Result<Value> {
    let id: String = args.get(0)?;
    let meta = store::get(&id).await?;
    let flavor = Flavor::of(meta.kind).ok_or_else(|| anyhow!("This server isn't a proxy with servers behind it"))?;
    match channel {
        "server:proxyConfig" => {}
        "server:proxySave" => save(&meta, flavor, args.get(1)?).await?,
        "server:proxyApply" => apply(&meta, flavor, &args.get::<String>(1)?).await?,
        "server:proxySecret" => {
            if flavor != Flavor::Velocity {
                bail!("This proxy has no forwarding secret");
            }
            let (text, _) = proxy_text(&meta, flavor).await?;
            let secret = read_secret(&store::dir(&id)?, &text).await.ok_or_else(|| anyhow!("This proxy has no forwarding secret yet"))?;
            return Ok(Value::String(secret));
        }
        _ => bail!("Unknown request: {channel}"),
    }
    let meta = store::get(&id).await?;
    json(view(&meta, flavor).await?)
}

/// The web console's file manager: refuses a file that holds a forwarding secret. Works on the contents, so a copy
/// under another name is caught too.
pub async fn guard_remote_read(path: &str, file: &Value) -> Result<()> {
    let refuse = || anyhow!("This file holds a proxy's forwarding secret, so it can only be opened on this PC.");
    if path.rsplit(['/', '\\']).next().is_some_and(|n| n.eq_ignore_ascii_case(DEFAULT_SECRET_FILE)) {
        return Err(refuse());
    }
    let Some(text) = file["text"].as_str() else { return Ok(()) };
    for meta in store::list().await? {
        let Ok(dir) = store::dir(&meta.id) else { continue };
        let secret = if meta.kind == ServerKind::Velocity {
            read_secret(&dir, &tokio::fs::read_to_string(dir.join("velocity.toml")).await.unwrap_or_default()).await
        } else if paper_family(meta.kind) {
            Some(paper_state(&dir, &meta.mc_version).await.secret)
        } else {
            None
        };
        if secret.is_some_and(|s| !s.is_empty() && text.contains(&s)) {
            return Err(refuse());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VELOCITY: &str = r#"# Config version. Do not change this
config-version = "2.7"

# What port should the proxy be bound to?
bind = "0.0.0.0:25565"
motd = "<#09add3>A # Velocity Server" # keep this comment

# Should we forward IP addresses?
player-info-forwarding-mode = "NONE"
forwarding-secret-file = "forwarding.secret"

[servers]
# Configure your servers here. Each key represents the server's name, and the value
# represents the IP address of the server to connect to.
lobby = "127.0.0.1:30066" # the hub
factions = "127.0.0.1:30067"
minigames = "127.0.0.1:30068"

# In what order we should try servers when a player logs in or is kicked from a server.
try = [
    "lobby"
]

[forced-hosts]
# Configure your forced hosts here.
"lobby.example.com" = [
    "lobby"
]
"factions.example.com" = [
    "factions"
]

[advanced]
# How large a Minecraft packet has to be before we compress it.
compression-threshold = 256
"#;

    const BUNGEE: &str = "# My network\nserver_connect_timeout: 5000\nlisteners:\n- query_port: 25577\n  motd: '&1Another # Bungee server'\n  priorities:\n  - lobby\n  - pvp # second\n  forced_hosts:\n    pvp.md-5.net: pvp\n  host: 0.0.0.0:25577\n  max_players: 1\nip_forward: false\nonline_mode: true\n# The servers\nservers:\n  # The hub\n  lobby:\n    motd: '&1Just another BungeeCord - Forced Host'\n    address: localhost:25565\n    restricted: false\n  pvp:\n    motd: 'x'\n    address: 127.0.0.1:25566 # fights\n    restricted: true\ngroups:\n  md_5:\n  - admin\n";

    const WATERDOG: &str = "listener:\n  motd: Hi\n  priorities:\n  - lobby1\n  host: 0.0.0.0:19132\n  forced_hosts: {}\nservers:\n  lobby1:\n    address: 127.0.0.1:19133\n    public_address: play.example.com:19133\n    server_type: bedrock\nuse_fast_transfer: true\n";

    fn b(name: &str, address: &str, join: bool) -> Backend {
        Backend { name: name.into(), address: address.into(), join }
    }

    fn fh(host: &str, server: &str) -> ForcedHost {
        ForcedHost { host: host.into(), servers: vec![server.into()] }
    }

    #[test]
    fn reads_velocity() {
        let e = parse_velocity(VELOCITY);
        assert_eq!(e.backends, vec![b("lobby", "127.0.0.1:30066", true), b("factions", "127.0.0.1:30067", false), b("minigames", "127.0.0.1:30068", false)]);
        assert_eq!(e.forced_hosts, vec![fh("lobby.example.com", "lobby"), fh("factions.example.com", "factions")]);
        assert_eq!(e.forwarding, "none");
    }

    #[test]
    fn saving_velocity_unchanged_changes_nothing() {
        assert_eq!(Flavor::Velocity.write(VELOCITY, &parse_velocity(VELOCITY)).unwrap(), VELOCITY);
    }

    #[test]
    fn edits_velocity_keeping_the_rest() {
        let edit = ProxyEdit {
            backends: vec![b("survival", "10.0.0.5:25565", true), b("lobby", "127.0.0.1:40000", true), b("factions", "127.0.0.1:30067", false)],
            forced_hosts: vec![fh("lobby.example.com", "lobby"), fh("play.example.com", "survival")],
            forwarding: "modern".into(),
        };
        let out = Flavor::Velocity.write(VELOCITY, &edit).unwrap();
        assert_eq!(parse_velocity(&out), edit);
        for kept in ["# Config version. Do not change this", "motd = \"<#09add3>A # Velocity Server\" # keep this comment", "# Configure your servers here.", "# In what order we should try", "# Configure your forced hosts here.", "compression-threshold = 256"] {
            assert!(out.contains(kept), "lost {kept}: {out}");
        }
        assert!(out.contains("player-info-forwarding-mode = \"MODERN\""));
        assert!(out.contains("lobby = \"127.0.0.1:40000\" # the hub"), "a kept server keeps its comment: {out}");
        assert!(!out.contains("minigames"));
        assert!(!out.contains("factions.example.com"));
        assert!(out.find("[servers]").unwrap() < out.find("survival = ").unwrap() && out.find("survival = ").unwrap() < out.find("[forced-hosts]").unwrap());
    }

    #[test]
    fn velocity_tables_can_be_empty_or_new() {
        let empty = ProxyEdit { backends: vec![], forced_hosts: vec![], forwarding: "none".into() };
        let out = Flavor::Velocity.write(VELOCITY, &empty).unwrap();
        assert_eq!(parse_velocity(&out), empty);
        assert!(out.contains("[advanced]") && out.contains("# Configure your forced hosts here."));
        let back = ProxyEdit { backends: vec![b("a", "127.0.0.1:1", true)], forced_hosts: vec![fh("a.example.com", "a")], forwarding: "none".into() };
        assert_eq!(parse_velocity(&Flavor::Velocity.write(&out, &back).unwrap()), back);
        let bare = "bind = \"0.0.0.0:25565\"";
        let out = Flavor::Velocity.write(bare, &back).unwrap();
        assert_eq!(parse_velocity(&out), back);
        assert!(out.starts_with("bind = "));
    }

    #[test]
    fn velocity_keeps_windows_line_endings_and_odd_arrays() {
        let crlf = VELOCITY.replace('\n', "\r\n");
        let mut edit = parse_velocity(&crlf);
        edit.backends.push(b("new", "127.0.0.1:5", false));
        let out = Flavor::Velocity.write(&crlf, &edit).unwrap();
        assert!(!out.replace("\r\n", "").contains('\n'), "only CRLF: {out:?}");
        let text = "[servers]\na = \"127.0.0.1:1\"\ntry = [ 'a', # first\n  ]\n";
        assert_eq!(parse_velocity(text).backends, vec![b("a", "127.0.0.1:1", true)]);
    }

    #[test]
    fn a_new_try_order_moves_the_servers() {
        let text = "[servers]\na = \"h:1\"\nb = \"h:2\"\ntry = [\n  \"b\",\n  \"a\"\n]\n";
        let e = parse_velocity(text);
        assert_eq!(e.backends, vec![b("b", "h:2", true), b("a", "h:1", true)]);
        let out = Flavor::Velocity.write(text, &e).unwrap();
        assert_eq!(out, "[servers]\nb = \"h:2\"\na = \"h:1\"\ntry = [\n  \"b\",\n  \"a\"\n]\n");
    }

    #[test]
    fn reads_bungee() {
        let e = parse_bungee(BUNGEE, false).unwrap();
        assert_eq!(e.backends, vec![b("lobby", "localhost:25565", true), b("pvp", "127.0.0.1:25566", true)]);
        assert_eq!(e.forced_hosts, vec![fh("pvp.md-5.net", "pvp")]);
        assert_eq!(e.forwarding, "none");
    }

    #[test]
    fn saving_bungee_unchanged_changes_nothing() {
        let e = parse_bungee(BUNGEE, false).unwrap();
        assert_eq!(Flavor::Bungee.write(BUNGEE, &e).unwrap(), BUNGEE);
        let crlf = BUNGEE.replace('\n', "\r\n");
        assert_eq!(Flavor::Bungee.write(&crlf, &e).unwrap(), crlf);
    }

    #[test]
    fn edits_bungee_keeping_the_rest() {
        let edit = ProxyEdit { backends: vec![b("pvp", "127.0.0.1:30000", true), b("skyblock", "10.0.0.5:25565", true), b("lobby", "localhost:25565", false)], forced_hosts: vec![fh("sky.example.com", "skyblock")], forwarding: "legacy".into() };
        let out = Flavor::Bungee.write(BUNGEE, &edit).unwrap();
        assert_eq!(parse_bungee(&out, false).unwrap(), edit);
        for kept in ["# My network", "server_connect_timeout: 5000", "motd: '&1Another # Bungee server'", "# The servers", "# The hub", "restricted: true", "address: 127.0.0.1:30000 # fights", "groups:\n  md_5:\n  - admin", "online_mode: true"] {
            assert!(out.contains(kept), "lost {kept}: {out}");
        }
        assert!(out.contains("ip_forward: true") && !out.contains("pvp.md-5.net"));
        assert!(out.contains("  skyblock:\n    address: 10.0.0.5:25565\n    restricted: false"), "{out}");
        assert!(out.find("  pvp:").unwrap() < out.find("  skyblock:").unwrap(), "{out}");
    }

    #[test]
    fn fresh_bungee_files_get_what_they_need() {
        let edit = ProxyEdit { backends: vec![b("a", "127.0.0.1:1", true)], forced_hosts: vec![fh("x.example.com", "a")], forwarding: "legacy".into() };
        let out = Flavor::Bungee.write(&Flavor::Bungee.starter(25577).unwrap(), &edit).unwrap();
        assert_eq!(parse_bungee(&out, false).unwrap(), edit);
        assert!(out.starts_with("listeners:\n- host: 0.0.0.0:25577\n  priorities:\n  - a\n"), "{out}");
        let flow = "listeners:\n- priorities: []\n  forced_hosts: {}\n  host: 0.0.0.0:25577\nservers: {}\nip_forward: false\n";
        let out = Flavor::Bungee.write(flow, &edit).unwrap();
        assert_eq!(parse_bungee(&out, false).unwrap(), edit);
    }

    #[test]
    fn flow_style_servers_are_refused_not_broken() {
        assert!(parse_bungee("servers: {lobby: {address: 'h:1'}}\n", false).is_err());
        assert!(parse_bungee("servers:\n  lobby: {address: 'h:1'}\n", false).is_err());
    }

    #[test]
    fn edits_waterdog() {
        let e = parse_bungee(WATERDOG, true).unwrap();
        assert_eq!(e.backends, vec![b("lobby1", "127.0.0.1:19133", true)]);
        assert_eq!(Flavor::Waterdog.write(WATERDOG, &e).unwrap(), WATERDOG);
        let edit = ProxyEdit { backends: vec![b("lobby1", "127.0.0.1:19140", true), b("games", "127.0.0.1:19150", true)], forced_hosts: vec![fh("g.example.com", "games")], forwarding: "none".into() };
        let out = Flavor::Waterdog.write(WATERDOG, &edit).unwrap();
        assert_eq!(parse_bungee(&out, true).unwrap(), edit);
        assert!(out.contains("public_address: play.example.com:19133") && out.contains("use_fast_transfer: true") && !out.contains("ip_forward"));
    }

    #[test]
    fn cleans_what_the_page_sends() {
        let raw = |backends, forced_hosts| ProxyEdit { backends, forced_hosts, forwarding: "MODERN".into() };
        let e = clean(raw(vec![b("off", "10.0.0.5", false), b("lobby", " play.example.com:25566 ", true)], vec![fh(" Play.Example.COM. ", "off")]), Flavor::Velocity, 25577).unwrap();
        assert_eq!(e.backends, vec![b("lobby", "play.example.com:25566", true), b("off", "10.0.0.5:25565", false)]);
        assert_eq!((e.forced_hosts[0].host.as_str(), e.forwarding.as_str()), ("play.example.com", "modern"));
        assert_eq!(clean_address("[::1]:25566", 25565).unwrap(), "[::1]:25566");
        assert_eq!(clean_address("h", 19132).unwrap(), "h:19132");
        for bad in ["", "a b:1", "h:0", "h:99999", "h:", "\"x\":1", "[oops]:1"] {
            assert!(clean_address(bad, 25565).is_err(), "{bad}");
        }
        let no = |backends, hosts, flavor| clean(raw(backends, hosts), flavor, 25577).is_err();
        assert!(no(vec![b("a b", "h:1", true)], vec![], Flavor::Velocity));
        assert!(no(vec![b("try", "h:1", true)], vec![], Flavor::Velocity), "Velocity keeps its try list there");
        assert!(no(vec![b("a", "h:1", true), b("A", "h:2", true)], vec![], Flavor::Velocity));
        assert!(no(vec![b("a", "h:1", false)], vec![], Flavor::Velocity), "someone must get in");
        assert!(no(vec![b("a", "127.0.0.1:25577", true)], vec![], Flavor::Velocity), "not itself");
        assert!(no(vec![b("a", "h:1", true)], vec![fh("x.example.com", "gone")], Flavor::Velocity));
        assert!(no(vec![], vec![], Flavor::Bungee));
        assert!(no(vec![b("a", "h:1", true)], vec![], Flavor::Bungee), "BungeeCord has no modern forwarding");
    }

    #[test]
    fn sets_up_paper_for_modern_forwarding() {
        let paper = "# This is the global configuration file for Paper.\n_version: 29\nproxies:\n  bungee-cord:\n    online-mode: true\n  proxy-protocol: false\n  velocity:\n    enabled: false # off\n    online-mode: true\n    secret: ''\nscoreboards:\n  save-empty: true\n";
        let spigot = "# Spigot\nsettings:\n  debug: false\n  bungeecord: true\nconfig-version: 12\n";
        let (p, s) = paper_config(paper, ["proxies", "velocity"], spigot, Some("it's-secret")).unwrap();
        assert!(p.contains("    enabled: true # off\n    online-mode: true\n    secret: 'it''s-secret'\nscoreboards:"), "{p}");
        assert!(p.contains("# This is the global configuration file") && p.contains("bungee-cord:\n    online-mode: true"));
        assert_eq!(yaml_get(&p, &["proxies", "velocity", "secret"]).unwrap(), "it's-secret");
        assert_eq!(s, "# Spigot\nsettings:\n  debug: false\n  bungeecord: false\nconfig-version: 12\n");
    }

    #[test]
    fn sets_up_a_paper_that_never_started() {
        let (p, s) = paper_config("", ["proxies", "velocity"], "", Some("abc")).unwrap();
        assert_eq!(p, "proxies:\n  velocity:\n    enabled: true\n    online-mode: true\n    secret: 'abc'\n");
        assert_eq!(s, "settings:\n  bungeecord: false\n");
        let (p, s) = paper_config("", ["settings", "velocity-support"], "", None).unwrap();
        assert_eq!(p, "settings:\n  velocity-support:\n    enabled: false\n");
        assert_eq!(s, "settings:\n  bungeecord: true\n");
    }

    #[test]
    fn modes_follow_the_proxy_and_the_version() {
        assert_eq!(mode_for(Flavor::Bungee, "none", "1.8.8").unwrap(), "legacy");
        assert_eq!(mode_for(Flavor::Velocity, "none", "1.21.4").unwrap(), "modern");
        assert_eq!(mode_for(Flavor::Velocity, "none", "1.12.2").unwrap(), "legacy");
        assert_eq!(mode_for(Flavor::Velocity, "legacy", "1.21.4").unwrap(), "legacy");
        assert!(mode_for(Flavor::Velocity, "modern", "1.12.2").is_err());
        assert!(mode_for(Flavor::Velocity, "bungeeguard", "1.21.4").is_err());
        assert!(mc_at_least("26.1", 19) && mc_at_least("1.19", 19) && !mc_at_least("1.18.2", 19));
    }

    #[test]
    fn only_local_addresses_count_as_this_pc() {
        assert_eq!(port_of_local("127.0.0.1:25566"), Some(25566));
        assert_eq!(port_of_local("LOCALHOST:1"), Some(1));
        assert_eq!(port_of_local("[::1]:7"), Some(7));
        assert_eq!(port_of_local("10.0.0.5:25566"), None);
    }

    #[test]
    fn old_inline_secrets_are_found_and_custom_files_followed() {
        let dir = std::env::temp_dir().join(format!("rc-proxy-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
            assert_eq!(read_secret(&dir, "forwarding-secret = \"old\"\n").await.as_deref(), Some("old"));
            let made = ensure_secret(&dir, "forwarding-secret-file = \"my.secret\"\n").await.unwrap();
            assert_eq!(std::fs::read_to_string(dir.join("my.secret")).unwrap(), made);
            assert_eq!(ensure_secret(&dir, "forwarding-secret-file = \"my.secret\"\n").await.unwrap(), made, "kept");
            assert!(read_secret(&dir, "forwarding-secret-file = \"../x\"\n").await.is_none());
        });
        let _ = std::fs::remove_dir_all(dir);
    }
}
