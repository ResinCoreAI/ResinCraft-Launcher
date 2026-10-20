//! The skin library: skins the player uploaded, imported from a player name or found online, plus the account's
//! previous skins. It belongs to the launcher, not an account (`<data>/skins/`), so any Microsoft account can wear
//! any saved skin. Online search uses MineSkin's public gallery API (its terms allow automated API use from apps
//! that send a User-Agent and respect the rate limits); player names are looked up through Mojang's public API.

use std::time::Duration;

use anyhow::Result;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use super::accounts::https_texture;
use super::profile::{self, decode_skin_png};
use crate::core::ctx::http;
use crate::core::fsutil::{now_ms, read_json, write_json};
use crate::core::net::{hash_bytes, HashAlgo};
use crate::core::paths;
use crate::core::settings;
use crate::core::types::{OnlineSkin, OnlineSkinPage, PlayerProfile, SavedSkin, SkinUpload};
use crate::{bail_tr, err_tr};

const TEXTURES: &str = "https://textures.minecraft.net/texture/";
const MINESKIN: &str = "https://api.mineskin.org/v2/skins";
const PAGE_SIZE: usize = 24;
const NAME_MAX: usize = 40;
const TIMEOUT: Duration = Duration::from_secs(20);

/// One library entry as stored in skins/library.json (the PNG sits next to it as `<id>.png`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Entry {
    id: String,
    name: String,
    /// "classic" | "slim"
    variant: String,
    /// "upload" | "player" | "online" | "history"
    source: String,
    /// The player name or MineSkin id it came from, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    origin: Option<String>,
    added_at: i64,
    /// SHA-1 of the PNG, so the same skin is never saved twice.
    hash: String,
}

/// Library reads and writes go one at a time so two quick clicks can't lose an entry.
static LOCK: Mutex<()> = Mutex::const_new(());

fn dir() -> std::path::PathBuf {
    paths::root().join("skins")
}

async fn load() -> Vec<Entry> {
    read_json(&dir().join("library.json")).await.unwrap_or_default()
}

async fn save(list: &[Entry]) -> Result<()> {
    write_json(&dir().join("library.json"), list).await
}

fn png_path(id: &str) -> std::path::PathBuf {
    dir().join(format!("{id}.png"))
}

fn data_url(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))
}

async fn view(e: Entry) -> Option<SavedSkin> {
    let png = tokio::fs::read(png_path(&e.id)).await.ok()?;
    Some(SavedSkin { id: e.id, name: e.name, variant: e.variant, source: e.source, origin: e.origin, added_at: e.added_at, preview: data_url(&png) })
}

fn clean_name(name: &str) -> String {
    let name: String = name.trim().chars().filter(|c| !c.is_control()).take(NAME_MAX).collect();
    if name.is_empty() {
        "Skin".into()
    } else {
        name
    }
}

fn check_variant(v: &str) -> Result<String> {
    match v {
        "classic" | "slim" => Ok(v.to_string()),
        _ => bail_tr!("profile.skin.badVariant", "Choose Classic or Slim arms."),
    }
}

/// Saves a checked PNG into the library, or returns the entry that already has the same image.
async fn add(png: &[u8], name: &str, variant: &str, source: &str, origin: Option<String>) -> Result<SavedSkin> {
    let variant = check_variant(variant)?;
    let hash = hash_bytes(png, HashAlgo::Sha1);
    let _guard = LOCK.lock().await;
    let mut list = load().await;
    if let Some(found) = list.iter().find(|e| e.hash == hash).cloned() {
        if let Some(v) = view(found).await {
            return Ok(v);
        }
        // The PNG went missing: drop the stale entry and save it again below.
        list.retain(|e| e.hash != hash);
    }
    tokio::fs::create_dir_all(dir()).await?;
    let entry = Entry { id: uuid::Uuid::new_v4().simple().to_string(), name: clean_name(name), variant, source: source.into(), origin, added_at: now_ms(), hash };
    tokio::fs::write(png_path(&entry.id), png).await?;
    list.push(entry.clone());
    save(&list).await?;
    view(entry).await.ok_or_else(|| err_tr!("skins.saveFailed", "Couldn't save that skin to the library."))
}

/// Downloads a skin texture. Only Minecraft's own texture server is allowed, so a link can't point anywhere else.
async fn download_texture(url: &str) -> Result<Vec<u8>> {
    let url = https_texture(url);
    if !url.starts_with(TEXTURES) {
        bail_tr!("skins.badTexture", "That skin isn't stored on Minecraft's texture server, so ResinCraft can't save it.");
    }
    let res = http().get(&url).timeout(TIMEOUT).send().await.map_err(|_| offline())?;
    if !res.status().is_success() {
        bail_tr!("skins.textureMissing", "Minecraft's texture server doesn't have that skin any more.");
    }
    let bytes = res.bytes().await.map_err(|_| offline())?;
    decode_skin_png(&base64::engine::general_purpose::STANDARD.encode(&bytes))
}

fn offline() -> anyhow::Error {
    err_tr!("skins.offline", "Couldn't reach the skin servers. Check your internet connection, then try again.")
}

// ---------- Library (IPC) ----------

/// Newest first.
pub async fn list() -> Result<Vec<SavedSkin>> {
    let mut list = load().await;
    list.sort_by(|a, b| b.added_at.cmp(&a.added_at));
    let mut out = Vec::with_capacity(list.len());
    for e in list {
        out.extend(view(e).await);
    }
    Ok(out)
}

/// A PNG from disk (base64 or a data: URL), checked to be a 64×64 or 64×32 skin.
pub async fn import_png(png: &str, name: &str, variant: &str) -> Result<SavedSkin> {
    let bytes = decode_skin_png(png)?;
    add(&bytes, name, variant, "upload", None).await
}

/// Renames an entry or changes its arm style.
pub async fn update(id: &str, name: Option<&str>, variant: Option<&str>) -> Result<SavedSkin> {
    let _guard = LOCK.lock().await;
    let mut list = load().await;
    let Some(e) = list.iter_mut().find(|e| e.id == id) else { bail_tr!("skins.gone", "That skin isn't in the library any more.") };
    if let Some(n) = name {
        e.name = clean_name(n);
    }
    if let Some(v) = variant {
        e.variant = check_variant(v)?;
    }
    let e = e.clone();
    save(&list).await?;
    view(e).await.ok_or_else(|| err_tr!("skins.gone", "That skin isn't in the library any more."))
}

pub async fn remove(id: &str) -> Result<()> {
    let _guard = LOCK.lock().await;
    let mut list = load().await;
    let before = list.len();
    list.retain(|e| e.id != id);
    if list.len() != before {
        save(&list).await?;
        // Ids are generated hex, never a path.
        if id.chars().all(|c| c.is_ascii_alphanumeric()) {
            let _ = tokio::fs::remove_file(png_path(id)).await;
        }
    }
    Ok(())
}

/// Saves the account's current skin into the library (as a previous skin) so changing it never loses it.
/// Best effort: a default skin or a network problem just means nothing is saved.
pub(super) async fn remember_current(account_id: &str) {
    let Ok(profile) = profile::get_profile(account_id).await else { return };
    let Some(skin) = &profile.skin else { return };
    if let Ok(png) = download_texture(&skin.url).await {
        let _ = add(&png, &profile.name, &skin.variant, "history", Some(profile.name.clone())).await;
    }
}

/// Wears a library skin on a Microsoft account. The skin it replaces is saved to the library first.
pub async fn apply(account_id: &str, id: &str) -> Result<PlayerProfile> {
    let Some(e) = load().await.into_iter().find(|e| e.id == id) else { bail_tr!("skins.gone", "That skin isn't in the library any more.") };
    let png = tokio::fs::read(png_path(&e.id)).await.map_err(|_| err_tr!("skins.gone", "That skin isn't in the library any more."))?;
    let upload = SkinUpload { variant: e.variant, png: Some(base64::engine::general_purpose::STANDARD.encode(png)), url: None };
    profile::set_skin(account_id, &upload).await
}

// ---------- Player names (Mojang) ----------

#[derive(Deserialize)]
struct NameLookup {
    id: String,
    name: String,
}

/// The skin inside a sessionserver profile's base64 `textures` property: (url, variant).
fn parse_textures(profile: &Value) -> Option<(String, &'static str)> {
    let props = profile.get("properties")?.as_array()?;
    let value = props.iter().find(|p| p.get("name").and_then(Value::as_str) == Some("textures"))?.get("value")?.as_str()?;
    let decoded = base64::engine::general_purpose::STANDARD.decode(value).ok()?;
    let v: Value = serde_json::from_slice(&decoded).ok()?;
    let skin = v.pointer("/textures/SKIN")?;
    let url = skin.get("url")?.as_str()?;
    let slim = skin.pointer("/metadata/model").and_then(Value::as_str) == Some("slim");
    Some((https_texture(url), if slim { "slim" } else { "classic" }))
}

fn player_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.len() < 2 || name.len() > 16 || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        bail_tr!("skins.badPlayer", "Player names are up to 16 characters: letters, numbers and _.");
    }
    Ok(name)
}

/// Looks up a Java player's current skin by name.
pub async fn lookup_player(name: &str) -> Result<OnlineSkin> {
    let name = player_name(name)?;
    let not_found = || err_tr!("skins.noPlayer", "There's no Java Edition player called {name}.", name = name.to_string());
    let res = http()
        .get(format!("https://api.mojang.com/users/profiles/minecraft/{}", urlencoding::encode(name)))
        .timeout(TIMEOUT)
        .send()
        .await
        .map_err(|_| offline())?;
    match res.status().as_u16() {
        200 => {}
        204 | 404 => return Err(not_found()),
        429 => bail_tr!("skins.mojangBusy", "Mojang is limiting lookups right now. Wait a minute, then try again."),
        _ => bail_tr!("skins.lookupFailed", "Couldn't look up that player right now. Try again in a moment."),
    }
    let found: NameLookup = res.json().await.map_err(|_| not_found())?;
    let res = http()
        .get(format!("https://sessionserver.mojang.com/session/minecraft/profile/{}", found.id))
        .timeout(TIMEOUT)
        .send()
        .await
        .map_err(|_| offline())?;
    if !res.status().is_success() {
        bail_tr!("skins.lookupFailed", "Couldn't look up that player right now. Try again in a moment.");
    }
    let profile: Value = res.json().await.map_err(|_| offline())?;
    let Some((url, variant)) = parse_textures(&profile) else {
        bail_tr!("skins.defaultSkin", "{name} uses one of Minecraft's default skins, so there's nothing to save.", name = found.name);
    };
    Ok(OnlineSkin { id: found.id, name: found.name, url, variant: Some(variant.into()), source: "player".into() })
}

// ---------- Online search (MineSkin) ----------

#[derive(Deserialize)]
struct MsPage {
    #[serde(default)]
    skins: Vec<MsSkin>,
    #[serde(default)]
    pagination: Option<Value>,
}

#[derive(Deserialize)]
struct MsSkin {
    uuid: String,
    #[serde(default)]
    name: Option<String>,
    texture: String,
}

fn search_url(query: &str, after: Option<&str>) -> String {
    let mut url = format!("{MINESKIN}?size={PAGE_SIZE}");
    let q = query.trim();
    if !q.is_empty() {
        url.push_str(&format!("&filter={}", urlencoding::encode(q)));
    }
    if let Some(a) = after.filter(|a| !a.is_empty()) {
        url.push_str(&format!("&after={}", urlencoding::encode(a)));
    }
    url
}

/// MineSkin's gallery lists the same texture many times (every generation of it), so keep the first of each.
fn parse_page(page: MsPage) -> OnlineSkinPage {
    let next = page.pagination.as_ref().and_then(|p| p.pointer("/next/after")).and_then(Value::as_str).map(String::from);
    let mut seen = std::collections::HashSet::new();
    let skins = page
        .skins
        .into_iter()
        .filter(|s| s.texture.len() >= 32 && s.texture.chars().all(|c| c.is_ascii_hexdigit()) && seen.insert(s.texture.clone()))
        .map(|s| OnlineSkin {
            name: s.name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_default(),
            url: format!("{TEXTURES}{}", s.texture),
            id: s.uuid,
            variant: None,
            source: "mineskin".into(),
        })
        .collect();
    OnlineSkinPage { skins, next }
}

fn mineskin_error(status: u16) -> anyhow::Error {
    match status {
        429 => err_tr!("skins.mineskinBusy", "MineSkin is limiting searches right now. Wait a minute, then try again."),
        _ => err_tr!("skins.searchFailed", "MineSkin's skin search isn't responding right now. Try again in a few minutes."),
    }
}

async fn mineskin_get(url: &str) -> Result<Value> {
    let mut req = http().get(url).timeout(TIMEOUT).header("Accept", "application/json");
    let key = settings::mineskin_key();
    if !key.is_empty() {
        req = req.bearer_auth(key);
    }
    let res = req.send().await.map_err(|_| offline())?;
    if !res.status().is_success() {
        return Err(mineskin_error(res.status().as_u16()));
    }
    res.json().await.map_err(|_| mineskin_error(500))
}

/// Public skins from MineSkin's gallery; an empty query lists the newest.
pub async fn search(query: &str, after: Option<&str>) -> Result<OnlineSkinPage> {
    let v = mineskin_get(&search_url(query, after)).await?;
    let page: MsPage = serde_json::from_value(v).map_err(|_| mineskin_error(500))?;
    Ok(parse_page(page))
}

/// The arm style of a MineSkin skin (the gallery list doesn't include it).
async fn mineskin_variant(uuid: &str) -> String {
    if !uuid.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return "classic".into();
    }
    let v = mineskin_get(&format!("{MINESKIN}/{uuid}")).await.ok();
    match v.as_ref().and_then(|v| v.pointer("/skin/variant")).and_then(Value::as_str) {
        Some("slim") => "slim".into(),
        _ => "classic".into(),
    }
}

/// Saves a skin from a player lookup or the online search into the library.
pub async fn save_online(skin: &OnlineSkin) -> Result<SavedSkin> {
    let png = download_texture(&skin.url).await?;
    let variant = match skin.variant.as_deref() {
        Some(v) => v.to_string(),
        None if skin.source == "mineskin" => mineskin_variant(&skin.id).await,
        None => "classic".into(),
    };
    let (source, origin) = if skin.source == "player" { ("player", Some(skin.name.clone())) } else { ("online", Some(format!("mineskin:{}", skin.id))) };
    let name = if skin.name.trim().is_empty() { "MineSkin" } else { &skin.name };
    add(&png, name, &variant, source, origin).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_player_textures() {
        let textures = json!({
            "textures": { "SKIN": { "url": "http://textures.minecraft.net/texture/abc", "metadata": { "model": "slim" } } }
        });
        let value = base64::engine::general_purpose::STANDARD.encode(textures.to_string());
        let profile = json!({ "id": "x", "name": "Alex", "properties": [{ "name": "textures", "value": value }] });
        assert_eq!(parse_textures(&profile), Some(("https://textures.minecraft.net/texture/abc".into(), "slim")));

        let classic = base64::engine::general_purpose::STANDARD.encode(json!({ "textures": { "SKIN": { "url": "http://textures.minecraft.net/texture/d" } } }).to_string());
        assert_eq!(parse_textures(&json!({ "properties": [{ "name": "textures", "value": classic }] })).unwrap().1, "classic");

        let none = base64::engine::general_purpose::STANDARD.encode(json!({ "textures": {} }).to_string());
        assert_eq!(parse_textures(&json!({ "properties": [{ "name": "textures", "value": none }] })), None);
        assert_eq!(parse_textures(&json!({})), None);
    }

    #[test]
    fn checks_player_names() {
        assert_eq!(player_name(" Notch ").unwrap(), "Notch");
        assert!(player_name("jeb_").is_ok());
        assert!(player_name("a").is_err());
        assert!(player_name("seventeen_chars_x").is_err());
        assert!(player_name("../x").is_err());
        assert!(player_name("no spaces").is_err());
    }

    #[test]
    fn builds_search_urls() {
        assert_eq!(search_url("", None), "https://api.mineskin.org/v2/skins?size=24");
        assert_eq!(search_url(" red fox ", Some("abc")), "https://api.mineskin.org/v2/skins?size=24&filter=red%20fox&after=abc");
    }

    #[test]
    fn parses_gallery_pages() {
        let page: MsPage = serde_json::from_value(json!({
            "skins": [
                { "uuid": "u1", "name": "Fox", "texture": "a".repeat(64) },
                { "uuid": "u2", "name": null, "texture": "a".repeat(64) },
                { "uuid": "u3", "name": " ", "texture": "b".repeat(64) },
                { "uuid": "u4", "name": "Bad", "texture": "../../etc" }
            ],
            "pagination": { "next": { "after": "u3" } }
        }))
        .unwrap();
        let out = parse_page(page);
        assert_eq!(out.skins.len(), 2, "duplicate texture and bad hash dropped");
        assert_eq!(out.skins[0].url, format!("https://textures.minecraft.net/texture/{}", "a".repeat(64)));
        assert_eq!(out.skins[0].name, "Fox");
        assert_eq!(out.skins[1].name, "");
        assert_eq!(out.next.as_deref(), Some("u3"));
        assert!(parse_page(serde_json::from_value(json!({ "skins": [] })).unwrap()).next.is_none());
    }

    #[test]
    fn cleans_names() {
        assert_eq!(clean_name("  My skin\n "), "My skin");
        assert_eq!(clean_name(""), "Skin");
        assert_eq!(clean_name(&"x".repeat(100)).len(), NAME_MAX);
        assert!(check_variant("slim").is_ok() && check_variant("wide").is_err());
    }
}
