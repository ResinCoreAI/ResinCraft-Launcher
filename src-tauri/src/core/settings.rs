use std::sync::Mutex;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::fsutil::{read_json, write_json};
use super::paths;
use super::types::{EnvVar, LaunchCommands};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// "system" or a theme id (see src/renderer/src/themes.ts).
    pub theme: String,
    /// Holiday themes turn on by themselves for the 15 days before Halloween and Christmas.
    pub seasonal_themes: bool,
    /// The year's holiday the player picked their own theme for (e.g. "halloween-2026"), so it isn't forced.
    pub season_off: String,
    /// The player's own CSS, applied over any theme.
    pub custom_css: String,
    /// Themes the player made in Settings → Appearance → Create theme (the UI owns their shape; see themes.ts).
    /// A theme's uploaded background picture is a file in `<data>/themes` (`app:themeImage`).
    pub custom_themes: Vec<Value>,
    /// Remote control from the web console (console.<relay domain>) while ResinCraft runs, signed in with the same
    /// Minecraft account (Settings → Remote control; off by default).
    pub remote_control: bool,
    /// Remote control may also use the file manager, Java settings and plugins from any link (they can run programs
    /// on this PC). Off by default.
    pub remote_full_control: bool,
    /// UI language: "system" (follow the OS) or a locale code such as "de" or "pt-BR".
    pub language: String,
    pub memory_mb: u32,
    pub jvm_args: String,
    /// ResinCraft's G1GC flags for games (an instance can choose its own; skipped when the JVM arguments pick a
    /// collector themselves).
    pub optimized_flags: bool,
    pub java_path: String,
    pub auto_java: bool,
    /// "keep" | "minimize" | "hide"
    pub on_launch: String,
    pub game_width: u32,
    pub game_height: u32,
    /// Overrides the Client ID built into the app (advanced; not shown in the UI).
    pub ms_client_id: String,
    /// Overrides the CurseForge key built into the app (advanced; not shown in the UI).
    pub curseforge_key: String,
    pub concurrency: usize,
    pub show_snapshots: bool,
    /// Install a new ResinCraft version by itself when the launcher starts (Settings → Updates).
    pub auto_update: bool,
    /// Install a found update in the background without asking, as soon as no game or hosted server is running
    /// (Settings → Updates).
    pub silent_update: bool,
    /// Closing the window keeps ResinCraft running in the system tray (hosted servers, the Discord bot and
    /// downloads go on); Quit is in the tray icon's menu (Settings → Appearance).
    pub tray: bool,
    /// When the same mod is on Modrinth and CurseForge: "ask" | "modrinth" | "curseforge" (Settings → Downloads,
    /// or "Don't ask again" in the duplicate dialog).
    pub preferred_mod_source: String,
    /// Folders besides Downloads where files for Download all are looked for (a browser that saves elsewhere).
    pub watch_folders: Vec<String>,
    /// Look for newer versions of installed modpacks (and instances changed since a server synced) at start and every
    /// few hours (Settings → Updates).
    pub auto_check_packs: bool,
    /// A RAM card on Home: the launcher, running games and servers, the PC (Settings → Appearance; off by default).
    pub show_memory_on_home: bool,
    /// Discord Rich Presence while a game runs (only in builds with a Discord app ID).
    pub discord_presence: bool,
    /// Rich Presence names the server joined; off by default, the address can be private.
    pub discord_show_server: bool,
    /// Rich Presence shows "In the launcher" while ResinCraft is open and no game runs (on by default).
    pub discord_in_launcher: bool,
    /// Which changed defaults this file has had applied (see `upgrade`). Missing in older files, so 0.
    #[serde(default)]
    pub defaults_version: u32,
    /// Environment variables for every game (an instance's own win; Settings → Commands).
    pub env: Vec<EnvVar>,
    /// Pre-launch, wrapper and post-exit commands for instances without their own (Settings → Commands).
    pub commands: LaunchCommands,
}

pub fn total_memory_mb() -> u64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.total_memory() / 1024 / 1024
}

impl Default for Settings {
    fn default() -> Self {
        let total = total_memory_mb() as u32;
        Settings {
            theme: "dark".into(),
            seasonal_themes: true,
            season_off: String::new(),
            custom_css: String::new(),
            custom_themes: Vec::new(),
            remote_control: false,
            remote_full_control: false,
            language: "system".into(),
            // A quarter of system RAM, kept between 2 GB and 6 GB, suits most packs.
            memory_mb: ((total / 4 / 512) * 512).clamp(2048, 6144),
            jvm_args: String::new(),
            optimized_flags: true,
            java_path: String::new(),
            auto_java: true,
            on_launch: "minimize".into(),
            game_width: 1280,
            game_height: 720,
            ms_client_id: String::new(),
            curseforge_key: String::new(),
            concurrency: 16,
            show_snapshots: false,
            auto_update: false,
            silent_update: false,
            tray: false,
            preferred_mod_source: "ask".into(),
            watch_folders: Vec::new(),
            auto_check_packs: true,
            show_memory_on_home: false,
            discord_presence: true,
            discord_show_server: false,
            discord_in_launcher: true,
            defaults_version: DEFAULTS_VERSION,
            env: Vec::new(),
            commands: LaunchCommands::default(),
        }
    }
}

static CACHE: Mutex<Option<Settings>> = Mutex::new(None);

/// Whether closing the window should keep ResinCraft in the tray (read where waiting isn't possible: the close
/// button's handler). Off until settings were read once.
pub fn tray_on() -> bool {
    CACHE.lock().unwrap().as_ref().is_some_and(|s| s.tray)
}

/// A setting override if present, else the value compiled in by build.rs, else a runtime env var.
fn key_or_builtin(overridden: &str, builtin: Option<&'static str>, env: &str) -> String {
    let v = overridden.trim();
    if !v.is_empty() {
        return v.to_string();
    }
    builtin.map(str::to_string).or_else(|| std::env::var(env).ok()).unwrap_or_default().trim().to_string()
}

/// The Azure Client ID used for Microsoft sign-in ("" if this build has none).
pub async fn ms_client_id() -> String {
    key_or_builtin(&get().await.ms_client_id, option_env!("RESINCRAFT_MS_CLIENT_ID"), "RESINCRAFT_MS_CLIENT_ID")
}

/// The CurseForge API key ("" if this build has none).
pub async fn curseforge_key() -> String {
    key_or_builtin(&get().await.curseforge_key, option_env!("RESINCRAFT_CURSEFORGE_KEY"), "RESINCRAFT_CURSEFORGE_KEY")
}

/// MineSkin API key for the online skin search ("" if this build has none; the search still works, with lower limits).
pub fn mineskin_key() -> String {
    key_or_builtin("", option_env!("RESINCRAFT_MINESKIN_KEY"), "RESINCRAFT_MINESKIN_KEY")
}

/// Settings as the UI may see them: the service keys never leave the engine.
pub fn redacted(mut s: Settings) -> Settings {
    s.ms_client_id.clear();
    s.curseforge_key.clear();
    s
}

/// Bumped when a default changes in a way that should also reach existing settings files, once.
const DEFAULTS_VERSION: u32 = 1;

/// Applies the default changes a settings file from an older version hasn't had yet. Returns whether anything changed.
fn upgrade(s: &mut Settings) -> bool {
    let from = s.defaults_version;
    if from >= DEFAULTS_VERSION {
        return false;
    }
    // 1: "In the launcher" on Discord became on by default (1.2.4 rebuild); before, settings files saved it as off.
    if from < 1 {
        s.discord_in_launcher = true;
    }
    s.defaults_version = DEFAULTS_VERSION;
    true
}

pub async fn get() -> Settings {
    if let Some(s) = CACHE.lock().unwrap().clone() {
        return s;
    }
    // Missing fields fall back to defaults thanks to #[serde(default)].
    let file = paths::settings_file();
    let mut s: Settings = read_json(&file).await.unwrap_or_default();
    if upgrade(&mut s) && file.exists() {
        let _ = write_json(&file, &s).await;
    }
    *CACHE.lock().unwrap() = Some(s.clone());
    s
}

/// Merges a partial settings object (as sent by the UI) and saves.
pub async fn set(patch: Value) -> Result<Settings> {
    let mut current = serde_json::to_value(get().await)?;
    if let (Value::Object(cur), Value::Object(p)) = (&mut current, patch) {
        for (k, v) in p {
            cur.insert(k, v);
        }
    }
    let s: Settings = serde_json::from_value(current)?;
    crate::game::commands::check_env(&s.env)?;
    write_json(&paths::settings_file(), &s).await?;
    *CACHE.lock().unwrap() = Some(s.clone());
    crate::tray::sync(s.tray);
    crate::remote::sync(s.remote_control);
    Ok(s)
}

#[cfg(test)]
mod defaults_tests {
    use super::*;

    #[test]
    fn older_files_get_in_launcher_switched_on_once() {
        // A 1.2.3 file: the field written as off, no defaultsVersion.
        let mut old: Settings = serde_json::from_str(r#"{"discordInLauncher": false, "theme": "dark"}"#).unwrap();
        assert_eq!(old.defaults_version, 0);
        assert!(upgrade(&mut old));
        assert!(old.discord_in_launcher);
        // Turned off again afterwards: that choice stays.
        old.discord_in_launcher = false;
        let saved: Settings = serde_json::from_str(&serde_json::to_string(&old).unwrap()).unwrap();
        let mut again = saved;
        assert!(!upgrade(&mut again));
        assert!(!again.discord_in_launcher);
        // New installs start on.
        assert!(Settings::default().discord_in_launcher);
    }
}
