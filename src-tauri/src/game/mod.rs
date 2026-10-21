//! Minecraft versions, mod loaders, Java and launching (`versions:*`, `java:*`, `instances:launch|kill|running|runs`).

pub mod forge;
pub mod jarmods;
pub mod java;
pub mod launch;
pub mod liteloader;
pub mod loaders;
pub mod mojang;
pub mod optifine;
pub mod versionpatch;

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
    crate::core::storage::ensure_not_moving()?;
    if crate::content::packswitch::is_switching(id) {
        crate::bail_tr!("packswitch.wait", "Wait for the modpack version switch to finish first");
    }
    let settings = settings::get().await;
    let auth = crate::player::accounts::launch_auth_for(account).await?;
    let mode = settings.on_launch.clone();
    let exit_mode = mode.clone();
    let hooks = launch::LaunchHooks {
        on_started: Some(Box::new(move || {
            if let Some(win) = main_window() {
                match mode.as_str() {
                    "minimize" => {
                        let _ = win.minimize();
                    }
                    "hide" => {
                        let _ = win.hide();
                    }
                    _ => {}
                }
            }
        })),
        on_exit: Some(Box::new(move |_code| {
            if exit_mode != "keep" && running_ids().is_empty() {
                if let Some(win) = main_window() {
                    let _ = win.show();
                    let _ = win.unminimize();
                }
            }
        })),
    };
    launch::launch_instance_to(id, &settings, auth, hooks, target.as_ref()).await
}

fn main_window() -> Option<tauri::WebviewWindow> {
    ctx::app()?.get_webview_window("main")
}

/// Ids of instances whose game is currently running.
pub fn running_ids() -> Vec<String> {
    launch::running_instances()
}
