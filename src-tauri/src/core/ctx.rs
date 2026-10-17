//! Process-wide state. Initialised once at startup (from Tauri's setup, or from a test/example binary).

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use tauri::AppHandle;

pub const USER_AGENT: &str = concat!("ResinCraftLauncher/", env!("CARGO_PKG_VERSION"), " (by Puirkle)");

pub struct Ctx {
    /// Root of all launcher data (instances, libraries, assets, settings…).
    pub data_dir: PathBuf,
    /// The running app, or None when the engine runs headless (smoke tests).
    pub app: Option<AppHandle>,
    pub http: reqwest::Client,
    /// Development build: relaxes checks meant for releases (e.g. offline profiles).
    pub dev: bool,
}

static CTX: OnceLock<Ctx> = OnceLock::new();

pub fn init(data_dir: PathBuf, app: Option<AppHandle>) {
    let http = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(15))
        .gzip(true)
        .build()
        .expect("HTTP client");
    let _ = CTX.set(Ctx { data_dir, app, http, dev: cfg!(debug_assertions) });
}

pub fn ctx() -> &'static Ctx {
    CTX.get().expect("ResinCraft core used before init()")
}

pub fn app() -> Option<&'static AppHandle> {
    ctx().app.as_ref()
}

pub fn http() -> &'static reqwest::Client {
    &ctx().http
}
