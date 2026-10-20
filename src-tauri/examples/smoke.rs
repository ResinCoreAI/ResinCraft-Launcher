//! Headless end-to-end check of the launch pipeline (no window needed):
//!   cargo run --example smoke -- [mcVersion] [vanilla|fabric|quilt|forge|neoforge] [loaderVersion]
//! Creates a throwaway instance, downloads everything, boots the game with an offline profile and
//! stops it as soon as Minecraft reports the user is set (or, for Forge/NeoForge, that LWJGL is up).
//! RESIN_SMOKE_OPTIFINE=<path to an OptiFine jar you downloaded> also adds OptiFine and checks it loads.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use resincraft_lib::core::types::{LoaderType, NewInstance};
use resincraft_lib::core::{ctx, events, instances, paths, settings};
use resincraft_lib::game::{launch, mojang, optifine};
use resincraft_lib::player::accounts::LaunchAuth;
use serde_json::Value;
use tokio::sync::mpsc;

const BOOT_MARKERS: [&str; 2] = ["Setting user:", "Backend library: LWJGL"];

#[tokio::main]
async fn main() {
    let code = match run().await {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(e) => {
            eprintln!("SMOKE FAILED: {e:#}");
            1
        }
    };
    std::process::exit(code);
}

async fn run() -> anyhow::Result<bool> {
    // Same data folder as a debug build of the app (ResinCraft/.dev-data); RESIN_DATA_DIR overrides it.
    let data_dir = std::env::var_os("RESIN_DATA_DIR").map(Into::into).unwrap_or_else(|| paths::default_data_dir(None));
    std::fs::create_dir_all(&data_dir)?;
    ctx::init(data_dir, None);

    // RESIN_SMOKE_INSTALL_JAVA=<major>: also exercise the Temurin download (even if a suitable Java is installed).
    if let Some(major) = std::env::var("RESIN_SMOKE_INSTALL_JAVA").ok().and_then(|v| v.parse().ok()) {
        let task = events::Task::new(format!("Installing Java {major}"), None);
        let java = resincraft_lib::game::java::install_java(major, &task).await?;
        task.done();
        println!("installed Java {} at {} (managed: {})", java.version, java.path, java.managed);
    }

    let argv: Vec<String> = std::env::args().skip(1).collect();
    let loader: LoaderType = serde_json::from_value(Value::String(argv.get(1).cloned().unwrap_or_else(|| "fabric".into())))
        .map_err(|_| anyhow::anyhow!("loader must be vanilla, fabric, quilt, forge or neoforge"))?;
    let mc = match argv.first() {
        Some(v) => v.clone(),
        None => mojang::list_versions().await?.into_iter().find(|v| v.kind == "release").map(|v| v.id).unwrap_or_default(),
    };
    let inst = instances::create(
        NewInstance { name: format!("Smoke {mc} {}", loader.as_str()), mc_version: mc.clone(), loader, loader_version: argv.get(2).cloned(), color: None, group: None },
        |_| {},
    )
    .await?;
    println!("instance {} ({mc} {} {})", inst.id, loader.as_str(), inst.loader_version.clone().unwrap_or_default());
    let with_optifine = match std::env::var_os("RESIN_SMOKE_OPTIFINE") {
        Some(jar) => match optifine::add(&inst.id, std::path::Path::new(&jar)).await {
            Ok(i) => {
                println!("added OptiFine {}", i.optifine.map(|o| o.version).unwrap_or_default());
                true
            }
            Err(e) => {
                let _ = tokio::fs::remove_dir_all(paths::instance_dir(&inst.id)).await;
                return Err(e);
            }
        },
        None => false,
    };
    let optifine_seen = Arc::new(AtomicBool::new(false));

    let booted = Arc::new(AtomicBool::new(false));
    let (stop_tx, mut stop_rx) = mpsc::unbounded_channel::<Option<i64>>();
    {
        let (booted, id, optifine_seen) = (booted.clone(), inst.id.clone(), optifine_seen.clone());
        events::set_sink(move |channel, payload| match channel {
            "task" => {
                let state = payload["state"].as_str().unwrap_or_default();
                let progress = payload["progress"].as_f64().unwrap_or(0.0);
                if state != "running" || progress == 1.0 {
                    println!("[task] {state} {} {}", payload["detail"].as_str().unwrap_or_default(), payload["error"].as_str().unwrap_or_default());
                }
            }
            "log" => {
                for l in payload["lines"].as_array().into_iter().flatten() {
                    let text = l["text"].as_str().unwrap_or_default();
                    println!("[game:{}] {}", l["level"].as_str().unwrap_or_default(), text.chars().take(300).collect::<String>());
                    if text.contains("[OptiFine]") {
                        optifine_seen.store(true, Ordering::SeqCst);
                    }
                    if !booted.load(Ordering::SeqCst) && BOOT_MARKERS.iter().any(|m| text.contains(m)) {
                        booted.store(true, Ordering::SeqCst);
                        launch::kill_instance(&id);
                    }
                }
            }
            "game" => {
                let state = payload["state"].as_str().unwrap_or_default();
                println!("[game] {state}");
                if state == "stopped" || state == "crashed" {
                    let _ = stop_tx.send(payload["code"].as_i64());
                }
            }
            _ => {}
        });
    }

    let mut s = settings::get().await;
    s.memory_mb = 2048;
    let t0 = Instant::now();
    let auth = LaunchAuth {
        username: "ResinTester".into(),
        uuid: "00000000-0000-0000-0000-000000000000".into(),
        access_token: "0".into(),
        user_type: "legacy",
        xuid: None,
    };
    let launched = launch::launch_instance(&inst.id, &s, auth, launch::LaunchHooks::default()).await;
    if launched.is_ok() {
        if tokio::time::timeout(Duration::from_secs(180), stop_rx.recv()).await.is_err() {
            launch::kill_instance(&inst.id);
            let _ = tokio::time::timeout(Duration::from_secs(30), stop_rx.recv()).await;
        }
    }
    // Give the exit handler a moment to save playtime before the folder goes.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let natives = std::fs::read_dir(paths::natives_dir(&inst.id)).map(|d| d.count()).unwrap_or(0);
    println!("natives extracted: {natives}");
    let _ = tokio::fs::remove_dir_all(paths::instance_dir(&inst.id)).await;
    launched?;

    let mut ok = booted.load(Ordering::SeqCst);
    if with_optifine {
        let seen = optifine_seen.load(Ordering::SeqCst);
        println!("OptiFine {}", if seen { "loaded" } else { "never showed up in the log" });
        ok &= seen;
    }
    if ok {
        println!("SMOKE OK in {}s", t0.elapsed().as_secs());
    } else {
        println!("SMOKE FAILED: game never reached \"Setting user\"");
    }
    Ok(ok)
}
