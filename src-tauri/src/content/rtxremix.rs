//! NVIDIA RTX Remix for Minecraft (github.com/NVIDIAGameWorks/rtx-remix-minecraft, Apache-2.0, beta): path tracing
//! for Java Edition. It's a Fabric mod whose jar carries the RTX Remix runtime and unpacks it into the game folder
//! (`remix-sdk/`, `rtx.conf`, `rtx-remix/`) the first time the game starts, so the launcher only has to put files in
//! place. This sets up an instance the way NVIDIA's own installer sets up a Prism instance: Minecraft 1.21.1, Fabric
//! Loader 0.16.5, Fabric API, the mod, the RTX Remix Materials resource pack (turned on in options.txt), the
//! showcase world if wanted, and a 2 GB heap with G1GC.
//!
//! It needs Windows and a GeForce RTX 30-series or newer with driver 616.92 or newer. `check` reads the cards and
//! drivers from nvidia-smi (installed with every NVIDIA driver); the UI warns but doesn't stop anyone, since names
//! can't tell every card apart.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use serde::Serialize;

use super::github::{self, Asset, Release};
use super::index::{extract_where, index_key, load_index, open_zip, save_index, IndexEntry};
use crate::core::events::Task;
use crate::core::instances;
use crate::core::net::{download_file, hash_file, DownloadItem, Hash, HashAlgo};
use crate::core::paths;
use crate::core::types::{ContentKind, Instance, LoaderType, NewInstance};
use crate::{bail_tr, tr};

pub const REPO: &str = "NVIDIAGameWorks/rtx-remix-minecraft";
const MC: &str = "1.21.1";
const FABRIC: &str = "0.16.5";
/// The oldest driver NVIDIA supports it on.
const DRIVER: (u32, u32) = (616, 92);
/// NVIDIA's installer: `-Xmx2G` with G1GC (ResinCraft's optimized flags are G1GC).
const HEAP_MB: u32 = 2048;
/// The world folder NVIDIA's showcase unpacks to.
const WORLD: &str = "NVIDIA RTX Demo";

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Gpu {
    pub name: String,
    pub driver: String,
    /// An RTX card from the 30-series (Ampere) on.
    pub rtx: bool,
    pub driver_ok: bool,
    /// Video memory in MB.
    pub vram_mb: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RtxCheck {
    pub windows: bool,
    /// NVIDIA cards nvidia-smi lists; empty without an NVIDIA driver.
    pub gpus: Vec<Gpu>,
    /// Windows and at least one card that fits with a new enough driver.
    pub ready: bool,
    pub mc_version: &'static str,
    /// The newest release, when GitHub answered.
    pub version: Option<String>,
    pub release_url: Option<String>,
    /// The mod and the materials pack, in bytes.
    pub download_size: u64,
    pub world_size: u64,
    /// Why the release couldn't be read.
    pub error: Option<String>,
    /// The RTX Pack: this PC, the settings it gets, and the texture packs on offer.
    pub spec: super::rtxpack::Spec,
    pub recommended: super::rtxpack::Tuning,
    pub packs: Vec<super::rtxpack::PackInfo>,
}

/// "616.92" → (616, 92).
fn driver_version(s: &str) -> Option<(u32, u32)> {
    let mut it = s.trim().split('.');
    let major = it.next()?.trim().parse().ok()?;
    let minor = it.next().map(|m| m.trim().parse().ok()).unwrap_or(Some(0))?;
    Some((major, minor))
}

/// Whether a card's name is an RTX card from Ampere (30-series, RTX A…) on. Turing (RTX 20-series, Quadro RTX,
/// TITAN RTX) has RT cores but isn't supported.
fn is_supported_rtx(name: &str) -> bool {
    let n = name.to_uppercase();
    if !n.contains("RTX") || n.contains("QUADRO") || n.contains("TITAN RTX") {
        return false;
    }
    // "RTX 2060", "RTX 2080 Ti", "RTX 2050": the number right after RTX.
    let after = n.split("RTX").nth(1).unwrap_or("").trim_start();
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    !(digits.len() == 4 && digits.starts_with("20"))
}

/// nvidia-smi's `name, driver_version, memory.total` CSV lines (memory in MiB, no unit) → cards.
fn parse_gpus(csv: &str) -> Vec<Gpu> {
    csv.lines()
        .filter_map(|line| {
            let mut parts = line.rsplitn(3, ',');
            let (memory, driver, name) = (parts.next()?, parts.next()?, parts.next()?);
            let (name, driver) = (name.trim().to_string(), driver.trim().to_string());
            if name.is_empty() {
                return None;
            }
            let driver_ok = driver_version(&driver).is_some_and(|v| v >= DRIVER);
            Some(Gpu { rtx: is_supported_rtx(&name), driver_ok, vram_mb: memory.trim().parse().ok(), name, driver })
        })
        .collect()
}

async fn gpus() -> Vec<Gpu> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let run = crate::game::java::hidden_command("nvidia-smi").args(["--query-gpu=name,driver_version,memory.total", "--format=csv,noheader,nounits"]).output();
    match tokio::time::timeout(Duration::from_secs(10), run).await {
        Ok(Ok(out)) if out.status.success() => parse_gpus(&String::from_utf8_lossy(&out.stdout)),
        _ => Vec::new(),
    }
}

/// The parts of a release: the mod jar for this PC, the materials pack, the world. The installer bundle is skipped
/// (it holds the same files). Jars named `-win64`/`-arm64` carry one runtime; others carry both.
struct Parts<'a> {
    jar: &'a Asset,
    materials: Option<&'a Asset>,
    world: Option<&'a Asset>,
}

fn parts_of(release: &Release) -> Option<Parts<'_>> {
    let lower = |a: &&Asset| a.name.to_lowercase();
    let jars: Vec<&Asset> = release.assets.iter().filter(|a| lower(a).starts_with("rtx-remix-renderer") && lower(a).ends_with(".jar")).collect();
    let mine = if cfg!(target_arch = "aarch64") { "-arm64" } else { "-win64" };
    let other = if cfg!(target_arch = "aarch64") { "-win64" } else { "-arm64" };
    let jar = jars
        .iter()
        .find(|a| lower(a).trim_end_matches(".jar").ends_with(mine))
        .or_else(|| jars.iter().find(|a| !lower(a).trim_end_matches(".jar").ends_with(other)))
        .copied()?;
    let zip = |prefix: &str| release.assets.iter().find(|a| lower(a).starts_with(prefix) && lower(a).ends_with(".zip"));
    Some(Parts { jar, materials: zip("rtx-remix-materials"), world: zip("rtx-remix-showcase-world") })
}

/// The newest release that has a mod jar (v0.1.0 only shipped an installer zip).
async fn latest() -> Result<Release> {
    let list = github::all_releases(REPO).await?;
    list.into_iter().find(|r| !r.prerelease && parts_of(r).is_some()).ok_or_else(|| {
        crate::err_tr!("rtxRemix.noRelease", "NVIDIA hasn't published an RTX Remix for Minecraft release with a mod file yet.")
    })
}

pub async fn check() -> RtxCheck {
    let gpus = gpus().await;
    let windows = cfg!(windows);
    let ready = windows && gpus.iter().any(|g| g.rtx && g.driver_ok);
    let spec = super::rtxpack::spec(gpus.iter().filter_map(|g| g.vram_mb).max()).await;
    let recommended = super::rtxpack::tune(&spec, super::rtxpack::recommended_pack(&spec));
    let (release, packs) = tokio::join!(latest(), super::rtxpack::pack_infos());
    let mut out = RtxCheck {
        windows,
        gpus,
        ready,
        mc_version: MC,
        version: None,
        release_url: None,
        download_size: 0,
        world_size: 0,
        error: None,
        spec,
        recommended,
        packs,
    };
    match release {
        Ok(r) => {
            let p = parts_of(&r).expect("latest() only returns releases with a jar");
            out.download_size = p.jar.size + p.materials.map_or(0, |a| a.size);
            out.world_size = p.world.map_or(0, |a| a.size);
            out.version = Some(r.tag_name.clone());
            out.release_url = Some(r.html_url.clone());
        }
        Err(e) => out.error = Some(e.to_string()),
    }
    out
}

/// Downloads one big file, reporting how far `<dest>.part` has got.
async fn fetch(task: &Arc<Task>, asset: &Asset, dest: &Path, label: crate::core::text::Text) -> Result<()> {
    // Already there from an earlier try: same size, and the same SHA-256 when GitHub gives one.
    if tokio::fs::metadata(dest).await.is_ok_and(|m| m.len() == asset.size) {
        let same = match asset.sha256() {
            Some(want) => hash_file(dest, HashAlgo::Sha256).await.is_ok_and(|got| got.eq_ignore_ascii_case(&want)),
            None => true,
        };
        if same {
            return Ok(());
        }
    }
    let part = PathBuf::from(format!("{}.part", dest.display()));
    let size = asset.size.max(1) as f64;
    let ticker = {
        let (part, label) = (part.clone(), label.clone());
        let task = task.clone();
        tokio::spawn(async move {
            loop {
                let got = tokio::fs::metadata(&part).await.map(|m| m.len()).unwrap_or(0) as f64;
                task.update(label.clone(), Some((got / size).min(0.99)));
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        })
    };
    task.update(label, Some(0.0));
    let res = download_file(&DownloadItem::new(&asset.url, dest).hash(asset.sha256().map(Hash::sha256)).size(Some(asset.size).filter(|s| *s > 0))).await;
    ticker.abort();
    res
}

/// Puts the world from NVIDIA's zip into `saves/`: the folder holding `level.dat` becomes `saves/<its name>`
/// (`NVIDIA RTX Demo` when level.dat is at the top).
fn unpack_world(zip_path: &Path, saves: &Path) -> Result<()> {
    let mut zip = open_zip(zip_path)?;
    let root = zip
        .file_names()
        .filter(|n| n.rsplit('/').next() == Some("level.dat"))
        .min_by_key(|n| n.len())
        .map(|n| n[..n.len() - "level.dat".len()].to_string());
    let Some(root) = root else {
        bail_tr!("rtxRemix.noWorld", "The showcase world download has no level.dat.");
    };
    let folder = root.trim_end_matches('/').rsplit('/').next().filter(|s| !s.is_empty()).unwrap_or(WORLD).to_string();
    extract_where(&mut zip, saves, |name| name.strip_prefix(root.as_str()).map(|rest| format!("{folder}/{rest}")))
}

/// Turns the materials pack on in options.txt, after vanilla's and Fabric's (and any packs already on).
pub(crate) async fn enable_pack(game: &Path, pack_file: &str) -> Result<()> {
    let options = game.join("options.txt");
    let text = tokio::fs::read_to_string(&options).await.unwrap_or_default();
    tokio::fs::write(&options, with_pack(&text, pack_file)?).await?;
    Ok(())
}

/// options.txt with `file/<pack>` in its `resourcePacks` list (added at the end, so it's on top).
fn with_pack(text: &str, pack_file: &str) -> Result<String> {
    let entry = format!("file/{pack_file}");
    let mut found = false;
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        match line.strip_prefix("resourcePacks:") {
            Some(list) if !found => {
                found = true;
                let mut packs: Vec<String> = serde_json::from_str(list.trim()).unwrap_or_else(|_| vec!["vanilla".into()]);
                if !packs.contains(&entry) {
                    packs.push(entry.clone());
                }
                lines.push(format!("resourcePacks:{}", serde_json::to_string(&packs)?));
            }
            _ => lines.push(line.to_string()),
        }
    }
    if !found {
        lines.push(format!("resourcePacks:{}", serde_json::to_string(&["vanilla", "fabric", entry.as_str()])?));
    }
    Ok(lines.join("
") + "
")
}

/// Makes an empty instance set up for RTX Remix (Fabric 1.21.1, NVIDIA's heap and GC); `install` fills it. Two
/// steps so a download that fails leaves an instance the player can pick to try again.
pub async fn create(name: String) -> Result<Instance> {
    if !cfg!(windows) {
        bail_tr!("rtxRemix.windowsOnly", "RTX Remix for Minecraft only runs on Windows.");
    }
    latest().await?;
    let name = if name.trim().is_empty() { "RTX Remix".to_string() } else { name };
    instances::create(
        NewInstance { name, mc_version: MC.into(), loader: LoaderType::Fabric, loader_version: Some(FABRIC.into()), color: None, group: None },
        |i| {
            i.memory_mb = Some(HEAP_MB);
            i.optimized_flags = Some(true);
            i.use_zgc = Some(false);
        },
    )
    .await
}

/// Puts RTX Remix into a Fabric 1.21.1 instance: a new one from `create`, an existing one, one whose setup stopped,
/// or one to move to the newest release. Files already there aren't downloaded again.
pub async fn install(instance_id: &str, opts: super::rtxpack::RtxOptions) -> Result<Instance> {
    if !cfg!(windows) {
        bail_tr!("rtxRemix.windowsOnly", "RTX Remix for Minecraft only runs on Windows.");
    }
    let inst = instances::get(instance_id).await?;
    if inst.mc_version != MC || inst.loader != LoaderType::Fabric {
        bail_tr!("rtxRemix.wrongInstance", "{name} isn't a Fabric {version} instance, so RTX Remix can't go in it.", name = inst.name, version = MC);
    }
    if crate::game::running_ids().contains(&inst.id) {
        bail_tr!("rtxRemix.running", "Stop {name} before setting up RTX Remix in it.", name = inst.name);
    }
    let release = latest().await?;
    Task::run(tr!("rtxRemix.installing", "Setting up RTX Remix"), Some(inst.id.clone()), {
        let inst = inst.clone();
        move |task| async move {
            fill(&task, &inst, &release, opts.world).await?;
            let vram = gpus().await.iter().filter_map(|g| g.vram_mb).max();
            let spec = super::rtxpack::spec(vram).await;
            super::rtxpack::apply(&task, &inst, &opts, &spec).await
        }
    })
    .await
    .map_err(|e: anyhow::Error| {
        crate::err_tr!(
            "rtxRemix.stopped",
            "RTX Remix setup stopped: {error}. {name} is kept: pick it under New instance → RTX Remix → Add to an instance to finish.",
            error = e.to_string(),
            name = inst.name
        )
    })?;
    instances::get(instance_id).await
}

async fn fill(task: &Arc<Task>, inst: &Instance, release: &Release, with_world: bool) -> Result<()> {
    let p = parts_of(release).expect("latest() only returns releases with a jar");
    let game = paths::game_dir(&inst.id);

    let has_fabric_api = std::fs::read_dir(game.join("mods"))
        .map(|d| d.flatten().any(|e| e.file_name().to_string_lossy().to_lowercase().starts_with("fabric-api")))
        .unwrap_or(false);
    if !has_fabric_api {
        task.update(tr!("content.installingItem", "Installing {name}…", name = "Fabric API"), None);
        super::modrinth::install(&inst.id, "fabric-api", ContentKind::Mod, None).await?;
    }

    let mods = game.join("mods");
    let jar = mods.join(&p.jar.name);
    fetch(task, p.jar, &jar, tr!("rtxRemix.downloadingMod", "Downloading RTX Remix ({size})", size = mb(p.jar.size))).await?;
    // A renderer jar from an older release would load twice.
    let mut idx = load_index(&inst.id).await;
    if let Ok(mut dir) = tokio::fs::read_dir(&mods).await {
        while let Ok(Some(e)) = dir.next_entry().await {
            let n = e.file_name().to_string_lossy().to_string();
            if n != p.jar.name && n.to_lowercase().starts_with("rtx-remix-renderer") && n.to_lowercase().ends_with(".jar") {
                let _ = tokio::fs::remove_file(e.path()).await;
                idx.remove(&index_key(ContentKind::Mod, &n));
            }
        }
    }
    let repo = github::repo(REPO).await.ok();
    idx.insert(
        index_key(ContentKind::Mod, &p.jar.name),
        IndexEntry {
            project_id: format!("{}{REPO}", github::PREFIX),
            version_id: release.tag_name.clone(),
            version_number: release.tag_name.clone(),
            title: "RTX Remix for Minecraft".into(),
            icon_url: repo.as_ref().and_then(|r| r.avatar()),
            sha1: hash_file(&jar, HashAlgo::Sha1).await.unwrap_or_default(),
        },
    );
    save_index(&inst.id, &idx).await?;

    if let Some(m) = p.materials {
        let dest = game.join("resourcepacks").join(&m.name);
        fetch(task, m, &dest, tr!("rtxRemix.downloadingMaterials", "Downloading RTX Remix Materials ({size})", size = mb(m.size))).await?;
        enable_pack(&game, &m.name).await?;
    }

    // A showcase world already there may hold the player's own changes: it's left as it is.
    let has_world = tokio::fs::try_exists(game.join("saves").join(WORLD)).await.unwrap_or(false);
    if let (true, false, Some(w)) = (with_world, has_world, p.world) {
        let zip = paths::instance_dir(&inst.id).join(&w.name);
        fetch(task, w, &zip, tr!("rtxRemix.downloadingWorld", "Downloading the showcase world ({size})", size = mb(w.size))).await?;
        task.update(tr!("rtxRemix.unpackingWorld", "Unpacking the showcase world…"), None);
        let saves = game.join("saves");
        let z = zip.clone();
        tokio::task::spawn_blocking(move || unpack_world(&z, &saves)).await??;
        let _ = tokio::fs::remove_file(&zip).await;
    }
    Ok(())
}

/// Whether an instance has RTX Remix and whether it's on, for the instance page's switch.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RtxState {
    pub installed: bool,
    pub on: bool,
    /// The renderer jar's file name (with `.disabled` when off).
    pub file: Option<String>,
    /// The texture packs that go with it (NVIDIA's materials, the RTX Pack's PBR pack).
    pub packs: Vec<String>,
}

fn is_renderer_jar(name: &str) -> bool {
    let n = name.to_lowercase();
    n.starts_with("rtx-remix-renderer") && (n.ends_with(".jar") || n.ends_with(".jar.disabled"))
}

/// The resource packs that belong to RTX Remix: NVIDIA's materials and a PBR pack the RTX Pack put in.
async fn rtx_packs(inst: &Instance) -> Vec<String> {
    let idx = load_index(&inst.id).await;
    let pbr: Vec<&str> = super::rtxpack::PACKS.iter().map(|p| p.id).collect();
    let mut out: Vec<String> = Vec::new();
    if let Ok(mut dir) = tokio::fs::read_dir(paths::game_dir(&inst.id).join("resourcepacks")).await {
        while let Ok(Some(e)) = dir.next_entry().await {
            let n = e.file_name().to_string_lossy().to_string();
            let ours = n.to_lowercase().starts_with("rtx-remix-materials")
                || idx.get(&index_key(ContentKind::Resourcepack, &n)).is_some_and(|x| pbr.contains(&x.project_id.as_str()));
            if ours {
                out.push(n);
            }
        }
    }
    out.sort();
    out
}

pub async fn state(instance_id: &str) -> Result<RtxState> {
    let inst = instances::get(instance_id).await?;
    let mut file = None;
    if let Ok(mut dir) = tokio::fs::read_dir(paths::game_dir(&inst.id).join("mods")).await {
        while let Ok(Some(e)) = dir.next_entry().await {
            let n = e.file_name().to_string_lossy().to_string();
            if is_renderer_jar(&n) {
                file = Some(n);
                break;
            }
        }
    }
    let on = file.as_deref().is_some_and(|f| !f.ends_with(".disabled"));
    Ok(RtxState { installed: file.is_some(), on, file, packs: rtx_packs(&inst).await })
}

/// Turns RTX Remix on or off in an instance. Off: the renderer jar gets `.disabled` and its texture packs leave
/// options.txt, so the instance plays as plain Fabric 1.21.1; nothing is removed. On puts both back.
pub async fn set_on(instance_id: &str, on: bool) -> Result<RtxState> {
    let inst = instances::get(instance_id).await?;
    if crate::game::running_ids().contains(&inst.id) {
        bail_tr!("rtxRemix.runningToggle", "Stop {name} before turning RTX Remix on or off.", name = inst.name);
    }
    let now = state(instance_id).await?;
    let Some(file) = now.file.clone() else {
        bail_tr!("rtxRemix.notInstalled", "{name} doesn't have RTX Remix.", name = inst.name);
    };
    if now.on != on {
        super::modrinth::toggle_content(&inst.id, ContentKind::Mod, &file).await?;
    }
    let game = paths::game_dir(&inst.id);
    // NVIDIA's materials first, so a PBR pack stays on top of them.
    let mut packs = now.packs.clone();
    packs.sort_by_key(|p| !p.to_lowercase().starts_with("rtx-remix-materials"));
    for p in &packs {
        if on {
            enable_pack(&game, p).await?;
        } else {
            let path = game.join("options.txt");
            let text = tokio::fs::read_to_string(&path).await.unwrap_or_default();
            tokio::fs::write(&path, super::rtxpack::without_pack(&text, p)).await?;
        }
    }
    state(instance_id).await
}

fn mb(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1} GB", bytes as f64 / 1e9)
    } else {
        format!("{} MB", (bytes as f64 / 1e6).round() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_nvidia_smi() {
        let gpus = parse_gpus("NVIDIA GeForce RTX 4070 Ti, 621.05, 12282\nNVIDIA GeForce RTX 2080 SUPER, 620.10, 8192\nNVIDIA GeForce RTX 3060, 600.12, [N/A]\n");
        assert_eq!(gpus.len(), 3);
        assert!(gpus[0].rtx && gpus[0].driver_ok);
        assert!(!gpus[1].rtx && gpus[1].driver_ok);
        assert!(gpus[2].rtx && !gpus[2].driver_ok);
        assert_eq!((gpus[0].vram_mb, gpus[2].vram_mb), (Some(12282), None));
    }

    #[test]
    fn tells_ampere_from_turing() {
        for yes in ["NVIDIA GeForce RTX 3050 Laptop GPU", "NVIDIA GeForce RTX 5090", "NVIDIA RTX A4000", "NVIDIA RTX 4000 Ada Generation", "NVIDIA RTX PRO 6000 Blackwell"] {
            assert!(is_supported_rtx(yes), "{yes}");
        }
        for no in ["NVIDIA GeForce RTX 2060", "NVIDIA GeForce RTX 2080 Ti", "Quadro RTX 5000", "NVIDIA TITAN RTX", "NVIDIA GeForce GTX 1080"] {
            assert!(!is_supported_rtx(no), "{no}");
        }
    }

    #[test]
    fn turns_the_pack_on() {
        assert_eq!(with_pack("", "M.zip").unwrap(), "resourcePacks:[\"vanilla\",\"fabric\",\"file/M.zip\"]
");
        let had = "fov:0.5
resourcePacks:[\"vanilla\",\"file/Faithful.zip\"]
lang:en_us
";
        let out = with_pack(had, "M.zip").unwrap();
        assert_eq!(out, "fov:0.5
resourcePacks:[\"vanilla\",\"file/Faithful.zip\",\"file/M.zip\"]
lang:en_us
");
        assert_eq!(with_pack(&out, "M.zip").unwrap(), out);
    }

    #[test]
    fn compares_drivers() {
        assert!(driver_version("616.92").unwrap() >= DRIVER);
        assert!(driver_version("617.0").unwrap() >= DRIVER);
        assert!(driver_version("616.9").unwrap() < DRIVER);
        assert!(driver_version("abc").is_none());
    }

    #[test]
    fn unpacks_world_under_its_folder() {
        let dir = std::env::temp_dir().join(format!("rc-rtx-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        for (zip_name, entries, want) in [
            ("nested.zip", vec!["NVIDIA RTX Demo/level.dat", "NVIDIA RTX Demo/region/r.0.0.mca"], "NVIDIA RTX Demo/region/r.0.0.mca"),
            ("flat.zip", vec!["level.dat", "region/r.0.0.mca"], "NVIDIA RTX Demo/region/r.0.0.mca"),
        ] {
            let path = dir.join(zip_name);
            let mut w = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            for e in &entries {
                w.start_file(*e, zip::write::SimpleFileOptions::default()).unwrap();
                std::io::Write::write_all(&mut w, b"x").unwrap();
            }
            w.finish().unwrap();
            let saves = dir.join(format!("saves-{zip_name}"));
            unpack_world(&path, &saves).unwrap();
            assert!(saves.join(want).is_file(), "{zip_name}");
            assert!(saves.join("NVIDIA RTX Demo/level.dat").is_file(), "{zip_name}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
