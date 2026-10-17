use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Result};
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
/// Accepts both `/` and `\` separators.
pub fn safe_join(base: &Path, rel: &str) -> Result<PathBuf> {
    let mut out = base.to_path_buf();
    for part in rel.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        let p = Path::new(part);
        match p.components().next() {
            Some(Component::Normal(_)) if p.components().count() == 1 => out.push(part),
            _ => bail!("Unsafe path: {rel}"),
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
