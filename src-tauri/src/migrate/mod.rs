//! "Move from another launcher" (`migrate:*`): finds Prism Launcher / MultiMC, Modrinth App, CurseForge,
//! ATLauncher, GDLauncher and official launcher data on this PC (or in a folder the player picks) and copies
//! each instance into a new ResinCraft instance, and server folders into ResinCraft servers: mods, configs,
//! worlds, packs, screenshots, options and server lists, with the game version, loader, memory, Java
//! arguments, name and icon. The other launcher's files are only read, never moved or changed.

pub mod copy;
pub mod formats;
pub mod remove;
pub mod server_dir;

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::content::index::InstanceGuard;
use crate::core::events::{is_cancelled, Task};
use crate::core::fsutil::{now_ms, read_json, write_json};
use crate::core::router::json;
use crate::core::settings::total_memory_mb;
use crate::core::text::{text_of, Text};
use crate::core::types::{ContentKind, LoaderType, NewInstance, ServerKind, ServerMeta, TunnelMode};
use crate::core::{instances, paths, shell, Args};
use crate::server::{installers, store};
use crate::{bail_tr, tr};

use copy::Skipped;
use formats::{Found, Icon, Launcher};

pub async fn handle(channel: &str, args: Args) -> Result<Value> {
    match channel {
        "migrate:scan" => json(scan().await?),
        "migrate:scanFolder" => {
            let path: Option<String> = args.get(0)?;
            let dir = match path {
                Some(p) => PathBuf::from(p),
                None => match shell::pick_folder("Choose another launcher's folder, an instance or a server").await? {
                    Some(p) => p,
                    None => return Ok(Value::Null),
                },
            };
            json(scan_folder(dir).await?)
        }
        "migrate:size" => json(size_of(args.get::<ItemRef>(0)?).await?),
        "migrate:import" => json(import(args.get::<Vec<ItemRef>>(0)?, args.get::<Option<bool>>(1)?.unwrap_or(false)).await?),
        _ => anyhow::bail!("Unknown request: {channel}"),
    }
}

// ---------- What the UI sees ----------

/// One launcher install (or picked folder) and what's in it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub launcher: Launcher,
    pub label: String,
    pub path: String,
    pub items: Vec<Item>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    #[serde(flatten)]
    pub r: ItemRef,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mc_version: Option<String>,
    pub loader: LoaderType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loader_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_type: Option<ServerKind>,
    pub mods: u32,
    pub worlds: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<Text>,
    /// Imported before (and that instance or server still exists).
    pub imported: bool,
    /// Why "Remove the originals" can't take this one out of its launcher (absent when it can).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove_blocked: Option<Text>,
}

/// Which item to import; the engine reads it again from disk rather than trusting the list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemRef {
    pub launcher: Launcher,
    /// The launcher's own folder (for Prism icons, Modrinth's database, official profiles).
    pub root: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default)]
    pub server: bool,
}

impl ItemRef {
    fn key(&self) -> String {
        format!("{:?}|{}|{}|{}", self.launcher, self.server, self.path, self.profile.as_deref().unwrap_or(""))
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub name: String,
    pub server: bool,
    /// The new instance or server.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Text>,
    pub skipped: Vec<Skipped>,
    /// The original was moved to the Recycle Bin.
    pub removed: bool,
    /// Why the original was kept, when removing was asked for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kept: Option<Text>,
}

impl Outcome {
    fn failed(name: String, server: bool, error: Text) -> Outcome {
        Outcome { name, server, id: None, error: Some(error), skipped: Vec::new(), removed: false, kept: None }
    }
}

// ---------- Where launchers keep their data ----------

fn home() -> PathBuf {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from).unwrap_or_default()
}

/// Default data folders, per system. Portable installs are found with "Choose a folder…".
fn default_roots() -> Vec<(Launcher, PathBuf)> {
    use Launcher::*;
    let h = home();
    let mut out = Vec::new();
    if cfg!(windows) {
        let appdata = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| h.join("AppData").join("Roaming"));
        out.extend([
            (Prism, appdata.join("PrismLauncher")),
            (Prism, appdata.join("PolyMC")),
            (Prism, h.join("scoop").join("persist").join("prismlauncher")),
            (Multimc, h.join("scoop").join("persist").join("multimc")),
            (Multimc, appdata.join("MultiMC")),
            (Multimc, PathBuf::from("C:\\MultiMC")),
            (Multimc, h.join("MultiMC")),
            (Multimc, h.join("Desktop").join("MultiMC")),
            (Modrinth, appdata.join("ModrinthApp")),
            (Modrinth, appdata.join("com.modrinth.theseus")),
            (Curseforge, h.join("curseforge").join("minecraft")),
            (Curseforge, h.join("Documents").join("Curse").join("Minecraft")),
            (Atlauncher, appdata.join("ATLauncher")),
            (Atlauncher, h.join("ATLauncher")),
            (Gdlauncher, appdata.join("gdlauncher_next")),
            (Gdlauncher, appdata.join("gdlauncher_carbon").join("data")),
            (Official, appdata.join(".minecraft")),
        ]);
    } else if cfg!(target_os = "macos") {
        let support = h.join("Library").join("Application Support");
        out.extend([
            (Prism, support.join("PrismLauncher")),
            (Multimc, support.join("MultiMC")),
            (Modrinth, support.join("ModrinthApp")),
            (Modrinth, support.join("com.modrinth.theseus")),
            (Curseforge, h.join("Documents").join("curseforge").join("minecraft")),
            (Atlauncher, support.join("ATLauncher")),
            (Gdlauncher, support.join("gdlauncher_next")),
            (Gdlauncher, support.join("gdlauncher_carbon").join("data")),
            (Official, support.join("minecraft")),
        ]);
    } else {
        let share = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| h.join(".local").join("share"));
        let config = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| h.join(".config"));
        out.extend([
            (Prism, share.join("PrismLauncher")),
            (Prism, h.join(".var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher")),
            (Prism, share.join("PolyMC")),
            (Multimc, share.join("multimc")),
            (Modrinth, share.join("ModrinthApp")),
            (Modrinth, share.join("com.modrinth.theseus")),
            (Curseforge, h.join("curseforge").join("minecraft")),
            (Atlauncher, share.join("atlauncher")),
            (Atlauncher, h.join(".atlauncher")),
            (Gdlauncher, config.join("gdlauncher_next")),
            (Gdlauncher, share.join("gdlauncher_carbon").join("data")),
            (Official, h.join(".minecraft")),
        ]);
    }
    out
}

fn children(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    v.sort();
    v
}

/// Which launcher `dir` is the data folder of, if any.
fn root_kind(dir: &Path) -> Option<Launcher> {
    let any_child = |sub: &str, file: &str| children(&dir.join(sub)).iter().any(|c| c.join(file).is_file());
    if dir.join("multimc.cfg").is_file() {
        return Some(Launcher::Multimc);
    }
    if dir.join("prismlauncher.cfg").is_file() || dir.join("polymc.cfg").is_file() || any_child("instances", "instance.cfg") {
        return Some(Launcher::Prism);
    }
    if dir.join("app.db").is_file() || any_child("profiles", "profile.json") {
        return Some(Launcher::Modrinth);
    }
    if any_child("Instances", "minecraftinstance.json") {
        return Some(Launcher::Curseforge);
    }
    if any_child("instances", "config.json")
        || dir.join("instances").is_dir() && children(&dir.join("instances")).iter().any(|c| formats::gdlauncher_instance(c).is_some())
    {
        return Some(Launcher::Gdlauncher);
    }
    if children(&dir.join("instances")).iter().any(|c| formats::atlauncher_instance(c).is_some()) || dir.join("configs").join("ATLauncher.json").is_file() {
        return Some(Launcher::Atlauncher);
    }
    if dir.join("launcher_profiles.json").is_file() {
        return Some(Launcher::Official);
    }
    None
}

/// Everything in one launcher data folder.
/// `f` over `items` on a few threads (reading hundreds of instance folders one by one is slow on Windows).
fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 8);
    let chunk = items.len().div_ceil(threads).max(1);
    std::thread::scope(|s| {
        let f = &f;
        let handles: Vec<_> = items.chunks(chunk).map(|c| s.spawn(move || c.iter().map(f).collect::<Vec<R>>())).collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    })
}

fn read_each(dirs: Vec<PathBuf>, f: impl Fn(&Path) -> Option<Found> + Sync) -> Vec<Found> {
    par_map(&dirs, |d| f(d)).into_iter().flatten().collect()
}

fn read_root(launcher: Launcher, root: &Path) -> Vec<Found> {
    match launcher {
        Launcher::Prism | Launcher::Multimc => read_each(children(&formats::mmc_instances_dir(root)), |d| formats::mmc_instance(d, Some(root), launcher)),
        Launcher::Modrinth => formats::modrinth_profiles(root),
        Launcher::Curseforge => read_each(children(&root.join("Instances")), formats::curseforge_instance),
        Launcher::Atlauncher => {
            let mut v = read_each(children(&root.join("instances")), formats::atlauncher_instance);
            v.extend(read_each(children(&root.join("servers")), |d| server_dir::server_folder(d, Launcher::Atlauncher)));
            v
        }
        Launcher::Gdlauncher => read_each(children(&root.join("instances")), formats::gdlauncher_instance),
        Launcher::Official => formats::official_profiles(root),
        Launcher::Folder => Vec::new(),
    }
}

/// One instance or server folder, whatever made it. Returns the launcher's data folder too.
fn single(dir: &Path) -> Option<(Found, PathBuf)> {
    let parent2 = dir.parent().and_then(Path::parent).map(Path::to_path_buf).unwrap_or_else(|| dir.to_path_buf());
    if dir.join("instance.cfg").is_file() {
        let launcher = if parent2.join("multimc.cfg").is_file() { Launcher::Multimc } else { Launcher::Prism };
        return formats::mmc_instance(dir, Some(&parent2), launcher).map(|f| (f, parent2));
    }
    let own = |f: Found| (f, dir.to_path_buf());
    formats::curseforge_instance(dir)
        .or_else(|| formats::atlauncher_instance(dir))
        .or_else(|| formats::gdlauncher_instance(dir))
        .or_else(|| formats::modrinth_profile_json(dir))
        .or_else(|| server_dir::server_folder(dir, Launcher::Folder))
        .map(own)
}

/// Reads an item again right before importing it.
fn reread(r: &ItemRef) -> Option<Found> {
    let (root, path) = (PathBuf::from(&r.root), PathBuf::from(&r.path));
    if r.server {
        return server_dir::server_folder(&path, r.launcher);
    }
    match r.launcher {
        Launcher::Prism | Launcher::Multimc => formats::mmc_instance(&path, Some(&root), r.launcher),
        Launcher::Modrinth => formats::modrinth_profiles(&root).into_iter().find(|f| f.path == path),
        Launcher::Curseforge => formats::curseforge_instance(&path),
        Launcher::Atlauncher => formats::atlauncher_instance(&path),
        Launcher::Gdlauncher => formats::gdlauncher_instance(&path),
        Launcher::Official => formats::official_profiles(&root).into_iter().find(|f| f.profile == r.profile),
        Launcher::Folder => formats::loose_minecraft(&path),
    }
}

// ---------- Scanning ----------

/// Instances and servers imported before: item key → new instance or server id.
fn record_file() -> PathBuf {
    paths::root().join("migrated.json")
}

async fn imported_ids() -> BTreeMap<String, String> {
    read_json(&record_file()).await.unwrap_or_default()
}

fn still_exists(id: &str, server: bool) -> bool {
    if server {
        store::dir(id).map(|d| d.join("server.json").is_file()).unwrap_or(false)
    } else {
        paths::instance_dir(id).join("instance.json").is_file()
    }
}

fn icon_preview(icon: &Option<Icon>) -> Option<String> {
    let b64 = |bytes: &[u8]| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes));
    match icon {
        Some(Icon::Url(u)) => Some(u.clone()),
        Some(Icon::Bytes(b)) if b.len() < 512 * 1024 => Some(b64(b)),
        Some(Icon::File(p)) => {
            let bytes = std::fs::read(p).ok().filter(|b| b.len() < 512 * 1024)?;
            let mime = match p.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref() {
                Some("jpg" | "jpeg") => "jpeg",
                Some("gif") => "gif",
                Some("webp") => "webp",
                _ => "png",
            };
            Some(format!("data:image/{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
        }
        _ => None,
    }
}

fn to_item(f: &Found, root: &Path, imported: &BTreeMap<String, String>) -> Item {
    let r = ItemRef {
        launcher: f.launcher,
        root: root.to_string_lossy().into_owned(),
        path: f.path.to_string_lossy().into_owned(),
        profile: f.profile.clone(),
        server: f.server,
    };
    let (mods, worlds) = if f.problem.is_none() { copy::counts(&f.game_dir) } else { (0, 0) };
    let imported = imported.get(&r.key()).is_some_and(|id| still_exists(id, f.server));
    Item {
        name: f.name.clone(),
        mc_version: f.mc_version.clone().filter(|v| v != "latest-release"),
        loader: f.loader,
        loader_version: f.loader_version.clone(),
        server_type: f.server_kind,
        mods,
        worlds,
        icon_url: icon_preview(&f.icon),
        problem: f.problem.clone(),
        imported,
        remove_blocked: remove::plan_for(f).err(),
        r,
    }
}

fn source(launcher: Launcher, root: &Path, found: Vec<Found>, imported: &BTreeMap<String, String>) -> Source {
    let mut items: Vec<Item> = par_map(&found, |f| to_item(f, root, imported));
    items.sort_by(|a, b| a.r.server.cmp(&b.r.server).then_with(|| crate::content::index::display_cmp(&a.name, &b.name)));
    Source { launcher, label: launcher.label().to_string(), path: root.to_string_lossy().into_owned(), items }
}

/// Bytes an item would copy. Asked for separately (and per item) because walking big instances takes a while,
/// and the list should show up at once.
pub async fn size_of(r: ItemRef) -> Result<u64> {
    Ok(tokio::task::spawn_blocking(move || reread(&r).map(|f| copy::size(&f.game_dir, &f.skip)).unwrap_or(0)).await?)
}

/// Every launcher found in its default place.
pub async fn scan() -> Result<Vec<Source>> {
    let imported = imported_ids().await;
    let mut tasks = Vec::new();
    let mut seen = HashSet::new();
    for (launcher, root) in default_roots() {
        if !root.is_dir() || !seen.insert(std::fs::canonicalize(&root).unwrap_or_else(|_| root.clone())) {
            continue;
        }
        let imported = imported.clone();
        tasks.push(tokio::task::spawn_blocking(move || {
            let found = read_root(launcher, &root);
            (!found.is_empty()).then(|| source(launcher, &root, found, &imported))
        }));
    }
    let mut out = Vec::new();
    for t in tasks {
        if let Some(s) = t.await? {
            out.push(s);
        }
    }
    Ok(out)
}

/// What's in a folder the player picked: a launcher's data folder, one instance or server, a folder of them,
/// or a loose `.minecraft`.
pub async fn scan_folder(dir: PathBuf) -> Result<Source> {
    let imported = imported_ids().await;
    let result = tokio::task::spawn_blocking(move || -> Option<Source> {
        if let Some(launcher) = root_kind(&dir) {
            return Some(source(launcher, &dir, read_root(launcher, &dir), &imported));
        }
        if let Some((f, root)) = single(&dir) {
            return Some(source(f.launcher, &root, vec![f], &imported));
        }
        let mut found = Vec::new();
        let mut root = dir.clone();
        for child in children(&dir) {
            if let Some((f, r)) = single(&child) {
                root = r;
                found.push(f);
            }
        }
        if !found.is_empty() {
            let launcher = found[0].launcher;
            let root = if found.iter().all(|f| f.launcher == launcher) { root } else { dir.clone() };
            return Some(source(launcher, &root, found, &imported));
        }
        formats::loose_minecraft(&dir).map(|f| source(Launcher::Folder, &dir, vec![f], &imported))
    })
    .await?;
    match result {
        Some(s) if !s.items.is_empty() => Ok(s),
        _ => bail_tr!("migrate.nothingFound", "There's no instance, launcher data or server in that folder"),
    }
}

// ---------- Importing ----------

/// Imports the chosen items one after another as one cancellable task. An item that fails is reported and the
/// rest still run; Cancel keeps the finished ones and removes the half-copied one.
/// With `remove_originals`, each instance whose launcher allows it is then taken out of that launcher (Recycle Bin),
/// but only after a clean copy (see remove.rs).
pub async fn import(refs: Vec<ItemRef>, remove_originals: bool) -> Result<Vec<Outcome>> {
    let total = refs.len().max(1);
    Task::run(tr!("migrate.task", "Moving from other launchers"), None, move |task| async move {
        let mut out = Vec::new();
        for (i, r) in refs.into_iter().enumerate() {
            let r2 = r.clone();
            let found = tokio::task::spawn_blocking(move || reread(&r2)).await?;
            let Some(f) = found else {
                out.push(Outcome::failed(file_name(&r.path), r.server, tr!("migrate.gone", "It's no longer there")));
                continue;
            };
            let name = f.name.clone();
            let server = f.server;
            if let Some(problem) = f.problem.clone() {
                out.push(Outcome::failed(name, server, problem));
                continue;
            }
            let original = remove_originals.then(|| (f.clone(), remove::plan_for(&f)));
            let progress = |p: f64| (i as f64 + p.clamp(0.0, 1.0)) / total as f64;
            let result = if server { import_server(f, &task, &progress).await } else { import_instance(f, &task, &progress).await };
            match result {
                Ok((id, skipped)) => {
                    let mut record = imported_ids().await;
                    record.insert(r.key(), id.clone());
                    let _ = write_json(&record_file(), &record).await;
                    let (mut removed, mut kept) = (false, None);
                    if let Some((original, plan)) = original {
                        task.update(tr!("migrate.removing", "Removing the original {name}…", name = name.clone()), Some(progress(1.0)));
                        let copied = remove::after_copy(&skipped, still_exists(&id, server));
                        let result = match (plan, copied) {
                            (Err(why), _) | (_, Err(why)) => Err(why),
                            (Ok(plan), Ok(())) => tokio::task::spawn_blocking(move || {
                                remove::remove(&original, &plan, remove::launcher_running, |p| trash::delete(p).map_err(|e| anyhow::anyhow!(e.to_string())))
                            })
                            .await
                            .unwrap_or_else(|e| Err(Text::plain(e.to_string()))),
                        };
                        match result {
                            Ok(()) => removed = true,
                            Err(why) => kept = Some(why),
                        }
                    }
                    out.push(Outcome { name, server, id: Some(id), error: None, skipped, removed, kept });
                }
                Err(e) if is_cancelled(&e) => return Err(e),
                Err(e) => {
                    let error = text_of(&e).cloned().unwrap_or_else(|| Text::plain(e.to_string()));
                    out.push(Outcome::failed(name, server, error));
                }
            }
        }
        Ok(out)
    })
    .await
}

fn file_name(path: &str) -> String {
    formats::folder_name(Path::new(path))
}

/// Copies the item's files into `to`, reporting progress as "Copying <name> (12 MB of 340 MB)".
async fn copy_files(f: &Found, to: &Path, task: &std::sync::Arc<Task>, progress: &impl Fn(f64) -> f64) -> Result<Vec<Skipped>> {
    task.update(tr!("migrate.reading", "Reading {name}…", name = f.name.clone()), Some(progress(0.0)));
    let (from, skip) = (f.game_dir.clone(), f.skip.clone());
    let listing = tokio::task::spawn_blocking(move || copy::list(&from, &skip)).await?;
    let total = listing.bytes.max(1);
    let name = f.name.clone();
    let skipped = copy::copy_listed(&f.game_dir, to, &listing, |done| {
        let label = tr!("migrate.copying", "Copying {name}", name = name.clone());
        task.update(
            tr!("task.progress", "{label} ({done}/{total})", label = label, done = mb(done), total = mb(total)),
            Some(progress(0.9 * done as f64 / total as f64)),
        )
    })
    .await
    .map_err(|e| tr!("migrate.writeFailed", "Couldn't write the copy: {error}", error = e.to_string()))?;
    Ok(skipped)
}

fn mb(bytes: u64) -> String {
    if bytes >= 1 << 30 {
        format!("{:.1} GB", bytes as f64 / (1u64 << 30) as f64)
    } else {
        format!("{} MB", (bytes as f64 / (1u64 << 20) as f64).ceil() as u64)
    }
}

async fn resolve_mc(f: &Found) -> Result<String> {
    let mc = f.mc_version.clone().unwrap_or_default();
    if mc != "latest-release" {
        return Ok(mc);
    }
    let versions = crate::game::mojang::list_versions().await?;
    versions
        .into_iter()
        .find(|v| v.kind == "release")
        .map(|v| v.id)
        .ok_or_else(|| tr!("migrate.noVersion", "Couldn't tell which Minecraft version it uses").into())
}

async fn import_instance(f: Found, task: &std::sync::Arc<Task>, progress: &impl Fn(f64) -> f64) -> Result<(String, Vec<Skipped>)> {
    let mc = resolve_mc(&f).await?;
    let new = NewInstance {
        name: f.name.clone(),
        mc_version: mc,
        loader: f.loader,
        loader_version: f.loader_version.clone().filter(|_| f.loader != LoaderType::Vanilla),
        color: None,
        group: None,
    };
    let (memory, jvm, pack) = (f.memory_mb, f.jvm_args.clone(), f.modpack.clone());
    let web_icon = match &f.icon {
        Some(Icon::Url(u)) => Some(u.clone()),
        _ => None,
    };
    let inst = instances::create(new, move |i| {
        i.memory_mb = memory;
        i.jvm_args = jvm;
        i.modpack = pack;
        i.icon_url = web_icon;
    })
    .await?;
    let guard = InstanceGuard::new(&inst.id);
    let skipped = copy_files(&f, &paths::game_dir(&inst.id), task, progress).await?;
    let bytes = match &f.icon {
        Some(Icon::File(p)) => tokio::fs::read(p).await.ok(),
        Some(Icon::Bytes(b)) => Some(b.clone()),
        _ => None,
    };
    if let Some(bytes) = bytes.filter(|b| !b.is_empty()) {
        let url = format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes));
        let _ = crate::core::art::set_image(&inst.id, crate::core::art::Kind::Icon, Some(url)).await;
    }
    guard.keep();
    // Look the copied mods and packs up on Modrinth / CurseForge by hash, so updates work right away: all three
    // kinds at once, in the background, so the next item starts copying meanwhile (best effort, as before).
    let id = inst.id.clone();
    tokio::spawn(async move {
        let look = |kind| tokio::time::timeout(Duration::from_secs(60), crate::content::sources::list_content(&id, kind));
        let _ = tokio::join!(look(ContentKind::Mod), look(ContentKind::Resourcepack), look(ContentKind::Shader));
    });
    Ok((inst.id, skipped))
}

/// Removes a half-copied server folder if the import stops (error or Cancel).
struct DirGuard(Option<PathBuf>);

impl Drop for DirGuard {
    fn drop(&mut self) {
        if let Some(dir) = self.0.take() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

async fn import_server(f: Found, task: &std::sync::Arc<Task>, progress: &impl Fn(f64) -> f64) -> Result<(String, Vec<Skipped>)> {
    let kind = f.server_kind.unwrap_or(ServerKind::Vanilla);
    let mc = resolve_mc(&f).await?;
    let id = store::new_id(&f.name);
    let dir = store::dir(&id)?;
    let mut guard = DirGuard(Some(dir.clone()));
    let skipped = copy_files(&f, &dir, task, progress).await?;
    let used: HashSet<u16> = store::list().await.unwrap_or_default().into_iter().map(|m| m.port).collect();
    let wanted = f.port.unwrap_or(if kind.bedrock() { 19132 } else { 25565 });
    let port = (0..200u16).map(|i| wanted.saturating_add(i)).find(|p| !used.contains(p)).unwrap_or(wanted);
    let total = total_memory_mb() as u32;
    let memory = f.memory_mb.unwrap_or(if kind.modded() { 4096 } else { 2048 }).min((total / 2).max(1024)).max(512);
    let icon_url = tokio::fs::read(dir.join("server-icon.png"))
        .await
        .ok()
        .map(|b| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b)));
    let mut meta = ServerMeta {
        id: id.clone(),
        name: f.name.clone(),
        kind,
        mc_version: mc,
        loader: kind.loader(),
        loader_version: f.loader_version.clone(),
        memory_mb: memory,
        port,
        java_path: None,
        jvm_args: None,
        created_at: now_ms(),
        last_started: None,
        modpack: None,
        icon_url,
        launch: f.launch.clone().unwrap_or_default(),
        public: true,
        optimized_flags: true,
        pack_flags: false,
        join_requests: Default::default(),
        tunnel: TunnelMode::Playit,
        relay_host: None,
        relay_secret: None,
        domain: None,
        dns_auto: false,
        relay_name: None,
        relay_account: None,
    };
    if port != wanted {
        let _ = crate::server::properties::set_values(&dir, &[("server-port", port.to_string())]).await;
    }
    store::save(&meta).await?;
    // Nothing to start it with (the jar wasn't found): install the same software fresh, keeping the world.
    let l = &meta.launch;
    let startable = [&l.jar, &l.args_file, &l.exe].iter().any(|p| p.as_deref().is_some_and(|p| dir.join(p).is_file()));
    if !startable {
        meta.launch = Default::default();
        installers::install(&mut meta, &dir, task).await?;
        store::save(&meta).await?;
    }
    guard.0 = None;
    Ok((id, skipped))
}

#[cfg(test)]
mod tests {
    use super::formats::tests::Fixture;
    use super::*;

    #[test]
    fn finds_launcher_folders_and_single_items() {
        let fx = Fixture::new();
        fx.file("prism/prismlauncher.cfg", "").file("prism/instances/A/instance.cfg", "name=A\n");
        fx.file("cf/Instances/B/minecraftinstance.json", r#"{"name":"B","gameVersion":"1.20.1"}"#);
        fx.file("atl/instances/C/instance.json", r#"{"id":"1.20.1","launcher":{"name":"C"}}"#).file("atl/servers/S/server.properties", "");
        fx.file("gd/instances/D/config.json", r#"{"loader":{"loaderType":"vanilla","mcVersion":"1.19"}}"#);
        fx.file("mc/launcher_profiles.json", r#"{"profiles":{}}"#);
        fx.file("mr/profiles/E/profile.json", r#"{"metadata":{"name":"E","game_version":"1.20"}}"#);
        assert_eq!(root_kind(&fx.path("prism")), Some(Launcher::Prism));
        assert_eq!(root_kind(&fx.path("cf")), Some(Launcher::Curseforge));
        assert_eq!(root_kind(&fx.path("atl")), Some(Launcher::Atlauncher));
        assert_eq!(root_kind(&fx.path("gd")), Some(Launcher::Gdlauncher));
        assert_eq!(root_kind(&fx.path("mc")), Some(Launcher::Official));
        assert_eq!(root_kind(&fx.path("mr")), Some(Launcher::Modrinth));
        let atl = read_root(Launcher::Atlauncher, &fx.path("atl"));
        assert_eq!(atl.len(), 2);
        assert!(atl.iter().any(|f| f.server && f.name == "S"), "ATLauncher's servers come along");
        let (a, root) = single(&fx.path("prism/instances/A")).unwrap();
        assert_eq!((a.launcher, root), (Launcher::Prism, fx.path("prism")));
        let (b, _) = single(&fx.path("cf/Instances/B")).unwrap();
        assert_eq!(b.launcher, Launcher::Curseforge);
        let r = ItemRef {
            launcher: Launcher::Atlauncher,
            root: String::new(),
            path: fx.path("atl/instances/C").to_string_lossy().into(),
            profile: None,
            server: false,
        };
        assert_eq!(reread(&r).map(|f| f.name).as_deref(), Some("C"));
    }

    /// Lists what this PC's launchers have (read only): `cargo test --lib real_launchers -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_launchers_on_this_pc() {
        for (launcher, root) in default_roots().into_iter().filter(|(_, r)| r.is_dir()) {
            let started = std::time::Instant::now();
            println!("== {} ({})", launcher.label(), root.display());
            let found = read_root(launcher, &root);
            println!("   read in {:?}", started.elapsed());
            for f in found {
                let (size, (mods, worlds)) = (copy::size(&f.game_dir, &f.skip), copy::counts(&f.game_dir));
                println!(
                    "  {}{} | {:?} {:?} {:?} | {} MB, {mods} mods, {worlds} worlds | mem {:?} icon {} | {}",
                    f.name,
                    if f.server { " [server]" } else { "" },
                    f.mc_version,
                    f.loader,
                    f.loader_version,
                    size >> 20,
                    f.memory_mb,
                    f.icon.is_some(),
                    f.problem.map(|p| p.text).unwrap_or_default()
                );
            }
            println!("   measured in {:?}", started.elapsed());
        }
    }
}
