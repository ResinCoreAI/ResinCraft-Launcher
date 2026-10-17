//! Path helpers for files the renderer names inside an instance (worlds, screenshots, notes).
//! Every path the UI sends goes through here, so it can never point outside the instance folder.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use crate::core::fsutil::safe_join;
use crate::core::{instances, paths};

fn is_plain_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\']) && name != "." && name != ".."
}

/// The instance folder, after checking `id` is a real instance and not a path trick.
pub async fn instance_root(id: &str) -> Result<PathBuf> {
    if !is_plain_name(id) {
        bail!("Invalid instance id");
    }
    let dir = safe_join(&paths::instances(), id)?;
    instances::get(id).await?;
    Ok(dir)
}

/// A folder inside the instance's game directory, e.g. "saves"; created if missing.
pub async fn game_subdir(id: &str, sub: &str) -> Result<PathBuf> {
    instance_root(id).await?;
    let dir = safe_join(&paths::game_dir(id), sub)?;
    tokio::fs::create_dir_all(&dir).await?;
    Ok(dir)
}

/// A direct child of `dir` named by the renderer: a single path segment, nothing else.
pub fn child_path(dir: &Path, name: &str) -> Result<PathBuf> {
    if !is_plain_name(name) {
        bail!("Invalid name: {name}");
    }
    safe_join(dir, name)
}

/// Total size of a folder, counting every file below it (off the async runtime).
pub async fn folder_size(dir: &Path) -> u64 {
    let dir = dir.to_path_buf();
    tokio::task::spawn_blocking(move || crate::core::fsutil::dir_size(&dir)).await.unwrap_or(0)
}

/// `base`, or `base (2)`, `base (3)`… whichever doesn't exist in `dir` yet (case-insensitive).
pub async fn unique_name(dir: &Path, base: &str, ext: &str) -> String {
    let mut taken = HashSet::new();
    if let Ok(mut rd) = tokio::fs::read_dir(dir).await {
        while let Ok(Some(e)) = rd.next_entry().await {
            taken.insert(e.file_name().to_string_lossy().to_lowercase());
        }
    }
    let mut i = 1;
    loop {
        let name = if i > 1 { format!("{base} ({i}){ext}") } else { format!("{base}{ext}") };
        if !taken.contains(&name.to_lowercase()) {
            return name;
        }
        i += 1;
    }
}

/// Writes via a temp file + rename, so a crash or full disk never leaves a half-written file.
pub async fn write_atomic(file: &Path, data: &[u8]) -> Result<()> {
    if let Some(parent) = file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = PathBuf::from(format!("{}.{}.{}.tmp", file.display(), std::process::id(), crate::core::fsutil::now_ms()));
    let res = async {
        tokio::fs::write(&tmp, data).await?;
        tokio::fs::rename(&tmp, file).await?;
        Ok(())
    }
    .await;
    if res.is_err() {
        let _ = tokio::fs::remove_file(&tmp).await;
    }
    res
}

/// Length in UTF-16 code units, which is what the TypeScript limits counted.
pub fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}
