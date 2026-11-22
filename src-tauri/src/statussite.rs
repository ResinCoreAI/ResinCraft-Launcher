//! ResinCraft Status (`https://status.<relay domain>`, web/status on the VPS): the team's own status page with posted
//! incidents, planned maintenance, 90 days of history and email updates. The launcher's Status page shows it, and the
//! Admin page posts to it.
//!
//! - Players: `status.json` (what its own page reads, written by monitor.js every minute) and `POST /api/subscribe`
//!   (its "Get email updates": the service emails a confirmation link; nothing else happens here).
//! - Admins: its posting page's API (`/admin/api/...`, admin.js) with its own password. The password is only passed
//!   through to sign in and never kept; the session cookie it hands back lives in memory until sign-out, a 401 or
//!   the launcher closes (the service also ends sessions after 12 hours or a restart).
//!
//! Sent from the engine, not the page: the service only takes calls with its own header and no foreign Origin, and its
//! sign-in cookie is HttpOnly.

use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;
use serde_json::{json, Value};

use crate::err_tr;

/// The relay's base domain when this build has none compiled in (status.rs's relay rows say "not in this build").
const FALLBACK_DOMAIN: &str = "rscraft.resincore.tech";
const TIMEOUT: Duration = Duration::from_secs(10);
/// admin.js's sign-in cookie.
const COOKIE: &str = "rcs_admin";

/// `https://status.<relay domain>`.
pub fn base() -> String {
    let domain = crate::server::relay::settings().map(|s| s.domain).unwrap_or_else(|| FALLBACK_DOMAIN.to_string());
    format!("https://status.{domain}")
}

/// The posting page's session, in memory only.
static SESSION: Mutex<Option<String>> = Mutex::new(None);

fn session() -> Option<String> {
    SESSION.lock().ok().and_then(|s| s.clone())
}
fn set_session(v: Option<String>) {
    if let Ok(mut s) = SESSION.lock() {
        *s = v;
    }
}

fn unreachable() -> anyhow::Error {
    err_tr!("statusSite.unreachable", "Couldn't reach ResinCraft Status. Try again in a moment.")
}

/// The service's own words for a refusal (`{"error": "..."}`), else a general one.
async fn answer(res: reqwest::Response) -> Result<Value> {
    let ok = res.status().is_success();
    let code = res.status().as_u16();
    let v: Value = res.json().await.unwrap_or(Value::Null);
    if ok {
        return Ok(v);
    }
    if let Some(e) = v.get("error").and_then(Value::as_str).filter(|e| !e.trim().is_empty()) {
        anyhow::bail!("{}", e.trim());
    }
    Err(err_tr!("statusSite.refused", "ResinCraft Status refused that (HTTP {code}).", code = code))
}

/// What the status page shows: parts with their day bars, incidents and maintenance, outages and what it offers
/// (`features.subscribe`, `features.reportEmail`). `app:statusSite`.
pub async fn public() -> Result<Value> {
    public_at(crate::core::ctx::http(), &base(), crate::core::fsutil::now_ms()).await
}

pub async fn public_at(client: &reqwest::Client, base: &str, now_ms: i64) -> Result<Value> {
    let res = client.get(format!("{base}/status.json?t={now_ms}")).header("Cache-Control", "no-cache").timeout(TIMEOUT).send().await.map_err(|_| unreachable())?;
    if !res.status().is_success() {
        return Err(unreachable());
    }
    let v: Value = res.json().await.map_err(|_| unreachable())?;
    if !v.is_object() {
        return Err(unreachable());
    }
    Ok(v)
}

/// "Get email updates": the service emails `email` a link to confirm (`app:statusSubscribe`).
pub async fn subscribe(email: &str) -> Result<()> {
    subscribe_at(crate::core::ctx::http(), &base(), email).await
}

pub async fn subscribe_at(client: &reqwest::Client, base: &str, email: &str) -> Result<()> {
    let email = email.trim();
    if email.is_empty() || email.len() > 254 || !email.contains('@') {
        return Err(err_tr!("statusSite.badEmail", "Type your email address."));
    }
    let res = client
        .post(format!("{base}/api/subscribe"))
        .header("x-status-report", "1")
        .json(&json!({ "email": email, "website": "" }))
        .timeout(TIMEOUT)
        .send()
        .await
        .map_err(|_| unreachable())?;
    answer(res).await.map(|_| ())
}

/// The token in a `Set-Cookie: rcs_admin=<token>; ...` header.
fn cookie_token(headers: &reqwest::header::HeaderMap) -> Option<String> {
    headers.get_all(reqwest::header::SET_COOKIE).iter().filter_map(|v| v.to_str().ok()).find_map(|c| {
        let first = c.split(';').next()?.trim();
        let (name, value) = first.split_once('=')?;
        (name.trim() == COOKIE && !value.is_empty() && value.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')).then(|| value.to_string())
    })
}

/// Only the posting page's API: `/admin/api/` and a plain path after it.
fn admin_path_ok(path: &str) -> bool {
    path.strip_prefix("/admin/api/").is_some_and(|rest| !rest.is_empty() && rest.split('/').all(|seg| !seg.is_empty() && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')))
}

/// Signs in to the posting page with its password (not kept): its state, as `/admin/api/state` gives it.
pub async fn admin_login(password: &str) -> Result<Value> {
    let (state, token) = admin_login_at(crate::core::ctx::http(), &base(), password).await?;
    set_session(Some(token));
    Ok(state)
}

pub async fn admin_login_at(client: &reqwest::Client, base: &str, password: &str) -> Result<(Value, String)> {
    let res = client
        .post(format!("{base}/admin/api/login"))
        .header("x-status-admin", "1")
        .json(&json!({ "password": password }))
        .timeout(TIMEOUT)
        .send()
        .await
        .map_err(|_| unreachable())?;
    let token = cookie_token(res.headers());
    let state = answer(res).await?;
    let token = token.ok_or_else(|| err_tr!("statusSite.noSession", "ResinCraft Status didn't start a session. Try signing in again."))?;
    Ok((state, token))
}

/// One call to the posting page's API with this launcher's session (`GET /admin/api/state`, `POST
/// /admin/api/incidents`...). Signed out (no session, or the service ended it): its state says `signedIn: false`.
pub async fn admin_call(method: &str, path: &str, body: Option<Value>) -> Result<Value> {
    let token = session();
    let out = admin_call_at(crate::core::ctx::http(), &base(), token.as_deref(), method, path, body).await;
    if let Err(e) = &out {
        if e.downcast_ref::<SignedOut>().is_some() {
            set_session(None);
        }
    }
    out
}

/// The service ended the session (401): sign in again.
#[derive(Debug)]
pub struct SignedOut;
impl std::fmt::Display for SignedOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Signed out of ResinCraft Status. Sign in again.")
    }
}
impl std::error::Error for SignedOut {}

pub async fn admin_call_at(client: &reqwest::Client, base: &str, token: Option<&str>, method: &str, path: &str, body: Option<Value>) -> Result<Value> {
    if !admin_path_ok(path) || !matches!(method, "GET" | "POST") {
        return Err(err_tr!("statusSite.badPath", "That isn't one of ResinCraft Status's admin calls."));
    }
    let url = format!("{base}{path}");
    let mut req = if method == "GET" { client.get(url) } else { client.post(url).json(&body.unwrap_or_else(|| json!({}))) };
    req = req.header("x-status-admin", "1").timeout(Duration::from_secs(60));
    if let Some(t) = token {
        req = req.header(reqwest::header::COOKIE, format!("{COOKIE}={t}"));
    }
    let res = req.send().await.map_err(|_| unreachable())?;
    if res.status().as_u16() == 401 && path != "/admin/api/login" {
        return Err(SignedOut.into());
    }
    answer(res).await
}

/// Signs out of the posting page and forgets the session.
pub async fn admin_logout() {
    if let Some(t) = session() {
        let _ = admin_call_at(crate::core::ctx::http(), &base(), Some(&t), "POST", "/admin/api/logout", None).await;
    }
    set_session(None);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// A one-request server: answers `status` with `headers` and `body`, and hands back the request it got.
    async fn serve(status: &'static str, headers: &'static str, body: &'static str) -> (String, tokio::sync::oneshot::Receiver<String>) {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let (mut s, _) = l.accept().await.unwrap();
            let mut buf = vec![0u8; 8192];
            let n = s.read(&mut buf).await.unwrap_or(0);
            let _ = tx.send(String::from_utf8_lossy(&buf[..n]).to_string());
            let reply = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            let _ = s.write_all(reply.as_bytes()).await;
        });
        (base, rx)
    }

    #[test]
    fn only_the_posting_pages_api() {
        assert!(admin_path_ok("/admin/api/state"));
        assert!(admin_path_ok("/admin/api/incidents/12/resolve"));
        assert!(admin_path_ok("/admin/api/subscribers/3/delete"));
        for bad in ["/admin/api/", "/admin/api/../x", "/api/subscribe", "/admin/api/a//b", "/admin/api/x?y=1", "/internal/mail", "https://evil/admin/api/state"] {
            assert!(!admin_path_ok(bad), "{bad}");
        }
    }

    #[test]
    fn reads_the_session_cookie() {
        let mut h = reqwest::header::HeaderMap::new();
        h.append(reqwest::header::SET_COOKIE, "other=1; Path=/".parse().unwrap());
        h.append(reqwest::header::SET_COOKIE, "rcs_admin=Ab_9-x; Path=/admin/; HttpOnly; Secure".parse().unwrap());
        assert_eq!(cookie_token(&h).as_deref(), Some("Ab_9-x"));
        let mut h = reqwest::header::HeaderMap::new();
        h.append(reqwest::header::SET_COOKIE, "rcs_admin=; Max-Age=0".parse().unwrap());
        assert_eq!(cookie_token(&h), None);
    }

    #[tokio::test]
    async fn signs_in_and_sends_the_cookie_and_header() {
        let client = reqwest::Client::new();
        let (base, req) = serve("200 OK", "Set-Cookie: rcs_admin=tok123; Path=/admin/; HttpOnly\r\n", r#"{"signedIn":true,"incidents":[]}"#).await;
        let (state, token) = admin_login_at(&client, &base, "a long password").await.unwrap();
        assert_eq!((state["signedIn"].as_bool(), token.as_str()), (Some(true), "tok123"));
        let sent = req.await.unwrap();
        assert!(sent.starts_with("POST /admin/api/login "), "{sent}");
        assert!(sent.to_ascii_lowercase().contains("x-status-admin: 1"));
        assert!(sent.contains(r#""password":"a long password""#));

        let (base, req) = serve("200 OK", "", r#"{"signedIn":true}"#).await;
        admin_call_at(&client, &base, Some("tok123"), "POST", "/admin/api/incidents/4/resolve", Some(json!({"message":"ok"}))).await.unwrap();
        let sent = req.await.unwrap().to_ascii_lowercase();
        assert!(sent.starts_with("post /admin/api/incidents/4/resolve "));
        assert!(sent.contains("cookie: rcs_admin=tok123") && sent.contains("x-status-admin: 1"));
    }

    #[tokio::test]
    async fn refusals_carry_the_services_words() {
        let client = reqwest::Client::new();
        let (base, _) = serve("401 Unauthorized", "", r#"{"error":"Wrong password."}"#).await;
        let e = admin_login_at(&client, &base, "nope").await.unwrap_err();
        assert_eq!(e.to_string(), "Wrong password.");
        let (base, _) = serve("401 Unauthorized", "", r#"{"error":"Signed out. Sign in again."}"#).await;
        let e = admin_call_at(&client, &base, Some("old"), "GET", "/admin/api/state", None).await.unwrap_err();
        assert!(e.downcast_ref::<SignedOut>().is_some());
        let (base, _) = serve("400 Bad Request", "", r#"{"error":"The title is needed."}"#).await;
        let e = admin_call_at(&client, &base, Some("t"), "POST", "/admin/api/incidents", Some(json!({}))).await.unwrap_err();
        assert_eq!(e.to_string(), "The title is needed.");
    }

    #[tokio::test]
    async fn subscribes_with_the_status_pages_header() {
        let client = reqwest::Client::new();
        let (base, req) = serve("200 OK", "", r#"{"ok":true}"#).await;
        subscribe_at(&client, &base, " me@example.com ").await.unwrap();
        let sent = req.await.unwrap();
        assert!(sent.starts_with("POST /api/subscribe "));
        assert!(sent.to_ascii_lowercase().contains("x-status-report: 1"));
        assert!(sent.contains(r#""email":"me@example.com""#));
        assert!(subscribe_at(&client, &base, "nope").await.is_err());
        let (base, _) = serve("400 Bad Request", "", r#"{"error":"Email updates aren't available right now."}"#).await;
        assert_eq!(subscribe_at(&client, &base, "me@example.com").await.unwrap_err().to_string(), "Email updates aren't available right now.");
    }

    #[tokio::test]
    async fn public_json_or_unreachable() {
        let client = reqwest::Client::new();
        let (base, req) = serve("200 OK", "", r#"{"updated":1,"components":[],"incidents":[]}"#).await;
        let v = public_at(&client, &base, 42).await.unwrap();
        assert_eq!(v["updated"], 1);
        assert!(req.await.unwrap().starts_with("GET /status.json?t=42 "));
        let (base, _) = serve("502 Bad Gateway", "", "").await;
        assert!(public_at(&client, &base, 1).await.is_err());
    }
}
