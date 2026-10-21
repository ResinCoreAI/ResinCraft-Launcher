//! Installs real modpacks and boots each one to the title screen, headless apart from the game window:
//!   cargo run --example check_packs -- search <modrinth|curseforge|ftb|atlauncher|technic> <query>
//!   cargo run --example check_packs -- <packId>[@versionId] ...      e.g. cf:285109  technic:tekkitmain  ftb:23
//! A pack passes when Minecraft gets as far as starting its sound engine, which happens after every mod has
//! loaded (old Forge: when it reports every mod loaded). Passing instances are deleted again; failing ones are kept (RESIN_KEEP=1 keeps all of them).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use resincraft_lib::content::sources;
use resincraft_lib::core::types::{ContentKind, ContentSource, SearchQuery};
use resincraft_lib::core::{ctx, events, paths, settings};
use resincraft_lib::game::launch;
use resincraft_lib::player::accounts::LaunchAuth;
use serde_json::Value;
use tokio::sync::mpsc;

const LOADED: [&str; 1] = ["Sound engine started"];
/// Forge stops on its own error screen (and waits there) for these.
const STUCK: [&str; 3] = ["MissingModsException", "Missing or unsupported mandatory dependencies", "DuplicateModsFoundException"];
/// Forge for Minecraft ≤1.12 starts the sound system before its mods finish loading; wait for this instead.
/// (Forge for 1.2.5 only writes "load complete" to ForgeModLoader-0.log in the game folder, which is watched too.)
const LEGACY_LOADED: [&str; 2] = ["Forge Mod Loader has successfully loaded", "Forge Mod Loader load complete"];
const BOOT_MINUTES: u64 = 12;

struct Current {
    id: String,
    legacy: bool,
    loaded: Arc<AtomicBool>,
    stuck: Arc<AtomicBool>,
    tail: Arc<Mutex<std::collections::VecDeque<String>>>,
    stop: mpsc::UnboundedSender<(String, Option<i64>)>,
}

/// The pack being booted. `events` keeps only the first sink it's given, so one sink routes to this.
static CURRENT: Mutex<Option<Current>> = Mutex::new(None);

fn on_event(channel: &str, payload: Value) {
    match channel {
        "task" => {
            let state = payload["state"].as_str().unwrap_or_default();
            if state != "running" {
                println!("[task] {state} {} {}", payload["detail"].as_str().unwrap_or_default(), payload["error"].as_str().unwrap_or_default());
            }
        }
        "log" => {
            let cur = CURRENT.lock().unwrap();
            let Some(c) = cur.as_ref() else { return };
            for l in payload["lines"].as_array().into_iter().flatten() {
                let text = l["text"].as_str().unwrap_or_default();
                {
                    let mut t = c.tail.lock().unwrap();
                    t.push_back(text.chars().take(400).collect());
                    if t.len() > 60 {
                        t.pop_front();
                    }
                }
                if !c.stuck.load(Ordering::SeqCst) && STUCK.iter().any(|m| text.contains(m)) {
                    c.stuck.store(true, Ordering::SeqCst);
                    println!("[game] stuck: {}", text.chars().take(300).collect::<String>());
                    launch::kill_instance(&c.id, None);
                }
                let markers: &[&str] = if c.legacy { &LEGACY_LOADED } else { &LOADED };
                if !c.loaded.load(Ordering::SeqCst) && markers.iter().any(|m| text.contains(m)) {
                    c.loaded.store(true, Ordering::SeqCst);
                    println!("[game] reached: {}", text.chars().take(200).collect::<String>());
                    launch::kill_instance(&c.id, None);
                }
            }
        }
        "game" => {
            let state = payload["state"].as_str().unwrap_or_default().to_string();
            println!("[game] {state} {}", payload["message"].as_str().unwrap_or_default());
            if state == "stopped" || state == "crashed" || state == "error" {
                if let Some(c) = CURRENT.lock().unwrap().as_ref() {
                    let _ = c.stop.send((state, payload["code"].as_i64()));
                }
            }
        }
        _ => {}
    }
}

#[tokio::main]
async fn main() {
    let data_dir = std::env::var_os("RESIN_DATA_DIR").map(Into::into).unwrap_or_else(|| paths::default_data_dir(None));
    std::fs::create_dir_all(&data_dir).unwrap();
    ctx::init(data_dir, None);
    events::set_sink(on_event);
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.first().map(String::as_str) == Some("search") {
        if let Err(e) = search(&argv[1], &argv[2..].join(" ")).await {
            eprintln!("search failed: {e:#}");
            std::process::exit(1);
        }
        return;
    }
    let mut failed = 0;
    let mut summary = Vec::new();
    for arg in &argv {
        let (id, version) = match arg.split_once('@') {
            Some((i, v)) => (i.to_string(), Some(v.to_string())),
            None => (arg.clone(), None),
        };
        let t0 = Instant::now();
        let r = check(&id, version).await;
        let line = match &r {
            Ok(note) => format!("PASS {arg} in {}s {note}", t0.elapsed().as_secs()),
            Err(e) => {
                failed += 1;
                format!("FAIL {arg} after {}s: {e:#}", t0.elapsed().as_secs())
            }
        };
        println!("==== {line}");
        summary.push(line);
    }
    println!("\n==== SUMMARY");
    for l in &summary {
        println!("{l}");
    }
    std::process::exit(if failed == 0 { 0 } else { 1 });
}

async fn search(source: &str, query: &str) -> anyhow::Result<()> {
    let source: ContentSource = serde_json::from_value(Value::String(source.into()))?;
    let q = SearchQuery {
        query: query.into(),
        kind: ContentKind::Modpack,
        source: Some(source),
        game_version: None,
        loader: None,
        sort: Some("downloads".into()),
        offset: None,
        limit: Some(15),
    };
    for h in sources::search(&q).await?.hits {
        println!("{:<28} {:>12} {}", h.id, h.downloads, h.title);
    }
    Ok(())
}

async fn check(id: &str, version: Option<String>) -> anyhow::Result<String> {
    println!("==== installing {id}");
    let inst = sources::install_modpack(id, version).await?;
    let pending = inst.pending_downloads.as_ref().map(|p| p.len()).unwrap_or(0);
    if let Ok(v) = resincraft_lib::content::dupes::pack_result(inst.clone()) {
        for r in v["duplicatesRemoved"].as_array().into_iter().flatten() {
            println!("removed as a duplicate: {} (kept {})", r["filename"].as_str().unwrap_or_default(), r["kept"].as_str().unwrap_or_default());
        }
    }
    println!("instance {} ({} {} {}), {pending} files need a manual download", inst.id, inst.mc_version, inst.loader.as_str(), inst.loader_version.clone().unwrap_or_default());

    let loaded = Arc::new(AtomicBool::new(false));
    let stuck = Arc::new(AtomicBool::new(false));
    let tail = Arc::new(Mutex::new(std::collections::VecDeque::<String>::new()));
    let (stop_tx, mut stop_rx) = mpsc::unbounded_channel::<(String, Option<i64>)>();
    *CURRENT.lock().unwrap() = Some(Current {
        id: inst.id.clone(),
        // Old Forge, or an old pack whose bin/modpack.jar (Forge and all) became a jar mod.
        legacy: (inst.loader.as_str() == "forge" || !inst.jar_mods.is_empty()) && resincraft_lib::game::loaders::is_legacy_mc(&inst.mc_version),
        loaded: loaded.clone(),
        stuck: stuck.clone(), tail: tail.clone(), stop: stop_tx });

    let mut s = settings::get().await;
    s.memory_mb = 6144;
    let auth = LaunchAuth {
        account_id: "offline-test".into(),
        username: "ResinTester".into(),
        uuid: "00000000-0000-0000-0000-000000000000".into(),
        access_token: "0".into(),
        user_type: "legacy",
        xuid: None,
    };
    let launched = launch::launch_instance(&inst.id, &s, auth, launch::LaunchHooks::default()).await;
    let fml_log = {
        let (file, id, loaded) = (paths::game_dir(&inst.id).join("ForgeModLoader-0.log"), inst.id.clone(), loaded.clone());
        let legacy = CURRENT.lock().unwrap().as_ref().is_some_and(|c| c.legacy);
        tokio::spawn(async move {
            while legacy && !loaded.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_secs(2)).await;
                let text = tokio::fs::read_to_string(&file).await.unwrap_or_default();
                if let Some(line) = text.lines().find(|l| LEGACY_LOADED.iter().any(|m| l.contains(m))) {
                    loaded.store(true, Ordering::SeqCst);
                    println!("[game] reached: {line}");
                    launch::kill_instance(&id, None);
                }
            }
        })
    };
    let mut ended = None;
    if launched.is_ok() {
        match tokio::time::timeout(Duration::from_secs(BOOT_MINUTES * 60), stop_rx.recv()).await {
            Ok(r) => ended = r,
            Err(_) => {
                launch::kill_instance(&inst.id, None);
                let _ = tokio::time::timeout(Duration::from_secs(30), stop_rx.recv()).await;
            }
        }
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    fml_log.abort();
    *CURRENT.lock().unwrap() = None;
    let ok = loaded.load(Ordering::SeqCst) && !stuck.load(Ordering::SeqCst);
    let keep = !ok || std::env::var_os("RESIN_KEEP").is_some();
    if !ok {
        println!("---- last log lines ----");
        for l in tail.lock().unwrap().iter() {
            println!("  {l}");
        }
        println!("---- kept at {}", paths::instance_dir(&inst.id).display());
    }
    if !keep {
        let _ = tokio::fs::remove_dir_all(paths::instance_dir(&inst.id)).await;
    }
    launched?;
    if ok {
        Ok(if pending > 0 { format!("({pending} manual downloads skipped)") } else { String::new() })
    } else {
        if stuck.load(Ordering::SeqCst) {
            anyhow::bail!("Forge stopped on a mod error screen (see [game] stuck above)");
        }
        match ended {
            Some((state, code)) => anyhow::bail!("game {state} (exit code {code:?}) before it finished loading"),
            None => anyhow::bail!("didn't finish loading within {BOOT_MINUTES} minutes"),
        }
    }
}
