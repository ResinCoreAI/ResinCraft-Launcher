//! Minecraft versions, mod loaders, Java and launching (`versions:*`, `java:*`, `instances:launch|kill|running`).

pub mod forge;
pub mod java;
pub mod launch;
pub mod loaders;
pub mod mojang;

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

        "java:list" => json(java::detect_java().await?),
        "java:browse" => {
            let filters: &[shell::Filter] = if cfg!(windows) { &[("Java", &["exe"])] } else { &[] };
            let picked = shell::pick_file("Choose a Java executable", filters).await?;
            json(picked.map(|p| p.to_string_lossy().into_owned()))
        }

        "instances:launch" => {
            launch(&args.get::<String>(0)?, None).await?;
            done()
        }
        "instances:kill" => {
            launch::kill_instance(&args.get::<String>(0)?);
            done()
        }
        "instances:running" => json(running_ids()),

        _ => bail!("Unknown request: {channel}"),
    }
}

/// Launches an instance with the player's account and the "on launch" window behaviour from Settings.
/// `server` ("host:port") joins that server once the game is up.
pub async fn launch(id: &str, server: Option<String>) -> Result<()> {
    let settings = settings::get().await;
    let auth = crate::player::accounts::get_launch_auth().await?;
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
    launch::launch_instance_to(id, &settings, auth, hooks, server.as_deref()).await
}

fn main_window() -> Option<tauri::WebviewWindow> {
    ctx::app()?.get_webview_window("main")
}

/// Ids of instances whose game is currently running.
pub fn running_ids() -> Vec<String> {
    launch::running_instances()
}
