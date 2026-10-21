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
    /// "dark" | "light" | "system"
    pub theme: String,
    /// UI language: "system" (follow the OS) or a locale code such as "de" or "pt-BR".
    pub language: String,
    pub memory_mb: u32,
    pub jvm_args: String,
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
    /// When the same mod is on Modrinth and CurseForge: "ask" | "modrinth" | "curseforge" (Settings → Downloads,
    /// or "Don't ask again" in the duplicate dialog).
    pub preferred_mod_source: String,
    /// Folders besides Downloads where files for Download all are looked for (a browser that saves elsewhere).
    pub watch_folders: Vec<String>,
    /// Look for newer versions of installed modpacks (and instances changed since a server synced) at start and every
    /// few hours (Settings → Updates).
    pub auto_check_packs: bool,
    /// Discord Rich Presence while a game runs (only in builds with a Discord app ID).
    pub discord_presence: bool,
    /// Rich Presence names the server joined; off by default, the address can be private.
    pub discord_show_server: bool,
    /// Rich Presence shows "In the launcher" while ResinCraft is open and no game runs.
    pub discord_in_launcher: bool,
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
            language: "system".into(),
            // A quarter of system RAM, kept between 2 GB and 6 GB, suits most packs.
            memory_mb: ((total / 4 / 512) * 512).clamp(2048, 6144),
            jvm_args: String::new(),
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
            preferred_mod_source: "ask".into(),
            watch_folders: Vec::new(),
            auto_check_packs: true,
            discord_presence: true,
            discord_show_server: false,
            discord_in_launcher: false,
            env: Vec::new(),
            commands: LaunchCommands::default(),
        }
    }
}

static CACHE: Mutex<Option<Settings>> = Mutex::new(None);

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

pub async fn get() -> Settings {
    if let Some(s) = CACHE.lock().unwrap().clone() {
        return s;
    }
    // Missing fields fall back to defaults thanks to #[serde(default)].
    let s: Settings = read_json(&paths::settings_file()).await.unwrap_or_default();
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
    Ok(s)
}
