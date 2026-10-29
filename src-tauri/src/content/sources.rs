//! Routes Discover / content calls to the right source, based on the project id prefix.

use std::path::Path;

use anyhow::Result;
use futures::stream::{self, StreamExt};
use serde::Serialize;

use super::dupes::Guard;
use super::index::{has_entry, load_index, open_zip, save_index, ContentIndex};
use super::{atlauncher, curseforge, ftb, github, modrinth, technic};
use crate::core::events::Task;
use crate::core::instances;
use crate::core::types::{source_of, ContentItem, ContentKind, ContentSource, Instance, LoaderType, ProjectVersion, SearchQuery, SearchResult};
use crate::{bail_tr, err_tr, tr};

pub async fn search(q: &SearchQuery) -> Result<SearchResult> {
    match q.source {
        Some(ContentSource::Curseforge) => curseforge::search(q).await,
        Some(ContentSource::Ftb) => ftb::search(q).await,
        Some(ContentSource::Atlauncher) => atlauncher::search(q).await,
        Some(ContentSource::Technic) => technic::search(q).await,
        Some(ContentSource::Github) => github::search_hits(q).await,
        _ => modrinth::search(q).await,
    }
}

pub async fn versions(project_id: &str, instance_id: Option<&str>) -> Result<Vec<ProjectVersion>> {
    match source_of(project_id) {
        ContentSource::Curseforge => curseforge::versions(project_id, instance_id).await,
        ContentSource::Ftb => ftb::versions(project_id).await,
        ContentSource::Atlauncher => atlauncher::versions(project_id).await,
        ContentSource::Technic => technic::versions(project_id).await,
        ContentSource::Github => github::versions_for(project_id, instance_id).await,
        ContentSource::Modrinth => modrinth::versions(project_id, instance_id).await,
    }
}

pub async fn install(instance_id: &str, project_id: &str, kind: ContentKind, version_id: Option<String>) -> Result<()> {
    match source_of(project_id) {
        ContentSource::Curseforge => curseforge::install(instance_id, project_id, kind, version_id).await,
        ContentSource::Ftb => bail_tr!("content.modpacksOnly", "{source} only publishes modpacks", source = "FTB"),
        ContentSource::Atlauncher => bail_tr!("content.modpacksOnly", "{source} only publishes modpacks", source = "ATLauncher"),
        ContentSource::Technic => bail_tr!("content.modpacksOnly", "{source} only publishes modpacks", source = "Technic"),
        ContentSource::Github => github::install(instance_id, project_id, kind, version_id).await,
        ContentSource::Modrinth => modrinth::install(instance_id, project_id, kind, version_id).await,
    }
}

pub async fn install_modpack(project_id: &str, version_id: Option<String>) -> Result<Instance> {
    match source_of(project_id) {
        ContentSource::Curseforge => curseforge::install_modpack(project_id, version_id).await,
        ContentSource::Ftb => ftb::install_modpack(project_id, version_id).await,
        ContentSource::Atlauncher => atlauncher::install_modpack(project_id, version_id).await,
        ContentSource::Technic => technic::install_modpack(project_id, version_id).await,
        ContentSource::Github => bail_tr!("github.modsOnly", "GitHub only offers mods here"),
        ContentSource::Modrinth => modrinth::install_modpack(project_id, version_id).await,
    }
}

/// Lists files, identifying hand-added ones via Modrinth first, then CurseForge fingerprints.
pub async fn list_content(instance_id: &str, kind: ContentKind) -> Result<Vec<ContentItem>> {
    let items = modrinth::list_content(instance_id, kind).await?;
    let unknown: Vec<String> = items.iter().filter(|i| i.project_id.is_none()).map(|i| i.filename.clone()).collect();
    if unknown.is_empty() {
        return Ok(items);
    }
    let identified = async { curseforge::identify(&instances::get(instance_id).await?, kind, &unknown).await }.await;
    if identified.is_err() {
        return Ok(items); // offline or CurseForge unavailable
    }
    modrinth::list_content(instance_id, kind).await
}

pub async fn check_updates(instance_id: &str) -> Result<Vec<ContentItem>> {
    let inst = instances::get(instance_id).await?;
    list_content(instance_id, ContentKind::Mod).await?;
    let mut mods = modrinth::check_updates(instance_id).await?;
    if inst.loader != LoaderType::Vanilla {
        let _ = curseforge::check_updates(&inst, &mut mods).await;
        let _ = github::check_updates(&inst, &mut mods).await;
    }
    Ok(mods)
}

pub async fn update_content(instance_id: &str, filename: &str) -> Result<()> {
    let m = check_updates(instance_id).await?.into_iter().find(|x| x.filename == filename);
    if let Some(u) = m.and_then(|m| m.update) {
        change_version(instance_id, ContentKind::Mod, filename, &u.version_id).await?;
    }
    Ok(())
}

/// Updates at once: this many downloads run side by side.
const PARALLEL_UPDATES: usize = 4;

/// What Update all did: the files updated, and the ones that couldn't be (with why).
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateManyResult {
    pub updated: Vec<String>,
    pub failed: Vec<UpdateFailure>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFailure {
    pub filename: String,
    pub title: Option<String>,
    pub error: String,
}

/// Updates several mods of an instance (all that have one when `filenames` is empty): one check, then
/// [`PARALLEL_UPDATES`] downloads at a time, each on its own copy of the content index; the changes are put
/// together and saved once at the end, and files that were disabled stay disabled.
pub async fn update_many(instance_id: &str, filenames: &[String]) -> Result<UpdateManyResult> {
    let inst = instances::get(instance_id).await?;
    let todo: Vec<ContentItem> = check_updates(instance_id)
        .await?
        .into_iter()
        .filter(|m| m.update.is_some() && m.project_id.is_some() && (filenames.is_empty() || filenames.contains(&m.filename)))
        .collect();
    if todo.is_empty() {
        return Ok(UpdateManyResult::default());
    }
    let total = todo.len();
    Task::run(tr!("content.updatingMany", "Updating {count} mods", count = total), Some(inst.id.clone()), move |task| async move {
        let base = load_index(&inst.id).await;
        let done = std::sync::atomic::AtomicUsize::new(0);
        let results: Vec<(ContentItem, Result<ContentIndex>)> = stream::iter(todo)
            .map(|m| {
                let (inst, task, base, done) = (&inst, &task, &base, &done);
                async move {
                    let mut idx = base.clone();
                    let r = update_one(inst, task, &mut idx, &m).await.map(|()| idx);
                    let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    task.update(tr!("content.updatedOf", "Updated {done} of {total}", done = n, total = total), Some(n as f64 / total as f64));
                    (m, r)
                }
            })
            .buffer_unordered(PARALLEL_UPDATES)
            .collect()
            .await;
        // Each update changed only its own project's entries: apply what each one removed and added.
        let mut idx = load_index(&inst.id).await;
        let mut out = UpdateManyResult::default();
        let mut disabled = Vec::new();
        for (m, r) in results {
            match r {
                Ok(after) => {
                    for k in base.keys().filter(|k| !after.contains_key(*k)) {
                        idx.remove(k);
                    }
                    for (k, e) in after.iter().filter(|(k, e)| base.get(*k).map(|b| b.version_id != e.version_id || b.sha1 != e.sha1).unwrap_or(true)) {
                        idx.insert(k.clone(), e.clone());
                    }
                    if !m.enabled {
                        disabled.push(m.project_id.clone().unwrap_or_default());
                    }
                    out.updated.push(m.filename);
                }
                Err(e) => out.failed.push(UpdateFailure { filename: m.filename, title: m.title, error: e.to_string() }),
            }
        }
        save_index(&inst.id, &idx).await?;
        // The new files come in enabled: put back the ones that were off.
        if !disabled.is_empty() {
            for f in modrinth::list_content(&inst.id, ContentKind::Mod).await? {
                if f.enabled && f.project_id.as_ref().is_some_and(|p| disabled.contains(p)) {
                    let _ = modrinth::toggle_content(&inst.id, ContentKind::Mod, &f.filename).await;
                }
            }
        }
        Ok(out)
    })
    .await
}

/// One mod to its update, on `idx` (a copy of the instance's index).
async fn update_one(inst: &Instance, task: &Task, idx: &mut ContentIndex, m: &ContentItem) -> Result<()> {
    let (Some(pid), Some(u)) = (m.project_id.as_deref(), m.update.as_ref()) else { return Ok(()) };
    let vid = Some(u.version_id.clone());
    match source_of(pid) {
        ContentSource::Curseforge => {
            let mut guard = Guard::open(inst, ContentKind::Mod, idx).await;
            let mut blocked = Vec::new();
            let r = curseforge::install_into(inst, task, idx, &mut guard, pid, ContentKind::Mod, vid, &mut blocked).await;
            if !blocked.is_empty() {
                curseforge::add_pending(&inst.id, blocked).await?;
            }
            r
        }
        ContentSource::Github => github::install_into(inst, task, idx, pid, ContentKind::Mod, vid).await,
        ContentSource::Modrinth => {
            let mut guard = Guard::open(inst, ContentKind::Mod, idx).await;
            modrinth::install_into(inst, task, idx, &mut guard, pid, ContentKind::Mod, vid).await
        }
        _ => Ok(()),
    }
}

/// Swaps an installed file for another version of the same project (newer or older), keeping it disabled if it was.
pub async fn change_version(instance_id: &str, kind: ContentKind, filename: &str, version_id: &str) -> Result<()> {
    let item = modrinth::list_content(instance_id, kind).await?.into_iter().find(|x| x.filename == filename);
    let Some((item, project_id)) = item.and_then(|i| i.project_id.clone().map(|p| (i, p))) else {
        bail_tr!(
            "content.unknownOrigin",
            "ResinCraft doesn't know where {filename} came from, so it can't switch its version",
            filename = filename
        );
    };
    if item.version_id.as_deref() == Some(version_id) {
        return Ok(());
    }
    install(instance_id, &project_id, kind, Some(version_id.to_string())).await?;
    if !item.enabled {
        let fresh = modrinth::list_content(instance_id, kind).await?.into_iter().find(|x| x.project_id.as_deref() == Some(project_id.as_str()));
        if let Some(fresh) = fresh.filter(|f| f.enabled) {
            modrinth::toggle_content(instance_id, kind, &fresh.filename).await?;
        }
    }
    Ok(())
}

/// Imports a modpack file from disk, detecting Modrinth (.mrpack) vs CurseForge (.zip with manifest.json).
pub async fn import_modpack_file(file: &Path) -> Result<Instance> {
    let ext = file.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if ext != "mrpack" && ext != "zip" {
        bail_tr!("modpack.pickFile", "Pick a Modrinth .mrpack or a CurseForge modpack .zip");
    }
    let f = file.to_path_buf();
    let (mr, cf) = tokio::task::spawn_blocking(move || -> Result<(bool, bool)> {
        let zip = open_zip(&f).map_err(|_| err_tr!("modpack.notModpack", "That file isn't a Modrinth or CurseForge modpack"))?;
        Ok((has_entry(&zip, "modrinth.index.json"), has_entry(&zip, "manifest.json")))
    })
    .await??;
    if mr {
        return modrinth::install_mrpack(file, Default::default()).await;
    }
    if cf {
        return curseforge::install_cf_zip(file, Default::default()).await;
    }
    bail_tr!("modpack.notModpack", "That file isn't a Modrinth or CurseForge modpack")
}
