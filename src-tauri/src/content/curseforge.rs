//! CurseForge's public API (https://docs.curseforge.com/rest-api/). Every call needs the user's API key.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use super::index::{
    discard_instance, extract_overrides, folder_of, index_key, load_index, loaders_for, open_zip, parse_date_ms, read_zip_text, remove_project_files,
    safe_name, save_index, ContentIndex, IndexEntry,
};
use crate::core::ctx::http;
use crate::core::events::Task;
use crate::core::fsutil::safe_join;
use crate::core::instances;
use crate::core::net::{download_all, download_file, send_json, status_of, DownloadItem, Hash};
use crate::core::paths;
use crate::core::settings;
use crate::core::text::Text;
use crate::core::types::{
    ContentItem, ContentKind, ContentSource, Instance, LoaderType, ModpackRef, NewInstance, PendingDownload, ProjectHit, ProjectVersion, SearchQuery,
    SearchResult, UpdateRef,
};
use crate::{bail_tr, err_tr, tr};

const API: &str = "https://api.curseforge.com/v1";
const MINECRAFT: u32 = 432;
const REQUIRED_DEPENDENCY: u32 = 3;
const SHA1: u32 = 1;

fn class_of(kind: ContentKind) -> u32 {
    match kind {
        ContentKind::Mod => 6,
        ContentKind::Modpack => 4471,
        ContentKind::Resourcepack => 12,
        ContentKind::Shader => 6552,
    }
}

fn kind_of_class(class: u32) -> Option<ContentKind> {
    match class {
        6 => Some(ContentKind::Mod),
        4471 => Some(ContentKind::Modpack),
        12 => Some(ContentKind::Resourcepack),
        6552 => Some(ContentKind::Shader),
        _ => None,
    }
}

fn folder_of_class(class: u32) -> &'static str {
    match class {
        12 => "resourcepacks",
        6552 => "shaderpacks",
        _ => "mods",
    }
}

fn loader_id(l: LoaderType) -> Option<u32> {
    match l {
        LoaderType::Forge => Some(1),
        LoaderType::Fabric => Some(4),
        LoaderType::Quilt => Some(5),
        LoaderType::Neoforge => Some(6),
        LoaderType::Vanilla => None,
    }
}

const LOADER_NAMES: [&str; 4] = ["forge", "fabric", "quilt", "neoforge"];

/// "cf:238222" → 238222.
pub fn cf_id(project_id: &str) -> u64 {
    project_id.strip_prefix("cf:").unwrap_or(project_id).parse().unwrap_or(0)
}

fn prefixed(id: u64) -> String {
    format!("cf:{id}")
}

// ---------- API shapes ----------

#[derive(Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
struct CfLogo {
    thumbnail_url: String,
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct CfName {
    name: String,
}

#[derive(Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
struct CfLinks {
    website_url: String,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CfMod {
    id: u64,
    name: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    class_id: Option<u32>,
    #[serde(default)]
    download_count: f64,
    #[serde(default)]
    thumbs_up_count: f64,
    #[serde(default)]
    date_modified: String,
    #[serde(default)]
    logo: Option<CfLogo>,
    #[serde(default)]
    authors: Vec<CfName>,
    #[serde(default)]
    categories: Vec<CfName>,
    #[serde(default)]
    links: CfLinks,
}

impl CfMod {
    fn icon(&self) -> Option<String> {
        self.logo.as_ref().map(|l| l.thumbnail_url.clone()).filter(|u| !u.is_empty())
    }
}

#[derive(Deserialize, Clone)]
struct CfHash {
    value: String,
    algo: u32,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CfDependency {
    mod_id: u64,
    relation_type: u32,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CfFile {
    id: u64,
    mod_id: u64,
    #[serde(default)]
    display_name: String,
    file_name: String,
    #[serde(default)]
    release_type: u32,
    #[serde(default)]
    file_date: String,
    #[serde(default)]
    file_length: Option<u64>,
    #[serde(default)]
    download_count: f64,
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    hashes: Vec<CfHash>,
    #[serde(default)]
    dependencies: Vec<CfDependency>,
    #[serde(default)]
    is_server_pack: Option<bool>,
    #[serde(default)]
    file_fingerprint: Option<u64>,
}

impl CfFile {
    fn sha1(&self) -> Option<String> {
        self.hashes.iter().find(|h| h.algo == SHA1).map(|h| h.value.clone())
    }
    fn mc_versions(&self) -> Vec<String> {
        self.game_versions.iter().filter(|v| v.starts_with(|c: char| c.is_ascii_digit())).cloned().collect()
    }
    fn loaders(&self) -> Vec<String> {
        self.game_versions.iter().map(|v| v.to_lowercase()).filter(|v| LOADER_NAMES.contains(&v.as_str())).collect()
    }
    fn url(&self) -> Option<&str> {
        self.download_url.as_deref().filter(|u| !u.is_empty())
    }
}

// ---------- Requests ----------

async fn api_key() -> Result<String> {
    let key = settings::curseforge_key().await;
    if key.is_empty() {
        bail_tr!("curseforge.unavailable", "CurseForge isn't available in this build of ResinCraft.");
    }
    Ok(key)
}

async fn has_key() -> bool {
    !settings::curseforge_key().await.is_empty()
}

/// Raw response (data + pagination).
pub(crate) async fn cf_raw<T: DeserializeOwned>(path: &str, body: Option<Value>) -> Result<T> {
    let key = api_key().await?;
    let url = format!("{API}{path}");
    let req = match body {
        Some(b) => http().post(&url).json(&b),
        None => http().get(&url),
    };
    match send_json(req.header("x-api-key", key)).await {
        Err(e) if matches!(status_of(&e), Some(401 | 403)) => bail_tr!("curseforge.badKey", "CurseForge rejected this build's API key. Try again later, or update ResinCraft."),
        r => r,
    }
}

pub(crate) async fn cf<T: DeserializeOwned>(path: &str, body: Option<Value>) -> Result<T> {
    #[derive(Deserialize)]
    struct Data<T> {
        data: T,
    }
    Ok(cf_raw::<Data<T>>(path, body).await?.data)
}

fn fits(f: &CfFile, inst: &Instance, kind: ContentKind) -> bool {
    if f.is_server_pack == Some(true) || !f.game_versions.contains(&inst.mc_version) {
        return false;
    }
    if kind != ContentKind::Mod {
        return true;
    }
    let l = f.loaders();
    // Old files often carry no loader tag at all; trust the version match then.
    l.is_empty() || l.iter().any(|x| loaders_for(inst).contains(x))
}

// ---------- Search ----------

pub async fn search(q: &SearchQuery) -> Result<SearchResult> {
    let page_size = q.limit.unwrap_or(20).min(50);
    let mut params = vec![
        format!("gameId={MINECRAFT}"),
        format!("classId={}", class_of(q.kind)),
        format!("index={}", q.offset.unwrap_or(0)),
        format!("pageSize={page_size}"),
        "sortOrder=desc".to_string(),
    ];
    if !q.query.is_empty() {
        params.push(format!("searchFilter={}", urlencoding::encode(&q.query)));
    }
    // CurseForge has no "relevance" sort: with a search term its default order is relevance, without one use popularity.
    let sort = match q.sort.as_deref().unwrap_or("relevance") {
        "relevance" => (q.query.is_empty()).then_some(2),
        "downloads" => Some(6),
        "follows" => Some(2),
        "newest" => Some(11),
        "updated" => Some(3),
        _ => None,
    };
    if let Some(s) = sort {
        params.push(format!("sortField={s}"));
    }
    let gv = q.game_version.as_deref().filter(|v| !v.is_empty());
    if let Some(v) = gv {
        params.push(format!("gameVersion={}", urlencoding::encode(v)));
    }
    // The loader filter only works together with a game version.
    let loader = match q.loader {
        Some(LoaderType::Quilt) => loader_id(LoaderType::Fabric),
        Some(l) => loader_id(l),
        None => None,
    };
    if let (ContentKind::Mod, Some(l), Some(_)) = (q.kind, loader, gv) {
        params.push(format!("modLoaderType={l}"));
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Pagination {
        index: u64,
        total_count: u64,
    }
    #[derive(Deserialize)]
    struct Res {
        data: Vec<CfMod>,
        pagination: Pagination,
    }
    let res: Res = cf_raw(&format!("/mods/search?{}", params.join("&")), None).await?;
    Ok(SearchResult {
        // CurseForge refuses offsets past 10,000.
        total: res.pagination.total_count.min(10_000 - page_size as u64),
        offset: res.pagination.index,
        hits: res.data.iter().map(to_hit).collect(),
    })
}

fn to_hit(m: &CfMod) -> ProjectHit {
    ProjectHit {
        id: prefixed(m.id),
        slug: m.slug.clone(),
        title: m.name.clone(),
        description: m.summary.clone(),
        icon_url: m.icon(),
        author: m.authors.first().map(|a| a.name.clone()).unwrap_or_else(|| "Unknown".into()),
        downloads: m.download_count as u64,
        follows: m.thumbs_up_count as u64,
        categories: m.categories.iter().map(|c| c.name.clone()).collect(),
        kind: m.class_id.and_then(kind_of_class).unwrap_or(ContentKind::Mod),
        updated: m.date_modified.clone(),
        source: ContentSource::Curseforge,
        url: m.links.website_url.clone(),
    }
}

// ---------- Versions ----------

async fn files(mod_id: u64, inst: Option<&Instance>, kind: Option<ContentKind>) -> Result<Vec<CfFile>> {
    let mut params = vec!["pageSize=50".to_string()];
    if let Some(inst) = inst {
        params.push(format!("gameVersion={}", urlencoding::encode(&inst.mc_version)));
        if kind == Some(ContentKind::Mod) {
            let l = if inst.loader == LoaderType::Quilt { LoaderType::Fabric } else { inst.loader };
            // Quilt and NeoForge-1.20.1 instances also accept other loaders' files, so filter those locally instead.
            let lenient = inst.loader == LoaderType::Quilt || (inst.loader == LoaderType::Neoforge && inst.mc_version == "1.20.1");
            if let (Some(id), false) = (loader_id(l), lenient) {
                params.push(format!("modLoaderType={id}"));
            }
        }
    }
    let list: Vec<CfFile> = cf(&format!("/mods/{mod_id}/files?{}", params.join("&")), None).await?;
    let mut list: Vec<CfFile> = list
        .into_iter()
        .filter(|f| {
            f.is_server_pack != Some(true)
                && match (inst, kind) {
                    (Some(i), Some(k)) => fits(f, i, k),
                    _ => true,
                }
        })
        .collect();
    list.sort_by_key(|f| std::cmp::Reverse(parse_date_ms(&f.file_date)));
    Ok(list)
}

fn release_name(t: u32) -> &'static str {
    match t {
        2 => "beta",
        3 => "alpha",
        _ => "release",
    }
}

pub async fn versions(project_id: &str, instance_id: Option<&str>) -> Result<Vec<ProjectVersion>> {
    let inst = match instance_id {
        Some(id) => Some(instances::get(id).await?),
        None => None,
    };
    let m: CfMod = cf(&format!("/mods/{}", cf_id(project_id)), None).await?;
    let kind = m.class_id.and_then(kind_of_class);
    Ok(files(m.id, inst.as_ref(), kind)
        .await?
        .into_iter()
        .map(|f| ProjectVersion {
            id: f.id.to_string(),
            game_versions: f.mc_versions(),
            loaders: f.loaders(),
            kind: release_name(f.release_type).into(),
            published: f.file_date.clone(),
            downloads: f.download_count as u64,
            name: f.file_name,
            version_number: f.display_name,
        })
        .collect())
}

// ---------- Install ----------

/// Returned when an author has turned off third-party downloads for a file.
#[derive(Debug)]
pub struct BlockedDownloadError {
    pub pending: PendingDownload,
}

impl BlockedDownloadError {
    fn text(&self) -> Text {
        let p = &self.pending;
        tr!(
            "curseforge.blockedDownload",
            "{name}'s author only allows downloads from the CurseForge website. Download {file} there and put it in the {folder} folder.",
            name = p.title,
            file = p.file_name,
            folder = p.folder
        )
    }
}

impl fmt::Display for BlockedDownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.text().fmt(f)
    }
}
impl std::error::Error for BlockedDownloadError {}

fn file_url(m: &CfMod, f: &CfFile) -> String {
    format!("{}/files/{}", m.links.website_url, f.id)
}

async fn install_file(inst: &Instance, kind: ContentKind, m: &CfMod, f: &CfFile, idx: &mut ContentIndex) -> Result<()> {
    let folder = folder_of(inst, kind);
    let Some(url) = f.url() else {
        return Err(BlockedDownloadError {
            pending: PendingDownload { title: m.name.clone(), file_name: f.file_name.clone(), url: file_url(m, f), folder: kind.folder().to_string() },
        }
        .into());
    };
    remove_project_files(inst, kind, &prefixed(m.id), idx).await?;
    let sha1 = f.sha1();
    let item = DownloadItem::new(url, safe_join(&folder, safe_name(&f.file_name)?)?).hash(sha1.clone().map(Hash::sha1)).size(f.file_length);
    download_file(&item).await?;
    idx.insert(
        index_key(kind, &f.file_name),
        IndexEntry {
            project_id: prefixed(m.id),
            version_id: f.id.to_string(),
            version_number: f.display_name.clone(),
            title: m.name.clone(),
            icon_url: m.icon(),
            sha1: sha1.unwrap_or_default(),
        },
    );
    Ok(())
}

pub async fn install(instance_id: &str, project_id: &str, kind: ContentKind, version_id: Option<String>) -> Result<()> {
    let inst = instances::get(instance_id).await?;
    if kind == ContentKind::Mod && inst.loader == LoaderType::Vanilla {
        bail_tr!("curseforge.vanillaInstance", "{name} is a vanilla instance. Mods need a mod loader; create a modded instance first.", name = inst.name);
    }
    let root = cf_id(project_id);
    let file_id: Option<u64> = version_id.and_then(|v| v.parse().ok());
    Task::run(tr!("curseforge.installing", "Installing from CurseForge"), Some(inst.id.clone()), move |task| async move {
        let mut idx = load_index(&inst.id).await;
        let mut installed: HashSet<String> = idx.values().map(|e| e.project_id.clone()).collect();
        let mut blocked: Vec<PendingDownload> = Vec::new();
        // Depth-first over (mod, pinned file, is a dependency).
        let mut stack: Vec<(u64, Option<u64>, bool)> = vec![(root, file_id, false)];
        let result: Result<()> = async {
            while let Some((mod_id, fid, is_dep)) = stack.pop() {
                if is_dep && installed.contains(&prefixed(mod_id)) {
                    continue;
                }
                let m: CfMod = cf(&format!("/mods/{mod_id}"), None).await?;
                task.update(tr!("content.installingItem", "Installing {name}…", name = m.name), None);
                let f = match fid {
                    Some(fid) => Some(cf::<CfFile>(&format!("/mods/{mod_id}/files/{fid}"), None).await?),
                    None => files(mod_id, Some(&inst), Some(kind)).await?.into_iter().next(),
                };
                let Some(f) = f else {
                    if kind == ContentKind::Mod {
                        bail_tr!(
                            "curseforge.noFileWithLoader",
                            "{name} has no file for Minecraft {mc} with {loader}",
                            name = m.name,
                            mc = inst.mc_version,
                            loader = inst.loader.as_str()
                        );
                    }
                    bail_tr!("curseforge.noFile", "{name} has no file for Minecraft {mc}", name = m.name, mc = inst.mc_version);
                };
                if let Err(e) = install_file(&inst, kind, &m, &f, &mut idx).await {
                    // A blocked dependency shouldn't stop the main install; list it for the user instead.
                    match e.downcast::<BlockedDownloadError>() {
                        Ok(b) if is_dep => blocked.push(b.pending),
                        Ok(b) => return Err(b.text().into()),
                        Err(e) => return Err(e),
                    }
                }
                installed.insert(prefixed(m.id));
                let deps: Vec<_> = f
                    .dependencies
                    .iter()
                    .filter(|d| d.relation_type == REQUIRED_DEPENDENCY && !installed.contains(&prefixed(d.mod_id)))
                    .map(|d| (d.mod_id, None, true))
                    .collect();
                stack.extend(deps.into_iter().rev());
            }
            Ok(())
        }
        .await;
        save_index(&inst.id, &idx).await?;
        if !blocked.is_empty() {
            add_pending(&inst.id, blocked).await?;
        }
        result
    })
    .await
}

async fn add_pending(instance_id: &str, list: Vec<PendingDownload>) -> Result<()> {
    let inst = instances::get(instance_id).await?;
    let mut merged = inst.pending_downloads.unwrap_or_default();
    for p in list {
        if !merged.iter().any(|e| e.file_name == p.file_name) {
            merged.push(p);
        }
    }
    instances::update(instance_id, json!({ "pendingDownloads": merged })).await?;
    Ok(())
}

// ---------- Updates & identification ----------

/// Fills in `update` for CurseForge-tracked mods in `mods`.
pub async fn check_updates(inst: &Instance, mods: &mut [ContentItem]) -> Result<()> {
    let idx = load_index(&inst.id).await;
    let tracked: Vec<usize> = mods
        .iter()
        .enumerate()
        .filter(|(_, m)| m.project_id.as_deref().is_some_and(|p| p.starts_with("cf:")) && idx.contains_key(&index_key(ContentKind::Mod, &m.filename)))
        .map(|(i, _)| i)
        .collect();
    for chunk in tracked.chunks(6) {
        let results = futures::future::join_all(chunk.iter().map(|&i| {
            let pid = cf_id(mods[i].project_id.as_deref().unwrap_or_default());
            async move { files(pid, Some(inst), Some(ContentKind::Mod)).await.map(|l| (i, l.into_iter().next())) }
        }))
        .await;
        for r in results {
            let (i, latest) = r?;
            if let Some(latest) = latest {
                if mods[i].version_id.as_deref() != Some(latest.id.to_string().as_str()) {
                    mods[i].update = Some(UpdateRef { version_id: latest.id.to_string(), version_number: latest.display_name });
                }
            }
        }
    }
    Ok(())
}

/// CurseForge identifies files by a MurmurHash2 (seed 1) of the file with all whitespace bytes
/// (tab, LF, CR, space) removed.
pub fn fingerprint(data: &[u8]) -> u32 {
    let buf: Vec<u8> = data.iter().copied().filter(|&b| b != 9 && b != 10 && b != 13 && b != 32).collect();
    const M: u32 = 0x5bd1_e995;
    let mut len = buf.len();
    let mut h: u32 = 1 ^ (len as u32);
    let mut i = 0;
    while len >= 4 {
        let mut k = u32::from_le_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]);
        k = k.wrapping_mul(M);
        k ^= k >> 24;
        k = k.wrapping_mul(M);
        h = h.wrapping_mul(M) ^ k;
        i += 4;
        len -= 4;
    }
    if len >= 3 {
        h ^= (buf[i + 2] as u32) << 16;
    }
    if len >= 2 {
        h ^= (buf[i + 1] as u32) << 8;
    }
    if len >= 1 {
        h ^= buf[i] as u32;
        h = h.wrapping_mul(M);
    }
    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^= h >> 15;
    h
}

/// Fingerprints files without blocking the async runtime.
async fn fingerprints(files: Vec<PathBuf>) -> Result<Vec<(u32, PathBuf)>> {
    tokio::task::spawn_blocking(move || files.into_iter().map(|p| Ok((fingerprint(&std::fs::read(&p)?), p))).collect::<Result<Vec<_>>>()).await?
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FingerprintMatch {
    id: u64,
    file: CfFile,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FingerprintRes {
    #[serde(default)]
    exact_matches: Vec<FingerprintMatch>,
}

/// Recognises hand-added files Modrinth didn't know, by asking CurseForge about their fingerprints. Silently skips without a key.
pub async fn identify(inst: &Instance, kind: ContentKind, filenames: &[String]) -> Result<()> {
    if filenames.is_empty() || !has_key().await {
        return Ok(());
    }
    let folder = folder_of(inst, kind);
    let prints = fingerprints(filenames.iter().map(|n| folder.join(n)).collect()).await?;
    let res: FingerprintRes = cf(&format!("/fingerprints/{MINECRAFT}"), Some(json!({ "fingerprints": prints.iter().map(|p| p.0).collect::<Vec<_>>() }))).await?;
    if res.exact_matches.is_empty() {
        return Ok(());
    }
    let ids: Vec<u64> = res.exact_matches.iter().map(|x| x.id).collect::<HashSet<_>>().into_iter().collect();
    let mods: Vec<CfMod> = cf("/mods", Some(json!({ "modIds": ids }))).await?;
    let mut idx = load_index(&inst.id).await;
    let names: Vec<String> = prints.iter().map(|(_, p)| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).collect();
    for FingerprintMatch { id, file } in res.exact_matches {
        let Some(m) = mods.iter().find(|x| x.id == id) else { continue };
        let name = filenames
            .iter()
            .find(|n| n.strip_suffix(".disabled").unwrap_or(n) == file.file_name)
            .or_else(|| names.iter().find(|n| n.starts_with(&file.file_name)));
        let Some(name) = name else { continue };
        idx.insert(
            index_key(kind, name),
            IndexEntry {
                project_id: prefixed(id),
                version_id: file.id.to_string(),
                version_number: file.display_name.clone(),
                title: m.name.clone(),
                icon_url: m.icon(),
                sha1: file.sha1().unwrap_or_default(),
            },
        );
    }
    save_index(&inst.id, &idx).await
}

/// Finds the CurseForge project/file for each local file by fingerprint (used by modpack export).
/// Returns an empty map without an API key.
pub async fn lookup_files(files: Vec<PathBuf>) -> Result<HashMap<PathBuf, (u64, u64)>> {
    let mut out = HashMap::new();
    if files.is_empty() || !has_key().await {
        return Ok(out);
    }
    let by_print: HashMap<u32, PathBuf> = fingerprints(files).await?.into_iter().collect();
    let res: FingerprintRes = cf(&format!("/fingerprints/{MINECRAFT}"), Some(json!({ "fingerprints": by_print.keys().collect::<Vec<_>>() }))).await?;
    for FingerprintMatch { id, file } in res.exact_matches {
        if let Some(path) = file.file_fingerprint.and_then(|fp| by_print.get(&(fp as u32))) {
            out.insert(path.clone(), (id, file.id));
        }
    }
    Ok(out)
}

// ---------- Modpacks ----------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfLoader {
    id: String,
    #[serde(default)]
    primary: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMinecraft {
    version: String,
    #[serde(default)]
    mod_loaders: Vec<CfLoader>,
}

#[derive(Deserialize)]
struct CfManifestFile {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
    #[serde(default)]
    required: Option<bool>,
}

#[derive(Deserialize)]
struct CfManifest {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    minecraft: CfMinecraft,
    #[serde(default)]
    files: Vec<CfManifestFile>,
    #[serde(default)]
    overrides: Option<String>,
}

fn parse_loader(id: Option<&str>) -> Result<(LoaderType, Option<String>)> {
    let Some(id) = id else { return Ok((LoaderType::Vanilla, None)) };
    let mut parts = id.split('-');
    let name = parts.next().unwrap_or_default();
    let rest = parts.collect::<Vec<_>>().join("-");
    let loader = match name.to_lowercase().as_str() {
        "forge" => LoaderType::Forge,
        "neoforge" => LoaderType::Neoforge,
        "fabric" => LoaderType::Fabric,
        "quilt" => LoaderType::Quilt,
        _ => bail_tr!("modpack.unsupportedLoader", "This modpack needs {name}, which ResinCraft doesn't support yet.", name = name),
    };
    Ok((loader, (!rest.is_empty()).then_some(rest)))
}

/// Where a CurseForge pack came from, stored on the new instance.
#[derive(Default, Clone)]
pub struct CfPackMeta {
    pub name: Option<String>,
    pub icon_url: Option<String>,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
}

pub async fn install_modpack(project_id: &str, version_id: Option<String>) -> Result<Instance> {
    let m: CfMod = cf(&format!("/mods/{}", cf_id(project_id)), None).await?;
    let pack = match version_id {
        Some(v) => Some(cf::<CfFile>(&format!("/mods/{}/files/{v}", m.id), None).await?),
        None => files(m.id, None, None).await?.into_iter().next(),
    };
    let pack = pack.ok_or_else(|| err_tr!("curseforge.noDownloadableVersion", "{name} has no downloadable version", name = m.name))?;
    let Some(url) = pack.url() else {
        bail_tr!("curseforge.packBlocked", "{name}'s author only allows downloads from the CurseForge website.", name = m.name)
    };

    let archive = safe_join(&paths::cache().join("modpacks"), safe_name(&pack.file_name)?)?;
    let item = DownloadItem::new(url, &archive).size(pack.file_length);
    Task::run(tr!("modpack.downloading", "Downloading {name}", name = m.name), None, |_| async move { download_file(&item).await }).await?;
    let meta = CfPackMeta { name: Some(m.name.clone()), icon_url: m.icon(), project_id: Some(prefixed(m.id)), version_id: Some(pack.id.to_string()) };
    let res = install_cf_zip(&archive, meta).await;
    let _ = tokio::fs::remove_file(&archive).await;
    res
}

/// Installs a CurseForge modpack .zip (downloaded or imported from disk) as a new instance.
pub async fn install_cf_zip(archive: &Path, meta: CfPackMeta) -> Result<Instance> {
    let a = archive.to_path_buf();
    let raw = tokio::task::spawn_blocking(move || -> Result<Option<String>> {
        let mut zip = open_zip(&a).map_err(|_| err_tr!("curseforge.notModpack", "This file isn't a CurseForge modpack (it has no manifest.json)"))?;
        read_zip_text(&mut zip, "manifest.json")
    })
    .await??;
    let raw = raw.filter(|r| !r.is_empty()).ok_or_else(|| err_tr!("curseforge.notModpack", "This file isn't a CurseForge modpack (it has no manifest.json)"))?;
    let manifest: CfManifest = serde_json::from_str(&raw)?;
    let name = meta.name.clone().or(manifest.name.clone()).unwrap_or_else(|| "Imported modpack".into());
    let primary = manifest.minecraft.mod_loaders.iter().find(|l| l.primary == Some(true)).or(manifest.minecraft.mod_loaders.first());
    let (loader, loader_version) = parse_loader(primary.map(|l| l.id.as_str()))?;
    // Fail before creating anything if we can't resolve the mod list.
    if !manifest.files.is_empty() && !has_key().await {
        bail_tr!("curseforge.importUnavailable", "Importing CurseForge modpacks isn't available in this build of ResinCraft.");
    }

    let archive = archive.to_path_buf();
    Task::run(tr!("modpack.installing", "Installing {name}", name = name), None, move |task| async move {
        let modpack = ModpackRef { name: name.clone(), project_id: meta.project_id.clone(), version_id: meta.version_id.clone().or(manifest.version.clone()) };
        let icon = meta.icon_url.clone();
        let new = NewInstance { name: name.clone(), mc_version: manifest.minecraft.version.clone(), loader, loader_version, color: None, group: None };
        let inst = instances::create(new, move |i| {
            i.icon_url = icon;
            i.modpack = Some(modpack);
        })
        .await?;
        let result: Result<Instance> = async {
            let pending = fill_pack(&inst, &manifest, &archive, &task).await?;
            if !pending.is_empty() {
                instances::update(&inst.id, json!({ "pendingDownloads": pending })).await?;
            }
            instances::get(&inst.id).await
        }
        .await;
        if result.is_err() {
            discard_instance(&inst.id).await;
        }
        result
    })
    .await
}

async fn fill_pack(inst: &Instance, manifest: &CfManifest, archive: &Path, task: &Arc<Task>) -> Result<Vec<PendingDownload>> {
    let dir = paths::game_dir(&inst.id);
    let wanted: Vec<&CfManifestFile> = manifest.files.iter().filter(|f| f.required != Some(false)).collect();
    task.update(tr!("curseforge.lookingUpFiles", "Looking up modpack files…"), None);
    let mut file_list: Vec<CfFile> = Vec::new();
    let mut mod_list: Vec<CfMod> = Vec::new();
    for chunk in wanted.chunks(500) {
        file_list.extend(cf::<Vec<CfFile>>("/mods/files", Some(json!({ "fileIds": chunk.iter().map(|f| f.file_id).collect::<Vec<_>>() }))).await?);
        mod_list.extend(cf::<Vec<CfMod>>("/mods", Some(json!({ "modIds": chunk.iter().map(|f| f.project_id).collect::<Vec<_>>() }))).await?);
    }

    let mut idx = load_index(&inst.id).await;
    let mut pending = Vec::new();
    let mut items = Vec::new();
    for f in &file_list {
        let m = mod_list.iter().find(|m| m.id == f.mod_id);
        let class = m.and_then(|m| m.class_id).unwrap_or(class_of(ContentKind::Mod));
        let folder = folder_of_class(class);
        let Some(url) = f.url() else {
            pending.push(PendingDownload {
                title: m.map(|m| m.name.clone()).unwrap_or_else(|| f.display_name.clone()),
                file_name: f.file_name.clone(),
                url: m.map(|m| file_url(m, f)).unwrap_or_else(|| "https://www.curseforge.com/".into()),
                folder: folder.into(),
            });
            continue;
        };
        let sha1 = f.sha1();
        items.push(DownloadItem::new(url, safe_join(&dir.join(folder), safe_name(&f.file_name)?)?).hash(sha1.clone().map(Hash::sha1)).size(f.file_length));
        idx.insert(
            index_key(kind_of_class(class).unwrap_or(ContentKind::Mod), &f.file_name),
            IndexEntry {
                project_id: prefixed(f.mod_id),
                version_id: f.id.to_string(),
                version_number: f.display_name.clone(),
                title: m.map(|m| m.name.clone()).unwrap_or_else(|| f.display_name.clone()),
                icon_url: m.and_then(|m| m.icon()),
                sha1: sha1.unwrap_or_default(),
            },
        );
    }
    download_all(items, 8, task.counter(tr!("modpack.downloadingFiles", "Downloading modpack files"))).await?;
    save_index(&inst.id, &idx).await?;

    task.update(tr!("modpack.applyingSettings", "Applying modpack settings…"), None);
    let mut prefix = manifest.overrides.clone().unwrap_or_else(|| "overrides".into());
    if !prefix.ends_with('/') {
        prefix.push('/');
    }
    let (archive, dir2) = (archive.to_path_buf(), dir.clone());
    tokio::task::spawn_blocking(move || extract_overrides(&archive, &[prefix], &dir2)).await??;
    Ok(pending)
}
