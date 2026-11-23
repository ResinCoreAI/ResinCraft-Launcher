//! "Report a problem" from inside the launcher (pages/Report.tsx), instead of a GitHub issue: the same reports as
//! report.rscraft.resincore.tech takes, sent to its API with this launcher's version and system filled in. Reports
//! go to the ResinCraft team only; nothing is public and no account is needed.
//!
//! Sent from the engine, not the page, so the site's cross-origin rules never get in the way.

use serde::{Deserialize, Serialize};
use serde_json::json;

use anyhow::Result;
use crate::{bail_tr, err_tr};

pub const REPORT_URL: &str = "https://report.rscraft.resincore.tech/api/report";
/// The status checker's lights, the same file the report site reads for its services.
pub const STATUS_URL: &str = "https://report.rscraft.resincore.tech/status.json";

/// The kinds the site knows (its form's first step).
const TYPES: &[&str] = &["bug", "crash", "idea", "question", "down"];
/// A "Service down" report names one or more of these (the site's list).
const SERVICES: &[&str] = &["java", "agents", "console", "list", "website", "tools", "payments", "other"];
const SINCE: &[&str] = &["just-now", "hour", "today", "longer"];

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Report {
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub message: String,
    pub steps: String,
    pub expected: String,
    /// The services a "Service down" report is about (the site lets the player tick several).
    pub services: Vec<String>,
    /// One service, as older pages send it; used when `services` is empty.
    pub service: String,
    pub address: String,
    pub since: String,
    /// The player's Minecraft name, only when they leave it in.
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Sent {
    /// The report's number at the team, when the site gives one.
    pub id: Option<String>,
}

fn clip(s: &str, max: usize) -> String {
    s.trim().chars().take(max).collect()
}

/// This system as the site's "System" field names it.
fn os() -> &'static str {
    match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "macos",
        "linux" => "linux",
        _ => "other",
    }
}

/// The site's own check: something@somewhere.tld, no spaces or brackets.
fn looks_like_email(v: &str) -> bool {
    let Some((user, host)) = v.split_once('@') else { return false };
    let bad = |c: char| c.is_whitespace() || "@<>()\",;:".contains(c);
    !user.is_empty() && !user.chars().any(bad) && !host.chars().any(|c| c.is_whitespace() || c == '@') && host.split_once('.').is_some_and(|(a, b)| !a.is_empty() && !b.is_empty())
}

/// What the status page says about each service right now ("java" → "operational", "major"…); empty when it
/// can't be reached. The report page lights its services with it.
pub async fn status() -> std::collections::HashMap<String, String> {
    let Ok(res) = crate::core::ctx::http().get(STATUS_URL).send().await else { return Default::default() };
    let v: serde_json::Value = res.json().await.unwrap_or_default();
    v.get("components")
        .and_then(|c| c.as_array())
        .map(|list| {
            list.iter()
                .filter_map(|c| Some((c.get("id")?.as_str()?.to_string(), c.get("state")?.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

/// The site's JSON for a report, after the same checks its form makes.
pub fn body(r: &Report) -> Result<serde_json::Value> {
    if !TYPES.contains(&r.kind.as_str()) {
        bail_tr!("report.kind", "Choose what kind of report it is.");
    }
    if r.title.trim().chars().count() < 5 {
        bail_tr!("report.title", "Give it a short title (at least 5 characters).");
    }
    if r.message.trim().chars().count() < 10 {
        bail_tr!("report.message", "Describe it in a few words (at least 10 characters).");
    }
    let down = r.kind == "down";
    let mut services: Vec<String> = Vec::new();
    for s in r.services.iter().chain(std::iter::once(&r.service)) {
        let s = s.trim();
        if SERVICES.contains(&s) && !services.iter().any(|x| x == s) {
            services.push(s.to_string());
        }
    }
    if down && services.is_empty() {
        bail_tr!("report.service", "Choose which service is down. You can pick more than one.");
    }
    if !down {
        services.clear();
    }
    let name = r.name.trim();
    if !name.is_empty() && (!(3..=16).contains(&name.len()) || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')) {
        bail_tr!("report.name", "That isn't a Minecraft name (3–16 letters, numbers or _).");
    }
    let email = r.email.trim();
    if !email.is_empty() && !looks_like_email(email) {
        bail_tr!("report.email", "That doesn't look like an email address.");
    }
    let bug_like = r.kind == "bug" || r.kind == "crash";
    Ok(json!({
        "kind": "launcher",
        "type": r.kind,
        "title": clip(&r.title, 120),
        "message": clip(&r.message, 3000),
        "steps": if bug_like { clip(&r.steps, 3000) } else { String::new() },
        "expected": if bug_like { clip(&r.expected, 1000) } else { String::new() },
        // A relay older than the services list reads only `service`: it gets the first.
        "service": services.first().cloned().unwrap_or_default(),
        "services": services,
        "address": if down { clip(&r.address, 120) } else { String::new() },
        "since": if down && SINCE.contains(&r.since.as_str()) { r.since.clone() } else { String::new() },
        "version": crate::updater::current_version(),
        "os": os(),
        "name": name,
        "email": clip(email, 254),
        // The site's form has a hidden field only bots fill in; it stays empty.
        "website": "",
    }))
}

/// Sends a report; the team's number for it when the site gives one.
pub async fn send(r: Report) -> Result<Sent> {
    let body = body(&r)?;
    let res = crate::core::ctx::http()
        .post(REPORT_URL)
        .header("x-status-report", "1")
        .json(&body)
        .send()
        .await
        .map_err(|_| err_tr!("report.offline", "Couldn't reach the report service. Check your internet connection and try again."))?;
    let ok = res.status().is_success();
    let out: serde_json::Value = res.json().await.unwrap_or(serde_json::Value::Null);
    if !ok {
        // The site says what was wrong in plain words ("title is too short"); passed on as it is.
        if let Some(e) = out.get("error").and_then(|e| e.as_str()).filter(|e| !e.trim().is_empty()) {
            let mut c = e.trim().chars();
            let first = c.next().map(|f| f.to_uppercase().collect::<String>()).unwrap_or_default();
            return Err(err_tr!("report.refused", "{error}.", error = format!("{first}{}", c.as_str())));
        }
        bail_tr!("report.failed", "Couldn't send it right now. Please try again in a minute.");
    }
    let id = out.get("id").and_then(|v| v.as_str().map(str::to_string).or_else(|| v.as_u64().map(|n| n.to_string())));
    Ok(Sent { id })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bug() -> Report {
        Report { kind: "bug".into(), title: "Discover crashes".into(), message: "It closes when I open Discover.".into(), steps: "Open Discover".into(), ..Default::default() }
    }

    #[test]
    fn a_report_reads_like_the_site_form() {
        let b = body(&bug()).unwrap();
        assert_eq!(b["kind"], "launcher");
        assert_eq!(b["type"], "bug");
        assert_eq!(b["steps"], "Open Discover");
        assert_eq!(b["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(b["website"], "");
        assert_eq!(b["service"], "");
        // Steps only go with bugs and crashes.
        let idea = body(&Report { kind: "idea".into(), ..bug() }).unwrap();
        assert_eq!(idea["steps"], "");
    }

    #[test]
    fn bad_reports_are_turned_back_before_sending() {
        assert!(body(&Report { kind: "rant".into(), ..bug() }).is_err());
        assert!(body(&Report { title: "x".into(), ..bug() }).is_err());
        assert!(body(&Report { message: "short".into(), ..bug() }).is_err());
        assert!(body(&Report { kind: "down".into(), service: "".into(), ..bug() }).is_err());
        assert!(body(&Report { kind: "down".into(), service: "java".into(), ..bug() }).is_ok());
        assert!(body(&Report { name: "no spaces allowed".into(), ..bug() }).is_err());
        assert!(body(&Report { name: "Puirkle_Gamer".into(), ..bug() }).is_ok());
        assert!(body(&Report { email: "not an email".into(), ..bug() }).is_err());
        assert!(body(&Report { email: "steve@example.com".into(), ..bug() }).is_ok());
    }

    #[test]
    fn a_down_report_names_every_service_ticked() {
        let down = |services: &[&str]| body(&Report { kind: "down".into(), services: services.iter().map(|s| s.to_string()).collect(), ..bug() });
        let b = down(&["java", "console", "java", "nope"]).unwrap();
        assert_eq!(b["services"], json!(["java", "console"]));
        // An older relay reads the first.
        assert_eq!(b["service"], "java");
        assert!(down(&[]).is_err());
        assert!(down(&["nope"]).is_err());
        // Only a down report names services.
        let bug = body(&Report { services: vec!["java".into()], ..bug() }).unwrap();
        assert_eq!(bug["services"], json!([]));
        assert_eq!(bug["service"], "");
    }
}
