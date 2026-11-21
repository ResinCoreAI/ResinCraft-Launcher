//! HTTP helpers and the verified, retrying, parallel downloader.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};

use anyhow::{anyhow, bail, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha1::Digest;

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

/// How long a request or download may go without a single byte arriving before it's given up on. The HTTP client
/// only has a connect timeout, so a connection that went quiet midway used to be waited on forever: Forge's Maven
/// did that to installer downloads (a 26.3 server's installer stopped at 2.2 of its 8 MB, and the server was
/// "being made" until the launcher closed; a Forge modpack's preparation stopped the same way). A file download
/// that stalls is tried again from where it stopped.
const STALL: Duration = Duration::from_secs(30);

/// A connection that went quiet for `STALL` (see there).
#[derive(Debug)]
pub struct Stalled {
    pub url: String,
}

impl fmt::Display for Stalled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Nothing arrived for {} seconds from {}", STALL.as_secs(), self.url)
    }
}
impl std::error::Error for Stalled {}

/// `step` (sending a request, or reading the next piece of an answer), or a `Stalled` error once nothing has come
/// for `STALL`.
pub(crate) async fn unless_stalled<T, E: Into<anyhow::Error>>(url: &str, step: impl std::future::Future<Output = std::result::Result<T, E>>) -> Result<T> {
    match tokio::time::timeout(STALL, step).await {
        Ok(r) => r.map_err(Into::into),
        Err(_) => Err(Stalled { url: url.to_string() }.into()),
    }
}

/// The whole body of `res`, giving up when it stalls (`res.bytes()` would wait forever).
async fn body_bytes(mut res: reqwest::Response) -> Result<Vec<u8>> {
    let url = res.url().to_string();
    let mut out = Vec::with_capacity(res.content_length().unwrap_or(0).min(16 << 20) as usize);
    while let Some(piece) = unless_stalled(&url, res.chunk()).await? {
        out.extend_from_slice(&piece);
    }
    Ok(out)
}

/// The body of an answer that failed, as text, for the error; empty if it doesn't come.
async fn error_body(res: reqwest::Response) -> String {
    body_bytes(res).await.map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
}

/// Sends a request and parses the JSON response; non-2xx becomes an `HttpError`.
pub async fn send_json<T: DeserializeOwned>(req: reqwest::RequestBuilder) -> Result<T> {
    let req = req.header("Accept", "application/json");
    let label = req.try_clone().and_then(|r| r.build().ok()).map(|r| r.url().to_string()).unwrap_or_default();
    let res = unless_stalled(&label, req.send()).await?;
    let status = res.status();
    let url = res.url().to_string();
    if !status.is_success() {
        let body = error_body(res).await;
        return Err(HttpError { status: status.as_u16(), url, body }.into());
    }
    let bytes = body_bytes(res).await?;
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
    let res = unless_stalled(url, http().get(url).send()).await?;
    let status = res.status();
    if !status.is_success() {
        let body = error_body(res).await;
        return Err(HttpError { status: status.as_u16(), url: url.to_string(), body }.into());
    }
    let bytes = body_bytes(res).await?;
    Ok(String::from_utf8(bytes).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()))
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

/// A hash worked out piece by piece, as the bytes arrive.
pub enum RunningHash {
    Md5(md5::Md5),
    Sha1(sha1::Sha1),
    Sha256(sha2::Sha256),
    Sha512(sha2::Sha512),
}

impl RunningHash {
    pub fn new(algo: HashAlgo) -> RunningHash {
        match algo {
            HashAlgo::Md5 => RunningHash::Md5(md5::Md5::new()),
            HashAlgo::Sha1 => RunningHash::Sha1(sha1::Sha1::new()),
            HashAlgo::Sha256 => RunningHash::Sha256(sha2::Sha256::new()),
            HashAlgo::Sha512 => RunningHash::Sha512(sha2::Sha512::new()),
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        match self {
            RunningHash::Md5(h) => h.update(data),
            RunningHash::Sha1(h) => h.update(data),
            RunningHash::Sha256(h) => h.update(data),
            RunningHash::Sha512(h) => h.update(data),
        }
    }

    pub fn finish(self) -> String {
        match self {
            RunningHash::Md5(h) => hex::encode(h.finalize()),
            RunningHash::Sha1(h) => hex::encode(h.finalize()),
            RunningHash::Sha256(h) => hex::encode(h.finalize()),
            RunningHash::Sha512(h) => hex::encode(h.finalize()),
        }
    }
}

/// Hashes everything `r` has to give (blocking).
pub fn hash_read(mut r: impl std::io::Read, algo: HashAlgo) -> std::io::Result<String> {
    let mut h = RunningHash::new(algo);
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = r.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finish())
}

/// Hashes a file without blocking the async runtime.
pub async fn hash_file(path: &Path, algo: HashAlgo) -> Result<String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<String> { Ok(hash_read(std::fs::File::open(&path)?, algo)?) }).await?
}

// ---------- Verified-file cache ----------
// Hashing every library before every launch costs 0.15–0.35 s (≈95 MB for 1.21.1). A file whose hash
// already matched is remembered with the size and modified time it had then; while both are unchanged
// it's trusted without re-hashing. Replacing, editing, truncating or deleting a file changes its size or
// modified time, so it gets hashed again (and re-downloaded if it no longer matches).
//
// The same goes for files the launcher hashes for other reasons (`file_hash`): a modpack's jars are checked when
// they're downloaded, looked up on Modrinth by hash, fingerprinted for CurseForge, recorded as the pack's files and
// compared again at every version switch. Each of those asks here first, so a file is read once per kind of hash
// instead of once per question.

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct Verified {
    size: u64,
    /// Modified time in nanoseconds since 1970.
    mtime: u128,
    /// The hash the file matched (or was found to have), lowercase. Empty when only something in `more` is known.
    hash: String,
    /// What else is known of the same bytes, by name: hashes with other algorithms ("md5", "sha1"…) and
    /// `FINGERPRINT`. Older launchers read `hash` only and leave this out when they save.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    more: BTreeMap<String, String>,
}

/// CurseForge's fingerprint of a file (content/curseforge.rs), as kept in `Verified::more`.
const FINGERPRINT: &str = "cf";

/// The name a hash made with `algo` is remembered under.
fn hash_name(algo: HashAlgo) -> &'static str {
    match algo {
        HashAlgo::Md5 => "md5",
        HashAlgo::Sha1 => "sha1",
        HashAlgo::Sha256 => "sha256",
        HashAlgo::Sha512 => "sha512",
    }
}

impl Verified {
    fn new(meta: &std::fs::Metadata, mtime: u128) -> Verified {
        Verified { size: meta.len(), mtime, hash: String::new(), more: BTreeMap::new() }
    }

    /// Which algorithm `hash` was made with: each one's hex has its own length.
    fn main_name(&self) -> Option<&'static str> {
        match self.hash.len() {
            32 => Some("md5"),
            40 => Some("sha1"),
            64 => Some("sha256"),
            128 => Some("sha512"),
            _ => None,
        }
    }

    fn get(&self, name: &str) -> Option<&str> {
        if self.main_name() == Some(name) {
            Some(&self.hash)
        } else {
            self.more.get(name).map(String::as_str)
        }
    }
}

/// path → last verification; `None` until loaded. The bool says it changed since the last save.
static VERIFIED: Mutex<Option<(HashMap<String, Verified>, bool)>> = Mutex::new(None);

fn verified_file() -> Option<PathBuf> {
    super::ctx::is_ready().then(|| super::paths::cache().join("verified-files.json"))
}

fn with_verified<R>(f: impl FnOnce(&mut (HashMap<String, Verified>, bool)) -> R) -> R {
    let mut guard = VERIFIED.lock().unwrap();
    let state = guard.get_or_insert_with(|| {
        let mut map: HashMap<String, Verified> = verified_file().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        // Launchers up to 1.6.1 also noted every asset they downloaded (about 4,000 a game version), though assets
        // are told by their size and nothing ever asks about them here: not kept in memory, and gone from the file
        // with the next save.
        let before = map.len();
        if super::ctx::is_ready() {
            let assets = format!("{}{}", key(&super::paths::assets().join("objects")), std::path::MAIN_SEPARATOR);
            map.retain(|path, _| !path.starts_with(&assets));
            map.shrink_to_fit();
        }
        let dropped = map.len() != before;
        (map, dropped)
    });
    f(state)
}

fn stamp(meta: &std::fs::Metadata) -> Option<u128> {
    meta.modified().ok()?.duration_since(UNIX_EPOCH).ok().map(|d| d.as_nanos())
}

/// A file's name in the cache. Windows takes both slashes, and paths get put together both ways
/// (`dir.join("mods/a.jar")`, `safe_join`), so one file is always found under its backslash form.
fn key(path: &Path) -> String {
    let text = path.to_string_lossy();
    if cfg!(windows) {
        text.replace('/', "\\")
    } else {
        text.into_owned()
    }
}

/// What's remembered as `name` of the file at `path`, if the file hasn't changed since (`meta` is the file's now).
fn recall(path: &Path, meta: &std::fs::Metadata, name: &str) -> Option<String> {
    let mtime = stamp(meta)?;
    with_verified(|(map, _)| map.get(&key(path)).filter(|v| v.size == meta.len() && v.mtime == mtime).and_then(|v| v.get(name)).map(str::to_string))
}

/// Remembers `value` as `name` of the file at `path`; `meta` is the file's from when `value` was worked out.
fn remember(path: &Path, meta: &std::fs::Metadata, name: &str, value: &str) {
    let Some(mtime) = stamp(meta) else { return };
    let value = value.to_ascii_lowercase();
    with_verified(|(map, dirty)| {
        let entry = map.entry(key(path)).or_insert_with(|| Verified::new(meta, mtime));
        let same_file = entry.size == meta.len() && entry.mtime == mtime;
        match entry.get(name) {
            Some(known) if same_file && known == value => return,
            // Other bytes under the same size and time: nothing else known of them holds either.
            Some(_) => *entry = Verified::new(meta, mtime),
            None if !same_file => *entry = Verified::new(meta, mtime),
            None => {}
        }
        if name != FINGERPRINT && (entry.hash.is_empty() || entry.main_name() == Some(name)) {
            entry.hash = value;
        } else {
            entry.more.insert(name.to_string(), value);
        }
        *dirty = true;
    });
}

/// True when `path` was verified against `hash` and hasn't changed since.
fn verified_unchanged(path: &Path, meta: &std::fs::Metadata, hash: &Hash) -> bool {
    recall(path, meta, hash_name(hash.algo)).is_some_and(|v| v.eq_ignore_ascii_case(&hash.value))
}

/// Remembers that `path` (as it is on disk now) matched `hash`.
async fn remember_verified(path: &Path, hash: &Hash) {
    let Ok(meta) = tokio::fs::metadata(path).await else { return };
    remember(path, &meta, hash_name(hash.algo), &hash.value);
}

fn forget_verified(path: &Path) {
    with_verified(|(map, dirty)| {
        if map.remove(&key(path)).is_some() {
            *dirty = true;
        }
    });
}

/// Forgets every file checked so far, so each one is hashed again (and downloaded again if it no longer matches)
/// the next time it's needed: Reinstall.
pub async fn forget_all_verified() {
    with_verified(|(map, dirty)| {
        if !map.is_empty() {
            map.clear();
            *dirty = true;
        }
    });
    save_verified().await;
}

/// Writes the verified-file cache if it changed. Losing it only means files get hashed again.
pub async fn save_verified() {
    let Some(file) = verified_file() else { return };
    // Written out under the lock rather than from a copy of the whole map.
    let Some(json) = with_verified(|(map, dirty)| std::mem::take(dirty).then(|| serde_json::to_vec(map).ok()).flatten()) else { return };
    let tmp = file.with_extension("json.tmp");
    let _ = tokio::fs::create_dir_all(file.parent().unwrap_or(&file)).await;
    if tokio::fs::write(&tmp, json).await.is_ok() {
        let _ = tokio::fs::rename(&tmp, &file).await;
    }
}

/// Files smaller than this are hashed again each time: reading one costs less than keeping track of it.
const REMEMBER_FROM: u64 = 64 * 1024;

/// A file's hash: the remembered one if the file was hashed (or checked against that hash) before and has had the
/// same size and modified time since, read from the file otherwise (blocking). Remembered hashes reach the disk
/// with the next `save_verified`.
pub fn file_hash_now(path: &Path, algo: HashAlgo) -> std::io::Result<String> {
    // The stamp from before reading: a file that changes while it's read gets read again next time.
    let meta = std::fs::metadata(path)?;
    if let Some(known) = recall(path, &meta, hash_name(algo)) {
        return Ok(known);
    }
    let hash = hash_read(std::fs::File::open(path)?, algo)?;
    if meta.len() >= REMEMBER_FROM {
        remember(path, &meta, hash_name(algo), &hash);
    }
    Ok(hash)
}

/// [`file_hash_now`] without blocking the async runtime.
pub async fn file_hash(path: &Path, algo: HashAlgo) -> Result<String> {
    let path = path.to_path_buf();
    Ok(tokio::task::spawn_blocking(move || file_hash_now(&path, algo)).await??)
}

/// The CurseForge fingerprint remembered for a file that hasn't changed since (`meta` is the file's now).
pub fn known_fingerprint(path: &Path, meta: &std::fs::Metadata) -> Option<u32> {
    recall(path, meta, FINGERPRINT)?.parse().ok()
}

/// Remembers a file's CurseForge fingerprint; `meta` is the file's from before it was read.
pub fn learn_fingerprint(path: &Path, meta: &std::fs::Metadata, print: u32) {
    remember(path, meta, FINGERPRINT, &print.to_string());
}

/// The file at `from` went to `to` as it is (a rename): what's known of it goes along.
pub fn file_moved(from: &Path, to: &Path) {
    with_verified(|(map, dirty)| {
        if let Some(v) = map.remove(&key(from)) {
            map.insert(key(to), v);
            *dirty = true;
        }
    });
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

/// Files being downloaded right now, so two downloads to the same place (two updates that both bring in a new
/// library) take turns instead of writing one `.part` file together.
static IN_FLIGHT: Mutex<Option<HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>> = Mutex::new(None);

/// Folders made during one `download_all`, so each is made once however many files go into it.
type Made = Mutex<HashSet<PathBuf>>;

/// Downloads one file to `dest` (via `<dest>.part`), verifying size and hash. Retries 3 times.
pub async fn download_file(item: &DownloadItem) -> Result<()> {
    download_one(item, None).await
}

async fn download_one(item: &DownloadItem, made: Option<&Made>) -> Result<()> {
    if item.url.trim().is_empty() {
        let file = item.dest.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        return Err(err_tr!("download.noAddress", "No download address for {file}", file = file));
    }
    if let Some(parent) = item.dest.parent() {
        if !made.is_some_and(|m| m.lock().unwrap().contains(parent)) {
            tokio::fs::create_dir_all(parent).await?;
            if let Some(m) = made {
                m.lock().unwrap().insert(parent.to_path_buf());
            }
        }
    }
    let lock = IN_FLIGHT.lock().unwrap().get_or_insert_with(HashMap::new).entry(item.dest.clone()).or_default().clone();
    let result = match lock.try_lock() {
        Ok(_turn) => download_file_now(item).await,
        Err(_) => {
            let _turn = lock.lock().await;
            // Someone else was downloading to this place. A file that can be checked (by hash or size) and is
            // right now isn't fetched a second time.
            if (item.hash.is_some() || item.size.is_some()) && is_valid(item).await {
                Ok(())
            } else {
                download_file_now(item).await
            }
        }
    };
    // The last one out takes the entry away.
    let mut map = IN_FLIGHT.lock().unwrap();
    if let Some(m) = map.as_mut() {
        if m.get(&item.dest).is_some_and(|l| Arc::strong_count(l) <= 2) {
            m.remove(&item.dest);
        }
    }
    result
}

async fn download_file_now(item: &DownloadItem) -> Result<()> {
    let tmp = PathBuf::from(format!("{}.part", item.dest.display()));
    // Left by a run that died: nothing says it belongs to this file.
    let _ = tokio::fs::remove_file(&tmp).await;
    let mut last: Option<anyhow::Error> = None;
    // How much of the file `tmp` holds.
    let mut have = 0;
    let mut attempt = 0;
    while attempt < 3 {
        match try_download(item, &tmp, have).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                let not_found = status_of(&e) == Some(404);
                last = Some(e);
                if not_found {
                    let _ = tokio::fs::remove_file(&tmp).await;
                    break;
                }
                // A cut-off connection that got further doesn't use up a try: some hosts (Cuberite's) drop
                // every connection after a couple of minutes, so a slow file only ever finishes in pieces.
                let now = tokio::fs::metadata(&tmp).await.map_or(0, |m| m.len());
                if now > have {
                    have = now;
                    attempt = 0;
                } else {
                    let _ = tokio::fs::remove_file(&tmp).await;
                    have = 0;
                    attempt += 1;
                }
                tokio::time::sleep(Duration::from_millis(500 * (attempt + 1))).await;
            }
        }
    }
    Err(last.unwrap_or_else(|| err_tr!("download.failed", "Download failed: {url}", url = item.url)))
}

/// Files up to this size are received whole and then checked, written and put in place in one go: for the
/// thousands of small files among a game's assets, creating each one, writing it, reading it back to hash it and
/// renaming it as separate steps took several times longer (3,000 files from a server on the same computer: about
/// 5 s that way, about 1 s this way). Small enough that the files in flight together stay a few megabytes.
const WHOLE_IN_MEMORY: u64 = 256 * 1024;

/// One try at `item`, into `tmp`, which holds the first `have` bytes from earlier tries.
async fn try_download(item: &DownloadItem, tmp: &Path, have: u64) -> Result<()> {
    use std::io::Write;
    let mut request = http().get(&item.url);
    if have > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={have}-")).header(reqwest::header::ACCEPT_ENCODING, "identity");
    }
    let mut res = unless_stalled(&item.url, request.send()).await?;
    if !res.status().is_success() {
        return Err(HttpError { status: res.status().as_u16(), url: item.url.clone(), body: String::new() }.into());
    }
    let resumed = res.status() == reqwest::StatusCode::PARTIAL_CONTENT;
    if resumed && range_start(res.headers()) != Some(have) {
        // Not the rest of our file: start it again on the next try.
        let _ = tokio::fs::remove_file(tmp).await;
        bail!("The host sent the wrong part of {}", item.url);
    }
    // Bytes that were unzipped on the way can't be matched to a position in the file on the host.
    let can_resume = resumed || res.content_length().is_some();

    // Either way the pieces are used as they arrive, never copied into a second buffer: this used to hold every
    // file in flight in memory twice (once as received, once in the file's own write buffer).
    if !resumed && res.content_length().is_some_and(|n| n <= WHOLE_IN_MEMORY) {
        let mut pieces = Vec::new();
        loop {
            match unless_stalled(&item.url, res.chunk()).await {
                Ok(Some(piece)) => pieces.push(piece),
                Ok(None) => break,
                Err(e) => {
                    // What arrived stays for the next try to go on from, as it would in a file being written.
                    if pieces.iter().any(|p| !p.is_empty()) {
                        let _ = tokio::fs::write(tmp, pieces.concat()).await;
                    }
                    return Err(e);
                }
            }
        }
        // Checked, written, put in place and looked at in one go, off the async runtime.
        let (tmp, dest, hash) = (tmp.to_path_buf(), item.dest.clone(), item.hash.clone());
        let placed = tokio::task::spawn_blocking(move || -> std::io::Result<Option<Option<std::fs::Metadata>>> {
            if let Some(h) = &hash {
                let mut running = RunningHash::new(h.algo);
                for p in &pieces {
                    running.update(p);
                }
                if !running.finish().eq_ignore_ascii_case(&h.value) {
                    // A whole file that's wrong isn't progress: gone, so the next try counts as one.
                    let _ = std::fs::remove_file(&tmp);
                    return Ok(None);
                }
            }
            let mut file = std::fs::File::create(&tmp)?;
            for p in &pieces {
                file.write_all(p)?;
            }
            drop(file);
            std::fs::rename(&tmp, &dest)?;
            Ok(Some(std::fs::metadata(&dest).ok()))
        })
        .await??;
        let Some(meta) = placed else { return Err(anyhow!("Checksum mismatch for {}", item.url)) };
        remember_download(item, meta);
        return Ok(());
    }

    let file = {
        let tmp = tmp.to_path_buf();
        tokio::task::spawn_blocking(move || match resumed {
            true => std::fs::OpenOptions::new().append(true).open(&tmp),
            false => std::fs::File::create(&tmp),
        })
        .await??
    };
    // A file that starts here is hashed as it arrives; the rest of an earlier try is read back once it's whole.
    let hashing = item.hash.as_ref().filter(|_| !resumed).map(|h| RunningHash::new(h.algo));
    // The file and its hash so far are either here or with the piece being written on a blocking thread. That
    // thread gets the piece itself, and the next piece is read from the network while it writes.
    let mut idle = Some((file, hashing));
    let mut busy: Option<tokio::task::JoinHandle<std::io::Result<(std::fs::File, Option<RunningHash>)>>> = None;
    loop {
        match unless_stalled(&item.url, res.chunk()).await {
            Ok(Some(piece)) => {
                let (mut file, mut hashing) = match busy.take() {
                    Some(write) => write.await??,
                    None => idle.take().expect("the file is here when nothing is being written"),
                };
                busy = Some(tokio::task::spawn_blocking(move || {
                    if let Some(h) = hashing.as_mut() {
                        h.update(&piece);
                    }
                    file.write_all(&piece)?;
                    Ok((file, hashing))
                }));
            }
            Ok(None) => break,
            Err(e) => {
                // What arrived is on disk (and the file closed) before the next try looks at it.
                if let Some(write) = busy.take() {
                    let _ = write.await;
                }
                drop(idle);
                if !can_resume {
                    let _ = tokio::fs::remove_file(tmp).await;
                }
                return Err(e);
            }
        }
    }
    let (file, hashing) = match busy.take() {
        Some(write) => write.await??,
        None => idle.take().expect("the file is here when nothing is being written"),
    };
    let got = match (&item.hash, hashing) {
        (Some(_), Some(h)) => Some(h.finish()),
        (Some(h), None) => {
            drop(file);
            let got = hash_file(tmp, h.algo).await?;
            return finish_download(item, tmp, None, Some(got)).await;
        }
        (None, _) => None,
    };
    finish_download(item, tmp, Some(file), got).await
}

/// Closes a finished `.part` file, checks the hash it was found to have and puts it in place.
async fn finish_download(item: &DownloadItem, tmp: &Path, file: Option<std::fs::File>, got: Option<String>) -> Result<()> {
    let wrong = match (&item.hash, &got) {
        (Some(h), Some(got)) => !got.eq_ignore_ascii_case(&h.value),
        _ => false,
    };
    let (tmp, dest) = (tmp.to_path_buf(), item.dest.clone());
    // Closing a file just written can take a while (virus scanners look at it then), so off the async runtime.
    let meta = tokio::task::spawn_blocking(move || -> std::io::Result<Option<Option<std::fs::Metadata>>> {
        drop(file);
        if wrong {
            // A whole file that's wrong isn't progress: gone, so the next try counts as one.
            let _ = std::fs::remove_file(&tmp);
            return Ok(None);
        }
        std::fs::rename(&tmp, &dest)?;
        Ok(Some(std::fs::metadata(&dest).ok()))
    })
    .await??;
    let Some(meta) = meta else { return Err(anyhow!("Checksum mismatch for {}", item.url)) };
    remember_download(item, meta);
    Ok(())
}

/// Remembers that a file just downloaded matched its hash. Not for files trusted by their size (assets): nothing
/// ever asks about those, and there are thousands of them.
fn remember_download(item: &DownloadItem, meta: Option<std::fs::Metadata>) {
    if let (Some(h), Some(meta), false) = (&item.hash, meta, item.trust_size) {
        remember(&item.dest, &meta, hash_name(h.algo), &h.value);
    }
}

/// Where a 206 answer's bytes start: `Content-Range: bytes 1000-1999/2000` → 1000.
fn range_start(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    let value = headers.get(reqwest::header::CONTENT_RANGE)?.to_str().ok()?;
    value.trim().strip_prefix("bytes ")?.split('-').next()?.trim().parse().ok()
}

/// Files downloading on the side while something else goes on (`download_in_background`). Dropped (the task that
/// started it was cancelled or failed), the downloads stop.
pub struct BackgroundDownload(Option<tokio::task::JoinHandle<Result<()>>>);

impl BackgroundDownload {
    /// Waits for the downloads to end and says how they went.
    pub async fn finish(mut self) -> Result<()> {
        match self.0.take() {
            Some(handle) => handle.await?,
            None => Ok(()),
        }
    }
}

impl Drop for BackgroundDownload {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.abort();
        }
    }
}

/// `download_all` on the side, without progress. Another `download_all` of the same files meanwhile doesn't fetch
/// any twice: a file being downloaded is waited for and then found in place.
pub fn download_in_background(items: Vec<DownloadItem>, concurrency: usize) -> BackgroundDownload {
    BackgroundDownload(Some(tokio::spawn(async move { download_all(items, concurrency, |_, _| {}).await })))
}

/// `download_in_background` of the files `listing` comes up with, once it has.
pub fn download_in_background_after(listing: impl std::future::Future<Output = Result<Vec<DownloadItem>>> + Send + 'static, concurrency: usize) -> BackgroundDownload {
    BackgroundDownload(Some(tokio::spawn(async move { download_all(listing.await?, concurrency, |_, _| {}).await })))
}

/// Downloads every missing or corrupt file, `concurrency` at a time, reporting (done, total).
pub async fn download_all<F>(items: Vec<DownloadItem>, concurrency: usize, on_progress: F) -> Result<()>
where
    F: Fn(usize, usize) + Send + Sync,
{
    // De-duplicate by destination.
    let mut seen = HashSet::new();
    let unique: Vec<DownloadItem> = items.into_iter().filter(|i| seen.insert(i.dest.clone())).collect();
    let total = unique.len();
    on_progress(0, total);
    let failed = Arc::new(AtomicBool::new(false));
    let made: Arc<Made> = Arc::new(Mutex::new(HashSet::new()));

    // Each file is a task of its own, so the files in flight are received, hashed and written on several cores at
    // once. Dropping this future (Cancel) drops the set, which stops them all.
    let mut waiting = unique.into_iter();
    let mut running = tokio::task::JoinSet::new();
    let mut done = 0;
    let mut first_error: Option<anyhow::Error> = None;
    loop {
        // After a failure the files not yet started are left alone.
        while running.len() < concurrency.max(1) && !failed.load(Ordering::Relaxed) {
            let Some(item) = waiting.next() else { break };
            let (failed, made) = (failed.clone(), made.clone());
            running.spawn(async move {
                if !is_valid(&item).await {
                    if let Err(e) = download_one(&item, Some(&*made)).await {
                        failed.store(true, Ordering::Relaxed);
                        return Err(e);
                    }
                }
                Ok(())
            });
        }
        let Some(joined) = running.join_next().await else { break };
        match joined.map_err(anyhow::Error::from).and_then(|r| r) {
            Ok(()) => {
                done += 1;
                on_progress(done, total);
            }
            Err(e) => {
                failed.store(true, Ordering::Relaxed);
                first_error.get_or_insert(e);
            }
        }
    }
    save_verified().await;
    first_error.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_where_a_partial_answer_starts() {
        let mut headers = reqwest::header::HeaderMap::new();
        assert_eq!(range_start(&headers), None);
        headers.insert(reqwest::header::CONTENT_RANGE, "bytes 1000-1999/2000".parse().unwrap());
        assert_eq!(range_start(&headers), Some(1000));
        headers.insert(reqwest::header::CONTENT_RANGE, "bytes */2000".parse().unwrap());
        assert_eq!(range_start(&headers), None);
    }

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

    #[test]
    fn running_hash_matches_hashing_at_once() {
        let data: Vec<u8> = (0..200_000u32).map(|i| (i * 31 % 251) as u8).collect();
        for algo in [HashAlgo::Md5, HashAlgo::Sha1, HashAlgo::Sha256, HashAlgo::Sha512] {
            let mut h = RunningHash::new(algo);
            for piece in data.chunks(7919) {
                h.update(piece);
            }
            assert_eq!(h.finish(), hash_bytes(&data, algo));
            assert_eq!(hash_read(&data[..], algo).unwrap(), hash_bytes(&data, algo));
        }
        assert_eq!(hash_bytes(b"hello world", HashAlgo::Sha1), "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed");
    }

    #[test]
    fn remembers_hashes_until_the_file_changes() {
        let dir = std::env::temp_dir().join(format!("resin-known-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mod.jar");
        let big = vec![7u8; REMEMBER_FROM as usize + 1];
        std::fs::write(&path, &big).unwrap();
        let sha1 = hash_bytes(&big, HashAlgo::Sha1);
        let meta = std::fs::metadata(&path).unwrap();
        assert_eq!(recall(&path, &meta, "sha1"), None);

        // Hashed once, then remembered: for that algorithm only.
        assert_eq!(file_hash_now(&path, HashAlgo::Sha1).unwrap(), sha1);
        assert_eq!(recall(&path, &meta, "sha1").as_deref(), Some(sha1.as_str()));
        assert_eq!(recall(&path, &meta, "sha512"), None);
        // A second algorithm and a fingerprint sit next to the first; none pushes another out.
        assert_eq!(file_hash_now(&path, HashAlgo::Sha512).unwrap(), hash_bytes(&big, HashAlgo::Sha512));
        learn_fingerprint(&path, &meta, 4_000_000_001);
        assert_eq!(known_fingerprint(&path, &meta), Some(4_000_000_001));
        assert_eq!(recall(&path, &meta, "sha1").as_deref(), Some(sha1.as_str()));
        assert_eq!(recall(&path, &meta, "sha512"), Some(hash_bytes(&big, HashAlgo::Sha512)));
        // What a download check remembers is found by `file_hash` too, and the other way round.
        assert!(verified_unchanged(&path, &meta, &Hash::sha1(sha1.to_uppercase())));
        assert!(!verified_unchanged(&path, &meta, &Hash::sha1("0".repeat(40))));

        // A move takes what's known along.
        let moved = dir.join("moved.jar");
        std::fs::rename(&path, &moved).unwrap();
        file_moved(&path, &moved);
        let meta = std::fs::metadata(&moved).unwrap();
        assert_eq!(recall(&moved, &meta, "sha1").as_deref(), Some(sha1.as_str()));
        assert_eq!(known_fingerprint(&moved, &meta), Some(4_000_000_001));

        // Other bytes of the same size: the modified time moves, so everything known is dropped and it's read again.
        std::thread::sleep(Duration::from_millis(20));
        let other = vec![8u8; big.len()];
        std::fs::write(&moved, &other).unwrap();
        let meta = std::fs::metadata(&moved).unwrap();
        assert_eq!(recall(&moved, &meta, "sha1"), None);
        assert_eq!(file_hash_now(&moved, HashAlgo::Sha1).unwrap(), hash_bytes(&other, HashAlgo::Sha1));
        assert_eq!(known_fingerprint(&moved, &meta), None);
        assert_eq!(recall(&moved, &meta, "sha512"), None);

        // Small files are read every time and never kept.
        let small = dir.join("options.txt");
        std::fs::write(&small, b"fov:70").unwrap();
        assert_eq!(file_hash_now(&small, HashAlgo::Sha1).unwrap(), hash_bytes(b"fov:70", HashAlgo::Sha1));
        assert_eq!(recall(&small, &std::fs::metadata(&small).unwrap(), "sha1"), None);
        assert!(file_hash_now(&dir.join("missing.jar"), HashAlgo::Sha1).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn old_cache_files_still_load() {
        // As launchers up to 1.6.1 wrote it.
        let old = r#"{"C:\\libs\\a.jar":{"size":11,"mtime":1700000000000000000,"hash":"2aae6c35c94fcfb415dbe95f408b9ce91ee846ed"}}"#;
        let map: HashMap<String, Verified> = serde_json::from_str(old).unwrap();
        let v = &map["C:\\libs\\a.jar"];
        assert_eq!((v.get("sha1"), v.get("sha256"), v.get(FINGERPRINT)), (Some("2aae6c35c94fcfb415dbe95f408b9ce91ee846ed"), None, None));
        // Written back without the new field while there's nothing in it, so an older launcher reads it as before.
        assert_eq!(serde_json::to_string(&map).unwrap(), old);
    }
}
