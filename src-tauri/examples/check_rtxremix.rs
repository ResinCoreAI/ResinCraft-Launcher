//! Sets up NVIDIA RTX Remix for Minecraft for real (NVIDIA's GitHub release, about 1.3 GB) in a new instance, as
//! New instance → RTX Remix does, runs the setup a second time (nothing may download again or double up), and checks
//! what landed. The data folder is the first argument (e.g. the dev build's `.dev-data`, to play it afterwards) or a
//! fresh temporary one:
//!   cargo run --example check_rtxremix [data-folder]

use std::path::PathBuf;
use std::time::Instant;

use resincraft_lib::content::index::load_index;
use resincraft_lib::content::rtxremix;
use resincraft_lib::core::{ctx, instances, paths};

#[tokio::main]
async fn main() {
    let data = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join(format!("resin-rtx-{}", std::process::id())));
    println!("data folder: {}", data.display());
    ctx::init(data, None);

    let c = rtxremix::check().await;
    println!("windows {} · ready {} · {:?} · release {:?} · mod+pack {} B · world {} B · error {:?}", c.windows, c.ready, c.gpus, c.version, c.download_size, c.world_size, c.error);
    assert!(c.version.is_some(), "no release");

    let inst = rtxremix::create("RTX Remix".into()).await.expect("create");
    let t = Instant::now();
    rtxremix::install(&inst.id, true).await.expect("install");
    println!("installed in {:.0?}", t.elapsed());
    let t = Instant::now();
    rtxremix::install(&inst.id, true).await.expect("second install");
    println!("second run in {:.1?}", t.elapsed());

    let inst = instances::get(&inst.id).await.unwrap();
    assert_eq!((inst.mc_version.as_str(), inst.loader_version.as_deref(), inst.memory_mb), ("1.21.1", Some("0.16.5"), Some(2048)));
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
    assert_eq!(options.matches("resourcePacks:").count(), 1);
    assert!(game.join("saves/NVIDIA RTX Demo/level.dat").is_file(), "world");
    println!("saves: {:?}", names("saves"));
    let idx = load_index(&inst.id).await;
    let rtx: Vec<_> = idx.values().filter(|e| e.project_id.contains("rtx-remix-minecraft")).collect();
    assert_eq!(rtx.len(), 1);
    println!("index: {} {}", rtx[0].title, rtx[0].version_number);
    assert!(!paths::instance_dir(&inst.id).read_dir().unwrap().flatten().any(|e| e.file_name().to_string_lossy().ends_with(".zip")), "world zip left behind");
    println!("OK: {}", game.display());
}
