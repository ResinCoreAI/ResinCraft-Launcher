//! Headless end-to-end check of the one-click multi-core setup (no window needed). Use RESIN_DATA_DIR for a throwaway folder:
//!   cargo run --example check_multicore -- game [mc] [loader]    plan, install, boot the game, undo, boot it again
//!   cargo run --example check_multicore -- server <id>           moves an existing stopped plugin server to Folia and back, starting it each time
//!   cargo run --example check_multicore -- modserver [mc] [type] plan, install and undo on a new mod server, then runs its jar with the setup
//! `server` never accepts the Minecraft EULA: the server must already have it. `modserver` doesn't either, so that
//! server stops at the EULA message, after the loader has loaded and checked its mods.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use resincraft_lib::content::multicore;
use resincraft_lib::core::types::{LoaderType, NewInstance, NewServer, ServerKind};
use resincraft_lib::core::{ctx, events, instances, paths, settings};
use resincraft_lib::game::launch;
use resincraft_lib::player::accounts::LaunchAuth;
use resincraft_lib::server::{create, multicore as server_multicore, plugins, runtime, store};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let code = match run().await {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("CHECK FAILED: {e:#}");
            1
        }
    };
    std::process::exit(code);
}

async fn run() -> Result<()> {
    let data_dir = std::env::var_os("RESIN_DATA_DIR").map(Into::into).unwrap_or_else(|| paths::default_data_dir(None));
    std::fs::create_dir_all(&data_dir)?;
    ctx::init(data_dir, None);
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize, d: &str| argv.get(i).cloned().unwrap_or_else(|| d.to_string());
    match arg(0, "").as_str() {
        "game" => game(&arg(1, "1.21.1"), &arg(2, "fabric")).await,
        "server" => folia(&arg(1, "")).await,
        "modserver" => mod_server(&arg(1, "1.21.1"), &arg(2, "fabric")).await,
        _ => bail!("use game, server or modserver"),
    }
}

fn show(plan: &multicore::Plan) {
    println!("  reason {:?}, undo {:?}", plan.reason, plan.undo);
    for i in &plan.items {
        println!("  {:7} {:10} {} {} {}", i.slot, i.state, i.title.as_deref().unwrap_or("-"), i.version.as_deref().unwrap_or(""), i.with.as_deref().unwrap_or(""));
    }
}

async fn boot(id: &str, booted: &Arc<AtomicBool>, stop: &mut mpsc::UnboundedReceiver<()>) -> Result<()> {
    booted.store(false, Ordering::SeqCst);
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
    launch::launch_instance(id, &s, auth, launch::LaunchHooks::default()).await?;
    if tokio::time::timeout(Duration::from_secs(240), stop.recv()).await.is_err() {
        launch::kill_instance(id, None);
        let _ = tokio::time::timeout(Duration::from_secs(30), stop.recv()).await;
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    if !booted.load(Ordering::SeqCst) {
        bail!("the game never reached the title screen stage");
    }
    Ok(())
}

async fn game(mc: &str, loader: &str) -> Result<()> {
    let loader: LoaderType = serde_json::from_value(serde_json::Value::String(loader.into()))?;
    let inst = instances::create(NewInstance { name: format!("Multicore {mc}"), mc_version: mc.into(), loader, loader_version: None, color: None, group: None }, |_| {}).await?;
    let booted = Arc::new(AtomicBool::new(false));
    let (tx, mut stop) = mpsc::unbounded_channel::<()>();
    {
        let (booted, id) = (booted.clone(), inst.id.clone());
        events::set_sink(move |channel, payload| match channel {
            "log" => {
                for l in payload["lines"].as_array().into_iter().flatten() {
                    let text = l["text"].as_str().unwrap_or_default();
                    if text.contains("Loading ") && text.contains(" mods") || text.to_lowercase().contains("sodium") && text.contains("OpenGL") || text.contains("Incompatible") {
                        println!("  [game] {}", text.chars().take(200).collect::<String>());
                    }
                    // Past mod loading and mixins: the resources and the window are up.
                    if !booted.load(Ordering::SeqCst) && text.contains("Created:") && text.contains("atlas") {
                        booted.store(true, Ordering::SeqCst);
                        println!("  [game] {}", text.chars().take(200).collect::<String>());
                        launch::kill_instance(&id, None);
                    }
                }
            }
            "game" => {
                let state = payload["state"].as_str().unwrap_or_default();
                println!("  [game] state {state}");
                if state == "stopped" || state == "crashed" {
                    let _ = tx.send(());
                }
            }
            _ => {}
        });
    }
    let folder = paths::game_dir(&inst.id).join("mods");
    let jars = || std::fs::read_dir(&folder).map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect::<Vec<_>>()).unwrap_or_default();

    println!("plan");
    let plan = multicore::instance_plan(&inst.id).await?;
    show(&plan);
    println!("install");
    let after = multicore::instance_apply(&inst.id).await?;
    show(&after);
    println!("  mods: {:?}", jars());
    if jars().is_empty() || after.undo.is_empty() {
        bail!("nothing was installed");
    }
    println!("boot with the mods");
    boot(&inst.id, &booted, &mut stop).await?;
    println!("undo");
    let undone = multicore::instance_undo(&inst.id).await?;
    show(&undone);
    println!("  mods: {:?}", jars());
    if !jars().is_empty() || !undone.undo.is_empty() {
        bail!("undo left something behind");
    }
    println!("boot without them");
    boot(&inst.id, &booted, &mut stop).await?;
    tokio::fs::remove_dir_all(paths::instance_dir(&inst.id)).await.ok();
    println!("GAME OK");
    Ok(())
}

async fn start_until_ready(id: &str) -> Result<()> {
    runtime::start(id).await?;
    let state = runtime::wait_state(id, &["running", "stopped", "crashed"], Duration::from_secs(420)).await?;
    let log = runtime::log(id);
    for l in log.iter().filter(|l| l.text.contains("Done (") || l.text.contains("This server is running") || l.text.contains("Folia") && l.text.contains("version")).take(4) {
        println!("  [server] {}", l.text.chars().take(160).collect::<String>());
    }
    if state != "running" {
        for l in log.iter().rev().take(20).rev() {
            println!("  {}", l.text);
        }
        bail!("the server {state} before it was ready");
    }
    runtime::stop(id)?;
    runtime::wait_state(id, &["stopped", "crashed"], Duration::from_secs(120)).await?;
    Ok(())
}

async fn folia(id: &str) -> Result<()> {
    if !store::eula_accepted(id).await {
        bail!("{id} hasn't accepted the EULA; this check won't do that for you");
    }
    let dir = store::dir(id)?;
    let was = store::get(id).await?;
    println!("before: {} {}", was.kind.label(), was.mc_version);
    let plan = server_multicore::plan(id).await?;
    println!("  plan: {}", serde_json::to_string(&plan)?);
    if server_multicore::move_to_folia(id, false).await.is_ok() {
        bail!("moved without confirmation");
    }
    if store::get(id).await?.kind != was.kind {
        bail!("the type changed without confirmation");
    }
    println!("start on {}", was.kind.label());
    start_until_ready(id).await?;
    println!("move");
    let moved = server_multicore::move_to_folia(id, true).await?;
    println!("  {}", serde_json::to_string(&moved)?);
    let now = store::get(id).await?;
    println!("  now {} {:?}, backups: {:?}", now.kind.label(), now.launch.jar, runtime::list_backups(id).await?.len());
    if now.kind != ServerKind::Folia {
        bail!("still not Folia");
    }
    println!("start on Folia");
    start_until_ready(id).await?;
    println!("undo");
    let back = server_multicore::undo_folia(id).await?;
    println!("  {}", serde_json::to_string(&back)?);
    let now = store::get(id).await?;
    if now.kind != was.kind || now.launch.jar != was.launch.jar || now.loader_version != was.loader_version {
        bail!("not what it was: {:?}", now.kind);
    }
    if dir.join("folia.jar").exists() {
        bail!("folia.jar is still there");
    }
    println!("start on {} again", now.kind.label());
    start_until_ready(id).await?;
    println!("FOLIA OK");
    Ok(())
}

async fn mod_server(mc: &str, kind: &str) -> Result<()> {
    let kind: ServerKind = serde_json::from_value(serde_json::Value::String(kind.into()))?;
    let made = create::create(NewServer { name: format!("Multicore {mc}"), kind: Some(kind), mc_version: Some(mc.into()), loader_version: None, memory_mb: Some(2048), port: Some(25878), source: None }).await?;
    let id = made.server.meta.id.clone();
    let names = |list: Vec<resincraft_lib::core::types::ServerAddon>| list.into_iter().map(|a| a.item.filename).collect::<Vec<_>>();
    println!("plan");
    let plan = server_multicore::plan(&id).await?;
    show(&plan.plan);
    println!("install");
    let after = server_multicore::apply(&id).await?;
    show(&after.plan);
    println!("  mods: {:?}", names(plugins::list(&id, None).await?));
    if after.plan.undo.is_empty() {
        bail!("nothing was installed");
    }
    // The loader checks every mod before Minecraft looks at eula.txt, so this proves they load without accepting it.
    let meta = store::get(&id).await?;
    let java = resincraft_lib::server::installers::choose_java(&meta).await?;
    let out = tokio::process::Command::new(java).args(runtime::launch_args(&meta)?).current_dir(store::dir(&id)?).kill_on_drop(true).output();
    let out = tokio::time::timeout(Duration::from_secs(180), out).await.map_err(|_| anyhow!("the server neither failed nor stopped at the EULA"))??;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    for l in text.lines().filter(|l| l.contains("Loading ") || l.contains("mods:") || l.contains("EULA") || l.contains("Incompatible") || l.contains("Mixin apply")) {
        println!("  [server] {}", l.chars().take(200).collect::<String>());
    }
    if !text.contains("EULA") && !text.contains("eula") {
        println!("{}", text.lines().rev().take(25).collect::<Vec<_>>().join("\n"));
        bail!("the server didn't get as far as the EULA check");
    }
    println!("undo");
    let undone = server_multicore::undo(&id).await?;
    show(&undone.plan);
    let left = names(plugins::list(&id, None).await?);
    println!("  mods: {left:?}");
    if !left.is_empty() {
        bail!("undo left something behind");
    }
    tokio::fs::remove_dir_all(store::dir(&id)?).await.ok();
    println!("MOD SERVER OK");
    Ok(())
}
