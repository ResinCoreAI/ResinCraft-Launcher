//! Reads the mod list of a modpack's newest version from each site, as the Discover details page does:
//!   cargo run --example check_packmods -- [projectId ...]
//! Prints how many mods were found and a few of them, and how long it took.

use std::time::Instant;

use resincraft_lib::content::{details, sources};
use resincraft_lib::core::{ctx, paths};

#[tokio::main]
async fn main() {
    let data_dir = std::env::var_os("RESIN_DATA_DIR").map(Into::into).unwrap_or_else(|| paths::default_data_dir(None));
    ctx::init(data_dir, None);
    let mut ids: Vec<String> = std::env::args().skip(1).collect();
    if ids.is_empty() {
        ids = ["1KVo5zza", "cf:285109", "ftb:23", "atl:SevTechAges", "technic:tekkit-legends"].map(String::from).to_vec();
    }
    let mut failed = false;
    for id in ids {
        let t0 = Instant::now();
        let version = match sources::versions(&id, None).await {
            Ok(v) if !v.is_empty() => v[0].id.clone(),
            Ok(_) => {
                println!("{id}: no versions");
                failed = true;
                continue;
            }
            Err(e) => {
                println!("{id}: versions failed: {e:#}");
                failed = true;
                continue;
            }
        };
        match details::pack_mods(&id, &version).await {
            Ok(list) => {
                let linked = list.iter().filter(|m| m.id.is_some()).count();
                let shown: Vec<String> = list.iter().take(4).map(|m| m.title.clone()).collect();
                println!("{id} @ {version}: {} mods ({linked} linked) in {} ms: {}", list.len(), t0.elapsed().as_millis(), shown.join(", "));
                if list.is_empty() {
                    failed = true;
                }
            }
            Err(e) => {
                println!("{id} @ {version}: FAILED {e:#}");
                failed = true;
            }
        }
    }
    std::process::exit(if failed { 1 } else { 0 });
}
