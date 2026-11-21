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
//! - `friends`: inviting friends into an Open to LAN world with a code
//! - `updater`: checking for and installing new ResinCraft versions
//! - `discord`: Discord Rich Presence for running games

pub mod app_cmds;
pub mod content;
pub mod core;
pub mod discord;
pub mod files;
pub mod friends;
pub mod game;
pub mod mcp;
pub mod migrate;
pub mod openfile;
pub mod player;
pub mod server;
pub mod remote;
pub mod remoteview;
pub mod report;
pub mod tray;
pub mod webview;
pub mod webstorage;
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

/// ResinCraft's identity on the taskbar: the same id tauri.conf.json has (`identifier`), which the installer also
/// puts on ResinCraft's own shortcuts.
#[cfg(windows)]
const APP_ID: &str = "ai.resincore.resincraft";

/// Tells Windows which app this process is, before any window exists. Without it, a launcher started from an
/// instance's desktop shortcut (core/shortcut.rs) is taken for that shortcut: its taskbar button wears the
/// instance's icon instead of ResinCraft's, and doesn't sit with a pinned ResinCraft.
#[cfg(windows)]
fn claim_taskbar_identity() {
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
    // A Store (MSIX) copy already has its identity from the package; setting another one would split its taskbar
    // button from the pinned Store app.
    if crate::updater::packaged() {
        return;
    }
    // Safety: a plain Win32 call with a string that outlives it. A failure only leaves things as they were.
    let _ = unsafe { SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(APP_ID)) };
}

pub fn run() {
    #[cfg(windows)]
    claim_taskbar_identity();
    tauri::Builder::default()
        // A second start (e.g. while hidden in the tray) brings the running ResinCraft back instead, with the file it was
        // asked to open (a theme, backup or modpack double-clicked in Windows).
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let fresh = tray::show(app);
            openfile::opened(args, fresh);
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let data_dir = core::paths::default_data_dir(app.path().config_dir().ok());
            std::fs::create_dir_all(&data_dir)?;
            core::ctx::init(data_dir, Some(app.handle().clone()));
            openfile::at_start();
            // resincraft:// links open this copy. Installed builds only: a test build would take them over.
            #[cfg(all(not(debug_assertions), any(windows, target_os = "linux")))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                if let Err(e) = app.deep_link().register_all() {
                    eprintln!("[links] couldn't register resincraft:// links: {e}");
                }
            }
            tauri::async_runtime::spawn(discord::init());
            tauri::async_runtime::spawn(server::discordbot::init());
            tauri::async_runtime::spawn(remote::init());
            tauri::async_runtime::spawn(mcp::init());
            tauri::async_runtime::spawn(core::autobackup::init());
            tauri::async_runtime::spawn(player::worlds::return_lent_worlds());
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
            // Looked at again (from the tray, the taskbar, or after another program): back to normal memory; left for
            // another program: little memory (see `webview::focus_changed`).
            if let tauri::RunEvent::WindowEvent { label, event: tauri::WindowEvent::Focused(focused), .. } = &event {
                if label == "main" {
                    webview::focus_changed(*focused);
                }
            }
            if let tauri::RunEvent::WindowEvent { label, event: tauri::WindowEvent::CloseRequested { api, .. }, .. } = &event {
                // Only the launcher itself goes to the tray; other windows (sign-in, relay dashboard) really close.
                if label == "main" && core::settings::tray_on() && !tray::quitting() {
                    api.prevent_close();
                    if let Some(w) = app.get_webview_window(label) {
                        let _ = w.hide();
                    }
                    webview::hidden();
                    return;
                }
            }
            // The window closed to save memory: ResinCraft goes on (tray, hosted servers, the game) until it's opened again.
            if let tauri::RunEvent::ExitRequested { api, code: None, .. } = &event {
                if webview::unloaded() {
                    api.prevent_exit();
                    return;
                }
            }
            // Hosted servers would otherwise keep running (and holding their port) after the launcher closes.
            if let tauri::RunEvent::Exit = event {
                discord::shutdown();
                tauri::async_runtime::block_on(friends::shutdown());
                tauri::async_runtime::block_on(server::runtime::shutdown_all(std::time::Duration::from_secs(20)));
            }
        });
}

#[cfg(all(test, windows))]
mod taskbar_identity {
    #[test]
    fn app_id_is_the_configured_identifier() {
        let conf = include_str!("../tauri.conf.json");
        assert!(conf.contains(&format!("\"identifier\": \"{}\"", super::APP_ID)), "APP_ID must match tauri.conf.json's identifier");
    }
}
