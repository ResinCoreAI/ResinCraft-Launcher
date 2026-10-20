//! OptiFine (`optifine:*`). Its terms forbid redistributing or mirroring it, so ResinCraft never downloads it:
//! the player gets the jar from optifine.net and the launcher imports it. How it's set up depends on the loader:
//! Forge → the jar goes in mods/; Fabric/Quilt → mods/ plus OptiFabric (from CurseForge); vanilla → the jar
//! is kept in the instance's optifine/ folder and turned into a launchwrapper profile at launch
//! ([`apply_vanilla`]), the way OptiFine's own installer sets it up. OptiFine doesn't support NeoForge.

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde_json::Value;

use crate::content::{curseforge, index, modrinth};
use crate::core::net::{download_all, DownloadItem};
use crate::core::router::{done, json};
use crate::core::types::{ContentKind, Instance, LoaderType, OptifineJar, OptifineRef};
use crate::core::{instances, paths, shell, Args};
use crate::{bail_tr, err_tr};

use super::java::hidden_command;
use super::mojang::{Argument, VersionJson};

/// OptiFabric on CurseForge: loads OptiFine on Fabric (and Quilt).
pub const OPTIFABRIC: &str = "cf:322385";
const LAUNCHWRAPPER_MAIN: &str = "net.minecraft.launchwrapper.Launch";
const TWEAKER: &str = "optifine.OptiFineTweaker";
/// Mojang's launchwrapper, for OptiFine builds that don't bring their own (Minecraft 1.12 and older).
const LAUNCHWRAPPER_URL: &str = "https://libraries.minecraft.net/net/minecraft/launchwrapper/1.12/launchwrapper-1.12.jar";

pub async fn handle(channel: &str, args: Args) -> Result<Value> {
    match channel {
        "optifine:browse" => {
            let picked = shell::pick_file("Choose the OptiFine jar", &[("OptiFine", &["jar"])]).await?;
            json(picked.map(|p| p.to_string_lossy().into_owned()))
        }
        "optifine:inspect" => {
            let mc: Option<String> = args.get(1)?;
            json(inspect(&PathBuf::from(args.get::<String>(0)?), mc.as_deref()).await?)
        }
        "optifine:add" => json(add(&args.get::<String>(0)?, &PathBuf::from(args.get::<String>(1)?)).await?),
        "optifine:remove" => {
            remove(&args.get::<String>(0)?).await?;
            done()
        }
        _ => bail!("Unknown request: {channel}"),
    }
}

/// "OptiFine_1.20.1_HD_U_I6.jar" (or "preview_OptiFine_…", or a browser's "… (1).jar") → ("1.20.1", "HD_U_I6").
fn parse_file_name(name: &str) -> Option<(String, String)> {
    let re = regex::Regex::new(r"(?i)^(?:preview_)?OptiFine_(\d+(?:\.\d+)+)_(HD\w*?)(?:\s*\(\d+\))?\.jar$").unwrap();
    let c = re.captures(name.trim())?;
    Some((c[1].to_string(), c[2].to_string()))
}

/// True when a class file's constant pool has `s` as a string (a CONSTANT_Utf8 entry: tag 1, u16 length, bytes).
fn class_has_string(class: &[u8], s: &str) -> bool {
    let len = s.len() as u16;
    let mut needle = vec![1u8, (len >> 8) as u8, len as u8];
    needle.extend_from_slice(s.as_bytes());
    class.windows(needle.len()).any(|w| w == needle.as_slice())
}

fn read_entry<R: Read + std::io::Seek>(zip: &mut zip::ZipArchive<R>, name: &str) -> Option<Vec<u8>> {
    let mut entry = zip.by_name(name).ok()?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn not_optifine(file: &str) -> anyhow::Error {
    err_tr!(
        "optifine.notOptifine",
        "{file} isn't an OptiFine jar. Download OptiFine from optifine.net and choose the .jar file it gives you.",
        file = file
    )
}

/// Checks that `path` is an OptiFine jar (for Minecraft `mc`, when given) and reads its version.
pub async fn inspect(path: &Path, mc: Option<&str>) -> Result<OptifineJar> {
    let (path, mc) = (path.to_path_buf(), mc.map(str::to_string));
    tokio::task::spawn_blocking(move || inspect_blocking(&path, mc.as_deref())).await?
}

fn inspect_blocking(path: &Path, mc: Option<&str>) -> Result<OptifineJar> {
    let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut zip = std::fs::File::open(path)
        .map_err(anyhow::Error::from)
        .and_then(|f| Ok(zip::ZipArchive::new(f)?))
        .map_err(|_| not_optifine(&file))?;
    if !index::has_entry(&zip, "optifine/OptiFineTweaker.class") && !index::has_entry(&zip, "optifine/Installer.class") {
        return Err(not_optifine(&file));
    }
    // OptiFine's Config class holds the Minecraft version it was built for (net/optifine/ since 1.13).
    let config = read_entry(&mut zip, "net/optifine/Config.class").or_else(|| read_entry(&mut zip, "Config.class"));
    let named = parse_file_name(&file);

    let jar_mc = match (&named, mc) {
        (Some((v, _)), _) => v.clone(),
        (None, Some(want)) if config.as_deref().is_some_and(|c| class_has_string(c, want)) => want.to_string(),
        _ => bail_tr!(
            "optifine.unknownVersion",
            "Couldn't tell which Minecraft version {file} is for. Choose the file as optifine.net names it (OptiFine_<version>_HD_U_….jar).",
            file = file
        ),
    };
    if let Some(want) = mc {
        let fits = match &config {
            Some(c) => class_has_string(c, want),
            None => jar_mc == want,
        };
        if !fits {
            bail_tr!(
                "optifine.wrongVersion",
                "This OptiFine is for Minecraft {jar}, but the instance is Minecraft {mc}. Download the OptiFine for Minecraft {mc}.",
                jar = jar_mc,
                mc = want
            );
        }
    }
    let edition = named.map(|(_, e)| e).unwrap_or_else(|| "HD_U".into());
    Ok(OptifineJar { version: format!("{jar_mc}_{edition}"), mc_version: jar_mc, edition: edition.replace('_', " ") })
}

/// Where an instance keeps its OptiFine jar.
fn jar_path(inst: &Instance, file: &str) -> PathBuf {
    match inst.loader {
        LoaderType::Vanilla => paths::instance_dir(&inst.id).join("optifine").join(file),
        _ => index::folder_of(inst, ContentKind::Mod).join(file),
    }
}

fn check_loader(inst: &Instance) -> Result<()> {
    if inst.loader == LoaderType::Neoforge {
        bail_tr!(
            "optifine.neoforge",
            "OptiFine doesn't run on NeoForge. Use Sodium (or Embeddium) and Iris (or Oculus) from Discover instead."
        );
    }
    Ok(())
}

/// Imports a downloaded OptiFine jar into an instance (replacing one added before) and sets it up for its loader.
pub async fn add(id: &str, src: &Path) -> Result<Instance> {
    let mut inst = instances::get(id).await?;
    check_loader(&inst)?;
    let jar = inspect(src, Some(&inst.mc_version)).await?;

    if matches!(inst.loader, LoaderType::Fabric | LoaderType::Quilt) {
        let idx = index::load_index(&inst.id).await;
        if !idx.values().any(|e| e.project_id == OPTIFABRIC) {
            if curseforge::versions(OPTIFABRIC, Some(&inst.id)).await?.is_empty() {
                bail_tr!(
                    "optifine.noOptifabric",
                    "OptiFabric, which runs OptiFine on {loader}, isn't available for Minecraft {mc}. Use Sodium and Iris instead, or a Forge instance.",
                    loader = inst.loader.label(),
                    mc = inst.mc_version
                );
            }
            curseforge::install(&inst.id, OPTIFABRIC, ContentKind::Mod, None).await?;
        }
    }

    let file = format!("OptiFine_{}.jar", jar.version);
    let dest = jar_path(&inst, &file);
    if let Some(old) = inst.optifine.take() {
        let old = jar_path(&inst, &old.file);
        if old != dest {
            let _ = tokio::fs::remove_file(old).await;
        }
    }
    let same = match (tokio::fs::canonicalize(src).await, tokio::fs::canonicalize(&dest).await) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if !same {
        if let Some(dir) = dest.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        tokio::fs::copy(src, &dest).await?;
    }
    inst.optifine = Some(OptifineRef { version: jar.version, file });
    instances::save(&inst).await?;
    Ok(inst)
}

/// Takes OptiFine (and the OptiFabric it needed) out of an instance; the files go to the Recycle Bin.
pub async fn remove(id: &str) -> Result<()> {
    let mut inst = instances::get(id).await?;
    if let Some(of) = inst.optifine.take() {
        let path = jar_path(&inst, &of.file);
        if tokio::fs::metadata(&path).await.is_ok() {
            shell::trash(&path).await?;
        }
    }
    let idx = index::load_index(&inst.id).await;
    let prefix = format!("{}/", ContentKind::Mod.as_str());
    for (key, _) in idx.iter().filter(|(_, e)| e.project_id == OPTIFABRIC) {
        if let Some(filename) = key.strip_prefix(&prefix) {
            let folder = index::folder_of(&inst, ContentKind::Mod);
            // The file may be switched off (".disabled").
            let name = [filename.to_string(), format!("{filename}.disabled")].into_iter().find(|n| folder.join(n).exists());
            if let Some(name) = name {
                modrinth::remove_content(&inst.id, ContentKind::Mod, &name).await?;
            }
        }
    }
    instances::save(&inst).await
}

/// What a vanilla instance's OptiFine jar contains besides OptiFine itself.
struct JarParts {
    /// OptiFine's patcher, which builds the library from the game jar (newer builds ship a diff, not classes).
    patcher: bool,
    /// OptiFine's own launchwrapper (file name, version, bytes), when it brings one.
    launchwrapper: Option<(String, String, Vec<u8>)>,
}

fn read_parts(installer: &Path) -> Result<JarParts> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(installer)?)?;
    let patcher = index::has_entry(&zip, "optifine/Patcher.class");
    let launchwrapper = match read_entry(&mut zip, "launchwrapper-of.txt") {
        Some(v) => {
            let ver = String::from_utf8_lossy(&v).trim().to_string();
            read_entry(&mut zip, &format!("launchwrapper-of-{ver}.jar")).map(|b| ("launchwrapper-of".to_string(), ver, b))
        }
        None => read_entry(&mut zip, "launchwrapper-2.0.jar").map(|b| ("launchwrapper".to_string(), "2.0".to_string(), b)),
    };
    Ok(JarParts { patcher, launchwrapper })
}

/// Changes a vanilla version so it starts through launchwrapper with OptiFine's tweaker,
/// and returns the jars to put in front of the classpath.
pub(crate) fn tweak_version(version: &mut VersionJson) {
    version.main_class = Some(LAUNCHWRAPPER_MAIN.to_string());
    let game = &mut version.arguments.get_or_insert_with(Default::default).game;
    if !game.iter().any(|a| matches!(a, Argument::Plain(s) if s == TWEAKER)) {
        game.extend([Argument::Plain("--tweakClass".into()), Argument::Plain(TWEAKER.into())]);
    }
}

/// Sets up OptiFine for a vanilla instance (once per OptiFine version): runs the jar's patcher against the game
/// jar like OptiFine's installer does, unpacks its launchwrapper, and switches `version` to launch through it.
/// Returns the jars to put in front of the classpath (none when the instance has no OptiFine).
pub async fn apply_vanilla(inst: &Instance, version: &mut VersionJson, client: &Path, java: &str, concurrency: usize) -> Result<Vec<PathBuf>> {
    let Some(of) = inst.optifine.as_ref() else { return Ok(Vec::new()) };
    let installer = jar_path(inst, &of.file);
    if tokio::fs::metadata(&installer).await.is_err() {
        bail_tr!("optifine.missing", "OptiFine's jar is missing from {name}. Add OptiFine again in the instance's settings.", name = inst.name);
    }
    let parts = {
        let installer = installer.clone();
        tokio::task::spawn_blocking(move || read_parts(&installer)).await??
    };

    let root = paths::libraries().join("optifine");
    let lib = root.join("OptiFine").join(&of.version).join(format!("OptiFine-{}.jar", of.version));
    if tokio::fs::metadata(&lib).await.is_err() {
        tokio::fs::create_dir_all(lib.parent().unwrap()).await?;
        let tmp = lib.with_extension("jar.part");
        if parts.patcher {
            let out = hidden_command(java).arg("-cp").arg(&installer).arg("optifine.Patcher").arg(client).arg(&installer).arg(&tmp).output().await?;
            if !out.status.success() || tokio::fs::metadata(&tmp).await.is_err() {
                let _ = tokio::fs::remove_file(&tmp).await;
                let log = String::from_utf8_lossy(if out.stderr.is_empty() { &out.stdout } else { &out.stderr }).into_owned();
                let tail: String = log.lines().rev().take(5).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
                bail_tr!("optifine.patchFailed", "Couldn't set up OptiFine: {error}", error = tail);
            }
        } else {
            tokio::fs::copy(&installer, &tmp).await?;
        }
        tokio::fs::rename(&tmp, &lib).await?;
    }

    let wrapper = match parts.launchwrapper {
        Some((name, ver, bytes)) => {
            let path = root.join(&name).join(&ver).join(format!("{name}-{ver}.jar"));
            if tokio::fs::metadata(&path).await.is_err() {
                tokio::fs::create_dir_all(path.parent().unwrap()).await?;
                tokio::fs::write(&path, bytes).await?;
            }
            path
        }
        None => {
            let path = paths::libraries().join("net/minecraft/launchwrapper/1.12/launchwrapper-1.12.jar");
            download_all(vec![DownloadItem::new(LAUNCHWRAPPER_URL, path.clone())], concurrency, |_, _| {}).await?;
            path
        }
    };
    tweak_version(version);
    Ok(vec![wrapper, lib])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn file_names() {
        assert_eq!(parse_file_name("OptiFine_1.20.1_HD_U_I6.jar"), Some(("1.20.1".into(), "HD_U_I6".into())));
        assert_eq!(parse_file_name("preview_OptiFine_1.21.1_HD_U_J1_pre9.jar"), Some(("1.21.1".into(), "HD_U_J1_pre9".into())));
        assert_eq!(parse_file_name("OptiFine_1.12.2_HD_U_G5 (1).jar"), Some(("1.12.2".into(), "HD_U_G5".into())));
        assert_eq!(parse_file_name("sodium-fabric-0.5.8.jar"), None);
    }

    #[test]
    fn class_strings() {
        // A constant pool fragment: Utf8 "1.20.1".
        let class = [&[0xCA, 0xFE, 1, 0, 6][..], b"1.20.1", &[1, 0, 4], b"HD_U"].concat();
        assert!(class_has_string(&class, "1.20.1"));
        assert!(class_has_string(&class, "HD_U"));
        assert!(!class_has_string(&class, "1.20"));
    }

    fn fake_jar(dir: &Path, name: &str, mc: &str) -> PathBuf {
        let path = dir.join(name);
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("optifine/OptiFineTweaker.class", opts).unwrap();
        zip.write_all(b"\xCA\xFE\xBA\xBE").unwrap();
        zip.start_file("net/optifine/Config.class", opts).unwrap();
        zip.write_all(&[&[0xCA, 0xFE, 1, 0, mc.len() as u8][..], mc.as_bytes()].concat()).unwrap();
        zip.start_file("launchwrapper-of.txt", opts).unwrap();
        zip.write_all(b"2.3").unwrap();
        zip.start_file("launchwrapper-of-2.3.jar", opts).unwrap();
        zip.write_all(b"PK").unwrap();
        zip.finish().unwrap();
        path
    }

    #[test]
    fn inspects_jars() {
        let dir = std::env::temp_dir().join(format!("resin-optifine-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let jar = fake_jar(&dir, "OptiFine_1.20.1_HD_U_I6.jar", "1.20.1");
        let found = inspect_blocking(&jar, Some("1.20.1")).unwrap();
        assert_eq!(found, OptifineJar { version: "1.20.1_HD_U_I6".into(), mc_version: "1.20.1".into(), edition: "HD U I6".into() });
        assert!(inspect_blocking(&jar, Some("1.21.1")).is_err());
        // A renamed file still works when the jar itself names the instance's version.
        let renamed = fake_jar(&dir, "optifine.jar", "1.20.1");
        assert_eq!(inspect_blocking(&renamed, Some("1.20.1")).unwrap().version, "1.20.1_HD_U");
        assert!(inspect_blocking(&renamed, None).is_err());
        let parts = read_parts(&jar).unwrap();
        assert!(!parts.patcher);
        assert_eq!(parts.launchwrapper.map(|(n, v, _)| (n, v)), Some(("launchwrapper-of".into(), "2.3".into())));
        std::fs::write(dir.join("notes.jar"), b"not a zip").unwrap();
        assert!(inspect_blocking(&dir.join("notes.jar"), None).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn tweaks_versions() {
        let mut v: VersionJson = serde_json::from_value(serde_json::json!({
            "id": "1.12.2", "mainClass": "net.minecraft.client.main.Main", "minecraftArguments": "--username ${auth_player_name}", "libraries": []
        }))
        .unwrap();
        tweak_version(&mut v);
        tweak_version(&mut v);
        assert_eq!(v.main_class.as_deref(), Some(LAUNCHWRAPPER_MAIN));
        assert_eq!(v.arguments.as_ref().unwrap().game.len(), 2);
    }
}
