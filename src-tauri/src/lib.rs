//! ResinCraft Launcher: Tauri shell + Rust engine.
//!
//! The React UI talks to the engine through one command, `ipc(channel, args)`, using the same channel
//! names as the Electron build. Feature groups:
//! - `app_cmds`: app, settings, instance CRUD
//! - `game`: Minecraft versions, mod loaders, Java, launching
//! - `content`: Modrinth / CurseForge / FTB / ATLauncher / Technic, modpack import & export
//! - `player`: accounts, worlds, servers, screenshots, notes
//! - `server`: hosting Minecraft servers on this computer
//! - `files`: the file manager and text editor for instance and server folders
//! - `updater`: checking for and installing new ResinCraft versions

pub mod app_cmds;
pub mod content;
pub mod core;
pub mod files;
pub mod game;
pub mod player;
pub mod server;
pub mod updater;

use serde_json::Value;
use tauri::Manager;

#[tauri::command]
async fn ipc(channel: String, args: Vec<Value>) -> Result<Value, Value> {
    core::router::dispatch(&channel, core::Args::new(args)).await.map_err(|e| {
        eprintln!("[{channel}] {e:#}");
        // {message, key?, params?}: the UI shows `key` in the player's language (see core::text).
        core::text::error_payload(&e)
    })
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let data_dir = core::paths::default_data_dir(app.path().config_dir().ok());
            std::fs::create_dir_all(&data_dir)?;
            core::ctx::init(data_dir, Some(app.handle().clone()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![ipc])
        .build(tauri::generate_context!())
        .expect("error while running ResinCraft")
        .run(|_, event| {
            // Hosted servers would otherwise keep running (and holding their port) after the launcher closes.
            if let tauri::RunEvent::Exit = event {
                tauri::async_runtime::block_on(server::runtime::shutdown_all(std::time::Duration::from_secs(20)));
            }
        });
}
