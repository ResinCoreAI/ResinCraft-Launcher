//! Compares the RAM display (`core::memory::usage`) with Windows' own "Working Set - Private" counter, the number
//! Task Manager shows, for a real running game:
//!   cargo run --example check_memory -- [mcVersion]
//! Boots a throwaway vanilla instance, waits until the game is up, then prints both readings for its process.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use resincraft_lib::core::types::{LoaderType, NewInstance};
use resincraft_lib::core::{ctx, events, instances, memory, paths, settings};
use resincraft_lib::game::launch;
use resincraft_lib::player::accounts::LaunchAuth;

#[tokio::main]
async fn main() {
    let mc = std::env::args().nth(1).unwrap_or_else(|| "1.21.1".into());
    let data_dir = std::env::var_os("RESIN_DATA_DIR").map(Into::into).unwrap_or_else(|| paths::default_data_dir(None));
    ctx::init(data_dir, None);
    let up = Arc::new(AtomicBool::new(false));
    {
        let up = up.clone();
        events::set_sink(move |channel, payload| {
            if channel == "log" {
                for l in payload["lines"].as_array().into_iter().flatten() {
                    if l["text"].as_str().unwrap_or_default().contains("Sound engine started") {
                        up.store(true, Ordering::SeqCst);
                    }
                }
            }
        });
    }
    let inst = instances::create(
        NewInstance { name: format!("RAM check {mc}"), mc_version: mc.clone(), loader: LoaderType::Vanilla, loader_version: None, color: None, group: None },
        |_| {},
    )
    .await
    .expect("instance");
    let mut s = settings::get().await;
    s.memory_mb = 2048;
    let auth = LaunchAuth {
        account_id: "offline-test".into(),
        username: "ResinTester".into(),
        uuid: "00000000-0000-0000-0000-000000000000".into(),
        access_token: "0".into(),
        user_type: "legacy",
        xuid: None,
    };
    launch::launch_instance(&inst.id, &s, auth, launch::LaunchHooks::default()).await.expect("launch");
    for _ in 0..240 {
        if up.load(Ordering::SeqCst) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    // Let the game settle on the title screen.
    tokio::time::sleep(Duration::from_secs(15)).await;

    let usage = memory::usage().await;
    let game = usage.games.iter().find(|g| g.instance_id == inst.id).expect("the game is listed");
    let pid = launch::game_processes().into_iter().find(|g| g.instance_id == inst.id).map(|g| g.pid).unwrap_or(0);
    let windows = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &format!("(Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Filter 'IDProcess={pid}').WorkingSetPrivate")])
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u64>().ok())
        .unwrap_or(0);
    let mb = |b: u64| b as f64 / 1048576.0;
    println!("game pid {pid}");
    println!("ResinCraft shows: {:.1} MB of {:.0} MB", mb(game.bytes), mb(game.max_bytes));
    println!("Windows private working set: {:.1} MB", mb(windows));
    println!("launcher share (this test program): {:.1} MB in {} process(es)", mb(usage.launcher), usage.launcher_processes);

    launch::kill_instance(&inst.id, None);
    tokio::time::sleep(Duration::from_secs(3)).await;
    let _ = tokio::fs::remove_dir_all(paths::instance_dir(&inst.id)).await;
    let diff = (mb(game.bytes) - mb(windows)).abs();
    if windows > 0 && diff <= mb(windows) * 0.05 {
        println!("RAM CHECK OK: within {:.1} MB of Windows", diff);
    } else {
        println!("RAM CHECK FAILED: {:.1} MB apart", diff);
        std::process::exit(1);
    }
}
