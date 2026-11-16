//! BuiltByBit (builtbybit.com) as a place to get plugins, through its Discovery API (api.builtbybit.com/v2,
//! github.com/BuiltByBit/api-openapi-spec). The player signs in with their own BuiltByBit account (OAuth 2
//! authorization code, in a ResinCraft window on BuiltByBit's own page; ResinCraft never sees the password), and
//! the launcher lists the plugins they own (`licenses`, paid and free) and installs or updates them with BuiltByBit's
//! direct download (`download/direct/initiate`, then `status` until the file's URL is ready).
//!
//! The app's OAuth client comes from build.rs like the other service keys: `RESINCRAFT_BBB_CLIENT_ID`,
//! `RESINCRAFT_BBB_CLIENT_SECRET`, and `RESINCRAFT_BBB_REDIRECT` (the redirect URI registered with BuiltByBit; the
//! window catches it, so nothing has to answer there). Without a client id the source is hidden. The player's tokens
//! are kept in `builtbybit.json` in the data folder, sealed like Microsoft's (DPAPI on Windows).
//!
//! Project ids are "bbb:<resource id>".

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::ctx::http;
use crate::core::fsutil::{read_json, write_json};
use crate::core::paths;
use crate::core::types::{ProjectVersion, ServerAddonHit, ServerAddonResult};
use crate::player::accounts::{seal, unseal};
use crate::{bail_tr, err_tr};

pub const PREFIX: &str = "bbb:";
const API: &str = "https://api.builtbybit.com";
const AUTHORIZE: &str = "https://builtbybit.com/account/external/authorize";
const DEFAULT_REDIRECT: &str = "https://resincore.ai/oauth/builtbybit";
const WINDOW: &str = "builtbybit-login";

fn key(builtin: Option<&'static str>, env: &str) -> String {
    builtin.map(str::to_string).or_else(|| std::env::var(env).ok()).unwrap_or_default().trim().to_string()
}
fn client_id() -> String {
    key(option_env!("RESINCRAFT_BBB_CLIENT_ID"), "RESINCRAFT_BBB_CLIENT_ID")
}
fn client_secret() -> String {
    key(option_env!("RESINCRAFT_BBB_CLIENT_SECRET"), "RESINCRAFT_BBB_CLIENT_SECRET")
}
fn redirect() -> String {
    Some(key(option_env!("RESINCRAFT_BBB_REDIRECT"), "RESINCRAFT_BBB_REDIRECT")).filter(|r| !r.is_empty()).unwrap_or_else(|| DEFAULT_REDIRECT.into())
}

/// Whether this build can sign in to BuiltByBit.
pub fn configured() -> bool {
    !client_id().is_empty() && !client_secret().is_empty()
}

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
struct Stored {
    /// "token": the player's own Private API token (in `access`, never expires); otherwise OAuth.
    #[serde(default)]
    kind: String,
    access: String,
    refresh: String,
    /// Unix seconds.
    expires: i64,
    username: String,
    #[serde(default)]
    avatar: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// This build has the OAuth sign-in (a player's own API token always works).
    pub configured: bool,
    /// Signed in with an API token rather than OAuth.
    pub token: bool,
    pub signed_in: bool,
    pub username: Option<String>,
    pub avatar: Option<String>,
}

fn file() -> std::path::PathBuf {
    paths::root().join("builtbybit.json")
}

async fn load() -> Option<Stored> {
    read_json::<Stored>(&file()).await.filter(|s| !s.access.is_empty())
}

async fn store(s: &Stored) -> Result<()> {
    write_json(&file(), s).await
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

pub async fn status() -> Status {
    let s = load().await;
    Status {
        configured: configured(),
        token: s.as_ref().is_some_and(|s| s.kind == "token"),
        signed_in: s.is_some(),
        username: s.as_ref().map(|s| s.username.clone()),
        avatar: s.and_then(|s| s.avatar),
    }
}

/// A BuiltByBit answer: `{"result":"success","data":…}` or `{"result":"error","error":{"message":…}}`.
fn data_of(v: Value, status: u16) -> Result<Value> {
    if v["result"] == "success" {
        return Ok(v["data"].clone());
    }
    let msg = v["error"]["message"].as_str().or_else(|| v["error"].as_str()).unwrap_or("no details").to_string();
    if status == 401 || status == 403 {
        bail_tr!("builtbybit.denied", "BuiltByBit refused the request ({error}). Sign in again under Plugins → BuiltByBit.", error = msg);
    }
    Err(anyhow!("BuiltByBit: {msg} (HTTP {status})"))
}

/// The token endpoint, with the app's client credentials (HTTP Basic).
async fn token(form: &[(&str, &str)]) -> Result<Value> {
    let res = http().post(format!("{API}/oauth2/token")).basic_auth(client_id(), Some(client_secret())).form(form).send().await?;
    let status = res.status().as_u16();
    let v: Value = res.json().await.unwrap_or(Value::Null);
    if v.get("access_token").and_then(Value::as_str).is_some_and(|t| !t.is_empty()) {
        return Ok(v);
    }
    let msg = v["error_description"].as_str().or(v["error"]["message"].as_str()).or(v["error"].as_str()).unwrap_or("no token").to_string();
    Err(err_tr!("builtbybit.tokenFailed", "BuiltByBit sign-in failed: {error}", error = format!("{msg} (HTTP {status})")))
}

fn stored_from(tok: &Value, username: String, avatar: Option<String>) -> Stored {
    Stored {
        access: seal(tok["access_token"].as_str().unwrap_or_default()),
        refresh: seal(tok["refresh_token"].as_str().unwrap_or_default()),
        kind: "oauth".into(),
        expires: now() + tok["expires_in"].as_i64().unwrap_or(3600) - 60,
        username,
        avatar,
    }
}

/// A fresh access token, refreshed when it has run out.
async fn access() -> Result<String> {
    let Some(mut s) = load().await else {
        bail_tr!("builtbybit.signedOut", "Sign in with BuiltByBit to see and install the plugins you own.");
    };
    if s.kind == "token" {
        return Ok(format!("Private {}", unseal(Some(&s.access))?));
    }
    if s.expires > now() {
        return Ok(format!("Bearer {}", unseal(Some(&s.access))?));
    }
    let refresh = unseal(Some(&s.refresh))?;
    let tok = token(&[("grant_type", "refresh_token"), ("refresh_token", &refresh)]).await.map_err(|_| {
        err_tr!("builtbybit.expired", "Your BuiltByBit sign-in expired. Sign in again under Plugins → BuiltByBit.")
    })?;
    let fresh = stored_from(&tok, s.username.clone(), s.avatar.clone());
    s = Stored { refresh: if tok["refresh_token"].is_string() { fresh.refresh.clone() } else { s.refresh }, ..fresh };
    store(&s).await?;
    Ok(format!("Bearer {}", unseal(Some(&s.access))?))
}

/// GET on the API as the signed-in player.
async fn get(path: &str) -> Result<Value> {
    let auth = access().await?;
    let res = http().get(format!("{API}{path}")).header("Authorization", auth).send().await?;
    let status = res.status().as_u16();
    let v: Value = res.json().await.unwrap_or(Value::Null);
    data_of(v, status)
}

fn enc(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

fn close_window() {
    use tauri::Manager;
    if let Some(w) = crate::core::ctx::app().and_then(|a| a.get_webview_window(WINDOW)) {
        let _ = w.close();
    }
}

/// Signs in on BuiltByBit's own page in a ResinCraft window and waits for it to finish.
pub async fn sign_in() -> Result<Status> {
    use tauri::{WebviewUrl, WebviewWindowBuilder, WindowEvent};
    if !configured() {
        bail_tr!("builtbybit.notSetUp", "BuiltByBit sign-in isn't set up in this build of ResinCraft.");
    }
    close_window();
    let state = uuid::Uuid::new_v4().simple().to_string();
    let redirect_uri = redirect();
    let mut url = format!("{AUTHORIZE}?response_type=code&client_id={}&redirect_uri={}&state={state}", enc(&client_id()), enc(&redirect_uri));
    let scopes = key(option_env!("RESINCRAFT_BBB_SCOPES"), "RESINCRAFT_BBB_SCOPES");
    if !scopes.is_empty() {
        url.push_str(&format!("&scope={}", enc(&scopes)));
    }
    let app = crate::core::ctx::app().ok_or_else(|| anyhow!("Not available without the app window"))?;
    let (tx, rx) = tokio::sync::oneshot::channel::<Option<Result<String>>>();
    let tx = Arc::new(Mutex::new(Some(tx)));
    let nav_tx = tx.clone();
    let expect_state = state.clone();
    let catch = redirect_uri.clone();
    let builder = WebviewWindowBuilder::new(app, WINDOW, WebviewUrl::External(url.parse()?));
    #[cfg(windows)]
    let builder = builder.additional_browser_args(crate::webview::BROWSER_ARGS);
    let window = builder
        .title(crate::tr!("builtbybit.window", "Sign in with BuiltByBit").to_string())
        .inner_size(520.0, 720.0)
        .center()
        .incognito(true)
        .on_navigation(move |u| {
            if !u.as_str().starts_with(&catch) {
                return true;
            }
            let q: std::collections::HashMap<String, String> = u.query_pairs().into_owned().collect();
            let result = match (q.get("code"), q.get("error")) {
                (Some(code), _) if q.get("state") == Some(&expect_state) => Ok(code.clone()),
                (Some(_), _) => Err(anyhow!("The sign-in answer didn't match. Please try again.")),
                (None, Some(e)) => Err(err_tr!("builtbybit.refused", "BuiltByBit sign-in was cancelled ({error}).", error = q.get("error_description").unwrap_or(e).clone())),
                _ => Err(anyhow!("BuiltByBit sent no sign-in code.")),
            };
            if let Some(tx) = nav_tx.lock().unwrap().take() {
                let _ = tx.send(Some(result));
            }
            false
        })
        .build()?;
    window.on_window_event(move |e| {
        if matches!(e, WindowEvent::Destroyed) {
            if let Some(tx) = tx.lock().unwrap().take() {
                let _ = tx.send(None);
            }
        }
    });
    let code = match rx.await {
        Ok(Some(code)) => code,
        _ => Err(err_tr!("builtbybit.closed", "The BuiltByBit window was closed before you finished.")),
    };
    close_window();
    let code = code?;
    let tok = token(&[("grant_type", "authorization_code"), ("code", &code), ("redirect_uri", &redirect_uri)]).await?;
    // Save first, so the member lookup below can use the token.
    store(&stored_from(&tok, String::new(), None)).await?;
    let me = get("/v2/members/self").await.unwrap_or(Value::Null);
    let username = me["self"]["username"].as_str().unwrap_or("BuiltByBit").to_string();
    let avatar = me["self"]["avatar_url_small"].as_str().map(str::to_string);
    store(&stored_from(&tok, username, avatar)).await?;
    Ok(status().await)
}

/// Signs in with the player's own Private API token (builtbybit.com/account/api), checked against their profile.
pub async fn set_token(token: &str) -> Result<Status> {
    let token = token.trim().trim_start_matches("Private ").trim();
    if token.len() < 10 || token.contains(char::is_whitespace) {
        bail_tr!("builtbybit.badToken", "That doesn't look like a BuiltByBit API token. Copy the whole Private token from builtbybit.com/account/api.");
    }
    let res = http().get(format!("{API}/v2/members/self")).header("Authorization", format!("Private {token}")).send().await?;
    let code = res.status().as_u16();
    let v: Value = res.json().await.unwrap_or(Value::Null);
    let me = data_of(v, code).map_err(|e| err_tr!("builtbybit.tokenRefused", "BuiltByBit didn't accept that token: {error}", error = e.to_string()))?;
    let stored = Stored {
        kind: "token".into(),
        access: seal(token),
        refresh: String::new(),
        expires: i64::MAX,
        username: me["self"]["username"].as_str().unwrap_or("BuiltByBit").to_string(),
        avatar: me["self"]["avatar_url_small"].as_str().map(str::to_string),
    };
    store(&stored).await?;
    Ok(status().await)
}

pub async fn sign_out() -> Result<Status> {
    if let Some(s) = load().await.filter(|s| s.kind != "token") {
        if let Ok(t) = unseal(Some(&s.refresh)) {
            let _ = http().post(format!("{API}/oauth2/token/revoke")).basic_auth(client_id(), Some(client_secret())).form(&[("token", t.as_str())]).send().await;
        }
    }
    let _ = tokio::fs::remove_file(file()).await;
    Ok(status().await)
}

/// A plugin the player owns.
#[derive(Clone, Debug)]
pub struct Owned {
    pub id: u64,
    pub title: String,
    pub summary: String,
    pub url: String,
    pub icon: Option<String>,
    pub downloads: u64,
    pub version_id: String,
    pub version: String,
    pub creator: String,
}

fn owned_from(r: &Value) -> Option<Owned> {
    Some(Owned {
        id: r["resource_id"].as_u64()?,
        title: r["title"].as_str().unwrap_or_default().to_string(),
        summary: r["summary"].as_str().unwrap_or_default().to_string(),
        url: r["url"].as_str().unwrap_or_default().to_string(),
        icon: r["cover_image_url"].as_str().filter(|s| !s.is_empty()).map(str::to_string),
        downloads: r["downloads"].as_u64().unwrap_or(0),
        version_id: r["LatestVersion"]["version_id"].as_u64().map(|v| v.to_string()).unwrap_or_default(),
        version: r["LatestVersion"]["version_string"].as_str().unwrap_or_default().to_string(),
        creator: r["Creator"]["username"].as_str().unwrap_or_default().to_string(),
    })
}

/// Whether a license still counts: a permanent one that's active, or a timed one that hasn't ended.
fn license_live(l: &Value) -> bool {
    if l["permanent"].as_bool().unwrap_or(true) {
        l["active"].as_bool().unwrap_or(true)
    } else {
        l["end_date"].as_i64().is_none_or(|end| end == 0 || end > now())
    }
}

/// Everything the player owns on BuiltByBit (resources only), newest list from the API.
pub async fn owned() -> Result<Vec<Owned>> {
    let mut out = Vec::new();
    for page in 1..=20 {
        let d = get(&format!("/v2/resources/discover/licenses?with=Resource&per_page=100&page={page}")).await?;
        let list = d["licenses"].as_array().cloned().unwrap_or_default();
        for l in &list {
            if l["content_type"] == "resource" && license_live(l) {
                if let Some(o) = owned_from(&l["Resource"]) {
                    out.push(o);
                }
            }
        }
        if list.len() < 100 {
            break;
        }
    }
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    out.dedup_by_key(|o| o.id);
    Ok(out)
}

/// The plugin browser's BuiltByBit tab: what the player owns, filtered by the search.
pub async fn search(query: &str, offset: usize, page: usize) -> Result<ServerAddonResult> {
    let q = query.trim().to_lowercase();
    let list: Vec<Owned> = owned().await?.into_iter().filter(|o| q.is_empty() || o.title.to_lowercase().contains(&q) || o.summary.to_lowercase().contains(&q)).collect();
    let total = list.len() as u64;
    Ok(ServerAddonResult {
        total,
        hits: list
            .into_iter()
            .skip(offset)
            .take(page)
            .map(|o| ServerAddonHit {
                id: format!("{PREFIX}{}", o.id),
                title: o.title,
                description: o.summary,
                icon_url: o.icon,
                author: o.creator,
                downloads: o.downloads,
                stars: None,
                source: "builtbybit",
                external: false,
                paid: false,
                url: o.url,
            })
            .collect(),
    })
}

/// One resource the player owns, by id.
pub async fn resource(rid: &str) -> Result<Owned> {
    let id: u64 = rid.parse().map_err(|_| anyhow!("Unknown BuiltByBit resource"))?;
    owned().await?.into_iter().find(|o| o.id == id).ok_or_else(|| {
        err_tr!("builtbybit.notOwned", "This BuiltByBit plugin isn't on your account. Buy or claim it on BuiltByBit first, then sign in with that account.")
    })
}

pub fn latest_version(o: &Owned) -> ProjectVersion {
    ProjectVersion {
        id: o.version_id.clone(),
        name: o.version.clone(),
        version_number: o.version.clone(),
        game_versions: Vec::new(),
        loaders: vec!["spigot".into()],
        kind: "release".into(),
        published: String::new(),
        downloads: o.downloads,
    }
}

/// The download URL for a resource's newest version (or `version_id`), from BuiltByBit's direct download.
pub async fn download_url(rid: &str, version_id: Option<&str>) -> Result<String> {
    let (kind, id) = match version_id.filter(|v| !v.is_empty()) {
        Some(v) => ("resource_version", v.to_string()),
        None => ("resource", rid.to_string()),
    };
    let d = get(&format!("/v2/resources/discover/download/direct/initiate?content_type={kind}&content_id={}", enc(&id))).await?;
    let token = d["request"]["token"].as_str().ok_or_else(|| anyhow!("BuiltByBit didn't start the download"))?.to_string();
    for _ in 0..60 {
        let s = get(&format!("/v2/resources/discover/download/direct/status?token={}", enc(&token))).await?;
        if let Some(url) = s["status"]["url"].as_str().filter(|u| !u.is_empty()) {
            return Ok(url.to_string());
        }
        if s["status"]["retry"] == false {
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Err(err_tr!("builtbybit.noDownload", "BuiltByBit didn't have the download ready. Try again in a minute."))
}

/// A file name for a resource's jar.
pub fn file_name(title: &str) -> String {
    let base: String = title.chars().map(|c| if c.is_ascii_alphanumeric() || "-_.".contains(c) { c } else { '-' }).collect();
    let base = base.trim_matches('-');
    format!("{}.jar", if base.is_empty() { "plugin" } else { base })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_licenses() {
        assert!(license_live(&json!({ "permanent": true, "active": true })));
        assert!(!license_live(&json!({ "permanent": true, "active": false })));
        assert!(!license_live(&json!({ "permanent": false, "end_date": 1 })));
        assert!(license_live(&json!({ "permanent": false, "end_date": now() + 3600 })));
        let o = owned_from(&json!({
            "resource_id": 123, "title": "Ultra Cosmetics", "summary": "Hats", "url": "https://builtbybit.com/resources/123/",
            "downloads": 5, "LatestVersion": { "version_id": 9, "version_string": "2.4.1" }, "Creator": { "username": "Maker" }
        }))
        .unwrap();
        assert_eq!((o.id, o.version_id.as_str(), o.version.as_str(), o.creator.as_str()), (123, "9", "2.4.1", "Maker"));
        assert_eq!(file_name("Ultra Cosmetics (Premium)"), "Ultra-Cosmetics--Premium.jar");
    }

    #[test]
    fn reads_answers() {
        assert_eq!(data_of(json!({ "result": "success", "data": { "x": 1 } }), 200).unwrap()["x"], 1);
        assert!(data_of(json!({ "result": "error", "error": { "message": "nope" } }), 500).unwrap_err().to_string().contains("nope"));
    }
}
