//! Headless check that "Play" on a world starts the game straight into it:
//!   cargo run --example check_world -- <mcVersion> <path to a world folder made by that version>
//! Copies the world into a throwaway instance, launches it with quick play and passes once Minecraft starts its
//! integrated server for that world. The throwaway instance is deleted afterwards; the original world isn't touched.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use resincraft_lib::core::types::{LoaderType, NewInstance};
use resincraft_lib::core::{ctx, events, instances, paths, settings};
use resincraft_lib::game::launch::{self, Target};
use resincraft_lib::player::accounts::LaunchAuth;
use tokio::sync::mpsc;

/// The game is opening the world (a world from an older version is upgraded first, after asking).
const IN_WORLD: [&str; 3] = ["Starting integrated minecraft server", "Preparing start region", "Starting upgrade for world"];

#[tokio::main]
async fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let (Some(mc), Some(world)) = (argv.first(), argv.get(1)) else {
        eprintln!("usage: check_world <mcVersion> <world folder>");
        std::process::exit(2);
    };
    let data_dir = std::env::var_os("RESIN_DATA_DIR").map(Into::into).unwrap_or_else(|| paths::default_data_dir(None));
    ctx::init(data_dir, None);

    let in_world = Arc::new(AtomicBool::new(false));
    let (stop_tx, mut stop_rx) = mpsc::unbounded_channel::<()>();
    let inst = instances::create(
        NewInstance { name: format!("World check {mc}"), mc_version: mc.clone(), loader: LoaderType::Vanilla, loader_version: None, color: None, group: None },
        |_| {},
    )
    .await
    .expect("instance");
    {
        let (in_world, id) = (in_world.clone(), inst.id.clone());
        events::set_sink(move |channel, payload| match channel {
            "log" => {
                for l in payload["lines"].as_array().into_iter().flatten() {
                    let text = l["text"].as_str().unwrap_or_default();
                    if std::env::var_os("RESIN_VERBOSE").is_some() {
                        println!("  {}", text);
                    }
                    if !in_world.load(Ordering::SeqCst) && IN_WORLD.iter().any(|m| text.contains(m)) {
                        println!("[game] {}", text.chars().take(200).collect::<String>());
                        in_world.store(true, Ordering::SeqCst);
                        launch::kill_instance(&id, None);
                    }
                }
            }
            "game" => {
                let state = payload["state"].as_str().unwrap_or_default();
                println!("[game] {state} {}", payload["message"].as_str().unwrap_or_default());
                if state == "stopped" || state == "crashed" {
                    let _ = stop_tx.send(());
                }
            }
            _ => {}
        });
    }

    let src = std::path::PathBuf::from(world);
    let folder = src.file_name().unwrap().to_string_lossy().into_owned();
    let dest = paths::game_dir(&inst.id).join("saves").join(&folder);
    instances::copy_dir(&src, &dest, &["session.lock"]).expect("copy world");

    let mut s = settings::get().await;
    s.memory_mb = 3072;
    let auth = LaunchAuth {
        account_id: "offline-test".into(),
        username: "ResinTester".into(),
        uuid: "00000000-0000-0000-0000-000000000000".into(),
        access_token: "0".into(),
        user_type: "legacy",
        xuid: None,
    };
    let launched = launch::launch_instance_to(&inst.id, &s, auth, launch::LaunchHooks::default(), Some(&Target::World(folder.clone()))).await;
    if launched.is_ok() && tokio::time::timeout(Duration::from_secs(240), stop_rx.recv()).await.is_err() {
        launch::kill_instance(&inst.id, None);
        let _ = tokio::time::timeout(Duration::from_secs(30), stop_rx.recv()).await;
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    let _ = tokio::fs::remove_dir_all(paths::instance_dir(&inst.id)).await;
    if let Err(e) = launched {
        println!("WORLD CHECK FAILED: {e:#}");
        std::process::exit(1);
    }
    if in_world.load(Ordering::SeqCst) {
        println!("WORLD CHECK OK: the game opened {folder}");
    } else {
        println!("WORLD CHECK FAILED: the game never started the world");
        std::process::exit(1);
    }
}
