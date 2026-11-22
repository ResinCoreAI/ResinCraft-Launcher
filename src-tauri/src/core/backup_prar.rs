//! Backups in PRAR, PuirkleRAR's archive format (see `backup.rs` for what goes in a backup).
//!
//! PRAR stores a piece of data only once across all files, so the mods and libraries that instances share take
//! the space of one copy, and it unpacks jars and PNGs to compress what's inside them. The manifest goes in twice:
//! as the archive comment (read without unpacking anything) and as `resincraft-backup.json` (so a backup
//! extracted with PuirkleRAR is complete).
//!
//! Source builds without access to PuirkleRAR use the stand-in crate in `src-tauri/prar-stub`
//! (`tools/use-prar-stub.mjs`): `puirklerar::AVAILABLE` is false there and only zip backups are offered.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use anyhow::{bail, Result};
use puirklerar::{prar, progress::Progress, util};

use super::backup::{Entry, Manifest, MANIFEST};

/// `done` counts up to this (instead of bytes): PRAR works in phases of different sizes.
pub const SCALE: u64 = 10_000;

/// Whether this build can make and open PRAR backups.
pub fn available() -> bool {
    puirklerar::AVAILABLE
}

/// PuirkleRAR's progress as `done` out of `SCALE`, and the task's Cancel as its cancel. The first phase with a
/// size (compressing, or extracting) fills the bar up to `main`, later ones (writing the archive) the rest.
struct Report<'a> {
    stop: &'a AtomicBool,
    done: &'a AtomicU64,
    main: f64,
    /// (phase total, phase done, bar at its start, bar at its end, sized phases so far)
    phase: Mutex<(u64, u64, f64, f64, u32)>,
}

impl<'a> Report<'a> {
    fn new(stop: &'a AtomicBool, done: &'a AtomicU64, main: f64) -> Self {
        Report { stop, done, main, phase: Mutex::new((0, 0, 0.0, 0.0, 0)) }
    }

    fn show(&self, p: &(u64, u64, f64, f64, u32)) {
        let part = if p.0 == 0 { 0.0 } else { (p.1 as f64 / p.0 as f64).min(1.0) };
        let at = p.2 + (p.3 - p.2) * part;
        self.done.fetch_max((at * SCALE as f64) as u64, Ordering::Relaxed);
    }
}

impl Progress for Report<'_> {
    fn start(&self, total: u64, _msg: &str) {
        let mut p = self.phase.lock().unwrap();
        if total == 0 {
            // A spinner phase: the bar stays where it is.
            p.0 = 0;
            return;
        }
        let at = self.done.load(Ordering::Relaxed) as f64 / SCALE as f64;
        let (from, to) = if p.4 == 0 { (0.0, self.main) } else { (at.max(self.main), 1.0) };
        *p = (total, 0, from, to, p.4 + 1);
        self.show(&p);
    }

    fn inc(&self, n: u64) {
        let mut p = self.phase.lock().unwrap();
        p.1 += n;
        self.show(&p);
    }

    fn grow(&self, extra: u64) {
        self.phase.lock().unwrap().0 += extra;
    }

    fn log(&self, _line: &str) {}

    fn cancelled(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
}

/// PuirkleRAR's own "cancelled" as the task's.
fn cancel_aware<T>(r: Result<T>, stop: &AtomicBool) -> Result<T> {
    match r {
        Err(_) if stop.load(Ordering::Relaxed) => bail!(super::events::cancelled_error()),
        r => r,
    }
}

/// The files that can be read now, with their current size and time. Like the zip writer, a file that vanished or
/// is locked since listing (a log being written, say) is left out.
fn readable(files: Vec<Entry>) -> Vec<util::Entry> {
    let check = |(path, name, _): Entry| {
        let md = std::fs::File::open(&path).and_then(|f| f.metadata()).ok()?;
        Some(util::Entry { name, size: md.len(), mtime_ns: util::mtime_ns(&md), path, is_dir: false })
    };
    // (Opening is slow on Windows with an antivirus: a few threads.)
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 8);
    let chunk = files.len().div_ceil(threads).max(1);
    let mut parts: Vec<Vec<Entry>> = Vec::new();
    let mut files = files;
    while !files.is_empty() {
        let rest = files.split_off(chunk.min(files.len()));
        parts.push(std::mem::replace(&mut files, rest));
    }
    std::thread::scope(|s| {
        let handles: Vec<_> = parts.into_iter().map(|part| s.spawn(move || part.into_iter().filter_map(check).collect::<Vec<_>>())).collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    })
}

/// Writes the backup to `dest` (PuirkleRAR writes a hidden file next to it and puts it in place at the end, so a
/// failed or cancelled backup leaves nothing behind).
pub fn write(dest: &Path, files: Vec<Entry>, manifest: &Manifest, stop: &AtomicBool, done: &AtomicU64) -> Result<()> {
    let json = serde_json::to_string(manifest)?;
    let manifest_file = std::env::temp_dir().join(format!("resincraft-backup-manifest-{}-{}.json", std::process::id(), super::fsutil::now_ms()));
    std::fs::write(&manifest_file, &json)?;
    let result = (|| {
        let mut entries = readable(files);
        if stop.load(Ordering::Relaxed) {
            bail!(super::events::cancelled_error());
        }
        let md = std::fs::metadata(&manifest_file)?;
        entries.push(util::Entry { name: MANIFEST.into(), path: manifest_file.clone(), is_dir: false, size: md.len(), mtime_ns: util::mtime_ns(&md) });
        // (A manifest too long for a comment is still read, from the file.)
        let comment = (json.len() <= util::MAX_COMMENT).then_some(json.as_str());
        let report = Report::new(stop, done, 0.9);
        cancel_aware(prar::pack(dest, entries, util::Level::Normal, None, comment, &report), stop)
    })();
    let _ = std::fs::remove_file(&manifest_file);
    result
}

/// The backup's manifest text: the archive comment, or else the manifest file inside.
pub fn manifest_text(path: &Path) -> Result<String> {
    if let Some(c) = prar::comment(path, None)? {
        if serde_json::from_str::<Manifest>(&c).is_ok() {
            return Ok(c);
        }
    }
    let dir = std::env::temp_dir().join(format!("resincraft-backup-manifest-{}-{}", std::process::id(), super::fsutil::now_ms()));
    let sel = util::Selection { names: vec![MANIFEST.into()], ..Default::default() };
    let never = AtomicBool::new(false);
    let sink = AtomicU64::new(0);
    let read = prar::extract(path, &dir, None, true, &sel, &Report::new(&never, &sink, 1.0)).and_then(|_| Ok(std::fs::read_to_string(dir.join(MANIFEST))?));
    let _ = std::fs::remove_dir_all(&dir);
    read
}

/// Unpacks the whole backup into `work` (a new folder on the same drive as the instances, so moving files out of
/// it is a rename). The bar goes to 95%; moving them into place is the rest.
pub fn unpack(path: &Path, work: &Path, stop: &AtomicBool, done: &AtomicU64) -> Result<()> {
    let report = Report::new(stop, done, 0.95);
    cancel_aware(prar::extract(path, work, None, true, &util::Selection::default(), &report), stop)
}

/// Every file under `dir` as (backup name, path), names '/'-separated.
pub fn files_in(dir: &Path) -> Vec<(String, PathBuf)> {
    fn go(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let full = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
            match e.file_type() {
                Ok(t) if t.is_dir() => go(&e.path(), &full, out),
                Ok(t) if t.is_file() => out.push((full, e.path())),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    go(dir, "", &mut out);
    out
}
