//! Plugins (Paper / Purpur / Folia / Spigot → `plugins/`) and mods (Fabric / Quilt / Forge / NeoForge →
//! `mods/`) for hosted servers. Sources: Modrinth, Hangar (PaperMC), SpigotMC (through the Spiget API) and
//! CurseForge (mods, and Bukkit plugins from dev.bukkit.org). Installed files are tracked in the server's
//! own `content-index.json`, the same format instances use.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::content::curseforge::{cf, cf_raw};
use crate::content::index::{display_cmp, index_key, iso_from_secs, safe_name, ContentIndex, IndexEntry};
use crate::core::ctx::http;
use crate::core::events::Task;
use crate::core::fsutil::{read_json, safe_join, write_json};
use crate::core::net::{download_file, fetch_json, hash_file, post_json, DownloadItem, Hash, HashAlgo};
use crate::core::types::{ContentItem, ContentKind, ProjectVersion, ServerAddonHit, ServerAddonResult, ServerKind, ServerMeta};
use crate::core::{settings, shell};

use super::store;

const MODRINTH: &str = "https://api.modrinth.com/v2";
const HANGAR: &str = "https://hangar.papermc.io/api/v1";
const SPIGET: &str = "https://api.spiget.org/v2";
const PAGE: usize = 20;

fn enc(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

/// The sources that make sense for a server type, in the order the UI offers them.
pub fn sources(kind: ServerKind) -> Vec<&'static str> {
    match kind {
        ServerKind::Paper | ServerKind::Purpur | ServerKind::Folia => vec!["modrinth", "hangar", "spigot", "curseforge"],
        ServerKind::Spigot => vec!["modrinth", "spigot", "curseforge"],
        ServerKind::Vanilla => Vec::new(),
        _ => vec!["modrinth", "curseforge"],
    }
}

fn folder(meta: &ServerMeta) -> Result<PathBuf> {
    let dir = store::dir(&meta.id)?;
    if meta.kind.plugins() {
        Ok(dir.join("plugins"))
    } else if meta.kind.loader().is_some() {
        Ok(dir.join("mods"))
    } else {
        bail!("Vanilla servers can't use plugins or mods. Create a Paper (plugins) or Fabric (mods) server instead.")
    }
}

fn index_file(meta: &ServerMeta) -> Result<PathBuf> {
    Ok(store::dir(&meta.id)?.join("content-index.json"))
}
async fn load_index(meta: &ServerMeta) -> ContentIndex {
    match index_file(meta) {
        Ok(f) => read_json(&f).await.unwrap_or_default(),
        Err(_) => ContentIndex::new(),
    }
}
async fn save_index(meta: &ServerMeta, idx: &ContentIndex) -> Result<()> {
    write_json(&index_file(meta)?, idx).await
}
fn key(filename: &str) -> String {
    index_key(ContentKind::Mod, filename)
}

/// Modrinth loader tags a server type can run.
fn mr_loaders(meta: &ServerMeta) -> Vec<&'static str> {
    match meta.kind {
        ServerKind::Paper => vec!["paper", "spigot", "bukkit"],
        ServerKind::Purpur => vec!["purpur", "paper", "spigot", "bukkit"],
        ServerKind::Folia => vec!["folia"],
        ServerKind::Spigot => vec!["spigot", "bukkit"],
        ServerKind::Fabric => vec!["fabric"],
        ServerKind::Quilt => vec!["quilt", "fabric"],
        ServerKind::Forge => vec!["forge"],
        ServerKind::Neoforge if meta.mc_version == "1.20.1" => vec!["neoforge", "forge"],
        ServerKind::Neoforge => vec!["neoforge"],
        ServerKind::Vanilla => Vec::new(),
    }
}

fn cf_loader_id(kind: ServerKind) -> Option<u32> {
    match kind {
        ServerKind::Forge => Some(1),
        ServerKind::Fabric | ServerKind::Quilt => Some(4),
        ServerKind::Neoforge => Some(6),
        _ => None,
    }
}

// ---------- Listing ----------

pub async fn list(id: &str) -> Result<Vec<ContentItem>> {
    let meta = store::get(id).await?;
    let folder = folder(&meta)?;
    tokio::fs::create_dir_all(&folder).await?;
    let mut names = Vec::new();
    let mut rd = tokio::fs::read_dir(&folder).await?;
    while let Some(e) = rd.next_entry().await? {
        let name = e.file_name().to_string_lossy().into_owned();
        let lower = name.to_lowercase();
        if e.file_type().await.map(|t| t.is_file()).unwrap_or(false) && (lower.ends_with(".jar") || lower.ends_with(".jar.disabled")) {
            names.push(name);
        }
    }
    let mut idx = load_index(&meta).await;
    let unknown: Vec<String> = names.iter().filter(|n| !idx.contains_key(&key(n))).cloned().collect();
    if !unknown.is_empty() && identify(&folder, &unknown, &mut idx).await.is_ok() {
        let _ = save_index(&meta, &idx).await;
    }
    let mut items = Vec::new();
    for n in names {
        let e = idx.get(&key(&n));
        let size = tokio::fs::metadata(folder.join(&n)).await.map(|m| m.len()).unwrap_or(0);
        items.push(ContentItem {
            enabled: !n.ends_with(".disabled"),
            size,
            kind: ContentKind::Mod,
            title: e.map(|e| e.title.clone()),
            project_id: e.map(|e| e.project_id.clone()),
            version_id: e.map(|e| e.version_id.clone()),
            version_number: e.map(|e| e.version_number.clone()),
            icon_url: e.and_then(|e| e.icon_url.clone()),
            update: None,
            filename: n,
        });
    }
    items.sort_by(|a, b| display_cmp(a.title.as_deref().unwrap_or(&a.filename), b.title.as_deref().unwrap_or(&b.filename)));
    Ok(items)
}

/// Names files dropped in by hand (or copied from an instance) via Modrinth's hash lookup.
async fn identify(folder: &Path, names: &[String], idx: &mut ContentIndex) -> Result<()> {
    let mut hashes: HashMap<String, String> = HashMap::new();
    for n in names {
        hashes.insert(hash_file(&folder.join(n), HashAlgo::Sha1).await?, n.clone());
    }
    let found: HashMap<String, MrVersion> = post_json(&format!("{MODRINTH}/version_files"), &json!({ "hashes": hashes.keys().collect::<Vec<_>>(), "algorithm": "sha1" })).await?;
    let ids: Vec<String> = found.values().map(|v| v.project_id.clone()).collect::<HashSet<_>>().into_iter().collect();
    if ids.is_empty() {
        return Ok(());
    }
    let projects: Vec<MrProject> = fetch_json(&format!("{MODRINTH}/projects?ids={}", enc(&serde_json::to_string(&ids)?))).await?;
    for (hash, v) in found {
        let (Some(p), Some(name)) = (projects.iter().find(|p| p.id == v.project_id), hashes.get(&hash)) else { continue };
        idx.insert(
            key(name),
            IndexEntry { project_id: p.id.clone(), version_id: v.id, version_number: v.version_number, title: p.title.clone(), icon_url: p.icon_url.clone(), sha1: hash },
        );
    }
    Ok(())
}

pub async fn toggle(id: &str, filename: &str) -> Result<()> {
    let folder = folder(&store::get(id).await?)?;
    let from = safe_join(&folder, safe_name(filename)?)?;
    let to = match filename.strip_suffix(".disabled") {
        Some(base) => safe_join(&folder, base)?,
        None => PathBuf::from(format!("{}.disabled", from.display())),
    };
    tokio::fs::rename(&from, &to).await?;
    Ok(())
}

/// Moves the file to the Recycle Bin (never a permanent delete).
pub async fn remove(id: &str, filename: &str) -> Result<()> {
    let meta = store::get(id).await?;
    shell::trash(&safe_join(&folder(&meta)?, safe_name(filename)?)?).await?;
    let mut idx = load_index(&meta).await;
    idx.remove(&key(filename));
    save_index(&meta, &idx).await
}

/// Deletes any installed copy of `project_id` so a new version can replace it.
async fn remove_project(folder: &Path, project_id: &str, idx: &mut ContentIndex) {
    let keys: Vec<String> = idx.iter().filter(|(_, e)| e.project_id == project_id).map(|(k, _)| k.clone()).collect();
    for k in keys {
        let name = k.split_once('/').map(|(_, n)| n).unwrap_or(&k);
        if let Ok(p) = safe_join(folder, name) {
            let _ = tokio::fs::remove_file(&p).await;
            let _ = tokio::fs::remove_file(PathBuf::from(format!("{}.disabled", p.display()))).await;
        }
        idx.remove(&k);
    }
}

// ---------- Search ----------

pub async fn search(id: &str, query: &str, source: &str, offset: usize) -> Result<ServerAddonResult> {
    let meta = store::get(id).await?;
    folder(&meta)?;
    match source {
        "modrinth" => modrinth_search(&meta, query, offset).await,
        "hangar" => hangar_search(&meta, query, offset).await,
        "spigot" => spiget_search(query, offset).await,
        "curseforge" => cf_search(&meta, query, offset).await,
        _ => bail!("Unknown source: {source}"),
    }
}

pub async fn versions(id: &str, project_id: &str) -> Result<Vec<ProjectVersion>> {
    let meta = store::get(id).await?;
    if let Some(slug) = project_id.strip_prefix("hangar:") {
        return hangar_versions(&meta, slug).await;
    }
    if let Some(rid) = project_id.strip_prefix("spigot:") {
        return Ok(vec![spiget_latest(rid).await?.1]);
    }
    if let Some(cid) = project_id.strip_prefix("cf:") {
        return Ok(cf_files(&meta, cid).await?.iter().map(cf_version).collect());
    }
    Ok(mr_versions(&meta, project_id).await?.into_iter().map(mr_version).collect())
}

pub async fn install(id: &str, project_id: &str, version_id: Option<String>) -> Result<()> {
    let meta = store::get(id).await?;
    let folder = folder(&meta)?;
    tokio::fs::create_dir_all(&folder).await?;
    let pid = project_id.to_string();
    Task::run("Installing on the server", Some(meta.id.clone()), move |task| async move {
        let mut idx = load_index(&meta).await;
        if let Some(slug) = pid.strip_prefix("hangar:") {
            hangar_install(&meta, &folder, slug, version_id, &mut idx, &task).await?;
        } else if let Some(rid) = pid.strip_prefix("spigot:") {
            spiget_install(&folder, rid, &mut idx, &task).await?;
        } else if let Some(cid) = pid.strip_prefix("cf:") {
            cf_install(&meta, &folder, cid, version_id, &mut idx, &task).await?;
        } else {
            mr_install(&meta, &folder, &pid, version_id, &mut idx, &task).await?;
        }
        save_index(&meta, &idx).await
    })
    .await
}

// ---------- Modrinth ----------

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
    project_type: String,
}
#[derive(Deserialize)]
struct MrSearch {
    hits: Vec<MrHit>,
    total_hits: u64,
}
#[derive(Deserialize, Clone)]
struct MrHashes {
    sha1: String,
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
    #[serde(default)]
    server_side: String,
}

async fn modrinth_search(meta: &ServerMeta, query: &str, offset: usize) -> Result<ServerAddonResult> {
    let plugin = meta.kind.plugins();
    let mut facets: Vec<Vec<String>> = vec![
        vec![format!("project_type:{}", if plugin { "plugin" } else { "mod" })],
        mr_loaders(meta).iter().map(|l| format!("categories:{l}")).collect(),
        vec![format!("versions:{}", meta.mc_version)],
    ];
    if !plugin {
        facets.push(vec!["server_side:required".into(), "server_side:optional".into()]);
    }
    let index = if query.trim().is_empty() { "downloads" } else { "relevance" };
    let url = format!(
        "{MODRINTH}/search?query={}&limit={PAGE}&offset={offset}&index={index}&facets={}",
        enc(query.trim()),
        enc(&serde_json::to_string(&facets)?)
    );
    let r: MrSearch = fetch_json(&url).await?;
    Ok(ServerAddonResult {
        total: r.total_hits,
        hits: r
            .hits
            .into_iter()
            .map(|h| ServerAddonHit {
                url: format!("https://modrinth.com/{}/{}", if plugin { "plugin" } else { h.project_type.as_str() }, h.slug),
                id: h.project_id,
                title: h.title,
                description: h.description,
                icon_url: h.icon_url.filter(|u| !u.is_empty()),
                author: h.author,
                downloads: h.downloads,
                source: "modrinth",
                external: false,
            })
            .collect(),
    })
}

async fn mr_versions(meta: &ServerMeta, project_id: &str) -> Result<Vec<MrVersion>> {
    let loaders = serde_json::to_string(&mr_loaders(meta))?;
    let games = serde_json::to_string(&[&meta.mc_version])?;
    fetch_json(&format!("{MODRINTH}/project/{}/version?loaders={}&game_versions={}", enc(project_id), enc(&loaders), enc(&games))).await
}

fn mr_version(v: MrVersion) -> ProjectVersion {
    ProjectVersion {
        id: v.id,
        name: v.name,
        version_number: v.version_number,
        game_versions: v.game_versions,
        loaders: v.loaders,
        kind: v.version_type,
        published: v.date_published,
        downloads: v.downloads,
    }
}

async fn mr_install(meta: &ServerMeta, folder: &Path, root: &str, version_id: Option<String>, idx: &mut ContentIndex, task: &std::sync::Arc<Task>) -> Result<()> {
    let mut installed: HashSet<String> = idx.values().map(|e| e.project_id.clone()).collect();
    let mut stack: Vec<(String, Option<String>, bool)> = vec![(root.to_string(), version_id, true)];
    while let Some((pid, vid, is_root)) = stack.pop() {
        if !is_root && installed.contains(&pid) {
            continue;
        }
        let p: MrProject = fetch_json(&format!("{MODRINTH}/project/{}", enc(&pid))).await?;
        if !meta.kind.plugins() && p.server_side == "unsupported" {
            bail!("{} only runs on the game client, so a server can't use it.", p.title);
        }
        task.update(format!("Installing {}…", p.title), None);
        let v = match vid {
            Some(v) => fetch_json::<MrVersion>(&format!("{MODRINTH}/version/{}", enc(&v))).await?,
            None => mr_versions(meta, &pid)
                .await?
                .into_iter()
                .next()
                .ok_or_else(|| anyhow!("{} has no version for {} {}", p.title, meta.kind.label(), meta.mc_version))?,
        };
        let f = v.files.iter().find(|f| f.primary).or(v.files.first()).ok_or_else(|| anyhow!("{} has no files to download", p.title))?;
        remove_project(folder, &p.id, idx).await;
        let item = DownloadItem::new(&f.url, safe_join(folder, safe_name(&f.filename)?)?).hash(Some(Hash::sha1(&f.hashes.sha1))).size(Some(f.size));
        download_file(&item).await?;
        idx.insert(
            key(&f.filename),
            IndexEntry {
                project_id: p.id.clone(),
                version_id: v.id.clone(),
                version_number: v.version_number.clone(),
                title: p.title.clone(),
                icon_url: p.icon_url.clone(),
                sha1: f.hashes.sha1.clone(),
            },
        );
        installed.insert(p.id.clone());
        for d in v.dependencies.iter().filter(|d| d.dependency_type == "required") {
            let dp = match (&d.project_id, &d.version_id) {
                (Some(p), _) => p.clone(),
                (None, Some(v)) => fetch_json::<MrVersion>(&format!("{MODRINTH}/version/{}", enc(v))).await?.project_id,
                _ => continue,
            };
            if !installed.contains(&dp) {
                stack.push((dp, d.version_id.clone(), false));
            }
        }
    }
    Ok(())
}

// ---------- Hangar (PaperMC) ----------

#[derive(Deserialize, Default)]
#[serde(default)]
struct HangarNamespace {
    owner: String,
    slug: String,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct HangarStats {
    downloads: u64,
}
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct HangarProject {
    name: String,
    namespace: HangarNamespace,
    stats: HangarStats,
    description: String,
    avatar_url: Option<String>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct HangarPagination {
    count: u64,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct HangarPage<T> {
    pagination: HangarPagination,
    result: Vec<T>,
}
#[derive(Deserialize, Default, Clone)]
#[serde(default, rename_all = "camelCase")]
struct HangarFileInfo {
    name: String,
    sha256_hash: Option<String>,
    size_bytes: Option<u64>,
}
#[derive(Deserialize, Default, Clone)]
#[serde(default, rename_all = "camelCase")]
struct HangarDownload {
    file_info: Option<HangarFileInfo>,
    external_url: Option<String>,
    download_url: Option<String>,
}
#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct HangarChannel {
    name: String,
}
#[derive(Deserialize, Default, Clone)]
#[serde(default, rename_all = "camelCase")]
struct HangarVersionStats {
    total_downloads: u64,
}
#[derive(Deserialize, Default, Clone)]
#[serde(default, rename_all = "camelCase")]
struct HangarDependency {
    name: String,
    required: bool,
}
#[derive(Deserialize, Default, Clone)]
#[serde(default, rename_all = "camelCase")]
struct HangarVersion {
    name: String,
    created_at: String,
    channel: HangarChannel,
    stats: HangarVersionStats,
    downloads: HashMap<String, HangarDownload>,
    plugin_dependencies: HashMap<String, Vec<HangarDependency>>,
    platform_dependencies: HashMap<String, Vec<String>>,
}

fn hangar_hit(p: HangarProject) -> ServerAddonHit {
    ServerAddonHit {
        id: format!("hangar:{}", p.namespace.slug),
        url: format!("https://hangar.papermc.io/{}/{}", p.namespace.owner, p.namespace.slug),
        title: p.name,
        description: p.description,
        icon_url: p.avatar_url,
        author: p.namespace.owner,
        downloads: p.stats.downloads,
        source: "hangar",
        external: false,
    }
}

async fn hangar_search(meta: &ServerMeta, query: &str, offset: usize) -> Result<ServerAddonResult> {
    let q = query.trim();
    let sort = if q.is_empty() { "&sort=-downloads" } else { "" };
    let url = format!("{HANGAR}/projects?q={}&limit={PAGE}&offset={offset}&platform=PAPER&version={}{sort}", enc(q), enc(&meta.mc_version));
    let page: HangarPage<HangarProject> = fetch_json(&url).await?;
    Ok(ServerAddonResult { total: page.pagination.count, hits: page.result.into_iter().map(hangar_hit).collect() })
}

async fn hangar_version_list(meta: &ServerMeta, slug: &str) -> Result<Vec<HangarVersion>> {
    let url = format!("{HANGAR}/projects/{}/versions?limit=25&offset=0&platform=PAPER&platformVersion={}", enc(slug), enc(&meta.mc_version));
    Ok(fetch_json::<HangarPage<HangarVersion>>(&url).await?.result)
}

async fn hangar_versions(meta: &ServerMeta, slug: &str) -> Result<Vec<ProjectVersion>> {
    Ok(hangar_version_list(meta, slug)
        .await?
        .into_iter()
        .map(|v| ProjectVersion {
            id: v.name.clone(),
            version_number: v.name.clone(),
            game_versions: v.platform_dependencies.get("PAPER").cloned().unwrap_or_default(),
            loaders: vec!["paper".into()],
            kind: if v.channel.name.eq_ignore_ascii_case("release") { "release".into() } else { "beta".into() },
            published: v.created_at,
            downloads: v.stats.total_downloads,
            name: v.name,
        })
        .collect())
}

async fn hangar_install(meta: &ServerMeta, folder: &Path, slug: &str, version: Option<String>, idx: &mut ContentIndex, task: &std::sync::Arc<Task>) -> Result<()> {
    let p: HangarProject = fetch_json(&format!("{HANGAR}/projects/{}", enc(slug))).await?;
    task.update(format!("Installing {}…", p.name), None);
    let list = hangar_version_list(meta, slug).await?;
    let v = match &version {
        Some(name) => list.iter().find(|v| &v.name == name),
        None => list.iter().find(|v| v.channel.name.eq_ignore_ascii_case("release")).or(list.first()),
    };
    let v = v.ok_or_else(|| anyhow!("{} has no version for Minecraft {}", p.name, meta.mc_version))?.clone();
    let dl = v.downloads.get("PAPER").cloned().ok_or_else(|| anyhow!("{} has no Paper download", p.name))?;
    let (Some(url), Some(info)) = (dl.download_url.clone(), dl.file_info.clone()) else {
        let link = dl.external_url.unwrap_or_else(|| format!("https://hangar.papermc.io/{}/{}", p.namespace.owner, p.namespace.slug));
        bail!("{} is downloaded from its author's own website: {link}", p.name);
    };
    let project_id = format!("hangar:{}", p.namespace.slug);
    remove_project(folder, &project_id, idx).await;
    let dest = safe_join(folder, safe_name(&info.name)?)?;
    download_file(&DownloadItem::new(url, &dest).hash(info.sha256_hash.clone().map(Hash::sha256)).size(info.size_bytes)).await?;
    idx.insert(
        key(&info.name),
        IndexEntry {
            project_id,
            version_id: v.name.clone(),
            version_number: v.name.clone(),
            title: p.name.clone(),
            icon_url: p.avatar_url.clone(),
            sha1: hash_file(&dest, HashAlgo::Sha1).await.unwrap_or_default(),
        },
    );
    // Required plugin dependencies hosted on Hangar (best effort; the plugin's page lists them too).
    let installed: HashSet<String> = idx.values().map(|e| e.project_id.clone()).collect();
    for d in v.plugin_dependencies.get("PAPER").cloned().unwrap_or_default().into_iter().filter(|d| d.required) {
        let dep_id = format!("hangar:{}", d.name);
        if installed.contains(&dep_id) || d.name.eq_ignore_ascii_case(slug) {
            continue;
        }
        if let Err(e) = Box::pin(hangar_install(meta, folder, &d.name, None, idx, task)).await {
            eprintln!("[server] dependency {} not installed: {e:#}", d.name);
        }
    }
    Ok(())
}

// ---------- SpigotMC (Spiget) ----------

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SpigetFile {
    #[serde(rename = "type")]
    kind: String,
    external_url: Option<String>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct SpigetResource {
    id: u64,
    name: String,
    tag: String,
    downloads: u64,
    premium: bool,
    external: bool,
    file: Option<SpigetFile>,
}
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SpigetVersion {
    id: u64,
    name: String,
    release_date: i64,
    downloads: u64,
}

impl SpigetResource {
    /// Free resources hosted on SpigotMC as a single jar can be downloaded; the rest open in the browser.
    fn installable(&self) -> bool {
        !self.premium && !self.external && self.file.as_ref().is_some_and(|f| f.kind == ".jar")
    }
    fn page(&self) -> String {
        format!("https://www.spigotmc.org/resources/{}/", self.id)
    }
}

const SPIGET_FIELDS: &str = "id%2Cname%2Ctag%2Cdownloads%2Cpremium%2Cexternal%2Cfile";

async fn spiget_search(query: &str, offset: usize) -> Result<ServerAddonResult> {
    let page = offset / PAGE + 1;
    let q = query.trim();
    let url = if q.is_empty() {
        format!("{SPIGET}/resources/free?size={PAGE}&page={page}&sort=-downloads&fields={SPIGET_FIELDS}")
    } else {
        format!("{SPIGET}/search/resources/{}?field=name&size={PAGE}&page={page}&sort=-downloads&fields={SPIGET_FIELDS}", enc(q))
    };
    let res = http().get(&url).header("Accept", "application/json").send().await?;
    // Spiget answers 404 when a search finds nothing.
    if res.status().as_u16() == 404 {
        return Ok(ServerAddonResult { hits: Vec::new(), total: 0 });
    }
    if !res.status().is_success() {
        bail!("SpigotMC search failed (HTTP {})", res.status().as_u16());
    }
    let total = res.headers().get("x-total").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()).unwrap_or(0);
    let list: Vec<SpigetResource> = res.json().await?;
    Ok(ServerAddonResult {
        total,
        hits: list
            .into_iter()
            .map(|r| ServerAddonHit {
                id: format!("spigot:{}", r.id),
                url: r.page(),
                external: !r.installable(),
                icon_url: Some(format!("https://www.spigotmc.org/data/resource_icons/{}/{}.jpg", r.id / 1000, r.id)),
                title: r.name,
                description: r.tag,
                author: String::new(),
                downloads: r.downloads,
                source: "spigot",
            })
            .collect(),
    })
}

async fn spiget_latest(rid: &str) -> Result<(SpigetResource, ProjectVersion)> {
    let rid: u64 = rid.parse().map_err(|_| anyhow!("Unknown SpigotMC resource"))?;
    let r: SpigetResource = fetch_json(&format!("{SPIGET}/resources/{rid}?fields={SPIGET_FIELDS}")).await?;
    let v: SpigetVersion = fetch_json(&format!("{SPIGET}/resources/{rid}/versions/latest")).await.unwrap_or_default();
    let pv = ProjectVersion {
        id: v.id.to_string(),
        name: v.name.clone(),
        version_number: v.name,
        game_versions: Vec::new(),
        loaders: vec!["spigot".into()],
        kind: "release".into(),
        published: iso_from_secs(v.release_date),
        downloads: v.downloads,
    };
    Ok((r, pv))
}

async fn spiget_install(folder: &Path, rid: &str, idx: &mut ContentIndex, task: &std::sync::Arc<Task>) -> Result<()> {
    let (r, v) = spiget_latest(rid).await?;
    if !r.installable() {
        let link = r.file.as_ref().and_then(|f| f.external_url.clone()).unwrap_or_else(|| r.page());
        if r.premium {
            bail!("{} is a paid plugin. Buy and download it on SpigotMC: {}", r.name, r.page());
        }
        bail!("{} is downloaded from its author's own website: {link}", r.name);
    }
    task.update(format!("Installing {}…", r.name), None);
    let project_id = format!("spigot:{}", r.id);
    remove_project(folder, &project_id, idx).await;
    let base: String = r.name.chars().map(|c| if c.is_ascii_alphanumeric() || "-_.".contains(c) { c } else { '-' }).collect();
    let file_name = format!("{}.jar", base.trim_matches('-'));
    let dest = safe_join(folder, safe_name(&file_name)?)?;
    // Spiget mirrors SpigotMC's free downloads (SpigotMC itself sits behind a browser check).
    download_file(&DownloadItem::new(format!("{SPIGET}/resources/{}/download", r.id), &dest)).await?;
    if !is_jar(&dest).await {
        let _ = tokio::fs::remove_file(&dest).await;
        bail!("SpigotMC didn't send a plugin file for {}. Download it from {}", r.name, r.page());
    }
    idx.insert(
        key(&file_name),
        IndexEntry {
            project_id,
            version_id: v.id,
            version_number: v.version_number,
            title: r.name.clone(),
            icon_url: Some(format!("https://www.spigotmc.org/data/resource_icons/{}/{}.jpg", r.id / 1000, r.id)),
            sha1: hash_file(&dest, HashAlgo::Sha1).await.unwrap_or_default(),
        },
    );
    Ok(())
}

/// A jar is a zip: it starts with "PK".
async fn is_jar(p: &Path) -> bool {
    use tokio::io::AsyncReadExt;
    let Ok(mut f) = tokio::fs::File::open(p).await else { return false };
    let mut magic = [0u8; 2];
    f.read_exact(&mut magic).await.is_ok() && &magic == b"PK"
}

// ---------- CurseForge (mods, and Bukkit plugins from dev.bukkit.org) ----------

fn cf_s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or_default().to_string()
}
fn cf_u(v: &Value, k: &str) -> u64 {
    v.get(k).and_then(|x| x.as_u64().or_else(|| x.as_f64().map(|f| f as u64))).unwrap_or(0)
}
fn cf_tags(f: &Value) -> Vec<String> {
    f.get("gameVersions").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()
}
fn cf_class(meta: &ServerMeta) -> u32 {
    // 5 = Bukkit plugins (dev.bukkit.org), 6 = mods.
    if meta.kind.plugins() {
        5
    } else {
        6
    }
}

async fn require_cf() -> Result<()> {
    if settings::curseforge_key().await.is_empty() {
        bail!("CurseForge isn't available in this build of ResinCraft.");
    }
    Ok(())
}

async fn cf_search(meta: &ServerMeta, query: &str, offset: usize) -> Result<ServerAddonResult> {
    require_cf().await?;
    #[derive(Deserialize)]
    struct Paged {
        data: Vec<Value>,
        pagination: Value,
    }
    let mut url = format!(
        "/mods/search?gameId=432&classId={}&searchFilter={}&sortField=2&sortOrder=desc&index={offset}&pageSize={PAGE}",
        cf_class(meta),
        enc(query.trim())
    );
    if !meta.kind.plugins() {
        url.push_str(&format!("&gameVersion={}", enc(&meta.mc_version)));
        if let Some(l) = cf_loader_id(meta.kind) {
            url.push_str(&format!("&modLoaderType={l}"));
        }
    }
    let r: Paged = cf_raw(&url, None).await?;
    let total = cf_u(&r.pagination, "totalCount").min(10_000);
    Ok(ServerAddonResult {
        total,
        hits: r
            .data
            .iter()
            .map(|m| ServerAddonHit {
                id: format!("cf:{}", cf_u(m, "id")),
                title: cf_s(m, "name"),
                description: cf_s(m, "summary"),
                icon_url: m.get("logo").and_then(|l| l.get("thumbnailUrl")).and_then(Value::as_str).filter(|u| !u.is_empty()).map(str::to_string),
                author: m.get("authors").and_then(Value::as_array).and_then(|a| a.first()).map(|a| cf_s(a, "name")).unwrap_or_default(),
                downloads: cf_u(m, "downloadCount"),
                source: "curseforge",
                url: m.get("links").map(|l| cf_s(l, "websiteUrl")).unwrap_or_default(),
                external: m.get("allowModDistribution").and_then(Value::as_bool) == Some(false),
            })
            .collect(),
    })
}

async fn cf_files(meta: &ServerMeta, mod_id: &str) -> Result<Vec<Value>> {
    require_cf().await?;
    let mod_id: u64 = mod_id.parse().map_err(|_| anyhow!("Unknown CurseForge project"))?;
    let mut url = format!("/mods/{mod_id}/files?pageSize=50");
    if !meta.kind.plugins() {
        url.push_str(&format!("&gameVersion={}", enc(&meta.mc_version)));
        if let Some(l) = cf_loader_id(meta.kind) {
            // Quilt also runs Fabric mods and NeoForge 1.20.1 Forge mods; filter those locally instead.
            if meta.kind != ServerKind::Quilt && !(meta.kind == ServerKind::Neoforge && meta.mc_version == "1.20.1") {
                url.push_str(&format!("&modLoaderType={l}"));
            }
        }
    }
    let mut files: Vec<Value> = cf(&url, None).await?;
    files.retain(|f| f.get("isServerPack").and_then(Value::as_bool) != Some(true));
    files.sort_by_key(|f| std::cmp::Reverse(cf_s(f, "fileDate")));
    Ok(files)
}

fn cf_version(f: &Value) -> ProjectVersion {
    let tags = cf_tags(f);
    let release = match cf_u(f, "releaseType") {
        2 => "beta",
        3 => "alpha",
        _ => "release",
    };
    ProjectVersion {
        id: cf_u(f, "id").to_string(),
        name: cf_s(f, "fileName"),
        version_number: cf_s(f, "displayName"),
        game_versions: tags.iter().filter(|t| t.starts_with(|c: char| c.is_ascii_digit())).cloned().collect(),
        loaders: tags.iter().map(|t| t.to_lowercase()).filter(|t| ["forge", "fabric", "quilt", "neoforge", "bukkit"].contains(&t.as_str())).collect(),
        kind: release.into(),
        published: cf_s(f, "fileDate"),
        downloads: cf_u(f, "downloadCount"),
    }
}

fn cf_sha1(f: &Value) -> Option<String> {
    f.get("hashes")?.as_array()?.iter().find(|h| h.get("algo").and_then(Value::as_u64) == Some(1))?.get("value")?.as_str().map(str::to_string)
}

async fn cf_install(meta: &ServerMeta, folder: &Path, mod_id: &str, version: Option<String>, idx: &mut ContentIndex, task: &std::sync::Arc<Task>) -> Result<()> {
    let mut installed: HashSet<String> = idx.values().map(|e| e.project_id.clone()).collect();
    let mut stack: Vec<(String, Option<String>, bool)> = vec![(mod_id.to_string(), version, true)];
    while let Some((mid, fid, is_root)) = stack.pop() {
        let pid = format!("cf:{mid}");
        if !is_root && installed.contains(&pid) {
            continue;
        }
        let m: Value = cf(&format!("/mods/{mid}"), None).await?;
        let title = cf_s(&m, "name");
        task.update(format!("Installing {title}…"), None);
        let file = match fid {
            Some(f) => cf::<Value>(&format!("/mods/{mid}/files/{f}"), None).await?,
            None => cf_files(meta, &mid)
                .await?
                .into_iter()
                .next()
                .ok_or_else(|| anyhow!("{title} has no version for {} {}", meta.kind.label(), meta.mc_version))?,
        };
        let tags: Vec<String> = cf_tags(&file).iter().map(|t| t.to_lowercase()).collect();
        if tags.iter().any(|t| t == "client") && !tags.iter().any(|t| t == "server") {
            bail!("{title} only runs on the game client, so a server can't use it.");
        }
        let site = m.get("links").map(|l| cf_s(l, "websiteUrl")).unwrap_or_default();
        let Some(url) = file.get("downloadUrl").and_then(Value::as_str).filter(|u| !u.is_empty()).map(str::to_string) else {
            bail!("{title}'s author only allows downloads from the CurseForge website: {site}");
        };
        let file_name = cf_s(&file, "fileName");
        remove_project(folder, &pid, idx).await;
        let sha1 = cf_sha1(&file);
        download_file(&DownloadItem::new(url, safe_join(folder, safe_name(&file_name)?)?).hash(sha1.clone().map(Hash::sha1))).await?;
        idx.insert(
            key(&file_name),
            IndexEntry {
                project_id: pid.clone(),
                version_id: cf_u(&file, "id").to_string(),
                version_number: cf_s(&file, "displayName"),
                title: title.clone(),
                icon_url: m.get("logo").and_then(|l| l.get("thumbnailUrl")).and_then(Value::as_str).map(str::to_string),
                sha1: sha1.unwrap_or_default(),
            },
        );
        installed.insert(pid);
        // relationType 3 = required dependency.
        if let Some(deps) = file.get("dependencies").and_then(Value::as_array) {
            for d in deps.iter().filter(|d| cf_u(d, "relationType") == 3) {
                let dep = cf_u(d, "modId").to_string();
                if !installed.contains(&format!("cf:{dep}")) {
                    stack.push((dep, None, false));
                }
            }
        }
    }
    Ok(())
}
