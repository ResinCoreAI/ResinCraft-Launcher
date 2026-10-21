//! Finding Java installs and downloading Eclipse Temurin.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use serde::Deserialize;

use crate::core::events::Task;
use crate::core::net::{download_file, fetch_json, DownloadItem, Hash};
use crate::core::paths;
use crate::core::types::JavaInstall;
use crate::{bail_tr, err_tr, tr};

const EXE: &str = if cfg!(windows) { "java.exe" } else { "java" };

/// Windows: start child processes without flashing a console window.
#[cfg(windows)]
pub(crate) const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub(crate) fn hidden_command(program: impl AsRef<std::ffi::OsStr>) -> tokio::process::Command {
    #[allow(unused_mut)]
    let mut cmd = tokio::process::Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Walks up to `depth` levels below `root` collecting folders that contain bin/java.
fn find_homes(root: &Path, depth: u32, out: &mut BTreeSet<PathBuf>) {
    if root.join("bin").join(EXE).is_file() {
        out.insert(root.to_path_buf());
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else { return };
    for e in entries.flatten() {
        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            find_homes(&e.path(), depth - 1, out);
        }
    }
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.is_empty())
}

fn home_dir() -> PathBuf {
    env("HOME").or_else(|| env("USERPROFILE")).map(PathBuf::from).unwrap_or_default()
}

fn search_roots() -> Vec<(PathBuf, u32)> {
    let mut roots = vec![(paths::java(), 3)];
    if let Some(h) = env("JAVA_HOME") {
        roots.push((PathBuf::from(h), 0));
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            if dir.as_os_str().is_empty() {
                continue;
            }
            let parent = dir.parent().map(Path::to_path_buf).unwrap_or(dir);
            roots.push((parent, 0));
        }
    }

    if cfg!(windows) {
        let pf: Vec<String> = ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"].iter().filter_map(|n| env(n)).collect();
        for base in pf {
            let base = PathBuf::from(base);
            for vendor in ["Java", "Eclipse Adoptium", "Eclipse Foundation", "Microsoft", "Zulu", "BellSoft", "Amazon Corretto", "Semeru", "AdoptOpenJDK"] {
                roots.push((base.join(vendor), 2));
            }
            roots.push((base.join("Minecraft Launcher").join("runtime"), 4));
        }
        if let Some(local) = env("LOCALAPPDATA") {
            roots.push((PathBuf::from(local).join("Packages").join("Microsoft.4297127D64EC6_8wekyb3d8bbwe").join("LocalCache").join("Local").join("runtime"), 4));
        }
    } else if cfg!(target_os = "macos") {
        roots.push((PathBuf::from("/Library/Java/JavaVirtualMachines"), 3));
        roots.push((home_dir().join("Library/Java/JavaVirtualMachines"), 3));
    } else {
        roots.push((PathBuf::from("/usr/lib/jvm"), 1));
        roots.push((PathBuf::from("/usr/java"), 1));
        roots.push((PathBuf::from("/opt"), 2));
        roots.push((home_dir().join(".sdkman/candidates/java"), 1));
    }
    roots
}

fn parse_major(version: &str) -> u32 {
    let parts: Vec<u32> = version.split(['.', '_', '+', '-']).map(|p| p.parse().unwrap_or(0)).collect();
    if parts.first() == Some(&1) {
        parts.get(1).copied().unwrap_or(0)
    } else {
        parts.first().copied().unwrap_or(0)
    }
}

fn capture(re: &str, text: &str) -> Option<String> {
    regex::Regex::new(re).ok()?.captures(text).map(|c| c[1].to_string())
}

async fn probe(home: &Path, managed: bool) -> Option<JavaInstall> {
    let path = home.join("bin").join(EXE);
    // The "release" file is quick to read; fall back to asking java itself.
    let release = tokio::fs::read_to_string(home.join("release")).await.unwrap_or_default();
    let mut version = capture(r#"JAVA_VERSION="([^"]+)""#, &release);
    let mut arch = capture(r#"OS_ARCH="([^"]+)""#, &release);
    if version.is_none() {
        let mut cmd = hidden_command(&path);
        cmd.arg("-version").stdin(std::process::Stdio::null()).kill_on_drop(true);
        let out = tokio::time::timeout(Duration::from_secs(8), cmd.output()).await.ok()?.ok()?;
        let stderr = String::from_utf8_lossy(&out.stderr);
        version = capture(r#"version "([^"]+)""#, &stderr);
        if arch.is_none() && stderr.contains("64-Bit") {
            arch = Some("x86_64".into());
        }
    }
    let version = version?;
    Some(JavaInstall { path: crate::game::mojang::plain(&path), major: parse_major(&version), version, arch, managed })
}

fn real(p: &Path) -> String {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()).to_string_lossy().into_owned()
}

pub async fn detect_java() -> Result<Vec<JavaInstall>> {
    let homes = tokio::task::spawn_blocking(|| {
        let mut homes = BTreeSet::new();
        for (root, depth) in search_roots() {
            find_homes(&root, depth, &mut homes);
        }
        homes
    })
    .await?;
    let mut unique: HashMap<String, PathBuf> = HashMap::new();
    for h in homes {
        unique.insert(real(&h), h);
    }
    let managed_root = real(&paths::java());
    let probes = unique.into_iter().map(|(r, h)| {
        let managed = r.starts_with(&managed_root);
        async move { probe(&h, managed).await }
    });
    let mut found: Vec<JavaInstall> = futures::future::join_all(probes).await.into_iter().flatten().collect();
    found.sort_by(|a, b| b.major.cmp(&a.major).then_with(|| a.path.cmp(&b.path)));
    Ok(found)
}

/// Minecraft ≤1.16 (Java 8) breaks on newer Java, so 8 needs an exact match; newer versions accept anything ≥ required.
fn suits(j: &JavaInstall, required: u32) -> bool {
    if required <= 8 {
        j.major == 8
    } else {
        j.major >= required
    }
}

/// The CPU this build of the launcher runs on, in Adoptium's names.
pub fn host_cpu() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    }
}

/// Adoptium's name for a `mojang::game_arch` CPU.
pub fn cpu_for(game_arch: &str) -> &'static str {
    if game_arch == "arm64" {
        "aarch64"
    } else {
        "x64"
    }
}

fn is_arm(arch: &str) -> bool {
    arch.contains("aarch64") || arch.contains("arm64")
}

/// A 64-bit Java for `cpu`. On a Mac both kinds can run (x64 through Rosetta 2), so the CPU has to match the
/// game's natives; elsewhere any 64-bit Java does, as before.
fn fits_cpu(j: &JavaInstall, cpu: &str) -> bool {
    match j.arch.as_deref() {
        None => cpu == host_cpu(),
        Some(a) => a.contains("64") && (!cfg!(target_os = "macos") || is_arm(a) == (cpu == "aarch64")),
    }
}

pub async fn find_java(required: u32) -> Result<Option<JavaInstall>> {
    find_java_for(required, host_cpu()).await
}

/// A Java for `required` that runs as `cpu` ("x64" or "aarch64").
pub async fn find_java_for(required: u32, cpu: &str) -> Result<Option<JavaInstall>> {
    let all: Vec<JavaInstall> = detect_java().await?.into_iter().filter(|j| fits_cpu(j, cpu)).collect();
    if let Some(j) = all.iter().find(|j| j.major == required) {
        return Ok(Some(j.clone()));
    }
    let mut ok: Vec<JavaInstall> = all.into_iter().filter(|j| suits(j, required)).collect();
    ok.sort_by_key(|j| j.major);
    Ok(ok.into_iter().next())
}

#[derive(Deserialize)]
struct AdoptiumPackage {
    link: String,
    checksum: String,
    name: String,
    size: u64,
}
#[derive(Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}
#[derive(Deserialize)]
struct AdoptiumAsset {
    binary: AdoptiumBinary,
}

/// macOS and Linux: makes sure the programs in `<home>/bin` can run (tar keeps the bits, but a copy may not).
async fn make_runnable(home: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let Ok(mut rd) = tokio::fs::read_dir(home.join("bin")).await else { return };
        while let Ok(Some(e)) = rd.next_entry().await {
            if let Ok(meta) = e.metadata().await {
                if meta.is_file() && meta.permissions().mode() & 0o111 != 0o111 {
                    let _ = tokio::fs::set_permissions(e.path(), std::fs::Permissions::from_mode(meta.permissions().mode() | 0o755)).await;
                }
            }
        }
    }
    #[cfg(not(unix))]
    let _ = home;
}

/// Downloads an Eclipse Temurin JRE for this computer's CPU into the launcher's own Java folder.
pub async fn install_java(required: u32, task: &Arc<Task>) -> Result<JavaInstall> {
    install_java_for(required, host_cpu(), task).await
}

/// Downloads an Eclipse Temurin JRE that runs as `cpu` ("x64" or "aarch64").
pub async fn install_java_for(required: u32, cpu: &str, task: &Arc<Task>) -> Result<JavaInstall> {
    // Temurin dropped its Java 16 builds; 17 runs Minecraft 1.17 fine.
    let major = if required == 16 { 17 } else { required };
    let os = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    };
    task.update(tr!("java.finding", "Finding Java {major}…", major = major), None);
    let assets: Vec<AdoptiumAsset> =
        fetch_json(&format!("https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture={cpu}&image_type=jre&os={os}&vendor=eclipse")).await?;
    let pkg = assets.into_iter().next().map(|a| a.binary.package).ok_or_else(|| err_tr!("java.noDownload", "No Java {major} download is available for this computer", major = major))?;

    let archive = crate::game::mojang::join_rel(&paths::cache(), &pkg.name);
    task.update(tr!("java.downloading", "Downloading Java {major}…", major = major), None);
    download_file(&DownloadItem::new(pkg.link.clone(), &archive).hash(Some(Hash::sha256(pkg.checksum.clone()))).size(Some(pkg.size))).await?;

    task.update(tr!("java.unpacking", "Unpacking Java {major}…", major = major), None);
    // Java for the other CPU (x64 on an Apple Silicon Mac) gets its own folder next to the native one.
    let suffix = if cpu == host_cpu() { String::new() } else { format!("-{cpu}") };
    let staging = paths::java().join(format!(".staging-{major}{suffix}"));
    let target = paths::java().join(format!("java-{major}{suffix}"));
    let _ = tokio::fs::remove_dir_all(&staging).await;
    tokio::fs::create_dir_all(&staging).await?;
    if pkg.name.ends_with(".zip") {
        let (a, s) = (archive.clone(), staging.clone());
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut zip = zip::ZipArchive::new(std::fs::File::open(&a)?)?;
            zip.extract(&s)?;
            Ok(())
        })
        .await??;
    } else {
        let status = hidden_command("tar").arg("-xzf").arg(&archive).arg("-C").arg(&staging).status().await?;
        if !status.success() {
            bail_tr!("java.tarFailed", "Couldn't unpack Java {major} (tar exited with {status})", major = major, status = status.to_string());
        }
    }
    let mut top = None;
    let mut rd = tokio::fs::read_dir(&staging).await?;
    if let Some(e) = rd.next_entry().await? {
        top = Some(e.path());
    }
    let top = top.ok_or_else(|| err_tr!("java.emptyDownload", "The Java {major} download was empty", major = major))?;
    let _ = tokio::fs::remove_dir_all(&target).await;
    tokio::fs::rename(&top, &target).await?;
    let _ = tokio::fs::remove_dir_all(&staging).await;
    let _ = tokio::fs::remove_file(&archive).await;

    let t = target.clone();
    let homes = tokio::task::spawn_blocking(move || {
        let mut homes = BTreeSet::new();
        find_homes(&t, 3, &mut homes);
        homes
    })
    .await?;
    let installed = match homes.into_iter().next() {
        Some(h) => {
            make_runnable(&h).await;
            probe(&h, true).await
        }
        None => None,
    };
    installed.ok_or_else(|| err_tr!("java.wontStart", "Java {major} was downloaded but couldn't be started", major = major))
}
