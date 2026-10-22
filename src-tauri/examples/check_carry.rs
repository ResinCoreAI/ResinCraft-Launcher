//! Copies worlds, packs and settings between two throwaway instances in a fresh data folder, as the instance page's
//! "Copy from another instance" does, and checks what landed:
//!   cargo run --example check_carry

use std::path::Path;

use resincraft_lib::content::index::{load_index, save_index, IndexEntry};
use resincraft_lib::core::types::{LoaderType, NewInstance};
use resincraft_lib::core::{ctx, instances, paths};
use resincraft_lib::player::carry::{copy_from, CopyPicks, CopyResult};

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

async fn vanilla(name: &str) -> String {
    let new = NewInstance { name: name.into(), mc_version: "1.21.1".into(), loader: LoaderType::Vanilla, loader_version: None, color: None, group: None };
    instances::create(new, |_| {}).await.unwrap().id
}

#[tokio::main]
async fn main() {
    let data = std::env::temp_dir().join(format!("resin-carry-{}", std::process::id()));
    ctx::init(data.clone(), None);
    let (from, to) = (vanilla("Source").await, vanilla("Target").await);
    let (src, dst) = (paths::game_dir(&from), paths::game_dir(&to));

    write(&src.join("saves/Home/level.dat"), "home");
    write(&src.join("saves/Home/region/r.0.0.mca"), "region");
    write(&src.join("saves/Home/session.lock"), "lock");
    write(&src.join("saves/Farm/level.dat"), "farm");
    write(&src.join("saves/junk/readme.txt"), "not a world");
    write(&src.join("shaderpacks/BSL.zip"), "bsl");
    write(&src.join("shaderpacks/BSL.zip.txt"), "bsl options");
    write(&src.join("shaderpacks/Complementary.zip.disabled"), "comp");
    write(&src.join("resourcepacks/Faithful.zip"), "faithful");
    write(&src.join("options.txt"), "fov:1.0\nkey_key.jump:key.keyboard.space");
    write(&src.join("optionsshaders.txt"), "shaderPack=BSL.zip");
    let mut idx = load_index(&from).await;
    idx.insert("shader/BSL.zip".into(), IndexEntry { project_id: "Q1vvjJYV".into(), title: "BSL".into(), ..Default::default() });
    save_index(&from, &idx).await.unwrap();

    // The target already has a world called Home and the Complementary pack.
    write(&dst.join("saves/Home/level.dat"), "theirs");
    write(&dst.join("shaderpacks/Complementary.zip"), "theirs");
    write(&dst.join("options.txt"), "fov:0.5");

    let picks = CopyPicks { worlds: vec!["Home".into(), "Farm".into(), "junk".into()], shaders: true, resource_packs: false, settings: true };
    let r = copy_from(&to, &from, picks).await.unwrap();
    println!("{r:?}");

    let read = |p: &str| std::fs::read_to_string(dst.join(p)).ok();
    let mut ok = r == CopyResult { worlds: vec!["Home (2)".into(), "Farm".into()], shaders: 1, resource_packs: 0, settings: 2 };
    ok &= read("saves/Home/level.dat").as_deref() == Some("theirs");
    ok &= read("saves/Home (2)/level.dat").as_deref() == Some("home");
    ok &= read("saves/Home (2)/region/r.0.0.mca").as_deref() == Some("region");
    ok &= read("saves/Home (2)/session.lock").is_none();
    ok &= read("saves/junk/readme.txt").is_none();
    ok &= read("shaderpacks/BSL.zip").as_deref() == Some("bsl");
    ok &= read("shaderpacks/BSL.zip.txt").as_deref() == Some("bsl options");
    ok &= read("shaderpacks/Complementary.zip").as_deref() == Some("theirs");
    ok &= read("shaderpacks/Complementary.zip.disabled").is_none();
    ok &= read("resourcepacks/Faithful.zip").is_none();
    ok &= read("options.txt").is_some_and(|s| s.starts_with("fov:1.0"));
    ok &= read("optionsshaders.txt").as_deref() == Some("shaderPack=BSL.zip");
    ok &= load_index(&to).await.get("shader/BSL.zip").is_some_and(|e| e.project_id == "Q1vvjJYV");
    ok &= copy_from(&to, &to, CopyPicks::default()).await.is_err();

    let _ = std::fs::remove_dir_all(&data);
    println!("{}", if ok { "PASS" } else { "FAIL" });
    std::process::exit(if ok { 0 } else { 1 });
}
