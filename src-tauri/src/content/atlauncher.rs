//! ATLauncher's public pack API. The pack list has no download counts or authors, so search is done
//! locally over the ~150 public packs. ATLauncher only publishes modpacks.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use super::details;
use super::dupes::dedupe_pack;
use super::index::{discard_instance, encode_uri, extract_where, iso_from_secs, lenient_u64, open_zip, safe_name, InstanceGuard};
use super::packswitch::{Layer, PackTarget, TargetFile};
use crate::core::events::Task;
use crate::core::fsutil::safe_join;
use crate::core::instances;
use crate::core::net::{download_all, fetch_json, DownloadItem, Hash};
use crate::core::paths;
use crate::core::types::{
    ContentKind, ContentSource, Instance, LoaderType, ModpackRef, NewInstance, PendingDownload, ProjectDetails, ProjectHit, ProjectLink, ProjectVersion,
    SearchQuery, SearchResult, VersionInfo,
};
use crate::{bail_tr, err_tr, tr};

const API: &str = "https://api.atlauncher.com/v1";
const CDN: &str = "https://download.nodecdn.net/containers/atl";

fn atl_name(project_id: &str) -> &str {
    project_id.strip_prefix("atl:").unwrap_or(project_id)
}

fn enc(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct AtlVersionSummary {
    version: String,
    minecraft: String,
    published: i64,
}

#[derive(Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
struct AtlPack {
    id: i64,
    name: String,
    safe_name: String,
    description: Option<String>,
    versions: Vec<AtlVersionSummary>,
    #[serde(rename = "discordInviteURL")]
    discord_invite_url: Option<String>,
    #[serde(rename = "supportURL")]
    support_url: Option<String>,
    #[serde(rename = "websiteURL")]
    website_url: Option<String>,
}

#[derive(Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
struct AtlMod {
    name: String,
    url: String,
    file: String,
    /// "server" = ATLauncher's CDN, "direct" = absolute URL, "browser" = the user must fetch it.
    download: String,
    md5: Option<String>,
    #[serde(deserialize_with = "lenient_u64")]
    filesize: Option<u64>,
    #[serde(rename = "type")]
    kind: String,
    client: Option<bool>,
    optional: Option<bool>,
    selected: Option<bool>,
    website: Option<String>,
    extract_to: Option<String>,
    extract_folder: Option<String>,
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct AtlConfigs {
    #[serde(deserialize_with = "lenient_u64")]
    filesize: Option<u64>,
    sha1: Option<String>,
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct AtlLoaderMeta {
    version: Option<String>,
    loader: Option<String>,
    latest: Option<bool>,
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct AtlLoader {
    #[serde(rename = "type")]
    kind: String,
    metadata: AtlLoaderMeta,
}

#[derive(Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
struct AtlConfig {
    version: String,
    minecraft: String,
    #[serde(deserialize_with = "lenient_u64")]
    memory: Option<u64>,
    no_configs: Option<bool>,
    configs: Option<AtlConfigs>,
    loader: Option<AtlLoader>,
    libraries: Option<Vec<Value>>,
    mods: Vec<AtlMod>,
}

/// Where each ATLauncher file type goes inside the game directory.
fn folder_for(kind: &str, mc: &str) -> Option<String> {
    Some(match kind {
        "mods" => "mods".into(),
        "dependency" => format!("mods/{mc}"),
        "resourcepack" => "resourcepacks".into(),
        "shaderpack" => "shaderpacks".into(),
        "texturepack" => "texturepacks".into(),
        _ => return None,
    })
}

// The full list is ~700 KB; fetch it once per session.
static LIST: tokio::sync::Mutex<Option<Arc<Vec<AtlPack>>>> = tokio::sync::Mutex::const_new(None);

fn sorted(mut p: AtlPack) -> AtlPack {
    p.versions.sort_by_key(|v| std::cmp::Reverse(v.published));
    p
}

async fn pack_list() -> Result<Arc<Vec<AtlPack>>> {
    let mut guard = LIST.lock().await;
    if let Some(l) = guard.as_ref() {
        return Ok(l.clone());
    }
    #[derive(Deserialize)]
    struct Res {
        data: Vec<AtlPack>,
    }
    let res: Res = fetch_json(&format!("{API}/packs/full/public")).await?;
    let list = Arc::new(res.data.into_iter().filter(|p| !p.versions.is_empty()).map(sorted).collect::<Vec<_>>());
    *guard = Some(list.clone());
    Ok(list)
}

async fn pack(name: &str) -> Result<AtlPack> {
    if let Some(p) = pack_list().await?.iter().find(|x| x.safe_name == name) {
        return Ok(p.clone());
    }
    // Not in the public list (semi-public pack, or the list is stale): ask for it directly.
    #[derive(Deserialize)]
    struct Res {
        data: AtlPack,
    }
    let one: Res = fetch_json(&format!("{API}/pack/{}", enc(name))).await?;
    Ok(sorted(one.data))
}

fn icon_of(p: &AtlPack) -> String {
    format!("https://cdn.atlcdn.net/images/packs/{}.png", p.safe_name.to_lowercase())
}

fn to_hit(p: &AtlPack) -> ProjectHit {
    let latest = p.versions.first().cloned().unwrap_or_default();
    ProjectHit {
        id: format!("atl:{}", p.safe_name),
        slug: p.safe_name.clone(),
        title: p.name.clone(),
        description: p.description.clone().unwrap_or_default(),
        icon_url: Some(icon_of(p)),
        author: "ATLauncher".into(),
        downloads: 0,
        follows: 0,
        categories: vec![format!("MC {}", latest.minecraft)],
        kind: ContentKind::Modpack,
        updated: iso_from_secs(latest.published),
        source: ContentSource::Atlauncher,
        url: format!("https://atlauncher.com/pack/{}", p.safe_name),
    }
}

pub async fn search(q: &SearchQuery) -> Result<SearchResult> {
    if q.kind != ContentKind::Modpack {
        return Ok(SearchResult { hits: vec![], total: 0, offset: 0 });
    }
    let query = q.query.trim().to_lowercase();
    let words: Vec<&str> = query.split_whitespace().collect();
    let all = pack_list().await?;
    let mut list: Vec<&AtlPack> = all
        .iter()
        .filter(|p| {
            let text = format!("{} {}", p.name, p.description.as_deref().unwrap_or_default()).to_lowercase();
            words.iter().all(|w| text.contains(w))
        })
        .collect();
    if let Some(gv) = q.game_version.as_deref().filter(|v| !v.is_empty()) {
        list.retain(|p| p.versions.iter().any(|v| v.minecraft == gv));
    }

    let latest = |p: &AtlPack| p.versions.first().map(|v| v.published).unwrap_or(0);
    let sort = q.sort.as_deref().unwrap_or("relevance");
    if sort == "newest" {
        list.sort_by_key(|p| std::cmp::Reverse(p.id));
    } else if !words.is_empty() && sort == "relevance" {
        // Title matches first, then the most recently updated.
        let in_title = |p: &AtlPack| words.iter().all(|w| p.name.to_lowercase().contains(w));
        list.sort_by(|a, b| in_title(b).cmp(&in_title(a)).then_with(|| latest(b).cmp(&latest(a))));
    } else {
        // No download / follow counts to sort by.
        list.sort_by_key(|p| std::cmp::Reverse(latest(p)));
    }

    let offset = q.offset.unwrap_or(0) as usize;
    let limit = q.limit.unwrap_or(20) as usize;
    Ok(SearchResult { hits: list.iter().skip(offset).take(limit).map(|p| to_hit(p)).collect(), total: list.len() as u64, offset: offset as u64 })
}

pub async fn versions(project_id: &str) -> Result<Vec<ProjectVersion>> {
    let p = pack(atl_name(project_id)).await?;
    Ok(p.versions
        .iter()
        .map(|v| ProjectVersion {
            id: v.version.clone(),
            name: v.version.clone(),
            version_number: v.version.clone(),
            game_versions: vec![v.minecraft.clone()],
            // The loader is only in each version's Configs.json; not worth one request per row.
            loaders: vec![],
            kind: "release".into(),
            published: iso_from_secs(v.published),
            downloads: 0,
        })
        .collect())
}

pub async fn details(project_id: &str) -> Result<ProjectDetails> {
    let p = pack(atl_name(project_id)).await?;
    Ok(ProjectDetails {
        // Plain text, not Markdown.
        body: details::markdown(p.description.as_deref().unwrap_or_default(), true),
        gallery: Vec::new(),
        links: ProjectLink::list([("website", p.website_url.clone()), ("issues", p.support_url.clone()), ("discord", p.discord_invite_url.clone())]),
        license: None,
        changelogs: true,
        dependencies: false,
        hit: to_hit(&p),
    })
}

pub async fn version_info(project_id: &str, version_id: &str) -> Result<VersionInfo> {
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct Version {
        changelog: Option<String>,
    }
    #[derive(Deserialize)]
    struct Res {
        data: Version,
    }
    let v: Res = fetch_json(&format!("{API}/pack/{}/{}", enc(atl_name(project_id)), enc(version_id))).await?;
    let text = v.data.changelog.filter(|c| !c.trim().is_empty());
    Ok(VersionInfo { changelog: text.map(|c| details::markdown(&c, true)), dependencies: Vec::new() })
}

/// Works out the loader, refusing (before anything is created) packs ResinCraft can't run.
fn loader_of(name: &str, cfg: &AtlConfig) -> Result<(LoaderType, Option<String>)> {
    let mc = &cfg.minecraft;
    let Some(l) = cfg.loader.as_ref().filter(|l| !l.kind.is_empty()) else {
        // Old packs install Forge as a jar mod / with custom libraries instead of declaring a loader.
        let legacy = cfg.libraries.as_ref().is_some_and(|l| !l.is_empty())
            || cfg.mods.iter().any(|m| m.client != Some(false) && ["forge", "jar", "coremods", "mods"].contains(&m.kind.as_str()));
        if legacy {
            bail_tr!(
                "atlauncher.legacyFormat",
                "{name} {version} uses ATLauncher's legacy install format (Minecraft {mc}), which ResinCraft doesn't support.",
                name = name,
                version = cfg.version,
                mc = mc
            );
        }
        return Ok((LoaderType::Vanilla, None));
    };
    let md = &l.metadata;
    match l.kind.to_lowercase().as_str() {
        "forge" if crate::game::loaders::forge_too_old(mc) => Err(crate::game::loaders::forge_floor_error(mc)),
        "forge" => Ok((LoaderType::Forge, if md.latest == Some(true) { None } else { md.version.clone() })),
        "neoforge" => Ok((LoaderType::Neoforge, if md.latest == Some(true) { None } else { md.version.clone() })),
        "fabric" => Ok((LoaderType::Fabric, md.loader.clone())),
        "quilt" => Ok((LoaderType::Quilt, md.loader.clone())),
        _ => bail_tr!("atlauncher.unsupportedLoader", "{name} needs the {loader} loader, which ResinCraft doesn't support yet.", name = name, loader = l.kind),
    }
}

/// Unpacks the entries under `folder` ("" = everything) of a zip into `dest` (blocking).
fn unzip(file: &std::path::Path, folder: &str, dest: &std::path::Path) -> Result<()> {
    let mut zip = open_zip(file)?;
    extract_where(&mut zip, dest, |name| name.strip_prefix(folder).map(str::to_string))
}

struct Extract {
    zip: PathBuf,
    to: String,
    folder: String,
}

pub async fn install_modpack(project_id: &str, version_id: Option<String>) -> Result<Instance> {
    let p = pack(atl_name(project_id)).await?;
    let summary = match &version_id {
        Some(v) => p.versions.iter().find(|x| &x.version == v),
        None => p.versions.first(),
    };
    let summary = summary.ok_or_else(|| {
        err_tr!("atlauncher.noSuchVersion", "{name} has no version {version} to install", name = p.name, version = version_id.clone().unwrap_or_default())
    })?;
    let base = format!("{CDN}/packs/{}/versions/{}", enc(&p.safe_name), enc(&summary.version));
    let cfg: AtlConfig = fetch_json(&format!("{base}/Configs.json")).await?;
    let (loader, loader_version) = loader_of(&p.name, &cfg)?;

    // Optional mods are pre-ticked in ATLauncher when "selected"; there's no picker here, so take that default.
    let mods: Vec<AtlMod> = cfg.mods.iter().filter(|m| m.client != Some(false) && (m.optional != Some(true) || m.selected == Some(true))).cloned().collect();
    if let Some(odd) = mods.iter().find(|m| m.kind != "extract" && folder_for(&m.kind, &cfg.minecraft).is_none()) {
        bail_tr!(
            "atlauncher.unsupportedFileType",
            "{name} {version} installs \"{file}\" as a {kind} file, which ResinCraft can't install.",
            name = p.name,
            version = cfg.version,
            file = odd.name,
            kind = odd.kind
        );
    }
    if let Some(m) = mods.iter().find(|m| m.kind == "extract" && m.download == "browser") {
        bail_tr!(
            "atlauncher.manualExtract",
            "{name} needs \"{file}\" downloaded by hand and unpacked, which ResinCraft can't do for you.",
            name = p.name,
            file = m.name
        );
    }

    Task::run(tr!("modpack.installing", "Installing {name}", name = p.name), None, move |task| async move {
        let modpack = ModpackRef { name: p.name.clone(), project_id: Some(format!("atl:{}", p.safe_name)), version_id: Some(cfg.version.clone()) };
        let icon = Some(icon_of(&p));
        let memory = cfg.memory.filter(|m| *m > 0).map(|m| m as u32);
        let new = NewInstance { name: p.name.clone(), mc_version: cfg.minecraft.clone(), loader, loader_version, color: None, group: None };
        let inst = instances::create(new, move |i| {
            i.icon_url = icon;
            i.memory_mb = memory;
            i.modpack = Some(modpack);
        })
        .await?;
        let guard = InstanceGuard::new(&inst.id);
        let tmp = paths::instance_dir(&inst.id).join(".atl-download");
        let result: Result<Instance> = async {
            let dir = paths::game_dir(&inst.id);
            let mut items = Vec::new();
            let mut pending = Vec::new();
            let mut extracts = Vec::new();
            for (i, m) in mods.iter().enumerate() {
                if m.download == "browser" {
                    let url = [&m.url, m.website.as_deref().unwrap_or_default()].into_iter().find(|u| !u.is_empty()).map(|u| u.to_string());
                    pending.push(PendingDownload {
                        title: m.name.clone(),
                        file_name: m.file.clone(),
                        url: url.unwrap_or_else(|| format!("https://atlauncher.com/pack/{}", p.safe_name)),
                        folder: folder_for(&m.kind, &cfg.minecraft).unwrap_or_default(),
                        ..Default::default()
                    });
                    continue;
                }
                let url = if m.download == "server" { format!("{CDN}/{}", encode_uri(&m.url)) } else { m.url.clone() };
                let extract = m.kind == "extract";
                let dest = if extract {
                    tmp.join(format!("{i}-{}", safe_name(&m.file)?))
                } else {
                    safe_join(&dir, &format!("{}/{}", folder_for(&m.kind, &cfg.minecraft).unwrap_or_default(), m.file))?
                };
                let hash = m.md5.as_ref().filter(|s| !s.is_empty()).map(Hash::md5);
                items.push(DownloadItem::new(url, &dest).hash(hash).size(m.filesize.filter(|s| *s > 0)));
                if extract {
                    let to = m.extract_to.clone().filter(|t| !t.is_empty() && t != "root").unwrap_or_default();
                    let mut folder = m.extract_folder.clone().unwrap_or_default().trim_start_matches('/').to_string();
                    if !folder.is_empty() && !folder.ends_with('/') {
                        folder.push('/');
                    }
                    extracts.push(Extract { zip: dest, to, folder });
                }
            }
            if cfg.no_configs != Some(true) {
                let zip = tmp.join("Configs.zip");
                let c = cfg.configs.clone().unwrap_or_default();
                let hash = c.sha1.filter(|s| !s.is_empty()).map(Hash::sha1);
                items.push(DownloadItem::new(format!("{base}/Configs.zip"), &zip).hash(hash).size(c.filesize));
                extracts.push(Extract { zip, to: String::new(), folder: String::new() });
            }
            download_all(items, 8, task.counter(tr!("modpack.downloadingFiles", "Downloading modpack files"))).await?;

            task.update(tr!("modpack.applyingSettings", "Applying modpack settings…"), None);
            let dir2 = dir.clone();
            tokio::task::spawn_blocking(move || -> Result<()> {
                for e in extracts {
                    unzip(&e.zip, &e.folder, &safe_join(&dir2, &e.to)?)?;
                }
                Ok(())
            })
            .await??;
            let _ = tokio::fs::remove_dir_all(&tmp).await;
            // Configs.zip may hold another copy of a listed mod.
            let listed: Vec<String> = mods.iter().filter(|m| m.download != "browser" && m.kind == "mods").map(|m| m.file.clone()).collect();
            dedupe_pack(&inst, &task, &listed).await;
            if !pending.is_empty() {
                instances::update(&inst.id, json!({ "pendingDownloads": pending })).await?;
            }
            let inst = instances::get(&inst.id).await?;
            super::packswitch::record_install(&inst).await;
            Ok(inst)
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

/// Version `version_id` of ATLauncher pack `project_id`, for switching an instance to it (see packswitch.rs).
/// Downloads the archives the version unpacks (Configs.zip and "extract" files) into `work`.
pub(crate) async fn pack_target(project_id: &str, version_id: &str, work: &Path, task: &Arc<Task>) -> Result<PackTarget> {
    let p = pack(atl_name(project_id)).await?;
    let base = format!("{CDN}/packs/{}/versions/{}", enc(&p.safe_name), enc(version_id));
    let cfg: AtlConfig = fetch_json(&format!("{base}/Configs.json")).await?;
    let (loader, loader_version) = loader_of(&p.name, &cfg)?;
    let mods: Vec<&AtlMod> = cfg.mods.iter().filter(|m| m.client != Some(false) && (m.optional != Some(true) || m.selected == Some(true))).collect();
    let (mut files, mut pending, mut layers, mut archives) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (i, m) in mods.iter().enumerate() {
        let url = if m.download == "server" { format!("{CDN}/{}", encode_uri(&m.url)) } else { m.url.clone() };
        let hash = m.md5.as_ref().filter(|s| !s.is_empty()).map(Hash::md5);
        let size = m.filesize.filter(|s| *s > 0);
        if m.kind == "extract" {
            if m.download == "browser" {
                bail_tr!("atlauncher.manualExtract", "{name} needs \"{file}\" downloaded by hand and unpacked, which ResinCraft can't do for you.", name = p.name.clone(), file = m.name.clone());
            }
            let zip = work.join(format!("{i}-{}", safe_name(&m.file)?));
            archives.push(DownloadItem::new(url, &zip).hash(hash).size(size));
            let mut prefix = m.extract_folder.clone().unwrap_or_default().trim_start_matches('/').to_string();
            if !prefix.is_empty() && !prefix.ends_with('/') {
                prefix.push('/');
            }
            let to = m.extract_to.clone().filter(|t| !t.is_empty() && t != "root").unwrap_or_default();
            layers.push(Layer { zip, prefix, to, skip_bin: false });
            continue;
        }
        let Some(folder) = folder_for(&m.kind, &cfg.minecraft) else {
            bail_tr!(
                "atlauncher.unsupportedFileType",
                "{name} {version} installs \"{file}\" as a {kind} file, which ResinCraft can't install.",
                name = p.name.clone(),
                version = cfg.version.clone(),
                file = m.name.clone(),
                kind = m.kind.clone()
            );
        };
        if m.download == "browser" {
            let page = [&m.url, m.website.as_deref().unwrap_or_default()].into_iter().find(|u| !u.is_empty()).map(|u| u.to_string());
            pending.push(PendingDownload {
                title: m.name.clone(),
                file_name: m.file.clone(),
                url: page.unwrap_or_else(|| format!("https://atlauncher.com/pack/{}", p.safe_name)),
                folder,
                ..Default::default()
            });
            continue;
        }
        files.push(TargetFile { path: format!("{folder}/{}", m.file), url, hash, size });
    }
    if cfg.no_configs != Some(true) {
        let zip = work.join("Configs.zip");
        let c = cfg.configs.clone().unwrap_or_default();
        archives.push(DownloadItem::new(format!("{base}/Configs.zip"), &zip).hash(c.sha1.filter(|s| !s.is_empty()).map(Hash::sha1)).size(c.filesize));
        layers.push(Layer::new(zip, ""));
    }
    download_all(archives, 4, task.counter(tr!("modpack.downloadingFiles", "Downloading modpack files"))).await?;
    Ok(PackTarget {
        version_id: cfg.version.clone(),
        version_name: cfg.version.clone(),
        mc_version: cfg.minecraft.clone(),
        loader,
        loader_version,
        files,
        layers,
        pending,
        index: vec![],
    })
}

/// Drops the cached pack list, so the update check sees versions published since it was fetched.
pub(crate) async fn forget_cache() {
    *LIST.lock().await = None;
}

/// The mods in an ATLauncher pack version, by the names its Configs.json gives them.
pub async fn pack_mods(project_id: &str, version_id: &str) -> Result<Vec<crate::core::types::PackMod>> {
    use crate::core::types::PackMod;
    let p = pack(atl_name(project_id)).await?;
    let base = format!("{CDN}/packs/{}/versions/{}", enc(&p.safe_name), enc(version_id));
    let cfg: AtlConfig = fetch_json(&format!("{base}/Configs.json")).await?;
    Ok(cfg
        .mods
        .iter()
        .filter(|m| m.client != Some(false) && matches!(m.kind.as_str(), "mods" | "forge" | "coremods" | "jar" | "dependency"))
        .map(|m| PackMod { title: m.name.clone(), id: None, source: None, icon_url: None, file: Some(m.file.clone()).filter(|f| !f.is_empty()) })
        .collect())
}
