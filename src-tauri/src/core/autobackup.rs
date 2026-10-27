//! Auto backup (Settings → Backups, on by default): while a hosted server or the game runs, its worlds are zipped
//! every `auto_backup_minutes` (30), and only the newest `auto_backup_keep` (3) auto backups of each world are kept.
//!
//! Auto backups go where manual ones do (`<server>/backups`, `<instance>/minecraft/backups`) with `_auto` after the
//! world's name; only those are ever deleted here, never a backup the player made. A server is backed up with its
//! saving paused (runtime::backup_as); a singleplayer world only when the game saved it since its last auto backup,
//! so an idle world doesn't push its older backups out.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use crate::core::settings;
use crate::player::{files, worlds};
use crate::server::{runtime, store};

/// What goes after the world's name in an auto backup's zip.
pub const SUFFIX: &str = "_auto";

/// When each running server ("s:<id>") or instance ("i:<id>") was last backed up, or first seen running.
static LAST: Mutex<Option<HashMap<String, (Instant, SystemTime)>>> = Mutex::new(None);

pub async fn init() {
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        let s = settings::get().await;
        if !s.auto_backup {
            LAST.lock().unwrap().take();
            continue;
        }
        let every = Duration::from_secs(u64::from(s.auto_backup_minutes.clamp(5, 24 * 60)) * 60);
        let keep = s.auto_backup_keep.clamp(1, 50) as usize;
        let servers: Vec<String> = runtime::running_ids().into_iter().filter(|id| runtime::state(id) == "running").collect();
        let games = crate::game::running_ids();
        let mut due = Vec::new();
        {
            let mut guard = LAST.lock().unwrap();
            let last = guard.get_or_insert_with(HashMap::new);
            let live: Vec<String> = servers.iter().map(|id| format!("s:{id}")).chain(games.iter().map(|id| format!("i:{id}"))).collect();
            // Forget what stopped: the next run starts its own count.
            last.retain(|k, _| live.contains(k));
            for key in live {
                let (at, since) = *last.entry(key.clone()).or_insert((Instant::now(), SystemTime::now()));
                if at.elapsed() >= every {
                    due.push((key, since));
                }
            }
        }
        for (key, since) in due {
            let started = SystemTime::now();
            if let Some(id) = key.strip_prefix("s:") {
                back_up_server(id, keep).await;
            } else if let Some(id) = key.strip_prefix("i:") {
                back_up_instance(id, since, keep).await;
            }
            if let Some(last) = LAST.lock().unwrap().as_mut() {
                last.insert(key, (Instant::now(), started));
            }
        }
    }
}

async fn back_up_server(id: &str, keep: usize) {
    let Ok(meta) = store::get(id).await else { return };
    // A Bedrock world can only be copied while the server is off.
    if meta.kind.bedrock() {
        return;
    }
    match runtime::backup_as(id, SUFFIX).await {
        Ok(path) => prune(Path::new(&path), keep).await,
        Err(e) => eprintln!("[autobackup] {}: {e:#}", meta.name),
    }
}

/// Every world the game saved since `since` (the last auto backup, or when the game started).
async fn back_up_instance(id: &str, since: SystemTime, keep: usize) {
    let Ok(saves) = files::game_subdir(id, "saves").await else { return };
    let Ok(mut rd) = tokio::fs::read_dir(&saves).await else { return };
    while let Ok(Some(entry)) = rd.next_entry().await {
        let level = entry.path().join("level.dat");
        let Ok(changed) = tokio::fs::metadata(&level).await.and_then(|m| m.modified()) else { continue };
        if changed < since {
            continue;
        }
        let folder = entry.file_name().to_string_lossy().into_owned();
        match worlds::backup_world_as(id, &folder, SUFFIX).await {
            Ok(path) => prune(Path::new(&path), keep).await,
            Err(e) => eprintln!("[autobackup] {id}/{folder}: {e:#}"),
        }
    }
}

/// Keeps the newest `keep` auto backups of the same world as `made` (the one just written) and deletes the rest.
async fn prune(made: &Path, keep: usize) {
    let (Some(dir), Some(name)) = (made.parent(), made.file_name().and_then(|n| n.to_str())) else { return };
    let Some(world) = world_of(name) else { return };
    let Ok(mut rd) = tokio::fs::read_dir(dir).await else { return };
    let mut mine = Vec::new();
    while let Ok(Some(e)) = rd.next_entry().await {
        let n = e.file_name().to_string_lossy().into_owned();
        if world_of(&n).as_deref() == Some(world.as_str()) {
            mine.push(n);
        }
    }
    for old in oldest_beyond(mine, keep) {
        if let Err(e) = tokio::fs::remove_file(dir.join(&old)).await {
            eprintln!("[autobackup] couldn't delete {old}: {e}");
        }
    }
}

/// "2026-10-27_14-30-00_world_auto.zip" (maybe with a " (2)" before .zip) → "world"; None for other files.
fn world_of(name: &str) -> Option<String> {
    let stem = name.strip_suffix(".zip")?;
    let stem = match stem.rfind(" (") {
        Some(i) if stem.ends_with(')') => &stem[..i],
        _ => stem,
    };
    let rest = stem.strip_suffix(SUFFIX)?;
    // The stamp is "YYYY-MM-DD_HH-MM-SS_" (20 characters).
    let (stamp, world) = (rest.get(..20)?, rest.get(20..)?);
    let ok = stamp.len() == 20 && stamp.as_bytes()[10] == b'_' && stamp.ends_with('_') && stamp[..19].chars().all(|c| c.is_ascii_digit() || c == '-' || c == '_');
    (ok && !world.is_empty()).then(|| world.to_string())
}

/// The names past the newest `keep` (names start with the stamp, so they sort oldest first).
fn oldest_beyond(mut names: Vec<String>, keep: usize) -> Vec<String> {
    names.sort();
    let extra = names.len().saturating_sub(keep);
    names.into_iter().take(extra).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_backups_are_told_apart() {
        assert_eq!(world_of("2026-10-27_14-30-00_world_auto.zip").as_deref(), Some("world"));
        assert_eq!(world_of("2026-10-27_14-30-00_My_World_auto (2).zip").as_deref(), Some("My_World"));
        // The player's own backups are never touched.
        assert_eq!(world_of("2026-10-27_14-30-00_world.zip"), None);
        assert_eq!(world_of("world_auto.zip"), None);
        assert_eq!(world_of("notes.txt"), None);
    }

    #[test]
    fn keeps_the_newest_three() {
        let names = ["2026-10-27_10-00-00_w_auto.zip", "2026-10-27_11-00-00_w_auto.zip", "2026-10-27_09-00-00_w_auto.zip", "2026-10-27_12-00-00_w_auto.zip"];
        let gone = oldest_beyond(names.iter().map(|s| s.to_string()).collect(), 3);
        assert_eq!(gone, vec!["2026-10-27_09-00-00_w_auto.zip".to_string()]);
        assert!(oldest_beyond(vec!["a".into()], 3).is_empty());
    }
}
