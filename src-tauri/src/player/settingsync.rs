//! Game settings shared by every instance (Settings → Instances → Sync game settings, and each instance's own
//! switch), as the Modrinth App does it: options.txt one setting at a time, OptiFine's settings, and optionally the
//! server list, command history and creative hotbars as whole files.
//!
//! The shared copy lives in `<games>/synced-settings`. Before a launch it is written into the instance; after the game
//! closes, whatever the player changed there goes back into it. "Changed" is measured against what the launch wrote
//! (`settings-sync.json` in the instance folder), so a game that only knows some of the settings, or two instances
//! played at once, don't undo each other's changes.
//!
//! Across Minecraft versions:
//! - Keybinds and the language were written differently before 1.13 (key codes, `en_US`), so those are kept apart
//!   for old and new versions. Other settings are shared; a version ignores the ones it doesn't know.
//! - A value is quoted the way the instance's own file quotes it (1.19 started writing `"true"`).
//! - Creative hotbars only go to versions at least as new as the one that saved them (the game can update items,
//!   not take them back).
//!
//! The first time an instance is synced its own settings files are kept in `own-settings/`, so they can be put back
//! once sync is turned off for it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha1::Digest;

use crate::core::fsutil::now_ms;
use crate::core::settings::Settings;
use crate::core::types::{Instance, LoaderType};
use crate::core::{instances, paths};
use crate::bail_tr;

/// One shared options.txt setting: its value, and the options.txt `version:` (data version) it came from.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Entry {
    pub value: String,
    pub version: u32,
}

/// The shared settings (`store.json`; the whole files sit next to it).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Store {
    /// options.txt settings by name; keybinds and the language from before 1.13 are kept as `legacy:<name>`.
    pub options: BTreeMap<String, Entry>,
    /// OptiFine's optionsof.txt.
    pub optifine: BTreeMap<String, String>,
    /// The data version inside the shared hotbar.nbt.
    pub hotbar_version: u32,
    /// The instance that last changed something, and when.
    pub from: String,
    pub updated: i64,
}

impl Store {
    fn is_empty(&self) -> bool {
        self.options.is_empty() && self.optifine.is_empty() && self.updated == 0
    }
}

/// What a launch wrote into an instance, to tell the player's changes from it afterwards (`settings-sync.json`).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct State {
    options: BTreeMap<String, String>,
    optifine: BTreeMap<String, String>,
    /// Whole files by name → SHA-1.
    files: BTreeMap<String, String>,
}

/// Which parts are shared (from Settings).
#[derive(Debug, Clone, Copy)]
pub struct Parts {
    pub servers: bool,
    pub history: bool,
    pub hotbars: bool,
}

impl Parts {
    pub fn of(s: &Settings) -> Self {
        Parts { servers: s.sync_servers, history: s.sync_command_history, hotbars: s.sync_hotbars }
    }
    fn files(self) -> Vec<&'static str> {
        [(self.servers, SERVERS), (self.history, HISTORY), (self.hotbars, HOTBAR)].into_iter().filter(|(on, _)| *on).map(|(_, f)| f).collect()
    }
}

const OPTIONS: &str = "options.txt";
const OPTIFINE: &str = "optionsof.txt";
const SERVERS: &str = "servers.dat";
const HISTORY: &str = "command_history.txt";
const HOTBAR: &str = "hotbar.nbt";
/// Settings that belong to the instance: its data version, and resource packs (they depend on what's installed).
const NOT_SHARED: [&str; 3] = ["version", "resourcePacks", "incompatibleResourcePacks"];
/// 17w43a, where keybinds became names (`key.keyboard.w`) and languages lower case.
const FLATTENING: u32 = 1451;

static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn store_dir() -> PathBuf {
    paths::games().join("synced-settings")
}
fn state_file(id: &str) -> PathBuf {
    paths::instance_dir(id).join("settings-sync.json")
}
fn own_dir(id: &str) -> PathBuf {
    paths::instance_dir(id).join("own-settings")
}

/// Whether this instance's game settings are synced: its own choice, else Settings. Horizon has its own settings.
pub fn enabled(inst: &Instance, s: &Settings) -> bool {
    inst.loader != LoaderType::Horizon && inst.sync_settings.unwrap_or(s.sync_settings)
}

// ---- options.txt ----

enum Line {
    Pair(String, String),
    Other(String),
}

fn parse(text: &str) -> Vec<Line> {
    text.lines()
        .map(|l| match l.split_once(':') {
            Some((k, v)) if !k.is_empty() => Line::Pair(k.to_string(), v.to_string()),
            _ => Line::Other(l.to_string()),
        })
        .collect()
}

fn pairs(text: &str) -> BTreeMap<String, String> {
    parse(text).into_iter().filter_map(|l| if let Line::Pair(k, v) = l { Some((k, v)) } else { None }).collect()
}

fn render(lines: &[Line], newline: &str) -> String {
    let mut out = String::new();
    for l in lines {
        match l {
            Line::Pair(k, v) => out.push_str(&format!("{k}:{v}")),
            Line::Other(s) => out.push_str(s),
        }
        out.push_str(newline);
    }
    out
}

fn newline_of(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// options.txt's `version:` (0 when it has none, as before 1.9).
fn data_version(options: &BTreeMap<String, String>) -> u32 {
    options.get("version").and_then(|v| v.trim().parse().ok()).unwrap_or(0)
}

/// Whether a Minecraft version (as instances name it) is from before keybinds became names.
fn legacy_version(mc: &str) -> bool {
    let mc = mc.trim().to_lowercase();
    if ["a", "b", "c0.", "rd-", "inf-"].iter().any(|p| mc.starts_with(p)) {
        return true;
    }
    let mut parts = mc.split(['.', '-', ' ']);
    match (parts.next(), parts.next().and_then(|m| m.parse::<u32>().ok())) {
        (Some("1"), Some(minor)) => minor < 13,
        _ => false,
    }
}

/// The name a setting is shared under: keybinds and the language are kept apart for versions before 1.13.
fn shared_name(key: &str, legacy: bool) -> String {
    if legacy && (key.starts_with("key_") || key == "lang") {
        format!("legacy:{key}")
    } else {
        key.to_string()
    }
}

/// `value` quoted the way `like` is (1.19+ writes some values as JSON strings, older versions don't read those).
fn quoted_like(value: &str, like: &str) -> String {
    let quoted = |s: &str| s.len() >= 2 && s.starts_with('"') && s.ends_with('"');
    match (quoted(value), quoted(like)) {
        (true, false) => value[1..value.len() - 1].to_string(),
        (false, true) => format!("\"{value}\""),
        _ => value.to_string(),
    }
}

/// options.txt with the shared settings written in (in place, new ones at the end). `own` is the instance's file;
/// without one, a new file starts with the oldest data version the values came from, so the game updates them.
fn merge_options(own: Option<&str>, store: &Store, legacy: bool) -> String {
    let mut lines = own.map(parse).unwrap_or_default();
    let mut placed = std::collections::BTreeSet::new();
    for line in lines.iter_mut() {
        if let Line::Pair(k, v) = line {
            placed.insert(k.clone());
            if NOT_SHARED.contains(&k.as_str()) {
                continue;
            }
            if let Some(e) = store.options.get(&shared_name(k, legacy)) {
                *v = quoted_like(&e.value, v);
            }
        }
    }
    let mut oldest = u32::MAX;
    for (name, e) in &store.options {
        let is_legacy = name.starts_with("legacy:");
        let key = name.strip_prefix("legacy:").unwrap_or(name);
        if placed.contains(key) || NOT_SHARED.contains(&key) {
            continue;
        }
        // Era-specific settings only go to their own era; `shared_name` of a modern keybind in a legacy file differs.
        if (key.starts_with("key_") || key == "lang") && is_legacy != legacy {
            continue;
        }
        if e.version > 0 {
            oldest = oldest.min(e.version);
        }
        lines.push(Line::Pair(key.to_string(), e.value.clone()));
    }
    if own.is_none() && oldest != u32::MAX {
        lines.insert(0, Line::Pair("version".into(), oldest.to_string()));
    }
    render(&lines, own.map(newline_of).unwrap_or("\n"))
}

/// optionsof.txt with the shared OptiFine settings written over the instance's own.
fn merge_plain(own: &str, shared: &BTreeMap<String, String>) -> String {
    let mut lines = parse(own);
    let mut placed = std::collections::BTreeSet::new();
    for line in lines.iter_mut() {
        if let Line::Pair(k, v) = line {
            placed.insert(k.clone());
            if let Some(s) = shared.get(k) {
                *v = s.clone();
            }
        }
    }
    for (k, v) in shared {
        if !placed.contains(k) {
            lines.push(Line::Pair(k.clone(), v.clone()));
        }
    }
    render(&lines, newline_of(own))
}

/// The settings in `now` that differ from what the launch wrote (`before`), or that nothing shares yet (`known` says
/// which are shared). Missing ones aren't removals: a version that doesn't know a setting just leaves it out.
fn changed(now: &BTreeMap<String, String>, before: &BTreeMap<String, String>, known: impl Fn(&str) -> bool) -> Vec<(String, String)> {
    now.iter().filter(|(k, v)| before.get(*k) != Some(*v) || !known(k)).map(|(k, v)| (k.clone(), v.clone())).collect()
}

// ---- files ----

fn read(path: &Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}
fn read_text(path: &Path) -> Option<String> {
    read(path).map(|b| String::from_utf8_lossy(&b).into_owned())
}
fn sha(bytes: &[u8]) -> String {
    hex::encode(sha1::Sha1::digest(bytes))
}
/// Written next to the file and moved over it, so a crash never leaves half a file.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("sync-tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
fn load<T: Default + for<'a> Deserialize<'a>>(path: &Path) -> T {
    read(path).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}
fn save<T: Serialize>(path: &Path, v: &T) -> Result<()> {
    write_atomic(path, &serde_json::to_vec_pretty(v)?)
}

/// The data version inside a hotbar.nbt (0 when unreadable or missing).
fn hotbar_version(bytes: &[u8]) -> u32 {
    super::nbt::parse(bytes).ok().and_then(|f| super::nbt::num(f.root.get("DataVersion"))).map(|n| n as u32).unwrap_or(0)
}

// ---- push and pull, on folders (tested below) ----

/// Writes the shared settings into a game folder and returns what it holds afterwards.
fn push(game: &Path, shared: &Path, store: &Store, parts: Parts, mc: &str) -> Result<State> {
    let own = read_text(&game.join(OPTIONS));
    let own_pairs = own.as_deref().map(pairs).unwrap_or_default();
    let legacy = if own_pairs.contains_key("version") { data_version(&own_pairs) < FLATTENING } else { legacy_version(mc) };
    if !store.options.is_empty() {
        let merged = merge_options(own.as_deref(), store, legacy);
        if own.as_deref() != Some(merged.as_str()) {
            write_atomic(&game.join(OPTIONS), merged.as_bytes())?;
        }
    }
    // OptiFine's file only where OptiFine made one: it means nothing to other instances.
    if let Some(of) = read_text(&game.join(OPTIFINE)) {
        if !store.optifine.is_empty() {
            let merged = merge_plain(&of, &store.optifine);
            if merged != of {
                write_atomic(&game.join(OPTIFINE), merged.as_bytes())?;
            }
        }
    }
    let game_version = data_version(&read_text(&game.join(OPTIONS)).as_deref().map(pairs).unwrap_or_default());
    for name in parts.files() {
        let Some(bytes) = read(&shared.join(name)) else { continue };
        // Items can be updated to a newer version, not taken back to an older one.
        if name == HOTBAR && (game_version == 0 || store.hotbar_version > game_version) {
            continue;
        }
        if read(&game.join(name)).as_deref() != Some(&bytes[..]) {
            write_atomic(&game.join(name), &bytes)?;
        }
    }
    Ok(snapshot(game, parts))
}

/// What a game folder holds now, as `State`.
fn snapshot(game: &Path, parts: Parts) -> State {
    let mut options = read_text(&game.join(OPTIONS)).as_deref().map(pairs).unwrap_or_default();
    options.retain(|k, _| !NOT_SHARED.contains(&k.as_str()));
    let optifine = read_text(&game.join(OPTIFINE)).as_deref().map(pairs).unwrap_or_default();
    let files = parts.files().into_iter().filter_map(|n| read(&game.join(n)).map(|b| (n.to_string(), sha(&b)))).collect();
    State { options, optifine, files }
}

/// Takes what the player changed in a game folder since `before` into the shared settings. Returns how many
/// settings and files changed.
fn pull(game: &Path, shared: &Path, store: &mut Store, before: &State, parts: Parts, mc: &str, name: &str) -> Result<usize> {
    let own = read_text(&game.join(OPTIONS)).as_deref().map(pairs).unwrap_or_default();
    let version = data_version(&own);
    let legacy = if own.contains_key("version") { version < FLATTENING } else { legacy_version(mc) };
    let mut count = 0;
    for (k, v) in changed(&own, &before.options, |k| store.options.contains_key(&shared_name(k, legacy))) {
        if NOT_SHARED.contains(&k.as_str()) {
            continue;
        }
        store.options.insert(shared_name(&k, legacy), Entry { value: v, version });
        count += 1;
    }
    let of = read_text(&game.join(OPTIFINE)).as_deref().map(pairs).unwrap_or_default();
    for (k, v) in changed(&of, &before.optifine, |k| store.optifine.contains_key(k)) {
        store.optifine.insert(k, v);
        count += 1;
    }
    for file in parts.files() {
        let Some(bytes) = read(&game.join(file)) else { continue };
        if before.files.get(file) == Some(&sha(&bytes)) {
            continue;
        }
        write_atomic(&shared.join(file), &bytes)?;
        if file == HOTBAR {
            store.hotbar_version = hotbar_version(&bytes);
        }
        count += 1;
    }
    if count > 0 {
        store.from = name.to_string();
        store.updated = now_ms();
    }
    Ok(count)
}

/// Keeps the instance's own settings files once, the first time sync writes over them.
fn keep_own(game: &Path, own: &Path) -> Result<()> {
    if own.exists() || !game.join(OPTIONS).is_file() {
        return Ok(());
    }
    std::fs::create_dir_all(own)?;
    for name in [OPTIONS, OPTIFINE, SERVERS, HISTORY, HOTBAR] {
        if game.join(name).is_file() {
            std::fs::copy(game.join(name), own.join(name))?;
        }
    }
    Ok(())
}

// ---- the launcher's side ----

/// What happened before a launch, for the game log.
pub struct Synced {
    pub settings: usize,
    pub first: bool,
}

/// Before a launch (the instance's first run, not a second account's): takes in anything a previous run left behind
/// unsynced, then writes the shared settings in. With sync off for the instance, forgets what was synced, so turning
/// it back on later takes the shared settings instead of pushing old ones.
pub async fn before_launch(inst: &Instance, s: &Settings) -> Result<Option<Synced>> {
    let _lock = LOCK.lock().await;
    let (id, mc, name, parts, on) = (inst.id.clone(), inst.mc_version.clone(), inst.name.clone(), Parts::of(s), enabled(inst, s));
    tokio::task::spawn_blocking(move || -> Result<Option<Synced>> {
        let state_path = state_file(&id);
        if !on {
            let _ = std::fs::remove_file(&state_path);
            return Ok(None);
        }
        let (game, shared) = (paths::game_dir(&id), store_dir());
        let store_path = shared.join("store.json");
        let mut store: Store = load(&store_path);
        let first = !state_path.exists();
        if store.is_empty() {
            // Nothing shared yet: this instance's settings become the shared ones.
            pull(&game, &shared, &mut store, &State::default(), parts, &mc, &name)?;
            if !store.is_empty() {
                save(&store_path, &store)?;
            }
        } else if !first {
            // The game closed while the launcher wasn't running: its changes come in now.
            let before: State = load(&state_path);
            if pull(&game, &shared, &mut store, &before, parts, &mc, &name)? > 0 {
                save(&store_path, &store)?;
            }
        } else {
            keep_own(&game, &own_dir(&id))?;
        }
        let state = push(&game, &shared, &store, parts, &mc)?;
        save(&state_path, &state)?;
        Ok(Some(Synced { settings: store.options.len(), first }))
    })
    .await?
}

/// After the game closed (and no other run of the instance is left), or the Game options tab saved: what the player
/// changed goes to the others.
pub async fn after_exit(id: &str) -> Result<usize> {
    let Ok(inst) = instances::get(id).await else { return Ok(0) };
    let s = crate::core::settings::get().await;
    if !enabled(&inst, &s) || !state_file(id).exists() {
        return Ok(0);
    }
    let _lock = LOCK.lock().await;
    let (id, parts) = (id.to_string(), Parts::of(&s));
    tokio::task::spawn_blocking(move || -> Result<usize> {
        let (game, shared, state_path) = (paths::game_dir(&id), store_dir(), state_file(&id));
        let store_path = shared.join("store.json");
        let mut store: Store = load(&store_path);
        let before: State = load(&state_path);
        let n = pull(&game, &shared, &mut store, &before, parts, &inst.mc_version, &inst.name)?;
        if n > 0 {
            save(&store_path, &store)?;
        }
        save(&state_path, &snapshot(&game, parts))?;
        Ok(n)
    })
    .await?
}

/// Settings → Sync game settings: how much is shared, and who changed it last.
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub settings: usize,
    pub files: Vec<String>,
    pub from: String,
    pub updated: i64,
}

pub async fn info() -> Info {
    let shared = store_dir();
    let store: Store = load(&shared.join("store.json"));
    let files = [SERVERS, HISTORY, HOTBAR].iter().filter(|f| shared.join(f).is_file()).map(|f| f.to_string()).collect();
    Info { settings: store.options.len() + store.optifine.len(), files, from: store.from, updated: store.updated }
}

/// Settings → Start from: the shared settings become this instance's (everything shared before is replaced).
pub async fn start_from(id: &str) -> Result<Info> {
    let inst = instances::get(id).await?;
    let s = crate::core::settings::get().await;
    let parts = Parts { servers: true, history: true, hotbars: true };
    let sync_parts = Parts::of(&s);
    {
        let _lock = LOCK.lock().await;
        let id = inst.id.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let (game, shared) = (paths::game_dir(&id), store_dir());
            for f in parts.files() {
                let _ = std::fs::remove_file(shared.join(f));
            }
            let mut store = Store::default();
            pull(&game, &shared, &mut store, &State::default(), sync_parts, &inst.mc_version, &inst.name)?;
            store.from = inst.name.clone();
            store.updated = now_ms();
            save(&shared.join("store.json"), &store)?;
            // This instance now matches the shared settings.
            if enabled(&inst, &s) {
                save(&state_file(&id), &snapshot(&game, sync_parts))?;
            }
            Ok(())
        })
        .await??;
    }
    Ok(info().await)
}

/// Whether the instance's own settings from before it was first synced are kept.
pub async fn has_own(id: &str) -> bool {
    own_dir(id).join(OPTIONS).is_file()
}

/// Puts back the settings the instance had before it was first synced (instance settings, once sync is off for it).
pub async fn restore_own(id: &str) -> Result<()> {
    let inst = instances::get(id).await?;
    if crate::game::running_ids().iter().any(|r| *r == inst.id) {
        bail_tr!("settingsSync.closeFirst", "Close {name} before you put its own settings back", name = inst.name);
    }
    if !has_own(id).await {
        bail_tr!("settingsSync.noOwn", "{name} has no settings of its own kept", name = inst.name);
    }
    let _lock = LOCK.lock().await;
    let id = inst.id.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let (own, game) = (own_dir(&id), paths::game_dir(&id));
        for name in [OPTIONS, OPTIFINE, SERVERS, HISTORY, HOTBAR] {
            if let Some(bytes) = read(&own.join(name)) {
                write_atomic(&game.join(name), &bytes)?;
            }
        }
        std::fs::remove_dir_all(&own)?;
        let _ = std::fs::remove_file(state_file(&id));
        Ok(())
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: Parts = Parts { servers: true, history: true, hotbars: true };

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("resin-settingsync-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    fn text(p: &Path) -> String {
        std::fs::read_to_string(p).unwrap()
    }

    #[test]
    fn settings_merge_in_place_and_keep_the_instances_own() {
        let mut store = Store::default();
        store.options.insert("fov".into(), Entry { value: "0.5".into(), version: 3955 });
        store.options.insert("key_key.jump".into(), Entry { value: "key.keyboard.space".into(), version: 3955 });
        store.options.insert("legacy:key_key.jump".into(), Entry { value: "57".into(), version: 1343 });
        store.options.insert("resourcePacks".into(), Entry { value: "[\"x\"]".into(), version: 3955 });
        store.options.insert("renderClouds".into(), Entry { value: "\"fast\"".into(), version: 3955 });
        let own = "version:3955\nfov:0.0\nresourcePacks:[\"vanilla\"]\nrenderClouds:\"true\"\n";
        let out = merge_options(Some(own), &store, false);
        assert_eq!(out, "version:3955\nfov:0.5\nresourcePacks:[\"vanilla\"]\nrenderClouds:\"fast\"\nkey_key.jump:key.keyboard.space\n");
        // 1.12: its own keybind codes, unquoted values.
        let old = "version:1343\r\nfov:0.0\r\nrenderClouds:true\r\n";
        let out = merge_options(Some(old), &store, true);
        assert_eq!(out, "version:1343\r\nfov:0.5\r\nrenderClouds:fast\r\nkey_key.jump:57\r\n");
    }

    #[test]
    fn a_new_instance_gets_the_oldest_data_version_so_the_game_updates_the_values() {
        let mut store = Store::default();
        store.options.insert("fov".into(), Entry { value: "0.5".into(), version: 3955 });
        store.options.insert("gamma".into(), Entry { value: "1.0".into(), version: 2586 });
        assert_eq!(merge_options(None, &store, false), "version:2586\nfov:0.5\ngamma:1.0\n");
    }

    #[test]
    fn versions_before_1_13_are_told_apart() {
        for v in ["1.12.2", "1.8.9", "b1.7.3", "a1.2.6", "rd-132211", "c0.30_01c", "1.7"] {
            assert!(legacy_version(v), "{v}");
        }
        for v in ["1.13", "1.21.4", "24w14a", "1.20.1-forge"] {
            assert!(!legacy_version(v), "{v}");
        }
    }

    #[test]
    fn changes_go_from_one_instance_to_the_other_and_not_back() {
        let (a, b, shared) = (dir("a"), dir("b"), dir("shared"));
        std::fs::write(a.join(OPTIONS), "version:3955\nfov:0.0\nkey_key.jump:key.keyboard.space\n").unwrap();
        std::fs::write(a.join(SERVERS), b"servers-a").unwrap();
        std::fs::write(b.join(OPTIONS), "version:3955\nfov:1.0\nguiScale:3\n").unwrap();
        let mut store = Store::default();

        // A seeds the shared settings; B gets them, keeping what A doesn't have.
        pull(&a, &shared, &mut store, &State::default(), ALL, "1.21.1", "A").unwrap();
        let a_state = push(&a, &shared, &store, ALL, "1.21.1").unwrap();
        let b_state = push(&b, &shared, &store, ALL, "1.21.1").unwrap();
        assert_eq!(text(&b.join(OPTIONS)), "version:3955\nfov:0.0\nguiScale:3\nkey_key.jump:key.keyboard.space\n");
        assert_eq!(std::fs::read(b.join(SERVERS)).unwrap(), b"servers-a");

        // B's player changes the fov; A's player, at the same time, a keybind. Neither undoes the other.
        std::fs::write(b.join(OPTIONS), "version:3955\nfov:0.7\nguiScale:3\nkey_key.jump:key.keyboard.space\n").unwrap();
        std::fs::write(a.join(OPTIONS), "version:3955\nfov:0.0\nkey_key.jump:key.keyboard.j\n").unwrap();
        // B also shares guiScale, which A never had.
        assert_eq!(pull(&b, &shared, &mut store, &b_state, ALL, "1.21.1", "B").unwrap(), 2);
        assert_eq!(pull(&a, &shared, &mut store, &a_state, ALL, "1.21.1", "A").unwrap(), 1);
        // Nothing changed since: nothing goes again.
        let b_state = snapshot(&b, ALL);
        assert_eq!(pull(&b, &shared, &mut store, &b_state, ALL, "1.21.1", "B").unwrap(), 0);
        assert_eq!(store.options["fov"].value, "0.7");
        assert_eq!(store.options["key_key.jump"].value, "key.keyboard.j");
        assert_eq!(store.options["guiScale"].value, "3");
        assert_eq!(store.from, "A");

        push(&a, &shared, &store, ALL, "1.21.1").unwrap();
        assert_eq!(text(&a.join(OPTIONS)), "version:3955\nfov:0.7\nkey_key.jump:key.keyboard.j\nguiScale:3\n");
        for d in [a, b, shared] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn hotbars_only_go_to_versions_that_can_read_them() {
        let (new, old, shared) = (dir("hb-new"), dir("hb-old"), dir("hb-shared"));
        let mut root = super::super::nbt::Compound::new();
        root.set("DataVersion", super::super::nbt::Tag::Int(3955));
        let bytes = super::super::nbt::write(&super::super::nbt::NbtFile { name: String::new(), root, compression: super::super::nbt::Compression::None }).unwrap();
        std::fs::write(new.join(OPTIONS), "version:3955\n").unwrap();
        std::fs::write(new.join(HOTBAR), &bytes).unwrap();
        std::fs::write(old.join(OPTIONS), "version:3465\n").unwrap();
        let mut store = Store::default();
        pull(&new, &shared, &mut store, &State::default(), ALL, "1.21.1", "New").unwrap();
        assert_eq!(store.hotbar_version, 3955);
        push(&old, &shared, &store, ALL, "1.20.1").unwrap();
        assert!(!old.join(HOTBAR).exists());
        // Turned off in Settings: not shared at all.
        let none = Parts { servers: false, history: false, hotbars: false };
        let later = dir("hb-later");
        std::fs::write(later.join(OPTIONS), "version:4000\n").unwrap();
        push(&later, &shared, &store, none, "1.21.5").unwrap();
        assert!(!later.join(HOTBAR).exists());
        push(&later, &shared, &store, ALL, "1.21.5").unwrap();
        assert_eq!(std::fs::read(later.join(HOTBAR)).unwrap(), bytes);
        for d in [new, old, shared, later] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn optifine_settings_only_go_where_optifine_is() {
        let (a, b, shared) = (dir("of-a"), dir("of-b"), dir("of-shared"));
        std::fs::write(a.join(OPTIFINE), "ofFastRender:true\nofSmoothFps:false\n").unwrap();
        let mut store = Store::default();
        pull(&a, &shared, &mut store, &State::default(), ALL, "1.20.1", "A").unwrap();
        push(&b, &shared, &store, ALL, "1.20.1").unwrap();
        assert!(!b.join(OPTIFINE).exists());
        std::fs::write(b.join(OPTIFINE), "ofFastRender:false\nofDynamicLights:3\n").unwrap();
        push(&b, &shared, &store, ALL, "1.20.1").unwrap();
        assert_eq!(text(&b.join(OPTIFINE)), "ofFastRender:true\nofDynamicLights:3\nofSmoothFps:false\n");
        for d in [a, b, shared] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn own_settings_are_kept_once() {
        let (game, own) = (dir("own-game"), dir("own-keep").join("own-settings"));
        std::fs::write(game.join(OPTIONS), "fov:0.3\n").unwrap();
        keep_own(&game, &own).unwrap();
        std::fs::write(game.join(OPTIONS), "fov:0.9\n").unwrap();
        keep_own(&game, &own).unwrap();
        assert_eq!(text(&own.join(OPTIONS)), "fov:0.3\n");
        let _ = std::fs::remove_dir_all(game);
        let _ = std::fs::remove_dir_all(own.parent().unwrap());
    }
}
