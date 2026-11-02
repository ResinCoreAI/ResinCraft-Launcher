//! Copying a stopped server: to a new server on this PC, or to a zip that any ResinCraft can import.
//!
//! A clone or an imported zip is a new server: new id, a free port, none of the original's relay name, domain, server
//! list entry, Discord or Cloudflare tokens (those live outside the folder, by id, or in server.json, which is rebuilt).
//!
//! eula.txt is never copied, written or kept, whatever a zip holds: agreeing is the player's decision, per server, in
//! the app (see `store::accept_eula`). Like a modpack's eula.txt in create.rs; `migrate` keeps the file of a folder the
//! player imports from their own disk, a zip may come from anyone.
//!
//! An export leaves out `server.json` and writes `resincraft-server.json` with only what's safe to hand to a stranger:
//! no Java path, Java arguments or pack flags (a zip must not choose how Java starts), no secrets. Passwords and tokens
//! in config files are blanked and key files left out, by name: see `redact` and `secret_file`.
//!
//! None of this is on `remote.rs`'s lists: the web console and other PCs can't copy a server out or put one in.

use std::collections::BTreeSet;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use base64::Engine;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::content::index::{extract_where, open_zip, safe_name};
use crate::core::events::{list_changed, Task};
use crate::core::fsutil::{now_ms, safe_join};
use crate::core::settings::total_memory_mb;
use crate::core::types::{ModpackRef, PendingDownload, ServerInfo, ServerKind, ServerLaunch, ServerMeta, TunnelMode};
use crate::core::{shell, storage};
use crate::migrate::copy::{copy_listed, Listing, Skipped};

use super::{create, installers, plugins, properties, runtime, schedule, store};

/// The description at the root of an exported zip.
const MANIFEST: &str = "resincraft-server.json";
const FORMAT: u32 = 1;
/// Config files bigger than this aren't looked through for secrets (nobody writes those by hand).
const MAX_CONFIG: u64 = 4 << 20;
const STORED: &[&str] = &["jar", "zip", "gz", "mca", "mcc", "png", "phar", "xz", "7z"];
const CONFIGS: &[&str] = &["yml", "yaml", "toml", "properties", "conf", "cfg", "ini", "json", "hocon"];

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneOpts {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default = "yes")]
    pub worlds: bool,
}

impl Default for CloneOpts {
    fn default() -> Self {
        Self { name: None, worlds: true }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOpts {
    #[serde(default = "yes")]
    pub worlds: bool,
    #[serde(default)]
    pub logs: bool,
    #[serde(default)]
    pub backups: bool,
    #[serde(default)]
    pub caches: bool,
    /// Where to write it (the panel, which has no file dialog). None asks.
    #[serde(default)]
    pub path: Option<String>,
}

impl Default for ExportOpts {
    fn default() -> Self {
        Self { worlds: true, logs: false, backups: false, caches: false, path: None }
    }
}

/// A new server, and what couldn't come along.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transferred {
    pub server: ServerInfo,
    pub skipped: Vec<Skipped>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exported {
    pub path: String,
    pub bytes: u64,
    pub skipped: Vec<Skipped>,
}

/// `resincraft-server.json`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    format: u32,
    name: String,
    kind: ServerKind,
    mc_version: String,
    #[serde(default)]
    loader_version: Option<String>,
    memory_mb: u32,
    #[serde(default)]
    launch: ServerLaunch,
    #[serde(default = "yes")]
    optimized_flags: bool,
    #[serde(default)]
    modpack: Option<ModpackRef>,
    #[serde(default)]
    built_pack: bool,
    #[serde(default)]
    pending_downloads: Vec<PendingDownload>,
}

// ---------- One thing at a time ----------

static BUSY: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

/// The server is being cloned or exported: it mustn't start or be deleted meanwhile.
pub fn busy(id: &str) -> bool {
    BUSY.lock().unwrap().contains(id)
}

struct Hold(String);

impl Drop for Hold {
    fn drop(&mut self) {
        BUSY.lock().unwrap().remove(&self.0);
    }
}

/// Claims a stopped server, marked before it's checked so a Start in between sees the mark.
fn hold(id: &str) -> Result<Hold> {
    storage::ensure_not_moving()?;
    if !BUSY.lock().unwrap().insert(id.to_string()) {
        bail!("This server is already being copied. Try again when that's done.");
    }
    let held = Hold(id.to_string());
    if runtime::is_running(id) {
        bail!("Stop the server first");
    }
    if schedule::busy(id) {
        bail!("A scheduled task is running on this server. Try again when it's done.");
    }
    if crate::content::packswitch::is_switching(&format!("server:{id}")) {
        crate::bail_tr!("packswitch.wait", "Wait for the modpack version switch to finish first");
    }
    Ok(held)
}

// ---------- What comes along ----------

#[derive(Clone, Copy, Debug, PartialEq)]
enum Purpose {
    Clone,
    Export,
    Import,
}

#[derive(Clone, Debug)]
struct Rules {
    purpose: Purpose,
    /// Top-level world folders, lower case; left out unless `keep_worlds`.
    worlds: BTreeSet<String>,
    keep_worlds: bool,
    logs: bool,
    backups: bool,
    caches: bool,
}

impl Rules {
    /// What a zip brings in: everything but what's always left out.
    fn import() -> Self {
        Self { purpose: Purpose::Import, worlds: BTreeSet::new(), keep_worlds: true, logs: true, backups: true, caches: true }
    }
}

/// A path as Windows would open it: lower case, no empty or `.` parts, no trailing dots or spaces.
fn parts(rel: &str) -> Vec<String> {
    rel.split(['/', '\\']).map(|p| p.trim_end_matches(['.', ' ']).to_lowercase()).filter(|p| !p.is_empty()).collect()
}

/// Files that are a key or a token whatever folder they're in (a proxy's forwarding secret, Floodgate's key…).
fn secret_file(name: &str) -> bool {
    [".secret", ".pem", ".key", ".token", ".p12", ".jks", ".keystore"].iter().any(|e| name.ends_with(e)) || matches!(name, ".env" | "token.txt")
}

fn is_log(top: &str, last: &str) -> bool {
    matches!(top, "logs" | "crash-reports" | "debug") || (top == last && last.starts_with("hs_err_pid"))
}

fn is_cache(top: &str) -> bool {
    matches!(top, "cache" | ".cache" | ".fabric" | ".quilt" | "usercache.json" | ".console_history")
}

/// True when `rel` (relative to the server folder) stays behind.
fn left_out(rules: &Rules, rel: &str) -> bool {
    let parts = parts(rel);
    let (Some(top), Some(last)) = (parts.first().map(String::as_str), parts.last().map(String::as_str)) else { return true };
    // The agreement, ResinCraft's own files about the original, and files of a server caught mid-work.
    if matches!(top, "eula.txt" | "server.json" | "schedule.json" | ".horizon-new" | ".pack-switch") || top == MANIFEST {
        return true;
    }
    if last == "session.lock" || last.ends_with(".part") || (top == "plugins" && parts.get(1).is_some_and(|p| p == "bstats")) {
        return true;
    }
    if rules.purpose != Purpose::Clone && (matches!(top, ".resincraft" | "instance-sync.json") || secret_file(last)) {
        return true;
    }
    (!rules.logs && is_log(top, last)) || (!rules.backups && top == "backups") || (!rules.caches && is_cache(top)) || (!rules.keep_worlds && rules.worlds.contains(top))
}

/// World folders in a server folder: the level's (and Bukkit's _nether and _the_end), Bedrock's `worlds`, and any
/// other folder that holds a level.dat (Multiverse and the like).
fn world_folders(dir: &Path, level: &str) -> BTreeSet<String> {
    let level = level.trim().to_lowercase();
    let mut out: BTreeSet<String> = ["", "_nether", "_the_end"].iter().map(|s| format!("{level}{s}")).collect();
    out.insert("worlds".into());
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if e.path().join("level.dat").is_file() {
            out.insert(e.file_name().to_string_lossy().to_lowercase());
        }
    }
    out
}

async fn level_name(dir: &Path) -> String {
    properties::get_value(dir, "level-name").await.filter(|l| !l.trim().is_empty()).unwrap_or_else(|| "world".into())
}

/// Everything that comes along. Unlike `migrate::copy::list`, links aren't followed: a link in a server folder can
/// point anywhere on this PC, and that must not end up in a zip.
fn list_files(root: &Path, rules: &Rules) -> Listing {
    let mut out = Listing { files: Vec::new(), skipped: Vec::new(), bytes: 0 };
    walk(root, Path::new(""), rules, &mut out);
    out
}

fn walk(dir: &Path, rel: &Path, rules: &Rules, out: &mut Listing) {
    let skip = |rel: &Path, why: String| Skipped { path: rel.to_string_lossy().replace('\\', "/"), reason: why.into() };
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => return out.skipped.push(skip(rel, format!("Couldn't read it: {e}"))),
    };
    for entry in entries.flatten() {
        let child = rel.join(entry.file_name());
        if left_out(rules, &child.to_string_lossy()) {
            continue;
        }
        match (entry.file_type(), entry.metadata()) {
            (Ok(t), _) if t.is_symlink() => out.skipped.push(skip(&child, "It's a link to somewhere else".into())),
            (Ok(t), _) if t.is_dir() => walk(&entry.path(), &child, rules, out),
            (Ok(_), Ok(m)) => {
                out.bytes += m.len();
                out.files.push((child, m.len()));
            }
            (Err(e), _) | (_, Err(e)) => out.skipped.push(skip(&child, format!("Couldn't read it: {e}"))),
        }
    }
}

fn enough_room(path: &Path, bytes: u64) -> Result<()> {
    if storage::free_bytes(path).is_some_and(|free| free < bytes) {
        bail!("There isn't enough free disk space ({} MB needed)", bytes >> 20);
    }
    Ok(())
}

// ---------- Secrets in config files ----------

static SECRET_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?imR)^([ \t]*"?[\w.\-]*(?:token|password|passwd|secret|api[-_.]?key)"?[ \t]*[:=][ \t]*)(.*?)(,?[ \t]*)$"#).expect("valid pattern"));

/// `true`, a number, nothing, or the start of a list or section: a setting about a secret, not the secret.
fn not_a_secret(value: &str) -> bool {
    let v = value.trim().to_lowercase();
    v.is_empty() || matches!(v.as_str(), "true" | "false" | "null" | "~" | "\"\"" | "''") || v.starts_with(['[', '{', '#']) || v.parse::<f64>().is_ok()
}

/// A config file with its passwords and tokens blanked. None when there's nothing to change.
fn redact(name: &str, text: &str) -> Option<String> {
    let blank = if name.ends_with(".properties") { "" } else { "\"\"" };
    let mut changed = false;
    let out = SECRET_LINE.replace_all(text, |c: &regex::Captures| {
        if not_a_secret(&c[2]) {
            c[0].to_string()
        } else {
            changed = true;
            format!("{}{blank}{}", &c[1], &c[3])
        }
    });
    changed.then(|| out.into_owned())
}

fn extension(name: &str) -> &str {
    name.rsplit_once('.').map_or("", |(_, e)| e)
}

/// The file with its secrets blanked, when it's a config file that has any. World folders are only game data.
fn redacted(path: &Path, rel: &str, size: u64, worlds: &BTreeSet<String>) -> Option<Vec<u8>> {
    let lower = rel.to_lowercase();
    if size > MAX_CONFIG || !CONFIGS.contains(&extension(&lower)) || parts(&lower).first().is_some_and(|t| worlds.contains(t)) {
        return None;
    }
    redact(&lower, &std::fs::read_to_string(path).ok()?).map(String::into_bytes)
}

// ---------- The zip ----------

/// Stops the blocking work when dropped: Cancel drops the task's future.
struct StopOnDrop(Arc<AtomicBool>);

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// Writes `listing` and the manifest to `target` (through a .part file, removed on any failure). Returns what couldn't
/// be read, and the zip's size.
fn write_zip(root: &Path, listing: &Listing, worlds: &BTreeSet<String>, manifest: &[u8], target: &Path, stop: &AtomicBool, done: &AtomicU64) -> Result<(Vec<Skipped>, u64)> {
    let tmp = PathBuf::from(format!("{}.part", target.display()));
    let result = write_zip_to(root, listing, worlds, manifest, &tmp, stop, done).and_then(|skipped| {
        let bytes = std::fs::metadata(&tmp)?.len();
        std::fs::rename(&tmp, target)?;
        Ok((skipped, bytes))
    });
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn write_zip_to(root: &Path, listing: &Listing, worlds: &BTreeSet<String>, manifest: &[u8], tmp: &Path, stop: &AtomicBool, done: &AtomicU64) -> Result<Vec<Skipped>> {
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod::{Deflated, Stored};
    let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(std::fs::File::create(tmp)?));
    zip.start_file(MANIFEST, SimpleFileOptions::default().compression_method(Deflated))?;
    zip.write_all(manifest)?;
    let mut skipped = listing.skipped.clone();
    for (rel, size) in &listing.files {
        if stop.load(Ordering::Relaxed) {
            return Err(crate::core::events::cancelled_error());
        }
        let name = rel.to_string_lossy().replace('\\', "/");
        let path = root.join(rel);
        let method = if STORED.contains(&extension(&name.to_lowercase())) { Stored } else { Deflated };
        let options = SimpleFileOptions::default().compression_method(method).large_file(*size >= u32::MAX as u64 / 2);
        if let Some(bytes) = redacted(&path, &name, *size, worlds) {
            zip.start_file(name.as_str(), options)?;
            zip.write_all(&bytes)?;
        } else {
            match std::fs::File::open(&path) {
                Ok(mut file) => {
                    zip.start_file(name.as_str(), options)?;
                    std::io::copy(&mut file, &mut zip)?;
                }
                Err(e) => skipped.push(Skipped { path: name, reason: format!("Couldn't read it: {e}").into() }),
            }
        }
        done.fetch_add(*size, Ordering::Relaxed);
    }
    zip.finish()?.flush()?;
    Ok(skipped)
}

/// A zip after a look at its names: nothing has been written yet.
#[derive(Debug)]
struct ZipPlan {
    /// The folder inside the zip that holds the server ("" at the root, "name/" when it was zipped again in a folder).
    prefix: String,
    manifest: Manifest,
    bytes: u64,
}

/// Refuses the whole zip when one name could land outside the server's folder: an honest export has none.
fn read_plan<R: Read + Seek>(zip: &mut zip::ZipArchive<R>) -> Result<ZipPlan> {
    let not_ours = || anyhow!("This zip isn't a server exported from ResinCraft");
    let names: Vec<String> = zip.file_names().map(str::to_string).collect();
    for name in &names {
        if name.contains(':') || safe_join(Path::new("x"), name).is_err() {
            bail!("This zip has an unsafe path in it: {name}");
        }
    }
    let manifest_name = names
        .iter()
        .filter(|n| n.replace('\\', "/").rsplit('/').next() == Some(MANIFEST) && n.replace('\\', "/").matches('/').count() <= 1)
        .min_by_key(|n| n.len())
        .ok_or_else(not_ours)?
        .clone();
    let prefix = manifest_name[..manifest_name.len() - MANIFEST.len()].to_string();
    let mut text = String::new();
    zip.by_name(&manifest_name)?.take(1 << 20).read_to_string(&mut text)?;
    let manifest: Manifest = serde_json::from_str(&text).map_err(|_| not_ours())?;
    if manifest.format != FORMAT {
        bail!("This zip was made by a newer ResinCraft. Update ResinCraft to import it.");
    }
    if manifest.name.trim().is_empty() || manifest.mc_version.trim().is_empty() {
        return Err(not_ours());
    }
    for file in [&manifest.launch.jar, &manifest.launch.args_file, &manifest.launch.exe].into_iter().flatten() {
        if file.contains(':') || safe_join(Path::new("x"), file).is_err() {
            bail!("This zip has an unsafe path in it: {file}");
        }
    }
    let mut bytes = 0u64;
    for i in 0..zip.len() {
        bytes = bytes.saturating_add(zip.by_index_raw(i)?.size());
    }
    Ok(ZipPlan { prefix, manifest, bytes })
}

/// Where an entry goes, relative to the server folder. None: it stays in the zip.
fn entry_target(rules: &Rules, prefix: &str, name: &str) -> Option<String> {
    let rel = name.strip_prefix(prefix)?;
    (!rel.is_empty() && !left_out(rules, rel)).then(|| rel.to_string())
}

/// Unpacks into `dir`, then makes sure of what must not exist whatever the zip's names were.
fn unpack<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, prefix: &str, dir: &Path, stop: &AtomicBool, progress: impl Fn(f64)) -> Result<()> {
    let rules = Rules::import();
    let total = zip.len().max(1);
    let seen = std::cell::Cell::new(0usize);
    extract_where(zip, dir, |name| {
        seen.set(seen.get() + 1);
        if seen.get() % 64 == 0 {
            progress(seen.get() as f64 / total as f64);
        }
        if stop.load(Ordering::Relaxed) {
            return None;
        }
        entry_target(&rules, prefix, name)
    })?;
    if stop.load(Ordering::Relaxed) {
        return Err(crate::core::events::cancelled_error());
    }
    let _ = std::fs::remove_file(dir.join("eula.txt"));
    Ok(())
}

// ---------- After the copy ----------

/// The new server's ports in server.properties (`start` keeps `server-port` right from then on, but not the others,
/// which would collide with the original's while both run). Software without that file gets its port at start.
async fn write_ports(dir: &Path, kind: ServerKind, port: u16) -> Result<()> {
    if !kind.uses_properties() || !dir.join("server.properties").is_file() {
        return Ok(());
    }
    let mut values = vec![("server-port", port.to_string())];
    if kind.bedrock() {
        values.push(("server-portv6", port.saturating_add(1).to_string()));
    }
    if properties::get_value(dir, "query.port").await.is_some() {
        values.push(("query.port", port.to_string()));
    }
    if properties::get_value(dir, "enable-rcon").await.as_deref() == Some("true") {
        values.push(("rcon.port", port.saturating_add(10).to_string()));
    }
    properties::set_values(dir, &values).await
}

/// Java arguments without the one that agrees to the EULA for the player.
fn without_eula_flag(args: Option<String>) -> Option<String> {
    let kept: Vec<&str> = args.as_deref().unwrap_or_default().split_whitespace().filter(|a| !a.to_lowercase().starts_with("-dcom.mojang.eula.agree=")).collect();
    (!kept.is_empty()).then(|| kept.join(" "))
}

/// "Name copy", or "Name copy 2" and so on when that's taken.
fn copy_name(name: &str, taken: &[String]) -> String {
    let first = format!("{name} copy");
    std::iter::once(first.clone()).chain((2..1000).map(|i| format!("{first} {i}"))).find(|n| !taken.contains(n)).unwrap_or(first)
}

/// Removes a half-made server folder if the work stops (error or Cancel).
struct DirGuard(Option<PathBuf>);

impl Drop for DirGuard {
    fn drop(&mut self) {
        if let Some(dir) = self.0.take() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

// ---------- Clone ----------

/// A stopped server's folder under a new id, for this PC.
pub async fn clone(id: &str, opts: CloneOpts) -> Result<Transferred> {
    let _held = hold(id)?;
    let source = store::get(id).await?;
    let from = store::dir(id)?;
    let taken: Vec<String> = store::list().await?.into_iter().map(|m| m.name).collect();
    let name = opts.name.as_deref().map(str::trim).filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(|| copy_name(&source.name, &taken));
    let rules = Rules {
        purpose: Purpose::Clone,
        worlds: world_folders(&from, &level_name(&from).await),
        keep_worlds: opts.worlds,
        logs: false,
        backups: false,
        caches: false,
    };
    let listing = {
        let (from, rules) = (from.clone(), rules);
        tokio::task::spawn_blocking(move || list_files(&from, &rules)).await?
    };
    enough_room(&store::root(), listing.bytes)?;
    let new_id = store::new_id(&name);
    let to = store::dir(&new_id)?;
    let kind = source.kind;
    let meta = clone_meta(source, &new_id, &name, create::free_port(kind.bedrock()).await);
    let title = format!("Cloning {}", meta.name);
    let (id2, from2) = (id.to_string(), from.clone());
    let (meta, skipped) = Task::run(title, Some(id.to_string()), move |task| async move {
        let mut guard = DirGuard(Some(to.clone()));
        let total = listing.bytes.max(1) as f64;
        let skipped = copy_listed(&from2, &to, &listing, |b| task.update("Copying files…", Some(b as f64 / total))).await?;
        write_ports(&to, kind, meta.port).await?;
        plugins::give_geyser_its_own_port(&to).await?;
        store::save(&meta).await?;
        schedule::copy_to(&id2, &meta.id).await?;
        guard.0 = None;
        Ok((meta, skipped))
    })
    .await?;
    list_changed("servers");
    Ok(Transferred { server: store::info(meta).await, skipped })
}

/// The original's settings; what ties a server to its place in the world starts empty, as on a new server.
fn clone_meta(source: ServerMeta, id: &str, name: &str, port: u16) -> ServerMeta {
    ServerMeta {
        id: id.to_string(),
        name: name.to_string(),
        port,
        jvm_args: without_eula_flag(source.jvm_args.clone()),
        created_at: now_ms(),
        last_started: None,
        join_requests: Default::default(),
        tunnel: TunnelMode::Auto,
        relay_host: None,
        relay_secret: None,
        domain: None,
        dns_auto: false,
        relay_name: None,
        relay_account: None,
        domain_txt: None,
        ..source
    }
}

// ---------- Export ----------

fn manifest_of(m: &ServerMeta) -> Manifest {
    Manifest {
        format: FORMAT,
        name: m.name.clone(),
        kind: m.kind,
        mc_version: m.mc_version.clone(),
        loader_version: m.loader_version.clone(),
        memory_mb: m.memory_mb,
        launch: m.launch.clone(),
        optimized_flags: m.optimized_flags,
        modpack: m.modpack.clone(),
        built_pack: m.built_pack,
        pending_downloads: m.pending_downloads.clone(),
    }
}

/// A stopped server as a zip. None when the player closed the file dialog.
pub async fn export(id: &str, opts: ExportOpts) -> Result<Option<Exported>> {
    let meta = store::get(id).await?;
    let target = match opts.path.clone() {
        Some(p) => PathBuf::from(p),
        None => {
            let default = shell::downloads_dir().join(format!("{}.zip", crate::core::fsutil::slugify(&meta.name)));
            match shell::save_file("Export server", &default, &[("Zip archive", &["zip"])]).await? {
                Some(p) => p,
                None => return Ok(None),
            }
        }
    };
    // After the dialog: it can stay open for as long as the player likes.
    let held = hold(id)?;
    let dir = store::dir(id)?;
    let worlds = world_folders(&dir, &level_name(&dir).await);
    let rules = Rules { purpose: Purpose::Export, worlds: worlds.clone(), keep_worlds: opts.worlds, logs: opts.logs, backups: opts.backups, caches: opts.caches };
    let manifest = serde_json::to_vec_pretty(&manifest_of(&meta))?;
    let exported = Task::run(format!("Exporting {}", meta.name), Some(id.to_string()), move |task| async move {
        let stop = Arc::new(AtomicBool::new(false));
        let _stop_on_drop = StopOnDrop(stop.clone());
        let (done, total) = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicU64::new(1)));
        let (done2, total2) = (done.clone(), total.clone());
        let mut work = tokio::task::spawn_blocking(move || {
            // Held by the thread: after a Cancel it stops at the next file, and only then may the server start.
            let _held = held;
            let listing = list_files(&dir, &rules);
            total2.store(listing.bytes.max(1), Ordering::Relaxed);
            let (skipped, bytes) = write_zip(&dir, &listing, &worlds, &manifest, &target, &stop, &done2)?;
            anyhow::Ok(Exported { path: target.to_string_lossy().into_owned(), bytes, skipped })
        });
        loop {
            tokio::select! {
                result = &mut work => break result?,
                _ = tokio::time::sleep(Duration::from_millis(200)) => {
                    task.update("Writing the zip…", Some(done.load(Ordering::Relaxed) as f64 / total.load(Ordering::Relaxed) as f64));
                }
            }
        }
    })
    .await?;
    Ok(Some(exported))
}

// ---------- Import ----------

/// A server from a zip made by `export`. None when the player closed the file dialog.
pub async fn import(path: Option<String>) -> Result<Option<Transferred>> {
    storage::ensure_not_moving()?;
    let file = match path {
        Some(p) => PathBuf::from(p),
        None => match shell::pick_file("Choose a server zip", &[("Zip archive", &["zip"])]).await? {
            Some(p) => p,
            None => return Ok(None),
        },
    };
    let file2 = file.clone();
    let plan = tokio::task::spawn_blocking(move || read_plan(&mut open_zip(&file2).map_err(|_| anyhow!("That file couldn't be opened as a zip"))?)).await??;
    enough_room(&store::root(), plan.bytes)?;
    let ZipPlan { prefix, manifest, .. } = plan;
    let name: String = manifest.name.trim().chars().take(80).collect();
    let port = create::free_port(manifest.kind.bedrock()).await;
    let id = store::new_id(&name);
    let dir = store::dir(&id)?;
    let meta = Task::run(format!("Importing {name}"), None, move |task| async move {
        let mut guard = DirGuard(Some(dir.clone()));
        tokio::fs::create_dir_all(&dir).await?;
        let stop = Arc::new(AtomicBool::new(false));
        let _stop_on_drop = StopOnDrop(stop.clone());
        let (dir2, t2) = (dir.clone(), task.clone());
        tokio::task::spawn_blocking(move || unpack(&mut open_zip(&file)?, &prefix, &dir2, &stop, |p| t2.update("Unpacking the server…", Some(p)))).await??;
        let mut meta = imported_meta(manifest, &id, &name, port);
        meta.icon_url = tokio::fs::read(dir.join("server-icon.png")).await.ok().map(|b| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b)));
        meta.launch = native_launch(&dir, meta.launch);
        write_ports(&dir, meta.kind, meta.port).await?;
        plugins::give_geyser_its_own_port(&dir).await?;
        store::save(&meta).await?;
        // Nothing to start it with (made by hand, or left out): install the same software fresh, as `migrate` does.
        let l = &meta.launch;
        if ![&l.jar, &l.args_file, &l.exe].iter().any(|p| p.as_deref().is_some_and(|p| dir.join(p).is_file())) {
            meta.launch = Default::default();
            installers::install(&mut meta, &dir, &task).await?;
            store::save(&meta).await?;
        }
        make_runnable(&dir, &meta).await;
        guard.0 = None;
        Ok(meta)
    })
    .await?;
    list_changed("servers");
    Ok(Some(Transferred { server: store::info(meta).await, skipped: Vec::new() }))
}

fn imported_meta(m: Manifest, id: &str, name: &str, port: u16) -> ServerMeta {
    let total = total_memory_mb() as u32;
    // Only files that stay inside the server folder: ResinCraft puts these in place before each start.
    let pending = m.pending_downloads.into_iter().filter(|p| safe_name(&p.file_name).is_ok() && !p.folder.contains(':') && safe_join(Path::new("x"), &p.folder).is_ok()).collect();
    ServerMeta {
        id: id.to_string(),
        name: name.to_string(),
        kind: m.kind,
        mc_version: m.mc_version,
        loader: m.kind.loader(),
        loader_version: m.loader_version,
        memory_mb: m.memory_mb.min((total / 2).max(1024)).max(512),
        port,
        java_path: None,
        jvm_args: None,
        created_at: now_ms(),
        last_started: None,
        modpack: m.modpack,
        icon_url: None,
        launch: m.launch,
        public: true,
        optimized_flags: m.optimized_flags,
        pack_flags: false,
        join_requests: Default::default(),
        tunnel: TunnelMode::Auto,
        relay_host: None,
        relay_secret: None,
        domain: None,
        dns_auto: false,
        relay_name: None,
        relay_account: None,
        domain_txt: None,
        pending_downloads: pending,
        built_pack: m.built_pack,
    }
}

/// Forge's and NeoForge's argument file for this system, when the zip came from the other one.
fn native_launch(dir: &Path, mut launch: ServerLaunch) -> ServerLaunch {
    let (mine, other) = if cfg!(windows) { ("win_args.txt", "unix_args.txt") } else { ("unix_args.txt", "win_args.txt") };
    if let Some(swapped) = launch.args_file.as_deref().and_then(|a| a.strip_suffix(other)).map(|base| format!("{base}{mine}")) {
        if dir.join(&swapped).is_file() {
            launch.args_file = Some(swapped);
        }
    }
    launch
}

/// A zip doesn't keep Linux's "this is a program" mark.
#[cfg(unix)]
async fn make_runnable(dir: &Path, meta: &ServerMeta) {
    use std::os::unix::fs::PermissionsExt;
    if let Some(exe) = &meta.launch.exe {
        let _ = tokio::fs::set_permissions(dir.join(exe), std::fs::Permissions::from_mode(0o755)).await;
    }
}

#[cfg(not(unix))]
async fn make_runnable(_: &Path, _: &ServerMeta) {}

#[cfg(test)]
#[path = "transfer_tests.rs"]
mod tests;
