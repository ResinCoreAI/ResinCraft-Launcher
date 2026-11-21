//! Copies of Bedrock worlds, made before Minecraft for Windows is switched to another version (and whenever the
//! player asks): a switch removes the installed game first, older versions keep their worlds inside the app's own
//! data, and Windows deletes that data with the app.
//!
//! A backup is a folder under `<ResinCraft's data>\bedrock-backups` holding every worlds folder of the edition
//! (`worlds-0`, `worlds-1`, …) and `backup.json`, which is written last: a folder without it is not a backup that
//! was checked. After copying, every file is read again on both sides and compared (same files, sizes and
//! contents); any difference fails the backup, and whatever asked for it must not go on.
//!
//! Restoring copies worlds back into the worlds folder the installed game uses. A world that is already there is
//! never replaced.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use super::bedrock::Edition;
use crate::{bail_tr, err_tr};

/// The file that marks a checked backup and says what is in it.
const MANIFEST: &str = "backup.json";
/// Files read at the same time while checking.
const CHECK_THREADS: usize = 8;

/// One world in a backup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackedUpWorld {
    /// The world's folder name.
    pub id: String,
    pub name: String,
}

/// One worlds folder that was copied.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackedUpRoot {
    /// Its folder inside the backup ("worlds-0").
    pub folder: String,
    /// Where it was copied from.
    pub source: String,
    pub worlds: Vec<BackedUpWorld>,
}

/// `backup.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub edition: Edition,
    /// When it was made (ms since 1970).
    pub created: i64,
    /// The game version that was installed, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub files: usize,
    pub bytes: u64,
    pub roots: Vec<BackedUpRoot>,
}

/// A backup as the Bedrock page lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    /// The backup's folder name: its id for `bedrock:restore`.
    pub id: String,
    pub edition: Edition,
    pub created: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The worlds' names.
    pub worlds: Vec<String>,
    pub bytes: u64,
    pub path: String,
}

/// What a backup or a restore is doing, for the task's progress and for stopping it.
#[derive(Default)]
pub struct Progress {
    pub done: AtomicU64,
    pub total: AtomicU64,
    /// Set while the copies are being checked.
    pub checking: AtomicBool,
    pub stop: AtomicBool,
}

/// Every file under `root` as its path below it (with `/`) and its size.
fn list_files(root: &Path) -> std::io::Result<BTreeMap<String, u64>> {
    fn walk(dir: &Path, prefix: &str, out: &mut BTreeMap<String, u64>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
            if entry.file_type()?.is_dir() {
                walk(&entry.path(), &rel, out)?;
            } else {
                out.insert(rel, entry.metadata()?.len());
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    walk(root, "", &mut out)?;
    Ok(out)
}

fn rel_path(root: &Path, rel: &str) -> PathBuf {
    rel.split('/').fold(root.to_path_buf(), |p, part| p.join(part))
}

fn hash_file(path: &Path) -> std::io::Result<[u8; 20]> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha1::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(hasher.finalize().into())
}

/// The worlds in a worlds folder: every folder with a `level.dat`, named from its `levelname.txt`.
fn worlds_in(root: &Path) -> Vec<BackedUpWorld> {
    let mut out: Vec<BackedUpWorld> = std::fs::read_dir(root)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().join("level.dat").is_file())
                .map(|e| {
                    let id = e.file_name().to_string_lossy().into_owned();
                    let name = std::fs::read_to_string(e.path().join("levelname.txt")).map(|s| s.trim().to_string()).ok().filter(|s| !s.is_empty()).unwrap_or_else(|| id.clone());
                    BackedUpWorld { id, name }
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Compares two folders file by file: the same files, with the same sizes and contents. Adds the bytes it has
/// read on the `from` side to `progress.done`.
fn check_copy(from: &Path, to: &Path, progress: &Progress) -> Result<()> {
    let (a, b) = (list_files(from)?, list_files(to)?);
    let differs = |file: &str| err_tr!("bedrock.backup.differs", "The backup doesn't match the worlds ({file}). Close Minecraft and try again.", file = file);
    if let Some(file) = a.keys().find(|k| !b.contains_key(*k)).or_else(|| b.keys().find(|k| !a.contains_key(*k))) {
        return Err(differs(file));
    }
    if let Some((file, _)) = a.iter().find(|(k, size)| b.get(*k) != Some(*size)) {
        return Err(differs(file));
    }
    let files: Vec<(&String, &u64)> = a.iter().collect();
    let next = AtomicUsize::new(0);
    let bad: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
    std::thread::scope(|scope| {
        for _ in 0..CHECK_THREADS.min(files.len()).max(1) {
            scope.spawn(|| loop {
                if progress.stop.load(Ordering::Relaxed) || bad.lock().unwrap().is_some() {
                    break;
                }
                let Some((rel, size)) = files.get(next.fetch_add(1, Ordering::Relaxed)) else { break };
                let same = matches!((hash_file(&rel_path(from, rel)), hash_file(&rel_path(to, rel))), (Ok(x), Ok(y)) if x == y);
                if !same {
                    *bad.lock().unwrap() = Some((*rel).clone());
                    break;
                }
                progress.done.fetch_add(**size, Ordering::Relaxed);
            });
        }
    });
    if let Some(file) = bad.into_inner().unwrap() {
        return Err(differs(&file));
    }
    if progress.stop.load(Ordering::Relaxed) {
        return Err(crate::core::events::cancelled_error());
    }
    Ok(())
}

/// Copies every worlds folder in `roots` into `dest` (which must not exist yet), checks the copies and writes
/// `backup.json`. Ok(None) when there are no worlds to copy (nothing is made then). On any failure `dest` is
/// removed again, so a folder with `backup.json` is always a whole, checked backup.
pub fn back_up(roots: &[PathBuf], dest: &Path, edition: Edition, version: Option<String>, created: i64, progress: &Progress) -> Result<Option<Manifest>> {
    let roots: Vec<(&PathBuf, Vec<BackedUpWorld>)> = roots.iter().map(|r| (r, worlds_in(r))).filter(|(_, w)| !w.is_empty()).collect();
    if roots.is_empty() {
        return Ok(None);
    }
    if dest.exists() {
        bail_tr!("bedrock.backup.exists", "The backup folder {folder} already exists.", folder = dest.display().to_string());
    }
    let result = (|| -> Result<Manifest> {
        let mut jobs = Vec::new();
        let mut out_roots = Vec::new();
        for (i, (root, worlds)) in roots.iter().enumerate() {
            let folder = format!("worlds-{i}");
            jobs.extend(crate::core::fsutil::list_copy(root, &dest.join(&folder), &[])?);
            out_roots.push(BackedUpRoot { folder, source: root.to_string_lossy().into_owned(), worlds: worlds.clone() });
        }
        let bytes: u64 = jobs.iter().map(|(_, _, size)| size).sum();
        // Copying, then checking: the bytes are counted once for each.
        progress.total.store(bytes * 2, Ordering::Relaxed);
        if let Some((i, e)) = crate::core::fsutil::copy_many(&jobs, &progress.stop, &progress.done).into_iter().next() {
            if crate::core::fsutil::is_disk_full(&e) {
                bail_tr!("bedrock.backup.diskFull", "There isn't enough free disk space to back up the worlds.");
            }
            bail_tr!("bedrock.backup.copyFailed", "Couldn't copy {file}: {error}", file = jobs[i].0.display().to_string(), error = e.to_string());
        }
        if progress.stop.load(Ordering::Relaxed) {
            return Err(crate::core::events::cancelled_error());
        }
        progress.checking.store(true, Ordering::Relaxed);
        for (root, out) in roots.iter().map(|(r, _)| r).zip(&out_roots) {
            check_copy(root, &dest.join(&out.folder), progress)?;
        }
        let manifest = Manifest { edition, created, version, files: jobs.len(), bytes, roots: out_roots };
        std::fs::write(dest.join(MANIFEST), serde_json::to_vec_pretty(&manifest)?)?;
        Ok(manifest)
    })();
    match result {
        Ok(m) => Ok(Some(m)),
        Err(e) => {
            // Only what this call made: `dest` didn't exist before it.
            let _ = std::fs::remove_dir_all(dest);
            Err(e)
        }
    }
}

/// The edition a backup folder's name starts with ("release-2026-11-20_14-00-00").
fn edition_of(folder: &str) -> Option<Edition> {
    if folder.starts_with("release-") {
        Some(Edition::Release)
    } else if folder.starts_with("preview-") {
        Some(Edition::Preview)
    } else {
        None
    }
}

fn modified_ms(p: &Path) -> i64 {
    std::fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// The worlds folders inside a backup folder, with their worlds. A backup made before a switch has `backup.json`;
/// the ones Uninstall made before (no file) hold the worlds directly, or in numbered folders.
fn roots_of(dir: &Path) -> (Option<Manifest>, Vec<(PathBuf, Vec<BackedUpWorld>)>) {
    if let Some(m) = std::fs::read(dir.join(MANIFEST)).ok().and_then(|b| serde_json::from_slice::<Manifest>(&b).ok()) {
        let roots = m.roots.iter().filter(|r| is_plain_name(&r.folder)).map(|r| (dir.join(&r.folder), worlds_in(&dir.join(&r.folder)))).collect();
        return (Some(m), roots);
    }
    let mut roots = Vec::new();
    let direct = worlds_in(dir);
    if !direct.is_empty() {
        roots.push((dir.to_path_buf(), direct));
    }
    if let Ok(rd) = std::fs::read_dir(dir) {
        let mut subs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir() && !p.join("level.dat").is_file()).collect();
        subs.sort();
        for sub in subs {
            let worlds = worlds_in(&sub);
            if !worlds.is_empty() {
                roots.push((sub, worlds));
            }
        }
    }
    (None, roots)
}

/// A single folder name: nothing that could lead out of the folder it is joined to.
fn is_plain_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', ':'])
}

/// Every backup under `base`, the newest first. Folders without worlds are left out.
pub fn list_in(base: &Path) -> Vec<BackupInfo> {
    let Ok(rd) = std::fs::read_dir(base) else { return Vec::new() };
    let mut out: Vec<BackupInfo> = rd
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().into_owned();
            let dir = e.path();
            let (manifest, roots) = roots_of(&dir);
            let worlds: Vec<String> = roots.iter().flat_map(|(_, w)| w.iter().map(|w| w.name.clone())).collect();
            if worlds.is_empty() {
                return None;
            }
            let edition = manifest.as_ref().map(|m| m.edition).or_else(|| edition_of(&id))?;
            Some(BackupInfo {
                edition,
                created: manifest.as_ref().map(|m| m.created).unwrap_or_else(|| modified_ms(&dir)),
                version: manifest.as_ref().and_then(|m| m.version.clone()),
                worlds,
                bytes: manifest.as_ref().map(|m| m.bytes).unwrap_or_else(|| crate::core::fsutil::dir_size(&dir)),
                path: dir.to_string_lossy().into_owned(),
                id,
            })
        })
        .collect();
    out.sort_by_key(|b| std::cmp::Reverse(b.created));
    out
}

/// How a restore ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Restored {
    /// Worlds copied back.
    pub restored: usize,
    /// Worlds left alone because a world with that folder name is already there.
    pub kept: usize,
}

/// Copies a backup's worlds into `target` (a worlds folder). A world whose folder is already there is kept as it
/// is. Each world is copied next to its place, checked, and only then given its name, so a world that stopped
/// halfway is never taken for a world.
pub fn restore_into(backup: &Path, target: &Path, progress: &Progress) -> Result<Restored> {
    let (_, roots) = roots_of(backup);
    if roots.is_empty() {
        bail_tr!("bedrock.backup.empty", "That backup has no worlds in it.");
    }
    std::fs::create_dir_all(target)?;
    let mut todo: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut kept = 0;
    for (root, worlds) in &roots {
        for w in worlds {
            if !is_plain_name(&w.id) {
                continue;
            }
            let dest = target.join(&w.id);
            if dest.exists() || todo.iter().any(|(_, d)| *d == dest) {
                kept += 1;
            } else {
                todo.push((root.join(&w.id), dest));
            }
        }
    }
    let total: u64 = todo.iter().map(|(from, _)| crate::core::fsutil::dir_size(from)).sum();
    progress.total.store(total * 2, Ordering::Relaxed);
    let mut restored = 0;
    for (from, dest) in &todo {
        let name = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let part = target.join(format!("{name}.restoring"));
        let _ = std::fs::remove_dir_all(&part);
        let result = (|| -> Result<()> {
            let jobs = crate::core::fsutil::list_copy(from, &part, &[])?;
            if let Some((i, e)) = crate::core::fsutil::copy_many(&jobs, &progress.stop, &progress.done).into_iter().next() {
                if crate::core::fsutil::is_disk_full(&e) {
                    bail_tr!("bedrock.backup.diskFull", "There isn't enough free disk space to back up the worlds.");
                }
                bail_tr!("bedrock.backup.copyFailed", "Couldn't copy {file}: {error}", file = jobs[i].0.display().to_string(), error = e.to_string());
            }
            if progress.stop.load(Ordering::Relaxed) {
                return Err(crate::core::events::cancelled_error());
            }
            check_copy(from, &part, progress)?;
            std::fs::rename(&part, dest)?;
            Ok(())
        })();
        if let Err(e) = result {
            let _ = std::fs::remove_dir_all(&part);
            return Err(e);
        }
        restored += 1;
    }
    Ok(Restored { restored, kept })
}

// ---------- On this PC ----------

/// Where backups are kept.
pub fn base_dir() -> PathBuf {
    crate::core::paths::root().join("bedrock-backups")
}

/// A backup's folder, found again by its id (never a path from the UI).
pub fn backup_dir(id: &str) -> Result<PathBuf> {
    let dir = base_dir().join(id);
    if !is_plain_name(id) || !dir.is_dir() {
        bail_tr!("bedrock.backup.missing", "That backup isn't there any more.");
    }
    Ok(dir)
}

pub fn list() -> Vec<BackupInfo> {
    list_in(&base_dir())
}

/// Sets `stop` when the task waiting for the work is cancelled (its future dropped), so the copying thread ends.
struct StopOnDrop {
    progress: std::sync::Arc<Progress>,
    armed: bool,
}

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        if self.armed {
            self.progress.stop.store(true, Ordering::Relaxed);
        }
    }
}

/// Runs copying work on a blocking thread, showing its progress on `task` with `copying` and then `checking`
/// as the detail.
async fn with_progress<T: Send + 'static>(
    task: &crate::core::events::Task,
    copying: crate::core::text::Text,
    checking: crate::core::text::Text,
    work: impl FnOnce(&Progress) -> Result<T> + Send + 'static,
) -> Result<T> {
    let progress = std::sync::Arc::new(Progress::default());
    let mut guard = StopOnDrop { progress: progress.clone(), armed: true };
    let worker = progress.clone();
    let mut job = tokio::task::spawn_blocking(move || work(&worker));
    let result = loop {
        tokio::select! {
            r = &mut job => break r,
            _ = tokio::time::sleep(std::time::Duration::from_millis(200)) => {
                let (done, total) = (progress.done.load(Ordering::Relaxed), progress.total.load(Ordering::Relaxed));
                let detail = if progress.checking.load(Ordering::Relaxed) { checking.clone() } else { copying.clone() };
                task.update(detail, (total > 0).then(|| (done as f64 / total as f64).clamp(0.0, 1.0)));
            }
        }
    };
    // Finished on its own: there is nothing left to stop.
    guard.armed = false;
    result?
}

/// Backs up every world of an edition, showing progress on `task`. Ok(None) when the edition has no worlds.
/// Minecraft has to be closed: a world that changes while it is copied fails the check.
pub async fn run(edition: Edition, version: Option<String>, task: &crate::core::events::Task) -> Result<Option<BackupInfo>> {
    let roots = super::bedrock::world_roots(edition);
    let now = chrono::Local::now();
    let id = format!("{}-{}", edition.key(), now.format("%Y-%m-%d_%H-%M-%S"));
    let dest = base_dir().join(&id);
    let needed: u64 = roots.iter().map(|r| crate::core::fsutil::dir_size(r)).sum();
    if let Some(free) = crate::core::storage::free_bytes(&crate::core::paths::root()) {
        // Room for the copy and a little more.
        if free < needed + 64 * 1024 * 1024 {
            bail_tr!("bedrock.backup.diskFull", "There isn't enough free disk space to back up the worlds.");
        }
    }
    let target = dest.clone();
    let created = now.timestamp_millis();
    let manifest = with_progress(
        task,
        crate::tr!("bedrock.backup.copying", "Backing up worlds"),
        crate::tr!("bedrock.backup.checking", "Checking the backup"),
        move |progress| back_up(&roots, &target, edition, version, created, progress),
    )
    .await?;
    Ok(manifest.map(|m| BackupInfo {
        id,
        edition,
        created: m.created,
        version: m.version.clone(),
        worlds: m.roots.iter().flat_map(|r| r.worlds.iter().map(|w| w.name.clone())).collect(),
        bytes: m.bytes,
        path: dest.to_string_lossy().into_owned(),
    }))
}

/// Restores a backup into `target`, showing progress on `task`.
pub async fn run_restore(id: &str, target: PathBuf, task: &crate::core::events::Task) -> Result<Restored> {
    let backup = backup_dir(id)?;
    with_progress(
        task,
        crate::tr!("bedrock.backup.restoring", "Copying worlds back"),
        crate::tr!("bedrock.backup.checkingRestore", "Checking the restored worlds"),
        move |progress| restore_into(&backup, &target, progress),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::text::text_of;

    fn key(e: &anyhow::Error) -> String {
        text_of(e).map(|t| t.key.clone()).unwrap_or_default()
    }

    /// A scratch folder that is removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Scratch {
            let dir = std::env::temp_dir().join(format!("resincraft-bedrock-backup-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }
        fn path(&self, rel: &str) -> PathBuf {
            rel_path(&self.0, rel)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write(path: &Path, data: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, data).unwrap();
    }

    /// A worlds folder with `names` as worlds, each with a level.dat, a name and a database file.
    fn make_worlds(root: &Path, names: &[(&str, &str)]) {
        for (id, name) in names {
            write(&root.join(id).join("level.dat"), format!("level {id}").as_bytes());
            write(&root.join(id).join("levelname.txt"), name.as_bytes());
            write(&root.join(id).join("db").join("000005.ldb"), &vec![7u8; 4096]);
            write(&root.join(id).join("db").join("CURRENT"), b"MANIFEST-000004\n");
        }
    }

    #[test]
    fn backs_up_and_checks_every_worlds_folder() {
        let s = Scratch::new("ok");
        let (a, b, empty) = (s.path("data/a/minecraftWorlds"), s.path("data/b/minecraftWorlds"), s.path("data/c/minecraftWorlds"));
        make_worlds(&a, &[("ZxKD=", "Island Survival"), ("xLIC=", "Creative Build")]);
        make_worlds(&b, &[("abcd=", "Old World")]);
        std::fs::create_dir_all(&empty).unwrap();
        // Not a world: no level.dat. It is still copied with its worlds folder.
        write(&a.join("notes.txt"), b"hello");
        let dest = s.path("backups/release-test");
        let progress = Progress::default();
        let m = back_up(&[a.clone(), b.clone(), empty, s.path("data/missing")], &dest, Edition::Release, Some("1.26.65.1".into()), 1234, &progress).unwrap().unwrap();
        assert_eq!(m.roots.len(), 2, "folders without worlds are left out");
        assert_eq!(m.roots[0].folder, "worlds-0");
        assert_eq!(m.roots[0].source, a.to_string_lossy());
        assert_eq!(m.roots[0].worlds.iter().map(|w| w.name.as_str()).collect::<Vec<_>>(), ["Island Survival", "Creative Build"]);
        assert_eq!(m.roots[1].worlds, vec![BackedUpWorld { id: "abcd=".into(), name: "Old World".into() }]);
        assert_eq!(m.files, 13);
        assert_eq!(m.version.as_deref(), Some("1.26.65.1"));
        assert_eq!(std::fs::read(dest.join("worlds-0").join("ZxKD=").join("db").join("000005.ldb")).unwrap(), vec![7u8; 4096]);
        assert_eq!(std::fs::read(dest.join("worlds-0").join("notes.txt")).unwrap(), b"hello");
        // Copied once and read once more for the check.
        assert_eq!(progress.done.load(Ordering::Relaxed), m.bytes * 2);
        assert_eq!(progress.total.load(Ordering::Relaxed), m.bytes * 2);
        // The file on disk is what was returned.
        let on_disk: Manifest = serde_json::from_slice(&std::fs::read(dest.join(MANIFEST)).unwrap()).unwrap();
        assert_eq!(on_disk, m);
        // The originals are untouched.
        assert!(a.join("ZxKD=").join("level.dat").is_file());
    }

    #[test]
    fn no_worlds_makes_no_backup() {
        let s = Scratch::new("none");
        let root = s.path("minecraftWorlds");
        write(&root.join("stray.txt"), b"x");
        let dest = s.path("backups/release-test");
        assert_eq!(back_up(&[root], &dest, Edition::Release, None, 1, &Progress::default()).unwrap(), None);
        assert!(!dest.exists());
    }

    #[test]
    fn an_existing_folder_is_never_written_over() {
        let s = Scratch::new("exists");
        let root = s.path("minecraftWorlds");
        make_worlds(&root, &[("w1", "One")]);
        let dest = s.path("backups/release-test");
        write(&dest.join("keep.txt"), b"mine");
        let e = back_up(&[root], &dest, Edition::Release, None, 1, &Progress::default()).unwrap_err();
        assert_eq!(key(&e), "bedrock.backup.exists");
        assert_eq!(std::fs::read(dest.join("keep.txt")).unwrap(), b"mine", "and it is not cleaned up either");
    }

    #[test]
    fn a_copy_that_differs_fails_the_check() {
        let s = Scratch::new("differs");
        let (from, to) = (s.path("from"), s.path("to"));
        make_worlds(&from, &[("w1", "One")]);
        make_worlds(&to, &[("w1", "One")]);
        assert!(check_copy(&from, &to, &Progress::default()).is_ok());
        // Same size, other bytes.
        write(&to.join("w1").join("db").join("000005.ldb"), &vec![8u8; 4096]);
        let e = check_copy(&from, &to, &Progress::default()).unwrap_err();
        assert_eq!(key(&e), "bedrock.backup.differs");
        assert!(e.to_string().contains("w1/db/000005.ldb"), "{e}");
        // Another size.
        write(&to.join("w1").join("db").join("000005.ldb"), &vec![7u8; 4095]);
        assert_eq!(key(&check_copy(&from, &to, &Progress::default()).unwrap_err()), "bedrock.backup.differs");
        // A file missing from the copy, and one only the copy has.
        write(&to.join("w1").join("db").join("000005.ldb"), &vec![7u8; 4096]);
        std::fs::remove_file(to.join("w1").join("level.dat")).unwrap();
        assert!(check_copy(&from, &to, &Progress::default()).unwrap_err().to_string().contains("w1/level.dat"));
        write(&to.join("w1").join("level.dat"), b"level w1");
        write(&to.join("w1").join("extra.txt"), b"x");
        assert!(check_copy(&from, &to, &Progress::default()).unwrap_err().to_string().contains("w1/extra.txt"));
    }

    #[test]
    fn a_failed_backup_leaves_nothing_behind() {
        let s = Scratch::new("stopped");
        let root = s.path("minecraftWorlds");
        make_worlds(&root, &[("w1", "One")]);
        let dest = s.path("backups/release-test");
        let progress = Progress::default();
        progress.stop.store(true, Ordering::Relaxed);
        let e = back_up(&[root], &dest, Edition::Release, None, 1, &progress).unwrap_err();
        assert!(crate::core::events::is_cancelled(&e));
        assert!(!dest.exists(), "a folder without backup.json would look like a backup that was never checked");
    }

    #[test]
    fn lists_backups_newest_first() {
        let s = Scratch::new("list");
        let base = s.path("backups");
        let root = s.path("minecraftWorlds");
        make_worlds(&root, &[("w1", "One"), ("w2", "Two")]);
        back_up(&[root.clone()], &base.join("release-2026-11-20_10-00-00"), Edition::Release, Some("1.26.65.1".into()), 2000, &Progress::default()).unwrap();
        back_up(&[root], &base.join("preview-2026-11-19_10-00-00"), Edition::Preview, None, 1000, &Progress::default()).unwrap();
        // One that Uninstall made before: worlds directly in the folder, no backup.json.
        make_worlds(&base.join("release-2026-01-01_00-00-00"), &[("old", "Before")]);
        // And one with several numbered folders.
        make_worlds(&base.join("release-2026-01-02_00-00-00").join("0"), &[("x", "Zero")]);
        make_worlds(&base.join("release-2026-01-02_00-00-00").join("1"), &[("y", "First")]);
        // Not backups: an empty folder, a folder with another name, a file.
        std::fs::create_dir_all(base.join("release-empty")).unwrap();
        make_worlds(&base.join("something-else"), &[("z", "Z")]);
        write(&base.join("release-file"), b"x");

        let list = list_in(&base);
        assert_eq!(list.len(), 4);
        assert_eq!(list.iter().filter(|b| b.version.is_some()).count(), 1);
        let newest = list.iter().find(|b| b.id == "release-2026-11-20_10-00-00").unwrap();
        assert_eq!((newest.edition, newest.created, newest.worlds.clone()), (Edition::Release, 2000, vec!["One".to_string(), "Two".to_string()]));
        assert!(newest.bytes > 8000);
        assert_eq!(list.iter().find(|b| b.id == "preview-2026-11-19_10-00-00").unwrap().edition, Edition::Preview);
        assert_eq!(list.iter().find(|b| b.id == "release-2026-01-01_00-00-00").unwrap().worlds, vec!["Before".to_string()]);
        assert_eq!(list.iter().find(|b| b.id == "release-2026-01-02_00-00-00").unwrap().worlds, vec!["Zero".to_string(), "First".to_string()]);
        assert!(list.windows(2).all(|w| w[0].created >= w[1].created));
        assert!(list_in(&s.path("nowhere")).is_empty());
    }

    #[test]
    fn restores_without_replacing_worlds_that_are_there() {
        let s = Scratch::new("restore");
        let root = s.path("minecraftWorlds");
        make_worlds(&root, &[("w1", "One"), ("w2", "Two")]);
        let backup = s.path("backups/release-test");
        back_up(&[root.clone()], &backup, Edition::Release, None, 1, &Progress::default()).unwrap();

        // The game's folder now: w1 was played on since, w2 is gone.
        let target = s.path("now/minecraftWorlds");
        make_worlds(&target, &[("w1", "One, played on")]);
        // Left by a restore that stopped halfway.
        write(&target.join("w2.restoring").join("junk"), b"x");

        let progress = Progress::default();
        let r = restore_into(&backup, &target, &progress).unwrap();
        assert_eq!(r, Restored { restored: 1, kept: 1 });
        assert_eq!(std::fs::read_to_string(target.join("w1").join("levelname.txt")).unwrap(), "One, played on");
        assert_eq!(std::fs::read_to_string(target.join("w2").join("levelname.txt")).unwrap(), "Two");
        assert_eq!(std::fs::read(target.join("w2").join("db").join("000005.ldb")).unwrap(), vec![7u8; 4096]);
        assert!(!target.join("w2.restoring").exists());
        assert!(!target.join("w2").join("junk").exists());
        assert_eq!(progress.done.load(Ordering::Relaxed), progress.total.load(Ordering::Relaxed));
        // Again: everything is there already.
        assert_eq!(restore_into(&backup, &target, &Progress::default()).unwrap(), Restored { restored: 0, kept: 2 });
        // The backup itself is still whole.
        assert!(backup.join("worlds-0").join("w2").join("level.dat").is_file());
        // A backup Uninstall made (no backup.json) restores too.
        let legacy = s.path("backups/release-legacy");
        make_worlds(&legacy, &[("w9", "Nine")]);
        assert_eq!(restore_into(&legacy, &target, &Progress::default()).unwrap(), Restored { restored: 1, kept: 0 });
        // A folder with no worlds isn't a backup.
        std::fs::create_dir_all(s.path("backups/release-none")).unwrap();
        assert_eq!(key(&restore_into(&s.path("backups/release-none"), &target, &Progress::default()).unwrap_err()), "bedrock.backup.empty");
    }

    #[test]
    fn a_manifest_cannot_point_outside_its_backup() {
        let s = Scratch::new("escape");
        let outside = s.path("outside");
        make_worlds(&outside, &[("secret", "Elsewhere")]);
        let backup = s.path("backups/release-test");
        let manifest = Manifest {
            edition: Edition::Release,
            created: 1,
            version: None,
            files: 0,
            bytes: 0,
            roots: vec![BackedUpRoot { folder: "..\\..\\outside".into(), source: String::new(), worlds: vec![] }, BackedUpRoot { folder: "../../outside".into(), source: String::new(), worlds: vec![] }],
        };
        write(&backup.join(MANIFEST), &serde_json::to_vec(&manifest).unwrap());
        assert!(roots_of(&backup).1.is_empty());
        assert!(list_in(&s.path("backups")).is_empty());
        assert!(is_plain_name("release-2026-11-20_10-00-00") && is_plain_name("ZxKDIpkP4Tc="));
        assert!(!is_plain_name("..") && !is_plain_name("a/b") && !is_plain_name("a\\b") && !is_plain_name("C:") && !is_plain_name(""));
    }
}
