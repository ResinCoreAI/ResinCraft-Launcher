//! Creating a server: plain server software, a modpack (Modrinth .mrpack with server-side files only,
//! CurseForge — its official server pack when there is one, FTB without client-only files, or a local
//! pack file), or a copy of a ResinCraft instance without its client-only mods.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::content::curseforge::cf;
use crate::content::index::{extract_where, open_zip, read_zip_text, safe_name};
use crate::core::events::Task;
use crate::core::fsutil::{now_ms, safe_join};
use crate::core::net::{download_all, download_file, fetch_json, hash_file, post_json, DownloadItem, Hash, HashAlgo};
use crate::core::settings::{self, total_memory_mb};
use crate::core::types::{
    source_of, ContentSource, Instance, LoaderType, ModpackRef, NewServer, PendingDownload, ServerCreated, ServerKind, ServerLaunch, ServerMeta,
    ServerSource,
};
use crate::core::{instances, paths, shell};

use super::{installers, properties, store};

const MODRINTH: &str = "https://api.modrinth.com/v2";
const FTB: &str = "https://api.feed-the-beast.com/v1/modpacks/public";
const PACK_HOSTS: [&str; 4] = ["cdn.modrinth.com", "github.com", "raw.githubusercontent.com", "gitlab.com"];
const DEFAULT_PORT: u16 = 25565;

/// What a server is built from, once the source has been looked at.
struct Plan {
    name: String,
    kind: ServerKind,
    mc: String,
    loader_version: Option<String>,
    memory: Option<u32>,
    icon: Option<String>,
    modpack: Option<ModpackRef>,
    content: Content,
    /// Downloaded archives to delete afterwards.
    temp: Vec<PathBuf>,
}

enum Content {
    None,
    Mrpack(PathBuf),
    Curseforge { archive: PathBuf, server_pack: Option<PathBuf> },
    Ftb(FtbVersion),
    Instance(Instance),
}

fn enc(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

fn kind_of_loader(loader: Option<LoaderType>) -> ServerKind {
    loader.map(ServerKind::from_loader).unwrap_or(ServerKind::Vanilla)
}

// ---------- Entry point ----------

pub async fn create(req: NewServer) -> Result<ServerCreated> {
    let plan = resolve(&req).await?;
    let name = Some(req.name.trim()).filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(|| plan.name.clone());
    let id = store::new_id(&name);
    let dir = store::dir(&id)?;
    let modded = plan.kind.loader().is_some();
    let total = total_memory_mb() as u32;
    let memory = req.memory_mb.or(plan.memory).unwrap_or(if modded { 4096 } else { 2048 }).min((total / 2).max(1024)).max(512);
    let meta = ServerMeta {
        id: id.clone(),
        name: name.clone(),
        kind: plan.kind,
        mc_version: plan.mc.clone(),
        loader: plan.kind.loader(),
        loader_version: req.loader_version.clone().filter(|v| !v.is_empty()).or(plan.loader_version.clone()),
        memory_mb: memory,
        port: match req.port {
            Some(p) if p > 0 => p,
            _ => free_port().await,
        },
        java_path: None,
        jvm_args: None,
        created_at: now_ms(),
        last_started: None,
        modpack: plan.modpack.clone(),
        icon_url: plan.icon.clone(),
        launch: ServerLaunch::default(),
        public: true,
        optimized_flags: true,
        tunnel: Default::default(),
        relay_host: None,
        relay_secret: None,
        domain: None,
        dns_auto: false,
        relay_name: None,
        relay_account: None,
    };
    tokio::fs::create_dir_all(&dir).await?;
    store::save(&meta).await?;

    let temp = plan.temp.clone();
    let content = plan.content;
    let dir2 = dir.clone();
    let result = Task::run(format!("Creating {name}"), Some(id.clone()), move |task| async move {
        let mut meta = meta;
        // Content first, so the loader installed by ResinCraft wins over anything a server pack brought along.
        let (dropped, pending) = fill(&dir2, content, &task).await?;
        installers::install(&mut meta, &dir2, &task).await?;
        // Saving chunks asynchronously is much smoother on most disks; keep a value a pack brought along.
        if properties::get_value(&dir2, "sync-chunk-writes").await.is_none() {
            properties::set_values(&dir2, &[("sync-chunk-writes", "false".to_string())]).await?;
        }
        store::save(&meta).await?;
        Ok((meta, dropped, pending))
    })
    .await;
    for t in temp {
        let _ = tokio::fs::remove_file(t).await;
    }
    match result {
        Ok((meta, dropped, pending)) => Ok(ServerCreated { server: store::info(meta).await, dropped, pending }),
        Err(e) => {
            let _ = tokio::fs::remove_dir_all(&dir).await;
            Err(e)
        }
    }
}

/// 25565, or the next port no other server uses.
async fn free_port() -> u16 {
    let used: HashSet<u16> = store::list().await.unwrap_or_default().into_iter().map(|m| m.port).collect();
    (DEFAULT_PORT..DEFAULT_PORT + 100).find(|p| !used.contains(p)).unwrap_or(DEFAULT_PORT)
}

async fn resolve(req: &NewServer) -> Result<Plan> {
    let plain = |kind: ServerKind, mc: String| Plan {
        name: format!("{} {mc} server", kind.label()),
        kind,
        mc,
        loader_version: None,
        memory: None,
        icon: None,
        modpack: None,
        content: Content::None,
        temp: Vec::new(),
    };
    match &req.source {
        None => {
            let mc = req.mc_version.clone().filter(|v| !v.is_empty()).ok_or_else(|| anyhow!("Choose a Minecraft version"))?;
            Ok(plain(req.kind.unwrap_or(ServerKind::Vanilla), mc))
        }
        Some(ServerSource::Instance { instance_id }) => {
            let inst = instances::get(instance_id).await?;
            let mut p = plain(ServerKind::from_loader(inst.loader), inst.mc_version.clone());
            p.name = format!("{} server", inst.name);
            p.loader_version = inst.loader_version.clone();
            p.memory = inst.memory_mb;
            p.icon = inst.icon_url.clone();
            p.modpack = inst.modpack.clone();
            p.content = Content::Instance(inst);
            Ok(p)
        }
        Some(ServerSource::Modpack { project_id, version_id }) => match source_of(project_id) {
            ContentSource::Modrinth => modrinth_plan(project_id, version_id.clone()).await,
            ContentSource::Curseforge => curseforge_plan(project_id, version_id.clone()).await,
            ContentSource::Ftb => ftb_plan(project_id, version_id.clone()).await,
            _ => bail!("This modpack source can't be turned into a server yet. Install the pack as an instance, then use \"Make a server from this instance\"."),
        },
        Some(ServerSource::File { path }) => {
            let file = match path {
                Some(p) => PathBuf::from(p),
                None => match shell::pick_file("Choose a modpack", &[("Modrinth or CurseForge modpack", &["mrpack", "zip"])]).await? {
                    Some(p) => p,
                    None => bail!("No file chosen"),
                },
            };
            file_plan(&file, None, None).await
        }
    }
}

// ---------- Modrinth ----------

#[derive(Deserialize)]
struct MrHashes {
    sha1: String,
}
#[derive(Deserialize)]
struct MrFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    size: u64,
    hashes: MrHashes,
}
#[derive(Deserialize)]
struct MrVersion {
    id: String,
    #[serde(default)]
    loaders: Vec<String>,
    #[serde(default)]
    files: Vec<MrFile>,
}
#[derive(Deserialize)]
struct MrProject {
    id: String,
    title: String,
    #[serde(default)]
    icon_url: Option<String>,
}

#[derive(Deserialize)]
struct PackEnv {
    #[serde(default)]
    server: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackFile {
    path: String,
    hashes: MrHashes,
    #[serde(default)]
    env: Option<PackEnv>,
    #[serde(default)]
    downloads: Vec<String>,
    #[serde(default)]
    file_size: Option<u64>,
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

async fn modrinth_plan(project_id: &str, version_id: Option<String>) -> Result<Plan> {
    let p: MrProject = fetch_json(&format!("{MODRINTH}/project/{}", enc(project_id))).await?;
    let v: MrVersion = match &version_id {
        Some(v) => fetch_json(&format!("{MODRINTH}/version/{}", enc(v))).await?,
        None => fetch_json::<Vec<MrVersion>>(&format!("{MODRINTH}/project/{}/version", enc(project_id)))
            .await?
            .into_iter()
            .find(|v| v.loaders.iter().any(|l| ["fabric", "quilt", "forge", "neoforge", "minecraft"].contains(&l.as_str())))
            .ok_or_else(|| anyhow!("{} has no version ResinCraft can install", p.title))?,
    };
    let f = v.files.iter().find(|f| f.primary).or(v.files.first()).ok_or_else(|| anyhow!("{} has no files", p.title))?;
    let dest = safe_join(&paths::cache().join("modpacks"), safe_name(&f.filename)?)?;
    let item = DownloadItem::new(&f.url, &dest).hash(Some(Hash::sha1(&f.hashes.sha1))).size(Some(f.size));
    Task::run(format!("Downloading {}", p.title), None, |_| async move { download_file(&item).await }).await?;
    let modpack = ModpackRef { name: p.title.clone(), project_id: Some(p.id.clone()), version_id: Some(v.id.clone()) };
    let mut plan = file_plan(&dest, Some(modpack), p.icon_url.clone()).await?;
    plan.temp.push(dest);
    Ok(plan)
}

/// A local .mrpack or CurseForge .zip.
async fn file_plan(file: &Path, modpack: Option<ModpackRef>, icon: Option<String>) -> Result<Plan> {
    let f = file.to_path_buf();
    let (mr, cf_manifest) = tokio::task::spawn_blocking(move || -> Result<(Option<String>, Option<String>)> {
        let mut zip = open_zip(&f).map_err(|_| anyhow!("This file isn't a modpack"))?;
        Ok((read_zip_text(&mut zip, "modrinth.index.json")?, read_zip_text(&mut zip, "manifest.json")?))
    })
    .await??;
    if let Some(raw) = mr {
        let pack: PackIndex = serde_json::from_str(&raw)?;
        let d = &pack.dependencies;
        let (loader, loader_version) = if let Some(v) = d.get("fabric-loader") {
            (Some(LoaderType::Fabric), Some(v.clone()))
        } else if let Some(v) = d.get("quilt-loader") {
            (Some(LoaderType::Quilt), Some(v.clone()))
        } else if let Some(v) = d.get("neoforge") {
            (Some(LoaderType::Neoforge), Some(v.clone()))
        } else if let Some(v) = d.get("forge") {
            (Some(LoaderType::Forge), Some(v.clone()))
        } else {
            (None, None)
        };
        let mc = d.get("minecraft").cloned().ok_or_else(|| anyhow!("The modpack doesn't say which Minecraft version it needs"))?;
        return Ok(Plan {
            name: pack.name.clone(),
            kind: kind_of_loader(loader),
            mc,
            loader_version,
            memory: None,
            icon,
            modpack: Some(modpack.unwrap_or(ModpackRef { name: pack.name.clone(), project_id: None, version_id: pack.version_id.clone() })),
            content: Content::Mrpack(file.to_path_buf()),
            temp: Vec::new(),
        });
    }
    if let Some(raw) = cf_manifest {
        let m: CfManifest = serde_json::from_str(&raw)?;
        let (loader, loader_version) = m.loader()?;
        let name = m.name.clone().unwrap_or_else(|| "Modpack".into());
        return Ok(Plan {
            name: name.clone(),
            kind: kind_of_loader(loader),
            mc: m.minecraft.version.clone(),
            loader_version,
            memory: None,
            icon,
            modpack: Some(modpack.unwrap_or(ModpackRef { name, project_id: None, version_id: m.version.clone() })),
            content: Content::Curseforge { archive: file.to_path_buf(), server_pack: None },
            temp: Vec::new(),
        });
    }
    bail!("This file isn't a Modrinth (.mrpack) or CurseForge modpack")
}

/// Files that only matter to a game client; left out of server folders.
fn client_junk(rel: &str) -> bool {
    let lower = rel.to_lowercase();
    let first = lower.split('/').next().unwrap_or_default();
    matches!(first, "resourcepacks" | "shaderpacks" | "screenshots" | "saves")
        || matches!(lower.as_str(), "options.txt" | "optionsof.txt" | "optionsshaders.txt" | "servers.dat" | "eula.txt")
}

/// Unpacks override folders into the server folder (zip-slip guarded, client files skipped).
/// eula.txt is never taken from a pack: agreeing is the player's decision.
async fn extract_prefixes(archive: &Path, prefixes: Vec<String>, dir: &Path) -> Result<()> {
    let (archive, dir) = (archive.to_path_buf(), dir.to_path_buf());
    tokio::task::spawn_blocking(move || -> Result<()> {
        let mut zip = open_zip(&archive)?;
        extract_where(&mut zip, &dir, |name| {
            let rel = prefixes.iter().find_map(|p| name.strip_prefix(p.as_str()))?;
            (!rel.is_empty() && !client_junk(rel)).then(|| rel.to_string())
        })
    })
    .await?
}

async fn fill_mrpack(dir: &Path, archive: &Path, task: &Arc<Task>) -> Result<()> {
    let a = archive.to_path_buf();
    let raw = tokio::task::spawn_blocking(move || -> Result<Option<String>> { read_zip_text(&mut open_zip(&a)?, "modrinth.index.json") }).await??;
    let pack: PackIndex = serde_json::from_str(&raw.unwrap_or_default())?;
    let mut items = Vec::new();
    for f in &pack.files {
        // env.server "unsupported" = client-only (shaders, minimaps, HUD mods…).
        if f.env.as_ref().and_then(|e| e.server.as_deref()) == Some("unsupported") || client_junk(&f.path) {
            continue;
        }
        let url = f
            .downloads
            .iter()
            .find(|u| reqwest::Url::parse(u).ok().and_then(|x| x.host_str().map(|h| PACK_HOSTS.contains(&h))).unwrap_or(false))
            .ok_or_else(|| anyhow!("Modpack file {} comes from an untrusted host", f.path))?;
        items.push(DownloadItem::new(url, safe_join(dir, &f.path)?).hash(Some(Hash::sha1(&f.hashes.sha1))).size(f.file_size));
    }
    download_all(items, 8, task.counter("Downloading server files")).await?;
    task.update("Applying modpack settings…", None);
    extract_prefixes(archive, vec!["overrides/".into(), "server-overrides/".into()], dir).await
}

// ---------- CurseForge ----------

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct CfModLoader {
    id: String,
    primary: Option<bool>,
}
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct CfMinecraft {
    version: String,
    mod_loaders: Vec<CfModLoader>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfManifestFile {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
    #[serde(default)]
    required: Option<bool>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfManifest {
    minecraft: CfMinecraft,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    files: Vec<CfManifestFile>,
    #[serde(default)]
    overrides: Option<String>,
}

impl CfManifest {
    fn loader(&self) -> Result<(Option<LoaderType>, Option<String>)> {
        let l = self.minecraft.mod_loaders.iter().find(|l| l.primary == Some(true)).or(self.minecraft.mod_loaders.first());
        let Some(l) = l else { return Ok((None, None)) };
        let (name, ver) = l.id.split_once('-').unwrap_or((l.id.as_str(), ""));
        let loader = match name.to_lowercase().as_str() {
            "forge" => LoaderType::Forge,
            "neoforge" => LoaderType::Neoforge,
            "fabric" => LoaderType::Fabric,
            "quilt" => LoaderType::Quilt,
            _ => bail!("This modpack needs {name}, which ResinCraft doesn't support yet."),
        };
        Ok((Some(loader), (!ver.is_empty()).then(|| ver.to_string())))
    }
}

fn cf_url(f: &Value) -> Option<String> {
    f.get("downloadUrl").and_then(Value::as_str).filter(|u| !u.is_empty()).map(str::to_string)
}
fn cf_sha1(f: &Value) -> Option<String> {
    f.get("hashes")?.as_array()?.iter().find(|h| h.get("algo").and_then(Value::as_u64) == Some(1))?.get("value")?.as_str().map(str::to_string)
}
fn cf_str(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or_default().to_string()
}

async fn curseforge_plan(project_id: &str, version_id: Option<String>) -> Result<Plan> {
    if settings::curseforge_key().await.is_empty() {
        bail!("CurseForge isn't available in this build of ResinCraft.");
    }
    let id = project_id.strip_prefix("cf:").unwrap_or(project_id);
    let m: Value = cf(&format!("/mods/{id}"), None).await?;
    let title = cf_str(&m, "name");
    let file: Value = match version_id {
        Some(v) => cf(&format!("/mods/{id}/files/{v}"), None).await?,
        None => {
            let mut files: Vec<Value> = cf(&format!("/mods/{id}/files?pageSize=50"), None).await?;
            files.retain(|f| f.get("isServerPack").and_then(Value::as_bool) != Some(true));
            files.sort_by_key(|f| std::cmp::Reverse(cf_str(f, "fileDate")));
            files.into_iter().next().ok_or_else(|| anyhow!("{title} has no downloadable version"))?
        }
    };
    let Some(url) = cf_url(&file) else { bail!("{title}'s author only allows downloads from the CurseForge website.") };
    let cache = paths::cache().join("modpacks");
    let archive = safe_join(&cache, safe_name(&cf_str(&file, "fileName"))?)?;
    let mut temp = vec![archive.clone()];
    // Prefer the pack's official server pack: its author picked the server-side files.
    let server_pack_id = file.get("serverPackFileId").and_then(Value::as_u64).filter(|v| *v > 0);
    let mut server_pack = None;
    let sp_file: Option<Value> = match server_pack_id {
        Some(sp) => cf(&format!("/mods/{id}/files/{sp}"), None).await.ok(),
        None => None,
    };
    let (a, sp) = (archive.clone(), sp_file.clone());
    let cache2 = cache.clone();
    let sp_path = Task::run(format!("Downloading {title}"), None, |t| async move {
        t.update("Downloading the modpack…", None);
        download_file(&DownloadItem::new(url, &a)).await?;
        if let Some(sp) = sp.as_ref() {
            if let Some(u) = cf_url(sp) {
                t.update("Downloading the server pack…", None);
                let dest = safe_join(&cache2, &format!("server-{}", safe_name(&cf_str(sp, "fileName"))?))?;
                download_file(&DownloadItem::new(u, &dest).hash(cf_sha1(sp).map(Hash::sha1))).await?;
                return Ok(Some(dest));
            }
        }
        Ok(None)
    })
    .await?;
    if let Some(p) = sp_path {
        temp.push(p.clone());
        server_pack = Some(p);
    }
    let icon = m.get("logo").and_then(|l| l.get("thumbnailUrl")).and_then(Value::as_str).map(str::to_string);
    let modpack = ModpackRef { name: title, project_id: Some(format!("cf:{id}")), version_id: file.get("id").and_then(Value::as_u64).map(|v| v.to_string()) };
    let mut plan = file_plan(&archive, Some(modpack), icon).await?;
    plan.content = Content::Curseforge { archive, server_pack };
    plan.temp = temp;
    Ok(plan)
}

async fn fill_curseforge(dir: &Path, archive: &Path, server_pack: Option<&Path>, task: &Arc<Task>) -> Result<(Vec<String>, Vec<PendingDownload>)> {
    if let Some(sp) = server_pack {
        task.update("Unpacking the server pack…", None);
        let (sp, d) = (sp.to_path_buf(), dir.to_path_buf());
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut zip = open_zip(&sp)?;
            // Many server packs wrap everything in one top-level folder; unwrap it.
            let names: Vec<String> = zip.file_names().map(str::to_string).collect();
            let tops: HashSet<&str> = names.iter().filter_map(|n| n.split_once('/').map(|(a, _)| a)).collect();
            let wrapped = tops.len() == 1 && names.iter().all(|n| n.contains('/'));
            let top = if wrapped { tops.into_iter().next().map(|t| format!("{t}/")) } else { None };
            extract_where(&mut zip, &d, |name| {
                let rel = match &top {
                    Some(t) => name.strip_prefix(t.as_str())?,
                    None => name,
                };
                (!rel.is_empty() && !client_junk(rel)).then(|| rel.to_string())
            })
        })
        .await??;
        return Ok((Vec::new(), Vec::new()));
    }

    let a = archive.to_path_buf();
    let raw = tokio::task::spawn_blocking(move || -> Result<Option<String>> { read_zip_text(&mut open_zip(&a)?, "manifest.json") }).await??;
    let manifest: CfManifest = serde_json::from_str(&raw.unwrap_or_default())?;
    let wanted: Vec<&CfManifestFile> = manifest.files.iter().filter(|f| f.required != Some(false)).collect();
    if !wanted.is_empty() && settings::curseforge_key().await.is_empty() {
        bail!("CurseForge isn't available in this build of ResinCraft.");
    }
    task.update("Looking up modpack files…", None);
    let mut files: Vec<Value> = Vec::new();
    let mut mods: Vec<Value> = Vec::new();
    for chunk in wanted.chunks(500) {
        files.extend(cf::<Vec<Value>>("/mods/files", Some(json!({ "fileIds": chunk.iter().map(|f| f.file_id).collect::<Vec<_>>() }))).await?);
        mods.extend(cf::<Vec<Value>>("/mods", Some(json!({ "modIds": chunk.iter().map(|f| f.project_id).collect::<Vec<_>>() }))).await?);
    }
    let mut items = Vec::new();
    let mut pending = Vec::new();
    let mut dropped = Vec::new();
    for f in &files {
        let mod_id = f.get("modId").and_then(Value::as_u64).unwrap_or(0);
        let m = mods.iter().find(|m| m.get("id").and_then(Value::as_u64) == Some(mod_id));
        let title = m.map(|m| cf_str(m, "name")).unwrap_or_else(|| cf_str(f, "displayName"));
        // Only mods go to a server (class 6); resource packs and shaders are client-side.
        let class = m.and_then(|m| m.get("classId")).and_then(Value::as_u64).unwrap_or(6);
        if class != 6 {
            continue;
        }
        // Newer files are tagged with the environments they run in.
        let tags: Vec<String> = f.get("gameVersions").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_lowercase).collect()).unwrap_or_default();
        if tags.iter().any(|t| t == "client") && !tags.iter().any(|t| t == "server") {
            dropped.push(title);
            continue;
        }
        let file_name = cf_str(f, "fileName");
        let Some(url) = cf_url(f) else {
            let site = m.and_then(|m| m.get("links")).and_then(|l| l.get("websiteUrl")).and_then(Value::as_str).unwrap_or("https://www.curseforge.com");
            pending.push(PendingDownload { title, file_name, url: format!("{site}/files/{}", f.get("id").and_then(Value::as_u64).unwrap_or(0)), folder: "mods".into() });
            continue;
        };
        items.push(DownloadItem::new(url, safe_join(&dir.join("mods"), safe_name(&file_name)?)?).hash(cf_sha1(f).map(Hash::sha1)));
    }
    download_all(items, 8, task.counter("Downloading server files")).await?;
    task.update("Applying modpack settings…", None);
    let mut prefix = manifest.overrides.clone().unwrap_or_else(|| "overrides".into());
    if !prefix.ends_with('/') {
        prefix.push('/');
    }
    extract_prefixes(archive, vec![prefix], dir).await?;
    dropped.extend(drop_client_only(&dir.join("mods"), task).await);
    Ok((dropped, pending))
}

// ---------- FTB ----------

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
    clientonly: bool,
    curseforge: Option<FtbCurse>,
}
#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct FtbVersion {
    id: u64,
    #[serde(rename = "type")]
    kind: String,
    released: i64,
    targets: Vec<FtbTarget>,
    specs: Option<FtbSpecs>,
    files: Vec<FtbFile>,
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
struct FtbPack {
    id: u64,
    name: String,
    art: Vec<FtbArt>,
    versions: Vec<FtbVersion>,
}

async fn ftb_plan(project_id: &str, version_id: Option<String>) -> Result<Plan> {
    let id: u64 = project_id.strip_prefix("ftb:").unwrap_or(project_id).parse().map_err(|_| anyhow!("Unknown FTB pack"))?;
    let p: FtbPack = fetch_json(&format!("{FTB}/modpack/{id}")).await?;
    let summary = match &version_id {
        Some(v) => p.versions.iter().find(|x| &x.id.to_string() == v),
        None => p.versions.iter().filter(|v| v.kind == "release").max_by_key(|v| v.released).or(p.versions.last()),
    };
    let summary = summary.ok_or_else(|| anyhow!("{} has no version to install", p.name))?;
    let v: FtbVersion = fetch_json(&format!("{FTB}/modpack/{}/{}", p.id, summary.id)).await?;
    let mc = v.targets.iter().find(|t| t.kind == "game").map(|t| t.version.clone()).ok_or_else(|| anyhow!("{} doesn't say which Minecraft version it needs", p.name))?;
    let ml = v.targets.iter().find(|t| t.kind == "modloader").cloned();
    let loader = match &ml {
        None => None,
        Some(t) => Some(crate::content::index::parse_loader(&t.name).ok_or_else(|| anyhow!("{} needs {}, which ResinCraft doesn't support yet.", p.name, t.name))?),
    };
    let icon = p.art.iter().find(|a| a.kind == "square").or(p.art.first()).map(|a| a.url.clone());
    Ok(Plan {
        name: p.name.clone(),
        kind: kind_of_loader(loader),
        mc,
        loader_version: ml.map(|t| t.version).filter(|v| !v.is_empty()),
        memory: v.specs.as_ref().map(|s| s.recommended).filter(|m| *m > 0),
        icon,
        modpack: Some(ModpackRef { name: p.name.clone(), project_id: Some(format!("ftb:{}", p.id)), version_id: Some(v.id.to_string()) }),
        content: Content::Ftb(v),
        temp: Vec::new(),
    })
}

async fn fill_ftb(dir: &Path, v: &FtbVersion, task: &Arc<Task>) -> Result<Vec<PendingDownload>> {
    let mut items = Vec::new();
    let mut pending = Vec::new();
    for f in v.files.iter().filter(|f| !f.clientonly) {
        let folder = f.path.trim_start_matches("./").trim_start_matches('.').trim_matches('/').to_string();
        if client_junk(&format!("{folder}/{}", f.name).trim_start_matches('/')) {
            continue;
        }
        if f.url.is_empty() {
            pending.push(PendingDownload {
                title: f.name.clone(),
                file_name: f.name.clone(),
                url: f.curseforge.as_ref().map(|c| format!("https://www.curseforge.com/projects/{}", c.project)).unwrap_or_else(|| "https://www.feed-the-beast.com/".into()),
                folder,
            });
            continue;
        }
        let dest = safe_join(dir, &format!("{folder}/{}", safe_name(&f.name)?))?;
        items.push(DownloadItem::new(&f.url, dest).hash((!f.sha1.is_empty()).then(|| Hash::sha1(&f.sha1))).size(f.size));
    }
    download_all(items, 8, task.counter("Downloading server files")).await?;
    Ok(pending)
}

// ---------- Instance ----------

async fn fill_instance(dir: &Path, inst: &Instance, task: &Arc<Task>) -> Result<Vec<String>> {
    task.update("Copying mods and configs…", None);
    let src = paths::game_dir(&inst.id);
    let (src2, dir2) = (src.clone(), dir.to_path_buf());
    tokio::task::spawn_blocking(move || -> Result<()> {
        if let Ok(rd) = std::fs::read_dir(src2.join("mods")) {
            std::fs::create_dir_all(dir2.join("mods"))?;
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                // Enabled mods only; disabled ones end in .disabled.
                if e.file_type().map(|t| t.is_file()).unwrap_or(false) && name.to_lowercase().ends_with(".jar") {
                    std::fs::copy(e.path(), dir2.join("mods").join(&name))?;
                }
            }
        }
        for folder in ["config", "defaultconfigs", "kubejs", "scripts"] {
            let from = src2.join(folder);
            if from.is_dir() {
                instances::copy_dir(&from, &dir2.join(folder), &[])?;
            }
        }
        Ok(())
    })
    .await??;
    Ok(drop_client_only(&dir.join("mods"), task).await)
}

// ---------- Client-only mods ----------

/// Removes mods Modrinth marks as not running on servers (server_side "unsupported"); returns their names.
/// Mods Modrinth doesn't know are kept. Offline: nothing is removed.
pub async fn drop_client_only(mods: &Path, task: &Arc<Task>) -> Vec<String> {
    task.update("Checking which mods are client-only…", None);
    match find_client_only(mods).await {
        Ok(found) => {
            let mut names = Vec::new();
            for (path, title) in found {
                if tokio::fs::remove_file(&path).await.is_ok() {
                    names.push(title);
                }
            }
            names.sort();
            names
        }
        Err(e) => {
            eprintln!("[server] client-only check skipped: {e:#}");
            Vec::new()
        }
    }
}

async fn find_client_only(mods: &Path) -> Result<Vec<(PathBuf, String)>> {
    #[derive(Deserialize)]
    struct Ver {
        project_id: String,
    }
    #[derive(Deserialize)]
    struct Proj {
        id: String,
        title: String,
        #[serde(default)]
        server_side: String,
    }
    let mut by_hash: HashMap<String, PathBuf> = HashMap::new();
    let Ok(mut rd) = tokio::fs::read_dir(mods).await else { return Ok(Vec::new()) };
    while let Some(e) = rd.next_entry().await? {
        if e.file_name().to_string_lossy().to_lowercase().ends_with(".jar") {
            by_hash.insert(hash_file(&e.path(), HashAlgo::Sha1).await?, e.path());
        }
    }
    if by_hash.is_empty() {
        return Ok(Vec::new());
    }
    let found: HashMap<String, Ver> = post_json(&format!("{MODRINTH}/version_files"), &json!({ "hashes": by_hash.keys().collect::<Vec<_>>(), "algorithm": "sha1" })).await?;
    let ids: Vec<String> = found.values().map(|v| v.project_id.clone()).collect::<HashSet<_>>().into_iter().collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut projects: Vec<Proj> = Vec::new();
    for chunk in ids.chunks(100) {
        projects.extend(fetch_json::<Vec<Proj>>(&format!("{MODRINTH}/projects?ids={}", enc(&serde_json::to_string(chunk)?))).await?);
    }
    let mut out = Vec::new();
    for (hash, v) in found {
        let Some(p) = projects.iter().find(|p| p.id == v.project_id) else { continue };
        if p.server_side == "unsupported" {
            if let Some(path) = by_hash.get(&hash) {
                out.push((path.clone(), p.title.clone()));
            }
        }
    }
    Ok(out)
}

// ---------- Filling ----------

async fn fill(dir: &Path, content: Content, task: &Arc<Task>) -> Result<(Vec<String>, Vec<PendingDownload>)> {
    match content {
        Content::None => Ok((Vec::new(), Vec::new())),
        Content::Mrpack(archive) => {
            fill_mrpack(dir, &archive, task).await?;
            Ok((Vec::new(), Vec::new()))
        }
        Content::Curseforge { archive, server_pack } => fill_curseforge(dir, &archive, server_pack.as_deref(), task).await,
        Content::Ftb(v) => Ok((Vec::new(), fill_ftb(dir, &v, task).await?)),
        Content::Instance(inst) => Ok((fill_instance(dir, &inst, task).await?, Vec::new())),
    }
}
