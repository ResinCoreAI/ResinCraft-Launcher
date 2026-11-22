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
    /// Not an admin but one of the relay's developers: the Admin page shows only Dev, through [`dev_call`].
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub developer: bool,
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
        developer: false,
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
        return AdminStatus { admin: false, unlocked: false, username: None, dashboard_user: None, need_code: false, remembered: false, developer: false };
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
            // Revoked on the dashboard (or expired): forget it and ask again. Only when the relay answered: a relay
            // that's down or unreachable says nothing about the token, so it stays for when the relay is back.
            _ => {
                if relay::connect_for_list(&account).await.is_err() {
                    let mut st = status_of(&account, false, None, false, true);
                    st.developer = crate::updater::is_developer().await;
                    return st;
                }
                save_token(&account.uuid, None).await
            }
        }
    }
    let admin = match relay::connect_for_list(&account).await {
        Ok(conn) => conn.has_listings() && conn.admin,
        Err(_) => false,
    };
    let mut st = status_of(&account, admin, None, false, false);
    st.developer = !admin && crate::updater::is_developer().await;
    st
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

// ---------- Admin → Dev: uploading a Dev build from this PC ----------

/// A Dev build's update file picked on this PC: what the upload form shows and needs.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevBuildFile {
    pub path: String,
    pub name: String,
    pub size: u64,
    /// From the bundle's name ("ResinCraft Launcher_1.7.0-dev.1_x64-setup.exe"); empty when it has none.
    pub version: String,
    /// The `.sig` the build made next to it (the update signature) is there.
    pub signature: bool,
    /// The updater platform it's for, guessed from the name ("windows-x86_64").
    pub target: String,
}

/// What's in a bundle's file name.
fn dev_build_file(path: &std::path::Path, size: u64) -> DevBuildFile {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let version = name.split('_').find(|p| semver::Version::parse(p).is_ok()).unwrap_or_default().to_string();
    let lower = name.to_ascii_lowercase();
    let arm = lower.contains("arm64") || lower.contains("aarch64");
    let target = if lower.ends_with(".app.tar.gz") {
        "darwin-aarch64"
    } else if lower.ends_with(".appimage") || lower.ends_with(".deb") {
        if arm { "linux-aarch64" } else { "linux-x86_64" }
    } else if arm {
        "windows-aarch64"
    } else {
        "windows-x86_64"
    };
    let signature = std::path::PathBuf::from(format!("{}.sig", path.display())).is_file();
    DevBuildFile { path: path.display().to_string(), name, size, version, signature, target: target.into() }
}

/// Picks a Dev build's update file (None: cancelled).
pub async fn dev_pick_build() -> Result<Option<DevBuildFile>> {
    let picked = crate::core::shell::pick_file("Choose a Dev build", &[("Update file", &["exe", "gz", "AppImage", "deb"])]).await?;
    let Some(path) = picked else { return Ok(None) };
    let size = tokio::fs::metadata(&path).await?.len();
    Ok(Some(dev_build_file(&path, size)))
}

#[derive(Debug, Deserialize)]
pub struct DevUploadReq {
    pub path: String,
    pub version: String,
    pub target: String,
    #[serde(default)]
    pub notes: String,
    /// A developer's upload: the ticket comes from [`dev_call`], not the admin connection.
    #[serde(default)]
    pub developer: bool,
}

/// A developer's Dev API: `path` is the admin route (`/api/devs`, `/api/devs/add`...), sent to the relay as
/// `/api/dev/manage...` over HTTPS with the account's relay session (no admin unlock). The relay checks the role.
pub async fn dev_call(method: &str, path: &str, body: Option<Value>) -> Result<Value> {
    let Some(rest) = path.strip_prefix("/api/devs").filter(|r| r.is_empty() || r.starts_with('/')) else {
        anyhow::bail!("not a Dev route: {path}")
    };
    let Some(account) = accounts::microsoft_account(None).await else { return Err(relay::needs_microsoft()) };
    for retry in [false, true] {
        let (session, domain) = relay::session_token(&account).await?;
        let url = format!("https://{domain}/api/dev/manage{rest}");
        let http = crate::core::ctx::http();
        let req = if method == "GET" { http.get(url) } else { http.post(url).json(&body.clone().unwrap_or_else(|| serde_json::json!({}))) };
        let res = req.bearer_auth(&session).timeout(std::time::Duration::from_secs(30)).send().await.map_err(|e| anyhow::anyhow!("{}", e.without_url()))?;
        let status = res.status();
        // The saved session ran out: sign in again, once.
        if status.as_u16() == 401 && !retry {
            relay::forget_session(&account).await;
            continue;
        }
        let answer: Value = res.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            let why = answer.get("error").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
            return Err(tr!("admin.refused", "The relay refused: {error}", error = why).into());
        }
        return Ok(answer);
    }
    Err(tr!("admin.refused", "The relay refused: {error}", error = "HTTP 401").into())
}

/// Uploads a Dev build to the relay with a one-time ticket (`/api/devs/upload-ticket`, from the admin connection or a
/// developer's [`dev_call`]). The update signature comes from the `.sig` next to the file.
pub async fn dev_upload(req: DevUploadReq) -> Result<Value> {
    let signature = tokio::fs::read_to_string(format!("{}.sig", req.path)).await.map_err(|e| anyhow::anyhow!("{}.sig: {e}", req.path))?;
    let ticket = if req.developer { dev_call("POST", "/api/devs/upload-ticket", None).await? } else { call("POST", "/api/devs/upload-ticket", None).await? };
    let (Some(url), Some(t)) = (ticket["url"].as_str(), ticket["ticket"].as_str()) else { anyhow::bail!("the relay didn't give an upload ticket") };
    let body = tokio::fs::read(&req.path).await?;
    let res = crate::core::ctx::http()
        .put(format!("{url}/{}", urlencoding::encode(&req.target)))
        .bearer_auth(t)
        .header("X-Version", req.version.trim())
        .header("X-Signature", signature.trim())
        .header("X-Notes", urlencoding::encode(req.notes.trim()).into_owned())
        .header("Content-Type", "application/octet-stream")
        .timeout(std::time::Duration::from_secs(900))
        .body(body)
        .send()
        .await?;
    let status = res.status();
    let answer: Value = res.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        let why = answer.get("error").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
        return Err(tr!("admin.refused", "The relay refused: {error}", error = why).into());
    }
    Ok(answer)
}

#[cfg(test)]
mod dev_build_tests {
    use super::*;

    #[test]
    fn reads_bundle_names() {
        let f = dev_build_file(std::path::Path::new("C:/b/ResinCraft Launcher_1.6.2-dev.202611221530_x64-setup.exe"), 5);
        assert_eq!((f.version.as_str(), f.target.as_str(), f.signature), ("1.6.2-dev.202611221530", "windows-x86_64", false));
        let f = dev_build_file(std::path::Path::new("/b/ResinCraft.Launcher_1.7.0_arm64_linux.AppImage"), 5);
        assert_eq!((f.version.as_str(), f.target.as_str()), ("1.7.0", "linux-aarch64"));
        assert_eq!(dev_build_file(std::path::Path::new("setup.exe"), 5).version, "");
    }
}
