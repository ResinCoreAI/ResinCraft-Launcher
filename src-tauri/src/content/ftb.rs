//! Feed The Beast's public modpack API (the same one the FTB App uses). FTB only publishes modpacks.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use futures::stream::{self, StreamExt};
use serde::Deserialize;
use serde_json::json;

use super::dupes::dedupe_pack;
use super::index::{discard_instance, iso_from_secs, parse_loader, safe_name, InstanceGuard};
use crate::core::events::Task;
use crate::core::fsutil::safe_join;
use crate::core::instances;
use crate::core::net::{download_all, fetch_json, DownloadItem, Hash};
use crate::core::paths;
use crate::core::types::{
    ContentKind, ContentSource, Instance, LoaderType, ModpackRef, NewInstance, PendingDownload, ProjectHit, ProjectVersion, SearchQuery, SearchResult,
};
use crate::{err_tr, tr};

const API: &str = "https://api.feed-the-beast.com/v1/modpacks/public";

fn ftb_id(project_id: &str) -> u64 {
    project_id.strip_prefix("ftb:").unwrap_or(project_id).parse().unwrap_or(0)
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct FtbTarget {
    name: String,
    version: String,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct FtbSpecs {
    recommended: u32,
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct FtbVersionSummary {
    id: u64,
    name: String,
    #[serde(rename = "type")]
    kind: String,
    released: i64,
    targets: Vec<FtbTarget>,
    specs: Option<FtbSpecs>,
}

impl FtbVersionSummary {
    fn target(&self, kind: &str) -> Option<&FtbTarget> {
        self.targets.iter().find(|t| t.kind == kind)
    }
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct FtbArt {
    url: String,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct FtbNamed {
    name: String,
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct FtbPack {
    id: u64,
    name: String,
    slug: Option<String>,
    synopsis: String,
    installs: u64,
    plays: u64,
    updated: i64,
    art: Vec<FtbArt>,
    authors: Vec<FtbNamed>,
    tags: Vec<FtbNamed>,
    versions: Vec<FtbVersionSummary>,
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct FtbCurse {
    project: u64,
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct FtbFile {
    path: String,
    name: String,
    url: String,
    sha1: String,
    size: Option<u64>,
    serveronly: bool,
    curseforge: Option<FtbCurse>,
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct FtbVersion {
    #[serde(flatten)]
    summary: FtbVersionSummary,
    files: Vec<FtbFile>,
}

// Pack details rarely change; cache them for the session so paging through results is quick.
static PACKS: Mutex<Option<HashMap<u64, Arc<FtbPack>>>> = Mutex::new(None);

async fn pack(id: u64) -> Result<Arc<FtbPack>> {
    if let Some(p) = PACKS.lock().unwrap().as_ref().and_then(|m| m.get(&id).cloned()) {
        return Ok(p);
    }
    let p: Arc<FtbPack> = Arc::new(fetch_json(&format!("{API}/modpack/{id}")).await?);
    PACKS.lock().unwrap().get_or_insert_with(HashMap::new).insert(id, p.clone());
    Ok(p)
}

fn icon_of(p: &FtbPack) -> Option<String> {
    p.art.iter().find(|a| a.kind == "square").or(p.art.first()).map(|a| a.url.clone()).filter(|u| !u.is_empty())
}

fn to_hit(p: &FtbPack) -> ProjectHit {
    ProjectHit {
        id: format!("ftb:{}", p.id),
        slug: p.slug.clone().unwrap_or_else(|| p.id.to_string()),
        title: p.name.clone(),
        description: p.synopsis.clone(),
        icon_url: icon_of(p),
        author: p.authors.first().map(|a| a.name.clone()).unwrap_or_else(|| "FTB Team".into()),
        downloads: p.installs,
        follows: 0,
        categories: p.tags.iter().map(|t| t.name.clone()).collect(),
        kind: ContentKind::Modpack,
        updated: iso_from_secs(p.updated),
        source: ContentSource::Ftb,
        url: format!("https://www.feed-the-beast.com/modpacks/{}{}", p.id, p.slug.as_ref().map(|s| format!("-{s}")).unwrap_or_default()),
    }
}

pub async fn search(q: &SearchQuery) -> Result<SearchResult> {
    if q.kind != ContentKind::Modpack {
        return Ok(SearchResult { hits: vec![], total: 0, offset: 0 });
    }
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct Ids {
        packs: Option<Vec<u64>>,
    }
    let term = q.query.trim();
    // FTB's search needs at least 3 characters; otherwise list the most installed packs.
    let ids: Ids = if term.chars().count() >= 3 {
        fetch_json(&format!("{API}/modpack/search/50?term={}", urlencoding::encode(term))).await?
    } else {
        fetch_json(&format!("{API}/modpack/popular/installs/100")).await?
    };
    let packs: Vec<Option<Arc<FtbPack>>> = stream::iter(ids.packs.unwrap_or_default()).map(|id| async move { pack(id).await.ok() }).buffered(16).collect().await;
    let mut list: Vec<Arc<FtbPack>> = packs.into_iter().flatten().filter(|p| !p.versions.is_empty()).collect();
    if let Some(gv) = q.game_version.as_deref().filter(|v| !v.is_empty()) {
        list.retain(|p| p.versions.iter().any(|v| v.target("game").map(|t| t.version.as_str()) == Some(gv)));
    }
    match q.sort.as_deref() {
        Some("updated" | "newest") => list.sort_by_key(|p| std::cmp::Reverse(p.updated)),
        Some("downloads") => list.sort_by_key(|p| std::cmp::Reverse(p.installs)),
        Some("follows") => list.sort_by_key(|p| std::cmp::Reverse(p.plays)),
        _ => {}
    }
    let offset = q.offset.unwrap_or(0) as usize;
    let limit = q.limit.unwrap_or(20) as usize;
    Ok(SearchResult { hits: list.iter().skip(offset).take(limit).map(|p| to_hit(p)).collect(), total: list.len() as u64, offset: offset as u64 })
}

pub async fn versions(project_id: &str) -> Result<Vec<ProjectVersion>> {
    let p = pack(ftb_id(project_id)).await?;
    let mut vs = p.versions.clone();
    vs.sort_by_key(|v| std::cmp::Reverse(v.released));
    Ok(vs
        .iter()
        .map(|v| ProjectVersion {
            id: v.id.to_string(),
            name: v.name.clone(),
            version_number: v.name.clone(),
            game_versions: vec![v.target("game").map(|t| t.version.clone()).unwrap_or_else(|| "?".into())],
            loaders: vec![v.target("modloader").map(|t| t.name.clone()).unwrap_or_else(|| "minecraft".into())],
            kind: if v.kind == "beta" || v.kind == "alpha" { v.kind.clone() } else { "release".into() },
            published: iso_from_secs(v.released),
            downloads: 0,
        })
        .collect())
}

pub async fn install_modpack(project_id: &str, version_id: Option<String>) -> Result<Instance> {
    let p = pack(ftb_id(project_id)).await?;
    let summary = match &version_id {
        Some(vid) => p.versions.iter().find(|v| &v.id.to_string() == vid),
        None => p.versions.iter().filter(|v| v.kind == "release").max_by_key(|v| v.released).or(p.versions.last()),
    };
    let summary = summary.ok_or_else(|| err_tr!("ftb.noVersion", "{name} has no version to install", name = p.name))?;
    let version: FtbVersion = fetch_json(&format!("{API}/modpack/{}/{}", p.id, summary.id)).await?;

    let mc = version
        .summary
        .target("game")
        .map(|t| t.version.clone())
        .ok_or_else(|| err_tr!("modpack.noMcVersion", "{name} doesn't say which Minecraft version it needs", name = p.name))?;
    let ml = version.summary.target("modloader").cloned();
    let loader = match &ml {
        None => LoaderType::Vanilla,
        Some(t) => parse_loader(&t.name)
            .ok_or_else(|| err_tr!("modpack.needsUnsupportedLoader", "{name} needs {loader}, which ResinCraft doesn't support yet.", name = p.name, loader = t.name))?,
    };

    Task::run(tr!("modpack.installing", "Installing {name}", name = p.name), None, move |task| async move {
        let modpack = ModpackRef { name: p.name.clone(), project_id: Some(format!("ftb:{}", p.id)), version_id: Some(version.summary.id.to_string()) };
        let icon = icon_of(&p);
        // FTB tells us how much memory the pack wants; start there.
        let memory = version.summary.specs.as_ref().map(|s| s.recommended).filter(|m| *m > 0);
        let loader_version = ml.as_ref().map(|t| t.version.clone()).filter(|v| !v.is_empty());
        let new = NewInstance { name: p.name.clone(), mc_version: mc, loader, loader_version, color: None, group: None };
        let inst = instances::create(new, move |i| {
            i.icon_url = icon;
            i.memory_mb = memory;
            i.modpack = Some(modpack);
        })
        .await?;
        let guard = InstanceGuard::new(&inst.id);
        let result: Result<Instance> = async {
            let dir = paths::game_dir(&inst.id);
            let mut items = Vec::new();
            let mut pending = Vec::new();
            for f in &version.files {
                if f.serveronly {
                    continue;
                }
                let folder = f.path.strip_prefix("./").or_else(|| f.path.strip_prefix('.')).unwrap_or(&f.path);
                let folder = folder.strip_suffix('/').unwrap_or(folder).to_string();
                if f.url.is_empty() {
                    // Rare: a CurseForge-only file the author won't let others mirror.
                    pending.push(PendingDownload {
                        title: f.name.clone(),
                        file_name: f.name.clone(),
                        url: f
                            .curseforge
                            .as_ref()
                            .map(|c| format!("https://www.curseforge.com/projects/{}", c.project))
                            .unwrap_or_else(|| "https://www.feed-the-beast.com/".into()),
                        folder,
                    });
                    continue;
                }
                let dest = safe_join(&dir, &format!("{folder}/{}", safe_name(&f.name)?))?;
                let hash = (!f.sha1.is_empty()).then(|| Hash::sha1(&f.sha1));
                items.push(DownloadItem::new(&f.url, dest).hash(hash).size(f.size));
            }
            download_all(items, 8, task.counter(tr!("modpack.downloadingFiles", "Downloading modpack files"))).await?;
            let listed: Vec<String> = version.files.iter().filter(|f| !f.serveronly && !f.url.is_empty() && f.path.trim_matches(['.', '/']) == "mods").map(|f| f.name.clone()).collect();
            dedupe_pack(&inst, &task, &listed).await;
            if !pending.is_empty() {
                instances::update(&inst.id, json!({ "pendingDownloads": pending })).await?;
            }
            instances::get(&inst.id).await
        }
        .await;
        guard.keep();
        if result.is_err() {
            discard_instance(&inst.id).await;
        }
        result
    })
    .await
}
