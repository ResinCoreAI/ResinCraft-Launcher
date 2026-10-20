//! Keeping a hosted server in step with what it was made from:
//! - a server made from a modpack switches to another version of the pack (its server files), like an instance does;
//! - a server made from an instance syncs from it: the instance's server-side mods and configs, and its Minecraft and
//!   loader version, only when the host presses Sync.
//! Both run as cancellable tasks through `content::packswitch::guarded`: everything replaced or removed is backed up
//! first and put back if anything fails. Worlds, player data, the player lists, server.properties and files the host
//! added are kept; config files the host changed are kept unless the new files change them too (then theirs is kept as
//! `.bak`).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::content::jarmeta;
use crate::content::packswitch::{self, guarded, hash_reader_file, norm, plan, Keep, NewEntry, PackSwitch, Side, Site, Step};
use crate::core::fsutil::{now_ms, read_json, write_json};
use crate::core::instances;
use crate::core::net::{Hash, HashAlgo};
use crate::core::paths;
use crate::core::types::{source_of, ContentSource, Instance, LoaderType, ServerKind, ServerMeta};
use crate::{bail_tr, tr};

use super::{installers, properties, runtime, store};

fn stop_first(meta: &ServerMeta) -> anyhow::Error {
    crate::err_tr!("packswitch.serverRunning", "Stop {name} first: it's running", name = meta.name.clone())
}

async fn level_name(dir: &Path) -> String {
    properties::get_value(dir, "level-name").await.unwrap_or_default()
}

/// Applies a new Minecraft / loader version to a server and installs that server software.
async fn set_software(meta: &mut ServerMeta, dir: &Path, mc: &str, loader: LoaderType, loader_version: Option<String>, task: &std::sync::Arc<crate::core::events::Task>) -> Result<()> {
    let kind = ServerKind::from_loader(loader);
    let changed = meta.mc_version != mc || meta.kind != kind || meta.loader_version != loader_version;
    if !changed {
        return Ok(());
    }
    meta.kind = kind;
    meta.mc_version = mc.to_string();
    meta.loader = kind.loader();
    meta.loader_version = loader_version;
    store::save(meta).await?;
    installers::install(meta, dir, task).await?;
    store::save(meta).await
}

// ---------- Modpack servers ----------

/// Switches a server made from a modpack to `version_id` of the pack, using the pack's server files.
pub async fn switch_pack(id: &str, version_id: &str) -> Result<PackSwitch> {
    let meta = store::get(id).await?;
    let Some(pack) = meta.modpack.clone() else {
        bail_tr!("packswitch.notModpack", "{name} wasn't installed from a modpack", name = meta.name.clone())
    };
    let Some(project_id) = pack.project_id.clone().filter(|p| !p.is_empty() && source_of(p) != ContentSource::Github) else {
        bail_tr!("packswitch.noSource", "ResinCraft doesn't know where {name} came from, so it can't switch its version", name = pack.name.clone())
    };
    if pack.version_id.as_deref() == Some(version_id) {
        return Ok(PackSwitch::default());
    }
    if runtime::is_running(id) {
        return Err(stop_first(&meta));
    }
    let dir = store::dir(id)?;
    let keep = Keep::server(&level_name(&dir).await);
    let (sid, vid, d) = (id.to_string(), version_id.to_string(), dir.clone());
    let title = tr!("packswitch.title", "Switching {name} to another version", name = meta.name.clone());
    let busy = tr!("packswitch.busy", "{name} is already switching versions", name = meta.name.clone());
    guarded(Site::server(id, dir), title, busy, move |task, work, applier| async move {
        let sw = packswitch::switch_files(&task, &work, &applier, &project_id, pack.version_id.as_deref(), &vid, &pack.name, Side::Server, &keep, None).await?;
        let mut meta = store::get(&sid).await?;
        if let Some(m) = meta.modpack.as_mut() {
            m.version_id = Some(sw.target.version_id.clone());
        }
        store::save(&meta).await?;
        // The pack's own loader version, as when the server was created.
        set_software(&mut meta, &d, &sw.target.mc_version, sw.target.loader, sw.target.loader_version.clone(), &task).await?;
        let mut out = sw.summary();
        out.pending_files = sw.pending();
        out.dropped = sw.dropped.clone();
        Ok(out)
    })
    .await
}

// ---------- Servers made from an instance ----------

/// Which instance a server was made from, and what the last sync (or creation) copied.
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
struct Link {
    instance_id: String,
    instance_name: String,
    /// The server's files from the instance: path → "sha1:…".
    #[serde(default)]
    files: BTreeMap<String, String>,
    /// The instance's files at that time (before leaving out client-only mods), to notice changes.
    #[serde(default)]
    source: BTreeMap<String, String>,
    #[serde(default)]
    mc_version: String,
    #[serde(default)]
    loader: Option<LoaderType>,
    #[serde(default)]
    loader_version: Option<String>,
    #[serde(default)]
    synced_at: i64,
}

const LINK: &str = "instance-sync.json";

/// Folders a server takes from an instance (as when it was made from it), plus the enabled jars in mods/.
const SYNC_DIRS: [&str; 4] = ["config", "defaultconfigs", "kubejs", "scripts"];

// Jars are big: their hashes are remembered while size and modified time stay the same.
static HASHES: Mutex<Option<HashMap<PathBuf, (u64, SystemTime, String)>>> = Mutex::new(None);

fn sha1_cached(p: &Path) -> Option<String> {
    let md = std::fs::metadata(p).ok()?;
    let (len, modified) = (md.len(), md.modified().ok()?);
    if let Some((l, m, h)) = HASHES.lock().unwrap().as_ref().and_then(|c| c.get(p).cloned()) {
        if l == len && m == modified {
            return Some(h);
        }
    }
    let h = hash_reader_file(p, HashAlgo::Sha1)?;
    HASHES.lock().unwrap().get_or_insert_with(HashMap::new).insert(p.to_path_buf(), (len, modified, h.clone()));
    Some(h)
}

/// The files a server takes from the game folder `game`: enabled jars in mods/ and the config folders (blocking).
fn sync_set(game: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Ok(rd) = std::fs::read_dir(game.join("mods")) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.path().is_file() && name.to_lowercase().ends_with(".jar") {
                if let Some(h) = sha1_cached(&e.path()) {
                    out.insert(format!("mods/{name}"), h);
                }
            }
        }
    }
    for d in SYNC_DIRS {
        walk(game, &game.join(d), &mut out);
    }
    out
}

fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(root, &p, out);
        } else if let (Ok(rel), Some(h)) = (p.strip_prefix(root), sha1_cached(&p)) {
            out.insert(norm(&rel.to_string_lossy()), h);
        }
    }
}

fn texts(m: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    m.iter().map(|(k, v)| (k.clone(), format!("sha1:{v}"))).collect()
}

/// Remembers that the server in `dir` was just made from `inst` (called once its files are copied).
pub async fn link_instance(dir: &Path, inst: &Instance) {
    let (d, g) = (dir.to_path_buf(), paths::game_dir(&inst.id));
    let Ok((files, source)) = tokio::task::spawn_blocking(move || (sync_set(&d), sync_set(&g))).await else { return };
    let link = Link {
        instance_id: inst.id.clone(),
        instance_name: inst.name.clone(),
        files: texts(&files),
        source: texts(&source),
        mc_version: inst.mc_version.clone(),
        loader: Some(inst.loader),
        loader_version: inst.loader_version.clone(),
        synced_at: now_ms(),
    };
    let _ = write_json(&dir.join(LINK), &link).await;
}

/// What syncing a server from its instance would change (or did).
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncPreview {
    pub instance_id: String,
    pub instance_name: String,
    /// The instance was deleted: nothing to sync from.
    pub missing: bool,
    pub added: Vec<String>,
    pub updated: Vec<String>,
    pub removed: Vec<String>,
    /// Files the host changed that stay as they are.
    pub kept: Vec<String>,
    /// `.bak` copies of the host's changed files the sync would replace.
    pub backups: Vec<String>,
    /// Client-only mods left off the server, by name.
    pub dropped: Vec<String>,
    /// (server, instance) when they differ.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minecraft: Option<(String, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loader: Option<(String, String)>,
    /// Never synced with this launcher version: files only on the server are kept (unless they're an older copy of
    /// a mod the instance has).
    pub first_sync: bool,
}

impl SyncPreview {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.updated.is_empty() && self.removed.is_empty() && self.minecraft.is_none() && self.loader.is_none()
    }
}

fn loader_text(loader: Option<LoaderType>, version: Option<&str>) -> String {
    let l = loader.unwrap_or(LoaderType::Vanilla).label();
    match version.filter(|v| !v.is_empty()) {
        Some(v) => format!("{l} {v}"),
        None => l.to_string(),
    }
}

/// The server's link, if it was made from an instance.
async fn link_of(dir: &Path) -> Option<Link> {
    read_json(&dir.join(LINK)).await
}

/// The instance a server was made from: (id, name), and whether it still exists.
pub async fn source_instance(id: &str) -> Option<(String, String, bool)> {
    let link = link_of(&store::dir(id).ok()?).await?;
    let exists = instances::get(&link.instance_id).await.is_ok();
    Some((link.instance_id, link.instance_name, exists))
}

struct Computed {
    preview: SyncPreview,
    steps: Vec<Step>,
    inst: Instance,
    files: BTreeMap<String, String>,
    source: BTreeMap<String, String>,
}

/// Compares the server with its instance now.
async fn compute(meta: &ServerMeta, dir: &Path) -> Result<Option<Computed>> {
    let Some(link) = link_of(dir).await else {
        bail_tr!("sync.notFromInstance", "{name} wasn't made from an instance", name = meta.name.clone())
    };
    let mut preview = SyncPreview { instance_id: link.instance_id.clone(), instance_name: link.instance_name.clone(), ..Default::default() };
    let Ok(inst) = instances::get(&link.instance_id).await else {
        preview.missing = true;
        return Ok(Some(Computed { preview, steps: vec![], inst: dummy(), files: BTreeMap::new(), source: BTreeMap::new() }));
    };
    preview.instance_name = inst.name.clone();
    let game = paths::game_dir(&inst.id);
    let g = game.clone();
    let source = tokio::task::spawn_blocking(move || sync_set(&g)).await?;

    // Client-only mods stay off the server, as when it was made (offline: none are left out).
    let mut files = source.clone();
    let mods: Vec<String> = files.iter().filter(|(p, _)| p.starts_with("mods/")).map(|(_, h)| h.clone()).collect();
    if let Ok(found) = super::create::client_only_sha1s(&mods).await {
        files.retain(|p, h| {
            let drop = p.starts_with("mods/") && found.contains_key(h);
            if drop {
                preview.dropped.push(found[h].clone());
            }
            !drop
        });
    }
    preview.dropped.sort();

    let old: Option<BTreeMap<String, Option<Hash>>> =
        (!link.files.is_empty()).then(|| link.files.iter().map(|(k, v)| (k.clone(), v.strip_prefix("sha1:").map(Hash::sha1))).collect());
    preview.first_sync = old.is_none();
    let entries: BTreeMap<String, NewEntry> = files.keys().map(|k| (k.clone(), NewEntry::Staged)).collect();
    let keep = Keep::server(&level_name(dir).await);
    let (d, o, n) = (dir.to_path_buf(), old.clone(), entries.clone());
    let new_mods: Vec<String> = files.keys().filter(|p| p.starts_with("mods/")).cloned().collect();
    let (mut steps, kept) = tokio::task::spawn_blocking(move || {
        let p = plan(&d, &game, o.as_ref(), &n, &HashMap::new(), &keep);
        let mut steps = p.steps;
        // Without a record of the last sync, an older copy of a mod the instance has is still recognised by its id.
        if o.is_none() {
            steps.extend(older_copies(&d, &game, &new_mods));
        }
        (steps, p.kept)
    })
    .await?;
    steps.dedup();
    for s in &steps {
        match s {
            Step::Place { to, bak, .. } => {
                if *bak {
                    preview.backups.push(format!("{to}.bak"));
                }
                if dir.join(to).is_file() {
                    preview.updated.push(to.clone());
                } else {
                    preview.added.push(to.clone());
                }
            }
            Step::Remove { path } => preview.removed.push(path.clone()),
        }
    }
    preview.kept = kept;
    if meta.mc_version != inst.mc_version {
        preview.minecraft = Some((meta.mc_version.clone(), inst.mc_version.clone()));
    }
    let server_loader = meta.kind.loader().unwrap_or(LoaderType::Vanilla);
    let inst_lv = (inst.loader != LoaderType::Vanilla).then(|| inst.loader_version.clone()).flatten();
    if server_loader != inst.loader || (inst_lv.is_some() && meta.loader_version != inst_lv) {
        preview.loader = Some((loader_text(Some(server_loader), meta.loader_version.as_deref()), loader_text(Some(inst.loader), inst_lv.as_deref())));
    }
    Ok(Some(Computed { preview, steps, inst, files, source }))
}

/// Jars in the server's mods/ that aren't in the instance but carry the same mod id as one that is: older copies the
/// sync replaces (blocking).
fn older_copies(server: &Path, game: &Path, new_mods: &[String]) -> Vec<Step> {
    let ids: Vec<String> = new_mods.iter().flat_map(|m| jarmeta::read_jar(&game.join(m)).ids).collect();
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(server.join("mods")) else { return out };
    for e in rd.flatten() {
        let rel = format!("mods/{}", e.file_name().to_string_lossy());
        if !rel.to_lowercase().ends_with(".jar") || new_mods.contains(&rel) {
            continue;
        }
        if jarmeta::read_jar(&e.path()).ids.iter().any(|i| ids.contains(i)) {
            out.push(Step::Remove { path: rel });
        }
    }
    out
}

fn dummy() -> Instance {
    serde_json::from_value(serde_json::json!({ "id": "", "name": "", "mcVersion": "", "loader": "vanilla", "color": "", "createdAt": 0 })).expect("instance shape")
}

/// What syncing would change, without changing anything.
pub async fn sync_preview(id: &str) -> Result<SyncPreview> {
    let meta = store::get(id).await?;
    let dir = store::dir(id)?;
    Ok(compute(&meta, &dir).await?.map(|c| c.preview).unwrap_or_default())
}

/// Syncs a server from the instance it was made from (see the module docs). A cancellable task.
pub async fn sync_from_instance(id: &str) -> Result<SyncPreview> {
    let meta = store::get(id).await?;
    if runtime::is_running(id) {
        return Err(stop_first(&meta));
    }
    let dir = store::dir(id)?;
    let (sid, d) = (id.to_string(), dir.clone());
    let title = tr!("sync.title", "Syncing {name} from its instance", name = meta.name.clone());
    let busy = tr!("packswitch.busy", "{name} is already switching versions", name = meta.name.clone());
    guarded(Site::server(id, dir), title, busy, move |task, work, applier| async move {
        task.update(tr!("packswitch.comparing", "Comparing with your files…"), None);
        let meta = store::get(&sid).await?;
        let Some(c) = compute(&meta, &d).await? else { return Ok(SyncPreview::default()) };
        if c.preview.missing {
            bail_tr!("sync.instanceGone", "{name} no longer exists, so there's nothing to sync from", name = c.preview.instance_name.clone());
        }
        // Stage copies of the instance's files the sync places (the instance keeps its own).
        let stage = work.join("stage");
        let (game, st, steps) = (paths::game_dir(&c.inst.id), stage.clone(), c.steps.clone());
        tokio::task::spawn_blocking(move || -> Result<()> {
            for s in &steps {
                if let Step::Place { path, .. } = s {
                    let to = crate::core::fsutil::safe_join(&st, path)?;
                    if let Some(parent) = to.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::copy(crate::core::fsutil::safe_join(&game, path)?, &to)?;
                }
            }
            Ok(())
        })
        .await??;
        task.update(tr!("packswitch.applying", "Backing up and applying the changes…"), None);
        applier.apply(&stage, c.steps.clone()).await?;

        let mut meta = meta;
        let lv = (c.inst.loader != LoaderType::Vanilla).then(|| c.inst.loader_version.clone()).flatten();
        if c.preview.minecraft.is_some() || c.preview.loader.is_some() {
            set_software(&mut meta, &d, &c.inst.mc_version, c.inst.loader, lv.clone(), &task).await?;
        }
        let link = Link {
            instance_id: c.inst.id.clone(),
            instance_name: c.inst.name.clone(),
            files: texts(&c.files),
            source: texts(&c.source),
            mc_version: c.inst.mc_version.clone(),
            loader: Some(c.inst.loader),
            loader_version: lv,
            synced_at: now_ms(),
        };
        write_json(&d.join(LINK), &link).await?;
        Ok(c.preview)
    })
    .await
}

/// True when the instance a server was made from changed since the last sync (its server-relevant files, Minecraft
/// or loader version). None when the server wasn't made from an instance, or that instance is gone.
pub async fn sync_available(id: &str) -> Option<bool> {
    let dir = store::dir(id).ok()?;
    let link = link_of(&dir).await?;
    let inst = instances::get(&link.instance_id).await.ok()?;
    let game = paths::game_dir(&inst.id);
    let source = tokio::task::spawn_blocking(move || sync_set(&game)).await.ok()?;
    let lv = (inst.loader != LoaderType::Vanilla).then(|| inst.loader_version.clone()).flatten();
    Some(texts(&source) != link.source || inst.mc_version != link.mc_version || Some(inst.loader) != link.loader || lv != link.loader_version)
}

/// What a server's page shows about where it came from.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerOrigin {
    /// Made from this instance (id, name); `instanceGone` when it was deleted since.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_name: Option<String>,
    pub instance_gone: bool,
    pub sync_available: bool,
}

pub async fn origin(id: &str) -> Result<ServerOrigin> {
    store::get(id).await?;
    let Some((iid, name, exists)) = source_instance(id).await else { return Ok(ServerOrigin::default()) };
    let sync_available = exists && sync_available(id).await.unwrap_or(false);
    Ok(ServerOrigin { instance_id: Some(iid), instance_name: Some(name), instance_gone: !exists, sync_available })
}

#[cfg(test)]
#[path = "packs_tests.rs"]
mod tests;
