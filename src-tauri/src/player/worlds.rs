//! The instance page's Worlds tab: list, rename, back up, import and remove singleplayer worlds.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use base64::Engine;

use super::files::{child_path, folder_size, game_subdir, js_len, unique_name, write_atomic};
use super::nbt::{self, compound, long, num, str, Compound, NbtFile, Tag};
use crate::core::events::Task;
use crate::core::fsutil::safe_join;
use crate::core::text::Text;
use crate::core::types::WorldInfo;
use crate::{bail_tr, err_tr, tr};

const GAME_MODES: [&str; 4] = ["survival", "creative", "adventure", "spectator"];

async fn saves_dir(id: &str) -> Result<PathBuf> {
    game_subdir(id, "saves").await
}

/// The folder of one world, checked to be a direct child of saves/.
pub async fn world_path(id: &str, folder: &str) -> Result<PathBuf> {
    child_path(&saves_dir(id).await?, folder)
}

/// `message` says what to close the game for, e.g. "Close the game before you rename this world".
fn assert_stopped(id: &str, message: Text) -> Result<()> {
    if crate::game::running_ids().iter().any(|r| r == id) {
        return Err(message.into());
    }
    Ok(())
}

async fn read_level(dir: &Path) -> Result<NbtFile> {
    // level.dat_old is the game's own previous copy; use it if the main file is damaged.
    let mut last_error = anyhow!("No level.dat found");
    for name in ["level.dat", "level.dat_old"] {
        match tokio::fs::read(dir.join(name)).await {
            Ok(bytes) => match nbt::parse(&bytes) {
                Ok(f) => return Ok(f),
                Err(e) => last_error = e,
            },
            Err(e) => last_error = e.into(),
        }
    }
    Err(last_error)
}

/// JS truthiness of an optional number (`!!x`).
fn truthy(v: Option<f64>) -> bool {
    matches!(v, Some(n) if n != 0.0 && !n.is_nan())
}

fn describe_level(data: &Compound, folder: &str, size: u64, icon: Option<String>) -> WorldInfo {
    // Newer versions moved a few fields around, so look in both places.
    let difficulty = compound(data.get("difficulty_settings"));
    let player = compound(data.get("Player"));
    let mode = num(data.get("GameType")).or_else(|| num(player.and_then(|p| p.get("playerGameType"))));
    let gen = compound(data.get("WorldGenSettings"));
    let seed = long(gen.and_then(|g| g.get("seed"))).or_else(|| long(data.get("RandomSeed")));
    let last_played = num(data.get("LastPlayed"));
    let hardcore = num(data.get("hardcore")).or_else(|| num(difficulty.and_then(|d| d.get("hardcore"))));
    let name = str(data.get("LevelName")).unwrap_or_default();
    WorldInfo {
        folder: folder.to_string(),
        name: if name.is_empty() { folder.to_string() } else { name.to_string() },
        game_mode: mode.filter(|m| m.fract() == 0.0 && (0.0..4.0).contains(m)).map(|m| GAME_MODES[m as usize].to_string()),
        hardcore: truthy(hardcore),
        cheats: truthy(num(data.get("allowCommands"))),
        last_played: last_played.filter(|v| truthy(Some(*v))).map(|v| v as i64),
        version: str(compound(data.get("Version")).and_then(|v| v.get("Name"))).map(String::from),
        seed: seed.map(|s| s.to_string()),
        icon,
        size,
        error: None,
        error_text: None,
    }
}

async fn read_world(dir: &Path, folder: &str) -> WorldInfo {
    let (size, icon) = tokio::join!(folder_size(dir), async {
        tokio::fs::read(dir.join("icon.png"))
            .await
            .ok()
            .map(|b| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b)))
    });
    let described = read_level(dir).await.and_then(|level| {
        let data = compound(level.root.get("Data")).ok_or_else(|| err_tr!("worlds.noDataSection", "level.dat has no Data section"))?;
        Ok(describe_level(data, folder, size, icon.clone()))
    });
    match described {
        Ok(info) => info,
        Err(e) => {
            let missing = e.downcast_ref::<std::io::Error>().is_some_and(|io| io.kind() == ErrorKind::NotFound);
            let error = if missing { tr!("worlds.noLevelDat", "No level.dat found") } else { tr!("worlds.levelUnreadable", "level.dat could not be read") };
            WorldInfo {
                folder: folder.to_string(),
                name: folder.to_string(),
                game_mode: None,
                hardcore: false,
                cheats: false,
                last_played: None,
                version: None,
                seed: None,
                icon,
                size,
                error: Some(error.text.clone()),
                error_text: Some(error),
            }
        }
    }
}

pub async fn list_worlds(id: &str) -> Result<Vec<WorldInfo>> {
    let dir = saves_dir(id).await?;
    let mut rd = tokio::fs::read_dir(&dir).await?;
    let mut folders = Vec::new();
    while let Some(e) = rd.next_entry().await? {
        if e.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
            folders.push(e.file_name().to_string_lossy().into_owned());
        }
    }
    let mut worlds = futures::future::join_all(folders.iter().map(|f| {
        let path = dir.join(f);
        async move { read_world(&path, f).await }
    })).await;
    worlds.sort_by(|a, b| {
        b.last_played
            .unwrap_or(0)
            .cmp(&a.last_played.unwrap_or(0))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(worlds)
}

/// A world in one of the instances, for Quick join's Worlds.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentWorld {
    pub instance_id: String,
    #[serde(flatten)]
    pub world: WorldInfo,
}

/// The `limit` most recently played worlds across every instance. Only level.dat's date is checked for all of them;
/// the full details are read for the ones returned.
pub async fn recent_worlds(limit: usize) -> Result<Vec<RecentWorld>> {
    let mut found: Vec<(std::time::SystemTime, String, String, PathBuf)> = Vec::new();
    for inst in crate::core::instances::list().await.unwrap_or_default() {
        let Ok(dir) = saves_dir(&inst.id).await else { continue };
        let Ok(mut rd) = tokio::fs::read_dir(&dir).await else { continue };
        while let Ok(Some(e)) = rd.next_entry().await {
            let path = e.path();
            let Ok(meta) = tokio::fs::metadata(path.join("level.dat")).await else { continue };
            let when = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            found.push((when, inst.id.clone(), e.file_name().to_string_lossy().into_owned(), path));
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found.truncate(limit);
    Ok(futures::future::join_all(found.into_iter().map(|(_, instance_id, folder, path)| async move {
        RecentWorld { instance_id, world: read_world(&path, &folder).await }
    }))
    .await)
}

/// Changes the name shown in game. The folder keeps its name, as in vanilla.
pub async fn rename_world(id: &str, folder: &str, name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        bail_tr!("worlds.nameMissing", "Enter a name for the world");
    }
    if js_len(name) > 100 {
        bail_tr!("worlds.nameTooLong", "That name is too long");
    }
    assert_stopped(id, tr!("worlds.closeToRename", "Close the game before you rename this world"))?;
    let dir = world_path(id, folder).await?;
    let file = dir.join("level.dat");
    let mut level = nbt::parse(&tokio::fs::read(&file).await?)?;
    let Some(Tag::Compound(data)) = level.root.get_mut("Data") else { bail_tr!("worlds.noDataSection", "level.dat has no Data section") };
    data.set("LevelName", Tag::String(name.to_string()));
    let out = nbt::write(&level)?;
    // Sanity check before touching the real file: what we wrote must read back.
    let check = nbt::parse(&out)?;
    if str(compound(check.root.get("Data")).and_then(|d| d.get("LevelName"))) != Some(name) {
        bail_tr!("worlds.writeFailed", "Could not write level.dat");
    }
    tokio::fs::copy(&file, dir.join("level.dat_old")).await?;
    write_atomic(&file, &out).await
}

fn stamp() -> String {
    chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string()
}

/// Adds every file below `dir` to the zip under `prefix/`, skipping session.lock.
pub(crate) fn zip_folder<W: std::io::Write + std::io::Seek>(zip: &mut zip::ZipWriter<W>, dir: &Path, prefix: &str) -> Result<()> {
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(true);
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<std::io::Result<_>>()?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let name = e.file_name().to_string_lossy().into_owned();
        // session.lock is held open by a running game and isn't part of the world anyway.
        if name == "session.lock" {
            continue;
        }
        let path = format!("{prefix}/{name}");
        let ft = e.file_type()?;
        if ft.is_dir() {
            zip.add_directory(format!("{path}/"), opts)?;
            zip_folder(zip, &e.path(), &path)?;
        } else if ft.is_file() {
            zip.start_file(path, opts)?;
            let mut f = std::fs::File::open(e.path())?;
            std::io::copy(&mut f, zip)?;
        }
    }
    Ok(())
}

/// Zips a world into minecraft/backups (the same folder vanilla's "Make Backup" uses).
pub async fn backup_world(id: &str, folder: &str) -> Result<String> {
    let dir = world_path(id, folder).await?;
    tokio::fs::metadata(&dir).await?;
    let backups = game_subdir(id, "backups").await?;
    let target = backups.join(unique_name(&backups, &format!("{}_{folder}", stamp()), ".zip").await);
    let folder = folder.to_string();
    Task::run_to_end(tr!("worlds.backingUp", "Backing up {folder}", folder = folder), Some(id.to_string()), |t| async move {
        t.update(tr!("worlds.compressing", "Compressing files…"), None);
        let tmp = PathBuf::from(format!("{}.part", target.display()));
        let (tmp2, t2) = (tmp.clone(), t.clone());
        let res = tokio::task::spawn_blocking(move || -> Result<()> {
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&tmp2)?);
            zip_folder(&mut zip, &dir, &folder)?;
            t2.update(tr!("worlds.writingZip", "Writing zip…"), None);
            zip.finish()?;
            Ok(())
        })
        .await
        .map_err(anyhow::Error::from)
        .and_then(|r| r);
        if let Err(e) = res {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(e);
        }
        tokio::fs::rename(&tmp, &target).await?;
        Ok(target.to_string_lossy().into_owned())
    })
    .await
}

/// Characters Windows won't accept in a folder name.
fn clean_folder_name(s: &str) -> String {
    let replaced: String = s.chars().map(|c| if "<>:\"/\\|?*".contains(c) || (c as u32) < 0x20 { '_' } else { c }).collect();
    let trimmed = replaced.trim_end_matches(['.', ' ']).trim();
    let out: String = trimmed.chars().take(64).collect();
    if out.is_empty() {
        "World".into()
    } else {
        out
    }
}

fn base_name(p: &str) -> &str {
    p.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or("")
}

/// Imports a world from a zip that has level.dat at its root or inside one folder.
pub async fn import_world(id: &str, zip_path: &Path) -> Result<WorldInfo> {
    assert_stopped(id, tr!("worlds.closeToImport", "Close the game before you import a world"))?;
    if !zip_path.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
        bail_tr!("worlds.chooseZip", "Choose a .zip file");
    }
    let saves = saves_dir(id).await?;
    let zp = zip_path.to_path_buf();
    // (index, normalised path) of every file entry.
    let entries: Vec<(usize, String)> = tokio::task::spawn_blocking(move || -> Result<Vec<(usize, String)>> {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&zp)?).map_err(|_| err_tr!("worlds.invalidZip", "That file isn't a valid zip"))?;
        let mut out = Vec::new();
        for i in 0..zip.len() {
            let f = zip.by_index_raw(i)?;
            if !f.is_dir() {
                out.push((i, f.name().replace('\\', "/")));
            }
        }
        Ok(out)
    })
    .await??;
    let mut levels: Vec<&str> = entries.iter().map(|(_, p)| p.as_str()).filter(|p| *p == "level.dat" || p.ends_with("/level.dat")).collect();
    if levels.is_empty() {
        bail_tr!("worlds.noWorldInZip", "That zip doesn't contain a Minecraft world (no level.dat)");
    }
    // The shallowest level.dat is the world root; deeper ones belong to nested dimensions or junk.
    levels.sort_by_key(|p| p.split('/').count());
    let prefix = levels[0][..levels[0].len() - "level.dat".len()].to_string();
    let base = if prefix.is_empty() {
        clean_folder_name(&zip_path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())
    } else {
        clean_folder_name(base_name(&prefix))
    };
    let folder = unique_name(&saves, &base, "").await;
    let dest = child_path(&saves, &folder)?;
    let files: Vec<(usize, String)> = entries
        .into_iter()
        .filter(|(_, p)| p.starts_with(&prefix) && base_name(p) != "session.lock")
        .map(|(i, p)| (i, p[prefix.len()..].to_string()))
        .collect();
    let zp = zip_path.to_path_buf();

    Task::run_to_end(tr!("worlds.importing", "Importing {folder}", folder = folder), Some(id.to_string()), |t| async move {
        tokio::fs::create_dir(&dest).await?;
        let d = dest.clone();
        let res = tokio::task::spawn_blocking(move || -> Result<()> {
            let mut zip = zip::ZipArchive::new(std::fs::File::open(&zp)?)?;
            let n = files.len();
            for (k, (i, rel)) in files.iter().enumerate() {
                let out = safe_join(&d, rel)?;
                if let Some(parent) = out.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut entry = zip.by_index(*i)?;
                let mut f = std::fs::File::create(&out)?;
                std::io::copy(&mut entry, &mut f)?;
                t.update(tr!("worlds.extracting", "Extracting ({done}/{total})", done = k + 1, total = n), Some((k + 1) as f64 / n as f64));
            }
            Ok(())
        })
        .await
        .map_err(anyhow::Error::from)
        .and_then(|r| r);
        if let Err(e) = res {
            // Only ever removes the folder this import just created.
            let _ = tokio::fs::remove_dir_all(&dest).await;
            return Err(e);
        }
        Ok(read_world(&dest, &folder).await)
    })
    .await
}

/// Moves the world folder to the Recycle Bin, never a permanent delete.
pub async fn remove_world(id: &str, folder: &str) -> Result<()> {
    assert_stopped(id, tr!("worlds.closeToDelete", "Close the game before you delete this world"))?;
    let dir = world_path(id, folder).await?;
    tokio::fs::metadata(&dir).await?;
    crate::core::shell::trash(&dir).await
}
