//! The RTX look for instances RTX Remix can't run in (it's Fabric 1.21.1 only, content/rtxremix.rs): a shader pack
//! with traced lighting, a LabPBR texture pack (the RTX Pack's, content/rtxpack.rs) and settings tuned to the PC.
//!
//! The shader needs a shader loader: Iris (Fabric, Quilt, NeoForge), Oculus (Forge 1.16.5 to 1.20.1), or OptiFine
//! when the instance already has it. Shaders, by how hard they work the graphics card:
//!   - Rethinking Voxels: voxel-traced light with coloured block lights and LabPBR; the RTX look most players know.
//!   - Dirt RT: real path tracing; heavy, for big cards.
//!   - Shrimple: a light path-traced look that runs on modest cards.
//!
//! Everything comes from Modrinth, like Discover. On and off: the shader loader's own setting (`enableShaders`) and the
//! texture packs in options.txt; nothing is removed.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::index::{index_key, load_index, save_index};
use super::rtxpack::{self, PackInfo, RtxOptions, Spec, Tuning, PACKS};
use crate::core::events::Task;
use crate::core::instances;
use crate::core::net::fetch_json;
use crate::core::paths;
use crate::core::types::{ContentKind, Instance, LoaderType};
use crate::{bail_tr, tr};

pub struct ShaderPack {
    pub key: &'static str,
    pub id: &'static str,
    pub title: &'static str,
    /// Video memory (GB) it wants to look and run right.
    pub min_vram: u64,
}

pub const SHADERS: &[ShaderPack] = &[
    ShaderPack { key: "voxels", id: "kmwfVOoi", title: "Rethinking Voxels", min_vram: 6 },
    ShaderPack { key: "pathtraced", id: "zf7q9mVY", title: "Dirt RT", min_vram: 12 },
    ShaderPack { key: "light", id: "BS9T99lD", title: "Shrimple", min_vram: 0 },
];

const IRIS: &str = "YL57xq9U";
const OCULUS: &str = "GchcoXML";

/// What shows a shader in an instance.
#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    Iris,
    Oculus,
    Optifine,
}

impl Loader {
    fn config(self) -> &'static str {
        match self {
            Loader::Iris => "config/iris.properties",
            Loader::Oculus => "config/oculus.properties",
            Loader::Optifine => "optionsshaders.txt",
        }
    }
}

/// "1.20.1" → (1, 20, 1); year versions ("26.3") come out as (26, 3, 0).
fn parse(mc: &str) -> (u32, u32, u32) {
    let mut it = mc.split(['.', '-']).map(|p| p.parse::<u32>().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

/// The shader loader an instance can use, or why none fits.
pub fn loader_for(inst: &Instance) -> std::result::Result<Loader, &'static str> {
    if inst.optifine.is_some() {
        return Ok(Loader::Optifine);
    }
    let v = parse(&inst.mc_version);
    match inst.loader {
        LoaderType::Fabric | LoaderType::Quilt if v >= (1, 16, 5) => Ok(Loader::Iris),
        LoaderType::Neoforge if v >= (1, 20, 6) => Ok(Loader::Iris),
        LoaderType::Forge | LoaderType::Neoforge if v >= (1, 16, 5) && v <= (1, 20, 1) => Ok(Loader::Oculus),
        LoaderType::Vanilla | LoaderType::Horizon | LoaderType::Liteloader => Err("loader"),
        _ => Err("version"),
    }
}

/// The shader that fits a PC: the RTX look from 6 GB of video memory, the light one below.
pub fn recommended_shader(spec: &Spec) -> &'static str {
    let gb = spec.vram_mb.map(|m| (m + 512) / 1024).unwrap_or(0);
    if gb >= 6 {
        "voxels"
    } else {
        "light"
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShaderInfo {
    pub key: &'static str,
    pub title: &'static str,
    pub min_vram: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LookState {
    /// A shader loader fits this instance; else `why` ("loader": needs a mod loader, "version": too old or new).
    pub supported: bool,
    pub why: Option<&'static str>,
    pub loader: Option<Loader>,
    /// The RTX look is in (one of SHADERS installed).
    pub installed: bool,
    /// Shaders on and its packs on.
    pub on: bool,
    pub shader: Option<String>,
    pub spec: Spec,
    pub recommended_shader: &'static str,
    pub recommended: Tuning,
    pub shaders: Vec<ShaderInfo>,
    pub packs: Vec<PackInfo>,
}

async fn shader_file(inst: &Instance) -> Option<(String, &'static ShaderPack)> {
    let idx = load_index(&inst.id).await;
    let prefix = index_key(ContentKind::Shader, "");
    idx.iter().find_map(|(k, e)| {
        let s = SHADERS.iter().find(|s| s.id == e.project_id)?;
        Some((k.strip_prefix(&prefix)?.to_string(), s))
    })
}

/// `key=value` set in a properties file's text.
fn set_prop(text: &str, key: &str, value: &str) -> String {
    let prefix = format!("{key}=");
    let mut found = false;
    let mut lines: Vec<String> = text
        .lines()
        .map(|l| {
            if l.trim_start().starts_with(&prefix) {
                found = true;
                format!("{prefix}{value}")
            } else {
                l.to_string()
            }
        })
        .collect();
    if !found {
        lines.push(format!("{prefix}{value}"));
    }
    lines.join("\n") + "\n"
}

fn get_prop(text: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    text.lines().find_map(|l| l.trim_start().strip_prefix(&prefix).map(|v| v.trim().to_string()))
}

async fn edit_file(path: &Path, f: impl FnOnce(&str) -> String) -> Result<()> {
    let text = tokio::fs::read_to_string(path).await.unwrap_or_default();
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(path, f(&text)).await?;
    Ok(())
}

/// Whether the shader is switched on in the loader's settings.
async fn shader_on(game: &Path, loader: Loader, file: &str) -> bool {
    let text = tokio::fs::read_to_string(game.join(loader.config())).await.unwrap_or_default();
    let named = get_prop(&text, "shaderPack").is_some_and(|p| p == file);
    match loader {
        Loader::Optifine => named,
        _ => named && get_prop(&text, "enableShaders").is_none_or(|v| v == "true"),
    }
}

/// Points the loader at the shader and turns shaders on or off.
async fn set_shader(game: &Path, loader: Loader, file: &str, on: bool) -> Result<()> {
    let path = game.join(loader.config());
    match loader {
        // OptiFine has no on/off: "OFF" is its no-shader choice.
        Loader::Optifine => edit_file(&path, |t| set_prop(t, "shaderPack", if on { file } else { "OFF" })).await,
        _ => {
            edit_file(&path, |t| {
                let t = set_prop(t, "shaderPack", file);
                set_prop(&t, "enableShaders", if on { "true" } else { "false" })
            })
            .await
        }
    }
}

pub async fn state(instance_id: &str) -> Result<LookState> {
    let inst = instances::get(instance_id).await?;
    let vram = super::rtxremix::gpus().await.iter().filter_map(|g| g.vram_mb).max();
    let spec = rtxpack::spec(vram).await;
    let loader = loader_for(&inst);
    let shader = shader_file(&inst).await;
    let on = match (&loader, &shader) {
        (Ok(l), Some((file, _))) => shader_on(&paths::game_dir(&inst.id), *l, file).await,
        _ => false,
    };
    Ok(LookState {
        supported: loader.is_ok(),
        why: loader.err(),
        loader: loader.ok(),
        installed: shader.is_some(),
        on,
        shader: shader.map(|(_, s)| s.key.to_string()),
        recommended_shader: recommended_shader(&spec),
        recommended: rtxpack::with_shader(rtxpack::tune(&spec, rtxpack::recommended_pack(&spec))),
        spec,
        shaders: SHADERS.iter().map(|s| ShaderInfo { key: s.key, title: s.title, min_vram: s.min_vram }).collect(),
        packs: rtxpack::pack_infos(&inst.mc_version).await,
    })
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LookOptions {
    /// "auto" or a SHADERS key.
    #[serde(default = "auto")]
    pub shader: String,
    #[serde(flatten)]
    pub pack: RtxOptions,
}

fn auto() -> String {
    "auto".into()
}

/// The newest build of a shader for the instance's version, or the newest of all (shaders rarely care).
async fn shader_version(s: &ShaderPack, mc: &str) -> Result<String> {
    let url = format!("https://api.modrinth.com/v2/project/{}/version?game_versions={}", s.id, urlencoding::encode(&serde_json::to_string(&[mc])?));
    let mut list: Vec<Value> = fetch_json(&url).await?;
    if list.is_empty() {
        list = fetch_json(&format!("https://api.modrinth.com/v2/project/{}/version", s.id)).await?;
    }
    list.first().and_then(|v| v["id"].as_str()).map(str::to_string).ok_or_else(|| anyhow::anyhow!("{} has no download on Modrinth", s.title))
}

/// Sets up the RTX look in an instance: shader loader, shader, PBR pack, tuned settings.
pub async fn install(instance_id: &str, opts: LookOptions) -> Result<LookState> {
    let inst = instances::get(instance_id).await?;
    if crate::game::running_ids().contains(&inst.id) {
        bail_tr!("rtxLook.running", "Stop {name} before setting up the RTX look.", name = inst.name);
    }
    let loader = match loader_for(&inst) {
        Ok(l) => l,
        Err("loader") => bail_tr!("rtxLook.needsLoader", "{name} has no mod loader. Shaders need Fabric, Quilt, NeoForge or Forge (or OptiFine).", name = inst.name),
        Err(_) => bail_tr!(
            "rtxLook.noLoader",
            "No shader loader runs on {name} ({version}). Iris needs Fabric or Quilt 1.16.5+ or NeoForge 1.20.6+; Oculus needs Forge 1.16.5 to 1.20.1.",
            name = inst.name,
            version = inst.mc_version
        ),
    };
    Task::run(tr!("rtxLook.installing", "Setting up the RTX look"), Some(inst.id.clone()), {
        let inst = inst.clone();
        move |task| async move { fill(&task, &inst, loader, &opts).await }
    })
    .await?;
    state(instance_id).await
}

async fn fill(task: &Arc<Task>, inst: &Instance, loader: Loader, opts: &LookOptions) -> Result<()> {
    let game = paths::game_dir(&inst.id);
    let vram = super::rtxremix::gpus().await.iter().filter_map(|g| g.vram_mb).max();
    let spec = rtxpack::spec(vram).await;

    // The shader loader (its own dependencies, Sodium or Embeddium, come with it).
    let loader_mod = match loader {
        Loader::Iris => Some(("Iris", IRIS)),
        Loader::Oculus => Some(("Oculus", OCULUS)),
        Loader::Optifine => None,
    };
    if let Some((name, id)) = loader_mod {
        if !load_index(&inst.id).await.values().any(|e| e.project_id == id) {
            task.update(tr!("content.installingItem", "Installing {name}…", name = name), None);
            super::modrinth::install(&inst.id, id, ContentKind::Mod, None).await?;
        }
    }

    // The shader: another one the RTX look put in before comes out.
    let key = if opts.shader == "auto" { recommended_shader(&spec) } else { opts.shader.as_str() };
    let s = SHADERS.iter().find(|s| s.key == key).unwrap_or(&SHADERS[0]);
    let mut idx = load_index(&inst.id).await;
    let prefix = index_key(ContentKind::Shader, "");
    let old: Vec<String> = idx.iter().filter(|(_, e)| SHADERS.iter().any(|x| x.id == e.project_id && x.id != s.id)).map(|(k, _)| k.clone()).collect();
    for k in &old {
        idx.remove(k);
        if let Some(file) = k.strip_prefix(&prefix) {
            let _ = tokio::fs::remove_file(game.join("shaderpacks").join(file)).await;
        }
    }
    if !old.is_empty() {
        save_index(&inst.id, &idx).await?;
    }
    let version = shader_version(s, &inst.mc_version).await?;
    if !idx.values().any(|e| e.project_id == s.id && e.version_id == version) {
        task.update(tr!("content.installingItem", "Installing {name}…", name = s.title), None);
        super::modrinth::install(&inst.id, s.id, ContentKind::Shader, Some(version)).await?;
    }
    let Some((file, _)) = shader_file(inst).await else {
        bail_tr!("rtxLook.noShader", "{name} didn't install. Try again.", name = s.title);
    };
    set_shader(&game, loader, &file, true).await?;

    // The PBR pack and the tuning, sized for a shader.
    rtxpack::apply(task, inst, &opts.pack, &spec, true).await?;
    Ok(())
}

/// Turns the RTX look on or off: the shader in the loader's settings, and its texture packs in options.txt.
pub async fn set_on(instance_id: &str, on: bool) -> Result<LookState> {
    let inst = instances::get(instance_id).await?;
    if crate::game::running_ids().contains(&inst.id) {
        bail_tr!("rtxLook.runningToggle", "Stop {name} before turning the RTX look on or off.", name = inst.name);
    }
    let Ok(loader) = loader_for(&inst) else {
        bail_tr!("rtxLook.notInstalled", "{name} doesn't have the RTX look.", name = inst.name);
    };
    let Some((file, _)) = shader_file(&inst).await else {
        bail_tr!("rtxLook.notInstalled", "{name} doesn't have the RTX look.", name = inst.name);
    };
    let game = paths::game_dir(&inst.id);
    set_shader(&game, loader, &file, on).await?;
    let idx = load_index(&inst.id).await;
    let prefix = index_key(ContentKind::Resourcepack, "");
    let packs: Vec<String> =
        idx.iter().filter(|(_, e)| PACKS.iter().any(|p| p.id == e.project_id)).filter_map(|(k, _)| k.strip_prefix(&prefix).map(str::to_string)).collect();
    for p in &packs {
        if on {
            super::rtxremix::enable_pack(&game, p).await?;
        } else {
            let path = game.join("options.txt");
            let text = tokio::fs::read_to_string(&path).await.unwrap_or_default();
            tokio::fs::write(&path, rtxpack::without_pack(&text, p)).await?;
        }
    }
    state(instance_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inst(loader: LoaderType, mc: &str) -> Instance {
        let mut i: Instance = serde_json::from_value(serde_json::json!({
            "id": "x", "name": "X", "mcVersion": mc, "loader": loader, "color": "#fff", "createdAt": 0
        }))
        .unwrap();
        i.optifine = None;
        i
    }

    #[test]
    fn picks_the_shader_loader() {
        assert_eq!(loader_for(&inst(LoaderType::Fabric, "1.20.4")), Ok(Loader::Iris));
        assert_eq!(loader_for(&inst(LoaderType::Quilt, "1.16.5")), Ok(Loader::Iris));
        assert_eq!(loader_for(&inst(LoaderType::Fabric, "1.16.4")), Err("version"));
        assert_eq!(loader_for(&inst(LoaderType::Neoforge, "1.21.1")), Ok(Loader::Iris));
        assert_eq!(loader_for(&inst(LoaderType::Neoforge, "1.20.1")), Ok(Loader::Oculus));
        assert_eq!(loader_for(&inst(LoaderType::Forge, "1.18.2")), Ok(Loader::Oculus));
        assert_eq!(loader_for(&inst(LoaderType::Forge, "1.21.1")), Err("version"));
        assert_eq!(loader_for(&inst(LoaderType::Vanilla, "1.21.1")), Err("loader"));
        assert_eq!(loader_for(&inst(LoaderType::Fabric, "26.3")), Ok(Loader::Iris));
    }

    #[test]
    fn picks_the_shader() {
        let pc = |gb: Option<u64>| Spec { vram_mb: gb.map(|g| g * 1024 - 6), ram_mb: 32 * 1024, cores: 8 };
        assert_eq!(recommended_shader(&pc(Some(12))), "voxels");
        assert_eq!(recommended_shader(&pc(Some(6))), "voxels");
        assert_eq!(recommended_shader(&pc(Some(4))), "light");
        assert_eq!(recommended_shader(&pc(None)), "light");
    }

    #[test]
    fn edits_shader_settings() {
        let t = set_prop("disableUpdateMessage=true\nenableShaders=false\n", "enableShaders", "true");
        assert_eq!(t, "disableUpdateMessage=true\nenableShaders=true\n");
        let t = set_prop(&t, "shaderPack", "Rethinking-Voxels.zip");
        assert_eq!(get_prop(&t, "shaderPack").as_deref(), Some("Rethinking-Voxels.zip"));
    }
}
