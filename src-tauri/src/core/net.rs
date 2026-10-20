//! HTTP helpers and the verified, retrying, parallel downloader.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use futures::stream::{self, StreamExt};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha1::Digest;
use tokio::io::AsyncWriteExt;

use super::ctx::http;
use crate::err_tr;

/// A non-2xx response. Use `status_of(&err)` to read the status from an anyhow error.
#[derive(Debug)]
pub struct HttpError {
    pub status: u16,
    pub url: String,
    pub body: String,
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Request failed (HTTP {}) for {}", self.status, self.url)
    }
}
impl std::error::Error for HttpError {}

/// The HTTP status behind an error, if it was an `HttpError`.
pub fn status_of(err: &anyhow::Error) -> Option<u16> {
    err.downcast_ref::<HttpError>().map(|e| e.status)
}
/// The response body behind an error, if it was an `HttpError`.
pub fn body_of(err: &anyhow::Error) -> Option<&str> {
    err.downcast_ref::<HttpError>().map(|e| e.body.as_str())
}

/// Sends a request and parses the JSON response; non-2xx becomes an `HttpError`.
pub async fn send_json<T: DeserializeOwned>(req: reqwest::RequestBuilder) -> Result<T> {
    let res = req.header("Accept", "application/json").send().await?;
    let status = res.status();
    let url = res.url().to_string();
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(HttpError { status: status.as_u16(), url, body }.into());
    }
    let bytes = res.bytes().await?;
    serde_json::from_slice(&bytes).map_err(|_| err_tr!("download.unexpectedResponse", "Unexpected response from {url}", url = url))
}

pub async fn fetch_json<T: DeserializeOwned>(url: &str) -> Result<T> {
    send_json(http().get(url)).await
}

pub async fn post_json<T: DeserializeOwned, B: Serialize + ?Sized>(url: &str, body: &B) -> Result<T> {
    send_json(http().post(url).json(body)).await
}

/// GETs a URL as text (XML metadata, plain-text endpoints); non-2xx becomes an `HttpError`.
pub async fn fetch_text(url: &str) -> Result<String> {
    let res = http().get(url).send().await?;
    let status = res.status();
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(HttpError { status: status.as_u16(), url: url.to_string(), body }.into());
    }
    Ok(res.text().await?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgo {
    /// Only for sources that publish nothing better (ATLauncher, Technic Solder).
    Md5,
    Sha1,
    Sha256,
    Sha512,
}

#[derive(Debug, Clone)]
pub struct Hash {
    pub algo: HashAlgo,
    pub value: String,
}

impl Hash {
    pub fn md5(v: impl Into<String>) -> Hash {
        Hash { algo: HashAlgo::Md5, value: v.into() }
    }
    pub fn sha1(v: impl Into<String>) -> Hash {
        Hash { algo: HashAlgo::Sha1, value: v.into() }
    }
    pub fn sha256(v: impl Into<String>) -> Hash {
        Hash { algo: HashAlgo::Sha256, value: v.into() }
    }
    pub fn sha512(v: impl Into<String>) -> Hash {
        Hash { algo: HashAlgo::Sha512, value: v.into() }
    }
}

#[derive(Debug, Clone)]
pub struct DownloadItem {
    pub url: String,
    pub dest: PathBuf,
    pub hash: Option<Hash>,
    pub size: Option<u64>,
    /// Skip hashing existing files whose size matches (content-addressed assets).
    pub trust_size: bool,
}

impl DownloadItem {
    pub fn new(url: impl Into<String>, dest: impl Into<PathBuf>) -> Self {
        DownloadItem { url: url.into(), dest: dest.into(), hash: None, size: None, trust_size: false }
    }
    pub fn hash(mut self, h: Option<Hash>) -> Self {
        self.hash = h;
        self
    }
    pub fn size(mut self, s: Option<u64>) -> Self {
        self.size = s;
        self
    }
}

pub fn hash_bytes(data: &[u8], algo: HashAlgo) -> String {
    match algo {
        HashAlgo::Md5 => hex::encode(md5::Md5::digest(data)),
        HashAlgo::Sha1 => hex::encode(sha1::Sha1::digest(data)),
        HashAlgo::Sha256 => hex::encode(sha2::Sha256::digest(data)),
        HashAlgo::Sha512 => hex::encode(sha2::Sha512::digest(data)),
    }
}

/// Hashes a file without blocking the async runtime.
pub async fn hash_file(path: &Path, algo: HashAlgo) -> Result<String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<String> {
        use std::io::Read;
        let mut f = std::fs::File::open(&path)?;
        let mut buf = vec![0u8; 1 << 16];
        macro_rules! run {
            ($h:expr) => {{
                let mut h = $h;
                loop {
                    let n = f.read(&mut buf)?;
                    if n == 0 {
                        break;
                    }
                    h.update(&buf[..n]);
                }
                hex::encode(h.finalize())
            }};
        }
        Ok(match algo {
            HashAlgo::Md5 => run!(md5::Md5::new()),
            HashAlgo::Sha1 => run!(sha1::Sha1::new()),
            HashAlgo::Sha256 => run!(sha2::Sha256::new()),
            HashAlgo::Sha512 => run!(sha2::Sha512::new()),
        })
    })
    .await?
}

// ---------- Verified-file cache ----------
// Hashing every library before every launch costs 0.15–0.35 s (≈95 MB for 1.21.1). A file whose hash
// already matched is remembered with the size and modified time it had then; while both are unchanged
// it's trusted without re-hashing. Replacing, editing, truncating or deleting a file changes its size or
// modified time, so it gets hashed again (and re-downloaded if it no longer matches).

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct Verified {
    size: u64,
    /// Modified time in nanoseconds since 1970.
    mtime: u128,
    /// The hash the file matched, lowercase.
    hash: String,
}

/// path → last verification; `None` until loaded. The bool says it changed since the last save.
static VERIFIED: Mutex<Option<(HashMap<String, Verified>, bool)>> = Mutex::new(None);

fn verified_file() -> Option<PathBuf> {
    super::ctx::is_ready().then(|| super::paths::cache().join("verified-files.json"))
}

fn with_verified<R>(f: impl FnOnce(&mut (HashMap<String, Verified>, bool)) -> R) -> R {
    let mut guard = VERIFIED.lock().unwrap();
    let state = guard.get_or_insert_with(|| {
        let map = verified_file().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        (map, false)
    });
    f(state)
}

fn stamp(meta: &std::fs::Metadata) -> Option<u128> {
    meta.modified().ok()?.duration_since(UNIX_EPOCH).ok().map(|d| d.as_nanos())
}

fn key(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// True when `path` was verified against `hash` and hasn't changed since.
fn verified_unchanged(path: &Path, meta: &std::fs::Metadata, hash: &Hash) -> bool {
    let Some(mtime) = stamp(meta) else { return false };
    with_verified(|(map, _)| map.get(&key(path)).is_some_and(|v| v.size == meta.len() && v.mtime == mtime && v.hash.eq_ignore_ascii_case(&hash.value)))
}

/// Remembers that `path` (as it is on disk now) matched `hash`.
async fn remember_verified(path: &Path, hash: &Hash) {
    let Ok(meta) = tokio::fs::metadata(path).await else { return };
    let Some(mtime) = stamp(&meta) else { return };
    let entry = Verified { size: meta.len(), mtime, hash: hash.value.to_ascii_lowercase() };
    with_verified(|(map, dirty)| {
        if map.get(&key(path)) != Some(&entry) {
            map.insert(key(path), entry);
            *dirty = true;
        }
    });
}

fn forget_verified(path: &Path) {
    with_verified(|(map, dirty)| {
        if map.remove(&key(path)).is_some() {
            *dirty = true;
        }
    });
}

/// Writes the verified-file cache if it changed. Losing it only means files get hashed again.
async fn save_verified() {
    let Some(file) = verified_file() else { return };
    let Some(map) = with_verified(|(map, dirty)| std::mem::take(dirty).then(|| map.clone())) else { return };
    let Ok(json) = serde_json::to_vec(&map) else { return };
    let tmp = file.with_extension("json.tmp");
    let _ = tokio::fs::create_dir_all(file.parent().unwrap_or(&file)).await;
    if tokio::fs::write(&tmp, json).await.is_ok() {
        let _ = tokio::fs::rename(&tmp, &file).await;
    }
}

async fn is_valid(item: &DownloadItem) -> bool {
    let Ok(meta) = tokio::fs::metadata(&item.dest).await else { return false };
    if let Some(size) = item.size {
        if meta.len() != size {
            return false;
        }
    }
    match &item.hash {
        Some(h) if !item.trust_size => {
            if verified_unchanged(&item.dest, &meta, h) {
                return true;
            }
            let ok = hash_file(&item.dest, h.algo).await.map(|v| v.eq_ignore_ascii_case(&h.value)).unwrap_or(false);
            if ok {
                remember_verified(&item.dest, h).await;
            } else {
                forget_verified(&item.dest);
            }
            ok
        }
        _ => true,
    }
}

/// Downloads one file to `dest` (via `<dest>.part`), verifying size and hash. Retries 3 times.
pub async fn download_file(item: &DownloadItem) -> Result<()> {
    if item.url.trim().is_empty() {
        let file = item.dest.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        return Err(err_tr!("download.noAddress", "No download address for {file}", file = file));
    }
    if let Some(parent) = item.dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = PathBuf::from(format!("{}.part", item.dest.display()));
    let mut last: Option<anyhow::Error> = None;
    for attempt in 0..3u64 {
        match try_download(item, &tmp).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                let _ = tokio::fs::remove_file(&tmp).await;
                let not_found = status_of(&e) == Some(404);
                last = Some(e);
                if not_found {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(500 * (attempt + 1))).await;
            }
        }
    }
    Err(last.unwrap_or_else(|| err_tr!("download.failed", "Download failed: {url}", url = item.url)))
}

async fn try_download(item: &DownloadItem, tmp: &Path) -> Result<()> {
    let mut res = http().get(&item.url).send().await?;
    if !res.status().is_success() {
        return Err(HttpError { status: res.status().as_u16(), url: item.url.clone(), body: String::new() }.into());
    }
    let mut file = tokio::fs::File::create(tmp).await?;
    while let Some(chunk) = res.chunk().await? {
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    drop(file);
    if let Some(h) = &item.hash {
        let got = hash_file(tmp, h.algo).await?;
        if !got.eq_ignore_ascii_case(&h.value) {
            return Err(anyhow!("Checksum mismatch for {}", item.url));
        }
    }
    tokio::fs::rename(tmp, &item.dest).await?;
    if let Some(h) = &item.hash {
        remember_verified(&item.dest, h).await;
    }
    Ok(())
}

/// Downloads every missing or corrupt file, `concurrency` at a time, reporting (done, total).
pub async fn download_all<F>(items: Vec<DownloadItem>, concurrency: usize, on_progress: F) -> Result<()>
where
    F: Fn(usize, usize) + Send + Sync,
{
    // De-duplicate by destination.
    let mut seen = std::collections::HashSet::new();
    let unique: Vec<DownloadItem> = items.into_iter().filter(|i| seen.insert(i.dest.clone())).collect();
    let total = unique.len();
    on_progress(0, total);
    let done = Arc::new(AtomicUsize::new(0));
    let failed = Arc::new(AtomicBool::new(false));

    let results: Vec<Result<()>> = stream::iter(unique)
        .map(|item| {
            let done = done.clone();
            let failed = failed.clone();
            let on_progress = &on_progress;
            async move {
                if failed.load(Ordering::Relaxed) {
                    return Ok(());
                }
                if !is_valid(&item).await {
                    if let Err(e) = download_file(&item).await {
                        failed.store(true, Ordering::Relaxed);
                        return Err(e);
                    }
                }
                on_progress(done.fetch_add(1, Ordering::Relaxed) + 1, total);
                Ok(())
            }
        })
        .buffer_unordered(concurrency.max(1))
        .collect()
        .await;
    save_verified().await;
    results.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn changed_files_are_hashed_again() {
        let dir = std::env::temp_dir().join(format!("resin-verified-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("lib.jar");
        std::fs::write(&path, b"hello world").unwrap();
        let item = DownloadItem::new("", &path).hash(Some(Hash::sha1(hash_bytes(b"hello world", HashAlgo::Sha1)))).size(Some(11));

        assert!(is_valid(&item).await);
        // Verified and unchanged: trusted without hashing.
        let meta = std::fs::metadata(&path).unwrap();
        assert!(verified_unchanged(&path, &meta, item.hash.as_ref().unwrap()));

        // Same size, different content: the modified time moves, so it's hashed again and rejected.
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(&path, b"hello WORLD").unwrap();
        assert!(!is_valid(&item).await);
        // ...and stays rejected (the stale entry was dropped).
        assert!(!is_valid(&item).await);

        std::fs::remove_file(&path).unwrap();
        assert!(!is_valid(&item).await);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
