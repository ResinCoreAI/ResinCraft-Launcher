//! The RTX Pack on top of RTX Remix (content/rtxremix.rs): a LabPBR texture pack, which the renderer turns into
//! normals, height, roughness, metal and emission, sized for the PC, and Minecraft's distances and the game's memory
//! tuned to it. Remix's own quality tier stays on its Auto preset, which NVIDIA picks from the card.
//!
//! The packs are free LabPBR packs from Modrinth with a build for 1.21.1 (or 1.21, the same pack format). Builds above
//! 256x (512x to 4096x) are sold on their makers' Patreons, so the player adds such a pack's zip themselves ("own").

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::index::{index_key, load_index, safe_name, save_index};
use crate::core::fsutil::safe_join;
use crate::core::instances;
use crate::core::net::fetch_json;
use crate::core::paths;
use crate::core::types::{ContentKind, Instance};
use crate::{bail_tr, tr};

pub struct PbrPack {
    pub key: &'static str,
    pub res: u32,
    /// Modrinth project id.
    pub id: &'static str,
    pub slug: &'static str,
    pub title: &'static str,
}

pub const PACKS: &[PbrPack] = &[
    PbrPack { key: "64", res: 64, id: "2wBDguLB", slug: "seraphic-pbr", title: "Seraphic PBR" },
    PbrPack { key: "128", res: 128, id: "7junW3B9", slug: "avpbr-retextured", title: "AVPBR Retextured" },
    PbrPack { key: "256", res: 256, id: "F2D7CupJ", slug: "pbr-jg", title: "PBR JG" },
];

/// What the PC has, for sizing the pack and the settings.
#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Spec {
    /// The biggest NVIDIA card's video memory; None without nvidia-smi.
    pub vram_mb: Option<u64>,
    pub ram_mb: u64,
    pub cores: u32,
}

/// The settings the RTX Pack writes.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Tuning {
    /// "none", "64", "128", "256" or "own".
    pub pack: String,
    pub render_distance: u32,
    pub simulation_distance: u32,
    pub memory_mb: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackInfo {
    pub key: &'static str,
    pub res: u32,
    pub title: &'static str,
    pub url: String,
    /// The 1.21.1 build's download size; None when Modrinth didn't answer.
    pub size: Option<u64>,
}

/// What the RTX Remix setup is asked to do besides the mod itself.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RtxOptions {
    #[serde(default = "yes")]
    pub world: bool,
    /// "auto" (sized for the PC), "none", "64", "128", "256" or "own".
    #[serde(default = "auto")]
    pub pack: String,
    /// The zip for "own".
    #[serde(default)]
    pub own_pack: Option<String>,
    /// Write the tuned distances and memory.
    #[serde(default = "yes")]
    pub tune: bool,
    /// The player's own memory for the game (MB), instead of the tuned amount; written even with `tune` off.
    #[serde(default)]
    pub memory_mb: Option<u32>,
}

fn yes() -> bool {
    true
}
fn auto() -> String {
    "auto".into()
}

impl Default for RtxOptions {
    fn default() -> Self {
        RtxOptions { world: true, pack: auto(), own_pack: None, tune: true, memory_mb: None }
    }
}

pub async fn spec(vram_mb: Option<u64>) -> Spec {
    let cores = std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(4);
    Spec { vram_mb, ram_mb: crate::core::settings::total_memory_mb(), cores }
}

/// Memory as the GB it's sold as: a "12 GB" card reports 12282 MB, a "16 GB" PC a little under 16384.
fn gb(mb: u64) -> u64 {
    (mb + 512) / 1024
}

/// The pack size that fits: video memory first (path tracing and big textures share it), then RAM.
pub fn recommended_pack(spec: &Spec) -> &'static str {
    let by_vram = match spec.vram_mb.map(gb) {
        None => "64",
        Some(v) if v < 8 => "64",
        Some(v) if v < 12 => "128",
        Some(_) => "256",
    };
    let ram = gb(spec.ram_mb);
    let by_ram = if ram < 8 {
        "64"
    } else if ram < 12 {
        "128"
    } else {
        "256"
    };
    // The smaller of the two.
    [by_vram, by_ram].into_iter().min_by_key(|k| k.parse::<u32>().unwrap_or(0)).unwrap()
}

/// Distances and memory for a PC and a pack.
pub fn tune(spec: &Spec, pack: &str) -> Tuning {
    let render_distance = match spec.vram_mb.map(gb) {
        None => 8,
        Some(v) if v < 8 => 8,
        Some(v) if v < 12 => 12,
        Some(v) if v < 16 => 16,
        Some(_) => 20,
    };
    let simulation_distance = match spec.cores {
        0..=4 => 6,
        5..=8 => 8,
        _ => 10,
    }
    .min(render_distance);
    let want: u32 = match pack {
        "128" => 4096,
        "256" => 5120,
        "own" if gb(spec.ram_mb) >= 32 => 8192,
        "own" => 6144,
        _ => 3072,
    };
    // Never more than half the PC's memory, never less than NVIDIA's 2 GB.
    let cap = ((spec.ram_mb / 2) as u32 / 512 * 512).max(2048);
    Tuning { pack: pack.to_string(), render_distance, simulation_distance, memory_mb: want.min(cap).max(2048) }
}

/// The game versions to look for a pack build under: the instance's, then its major line ("1.20.4" → "1.20").
/// Resource packs mostly carry across versions, so the newest build of all is the last resort.
fn version_line(mc: &str) -> Vec<String> {
    let mut out = vec![mc.to_string()];
    let parts: Vec<&str> = mc.split('.').collect();
    if parts.len() == 3 {
        out.push(format!("{}.{}", parts[0], parts[1]));
    }
    out
}

/// The player's own memory, kept between 1 GB and all but 2 GB of the PC's RAM (Windows and the graphics driver
/// need room too), in whole 256 MB steps.
pub fn own_memory(spec: &Spec, mb: u32) -> u32 {
    let most = (spec.ram_mb.saturating_sub(2048) as u32).max(1024);
    (mb.clamp(1024, most) / 256) * 256
}

/// A shader pack costs more than RTX Remix's renderer at the same distance: a quarter less, never under 6.
pub fn with_shader(mut t: Tuning) -> Tuning {
    t.render_distance = (t.render_distance * 3 / 4).max(6);
    t.simulation_distance = t.simulation_distance.min(t.render_distance);
    t
}

/// The newest build of a pack for `mc` (or its line, or any): (version id, size).
pub(crate) async fn pack_version(p: &PbrPack, mc: &str) -> Result<(String, u64)> {
    let versions = serde_json::to_string(&version_line(mc))?;
    let url = format!("https://api.modrinth.com/v2/project/{}/version?game_versions={}", p.id, urlencoding::encode(&versions));
    let mut list: Vec<Value> = fetch_json(&url).await?;
    if list.is_empty() {
        list = fetch_json(&format!("https://api.modrinth.com/v2/project/{}/version", p.id)).await?;
    }
    let v = list.first().ok_or_else(|| crate::err_tr!("rtxPack.noBuild", "{name} has no build for Minecraft 1.21.1.", name = p.title))?;
    let size = v["files"].as_array().and_then(|f| f.first()).and_then(|f| f["size"].as_u64()).unwrap_or(0);
    Ok((v["id"].as_str().unwrap_or_default().to_string(), size))
}

pub async fn pack_infos(mc: &str) -> Vec<PackInfo> {
    let sizes = futures::future::join_all(PACKS.iter().map(|p| pack_version(p, mc))).await;
    PACKS
        .iter()
        .zip(sizes)
        .map(|(p, s)| PackInfo { key: p.key, res: p.res, title: p.title, url: format!("https://modrinth.com/resourcepack/{}", p.slug), size: s.ok().map(|(_, s)| s) })
        .collect()
}

/// options.txt with `key:value` set (replaced where it is, or added).
pub fn set_option(text: &str, key: &str, value: &str) -> String {
    let prefix = format!("{key}:");
    let mut found = false;
    let mut lines: Vec<String> = text
        .lines()
        .map(|l| {
            if l.starts_with(&prefix) {
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

/// options.txt without `file/<pack>` in its `resourcePacks` list.
pub(crate) fn without_pack(text: &str, pack_file: &str) -> String {
    let entry = format!("file/{pack_file}");
    let lines: Vec<String> = text
        .lines()
        .map(|l| match l.strip_prefix("resourcePacks:").and_then(|list| serde_json::from_str::<Vec<String>>(list.trim()).ok()) {
            Some(packs) => format!("resourcePacks:{}", serde_json::to_string(&packs.into_iter().filter(|p| *p != entry).collect::<Vec<_>>()).unwrap_or_default()),
            None => l.to_string(),
        })
        .collect();
    lines.join("\n") + "\n"
}

async fn edit_options(game: &Path, f: impl FnOnce(&str) -> Result<String>) -> Result<()> {
    let path = game.join("options.txt");
    let text = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    tokio::fs::write(&path, f(&text)?).await?;
    Ok(())
}

/// Puts the chosen pack in (taking out another size the RTX Pack put in before) and writes the tuned settings.
/// Runs after the mod and NVIDIA's materials are in, so the pack goes on top of them.
pub async fn apply(task: &crate::core::events::Task, inst: &Instance, opts: &RtxOptions, spec: &Spec, shader: bool) -> Result<Tuning> {
    let game = paths::game_dir(&inst.id);
    let pack = if opts.pack == "auto" { recommended_pack(spec).to_string() } else { opts.pack.clone() };
    let chosen = PACKS.iter().find(|p| p.key == pack);

    // A size picked before comes out, from the folder, the list and options.txt.
    let mut idx = load_index(&inst.id).await;
    let old: Vec<(String, String)> = idx
        .iter()
        .filter(|(_, e)| PACKS.iter().any(|p| p.id == e.project_id) && chosen.is_none_or(|c| c.id != e.project_id))
        .map(|(k, _)| (k.clone(), k.split_once('/').map(|(_, f)| f.to_string()).unwrap_or_default()))
        .collect();
    for (key, file) in &old {
        idx.remove(key);
        let _ = tokio::fs::remove_file(game.join("resourcepacks").join(file)).await;
        edit_options(&game, |t| Ok(without_pack(t, file))).await?;
    }
    if !old.is_empty() {
        save_index(&inst.id, &idx).await?;
    }

    if let Some(p) = chosen {
        task.update(tr!("content.installingItem", "Installing {name}…", name = format!("{} {}x", p.title, p.res)), None);
        let (version, _) = pack_version(p, &inst.mc_version).await?;
        // The same build already there isn't downloaded again (a big pack can still be held open by a virus scan).
        let have = idx.iter().any(|(k, e)| {
            e.project_id == p.id && e.version_id == version && k.split_once('/').is_some_and(|(_, f)| game.join("resourcepacks").join(f).is_file())
        });
        if !have {
            super::modrinth::install(&inst.id, p.id, ContentKind::Resourcepack, Some(version)).await?;
        }
        let idx = load_index(&inst.id).await;
        let file = idx
            .iter()
            .find(|(k, e)| e.project_id == p.id && k.starts_with(&index_key(ContentKind::Resourcepack, "")))
            .and_then(|(k, _)| k.split_once('/').map(|(_, f)| f.to_string()));
        if let Some(file) = file {
            super::rtxremix::enable_pack(&game, &file).await?;
        }
    } else if pack == "own" {
        let Some(src) = opts.own_pack.as_deref().map(Path::new) else {
            bail_tr!("rtxPack.ownMissing", "Pick your texture pack's .zip first.");
        };
        let name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let dest = safe_join(&game.join("resourcepacks"), safe_name(&name)?)?;
        if dest != src {
            tokio::fs::create_dir_all(game.join("resourcepacks")).await?;
            tokio::fs::copy(src, &dest).await?;
        }
        super::rtxremix::enable_pack(&game, &name).await?;
    }

    let mut tuning = tune(spec, &pack);
    if shader {
        tuning = with_shader(tuning);
    }
    if let Some(mb) = opts.memory_mb {
        tuning.memory_mb = own_memory(spec, mb);
    }
    if opts.tune {
        edit_options(&game, |t| {
            let t = set_option(t, "renderDistance", &tuning.render_distance.to_string());
            Ok(set_option(&t, "simulationDistance", &tuning.simulation_distance.to_string()))
        })
        .await?;
    }
    if opts.tune || opts.memory_mb.is_some() {
        instances::update(&inst.id, json!({ "memoryMb": tuning.memory_mb })).await?;
    }
    Ok(tuning)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pc(vram_gb: Option<u64>, ram_gb: u64, cores: u32) -> Spec {
        Spec { vram_mb: vram_gb.map(|g| g * 1024), ram_mb: ram_gb * 1024, cores }
    }

    #[test]
    fn sizes_the_pack_for_the_pc() {
        assert_eq!(recommended_pack(&pc(Some(12), 32, 16)), "256");
        assert_eq!(recommended_pack(&pc(Some(8), 32, 16)), "128");
        assert_eq!(recommended_pack(&pc(Some(6), 16, 8)), "64");
        assert_eq!(recommended_pack(&pc(Some(24), 8, 8)), "128", "RAM holds it back");
        assert_eq!(recommended_pack(&pc(None, 32, 8)), "64");
    }

    #[test]
    fn reads_memory_as_sold() {
        // An RTX 4070 Ti and a 40 GB PC, as nvidia-smi and the OS report them.
        let real = Spec { vram_mb: Some(12282), ram_mb: 40764, cores: 12 };
        assert_eq!(recommended_pack(&real), "256");
        assert_eq!(tune(&real, "256").render_distance, 16);
        assert_eq!(recommended_pack(&Spec { vram_mb: Some(8188), ram_mb: 16300, cores: 8 }), "128");
    }

    #[test]
    fn tunes_distances_and_memory() {
        let t = tune(&pc(Some(12), 32, 16), "256");
        assert_eq!((t.render_distance, t.simulation_distance, t.memory_mb), (16, 10, 5120));
        let t = tune(&pc(Some(6), 8, 4), "64");
        assert_eq!((t.render_distance, t.simulation_distance, t.memory_mb), (8, 6, 3072));
        // Half of 6 GB is the most it takes.
        assert_eq!(tune(&pc(Some(8), 6, 4), "128").memory_mb, 3072);
        assert_eq!(tune(&pc(Some(24), 64, 24), "own").memory_mb, 8192);
        assert_eq!(tune(&pc(Some(24), 3, 24), "own").memory_mb, 2048);
    }

    #[test]
    fn keeps_own_memory_sane() {
        let p = pc(Some(12), 16, 8);
        assert_eq!(own_memory(&p, 6144), 6144);
        assert_eq!(own_memory(&p, 64 * 1024), 14 * 1024);
        assert_eq!(own_memory(&p, 100), 1024);
        assert_eq!(own_memory(&p, 5000), 4864);
    }

    #[test]
    fn looks_for_pack_builds() {
        assert_eq!(version_line("1.20.4"), vec!["1.20.4", "1.20"]);
        assert_eq!(version_line("1.21"), vec!["1.21"]);
        assert_eq!(with_shader(tune(&pc(Some(12), 32, 16), "256")).render_distance, 12);
    }

    #[test]
    fn edits_options() {
        assert_eq!(set_option("fov:0.5\nrenderDistance:32\n", "renderDistance", "16"), "fov:0.5\nrenderDistance:16\n");
        assert_eq!(set_option("fov:0.5\n", "simulationDistance", "8"), "fov:0.5\nsimulationDistance:8\n");
        let t = "resourcePacks:[\"vanilla\",\"file/A.zip\",\"file/B.zip\"]\nlang:en_us\n";
        assert_eq!(without_pack(t, "A.zip"), "resourcePacks:[\"vanilla\",\"file/B.zip\"]\nlang:en_us\n");
    }
}
