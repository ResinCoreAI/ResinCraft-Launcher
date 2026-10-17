//! Skin, cape and name changes for Microsoft accounts, through Minecraft services.
//! Requests are built as plain data first (`Req`) so they can be checked in tests without a network.

use std::sync::OnceLock;

use anyhow::{anyhow, bail, Result};
use base64::Engine;
use regex::Regex;
use reqwest::Method;
use serde::Deserialize;
use serde_json::{json, Value};

use super::accounts::{https_texture, minecraft_token, sync_profile};
use crate::core::ctx::http;
use crate::core::net::{status_of, HttpError};
use crate::core::types::{CapeInfo, NameChange, PlayerProfile, SkinInfo, SkinUpload};

const API: &str = "https://api.minecraftservices.com/minecraft/profile";
/// Real skins are a few KB; anything this big isn't one.
const MAX_PNG: usize = 512 * 1024;

/// Which kind of change a request makes, so errors can be explained in its terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Profile,
    Skin,
    Cape,
    Name,
}

#[derive(Debug, Clone, PartialEq)]
enum Body {
    Empty,
    Json(Value),
    Multipart { boundary: String, bytes: Vec<u8> },
}

/// One Minecraft services request, before the token is attached.
#[derive(Debug, Clone)]
struct Req {
    op: Op,
    method: Method,
    url: String,
    body: Body,
}

impl Req {
    fn new(op: Op, method: Method, url: impl Into<String>) -> Self {
        Req { op, method, url: url.into(), body: Body::Empty }
    }
    fn json(mut self, v: Value) -> Self {
        self.body = Body::Json(v);
        self
    }
}

// ---------- Sending ----------

/// Sends a request with the account's token, refreshing the token once if Minecraft says it's no longer valid.
/// Returns the response JSON (`null` for an empty body).
async fn send(account_id: &str, req: &Req) -> Result<Value> {
    let token = minecraft_token(account_id, false).await?;
    match send_once(&token, req).await {
        Err(e) if status_of(&e) == Some(401) => {
            let token = minecraft_token(account_id, true).await?;
            send_once(&token, req).await
        }
        r => r,
    }
    .map_err(|e| match e.downcast_ref::<HttpError>() {
        Some(h) => anyhow!(friendly_error(req.op, h.status, &h.body)),
        None => e,
    })
}

async fn send_once(token: &str, req: &Req) -> Result<Value> {
    let mut b = http().request(req.method.clone(), &req.url).bearer_auth(token).header("Accept", "application/json");
    b = match &req.body {
        Body::Empty => b,
        Body::Json(v) => b.json(v),
        Body::Multipart { boundary, bytes } => b.header("Content-Type", format!("multipart/form-data; boundary={boundary}")).body(bytes.clone()),
    };
    let res = b.send().await.map_err(|_| anyhow!("Couldn't reach Minecraft services. Check your internet connection, then try again."))?;
    let status = res.status();
    let text = res.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(HttpError { status: status.as_u16(), url: req.url.clone(), body: text }.into());
    }
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&text).map_err(|_| anyhow!("Minecraft services sent a response ResinCraft couldn't read. Try again in a moment."))
}

/// Turns a Minecraft services error into something a player can act on.
fn friendly_error(op: Op, status: u16, body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let detail = v.pointer("/details/status").and_then(Value::as_str);
    let message = v.get("errorMessage").and_then(Value::as_str).map(str::trim).filter(|m| !m.is_empty());
    match (op, status) {
        (_, 401) => "Your Minecraft sign-in has expired. Remove this account and sign in again.".into(),
        (_, 429) => "Minecraft is limiting requests from this account right now. Wait a minute, then try again.".into(),
        (_, s) if s >= 500 => "Minecraft services aren't responding right now. Try again in a few minutes.".into(),
        (Op::Name, 400) => "That name isn't valid. Names are 3–16 characters: letters, numbers and _.".into(),
        (Op::Name, 403 | 409) => match detail {
            Some("DUPLICATE") => "That name is already taken. Try a different one.".into(),
            Some("NOT_ALLOWED") => "Mojang doesn't allow that name. Try a different one.".into(),
            _ => "You can't change your name right now. Minecraft allows one change every 30 days.".into(),
        },
        (Op::Skin, 400) => match message {
            Some(m) => format!("Minecraft didn't accept this skin: {m}"),
            None => "Minecraft didn't accept this skin. Use a 64×64 or 64×32 PNG, or a direct link to one.".into(),
        },
        (Op::Cape, 400 | 404) => "This account doesn't own that cape.".into(),
        (_, 404) => "This Microsoft account doesn't own Minecraft: Java Edition.".into(),
        (_, 403) => "Minecraft refused this change for this account.".into(),
        _ => format!("Minecraft services returned an error (HTTP {status}). Try again later."),
    }
}

// ---------- Parsing ----------

#[derive(Debug, Deserialize)]
struct RawTexture {
    #[serde(default)]
    id: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    variant: Option<String>,
    #[serde(default)]
    alias: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawProfile {
    id: String,
    name: String,
    #[serde(default)]
    skins: Vec<RawTexture>,
    #[serde(default)]
    capes: Vec<RawTexture>,
}

fn parse_profile(v: Value, name_change: Option<NameChange>) -> Result<PlayerProfile> {
    let raw: RawProfile = serde_json::from_value(v).map_err(|_| anyhow!("Minecraft services sent a profile ResinCraft couldn't read."))?;
    let skin = raw.skins.iter().find(|s| s.state == "ACTIVE" && !s.url.is_empty()).map(|s| SkinInfo {
        url: https_texture(&s.url),
        variant: if s.variant.as_deref().is_some_and(|v| v.eq_ignore_ascii_case("slim")) { "slim" } else { "classic" }.into(),
    });
    let capes = raw
        .capes
        .into_iter()
        .filter(|c| !c.id.is_empty())
        .map(|c| CapeInfo { alias: c.alias.unwrap_or_else(|| "Cape".into()), url: https_texture(&c.url), active: c.state == "ACTIVE", id: c.id })
        .collect();
    Ok(PlayerProfile { id: raw.id, name: raw.name, skin, capes, name_change })
}

fn parse_name_change(v: &Value) -> Option<NameChange> {
    let allowed = v.get("nameChangeAllowed")?.as_bool()?;
    Some(NameChange { allowed, changed_at: v.get("changedAt").and_then(Value::as_str).map(String::from) })
}

/// "available" | "taken" | "not_allowed"
fn parse_name_status(v: &Value) -> Result<&'static str> {
    Ok(match v.get("status").and_then(Value::as_str) {
        Some("AVAILABLE") => "available",
        Some("DUPLICATE") => "taken",
        Some("NOT_ALLOWED") => "not_allowed",
        _ => bail!("Couldn't check that name right now. Try again in a moment."),
    })
}

// ---------- Request building ----------

fn valid_name(name: &str) -> bool {
    static NAME: OnceLock<Regex> = OnceLock::new();
    NAME.get_or_init(|| Regex::new("^[A-Za-z0-9_]{3,16}$").unwrap()).is_match(name)
}

fn check_name(name: &str) -> Result<()> {
    if !valid_name(name) {
        bail!("Names are 3–16 characters: letters, numbers and _.");
    }
    Ok(())
}

/// Width and height from a PNG's header, without decoding the image.
fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || &bytes[..8] != SIG || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((w, h))
}

/// Decodes and checks an uploaded skin: base64 (or a data: URL) of a 64×64 or 64×32 PNG.
fn decode_skin_png(data: &str) -> Result<Vec<u8>> {
    let b64 = data.split_once(";base64,").map_or(data, |(_, rest)| rest).trim();
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).map_err(|_| anyhow!("That file couldn't be read as an image."))?;
    if bytes.len() > MAX_PNG {
        bail!("That file is too big to be a skin. Skins are small 64×64 PNG images.");
    }
    match png_size(&bytes) {
        Some((64, 64 | 32)) => Ok(bytes),
        Some((w, h)) => bail!("Skins must be 64×64 or 64×32 pixels. This image is {w}×{h}."),
        None => bail!("Skins must be PNG images."),
    }
}

fn check_variant(v: &str) -> Result<&'static str> {
    match v {
        "classic" => Ok("classic"),
        "slim" => Ok("slim"),
        _ => bail!("Choose Classic or Slim arms."),
    }
}

fn multipart_skin(boundary: &str, variant: &str, png: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(png.len() + 400);
    out.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"variant\"\r\n\r\n{variant}\r\n").as_bytes());
    out.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"skin.png\"\r\nContent-Type: image/png\r\n\r\n").as_bytes());
    out.extend_from_slice(png);
    out.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    out
}

fn skin_request(upload: &SkinUpload, boundary: &str) -> Result<Req> {
    let variant = check_variant(&upload.variant)?;
    let req = Req::new(Op::Skin, Method::POST, format!("{API}/skins"));
    if let Some(png) = upload.png.as_deref().filter(|p| !p.is_empty()) {
        let bytes = multipart_skin(boundary, variant, &decode_skin_png(png)?);
        return Ok(Req { body: Body::Multipart { boundary: boundary.to_string(), bytes }, ..req });
    }
    let url = upload.url.as_deref().map(str::trim).unwrap_or_default();
    if url.is_empty() {
        bail!("Choose a skin file or paste a link to one.");
    }
    if !(url.starts_with("https://") || url.starts_with("http://")) || url.contains(char::is_whitespace) {
        bail!("That doesn't look like a web link. Paste a direct link to a skin PNG (https://…).");
    }
    Ok(req.json(json!({ "variant": variant, "url": url })))
}

fn cape_request(cape_id: Option<&str>) -> Req {
    match cape_id {
        Some(id) => Req::new(Op::Cape, Method::PUT, format!("{API}/capes/active")).json(json!({ "capeId": id })),
        None => Req::new(Op::Cape, Method::DELETE, format!("{API}/capes/active")),
    }
}

fn name_url(name: &str) -> String {
    format!("{API}/name/{}", urlencoding::encode(name))
}

// ---------- Public API (IPC) ----------

/// Saves the new name/skin locally and hands the profile to the UI.
async fn finish(account_id: &str, profile: PlayerProfile) -> Result<PlayerProfile> {
    sync_profile(account_id, &profile.name, profile.skin.as_ref().map(|s| s.url.clone())).await?;
    Ok(profile)
}

/// Uses the profile a change returned, or fetches it when the response didn't include one.
async fn after_change(account_id: &str, response: Value) -> Result<PlayerProfile> {
    let response = if response.get("id").is_some() { response } else { send(account_id, &Req::new(Op::Profile, Method::GET, API)).await? };
    finish(account_id, parse_profile(response, None)?).await
}

async fn name_change(account_id: &str) -> Option<NameChange> {
    let v = send(account_id, &Req::new(Op::Profile, Method::GET, format!("{API}/namechange"))).await.ok()?;
    parse_name_change(&v)
}

pub async fn get_profile(account_id: &str) -> Result<PlayerProfile> {
    let v = send(account_id, &Req::new(Op::Profile, Method::GET, API)).await?;
    let nc = name_change(account_id).await;
    finish(account_id, parse_profile(v, nc)?).await
}

pub async fn set_skin(account_id: &str, upload: &SkinUpload) -> Result<PlayerProfile> {
    let boundary = format!("ResinCraft{}", uuid::Uuid::new_v4().simple());
    let req = skin_request(upload, &boundary)?;
    after_change(account_id, send(account_id, &req).await?).await
}

pub async fn reset_skin(account_id: &str) -> Result<PlayerProfile> {
    let req = Req::new(Op::Skin, Method::DELETE, format!("{API}/skins/active"));
    after_change(account_id, send(account_id, &req).await?).await
}

pub async fn set_cape(account_id: &str, cape_id: &str) -> Result<PlayerProfile> {
    if cape_id.trim().is_empty() {
        bail!("Choose a cape first.");
    }
    after_change(account_id, send(account_id, &cape_request(Some(cape_id))).await?).await
}

pub async fn hide_cape(account_id: &str) -> Result<PlayerProfile> {
    after_change(account_id, send(account_id, &cape_request(None)).await?).await
}

/// "available" | "taken" | "not_allowed" | "invalid" (checked locally, no request).
pub async fn name_available(account_id: &str, name: &str) -> Result<&'static str> {
    let name = name.trim();
    if !valid_name(name) {
        return Ok("invalid");
    }
    let v = send(account_id, &Req::new(Op::Name, Method::GET, format!("{}/available", name_url(name)))).await?;
    parse_name_status(&v)
}

pub async fn change_name(account_id: &str, name: &str) -> Result<PlayerProfile> {
    let name = name.trim();
    check_name(name)?;
    // Mojang answers a name change during the cooldown with a vague 403, so check first and say why.
    if let Some(NameChange { allowed: false, changed_at }) = name_change(account_id).await {
        match changed_at.as_deref().and_then(|d| d.get(..10)) {
            Some(day) => bail!("You can't change your name yet. Minecraft allows one change every 30 days, and you last changed it on {day}."),
            None => bail!("You can't change your name yet. Minecraft allows one change every 30 days."),
        }
    }
    let v = send(account_id, &Req::new(Op::Name, Method::PUT, name_url(name))).await?;
    let mut profile = after_change(account_id, v).await?;
    profile.name_change = name_change(account_id).await;
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::new(w, h);
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    #[test]
    fn reads_png_size() {
        assert_eq!(png_size(&png(64, 32)), Some((64, 32)));
        assert_eq!(png_size(b"not a png at all, just some text"), None);
    }

    #[test]
    fn accepts_only_skin_sized_pngs() {
        assert!(decode_skin_png(&b64(&png(64, 64))).is_ok());
        assert!(decode_skin_png(&format!("data:image/png;base64,{}", b64(&png(64, 32)))).is_ok());
        let err = decode_skin_png(&b64(&png(128, 128))).unwrap_err().to_string();
        assert!(err.contains("128×128"), "{err}");
        assert!(decode_skin_png(&b64(b"GIF89a....")).unwrap_err().to_string().contains("PNG"));
        assert!(decode_skin_png("%%%").is_err());
    }

    #[test]
    fn builds_multipart_upload() {
        let data = png(64, 64);
        let upload = SkinUpload { variant: "slim".into(), png: Some(b64(&data)), url: None };
        let req = skin_request(&upload, "XYZ").unwrap();
        assert_eq!(req.method, Method::POST);
        assert_eq!(req.url, "https://api.minecraftservices.com/minecraft/profile/skins");
        let Body::Multipart { boundary, bytes } = req.body else { panic!("expected multipart") };
        assert_eq!(boundary, "XYZ");
        let head = String::from_utf8_lossy(&bytes[..bytes.len().min(220)]).to_string();
        assert!(head.starts_with("--XYZ\r\nContent-Disposition: form-data; name=\"variant\"\r\n\r\nslim\r\n--XYZ\r\n"), "{head}");
        assert!(head.contains("name=\"file\"; filename=\"skin.png\"\r\nContent-Type: image/png\r\n\r\n"));
        assert!(bytes.windows(data.len()).any(|w| w == data.as_slice()), "PNG bytes are in the body");
        assert!(bytes.ends_with(b"\r\n--XYZ--\r\n"));
    }

    #[test]
    fn builds_url_upload_and_rejects_bad_input() {
        let req = skin_request(&SkinUpload { variant: "classic".into(), png: None, url: Some(" https://example.com/s.png ".into()) }, "b").unwrap();
        assert_eq!(req.body, Body::Json(json!({ "variant": "classic", "url": "https://example.com/s.png" })));
        assert!(skin_request(&SkinUpload { variant: "wide".into(), png: None, url: Some("https://x".into()) }, "b").is_err());
        assert!(skin_request(&SkinUpload { variant: "slim".into(), png: None, url: Some("not a link".into()) }, "b").is_err());
        assert!(skin_request(&SkinUpload { variant: "slim".into(), png: None, url: None }, "b").is_err());
    }

    #[test]
    fn builds_cape_requests() {
        let put = cape_request(Some("abc-123"));
        assert_eq!((put.method, put.url.as_str()), (Method::PUT, "https://api.minecraftservices.com/minecraft/profile/capes/active"));
        assert_eq!(put.body, Body::Json(json!({ "capeId": "abc-123" })));
        let del = cape_request(None);
        assert_eq!((del.method, del.body), (Method::DELETE, Body::Empty));
    }

    #[test]
    fn validates_names() {
        assert!(valid_name("Puirkle_01"));
        assert!(!valid_name("ab"));
        assert!(!valid_name("seventeen_chars_x"));
        assert!(!valid_name("no spaces"));
        assert!(!valid_name("../profile"));
        assert_eq!(name_url("Steve"), "https://api.minecraftservices.com/minecraft/profile/name/Steve");
    }

    #[test]
    fn parses_profile() {
        let v = json!({
            "id": "069a79f444e94726a5befca90e38aaf5",
            "name": "Notch",
            "skins": [
                { "id": "1", "state": "INACTIVE", "url": "http://textures.minecraft.net/texture/old", "variant": "CLASSIC" },
                { "id": "2", "state": "ACTIVE", "url": "http://textures.minecraft.net/texture/new", "variant": "SLIM" }
            ],
            "capes": [
                { "id": "c1", "state": "INACTIVE", "url": "http://textures.minecraft.net/texture/c1", "alias": "Migrator" },
                { "id": "c2", "state": "ACTIVE", "url": "http://textures.minecraft.net/texture/c2", "alias": "Vanilla" }
            ]
        });
        let p = parse_profile(v, Some(NameChange { allowed: true, changed_at: None })).unwrap();
        let skin = p.skin.unwrap();
        assert_eq!(skin.url, "https://textures.minecraft.net/texture/new");
        assert_eq!(skin.variant, "slim");
        assert_eq!(p.capes.len(), 2);
        assert!(!p.capes[0].active && p.capes[1].active);
        assert_eq!(p.capes[0].alias, "Migrator");
        assert!(p.name_change.unwrap().allowed);

        let bare = parse_profile(json!({ "id": "x", "name": "Alex" }), None).unwrap();
        assert!(bare.skin.is_none() && bare.capes.is_empty());
        assert!(parse_profile(json!({ "error": "nope" }), None).is_err());
    }

    #[test]
    fn parses_name_answers() {
        assert_eq!(parse_name_status(&json!({ "status": "AVAILABLE" })).unwrap(), "available");
        assert_eq!(parse_name_status(&json!({ "status": "DUPLICATE" })).unwrap(), "taken");
        assert_eq!(parse_name_status(&json!({ "status": "NOT_ALLOWED" })).unwrap(), "not_allowed");
        assert!(parse_name_status(&json!({})).is_err());
        let nc = parse_name_change(&json!({ "changedAt": "2026-10-01T10:00:00Z", "nameChangeAllowed": false })).unwrap();
        assert!(!nc.allowed);
        assert_eq!(nc.changed_at.as_deref(), Some("2026-10-01T10:00:00Z"));
        assert!(parse_name_change(&json!({})).is_none());
    }

    #[test]
    fn explains_errors() {
        let dup = r#"{"path":"/minecraft/profile/name/Notch","errorType":"FORBIDDEN","details":{"status":"DUPLICATE"}}"#;
        assert!(friendly_error(Op::Name, 403, dup).contains("already taken"));
        assert!(friendly_error(Op::Name, 403, r#"{"details":{"status":"NOT_ALLOWED"}}"#).contains("doesn't allow"));
        assert!(friendly_error(Op::Name, 403, "").contains("30 days"));
        assert!(friendly_error(Op::Name, 400, "").contains("3–16"));
        assert!(friendly_error(Op::Skin, 400, r#"{"errorMessage":"Could not validate image data."}"#).ends_with("Could not validate image data."));
        assert!(friendly_error(Op::Skin, 400, "<html>").contains("64×64"));
        assert!(friendly_error(Op::Cape, 400, "").contains("doesn't own that cape"));
        assert!(friendly_error(Op::Profile, 404, "").contains("doesn't own Minecraft"));
        assert!(friendly_error(Op::Skin, 401, "").contains("sign in again"));
        assert!(friendly_error(Op::Skin, 429, "").contains("Wait a minute"));
        assert!(friendly_error(Op::Cape, 503, "").contains("aren't responding"));
    }
}
