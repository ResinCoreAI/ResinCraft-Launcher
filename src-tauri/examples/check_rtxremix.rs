//! Sets up NVIDIA RTX Remix for Minecraft for real (NVIDIA's GitHub release, about 1.3 GB) in a new instance, as
//! New instance → RTX Remix does, runs the setup a second time (nothing may download again or double up), and checks
//! what landed. The data folder is the first argument (e.g. the dev build's `.dev-data`, to play it afterwards) or a
//! fresh temporary one:
//!   cargo run --example check_rtxremix [data-folder] [instance-id]
//! With an instance id, that instance is set up again (the RTX Pack's texture pack and tuning) instead of a new one.

use std::path::PathBuf;
use std::time::Instant;

use resincraft_lib::content::index::load_index;
use resincraft_lib::content::rtxremix;
use resincraft_lib::core::{ctx, instances, paths};

async fn idx_titles(id: &str) -> Vec<String> {
    let ids: Vec<&str> = resincraft_lib::content::rtxpack::PACKS.iter().map(|p| p.id).collect();
    load_index(id).await.into_iter().filter(|(_, e)| ids.contains(&e.project_id.as_str())).map(|(k, e)| format!("{k} ({})", e.title)).collect()
}

#[tokio::main]
async fn main() {
    let data = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join(format!("resin-rtx-{}", std::process::id())));
    println!("data folder: {}", data.display());
    ctx::init(data, None);

    let c = rtxremix::check().await;
    println!("windows {} · ready {} · {:?} · release {:?} · mod+pack {} B · world {} B · error {:?}", c.windows, c.ready, c.gpus, c.version, c.download_size, c.world_size, c.error);
    assert!(c.version.is_some(), "no release");

    let inst = match std::env::args().nth(2) {
        Some(id) => instances::get(&id).await.expect("instance"),
        None => rtxremix::create("RTX Remix".into()).await.expect("create"),
    };
    println!("spec {:?} → {:?}", c.spec, c.recommended);
    let t = Instant::now();
    rtxremix::install(&inst.id, Default::default()).await.expect("install");
    println!("installed in {:.0?}", t.elapsed());
    let t = Instant::now();
    rtxremix::install(&inst.id, Default::default()).await.expect("second install");
    println!("second run in {:.1?}", t.elapsed());

    let inst = instances::get(&inst.id).await.unwrap();
    assert_eq!((inst.mc_version.as_str(), inst.loader_version.as_deref()), ("1.21.1", Some("0.16.5")));
    assert_eq!(inst.memory_mb, Some(c.recommended.memory_mb), "tuned memory");
    let game = paths::game_dir(&inst.id);
    let names = |dir: &str| -> Vec<String> {
        std::fs::read_dir(game.join(dir)).map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default()
    };
    let mods = names("mods");
    println!("mods: {mods:?}");
    assert_eq!(mods.iter().filter(|m| m.starts_with("rtx-remix-renderer")).count(), 1);
    assert_eq!(mods.iter().filter(|m| m.starts_with("fabric-api")).count(), 1);
    assert!(!mods.iter().any(|m| m.ends_with(".part")));
    let packs = names("resourcepacks");
    println!("resourcepacks: {packs:?}");
    let options = std::fs::read_to_string(game.join("options.txt")).unwrap();
    println!("options.txt: {}", options.trim());
    assert!(packs.iter().any(|p| options.contains(&format!("file/{p}"))));
    assert!(options.contains(&format!("renderDistance:{}", c.recommended.render_distance)), "tuned render distance");
    let pbr: Vec<_> = idx_titles(&inst.id).await;
    println!("PBR pack: {pbr:?}");
    assert_eq!(pbr.len(), 1, "one PBR pack");
    assert_eq!(options.matches("resourcePacks:").count(), 1);
    assert!(game.join("saves/NVIDIA RTX Demo/level.dat").is_file(), "world");
    println!("saves: {:?}", names("saves"));
    let idx = load_index(&inst.id).await;
    let rtx: Vec<_> = idx.values().filter(|e| e.project_id.contains("rtx-remix-minecraft")).collect();
    assert_eq!(rtx.len(), 1);
    println!("index: {} {}", rtx[0].title, rtx[0].version_number);
    assert!(!paths::instance_dir(&inst.id).read_dir().unwrap().flatten().any(|e| e.file_name().to_string_lossy().ends_with(".zip")), "world zip left behind");
    // The instance page's switch: off disables the renderer and takes its packs out of the list; on puts them back.
    let off = rtxremix::set_on(&inst.id, false).await.expect("off");
    let opts = std::fs::read_to_string(game.join("options.txt")).unwrap();
    assert!(!off.on && off.file.as_deref().unwrap().ends_with(".jar.disabled"));
    assert!(off.packs.iter().all(|p| !opts.contains(&format!("file/{p}"))), "packs left on: {opts}");
    let on = rtxremix::set_on(&inst.id, true).await.expect("on");
    let opts = std::fs::read_to_string(game.join("options.txt")).unwrap();
    assert!(on.on && on.file.as_deref().unwrap().ends_with(".jar"));
    assert!(on.packs.iter().all(|p| opts.contains(&format!("file/{p}"))));
    println!("switch: off and on again, packs {:?}", on.packs);
    println!("OK: {}", game.display());
}
