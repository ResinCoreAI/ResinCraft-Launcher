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
use crate::core::types::{ContentKind, ExportEntry, ExportOptions, ExportResult, Instance, LoaderType};
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

fn content_files(inst: &Instance) -> Result<(Vec<LocalFile>, u64)> {
    let dir = paths::game_dir(&inst.id);
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

fn add_overrides(zip: &mut zip::ZipWriter<std::fs::File>, inst: &Instance, include: &[String], extra: &[LocalFile]) -> Result<()> {
    let dir = paths::game_dir(&inst.id);
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

/// Writes `instance_id` as a Modrinth .mrpack or CurseForge .zip to `dest`.
pub async fn export_modpack(instance_id: &str, opts: ExportOptions, dest: &Path) -> Result<ExportResult> {
    let inst = instances::get(instance_id).await?;
    let dest = dest.to_path_buf();
    Task::run_to_end(tr!("export.exporting", "Exporting {name}", name = opts.name), Some(instance_id.to_string()), move |task| async move {
        task.update(tr!("export.readingMods", "Reading mods…"), None);
        let i2 = inst.clone();
        let (files, skipped_disabled) = tokio::task::spawn_blocking(move || content_files(&i2)).await??;
        let (manifest_name, manifest, linked, bundled) =
            if opts.format == "mrpack" { build_mrpack(&inst, &opts, files, &task).await? } else { build_curseforge(&inst, &opts, files, &task).await? };
        task.update(tr!("export.packingConfigs", "Packing configs…"), None);
        let bundled_names: Vec<String> = bundled.iter().map(|f| f.rel.clone()).collect();
        let (d, include) = (dest.clone(), opts.include.clone());
        tokio::task::spawn_blocking(move || -> Result<()> {
            if let Some(parent) = d.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let tmp = PathBuf::from(format!("{}.part", d.display()));
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&tmp)?);
            zip.start_file(manifest_name, deflate())?;
            zip.write_all(&manifest)?;
            add_overrides(&mut zip, &inst, &include, &bundled)?;
            zip.finish()?;
            std::fs::rename(&tmp, &d)?;
            Ok(())
        })
        .await??;
        Ok(ExportResult { path: dest.to_string_lossy().into_owned(), linked, bundled: bundled_names, skipped_disabled })
    })
    .await
}
