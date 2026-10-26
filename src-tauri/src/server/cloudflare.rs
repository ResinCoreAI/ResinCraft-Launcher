//! Optional: keeping a hosted server's domain up to date through the Cloudflare API, for domains whose DNS is on
//! Cloudflare. The player makes an API token with the "Zone → DNS → Edit" permission; it's kept sealed (like
//! account tokens) in `<dataDir>/servers/dns-tokens.json`, apart from server.json, and never logged or sent
//! back to the UI. Records are created or updated, never deleted, and always "DNS only" (Cloudflare's proxy
//! can't carry Minecraft).

use std::collections::HashMap;

use anyhow::Result;
use serde_json::{json, Value};

use crate::core::ctx::http;
use crate::core::fsutil::{read_json, write_json};
use crate::core::types::DnsRecord;
use crate::err_tr;
use crate::player::accounts::{seal, unseal};

use super::store;

const API: &str = "https://api.cloudflare.com/client/v4";
/// Cloudflare's "automatic" TTL.
const TTL_AUTO: u32 = 1;

fn tokens_file() -> std::path::PathBuf {
    store::root().join("dns-tokens.json")
}

async fn tokens() -> HashMap<String, String> {
    read_json(&tokens_file()).await.unwrap_or_default()
}

/// Saves (or with None/empty, removes) the server's API token.
pub async fn set_token(id: &str, token: Option<String>) -> Result<()> {
    let mut all = tokens().await;
    match token.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) {
        Some(t) => all.insert(id.to_string(), seal(&t)),
        None => all.remove(id),
    };
    write_json(&tokens_file(), &all).await
}

pub async fn has_token(id: &str) -> bool {
    tokens().await.contains_key(id)
}

pub async fn token(id: &str) -> Option<String> {
    let sealed = tokens().await.remove(id)?;
    unseal(Some(&sealed)).ok().filter(|t| !t.is_empty())
}

/// One API call; Cloudflare's own error message when it refuses. The token only goes into the header.
async fn call(token: &str, method: reqwest::Method, path: &str, body: Option<Value>) -> Result<Value> {
    let mut req = http().request(method, format!("{API}{path}")).bearer_auth(token);
    if let Some(b) = body {
        req = req.json(&b);
    }
    let res = req.send().await.map_err(|_| err_tr!("hostDomain.cfUnreachable", "Couldn't reach Cloudflare"))?;
    let status = res.status();
    let v: Value = res.json().await.unwrap_or(Value::Null);
    if v.get("success").and_then(Value::as_bool) == Some(true) {
        return Ok(v.get("result").cloned().unwrap_or(Value::Null));
    }
    let error = v
        .pointer("/errors/0/message")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
    Err(err_tr!("hostDomain.cfRejected", "Cloudflare refused: {error}", error = error))
}

/// The zone the domain belongs to: the longest of its parent names that this token can see.
async fn zone_id(token: &str, domain: &str) -> Result<String> {
    let labels: Vec<&str> = domain.split('.').collect();
    for i in 0..labels.len().saturating_sub(1) {
        let name = labels[i..].join(".");
        let found = call(token, reqwest::Method::GET, &format!("/zones?name={}", urlencoding::encode(&name)), None).await?;
        if let Some(id) = found.pointer("/0/id").and_then(Value::as_str) {
            return Ok(id.to_string());
        }
    }
    Err(err_tr!("hostDomain.cfNoZone", "This Cloudflare token can't see a zone for {domain}", domain = domain))
}

/// The record as Cloudflare's API wants it.
pub fn record_body(r: &DnsRecord) -> Value {
    match r.kind {
        "SRV" => json!({
            "type": "SRV",
            "name": r.name,
            "ttl": TTL_AUTO,
            "data": { "priority": r.priority.unwrap_or(0), "weight": r.weight.unwrap_or(5), "port": r.port.unwrap_or(25565), "target": r.value },
        }),
        "TXT" => json!({ "type": "TXT", "name": r.name, "content": r.value, "ttl": TTL_AUTO }),
        kind => json!({ "type": kind, "name": r.name, "content": r.value, "ttl": TTL_AUTO, "proxied": false }),
    }
}

/// True when Cloudflare's record already says what `r` says.
pub fn matches(existing: &Value, r: &DnsRecord) -> bool {
    match r.kind {
        "SRV" => {
            let d = existing.get("data").cloned().unwrap_or(Value::Null);
            d.get("port").and_then(Value::as_u64) == r.port.map(u64::from)
                && d.get("priority").and_then(Value::as_u64) == r.priority.map(u64::from)
                && d.get("weight").and_then(Value::as_u64) == r.weight.map(u64::from)
                && d.get("target").and_then(Value::as_str).is_some_and(|t| t.trim_end_matches('.').eq_ignore_ascii_case(&r.value))
        }
        // Cloudflare may hand TXT content back in quotes.
        "TXT" => existing.get("content").and_then(Value::as_str).map(|c| c.trim_matches('"')) == Some(r.value.as_str()),
        _ => existing.get("content").and_then(Value::as_str) == Some(r.value.as_str()) && existing.get("proxied").and_then(Value::as_bool) != Some(true),
    }
}

/// Record types that can't share a name with a CNAME (and the other way round).
fn clashing(kind: &str) -> &'static [&'static str] {
    match kind {
        "CNAME" => &["A", "AAAA"],
        "A" | "AAAA" => &["CNAME"],
        _ => &[],
    }
}

/// Creates or updates `records` for `domain`. `srv_guard` is an SRV record that an existing `_minecraft._tcp`
/// record is changed to (direct route on the default port, ResinCraft Relay), so a stale one can't send players
/// elsewhere. Records of a clashing type at the same name (an A record from the direct route where ResinCraft
/// Relay needs a CNAME, or the reverse) are removed. Returns how many records changed.
pub async fn sync(token: &str, domain: &str, records: &[DnsRecord], srv_guard: Option<DnsRecord>) -> Result<usize> {
    let zone = zone_id(token, domain).await?;
    let mut wanted: Vec<(DnsRecord, bool)> = records.iter().cloned().map(|r| (r, false)).collect();
    if let Some(srv) = srv_guard.filter(|g| !records.contains(g)) {
        wanted.push((srv, true));
    }
    let mut changed = 0;
    for (r, _) in wanted.iter().filter(|(_, only_if_present)| !only_if_present) {
        for kind in clashing(r.kind) {
            let path = format!("/zones/{zone}/dns_records?type={kind}&name={}", urlencoding::encode(&r.name));
            for old in call(token, reqwest::Method::GET, &path, None).await?.as_array().cloned().unwrap_or_default() {
                let rid = old.get("id").and_then(Value::as_str).unwrap_or_default();
                call(token, reqwest::Method::DELETE, &format!("/zones/{zone}/dns_records/{rid}"), None).await?;
                changed += 1;
            }
        }
    }
    for (r, only_if_present) in wanted {
        let path = format!("/zones/{zone}/dns_records?type={}&name={}", r.kind, urlencoding::encode(&r.name));
        let existing = call(token, reqwest::Method::GET, &path, None).await?;
        let first = existing.get(0).cloned();
        match first {
            Some(e) if matches(&e, &r) => {}
            Some(e) => {
                let rid = e.get("id").and_then(Value::as_str).unwrap_or_default();
                call(token, reqwest::Method::PUT, &format!("/zones/{zone}/dns_records/{rid}"), Some(record_body(&r))).await?;
                changed += 1;
            }
            None if only_if_present => {}
            None => {
                call(token, reqwest::Method::POST, &format!("/zones/{zone}/dns_records"), Some(record_body(&r))).await?;
                changed += 1;
            }
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_and_compares_records() {
        let a = DnsRecord { kind: "A", name: "play.example.com".into(), value: "203.0.113.7".into(), port: None, priority: None, weight: None };
        let body = record_body(&a);
        assert_eq!(body["content"], "203.0.113.7");
        assert_eq!(body["proxied"], false);
        assert!(matches(&json!({ "content": "203.0.113.7", "proxied": false }), &a));
        assert!(!matches(&json!({ "content": "203.0.113.7", "proxied": true }), &a));
        assert!(!matches(&json!({ "content": "198.51.100.1" }), &a));

        let cname = DnsRecord { kind: "CNAME", name: "play.example.com".into(), value: "smp.rscraft.resincore.tech".into(), port: None, priority: None, weight: None };
        let body = record_body(&cname);
        assert_eq!((body["type"].as_str(), body["content"].as_str(), body["proxied"].as_bool()), (Some("CNAME"), Some("smp.rscraft.resincore.tech"), Some(false)));
        assert!(matches(&json!({ "content": "smp.rscraft.resincore.tech", "proxied": false }), &cname));
        assert!(!matches(&json!({ "content": "smp.rscraft.resincore.tech", "proxied": true }), &cname));
        assert_eq!(clashing("CNAME"), ["A", "AAAA"]);
        assert_eq!(clashing("A"), ["CNAME"]);
        assert!(clashing("SRV").is_empty());

        let srv = DnsRecord { kind: "SRV", name: "_minecraft._tcp.play.example.com".into(), value: "abc.ply.gg".into(), port: Some(12345), priority: Some(0), weight: Some(5) };
        let body = record_body(&srv);
        assert_eq!(body["data"]["port"], 12345);
        assert_eq!(body["data"]["target"], "abc.ply.gg");
        assert!(matches(&json!({ "data": { "priority": 0, "weight": 5, "port": 12345, "target": "ABC.ply.gg." } }), &srv));
        assert!(!matches(&json!({ "data": { "priority": 0, "weight": 5, "port": 1, "target": "abc.ply.gg" } }), &srv));
    }
}
