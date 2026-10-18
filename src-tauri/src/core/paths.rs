//! Where everything lives on disk. Same layout as the Electron build, so both can share a data folder.

use std::path::PathBuf;

use super::ctx::ctx;

/// Development builds keep data in `ResinCraft/.dev-data` (same as Electron's `npm run dev`);
/// release builds use `%APPDATA%\ResinCraft Launcher`, which is what the Electron release used too.
/// A build made with `RESINCRAFT_DATA_FOLDER` set uses that folder name instead (only for the updater's
/// end-to-end test, so test installs never touch real instances and accounts).
pub fn default_data_dir(config_dir: Option<PathBuf>) -> PathBuf {
    if cfg!(debug_assertions) {
        // Not canonicalize(): on Windows it adds the \?\ long-path prefix, which Java can't load classes from.
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        return manifest.parent().unwrap_or(&manifest).join(".dev-data");
    }
    config_dir.unwrap_or_else(std::env::temp_dir).join(option_env!("RESINCRAFT_DATA_FOLDER").unwrap_or("ResinCraft Launcher"))
}

pub fn root() -> PathBuf {
    ctx().data_dir.clone()
}
pub fn instances() -> PathBuf {
    root().join("instances")
}
pub fn libraries() -> PathBuf {
    root().join("libraries")
}
pub fn assets() -> PathBuf {
    root().join("assets")
}
pub fn versions() -> PathBuf {
    root().join("versions")
}
pub fn java() -> PathBuf {
    root().join("java")
}
pub fn cache() -> PathBuf {
    root().join("cache")
}
pub fn settings_file() -> PathBuf {
    root().join("settings.json")
}
pub fn accounts_file() -> PathBuf {
    root().join("accounts.json")
}
pub fn instance_dir(id: &str) -> PathBuf {
    instances().join(id)
}
/// The instance's `.minecraft` equivalent.
pub fn game_dir(id: &str) -> PathBuf {
    instance_dir(id).join("minecraft")
}
pub fn natives_dir(id: &str) -> PathBuf {
    instance_dir(id).join("natives")
}
