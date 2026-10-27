//! The backups a server or an instance has (`<server>/backups`, `<instance>/minecraft/backups`): listing them for
//! the Restore lists, and deleting one (to the Recycle Bin). Restoring lives with each kind (server/runtime.rs,
//! player/worlds.rs), since what a world is differs.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;

use crate::bail_tr;

/// One backup zip, newest first in lists.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    /// The zip's file name in the backups folder.
    pub file: String,
    /// The world it holds, from the name ("world", "My World").
    pub world: String,
    /// When it was made (ms), from the name's stamp, else the file's time.
    pub at: i64,
    pub size: u64,
    /// Made by auto backup (it may be replaced by a newer one).
    pub auto: bool,
}

/// "2026-10-27_14-30-00_My World_auto (2).zip" → (stamp ms, "My World", auto). None for a name without a stamp.
fn parse(name: &str) -> Option<(i64, String, bool)> {
    let stem = name.strip_suffix(".zip").or_else(|| name.strip_suffix(".ZIP"))?;
    let stem = match stem.rfind(" (") {
        Some(i) if stem.ends_with(')') => &stem[..i],
        _ => stem,
    };
    let stamp = stem.get(..19)?;
    let rest = stem.get(19..)?.strip_prefix('_')?;
    let at = chrono::NaiveDateTime::parse_from_str(stamp, "%Y-%m-%d_%H-%M-%S").ok()?;
    let at = at.and_local_timezone(chrono::Local).earliest()?.timestamp_millis();
    let (world, auto) = match rest.strip_suffix(super::autobackup::SUFFIX) {
        Some(w) => (w, true),
        None => (rest, false),
    };
    (!world.is_empty()).then(|| (at, world.to_string(), auto))
}

/// Every .zip in `dir`, newest first; nothing when the folder doesn't exist yet.
pub async fn list(dir: &Path) -> Vec<BackupFile> {
    let mut out = Vec::new();
    let Ok(mut rd) = tokio::fs::read_dir(dir).await else { return out };
    while let Ok(Some(e)) = rd.next_entry().await {
        let name = e.file_name().to_string_lossy().into_owned();
        if !name.to_lowercase().ends_with(".zip") {
            continue;
        }
        let Ok(meta) = e.metadata().await else { continue };
        if !meta.is_file() {
            continue;
        }
        let mtime = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0);
        let (at, world, auto) = parse(&name).unwrap_or_else(|| (mtime, name.trim_end_matches(".zip").to_string(), false));
        out.push(BackupFile { file: name, world, at, size: meta.len(), auto });
    }
    out.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| b.file.cmp(&a.file)));
    out
}

/// The path of backup `file` in `dir`, refusing anything that isn't a plain .zip name there.
pub fn path_of(dir: &Path, file: &str) -> Result<PathBuf> {
    if file.is_empty() || file.contains(['/', '\\', ':']) || file.starts_with('.') || !file.to_lowercase().ends_with(".zip") {
        bail_tr!("backups.badName", "That isn't one of the backups here.");
    }
    let p = dir.join(file);
    if !p.is_file() {
        bail_tr!("backups.gone", "That backup isn't there any more.");
    }
    Ok(p)
}

/// Moves a backup to the Recycle Bin.
pub async fn delete(dir: &Path, file: &str) -> Result<()> {
    crate::core::shell::trash(&path_of(dir, file)?).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_say_world_time_and_kind() {
        let (_, w, auto) = parse("2026-10-27_14-30-00_My World_auto (2).zip").unwrap();
        assert_eq!((w.as_str(), auto), ("My World", true));
        let (_, w, auto) = parse("2026-10-27_14-30-00_world.zip").unwrap();
        assert_eq!((w.as_str(), auto), ("world", false));
        assert!(parse("world.zip").is_none());
        assert!(parse("2026-10-27_14-30-00_.zip").is_none());
    }

    #[test]
    fn only_plain_names_in_the_folder() {
        let dir = std::env::temp_dir();
        assert!(path_of(&dir, "../x.zip").is_err());
        assert!(path_of(&dir, "C:\\x.zip").is_err());
        assert!(path_of(&dir, "notes.txt").is_err());
    }
}
