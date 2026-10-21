//! Moving to another PC: everything the player made, in one `.resincraft` file (a zip), and back.
//!
//! In the file: `resincraft-backup.json` (what's inside), `games/instances/<id>/…` and `games/servers/<id>/…`,
//! optionally the shared game files (`games/libraries|assets|versions|java/…`, which can also re-download), and
//! `launcher/settings.json`, `launcher/quickjoin.json` and `launcher/skins/…`. Never in it: signed-in accounts
//! (Windows encrypts them for this PC only), relay sessions, the playit.gg agent secret, DNS tokens, update state,
//! the chosen storage folder, per-launch natives, and the WebView's own caches.
//!
//! Importing adds to what's there: an instance or server whose id already exists comes in under a new id, the
//! Quick join list and skin library are merged, and the backup's settings are used only when asked (keeping this
//! PC's Java path and watch folders). A failed or cancelled import removes what it added.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use zip::write::SimpleFileOptions;

use super::events::Task;
use super::{paths, settings, shell, storage};
use crate::{bail_tr, err_tr, tr};

const MANIFEST: &str = "resincraft-backup.json";
const FORMAT: &str = "resincraft-backup";
const VERSION: u32 = 1;
/// Shared game files that can come along (they also re-download by themselves).
const GAME_FILES: &[&str] = &["libraries", "assets", "versions", "java"];
/// Already compressed: stored as they are (much faster, and no bigger).
const STORED: &[&str] = &["jar", "zip", "png", "jpg", "jpeg", "webp", "gif", "mca", "mcr", "ogg", "mp3", "gz", "xz", "7z", "mrpack", "dat", "nbt"];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub name: String,
}

/// `resincraft-backup.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    pub app_version: String,
    /// When it was made (Unix ms).
    pub created: i64,
    pub instances: Vec<Item>,
    pub servers: Vec<Item>,
    pub game_files: bool,
    /// Uncompressed size of everything in it.
    pub bytes: u64,
}

/// What a backup file holds, for the import dialog.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub path: String,
    #[serde(flatten)]
    pub manifest: Manifest,
    /// Instance and server ids that already exist here (they come in as copies).
    pub existing: Vec<String>,
    /// Free space where instances are kept (None: unknown).
    pub free_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub path: String,
    pub bytes: u64,
    pub instances: usize,
    pub servers: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub instances: usize,
    pub servers: usize,
    /// Came in under a new id because the id was taken here.
    pub renamed: usize,
    pub skins: usize,
    pub quick_join: usize,
    pub settings: bool,
}

/// One file to put in the backup: where it is, its name in the zip, its size.
type Entry = (PathBuf, String, u64);

/// The name of an instance or server, from its own meta file.
fn name_of(meta: &Path) -> String {
    std::fs::read(meta)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v.get("name").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default()
}

/// Every file under `dir` as zip entries under `prefix`, leaving out top-level names in `skip` (relative to `dir`).
fn walk(dir: &Path, prefix: &str, skip: &[&str], out: &mut Vec<Entry>) {
    fn go(dir: &Path, prefix: &str, skip: &[&str], top: bool, out: &mut Vec<Entry>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if top && skip.contains(&name.as_str()) {
                continue;
            }
            let zip_name = format!("{prefix}/{name}");
            match e.file_type() {
                Ok(t) if t.is_dir() => go(&e.path(), &zip_name, skip, false, out),
                Ok(t) if t.is_file() => out.push((e.path(), zip_name, e.metadata().map(|m| m.len()).unwrap_or(0))),
                _ => {}
            }
        }
    }
    go(dir, prefix, skip, true, out);
}

/// Everything that goes into a backup, and its manifest.
fn collect(game_files: bool) -> (Vec<Entry>, Manifest) {
    let mut files = Vec::new();
    let mut manifest = Manifest {
        format: FORMAT.into(),
        version: VERSION,
        app_version: env!("CARGO_PKG_VERSION").into(),
        created: super::fsutil::now_ms(),
        game_files,
        ..Manifest::default()
    };
    for (kind, dir, meta, list) in [
        ("instances", paths::instances(), "instance.json", &mut manifest.instances),
        ("servers", paths::servers(), "server.json", &mut manifest.servers),
    ] {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut found: Vec<_> = rd.flatten().filter(|e| e.path().join(meta).is_file()).collect();
        found.sort_by_key(|e| e.file_name());
        for e in found {
            let id = e.file_name().to_string_lossy().to_string();
            list.push(Item { name: name_of(&e.path().join(meta)), id: id.clone() });
            // Natives are unpacked again on every launch.
            walk(&e.path(), &format!("games/{kind}/{id}"), &["natives"], &mut files);
        }
    }
    if game_files {
        for folder in GAME_FILES {
            walk(&paths::games().join(folder), &format!("games/{folder}"), &[], &mut files);
        }
    }
    let root = paths::root();
    for file in ["settings.json", "quickjoin.json"] {
        let p = root.join(file);
        if let Ok(m) = std::fs::metadata(&p) {
            files.push((p, format!("launcher/{file}"), m.len()));
        }
    }
    walk(&root.join("skins"), "launcher/skins", &[], &mut files);
    manifest.bytes = files.iter().map(|f| f.2).sum();
    (files, manifest)
}

fn options_for(name: &str, size: u64) -> SimpleFileOptions {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    let opts = if STORED.contains(&ext.as_str()) {
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored)
    } else {
        // Fast deflate: configs and logs shrink a lot even at level 1.
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).compression_level(Some(1))
    };
    opts.large_file(size >= u32::MAX as u64 / 2)
}

/// Writes the zip (on a blocking thread). Stops between files when `stop` is set.
fn write_backup(dest: &Path, files: &[Entry], manifest: &Manifest, stop: &AtomicBool, done: &AtomicU64) -> Result<()> {
    let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(std::fs::File::create(dest)?));
    zip.start_file(MANIFEST, SimpleFileOptions::default())?;
    zip.write_all(&serde_json::to_vec_pretty(manifest)?)?;
    let mut buf = vec![0u8; 1 << 20];
    for (path, name, size) in files {
        if stop.load(Ordering::Relaxed) {
            bail!(super::events::cancelled_error());
        }
        // A file that vanished or is locked since listing is left out (a log being written, say).
        let Ok(mut f) = std::fs::File::open(path) else { continue };
        zip.start_file(name.as_str(), options_for(name, *size))?;
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            zip.write_all(&buf[..n])?;
            done.fetch_add(n as u64, Ordering::Relaxed);
        }
    }
    zip.finish()?.flush()?;
    Ok(())
}

/// Sets the flag when the task's work is dropped (Cancel in the task list).
struct StopOnDrop(Arc<AtomicBool>);
impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// Runs `work` on a blocking thread, reporting `done` of `total` bytes as `label` until it ends.
async fn run_blocking<T: Send + 'static>(
    task: &Arc<Task>,
    label: impl Fn() -> crate::core::text::Text,
    total: u64,
    work: impl FnOnce(Arc<AtomicBool>, Arc<AtomicU64>) -> Result<T> + Send + 'static,
) -> Result<T> {
    let stop = Arc::new(AtomicBool::new(false));
    let _stop_on_drop = StopOnDrop(stop.clone());
    let done = Arc::new(AtomicU64::new(0));
    let (s, d) = (stop.clone(), done.clone());
    let mut handle = tokio::task::spawn_blocking(move || work(s, d));
    loop {
        tokio::select! {
            r = &mut handle => return r?,
            _ = tokio::time::sleep(Duration::from_millis(200)) => {
                let d = done.load(Ordering::Relaxed);
                task.update(label(), Some(if total == 0 { 0.0 } else { (d as f64 / total as f64).min(1.0) }));
            }
        }
    }
}

/// Makes a backup. `dest` None: asks where (Downloads by default). None back when the player cancelled the dialog.
pub async fn export(dest: Option<String>, game_files: bool) -> Result<Option<ExportResult>> {
    if let Some(why) = storage::blocker().await {
        bail!(why);
    }
    let dest = match dest {
        Some(d) => PathBuf::from(d),
        None => {
            let day = chrono_day();
            let default = shell::downloads_dir().join(format!("ResinCraft backup {day}.resincraft"));
            let title = tr!("backup.saveTitle", "Save the backup").to_string();
            match shell::save_file(&title, &default, &[("ResinCraft backup", &["resincraft"])]).await? {
                Some(p) => p,
                None => return Ok(None),
            }
        }
    };
    Task::run(tr!("backup.exporting", "Making a backup"), None, move |task| async move {
        let (files, manifest) = tokio::task::spawn_blocking(move || collect(game_files)).await?;
        let total = manifest.bytes;
        let tmp = dest.with_extension("resincraft.part");
        let (t2, files2, m2) = (tmp.clone(), files, manifest.clone());
        let written = run_blocking(&task, || tr!("backup.exportingDetail", "Making a backup"), total, move |stop, done| write_backup(&t2, &files2, &m2, &stop, &done)).await;
        if let Err(e) = written {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(e);
        }
        let _ = tokio::fs::remove_file(&dest).await;
        tokio::fs::rename(&tmp, &dest).await?;
        Ok(Some(ExportResult { path: dest.to_string_lossy().to_string(), bytes: total, instances: manifest.instances.len(), servers: manifest.servers.len() }))
    })
    .await
}

/// "2026-10-21" for file names.
fn chrono_day() -> String {
    let secs = super::fsutil::now_ms() / 1000;
    let z = secs / 86_400 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{m:02}-{d:02}", yoe + era * 400 + i64::from(m <= 2))
}

fn read_manifest(path: &Path) -> Result<Manifest> {
    let file = std::fs::File::open(path).map_err(|e| err_tr!("backup.unreadable", "Couldn't read {file}: {error}", file = path.display().to_string(), error = e.to_string()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| err_tr!("backup.notBackup", "That isn't a ResinCraft backup."))?;
    let mut text = String::new();
    zip.by_name(MANIFEST).map_err(|_| err_tr!("backup.notBackup", "That isn't a ResinCraft backup."))?.read_to_string(&mut text)?;
    let m: Manifest = serde_json::from_str(&text).map_err(|_| err_tr!("backup.notBackup", "That isn't a ResinCraft backup."))?;
    if m.format != FORMAT {
        bail_tr!("backup.notBackup", "That isn't a ResinCraft backup.");
    }
    if m.version > VERSION {
        bail_tr!("backup.tooNew", "This backup was made by a newer ResinCraft ({version}). Update the launcher first.", version = m.app_version.clone());
    }
    Ok(m)
}

/// Asks for a backup file to import. None if cancelled.
pub async fn pick() -> Result<Option<String>> {
    let title = tr!("backup.openTitle", "Choose a ResinCraft backup").to_string();
    Ok(shell::pick_file(&title, &[("ResinCraft backup", &["resincraft"])]).await?.map(|p| p.to_string_lossy().to_string()))
}

/// What's in a backup file, without importing it.
pub async fn inspect(path: &str) -> Result<BackupInfo> {
    let p = PathBuf::from(path);
    let manifest = tokio::task::spawn_blocking(move || read_manifest(&p)).await??;
    let existing = manifest
        .instances
        .iter()
        .filter(|i| paths::instance_dir(&i.id).exists())
        .chain(manifest.servers.iter().filter(|s| paths::servers().join(&s.id).exists()))
        .map(|i| i.id.clone())
        .collect();
    Ok(BackupInfo { path: path.to_string(), manifest, existing, free_bytes: storage::free_bytes(&paths::games()) })
}

/// A free id like `id`, given the ids already taken.
fn fresh_id(id: &str, taken: &dyn Fn(&str) -> bool) -> String {
    (2..).map(|n| format!("{id}-{n}")).find(|c| !taken(c)).unwrap_or_else(|| format!("{id}-copy"))
}

/// Where a zip entry goes, after mapping ids; None for anything this format doesn't put there (or unsafe names).
fn target(name: &str, map: &HashMap<(String, String), String>, games: &Path, staging: &Path) -> Option<PathBuf> {
    let parts: Vec<&str> = name.split('/').collect();
    if parts.iter().any(|p| p.is_empty() || *p == "." || *p == ".." || p.contains(':') || p.contains('\\')) {
        return None;
    }
    let join = |base: PathBuf, rest: &[&str]| rest.iter().fold(base, |b, p| b.join(p));
    match parts.as_slice() {
        ["games", kind @ ("instances" | "servers"), id, rest @ ..] if !rest.is_empty() => {
            let id = map.get(&(kind.to_string(), id.to_string())).cloned().unwrap_or_else(|| id.to_string());
            Some(join(games.join(kind).join(id), rest))
        }
        ["games", folder, rest @ ..] if GAME_FILES.contains(folder) && !rest.is_empty() => Some(join(games.join(folder), rest)),
        // Settings, Quick join and skins are merged after extracting, from a staging folder.
        ["launcher", rest @ ..] if !rest.is_empty() => Some(join(staging.to_path_buf(), rest)),
        _ => None,
    }
}

/// Extracts the backup (on a blocking thread). Game files that exist already are kept. Returns the instance and
/// server folders it made, so a failure can remove them.
fn extract(path: &Path, map: &HashMap<(String, String), String>, staging: &Path, stop: &AtomicBool, done: &AtomicU64, made: &std::sync::Mutex<Vec<PathBuf>>) -> Result<()> {
    let games = paths::games();
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path)?)?;
    let mut buf = vec![0u8; 1 << 20];
    for i in 0..zip.len() {
        if stop.load(Ordering::Relaxed) {
            bail!(super::events::cancelled_error());
        }
        let mut entry = zip.by_index(i)?;
        if entry.is_dir() || entry.name() == MANIFEST {
            continue;
        }
        let Some(dest) = target(entry.name(), map, &games, staging) else { continue };
        let is_item = dest.starts_with(games.join("instances")) || dest.starts_with(games.join("servers"));
        if !is_item && dest.exists() && !dest.starts_with(staging) {
            done.fetch_add(entry.size(), Ordering::Relaxed);
            continue;
        }
        if is_item {
            // The instance/server folder itself, so a failed import can take it away again.
            let rel = dest.strip_prefix(&games).map(Path::to_path_buf).unwrap_or_default();
            let top: PathBuf = rel.components().take(2).collect();
            let item = games.join(top);
            let mut m = made.lock().unwrap();
            if !m.contains(&item) {
                m.push(item);
            }
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::io::BufWriter::new(std::fs::File::create(&dest)?);
        loop {
            let n = entry.read(&mut buf)?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
            done.fetch_add(n as u64, Ordering::Relaxed);
        }
        out.flush()?;
    }
    Ok(())
}

/// Replaces quoted old ids with new ones in a small JSON file (an item's own meta, a server's link to its instance).
fn remap_ids_in(file: &Path, ids: &[(String, String)]) {
    let Ok(mut text) = std::fs::read_to_string(file) else { return };
    let before = text.clone();
    for (old, new) in ids {
        text = text.replace(&format!("\"{old}\""), &format!("\"{new}\""));
    }
    if text != before {
        let _ = std::fs::write(file, text);
    }
}

/// Imports a backup. `use_settings`: take the backup's settings (keeping this PC's Java path and watch folders).
pub async fn import(path: String, use_settings: bool) -> Result<ImportResult> {
    if let Some(why) = storage::blocker().await {
        bail!(why);
    }
    let info = inspect(&path).await?;
    if let Some(free) = info.free_bytes {
        if free < info.manifest.bytes + 200 * 1024 * 1024 {
            bail_tr!(
                "backup.noSpace",
                "Not enough free space: the backup needs {need} and the drive has {free}. Choose another place for your instances in Settings → Storage, or free some space.",
                need = gb(info.manifest.bytes),
                free = gb(free)
            );
        }
    }
    let m = info.manifest;
    // Ids taken here come in under new ones.
    let mut map: HashMap<(String, String), String> = HashMap::new();
    for (kind, list) in [("instances", &m.instances), ("servers", &m.servers)] {
        let base = paths::games().join(kind);
        let mut planned: HashSet<String> = HashSet::new();
        for item in list.iter() {
            if base.join(&item.id).exists() {
                let taken = |c: &str| base.join(c).exists() || planned.contains(c);
                let new = fresh_id(&item.id, &taken);
                planned.insert(new.clone());
                map.insert((kind.to_string(), item.id.clone()), new);
            }
        }
    }
    let staging = paths::cache().join(format!("backup-import-{}", super::fsutil::now_ms()));
    let total = m.bytes;
    Task::run(tr!("backup.importing", "Importing a backup"), None, move |task| async move {
        let made = Arc::new(std::sync::Mutex::new(Vec::<PathBuf>::new()));
        let (p, map2, st, made2) = (PathBuf::from(&path), map.clone(), staging.clone(), made.clone());
        let extracted = run_blocking(&task, || tr!("backup.importingDetail", "Importing a backup"), total, move |stop, done| extract(&p, &map2, &st, &stop, &done, &made2)).await;
        if let Err(e) = extracted {
            // Take back what this import added (game files that came in are only caches).
            for dir in made.lock().unwrap().iter() {
                let _ = std::fs::remove_dir_all(dir);
            }
            let _ = std::fs::remove_dir_all(&staging);
            return Err(e);
        }
        // New ids: fix each moved item's own meta, and links to renamed instances (a server's sync source).
        let instance_ids: Vec<(String, String)> = map.iter().filter(|((k, _), _)| k == "instances").map(|((_, old), new)| (old.clone(), new.clone())).collect();
        for ((kind, old), new) in &map {
            let dir = paths::games().join(kind).join(new);
            let meta = dir.join(if kind == "instances" { "instance.json" } else { "server.json" });
            remap_ids_in(&meta, &[(old.clone(), new.clone())]);
        }
        for s in &m.servers {
            let id = map.get(&("servers".to_string(), s.id.clone())).cloned().unwrap_or_else(|| s.id.clone());
            let dir = paths::servers().join(id);
            for f in ["server.json", "instance-sync.json"] {
                remap_ids_in(&dir.join(f), &instance_ids);
            }
        }
        let skins = merge_skins(&staging.join("skins")).await;
        let quick_join = merge_quick_join(&staging.join("quickjoin.json"), &instance_ids).await;
        let settings_used = use_settings && apply_settings(&staging.join("settings.json")).await;
        let _ = tokio::fs::remove_dir_all(&staging).await;
        Ok(ImportResult { instances: m.instances.len(), servers: m.servers.len(), renamed: map.len(), skins, quick_join, settings: settings_used })
    })
    .await
}

fn gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1e9)
}

/// Adds the backup's skins this library doesn't have yet (same id or same image). Returns how many.
async fn merge_skins(from: &Path) -> usize {
    let dir = paths::root().join("skins");
    let read = |p: PathBuf| async move { super::fsutil::read_json::<Vec<Value>>(&p).await.unwrap_or_default() };
    let theirs = read(from.join("library.json")).await;
    if theirs.is_empty() {
        return 0;
    }
    let mut ours = read(dir.join("library.json")).await;
    let key = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let have: HashSet<String> = ours.iter().flat_map(|v| [key(v, "id"), key(v, "hash")]).filter(|s| !s.is_empty()).collect();
    let mut added = 0;
    let _ = tokio::fs::create_dir_all(&dir).await;
    for v in theirs {
        let id = key(&v, "id");
        if id.is_empty() || have.contains(&id) || have.contains(&key(&v, "hash")) {
            continue;
        }
        if tokio::fs::copy(from.join(format!("{id}.png")), dir.join(format!("{id}.png"))).await.is_ok() {
            ours.push(v);
            added += 1;
        }
    }
    if added > 0 {
        let _ = super::fsutil::write_json(&dir.join("library.json"), &ours).await;
    }
    added
}

/// Adds the backup's Quick join groups and entries this list doesn't have yet. Returns how many entries.
async fn merge_quick_join(from: &Path, instance_ids: &[(String, String)]) -> usize {
    use crate::player::quickjoin::{self, QuickJoin};
    let Some(mut theirs) = super::fsutil::read_json::<QuickJoin>(from).await else { return 0 };
    let renamed: BTreeMap<&str, &str> = instance_ids.iter().map(|(o, n)| (o.as_str(), n.as_str())).collect();
    for e in &mut theirs.entries {
        if let Some(new) = e.instance_id.as_deref().and_then(|i| renamed.get(i)) {
            e.instance_id = Some(new.to_string());
        }
    }
    let mut ours = quickjoin::get().await;
    let groups: HashSet<String> = ours.groups.iter().map(|g| g.id.clone()).collect();
    ours.groups.extend(theirs.groups.into_iter().filter(|g| !groups.contains(&g.id)));
    let have: HashSet<String> = ours.entries.iter().map(|e| quickjoin::address_key(&e.address)).collect();
    let new: Vec<_> = theirs.entries.into_iter().filter(|e| !have.contains(&quickjoin::address_key(&e.address))).collect();
    let added = new.len();
    ours.entries.extend(new);
    if added > 0 {
        let _ = quickjoin::save(ours).await;
    }
    added
}

/// Uses the backup's settings, keeping what belongs to this PC. False when there were none.
async fn apply_settings(from: &Path) -> bool {
    let Some(Value::Object(mut theirs)) = super::fsutil::read_json::<Value>(from).await else { return false };
    for key in ["javaPath", "watchFolders"] {
        theirs.remove(key);
    }
    settings::set(Value::Object(theirs)).await.is_ok()
}

/// `backup:*` requests.
pub async fn handle(channel: &str, args: super::Args) -> Result<Value> {
    use super::router::json;
    match channel {
        "backup:export" => json(export(args.get::<Option<String>>(0)?, args.get::<Option<bool>>(1)?.unwrap_or(false)).await?),
        "backup:pick" => json(pick().await?),
        "backup:inspect" => json(inspect(&args.get::<String>(0)?).await?),
        "backup:import" => json(import(args.get::<String>(0)?, args.get::<Option<bool>>(1)?.unwrap_or(true)).await?),
        _ => bail!("Unknown request: {channel}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_entries_safely() {
        let games = Path::new("G");
        let staging = Path::new("S");
        let mut map = HashMap::new();
        map.insert(("instances".to_string(), "cozy".to_string()), "cozy-2".to_string());
        let t = |n: &str| target(n, &map, games, staging);
        assert_eq!(t("games/instances/cozy/minecraft/options.txt"), Some(games.join("instances").join("cozy-2").join("minecraft").join("options.txt")));
        assert_eq!(t("games/servers/smp/world/level.dat"), Some(games.join("servers").join("smp").join("world").join("level.dat")));
        assert_eq!(t("games/libraries/a/b.jar"), Some(games.join("libraries").join("a").join("b.jar")));
        assert_eq!(t("launcher/skins/x.png"), Some(staging.join("skins").join("x.png")));
        for bad in ["games/instances/../../evil", "games/instances/cozy", "games/cache/x", "launcher/../x", "other/x", "games/instances/a/C:/x", "games/instances//x", "games/instances/a\\..\\x"] {
            assert_eq!(t(bad), None, "{bad}");
        }
    }

    #[test]
    fn fresh_ids_skip_taken_ones() {
        let taken: HashSet<&str> = ["cozy-2", "cozy-3"].into_iter().collect();
        assert_eq!(fresh_id("cozy", &|c| taken.contains(c)), "cozy-4");
        assert_eq!(fresh_id("smp", &|_| false), "smp-2");
    }

    #[test]
    fn stores_compressed_files_as_they_are() {
        let stored = |n: &str| options_for(n, 10);
        // Can't read options back, so compare with what a stored entry is built from.
        assert_eq!(format!("{:?}", stored("mods/a.jar")), format!("{:?}", SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored).large_file(false)));
        assert_ne!(format!("{:?}", stored("config/a.toml")), format!("{:?}", stored("mods/a.jar")));
    }

    #[test]
    fn remaps_quoted_ids_only() {
        let dir = std::env::temp_dir().join(format!("resin-backup-remap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("server.json");
        std::fs::write(&f, r#"{"id":"cozy","name":"cozy server","from":"cozy"}"#).unwrap();
        remap_ids_in(&f, &[("cozy".into(), "cozy-2".into())]);
        assert_eq!(std::fs::read_to_string(&f).unwrap(), r#"{"id":"cozy-2","name":"cozy server","from":"cozy-2"}"#);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn round_trip_through_a_zip() {
        // write_backup + extract against fixture folders (not the real data folder).
        let dir = std::env::temp_dir().join(format!("resin-backup-rt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let src = dir.join("src");
        std::fs::create_dir_all(src.join("minecraft/saves/W")).unwrap();
        std::fs::write(src.join("instance.json"), r#"{"id":"cozy","name":"Cozy"}"#).unwrap();
        std::fs::write(src.join("minecraft/saves/W/level.dat"), [1u8, 2, 3]).unwrap();
        std::fs::write(src.join("minecraft/options.txt"), "fov:90").unwrap();
        let mut files = Vec::new();
        walk(&src, "games/instances/cozy", &["natives"], &mut files);
        assert_eq!(files.len(), 3);
        let manifest = Manifest { format: FORMAT.into(), version: VERSION, instances: vec![Item { id: "cozy".into(), name: "Cozy".into() }], ..Default::default() };
        let zip_path = dir.join("b.resincraft");
        write_backup(&zip_path, &files, &manifest, &AtomicBool::new(false), &AtomicU64::new(0)).unwrap();
        assert_eq!(read_manifest(&zip_path).unwrap().instances[0].name, "Cozy");
        // Extract with a remap, into a fake games folder through `target`.
        let games = dir.join("games");
        let mut map = HashMap::new();
        map.insert(("instances".to_string(), "cozy".to_string()), "cozy-2".to_string());
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&zip_path).unwrap()).unwrap();
        for i in 0..zip.len() {
            let mut e = zip.by_index(i).unwrap();
            let Some(dest) = target(e.name(), &map, &games, &dir.join("staging")) else { continue };
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::io::copy(&mut e, &mut std::fs::File::create(&dest).unwrap()).unwrap();
        }
        let inst = games.join("instances").join("cozy-2");
        assert_eq!(std::fs::read(inst.join("minecraft/saves/W/level.dat")).unwrap(), vec![1, 2, 3]);
        assert_eq!(std::fs::read_to_string(inst.join("minecraft/options.txt")).unwrap(), "fov:90");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn day_is_a_date() {
        let d = chrono_day();
        assert_eq!(d.len(), 10);
        assert!(d.starts_with("20") && d.as_bytes()[4] == b'-' && d.as_bytes()[7] == b'-', "{d}");
    }
}
