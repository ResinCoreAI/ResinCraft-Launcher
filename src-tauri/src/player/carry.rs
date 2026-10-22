//! Brings worlds, shader packs, resource packs and game settings over from another instance.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::files::{child_path, game_subdir, unique_name};
use crate::content::index::{load_index, save_index};
use crate::core::events::Task;
use crate::core::fsutil::{copy_many, list_copy};
use crate::core::types::ContentKind;
use crate::core::{instances, paths};
use crate::{bail_tr, tr};

/// The game's own settings files: video and controls, OptiFine's, and the chosen shader with its options.
pub const SETTINGS_FILES: [&str; 3] = ["options.txt", "optionsof.txt", "optionsshaders.txt"];

/// What to bring over (`instances:copyFrom`).
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CopyPicks {
    /// World folders in the other instance's saves/.
    pub worlds: Vec<String>,
    pub shaders: bool,
    pub resource_packs: bool,
    pub settings: bool,
}

/// What was brought over.
#[derive(Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CopyResult {
    /// The new world folders (renamed when the name was taken here).
    pub worlds: Vec<String>,
    pub shaders: usize,
    pub resource_packs: usize,
    /// Settings files replaced.
    pub settings: usize,
}

/// Copies `picks` from instance `source` into `target`. Worlds come in beside the ones already here (renamed when
/// the name is taken); packs come in when this instance doesn't have them yet; settings files replace these.
pub async fn copy_from(target: &str, source: &str, picks: CopyPicks) -> Result<CopyResult> {
    if target == source {
        bail_tr!("carry.sameInstance", "Choose a different instance to copy from");
    }
    let (to, from) = (instances::get(target).await?, instances::get(source).await?);
    let running = crate::game::running_ids();
    if running.iter().any(|r| *r == to.id) {
        // The game writes options.txt when it closes, and an open world can't take new folders safely.
        bail_tr!("carry.closeTarget", "Close {name} before you copy into it", name = to.name);
    }
    if !picks.worlds.is_empty() && running.iter().any(|r| *r == from.id) {
        bail_tr!("carry.closeSource", "Close {name} before you copy its worlds", name = from.name);
    }

    let (src_game, dst_game) = (paths::game_dir(&from.id), paths::game_dir(&to.id));
    let mut out = CopyResult::default();
    let mut jobs: Vec<(PathBuf, PathBuf, u64)> = Vec::new();

    if !picks.worlds.is_empty() {
        let (src_saves, dst_saves) = (src_game.join("saves"), game_subdir(&to.id, "saves").await?);
        for folder in &picks.worlds {
            let src = child_path(&src_saves, folder)?;
            if !src.join("level.dat").is_file() && !src.join("level.dat_old").is_file() {
                continue;
            }
            // Listing makes the folder, so the next world picked can't take the same name.
            let name = unique_name(&dst_saves, folder, "").await;
            let dst = child_path(&dst_saves, &name)?;
            jobs.extend(list(&src, &dst, &["session.lock"]).await?);
            out.worlds.push(name);
        }
    }

    // Packs this instance already has (by file name, with or without .disabled) stay as they are.
    let mut kinds: Vec<(ContentKind, Vec<String>)> = Vec::new();
    for (kind, wanted) in [(ContentKind::Shader, picks.shaders), (ContentKind::Resourcepack, picks.resource_packs)] {
        let src = src_game.join(kind.folder());
        if !wanted || !src.is_dir() {
            continue;
        }
        let dst = game_subdir(&to.id, kind.folder()).await?;
        let have: BTreeSet<String> = names(&dst).await.iter().map(|n| base(n).to_lowercase()).collect();
        let new: Vec<String> = names(&src).await.into_iter().filter(|n| !have.contains(&base(n).to_lowercase())).collect();
        let skip: Vec<String> = names(&src).await.into_iter().filter(|n| !new.contains(n)).collect();
        let skip: Vec<&str> = skip.iter().map(String::as_str).collect();
        jobs.extend(list(&src, &dst, &skip).await?);
        // Shaders' own option files (<pack>.txt) come along but aren't packs.
        let count = new.iter().filter(|n| !(kind == ContentKind::Shader && n.to_lowercase().ends_with(".txt"))).count();
        match kind {
            ContentKind::Shader => out.shaders = count,
            _ => out.resource_packs = count,
        }
        kinds.push((kind, new));
    }

    if picks.settings {
        for name in SETTINGS_FILES {
            let src = src_game.join(name);
            if let Ok(meta) = tokio::fs::metadata(&src).await {
                if meta.is_file() {
                    jobs.push((src, dst_game.join(name), meta.len()));
                    out.settings += 1;
                }
            }
        }
    }

    if jobs.is_empty() {
        return Ok(out);
    }
    let total: u64 = jobs.iter().map(|(_, _, n)| n).sum::<u64>().max(1);
    let new_worlds: Vec<PathBuf> = out.worlds.iter().map(|w| dst_game.join("saves").join(w)).collect();
    let title = tr!("carry.copying", "Copying from {name}", name = from.name.clone());
    Task::run_to_end(title, Some(to.id.clone()), |t| async move {
        let stop = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicU64::new(0));
        let (jobs, worker_stop, worker_done) = (Arc::new(jobs), stop.clone(), done.clone());
        let worker_jobs = jobs.clone();
        let mut work = tokio::task::spawn_blocking(move || copy_many(&worker_jobs, &worker_stop, &worker_done));
        let failures = loop {
            tokio::select! {
                result = &mut work => break result?,
                _ = tokio::time::sleep(Duration::from_millis(150)) => {
                    let n = done.load(Ordering::Relaxed);
                    t.update(tr!("carry.progress", "{done} of {total} MB", done = n / 1_048_576, total = total / 1_048_576), Some(n as f64 / total as f64));
                }
            }
        };
        if let Some((_, e)) = failures.into_iter().next() {
            // Only removes the world folders this copy made; packs and settings that did copy are fine as they are.
            for w in &new_worlds {
                let _ = tokio::fs::remove_dir_all(w).await;
            }
            return Err(e.into());
        }
        Ok(())
    })
    .await?;

    // The packs keep their Discover links (updates, versions) in this instance too.
    if !kinds.is_empty() {
        let from_index = load_index(&from.id).await;
        let mut index = load_index(&to.id).await;
        let before = index.len();
        for (kind, new) in &kinds {
            for name in new {
                let key = crate::content::index::index_key(*kind, name);
                if let Some(entry) = from_index.get(&key) {
                    index.entry(key).or_insert_with(|| entry.clone());
                }
            }
        }
        if index.len() != before {
            save_index(&to.id, &index).await?;
        }
    }
    Ok(out)
}

/// Names in a folder (files and folders); empty when it doesn't exist.
async fn names(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(mut rd) = tokio::fs::read_dir(dir).await {
        while let Ok(Some(e)) = rd.next_entry().await {
            out.push(e.file_name().to_string_lossy().into_owned());
        }
    }
    out.sort();
    out
}

/// "Pack.zip.disabled" → "Pack.zip".
fn base(name: &str) -> &str {
    name.strip_suffix(".disabled").unwrap_or(name)
}

async fn list(from: &Path, to: &Path, skip: &[&str]) -> Result<Vec<(PathBuf, PathBuf, u64)>> {
    let (from, to) = (from.to_path_buf(), to.to_path_buf());
    let skip: Vec<String> = skip.iter().map(|s| s.to_string()).collect();
    Ok(tokio::task::spawn_blocking(move || {
        let skip: Vec<&str> = skip.iter().map(String::as_str).collect();
        list_copy(&from, &to, &skip)
    })
    .await??)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_packs_count_as_the_same_pack() {
        assert_eq!(base("BSL.zip.disabled"), "BSL.zip");
        assert_eq!(base("BSL.zip"), "BSL.zip");
    }
}
