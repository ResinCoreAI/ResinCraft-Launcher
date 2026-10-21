//! The per-instance content index (`<instance>/content-index.json`) plus helpers shared by every
//! content source: modpack override extraction, zip reading, md5 checks and small format helpers.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::core::fsutil::{read_json, safe_join, write_json};
use crate::core::paths;
use crate::core::types::{ContentKind, Instance, LoaderType};

// ---------- Per-instance content index ----------

/// Where one installed file came from. Shared by every content source.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct IndexEntry {
    /// Bare id for Modrinth, "cf:<id>" for CurseForge.
    pub project_id: String,
    pub version_id: String,
    pub version_number: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    pub sha1: String,
}

/// Maps "kind/filename" (without `.disabled`) to where the file came from.
pub type ContentIndex = BTreeMap<String, IndexEntry>;

fn index_file(id: &str) -> PathBuf {
    paths::instance_dir(id).join("content-index.json")
}

pub async fn load_index(id: &str) -> ContentIndex {
    read_json(&index_file(id)).await.unwrap_or_default()
}

pub async fn save_index(id: &str, idx: &ContentIndex) -> Result<()> {
    write_json(&index_file(id), idx).await
}

pub fn index_key(kind: ContentKind, filename: &str) -> String {
    format!("{}/{}", kind.as_str(), filename.strip_suffix(".disabled").unwrap_or(filename))
}

/// The folder inside the instance's game directory that holds `kind`.
pub fn folder_of(inst: &Instance, kind: ContentKind) -> PathBuf {
    paths::game_dir(&inst.id).join(kind.folder())
}

/// Loaders whose mods an instance can run (Quilt runs Fabric mods; NeoForge 1.20.1 still runs Forge mods;
/// Forge with LiteLoader runs both).
pub fn loaders_for(inst: &Instance) -> Vec<String> {
    match inst.loader {
        LoaderType::Forge if inst.liteloader.as_deref().is_some_and(|v| !v.is_empty()) => vec!["forge".into(), "liteloader".into()],
        LoaderType::Quilt => vec!["quilt".into(), "fabric".into()],
        LoaderType::Neoforge if inst.mc_version == "1.20.1" => vec!["neoforge".into(), "forge".into()],
        l => vec![l.as_str().into()],
    }
}

/// Deletes any installed copy of `project_id` so a new version can take its place.
pub async fn remove_project_files(inst: &Instance, kind: ContentKind, project_id: &str, idx: &mut ContentIndex) -> Result<()> {
    let folder = folder_of(inst, kind);
    let prefix = format!("{}/", kind.as_str());
    let keys: Vec<String> = idx.iter().filter(|(k, e)| e.project_id == project_id && k.starts_with(&prefix)).map(|(k, _)| k.clone()).collect();
    for key in keys {
        let old = &key[prefix.len()..];
        if let Ok(p) = safe_join(&folder, old) {
            remove_if_exists(&p).await?;
            remove_if_exists(&PathBuf::from(format!("{}.disabled", p.display()))).await?;
        }
        idx.remove(&key);
    }
    Ok(())
}

async fn remove_if_exists(p: &Path) -> Result<()> {
    match tokio::fs::remove_file(p).await {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

/// Parses a loader name ("fabric", "NeoForge"…) into a LoaderType.
pub fn parse_loader(name: &str) -> Option<LoaderType> {
    serde_json::from_value(Value::String(name.to_lowercase())).ok()
}

// ---------- Zips (blocking: call from spawn_blocking) ----------

pub type Zip = zip::ZipArchive<std::fs::File>;

pub fn open_zip(path: &Path) -> Result<Zip> {
    let f = std::fs::File::open(path)?;
    Ok(zip::ZipArchive::new(f)?)
}

/// Reads one entry as text; None if missing.
pub fn read_zip_text<R: std::io::Read + std::io::Seek>(zip: &mut zip::ZipArchive<R>, name: &str) -> Result<Option<String>> {
    let mut entry = match zip.by_name(name) {
        Ok(e) => e,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let mut s = String::new();
    entry.read_to_string(&mut s)?;
    Ok(Some(s))
}

pub fn has_entry<R: std::io::Read + std::io::Seek>(zip: &zip::ZipArchive<R>, name: &str) -> bool {
    zip.index_for_name(name).is_some()
}

/// Unpacks every file whose name passes `map` (which returns the destination path relative to `dir`).
/// Every destination goes through `safe_join` (zip-slip guard).
pub fn extract_where<R: std::io::Read + std::io::Seek>(zip: &mut zip::ZipArchive<R>, dir: &Path, map: impl Fn(&str) -> Option<String>) -> Result<()> {
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        let Some(rel) = map(&name) else { continue };
        let dest = safe_join(dir, &rel)?;
        if dest == dir {
            continue;
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::File::create(&dest)?;
        std::io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}

/// Copies a modpack archive's override folders (configs, scripts…) into the game directory.
pub fn extract_overrides(zip_path: &Path, prefixes: &[String], dir: &Path) -> Result<()> {
    let mut zip = open_zip(zip_path)?;
    extract_where(&mut zip, dir, |name| prefixes.iter().find(|p| name.starts_with(p.as_str())).map(|p| name[p.len()..].to_string()))
}


// ---------- Small format helpers ----------

/// Unix seconds to the same ISO string JavaScript's `toISOString()` produces.
pub fn iso_from_secs(secs: i64) -> String {
    chrono::DateTime::from_timestamp(secs, 0).map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)).unwrap_or_default()
}

/// Milliseconds since the epoch of an ISO date (like `Date.parse`); 0 if unparseable.
pub fn parse_date_ms(s: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(s).map(|d| d.timestamp_millis()).unwrap_or(0)
}

/// JavaScript's `encodeURI`: escapes everything except URL syntax characters.
pub fn encode_uri(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b";,/?:@&=+$-_.!~*'()#".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `a.localeCompare(b, undefined, { numeric: true })`, roughly: digit runs compare as numbers.
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    fn chunks(s: &str) -> Vec<(bool, String)> {
        let mut out: Vec<(bool, String)> = Vec::new();
        for c in s.chars() {
            let d = c.is_ascii_digit();
            match out.last_mut() {
                Some((ld, cur)) if *ld == d => cur.push(c),
                _ => out.push((d, c.to_string())),
            }
        }
        out
    }
    let (ca, cb) = (chunks(a), chunks(b));
    for (x, y) in ca.iter().zip(cb.iter()) {
        let ord = if x.0 && y.0 {
            let (xt, yt) = (x.1.trim_start_matches('0'), y.1.trim_start_matches('0'));
            xt.len().cmp(&yt.len()).then_with(|| xt.cmp(yt))
        } else {
            x.1.to_lowercase().cmp(&y.1.to_lowercase())
        };
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
    }
    ca.len().cmp(&cb.len())
}

/// Case-insensitive ordering for display lists (close to `localeCompare`).
/// Sorts names the way people read them (like JS `localeCompare`): spaces and punctuation are ignored, letters
/// compare case-insensitively, and digit runs compare as numbers. "[EMF] Entity…" sorts under E.
pub fn display_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let key = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).collect::<String>();
    natural_cmp(&key(a), &key(b)).then_with(|| a.to_lowercase().cmp(&b.to_lowercase())).then_with(|| a.cmp(b))
}

/// Accepts a number, a numeric string, or null (APIs that send `"123"` or `123` interchangeably).
pub fn lenient_u64<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Option<u64>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Number(n) => n.as_u64().or_else(|| n.as_f64().map(|f| f.max(0.0) as u64)),
        Value::String(s) => s.trim().parse::<f64>().ok().map(|f| f.max(0.0) as u64),
        _ => None,
    })
}

/// Like `lenient_u64` but defaulting to 0.
pub fn lenient_u64_zero<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<u64, D::Error> {
    lenient_u64(d).map(|v| v.unwrap_or(0))
}

/// Removes a half-installed instance after a failed modpack install.
pub async fn discard_instance(id: &str) {
    let _ = tokio::fs::remove_dir_all(paths::instance_dir(id)).await;
}

/// Removes a half-installed modpack instance when its install is cancelled (the work is dropped, so the install's
/// own error handling never runs). `keep` once the instance is complete.
pub struct InstanceGuard(Option<String>);

impl InstanceGuard {
    pub fn new(id: &str) -> InstanceGuard {
        InstanceGuard(Some(id.to_string()))
    }
    pub fn keep(mut self) {
        self.0 = None;
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        if let Some(id) = self.0.take() {
            tokio::spawn(async move { discard_instance(&id).await });
        }
    }
}

/// A file name from remote data, refused if it would escape its folder.
pub fn safe_name<'a>(name: &'a str) -> Result<&'a str> {
    if name.is_empty() || name.contains(['/', '\\']) || name == "." || name == ".." {
        return Err(anyhow!("Unsafe path: {name}"));
    }
    Ok(name)
}
