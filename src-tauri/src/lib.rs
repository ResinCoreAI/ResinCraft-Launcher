//! ResinCraft Launcher: Tauri shell + Rust engine.
//!
//! The React UI talks to the engine through one command, `ipc(channel, args)`, using the same channel
//! names as the Electron build. Feature groups:
//! - `app_cmds`: app, settings, instance CRUD
//! - `game`: Minecraft versions, mod loaders, Java, launching
//! - `content`: Modrinth / CurseForge / FTB / ATLauncher / Technic, modpack import & export
//! - `player`: accounts, worlds, servers, screenshots, notes

pub mod app_cmds;
pub mod content;
pub mod core;
pub mod game;
pub mod player;

use serde_json::Value;
use tauri::Manager;

#[tauri::command]
async fn ipc(channel: String, args: Vec<Value>) -> Result<Value, String> {
    core::router::dispatch(&channel, core::Args::new(args)).await.map_err(|e| {
        eprintln!("[{channel}] {e:#}");
        e.to_string()
    })
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let data_dir = core::paths::default_data_dir(app.path().config_dir().ok());
            std::fs::create_dir_all(&data_dir)?;
            core::ctx::init(data_dir, Some(app.handle().clone()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![ipc])
        .run(tauri::generate_context!())
        .expect("error while running ResinCraft");
}
