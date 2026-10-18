//! Routes Discover / content calls to the right source, based on the project id prefix.

use std::path::Path;

use anyhow::Result;

use super::index::{has_entry, open_zip};
use super::{atlauncher, curseforge, ftb, github, modrinth, technic};
use crate::core::instances;
use crate::core::types::{source_of, ContentItem, ContentKind, ContentSource, Instance, LoaderType, ProjectVersion, SearchQuery, SearchResult};
use crate::{bail_tr, err_tr};

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
