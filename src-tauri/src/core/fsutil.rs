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
