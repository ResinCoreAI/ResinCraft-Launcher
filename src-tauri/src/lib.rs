//! ResinCraft Launcher: Tauri shell + Rust engine.
//!
//! The React UI talks to the engine through one command, `ipc(channel, args)`, with the channel names
//! from src/shared/bridge.ts. Feature groups:
//! - `app_cmds`: app, settings, instance CRUD
//! - `game`: Minecraft versions, mod loaders, Java, launching
//! - `content`: Modrinth / CurseForge / FTB / ATLauncher / Technic, modpack import & export
//! - `player`: accounts, worlds, servers, screenshots, notes
//! - `server`: hosting Minecraft servers on this computer
//! - `files`: the file manager and text editor for instance and server folders
//! - `updater`: checking for and installing new ResinCraft versions
//! - `discord`: Discord Rich Presence for running games

pub mod app_cmds;
pub mod content;
pub mod core;
pub mod discord;
pub mod files;
pub mod game;
pub mod migrate;
pub mod openfile;
pub mod player;
pub mod server;
pub mod remote;
pub mod remoteview;
pub mod tray;
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
        // A second start (e.g. while hidden in the tray) brings the running ResinCraft back instead, with the file it was
        // asked to open (a theme, backup or modpack double-clicked in Windows).
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            tray::show(app);
            openfile::opened(args);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let data_dir = core::paths::default_data_dir(app.path().config_dir().ok());
            std::fs::create_dir_all(&data_dir)?;
            core::ctx::init(data_dir, Some(app.handle().clone()));
            openfile::at_start();
            tauri::async_runtime::spawn(discord::init());
            tauri::async_runtime::spawn(server::discordbot::init());
            tauri::async_runtime::spawn(remote::init());
            tauri::async_runtime::spawn(core::autobackup::init());
            tauri::async_runtime::spawn(server::clientfiles::init());
            tauri::async_runtime::spawn(server::listing::watch_joins());
            // Reads the settings once, so the close button knows about tray mode.
            tauri::async_runtime::spawn(async {
                tray::sync(core::settings::get().await.tray);
            });
            tray::build(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![ipc])
        .build(tauri::generate_context!())
        .expect("error while running ResinCraft")
        .run(|app, event| {
            // Tray mode: the close button hides the window; ResinCraft goes on in the background.
            if let tauri::RunEvent::WindowEvent { label, event: tauri::WindowEvent::CloseRequested { api, .. }, .. } = &event {
                // Only the launcher itself goes to the tray; other windows (sign-in, relay dashboard) really close.
                if label == "main" && core::settings::tray_on() && !tray::quitting() {
                    api.prevent_close();
                    if let Some(w) = app.get_webview_window(label) {
                        let _ = w.hide();
                    }
                    return;
                }
            }
            // Hosted servers would otherwise keep running (and holding their port) after the launcher closes.
            if let tauri::RunEvent::Exit = event {
                discord::shutdown();
                tauri::async_runtime::block_on(server::runtime::shutdown_all(std::time::Duration::from_secs(20)));
            }
        });
}
