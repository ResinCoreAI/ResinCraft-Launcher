//! Microsoft and offline accounts.
//! Microsoft sign-in: device-code flow → Xbox Live → XSTS → Minecraft services.
//! Interface used by `game::launch`: keep `LaunchAuth` and `get_launch_auth` signatures stable.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use anyhow::{bail, Result};
use base64::Engine;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::core::ctx::http;
use crate::core::events::emit;
use crate::core::fsutil::{now_ms, read_json, write_json};
use crate::core::net::{body_of, send_json, status_of, HttpError};
use crate::core::types::{AccountView, DeviceCode, LoginEvent};
use crate::core::{paths, settings};
use crate::{bail_tr, err_tr};

const MS_BASE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
const SCOPE: &str = "XboxLive.signin offline_access";

/// What the game needs to start as a player.
#[derive(Debug, Clone)]
pub struct LaunchAuth {
    pub username: String,
    /// Dashed or undashed UUID.
    pub uuid: String,
    pub access_token: String,
    /// "msa" or "legacy" (offline).
    pub user_type: &'static str,
    pub xuid: Option<String>,
}

/// One entry of accounts.json (same format since launcher 0.1).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredAccount {
    #[serde(default)]
    id: String,
    /// "msa" | "offline"
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    uuid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    skin_url: Option<String>,
    #[serde(default)]
    active: bool,
    /// Sealed: "dpapi:…" (Windows), "raw:…", or "enc:…" (launcher 0.1; unreadable here).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mc_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mc_token_expires: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    xuid: Option<String>,
    /// Anything else in the file is kept as is.
    #[serde(flatten)]
    extra: Map<String, Value>,
}

// ---------- Token sealing ----------

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// Raised when a token was sealed by launcher 0.1, which this version can't open.
#[derive(Debug)]
struct SignedOut;
impl std::fmt::Display for SignedOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("This sign-in was saved by an older version of ResinCraft")
    }
}
impl std::error::Error for SignedOut {}

#[cfg(windows)]
mod dpapi {
    use anyhow::{anyhow, Result};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};

    fn run(data: &[u8], protect: bool) -> Result<Vec<u8>> {
        let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
        let mut output = CRYPT_INTEGER_BLOB::default();
        // SAFETY: `input` borrows `data` for the call; `output` is allocated by Windows and freed with LocalFree.
        unsafe {
            let res = if protect {
                CryptProtectData(&input, PCWSTR::null(), None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut output)
            } else {
                CryptUnprotectData(&input, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut output)
            };
            res.map_err(|e| anyhow!("Couldn't {} the saved sign-in: {e}", if protect { "protect" } else { "read" }))?;
            let out = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            LocalFree(HLOCAL(output.pbData as *mut core::ffi::c_void));
            Ok(out)
        }
    }

    pub fn protect(data: &[u8]) -> Result<Vec<u8>> {
        run(data, true)
    }
    pub fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
        run(data, false)
    }
}

/// Encrypts a token for accounts.json: DPAPI (current Windows user) where available, else base64.
pub fn seal(s: &str) -> String {
    #[cfg(windows)]
    if let Ok(enc) = dpapi::protect(s.as_bytes()) {
        return format!("dpapi:{}", B64.encode(enc));
    }
    format!("raw:{}", B64.encode(s.as_bytes()))
}

/// Opens a sealed token. Tokens sealed by launcher 0.1 ("enc:") fail with a sign-in-again error.
pub fn unseal(s: Option<&str>) -> Result<String> {
    let Some(s) = s.filter(|s| !s.is_empty()) else { return Ok(String::new()) };
    if s.starts_with("enc:") {
        return Err(SignedOut.into());
    }
    if let Some(data) = s.strip_prefix("dpapi:") {
        #[cfg(windows)]
        {
            let bytes = dpapi::unprotect(&B64.decode(data)?)?;
            return Ok(String::from_utf8(bytes)?);
        }
        #[cfg(not(windows))]
        {
            let _ = data;
            return Err(SignedOut.into());
        }
    }
    let data = s.get(4..).unwrap_or_default();
    Ok(String::from_utf8_lossy(&B64.decode(data)?).into_owned())
}

fn is_unreadable(s: Option<&str>) -> bool {
    s.is_some_and(|s| s.starts_with("enc:") || (cfg!(not(windows)) && s.starts_with("dpapi:")))
}

fn sign_in_again(acc: &StoredAccount) -> anyhow::Error {
    err_tr!(
        "accounts.signInAgain",
        "Please sign in to {name} again: add your Microsoft account under Accounts (this replaces the old entry). \
         Sign-ins saved by the previous version of ResinCraft can't be read by this one.",
        name = acc.username
    )
}

// ---------- Storage ----------

async fn load() -> Vec<StoredAccount> {
    read_json(&paths::accounts_file()).await.unwrap_or_default()
}

async fn save(list: &[StoredAccount]) -> Result<()> {
    write_json(&paths::accounts_file(), list).await
}

fn view(a: &StoredAccount) -> AccountView {
    AccountView { id: a.id.clone(), kind: a.kind.clone(), username: a.username.clone(), uuid: a.uuid.clone(), skin_url: a.skin_url.clone(), active: a.active }
}

pub async fn list_accounts() -> Result<Vec<AccountView>> {
    Ok(load().await.iter().map(view).collect())
}

pub async fn set_active_account(id: &str) -> Result<()> {
    let mut list = load().await;
    for a in &mut list {
        a.active = a.id == id;
    }
    save(&list).await
}

pub async fn remove_account(id: &str) -> Result<()> {
    let mut list: Vec<StoredAccount> = load().await.into_iter().filter(|a| a.id != id).collect();
    if !list.is_empty() && !list.iter().any(|a| a.active) {
        list[0].active = true;
    }
    save(&list).await
}

async fn upsert(mut acc: StoredAccount) -> Result<StoredAccount> {
    let mut list: Vec<StoredAccount> = load().await.into_iter().filter(|a| a.uuid != acc.uuid || a.kind != acc.kind).collect();
    for a in &mut list {
        a.active = false;
    }
    acc.active = true;
    list.insert(0, acc.clone());
    save(&list).await?;
    Ok(acc)
}

// ---------- Microsoft / Xbox / Minecraft ----------

async fn client_id() -> Result<String> {
    let id = settings::ms_client_id().await;
    if id.is_empty() {
        bail_tr!("accounts.noClientId", "Microsoft sign-in isn't set up in this build of ResinCraft.");
    }
    Ok(id)
}

/// POSTs a form to the Microsoft identity endpoint. OAuth errors (400 with `error`) come back as JSON, not as Err.
async fn ms_form(path: &str, body: &[(&str, &str)]) -> Result<Value> {
    let form = body.iter().map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v))).collect::<Vec<_>>().join("&");
    let res = http()
        .post(format!("{MS_BASE}/{path}"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .body(form)
        .send()
        .await?;
    let status = res.status();
    let text = res.text().await?;
    let parsed: Option<Value> = serde_json::from_str(&text).ok();
    let has_error = parsed.as_ref().is_some_and(|v| v.get("error").is_some_and(|e| !e.is_null()));
    if !status.is_success() && !has_error {
        return Err(HttpError { status: status.as_u16(), url: path.to_string(), body: String::new() }.into());
    }
    parsed.ok_or_else(|| err_tr!("accounts.badResponse", "Unexpected response from Microsoft sign-in"))
}

fn s<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn xsts_error(code: &str) -> Option<anyhow::Error> {
    Some(match code {
        "2148916227" => err_tr!("accounts.xbox.banned", "This Xbox account is banned."),
        "2148916233" => err_tr!("accounts.xbox.noProfile", "This Microsoft account doesn't have an Xbox profile yet. Sign in once at xbox.com, then try again."),
        "2148916235" => err_tr!("accounts.xbox.country", "Xbox Live isn't available in this account's country."),
        "2148916236" | "2148916237" => err_tr!("accounts.xbox.adultCheck", "This account needs adult verification on xbox.com (South Korea)."),
        "2148916238" => err_tr!("accounts.xbox.child", "This is a child account. An adult needs to add it to a Microsoft Family group first."),
        _ => return None,
    })
}

#[derive(Deserialize)]
struct XuiClaim {
    uhs: String,
    #[serde(default)]
    xid: Option<String>,
}
#[derive(Deserialize)]
struct DisplayClaims {
    xui: Vec<XuiClaim>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XboxToken {
    token: String,
    display_claims: DisplayClaims,
}
#[derive(Deserialize)]
struct McLogin {
    access_token: String,
    expires_in: i64,
}
#[derive(Deserialize)]
struct Skin {
    url: String,
    state: String,
}
#[derive(Deserialize)]
struct Profile {
    id: String,
    name: String,
    #[serde(default)]
    skins: Vec<Skin>,
}

struct McLoginResult {
    username: String,
    uuid: String,
    skin_url: Option<String>,
    mc_token: String,
    mc_token_expires: i64,
    xuid: Option<String>,
}

async fn minecraft_login(ms_access_token: &str) -> Result<McLoginResult> {
    let xbl: XboxToken = send_json(http().post("https://user.auth.xboxlive.com/user/authenticate").json(&json!({
        "Properties": { "AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={ms_access_token}") },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    })))
    .await?;

    let xsts: XboxToken = match send_json(http().post("https://xsts.auth.xboxlive.com/xsts/authorize").json(&json!({
        "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl.token] },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT"
    })))
    .await
    {
        Ok(x) => x,
        Err(e) => {
            static XERR: OnceLock<Regex> = OnceLock::new();
            let re = XERR.get_or_init(|| Regex::new(r#""XErr"\s*:\s*(\d+)"#).unwrap());
            let code = body_of(&e).and_then(|b| re.captures(b)).map(|c| c[1].to_string());
            match code.as_deref().and_then(xsts_error) {
                Some(err) => return Err(err),
                None => bail_tr!("accounts.xbox.failed", "Xbox sign-in failed: {error}", error = e.to_string()),
            }
        }
    };
    let claim = xsts.display_claims.xui.first().ok_or_else(|| err_tr!("accounts.xbox.failed", "Xbox sign-in failed: {error}", error = "no user claims"))?;

    let mc: McLogin = match send_json(
        http().post("https://api.minecraftservices.com/authentication/login_with_xbox").json(&json!({ "identityToken": format!("XBL3.0 x={};{}", claim.uhs, xsts.token) })),
    )
    .await
    {
        Ok(m) => m,
        Err(e) if status_of(&e) == Some(403) => {
            bail_tr!(
                "accounts.clientRejected",
                "Minecraft rejected this app's Client ID. New Azure apps must be approved by Mojang before they can sign in to Minecraft."
            )
        }
        Err(e) => return Err(e),
    };

    let profile: Profile = match send_json(http().get("https://api.minecraftservices.com/minecraft/profile").bearer_auth(&mc.access_token)).await {
        Ok(p) => p,
        Err(e) if status_of(&e) == Some(404) => bail_tr!("accounts.noMinecraft", "This Microsoft account doesn't own Minecraft: Java Edition."),
        Err(e) => return Err(e),
    };

    Ok(McLoginResult {
        username: profile.name,
        uuid: profile.id,
        skin_url: profile.skins.into_iter().find(|s| s.state == "ACTIVE").map(|s| https_texture(&s.url)),
        mc_token: mc.access_token,
        mc_token_expires: now_ms() + mc.expires_in * 1000,
        xuid: claim.xid.clone(),
    })
}

/// The running device-code poller's cancel flag.
static LOGIN_ABORT: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

/// Starts the device-code flow; the result arrives later as a `login` event.
pub async fn begin_microsoft_login() -> Result<DeviceCode> {
    let abort = Arc::new(AtomicBool::new(false));
    if let Some(old) = LOGIN_ABORT.lock().unwrap().replace(abort.clone()) {
        old.store(true, Ordering::SeqCst);
    }
    let client = client_id().await?;
    let dc = ms_form("devicecode", &[("client_id", &client), ("scope", SCOPE)]).await?;
    let Some(device_code) = s(&dc, "device_code").map(String::from) else {
        match s(&dc, "error_description") {
            Some(desc) => bail!("{desc}"),
            None => bail_tr!("accounts.loginNoStart", "Microsoft sign-in could not start"),
        }
    };
    let expires_in = dc.get("expires_in").and_then(Value::as_u64).unwrap_or(0);
    let interval = dc.get("interval").and_then(Value::as_u64).filter(|i| *i > 0).unwrap_or(5);

    let poll_abort = abort.clone();
    tokio::spawn(async move {
        let abort = poll_abort;
        let result = poll_device_code(&client, &device_code, interval, expires_in, &abort).await;
        if abort.load(Ordering::SeqCst) {
            return;
        }
        match result {
            Ok(Some(acc)) => emit("login", LoginEvent { state: "success", account: Some(view(&acc)), error: None, error_text: None }),
            Ok(None) => {}
            Err(e) => {
                let error_text = crate::core::text::text_of(&e).cloned();
                emit("login", LoginEvent { state: "error", account: None, error: Some(e.to_string()), error_text })
            }
        }
    });

    Ok(DeviceCode {
        user_code: s(&dc, "user_code").unwrap_or_default().to_string(),
        verification_uri: s(&dc, "verification_uri").unwrap_or_default().to_string(),
        expires_in,
    })
}

/// Polls the token endpoint until the user finishes in the browser. Ok(None) means cancelled.
async fn poll_device_code(client: &str, device_code: &str, interval_s: u64, expires_in: u64, abort: &AtomicBool) -> Result<Option<StoredAccount>> {
    let mut interval = Duration::from_secs(interval_s);
    let deadline = std::time::Instant::now() + Duration::from_secs(expires_in);
    while std::time::Instant::now() < deadline {
        tokio::time::sleep(interval).await;
        if abort.load(Ordering::SeqCst) {
            return Ok(None);
        }
        let tok = ms_form(
            "token",
            &[("grant_type", "urn:ietf:params:oauth:grant-type:device_code"), ("client_id", client), ("device_code", device_code)],
        )
        .await?;
        match s(&tok, "error") {
            Some("authorization_pending") => continue,
            Some("slow_down") => {
                interval += Duration::from_secs(5);
                continue;
            }
            Some("authorization_declined") => bail_tr!("accounts.loginCancelled", "Sign-in was cancelled in the browser."),
            _ => {}
        }
        let access = s(&tok, "access_token").filter(|t| !t.is_empty());
        let Some(access) = access.filter(|_| tok.get("error").map_or(true, Value::is_null)) else {
            match s(&tok, "error_description") {
                Some(desc) => bail!("{desc}"),
                None => bail_tr!("accounts.loginFailed", "Microsoft sign-in failed"),
            }
        };
        let mc = minecraft_login(access).await?;
        if abort.load(Ordering::SeqCst) {
            return Ok(None);
        }
        let acc = upsert(StoredAccount {
            id: uuid::Uuid::new_v4().to_string(),
            kind: "msa".into(),
            username: mc.username,
            uuid: mc.uuid,
            skin_url: mc.skin_url,
            active: true,
            refresh_token: Some(seal(s(&tok, "refresh_token").unwrap_or_default())),
            mc_token: Some(seal(&mc.mc_token)),
            mc_token_expires: Some(mc.mc_token_expires),
            xuid: mc.xuid,
            extra: Map::new(),
        })
        .await?;
        return Ok(Some(acc));
    }
    bail_tr!("accounts.codeExpired", "The sign-in code expired. Please try again.")
}

pub fn cancel_microsoft_login() {
    if let Some(abort) = LOGIN_ABORT.lock().unwrap().as_ref() {
        abort.store(true, Ordering::SeqCst);
    }
}

async fn refresh(acc: &StoredAccount) -> Result<StoredAccount> {
    let refresh_token = unseal(acc.refresh_token.as_deref()).map_err(|_| sign_in_again(acc))?;
    let client = client_id().await?;
    let tok = ms_form("token", &[("grant_type", "refresh_token"), ("client_id", &client), ("refresh_token", &refresh_token), ("scope", SCOPE)]).await?;
    let Some(access) = s(&tok, "access_token").filter(|t| !t.is_empty()) else {
        bail_tr!(
            "accounts.sessionExpired",
            "Your Microsoft session expired. Remove and re-add {name}. ({reason})",
            name = acc.username,
            reason = s(&tok, "error_description").unwrap_or("no token")
        );
    };
    let mc = minecraft_login(access).await?;
    let updated = StoredAccount {
        username: mc.username,
        uuid: mc.uuid,
        skin_url: mc.skin_url,
        mc_token: Some(seal(&mc.mc_token)),
        mc_token_expires: Some(mc.mc_token_expires),
        xuid: mc.xuid,
        refresh_token: match s(&tok, "refresh_token").filter(|t| !t.is_empty()) {
            Some(t) => Some(seal(t)),
            None => acc.refresh_token.clone(),
        },
        ..acc.clone()
    };
    let list: Vec<StoredAccount> = load().await.into_iter().map(|a| if a.id == acc.id { updated.clone() } else { a }).collect();
    save(&list).await?;
    Ok(updated)
}

// ---------- Offline ----------

/// Same scheme Java's UUID.nameUUIDFromBytes uses, so servers in offline mode agree with us.
pub fn offline_uuid(name: &str) -> String {
    use md5::{Digest, Md5};
    let mut b: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    b[6] = (b[6] & 0x0f) | 0x30;
    b[8] = (b[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(b).hyphenated().to_string()
}

pub async fn add_offline_account(username: &str) -> Result<AccountView> {
    let name = username.trim();
    static NAME: OnceLock<Regex> = OnceLock::new();
    if !NAME.get_or_init(|| Regex::new("^[A-Za-z0-9_]{3,16}$").unwrap()).is_match(name) {
        bail_tr!("accounts.badUsername", "Usernames are 3–16 characters: letters, numbers and _");
    }
    // Offline profiles don't need a Microsoft account first (the owner's choice).
    let acc = upsert(StoredAccount {
        id: uuid::Uuid::new_v4().to_string(),
        kind: "offline".into(),
        username: name.to_string(),
        uuid: offline_uuid(name),
        active: true,
        ..Default::default()
    })
    .await?;
    Ok(view(&acc))
}

// ---------- Launch ----------

/// The active account's credentials, refreshing the Minecraft token if it's about to expire.
pub async fn get_launch_auth() -> Result<LaunchAuth> {
    let list = load().await;
    let Some(acc) = list.iter().find(|a| a.active).or_else(|| list.first()).cloned() else {
        bail_tr!("accounts.signInFirst", "Sign in with your Microsoft account before playing.");
    };
    if acc.kind == "offline" {
        return Ok(LaunchAuth { username: acc.username, uuid: acc.uuid, access_token: "0".into(), user_type: "legacy", xuid: None });
    }
    let (acc, access_token) = fresh_token(acc, false).await?;
    Ok(LaunchAuth { username: acc.username, uuid: acc.uuid, access_token, user_type: "msa", xuid: acc.xuid })
}

/// Opens a Microsoft account's Minecraft token, refreshing it first when it's about to expire (or when `force`d).
async fn fresh_token(mut acc: StoredAccount, force: bool) -> Result<(StoredAccount, String)> {
    // Tokens sealed by launcher 0.1 can't be opened here: the account is effectively signed out.
    if is_unreadable(acc.mc_token.as_deref()) || is_unreadable(acc.refresh_token.as_deref()) {
        return Err(sign_in_again(&acc));
    }
    if force || acc.mc_token_expires.map_or(true, |exp| exp - now_ms() < 5 * 60_000) {
        acc = refresh(&acc).await?;
    }
    let token = unseal(acc.mc_token.as_deref()).map_err(|_| sign_in_again(&acc))?;
    Ok((acc, token))
}

// ---------- ResinCraft Relay (see `server::relay`) ----------

/// A signed-in Microsoft account, for signing in to ResinCraft Relay.
#[derive(Debug, Clone)]
pub struct MicrosoftAccount {
    pub id: String,
    pub uuid: String,
    pub username: String,
}

/// The Microsoft account the relay uses: `preferred` (a UUID, the account a server's name belongs to) while
/// it's still signed in, else the active account if it's a Microsoft one, else the first Microsoft account.
pub async fn microsoft_account(preferred: Option<&str>) -> Option<MicrosoftAccount> {
    let list: Vec<StoredAccount> = load().await.into_iter().filter(|a| a.kind == "msa" && !is_unreadable(a.refresh_token.as_deref())).collect();
    let same = |a: &&StoredAccount, uuid: &str| a.uuid.replace('-', "").eq_ignore_ascii_case(&uuid.replace('-', ""));
    let acc = preferred
        .and_then(|p| list.iter().find(|a| same(a, p)))
        .or_else(|| list.iter().find(|a| a.active))
        .or_else(|| list.first())?;
    Some(MicrosoftAccount { id: acc.id.clone(), uuid: acc.uuid.clone(), username: acc.username.clone() })
}

/// A fresh Minecraft services token for a Microsoft account (`force`: refresh even if it looks valid).
pub async fn microsoft_token(id: &str, force: bool) -> Result<String> {
    minecraft_token(id, force).await
}

// ---------- Profile changes (see `player::profile`) ----------

/// A Minecraft services token for one Microsoft account. `force` refreshes even if the saved one looks valid
/// (used after a 401).
pub(super) async fn minecraft_token(id: &str, force: bool) -> Result<String> {
    let Some(acc) = load().await.into_iter().find(|a| a.id == id) else {
        bail_tr!("accounts.gone", "This account isn't signed in any more. It may have been removed.");
    };
    if acc.kind != "msa" {
        bail_tr!("accounts.offlineNoProfile", "Offline profiles can't change skins, capes or names. Sign in with Microsoft to do that.");
    }
    Ok(fresh_token(acc, force).await?.1)
}

/// Saves a changed name or skin into accounts.json so the sidebar avatar and the next launch use it.
pub(super) async fn sync_profile(id: &str, username: &str, skin_url: Option<String>) -> Result<()> {
    let mut list = load().await;
    let Some(acc) = list.iter_mut().find(|a| a.id == id) else { return Ok(()) };
    if acc.username == username && acc.skin_url == skin_url {
        return Ok(());
    }
    acc.username = username.to_string();
    acc.skin_url = skin_url;
    save(&list).await
}

/// Mojang hands out texture links as plain http; the window's content policy only loads https images.
pub(super) fn https_texture(url: &str) -> String {
    match url.strip_prefix("http://") {
        Some(rest) if rest.starts_with("textures.minecraft.net/") => format!("https://{rest}"),
        _ => url.to_string(),
    }
}
