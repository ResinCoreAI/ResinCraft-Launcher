//! Modrinth: search, versions, installs with dependencies, the content list, updates and .mrpack modpacks.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use super::dupes::{dedupe_pack, dep_facts, skipped_text, Guard, ModFacts};
use super::index::{
    discard_instance, display_cmp, extract_overrides, folder_of, index_key, load_index, loaders_for, open_zip, read_zip_text,
    remove_project_files, safe_name, save_index, ContentIndex, IndexEntry,
};
use super::jarmeta::RemoteFile;
use crate::core::events::Task;
use crate::core::fsutil::safe_join;
use crate::core::instances;
use crate::core::net::{download_all, download_file, fetch_json, hash_file, post_json, DownloadItem, Hash, HashAlgo};
use crate::core::paths;
use crate::core::types::{
    source_of, ContentItem, ContentKind, ContentSource, Instance, LoaderType, ModpackRef, NewInstance, ProjectHit, ProjectVersion, SearchQuery,
    SearchResult, UpdateRef,
};
use crate::{bail_tr, err_tr, tr};

const API: &str = "https://api.modrinth.com/v2";

fn enc(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

// ---------- API shapes ----------

#[derive(Deserialize)]
struct MrHit {
    project_id: String,
    slug: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    author: String,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    follows: u64,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    display_categories: Option<Vec<String>>,
    project_type: String,
    #[serde(default)]
    date_modified: String,
}

#[derive(Deserialize, Clone)]
struct MrHashes {
    sha1: String,
    #[serde(default)]
    sha512: String,
}

#[derive(Deserialize, Clone)]
struct MrFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    size: u64,
    hashes: MrHashes,
}

#[derive(Deserialize, Clone)]
struct MrDependency {
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    version_id: Option<String>,
    dependency_type: String,
}

#[derive(Deserialize, Clone)]
struct MrVersion {
    id: String,
    project_id: String,
    #[serde(default)]
    name: String,
    version_number: String,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    loaders: Vec<String>,
    #[serde(default)]
    version_type: String,
    #[serde(default)]
    date_published: String,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    files: Vec<MrFile>,
    #[serde(default)]
    dependencies: Vec<MrDependency>,
}

#[derive(Deserialize, Clone)]
struct MrProject {
    id: String,
    title: String,
    #[serde(default)]
    icon_url: Option<String>,
    project_type: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    team: String,
    #[serde(default)]
    source_url: Option<String>,
    #[serde(default)]
    issues_url: Option<String>,
}

impl MrProject {
    fn links(&self) -> Vec<String> {
        [&self.source_url, &self.issues_url].into_iter().flatten().filter(|l| !l.is_empty()).cloned().collect()
    }
}

fn kind_of(project_type: &str) -> Option<ContentKind> {
    serde_json::from_value(Value::String(project_type.to_string())).ok()
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.filter(|x| !x.is_empty())
}

// ---------- Search ----------

pub async fn search(q: &SearchQuery) -> Result<SearchResult> {
    let mut facets: Vec<Vec<String>> = vec![vec![format!("project_type:{}", q.kind.as_str())]];
    if let Some(v) = q.game_version.as_deref().filter(|v| !v.is_empty()) {
        facets.push(vec![format!("versions:{v}")]);
    }
    if q.kind == ContentKind::Mod {
        match q.loader {
            Some(LoaderType::Quilt) => facets.push(vec!["categories:quilt".into(), "categories:fabric".into()]),
            Some(l) if l != LoaderType::Vanilla => facets.push(vec![format!("categories:{}", l.as_str())]),
            _ => {}
        }
    }
    if q.kind == ContentKind::Modpack {
        facets.push(["fabric", "quilt", "forge", "neoforge"].iter().map(|l| format!("categories:{l}")).collect());
    }
    let url = format!(
        "{API}/search?query={}&facets={}&index={}&offset={}&limit={}",
        enc(&q.query),
        enc(&serde_json::to_string(&facets)?),
        enc(q.sort.as_deref().unwrap_or("relevance")),
        q.offset.unwrap_or(0),
        q.limit.unwrap_or(20)
    );
    #[derive(Deserialize)]
    struct Res {
        hits: Vec<MrHit>,
        total_hits: u64,
        offset: u64,
    }
    let res: Res = fetch_json(&url).await?;
    Ok(SearchResult {
        total: res.total_hits,
        offset: res.offset,
        hits: res
            .hits
            .into_iter()
            .map(|h| ProjectHit {
                url: format!("https://modrinth.com/{}/{}", h.project_type, h.slug),
                kind: kind_of(&h.project_type).unwrap_or(q.kind),
                id: h.project_id,
                slug: h.slug,
                title: h.title,
                description: h.description,
                icon_url: non_empty(h.icon_url),
                author: h.author,
                downloads: h.downloads,
                follows: h.follows,
                categories: h.display_categories.unwrap_or(h.categories),
                updated: h.date_modified,
                source: ContentSource::Modrinth,
            })
            .collect(),
    })
}

async fn project(project_id: &str) -> Result<MrProject> {
    fetch_json(&format!("{API}/project/{}", enc(project_id))).await
}

async fn version(version_id: &str) -> Result<MrVersion> {
    fetch_json(&format!("{API}/version/{}", enc(version_id))).await
}

async fn project_versions(project_id: &str, inst: Option<&Instance>, kind: Option<ContentKind>) -> Result<Vec<MrVersion>> {
    let mut params = Vec::new();
    if let Some(inst) = inst {
        params.push(format!("game_versions={}", enc(&serde_json::to_string(&[&inst.mc_version])?)));
        if kind == Some(ContentKind::Mod) {
            params.push(format!("loaders={}", enc(&serde_json::to_string(&loaders_for(inst))?)));
        }
    }
    fetch_json(&format!("{API}/project/{}/version?{}", enc(project_id), params.join("&"))).await
}

pub async fn versions(project_id: &str, instance_id: Option<&str>) -> Result<Vec<ProjectVersion>> {
    let inst = match instance_id {
        Some(id) => Some(instances::get(id).await?),
        None => None,
    };
    let p = project(project_id).await?;
    let list = project_versions(project_id, inst.as_ref(), kind_of(&p.project_type)).await?;
    Ok(list
        .into_iter()
        .map(|v| ProjectVersion {
            id: v.id,
            name: v.name,
            version_number: v.version_number,
            game_versions: v.game_versions,
            loaders: v.loaders,
            kind: v.version_type,
            published: v.date_published,
            downloads: v.downloads,
        })
        .collect())
}

// ---------- Install ----------

async fn install_version(inst: &Instance, kind: ContentKind, version: &MrVersion, project: &MrProject, idx: &mut ContentIndex) -> Result<()> {
    let file = version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
        .ok_or_else(|| {
            err_tr!("modrinth.noFiles", "{name} {version} has no files to download", name = project.title, version = version.version_number)
        })?;
    let folder = folder_of(inst, kind);
    remove_project_files(inst, kind, &project.id, idx).await?;

    let item = DownloadItem::new(&file.url, safe_join(&folder, safe_name(&file.filename)?)?)
        .hash(Some(Hash::sha1(&file.hashes.sha1)))
        .size(Some(file.size));
    download_file(&item).await?;
    idx.insert(
        index_key(kind, &file.filename),
        IndexEntry {
            project_id: project.id.clone(),
            version_id: version.id.clone(),
            version_number: version.version_number.clone(),
            title: project.title.clone(),
            icon_url: project.icon_url.clone(),
            sha1: file.hashes.sha1.clone(),
        },
    );
    Ok(())
}

pub async fn install(instance_id: &str, project_id: &str, kind: ContentKind, version_id: Option<String>) -> Result<()> {
    let inst = instances::get(instance_id).await?;
    if kind == ContentKind::Mod && inst.loader == LoaderType::Vanilla {
        bail_tr!(
            "modrinth.vanillaInstance",
            "{name} is a vanilla instance. Mods need a mod loader such as Fabric or NeoForge; create a modded instance first.",
            name = inst.name
        );
    }
    let id = inst.id.clone();
    let project_id = project_id.to_string();
    Task::run(tr!("modrinth.installing", "Installing content"), Some(id), move |task| async move {
        let mut idx = load_index(&inst.id).await;
        let mut guard = Guard::open(&inst, kind, &idx).await;
        let res = install_into(&inst, &task, &mut idx, &mut guard, &project_id, kind, version_id).await;
        guard.save().await;
        res?;
        save_index(&inst.id, &idx).await
    })
    .await
}

/// Installs a project and the required dependencies the instance doesn't have yet, recording them in `idx`.
/// A dependency the instance already has from CurseForge (see `guard`) is skipped rather than installed twice.
pub(crate) async fn install_into(
    inst: &Instance,
    task: &Task,
    idx: &mut ContentIndex,
    guard: &mut Guard,
    project_id: &str,
    kind: ContentKind,
    version_id: Option<String>,
) -> Result<()> {
    let mut installed: HashSet<String> = idx.values().map(|e| e.project_id.clone()).collect();
    // Depth-first, like the recursive TypeScript version: (project, pinned version, is the root).
    let mut stack: Vec<(String, Option<String>, bool)> = vec![(project_id.to_string(), version_id, true)];
    while let Some((pid, vid, root)) = stack.pop() {
        if !root && installed.contains(&pid) {
            continue;
        }
        let project = project(&pid).await?;
        task.update(tr!("content.installingItem", "Installing {name}…", name = project.title), None);
        let version = match &vid {
            Some(v) => Some(version(v).await?),
            None => project_versions(&pid, Some(inst), Some(kind)).await?.into_iter().next(),
        };
        let Some(version) = version else {
            if kind == ContentKind::Mod {
                bail_tr!(
                    "modrinth.noVersionWithLoader",
                    "{name} has no version for Minecraft {mc} with {loader}",
                    name = project.title,
                    mc = inst.mc_version,
                    loader = inst.loader.as_str()
                );
            }
            bail_tr!("modrinth.noVersion", "{name} has no version for Minecraft {mc}", name = project.title, mc = inst.mc_version);
        };
        let mut facts = dep_facts(
            project.id.clone(),
            &project.title,
            &project.slug,
            vec![],
            project.links(),
            version.files.iter().map(|f| f.hashes.sha1.clone()).collect(),
        );
        if !root {
            if let Some((copy, _)) = guard.copy_of(&mut facts).await {
                task.update(skipped_text(&project.title, &copy), None);
                installed.insert(project.id.clone());
                continue;
            }
        }
        install_version(inst, kind, &version, &project, idx).await?;
        installed.insert(project.id.clone());
        guard.add(facts);
        // Pull in required dependencies we don't already have.
        let mut deps = Vec::new();
        for dep in &version.dependencies {
            if dep.dependency_type != "required" {
                continue;
            }
            let mut dep_project = dep.project_id.clone();
            if dep_project.is_none() {
                if let Some(v) = &dep.version_id {
                    dep_project = Some(self::version(v).await?.project_id);
                }
            }
            let Some(dp) = dep_project else { continue };
            if installed.contains(&dp) {
                continue;
            }
            deps.push((dp, dep.version_id.clone(), false));
        }
        // Reverse so the first dependency is visited first.
        stack.extend(deps.into_iter().rev());
    }
    Ok(())
}

// ---------- Listing / toggling ----------

fn matches_kind(kind: ContentKind, name: &str) -> bool {
    let n = name.to_lowercase();
    let n = n.strip_suffix(".disabled").unwrap_or(&n);
    match kind {
        ContentKind::Mod => n.ends_with(".jar"),
        ContentKind::Resourcepack | ContentKind::Shader => n.ends_with(".zip"),
        ContentKind::Modpack => false,
    }
}

pub async fn list_content(instance_id: &str, kind: ContentKind) -> Result<Vec<ContentItem>> {
    let inst = instances::get(instance_id).await?;
    let folder = folder_of(&inst, kind);
    tokio::fs::create_dir_all(&folder).await?;
    let mut names = Vec::new();
    let mut dir = tokio::fs::read_dir(&folder).await?;
    while let Some(e) = dir.next_entry().await? {
        let name = e.file_name().to_string_lossy().into_owned();
        if matches_kind(kind, &name) {
            names.push(name);
        }
    }
    let mut idx = load_index(&inst.id).await;

    // Identify files we didn't install ourselves (dropped in by hand) via their hash.
    let unknown: Vec<&String> = names.iter().filter(|n| !idx.contains_key(&index_key(kind, n))).collect();
    if !unknown.is_empty() {
        // Offline or Modrinth unavailable: show the bare file names.
        let _ = identify_by_hash(&inst, kind, &folder, &unknown, &mut idx).await;
    }

    let mut items = Vec::new();
    for n in &names {
        let e = idx.get(&index_key(kind, n));
        let size = tokio::fs::metadata(folder.join(n)).await.map(|m| m.len()).unwrap_or(0);
        items.push(ContentItem {
            filename: n.clone(),
            enabled: !n.ends_with(".disabled"),
            size,
            kind,
            title: e.map(|e| e.title.clone()),
            project_id: e.map(|e| e.project_id.clone()),
            version_id: e.map(|e| e.version_id.clone()),
            version_number: e.map(|e| e.version_number.clone()),
            icon_url: e.and_then(|e| e.icon_url.clone()),
            update: None,
        });
    }
    items.sort_by(|a, b| display_cmp(a.title.as_deref().unwrap_or(&a.filename), b.title.as_deref().unwrap_or(&b.filename)));
    Ok(items)
}

async fn identify_by_hash(inst: &Instance, kind: ContentKind, folder: &Path, unknown: &[&String], idx: &mut ContentIndex) -> Result<()> {
    let mut hashes: HashMap<String, String> = HashMap::new();
    for n in unknown {
        hashes.insert(hash_file(&folder.join(n), HashAlgo::Sha1).await?, (*n).clone());
    }
    let keys: Vec<&String> = hashes.keys().collect();
    let found: HashMap<String, MrVersion> = post_json(&format!("{API}/version_files"), &json!({ "hashes": keys, "algorithm": "sha1" })).await?;
    let ids: Vec<String> = found.values().map(|v| v.project_id.clone()).collect::<HashSet<_>>().into_iter().collect();
    let projects: Vec<MrProject> =
        if ids.is_empty() { vec![] } else { fetch_json(&format!("{API}/projects?ids={}", enc(&serde_json::to_string(&ids)?))).await? };
    for (hash, v) in found {
        let (Some(p), Some(name)) = (projects.iter().find(|x| x.id == v.project_id), hashes.get(&hash)) else { continue };
        idx.insert(
            index_key(kind, name),
            IndexEntry {
                project_id: p.id.clone(),
                version_id: v.id,
                version_number: v.version_number,
                title: p.title.clone(),
                icon_url: p.icon_url.clone(),
                sha1: hash,
            },
        );
    }
    save_index(&inst.id, idx).await
}

pub async fn toggle_content(instance_id: &str, kind: ContentKind, filename: &str) -> Result<()> {
    let folder = folder_of(&instances::get(instance_id).await?, kind);
    let from = safe_join(&folder, filename)?;
    let to = match filename.strip_suffix(".disabled") {
        Some(base) => safe_join(&folder, base)?,
        None => PathBuf::from(format!("{}.disabled", from.display())),
    };
    tokio::fs::rename(&from, &to).await?;
    Ok(())
}

/// Moves a file to the Recycle Bin (not a permanent delete) so mistakes are recoverable.
pub async fn remove_content(instance_id: &str, kind: ContentKind, filename: &str) -> Result<()> {
    let inst = instances::get(instance_id).await?;
    crate::core::shell::trash(&safe_join(&folder_of(&inst, kind), filename)?).await?;
    let mut idx = load_index(&inst.id).await;
    idx.remove(&index_key(kind, filename));
    save_index(&inst.id, &idx).await
}

// ---------- Updates ----------

pub async fn check_updates(instance_id: &str) -> Result<Vec<ContentItem>> {
    let inst = instances::get(instance_id).await?;
    let mut mods = list_content(instance_id, ContentKind::Mod).await?;
    let idx = load_index(&inst.id).await;
    // CurseForge-tracked files are checked by curseforge.rs; asking Modrinth about them would mix up version ids.
    let mut by_hash: HashMap<String, usize> = HashMap::new();
    for (i, m) in mods.iter().enumerate() {
        if let Some(e) = idx.get(&index_key(ContentKind::Mod, &m.filename)) {
            if source_of(&e.project_id) == ContentSource::Modrinth {
                by_hash.insert(e.sha1.clone(), i);
            }
        }
    }
    if by_hash.is_empty() || inst.loader == LoaderType::Vanilla {
        return Ok(mods);
    }
    let latest: HashMap<String, MrVersion> = post_json(
        &format!("{API}/version_files/update"),
        &json!({
            "hashes": by_hash.keys().collect::<Vec<_>>(),
            "algorithm": "sha1",
            "loaders": loaders_for(&inst),
            "game_versions": [inst.mc_version],
        }),
    )
    .await?;
    for (hash, v) in latest {
        if let Some(&i) = by_hash.get(&hash) {
            if mods[i].version_id.as_deref() != Some(v.id.as_str()) {
                mods[i].update = Some(UpdateRef { version_id: v.id, version_number: v.version_number });
            }
        }
    }
    Ok(mods)
}

// ---------- Modpacks (.mrpack) ----------

const PACK_HOSTS: [&str; 4] = ["cdn.modrinth.com", "github.com", "raw.githubusercontent.com", "gitlab.com"];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackFile {
    path: String,
    hashes: PackHashes,
    #[serde(default)]
    env: Option<PackEnv>,
    #[serde(default)]
    downloads: Vec<String>,
    #[serde(default)]
    file_size: Option<u64>,
}

#[derive(Deserialize)]
struct PackHashes {
    sha1: String,
}

#[derive(Deserialize)]
struct PackEnv {
    #[serde(default)]
    client: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackIndex {
    name: String,
    #[serde(default)]
    version_id: Option<String>,
    #[serde(default)]
    files: Vec<PackFile>,
    #[serde(default)]
    dependencies: HashMap<String, String>,
}

/// Extra details about where a pack came from, stored on the new instance.
#[derive(Default, Clone)]
pub struct PackMeta {
    pub icon_url: Option<String>,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
}

pub async fn install_mrpack(file: &Path, meta: PackMeta) -> Result<Instance> {
    let f = file.to_path_buf();
    let raw = tokio::task::spawn_blocking(move || -> Result<Option<String>> {
        let mut zip = open_zip(&f).map_err(|_| err_tr!("modrinth.notModpack", "This file isn't a Modrinth modpack (.mrpack)"))?;
        read_zip_text(&mut zip, "modrinth.index.json")
    })
    .await??;
    let raw = raw.filter(|r| !r.is_empty()).ok_or_else(|| err_tr!("modrinth.notModpack", "This file isn't a Modrinth modpack (.mrpack)"))?;
    let pack: PackIndex = serde_json::from_str(&raw)?;
    let deps = &pack.dependencies;
    let (loader, loader_version) = if let Some(v) = deps.get("fabric-loader") {
        (LoaderType::Fabric, Some(v.clone()))
    } else if let Some(v) = deps.get("quilt-loader") {
        (LoaderType::Quilt, Some(v.clone()))
    } else if let Some(v) = deps.get("neoforge") {
        (LoaderType::Neoforge, Some(v.clone()))
    } else if let Some(v) = deps.get("forge") {
        (LoaderType::Forge, Some(v.clone()))
    } else {
        (LoaderType::Vanilla, None)
    };
    let mc = deps.get("minecraft").cloned().ok_or_else(|| err_tr!("modrinth.noMcVersion", "The modpack does not say which Minecraft version it needs"))?;

    let file = file.to_path_buf();
    Task::run(tr!("modpack.installing", "Installing {name}", name = pack.name), None, move |task| async move {
        let modpack = ModpackRef { name: pack.name.clone(), project_id: meta.project_id.clone(), version_id: meta.version_id.clone().or(pack.version_id.clone()) };
        let icon = meta.icon_url.clone();
        // Without a pinned loader version, `create` picks the latest one.
        let new = NewInstance { name: pack.name.clone(), mc_version: mc, loader, loader_version, color: None, group: None };
        let inst = instances::create(new, move |i| {
            i.icon_url = icon;
            i.modpack = Some(modpack);
        })
        .await?;
        match fill_pack(&inst, &pack, &file, &task).await {
            Ok(()) => Ok(inst),
            Err(e) => {
                // Don't leave a half-installed instance behind.
                discard_instance(&inst.id).await;
                Err(e)
            }
        }
    })
    .await
}

async fn fill_pack(inst: &Instance, pack: &PackIndex, file: &Path, task: &Arc<Task>) -> Result<()> {
    let dir = paths::game_dir(&inst.id);
    let mut items = Vec::new();
    let mut listed = Vec::new();
    for f in pack.files.iter().filter(|f| f.env.as_ref().and_then(|e| e.client.as_deref()) != Some("unsupported")) {
        let url = f
            .downloads
            .iter()
            .find(|u| reqwest::Url::parse(u).ok().and_then(|x| x.host_str().map(|h| PACK_HOSTS.contains(&h))).unwrap_or(false))
            .ok_or_else(|| err_tr!("modrinth.untrustedHost", "Modpack file {path} comes from an untrusted host", path = f.path))?;
        items.push(DownloadItem::new(url, safe_join(&dir, &f.path)?).hash(Some(Hash::sha1(&f.hashes.sha1))).size(f.file_size));
        listed.extend(f.path.strip_prefix("mods/").filter(|n| !n.contains('/')).map(str::to_string));
    }
    download_all(items, 8, task.counter(tr!("modpack.downloadingFiles", "Downloading modpack files"))).await?;

    task.update(tr!("modpack.applyingSettings", "Applying modpack settings…"), None);
    let (file, dir) = (file.to_path_buf(), dir.clone());
    tokio::task::spawn_blocking(move || extract_overrides(&file, &["overrides/".into(), "client-overrides/".into()], &dir)).await??;
    // The overrides may hold another copy of a listed mod.
    dedupe_pack(inst, task, &listed).await;
    Ok(())
}

pub async fn install_modpack(project_id: &str, version_id: Option<String>) -> Result<Instance> {
    let p = project(project_id).await?;
    let v = match &version_id {
        Some(v) => Some(version(v).await?),
        None => fetch_json::<Vec<MrVersion>>(&format!("{API}/project/{}/version", enc(project_id)))
            .await?
            .into_iter()
            .find(|v| v.loaders.iter().any(|l| ["fabric", "quilt", "forge", "neoforge", "minecraft"].contains(&l.as_str()))),
    };
    let no_version = || err_tr!("modrinth.noInstallableVersion", "{name} has no version ResinCraft can install", name = p.title);
    let v = v.ok_or_else(no_version)?;
    let file = v.files.iter().find(|f| f.primary).or_else(|| v.files.first()).ok_or_else(no_version)?;
    let dest = safe_join(&paths::cache().join("modpacks"), safe_name(&file.filename)?)?;
    let item = DownloadItem::new(&file.url, &dest).hash(Some(Hash::sha1(&file.hashes.sha1))).size(Some(file.size));
    Task::run(tr!("modpack.downloading", "Downloading {name}", name = p.title), None, |_| async move { download_file(&item).await }).await?;
    let res = install_mrpack(&dest, PackMeta { icon_url: p.icon_url.clone(), project_id: Some(p.id.clone()), version_id: Some(v.id.clone()) }).await;
    let _ = tokio::fs::remove_file(&dest).await;
    res
}

/// Download info for a file Modrinth knows.
#[derive(Debug, Clone)]
pub struct HashHit {
    pub url: String,
    pub sha512: String,
    pub size: u64,
}

/// Finds Modrinth download info for local files by SHA-1 (used by modpack export). Missing hashes aren't on Modrinth.
pub async fn lookup_hashes(sha1s: &[String]) -> Result<HashMap<String, HashHit>> {
    let mut out = HashMap::new();
    if sha1s.is_empty() {
        return Ok(out);
    }
    let found: HashMap<String, MrVersion> = post_json(&format!("{API}/version_files"), &json!({ "hashes": sha1s, "algorithm": "sha1" })).await?;
    for (sha1, v) in found {
        if let Some(f) = v.files.iter().find(|x| x.hashes.sha1 == sha1) {
            out.insert(sha1.clone(), HashHit { url: f.url.clone(), sha512: f.hashes.sha512.clone(), size: f.size });
        }
    }
    Ok(out)
}

/// Which Modrinth project publishes each of these files (by SHA-1). Unknown hashes are left out.
pub(crate) async fn projects_by_sha1(sha1s: &[String]) -> Result<HashMap<String, String>> {
    let mut out = HashMap::new();
    for chunk in sha1s.chunks(500) {
        let found: HashMap<String, MrVersion> = post_json(&format!("{API}/version_files"), &json!({ "hashes": chunk, "algorithm": "sha1" })).await?;
        out.extend(found.into_iter().map(|(h, v)| (h, v.project_id)));
    }
    Ok(out)
}

/// The file installing `project_id` into `inst` would download (`version_id`, else the newest compatible version).
pub(crate) async fn planned_file(inst: &Instance, project_id: &str, kind: ContentKind, version_id: Option<&str>) -> Result<Option<RemoteFile>> {
    let v = match version_id {
        Some(v) => Some(version(v).await?),
        None => project_versions(project_id, Some(inst), Some(kind)).await?.into_iter().next(),
    };
    Ok(v.and_then(|v| v.files.iter().find(|f| f.primary).or(v.files.first()).map(|f| RemoteFile { url: f.url.clone(), size: Some(f.size) })))
}

/// Names, slugs, links and team members of Modrinth projects, for spotting the same mod on CurseForge.
pub(crate) async fn facts(ids: &[String]) -> Result<Vec<ModFacts>> {
    #[derive(Deserialize)]
    struct User {
        username: String,
    }
    #[derive(Deserialize)]
    struct Member {
        team_id: String,
        user: User,
    }
    let mut out = Vec::new();
    for chunk in ids.chunks(100) {
        let projects: Vec<MrProject> = fetch_json(&format!("{API}/projects?ids={}", enc(&serde_json::to_string(chunk)?))).await?;
        let teams: Vec<&String> = projects.iter().map(|p| &p.team).filter(|t| !t.is_empty()).collect::<HashSet<_>>().into_iter().collect();
        // Authors are a nice-to-have: without them, matching falls back to links, slugs and files.
        let members: Vec<Vec<Member>> = if teams.is_empty() {
            vec![]
        } else {
            fetch_json(&format!("{API}/teams?ids={}", enc(&serde_json::to_string(&teams)?))).await.unwrap_or_default()
        };
        for p in projects {
            let authors = members.iter().flatten().filter(|m| m.team_id == p.team).map(|m| m.user.username.clone()).collect();
            out.push(dep_facts(p.id.clone(), &p.title, &p.slug, authors, p.links(), vec![]));
        }
    }
    Ok(out)
}
