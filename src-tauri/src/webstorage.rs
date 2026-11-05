//! The launcher's window is served over HTTPS (`https://tauri.localhost`, which Stripe's card form needs for real
//! payments). Before, it was `http://tauri.localhost`, and the browser keeps each origin's storage apart: what the UI
//! kept in `localStorage` (Library groups and order, Discover's site, the server list's hidden and joined servers…)
//! would be gone. So the first start after the change opens the old origin once, in a hidden window, which hands its
//! storage over (`exported`); the launcher's window waits for it (`import`) and keeps every entry it doesn't have yet.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const WINDOW: &str = "storage-export";
const DONE: &str = "webstorage-https.done";

static EXPORTED: Mutex<Option<Value>> = Mutex::new(None);

fn marker() -> PathBuf {
    crate::core::paths::root().join(DONE)
}

fn mark_done() {
    let _ = std::fs::write(marker(), "");
}

/// At start: opens the old origin in a hidden window, once.
pub fn start(app: &AppHandle) {
    if marker().exists() {
        return;
    }
    let built = WebviewWindowBuilder::new(app, WINDOW, WebviewUrl::App("index.html".into())).use_https_scheme(false).visible(false).focused(false).skip_taskbar(true).build();
    if let Err(e) = built {
        eprintln!("[storage] couldn't open the old storage: {e}");
        mark_done();
    }
}

/// The old origin's `localStorage`, from the hidden window.
pub fn exported(entries: Value) {
    *EXPORTED.lock().unwrap() = Some(entries);
    if let Some(w) = crate::core::ctx::app().and_then(|a| a.get_webview_window(WINDOW)) {
        let _ = w.close();
    }
}

/// For the launcher's window: the old storage's entries once (null when there is nothing to carry over). Waits a few
/// seconds for the hidden window; after that (or once it answered) it isn't asked again.
pub async fn import() -> Value {
    if marker().exists() {
        return Value::Null;
    }
    for _ in 0..60 {
        if let Some(v) = EXPORTED.lock().unwrap().take() {
            mark_done();
            return v;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    eprintln!("[storage] the old storage didn't answer");
    mark_done();
    if let Some(w) = crate::core::ctx::app().and_then(|a| a.get_webview_window(WINDOW)) {
        let _ = w.close();
    }
    Value::Null
}
