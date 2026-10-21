//! Jar mods: zips merged into a copy of the game jar (`META-INF` left out, so Mojang's signatures don't reject
//! the changed classes), the way mods were installed before mod loaders. They come from the player (instance
//! settings → Version), from Forge for Minecraft 1.5.2 and older, and from old Technic packs' bin/modpack.jar.
//! Forge 1.3.2–1.5.2 also downloads a few libraries into `<game dir>/lib` at startup from a server that's gone;
//! [`ensure_fml_libs`] puts them there first.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::events::Task;
use crate::core::fsutil::{read_json, write_json};
use crate::core::net::{download_file, hash_file, DownloadItem, Hash, HashAlgo};
use crate::core::types::{Instance, JarMod};
use crate::core::{instances, paths, shell};
use crate::{bail_tr, err_tr, tr};

use super::mojang::{exists, plain, Argument, VersionJson};

/// Where a loader's version JSON lists its jar mods (absolute paths), like Prism's `jarMods`.
const LOADER_JAR_MODS: &str = "jarMods";
/// FML's library downloader; present in Forge 1.3.2–1.5.2.
const FML_RELAUNCHER: &str = "cpw/mods/fml/relauncher/RelaunchLibraryManager.class";

pub(crate) fn set_loader_jar_mods(v: &mut VersionJson, files: &[PathBuf]) {
    v.extra.insert(LOADER_JAR_MODS.into(), Value::Array(files.iter().map(|f| Value::String(plain(f))).collect()));
}

pub(crate) fn loader_jar_mods(v: &VersionJson) -> Vec<PathBuf> {
    v.extra.get(LOADER_JAR_MODS).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(PathBuf::from).collect()
}

/// JVM flags old games need when they run Forge or jar mods:
/// - FML refuses a game jar without Mojang's signature, and today's Java counts the 2013 signatures as missing
///   (weak algorithm); a patched jar has none at all. Prism passes the same two flags.
/// - Before 1.6, Forge relaunches the game before launchwrapper has set the game's folder, so tell it directly;
///   otherwise it uses %APPDATA%\.minecraft.
pub(crate) fn old_game_flags(v: &mut VersionJson, inst: &Instance) {
    use super::loaders::{is_ancient_forge, is_jar_mod_forge};
    let has_mods = !loader_jar_mods(v).is_empty() || inst.jar_mods.iter().any(|m| !m.disabled);
    let old_forge = inst.loader == crate::core::types::LoaderType::Forge && is_ancient_forge(&inst.mc_version);
    let mut flags = Vec::new();
    if has_mods || old_forge {
        flags.extend(["-Dfml.ignoreInvalidMinecraftCertificates=true", "-Dfml.ignorePatchDiscrepancies=true"]);
    }
    if has_mods && is_jar_mod_forge(&inst.mc_version) {
        flags.push("-Dminecraft.applet.TargetDirectory=${game_directory}");
    }
    if !flags.is_empty() {
        v.arguments.get_or_insert_with(Default::default).jvm.extend(flags.into_iter().map(|f| Argument::Plain(f.into())));
    }
}

/// The instance's own jar mods live here.
pub fn jar_dir(id: &str) -> PathBuf {
    paths::instance_dir(id).join("jarmods")
}

// ---------- The player's jar mods ----------

fn is_zip(path: &Path) -> bool {
    std::fs::File::open(path).ok().and_then(|f| zip::ZipArchive::new(f).ok()).is_some_and(|z| !z.is_empty())
}

/// `name`, or "name (2).ext" … when a file of that name is already there.
fn free_name(dir: &Path, name: &str) -> String {
    if !dir.join(name).exists() {
        return name.to_string();
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) => (s, format!(".{e}")),
        None => (name, String::new()),
    };
    (2..).map(|n| format!("{stem} ({n}){ext}")).find(|n| !dir.join(n).exists()).unwrap_or_else(|| name.to_string())
}

/// Copies zips/jars into the instance's jarmods/ folder and adds them at the end of its list.
pub async fn add(id: &str, files: &[PathBuf]) -> Result<Instance> {
    let mut inst = instances::get(id).await?;
    let dir = jar_dir(id);
    tokio::fs::create_dir_all(&dir).await?;
    for src in files {
        let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let check = src.clone();
        if name.is_empty() || !tokio::task::spawn_blocking(move || is_zip(&check)).await? {
            bail_tr!("jarmods.notZip", "{file} isn't a zip or jar file, so it can't be added as a jar mod.", file = name);
        }
        let file = free_name(&dir, &name);
        tokio::fs::copy(src, dir.join(&file)).await?;
        inst.jar_mods.push(JarMod { file, name, disabled: false });
    }
    instances::save(&inst).await?;
    Ok(inst)
}

/// Takes a jar mod out of the instance; its file goes to the Recycle Bin.
pub async fn remove(id: &str, file: &str) -> Result<Instance> {
    let mut inst = instances::get(id).await?;
    let path = jar_dir(id).join(crate::content::index::safe_name(file)?);
    if exists(&path).await {
        shell::trash(&path).await?;
    }
    inst.jar_mods.retain(|m| m.file != file);
    instances::save(&inst).await?;
    Ok(inst)
}

/// Saves a new order and on/off state; the list must hold the same files as before.
pub async fn set(id: &str, list: Vec<JarMod>) -> Result<Instance> {
    let mut inst = instances::get(id).await?;
    let mut before: Vec<&str> = inst.jar_mods.iter().map(|m| m.file.as_str()).collect();
    let mut after: Vec<&str> = list.iter().map(|m| m.file.as_str()).collect();
    before.sort_unstable();
    after.sort_unstable();
    if before != after {
        bail_tr!("jarmods.changed", "The jar mods changed in the meantime. Reopen the instance's settings and try again.");
    }
    inst.jar_mods = list;
    instances::save(&inst).await?;
    Ok(inst)
}

// ---------- The patched game jar ----------

#[derive(Serialize, Deserialize, Default)]
struct PatchedInfo {
    key: String,
    /// The result contains FML's library downloader (see `ensure_fml_libs`).
    fml: bool,
}

/// Size and modification time of each input, so the jar is rebuilt only when one of them changes.
async fn stamp(files: &[PathBuf]) -> String {
    let mut out = String::new();
    for f in files {
        let (len, ms) = match tokio::fs::metadata(f).await {
            Ok(m) => (m.len(), m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis()).unwrap_or(0)),
            Err(_) => (0, 0),
        };
        out.push_str(&format!("{}|{len}|{ms}\n", plain(f)));
    }
    crate::core::net::hash_bytes(out.as_bytes(), HashAlgo::Sha1)
}

/// Writes `out`: every jar mod over `client`, the last mod winning, without anyone's `META-INF`.
/// Returns whether the result contains FML's library downloader.
fn merge(client: &Path, mods: &[PathBuf], out: &Path) -> Result<bool> {
    let tmp = out.with_extension("jar.part");
    let mut w = zip::ZipWriter::new(std::fs::File::create(&tmp)?);
    let mut seen = HashSet::new();
    for src in mods.iter().rev().map(PathBuf::as_path).chain(std::iter::once(client)) {
        let file = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mut zip = std::fs::File::open(src)
            .map_err(anyhow::Error::from)
            .and_then(|f| Ok(zip::ZipArchive::new(f)?))
            .map_err(|_| err_tr!("jarmods.notZip", "{file} isn't a zip or jar file, so it can't be added as a jar mod.", file = file.clone()))?;
        for i in 0..zip.len() {
            let entry = zip.by_index_raw(i)?;
            // Some very old zips were made on Windows with backslashes.
            let name = entry.name().replace('\\', "/");
            if name.to_ascii_uppercase().starts_with("META-INF/") || !seen.insert(name.clone()) {
                continue;
            }
            w.raw_copy_file_rename(entry, name)?;
        }
    }
    w.finish()?;
    std::fs::rename(&tmp, out)?;
    Ok(seen.contains(FML_RELAUNCHER))
}

/// The game jar to start: `client` itself, or (when the loader or the instance has jar mods) a per-instance
/// copy with them merged in. Also lays out FML's libraries when the merged jar needs them.
pub async fn client_jar_for(inst: &Instance, version: &VersionJson, client: &Path, task: &Arc<Task>) -> Result<PathBuf> {
    let mut mods = loader_jar_mods(version);
    mods.extend(inst.jar_mods.iter().filter(|m| !m.disabled).map(|m| jar_dir(&inst.id).join(&m.file)));
    if mods.is_empty() {
        return Ok(client.to_path_buf());
    }
    for m in &mods {
        if !exists(m).await {
            let file = m.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            bail_tr!("jarmods.missing", "The jar mod {file} is missing. Remove it in the instance's settings, or reinstall the mod loader.", file = file);
        }
    }
    let dir = paths::instance_dir(&inst.id).join("patched");
    let jar = dir.join("minecraft.jar");
    let info_file = dir.join("patched.json");
    let mut inputs = vec![client.to_path_buf()];
    inputs.extend(mods.iter().cloned());
    let key = stamp(&inputs).await;
    let fml = match read_json::<PatchedInfo>(&info_file).await {
        Some(info) if info.key == key && exists(&jar).await => info.fml,
        _ => {
            task.update(tr!("jarmods.building", "Adding jar mods to the game…"), None);
            tokio::fs::create_dir_all(&dir).await?;
            let (client, out) = (client.to_path_buf(), jar.clone());
            let fml = tokio::task::spawn_blocking(move || merge(&client, &mods, &out)).await??;
            write_json(&info_file, &PatchedInfo { key, fml }).await?;
            fml
        }
    };
    if fml {
        ensure_fml_libs(&inst.mc_version, &paths::game_dir(&inst.id), task).await?;
    }
    Ok(jar)
}

// ---------- FML's runtime libraries (Forge 1.3.2–1.5.2) ----------

struct FmlLib {
    file: &'static str,
    sha1: &'static str,
    /// Path on Maven Central, for the ones published there byte for byte.
    maven: Option<&'static str>,
}

const fn lib(file: &'static str, sha1: &'static str, maven: Option<&'static str>) -> FmlLib {
    FmlLib { file, sha1, maven }
}

const ARGO: FmlLib = lib("argo-2.25.jar", "bb672829fde76cb163004752b86b0484bd0a7f4b", Some("net/sourceforge/argo/argo/2.25/argo-2.25.jar"));
const GUAVA_12: FmlLib = lib("guava-12.0.1.jar", "b8e78b9af7bf45900e14c6f958486b6ca682195f", Some("com/google/guava/guava/12.0.1/guava-12.0.1.jar"));
const ASM_40: FmlLib = lib("asm-all-4.0.jar", "98308890597acb64047f7e896638e0d98753ae82", None);
const BCPROV_147: FmlLib = lib("bcprov-jdk15on-147.jar", "b6f5d9926b0afbde9f4dbe3db88c5247be7794bb", Some("org/bouncycastle/bcprov-jdk15on/1.47/bcprov-jdk15on-1.47.jar"));
const ARGO_SMALL: FmlLib = lib("argo-small-3.2.jar", "58912ea2858d168c50781f956fa5b59f0f7c6b51", None);
const GUAVA_14: FmlLib = lib("guava-14.0-rc3.jar", "931ae21fa8014c3ce686aaa621eae565fefb1a6a", Some("com/google/guava/guava/14.0-rc3/guava-14.0-rc3.jar"));
const ASM_41: FmlLib = lib("asm-all-4.1.jar", "054986e962b88d8660ae4566475658469595ef58", Some("org/ow2/asm/asm-all/4.1/asm-all-4.1.jar"));
const BCPROV_148: FmlLib = lib("bcprov-jdk15on-148.jar", "960dea7c9181ba0b17e8bab0c06a43f0a5f04e65", Some("org/bouncycastle/bcprov-jdk15on/1.48/bcprov-jdk15on-1.48.jar"));
const SCALA: FmlLib = lib("scala-library.jar", "458d046151ad179c85429ed7420ffb1eaf6ddf85", None);

/// What FML expects in lib/ for each Minecraft version (FML's own list; Prism keeps the same one).
fn fml_libs(mc: &str) -> Vec<&'static FmlLib> {
    const DEOBF_15: FmlLib = lib("deobfuscation_data_1.5.zip", "5f7c142d53776f16304c0bbe10542014abad6af8", None);
    const DEOBF_151: FmlLib = lib("deobfuscation_data_1.5.1.zip", "22e221a0d89516c1f721d6cab056a7e37471d0a6", None);
    const DEOBF_152: FmlLib = lib("deobfuscation_data_1.5.2.zip", "446e55cd986582c70fcf12cb27bc00114c5adfd9", None);
    match mc {
        "1.3.2" => vec![&ARGO, &GUAVA_12, &ASM_40],
        "1.4" | "1.4.1" | "1.4.2" | "1.4.3" | "1.4.4" | "1.4.5" | "1.4.6" | "1.4.7" => vec![&ARGO, &GUAVA_12, &ASM_40, &BCPROV_147],
        "1.5" => vec![&ARGO_SMALL, &GUAVA_14, &ASM_41, &BCPROV_148, &DEOBF_15, &SCALA],
        "1.5.1" => vec![&ARGO_SMALL, &GUAVA_14, &ASM_41, &BCPROV_148, &DEOBF_151, &SCALA],
        "1.5.2" => vec![&ARGO_SMALL, &GUAVA_14, &ASM_41, &BCPROV_148, &DEOBF_152, &SCALA],
        _ => Vec::new(),
    }
}

/// Mirrors of Forge's old fmllibs folder (files.minecraftforge.net/fmllibs is gone), tried after Maven Central.
const FMLLIBS_MIRRORS: [&str; 2] = ["https://files.prismlauncher.org/fmllibs/", "https://files.multimc.org/fmllibs/"];
const MAVEN_CENTRAL: &str = "https://repo1.maven.org/maven2/";

async fn valid(path: &Path, sha1: &str) -> bool {
    exists(path).await && hash_file(path, HashAlgo::Sha1).await.is_ok_and(|h| h.eq_ignore_ascii_case(sha1))
}

/// Puts the libraries FML would download at startup into `<game dir>/lib`, from a shared copy in the
/// launcher's libraries folder.
pub async fn ensure_fml_libs(mc: &str, game_dir: &Path, task: &Arc<Task>) -> Result<()> {
    let libs = fml_libs(mc);
    let lib_dir = game_dir.join("lib");
    let mut missing = Vec::new();
    for l in libs {
        if !valid(&lib_dir.join(l.file), l.sha1).await {
            missing.push(l);
        }
    }
    if missing.is_empty() {
        return Ok(());
    }
    tokio::fs::create_dir_all(&lib_dir).await?;
    let shared = paths::libraries().join("net").join("minecraftforge").join("fmllibs");
    let progress = task.counter(tr!("jarmods.fmlLibs", "Downloading Forge's libraries"));
    let total = missing.len();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let results = futures::future::join_all(missing.into_iter().map(|l| {
        let (shared, lib_dir, progress, done) = (&shared, &lib_dir, &progress, &done);
        async move {
            let cached = shared.join(l.file);
            if !valid(&cached, l.sha1).await {
                let mut urls: Vec<String> = l.maven.map(|m| format!("{MAVEN_CENTRAL}{m}")).into_iter().collect();
                urls.extend(FMLLIBS_MIRRORS.iter().map(|m| format!("{m}{}", l.file)));
                let mut last = None;
                for url in urls {
                    match download_file(&DownloadItem::new(url, &cached).hash(Some(Hash::sha1(l.sha1)))).await {
                        Ok(()) => {
                            last = None;
                            break;
                        }
                        Err(e) => last = Some(e),
                    }
                }
                if let Some(e) = last {
                    return Err(e);
                }
            }
            tokio::fs::copy(&cached, lib_dir.join(l.file)).await?;
            progress(done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1, total);
            Ok::<_, anyhow::Error>(())
        }
    }))
    .await;
    results.into_iter().collect::<Result<Vec<_>>>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_zip(path: &Path, files: &[(&str, &str)]) {
        let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, body) in files {
            z.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(body.as_bytes()).unwrap();
        }
        z.finish().unwrap();
    }

    fn read(path: &Path, name: &str) -> Option<String> {
        let mut z = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        let mut s = String::new();
        std::io::Read::read_to_string(&mut z.by_name(name).ok()?, &mut s).unwrap();
        Some(s)
    }

    #[test]
    fn merges_mods_over_the_game_jar() {
        let dir = std::env::temp_dir().join(format!("resin-jarmods-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let client = dir.join("client.jar");
        make_zip(&client, &[("a.class", "vanilla a"), ("b.class", "vanilla b"), ("META-INF/MOJANG_C.SF", "sig")]);
        let forge = dir.join("forge.zip");
        make_zip(&forge, &[("a.class", "forge a"), ("cpw/mods/fml/relauncher/RelaunchLibraryManager.class", "fml"), ("META-INF/MANIFEST.MF", "m")]);
        let mine = dir.join("mine.zip");
        make_zip(&mine, &[("a.class", "mine a"), ("c.class", "mine c")]);
        let out = dir.join("out.jar");
        let fml = merge(&client, &[forge, mine], &out).unwrap();
        assert!(fml);
        assert_eq!(read(&out, "a.class").as_deref(), Some("mine a"), "the last jar mod wins");
        assert_eq!(read(&out, "b.class").as_deref(), Some("vanilla b"));
        assert_eq!(read(&out, "c.class").as_deref(), Some("mine c"));
        assert!(read(&out, "META-INF/MOJANG_C.SF").is_none() && read(&out, "META-INF/MANIFEST.MF").is_none(), "no META-INF");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn loader_jar_mods_round_trip() {
        let mut v = VersionJson::default();
        assert!(loader_jar_mods(&v).is_empty());
        set_loader_jar_mods(&mut v, &[PathBuf::from("C:/x/forge.zip")]);
        let back: VersionJson = serde_json::from_value(serde_json::to_value(&v).unwrap()).unwrap();
        assert_eq!(loader_jar_mods(&back), vec![PathBuf::from("C:/x/forge.zip")]);
        assert_eq!(fml_libs("1.5.2").len(), 6);
        assert!(fml_libs("1.6.4").is_empty());
    }
}
