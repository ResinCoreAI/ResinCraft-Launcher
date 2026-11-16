//! Sets up the RTX look for real (Iris, a traced-light shader and a PBR pack from Modrinth) in a new Fabric
//! instance of another version, as an instance's Settings → RTX look does, then switches it off and on, and checks
//! what landed. Uses a fresh temporary data folder.
//!   cargo run --example check_rtxlook [minecraft-version] [pack]

use resincraft_lib::content::index::load_index;
use resincraft_lib::content::rtxlook::{self, LookOptions};
use resincraft_lib::content::rtxpack::RtxOptions;
use resincraft_lib::core::types::{LoaderType, NewInstance};
use resincraft_lib::core::{ctx, instances, paths};

#[tokio::main]
async fn main() {
    let mc = std::env::args().nth(1).unwrap_or_else(|| "1.20.4".into());
    let pack = std::env::args().nth(2).unwrap_or_else(|| "64".into());
    let data = std::env::temp_dir().join(format!("resin-rtxlook-{}", std::process::id()));
    println!("data folder: {}", data.display());
    ctx::init(data, None);

    let new = NewInstance { name: format!("Fabric {mc}"), mc_version: mc.clone(), loader: LoaderType::Fabric, loader_version: None, color: None, group: None };
    let inst = instances::create(new, |_| {}).await.expect("create");
    let st = rtxlook::state(&inst.id).await.expect("state");
    println!("supported {} loader {:?} spec {:?} → shader {} {:?}", st.supported, st.loader, st.spec, st.recommended_shader, st.recommended);
    assert!(st.supported && !st.installed);

    let opts = LookOptions { shader: "auto".into(), pack: RtxOptions { world: false, pack: pack.clone(), own_pack: None, tune: true, memory_mb: None } };
    let st = rtxlook::install(&inst.id, opts).await.expect("install");
    assert!(st.installed && st.on, "installed and on");
    println!("shader: {:?}", st.shader);

    let game = paths::game_dir(&inst.id);
    let names = |dir: &str| -> Vec<String> {
        std::fs::read_dir(game.join(dir)).map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default()
    };
    let mods = names("mods");
    println!("mods: {mods:?}");
    assert!(mods.iter().any(|m| m.to_lowercase().contains("iris")), "Iris");
    assert!(mods.iter().any(|m| m.to_lowercase().contains("sodium")), "Sodium");
    println!("shaderpacks: {:?}", names("shaderpacks"));
    let iris = std::fs::read_to_string(game.join("config/iris.properties")).unwrap();
    println!("iris.properties: {}", iris.trim().replace('\n', " | "));
    assert!(iris.contains("enableShaders=true"));
    let options = std::fs::read_to_string(game.join("options.txt")).unwrap();
    println!("options.txt: {}", options.trim().replace('\n', " | "));
    if pack != "none" {
        assert!(options.contains("resourcePacks:") && options.contains("file/"), "pack on");
    }
    let mem = instances::get(&inst.id).await.unwrap().memory_mb;
    println!("memory: {mem:?}");
    println!("index: {:?}", load_index(&inst.id).await.into_iter().map(|(k, e)| format!("{k} ({})", e.title)).collect::<Vec<_>>());

    let off = rtxlook::set_on(&inst.id, false).await.expect("off");
    let iris = std::fs::read_to_string(game.join("config/iris.properties")).unwrap();
    assert!(!off.on && iris.contains("enableShaders=false"));
    let on = rtxlook::set_on(&inst.id, true).await.expect("on");
    assert!(on.on);
    println!("switch: off and on again");
    println!("OK");
}
