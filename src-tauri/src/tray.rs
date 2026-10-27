//! The system tray icon for tray mode (Settings → Appearance): closing the window keeps ResinCraft running in the
//! background (hosted servers, the Discord bot, downloads), and the icon brings the window back or quits.
//! The icon only shows while tray mode is on.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

const ID: &str = "main";

static QUITTING: AtomicBool = AtomicBool::new(false);

/// Whether the player chose Quit (the window's close then really closes).
pub fn quitting() -> bool {
    QUITTING.load(Ordering::SeqCst)
}

/// Quits for real (tray mode would otherwise only hide the window), e.g. so the uninstaller can run.
pub fn quit(app: &AppHandle) {
    QUITTING.store(true, Ordering::SeqCst);
    app.exit(0);
}

/// Brings the main window back (from the tray, minimised, or behind other windows).
pub fn show(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open ResinCraft", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit ResinCraft", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut tray = TrayIconBuilder::with_id(ID)
        .tooltip("ResinCraft")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "open" => show(app),
            "quit" => self::quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                show(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    let tray = tray.build(app)?;
    let _ = tray.set_visible(false);
    Ok(())
}

/// Shows the tray icon while tray mode is on (at start, and when the setting changes).
pub fn sync(on: bool) {
    if let Some(tray) = crate::core::ctx::app().and_then(|a| a.tray_by_id(ID)) {
        let _ = tray.set_visible(on);
    }
}
