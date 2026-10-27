//! The Admin page (relay protocol 8): for Microsoft accounts linked to a relay dashboard user, the server list's
//! admin tools over the relay connection, and the relay's full dashboard shown inside the launcher.
//!
//! Unlocking: the account is already Microsoft-verified on the relay connection; the relay also wants the dashboard
//! user's password, or a 2FA code when that user has 2FA on. "Remember this PC" keeps the relay's revocable device
//! token (sealed, never the password) so later unlocks ask nothing. The relay checks every request itself.

use std::collections::HashMap;

use anyhow::Result;
use resincraft_relay::agent::{AdminUnlock, Connection};
use resincraft_relay::proto::ErrorCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::core::fsutil::{read_json, write_json};
use crate::core::paths;
use crate::player::accounts::{self, MicrosoftAccount};
use crate::{err_tr, tr};

use super::relay;

struct Unlocked {
    conn: Connection,
    uuid: String,
    /// The dashboard user it's unlocked as.
    user: Option<String>,
    /// The relay's device token for this PC: reconnects unlock with it (kept on disk only with "remember").
    token: Option<String>,
    /// Without a token: the password, for reconnects (memory only).
    password: String,
}

static ADMIN: Mutex<Option<Unlocked>> = Mutex::const_new(None);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminStatus {
    /// The active Microsoft account is one of the relay's admins.
    pub admin: bool,
    /// Unlocked in this session.
    pub unlocked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// The relay dashboard user it's signed in as (once unlocked).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dashboard_user: Option<String>,
    /// The last unlock needs the user's 2FA code (ask for it and try again).
    pub need_code: bool,
    /// This PC unlocks by itself (a remembered device token).
    pub remembered: bool,
}

// ---------- Remembered PCs ----------

fn tokens_file() -> std::path::PathBuf {
    paths::root().join("relay-admin.json")
}

fn token_key(uuid: &str) -> Option<String> {
    Some(format!("{}|{}", relay::settings()?.url, uuid.replace('-', "").to_lowercase()))
}

async fn saved_token(uuid: &str) -> Option<String> {
    let all: HashMap<String, String> = read_json(&tokens_file()).await.unwrap_or_default();
    accounts::unseal(all.get(&token_key(uuid)?).map(String::as_str)).ok().filter(|t| !t.is_empty())
}

async fn save_token(uuid: &str, token: Option<&str>) {
    let Some(key) = token_key(uuid) else { return };
    let mut all: HashMap<String, String> = read_json(&tokens_file()).await.unwrap_or_default();
    match token {
        Some(t) => all.insert(key, accounts::seal(t)),
        None => all.remove(&key),
    };
    if let Err(e) = write_json(&tokens_file(), &all).await {
        eprintln!("[admin] couldn't save the remembered PC: {e:#}");
    }
}

fn device_name() -> String {
    std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "ResinCraft".into())
}

// ---------- Unlocking ----------

fn same(a: &str, b: &str) -> bool {
    a.replace('-', "").eq_ignore_ascii_case(&b.replace('-', ""))
}

fn status_of(account: &MicrosoftAccount, admin: bool, u: Option<&Unlocked>, need_code: bool, remembered: bool) -> AdminStatus {
    AdminStatus {
        admin,
        unlocked: u.is_some(),
        username: Some(account.username.clone()),
        dashboard_user: u.and_then(|u| u.user.clone()),
        need_code,
        remembered,
    }
}

/// How an unlock went: open, the user's 2FA code is needed, or refused.
enum Opened {
    Ready(Unlocked),
    NeedCode,
}

/// Connects as the active account and unlocks with a token, or the password and/or a 2FA code.
async fn open(account: &MicrosoftAccount, token: Option<&str>, password: &str, code: Option<&str>, remember: bool) -> Result<Opened> {
    let mut conn = relay::connect_for_list(account).await?;
    if !conn.has_listings() || !conn.admin {
        return Err(err_tr!("admin.notAdmin", "This account isn't one of the relay's admins."));
    }
    let device = device_name();
    // Always ask for a token: reconnects use it (on disk only when the player chose to remember this PC).
    let how = AdminUnlock { password, remember: token.is_none() || remember, device_token: token, device: Some(&device), totp: code, user: None };
    match conn.admin_unlock(how).await {
        Ok((user, new_token)) => Ok(Opened::Ready(Unlocked {
            conn,
            uuid: account.uuid.clone(),
            user,
            token: new_token.or_else(|| token.map(str::to_string)),
            password: password.to_string(),
        })),
        Err(e) if e.code() == Some(ErrorCode::AdminTwoFactor) && code.is_none() => Ok(Opened::NeedCode),
        Err(e) if e.code() == Some(ErrorCode::AdminTwoFactor) => {
            Err(err_tr!("admin.badCode", "That 2FA code didn't work. Use the newest code from your authenticator app."))
        }
        Err(e) if e.code() == Some(ErrorCode::AdminDenied) => Err(err_tr!(
            "admin.denied",
            "The relay didn't accept that. Check the password, or wait a few minutes after several wrong tries."
        )),
        Err(e) => Err(relay::describe(&e, "")),
    }
}

/// Whether the active account may see the Admin page; a remembered PC unlocks right here.
pub async fn status() -> AdminStatus {
    let Some(account) = accounts::microsoft_account(None).await else {
        return AdminStatus { admin: false, unlocked: false, username: None, dashboard_user: None, need_code: false, remembered: false };
    };
    let mut guard = ADMIN.lock().await;
    if let Some(u) = guard.as_ref().filter(|u| same(&u.uuid, &account.uuid)) {
        let remembered = saved_token(&account.uuid).await.is_some();
        return status_of(&account, true, Some(u), false, remembered);
    }
    *guard = None;
    if let Some(token) = saved_token(&account.uuid).await {
        match open(&account, Some(&token), "", None, true).await {
            Ok(Opened::Ready(u)) => {
                let st = status_of(&account, true, Some(&u), false, true);
                *guard = Some(u);
                return st;
            }
            // Revoked on the dashboard (or expired): forget it and ask again.
            _ => save_token(&account.uuid, None).await,
        }
    }
    let admin = match relay::connect_for_list(&account).await {
        Ok(conn) => conn.has_listings() && conn.admin,
        Err(_) => false,
    };
    status_of(&account, admin, None, false, false)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlockReq {
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub remember: bool,
}

pub async fn unlock(req: UnlockReq) -> Result<AdminStatus> {
    let Some(account) = accounts::microsoft_account(None).await else { return Err(relay::needs_microsoft()) };
    let code = req.code.as_deref().map(str::trim).filter(|c| !c.is_empty());
    match open(&account, None, &req.password, code, req.remember).await? {
        Opened::NeedCode => Ok(status_of(&account, true, None, true, false)),
        Opened::Ready(u) => {
            if req.remember {
                save_token(&account.uuid, u.token.as_deref()).await;
            }
            let st = status_of(&account, true, Some(&u), false, req.remember && u.token.is_some());
            *ADMIN.lock().await = Some(u);
            Ok(st)
        }
    }
}

/// Locks the page; `forget` also forgets this PC (the next unlock asks again).
pub async fn lock(forget: bool) {
    let old = ADMIN.lock().await.take();
    if forget {
        if let Some(account) = accounts::microsoft_account(None).await {
            save_token(&account.uuid, None).await;
        } else if let Some(u) = old {
            save_token(&u.uuid, None).await;
        }
    }
}

/// One admin request; a dropped connection is opened and unlocked again once (with the token, else the password).
pub async fn call(method: &str, path: &str, body: Option<Value>) -> Result<Value> {
    let mut guard = ADMIN.lock().await;
    let Some(u) = guard.as_mut() else { return Err(err_tr!("admin.locked", "Unlock the Admin page first.")) };
    let answer = match u.conn.admin_call(method, path, body.clone()).await {
        Ok(a) => a,
        Err(e) if e.retryable() => {
            let Some(account) = accounts::microsoft_account(Some(&u.uuid)).await else { return Err(relay::needs_microsoft()) };
            let (token, password) = (u.token.clone(), u.password.clone());
            match open(&account, token.as_deref(), &password, None, false).await? {
                Opened::Ready(fresh) => *u = fresh,
                Opened::NeedCode => {
                    *guard = None;
                    return Err(err_tr!("admin.locked", "Unlock the Admin page first."));
                }
            }
            u.conn.admin_call(method, path, body).await.map_err(|e| relay::describe(&e, ""))?
        }
        Err(e) => {
            *guard = None;
            return Err(relay::describe(&e, ""));
        }
    };
    answer.map_err(|why| tr!("admin.refused", "The relay refused: {error}", error = why).into())
}

// ---------- The relay's dashboard ----------

/// A one-time link that signs a page into the relay's dashboard (only ever the relay's own address).
pub async fn dashboard_link() -> Result<String> {
    let answer = call("POST", "/api/dashboard-ticket", None).await?;
    let no = || err_tr!("admin.noDashboard", "This relay can't open its dashboard from the launcher yet.");
    let url: tauri::Url = answer.get("url").and_then(Value::as_str).ok_or_else(no)?.parse().map_err(|_| no())?;
    let base = relay::settings().map(|s| s.domain).unwrap_or_default();
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if url.scheme() != "https" || base.is_empty() || !(host == base || host.ends_with(&format!(".{base}"))) {
        return Err(no());
    }
    Ok(url.to_string())
}

/// The dashboard in a window of its own (the Admin page's "pop out").
pub async fn open_dashboard() -> Result<()> {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
    const WINDOW: &str = "relay-dashboard";
    let url: tauri::Url = dashboard_link().await?.parse()?;
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
