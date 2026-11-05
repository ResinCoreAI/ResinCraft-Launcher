//! Live, headless check of Horizon native mods: in a scratch launcher folder that already holds a prepared Horizon
//! build (`horizon-builds/<id>/`, `current.txt`), makes a Horizon instance pinned to it, installs two of Horizon's own
//! native mods from the live registry through the engine's own calls, builds them with Horizon's launcher (a real
//! compile: the first one downloads Rust and the crates), and asks the built game's Launch API what it takes. It
//! never plays (that needs a signed-in Microsoft account).
//!
//!   cargo run --example check_native <scratch folder> <build id> [mod ids…]     (default mods: zoom fps_overlay)

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use anyhow::{anyhow, Result};
use resincraft_lib::content::{modrinth, sources};
use resincraft_lib::core::types::{ContentKind, LoaderType, NewInstance};
use resincraft_lib::core::{ctx, events, instances, paths};
use resincraft_lib::game::horizon_native::{self, Need};

static PASSED: AtomicUsize = AtomicUsize::new(0);
static FAILED: AtomicUsize = AtomicUsize::new(0);

fn report<T>(name: &str, r: Result<T>) -> Option<T> {
    match r {
        Ok(v) => {
            PASSED.fetch_add(1, Ordering::Relaxed);
            println!("PASS {name}");
            Some(v)
        }
        Err(e) => {
            FAILED.fetch_add(1, Ordering::Relaxed);
            println!("FAIL {name}: {e:#}");
            None
        }
    }
}

fn expect(name: &str, ok: bool, what: impl std::fmt::Debug) {
    report(name, if ok { Ok(()) } else { Err(anyhow!("{what:?}")) });
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(root), Some(build)) = (args.next().map(PathBuf::from), args.next()) else {
        eprintln!("usage: check_native <scratch folder> <build id> [mod ids…]");
        std::process::exit(2);
    };
    let mut mods: Vec<String> = args.collect();
    if mods.is_empty() {
        mods = vec!["zoom".into(), "fps_overlay".into()];
    }
    println!("scratch launcher folder: {}", root.display());
    let _ = std::fs::remove_dir_all(root.join("instances"));
    ctx::init(root.clone(), None);
    events::set_sink(|channel, payload| {
        if channel == "task" {
            match payload["state"].as_str() {
                Some("error") => println!("     [task failed] {}: {}", payload["title"].as_str().unwrap_or(""), payload["error"].as_str().unwrap_or("")),
                Some("running") => {
                    if let Some(d) = payload["detail"].as_str() {
                        println!("     [{}] {d}", payload["title"].as_str().unwrap_or(""));
                    }
                }
                _ => {}
            }
        }
    });

    let new = NewInstance { name: "check-native".into(), mc_version: String::new(), loader: LoaderType::Horizon, loader_version: Some(build.clone()), color: None, group: None };
    let Some(inst) = report("make a Horizon instance", instances::create(new, |_| {}).await) else { return finish() };
    println!("     {} plays Minecraft {} on {:?}", inst.id, inst.mc_version, inst.loader_version);

    for id in &mods {
        report(&format!("install native mod {id}"), sources::install(&inst.id, &format!("hz:{id}"), ContentKind::Mod, None).await);
    }
    let game = paths::game_dir(&inst.id);
    let list = modrinth::list_content(&inst.id, ContentKind::Mod).await.unwrap_or_default();
    for m in &list {
        println!("     {} · {:?} {:?} · native {} · on {}", m.filename, m.title, m.version_number, m.native, m.enabled);
    }
    expect("the Content tab lists them as native code", list.iter().filter(|m| m.native).count() == mods.len(), &list.len());
    expect("a native mod can't be turned off", modrinth::toggle_content(&inst.id, ContentKind::Mod, &format!("{}.hnmod", mods[0])).await.is_err(), "it toggled");

    let status = horizon_native::status(&inst).await;
    expect("a build is needed before it plays", status.as_ref().is_ok_and(|s| s.need == Need::FirstBuild), status.as_ref().map(|s| s.need).map_err(|e| e.to_string()));
    let refused = horizon_native::build(&inst.id, false).await;
    expect("nothing is compiled without the player's trust", refused.is_err(), "it built");

    let started = Instant::now();
    let built = report("build the native mods", horizon_native::build(&inst.id, true).await);
    let secs = started.elapsed().as_secs();
    println!("     build took {}m {}s", secs / 60, secs % 60);
    let tail = horizon_native::log_tail(12).await;
    println!("     build log's end:\n       {}", tail.join("\n       "));
    let Some(record) = built else { return finish() };
    println!("     {record:?}");

    let status = horizon_native::status(&inst).await;
    expect("now up to date", status.as_ref().is_ok_and(|s| s.need == Need::UpToDate), status.as_ref().map(|s| s.need).map_err(|e| e.to_string()));

    let base = resincraft_lib::game::horizon::build_for(&inst, &events::Task::new("check", None)).await.unwrap();
    let dir = horizon_native::for_launch(&inst, &base).await;
    let Some(Some(dir)) = report("the start plays the native build", dir) else { return finish() };
    let exe = horizon_native::client(&dir);
    println!("     {}", exe.display());
    let options = report("the built horizon-client answers --launch-api", resincraft_lib::game::horizon::launch_options(&exe).await);
    println!("     options: {options:?}");

    // A changed mod file asks for a build again; the player can still play once without the native mods.
    let file = game.join("horizon-native-mods").join(&mods[0]).join("horizon-native.json");
    let before = std::fs::read(&file).unwrap();
    std::fs::write(&file, [before.as_slice(), b"\n"].concat()).unwrap();
    let changed = horizon_native::status(&inst).await.map(|s| s.need).ok();
    expect("a changed mod needs a build", changed == Some(Need::ModsChanged), changed);
    expect("a start refuses to play the stale build", horizon_native::for_launch(&inst, &base).await.is_err(), "it played");
    horizon_native::skip_once(&inst.id);
    let plain = horizon_native::for_launch(&inst, &base).await;
    expect("play once without native mods", matches!(plain, Ok(None)), plain.map_err(|e| e.to_string()));
    std::fs::write(&file, before).unwrap();
    let back = horizon_native::status(&inst).await.map(|s| s.need).ok();
    expect("the same files are up to date again", back == Some(Need::UpToDate), back);

    let cache = horizon_native::cache(false).await;
    println!("     cache: {cache:?}");
    finish()
}

fn finish() {
    let (p, f) = (PASSED.load(Ordering::Relaxed), FAILED.load(Ordering::Relaxed));
    println!("\n{p} passed, {f} failed");
    if f > 0 {
        std::process::exit(1);
    }
}
