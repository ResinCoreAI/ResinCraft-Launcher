//! Times ResinCraft's launch from "Play" to the title screen (no window needed):
//!   cargo run --release --example launch_timing -- [mcVersion] [vanilla|fabric|quilt] [loaderVersion] [--breakdown]
//! Same throwaway-instance approach as `smoke`. Prints when Java starts and when the game logs its
//! boot milestones; the last texture atlas ("Created: … atlas") is logged right before the title screen.
//! --breakdown first runs each pre-launch step on its own and times it (this warms the caches, so the
//! launch that follows is a warm start). RESIN_TIMING_MODS=<folder of .jar files> copies mods into the
//! instance. RESIN_DATA_DIR picks the data folder (use an empty one for a first-ever start).
//! RESIN_TIMING_SHOW_COMMAND=1 prints the Java command; RESIN_TIMING_KEEP=1 keeps the instance folder;
//! RESIN_TIMING_INSTANCE=<id> launches that kept instance again (a true second start: Fabric's
//! per-instance mod cache is already built) instead of a new one.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use resincraft_lib::core::net::{download_all, DownloadItem};
use resincraft_lib::core::types::{LoaderType, NewInstance};
use resincraft_lib::core::{ctx, events, instances, paths, settings};
use resincraft_lib::game::loaders::{resolve_version, ResolveContext};
use resincraft_lib::game::{java, launch, mojang};
use resincraft_lib::player::accounts::LaunchAuth;
use serde_json::Value;
use tokio::sync::mpsc;

/// After the last atlas line, wait this long for more before calling the title screen reached.
const QUIET: Duration = Duration::from_secs(6);

fn secs(d: Duration) -> String {
    format!("{:>6.2}s", d.as_secs_f64())
}

fn bytes(items: &[DownloadItem]) -> u64 {
    items.iter().filter_map(|i| std::fs::metadata(&i.dest).ok()).map(|m| m.len()).sum()
}

async fn breakdown(mc: &str, loader: LoaderType, loader_version: Option<&str>, concurrency: usize) -> anyhow::Result<()> {
    let ctx = ResolveContext { task: None, concurrency: Some(concurrency), java: None };
    let total = Instant::now();
    let t = Instant::now();
    let version = resolve_version(mc, loader, loader_version, &ctx).await?;
    println!("[step] resolve version json (network) {}", secs(t.elapsed()));

    let t = Instant::now();
    let libraries = mojang::resolve_libraries(&version);
    let client = mojang::client_jar(&version)?;
    let logging = mojang::logging_config(&version);
    let mut items = vec![client.clone()];
    items.extend(libraries.iter().map(|l| l.item.clone()));
    items.extend(logging.iter().map(|l| l.item.clone()));
    println!("[step] resolve library list          {}", secs(t.elapsed()));

    let t = Instant::now();
    let assets = mojang::asset_items(&version).await?;
    println!("[step] asset index (sha1 + refresh)   {}", secs(t.elapsed()));

    let remote: Vec<DownloadItem> = items.iter().filter(|i| !i.url.is_empty()).cloned().collect();
    let t = Instant::now();
    download_all(remote.clone(), concurrency, |_, _| {}).await?;
    println!("[step] libraries + client jar check   {}  ({} files, {:.0} MB sha1-hashed)", secs(t.elapsed()), remote.len(), bytes(&remote) as f64 / 1e6);

    let t = Instant::now();
    let n = assets.items.len();
    download_all(assets.items, concurrency * 2, |_, _| {}).await?;
    println!("[step] assets check (size only)       {}  ({n} files)", secs(t.elapsed()));

    let t = Instant::now();
    let found = java::find_java(version.java_major()).await?;
    println!("[step] find Java {} (scan + probe)     {}  -> {}", version.java_major(), secs(t.elapsed()), found.map(|j| j.path).unwrap_or_default());
    println!("[step] total (without natives/args)   {}", secs(total.elapsed()));
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("TIMING FAILED: {e:#}");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let data_dir = std::env::var_os("RESIN_DATA_DIR").map(Into::into).unwrap_or_else(|| paths::default_data_dir(None));
    std::fs::create_dir_all(&data_dir)?;
    ctx::init(data_dir, None);

    let argv: Vec<String> = std::env::args().skip(1).collect();
    let with_breakdown = argv.iter().any(|a| a == "--breakdown");
    let pos: Vec<&String> = argv.iter().filter(|a| !a.starts_with("--")).collect();
    let mc = pos.first().map(|s| s.to_string()).unwrap_or_else(|| "1.21.1".into());
    let loader: LoaderType = serde_json::from_value(Value::String(pos.get(1).map(|s| s.to_string()).unwrap_or_else(|| "vanilla".into())))
        .map_err(|_| anyhow::anyhow!("loader must be vanilla, fabric or quilt"))?;
    let loader_version = pos.get(2).map(|s| s.to_string());

    let mut s = settings::get().await;
    s.memory_mb = 4096;

    let reuse = std::env::var("RESIN_TIMING_INSTANCE").ok();
    let inst = match &reuse {
        Some(id) => instances::get(id).await?,
        None => {
            instances::create(
                NewInstance { name: format!("Timing {mc} {}", loader.as_str()), mc_version: mc.clone(), loader, loader_version: loader_version.clone(), color: None, group: None },
                |_| {},
            )
            .await?
        }
    };
    println!("instance {} ({mc} {} {})", inst.id, loader.as_str(), inst.loader_version.clone().unwrap_or_default());
    if let Some(dir) = std::env::var_os("RESIN_TIMING_MODS").filter(|_| reuse.is_none()) {
        let mods = paths::game_dir(&inst.id).join("mods");
        std::fs::create_dir_all(&mods)?;
        let mut n = 0;
        for e in std::fs::read_dir(dir)?.flatten() {
            if e.path().extension().is_some_and(|x| x == "jar") {
                std::fs::copy(e.path(), mods.join(e.file_name()))?;
                n += 1;
            }
        }
        println!("copied {n} mods");
    }

    if with_breakdown {
        breakdown(&mc, loader, inst.loader_version.as_deref(), s.concurrency).await?;
    }

    // Milestones, in seconds since Play.
    let marks: Arc<Mutex<Vec<(String, Duration)>>> = Arc::new(Mutex::new(Vec::new()));
    let last_atlas: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
    let t0 = Instant::now();
    let (stop_tx, mut stop_rx) = mpsc::unbounded_channel::<()>();
    {
        let (marks, last_atlas) = (marks.clone(), last_atlas.clone());
        let mark = move |name: &str| {
            let mut m = marks.lock().unwrap();
            if !m.iter().any(|(n, _)| n == name) {
                m.push((name.to_string(), t0.elapsed()));
            }
        };
        events::set_sink(move |channel, payload| match channel {
            "game" => {
                let state = payload["state"].as_str().unwrap_or_default();
                mark(&format!("game {state}"));
                if state == "stopped" || state == "crashed" {
                    let _ = stop_tx.send(());
                }
            }
            "log" => {
                for l in payload["lines"].as_array().into_iter().flatten() {
                    let text = l["text"].as_str().unwrap_or_default();
                    if text.starts_with("Command: ") && std::env::var_os("RESIN_TIMING_SHOW_COMMAND").is_some() {
                        println!("{text}");
                    }
                    if text.starts_with("ResinCraft is launching") || text.starts_with("Java: ") || text.starts_with("Command: ") {
                        continue;
                    }
                    mark("first game output");
                    for (needle, name) in [
                        ("Loading Minecraft", "fabric loader starts"),
                        ("Setting user:", "Setting user"),
                        ("Backend library: LWJGL", "LWJGL / window"),
                        ("Sound engine started", "Sound engine started"),
                    ] {
                        if text.contains(needle) {
                            mark(name);
                        }
                    }
                    if text.contains("Created:") && text.contains("atlas") {
                        *last_atlas.lock().unwrap() = Some(Instant::now());
                    }
                }
            }
            _ => {}
        });
    }

    let auth = LaunchAuth { username: "ResinTester".into(), uuid: "00000000-0000-0000-0000-000000000000".into(), access_token: "0".into(), user_type: "legacy", xuid: None };
    let launched = launch::launch_instance(&inst.id, &s, auth, launch::LaunchHooks::default()).await;
    if launched.is_ok() {
        let deadline = Instant::now() + Duration::from_secs(240);
        loop {
            if stop_rx.try_recv().is_ok() || Instant::now() > deadline {
                break;
            }
            if last_atlas.lock().unwrap().is_some_and(|t| t.elapsed() > QUIET) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        launch::kill_instance(&inst.id);
        let _ = tokio::time::timeout(Duration::from_secs(30), stop_rx.recv()).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    if std::env::var_os("RESIN_TIMING_KEEP").is_none() && reuse.is_none() {
        let _ = tokio::fs::remove_dir_all(paths::instance_dir(&inst.id)).await;
    }
    launched?;

    println!("--- since Play ---");
    for (name, at) in marks.lock().unwrap().iter() {
        println!("{} {name}", secs(*at));
    }
    match *last_atlas.lock().unwrap() {
        Some(t) => println!("{} last atlas = title screen", secs(t.duration_since(t0))),
        None => println!("title screen never reached"),
    }
    Ok(())
}
