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

/// How long the launcher may be out of focus (the player is in another program) before its page is asked to use
/// little memory.
const TRIM_AFTER_BLUR: std::time::Duration = std::time::Duration::from_secs(5);
/// Bumped whenever the main window gains or loses focus, so a trim planned before that is called off.
static FOCUS: AtomicU64 = AtomicU64::new(0);

/// The main window gained or lost focus. Focused: normal memory at once. Unfocused for `TRIM_AFTER_BLUR` (another
/// program in front, the launcher still open behind it): WebView2's low memory level, which gives back most of the
/// page's memory (Task Manager went from about 105 MB to 13-32 MB for the whole launcher) and takes it back as soon as
/// the window is used again. Part of Settings → Appearance → Free memory while hidden.
pub fn focus_changed(focused: bool) {
    let generation = FOCUS.fetch_add(1, Ordering::SeqCst) + 1;
    if focused {
        set_low_memory(false);
        tauri::async_runtime::spawn(watch_idle(generation));
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(TRIM_AFTER_BLUR).await;
        if FOCUS.load(Ordering::SeqCst) == generation && crate::core::settings::free_memory_hidden() {
            set_low_memory(true);
        }
    });
}

/// How long the launcher may sit in front with nobody using the computer before its page is asked to use little memory.
const TRIM_AFTER_IDLE: std::time::Duration = std::time::Duration::from_secs(60);
/// Input this recent means someone's back: normal memory again.
const BACK_WITHIN: std::time::Duration = std::time::Duration::from_secs(2);

/// While the launcher is in front: low memory once nobody has touched the mouse or keyboard for `TRIM_AFTER_IDLE`
/// (the launcher left open on screen), normal again at the first input. Not while a task runs (a download's progress
/// is being watched). Ends when the window loses focus (`focus_changed` takes over).
async fn watch_idle(generation: u64) {
    let mut low = false;
    while FOCUS.load(Ordering::SeqCst) == generation {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if FOCUS.load(Ordering::SeqCst) != generation {
            return;
        }
        let Some(idle) = input_idle() else { return };
        let busy = crate::core::events::running_tasks() > 0 || !crate::core::settings::free_memory_hidden();
        if let Some(now_low) = idle_decision(idle, low, busy) {
            set_low_memory(now_low);
            low = now_low;
        }
    }
}

/// What `watch_idle` does now: Some(true) to go low, Some(false) back to normal, None to leave it.
fn idle_decision(idle: std::time::Duration, low: bool, busy: bool) -> Option<bool> {
    if low && (idle < BACK_WITHIN || busy) {
        Some(false)
    } else if !low && !busy && idle >= TRIM_AFTER_IDLE {
        Some(true)
    } else {
        None
    }
}

/// How long since the last mouse or keyboard input anywhere on this computer; None where that can't be told.
fn input_idle() -> Option<std::time::Duration> {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::SystemInformation::GetTickCount;
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
        let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
        if !GetLastInputInfo(&mut info).as_bool() {
            return None;
        }
        Some(std::time::Duration::from_millis(GetTickCount().wrapping_sub(info.dwTime) as u64))
    }
    #[cfg(not(windows))]
    None
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

#[cfg(test)]
mod idle_tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn goes_low_after_a_minute_untouched_and_back_at_the_first_input() {
        let s = Duration::from_secs;
        assert_eq!(idle_decision(s(59), false, false), None);
        assert_eq!(idle_decision(s(60), false, false), Some(true));
        // Low stays low while nobody's there, and goes back as soon as someone is.
        assert_eq!(idle_decision(s(300), true, false), None);
        assert_eq!(idle_decision(Duration::from_millis(500), true, false), Some(false));
        // Never while a task runs; a task that starts brings it back.
        assert_eq!(idle_decision(s(600), false, true), None);
        assert_eq!(idle_decision(s(600), true, true), Some(false));
        assert!(input_idle().is_some() || cfg!(not(windows)));
    }
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn config_starts_webview2_with_the_same_arguments() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["app"]["windows"][0]["additionalBrowserArgs"], super::BROWSER_ARGS);
    }
}
