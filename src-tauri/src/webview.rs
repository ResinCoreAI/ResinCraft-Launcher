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

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn config_starts_webview2_with_the_same_arguments() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["app"]["windows"][0]["additionalBrowserArgs"], super::BROWSER_ARGS);
    }
}
