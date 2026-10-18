//! Installing server software into a server folder: the official vanilla server.jar, Paper / Purpur / Folia /
//! Leaf from their download APIs, Fabric's server launcher, Quilt's installer, the Forge / NeoForge installers
//! (`--installServer`), the Arclight hybrid (one jar; it fetches its libraries on first start) and Bedrock
//! Dedicated Server. Each fills in `ServerMeta::launch` (how to start it) and the loader/build version.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::content::index::natural_cmp;
use crate::core::events::Task;
use crate::core::net::{download_all, fetch_json, fetch_text, status_of, DownloadItem, Hash};
use crate::core::types::{LoaderType, ServerBuild, ServerKind, ServerLaunch, ServerMeta};
use crate::core::{paths, settings};
use crate::game::java::{find_java, hidden_command, install_java};
use crate::game::loaders::{installer_url, is_legacy_forge, latest_loader, legacy_forge_error, normalize_loader_version};
use crate::game::mojang::{get_vanilla_version, plain};

const FILL: &str = "https://fill.papermc.io/v3/projects";
const PURPUR: &str = "https://api.purpurmc.org/v2/purpur";
const FABRIC_META: &str = "https://meta.fabricmc.net/v2";
const SPIGOT_VERSIONS: &str = "https://hub.spigotmc.org/versions/";
const BUILDTOOLS: &str = "https://hub.spigotmc.org/jenkins/job/BuildTools/lastSuccessfulBuild/artifact/target/BuildTools.jar";
const QUILT_INSTALLER: &str = "https://maven.quiltmc.org/repository/release/org/quiltmc/quilt-installer";
const LEAF: &str = "https://api.leafmc.one/v2/projects/leaf";
/// Arclight's own file server: one folder per Minecraft version (some are aliases of the same branch).
const ARCLIGHT: &str = "https://files.hypertention.cn/v1/files/arclight/minecraft";
/// Every Bedrock Dedicated Server version, kept by the Bedrock OSS community (the downloads are Mojang's).
const BEDROCK_VERSIONS: &str = "https://raw.githubusercontent.com/Bedrock-OSS/BDS-Versions/main/versions.json";
/// Mojang's own download links (the newest release), in case that list hasn't caught up yet.
const BEDROCK_LINKS: &str = "https://net-secondary.web.minecraft-services.net/api/v1.0/download/links";
const BEDROCK_CDN: &str = "https://www.minecraft.net/bedrockdedicatedserver";

fn enc(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

// ---------- Java ----------

/// The Java major version a Minecraft version needs (from Mojang's version JSON).
pub async fn required_java(mc: &str) -> Result<u32> {
    Ok(get_vanilla_version(mc).await?.java_major())
}

fn non_empty(s: Option<&str>) -> Option<String> {
    s.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

/// The server's own Java, else the global one from Settings, else a detected install, else Temurin
/// downloaded into the launcher's Java folder (when automatic Java downloads are on).
pub async fn choose_java(meta: &ServerMeta) -> Result<String> {
    let settings = settings::get().await;
    if let Some(custom) = non_empty(meta.java_path.as_deref()).or_else(|| non_empty(Some(&settings.java_path))) {
        return Ok(custom);
    }
    let required = required_java(&meta.mc_version).await?;
    if let Some(found) = find_java(required).await? {
        return Ok(found.path);
    }
    if settings.auto_java {
        let installed = Task::run(format!("Downloading Java {required}"), Some(meta.id.clone()), |t| async move { install_java(required, &t).await }).await?;
        return Ok(installed.path);
    }
    bail!("Minecraft {} needs Java {required}. Install it, or turn on automatic Java downloads in Settings.", meta.mc_version)
}

// ---------- Version lists ----------

/// Minecraft versions a server type can be created for. Empty for vanilla and mod loaders (the UI uses
/// Mojang's list and the loader lists for those).
pub async fn software_versions(kind: ServerKind) -> Result<Vec<String>> {
    let mut list: Vec<String> = match kind {
        ServerKind::Paper | ServerKind::Folia => {
            #[derive(Deserialize)]
            struct Project {
                versions: HashMap<String, Vec<String>>,
            }
            let p: Project = fetch_json(&format!("{FILL}/{}", project_of(kind))).await?;
            p.versions.into_values().flatten().collect()
        }
        ServerKind::Purpur => {
            #[derive(Deserialize)]
            struct Project {
                versions: Vec<String>,
            }
            fetch_json::<Project>(PURPUR).await?.versions
        }
        ServerKind::Spigot => {
            // BuildTools' version list is a plain directory listing of <version>.json files.
            let html = fetch_text(SPIGOT_VERSIONS).await?;
            let re = regex::Regex::new(r#"href="(\d+\.\d+(?:\.\d+)?)\.json""#)?;
            re.captures_iter(&html).map(|c| c[1].to_string()).collect()
        }
        ServerKind::Leaf => {
            #[derive(Deserialize)]
            struct Project {
                versions: Vec<String>,
            }
            fetch_json::<Project>(LEAF).await?.versions
        }
        ServerKind::Arclight => arclight_versions().await?,
        ServerKind::Bedrock => bedrock_versions().await?,
        _ => return Ok(Vec::new()),
    };
    // Releases only; pre-releases and release candidates are experimental builds.
    list.retain(|v| !v.contains('-'));
    list.sort_by(|a, b| natural_cmp(b, a));
    list.dedup();
    Ok(list)
}

/// The builds a server type offers for a Minecraft version, when there's a choice to make: Arclight comes for
/// Forge, NeoForge or Fabric. Empty for the other types (they take the newest build).
pub async fn builds(kind: ServerKind, mc: &str) -> Result<Vec<ServerBuild>> {
    match kind {
        ServerKind::Arclight => arclight_builds(mc).await,
        _ => Ok(Vec::new()),
    }
}

fn project_of(kind: ServerKind) -> &'static str {
    match kind {
        ServerKind::Folia => "folia",
        _ => "paper",
    }
}

// ---------- Install ----------

/// Installs the server software for `meta` into `dir` and records how to start it.
pub async fn install(meta: &mut ServerMeta, dir: &Path, task: &Arc<Task>) -> Result<()> {
    tokio::fs::create_dir_all(dir).await?;
    let mc = meta.mc_version.clone();
    let wanted = meta.loader_version.clone().filter(|v| !v.is_empty());
    let mut loader = meta.kind.loader();
    let (launch, version) = match meta.kind {
        ServerKind::Vanilla => {
            vanilla_jar(&mc, &dir.join("server.jar"), task).await?;
            (jar("server.jar"), None)
        }
        ServerKind::Leaf => {
            let build = leaf(&mc, wanted.as_deref(), dir, task).await?;
            (jar("leaf.jar"), Some(build))
        }
        ServerKind::Arclight => {
            let (on, build) = arclight(&mc, wanted.as_deref(), dir, task).await?;
            loader = Some(on);
            (jar("arclight.jar"), Some(build))
        }
        ServerKind::Bedrock => {
            let exe = bedrock(&mc, dir, task).await?;
            (ServerLaunch { exe: Some(exe), ..Default::default() }, None)
        }
        ServerKind::Paper | ServerKind::Folia => {
            let build = paper_like(meta.kind, &mc, wanted.as_deref(), dir, task).await?;
            (jar(&format!("{}.jar", project_of(meta.kind))), Some(build))
        }
        ServerKind::Purpur => {
            let build = purpur(&mc, wanted.as_deref(), dir, task).await?;
            (jar("purpur.jar"), Some(build))
        }
        ServerKind::Spigot => {
            let java = choose_java(meta).await?;
            spigot(&mc, dir, &java, task).await?;
            (jar("spigot.jar"), None)
        }
        ServerKind::Fabric => {
            let loader = match wanted {
                Some(v) => v,
                None => latest_loader(LoaderType::Fabric, &mc).await?,
            };
            fabric(&mc, &loader, dir, task).await?;
            (jar("fabric-server-launch.jar"), Some(loader))
        }
        ServerKind::Quilt => {
            let loader = match wanted {
                Some(v) => v,
                None => latest_loader(LoaderType::Quilt, &mc).await?,
            };
            let java = choose_java(meta).await?;
            quilt(&mc, &loader, dir, &java, task).await?;
            (jar("quilt-server-launch.jar"), Some(loader))
        }
        ServerKind::Forge | ServerKind::Neoforge => {
            let loader = meta.kind.loader().unwrap_or(LoaderType::Forge);
            let ver = match wanted {
                Some(v) => normalize_loader_version(loader, &mc, &v),
                None => latest_loader(loader, &mc).await?,
            };
            let java = choose_java(meta).await?;
            let launch = forge_like(loader, &mc, &ver, dir, &java, task).await?;
            (launch, Some(ver))
        }
    };
    meta.launch = launch;
    meta.loader = loader;
    meta.loader_version = version;
    Ok(())
}

fn jar(name: &str) -> ServerLaunch {
    ServerLaunch { jar: Some(name.to_string()), ..Default::default() }
}

/// The official server.jar from the version JSON's `downloads.server`.
async fn vanilla_jar(mc: &str, dest: &Path, task: &Arc<Task>) -> Result<()> {
    task.update(format!("Downloading the Minecraft {mc} server…"), None);
    let v = get_vanilla_version(mc).await?;
    let f = v.downloads.as_ref().and_then(|d| d.get("server")).ok_or_else(|| anyhow!("Minecraft {mc} has no official server download"))?;
    let item = DownloadItem::new(&f.url, dest).hash(f.sha1.clone().map(Hash::sha1)).size(f.size);
    download_all(vec![item], 1, task.counter("Downloading the server")).await
}

#[derive(Deserialize)]
struct FillChecksums {
    sha256: String,
}
#[derive(Deserialize)]
struct FillDownload {
    url: String,
    checksums: FillChecksums,
    #[serde(default)]
    size: Option<u64>,
}
#[derive(Deserialize)]
struct FillBuild {
    id: u64,
    #[serde(default)]
    channel: String,
    #[serde(default)]
    downloads: HashMap<String, FillDownload>,
}

/// Paper or Folia from PaperMC's download API. Returns the build number.
async fn paper_like(kind: ServerKind, mc: &str, build: Option<&str>, dir: &Path, task: &Arc<Task>) -> Result<String> {
    let project = project_of(kind);
    task.update(format!("Finding {} for Minecraft {mc}…", kind.label()), None);
    let builds: Vec<FillBuild> = match fetch_json(&format!("{FILL}/{project}/versions/{}/builds", enc(mc))).await {
        Err(e) if status_of(&e) == Some(404) => bail!("{} isn't available for Minecraft {mc}", kind.label()),
        r => r?,
    };
    let pick = match build {
        Some(b) => builds.iter().find(|x| x.id.to_string() == b),
        None => builds.iter().find(|x| x.channel.eq_ignore_ascii_case("stable")).or(builds.first()),
    };
    let pick = pick.ok_or_else(|| anyhow!("{} has no build for Minecraft {mc}", kind.label()))?;
    let dl = pick.downloads.get("server:default").ok_or_else(|| anyhow!("{} build {} has no server download", kind.label(), pick.id))?;
    let item = DownloadItem::new(&dl.url, dir.join(format!("{project}.jar"))).hash(Some(Hash::sha256(&dl.checksums.sha256))).size(dl.size);
    download_all(vec![item], 1, task.counter(&format!("Downloading {}", kind.label()))).await?;
    Ok(pick.id.to_string())
}

/// Purpur from its download API. Returns the build number.
async fn purpur(mc: &str, build: Option<&str>, dir: &Path, task: &Arc<Task>) -> Result<String> {
    #[derive(Deserialize)]
    struct Build {
        build: String,
        #[serde(default)]
        md5: Option<String>,
    }
    task.update(format!("Finding Purpur for Minecraft {mc}…"), None);
    let b: Build = match fetch_json(&format!("{PURPUR}/{}/{}", enc(mc), enc(build.unwrap_or("latest")))).await {
        Err(e) if matches!(status_of(&e), Some(400 | 404)) => bail!("Purpur isn't available for Minecraft {mc}"),
        r => r?,
    };
    let url = format!("{PURPUR}/{}/{}/download", enc(mc), enc(&b.build));
    let item = DownloadItem::new(url, dir.join("purpur.jar")).hash(b.md5.map(Hash::md5));
    download_all(vec![item], 1, task.counter("Downloading Purpur")).await?;
    Ok(b.build)
}

/// Leaf from its download API (the same shape as PaperMC's old one). Returns the build number.
async fn leaf(mc: &str, build: Option<&str>, dir: &Path, task: &Arc<Task>) -> Result<String> {
    #[derive(Deserialize)]
    struct Download {
        name: String,
        sha256: String,
    }
    #[derive(Deserialize)]
    struct Build {
        build: u64,
        #[serde(default)]
        channel: String,
        downloads: HashMap<String, Download>,
    }
    #[derive(Deserialize)]
    struct Builds {
        builds: Vec<Build>,
    }
    task.update(format!("Finding Leaf for Minecraft {mc}…"), None);
    let list = match fetch_json::<Builds>(&format!("{LEAF}/versions/{}/builds", enc(mc))).await {
        Err(e) if matches!(status_of(&e), Some(400 | 404)) => bail!("Leaf isn't available for Minecraft {mc}"),
        r => r?.builds,
    };
    // Oldest first; "default" is the stable channel.
    let pick = match build {
        Some(b) => list.iter().find(|x| x.build.to_string() == b),
        None => list.iter().rev().find(|x| x.channel == "default").or(list.last()),
    };
    let pick = pick.ok_or_else(|| anyhow!("Leaf has no build for Minecraft {mc}"))?;
    let dl = pick.downloads.get("primary").ok_or_else(|| anyhow!("Leaf build {} has no server download", pick.build))?;
    let url = format!("{LEAF}/versions/{}/builds/{}/downloads/{}", enc(mc), pick.build, enc(&dl.name));
    let item = DownloadItem::new(url, dir.join("leaf.jar")).hash(Some(Hash::sha256(&dl.sha256)));
    download_all(vec![item], 1, task.counter("Downloading Leaf")).await?;
    Ok(pick.build.to_string())
}

/// One entry of an Arclight file-server listing.
#[derive(Deserialize, Clone)]
struct ArcFile {
    name: String,
    key: String,
    #[serde(default)]
    permlink: Option<String>,
}
#[derive(Deserialize)]
struct ArcListing {
    #[serde(default)]
    files: Vec<ArcFile>,
    #[serde(default)]
    key: String,
}

/// Minecraft versions with an Arclight build. Some names are aliases of the same branch ("1.21" is 1.21.1):
/// only the newest name of each branch is kept.
async fn arclight_versions() -> Result<Vec<String>> {
    let names: Vec<String> = fetch_json::<ArcListing>(ARCLIGHT).await?.files.into_iter().map(|f| f.name).collect();
    let checks = names.into_iter().map(|v| async move {
        // A listing's own key names the branch behind the alias.
        let branch = fetch_json::<ArcListing>(&format!("{ARCLIGHT}/{}/loaders", enc(&v))).await.ok().map(|l| l.key);
        (v, branch)
    });
    let mut newest: HashMap<String, String> = HashMap::new();
    for (v, branch) in futures::future::join_all(checks).await {
        let branch = branch.filter(|b| !b.is_empty()).unwrap_or_else(|| v.clone());
        match newest.get(&branch) {
            Some(have) if natural_cmp(have, &v).is_ge() => {}
            _ => {
                newest.insert(branch, v);
            }
        }
    }
    Ok(newest.into_values().collect())
}

fn arc_loader(name: &str) -> Option<LoaderType> {
    match name {
        "forge" => Some(LoaderType::Forge),
        "neoforge" => Some(LoaderType::Neoforge),
        "fabric" => Some(LoaderType::Fabric),
        _ => None,
    }
}

/// The build version is the folder above the loader's file: `.../versions-stable/1.0.1-8ec9529/neoforge`.
fn arc_version(f: &ArcFile) -> String {
    f.key.rsplit('/').nth(1).unwrap_or_default().to_string()
}

/// The newest stable Arclight for each loader, then the newer development builds. Ids are "loader@version".
async fn arclight_builds(mc: &str) -> Result<Vec<ServerBuild>> {
    let mut out: Vec<ServerBuild> = Vec::new();
    for (folder, stable) in [("latest-stable", true), ("latest-snapshot", false)] {
        let Ok(list) = fetch_json::<ArcListing>(&format!("{ARCLIGHT}/{}/{folder}", enc(mc))).await else { continue };
        let mut files = list.files;
        // NeoForge first, then Forge, then Fabric.
        files.sort_by_key(|f| ["neoforge", "forge", "fabric"].iter().position(|n| *n == f.name).unwrap_or(9));
        for f in files {
            let Some(loader) = arc_loader(&f.name) else { continue };
            let id = format!("{}@{}", f.name, arc_version(&f));
            if out.iter().any(|b| b.id == id) {
                continue;
            }
            out.push(ServerBuild { label: format!("{} · {}", loader.label(), arc_version(&f)), id, stable });
        }
    }
    Ok(out)
}

/// Arclight for `mc`: the build chosen in the UI ("loader@version"), else the newest stable one.
/// Returns the mod loader it runs on and the build version.
async fn arclight(mc: &str, build: Option<&str>, dir: &Path, task: &Arc<Task>) -> Result<(LoaderType, String)> {
    task.update(format!("Finding Arclight for Minecraft {mc}…"), None);
    let (name, version) = match build.and_then(|b| b.split_once('@')) {
        Some((l, v)) => (l.to_string(), v.to_string()),
        None => {
            let first = arclight_builds(mc).await?.into_iter().next().ok_or_else(|| anyhow!("Arclight isn't available for Minecraft {mc}"))?;
            let (l, v) = first.id.split_once('@').unwrap_or_default();
            (l.to_string(), v.to_string())
        }
    };
    let loader = arc_loader(&name).ok_or_else(|| anyhow!("Arclight doesn't come for {name}"))?;
    let folders = ["latest-stable".to_string(), "latest-snapshot".to_string(), format!("versions-stable/{version}"), format!("versions-snapshot/{version}")];
    let mut found = None;
    for folder in folders {
        let Ok(list) = fetch_json::<ArcListing>(&format!("{ARCLIGHT}/{}/{folder}", enc(mc))).await else { continue };
        found = list.files.into_iter().find(|f| f.name == name && arc_version(f) == version && f.permlink.is_some());
        if found.is_some() {
            break;
        }
    }
    let url = found.and_then(|f| f.permlink).ok_or_else(|| anyhow!("Arclight {version} for {} {mc} couldn't be found", loader.label()))?;
    // The object's name is the SHA-1 of the file.
    let sha1 = url.rsplit('/').next().filter(|h| h.len() == 40 && h.chars().all(|c| c.is_ascii_hexdigit())).map(Hash::sha1);
    download_all(vec![DownloadItem::new(&url, dir.join("arclight.jar")).hash(sha1)], 1, task.counter("Downloading Arclight")).await?;
    Ok((loader, version))
}

/// This computer's name in Mojang's download links and in the version list.
fn bedrock_platform() -> (&'static str, &'static str, &'static str) {
    if cfg!(windows) {
        ("windows", "bin-win", "serverBedrockWindows")
    } else {
        ("linux", "bin-linux", "serverBedrockLinux")
    }
}

/// Bedrock Dedicated Server versions (releases), from the community list plus Mojang's newest.
async fn bedrock_versions() -> Result<Vec<String>> {
    let (key, _, link) = bedrock_platform();
    let mut list: Vec<String> = match fetch_json::<Value>(BEDROCK_VERSIONS).await {
        Ok(v) => v.get(key).and_then(|p| p.get("versions")).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default(),
        Err(e) => {
            eprintln!("[server] Bedrock version list unavailable: {e:#}");
            Vec::new()
        }
    };
    if let Ok(v) = fetch_json::<Value>(BEDROCK_LINKS).await {
        let newest = v
            .pointer("/result/links")
            .and_then(Value::as_array)
            .and_then(|a| a.iter().find(|l| l.get("downloadType").and_then(Value::as_str) == Some(link)))
            .and_then(|l| l.get("downloadUrl"))
            .and_then(Value::as_str)
            .and_then(|u| u.rsplit("bedrock-server-").next())
            .and_then(|f| f.strip_suffix(".zip"))
            .map(str::to_string);
        if let Some(n) = newest.filter(|n| !list.contains(n)) {
            list.push(n);
        }
    }
    if list.is_empty() {
        bail!("Couldn't load the Bedrock server versions. Check the internet connection");
    }
    Ok(list)
}

/// Bedrock Dedicated Server from minecraft.net, unpacked into the server folder. Mojang publishes no checksums,
/// so the download is checked by unpacking it and finding the server program inside. Returns the program's name.
async fn bedrock(version: &str, dir: &Path, task: &Arc<Task>) -> Result<String> {
    let (_, folder, _) = bedrock_platform();
    let zip = paths::cache().join("server-installers").join(format!("bedrock-server-{version}.zip"));
    let url = format!("{BEDROCK_CDN}/{folder}/bedrock-server-{}.zip", enc(version));
    task.update(format!("Downloading the Bedrock server {version}…"), None);
    if let Err(e) = download_all(vec![DownloadItem::new(url, &zip)], 1, task.counter("Downloading the Bedrock server")).await {
        if matches!(status_of(&e), Some(403 | 404)) {
            bail!("Bedrock server {version} isn't available from minecraft.net");
        }
        return Err(e);
    }
    task.update("Unpacking the Bedrock server…", None);
    let (zip2, dir2) = (zip.clone(), dir.to_path_buf());
    let unpacked = tokio::task::spawn_blocking(move || -> Result<()> {
        let mut archive = crate::content::index::open_zip(&zip2)?;
        crate::content::index::extract_where(&mut archive, &dir2, |name| Some(name.to_string()))
    })
    .await?;
    let _ = tokio::fs::remove_file(&zip).await;
    unpacked.context("The Bedrock server download is damaged")?;
    let exe = if cfg!(windows) { "bedrock_server.exe" } else { "bedrock_server" };
    if !dir.join(exe).is_file() {
        bail!("The Bedrock server download didn't contain {exe}");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(dir.join(exe), std::fs::Permissions::from_mode(0o755)).await?;
    }
    Ok(exe.to_string())
}

/// Spigot can't be redistributed as a ready-made jar, so it's compiled locally with Spigot's official
/// BuildTools (which fetches Git and the sources itself). Takes a few minutes the first time.
async fn spigot(mc: &str, dir: &Path, java: &str, task: &Arc<Task>) -> Result<()> {
    let work = paths::cache().join("buildtools");
    tokio::fs::create_dir_all(&work).await?;
    let tool = work.join("BuildTools.jar");
    task.update("Downloading Spigot BuildTools…", None);
    // Always fetch the newest BuildTools: older ones can't build new Minecraft versions.
    crate::core::net::download_file(&DownloadItem::new(BUILDTOOLS, &tool)).await?;
    let args = vec![
        "-jar".to_string(),
        plain(&tool),
        "--rev".into(),
        mc.into(),
        "--compile".into(),
        "spigot".into(),
        "--output-dir".into(),
        plain(dir),
        "--nogui".into(),
    ];
    run_installer(java, &args, &work, task, "Spigot BuildTools").await?;
    let built = dir.join(format!("spigot-{mc}.jar"));
    if !built.is_file() {
        bail!("BuildTools finished without producing spigot-{mc}.jar");
    }
    tokio::fs::rename(&built, dir.join("spigot.jar")).await?;
    Ok(())
}

/// Fabric's server launcher (it fetches its libraries on first start) plus the vanilla server.jar it wraps.
async fn fabric(mc: &str, loader: &str, dir: &Path, task: &Arc<Task>) -> Result<()> {
    #[derive(Deserialize)]
    struct Installer {
        version: String,
        #[serde(default)]
        stable: bool,
    }
    task.update("Finding the Fabric server launcher…", None);
    let installers: Vec<Installer> = fetch_json(&format!("{FABRIC_META}/versions/installer")).await?;
    let installer = installers.iter().find(|i| i.stable).or(installers.first()).ok_or_else(|| anyhow!("Fabric's installer list is empty"))?;
    let url = format!("{FABRIC_META}/versions/loader/{}/{}/{}/server/jar", enc(mc), enc(loader), enc(&installer.version));
    download_all(vec![DownloadItem::new(url, dir.join("fabric-server-launch.jar"))], 1, task.counter("Downloading Fabric")).await?;
    vanilla_jar(mc, &dir.join("server.jar"), task).await
}

/// Quilt's installer in server mode, which writes quilt-server-launch.jar, its libraries and server.jar.
async fn quilt(mc: &str, loader: &str, dir: &Path, java: &str, task: &Arc<Task>) -> Result<()> {
    task.update("Finding the Quilt installer…", None);
    let xml = fetch_text(&format!("{QUILT_INSTALLER}/maven-metadata.xml")).await?;
    let ver = regex::Regex::new(r"<release>([^<]+)</release>")?
        .captures(&xml)
        .map(|c| c[1].to_string())
        .ok_or_else(|| anyhow!("Couldn't read Quilt's installer version"))?;
    let installer = paths::cache().join("server-installers").join(format!("quilt-installer-{ver}.jar"));
    let url = format!("{QUILT_INSTALLER}/{ver}/quilt-installer-{ver}.jar");
    download_all(vec![DownloadItem::new(url, &installer)], 1, task.counter("Downloading the Quilt installer")).await?;
    let args = vec![
        "-jar".to_string(),
        plain(&installer),
        "install".into(),
        "server".into(),
        mc.into(),
        loader.into(),
        "--download-server".into(),
        format!("--install-dir={}", plain(dir)),
    ];
    run_installer(java, &args, dir, task, "Quilt").await?;
    if !dir.join("quilt-server-launch.jar").is_file() {
        bail!("The Quilt installer finished without creating quilt-server-launch.jar");
    }
    Ok(())
}

/// Runs the Forge / NeoForge installer with `--installServer` and works out how to start the result:
/// the @argument file of modern versions (1.17+), or the forge jar of older ones.
async fn forge_like(loader: LoaderType, mc: &str, ver: &str, dir: &Path, java: &str, task: &Arc<Task>) -> Result<ServerLaunch> {
    if loader == LoaderType::Forge && is_legacy_forge(mc) {
        return Err(legacy_forge_error(mc));
    }
    let name = format!("{} {ver}", loader.label());
    task.update(format!("Downloading the {name} installer…"), None);
    let installer = paths::cache().join("server-installers").join(format!("{}-{mc}-{ver}-installer.jar", loader.as_str()));
    if let Err(e) = download_all(vec![DownloadItem::new(installer_url(loader, mc, ver), &installer)], 1, |_, _| {}).await {
        if status_of(&e) == Some(404) {
            bail!("{name} for Minecraft {mc} couldn't be found");
        }
        return Err(e);
    }
    let args = vec!["-jar".to_string(), plain(&installer), "--installServer".into(), plain(dir)];
    run_installer(java, &args, dir, task, &name).await?;

    let wanted = if cfg!(windows) { "win_args.txt" } else { "unix_args.txt" };
    let found = {
        let (root, ver, wanted) = (dir.join("libraries"), ver.to_string(), wanted.to_string());
        tokio::task::spawn_blocking(move || {
            let mut hits = Vec::new();
            find_files(&root, &wanted, 8, &mut hits);
            hits.sort_by_key(|p| !p.to_string_lossy().contains(&ver));
            hits.into_iter().next()
        })
        .await?
    };
    if let Some(args_file) = found {
        let rel = args_file.strip_prefix(dir).unwrap_or(&args_file).to_string_lossy().replace('\\', "/");
        return Ok(ServerLaunch { args_file: Some(rel), ..Default::default() });
    }
    // 1.13–1.16: the installer leaves forge-<mc>-<ver>.jar in the server folder.
    let mut rd = tokio::fs::read_dir(dir).await?;
    let mut jars = Vec::new();
    while let Some(e) = rd.next_entry().await? {
        let n = e.file_name().to_string_lossy().into_owned();
        let lower = n.to_lowercase();
        if (lower.starts_with("forge-") || lower.starts_with("neoforge-")) && lower.ends_with(".jar") && !lower.contains("installer") {
            jars.push(n);
        }
    }
    jars.sort_by_key(|n| !n.contains(ver));
    match jars.into_iter().next() {
        Some(j) => Ok(jar(&j)),
        None => bail!("The {name} installer finished, but ResinCraft couldn't find how to start the server"),
    }
}

fn find_files(dir: &Path, name: &str, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            if depth > 0 {
                find_files(&p, name, depth - 1, out);
            }
        } else if e.file_name() == name {
            out.push(p);
        }
    }
}

/// Runs an installer jar, showing its progress lines in the task; on failure the last lines become the error.
async fn run_installer(java: &str, args: &[String], cwd: &Path, task: &Arc<Task>, name: &str) -> Result<()> {
    task.update(format!("Running the {name} installer…"), None);
    let mut cmd = hidden_command(java);
    cmd.args(args)
        .current_dir(cwd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let mut child = cmd.spawn().with_context(|| format!("Couldn't start Java ({java})"))?;
    let tail = Arc::new(std::sync::Mutex::new(VecDeque::<String>::new()));
    let mut readers = Vec::new();
    for stream in [child.stdout.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Send + Unpin>), child.stderr.take().map(|s| Box::new(s) as _)]
        .into_iter()
        .flatten()
    {
        let (tail, task, name) = (tail.clone(), task.clone(), name.to_string());
        readers.push(tokio::spawn(async move {
            let mut lines = BufReader::new(stream).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                task.update(format!("{name}: {}", line.chars().take(120).collect::<String>()), None);
                let mut t = tail.lock().unwrap();
                t.push_back(line);
                if t.len() > 25 {
                    t.pop_front();
                }
            }
        }));
    }
    let status = child.wait().await?;
    for r in readers {
        let _ = r.await;
    }
    if !status.success() {
        let tail: Vec<String> = tail.lock().unwrap().iter().cloned().collect();
        let last = tail.iter().rev().find(|l| l.to_lowercase().contains("error") || l.to_lowercase().contains("exception")).or(tail.last());
        bail!("The {name} installer failed ({status}){}", last.map(|l| format!(": {l}")).unwrap_or_default());
    }
    Ok(())
}

/// The version-specific loader info packs and instances carry, as the UI shows it.
pub fn describe(meta: &ServerMeta) -> String {
    match (&meta.loader_version, meta.loader.filter(|_| meta.kind.hybrid())) {
        (Some(v), Some(on)) => format!("{} {} ({}, {v})", meta.kind.label(), meta.mc_version, on.label()),
        (Some(v), None) => format!("{} {} ({v})", meta.kind.label(), meta.mc_version),
        (None, _) => format!("{} {}", meta.kind.label(), meta.mc_version),
    }
}

