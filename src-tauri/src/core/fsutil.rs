use std::path::{Component, Path, PathBuf};

use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::err_tr;

/// Reads and parses a JSON file; None if it's missing or unreadable.
pub async fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let text = tokio::fs::read_to_string(path).await.ok()?;
    serde_json::from_str(&text).ok()
}

/// Writes JSON via a temp file + rename so a crash never leaves a half-written file.
pub async fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    tokio::fs::write(&tmp, serde_json::to_vec_pretty(value)?).await?;
    tokio::fs::rename(&tmp, path).await.map_err(|_| err_tr!("fs.saveFailed", "Couldn't save {path}", path = path.display().to_string()))?;
    Ok(())
}

/// Joins `rel` onto `base`, refusing anything that would escape `base` (zip-slip guard).
/// Accepts both `/` and `\` separators. Segments made only of dots and spaces (`..`, `.. `, `...`) are refused
/// too, because Windows trims trailing dots and spaces from names.
pub fn safe_join(base: &Path, rel: &str) -> Result<PathBuf> {
    let unsafe_path = || err_tr!("fs.unsafePath", "Unsafe path: {path}", path = rel.to_string());
    let mut out = base.to_path_buf();
    for part in rel.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part.contains('\0') || part.trim_end_matches(['.', ' ']).is_empty() {
            return Err(unsafe_path());
        }
        let p = Path::new(part);
        match p.components().next() {
            Some(Component::Normal(_)) if p.components().count() == 1 => out.push(part),
            _ => return Err(unsafe_path()),
        }
    }
    Ok(out)
}

/// [`safe_join`] for paths that are then read or written: also refuses a path that leaves `base` through a
/// symlink or junction. The deepest part of the result that exists must resolve inside `base`, and an entry
/// that can't be resolved (a dangling link, which a write would follow) is refused.
/// Returns the joined path, not the resolved one, so the caller acts on the entry itself.
pub fn safe_join_within(base: &Path, rel: &str) -> Result<PathBuf> {
    let out = safe_join(base, rel)?;
    let unsafe_path = || err_tr!("fs.unsafePath", "Unsafe path: {path}", path = rel.to_string());
    let root = std::fs::canonicalize(base).map_err(|_| unsafe_path())?;
    let mut probe = out.as_path();
    loop {
        match std::fs::canonicalize(probe) {
            Ok(real) if real.starts_with(&root) => break,
            Ok(_) => return Err(unsafe_path()),
            Err(_) if std::fs::symlink_metadata(probe).is_ok() => return Err(unsafe_path()),
            Err(_) => match probe.parent() {
                Some(parent) if parent.starts_with(base) => probe = parent,
                _ => return Err(unsafe_path()),
            },
        }
    }
    Ok(out)
}

pub fn slugify(name: &str) -> String {
    let mut s = String::new();
    let mut dash = false;
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
            dash = false;
        } else if !dash && !s.is_empty() {
            s.push('-');
            dash = true;
        }
    }
    let s = s.trim_end_matches('-').chars().take(32).collect::<String>();
    if s.is_empty() {
        "instance".into()
    } else {
        s
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

// ---------- Copying many files ----------

/// Files copied at the same time. Instances are mostly small files (world regions, configs, mods), where waiting on
/// the disk for each file in turn costs more than the copying itself.
const COPY_THREADS: usize = 8;

/// Copies `jobs` (from, to, size), creating every destination folder first, on up to COPY_THREADS threads. Stops
/// early when `stop` is set or the disk is full. Adds each copied file's size to `done`. Returns the failures as
/// (index into `jobs`, error); files after a stop are left out without an error.
pub fn copy_many(jobs: &[(PathBuf, PathBuf, u64)], stop: &std::sync::atomic::AtomicBool, done: &std::sync::atomic::AtomicU64) -> Vec<(usize, std::io::Error)> {
    use std::collections::BTreeSet;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    let failures = Mutex::new(Vec::new());
    let mut dirs: BTreeSet<&Path> = BTreeSet::new();
    for (_, to, _) in jobs {
        if let Some(parent) = to.parent() {
            dirs.insert(parent);
        }
    }
    for dir in dirs {
        if let Err(e) = std::fs::create_dir_all(dir) {
            // Can't make the destination: nothing that goes there can be copied.
            for (i, (_, to, _)) in jobs.iter().enumerate() {
                if to.parent() == Some(dir) {
                    failures.lock().unwrap().push((i, std::io::Error::new(e.kind(), e.to_string())));
                }
            }
        }
    }
    let failed_dirs: BTreeSet<usize> = failures.lock().unwrap().iter().map(|(i, _)| *i).collect();
    let next = AtomicUsize::new(0);
    let threads = COPY_THREADS.min(jobs.len()).max(1);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some((from, to, size)) = jobs.get(i) else { break };
                if failed_dirs.contains(&i) {
                    continue;
                }
                match std::fs::copy(from, to) {
                    Ok(_) => {
                        done.fetch_add(*size, Ordering::Relaxed);
                    }
                    Err(e) => {
                        if is_disk_full(&e) {
                            stop.store(true, Ordering::Relaxed);
                        }
                        failures.lock().unwrap().push((i, e));
                    }
                }
            });
        }
    });
    let mut out = failures.into_inner().unwrap();
    out.sort_by_key(|(i, _)| *i);
    out
}

/// The disk (or quota) is full: copying more won't help.
pub fn is_disk_full(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::StorageFull || e.raw_os_error() == Some(112) || e.raw_os_error() == Some(28)
}

/// Every file under `from` as (from, to, size) for `to`, leaving out top-level names in `skip`.
pub fn list_copy(from: &Path, to: &Path, skip: &[&str]) -> std::io::Result<Vec<(PathBuf, PathBuf, u64)>> {
    fn walk(from: &Path, to: &Path, skip: &[&str], out: &mut Vec<(PathBuf, PathBuf, u64)>) -> std::io::Result<()> {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            let name = entry.file_name();
            if skip.iter().any(|s| name == *s) {
                continue;
            }
            let dest = to.join(&name);
            if entry.file_type()?.is_dir() {
                walk(&entry.path(), &dest, &[], out)?;
            } else {
                // The listing's own metadata: asking the file system per file is slow on Windows.
                out.push((entry.path(), dest, entry.metadata().map(|m| m.len()).unwrap_or(0)));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(from, to, skip, &mut out)?;
    Ok(out)
}

/// Total size of a file or directory tree.
pub fn dir_size(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else { return 0 };
    if meta.is_file() {
        return meta.len();
    }
    let Ok(entries) = std::fs::read_dir(path) else { return 0 };
    entries.flatten().map(|e| dir_size(&e.path())).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh `<tmp>/resin-fsutil-<name>-<pid>/root` (with a `sub` folder) and a sibling `outside` holding a file.
    fn sandbox(name: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("resin-fsutil-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (root, outside) = (dir.join("root"), dir.join("outside"));
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.txt"), "secret").unwrap();
        (root, outside)
    }

    /// Links `link` to the folder `target`: a symlink, or on Windows a junction when symlinks need admin rights.
    fn link_dir(target: &Path, link: &Path) -> bool {
        #[cfg(unix)]
        return std::os::unix::fs::symlink(target, link).is_ok();
        #[cfg(windows)]
        return std::os::windows::fs::symlink_dir(target, link).is_ok()
            || std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
    }

    #[test]
    fn copies_many_files_and_reports_failures() {
        use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
        let (root, _) = sandbox("copy");
        let src = root.join("src");
        for i in 0..200 {
            let p = src.join(format!("d{}", i % 7)).join(format!("deep{}", i % 3)).join(format!("f{i}.txt"));
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, format!("file {i}")).unwrap();
        }
        std::fs::write(src.join("skip-me.json"), "meta").unwrap();
        let dst = root.join("dst");
        let mut jobs = list_copy(&src, &dst, &["skip-me.json"]).unwrap();
        assert_eq!(jobs.len(), 200);
        // One source vanishes before copying: reported, the rest still copied.
        jobs.push((src.join("gone.txt"), dst.join("gone.txt"), 4));
        let (stop, done) = (AtomicBool::new(false), AtomicU64::new(0));
        let failed = copy_many(&jobs, &stop, &done);
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].0, 200);
        for i in [0, 57, 199] {
            let rel = format!("d{}/deep{}/f{i}.txt", i % 7, i % 3);
            assert_eq!(std::fs::read_to_string(dst.join(&rel)).unwrap(), format!("file {i}"), "{rel}");
        }
        assert!(!dst.join("skip-me.json").exists());
        let expected: u64 = jobs[..200].iter().map(|j| j.2).sum();
        assert_eq!(done.load(Ordering::Relaxed), expected);
        // Stopped before starting: nothing is copied, nothing is reported.
        let stopped = AtomicBool::new(true);
        let empty = root.join("dst2");
        let again: Vec<_> = jobs[..5].iter().map(|(f, t, s)| (f.clone(), empty.join(t.file_name().unwrap()), *s)).collect();
        assert!(copy_many(&again, &stopped, &AtomicU64::new(0)).is_empty());
        assert!(!empty.join("f0.txt").exists());
    }

    /// Timing: `cargo test --lib copy_speed -- --ignored --nocapture`. A folder like a played instance: thousands
    /// of small files plus a few big ones, copied file by file (as before) and with copy_many.
    #[test]
    #[ignore]
    fn copy_speed() {
        use std::sync::atomic::{AtomicBool, AtomicU64};
        let (root, _) = sandbox("speed");
        let src = root.join("src");
        let small = vec![7u8; 6 * 1024];
        for i in 0..6000 {
            let p = src.join(format!("saves/World{}/region", i % 6)).join(format!("r.{i}.mca"));
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, &small).unwrap();
        }
        let big = vec![9u8; 8 * 1024 * 1024];
        for i in 0..40 {
            let p = src.join("mods").join(format!("mod{i}.jar"));
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, &big).unwrap();
        }
        let one_by_one = |to: &Path| {
            for (f, t, _) in list_copy(&src, to, &[]).unwrap() {
                std::fs::copy(f, t).unwrap();
            }
        };
        let parallel = |to: &Path| {
            let jobs = list_copy(&src, to, &[]).unwrap();
            assert!(copy_many(&jobs, &AtomicBool::new(false), &AtomicU64::new(0)).is_empty());
        };
        for round in 0..2 {
            let t = std::time::Instant::now();
            one_by_one(&root.join(format!("old{round}")));
            let old = t.elapsed();
            let t = std::time::Instant::now();
            parallel(&root.join(format!("new{round}")));
            let new = t.elapsed();
            println!("round {round}: one by one {old:.2?}, copy_many {new:.2?} ({:.1}x)", old.as_secs_f64() / new.as_secs_f64());
        }
        let _ = std::fs::remove_dir_all(root.parent().unwrap());
    }

    #[test]
    fn joins_plain_paths() {
        let base = Path::new("base");
        assert_eq!(safe_join(base, "").unwrap(), base);
        assert_eq!(safe_join(base, "a/b\\c.txt").unwrap(), base.join("a").join("b").join("c.txt"));
        assert_eq!(safe_join(base, "./a//b/").unwrap(), base.join("a").join("b"));
        assert_eq!(safe_join(base, ".hidden/x..y").unwrap(), base.join(".hidden").join("x..y"));
    }

    #[test]
    fn refuses_parent_and_dot_segments() {
        let base = Path::new("base");
        for rel in ["..", "../x", "a/../../x", "a/..", "..\\x", "a\\..\\..\\x", ".. ", "...", "a/. ./b", "a\0b"] {
            assert!(safe_join(base, rel).is_err(), "{rel:?} should be refused");
        }
    }

    #[test]
    fn leading_separators_stay_inside() {
        let base = Path::new("base");
        // A leading separator is only an empty segment, so the path is still relative to `base`.
        for rel in ["/etc/passwd", "\\\\server\\share\\x", "\\Windows"] {
            let p = safe_join(base, rel).unwrap();
            assert!(p.starts_with(base), "{rel:?} -> {p:?}");
        }
        // A drive letter is a path prefix on Windows (refused); elsewhere it's an ordinary folder name.
        for rel in ["C:\\Windows\\win.ini", "C:/x", "c:"] {
            match safe_join(base, rel) {
                Ok(p) => assert!(!cfg!(windows) && p.starts_with(base), "{rel:?} -> {p:?}"),
                Err(_) => assert!(cfg!(windows), "{rel:?} refused off Windows"),
            }
        }
    }

    #[test]
    fn within_accepts_existing_and_new_paths() {
        let (root, _) = sandbox("ok");
        assert_eq!(safe_join_within(&root, "").unwrap(), root);
        assert_eq!(safe_join_within(&root, "sub").unwrap(), root.join("sub"));
        assert_eq!(safe_join_within(&root, "sub/new/deeper.txt").unwrap(), root.join("sub").join("new").join("deeper.txt"));
        assert!(safe_join_within(&root, "../outside/secret.txt").is_err());
        assert!(safe_join_within(&root.join("missing"), "x").is_err(), "the root itself must exist");
    }

    #[test]
    fn within_refuses_link_escapes() {
        let (root, outside) = sandbox("link");
        if !link_dir(&outside, &root.join("escape")) {
            eprintln!("skipping: can't create a directory link here");
            return;
        }
        assert!(safe_join(&root, "escape/secret.txt").is_ok(), "lexically it looks fine");
        assert!(safe_join_within(&root, "escape").is_err());
        assert!(safe_join_within(&root, "escape/secret.txt").is_err());
        assert!(safe_join_within(&root, "escape/new/file.txt").is_err());
        // A link that stays inside the root is fine.
        assert!(link_dir(&root.join("sub"), &root.join("inner")));
        assert!(safe_join_within(&root, "inner/x.txt").is_ok());
        // A dangling link would be followed by a write, so it's refused too.
        let gone = outside.parent().unwrap().join("gone");
        std::fs::create_dir_all(&gone).unwrap();
        assert!(link_dir(&gone, &root.join("dangling")));
        std::fs::remove_dir(&gone).unwrap();
        assert!(safe_join_within(&root, "dangling").is_err());
        assert!(safe_join_within(&root, "dangling/x.txt").is_err());
    }
}
