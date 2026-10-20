//! Copying another launcher's files. The source is only ever read: files are copied one by one (so Cancel
//! stops between files), and anything that can't be read is skipped and reported instead of failing the import.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::formats::is_skipped;
use crate::core::text::Text;
use crate::tr;

/// A file or folder that was left behind, and why.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    pub path: String,
    pub reason: Text,
}

/// Everything to copy: (path relative to the source, size), plus what couldn't even be listed.
pub struct Listing {
    pub files: Vec<(PathBuf, u64)>,
    pub skipped: Vec<Skipped>,
    pub bytes: u64,
}

fn unreadable(rel: &Path, err: &std::io::Error) -> Skipped {
    let reason = match err.kind() {
        std::io::ErrorKind::PermissionDenied => tr!("migrate.denied", "Access was denied (another program may be using it)"),
        _ => tr!("migrate.unreadable", "Couldn't read it: {error}", error = err.to_string()),
    };
    Skipped { path: rel.to_string_lossy().replace('\\', "/"), reason }
}

/// Lists `root` recursively, leaving out top-level names that match `skip`. Missing `root` = nothing to copy.
pub fn list(root: &Path, skip: &[String]) -> Listing {
    let mut out = Listing { files: Vec::new(), skipped: Vec::new(), bytes: 0 };
    if root.is_dir() {
        walk(root, Path::new(""), skip, 0, &mut out);
    }
    out
}

fn walk(dir: &Path, rel: &Path, skip: &[String], depth: usize, out: &mut Listing) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => return out.skipped.push(unreadable(rel, &e)),
    };
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                out.skipped.push(unreadable(rel, &e));
                continue;
            }
        };
        let name = entry.file_name();
        if depth == 0 && is_skipped(&name.to_string_lossy(), skip) {
            continue;
        }
        let path = entry.path();
        let child = rel.join(&name);
        // Follows links (a symlinked saves folder is common), with a depth limit against loops. Other entries use the
        // directory listing's own metadata: asking the file system per file is ~100x slower on Windows.
        let link = entry.file_type().map(|t| t.is_symlink()).unwrap_or(true);
        match if link { std::fs::metadata(&path) } else { entry.metadata() } {
            Ok(m) if m.is_dir() => {
                if depth < 48 {
                    walk(&path, &child, skip, depth + 1, out);
                }
            }
            Ok(m) => {
                out.bytes += m.len();
                out.files.push((child, m.len()));
            }
            Err(e) => out.skipped.push(unreadable(&child, &e)),
        }
    }
}

/// Copies the listed files from `from` into `to`. `progress(bytes copied)` is called after each file.
/// Returns what couldn't be copied.
pub async fn copy_listed(from: &Path, to: &Path, listing: &Listing, mut progress: impl FnMut(u64)) -> std::io::Result<Vec<Skipped>> {
    tokio::fs::create_dir_all(to).await?;
    let mut skipped = listing.skipped.clone();
    let mut done = 0u64;
    for (rel, size) in &listing.files {
        let dest = to.join(rel);
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        if let Err(e) = tokio::fs::copy(from.join(rel), &dest).await {
            // A failure writing here (disk full) stops the import; a file we can't read is skipped.
            if tokio::fs::metadata(to).await.is_err() || e.raw_os_error() == Some(112) || e.kind() == std::io::ErrorKind::StorageFull {
                return Err(e);
            }
            let _ = tokio::fs::remove_file(&dest).await;
            skipped.push(unreadable(rel, &e));
        }
        done += size;
        progress(done);
    }
    Ok(skipped)
}

/// Bytes that would be copied.
pub fn size(root: &Path, skip: &[String]) -> u64 {
    list(root, skip).bytes
}

/// Mods (or plugins) and worlds in a game or server folder: quick, for the list before importing.
pub fn counts(root: &Path) -> (u32, u32) {
    let count = |dir: &str, dirs: bool| -> u32 {
        std::fs::read_dir(root.join(dir))
            .map(|rd| {
                rd.flatten()
                    .filter(|e| {
                        let p = e.path();
                        if dirs {
                            p.join("level.dat").is_file()
                        } else {
                            let n = e.file_name().to_string_lossy().to_lowercase();
                            // The listing's own file type: asking the file system per jar is slow on Windows.
                            !e.file_type().is_ok_and(|t| t.is_dir()) && (n.ends_with(".jar") || n.ends_with(".jar.disabled") || n.ends_with(".zip"))
                        }
                    })
                    .count() as u32
            })
            .unwrap_or(0)
    };
    // Instances keep worlds in saves/, servers in world/ (level-name).
    let worlds = count("saves", true) + u32::from(root.join("world").join("level.dat").is_file());
    let mods = count("mods", false) + count("plugins", false);
    (mods, worlds)
}

#[cfg(test)]
mod tests {
    use super::super::formats::tests::Fixture;
    use super::*;

    #[test]
    fn copies_everything_but_skipped_names() {
        let fx = Fixture::new();
        fx.file("src/mods/a.jar", "aaaa")
            .file("src/saves/World/level.dat", "lvl")
            .file("src/saves/World/region/r.0.0.mca", "r")
            .file("src/options.txt", "o")
            .file("src/logs/latest.log", "log")
            .file("src/launcher_accounts.json", "secret");
        let skip: Vec<String> = ["logs", "launcher*"].iter().map(|s| s.to_string()).collect();
        let listing = list(&fx.path("src"), &skip);
        assert_eq!(listing.files.len(), 4);
        assert_eq!(listing.bytes, 9);
        let mut seen = Vec::new();
        let skipped = tokio::runtime::Runtime::new().unwrap().block_on(copy_listed(&fx.path("src"), &fx.path("dst"), &listing, |b| seen.push(b))).unwrap();
        assert!(skipped.is_empty());
        assert_eq!(seen.last(), Some(&9));
        assert_eq!(std::fs::read_to_string(fx.path("dst/saves/World/region/r.0.0.mca")).unwrap(), "r");
        assert!(!fx.path("dst/logs").exists() && !fx.path("dst/launcher_accounts.json").exists());
        assert!(fx.path("src/logs/latest.log").is_file(), "the source is left as it was");
        assert_eq!((size(&fx.path("src"), &skip), counts(&fx.path("src"))), (9, (1, 1)));
    }

    #[test]
    fn missing_source_is_empty_and_unreadable_files_are_skipped() {
        let fx = Fixture::new();
        assert!(list(&fx.path("nope"), &[]).files.is_empty());
        fx.file("src/a.txt", "a").file("src/b.txt", "b");
        let listing = list(&fx.path("src"), &[]);
        std::fs::remove_file(fx.path("src/b.txt")).unwrap(); // vanishes between listing and copying
        let skipped = tokio::runtime::Runtime::new().unwrap().block_on(copy_listed(&fx.path("src"), &fx.path("dst"), &listing, |_| {})).unwrap();
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].path, "b.txt");
        assert!(fx.path("dst/a.txt").is_file() && !fx.path("dst/b.txt").exists());
    }
}
