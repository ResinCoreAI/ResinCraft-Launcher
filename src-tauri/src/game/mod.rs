//! Minecraft versions, mod loaders, Java and launching (`versions:*`, `java:*`, `instances:launch|kill|running|runs`).

pub mod commands;
pub mod crashfix;
pub mod forge;
pub mod horizon;
pub mod horizon_native;
pub mod jarmods;
pub mod java;
pub mod launch;
pub mod liteloader;
pub mod loaders;
pub mod mojang;
pub mod optifine;
pub mod priority;
pub mod versionpatch;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{bail, Result};
use serde_json::Value;
use tauri::Manager;

use crate::core::router::{done, json};
use crate::core::types::LoaderType;
use crate::core::{ctx, settings, shell, Args};

pub async fn handle(channel: &str, args: Args) -> Result<Value> {
    match channel {
        "versions:list" => json(mojang::list_versions().await?),
        "versions:loaders" => {
            let loader: LoaderType = args.get(0)?;
            let mc: String = args.get(1)?;
            json(loaders::loader_versions(loader, &mc).await?)
        }
        // Instance settings → Version: what the game starts with, jar mods, and jars added as libraries.
        "versions:resolved" => json(versionpatch::resolved(&args.get::<String>(0)?).await?),
        "versions:browseJars" => {
            let picked = shell::pick_files("Choose jar mods", &[("Jar mods", &["jar", "zip"])]).await?;
            json(picked.into_iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>())
        }
        "versions:addJarMods" => {
            let files: Vec<String> = args.get(1)?;
            json(jarmods::add(&args.get::<String>(0)?, &files.iter().map(std::path::PathBuf::from).collect::<Vec<_>>()).await?)
        }
        "versions:removeJarMod" => json(jarmods::remove(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        "versions:setJarMods" => json(jarmods::set(&args.get::<String>(0)?, args.get(1)?).await?),
        "versions:addLibrary" => {
            let picked = shell::pick_file("Choose a library jar", &[("Java library", &["jar"])]).await?;
            json(match picked {
                Some(p) => Some(versionpatch::add_local_library(&p).await?),
                None => None,
            })
        }

        "java:list" => json(java::detect_java().await?),
        "java:browse" => {
            let filters: &[shell::Filter] = if cfg!(windows) { &[("Java", &["exe"])] } else { &[] };
            let picked = shell::pick_file("Choose a Java executable", filters).await?;
            json(picked.map(|p| p.to_string_lossy().into_owned()))
        }

        // (instance id, account id?): the account plays this once without becoming the active one.
        // A Fix button in the "Minecraft crashed" window: [instance id, fix].
        "instances:crashFix" => {
            crashfix::apply(&args.get::<String>(0)?, &args.get::<crashfix::Fix>(1)?).await?;
            done()
        }
        // Downloads the instance's game files again (Instance → Reinstall).
        "instances:reinstall" => {
            launch::reinstall(&args.get::<String>(0)?).await?;
            done()
        }
        "instances:launch" => {
            launch_as(&args.get::<String>(0)?, args.get::<Option<String>>(1)?.as_deref(), None).await?;
            done()
        }
        // (instance id, run id?): without a run id, every run of the instance stops.
        "instances:kill" => {
            launch::kill_instance(&args.get::<String>(0)?, args.get::<Option<String>>(1)?.as_deref());
            done()
        }
        "instances:running" => json(running_ids()),
        "instances:runs" => json(launch::runs()),

        _ => bail!("Unknown request: {channel}"),
    }
}

/// Launches an instance with the player's account and the "on launch" window behaviour from Settings.
/// `server` ("host:port") joins that server once the game is up.
pub async fn launch(id: &str, server: Option<String>) -> Result<()> {
    launch_as(id, None, server.map(launch::Target::Server)).await
}

/// Like `launch`, with a given account instead of the active one.
pub async fn launch_as(id: &str, account: Option<&str>, target: Option<launch::Target>) -> Result<()> {
    launch_then(id, account, target, None).await
}

/// Like `launch_as`; `after` runs once the game has exited (not when the launch fails).
pub async fn launch_then(id: &str, account: Option<&str>, target: Option<launch::Target>, after: Option<Box<dyn FnOnce() + Send>>) -> Result<()> {
    crate::core::storage::ensure_not_moving()?;
    if crate::content::packswitch::is_switching(id) {
        crate::bail_tr!("packswitch.wait", "Wait for the modpack version switch to finish first");
    }
    let settings = settings::get().await;
    let auth = crate::player::accounts::launch_auth_for(account).await?;
    let mode = settings.on_launch.clone();
    let exit_mode = mode.clone();
    let closing_mode = mode.clone();
    // Set once the launcher came back as the game closed its window, so the end of Java doesn't pull it to the front
    // a second time.
    let back = Arc::new(AtomicBool::new(false));
    let back_at_exit = back.clone();
    let hooks = launch::LaunchHooks {
        on_started: Some(Box::new(move || {
            if let Some(win) = main_window() {
                match mode.as_str() {
                    "minimize" => {
                        let _ = win.minimize();
                    }
                    "hide" => {
                        let _ = win.hide();
                        crate::webview::hidden();
                        return;
                    }
                    _ => return,
                }
                low_memory(true);
            }
        })),
        // The player quit the game: its window goes now, so the launcher comes back now (if no other game is running),
        // not about 5 s later when Java has ended.
        on_closing: Some(Box::new(move || {
            let others = launch::runs().iter().filter(|r| r.state == "running").count() > 1;
            if closing_mode != "keep" && !others {
                if let Some(app) = ctx::app() {
                    back.store(true, Ordering::SeqCst);
                    crate::webview::show(&app);
                }
            }
        })),
        on_exit: Some(Box::new(move |_code| {
            if let Some(f) = after {
                f();
            }
            if exit_mode != "keep" && running_ids().is_empty() && !back_at_exit.load(Ordering::SeqCst) {
                if let Some(app) = ctx::app() {
                    crate::webview::show(&app);
                }
            }
        })),
    };
    launch::launch_instance_to(id, &settings, auth, hooks, target.as_ref()).await
}

fn main_window() -> Option<tauri::WebviewWindow> {
    ctx::app()?.get_webview_window("main")
}

/// Out of sight while a game runs: the launcher gives its memory back to the game.
fn low_memory(low: bool) {
    crate::webview::set_low_memory(low);
}

/// Ids of instances whose game is currently running.
pub fn running_ids() -> Vec<String> {
    launch::running_instances()
}
