//! The launcher's WebView2 memory: what it starts with, and giving memory back while nobody looks at the window.

/// Every window's WebView2 must start with the same arguments (a second set fails to open): this is
/// `additionalBrowserArgs` in tauri.conf.json. Tauri's default `--disable-features` list plus V8's smaller-heap
/// mode, which takes about 30 % off the launcher's idle memory.
#[cfg(windows)]
pub const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --js-flags=--optimize-for-size";

/// Asks WebView2 to trim the page's memory (`low`: a game took over, the window went to the tray or behind another
/// program) or to the level for using it (`false`). That level is low as well while Settings → Appearance → Free
/// memory while hidden is on (`lean_in_use`). An older WebView2 without the setting just stays as it is.
#[cfg(windows)]
pub fn set_low_memory(low: bool) {
    use tauri::Manager;
    use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL};
    use windows_core::Interface;

    let low = low || lean_in_use();
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

/// The page also runs at WebView2's low memory level while it's being used, from a moment after the start (the
/// first page loads at the normal level). Measured on the UI rework with 9 instances, every page opened twice: the
/// whole launcher (Task Manager) peaked at 175-193 MB at the normal level and stayed at 147-158 MB, against a 137 MB
/// peak and 110-128 MB at the low level; pages opened as fast (4-47 ms), scrolling had no slow frames either way,
/// and timers ran on time. Part of Settings → Appearance → Free memory while hidden.
static LEAN_FROM_NOW: AtomicBool = AtomicBool::new(false);
const LEAN_AFTER_START: std::time::Duration = std::time::Duration::from_secs(3);

fn lean_in_use() -> bool {
    LEAN_FROM_NOW.load(Ordering::SeqCst) && crate::core::settings::free_memory_hidden()
}

/// The launcher has started: after `LEAN_AFTER_START` its page goes to the level for using it.
pub fn started() {
    tauri::async_runtime::spawn(async {
        tokio::time::sleep(LEAN_AFTER_START).await;
        LEAN_FROM_NOW.store(true, Ordering::SeqCst);
        if !crate::game::running_ids().is_empty() {
            return;
        }
        set_low_memory(false);
    });
}

/// How long the window stays out of sight before it closes: long enough that a quick look at the tray or a game
/// that crashes at start doesn't pay for a reload.
const UNLOAD_AFTER: std::time::Duration = std::time::Duration::from_secs(30);

/// How long the launcher may be out of focus (the player is in another program) before its page is asked to use
/// little memory.
const TRIM_AFTER_BLUR: std::time::Duration = std::time::Duration::from_secs(5);
/// Bumped whenever the main window gains or loses focus, so a trim planned before that is called off.
static FOCUS: AtomicU64 = AtomicU64::new(0);

/// The main window gained or lost focus. Focused: the level for using it at once. Unfocused for `TRIM_AFTER_BLUR`
/// (another program in front, the launcher still open behind it): WebView2's low memory level, which gives back most
/// of the page's memory (Task Manager went from about 105 MB to 13-32 MB for the whole launcher). Part of Settings →
/// Appearance → Free memory while hidden.
pub fn focus_changed(focused: bool) {
    let generation = FOCUS.fetch_add(1, Ordering::SeqCst) + 1;
    if focused {
        set_low_memory(false);
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(TRIM_AFTER_BLUR).await;
        if FOCUS.load(Ordering::SeqCst) == generation && crate::core::settings::free_memory_hidden() {
            set_low_memory(true);
        }
    });
}

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
        // A position far off screen is Windows' parking spot for minimised windows, not a place to come back to.
        if let (Ok(size), Ok(position)) = (window.outer_size(), window.outer_position()) {
            if position.x > -10000 && position.y > -10000 {
                *PLACE.lock().unwrap() = Some(Place { maximized: window.is_maximized().unwrap_or(true), size, position });
            }
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
                // A window made from the background can open minimised (Windows' focus rules): put it back in front,
                // maximised or where it was.
                let _ = w.unminimize();
                match place {
                    Some(p) if !p.maximized => {
                        let _ = w.set_size(p.size);
                        let _ = w.set_position(p.position);
                    }
                    _ if config.maximized => {
                        let _ = w.maximize();
                    }
                    _ => {}
                }
                let _ = w.show();
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
