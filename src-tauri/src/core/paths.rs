//! Where everything lives on disk. The layout hasn't changed since launcher 0.1, so old data folders keep working.
//!
//! Two roots: `root()` is the launcher's own folder (settings, accounts, skins and other small files, always in
//! %APPDATA%), and `games()` is where instances, hosted servers and the shared game files live. They are the same
//! folder until the player picks another one in Settings → Storage (see `core::storage`); that choice is kept in
//! `<root>/games-folder.json` and read once at start-up by `ctx::init`.

use std::path::{Path, PathBuf};
use std::sync::RwLock;

use super::ctx::ctx;

/// The folders that live under `games()` and move with it, in the order they are moved.
pub const GAME_FOLDERS: &[&str] = &["instances", "servers", "libraries", "assets", "versions", "java", "cache"];

/// The chosen games folder; None = `root()`.
static GAMES: RwLock<Option<PathBuf>> = RwLock::new(None);

/// The file under the launcher folder that names the games folder.
pub fn games_folder_file(home: &Path) -> PathBuf {
    home.join("games-folder.json")
}

/// The games folder chosen for the launcher folder `home`, if one other than `home` itself was chosen.
/// A missing or unreadable file means the default.
pub fn read_games_folder(home: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(games_folder_file(home)).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let folder = PathBuf::from(v.get("folder")?.as_str()?.trim());
    (folder.is_absolute() && folder != home).then_some(folder)
}

/// Uses `folder` for `games()` from now on (None = the launcher folder). Doesn't move or save anything.
pub fn set_games(folder: Option<PathBuf>) {
    *GAMES.write().unwrap() = folder;
}

/// Development builds keep data in `ResinCraft/.dev-data`;
/// release builds use `%APPDATA%\ResinCraft Launcher`.
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
/// Where instances, hosted servers and the shared game files live.
pub fn games() -> PathBuf {
    GAMES.read().unwrap().clone().unwrap_or_else(root)
}
pub fn instances() -> PathBuf {
    games().join("instances")
}
pub fn servers() -> PathBuf {
    games().join("servers")
}
pub fn libraries() -> PathBuf {
    games().join("libraries")
}
pub fn assets() -> PathBuf {
    games().join("assets")
}
pub fn versions() -> PathBuf {
    games().join("versions")
}
pub fn java() -> PathBuf {
    games().join("java")
}
pub fn cache() -> PathBuf {
    games().join("cache")
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

#[cfg(test)]
mod tests {
    use super::*;

    fn home(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("resin-paths-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn games_folder_defaults_to_the_launcher_folder() {
        let h = home("default");
        assert_eq!(read_games_folder(&h), None, "no file");
        std::fs::write(games_folder_file(&h), "not json").unwrap();
        assert_eq!(read_games_folder(&h), None, "unreadable file");
        std::fs::write(games_folder_file(&h), r#"{"folder":"relative\dir"}"#).unwrap();
        assert_eq!(read_games_folder(&h), None, "relative path");
        let same = serde_json::json!({ "folder": h.to_string_lossy() }).to_string();
        std::fs::write(games_folder_file(&h), same).unwrap();
        assert_eq!(read_games_folder(&h), None, "the launcher folder itself");
        let _ = std::fs::remove_dir_all(&h);
    }

    #[test]
    fn games_folder_is_read_from_its_file() {
        let h = home("chosen");
        let chosen = h.join("elsewhere").join("ResinCraft");
        std::fs::write(games_folder_file(&h), serde_json::json!({ "folder": chosen.to_string_lossy() }).to_string()).unwrap();
        assert_eq!(read_games_folder(&h), Some(chosen));
        let _ = std::fs::remove_dir_all(&h);
    }
}
