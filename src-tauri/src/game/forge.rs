//! Forge & NeoForge installation: installer download, install_profile.json + version.json, processors run
//! with Java, and a cached merged version JSON.

use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::fsutil::{read_json, safe_join, write_json};
use crate::core::net::{download_all, download_file, hash_file, status_of, DownloadItem, HashAlgo};
use crate::core::paths;
use crate::core::text::Text;
use crate::core::types::LoaderType;
use crate::{bail_tr, err_tr, tr};

use super::java::{find_java, hidden_command};
use super::loaders::{full_forge_version, installer_url, is_ancient_forge, legacy_forge_error, ResolveContext};
use super::mojang::{client_jar, exists, get_vanilla_version, lib_path, maven_path, merge_versions, resolve_libraries, resolve_library_list, FileRef, Library, VersionJson};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Processor {
    sides: Option<Vec<String>>,
    jar: String,
    classpath: Option<Vec<String>>,
    args: Option<Vec<String>>,
    outputs: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct DataEntry {
    client: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct InstallProfile {
    json: Option<String>,
    data: Option<BTreeMap<String, DataEntry>>,
    processors: Option<Vec<Processor>>,
    libraries: Option<Vec<Library>>,
    // Present only in the pre-1.13 installer format.
    install: Option<Value>,
    version_info: Option<Value>,
}

/// What we keep after installing, so later launches skip the installer entirely.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstalledLoader {
    version: VersionJson,
    /// Files produced by the installer that must still exist for the cache to be valid.
    files: Vec<String>,
}

fn path_str(p: &Path) -> String {
    super::mojang::plain(p)
}

pub async fn resolve_forge_like(mc: &str, loader: LoaderType, ver: &str, ctx: &ResolveContext) -> Result<VersionJson> {
    if loader == LoaderType::Forge && is_ancient_forge(mc) {
        return Err(legacy_forge_error(mc));
    }
    let full = if loader == LoaderType::Forge { full_forge_version(mc, ver).await } else { ver.to_string() };
    let ver = full.as_str();
    let dir = paths::versions().join(format!("{}-{mc}-{ver}", loader.as_str()));
    let cache_file = dir.join("installed.json");
    if let Some(cached) = read_json::<InstalledLoader>(&cache_file).await {
        let mut all = true;
        for f in &cached.files {
            if !exists(Path::new(f)).await {
                all = false;
                break;
            }
        }
        if all {
            return Ok(cached.version);
        }
    }

    let installed = install_forge_like(mc, loader, ver, &dir, ctx).await?;
    write_json(&cache_file, &installed).await?;
    Ok(installed.version)
}

fn update(ctx: &ResolveContext, detail: impl Into<Text>, progress: Option<f64>) {
    if let Some(t) = &ctx.task {
        t.update(detail, progress);
    }
}

async fn install_forge_like(mc: &str, loader: LoaderType, ver: &str, dir: &Path, ctx: &ResolveContext) -> Result<InstalledLoader> {
    let name = format!("{} {ver}", loader.label());
    update(ctx, tr!("forge.downloadingInstaller", "Downloading the {name} installer…", name = name), None);
    let installer = dir.join("installer.jar");
    if let Err(e) = download_file(&DownloadItem::new(installer_url(loader, mc, ver), &installer)).await {
        if status_of(&e) == Some(404) {
            bail_tr!("forge.notFound", "{name} for Minecraft {mc} couldn't be found", name = name, mc = mc);
        }
        return Err(e);
    }
    let work = dir.join("installer-data");
    let result = install_from(mc, &name, &installer, &work, ctx).await;
    let _ = tokio::fs::remove_dir_all(&work).await;
    let _ = tokio::fs::remove_file(&installer).await;
    result
}

/// Runs blocking zip work off the async runtime.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f).await?
}

fn open_zip(path: &Path) -> Result<zip::ZipArchive<std::fs::File>> {
    Ok(zip::ZipArchive::new(std::fs::File::open(path)?)?)
}

fn read_entry(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<Vec<u8>> {
    let mut f = zip.by_name(name).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    Some(buf)
}

async fn install_from(mc: &str, name: &str, installer: &Path, work: &Path, ctx: &ResolveContext) -> Result<InstalledLoader> {
    let concurrency = ctx.concurrency.unwrap_or(8);

    let (profile, child) = {
        let (installer, name) = (installer.to_path_buf(), name.to_string());
        blocking(move || -> Result<(InstallProfile, Option<VersionJson>)> {
            let mut zip = open_zip(&installer)?;
            let text = read_entry(&mut zip, "install_profile.json").filter(|t| !t.is_empty()).ok_or_else(|| err_tr!("forge.installerMissing", "The {name} installer is missing {file}", name = name, file = "install_profile.json"))?;
            let profile: InstallProfile = serde_json::from_slice(&text)?;
            if profile.install.is_some() && profile.version_info.is_some() {
                return Ok((profile, None));
            }
            let json = profile.json.clone().filter(|j| !j.is_empty()).ok_or_else(|| err_tr!("forge.installerMissing", "The {name} installer is missing {file}", name = name, file = "version.json"))?;
            let text = read_entry(&mut zip, json.trim_start_matches('/')).filter(|t| !t.is_empty()).ok_or_else(|| err_tr!("forge.installerMissing", "The {name} installer is missing {file}", name = name, file = json))?;
            Ok((profile, Some(serde_json::from_slice(&text)?)))
        })
        .await?
    };
    let Some(child) = child else {
        return install_legacy(mc, name, installer, profile, ctx).await;
    };

    let vanilla = get_vanilla_version(mc).await?;
    let client = client_jar(&vanilla)?;

    // Some libraries (the loader's own jars) ship inside the installer instead of on a Maven server.
    update(ctx, tr!("forge.unpacking", "Unpacking {name}…", name = name), None);
    {
        let installer = installer.to_path_buf();
        blocking(move || -> Result<()> {
            let mut zip = open_zip(&installer)?;
            let libs = paths::libraries();
            for i in 0..zip.len() {
                let mut entry = zip.by_index(i)?;
                let entry_name = entry.name().to_string();
                if entry.is_dir() || !entry_name.starts_with("maven/") {
                    continue;
                }
                let dest = safe_join(&libs, &entry_name["maven/".len()..])?;
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf)?;
                std::fs::write(&dest, buf)?;
            }
            Ok(())
        })
        .await?;
    }

    // Installer tools need several versions of the same artifact side by side (one per tool), so skip de-duplication there.
    let installer_libs: Vec<DownloadItem> =
        profile.libraries.iter().flatten().flat_map(|lib| resolve_library_list(std::slice::from_ref(lib), true)).map(|l| l.item).collect();
    let game_libs = resolve_libraries(&child);
    let mut items = vec![client.clone()];
    items.extend(installer_libs);
    items.extend(game_libs.iter().map(|l| l.item.clone()));
    items.retain(|i| !i.url.is_empty());
    let progress: Box<dyn Fn(usize, usize) + Send + Sync> = match &ctx.task {
        Some(t) => Box::new(t.counter(tr!("forge.downloadingLibraries", "Downloading {name} libraries", name = name))),
        None => Box::new(|_, _| {}),
    };
    download_all(items, concurrency, progress).await?;

    // Values for the {TOKENS} in processor arguments.
    let mut data: HashMap<String, String> = HashMap::from([
        ("SIDE".into(), "client".into()),
        ("MINECRAFT_JAR".into(), path_str(&client.dest)),
        ("MINECRAFT_VERSION".into(), mc.to_string()),
        ("ROOT".into(), path_str(&paths::games())),
        ("INSTALLER".into(), path_str(installer)),
        ("LIBRARY_DIR".into(), path_str(&paths::libraries())),
    ]);
    let mut bundled: Vec<(String, String, PathBuf)> = Vec::new();
    for (key, value) in profile.data.iter().flatten() {
        let v = &value.client;
        if v.len() >= 2 && v.starts_with('[') && v.ends_with(']') {
            data.insert(key.clone(), path_str(&lib_path(&v[1..v.len() - 1])));
        } else if v.len() >= 2 && v.starts_with('\'') && v.ends_with('\'') {
            data.insert(key.clone(), v[1..v.len() - 1].to_string());
        } else {
            // A file bundled inside the installer, e.g. /data/client.lzma.
            let rel = v.trim_start_matches('/').to_string();
            let dest = safe_join(work, &rel)?;
            data.insert(key.clone(), path_str(&dest));
            bundled.push((v.clone(), rel, dest));
        }
    }
    if !bundled.is_empty() {
        let (installer, name) = (installer.to_path_buf(), name.to_string());
        blocking(move || -> Result<()> {
            let mut zip = open_zip(&installer)?;
            for (v, rel, dest) in bundled {
                let bytes = read_entry(&mut zip, &rel).ok_or_else(|| err_tr!("forge.installerMissing", "The {name} installer is missing {file}", name = name, file = v))?;
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&dest, bytes)?;
            }
            Ok(())
        })
        .await?;
    }

    let token = regex::Regex::new(r"\{([^}]+)\}").unwrap();
    let sub = |arg: &str| -> Result<String> {
        if arg.len() >= 2 && arg.starts_with('[') && arg.ends_with(']') {
            return Ok(path_str(&lib_path(&arg[1..arg.len() - 1])));
        }
        let mut out = String::new();
        let mut last = 0;
        for c in token.captures_iter(arg) {
            let m = c.get(0).unwrap();
            let key = &c[1];
            let val = data.get(key).ok_or_else(|| err_tr!("forge.unknownValue", "The {name} installer refers to an unknown value {value}", name = name, value = format!("{{{key}}}")))?;
            out.push_str(&arg[last..m.start()]);
            out.push_str(val);
            last = m.end();
        }
        out.push_str(&arg[last..]);
        Ok(out)
    };

    let processors: Vec<&Processor> =
        profile.processors.iter().flatten().filter(|p| p.sides.as_ref().map(|s| s.iter().any(|x| x == "client")).unwrap_or(true)).collect();
    let java = if processors.is_empty() { String::new() } else { pick_java(vanilla.java_major(), ctx).await? };
    let n = processors.len();
    for (i, p) in processors.iter().enumerate() {
        update(ctx, tr!("forge.installingStep", "Installing {name} (step {step} of {total})…", name = name, step = i + 1, total = n), Some(i as f64 / n as f64));
        let mut outputs = Vec::new();
        for (file, sha1) in p.outputs.iter().flatten() {
            outputs.push((PathBuf::from(sub(file)?), sub(sha1)?));
        }
        if !outputs.is_empty() && outputs_valid(&outputs).await {
            continue;
        }

        let jar = lib_path(&p.jar);
        let mut classpath = vec![jar.clone()];
        for c in p.classpath.iter().flatten() {
            let cp = lib_path(c);
            if !classpath.contains(&cp) {
                classpath.push(cp);
            }
        }
        let args = p.args.iter().flatten().map(|a| sub(a)).collect::<Result<Vec<_>>>()?;
        let main = main_class_of(&jar).await?;
        run_processor(&java, &classpath, &main, &args, name).await?;
        if !outputs.is_empty() && !outputs_valid(&outputs).await {
            bail_tr!("forge.unexpectedFiles", "{name} installation produced unexpected files. Try again, or pick another version.", name = name);
        }
    }

    // Jars with no download URL come from the installer itself or from the processors above.
    let local: Vec<PathBuf> = game_libs.iter().filter(|l| l.item.url.is_empty()).map(|l| l.item.dest.clone()).collect();
    for f in &local {
        if !exists(f).await {
            bail_tr!("forge.fileNotProduced", "{name} installation didn't produce {file}", name = name, file = f.display().to_string());
        }
    }
    let mut patched = child.clone();
    for lib in &mut patched.libraries {
        let Some(downloads) = lib.downloads.as_mut() else { continue };
        if let Some(art) = downloads.artifact.as_mut() {
            // Locally built jars can't match the published checksum, so don't verify them at launch.
            if art.url.is_empty() {
                *art = FileRef { url: String::new(), path: art.path.clone(), ..Default::default() };
            }
        }
    }
    let merged = merge_versions(&vanilla, &patched);

    let libs = path_str(&paths::libraries());
    let mut files: Vec<String> = local.iter().map(|p| path_str(p)).collect();
    let mut values: Vec<&String> = data.values().collect();
    values.sort();
    for p in values {
        if p.starts_with(&libs) && exists(Path::new(p)).await {
            files.push(p.clone());
        }
    }
    let mut seen = std::collections::HashSet::new();
    files.retain(|f| seen.insert(f.clone()));
    Ok(InstalledLoader { version: merged, files })
}

// ---------- Forge for Minecraft 1.7.10 – 1.12.2 (installer with install + versionInfo) ----------

/// Where old Forge profiles point (some of these hosts are gone), in the order tried.
const LEGACY_MAVENS: [&str; 3] = ["https://maven.minecraftforge.net/", "https://libraries.minecraft.net/", "https://repo1.maven.org/maven2/"];

/// Maven bases worth trying for one library: its own (moved hosts rewritten), then the usual ones.
fn legacy_lib_bases(url: Option<&str>) -> Vec<String> {
    let own = match url.map(str::trim).filter(|u| !u.is_empty()) {
        Some(u) => {
            let mut u = u.replace("http://", "https://");
            for old in ["https://files.minecraftforge.net/maven/", "https://files.minecraftforge.net/", "https://maven.minecraftforge.net/maven/"] {
                if u.starts_with(old) {
                    u = format!("https://maven.minecraftforge.net/{}", &u[old.len()..]);
                }
            }
            if !u.ends_with('/') {
                u.push('/');
            }
            u
        }
        None => LEGACY_MAVENS[1].to_string(),
    };
    let mut out = vec![own];
    for m in LEGACY_MAVENS {
        if !out.iter().any(|o| o == m) {
            out.push(m.to_string());
        }
    }
    out
}

/// The old format needs no Java: the Forge jar is inside the installer and the rest are plain libraries.
async fn install_legacy(mc: &str, name: &str, installer: &Path, profile: InstallProfile, ctx: &ResolveContext) -> Result<InstalledLoader> {
    let install = profile.install.unwrap_or_default();
    let mut child: VersionJson = serde_json::from_value(profile.version_info.unwrap_or_default())?;
    let forge_lib = install["path"].as_str().unwrap_or_default().to_string();
    let file_path = install["filePath"].as_str().unwrap_or_default().to_string();
    if forge_lib.is_empty() || file_path.is_empty() {
        bail_tr!("forge.installerMissing", "The {name} installer is missing {file}", name = name, file = "install.path");
    }

    update(ctx, tr!("forge.unpacking", "Unpacking {name}…", name = name), None);
    let forge_jar = lib_path(&forge_lib);
    {
        let (installer, dest, file_path, name) = (installer.to_path_buf(), forge_jar.clone(), file_path.clone(), name.to_string());
        blocking(move || -> Result<()> {
            let mut zip = open_zip(&installer)?;
            let bytes = read_entry(&mut zip, &file_path).ok_or_else(|| err_tr!("forge.installerMissing", "The {name} installer is missing {file}", name = name, file = file_path))?;
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&dest, bytes)?;
            Ok(())
        })
        .await?;
    }

    let vanilla = get_vanilla_version(mc).await?;
    let client = client_jar(&vanilla)?;
    let concurrency = ctx.concurrency.unwrap_or(8);
    let progress: Box<dyn Fn(usize, usize) + Send + Sync> = match &ctx.task {
        Some(t) => Box::new(t.counter(tr!("forge.downloadingLibraries", "Downloading {name} libraries", name = name))),
        None => Box::new(|_, _| {}),
    };
    download_all(vec![client], 1, |_, _| {}).await?;

    // Each library: the first host that has it. Server-only extras that nobody has any more are dropped.
    let libs = std::mem::take(&mut child.libraries);
    let total = libs.len();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let results = futures::stream::iter(libs.into_iter().map(|lib| {
        let (forge_lib, done, progress) = (&forge_lib, &done, &progress);
        async move {
            let r = fetch_legacy_lib(lib, forge_lib).await;
            progress(done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1, total);
            r
        }
    }))
    .buffered(concurrency)
    .collect::<Vec<_>>()
    .await;
    for r in results {
        if let Some(lib) = r? {
            child.libraries.push(lib);
        }
    }
    Ok(InstalledLoader { version: merge_versions(&vanilla, &child), files: vec![path_str(&forge_jar)] })
}

async fn fetch_legacy_lib(mut lib: Library, forge_lib: &str) -> Result<Option<Library>> {
    let rel = maven_path(&lib.name);
    if lib.name == forge_lib {
        // Unpacked from the installer above; nothing to download.
        lib.url = None;
        lib.downloads = Some(super::mojang::LibDownloads { artifact: Some(FileRef { url: String::new(), path: Some(rel), ..Default::default() }), classifiers: None });
        return Ok(Some(lib));
    }
    let client = lib.extra.get("clientreq").and_then(Value::as_bool);
    let server_only = client != Some(true) && lib.extra.get("serverreq").and_then(Value::as_bool) == Some(true);
    let dest = lib_path(&lib.name);
    let bases = legacy_lib_bases(lib.url.as_deref());
    if exists(&dest).await {
        lib.url = bases.into_iter().next();
        return Ok(Some(lib));
    }
    let mut last = None;
    for base in bases {
        match download_file(&DownloadItem::new(format!("{base}{rel}"), &dest)).await {
            Ok(()) => {
                lib.url = Some(base);
                return Ok(Some(lib));
            }
            Err(e) => last = Some(e),
        }
    }
    if server_only {
        return Ok(None);
    }
    Err(last.unwrap_or_else(|| anyhow!("{} couldn't be downloaded", lib.name)))
}

async fn pick_java(major: u32, ctx: &ResolveContext) -> Result<String> {
    if let Some(java) = &ctx.java {
        return java(major).await;
    }
    match find_java(major).await? {
        Some(j) => Ok(j.path),
        None => bail_tr!("forge.needJava", "Installing this mod loader needs Java {major}", major = major),
    }
}

async fn outputs_valid(outputs: &[(PathBuf, String)]) -> bool {
    for (file, sha1) in outputs {
        if !exists(file).await {
            return false;
        }
        match hash_file(file, HashAlgo::Sha1).await {
            Ok(h) if h == sha1.to_lowercase() => {}
            _ => return false,
        }
    }
    true
}

async fn main_class_of(jar: &Path) -> Result<String> {
    let jar = jar.to_path_buf();
    blocking(move || {
        let mut zip = open_zip(&jar).map_err(|e| err_tr!("shell.openFailed", "Couldn't open {path}: {error}", path = jar.display().to_string(), error = e.to_string()))?;
        let manifest = String::from_utf8_lossy(&read_entry(&mut zip, "META-INF/MANIFEST.MF").unwrap_or_default()).into_owned();
        // Manifest lines wrap at 72 bytes; continuation lines start with a space.
        let unfolded = manifest.replace("\r\n ", "").replace("\n ", "");
        let re = regex::Regex::new(r"(?m)^Main-Class:\s*(.+)$").unwrap();
        re.captures(&unfolded).map(|c| c[1].trim().to_string()).filter(|m| !m.is_empty()).ok_or_else(|| err_tr!("forge.noMainClass", "{jar} has no Main-Class", jar = jar.display().to_string()))
    })
    .await
}

async fn run_processor(java: &str, classpath: &[PathBuf], main_class: &str, args: &[String], name: &str) -> Result<()> {
    let cp = std::env::join_paths(classpath.iter().map(|p| path_str(p))).map_err(|e| anyhow!("Bad classpath: {e}"))?;
    let mut cmd = hidden_command(java);
    cmd.arg("-cp").arg(cp).arg(main_class).args(args).stdin(std::process::Stdio::null()).kill_on_drop(true);
    let out = cmd.output().await.map_err(|e| err_tr!("forge.javaRunFailed", "Couldn't run Java ({java}) to install {name}: {error}", java = java, name = name, error = e.to_string()))?;
    if out.status.success() {
        return Ok(());
    }
    let text = format!("{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
    let tail = lines[lines.len().saturating_sub(15)..].join("\n");
    let code = out.status.code().map(|c| c.to_string()).unwrap_or_else(|| "null".into());
    bail_tr!("forge.processorFailed", "Installing {name} failed ({main_class} exited with code {code}):\n{tail}", name = name, main_class = main_class, code = code, tail = tail)
}
