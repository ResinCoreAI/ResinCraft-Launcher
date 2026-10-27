//! The Admin page (relay protocol 8): for Microsoft accounts the relay lists as its admins, the server list's admin
//! tools (approve, pin, recommend, ban…) over the relay connection, unlocked with the dashboard's password.
//!
//! The relay checks every request itself; this side only keeps one unlocked connection, with the password in memory
//! (never on disk) so a dropped connection can be unlocked again without asking.

use anyhow::Result;
use resincraft_relay::agent::Connection;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::player::accounts;
use crate::{err_tr, tr};

use super::relay;

struct Unlocked {
    conn: Connection,
    uuid: String,
    /// The dashboard user it's unlocked as (None: the relay's legacy `admin`).
    user: Option<String>,
    password: String,
}

static ADMIN: Mutex<Option<Unlocked>> = Mutex::const_new(None);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminStatus {
    /// The active Microsoft account is one of the relay's admins.
    pub admin: bool,
    /// Unlocked with the password in this session.
    pub unlocked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// The relay dashboard user it's signed in as (once unlocked).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dashboard_user: Option<String>,
}

/// Whether the active account may see the Admin page (asks the relay; false whenever it can't be asked).
pub async fn status() -> AdminStatus {
    let Some(account) = accounts::microsoft_account(None).await else { return AdminStatus { admin: false, unlocked: false, username: None, dashboard_user: None } };
    if let Some(u) = ADMIN.lock().await.as_ref() {
        if same(&u.uuid, &account.uuid) {
            return AdminStatus { admin: true, unlocked: true, username: Some(account.username), dashboard_user: u.user.clone() };
        }
    }
    let admin = match relay::connect_for_list(&account).await {
        Ok(conn) => conn.has_listings() && conn.admin,
        Err(_) => false,
    };
    AdminStatus { admin, unlocked: false, username: Some(account.username), dashboard_user: None }
}

fn same(a: &str, b: &str) -> bool {
    a.replace('-', "").eq_ignore_ascii_case(&b.replace('-', ""))
}

fn denied() -> anyhow::Error {
    err_tr!("admin.denied", "The relay didn't accept that. Check the username and password, or wait a few minutes after several wrong tries.")
}

async fn open(user: Option<String>, password: &str) -> Result<Unlocked> {
    let Some(account) = accounts::microsoft_account(None).await else { return Err(relay::needs_microsoft()) };
    let mut conn = relay::connect_for_list(&account).await?;
    if !conn.has_listings() || !conn.admin {
        return Err(err_tr!("admin.notAdmin", "This account isn't one of the relay's admins."));
    }
    match conn.admin_unlock(user.as_deref(), password).await {
        Ok(signed_in) => Ok(Unlocked { conn, uuid: account.uuid, user: signed_in.or(user), password: password.to_string() }),
        Err(e) if e.code() == Some(resincraft_relay::proto::ErrorCode::AdminDenied) => Err(denied()),
        Err(e) => Err(relay::describe(&e, "")),
    }
}

pub async fn unlock(user: Option<String>, password: &str) -> Result<AdminStatus> {
    let user = user.map(|u| u.trim().to_string()).filter(|u| !u.is_empty());
    let u = open(user, password).await?;
    *ADMIN.lock().await = Some(u);
    Ok(status().await)
}

pub async fn lock() {
    *ADMIN.lock().await = None;
}

/// One admin request; a dropped connection is opened and unlocked again once.
pub async fn call(method: &str, path: &str, body: Option<Value>) -> Result<Value> {
    let mut guard = ADMIN.lock().await;
    let Some(u) = guard.as_mut() else { return Err(err_tr!("admin.locked", "Unlock the Admin page with the relay's password first.")) };
    let answer = match u.conn.admin_call(method, path, body.clone()).await {
        Ok(a) => a,
        Err(e) if e.retryable() => {
            let (user, password) = (u.user.clone(), u.password.clone());
            *u = open(user, &password).await?;
            u.conn.admin_call(method, path, body).await.map_err(|e| relay::describe(&e, ""))?
        }
        Err(e) => {
            *guard = None;
            return Err(relay::describe(&e, ""));
        }
    };
    answer.map_err(|why| tr!("admin.refused", "The relay refused: {error}", error = why).into())
}

/// The relay's own dashboard (https://<base domain>/admin: tunnels, names, bans, usage, events…) in a window of its
/// own, signed in through a one-time link the relay hands out to an unlocked Admin page.
pub async fn open_dashboard() -> Result<()> {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
    const WINDOW: &str = "relay-dashboard";
    let answer = call("POST", "/api/dashboard-ticket", None).await?;
    let Some(url) = answer.get("url").and_then(Value::as_str) else {
        return Err(err_tr!("admin.noDashboard", "This relay can't open its dashboard from the launcher yet."));
    };
    let url: tauri::Url = url.parse().map_err(|_| err_tr!("admin.noDashboard", "This relay can't open its dashboard from the launcher yet."))?;
    // Only the relay's own address: the link signs the window in, so it mustn't lead anywhere else.
    let base = relay::settings().map(|s| s.domain).unwrap_or_default();
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if url.scheme() != "https" || base.is_empty() || !(host == base || host.ends_with(&format!(".{base}"))) {
        return Err(err_tr!("admin.noDashboard", "This relay can't open its dashboard from the launcher yet."));
    }
    let Some(app) = crate::core::ctx::app().cloned() else { return Ok(()) };
    if let Some(w) = app.get_webview_window(WINDOW) {
        let _ = w.navigate(url);
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }
    WebviewWindowBuilder::new(&app, WINDOW, WebviewUrl::External(url))
        .title(tr!("admin.dashboardWindow", "ResinCraft Relay dashboard").to_string())
        .inner_size(1280.0, 860.0)
        .min_inner_size(720.0, 480.0)
        .center()
        .build()?;
    Ok(())
}