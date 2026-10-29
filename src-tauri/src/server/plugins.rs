//! Plugins (Paper / Purpur / Folia / Spigot / Leaf → `plugins/`) and mods (Fabric / Quilt / Forge / NeoForge →
//! `mods/`) for hosted servers; the Arclight hybrid has both, managed as two areas. Sources:
//! Modrinth, Hangar (PaperMC), SpigotMC (through the Spiget API), dev.bukkit.org and CurseForge (both through
//! CurseForge's API), GitHub releases, and GeyserMC (Geyser and Floodgate, for Bedrock players). Installed files
//! are tracked in the server's own `content-index.json`, the same format instances use.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::content::curseforge::{cf, cf_raw};
use crate::content::github;
use crate::content::links::{self, Link};
use crate::content::index::{display_cmp, index_key, iso_from_secs, safe_name, ContentIndex, IndexEntry};
use crate::core::ctx::http;
use crate::core::events::Task;
use crate::core::fsutil::{read_json, safe_join, write_json};
use crate::core::net::{download_file, fetch_json, hash_file, post_json, DownloadItem, Hash, HashAlgo};
use crate::core::types::{
    ContentItem, ContentKind, LoaderType, ProjectVersion, ServerAddon, ServerAddonHit, ServerAddonResult, ServerAddonVersion, ServerKind, ServerMeta,
    UpdateRef,
};
use crate::{bail_tr, err_tr, tr};
use crate::core::{settings, shell};

use super::{runtime, store};

const MODRINTH: &str = "https://api.modrinth.com/v2";
const HANGAR: &str = "https://hangar.papermc.io/api/v1";
const SPIGET: &str = "https://api.spiget.org/v2";
const GEYSERMC: &str = "https://download.geysermc.org/v2/projects";
const PAGE: usize = 20;

fn enc(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

/// A server's plugins or its mods, handled as if they were a server of their own: a hybrid's plugins like a Spigot
/// server's, its mods like a server of the mod loader it runs on. `meta.kind` is that stand-in, so everything below
/// only ever sees plugin servers and mod servers.
struct View {
    meta: ServerMeta,
    /// A hybrid's plugins: kept apart from its mods (their own index and queue files).
    hybrid_plugins: bool,
}

impl std::ops::Deref for View {
    type Target = ServerMeta;
    fn deref(&self) -> &ServerMeta {
        &self.meta
    }
}

/// `area` is "plugins" or "mods" for hybrids; other servers have one area and ignore it.
async fn view(id: &str, area: Option<&str>) -> Result<View> {
    let mut meta = store::get(id).await?;
    let mut hybrid_plugins = false;
    if meta.kind.hybrid() {
        if area == Some("mods") {
            meta.kind = ServerKind::from_loader(meta.loader.unwrap_or(LoaderType::Forge));
        } else {
            meta.kind = ServerKind::Spigot;
            hybrid_plugins = true;
        }
    }
    Ok(View { meta, hybrid_plugins })
}

/// The areas of a server: one, or plugins and mods for a hybrid.
fn areas(kind: ServerKind) -> Vec<Option<&'static str>> {
    if kind.hybrid() {
        vec![Some("plugins"), Some("mods")]
    } else {
        vec![None]
    }
}

/// The sources that make sense for a server type, in the order the UI offers them.
fn sources_for(kind: ServerKind) -> Vec<&'static str> {
    match kind {
        ServerKind::Paper | ServerKind::Purpur | ServerKind::Folia | ServerKind::Leaf => vec!["modrinth", "hangar", "spigot", "bukkit", "github", "geysermc"],
        ServerKind::Spigot => vec!["modrinth", "spigot", "bukkit", "github", "geysermc"],
        ServerKind::Vanilla | ServerKind::Bedrock => Vec::new(),
        // Geyser has no Forge build.
        ServerKind::Forge => vec!["modrinth", "curseforge", "github"],
        _ => vec!["modrinth", "curseforge", "github", "geysermc"],
    }
}

pub async fn sources(id: &str, area: Option<&str>) -> Result<Vec<&'static str>> {
    Ok(sources_for(view(id, area).await?.kind))
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

fn index_file(v: &View) -> Result<PathBuf> {
    Ok(store::dir(&v.id)?.join(if v.hybrid_plugins { "content-index-plugins.json" } else { "content-index.json" }))
}
async fn load_index(v: &View) -> ContentIndex {
    match index_file(v) {
        Ok(f) => read_json(&f).await.unwrap_or_default(),
        Err(_) => ContentIndex::new(),
    }
}
async fn save_index(v: &View, idx: &ContentIndex) -> Result<()> {
    write_json(&index_file(v)?, idx).await
}
fn key(filename: &str) -> String {
    index_key(ContentKind::Mod, filename)
}

/// Modrinth loader tags a server type can run.
fn mr_loaders(meta: &ServerMeta) -> Vec<&'static str> {
    match meta.kind {
        ServerKind::Paper | ServerKind::Leaf => vec!["paper", "spigot", "bukkit"],
        ServerKind::Purpur => vec!["purpur", "paper", "spigot", "bukkit"],
        ServerKind::Folia => vec!["folia"],
        ServerKind::Spigot => vec!["spigot", "bukkit"],
        ServerKind::Fabric => vec!["fabric"],
        ServerKind::Quilt => vec!["quilt", "fabric"],
        ServerKind::Forge => vec!["forge"],
        ServerKind::Neoforge if meta.mc_version == "1.20.1" => vec!["neoforge", "forge"],
        ServerKind::Neoforge => vec!["neoforge"],
        // Hybrids are always viewed as Spigot (plugins) or their loader (mods); see `View`.
        ServerKind::Arclight => vec!["spigot", "bukkit"],
        ServerKind::Vanilla | ServerKind::Bedrock => Vec::new(),
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

/// Where a project id comes from. CurseForge's Bukkit plugins are dev.bukkit.org's.
fn source_of(project_id: &str, plugins: bool) -> Option<&'static str> {
    if project_id.is_empty() {
        None
    } else if project_id.starts_with("hangar:") {
        Some("hangar")
    } else if project_id.starts_with("spigot:") {
        Some("spigot")
    } else if project_id.starts_with("cf:") {
        Some(if plugins { "bukkit" } else { "curseforge" })
    } else if project_id.starts_with(github::PREFIX) {
        Some("github")
    } else if project_id.starts_with("geysermc:") {
        Some("geysermc")
    } else {
        Some("modrinth")
    }
}

async fn jar_names(folder: &Path) -> Result<Vec<String>> {
    tokio::fs::create_dir_all(folder).await?;
    let mut names = Vec::new();
    let mut rd = tokio::fs::read_dir(folder).await?;
    while let Some(e) = rd.next_entry().await? {
        let name = e.file_name().to_string_lossy().into_owned();
        let lower = name.to_lowercase();
        if e.file_type().await.map(|t| t.is_file()).unwrap_or(false) && (lower.ends_with(".jar") || lower.ends_with(".jar.disabled")) {
            names.push(name);
        }
    }
    Ok(names)
}

pub async fn list(id: &str, area: Option<&str>) -> Result<Vec<ServerAddon>> {
    let meta = view(id, area).await?;
    let folder = folder(&meta)?;
    let names = jar_names(&folder).await?;
    let mut idx = load_index(&meta).await;
    let unknown: Vec<String> = names.iter().filter(|n| !idx.contains_key(&key(n))).cloned().collect();
    if !unknown.is_empty() && identify(&meta, &folder, &unknown, &mut idx).await.is_ok() {
        let _ = save_index(&meta, &idx).await;
    }
    let pending = load_pending(&meta).await;
    let mut items = Vec::new();
    for n in names {
        let e = idx.get(&key(&n));
        let known = e.filter(|e| !e.project_id.is_empty());
        let size = tokio::fs::metadata(folder.join(&n)).await.map(|m| m.len()).unwrap_or(0);
        let op = pending.iter().find(|p| p.filename() == n);
        items.push(ServerAddon {
            source: known.and_then(|e| source_of(&e.project_id, meta.kind.plugins())),
            pending: op.map(PendingOp::kind),
            pending_version: op.and_then(|p| match p {
                PendingOp::Replace { entry, .. } => Some(entry.version_number.clone()),
                _ => None,
            }),
            update_url: None,
            item: ContentItem {
                enabled: !n.ends_with(".disabled"),
                size,
                kind: ContentKind::Mod,
                // Unknown-source jars still show the name and version from their plugin.yml.
                title: e.map(|e| e.title.clone()).filter(|t| !t.is_empty()),
                project_id: known.map(|e| e.project_id.clone()),
                version_id: known.map(|e| e.version_id.clone()),
                version_number: e.map(|e| e.version_number.clone()).filter(|v| !v.is_empty()),
                icon_url: e.and_then(|e| e.icon_url.clone()),
                update: None,
                filename: n,
            },
        });
    }
    items.sort_by(|a, b| display_cmp(a.item.title.as_deref().unwrap_or(&a.item.filename), b.item.title.as_deref().unwrap_or(&b.item.filename)));
    Ok(items)
}

/// Names files dropped in by hand (or copied from an instance): by hash on Modrinth, then (plugin servers) by the
/// plugin's name on Hangar. Files neither knows are remembered with an empty project id ("unknown source") so
/// they aren't looked up again on every refresh.
async fn identify(meta: &ServerMeta, folder: &Path, names: &[String], idx: &mut ContentIndex) -> Result<()> {
    let mut hashes: HashMap<String, String> = HashMap::new();
    for n in names {
        hashes.insert(hash_file(&folder.join(n), HashAlgo::Sha1).await?, n.clone());
    }
    let found: HashMap<String, MrVersion> =
        post_json(&format!("{MODRINTH}/version_files"), &json!({ "hashes": hashes.keys().collect::<Vec<_>>(), "algorithm": "sha1" })).await?;
    let ids: Vec<String> = found.values().map(|v| v.project_id.clone()).collect::<HashSet<_>>().into_iter().collect();
    let projects: Vec<MrProject> =
        if ids.is_empty() { Vec::new() } else { fetch_json(&format!("{MODRINTH}/projects?ids={}", enc(&serde_json::to_string(&ids)?))).await? };
    for (hash, v) in found {
        let (Some(p), Some(name)) = (projects.iter().find(|p| p.id == v.project_id), hashes.get(&hash)) else { continue };
        idx.insert(
            key(name),
            IndexEntry { project_id: p.id.clone(), version_id: v.id, version_number: v.version_number, title: p.title.clone(), icon_url: p.icon_url.clone(), sha1: hash },
        );
    }
    for (hash, name) in hashes {
        if idx.contains_key(&key(&name)) {
            continue;
        }
        let mut entry = IndexEntry { sha1: hash, ..Default::default() };
        if let Some((plugin, version)) = plugin_yml(&folder.join(&name)).await {
            entry.title = plugin.clone();
            entry.version_number = version.clone();
            if sources_for(meta.kind).contains(&"hangar") {
                if let Ok(p) = fetch_json::<HangarProject>(&format!("{HANGAR}/projects/{}", enc(&plugin))).await {
                    entry.project_id = format!("hangar:{}", p.namespace.slug);
                    entry.version_id = version;
                    entry.title = p.name;
                    entry.icon_url = p.avatar_url;
                }
            }
        }
        idx.insert(key(&name), entry);
    }
    Ok(())
}

/// The `name` and `version` a plugin declares in paper-plugin.yml or plugin.yml.
async fn plugin_yml(jar: &Path) -> Option<(String, String)> {
    let jar = jar.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut zip = crate::content::index::open_zip(&jar).ok()?;
        let text = ["paper-plugin.yml", "plugin.yml"].iter().find_map(|n| crate::content::index::read_zip_text(&mut zip, n).ok().flatten())?;
        let field = |k: &str| {
            text.lines().find_map(|l| {
                let v = l.strip_prefix(k)?.trim_start().strip_prefix(':')?;
                Some(v.trim().trim_matches(['"', '\'']).to_string()).filter(|v| !v.is_empty())
            })
        };
        Some((field("name")?, field("version").unwrap_or_default()))
    })
    .await
    .ok()
    .flatten()
}

pub async fn toggle(id: &str, filename: &str, area: Option<&str>) -> Result<()> {
    let meta = view(id, area).await?;
    let folder = folder(&meta)?;
    safe_name(filename)?;
    if runtime::is_running(id) {
        // A running Java server keeps its jars open (locked on Windows): switch it once the server stops.
        return queue(&meta, PendingOp::Toggle { filename: filename.to_string() }).await;
    }
    toggle_now(&folder, filename).await
}

async fn toggle_now(folder: &Path, filename: &str) -> Result<()> {
    let from = safe_join(folder, safe_name(filename)?)?;
    let to = match filename.strip_suffix(".disabled") {
        Some(base) => safe_join(folder, base)?,
        None => PathBuf::from(format!("{}.disabled", from.display())),
    };
    tokio::fs::rename(&from, &to).await?;
    Ok(())
}

/// Moves the file to the Recycle Bin (never a permanent delete); queued while the server runs.
pub async fn remove(id: &str, filename: &str, area: Option<&str>) -> Result<()> {
    let meta = view(id, area).await?;
    safe_name(filename)?;
    if runtime::is_running(id) {
        return queue(&meta, PendingOp::Remove { filename: filename.to_string() }).await;
    }
    shell::trash(&safe_join(&folder(&meta)?, filename)?).await?;
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

pub async fn search(id: &str, query: &str, source: &str, offset: usize, area: Option<&str>) -> Result<ServerAddonResult> {
    let meta = view(id, area).await?;
    folder(&meta)?;
    match source {
        "modrinth" => modrinth_search(&meta, query, offset).await,
        "hangar" => hangar_search(&meta, query, offset).await,
        "spigot" => spiget_search(query, offset).await,
        "curseforge" | "bukkit" => cf_search(&meta, query, offset).await,
        "github" => gh_search(&meta, query, offset).await,
        "geysermc" => Ok(geyser_search(&meta, query, offset)),
        _ => bail!("Unknown source: {source}"),
    }
}

pub async fn install(id: &str, project_id: &str, version_id: Option<String>, area: Option<&str>) -> Result<()> {
    let meta = view(id, area).await?;
    let folder = folder(&meta)?;
    tokio::fs::create_dir_all(&folder).await?;
    let pid = project_id.to_string();
    Task::run("Installing on the server", Some(meta.id.clone()), move |task| async move {
        let mut idx = load_index(&meta).await;
        install_into(&meta, &folder, &pid, version_id, &mut idx, &task).await?;
        save_index(&meta, &idx).await
    })
    .await
}

async fn install_into(meta: &ServerMeta, folder: &Path, pid: &str, version_id: Option<String>, idx: &mut ContentIndex, task: &std::sync::Arc<Task>) -> Result<()> {
    if let Some(slug) = pid.strip_prefix("hangar:") {
        hangar_install(meta, folder, slug, version_id, idx, task).await
    } else if let Some(rid) = pid.strip_prefix("spigot:") {
        spiget_install(folder, rid, idx, task).await
    } else if let Some(cid) = pid.strip_prefix("cf:") {
        cf_install(meta, folder, cid, version_id, idx, task).await
    } else if pid.starts_with(github::PREFIX) || pid.starts_with("geysermc:") {
        // Geyser and Floodgate for Fabric need Fabric API.
        if pid.starts_with("geysermc:") && geyser_platform(meta).ok() == Some("fabric") && !idx.values().any(|e| e.project_id == FABRIC_API) {
            mr_install(meta, folder, FABRIC_API, None, idx, task).await?;
        }
        task.update(format!("Installing {}…", pid.rsplit([':', '/']).next().unwrap_or(pid)), None);
        let r = resolve(meta, pid, version_id.as_deref()).await?;
        let dest = safe_join(folder, &r.filename)?;
        remove_project(folder, &r.entry.project_id, idx).await;
        download_file(&DownloadItem::new(&r.url, &dest).hash(r.hash.clone()).size(r.size)).await?;
        if !is_jar(&dest).await {
            let _ = tokio::fs::remove_file(&dest).await;
            bail_tr!("hostAddon.notJar", "The download for {name} isn't a plugin file", name = r.entry.title);
        }
        let mut entry = r.entry;
        entry.sha1 = hash_file(&dest, HashAlgo::Sha1).await.unwrap_or_default();
        idx.insert(key(&r.filename), entry);
        Ok(())
    } else {
        mr_install(meta, folder, pid, version_id, idx, task).await
    }
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
                stars: None,
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
            None => newest_release(mr_versions(meta, &pid).await?)
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
        stars: None,
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
    #[serde(rename = "testedVersions")]
    tested_versions: Vec<String>,
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

const SPIGET_FIELDS: &str = "id%2Cname%2Ctag%2Cdownloads%2Cpremium%2Cexternal%2Cfile%2CtestedVersions";

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
                stars: None,
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
                stars: None,
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
            None => newest_cf_release(cf_files(meta, &mid).await?)
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

// ---------- From a link ----------

/// Installs whatever a link points to: a project page on one of the sources (with its version, when the link
/// names one), or a .jar from anywhere else.
pub async fn install_link(id: &str, url: &str, area: Option<&str>) -> Result<()> {
    let meta = view(id, area).await?;
    let folder = folder(&meta)?;
    tokio::fs::create_dir_all(&folder).await?;
    let link = links::parse_link(url)?;
    Task::run("Installing on the server", Some(meta.id.clone()), move |task| async move {
        let mut idx = load_index(&meta).await;
        match link {
            Link::Project { id, mut version } => {
                // Modrinth links name a version by its number as often as by its id.
                if let (false, Some(v)) = (id.contains(':'), &version) {
                    version = Some(links::modrinth_version_id(&id, v).await?);
                }
                install_into(&meta, &folder, &id, version, &mut idx, &task).await?
            }
            Link::CurseForge { slug, class, file } => {
                let cid = links::cf_id_of_slug(&slug, class).await?;
                install_into(&meta, &folder, &format!("cf:{cid}"), file, &mut idx, &task).await?
            }
            Link::File { url, name } => {
                task.update(format!("Downloading {name}…"), None);
                let dest = safe_join(&folder, safe_name(&name)?)?;
                download_file(&DownloadItem::new(&url, &dest)).await?;
                if !is_jar(&dest).await {
                    let _ = tokio::fs::remove_file(&dest).await;
                    bail_tr!("hostAddon.notJar", "The download for {name} isn't a plugin file", name = name);
                }
                // Left out of the index: the next listing names it from its plugin.yml or by hash.
                idx.remove(&key(&name));
            }
        }
        save_index(&meta, &idx).await
    })
    .await
}

// ---------- GitHub releases ----------

/// Repositories tagged `minecraft-plugin` or `minecraft-mod`. Whether a release fits the server is judged from its
/// file names when installing (see `github::pick`).
async fn gh_search(meta: &ServerMeta, query: &str, offset: usize) -> Result<ServerAddonResult> {
    let topic = if meta.kind.plugins() { "minecraft-plugin" } else { "minecraft-mod" };
    let (repos, total) = github::search(query, topic, offset, PAGE).await?;
    Ok(ServerAddonResult {
        total,
        hits: repos
            .into_iter()
            .map(|r| ServerAddonHit {
                id: r.id(),
                url: r.html_url.clone(),
                title: r.name.clone(),
                description: r.description(),
                icon_url: r.avatar(),
                author: r.owner(),
                downloads: 0,
                stars: Some(r.stargazers_count),
                source: "github",
                external: false,
            })
            .collect(),
    })
}

// ---------- GeyserMC (Geyser and Floodgate) ----------

/// GeyserMC's name for the platform a server (area) is: plugin servers run the Spigot builds.
fn geyser_platform(meta: &ServerMeta) -> Result<&'static str> {
    match meta.kind {
        k if k.plugins() => Ok("spigot"),
        ServerKind::Fabric | ServerKind::Quilt => Ok("fabric"),
        ServerKind::Neoforge => Ok("neoforge"),
        _ => bail_tr!(
            "bedrockJoin.unsupported",
            "Geyser doesn't run on {kind} servers. Use a plugin server (Paper, Purpur, Leaf…), Fabric or NeoForge.",
            kind = meta.kind.label()
        ),
    }
}

#[derive(Deserialize)]
struct GeyserDownload {
    name: String,
    sha256: String,
}
#[derive(Deserialize)]
struct GeyserBuild {
    version: String,
    build: u64,
    #[serde(default)]
    time: String,
    #[serde(default)]
    downloads: HashMap<String, GeyserDownload>,
}

/// GeyserMC's own downloads as search results: Geyser and Floodgate (Modrinth lists Geyser as a mod and doesn't
/// have Floodgate's plugin build, so a plugin search there misses them).
fn geyser_search(meta: &ServerMeta, query: &str, offset: usize) -> ServerAddonResult {
    if offset > 0 || geyser_platform(meta).is_err() {
        return ServerAddonResult { hits: Vec::new(), total: 0 };
    }
    let q = query.trim().to_lowercase();
    let wanted = |words: &[&str]| q.is_empty() || words.iter().any(|w| w.contains(q.as_str()) || q.contains(w));
    let mut hits = Vec::new();
    for (project, words, about) in [
        ("geyser", &["geyser", "bedrock", "crossplay"][..], "Lets Bedrock Edition players join this Java server."),
        ("floodgate", &["floodgate", "bedrock", "geyser"][..], "Lets Bedrock players join with no Java account (works with Geyser)."),
    ] {
        if wanted(words) {
            hits.push(ServerAddonHit {
                id: format!("geysermc:{project}"),
                url: format!("https://geysermc.org/download?project={project}"),
                title: geyser_title(project).to_string(),
                description: about.to_string(),
                icon_url: Some(geyser_icon(project).to_string()),
                author: "GeyserMC".into(),
                downloads: 0,
                stars: None,
                source: "geysermc",
                external: false,
            });
        }
    }
    ServerAddonResult { total: hits.len() as u64, hits }
}

/// The newest build of "geyser" or "floodgate".
async fn geyser_latest(project: &str) -> Result<GeyserBuild> {
    fetch_json(&format!("{GEYSERMC}/{project}/versions/latest/builds/latest")).await
}

fn geyser_title(project: &str) -> &'static str {
    if project == "floodgate" {
        "Floodgate"
    } else {
        "Geyser"
    }
}

/// The icons GeyserMC uses for its projects on Modrinth.
fn geyser_icon(project: &str) -> &'static str {
    if project == "floodgate" {
        "https://cdn.modrinth.com/data/bWrNNfkb/851eeb9b5daf37baaebe7e8f25e8437735897c9c_96.webp"
    } else {
        "https://cdn.modrinth.com/data/wKkoqHrH/6e8e781d5669ecef9e19a73b5c2eed060d6f73e7_96.webp"
    }
}

// ---------- Bedrock players (Geyser + Floodgate) ----------

const FABRIC_API: &str = "P7dR8mSH";

/// Where a server stands with Bedrock players: whether Geyser and Floodgate are installed (in the plugins of a
/// hybrid), and which UDP port Bedrock players use.
pub async fn bedrock_status(id: &str) -> Result<Value> {
    let meta = view(id, Some("plugins")).await?;
    let supported = geyser_platform(&meta).is_ok();
    let (mut geyser, mut floodgate) = (false, false);
    if supported {
        let idx = load_index(&meta).await;
        let names = jar_names(&folder(&meta)?).await.unwrap_or_default();
        for n in names.iter().filter(|n| !n.ends_with(".disabled")) {
            let pid = idx.get(&key(n)).map(|e| e.project_id.as_str()).unwrap_or_default();
            let lower = n.to_lowercase();
            geyser |= pid == "geysermc:geyser" || pid == "wKkoqHrH" || lower.starts_with("geyser");
            floodgate |= pid == "geysermc:floodgate" || pid == "bWrNNfkb" || lower.starts_with("floodgate");
        }
    }
    let port = geyser_port(&meta).await.unwrap_or(19132);
    Ok(json!({ "supported": supported, "geyser": geyser, "floodgate": floodgate, "port": port }))
}

/// The Bedrock port in Geyser's config once it exists (`bedrock: port:`), else Geyser's default.
async fn geyser_port(meta: &ServerMeta) -> Option<u16> {
    let dir = store::dir(&meta.id).ok()?;
    for cfg in ["plugins/Geyser-Spigot/config.yml", "config/Geyser-Fabric/config.yml", "config/Geyser-NeoForge/config.yml"] {
        let Ok(text) = tokio::fs::read_to_string(dir.join(cfg)).await else { continue };
        let mut in_bedrock = false;
        for line in text.lines() {
            if !line.starts_with(' ') && !line.trim().is_empty() && !line.starts_with('#') {
                in_bedrock = line.trim_end() == "bedrock:";
            } else if in_bedrock {
                if let Some(p) = line.trim().strip_prefix("port:") {
                    return p.trim().parse().ok();
                }
            }
        }
    }
    None
}

/// Grim Anticheat on Modrinth (GPL-3.0): what Settings → Anti-cheat installs on plugin servers.
const GRIM: &str = "LJNGWSvH";

/// The server's Grim jars (on or switched off).
async fn grim_jars(meta: &View) -> Vec<String> {
    let idx = load_index(meta).await;
    let Ok(dir) = folder(meta) else { return Vec::new() };
    jar_names(&dir)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|n| {
            let base = n.strip_suffix(".disabled").unwrap_or(n);
            let pid = idx.get(&key(n)).or_else(|| idx.get(&key(base))).map(|e| e.project_id.as_str()).unwrap_or_default();
            pid == GRIM || base.to_lowercase().starts_with("grim")
        })
        .collect()
}

/// Settings → Anti-cheat: whether the server can have it and whether Grim is there and on.
pub async fn anticheat_status(id: &str) -> Result<Value> {
    let meta = view(id, Some("plugins")).await?;
    let supported = meta.kind.plugins();
    let jars = if supported { grim_jars(&meta).await } else { Vec::new() };
    Ok(json!({ "supported": supported, "installed": !jars.is_empty(), "enabled": jars.iter().any(|n| !n.ends_with(".disabled")) }))
}

/// Anti-cheat on: installs Grim (the version for this server's Minecraft and software; it writes its recommended
/// config on the first start, with alerts for ops and the console), or switches a Grim that was off back on. Off: Grim
/// goes to the Recycle Bin. While the server runs, the change waits for it to stop, like other plugin changes.
pub async fn anticheat_set(id: &str, on: bool) -> Result<()> {
    let meta = view(id, Some("plugins")).await?;
    if !meta.kind.plugins() {
        bail!("An anti-cheat needs a plugin server (Paper, Purpur, Spigot…).");
    }
    let jars = grim_jars(&meta).await;
    let running_one = jars.iter().any(|n| !n.ends_with(".disabled"));
    if on {
        if running_one {
            return Ok(());
        }
        if let Some(off) = jars.iter().find(|n| n.ends_with(".disabled")) {
            return toggle(id, off, Some("plugins")).await;
        }
        return install(id, GRIM, None, Some("plugins")).await;
    }
    for n in jars {
        remove(id, &n, Some("plugins")).await?;
    }
    Ok(())
}

/// Lets Bedrock players join a Java server: installs Floodgate, then Geyser (and Fabric API on Fabric). With
/// Floodgate there, Geyser signs Bedrock players in through Floodgate by itself, so they need no Java account.
pub async fn bedrock_enable(id: &str) -> Result<()> {
    let meta = view(id, Some("plugins")).await?;
    let platform = geyser_platform(&meta)?;
    let folder = folder(&meta)?;
    tokio::fs::create_dir_all(&folder).await?;
    Task::run(tr!("bedrockJoin.task", "Letting Bedrock players join {server}", server = meta.name), Some(meta.id.clone()), move |task| async move {
        let mut idx = load_index(&meta).await;
        if platform == "fabric" && !idx.values().any(|e| e.project_id == FABRIC_API) {
            mr_install(&meta, &folder, FABRIC_API, None, &mut idx, &task).await?;
        }
        for project in ["floodgate", "geyser"] {
            install_into(&meta, &folder, &format!("geysermc:{project}"), None, &mut idx, &task).await?;
        }
        save_index(&meta, &idx).await
    })
    .await
}

/// Removes Geyser and Floodgate again (to the Recycle Bin, or once the server stops).
pub async fn bedrock_disable(id: &str) -> Result<()> {
    let meta = view(id, Some("plugins")).await?;
    let idx = load_index(&meta).await;
    let names = jar_names(&folder(&meta)?).await.unwrap_or_default();
    for n in names {
        let pid = idx.get(&key(&n)).map(|e| e.project_id.clone()).unwrap_or_default();
        let lower = n.to_lowercase();
        if pid.starts_with("geysermc:") || ["wKkoqHrH", "bWrNNfkb"].contains(&pid.as_str()) || lower.starts_with("geyser") || lower.starts_with("floodgate") {
            remove(id, &n, Some("plugins")).await?;
        }
    }
    Ok(())
}

// ---------- Versions, switching and updates ----------
//
// Switching a jar to another version (or updating it) always downloads into `<server>/.resincraft/staged`,
// verifies it, then swaps it in with one rename, keeping the enabled/disabled state. While the server runs,
// Java holds its jars open (locked on Windows), so swaps, toggles and removals wait in
// `<server>/.resincraft/pending-addons.json` and are applied as soon as the server stops (or before it starts).

/// A change to a jar that waits until the server isn't running.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
enum PendingOp {
    /// Replace `old` with the staged download, saved as `target`.
    Replace { old: String, staged: String, target: String, entry: IndexEntry },
    Toggle { filename: String },
    Remove { filename: String },
}

impl PendingOp {
    fn filename(&self) -> &str {
        match self {
            PendingOp::Replace { old, .. } => old,
            PendingOp::Toggle { filename } | PendingOp::Remove { filename } => filename,
        }
    }
    fn kind(&self) -> &'static str {
        match self {
            PendingOp::Replace { .. } => "update",
            PendingOp::Toggle { filename } if filename.ends_with(".disabled") => "enable",
            PendingOp::Toggle { .. } => "disable",
            PendingOp::Remove { .. } => "remove",
        }
    }
}

fn work_dir(meta: &ServerMeta) -> Result<PathBuf> {
    Ok(store::dir(&meta.id)?.join(".resincraft"))
}

fn pending_file(v: &View) -> Result<PathBuf> {
    Ok(work_dir(v)?.join(if v.hybrid_plugins { "pending-plugins.json" } else { "pending-addons.json" }))
}

async fn load_pending(v: &View) -> Vec<PendingOp> {
    match pending_file(v) {
        Ok(f) => read_json(&f).await.unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

async fn save_pending(v: &View, ops: &[PendingOp]) -> Result<()> {
    let file = pending_file(v)?;
    if ops.is_empty() {
        let _ = tokio::fs::remove_file(&file).await;
        return Ok(());
    }
    write_json(&file, ops).await
}

async fn drop_staged(meta: &ServerMeta, op: &PendingOp) {
    if let (PendingOp::Replace { staged, .. }, Ok(dir)) = (op, work_dir(meta)) {
        if let Ok(p) = safe_join(&dir.join("staged"), staged) {
            let _ = tokio::fs::remove_file(p).await;
        }
    }
}

fn flip_disabled(name: &str) -> String {
    match name.strip_suffix(".disabled") {
        Some(base) => base.to_string(),
        None => format!("{name}.disabled"),
    }
}

/// Queues a change for when the server stops. One change per file: a second toggle cancels the first, a toggle
/// after an update flips the update's enabled state, and anything else replaces what was queued.
async fn queue(meta: &View, op: PendingOp) -> Result<()> {
    let mut ops = load_pending(meta).await;
    let existing = ops.iter().position(|p| p.filename() == op.filename());
    match (existing.map(|i| ops[i].clone()), op) {
        (Some(PendingOp::Toggle { .. }), PendingOp::Toggle { .. }) => {
            ops.remove(existing.unwrap_or(0));
        }
        (Some(PendingOp::Replace { old, staged, target, entry }), PendingOp::Toggle { .. }) => {
            ops[existing.unwrap_or(0)] = PendingOp::Replace { old, staged, target: flip_disabled(&target), entry };
        }
        (Some(PendingOp::Toggle { .. }), PendingOp::Replace { old, staged, target, entry }) => {
            ops[existing.unwrap_or(0)] = PendingOp::Replace { old, staged, target: flip_disabled(&target), entry };
        }
        (Some(prev), op) => {
            drop_staged(meta, &prev).await;
            ops[existing.unwrap_or(0)] = op;
        }
        (None, op) => ops.push(op),
    }
    save_pending(meta, &ops).await
}

async fn apply_op(meta: &ServerMeta, folder: &Path, op: &PendingOp, idx: &mut ContentIndex) -> Result<()> {
    match op {
        PendingOp::Replace { old, staged, target, entry } => {
            let from = safe_join(&work_dir(meta)?.join("staged"), staged)?;
            let to = safe_join(folder, safe_name(target)?)?;
            // One rename: the new jar is either fully there or not at all (it replaces a same-named file).
            tokio::fs::rename(&from, &to).await?;
            if old != target {
                let _ = tokio::fs::remove_file(safe_join(folder, safe_name(old)?)?).await;
            }
            idx.remove(&key(old));
            idx.insert(key(target), entry.clone());
        }
        PendingOp::Toggle { filename } => toggle_now(folder, filename).await?,
        PendingOp::Remove { filename } => {
            shell::trash(&safe_join(folder, safe_name(filename)?)?).await?;
            idx.remove(&key(filename));
        }
    }
    Ok(())
}

/// Applies queued changes. Only for the runtime, at moments when no Java process has the jars open (just
/// before starting, just after exiting). Returns a line per change for the console; failures stay queued.
pub async fn apply_pending(id: &str) -> Result<Vec<String>> {
    let mut done = Vec::new();
    for area in areas(store::get(id).await?.kind) {
        done.extend(apply_area(&view(id, area).await?).await?);
    }
    Ok(done)
}

async fn apply_area(meta: &View) -> Result<Vec<String>> {
    let ops = load_pending(meta).await;
    if ops.is_empty() {
        return Ok(Vec::new());
    }
    let folder = folder(meta)?;
    let mut idx = load_index(meta).await;
    let (mut left, mut done) = (Vec::new(), Vec::new());
    for op in ops {
        let name = op.filename().to_string();
        let mut result = apply_op(meta, &folder, &op, &mut idx).await;
        if result.is_err() {
            // The Java process may need a moment to let go of its files.
            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
            result = apply_op(meta, &folder, &op, &mut idx).await;
        }
        match result {
            Ok(()) => done.push(match &op {
                PendingOp::Replace { entry, .. } => format!("Updated {} to {}", if entry.title.is_empty() { &name } else { &entry.title }, entry.version_number),
                PendingOp::Remove { .. } => format!("Removed {name}"),
                PendingOp::Toggle { filename } if filename.ends_with(".disabled") => format!("Enabled {name}"),
                PendingOp::Toggle { .. } => format!("Disabled {name}"),
            }),
            Err(e) => {
                eprintln!("[server] queued change to {name} failed: {e:#}");
                left.push(op);
            }
        }
    }
    save_index(meta, &idx).await?;
    save_pending(meta, &left).await?;
    Ok(done)
}

/// Cancels whatever is queued for a file.
pub async fn cancel_pending(id: &str, filename: &str, area: Option<&str>) -> Result<()> {
    let meta = view(id, area).await?;
    let mut ops = load_pending(&meta).await;
    if let Some(i) = ops.iter().position(|p| p.filename() == filename) {
        let op = ops.remove(i);
        drop_staged(&meta, &op).await;
    }
    save_pending(&meta, &ops).await
}

/// What switching to a version needs: the download and what to record about it.
struct Resolved {
    url: String,
    filename: String,
    hash: Option<Hash>,
    size: Option<u64>,
    entry: IndexEntry,
}

fn mr_compatible(meta: &ServerMeta, v: &MrVersion) -> bool {
    let loaders = mr_loaders(meta);
    v.game_versions.contains(&meta.mc_version) && v.loaders.iter().any(|l| loaders.contains(&l.as_str()))
}

/// The newest stable release (the list is newest first), else the newest version of any kind.
fn newest_release(list: Vec<MrVersion>) -> Option<MrVersion> {
    let release = list.iter().position(|v| v.version_type == "release");
    let mut list = list;
    match release {
        Some(i) => Some(list.swap_remove(i)),
        None => list.into_iter().next(),
    }
}

/// CurseForge: releaseType 1 = release, 2 = beta, 3 = alpha.
fn newest_cf_release(list: Vec<Value>) -> Option<Value> {
    let release = list.iter().position(|f| cf_u(f, "releaseType") == 1);
    let mut list = list;
    match release {
        Some(i) => Some(list.swap_remove(i)),
        None => list.into_iter().next(),
    }
}

/// Plugin sites often tag "1.21" for every 1.21.x release.
fn tag_matches(tag: &str, mc: &str) -> bool {
    tag == mc || mc.starts_with(&format!("{tag}."))
}

fn cf_fits(meta: &ServerMeta, f: &Value) -> bool {
    let tags: Vec<String> = cf_tags(f).iter().map(|t| t.to_lowercase()).collect();
    if meta.kind.plugins() {
        let versions: Vec<&String> = tags.iter().filter(|t| t.starts_with(|c: char| c.is_ascii_digit())).collect();
        return versions.is_empty() || versions.iter().any(|t| tag_matches(t, &meta.mc_version));
    }
    let loaders = mr_loaders(meta);
    let tagged: Vec<&String> = tags.iter().filter(|t| ["forge", "fabric", "quilt", "neoforge"].contains(&t.as_str())).collect();
    tags.contains(&meta.mc_version) && (tagged.is_empty() || tagged.iter().any(|t| loaders.contains(&t.as_str())))
}

/// Every published version of an installed plugin or mod, newest first, marked compatible or not.
pub async fn versions(id: &str, project_id: &str, area: Option<&str>) -> Result<Vec<ServerAddonVersion>> {
    let meta = view(id, area).await?;
    let mc = meta.mc_version.clone();
    if project_id.starts_with(github::PREFIX) {
        let loaders = mr_loaders(&meta);
        return Ok(github::releases(project_id)
            .await?
            .into_iter()
            .map(|r| {
                let file = github::pick(&r, &mc, &loaders).map(|a| a.name.clone());
                ServerAddonVersion {
                    compatible: file.is_some(),
                    changelog_url: Some(r.html_url.clone()),
                    version: ProjectVersion {
                        id: r.tag_name.clone(),
                        name: file.unwrap_or_else(|| r.title()),
                        version_number: r.title(),
                        game_versions: Vec::new(),
                        loaders: Vec::new(),
                        kind: if r.prerelease { "beta".into() } else { "release".into() },
                        published: r.published_at.clone().unwrap_or_default(),
                        downloads: r.downloads(),
                    },
                }
            })
            .collect());
    }
    if let Some(project) = project_id.strip_prefix("geysermc:") {
        // GeyserMC only serves the newest build of each version.
        let b = geyser_latest(project).await?;
        let platform = geyser_platform(&meta).ok();
        return Ok(vec![ServerAddonVersion {
            compatible: platform.is_some_and(|p| b.downloads.contains_key(p)),
            changelog_url: Some(format!("https://geysermc.org/download?project={project}")),
            version: ProjectVersion {
                id: format!("{}-b{}", b.version, b.build),
                name: format!("{} {}", geyser_title(project), b.version),
                version_number: format!("{} (build {})", b.version, b.build),
                game_versions: Vec::new(),
                loaders: platform.map(|p| vec![p.to_string()]).unwrap_or_default(),
                kind: "release".into(),
                published: b.time,
                downloads: 0,
            },
        }]);
    }
    if let Some(slug) = project_id.strip_prefix("hangar:") {
        let p: HangarProject = fetch_json(&format!("{HANGAR}/projects/{}", enc(slug))).await?;
        let list: Vec<HangarVersion> = fetch_json::<HangarPage<HangarVersion>>(&format!("{HANGAR}/projects/{}/versions?limit=25&offset=0&platform=PAPER", enc(slug))).await?.result;
        return Ok(list
            .into_iter()
            .map(|v| {
                let games = v.platform_dependencies.get("PAPER").cloned().unwrap_or_default();
                ServerAddonVersion {
                    compatible: games.iter().any(|g| tag_matches(g, &mc)),
                    changelog_url: Some(format!("https://hangar.papermc.io/{}/{}/versions/{}", p.namespace.owner, p.namespace.slug, enc(&v.name))),
                    version: ProjectVersion {
                        id: v.name.clone(),
                        version_number: v.name.clone(),
                        game_versions: games,
                        loaders: vec!["paper".into()],
                        kind: if v.channel.name.eq_ignore_ascii_case("release") { "release".into() } else { "beta".into() },
                        published: v.created_at,
                        downloads: v.stats.total_downloads,
                        name: v.name,
                    },
                }
            })
            .collect());
    }
    if let Some(rid) = project_id.strip_prefix("spigot:") {
        // Spiget only mirrors the newest file; older ones are on SpigotMC behind a browser check.
        let (r, v) = spiget_latest(rid).await?;
        let tested = r.tested_versions.clone();
        return Ok(vec![ServerAddonVersion {
            compatible: r.installable() && (tested.is_empty() || tested.iter().any(|t| tag_matches(t, &mc))),
            changelog_url: Some(format!("https://www.spigotmc.org/resources/{}/updates", r.id)),
            version: ProjectVersion { game_versions: tested, ..v },
        }]);
    }
    if let Some(cid) = project_id.strip_prefix("cf:") {
        require_cf().await?;
        let m: Value = cf(&format!("/mods/{cid}"), None).await?;
        let site = m.get("links").map(|l| cf_s(l, "websiteUrl")).unwrap_or_default();
        let mut files: Vec<Value> = cf(&format!("/mods/{cid}/files?pageSize=50"), None).await?;
        files.retain(|f| f.get("isServerPack").and_then(Value::as_bool) != Some(true));
        files.sort_by_key(|f| std::cmp::Reverse(cf_s(f, "fileDate")));
        return Ok(files
            .iter()
            .map(|f| ServerAddonVersion {
                compatible: cf_fits(&meta, f),
                changelog_url: Some(format!("{site}/files/{}", cf_u(f, "id"))),
                version: cf_version(f),
            })
            .collect());
    }
    let list: Vec<MrVersion> = fetch_json(&format!("{MODRINTH}/project/{}/version", enc(project_id))).await?;
    Ok(list
        .into_iter()
        .map(|v| ServerAddonVersion {
            compatible: mr_compatible(&meta, &v),
            changelog_url: Some(format!("https://modrinth.com/project/{}/version/{}", v.project_id, v.id)),
            version: mr_version(v),
        })
        .collect())
}

/// Finds the file for `version_id` (or the newest compatible version) of a project.
async fn resolve(meta: &ServerMeta, project_id: &str, version_id: Option<&str>) -> Result<Resolved> {
    let target = format!("{} {}", meta.kind.label(), meta.mc_version);
    if project_id.starts_with(github::PREFIX) {
        let repo = github::repo(project_id).await?;
        let loaders = mr_loaders(meta);
        let (release, asset) = match version_id {
            Some(tag) => {
                let r = github::release(project_id, tag).await?;
                let a = github::pick(&r, &meta.mc_version, &loaders).or_else(|| r.jars().into_iter().min_by_key(|a| a.name.len())).cloned();
                (r, a)
            }
            None => {
                let list = github::releases(project_id).await?;
                // The newest full release with a file that fits, else the newest pre-release that has one.
                let pick = |pre: bool| list.iter().filter(|r| r.prerelease == pre).find_map(|r| github::pick(r, &meta.mc_version, &loaders).map(|a| (r.clone(), a.clone())));
                match pick(false).or_else(|| pick(true)) {
                    Some((r, a)) => (r, Some(a)),
                    None => {
                        if list.is_empty() {
                            bail_tr!("hostAddon.noReleases", "{name} doesn't publish its files as GitHub releases. Get it from its page: {link}", name = repo.name, link = repo.html_url);
                        }
                        bail_tr!("hostAddon.noVersion", "{name} has no version for {target}", name = repo.name, target = target);
                    }
                }
            }
        };
        let a = asset.ok_or_else(|| err_tr!("hostAddon.noVersion", "{name} has no version for {target}", name = repo.name, target = target))?;
        return Ok(Resolved {
            url: a.url.clone(),
            filename: safe_name(&a.name)?.to_string(),
            hash: a.sha256().map(Hash::sha256),
            size: Some(a.size).filter(|s| *s > 0),
            entry: IndexEntry {
                project_id: repo.id(),
                version_id: release.tag_name.clone(),
                version_number: release.title(),
                title: repo.name.clone(),
                icon_url: repo.avatar(),
                sha1: String::new(),
            },
        });
    }
    if let Some(project) = project_id.strip_prefix("geysermc:") {
        let platform = geyser_platform(meta)?;
        let b = geyser_latest(project).await?;
        let dl = b.downloads.get(platform).ok_or_else(|| anyhow!("{} has no {platform} download", geyser_title(project)))?;
        return Ok(Resolved {
            url: format!("{GEYSERMC}/{project}/versions/{}/builds/{}/downloads/{platform}", enc(&b.version), b.build),
            filename: safe_name(&dl.name)?.to_string(),
            hash: Some(Hash::sha256(&dl.sha256)),
            size: None,
            entry: IndexEntry {
                project_id: format!("geysermc:{project}"),
                version_id: format!("{}-b{}", b.version, b.build),
                version_number: format!("{} (build {})", b.version, b.build),
                title: geyser_title(project).to_string(),
                icon_url: Some(geyser_icon(project).to_string()),
                sha1: String::new(),
            },
        });
    }
    if let Some(slug) = project_id.strip_prefix("hangar:") {
        let p: HangarProject = fetch_json(&format!("{HANGAR}/projects/{}", enc(slug))).await?;
        let v: HangarVersion = match version_id {
            Some(name) => fetch_json(&format!("{HANGAR}/projects/{}/versions/{}", enc(slug), enc(name))).await?,
            None => {
                let list = hangar_version_list(meta, slug).await?;
                let pick = list.iter().find(|v| v.channel.name.eq_ignore_ascii_case("release")).or(list.first()).cloned();
                pick.ok_or_else(|| err_tr!("hostAddon.noVersion", "{name} has no version for {target}", name = p.name, target = target))?
            }
        };
        let dl = v.downloads.get("PAPER").cloned().unwrap_or_default();
        let (Some(url), Some(info)) = (dl.download_url, dl.file_info) else {
            let link = dl.external_url.unwrap_or_else(|| format!("https://hangar.papermc.io/{}/{}", p.namespace.owner, p.namespace.slug));
            bail_tr!("hostAddon.external", "{name} is downloaded from its author's own website: {link}", name = p.name, link = link);
        };
        return Ok(Resolved {
            url,
            filename: safe_name(&info.name)?.to_string(),
            hash: info.sha256_hash.clone().map(Hash::sha256),
            size: info.size_bytes,
            entry: IndexEntry {
                project_id: format!("hangar:{}", p.namespace.slug),
                version_id: v.name.clone(),
                version_number: v.name.clone(),
                title: p.name.clone(),
                icon_url: p.avatar_url.clone(),
                sha1: String::new(),
            },
        });
    }
    if let Some(rid) = project_id.strip_prefix("spigot:") {
        let (r, v) = spiget_latest(rid).await?;
        if version_id.is_some_and(|want| want != v.id) {
            bail_tr!("hostAddon.spigotLatestOnly", "SpigotMC only offers the newest version of {name} for download", name = r.name);
        }
        if !r.installable() {
            bail_tr!("hostAddon.external", "{name} is downloaded from its author's own website: {link}", name = r.name, link = r.page());
        }
        let base: String = r.name.chars().map(|c| if c.is_ascii_alphanumeric() || "-_.".contains(c) { c } else { '-' }).collect();
        return Ok(Resolved {
            url: format!("{SPIGET}/resources/{}/download", r.id),
            filename: format!("{}.jar", base.trim_matches('-')),
            hash: None,
            size: None,
            entry: IndexEntry {
                project_id: format!("spigot:{}", r.id),
                version_id: v.id,
                version_number: v.version_number,
                title: r.name.clone(),
                icon_url: Some(format!("https://www.spigotmc.org/data/resource_icons/{}/{}.jpg", r.id / 1000, r.id)),
                sha1: String::new(),
            },
        });
    }
    if let Some(cid) = project_id.strip_prefix("cf:") {
        require_cf().await?;
        let m: Value = cf(&format!("/mods/{cid}"), None).await?;
        let title = cf_s(&m, "name");
        let f: Value = match version_id {
            Some(fid) => cf(&format!("/mods/{cid}/files/{fid}"), None).await?,
            None => newest_cf_release(cf_files(meta, cid).await?)
                .ok_or_else(|| err_tr!("hostAddon.noVersion", "{name} has no version for {target}", name = title, target = target))?,
        };
        let Some(url) = f.get("downloadUrl").and_then(Value::as_str).filter(|u| !u.is_empty()).map(str::to_string) else {
            let site = m.get("links").map(|l| cf_s(l, "websiteUrl")).unwrap_or_default();
            bail_tr!("hostAddon.external", "{name} is downloaded from its author's own website: {link}", name = title, link = site);
        };
        return Ok(Resolved {
            url,
            filename: safe_name(&cf_s(&f, "fileName"))?.to_string(),
            hash: cf_sha1(&f).map(Hash::sha1),
            size: f.get("fileLength").and_then(Value::as_u64),
            entry: IndexEntry {
                project_id: format!("cf:{cid}"),
                version_id: cf_u(&f, "id").to_string(),
                version_number: cf_s(&f, "displayName"),
                title: title.clone(),
                icon_url: m.get("logo").and_then(|l| l.get("thumbnailUrl")).and_then(Value::as_str).map(str::to_string),
                sha1: String::new(),
            },
        });
    }
    let p: MrProject = fetch_json(&format!("{MODRINTH}/project/{}", enc(project_id))).await?;
    if !meta.kind.plugins() && p.server_side == "unsupported" {
        bail_tr!("hostAddon.clientOnly", "{name} only runs on the game client, so a server can't use it.", name = p.title);
    }
    let v: MrVersion = match version_id {
        Some(vid) => fetch_json(&format!("{MODRINTH}/version/{}", enc(vid))).await?,
        None => newest_release(mr_versions(meta, project_id).await?)
            .ok_or_else(|| err_tr!("hostAddon.noVersion", "{name} has no version for {target}", name = p.title, target = target))?,
    };
    let f = v.files.iter().find(|f| f.primary).or(v.files.first()).ok_or_else(|| anyhow!("{} {} has no files to download", p.title, v.version_number))?;
    Ok(Resolved {
        url: f.url.clone(),
        filename: safe_name(&f.filename)?.to_string(),
        hash: Some(Hash::sha1(&f.hashes.sha1)),
        size: Some(f.size),
        entry: IndexEntry {
            project_id: p.id.clone(),
            version_id: v.id.clone(),
            version_number: v.version_number.clone(),
            title: p.title.clone(),
            icon_url: p.icon_url.clone(),
            sha1: f.hashes.sha1.clone(),
        },
    })
}

/// Switches an installed jar to `version_id`, keeping it enabled or disabled. Returns true when the change
/// waits for the server to stop.
pub async fn change_version(id: &str, filename: &str, version_id: &str, area: Option<&str>) -> Result<bool> {
    let meta = view(id, area).await?;
    let folder = folder(&meta)?;
    safe_name(filename)?;
    let idx = load_index(&meta).await;
    let pid = idx.get(&key(filename)).map(|e| e.project_id.clone()).filter(|p| !p.is_empty());
    let Some(pid) = pid else {
        bail_tr!("hostAddon.unknownSource", "ResinCraft doesn't know where {file} came from, so it can't change its version", file = filename);
    };
    let (filename, version_id) = (filename.to_string(), version_id.to_string());
    Task::run(tr!("hostAddon.switching", "Switching versions on {server}", server = meta.name), Some(meta.id.clone()), move |task| async move {
        let op = stage_version(&meta, &task, &pid, filename, &version_id).await?;
        if runtime::is_running(&meta.id) {
            queue(&meta, op).await?;
            return Ok(true);
        }
        let mut idx = load_index(&meta).await;
        apply_op(&meta, &folder, &op, &mut idx).await?;
        save_index(&meta, &idx).await?;
        Ok(false)
    })
    .await
}

/// Downloads `version_id` of project `pid` next to the server (in `.resincraft/staged`), checked, as the swap for
/// `filename`; nothing in the server's folder changes yet.
async fn stage_version(meta: &ServerMeta, task: &Task, pid: &str, filename: String, version_id: &str) -> Result<PendingOp> {
    let r = resolve(meta, pid, Some(version_id)).await?;
    task.update(tr!("hostAddon.downloading", "Downloading {name} {version}…", name = r.entry.title, version = r.entry.version_number), None);
    let work = work_dir(meta)?.join("staged");
    tokio::fs::create_dir_all(&work).await?;
    let staged = format!("{}-{}", &uuid::Uuid::new_v4().simple().to_string()[..8], r.filename);
    let staged_path = safe_join(&work, &staged)?;
    download_file(&DownloadItem::new(&r.url, &staged_path).hash(r.hash.clone()).size(r.size)).await?;
    if !is_jar(&staged_path).await {
        let _ = tokio::fs::remove_file(&staged_path).await;
        bail_tr!("hostAddon.notJar", "The download for {name} isn't a plugin file", name = r.entry.title);
    }
    let mut entry = r.entry;
    entry.sha1 = hash_file(&staged_path, HashAlgo::Sha1).await?;
    let target = if filename.ends_with(".disabled") { format!("{}.disabled", r.filename) } else { r.filename };
    Ok(PendingOp::Replace { old: filename, staged, target, entry })
}

/// Updates at once on a server: this many downloads run side by side.
const PARALLEL_UPDATES: usize = 4;

/// What Update all did on a server: switched now, waiting for the next restart, or not possible (with why).
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateManyResult {
    pub now: Vec<String>,
    pub queued: Vec<String>,
    pub failed: Vec<crate::content::sources::UpdateFailure>,
}

/// One version switch for [`update_many`].
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionSwitch {
    pub filename: String,
    pub version_id: String,
}

/// Switches several jars at once: [`PARALLEL_UPDATES`] downloads side by side into `.resincraft/staged`, then every
/// swap in one go (or all queued for the next restart while the server runs), with one save of the index.
pub async fn update_many(id: &str, list: Vec<VersionSwitch>, area: Option<&str>) -> Result<UpdateManyResult> {
    let meta = view(id, area).await?;
    let folder = folder(&meta)?;
    let idx = load_index(&meta).await;
    let mut out = UpdateManyResult::default();
    let mut todo = Vec::new();
    for s in list {
        if safe_name(&s.filename).is_err() {
            continue;
        }
        match idx.get(&key(&s.filename)).map(|e| e.project_id.clone()).filter(|p| !p.is_empty()) {
            Some(pid) => todo.push((pid, s)),
            None => out.failed.push(crate::content::sources::UpdateFailure {
                filename: s.filename.clone(),
                title: None,
                error: err_tr!("hostAddon.unknownSource", "ResinCraft doesn't know where {file} came from, so it can't change its version", file = s.filename).to_string(),
            }),
        }
    }
    if todo.is_empty() {
        return Ok(out);
    }
    let total = todo.len();
    Task::run(tr!("hostAddon.updatingMany", "Updating {count} on {server}", count = total, server = meta.name), Some(meta.id.clone()), move |task| async move {
        let done = std::sync::atomic::AtomicUsize::new(0);
        let staged: Vec<(VersionSwitch, Result<PendingOp>)> = futures::stream::iter(todo)
            .map(|(pid, s)| {
                let (meta, task, done) = (&meta, &task, &done);
                async move {
                    let r = stage_version(meta, task, &pid, s.filename.clone(), &s.version_id).await;
                    let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    task.update(tr!("content.updatedOf", "Updated {done} of {total}", done = n, total = total), Some(n as f64 / total as f64));
                    (s, r)
                }
            })
            .buffer_unordered(PARALLEL_UPDATES)
            .collect()
            .await;
        let running = runtime::is_running(&meta.id);
        let mut idx = load_index(&meta).await;
        for (s, r) in staged {
            let applied = match r {
                Ok(op) if running => queue(&meta, op).await.map(|()| true),
                Ok(op) => apply_op(&meta, &folder, &op, &mut idx).await.map(|()| false),
                Err(e) => Err(e),
            };
            match applied {
                Ok(true) => out.queued.push(s.filename),
                Ok(false) => out.now.push(s.filename),
                Err(e) => out.failed.push(crate::content::sources::UpdateFailure { filename: s.filename, title: None, error: e.to_string() }),
            }
        }
        if !out.now.is_empty() {
            save_index(&meta, &idx).await?;
        }
        Ok(out)
    })
    .await
}

/// Marks every plugin/mod with a known source that has a newer compatible version.
pub async fn check_updates(id: &str, area: Option<&str>) -> Result<Vec<ServerAddon>> {
    let meta = view(id, area).await?;
    let mut items = list(id, area).await?;
    let idx = load_index(&meta).await;

    // Modrinth: one request for all of them, by file hash.
    let mut by_hash: HashMap<String, usize> = HashMap::new();
    for (i, a) in items.iter().enumerate() {
        if let Some(e) = idx.get(&key(&a.item.filename)) {
            if a.source == Some("modrinth") && !e.sha1.is_empty() {
                by_hash.insert(e.sha1.clone(), i);
            }
        }
    }
    if !by_hash.is_empty() {
        let latest: HashMap<String, MrVersion> = post_json(
            &format!("{MODRINTH}/version_files/update"),
            &json!({ "hashes": by_hash.keys().collect::<Vec<_>>(), "algorithm": "sha1", "loaders": mr_loaders(&meta), "game_versions": [meta.mc_version] }),
        )
        .await?;
        for (hash, mut v) in latest {
            if let Some(&i) = by_hash.get(&hash) {
                if v.version_type != "release" {
                    // Modrinth's update lookup also offers betas and snapshots; stay on stable releases.
                    match mr_versions(&meta, &v.project_id).await.ok().and_then(newest_release) {
                        Some(r) if r.version_type == "release" => v = r,
                        _ => continue,
                    }
                }
                if items[i].item.version_id.as_deref() != Some(v.id.as_str()) {
                    items[i].update_url = Some(format!("https://modrinth.com/project/{}/version/{}", v.project_id, v.id));
                    items[i].item.update = Some(UpdateRef { version_id: v.id, version_number: v.version_number });
                }
            }
        }
    }

    // The other sites: one project at a time.
    let checks = items.iter().enumerate().filter(|(_, a)| matches!(a.source, Some("hangar" | "spigot" | "curseforge" | "bukkit" | "github" | "geysermc"))).map(|(i, a)| {
        let (pid, current, meta) = (a.item.project_id.clone().unwrap_or_default(), a.item.version_id.clone().unwrap_or_default(), &meta);
        async move {
            let newest: Result<Option<(String, String, String)>> = async {
                if let Some(slug) = pid.strip_prefix("hangar:") {
                    let list = hangar_version_list(meta, slug).await?;
                    let Some(v) = list.iter().find(|v| v.channel.name.eq_ignore_ascii_case("release")).or(list.first()) else { return Ok(None) };
                    let p: HangarProject = fetch_json(&format!("{HANGAR}/projects/{}", enc(slug))).await?;
                    let url = format!("https://hangar.papermc.io/{}/{}/versions/{}", p.namespace.owner, p.namespace.slug, enc(&v.name));
                    return Ok(Some((v.name.clone(), v.name.clone(), url)));
                }
                if let Some(rid) = pid.strip_prefix("spigot:") {
                    let (r, v) = spiget_latest(rid).await?;
                    return Ok(r.installable().then(|| (v.id, v.version_number, format!("https://www.spigotmc.org/resources/{}/updates", r.id))));
                }
                if pid.starts_with(github::PREFIX) || pid.starts_with("geysermc:") {
                    let r = resolve(meta, &pid, None).await?;
                    return Ok(Some((r.entry.version_id, r.entry.version_number, String::new())));
                }
                let cid = pid.trim_start_matches("cf:");
                let Some(f) = newest_cf_release(cf_files(meta, cid).await?) else { return Ok(None) };
                Ok(Some((cf_u(&f, "id").to_string(), cf_s(&f, "displayName"), String::new())))
            }
            .await;
            (i, current, newest)
        }
    });
    for (i, current, newest) in futures::future::join_all(checks).await {
        match newest {
            Ok(Some((vid, number, url))) if vid != current => {
                items[i].update_url = Some(url).filter(|u| !u.is_empty());
                items[i].item.update = Some(UpdateRef { version_id: vid, version_number: number });
            }
            Ok(_) => {}
            Err(e) => eprintln!("[server] update check for {} failed: {e:#}", items[i].item.filename),
        }
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mr(id: &str, kind: &str) -> MrVersion {
        serde_json::from_value(json!({ "id": id, "project_id": "p", "version_number": id, "version_type": kind })).unwrap()
    }

    #[test]
    fn version_tags_match_patch_releases() {
        assert!(tag_matches("1.21", "1.21.1"));
        assert!(tag_matches("1.21.1", "1.21.1"));
        assert!(!tag_matches("1.2", "1.21.1"));
        assert!(!tag_matches("1.21.4", "1.21.1"));
    }

    #[test]
    fn prefers_stable_releases() {
        let list = vec![mr("snap", "beta"), mr("rel", "release"), mr("old", "release")];
        assert_eq!(newest_release(list).unwrap().id, "rel");
        assert_eq!(newest_release(vec![mr("a", "alpha")]).unwrap().id, "a");
        let cf = vec![json!({ "id": 3, "releaseType": 2 }), json!({ "id": 2, "releaseType": 1 })];
        assert_eq!(cf_u(&newest_cf_release(cf).unwrap(), "id"), 2);
    }

    #[test]
    fn queued_changes_describe_themselves() {
        let toggle_off = PendingOp::Toggle { filename: "a.jar".into() };
        let toggle_on = PendingOp::Toggle { filename: "a.jar.disabled".into() };
        assert_eq!((toggle_off.kind(), toggle_on.kind()), ("disable", "enable"));
        assert_eq!(flip_disabled("a.jar"), "a.jar.disabled");
        assert_eq!(flip_disabled("a.jar.disabled"), "a.jar");
        let replace = PendingOp::Replace { old: "a.jar".into(), staged: "x-a.jar".into(), target: "a2.jar".into(), entry: IndexEntry::default() };
        assert_eq!((replace.kind(), replace.filename()), ("update", "a.jar"));
    }
}
