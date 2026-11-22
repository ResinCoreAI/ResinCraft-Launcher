//! The Inbox page: mail from ResinCraft (the relay's admins) to the active Microsoft account, and the email address
//! it confirms for mail by email. `mail:call [method, path, body]` sends one of the relay's `/api/inbox…` requests over
//! HTTPS with the account's relay session (relay-server/docs/admin-api.md, section 9).

use anyhow::{bail, Result};
use serde_json::Value;

use crate::core::router::json;
use crate::core::Args;
use crate::err_tr;
use crate::player::accounts;
use crate::server::relay;

pub async fn handle(channel: &str, args: Args) -> Result<Value> {
    match channel {
        "mail:call" => json(call(&args.get::<String>(0)?, &args.get::<String>(1)?, args.get::<Option<Value>>(2)?).await?),
        _ => bail!("Unknown channel {channel}"),
    }
}

/// Only the player's own inbox routes go through here (the admins' mail goes over the admin connection).
fn inbox_path(path: &str) -> bool {
    (path == "/api/inbox" || path.starts_with("/api/inbox/")) && !path.contains("..") && !path.contains(['?', '#', ' '])
}

pub async fn call(method: &str, path: &str, body: Option<Value>) -> Result<Value> {
    if !inbox_path(path) || !matches!(method, "GET" | "POST") {
        bail!("not an inbox route: {method} {path}");
    }
    if relay::settings().is_none() {
        return Err(err_tr!("mail.noRelay", "This build of ResinCraft has no relay, so it has no inbox."));
    }
    let Some(account) = accounts::microsoft_account(None).await else { return Err(relay::needs_microsoft()) };
    for retry in [false, true] {
        let (session, domain) = relay::session_token(&account).await?;
        let url = format!("https://{domain}{path}");
        let http = crate::core::ctx::http();
        let req = if method == "GET" { http.get(url) } else { http.post(url).json(&body.clone().unwrap_or_else(|| serde_json::json!({}))) };
        let res = req
            .bearer_auth(&session)
            .timeout(std::time::Duration::from_secs(45))
            .send()
            .await
            .map_err(|e| err_tr!("mail.unreachable", "Couldn't reach ResinCraft Relay ({error}).", error = e.without_url().to_string()))?;
        let status = res.status();
        // The saved session ran out: sign in again, once.
        if status.as_u16() == 401 && !retry {
            relay::forget_session(&account).await;
            continue;
        }
        // A relay from before mail.
        if status.as_u16() == 404 && path == "/api/inbox" {
            return Err(err_tr!("mail.notYet", "ResinCraft Relay doesn't have the inbox yet."));
        }
        let answer: Value = res.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            let why = answer.get("error").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
            return Err(err_tr!("mail.relaySaid", "ResinCraft Relay: {error}", error = why));
        }
        return Ok(answer);
    }
    Err(err_tr!("mail.relaySaid", "ResinCraft Relay: {error}", error = "HTTP 401"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_inbox_routes() {
        for ok in ["/api/inbox", "/api/inbox/unread", "/api/inbox/email/confirm"] {
            assert!(inbox_path(ok), "{ok}");
        }
        for bad in ["/api/mail/send", "/api/inboxes", "/api/inbox/../mail", "/api/inbox?x=1", "/admin/api/inbox", ""] {
            assert!(!inbox_path(bad), "{bad}");
        }
    }
}
