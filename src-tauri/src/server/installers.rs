//! Installing server software into a server folder: the official vanilla server.jar, Paper / Purpur / Folia
//! from their download APIs, Fabric's server launcher, Quilt's installer, and the Forge / NeoForge installers
//! (`--installServer`). Each fills in `ServerMeta::launch` (how to start it) and the loader/build version.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::content::index::natural_cmp;
use crate::core::events::Task;
use crate::core::net::{download_all, fetch_json, fetch_text, status_of, DownloadItem, Hash};
use crate::core::types::{LoaderType, ServerKind, ServerLaunch, ServerMeta};
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
        _ => return Ok(Vec::new()),
    };
    // Releases only; pre-releases and release candidates are experimental builds.
    list.retain(|v| !v.contains('-'));
    list.sort_by(|a, b| natural_cmp(b, a));
    list.dedup();
    Ok(list)
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
    let (launch, version) = match meta.kind {
        ServerKind::Vanilla => {
            vanilla_jar(&mc, &dir.join("server.jar"), task).await?;
            (jar("server.jar"), None)
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
    meta.loader = meta.kind.loader();
    meta.loader_version = version;
    Ok(())
}

fn jar(name: &str) -> ServerLaunch {
    ServerLaunch { jar: Some(name.to_string()), args_file: None }
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
        return Ok(ServerLaunch { jar: None, args_file: Some(rel) });
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
    match &meta.loader_version {
        Some(v) => format!("{} {} ({v})", meta.kind.label(), meta.mc_version),
        None => format!("{} {}", meta.kind.label(), meta.mc_version),
    }
}

