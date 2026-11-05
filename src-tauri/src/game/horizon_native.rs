//! Horizon native mods (`native-mod`, `.hnmod`): mixin bundles that change the game's own Rust source, so the game
//! is compiled again with them (horizon-rust/horizon `docs/native-mods.md`). ResinCraft doesn't compile anything
//! itself: Horizon's own launcher (`player::horizon`, in `<data>/horizon-launcher`) does, with its own pinned Rust,
//! through `horizon-launcher native-build FOLDER --version BASE --trust-native-code`.
//!
//! - An installed native mod is unpacked, checked as Horizon's registry checks it, into
//!   `<game_dir>/horizon-native-mods/<id>/` (`horizon-native.json` and `mixins/<id>/`). Native mods are always on.
//! - Horizon's launcher keeps its own data folder for ResinCraft, `<games>/horizon-native/` (`HORIZON_LAUNCHER_HOME`;
//!   `HORIZON_HOME` is its `game/`, so the player's own Horizon is never touched). The build it starts from is the
//!   instance's prepared build, linked into its `versions/<build id>/`. Its compile cache (Rust, crates, `native/target`)
//!   is in its build folder: that data folder, or, when the path has a space or isn't ASCII on Windows, a folder of
//!   ProgramData's that Horizon's launcher names.
//! - A finished build (a copy of the base build with the compiled `horizon-client`) moves into the instance's own
//!   `horizon-native-build/<version>/`, with a record of what it was made from: a fingerprint of every mod file, the
//!   base build and the profile. A start plays it while that fingerprint still matches; otherwise it asks for a build
//!   first (`horizon.nativeNeedsBuild`), and the player can play once without the native mods.
//! - Finished builds are kept once, by that fingerprint, in `<games>/horizon-native-store/<key>/`: a set of mods
//!   built before (on any instance, or before a mod was taken out and put back) plays at once, with nothing compiled.
//!   Files the build shares with its base build are hard links to it, so a build costs about its game program.
//! - After a build, Horizon's launcher's compile files (its Rust, Cargo's home, the compiler's output) are deleted
//!   unless the player keeps them for quick rebuilds (`keep_cache`).

use std::collections::HashSet;
use std::io::{Read, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

use crate::core::events::Task;
use crate::core::paths;
use crate::core::types::Instance;
use crate::{bail_tr, err_tr, tr};

use super::horizon::Build;
use super::java::hidden_command;

/// Where a game folder keeps its native mods, as in Horizon.
pub const NATIVE_DIR: &str = "horizon-native-mods";
/// A registry download's manifest, beside `mixins/<id>/`.
pub const NATIVE_MANIFEST: &str = "horizon-native.json";
const MIXIN_MANIFEST: &str = "horizon-mixin.toml";
/// In the instance's folder: the build it plays, and what it was made from.
const BUILD_DIR: &str = "horizon-native-build";
const RECORD: &str = "resincraft-native.json";
/// Horizon's limits for the native part of a file (docs/api/mod-files.md).
const MAX_ENTRIES: usize = 512;
const MAX_UNPACKED: u64 = 64 << 20;
const MAX_MANIFEST: u64 = 1 << 20;
/// What ResinCraft builds with: Horizon's default, incremental and quick to rebuild.
pub const PROFILE: &str = "performance";

// ---------- The bundle ----------

/// `horizon-native.json`. Unknown fields refuse the file, as in Horizon.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub id: String,
    pub version: String,
    pub game: String,
    pub horizon: String,
    pub mixins: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks: Option<u32>,
}

fn bad_bundle(why: impl std::fmt::Display) -> anyhow::Error {
    err_tr!("horizon.badNative", "That native mod can't be installed: {why}", why = why.to_string())
}

fn revision(text: &str) -> bool {
    text.len() == 40 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn parse_manifest(text: &str) -> Result<Manifest> {
    let m: Manifest = serde_json::from_str(text).map_err(|e| bad_bundle(format!("{NATIVE_MANIFEST}: {e}")))?;
    if m.schema != 1 {
        return Err(bad_bundle(format!("{NATIVE_MANIFEST}: schema {}", m.schema)));
    }
    if !crate::content::horizon::valid_id(&m.id) || m.version.trim().is_empty() || m.game.trim().is_empty() {
        return Err(bad_bundle(format!("{NATIVE_MANIFEST}: id {:?}, version {:?}", m.id, m.version)));
    }
    if !revision(&m.horizon) || !revision(&m.mixins) {
        return Err(bad_bundle(format!("{NATIVE_MANIFEST}: its pinned commits")));
    }
    Ok(m)
}

/// A zip entry's path split in parts, when it is one Horizon allows at all: relative, `/`-separated, no `\` or `:`,
/// no empty, `.` or `..` part.
fn parts(name: &str) -> Option<Vec<&str>> {
    if name.is_empty() || name.starts_with('/') || name.contains('\\') || name.contains(':') || name.contains('\0') {
        return None;
    }
    let parts: Vec<&str> = name.split('/').collect();
    parts.iter().all(|p| !p.is_empty() && *p != "." && *p != "..").then_some(parts)
}

/// Whether a path under `mixins/<id>/` may be in a native file: `.rs` and `.toml` files, files under `data/` (or a
/// top-level folder's `data/`), and the notices (README, LICENSE…, `.gitignore`) in the bundle's folder or a
/// top-level folder.
pub fn bundle_file_allowed(rest: &[&str]) -> bool {
    let Some(file) = rest.last() else { return false };
    let lower = file.to_ascii_lowercase();
    if lower.ends_with(".rs") || lower.ends_with(".toml") {
        return true;
    }
    if rest.first() == Some(&"data") && rest.len() > 1 || rest.len() > 2 && rest[1] == "data" {
        return true;
    }
    let notice = ["readme", "license", "licence", "copying", "notice"].iter().any(|n| lower.starts_with(n)) || lower == ".gitignore";
    notice && rest.len() <= 2
}

/// Where a native file's entry goes, relative to the mod's folder, or why the file is refused.
pub fn entry_target(name: &str, id: &str) -> Result<String> {
    let refuse = || bad_bundle(format!("it holds {name}"));
    let p = parts(name).ok_or_else(refuse)?;
    match p.as_slice() {
        [NATIVE_MANIFEST] => Ok(NATIVE_MANIFEST.to_string()),
        ["mixins", own, rest @ ..] if *own == id && bundle_file_allowed(rest) => Ok(p.join("/")),
        _ => Err(refuse()),
    }
}

/// Reads a `.hnmod` and unpacks it into `<into>/<id>/`, replacing a copy that's there: everything is checked and
/// written into a hidden folder first, so a refused file leaves the installed one as it was. `expect` is the id the
/// registry lists it under. The bytes actually unpacked count against the limits (a zip's sizes are its own claim).
pub fn unpack(file: &Path, into: &Path, expect: Option<&str>) -> Result<Manifest> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(file)?).map_err(bad_bundle)?;
    let mut text = String::new();
    zip.by_name(NATIVE_MANIFEST)
        .map_err(|_| bad_bundle(format!("{NATIVE_MANIFEST} is missing")))?
        .take(MAX_MANIFEST)
        .read_to_string(&mut text)
        .map_err(bad_bundle)?;
    let manifest = parse_manifest(&text)?;
    if let Some(want) = expect.filter(|w| *w != manifest.id) {
        return Err(bad_bundle(format!("it is {} inside, not {want}", manifest.id)));
    }
    let id = manifest.id.clone();
    if zip.len() > MAX_ENTRIES {
        return Err(bad_bundle(format!("it has over {MAX_ENTRIES} files")));
    }
    let mut seen = HashSet::new();
    let mut plan = Vec::new();
    for i in 0..zip.len() {
        let entry = zip.by_index_raw(i).map_err(bad_bundle)?;
        if entry.is_dir() {
            continue;
        }
        if entry.unix_mode().is_some_and(|m| m & 0o170000 != 0 && m & 0o170000 != 0o100000) {
            return Err(bad_bundle(format!("{} isn't a plain file", entry.name())));
        }
        let target = entry_target(entry.name(), &id)?;
        if !seen.insert(target.to_lowercase()) {
            return Err(bad_bundle(format!("{} is in it twice", entry.name())));
        }
        plan.push((i, target));
    }
    if !seen.contains(&format!("mixins/{id}/{MIXIN_MANIFEST}")) {
        return Err(bad_bundle(format!("mixins/{id}/{MIXIN_MANIFEST} is missing")));
    }

    std::fs::create_dir_all(into)?;
    let incoming = into.join(format!(".{id}.incoming"));
    let _ = std::fs::remove_dir_all(&incoming);
    let written = (|| -> Result<()> {
        let mut total = 0u64;
        for (i, target) in &plan {
            let mut entry = zip.by_index(*i).map_err(bad_bundle)?;
            let path = crate::core::fsutil::safe_join(&incoming, target)?;
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let mut out = std::fs::File::create(&path)?;
            let copied = std::io::copy(&mut (&mut entry).take(MAX_UNPACKED + 1 - total), &mut out)?;
            out.flush()?;
            total += copied;
            if total > MAX_UNPACKED {
                return Err(bad_bundle(format!("it unpacks to more than {} MiB", MAX_UNPACKED >> 20)));
            }
        }
        Ok(())
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_dir_all(&incoming);
        return Err(e);
    }
    let dest = into.join(&id);
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::rename(&incoming, &dest).with_context(|| format!("Couldn't put {id} in {}", into.display()))?;
    Ok(manifest)
}

/// A native mod installed in a game folder.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub id: String,
    pub version: String,
    pub game: String,
    /// The Horizon commit it pins.
    pub horizon: String,
}

/// The native mods in `<game>/horizon-native-mods/`, by id. Hidden folders (an unpack in progress) aren't mods.
pub fn installed(game: &Path) -> Vec<Installed> {
    let mut out = Vec::new();
    let Ok(dir) = std::fs::read_dir(game.join(NATIVE_DIR)) else { return out };
    for e in dir.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || !e.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(e.path().join(NATIVE_MANIFEST)) else { continue };
        if let Ok(m) = parse_manifest(&text).filter_ok(|m| m.id == name) {
            out.push(Installed { id: m.id, version: m.version, game: m.game, horizon: m.horizon });
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

trait FilterOk<T> {
    fn filter_ok(self, f: impl FnOnce(&T) -> bool) -> Result<T>;
}
impl<T> FilterOk<T> for Result<T> {
    fn filter_ok(self, f: impl FnOnce(&T) -> bool) -> Result<T> {
        match self {
            Ok(v) if f(&v) => Ok(v),
            Ok(_) => Err(anyhow!("not this one")),
            Err(e) => Err(e),
        }
    }
}

/// Removes one native mod's folder.
pub async fn remove(game: &Path, id: &str) -> Result<()> {
    if !crate::content::horizon::valid_id(id) {
        bail_tr!("horizon.unknownProject", "Horizon's registry has no project {id}", id = id);
    }
    let dir = game.join(NATIVE_DIR).join(id);
    if dir.is_dir() && crate::core::shell::trash(&dir).await.is_err() {
        eprintln!("[horizon] couldn't move {} to the Recycle Bin: removing it", dir.display());
    }
    if dir.exists() {
        tokio::fs::remove_dir_all(&dir).await?;
    }
    Ok(())
}

// ---------- Is a build needed? ----------

/// Every file of the native mods in `mods` (their path and bytes, in path order), the base build and the profile,
/// hashed. None when there are no native mods. A link is refused: Horizon's tools refuse them too.
pub fn fingerprint(mods: &Path, base: &str, profile: &str) -> Result<Option<String>> {
    fn walk(dir: &Path, rel: &str, out: &mut Vec<(String, PathBuf)>) -> Result<()> {
        let mut list: Vec<_> = std::fs::read_dir(dir)?.flatten().collect();
        list.sort_by_key(|e| e.file_name());
        for e in list {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = rel.is_empty().then(|| name.clone()).unwrap_or_else(|| format!("{rel}/{name}"));
            let kind = e.file_type()?;
            if kind.is_symlink() {
                return Err(anyhow!("{} is a link", e.path().display()));
            }
            if kind.is_dir() {
                walk(&e.path(), &path, out)?;
            } else {
                out.push((path, e.path()));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    let mut any = false;
    if let Ok(dir) = std::fs::read_dir(mods) {
        let mut dirs: Vec<_> = dir.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        dirs.sort();
        for name in dirs.into_iter().filter(|n| !n.starts_with('.')) {
            if !mods.join(&name).join(NATIVE_MANIFEST).is_file() {
                continue;
            }
            any = true;
            walk(&mods.join(&name), &name, &mut files)?;
        }
    }
    if !any {
        return Ok(None);
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut h = Sha256::new();
    for part in ["resincraft-native-1", base, profile] {
        h.update(part.as_bytes());
        h.update([0]);
    }
    for (rel, path) in files {
        let bytes = std::fs::read(&path)?;
        h.update(rel.as_bytes());
        h.update([0]);
        h.update((bytes.len() as u64).to_le_bytes());
        h.update(&bytes);
    }
    Ok(Some(hex::encode(h.finalize())))
}

/// What an instance's build was made from.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub fingerprint: String,
    /// The prepared Horizon build it started from.
    pub base: String,
    pub profile: String,
    /// Its folder in the instance's `horizon-native-build/` (Horizon's own name for it, `native-<commit>-<key>`).
    pub version: String,
    /// The mods in it, `id version`.
    pub mods: Vec<String>,
    /// Unix seconds.
    pub built: u64,
    /// `version` is its key in the shared store, not a folder of the instance's own.
    #[serde(default)]
    pub stored: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Need {
    /// No native mods: the plain build plays.
    NoMods,
    UpToDate,
    /// Never built for this instance (or the build is gone).
    FirstBuild,
    ModsChanged,
    /// The instance plays another Horizon build now.
    BaseChanged,
}

impl Need {
    pub fn build(self) -> bool {
        !matches!(self, Need::NoMods | Need::UpToDate)
    }
}

/// Whether the instance's native mods have to be built before it plays: `now` is the fingerprint of what is
/// installed (None: no native mods), `base` the build it plays now, `built` whether the recorded build's game is
/// still there.
pub fn need(now: Option<&str>, base: &str, record: Option<&Record>, built: bool) -> Need {
    let Some(now) = now else { return Need::NoMods };
    match record {
        Some(r) if built && r.fingerprint == now => Need::UpToDate,
        Some(r) if built && r.base != base => Need::BaseChanged,
        Some(_) if built => Need::ModsChanged,
        _ => Need::FirstBuild,
    }
}

fn build_dir(inst: &str) -> PathBuf {
    paths::instance_dir(inst).join(BUILD_DIR)
}

fn read_record(inst: &str) -> Option<Record> {
    serde_json::from_str(&std::fs::read_to_string(build_dir(inst).join(RECORD)).ok()?).ok()
}

fn client_in(dir: &Path) -> PathBuf {
    dir.join(if cfg!(windows) { "horizon-client.exe" } else { "horizon-client" })
}

/// The recorded build's folder, when its game is there.
fn built(record: Option<&Record>, inst: &str) -> Option<PathBuf> {
    let r = record?;
    plain_name(&r.version).then_some(())?;
    let dir = if r.stored { store().join(&r.version) } else { build_dir(inst).join(&r.version) };
    client_in(&dir).is_file().then_some(dir)
}

fn need_for(inst: &str, base: &str) -> Result<(Need, Option<Record>, Option<String>)> {
    let now = fingerprint(&paths::game_dir(inst).join(NATIVE_DIR), base, PROFILE)?;
    let record = read_record(inst);
    let have = built(record.as_ref(), inst).is_some();
    let need = need(now.as_deref(), base, record.as_ref(), have);
    // The same mods on the same base were built before: that build plays, nothing is compiled.
    if need.build() {
        if let Some(found) = now.as_deref().and_then(from_store) {
            if write_record(inst, &found).is_ok() {
                return Ok((Need::UpToDate, Some(found), now));
            }
        }
    }
    Ok((need, record, now))
}

fn write_record(inst: &str, r: &Record) -> Result<()> {
    let dir = build_dir(inst);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(RECORD), serde_json::to_string_pretty(r)?)?;
    Ok(())
}

// ---------- The shared store ----------

/// Every finished build, once, by what it was made from.
pub fn store() -> PathBuf {
    paths::games().join("horizon-native-store")
}

/// A fingerprint's folder name in the store.
pub fn store_key(fingerprint: &str) -> String {
    fingerprint.chars().take(24).collect()
}

/// The stored build made from exactly this fingerprint, when its game is there.
fn from_store(fingerprint: &str) -> Option<Record> {
    let key = store_key(fingerprint);
    let dir = store().join(&key);
    let r: Record = serde_json::from_str(&std::fs::read_to_string(dir.join(RECORD)).ok()?).ok()?;
    (r.fingerprint == fingerprint && r.version == key && r.stored && client_in(&dir).is_file()).then_some(r)
}

/// Replaces each file of `dir` that is byte for byte the same file of `base` (same path) with a hard link to it.
/// Returns the bytes saved. Where a link can't be made (another drive), the copy stays.
pub fn link_to_base(dir: &Path, base: &Path) -> u64 {
    fn read_full(f: &mut std::fs::File, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut n = 0;
        while n < buf.len() {
            match f.read(&mut buf[n..])? {
                0 => break,
                k => n += k,
            }
        }
        Ok(n)
    }
    fn same(a: &Path, b: &Path) -> bool {
        let (Ok(mut fa), Ok(mut fb)) = (std::fs::File::open(a), std::fs::File::open(b)) else { return false };
        let (mut ba, mut bb) = (vec![0u8; 1 << 16], vec![0u8; 1 << 16]);
        loop {
            let (Ok(na), Ok(nb)) = (read_full(&mut fa, &mut ba), read_full(&mut fb, &mut bb)) else { return false };
            if na != nb || ba[..na] != bb[..nb] {
                return false;
            }
            if na == 0 {
                return true;
            }
        }
    }
    fn walk(dir: &Path, base: &Path, saved: &mut u64) {
        let Ok(list) = std::fs::read_dir(dir) else { return };
        for e in list.flatten() {
            let Ok(kind) = e.file_type() else { continue };
            let (path, other) = (e.path(), base.join(e.file_name()));
            if kind.is_dir() {
                walk(&path, &other, saved);
                continue;
            }
            if !kind.is_file() {
                continue;
            }
            let (Ok(a), Ok(b)) = (std::fs::metadata(&path), std::fs::metadata(&other)) else { continue };
            if !b.is_file() || a.len() != b.len() || a.len() == 0 || !same(&path, &other) {
                continue;
            }
            let mut aside = path.clone().into_os_string();
            aside.push(".resincraft-link");
            let aside = PathBuf::from(aside);
            if std::fs::rename(&path, &aside).is_err() {
                continue;
            }
            if std::fs::hard_link(&other, &path).is_ok() {
                let _ = std::fs::remove_file(&aside);
                *saved += a.len();
            } else {
                let _ = std::fs::rename(&aside, &path);
            }
        }
    }
    let mut saved = 0;
    walk(dir, base, &mut saved);
    saved
}

/// Deletes stored builds no instance plays, but the newest `spare` of them (taking a mod out and putting it back
/// then plays at once). Returns the bytes freed.
pub fn prune_store(used: &HashSet<String>, spare: usize) -> u64 {
    let Ok(list) = std::fs::read_dir(store()) else { return 0 };
    let mut unused: Vec<(u64, PathBuf)> = list
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| !used.contains(&e.file_name().to_string_lossy().into_owned()))
        .map(|e| {
            let built = std::fs::read_to_string(e.path().join(RECORD)).ok().and_then(|t| serde_json::from_str::<Record>(&t).ok()).map(|r| r.built).unwrap_or(0);
            (built, e.path())
        })
        .collect();
    unused.sort_by(|a, b| b.0.cmp(&a.0));
    let mut freed = 0;
    for (_, dir) in unused.into_iter().skip(spare) {
        freed += crate::core::fsutil::dir_size(&dir);
        let _ = std::fs::remove_dir_all(&dir);
    }
    freed
}

/// The store keys the instances play.
async fn used_keys() -> HashSet<String> {
    let mut used = HashSet::new();
    for inst in crate::core::instances::list().await.unwrap_or_default() {
        if let Some(r) = read_record(&inst.id).filter(|r| r.stored) {
            used.insert(r.version);
        }
    }
    used
}

// ---------- Horizon's launcher ----------

/// Horizon's launcher's data folder for ResinCraft's builds.
pub fn launcher_home() -> PathBuf {
    paths::games().join("horizon-native")
}

/// Horizon's launcher, run on ResinCraft's own data folder: never the player's own Horizon data.
fn launcher(exe: &Path) -> tokio::process::Command {
    let home = launcher_home();
    let mut cmd = hidden_command(exe);
    cmd.env("HORIZON_LAUNCHER_HOME", &home)
        .env("HORIZON_HOME", home.join("game"))
        .env("HORIZON_LAUNCHER_NO_SELF_UPDATE", "1")
        .current_dir(&home)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    cmd
}

async fn run_json(exe: &Path, args: &[&str]) -> Result<Value> {
    tokio::fs::create_dir_all(launcher_home()).await?;
    let out = launcher(exe).args(args).output().await.context("Couldn't start Horizon's launcher")?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim_start_matches("error: ").to_string();
        return Err(anyhow!("{line}"));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(serde_json::from_str(text.trim()).unwrap_or(Value::Null))
}

/// The compile cache, as Horizon's launcher reports it.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheInfo {
    /// Horizon's launcher is installed: without it there's no cache to speak of.
    pub installed: bool,
    /// Its `native/target/`.
    pub path: Option<String>,
    pub bytes: u64,
    pub cleared_bytes: u64,
    pub limit_gib: u64,
    /// A build runs: the cache can't be read now.
    pub busy: bool,
    /// Everything the compiler keeps: its Rust, Cargo's home and its output.
    pub total_bytes: u64,
    /// The finished builds kept in the store.
    pub store_bytes: u64,
    pub keep_cache: bool,
}

/// ResinCraft's own settings for native builds, beside Horizon's launcher's.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    /// Keep the compile files after a build: rebuilds take minutes instead of a full compile, for several GiB.
    #[serde(default)]
    keep_cache: bool,
}

fn settings_file() -> PathBuf {
    launcher_home().join("resincraft.json")
}

fn settings() -> Settings {
    std::fs::read_to_string(settings_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn keep_cache() -> bool {
    settings().keep_cache
}

pub async fn set_keep_cache(keep: bool) -> Result<CacheInfo> {
    tokio::fs::create_dir_all(launcher_home()).await?;
    let s = Settings { keep_cache: keep };
    tokio::fs::write(settings_file(), serde_json::to_string_pretty(&s)?).await?;
    cache(false).await
}

/// The folder Horizon's launcher builds in (`<build dir>/native`), once read: its log is there.
static NATIVE_ROOT: Mutex<Option<PathBuf>> = Mutex::new(None);
static BUILDING: AtomicBool = AtomicBool::new(false);

fn read_cache(v: &Value) -> CacheInfo {
    let path = v["path"].as_str().map(str::to_string);
    if let Some(p) = &path {
        *NATIVE_ROOT.lock().unwrap() = Path::new(p).parent().map(Path::to_path_buf);
    }
    CacheInfo {
        installed: true,
        path,
        bytes: v["bytes"].as_u64().unwrap_or(0),
        cleared_bytes: v["cleared_bytes"].as_u64().unwrap_or(0),
        limit_gib: v["limit_gib"].as_u64().unwrap_or(30),
        keep_cache: keep_cache(),
        ..Default::default()
    }
}

/// Horizon's launcher's build folder: its Rust, Cargo's home and `native/`.
fn build_root() -> Option<PathBuf> {
    NATIVE_ROOT.lock().unwrap().as_ref().and_then(|r| r.parent().map(Path::to_path_buf))
}

/// The size of what the compiler keeps, read from disk.
fn compile_bytes() -> u64 {
    build_root().map(|b| ["rust", "cargo", "native"].iter().map(|d| crate::core::fsutil::dir_size(&b.join(d))).sum()).unwrap_or(0)
}

async fn sizes(mut info: CacheInfo) -> CacheInfo {
    let store = store();
    let (total, stored) = tokio::task::spawn_blocking(move || (compile_bytes(), crate::core::fsutil::dir_size(&store))).await.unwrap_or((0, 0));
    info.total_bytes = total;
    info.store_bytes = stored;
    info
}

/// Deletes the compile files: the compiler's output, then Horizon's launcher's Rust and Cargo's home. Finished
/// builds stay. Returns the bytes freed.
async fn clean_compile_files(exe: &Path) -> Result<u64> {
    let before = tokio::task::spawn_blocking(compile_bytes).await.unwrap_or(0);
    let v = run_json(exe, &["native-cache", "--clear"]).await?;
    read_cache(&v);
    run_json(exe, &["rust", "remove"]).await?;
    let after = tokio::task::spawn_blocking(compile_bytes).await.unwrap_or(0);
    Ok(before.saturating_sub(after))
}

pub async fn cache(clear: bool) -> Result<CacheInfo> {
    let Some(exe) = crate::player::horizon::installed_program() else {
        return Ok(sizes(CacheInfo { limit_gib: 30, keep_cache: keep_cache(), ..Default::default() }).await);
    };
    if BUILDING.load(Ordering::SeqCst) {
        if clear {
            bail_tr!("horizon.nativeBusy", "A native build is running. Try again when it finishes.");
        }
        return Ok(CacheInfo { installed: true, busy: true, limit_gib: 30, keep_cache: keep_cache(), ..Default::default() });
    }
    if clear {
        let mut freed = clean_compile_files(&exe).await?;
        let used = used_keys().await;
        freed += tokio::task::spawn_blocking(move || prune_store(&used, 0)).await.unwrap_or(0);
        let mut info = read_cache(&run_json(&exe, &["native-cache"]).await?);
        info.cleared_bytes = freed;
        return Ok(sizes(info).await);
    }
    Ok(sizes(read_cache(&run_json(&exe, &["native-cache"]).await?)).await)
}

/// The cache's limit in GiB (0: no automatic cleanup), kept in Horizon's launcher's settings.
pub async fn set_cache_limit(gib: u64) -> Result<CacheInfo> {
    let exe = crate::player::horizon::ensure_program().await?;
    run_json(&exe, &["settings", "native-cache-gib", &gib.min(10_000).to_string()]).await?;
    cache(false).await
}

/// The last `lines` lines of the newest build's log.
pub async fn log_tail(lines: usize) -> Vec<String> {
    let root = NATIVE_ROOT.lock().unwrap().clone();
    let Some(root) = root else { return Vec::new() };
    let Ok(bytes) = tokio::fs::read(root.join("build.log")).await else { return Vec::new() };
    tail(&String::from_utf8_lossy(&bytes), lines)
}

/// A log's last non-empty lines, without the progress bar's carriage returns or the C compiler's warnings that
/// build scripts pass on (hundreds of them, which would hide the lines that matter).
pub fn tail(text: &str, lines: usize) -> Vec<String> {
    let all: Vec<String> = text
        .lines()
        .map(|l| l.rsplit('\r').next().unwrap_or(l).trim_end().to_string())
        .filter(|l| !l.trim().is_empty() && !(l.starts_with("warning: ") && l.contains("@")))
        .collect();
    all[all.len().saturating_sub(lines)..].to_vec()
}

/// Horizon's launcher's settings for ResinCraft's builds: English, so its progress reads the same everywhere.
async fn prepare_launcher(exe: &Path) {
    let _ = run_json(exe, &["settings", "language", "en"]).await;
}

/// Puts the prepared base build where Horizon's launcher looks for versions, as hard links where the disk allows
/// (nothing is copied twice), and marks it installed.
async fn link_base(build: &Build) -> Result<()> {
    let to = launcher_home().join("versions").join(&build.id);
    if to.join(".installed").is_file() {
        return Ok(());
    }
    let (from, partial) = (build.root.clone(), launcher_home().join("versions").join(format!(".{}.partial", build.id)));
    tokio::task::spawn_blocking(move || -> Result<()> {
        let _ = std::fs::remove_dir_all(&partial);
        let jobs = crate::core::fsutil::list_copy(&from, &partial, &[])?;
        for (src, dst, _) in &jobs {
            if std::fs::hard_link(src, dst).is_err() {
                std::fs::copy(src, dst).with_context(|| format!("Couldn't copy {}", src.display()))?;
            }
        }
        let _ = std::fs::remove_dir_all(&to);
        std::fs::rename(&partial, &to)?;
        std::fs::write(to.join(".installed"), "")?;
        Ok(())
    })
    .await?
}

/// A folder name with nothing that climbs out of its folder.
fn plain_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 120 && !s.starts_with('.') && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}

/// The version `native-build` printed ("native-… built and selected for its instance").
pub fn built_version(stdout: &str) -> Option<String> {
    stdout
        .split_whitespace()
        .find(|w| w.starts_with("native-") && w.len() > 7)
        .map(|w| w.trim_end_matches(['.', ',', ':']).to_string())
        .filter(|w| plain_name(w))
}

/// "Compiling the game (performance): 42 of 391 crates, now foldhash" as a fraction. Horizon's launcher speaks
/// English here: ResinCraft sets its language (`prepare_launcher`).
pub fn step_progress(line: &str) -> Option<f64> {
    let rest = line.split_once("Compiling the game")?.1.split_once("): ")?.1;
    let mut words = rest.split_whitespace();
    let done: f64 = words.next()?.parse().ok()?;
    (words.next()? == "of").then_some(())?;
    let total: f64 = words.next()?.parse().ok()?;
    (total > 0.0 && done <= total).then(|| done / total)
}

/// Moves a finished build out of Horizon's launcher's versions into the instance: across drives, a copy.
fn move_dir(from: &Path, to: &Path) -> Result<()> {
    let _ = std::fs::remove_dir_all(to);
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    crate::core::instances::copy_dir(from, to, &[])?;
    let _ = std::fs::remove_dir_all(from);
    Ok(())
}

// ---------- Building ----------

/// What the Content tab and the build warning show.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub mods: Vec<Installed>,
    pub need: Need,
    /// The base the build starts from; None when the instance has no build yet (it is installed on its first start).
    pub base: Option<String>,
    /// The build it plays now, if it has one.
    pub built: Option<Record>,
    /// Horizon's launcher has no compiler here yet: the first build downloads Rust and compiles the whole game.
    pub first_ever: bool,
    pub building: bool,
}

/// The base build an instance plays without installing anything: its pinned build, else the newest installed.
async fn base_now(inst: &Instance) -> Option<String> {
    match inst.loader_version.as_deref().filter(|v| !v.is_empty()) {
        Some(v) => Some(v.to_string()),
        None => super::horizon::current_id().await,
    }
}

pub async fn status(inst: &Instance) -> Result<Status> {
    let mods = installed(&paths::game_dir(&inst.id));
    let base = base_now(inst).await;
    let (need, record) = match &base {
        Some(b) => {
            let (n, r, _) = need_for(&inst.id, b)?;
            (n, r)
        }
        None => (if mods.is_empty() { Need::NoMods } else { Need::FirstBuild }, None),
    };
    let first_ever = match NATIVE_ROOT.lock().unwrap().clone() {
        Some(root) => !root.parent().is_some_and(|b| b.join("rust").is_dir()),
        None => true,
    };
    let built = record.filter(|r| built(Some(r), &inst.id).is_some());
    Ok(Status { built, mods, need, base, first_ever, building: BUILDING.load(Ordering::SeqCst) })
}

struct Building;
impl Drop for Building {
    fn drop(&mut self) {
        BUILDING.store(false, Ordering::SeqCst);
    }
}

/// Builds the instance's native mods into the game it plays, with Horizon's launcher. `trusted` is the player's
/// answer to the warning the UI shows before every build: without it nothing is compiled.
pub async fn build(instance_id: &str, trusted: bool, anyway: bool) -> Result<Record> {
    if !trusted {
        bail_tr!("horizon.nativeTrust", "Native mods run with full access to this computer: confirm that you trust them before they are built.");
    }
    let inst = crate::core::instances::get(instance_id).await?;
    if crate::game::launch::running_instances().iter().any(|r| r == &inst.id) {
        bail_tr!("horizon.nativeRunning", "Close {name} first: its game is running.", name = inst.name);
    }
    if installed(&paths::game_dir(&inst.id)).is_empty() {
        bail_tr!("horizon.nativeNone", "{name} has no native mods to build.", name = inst.name);
    }
    if BUILDING.swap(true, Ordering::SeqCst) {
        bail_tr!("horizon.nativeBusy", "A native build is running. Try again when it finishes.");
    }
    let guard = Building;
    let title = tr!("horizon.nativeBuilding", "Building native mods for {name}", name = inst.name);
    let r = Task::run(title, Some(inst.id.clone()), |task| async move {
        let _guard = guard;
        let build = super::horizon::build_for(&inst, &task).await?;
        task.update(tr!("horizon.nativeLauncher", "Getting Horizon's launcher…"), None);
        let exe = crate::player::horizon::ensure_program().await?;
        prepare_launcher(&exe).await;
        // Where the build log will be: asked before the build takes the cache's lock.
        if let Ok(v) = run_json(&exe, &["native-cache"]).await {
            read_cache(&v);
        }
        // Built before with exactly these mods on this base: nothing to compile.
        if let Some(print) = fingerprint(&paths::game_dir(&inst.id).join(NATIVE_DIR), &build.id, PROFILE)? {
            if let Some(found) = from_store(&print) {
                write_record(&inst.id, &found)?;
                return Ok(found);
            }
        }
        task.update(tr!("horizon.nativeBase", "Getting Horizon {build} ready to build on…", build = build.id), None);
        link_base(&build).await?;
        run_build(&exe, &inst, &build, &task, anyway).await
    })
    .await;
    r
}

/// Stops Horizon's launcher and the compiler it started when the build is dropped before it ended (Cancel in the
/// dock): `kill_on_drop` alone would leave Cargo and rustc compiling.
struct StopTree(Option<u32>);

impl Drop for StopTree {
    fn drop(&mut self) {
        if let Some(pid) = self.0.take() {
            super::commands::kill_tree(pid);
        }
    }
}

async fn run_build(exe: &Path, inst: &Instance, build: &Build, task: &Arc<Task>, anyway: bool) -> Result<Record> {
    let game = paths::game_dir(&inst.id);
    let mods_dir = game.join(NATIVE_DIR);
    let mods = installed(&game);
    let print = fingerprint(&mods_dir, &build.id, PROFILE)?.ok_or_else(|| anyhow!("no native mods"))?;
    let mut cmd = launcher(exe);
    cmd.arg("native-build")
        .arg(&mods_dir)
        .args(["--version", &build.id, "--trust-native-code", "--profile", PROFILE])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    // The player chose to build mods made for other Horizon versions together: on the newest of them, each on it.
    if anyway {
        cmd.args(["--allow-version-mismatch", "--ignore-pins"]);
    }
    let mut child = cmd.spawn().context("Couldn't start Horizon's launcher")?;
    let mut stop = StopTree(child.id());
    let (out, err) = (child.stdout.take().unwrap(), child.stderr.take().unwrap());
    let stdout = tokio::spawn(async move {
        let mut text = String::new();
        let _ = BufReader::new(out).read_to_string(&mut text).await;
        text
    });
    let errors = Arc::new(Mutex::new(Vec::<String>::new()));
    let reader = {
        let (task, errors) = (task.clone(), errors.clone());
        tokio::spawn(async move {
            let mut lines = BufReader::new(err).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let line = line.rsplit('\r').next().unwrap_or(&line).trim().to_string();
                if line.is_empty() {
                    continue;
                }
                task.update(line.chars().take(160).collect::<String>(), step_progress(&line));
                let mut e = errors.lock().unwrap();
                e.push(line);
                let n = e.len();
                if n > 400 {
                    e.drain(..n - 400);
                }
            }
        })
    };
    let status = child.wait().await?;
    stop.0 = None;
    let _ = reader.await;
    let stdout = stdout.await.unwrap_or_default();
    if !status.success() {
        let said: Vec<String> = errors.lock().unwrap().clone();
        let from = said.iter().position(|l| l.starts_with("error:")).unwrap_or(said.len().saturating_sub(12));
        let why = said[from..].join("\n");
        let why = why.trim_start_matches("error: ").to_string();
        bail_tr!("horizon.nativeFailed", "The native build failed: {why}", why = if why.is_empty() { status.to_string() } else { why });
    }
    let version = built_version(&stdout).ok_or_else(|| anyhow!("Horizon's launcher didn't say which version it built ({})", stdout.trim()))?;
    task.update(tr!("horizon.nativeInstalling", "Putting the build in {name}…", name = inst.name), None);
    let from = launcher_home().join("versions").join(&version);
    let key = store_key(&print);
    let record = Record {
        fingerprint: print,
        base: build.id.clone(),
        profile: PROFILE.to_string(),
        version: key.clone(),
        mods: mods.iter().map(|m| format!("{} {}", m.id, m.version)).collect(),
        built: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
        stored: true,
    };
    let (r, base_root, inst_id) = (record.clone(), build.root.clone(), inst.id.clone());
    tokio::task::spawn_blocking(move || -> Result<()> {
        let to = store().join(&key);
        move_dir(&from, &to)?;
        let saved = link_to_base(&to, &base_root);
        eprintln!("[horizon] native build {key}: {} MiB shared with {}", saved >> 20, base_root.display());
        std::fs::write(to.join(RECORD), serde_json::to_string_pretty(&r)?)?;
        // The instance's own builds from before the store go.
        if let Ok(list) = std::fs::read_dir(build_dir(&inst_id)) {
            for e in list.flatten() {
                if e.file_type().is_ok_and(|t| t.is_dir()) {
                    let _ = std::fs::remove_dir_all(e.path());
                }
            }
        }
        write_record(&inst_id, &r)
    })
    .await??;
    let used = used_keys().await;
    let _ = tokio::task::spawn_blocking(move || prune_store(&used, 1)).await;
    if !keep_cache() {
        task.update(tr!("horizon.nativeCleaning", "Deleting the compile files to free space…"), None);
        match clean_compile_files(exe).await {
            Ok(freed) => eprintln!("[horizon] freed {} MiB of compile files", freed >> 20),
            Err(e) => eprintln!("[horizon] couldn't delete the compile files: {e:#}"),
        }
    }
    Ok(record)
}

// ---------- Playing ----------

/// Instances whose next start plays without their native mods (the player chose so after a failed build).
static SKIP_ONCE: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn skip_once(instance_id: &str) {
    SKIP_ONCE.lock().unwrap().push(instance_id.to_string());
}

/// The folder of the native build an instance plays on `base` (its game and resources), None to play `base` as it
/// is. Native mods that aren't built (or changed since) stop the start: the UI then asks to build them.
pub async fn for_launch(inst: &Instance, base: &Build) -> Result<Option<PathBuf>> {
    let (need, record, _) = need_for(&inst.id, &base.id)?;
    let skip = {
        let mut list = SKIP_ONCE.lock().unwrap();
        let had = list.iter().any(|i| i == &inst.id);
        list.retain(|i| i != &inst.id);
        had
    };
    match need {
        Need::NoMods => Ok(None),
        _ if skip => Ok(None),
        Need::UpToDate => Ok(built(record.as_ref(), &inst.id)),
        _ => Err(err_tr!("horizon.nativeNeedsBuild", "{name}'s native mods need to be built before it plays.", name = inst.name)),
    }
}

/// The game program in a build folder.
pub fn client(dir: &Path) -> PathBuf {
    client_in(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZOOM: &str = r#"{"game":"26.3","hooks":1,"horizon":"6f8c7a0e5d531d3834c4d0be4659a1db8e7e5c2a","id":"zoom","mixins":"0d6b103c70633d8fd9443d8d0acd3aadaacedb06","schema":1,"version":"2.0.0"}"#;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("resincraft-native-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn hnmod(dir: &Path, files: &[(&str, &[u8])]) -> PathBuf {
        let path = dir.join("mod.hnmod");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (name, bytes) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    fn good() -> Vec<(&'static str, &'static [u8])> {
        vec![
            (NATIVE_MANIFEST, ZOOM.as_bytes()),
            ("mixins/zoom/horizon-mixin.toml", b"[mod]\nid = \"zoom\"\nversion = \"2.0.0\"\n"),
            ("mixins/zoom/zoom.rs", b"// zoom"),
            ("mixins/zoom/data/levels.txt", b"1 2 4"),
            ("mixins/zoom/engine/data/salt.txt", b"salt"),
            ("mixins/zoom/LICENSE", b"MIT"),
        ]
    }

    #[test]
    fn manifest_is_checked() {
        let m = parse_manifest(ZOOM).unwrap();
        assert_eq!((m.id.as_str(), m.version.as_str(), m.hooks), ("zoom", "2.0.0", Some(1)));
        assert!(parse_manifest(&ZOOM.replace("\"schema\":1", "\"schema\":2")).is_err());
        assert!(parse_manifest(&ZOOM.replace("\"id\":\"zoom\"", "\"id\":\"../zoom\"")).is_err());
        assert!(parse_manifest(&ZOOM.replace("6f8c7a0e5d531d3834c4d0be4659a1db8e7e5c2a", "main")).is_err());
        assert!(parse_manifest(&ZOOM.replace("\"schema\":1", "\"schema\":1,\"run\":\"x\"")).is_err());
    }

    #[test]
    fn entry_paths_are_filtered() {
        for ok in ["horizon-native.json", "mixins/zoom/horizon-mixin.toml", "mixins/zoom/a/b.rs", "mixins/zoom/data/x/y.png", "mixins/zoom/engine/data/salt.bin", "mixins/zoom/README.md", "mixins/zoom/engine/LICENSE-MIT"] {
            assert!(entry_target(ok, "zoom").is_ok(), "{ok}");
        }
        for bad in [
            "../horizon-native.json",
            "mixins/zoom/../../evil.rs",
            "mixins/zoom/./zoom.rs",
            "mixins/other/zoom.rs",
            "mixins/zoom/build.sh",
            "mixins/zoom/a/b/README",
            "mixins/zoom\\..\\evil.rs",
            "/mixins/zoom/zoom.rs",
            "C:/mixins/zoom/zoom.rs",
            "mixins//zoom/zoom.rs",
            "mod.wasm",
            "horizon-mod.toml",
            "",
        ] {
            assert!(entry_target(bad, "zoom").is_err(), "{bad}");
        }
    }

    #[test]
    fn unpacks_into_its_own_folder() {
        let dir = scratch("unpack");
        let into = dir.join(NATIVE_DIR);
        let file = hnmod(&dir, &good());
        let m = unpack(&file, &into, Some("zoom")).unwrap();
        assert_eq!(m.id, "zoom");
        assert!(into.join("zoom/horizon-native.json").is_file());
        assert_eq!(std::fs::read(into.join("zoom/mixins/zoom/engine/data/salt.txt")).unwrap(), b"salt");
        assert_eq!(installed(&dir), vec![Installed { id: "zoom".into(), version: "2.0.0".into(), game: "26.3".into(), horizon: "6f8c7a0e5d531d3834c4d0be4659a1db8e7e5c2a".into() }]);

        // A refused file leaves the installed copy as it was, and writes nothing outside.
        for files in [
            vec![(NATIVE_MANIFEST, ZOOM.as_bytes()), ("mixins/zoom/horizon-mixin.toml", b"x".as_slice()), ("mixins/zoom/../../../evil.rs", b"x")],
            vec![(NATIVE_MANIFEST, ZOOM.as_bytes()), ("mixins/zoom/horizon-mixin.toml", b"x".as_slice()), ("evil.exe", b"MZ")],
            vec![(NATIVE_MANIFEST, ZOOM.as_bytes()), ("mixins/zoom/zoom.rs", b"x".as_slice())],
            vec![("mixins/zoom/horizon-mixin.toml", b"x".as_slice())],
        ] {
            let file = hnmod(&dir, &files);
            assert!(unpack(&file, &into, Some("zoom")).is_err());
            assert!(into.join("zoom/mixins/zoom/zoom.rs").is_file());
            assert!(!dir.join("evil.rs").exists() && !into.join(".zoom.incoming").exists());
        }
        // The registry's id must be the one inside.
        let file = hnmod(&dir, &good());
        assert!(unpack(&file, &into, Some("fps_overlay")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_link_in_the_zip_is_refused() {
        let dir = scratch("link");
        let path = dir.join("link.hnmod");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (name, bytes) in good() {
            zip.start_file(name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.add_symlink("mixins/zoom/link.rs", "/etc/passwd", options).unwrap();
        zip.finish().unwrap();
        let e = unpack(&path, &dir.join(NATIVE_DIR), None).unwrap_err();
        assert!(e.to_string().contains("plain file"), "{e}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn fingerprint_follows_the_files_and_the_base() {
        let dir = scratch("print");
        let mods = dir.join(NATIVE_DIR);
        assert_eq!(fingerprint(&mods, "nightly-a", PROFILE).unwrap(), None);
        unpack(&hnmod(&dir, &good()), &mods, None).unwrap();
        let first = fingerprint(&mods, "nightly-a", PROFILE).unwrap().unwrap();
        assert_eq!(fingerprint(&mods, "nightly-a", PROFILE).unwrap().unwrap(), first);
        assert_ne!(fingerprint(&mods, "nightly-b", PROFILE).unwrap().unwrap(), first);
        assert_ne!(fingerprint(&mods, "nightly-a", "release").unwrap().unwrap(), first);
        // A leftover unpack and stray files beside the mods don't count.
        std::fs::create_dir_all(mods.join(".zoom.incoming")).unwrap();
        std::fs::write(mods.join(".zoom.incoming/x.rs"), "x").unwrap();
        std::fs::write(mods.join("notes.txt"), "x").unwrap();
        assert_eq!(fingerprint(&mods, "nightly-a", PROFILE).unwrap().unwrap(), first);
        std::fs::write(mods.join("zoom/mixins/zoom/zoom.rs"), "// changed").unwrap();
        let changed = fingerprint(&mods, "nightly-a", PROFILE).unwrap().unwrap();
        assert_ne!(changed, first);
        // A file moved to another path is another set, even with the same bytes.
        std::fs::rename(mods.join("zoom/mixins/zoom/zoom.rs"), mods.join("zoom/mixins/zoom/zoom2.rs")).unwrap();
        assert_ne!(fingerprint(&mods, "nightly-a", PROFILE).unwrap().unwrap(), changed);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn build_needed_when_mods_or_base_change() {
        let r = Record { fingerprint: "f1".into(), base: "nightly-a".into(), profile: PROFILE.into(), version: "native-x".into(), mods: vec![], built: 0, stored: false };
        assert_eq!(need(None, "nightly-a", Some(&r), true), Need::NoMods);
        assert_eq!(need(Some("f1"), "nightly-a", Some(&r), true), Need::UpToDate);
        assert_eq!(need(Some("f2"), "nightly-a", Some(&r), true), Need::ModsChanged);
        assert_eq!(need(Some("f3"), "nightly-b", Some(&r), true), Need::BaseChanged);
        assert_eq!(need(Some("f1"), "nightly-a", Some(&r), false), Need::FirstBuild);
        assert_eq!(need(Some("f1"), "nightly-a", None, false), Need::FirstBuild);
        assert!(!Need::UpToDate.build() && !Need::NoMods.build() && Need::ModsChanged.build());
    }

    #[test]
    fn built_files_become_links_to_the_base() {
        let dir = scratch("links");
        let (base, built) = (dir.join("base"), dir.join("built"));
        for d in [&base, &built] {
            std::fs::create_dir_all(d.join("res/sub")).unwrap();
            std::fs::write(d.join("res/sub/same.bin"), vec![7u8; 200_000]).unwrap();
        }
        std::fs::write(base.join("horizon-client.exe"), "old").unwrap();
        std::fs::write(built.join("horizon-client.exe"), "new").unwrap();
        std::fs::write(base.join("res/other.txt"), "aaaa").unwrap();
        std::fs::write(built.join("res/other.txt"), "aaab").unwrap();
        assert_eq!(link_to_base(&built, &base), 200_000);
        assert_eq!(std::fs::read(built.join("res/sub/same.bin")).unwrap(), vec![7u8; 200_000]);
        assert_eq!(std::fs::read_to_string(built.join("horizon-client.exe")).unwrap(), "new");
        assert_eq!(std::fs::read_to_string(built.join("res/other.txt")).unwrap(), "aaab");
        assert!(!built.join("res/sub/same.bin.resincraft-link").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn store_keys_are_plain_names() {
        let key = store_key(&"ab12".repeat(16));
        assert_eq!(key.len(), 24);
        assert!(plain_name(&key));
    }

    #[test]
    fn reads_what_the_launcher_says() {
        let out = "native-6475f62-1a2b3c4d built and selected for its instance (812 s).\n";
        assert_eq!(built_version(out).as_deref(), Some("native-6475f62-1a2b3c4d"));
        assert_eq!(built_version("Nothing built."), None);
        assert_eq!(step_progress("[ 5:35] Compiling the game (performance): 42 of 391 crates, now foldhash"), Some(42.0 / 391.0));
        assert_eq!(step_progress("[ 5:35] Compiling the game (performance): 42 crates, now foldhash"), None);
        assert_eq!(step_progress("[ 0:54] Downloading Rust 1.99.1 (once): 100%"), None);
        assert_eq!(tail("a\n\nb\rc\nd\n", 2), ["c", "d"]);
        assert_eq!(tail("error[E0425]: x\nwarning: libmimalloc-sys@0.1.49: static.c:41: warning\n", 2), ["error[E0425]: x"]);
    }
}
