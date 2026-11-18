//! The launcher's WebView2 memory: what it starts with, and giving memory back while nobody looks at the window.

/// Every window's WebView2 must start with the same arguments (a second set fails to open): this is
/// `additionalBrowserArgs` in tauri.conf.json. Tauri's default `--disable-features` list plus V8's smaller-heap
/// mode, which takes about 30 % off the launcher's idle memory.
#[cfg(windows)]
pub const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --js-flags=--optimize-for-size";

/// Asks WebView2 to trim the page's memory (`low`) or go back to normal. Only for a window that's out of sight:
/// a game took over, or it went to the tray. An older WebView2 without the setting just stays as it is.
#[cfg(windows)]
pub fn set_low_memory(low: bool) {
    use tauri::Manager;
    use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL};
    use windows_core::Interface;

    let Some(window) = crate::core::ctx::app().and_then(|a| a.get_webview_window("main")) else { return };
    let _ = window.with_webview(move |w| unsafe {
        let level = if low { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW } else { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL };
        if let Some(core) = w.controller().CoreWebView2().ok().and_then(|c| c.cast::<ICoreWebView2_19>().ok()) {
            let _ = core.SetMemoryUsageTargetLevel(level);
        }
    });
}

#[cfg(not(windows))]
pub fn set_low_memory(_low: bool) {}

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use tauri::{AppHandle, Manager};

/// How long the window stays out of sight before it closes: long enough that a quick look at the tray or a game
/// that crashes at start doesn't pay for a reload.
const UNLOAD_AFTER: std::time::Duration = std::time::Duration::from_secs(30);

/// The main window is closed to save memory (not quit): the app keeps running and `show` opens it again.
static UNLOADED: AtomicBool = AtomicBool::new(false);
/// Bumped whenever the window is shown, so an unload planned before that is called off.
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// Where the window was when it closed, so it comes back the same way.
static PLACE: Mutex<Option<Place>> = Mutex::new(None);

#[derive(Clone, Copy)]
struct Place {
    maximized: bool,
    size: tauri::PhysicalSize<u32>,
    position: tauri::PhysicalPosition<i32>,
}

/// The main window is closed to save memory; while it is, the app mustn't quit when its last window goes.
pub fn unloaded() -> bool {
    UNLOADED.load(Ordering::SeqCst)
}

/// The window just went out of sight: trim its memory now, and close it entirely if it stays hidden
/// (Settings → Appearance → Free memory while hidden).
pub fn hidden() {
    set_low_memory(true);
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(UNLOAD_AFTER).await;
        if GENERATION.load(Ordering::SeqCst) != generation || !crate::core::settings::free_memory_hidden() {
            return;
        }
        let Some(window) = crate::core::ctx::app().and_then(|a| a.get_webview_window("main")) else { return };
        // Shown again, or only minimised (a closed window would lose its taskbar button): leave it.
        if window.is_visible().unwrap_or(true) || window.is_minimized().unwrap_or(false) {
            return;
        }
        if let (Ok(size), Ok(position)) = (window.outer_size(), window.outer_position()) {
            *PLACE.lock().unwrap() = Some(Place { maximized: window.is_maximized().unwrap_or(true), size, position });
        }
        UNLOADED.store(true, Ordering::SeqCst);
        if window.destroy().is_err() {
            UNLOADED.store(false, Ordering::SeqCst);
        }
    });
}

/// Brings the main window back: from the tray, minimised, behind other windows, or closed to save memory (then
/// it's made again, which takes about a second). True when it was made again, so the page hasn't loaded yet.
pub fn show(app: &AppHandle) -> bool {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        set_low_memory(false);
        return false;
    }
    if !UNLOADED.load(Ordering::SeqCst) {
        return false;
    }
    // Made off the event loop's thread: building a webview on it from its own event handler can deadlock.
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(config) = app.config().app.windows.iter().find(|w| w.label == "main").cloned() else { return };
        let place = *PLACE.lock().unwrap();
        let mut config = config;
        if let Some(p) = place {
            config.maximized = p.maximized;
            config.center = false;
        }
        match tauri::WebviewWindowBuilder::from_config(&app, &config).and_then(|b| b.build()) {
            Ok(w) => {
                if let Some(p) = place.filter(|p| !p.maximized) {
                    let _ = w.set_size(p.size);
                    let _ = w.set_position(p.position);
                }
                let _ = w.set_focus();
                UNLOADED.store(false, Ordering::SeqCst);
            }
            Err(e) => eprintln!("[window] couldn't open the launcher again: {e}"),
        }
    });
    true
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn config_starts_webview2_with_the_same_arguments() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["app"]["windows"][0]["additionalBrowserArgs"], super::BROWSER_ARGS);
    }
}
