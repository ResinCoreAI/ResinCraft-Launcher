//! Modpack export: Modrinth .mrpack and CurseForge .zip, with files linked to the site where possible
//! and everything else bundled as overrides.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};
use zip::write::SimpleFileOptions;

use super::curseforge;
use super::index::{index_key, load_index};
use super::modrinth;
use crate::core::events::Task;
use crate::core::instances;
use crate::core::net::{hash_bytes, HashAlgo};
use crate::core::paths;
use crate::core::types::{ContentKind, ExportEntry, ExportOptions, ExportResult, Instance, LoaderType, OwnPack};
use crate::tr;

// Content folders are exported file-by-file (as links where possible); everything else is opt-in overrides.
const CONTENT_FOLDERS: [&str; 3] = ["mods", "resourcepacks", "shaderpacks"];
// Never worth shipping: generated at runtime, personal, or huge.
const HIDDEN: [&str; 11] = [
    "natives",
    "logs",
    "crash-reports",
    ".fabric",
    ".mixin.out",
    ".cache",
    "debug",
    "usercache.json",
    "usernamecache.json",
    "command_history.txt",
    "patchouli_data.json",
];
// Pre-ticked: what modpack authors almost always ship.
const SUGGESTED: [&str; 8] = ["config", "defaultconfigs", "kubejs", "scripts", "global_packs", "datapacks", "patchouli_books", "openloader"];

pub async fn export_preview(instance_id: &str) -> Result<Vec<ExportEntry>> {
    let dir = paths::game_dir(instance_id);
    tokio::fs::create_dir_all(&dir).await?;
    let mut out = Vec::new();
    let mut rd = tokio::fs::read_dir(&dir).await?;
    while let Some(e) = rd.next_entry().await? {
        let name = e.file_name().to_string_lossy().into_owned();
        if HIDDEN.contains(&name.as_str()) || CONTENT_FOLDERS.contains(&name.as_str()) {
            continue;
        }
        let is_dir = e.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
        out.push(ExportEntry { suggested: SUGGESTED.contains(&name.as_str()), name, is_dir });
    }
    out.sort_by(|a, b| b.suggested.cmp(&a.suggested).then(b.is_dir.cmp(&a.is_dir)).then_with(|| super::index::display_cmp(&a.name, &b.name)));
    Ok(out)
}

struct LocalFile {
    /// Path inside the game directory, with forward slashes ("mods/sodium.jar").
    rel: String,
    abs: PathBuf,
    data: Vec<u8>,
    sha1: String,
}

/// The enabled mods, resource packs and shaders in game directory `dir`, and how many disabled ones were left out.
fn content_files(dir: &Path) -> Result<(Vec<LocalFile>, u64)> {
    let mut files = Vec::new();
    let mut skipped_disabled = 0;
    for folder in CONTENT_FOLDERS {
        let Ok(rd) = std::fs::read_dir(dir.join(folder)) else { continue };
        let mut names: Vec<String> = rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        for name in names {
            if name.ends_with(".disabled") {
                skipped_disabled += 1;
                continue;
            }
            let lower = name.to_lowercase();
            if !lower.ends_with(".jar") && !lower.ends_with(".zip") {
                continue;
            }
            // OptiFine's terms forbid redistributing it, so it never goes into a pack.
            if folder == "mods" && (lower.starts_with("optifine_") || lower.starts_with("preview_optifine_")) {
                continue;
            }
            let abs = dir.join(folder).join(&name);
            let data = std::fs::read(&abs)?;
            let sha1 = hash_bytes(&data, HashAlgo::Sha1);
            files.push(LocalFile { rel: format!("{folder}/{name}"), abs, data, sha1 });
        }
    }
    Ok((files, skipped_disabled))
}

fn deflate() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)
}

fn add_folder(zip: &mut zip::ZipWriter<std::fs::File>, abs: &Path, prefix: &str) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(abs)?.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let name = format!("{prefix}/{}", e.file_name().to_string_lossy());
        let ft = e.file_type()?;
        if ft.is_dir() {
            add_folder(zip, &e.path(), &name)?;
        } else if ft.is_file() {
            zip.start_file(name, deflate())?;
            std::io::copy(&mut std::fs::File::open(e.path())?, zip)?;
        }
    }
    Ok(())
}

fn add_overrides(zip: &mut zip::ZipWriter<std::fs::File>, dir: &Path, include: &[String], extra: &[LocalFile]) -> Result<()> {
    for name in include {
        // Only top-level names from export_preview; never a path that could reach outside the instance.
        if name.is_empty() || name.contains(['/', '\\']) || name == ".." || name == "." || CONTENT_FOLDERS.contains(&name.as_str()) {
            continue;
        }
        let abs = dir.join(name);
        let Ok(meta) = std::fs::metadata(&abs) else { continue };
        if meta.is_dir() {
            add_folder(zip, &abs, &format!("overrides/{name}"))?;
        } else if meta.is_file() {
            zip.start_file(format!("overrides/{name}"), deflate())?;
            std::io::copy(&mut std::fs::File::open(&abs)?, zip)?;
        }
    }
    for f in extra {
        zip.start_file(format!("overrides/{}", f.rel), deflate())?;
        zip.write_all(&f.data)?;
    }
    Ok(())
}

/// A JSON object that keeps insertion order (serde_json's own map sorts keys).
struct Ordered(Vec<(String, String)>);

impl Serialize for Ordered {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            m.serialize_entry(k, v)?;
        }
        m.end()
    }
}

#[derive(Serialize)]
struct MrHashes {
    sha1: String,
    sha512: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MrFile {
    path: String,
    hashes: MrHashes,
    downloads: Vec<String>,
    file_size: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MrIndex {
    format_version: u32,
    game: &'static str,
    version_id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    files: Vec<MrFile>,
    dependencies: Ordered,
}

#[derive(Serialize)]
struct CfLoader {
    id: String,
    primary: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CfMinecraft {
    version: String,
    mod_loaders: Vec<CfLoader>,
}

#[derive(Serialize)]
struct CfFileRef {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
    required: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CfManifest {
    minecraft: CfMinecraft,
    manifest_type: &'static str,
    manifest_version: u32,
    name: String,
    version: String,
    author: String,
    files: Vec<CfFileRef>,
    overrides: &'static str,
}

fn loader_key(l: LoaderType) -> Option<&'static str> {
    match l {
        LoaderType::Fabric => Some("fabric-loader"),
        LoaderType::Quilt => Some("quilt-loader"),
        LoaderType::Forge => Some("forge"),
        LoaderType::Neoforge => Some("neoforge"),
        LoaderType::Vanilla => None,
    }
}

/// (manifest file name, manifest JSON, linked count, files to bundle)
type Built = (&'static str, Vec<u8>, u64, Vec<LocalFile>);

async fn build_mrpack(inst: &Instance, opts: &ExportOptions, files: Vec<LocalFile>, task: &Arc<Task>) -> Result<Built> {
    task.update(tr!("export.lookingUpModrinth", "Looking up files on Modrinth…"), None);
    let found = modrinth::lookup_hashes(&files.iter().map(|f| f.sha1.clone()).collect::<Vec<_>>()).await?;
    mrpack_from(inst, opts, files, &found)
}

/// The .mrpack index for `files`, given which of them Modrinth has (by SHA-1); the rest are bundled.
fn mrpack_from(inst: &Instance, opts: &ExportOptions, files: Vec<LocalFile>, found: &HashMap<String, modrinth::HashHit>) -> Result<Built> {
    let (linked, bundled): (Vec<LocalFile>, Vec<LocalFile>) = files.into_iter().partition(|f| found.contains_key(&f.sha1));

    let mut deps = vec![("minecraft".to_string(), inst.mc_version.clone())];
    if let (Some(key), Some(v)) = (loader_key(inst.loader), inst.loader_version.as_ref().filter(|v| !v.is_empty())) {
        deps.push((key.to_string(), v.clone()));
    }
    let index = MrIndex {
        format_version: 1,
        game: "minecraft",
        version_id: opts.version.clone(),
        name: opts.name.clone(),
        summary: Some(opts.summary.clone()).filter(|s| !s.is_empty()),
        files: linked
            .iter()
            .map(|f| {
                let hit = &found[&f.sha1];
                MrFile {
                    path: f.rel.clone(),
                    hashes: MrHashes { sha1: f.sha1.clone(), sha512: hit.sha512.clone() },
                    downloads: vec![hit.url.clone()],
                    file_size: hit.size,
                }
            })
            .collect(),
        dependencies: Ordered(deps),
    };
    Ok(("modrinth.index.json", serde_json::to_vec_pretty(&index)?, linked.len() as u64, bundled))
}

async fn build_curseforge(inst: &Instance, opts: &ExportOptions, files: Vec<LocalFile>, task: &Arc<Task>) -> Result<Built> {
    task.update(tr!("export.lookingUpCurseforge", "Looking up files on CurseForge…"), None);
    // Prefer what we installed from CurseForge ourselves; fingerprint the rest (needs an API key).
    let idx = load_index(&inst.id).await;
    let mut refs: HashMap<PathBuf, (u64, u64)> = HashMap::new();
    for f in &files {
        let (folder, name) = f.rel.split_once('/').unwrap_or_default();
        let kind = match folder {
            "mods" => ContentKind::Mod,
            "resourcepacks" => ContentKind::Resourcepack,
            _ => ContentKind::Shader,
        };
        if let Some(e) = idx.get(&index_key(kind, name)) {
            if e.project_id.starts_with("cf:") && e.sha1 == f.sha1 {
                refs.insert(f.abs.clone(), (curseforge::cf_id(&e.project_id), e.version_id.parse().unwrap_or(0)));
            }
        }
    }
    let unknown: Vec<PathBuf> = files.iter().filter(|f| !refs.contains_key(&f.abs)).map(|f| f.abs.clone()).collect();
    refs.extend(curseforge::lookup_files(unknown).await?);
    curseforge_from(inst, opts, files, &refs)
}

/// The CurseForge manifest for `files`, given the (project, file) ids found for them; the rest are bundled.
fn curseforge_from(inst: &Instance, opts: &ExportOptions, files: Vec<LocalFile>, refs: &HashMap<PathBuf, (u64, u64)>) -> Result<Built> {
    let (linked, bundled): (Vec<LocalFile>, Vec<LocalFile>) = files.into_iter().partition(|f| refs.contains_key(&f.abs));
    let loaders = match (inst.loader, inst.loader_version.as_ref().filter(|v| !v.is_empty())) {
        (LoaderType::Vanilla, _) | (_, None) => vec![],
        (l, Some(v)) => vec![CfLoader { id: format!("{}-{v}", l.as_str()), primary: true }],
    };
    let manifest = CfManifest {
        minecraft: CfMinecraft { version: inst.mc_version.clone(), mod_loaders: loaders },
        manifest_type: "minecraftModpack",
        manifest_version: 1,
        name: opts.name.clone(),
        version: opts.version.clone(),
        author: opts.author.clone(),
        files: linked
            .iter()
            .map(|f| {
                let (p, id) = refs[&f.abs];
                CfFileRef { project_id: p, file_id: id, required: true }
            })
            .collect(),
        overrides: "overrides",
    };
    Ok(("manifest.json", serde_json::to_vec_pretty(&manifest)?, linked.len() as u64, bundled))
}

/// Writes the pack zip: the manifest, then `include` (top-level names in `dir`) and `bundled` under overrides/.
fn write_pack(dest: &Path, dir: &Path, manifest_name: &str, manifest: &[u8], include: &[String], bundled: &[LocalFile]) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = PathBuf::from(format!("{}.part", dest.display()));
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&tmp)?);
    zip.start_file(manifest_name, deflate())?;
    zip.write_all(manifest)?;
    add_overrides(&mut zip, dir, include, bundled)?;
    zip.finish()?;
    std::fs::rename(&tmp, dest)?;
    Ok(())
}

/// The player's own modpack keeps the details it was last exported with, so the next export starts from them.
fn exported(pack: &OwnPack, opts: &ExportOptions, at: i64) -> OwnPack {
    OwnPack {
        name: opts.name.clone(),
        version: opts.version.clone(),
        author: opts.author.clone(),
        summary: opts.summary.clone(),
        bump_on_export: opts.bump_on_export.unwrap_or(pack.bump_on_export),
        exported_version: Some(opts.version.clone()),
        exported_at: Some(at),
    }
}

async fn remember_export(instance_id: &str, opts: &ExportOptions) -> Result<()> {
    let mut inst = instances::get(instance_id).await?;
    if let Some(pack) = &inst.own_pack {
        inst.own_pack = Some(exported(pack, opts, crate::core::fsutil::now_ms()));
        instances::save(&inst).await?;
    }
    Ok(())
}

/// Writes `instance_id` as a Modrinth .mrpack or CurseForge .zip to `dest`.
pub async fn export_modpack(instance_id: &str, opts: ExportOptions, dest: &Path) -> Result<ExportResult> {
    let inst = instances::get(instance_id).await?;
    let dest = dest.to_path_buf();
    Task::run_to_end(tr!("export.exporting", "Exporting {name}", name = opts.name), Some(instance_id.to_string()), move |task| async move {
        task.update(tr!("export.readingMods", "Reading mods…"), None);
        let dir = paths::game_dir(&inst.id);
        let d2 = dir.clone();
        let (files, skipped_disabled) = tokio::task::spawn_blocking(move || content_files(&d2)).await??;
        let (manifest_name, manifest, linked, bundled) =
            if opts.format == "mrpack" { build_mrpack(&inst, &opts, files, &task).await? } else { build_curseforge(&inst, &opts, files, &task).await? };
        task.update(tr!("export.packingConfigs", "Packing configs…"), None);
        let bundled_names: Vec<String> = bundled.iter().map(|f| f.rel.clone()).collect();
        let (d, include) = (dest.clone(), opts.include.clone());
        tokio::task::spawn_blocking(move || write_pack(&d, &dir, manifest_name, &manifest, &include, &bundled)).await??;
        if inst.own_pack.is_some() {
            remember_export(&inst.id, &opts).await?;
        }
        Ok(ExportResult { path: dest.to_string_lossy().into_owned(), linked, bundled: bundled_names, skipped_disabled })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::io::Read;

    fn instance() -> Instance {
        serde_json::from_value(serde_json::json!({
            "id": "my-pack", "name": "My Pack", "mcVersion": "1.21.1", "loader": "fabric", "loaderVersion": "0.16.9",
            "color": "#b26bff", "createdAt": 0,
            "ownPack": { "name": "My Pack", "version": "1.0.0", "author": "Puirkle", "bumpOnExport": true }
        }))
        .unwrap()
    }

    fn options(format: &str) -> ExportOptions {
        ExportOptions {
            format: format.into(),
            name: "My Pack".into(),
            version: "1.0.0".into(),
            author: "Puirkle".into(),
            summary: "Cozy survival".into(),
            include: vec!["config".into()],
            bump_on_export: None,
        }
    }

    /// A game folder with linked and bundled content, plus files an export must leave out.
    fn game_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("resincraft-export-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (rel, data) in [
            ("mods/sodium.jar", "sodium"),
            ("mods/handmade.jar", "handmade"),
            ("mods/old.jar.disabled", "old"),
            ("mods/OptiFine_1.21.1_HD_U_J1.jar", "optifine"),
            ("mods/readme.txt", "not a mod"),
            ("resourcepacks/faithful.zip", "faithful"),
            ("shaderpacks/complementary.zip", "complementary"),
            ("config/sodium-options.json", "{}"),
            ("saves/World/level.dat", "world"),
            ("options.txt", "fov:1.0"),
        ] {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, data).unwrap();
        }
        dir
    }

    fn read_zip(path: &Path) -> HashMap<String, Vec<u8>> {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        (0..zip.len())
            .map(|i| {
                let mut f = zip.by_index(i).unwrap();
                let mut data = Vec::new();
                f.read_to_end(&mut data).unwrap();
                (f.name().to_string(), data)
            })
            .collect()
    }

    /// Every enabled mod, resource pack and shader is in the pack once: linked in the manifest or bundled.
    fn assert_matches_instance(entries: &HashMap<String, Vec<u8>>, linked: BTreeSet<String>, dir: &Path) {
        let bundled: BTreeSet<String> = entries
            .keys()
            .filter_map(|k| k.strip_prefix("overrides/"))
            .filter(|k| CONTENT_FOLDERS.iter().any(|f| k.starts_with(&format!("{f}/"))))
            .map(str::to_string)
            .collect();
        assert!(linked.is_disjoint(&bundled));
        let all: BTreeSet<String> = linked.union(&bundled).cloned().collect();
        let want: BTreeSet<String> =
            ["mods/handmade.jar", "mods/sodium.jar", "resourcepacks/faithful.zip", "shaderpacks/complementary.zip"].map(String::from).into();
        assert_eq!(all, want);
        for rel in &bundled {
            assert_eq!(entries[&format!("overrides/{rel}")], std::fs::read(dir.join(rel)).unwrap(), "{rel}");
        }
        assert_eq!(entries["overrides/config/sodium-options.json"], b"{}");
        assert!(!entries.keys().any(|k| k.contains("saves") || k.contains("options.txt") || k.contains("OptiFine") || k.contains(".disabled")));
    }

    #[test]
    fn mrpack_matches_instance_content() {
        let dir = game_dir("mr");
        let (files, skipped) = content_files(&dir).unwrap();
        assert_eq!(skipped, 1);
        let hit = |name: &str| modrinth::HashHit { url: format!("https://cdn.modrinth.com/{name}"), sha512: format!("{name}-512"), size: 7 };
        let by_name: HashMap<String, String> = files.iter().map(|f| (f.rel.clone(), f.sha1.clone())).collect();
        let found = HashMap::from([
            (by_name["mods/sodium.jar"].clone(), hit("sodium.jar")),
            (by_name["resourcepacks/faithful.zip"].clone(), hit("faithful.zip")),
        ]);
        let (manifest_name, manifest, linked, bundled) = mrpack_from(&instance(), &options("mrpack"), files, &found).unwrap();
        assert_eq!((manifest_name, linked), ("modrinth.index.json", 2));

        let dest = dir.with_extension("mrpack");
        write_pack(&dest, &dir, manifest_name, &manifest, &["config".into()], &bundled).unwrap();
        let entries = read_zip(&dest);
        let index: serde_json::Value = serde_json::from_slice(&entries["modrinth.index.json"]).unwrap();
        assert_eq!(index["name"], "My Pack");
        assert_eq!(index["versionId"], "1.0.0");
        assert_eq!(index["summary"], "Cozy survival");
        assert_eq!(index["dependencies"], serde_json::json!({ "minecraft": "1.21.1", "fabric-loader": "0.16.9" }));
        let files = index["files"].as_array().unwrap();
        for f in files {
            let path = f["path"].as_str().unwrap();
            assert_eq!(f["hashes"]["sha1"], by_name[path]);
        }
        let linked: BTreeSet<String> = files.iter().map(|f| f["path"].as_str().unwrap().to_string()).collect();
        assert_matches_instance(&entries, linked, &dir);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_file(&dest);
    }

    #[test]
    fn curseforge_zip_matches_instance_content() {
        let dir = game_dir("cf");
        let (files, _) = content_files(&dir).unwrap();
        let refs = HashMap::from([(dir.join("mods").join("sodium.jar"), (394468, 5_000_001)), (dir.join("shaderpacks").join("complementary.zip"), (627557, 6_000_002))]);
        let (manifest_name, manifest, linked, bundled) = curseforge_from(&instance(), &options("curseforge"), files, &refs).unwrap();
        assert_eq!((manifest_name, linked), ("manifest.json", 2));

        let dest = dir.with_extension("zip");
        write_pack(&dest, &dir, manifest_name, &manifest, &["config".into()], &bundled).unwrap();
        let entries = read_zip(&dest);
        let m: serde_json::Value = serde_json::from_slice(&entries["manifest.json"]).unwrap();
        assert_eq!((m["name"].as_str(), m["version"].as_str(), m["author"].as_str()), (Some("My Pack"), Some("1.0.0"), Some("Puirkle")));
        assert_eq!(m["minecraft"]["version"], "1.21.1");
        assert_eq!(m["minecraft"]["modLoaders"][0]["id"], "fabric-0.16.9");
        assert_eq!(m["overrides"], "overrides");
        let ids: BTreeSet<(u64, u64)> = m["files"].as_array().unwrap().iter().map(|f| (f["projectID"].as_u64().unwrap(), f["fileID"].as_u64().unwrap())).collect();
        assert_eq!(ids, BTreeSet::from([(394468, 5_000_001), (627557, 6_000_002)]));
        assert_matches_instance(&entries, ["mods/sodium.jar", "shaderpacks/complementary.zip"].map(String::from).into(), &dir);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_file(&dest);
    }

    #[test]
    fn own_pack_remembers_the_export() {
        let pack = instance().own_pack.unwrap();
        let mut opts = options("mrpack");
        opts.version = "1.0.3".into();
        let after = exported(&pack, &opts, 42);
        assert_eq!((after.version.as_str(), after.exported_version.as_deref(), after.exported_at), ("1.0.3", Some("1.0.3"), Some(42)));
        assert_eq!(after.summary, "Cozy survival");
        assert!(after.bump_on_export);
        opts.bump_on_export = Some(false);
        assert!(!exported(&pack, &opts, 43).bump_on_export);
    }
}
