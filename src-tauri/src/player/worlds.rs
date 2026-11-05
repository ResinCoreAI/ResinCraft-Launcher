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
    backup_world_as(id, folder, "").await
}

/// `backup_world` with `suffix` after the folder in the zip's name ("_auto" for auto backups).
pub async fn backup_world_as(id: &str, folder: &str, suffix: &str) -> Result<String> {
    let dir = world_path(id, folder).await?;
    tokio::fs::metadata(&dir).await?;
    let backups = game_subdir(id, "backups").await?;
    let target = backups.join(unique_name(&backups, &format!("{}_{folder}{suffix}", stamp()), ".zip").await);
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
        return "World".into();
    }
    // Windows won't make a folder called CON, NUL, COM1… (with or without an extension).
    let stem = out.split('.').next().unwrap_or("").trim().to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
        || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.as_bytes()[3].is_ascii_digit());
    if reserved {
        format!("{out}_")
    } else {
        out
    }
}

fn base_name(p: &str) -> &str {
    p.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or("")
}

/// Imports a world from a zip that has level.dat at its root or inside one folder.
pub async fn import_world(id: &str, zip_path: &Path) -> Result<WorldInfo> {
    unpack_world(id, zip_path, None).await
}

/// The instance's world backups (manual and auto), newest first.
pub async fn list_backups(id: &str) -> Result<Vec<crate::core::backupfiles::BackupFile>> {
    Ok(crate::core::backupfiles::list(&game_subdir(id, "backups").await?).await)
}

/// Puts a backup back: over its world (the current one goes to the Recycle Bin first), or `as_copy` as a new world.
pub async fn restore_backup(id: &str, file: &str, as_copy: bool) -> Result<WorldInfo> {
    let dir = game_subdir(id, "backups").await?;
    let zip = crate::core::backupfiles::path_of(&dir, file)?;
    if as_copy {
        return unpack_world(id, &zip, None).await;
    }
    let world = crate::core::backupfiles::list(&dir).await.into_iter().find(|b| b.file == file).map(|b| b.world);
    unpack_world(id, &zip, Some(world.unwrap_or_else(|| file.trim_end_matches(".zip").to_string()))).await
}

pub async fn delete_backup(id: &str, file: &str) -> Result<()> {
    crate::core::backupfiles::delete(&game_subdir(id, "backups").await?, file).await
}

/// Where the world sits in a zip: the folder its level.dat is in ("" for the root, else ending in "/") and the
/// zip entries below it as (index, path inside the world).
#[derive(Debug, PartialEq)]
pub(crate) struct WorldPlan {
    pub prefix: String,
    pub files: Vec<(usize, String)>,
}

/// A zip entry's path that stays inside the folder it's unpacked to: relative, no `..`, no drive or stream (`:`).
fn entry_is_safe(p: &str) -> bool {
    !p.is_empty() && !p.starts_with('/') && !p.contains([':', '\0']) && p.split('/').all(|s| s != "..")
}

/// Finds the world in a zip's file entries (paths with `/`). The whole zip is refused if any entry would land
/// outside the world's folder, or when there's no level.dat within `max_depth` folders of the root (None: any depth).
pub(crate) fn plan_world(entries: Vec<(usize, String)>, max_depth: Option<usize>) -> Result<WorldPlan> {
    if let Some((_, bad)) = entries.iter().find(|(_, p)| !entry_is_safe(p)) {
        bail_tr!("worlds.unsafeZip", "That zip has a file outside its own folders ({path}), so it wasn't unpacked", path = bad.clone());
    }
    let depth = |p: &str| p.split('/').count() - 1;
    let mut levels: Vec<&str> = entries
        .iter()
        .map(|(_, p)| p.as_str())
        .filter(|p| (*p == "level.dat" || p.ends_with("/level.dat")) && max_depth.is_none_or(|m| depth(p) <= m))
        .collect();
    if levels.is_empty() {
        bail_tr!("worlds.noWorldInZip", "That zip doesn't contain a Minecraft world (no level.dat)");
    }
    // The shallowest level.dat is the world root; deeper ones belong to nested dimensions or junk.
    levels.sort_by_key(|p| depth(p));
    let prefix = levels[0][..levels[0].len() - "level.dat".len()].to_string();
    let files = entries
        .into_iter()
        .filter(|(_, p)| p.starts_with(&prefix) && base_name(p) != "session.lock")
        .map(|(i, p)| (i, p[prefix.len()..].to_string()))
        .collect();
    Ok(WorldPlan { prefix, files })
}

/// The (index, path) of every file entry in a zip, with `\` turned into `/`.
fn zip_entries(zip_path: &Path) -> Result<Vec<(usize, String)>> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(zip_path)?).map_err(|_| err_tr!("worlds.invalidZip", "That file isn't a valid zip"))?;
    let mut out = Vec::new();
    for i in 0..zip.len() {
        let f = zip.by_index_raw(i)?;
        if !f.is_dir() {
            out.push((i, f.name().replace('\\', "/")));
        }
    }
    Ok(out)
}

/// Writes the planned files into `dest`, which must not exist yet (so nothing is ever overwritten). On failure the
/// folder this created is removed again.
fn extract_world(zip_path: &Path, files: &[(usize, String)], dest: &Path, progress: impl Fn(usize, usize)) -> Result<()> {
    std::fs::create_dir(dest)?;
    let res = (|| -> Result<()> {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(zip_path)?)?;
        let n = files.len();
        for (k, (i, rel)) in files.iter().enumerate() {
            let out = safe_join(dest, rel)?;
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut entry = zip.by_index(*i)?;
            let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&out)?;
            std::io::copy(&mut entry, &mut f)?;
            progress(k + 1, n);
        }
        Ok(())
    })();
    if res.is_err() {
        // Only ever removes the folder this call just created.
        let _ = std::fs::remove_dir_all(dest);
    }
    res
}

/// Unpacks a world zip into saves/: into a new folder, or into `replace` (whose current world goes to the Recycle
/// Bin first, so a restore can be undone).
async fn unpack_world(id: &str, zip_path: &Path, replace: Option<String>) -> Result<WorldInfo> {
    assert_stopped(id, tr!("worlds.closeToImport", "Close the game before you import a world"))?;
    if !zip_path.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
        bail_tr!("worlds.chooseZip", "Choose a .zip file");
    }
    let saves = saves_dir(id).await?;
    let zp = zip_path.to_path_buf();
    let plan = tokio::task::spawn_blocking(move || zip_entries(&zp).and_then(|e| plan_world(e, None))).await??;
    let base = if plan.prefix.is_empty() {
        clean_folder_name(&zip_path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())
    } else {
        clean_folder_name(base_name(&plan.prefix))
    };
    let folder = match replace {
        Some(name) => {
            let folder = clean_folder_name(&name);
            let current = child_path(&saves, &folder)?;
            if tokio::fs::metadata(&current).await.is_ok() {
                crate::core::shell::trash(&current).await?;
            }
            folder
        }
        None => unique_name(&saves, &base, "").await,
    };
    let dest = child_path(&saves, &folder)?;
    let zp = zip_path.to_path_buf();

    Task::run_to_end(tr!("worlds.importing", "Importing {folder}", folder = folder), Some(id.to_string()), |t| async move {
        let d = dest.clone();
        tokio::task::spawn_blocking(move || {
            extract_world(&zp, &plan.files, &d, |done, total| {
                t.update(tr!("worlds.extracting", "Extracting ({done}/{total})", done = done, total = total), Some(done as f64 / total as f64))
            })
        })
        .await??;
        Ok(read_world(&dest, &folder).await)
    })
    .await
}

/// Puts a downloaded map into `saves` under a new folder named after `name` ("name (2)"… when that's taken; an
/// existing world is never touched). Strict: level.dat must be at the zip's root or one folder down and readable,
/// or nothing is kept. `progress` gets (done, total) files.
pub(crate) async fn unpack_download(saves: &Path, zip_path: &Path, name: &str, progress: impl Fn(usize, usize) + Send + 'static) -> Result<WorldInfo> {
    let zp = zip_path.to_path_buf();
    let plan = tokio::task::spawn_blocking(move || zip_entries(&zp).and_then(|e| plan_world(e, Some(1)))).await??;
    let folder = unique_name(saves, &clean_folder_name(name), "").await;
    let dest = child_path(saves, &folder)?;
    let (zp, d) = (zip_path.to_path_buf(), dest.clone());
    tokio::task::spawn_blocking(move || extract_world(&zp, &plan.files, &d, progress)).await??;
    let info = read_world(&dest, &folder).await;
    if info.error.is_some() {
        let _ = tokio::fs::remove_dir_all(&dest).await;
        bail_tr!("worlds.notAWorld", "That download isn't a Minecraft world (its level.dat can't be read)");
    }
    Ok(info)
}

/// [`unpack_download`] into an instance's saves/.
pub async fn install_downloaded(id: &str, zip_path: &Path, name: &str, progress: impl Fn(usize, usize) + Send + 'static) -> Result<WorldInfo> {
    unpack_download(&saves_dir(id).await?, zip_path, name, progress).await
}

// ---------- Other instances ----------

/// In a world lent to another instance for one game: where it goes back to.
const LENT: &str = "resincraft-lent.json";

#[derive(serde::Serialize, serde::Deserialize)]
struct Lent {
    instance: String,
    folder: String,
}

/// The other instance, checked to exist and to be a different one, with its game closed.
async fn other_instance(from: &str, to: &str) -> Result<crate::core::types::Instance> {
    if from == to {
        bail_tr!("worlds.sameInstance", "Pick a different instance");
    }
    let other = crate::core::instances::get(to).await?;
    assert_stopped(to, tr!("worlds.closeOther", "Close the game in {name} first", name = other.name))?;
    Ok(other)
}

/// The world's folder, checked to hold a world (level.dat).
async fn existing_world(id: &str, folder: &str) -> Result<PathBuf> {
    let dir = world_path(id, folder).await?;
    if !dir.join("level.dat").is_file() {
        bail_tr!("worlds.notFound", "{folder} isn't a world in this instance any more.", folder = folder);
    }
    Ok(dir)
}

/// Moves or copies (`keep`) a world into another instance's saves/, under a free folder name.
pub async fn transfer_world(from: &str, folder: &str, to: &str, keep: bool) -> Result<WorldInfo> {
    let other = other_instance(from, to).await?;
    let close = if keep { tr!("worlds.closeToCopy", "Close the game before you copy this world") } else { tr!("worlds.closeToMove", "Close the game before you move this world") };
    assert_stopped(from, close)?;
    let src = existing_world(from, folder).await?;
    let saves = saves_dir(to).await?;
    let name = unique_name(&saves, folder, "").await;
    let dest = child_path(&saves, &name)?;
    // Instances share one folder, so a move is usually just a rename.
    if !keep && tokio::fs::rename(&src, &dest).await.is_ok() {
        return Ok(read_world(&dest, &name).await);
    }
    let title = if keep {
        tr!("worlds.copyingTo", "Copying {folder} to {name}", folder = folder, name = other.name)
    } else {
        tr!("worlds.movingTo", "Moving {folder} to {name}", folder = folder, name = other.name)
    };
    let (s, d) = (src.clone(), dest.clone());
    Task::run_to_end(title, Some(to.to_string()), |t| async move {
        t.update(tr!("worlds.copyingFiles", "Copying files…"), None);
        let res = tokio::task::spawn_blocking(move || -> Result<()> {
            use std::sync::atomic::{AtomicBool, AtomicU64};
            let jobs = crate::core::fsutil::list_copy(&s, &d, &["session.lock"])?;
            let failed = crate::core::fsutil::copy_many(&jobs, &AtomicBool::new(false), &AtomicU64::new(0));
            match failed.into_iter().next() {
                Some((_, e)) => Err(e.into()),
                None => Ok(()),
            }
        })
        .await
        .map_err(anyhow::Error::from)
        .and_then(|r| r);
        if let Err(e) = res {
            // Only ever removes the folder this copy just made.
            let _ = tokio::fs::remove_dir_all(&dest).await;
            return Err(e);
        }
        if !keep {
            crate::core::shell::trash(&src).await?;
        }
        Ok(read_world(&dest, &name).await)
    })
    .await
}

/// Plays a world in another instance: the folder moves into that instance's saves/ for the game and back to its
/// own instance once the game closes. A world from another Minecraft version is backed up first, since the game
/// upgrades it.
pub async fn play_elsewhere(from: &str, folder: &str, to: &str, account: Option<&str>) -> Result<()> {
    let other = other_instance(from, to).await?;
    assert_stopped(from, tr!("worlds.closeToLend", "Close the game in this instance before you play its world somewhere else"))?;
    let src = existing_world(from, folder).await?;
    let world = read_world(&src, folder).await;
    if world.version.as_deref().is_some_and(|v| v != other.mc_version) {
        backup_world_as(from, folder, "_before_other_version").await?;
    }
    let saves = saves_dir(to).await?;
    let name = unique_name(&saves, folder, "").await;
    let dest = child_path(&saves, &name)?;
    tokio::fs::rename(&src, &dest).await?;
    let lent = Lent { instance: from.to_string(), folder: folder.to_string() };
    if let Err(e) = crate::core::fsutil::write_json(&dest.join(LENT), &lent).await {
        let _ = tokio::fs::rename(&dest, &src).await;
        return Err(e);
    }
    let back = dest.clone();
    let after: Box<dyn FnOnce() + Send> = Box::new(move || {
        tokio::spawn(async move {
            if let Err(e) = give_back(&back).await {
                eprintln!("[worlds] couldn't return the lent world {}: {e:#}", back.display());
            }
        });
    });
    if let Err(e) = crate::game::launch_then(to, account, Some(crate::game::launch::Target::World(name)), Some(after)).await {
        let _ = give_back(&dest).await;
        return Err(e);
    }
    Ok(())
}

/// Moves a lent world back to its own instance (under a free name if that one was taken meanwhile).
async fn give_back(dir: &Path) -> Result<()> {
    let marker = dir.join(LENT);
    let Some(lent) = crate::core::fsutil::read_json::<Lent>(&marker).await else { return Ok(()) };
    // Its instance is gone: the world stays where it is.
    if crate::core::instances::get(&lent.instance).await.is_err() {
        return Ok(tokio::fs::remove_file(&marker).await?);
    }
    let saves = saves_dir(&lent.instance).await?;
    let home = child_path(&saves, &unique_name(&saves, &clean_folder_name(&lent.folder), "").await)?;
    tokio::fs::remove_file(&marker).await?;
    if let Err(e) = tokio::fs::rename(dir, &home).await {
        // Keep the way back for the next start.
        let _ = crate::core::fsutil::write_json(&marker, &lent).await;
        return Err(e.into());
    }
    Ok(())
}

/// At start: returns worlds still lent out (ResinCraft closed while the game ran), unless that game is running.
pub async fn return_lent_worlds() {
    let running = crate::game::running_ids();
    for inst in crate::core::instances::list().await.unwrap_or_default() {
        if running.contains(&inst.id) {
            continue;
        }
        let Ok(dir) = saves_dir(&inst.id).await else { continue };
        let Ok(mut rd) = tokio::fs::read_dir(&dir).await else { continue };
        while let Ok(Some(e)) = rd.next_entry().await {
            if e.path().join(LENT).is_file() {
                if let Err(err) = give_back(&e.path()).await {
                    eprintln!("[worlds] couldn't return the lent world {}: {err:#}", e.path().display());
                }
            }
        }
    }
}

/// Moves the world folder to the Recycle Bin, never a permanent delete.
pub async fn remove_world(id: &str, folder: &str) -> Result<()> {
    assert_stopped(id, tr!("worlds.closeToDelete", "Close the game before you delete this world"))?;
    let dir = world_path(id, folder).await?;
    tokio::fs::metadata(&dir).await?;
    crate::core::shell::trash(&dir).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn entries(paths: &[&str]) -> Vec<(usize, String)> {
        paths.iter().enumerate().map(|(i, p)| (i, p.to_string())).collect()
    }

    #[test]
    fn finds_the_world_at_the_root_or_one_folder_down() {
        let p = plan_world(entries(&["level.dat", "region/r.0.0.mca", "session.lock"]), Some(1)).unwrap();
        assert_eq!(p.prefix, "");
        assert_eq!(p.files, vec![(0, "level.dat".into()), (1, "region/r.0.0.mca".into())]);

        let p = plan_world(entries(&["Map/level.dat", "Map/region/r.0.0.mca", "readme.txt", "Map/DIM-1/level.dat"]), Some(1)).unwrap();
        assert_eq!(p.prefix, "Map/");
        assert_eq!(p.files, vec![(0, "level.dat".into()), (1, "region/r.0.0.mca".into()), (3, "DIM-1/level.dat".into())]);
    }

    #[test]
    fn refuses_zips_that_are_not_worlds() {
        assert!(plan_world(entries(&["pack.mcmeta", "assets/x.png"]), Some(1)).is_err());
        assert!(plan_world(entries(&[]), Some(1)).is_err());
        // Two folders down is too deep for a download, though a backup import still accepts it.
        assert!(plan_world(entries(&["a/b/level.dat"]), Some(1)).is_err());
        assert_eq!(plan_world(entries(&["a/b/level.dat"]), None).unwrap().prefix, "a/b/");
        // Only a file called exactly level.dat counts.
        assert!(plan_world(entries(&["Map/level.dat_old", "xlevel.dat"]), Some(1)).is_err());
    }

    #[test]
    fn refuses_path_traversal() {
        for bad in ["../evil.txt", "Map/../../evil.txt", "/etc/passwd", "C:/Windows/evil.dll", "Map/file.txt:stream", "Map/..", ""] {
            let list = entries(&["Map/level.dat", bad]);
            assert!(plan_world(list, Some(1)).is_err(), "{bad:?} should be refused");
        }
        assert!(entry_is_safe("Map/..data/x"));
        assert!(entry_is_safe("Map/region/r.0.0.mca"));
    }

    #[test]
    fn folder_names_are_safe_for_windows() {
        assert_eq!(clean_folder_name("Sky: Block?"), "Sky_ Block_");
        assert_eq!(clean_folder_name("  ... "), "World");
        assert_eq!(clean_folder_name("con"), "con_");
        assert_eq!(clean_folder_name("COM1.txt"), "COM1.txt_");
        assert_eq!(clean_folder_name("Console"), "Console");
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("resincraft-worlds-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_zip(path: &Path, files: &[(&str, &[u8])]) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, data) in files {
            zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }

    /// A minimal level.dat: gzipped NBT with Data.LevelName.
    fn level_dat(name: &str) -> Vec<u8> {
        let mut data = Compound::default();
        data.set("LevelName", Tag::String(name.into()));
        let mut root = Compound::default();
        root.set("Data", Tag::Compound(data));
        nbt::write(&NbtFile { name: String::new(), root, compression: nbt::Compression::Gzip }).unwrap()
    }

    #[tokio::test]
    async fn never_overwrites_a_world_with_the_same_name() {
        let dir = temp_dir("clash");
        let saves = dir.join("saves");
        std::fs::create_dir_all(saves.join("Skyblock")).unwrap();
        std::fs::write(saves.join("Skyblock/level.dat"), b"mine").unwrap();
        std::fs::create_dir_all(saves.join("skyblock (2)")).unwrap();
        let zip = dir.join("map.zip");
        write_zip(&zip, &[("Skyblock/level.dat", &level_dat("Skyblock")), ("Skyblock/region/r.0.0.mca", b"r")]);

        let info = unpack_download(&saves, &zip, "Skyblock", |_, _| {}).await.unwrap();
        assert_eq!(info.folder, "Skyblock (3)");
        assert_eq!(info.name, "Skyblock");
        assert!(info.error.is_none());
        assert_eq!(std::fs::read(saves.join("Skyblock/level.dat")).unwrap(), b"mine");
        assert_eq!(std::fs::read(saves.join("Skyblock (3)/region/r.0.0.mca")).unwrap(), b"r");

        let again = unpack_download(&saves, &zip, "Skyblock", |_, _| {}).await.unwrap();
        assert_eq!(again.folder, "Skyblock (4)");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn bad_downloads_leave_nothing_behind() {
        let dir = temp_dir("bad");
        let saves = dir.join("saves");
        std::fs::create_dir_all(&saves).unwrap();
        let count = || std::fs::read_dir(&saves).unwrap().count();

        let evil = dir.join("evil.zip");
        write_zip(&evil, &[("level.dat", &level_dat("X")), ("../../escaped.txt", b"x")]);
        assert!(unpack_download(&saves, &evil, "Evil", |_, _| {}).await.is_err());
        assert!(!dir.join("escaped.txt").exists() && !std::env::temp_dir().join("escaped.txt").exists());

        let not_world = dir.join("pack.zip");
        write_zip(&not_world, &[("pack.mcmeta", b"{}")]);
        assert!(unpack_download(&saves, &not_world, "Pack", |_, _| {}).await.is_err());

        let broken = dir.join("broken.zip");
        write_zip(&broken, &[("Map/level.dat", b"not nbt")]);
        assert!(unpack_download(&saves, &broken, "Broken", |_, _| {}).await.is_err());

        let not_zip = dir.join("x.zip");
        std::fs::write(&not_zip, b"hello").unwrap();
        assert!(unpack_download(&saves, &not_zip, "X", |_, _| {}).await.is_err());

        assert_eq!(count(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
