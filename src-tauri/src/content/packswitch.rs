//! Switching a modpack instance to another version of its pack, newer or older.
//!
//! Each source describes one pack version as a `PackTarget`: files to download, archives whose entries go over the
//! game directory (overrides, configs) and files only downloadable from the author's website. A switch:
//! 1. works out the files of the installed version (`modpack-files.json`, written at install and after each switch,
//!    else the source's manifest of that version),
//! 2. stages the new version's files that differ from what's on disk under `<instance>/.pack-switch/stage`,
//! 3. plans what to place and remove (`plan`), keeping what belongs to the player,
//! 4. applies it (`apply`), moving everything it replaces or removes into `.pack-switch/backup` and journaling each
//!    step first, so a failure, Cancel or crash puts the instance back exactly as it was (`restore`).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use futures::future::{FutureExt, Shared};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::dupes::dedupe_pack;
use super::index::{index_key, load_index, open_zip, save_index, IndexEntry};
use super::{atlauncher, curseforge, ftb, modrinth, technic};
use crate::core::events::Task;
use crate::core::fsutil::{read_json, safe_join, write_json};
use crate::core::instances;
use crate::core::net::{download_all, DownloadItem, Hash, HashAlgo};
use crate::core::paths;
use crate::core::types::{source_of, ContentKind, ContentSource, Instance, LoaderType, PendingDownload};
use crate::{bail_tr, err_tr, tr};

// ---------- What a pack version holds ----------

/// A file the pack downloads to `path` (relative to the game directory).
#[derive(Debug, Clone)]
pub struct TargetFile {
    pub path: String,
    pub url: String,
    pub hash: Option<Hash>,
    pub size: Option<u64>,
}

/// An archive (already downloaded) whose entries under `prefix` go to `to` in the game directory.
#[derive(Debug, Clone)]
pub struct Layer {
    pub zip: PathBuf,
    pub prefix: String,
    pub to: String,
    /// Technic packs carry the Technic Launcher's own loader in bin/; ResinCraft installs the loader itself.
    pub skip_bin: bool,
}

impl Layer {
    pub fn new(zip: PathBuf, prefix: impl Into<String>) -> Layer {
        Layer { zip, prefix: prefix.into(), to: String::new(), skip_bin: false }
    }

    /// Where an entry of the archive goes, relative to the game directory; None for entries this layer skips.
    fn map(&self, name: &str) -> Option<String> {
        if self.skip_bin && name.get(..4).is_some_and(|p| p.eq_ignore_ascii_case("bin/")) {
            return None;
        }
        let rest = name.strip_prefix(self.prefix.as_str())?;
        if rest.is_empty() || rest.ends_with('/') {
            return None;
        }
        let to = self.to.trim_matches('/');
        Some(norm(&if to.is_empty() { rest.to_string() } else { format!("{to}/{rest}") }))
    }
}

/// One version of a pack, as its source describes it. Later layers win over earlier ones and over `files`.
#[derive(Debug, Clone)]
pub struct PackTarget {
    pub version_id: String,
    pub version_name: String,
    pub mc_version: String,
    pub loader: LoaderType,
    pub loader_version: Option<String>,
    pub files: Vec<TargetFile>,
    pub layers: Vec<Layer>,
    pub pending: Vec<PendingDownload>,
    /// Content index entries for downloaded files, by path (CurseForge knows them up front).
    pub index: Vec<(String, IndexEntry)>,
}

/// Which side of the game a pack version is wanted for: an instance (client) or a hosted server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Client,
    Server,
}

/// Fetches the description of `version_id` of a pack, downloading its archives into `work`.
pub(crate) async fn target(project_id: &str, version_id: &str, work: &Path, task: &Arc<Task>, side: Side) -> Result<PackTarget> {
    tokio::fs::create_dir_all(work).await?;
    match (source_of(project_id), side) {
        (ContentSource::Modrinth, _) => modrinth::pack_target(version_id, work, task, side).await,
        (ContentSource::Curseforge, _) => curseforge::pack_target(project_id, version_id, work, task, side).await,
        (ContentSource::Ftb, _) => ftb::pack_target(project_id, version_id, side).await,
        (ContentSource::Atlauncher, Side::Client) => atlauncher::pack_target(project_id, version_id, work, task).await,
        (ContentSource::Technic, Side::Client) => technic::pack_target(project_id, version_id, work, task).await,
        (ContentSource::Github, _) => bail_tr!("github.modsOnly", "GitHub only offers mods here"),
        (_, Side::Server) => bail_tr!("packswitch.noServerFiles", "This modpack's site doesn't offer files for servers"),
    }
}

// ---------- Paths and hashes ----------

/// A path relative to the game directory, with forward slashes and no leading "./".
pub fn norm(p: &str) -> String {
    p.replace('\\', "/").split('/').filter(|s| !s.is_empty() && *s != ".").collect::<Vec<_>>().join("/")
}

fn pending_path(p: &PendingDownload) -> String {
    norm(&format!("{}/{}", p.folder, p.file_name))
}

/// What belongs to the player (or the host) whatever a pack ships: never changed or removed once it exists.
#[derive(Debug, Clone, Default)]
pub struct Keep {
    /// Files (lowercase paths).
    files: Vec<String>,
    /// Folders kept whole once they exist.
    dirs: Vec<String>,
    /// Folders whose entries are kept one by one (each world under saves/).
    per_entry: Vec<String>,
    /// Top folders a pack never provides on this side (a server's software, client-only folders).
    never: Vec<String>,
}

impl Keep {
    /// An instance: worlds, screenshots, logs, options and the server list.
    pub fn client() -> Keep {
        Keep {
            files: ["options.txt", "optionsof.txt", "optionsshaders.txt", "servers.dat", "servers.dat_old"].map(String::from).to_vec(),
            dirs: ["screenshots", "logs", "crash-reports"].map(String::from).to_vec(),
            per_entry: vec!["saves".into()],
            never: vec![],
        }
    }

    /// A hosted server whose world folder is `level` (server.properties `level-name`): its worlds and player data, the
    /// player lists, server.properties and eula.txt. The server software (jars, libraries, start scripts) is
    /// ResinCraft's, and client-only folders never go to a server.
    pub fn server(level: &str) -> Keep {
        let level = norm(level).to_lowercase();
        let mut dirs: Vec<String> = ["world", "world_nether", "world_the_end", "logs", "crash-reports", "backups"].map(String::from).to_vec();
        if !level.is_empty() {
            for d in [level.clone(), format!("{level}_nether"), format!("{level}_the_end")] {
                if !dirs.contains(&d) {
                    dirs.push(d);
                }
            }
        }
        let files = [
            "server.properties",
            "eula.txt",
            "whitelist.json",
            "ops.json",
            "banned-players.json",
            "banned-ips.json",
            "usercache.json",
            "permissions.json",
        ];
        let never = [".pack-switch", "libraries", "versions", ".fabric", ".quilt", "resourcepacks", "shaderpacks", "screenshots", "saves"];
        Keep { files: files.map(String::from).to_vec(), dirs, per_entry: vec![], never: never.map(String::from).to_vec() }
    }

    /// Paths a pack never provides on this side: left out of what a version holds, and never removed.
    pub fn excluded(&self, path: &str) -> bool {
        if self.never.is_empty() {
            return false;
        }
        let lower = path.to_lowercase();
        let first = lower.split('/').next().unwrap_or_default();
        if self.never.iter().any(|d| d == first) {
            return true;
        }
        // At the top of a server: the server software's jars and start scripts, ResinCraft's files, client settings.
        !lower.contains('/')
            && ([".jar", ".sh", ".bat", ".cmd", ".ps1"].iter().any(|e| lower.ends_with(e))
                || matches!(
                    lower.as_str(),
                    "user_jvm_args.txt" | "server.json" | "modpack-files.json" | "instance-sync.json" | "options.txt" | "optionsof.txt" | "optionsshaders.txt" | "servers.dat"
                ))
    }

    /// The file or folder that makes `path` the player's when it exists.
    fn root(&self, path: &str) -> Option<String> {
        let lower = path.to_lowercase();
        if self.files.contains(&lower) {
            return Some(path.to_string());
        }
        let mut parts = path.split('/');
        let first = parts.next()?;
        let first_l = first.to_lowercase();
        if self.dirs.contains(&first_l) {
            return Some(first.to_string());
        }
        if self.per_entry.contains(&first_l) {
            return Some(match parts.next() {
                Some(second) => format!("{first}/{second}"),
                None => path.to_string(),
            });
        }
        None
    }

    fn protects(&self, game: &Path, path: &str) -> bool {
        self.excluded(path) || self.root(path).is_some_and(|r| game.join(&r).exists())
    }
}

fn hash_text(h: &Option<Hash>) -> String {
    match h {
        Some(h) => format!("{}:{}", algo_name(h.algo), h.value.to_lowercase()),
        None => String::new(),
    }
}

fn algo_name(a: HashAlgo) -> &'static str {
    match a {
        HashAlgo::Md5 => "md5",
        HashAlgo::Sha1 => "sha1",
        HashAlgo::Sha256 => "sha256",
        HashAlgo::Sha512 => "sha512",
    }
}

fn parse_hash(s: &str) -> Option<Hash> {
    let (algo, value) = s.split_once(':')?;
    let algo = match algo {
        "md5" => HashAlgo::Md5,
        "sha1" => HashAlgo::Sha1,
        "sha256" => HashAlgo::Sha256,
        "sha512" => HashAlgo::Sha512,
        _ => return None,
    };
    Some(Hash { algo, value: value.to_string() })
}

pub fn hash_reader(mut r: impl Read, algo: HashAlgo) -> std::io::Result<String> {
    use sha2::Digest;
    let mut buf = vec![0u8; 1 << 16];
    macro_rules! run {
        ($h:expr) => {{
            let mut h = $h;
            loop {
                let n = r.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                h.update(&buf[..n]);
            }
            hex::encode(h.finalize())
        }};
    }
    Ok(match algo {
        HashAlgo::Md5 => run!(md5::Md5::new()),
        HashAlgo::Sha1 => run!(sha1::Sha1::new()),
        HashAlgo::Sha256 => run!(sha2::Sha256::new()),
        HashAlgo::Sha512 => run!(sha2::Sha512::new()),
    })
}

/// A file's hash; None if it can't be read (blocking).
pub fn hash_reader_file(p: &Path, algo: HashAlgo) -> Option<String> {
    let f = std::fs::File::open(p).ok()?;
    hash_reader(std::io::BufReader::new(f), algo).ok()
}

/// Hashes files on demand, each file and algorithm once (blocking).
#[derive(Default)]
struct Hasher {
    seen: HashMap<(PathBuf, u8), Option<String>>,
}

impl Hasher {
    fn of(&mut self, p: &Path, algo: HashAlgo) -> Option<String> {
        self.seen
            .entry((p.to_path_buf(), algo as u8))
            .or_insert_with(|| std::fs::File::open(p).ok().filter(|_| p.is_file()).and_then(|f| hash_reader(std::io::BufReader::new(f), algo).ok()))
            .clone()
    }
    fn matches(&mut self, p: &Path, h: &Hash) -> bool {
        self.of(p, h.algo).is_some_and(|v| v.eq_ignore_ascii_case(&h.value))
    }
    /// True when the file is still what the pack shipped (or the pack's hash is unknown).
    fn unchanged(&mut self, p: &Path, h: &Option<Hash>) -> bool {
        h.as_ref().map_or(true, |h| self.matches(p, h))
    }
    fn same(&mut self, a: &Path, b: &Path) -> bool {
        let x = self.of(a, HashAlgo::Sha1);
        x.is_some() && x == self.of(b, HashAlgo::Sha1)
    }
}

// ---------- Planning ----------

/// How a file of the new version is available while planning.
#[derive(Debug, Clone)]
pub enum NewEntry {
    /// Downloaded or unpacked to `<stage>/<path>`.
    Staged,
    /// Already on disk as the pack ships it; nothing to do.
    Current,
    /// Only downloadable from the author's website (SHA-1 when known).
    Pending(Option<Hash>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// `<stage>/<path>` goes to `<game>/<to>` (`to` is `path`, or `path.disabled` to keep a mod switched off).
    /// Whatever is at `to` goes to the backup; with `bak`, a copy of it is also left as `<to>.bak`.
    Place { path: String, to: String, bak: bool },
    /// `<game>/<path>` goes to the backup.
    Remove { path: String },
}

#[derive(Debug, Default)]
pub struct Plan {
    pub steps: Vec<Step>,
    /// Website-only files still to fetch, by path.
    pub pending: Vec<String>,
    /// Pack files the player changed that stay as they are.
    pub kept: Vec<String>,
}

fn disabled(p: &str) -> String {
    format!("{p}.disabled")
}

fn folder(p: &str) -> &str {
    p.rsplit_once('/').map(|(f, _)| f).unwrap_or("")
}

/// Decides, path by path, what switching from `old` (the installed version's files and their hashes; None when
/// unknown) to `new` does to the game directory:
/// - a file the new version didn't change stays as it is, with any change the player made to it;
/// - a file it changed is replaced; if the player had changed it too, their copy is kept as `<file>.bak`;
/// - a file it no longer has is removed, unless the player changed it;
/// - a mod the player disabled stays disabled, even when its new version has another file name;
/// - files that were never the pack's are the player's and stay, except where the new version puts a file at the
///   same path (theirs is kept as `.bak`);
/// - worlds, screenshots, options.txt and servers.dat are never changed once they exist.
///
/// `project_of` maps paths (old and new) to the project they belong to, where known. Blocking (hashes files).
pub fn plan(
    game: &Path,
    stage: &Path,
    old: Option<&BTreeMap<String, Option<Hash>>>,
    new: &BTreeMap<String, NewEntry>,
    project_of: &HashMap<String, String>,
    keep: &Keep,
) -> Plan {
    let empty = BTreeMap::new();
    let old = old.unwrap_or(&empty);
    let mut h = Hasher::default();
    let mut out = Plan::default();

    // Projects whose old file the player had disabled, by folder: their new file stays disabled too.
    let mut switched_off: HashSet<(String, String)> = HashSet::new();
    for p in old.keys() {
        if !game.join(p).is_file() && game.join(disabled(p)).is_file() {
            if let Some(project) = project_of.get(p) {
                switched_off.insert((folder(p).to_string(), project.clone()));
            }
        }
    }

    for (p, entry) in new {
        let cur = game.join(p);
        let off = game.join(disabled(p));
        match entry {
            NewEntry::Current => {}
            NewEntry::Pending(hash) => {
                if cur.is_file() && hash.as_ref().map_or(true, |x| h.matches(&cur, x)) {
                    continue;
                }
                // An outdated copy the pack put there makes way for the one the player will download.
                if cur.is_file() && old.get(p).is_some_and(|oh| h.unchanged(&cur, oh)) {
                    out.steps.push(Step::Remove { path: p.clone() });
                }
                out.pending.push(p.clone());
            }
            NewEntry::Staged => {
                if keep.protects(game, p) {
                    continue;
                }
                let staged = stage.join(p);
                if cur.is_file() && h.same(&cur, &staged) {
                    continue;
                }
                match old.get(p) {
                    Some(oh) => {
                        if oh.as_ref().is_some_and(|oh| h.matches(&staged, oh)) {
                            // The pack didn't change it: whatever the player did to it stands.
                            if cur.is_file() && !h.unchanged(&cur, oh) {
                                out.kept.push(p.clone());
                            }
                            continue;
                        }
                        if cur.is_file() {
                            let theirs = !h.unchanged(&cur, oh);
                            out.steps.push(Step::Place { path: p.clone(), to: p.clone(), bak: theirs });
                        } else if off.is_file() {
                            out.steps.push(Step::Place { path: p.clone(), to: disabled(p), bak: false });
                        } else {
                            out.steps.push(Step::Place { path: p.clone(), to: p.clone(), bak: false });
                        }
                    }
                    None => {
                        if cur.is_file() {
                            out.steps.push(Step::Place { path: p.clone(), to: p.clone(), bak: true });
                        } else {
                            let off = project_of.get(p).is_some_and(|pr| switched_off.contains(&(folder(p).to_string(), pr.clone())));
                            out.steps.push(Step::Place { path: p.clone(), to: if off { disabled(p) } else { p.clone() }, bak: false });
                        }
                    }
                }
            }
        }
    }

    for (p, oh) in old {
        if new.contains_key(p) || keep.protects(game, p) {
            continue;
        }
        let cur = game.join(p);
        let off = game.join(disabled(p));
        if cur.is_file() {
            if h.unchanged(&cur, oh) {
                out.steps.push(Step::Remove { path: p.clone() });
            } else {
                out.kept.push(p.clone());
            }
        } else if off.is_file() && h.unchanged(&off, oh) {
            out.steps.push(Step::Remove { path: disabled(p) });
        }
    }
    out
}

// ---------- Applying, with a journal to undo it ----------

const JOURNAL: &str = "journal.log";

/// `<instance>/.pack-switch`: the journal, the backup (`backup/game`, `backup/meta`), the stage and downloads.
pub fn work_dir(instance_dir: &Path) -> PathBuf {
    instance_dir.join(".pack-switch")
}

/// The instance's own files a switch changes; copied before applying and put back by `restore`.
pub const INSTANCE_META: [&str; 3] = ["instance.json", "content-index.json", "modpack-files.json"];
/// A hosted server keeps a copy of every file at its top (server.json, the server jar and start scripts the loader
/// installer may replace), and loses top-level files that weren't there before.
pub const ALL_TOP_FILES: [&str; 0] = [];
const LIST: &str = ".list";

/// The files `snapshot_meta` copies: `names`, or every file at the top of `root` when `names` is empty.
fn meta_names(root: &Path, names: &[&str]) -> Vec<String> {
    if !names.is_empty() {
        return names.iter().map(|n| n.to_string()).collect();
    }
    std::fs::read_dir(root)
        .map(|rd| rd.flatten().filter(|e| e.path().is_file()).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default()
}

fn log(j: &mut std::fs::File, kind: &str, rel: &str) -> Result<()> {
    writeln!(j, "{kind}\t{rel}")?;
    j.flush()?;
    Ok(())
}

fn move_file(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::rename(from, to).is_err() {
        std::fs::copy(from, to)?;
        std::fs::remove_file(from)?;
    }
    Ok(())
}

/// Creates the missing folders above `<game>/<rel>`, journaling each so `restore` can remove them again.
fn make_parents(game: &Path, rel: &str, j: &mut std::fs::File) -> Result<()> {
    let parts: Vec<&str> = rel.split('/').collect();
    for i in 1..parts.len() {
        let sub = parts[..i].join("/");
        let dir = safe_join(game, &sub)?;
        if !dir.exists() {
            log(j, "mkdir", &sub)?;
            std::fs::create_dir(&dir)?;
        }
    }
    Ok(())
}

/// Copies the instance's (or server's) own files a switch changes into the backup (blocking). `names` empty: every file
/// at the top of `root`, and `restore` also removes top-level files that appeared since.
pub fn snapshot_meta(root: &Path, work: &Path, names: &[&str]) -> Result<()> {
    let meta = work.join("backup").join("meta");
    std::fs::create_dir_all(&meta)?;
    let list = meta_names(root, names);
    for name in &list {
        let src = root.join(name);
        if src.is_file() {
            std::fs::copy(&src, meta.join(name))?;
        } else {
            std::fs::write(meta.join(format!("{name}.absent")), b"")?;
        }
    }
    if names.is_empty() {
        std::fs::write(meta.join(LIST), list.join("\n"))?;
    }
    Ok(())
}

/// Carries out `steps` (blocking). Each step is journaled before it happens; on an error the caller restores.
pub fn apply(game: &Path, stage: &Path, work: &Path, steps: &[Step]) -> Result<()> {
    let backup = work.join("backup").join("game");
    let mut j = std::fs::OpenOptions::new().create(true).append(true).open(work.join(JOURNAL))?;
    for step in steps {
        match step {
            Step::Place { path, to, bak } => {
                let dest = safe_join(game, to)?;
                let saved = safe_join(&backup, to)?;
                if dest.is_file() {
                    log(&mut j, "moved", to)?;
                    move_file(&dest, &saved)?;
                }
                if *bak && saved.is_file() {
                    let b = format!("{to}.bak");
                    let bp = safe_join(game, &b)?;
                    if bp.is_file() {
                        log(&mut j, "moved", &b)?;
                        move_file(&bp, &safe_join(&backup, &b)?)?;
                    }
                    log(&mut j, "added", &b)?;
                    std::fs::copy(&saved, &bp)?;
                }
                make_parents(game, to, &mut j)?;
                log(&mut j, "added", to)?;
                move_file(&safe_join(stage, path)?, &dest)?;
            }
            Step::Remove { path } => {
                let src = safe_join(game, path)?;
                if src.is_file() {
                    log(&mut j, "moved", path)?;
                    move_file(&src, &safe_join(&backup, path)?)?;
                }
            }
        }
    }
    Ok(())
}

/// Undoes an applied (or half-applied) switch from its journal and backup, and puts the instance's own files back
/// (blocking). Safe to run again if it's interrupted.
pub fn restore(root: &Path, game: &Path, work: &Path, names: &[&str]) -> Result<()> {
    let backup = work.join("backup").join("game");
    let text = std::fs::read_to_string(work.join(JOURNAL)).unwrap_or_default();
    for line in text.lines().rev() {
        let Some((kind, rel)) = line.split_once('\t') else { continue };
        let Ok(at) = safe_join(game, rel) else { continue };
        match kind {
            "added" => {
                if at.is_file() {
                    std::fs::remove_file(&at)?;
                }
            }
            "moved" => {
                let saved = safe_join(&backup, rel)?;
                if saved.is_file() {
                    if at.is_file() {
                        std::fs::remove_file(&at)?;
                    }
                    move_file(&saved, &at)?;
                }
            }
            "mkdir" => {
                // Only empty: anything else in it was there before or is the player's.
                let _ = std::fs::remove_dir(&at);
            }
            _ => {}
        }
    }
    let meta = work.join("backup").join("meta");
    if meta.is_dir() {
        let list: Vec<String> = if names.is_empty() {
            let all: Vec<String> = std::fs::read_to_string(meta.join(LIST)).unwrap_or_default().lines().filter(|l| !l.is_empty()).map(str::to_string).collect();
            // Top-level files that weren't there before (a loader installer's new jar or script) go.
            for now in meta_names(root, &[]) {
                if !all.contains(&now) {
                    let _ = std::fs::remove_file(root.join(&now));
                }
            }
            all
        } else {
            names.iter().map(|n| n.to_string()).collect()
        };
        for name in &list {
            let saved = meta.join(name);
            if saved.is_file() {
                std::fs::copy(&saved, root.join(name))?;
            } else if meta.join(format!("{name}.absent")).is_file() {
                let _ = std::fs::remove_file(root.join(name));
            }
        }
    }
    // The undo is complete: a later recovery must not run it again.
    let _ = std::fs::remove_file(work.join(JOURNAL));
    Ok(())
}

/// Where a switch happens: an instance (its folder, with the game directory inside) or a hosted server (one folder).
#[derive(Debug, Clone)]
pub struct Site {
    /// Marks the site busy while a switch runs ("server:<id>" for servers, so ids never clash).
    pub key: String,
    /// The task's instance or server id.
    pub owner: String,
    /// Holds `.pack-switch` and the files `meta` names.
    pub root: PathBuf,
    /// Where the pack's files go.
    pub game: PathBuf,
    /// The site's own files a switch may change (`ALL_TOP_FILES`: every file at the top of `root`).
    pub meta: &'static [&'static str],
}

impl Site {
    pub fn instance(id: &str) -> Site {
        Site { key: id.to_string(), owner: id.to_string(), root: paths::instance_dir(id), game: paths::game_dir(id), meta: &INSTANCE_META }
    }
    pub fn server(id: &str, dir: PathBuf) -> Site {
        Site { key: format!("server:{id}"), owner: id.to_string(), root: dir.clone(), game: dir, meta: &ALL_TOP_FILES }
    }
    pub fn work(&self) -> PathBuf {
        work_dir(&self.root)
    }
}

/// Finishes what an interrupted switch left behind: undoes it if it was applying, then clears its work folder.
pub async fn recover(site: &Site) -> Result<()> {
    let work = site.work();
    if !work.exists() {
        return Ok(());
    }
    if work.join(JOURNAL).exists() {
        let (root, game, w, meta) = (site.root.clone(), site.game.clone(), work.clone(), site.meta);
        tokio::task::spawn_blocking(move || restore(&root, &game, &w, meta)).await??;
    }
    let _ = tokio::fs::remove_dir_all(&work).await;
    Ok(())
}

type ApplyDone = Shared<futures::future::BoxFuture<'static, Result<(), Arc<anyhow::Error>>>>;

/// Applies a plan for a `guarded` switch: backs up the site's own files, then carries out the steps.
pub struct Applier {
    site: Site,
    slot: Arc<Mutex<Option<ApplyDone>>>,
}

impl Applier {
    /// Runs to the end even if Cancel drops the task: `guarded` waits for it, then undoes it.
    pub async fn apply(&self, stage: &Path, steps: Vec<Step>) -> Result<()> {
        let (root, game, work, meta, stage) = (self.site.root.clone(), self.site.game.clone(), self.site.work(), self.site.meta, stage.to_path_buf());
        let done: ApplyDone = tokio::task::spawn_blocking(move || -> Result<()> {
            snapshot_meta(&root, &work, meta)?;
            apply(&game, &stage, &work, &steps)
        })
        .map(|r| r.map_err(anyhow::Error::from).and_then(|r| r).map_err(Arc::new))
        .boxed()
        .shared();
        *self.slot.lock().unwrap() = Some(done.clone());
        done.await.map_err(|e| anyhow!("{e:#}"))
    }
}

/// Runs `work` as a cancellable task with the site marked busy. Whatever it applied stays only if it succeeds; a
/// failure, Cancel or crash undoes it (see `restore`). Leftovers of an interrupted earlier switch are undone first.
pub async fn guarded<T, F, Fut>(site: Site, title: crate::core::text::Text, busy: crate::core::text::Text, work: F) -> Result<T>
where
    F: FnOnce(Arc<Task>, PathBuf, Applier) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let Some(_busy) = Busy::claim(&site.key) else { return Err(anyhow::Error::from(busy)) };
    recover(&site).await?;
    let slot: Arc<Mutex<Option<ApplyDone>>> = Arc::new(Mutex::new(None));
    let applier = Applier { site: site.clone(), slot: slot.clone() };
    let dir = site.work();
    let res = Task::run(title, Some(site.owner.clone()), move |task| async move {
        tokio::fs::create_dir_all(&dir).await?;
        work(task, dir, applier).await
    })
    .await;
    match res {
        Ok(v) => {
            // Committed: nothing to undo any more.
            let _ = tokio::fs::remove_file(site.work().join(JOURNAL)).await;
            let _ = tokio::fs::remove_dir_all(site.work()).await;
            Ok(v)
        }
        Err(e) => {
            let pending = slot.lock().unwrap().take();
            if let Some(done) = pending {
                let _ = done.await;
            }
            recover(&site).await?;
            Err(e)
        }
    }
}

// ---------- The installed version's files ----------

/// What the pack installed, recorded at install time and after each switch: path → "algo:hash" ("" if unknown).
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Record {
    version_id: String,
    files: BTreeMap<String, String>,
}

pub const RECORD: &str = "modpack-files.json";

async fn write_record(root: &Path, version_id: &str, files: &BTreeMap<String, Option<Hash>>) -> Result<()> {
    let rec = Record { version_id: version_id.to_string(), files: files.iter().map(|(k, v)| (k.clone(), hash_text(v))).collect() };
    write_json(&root.join(RECORD), &rec).await
}

/// Every file under `game` with its SHA-1, leaving out what `keep` excludes (blocking).
pub fn hash_tree(game: &Path, keep: &Keep) -> BTreeMap<String, Option<Hash>> {
    let mut files = BTreeMap::new();
    walk(game, game, &mut |rel, p| {
        if keep.excluded(&rel) {
            return;
        }
        let sha1 = std::fs::File::open(p).ok().and_then(|f| hash_reader(std::io::BufReader::new(f), HashAlgo::Sha1).ok());
        files.insert(rel, sha1.map(Hash::sha1));
    });
    files
}

/// Records every file of a freshly installed modpack instance (all of them are the pack's), so a later version
/// switch knows them without asking the source. Best effort: without it, the switch reads the old manifest.
pub async fn record_install(inst: &Instance) {
    let Some(version_id) = inst.modpack.as_ref().and_then(|m| m.version_id.clone()) else { return };
    let game = paths::game_dir(&inst.id);
    let pending = inst.pending_downloads.clone().unwrap_or_default();
    let files = tokio::task::spawn_blocking(move || {
        let mut files = hash_tree(&game, &Keep::client());
        for p in pending {
            files.insert(pending_path(&p), p.sha1.map(Hash::sha1));
        }
        files
    })
    .await
    .unwrap_or_default();
    let _ = write_record(&paths::instance_dir(&inst.id), &version_id, &files).await;
}

/// The same for a server just filled from a modpack (before its server software is installed).
pub async fn record_server_install(dir: &Path, version_id: &str) {
    let d = dir.to_path_buf();
    let files = tokio::task::spawn_blocking(move || hash_tree(&d, &Keep::server(""))).await.unwrap_or_default();
    let _ = write_record(dir, version_id, &files).await;
}

fn walk(root: &Path, dir: &Path, f: &mut impl FnMut(String, &Path)) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(root, &p, f);
        } else if let Ok(rel) = p.strip_prefix(root) {
            f(norm(&rel.to_string_lossy()), &p);
        }
    }
}

/// Hashes (SHA-1) the entries each layer contributes, later layers winning (blocking).
fn layer_hashes(layers: &[Layer]) -> Result<BTreeMap<String, Option<Hash>>> {
    let mut out = BTreeMap::new();
    for l in layers {
        let mut zip = open_zip(&l.zip)?;
        for i in 0..zip.len() {
            let entry = zip.by_index(i)?;
            if entry.is_dir() {
                continue;
            }
            let Some(rel) = l.map(entry.name()) else { continue };
            out.insert(rel, Some(Hash::sha1(hash_reader(entry, HashAlgo::Sha1)?)));
        }
    }
    Ok(out)
}

/// The installed version's files and hashes: the record in `root` if it's for that version, else its manifest.
/// None when neither is available (the version was deleted, or the source can't be reached).
pub(crate) async fn installed_files(
    root: &Path,
    version_id: Option<&str>,
    project_id: &str,
    work: &Path,
    task: &Arc<Task>,
    side: Side,
    keep: &Keep,
) -> Option<BTreeMap<String, Option<Hash>>> {
    let vid = version_id?.to_string();
    if let Some(rec) = read_json::<Record>(&root.join(RECORD)).await.filter(|r| r.version_id == vid) {
        return Some(rec.files.into_iter().map(|(k, v)| (k, parse_hash(&v))).collect());
    }
    task.update(tr!("packswitch.readingInstalled", "Reading the installed version…"), None);
    let t = target(project_id, &vid, work, task, side).await.ok()?;
    let mut files: BTreeMap<String, Option<Hash>> = t.files.iter().map(|f| (norm(&f.path), f.hash.clone())).collect();
    for p in &t.pending {
        files.entry(pending_path(p)).or_insert(p.sha1.clone().map(Hash::sha1));
    }
    let layers = t.layers.clone();
    files.extend(tokio::task::spawn_blocking(move || layer_hashes(&layers)).await.ok()?.ok()?);
    files.retain(|p, _| !keep.excluded(p));
    Some(files)
}


// ---------- Staging the new version ----------

enum Src {
    File(usize),
    Layer(usize, String),
    Pending(usize),
}

pub(crate) struct Staged {
    pub entries: BTreeMap<String, NewEntry>,
    /// The new version's files and hashes, for the record.
    pub hashes: BTreeMap<String, Option<Hash>>,
}

/// Downloads and unpacks the new version's files that aren't on disk as the pack ships them into `stage`.
/// Paths `keep` excludes (on a server: client-only folders, the server software) are left out.
async fn stage_new(t: &PackTarget, game: &Path, stage: &Path, task: &Arc<Task>, keep: &Keep) -> Result<Staged> {
    let mut src: BTreeMap<String, Src> = BTreeMap::new();
    for (i, f) in t.files.iter().enumerate() {
        src.insert(norm(&f.path), Src::File(i));
    }
    let layers = t.layers.clone();
    let listed = tokio::task::spawn_blocking(move || -> Result<Vec<(String, usize, String)>> {
        let mut out = Vec::new();
        for (li, l) in layers.iter().enumerate() {
            let zip = open_zip(&l.zip)?;
            for name in zip.file_names() {
                if let Some(rel) = l.map(name) {
                    out.push((rel, li, name.to_string()));
                }
            }
        }
        Ok(out)
    })
    .await??;
    for (rel, li, name) in listed {
        src.insert(rel, Src::Layer(li, name));
    }
    for (i, p) in t.pending.iter().enumerate() {
        src.entry(pending_path(p)).or_insert(Src::Pending(i));
    }
    src.retain(|p, _| !keep.excluded(p));

    let mut entries = BTreeMap::new();
    let mut hashes = BTreeMap::new();
    let mut items = Vec::new();
    let mut unpack: Vec<(usize, String, PathBuf)> = Vec::new();
    {
        let mut h = Hasher::default();
        for (rel, s) in &src {
            match s {
                Src::File(i) => {
                    let f = &t.files[*i];
                    let cur = safe_join(game, rel)?;
                    if f.hash.as_ref().is_some_and(|x| h.matches(&cur, x)) {
                        entries.insert(rel.clone(), NewEntry::Current);
                        hashes.insert(rel.clone(), f.hash.clone());
                    } else {
                        items.push(DownloadItem::new(&f.url, safe_join(stage, rel)?).hash(f.hash.clone()).size(f.size));
                        entries.insert(rel.clone(), NewEntry::Staged);
                    }
                }
                Src::Layer(li, name) => {
                    unpack.push((*li, name.clone(), safe_join(stage, rel)?));
                    entries.insert(rel.clone(), NewEntry::Staged);
                }
                Src::Pending(i) => {
                    let hash = t.pending[*i].sha1.clone().map(Hash::sha1);
                    hashes.insert(rel.clone(), hash.clone());
                    entries.insert(rel.clone(), NewEntry::Pending(hash));
                }
            }
        }
    }
    download_all(items, 8, task.counter(tr!("modpack.downloadingFiles", "Downloading modpack files"))).await?;

    task.update(tr!("packswitch.unpacking", "Unpacking the new version…"), None);
    let layers = t.layers.clone();
    let stage2 = stage.to_path_buf();
    let staged: Vec<String> = entries.iter().filter(|(_, e)| matches!(e, NewEntry::Staged)).map(|(k, _)| k.clone()).collect();
    let fresh = tokio::task::spawn_blocking(move || -> Result<BTreeMap<String, Option<Hash>>> {
        let mut by_zip: BTreeMap<usize, Vec<(String, PathBuf)>> = BTreeMap::new();
        for (li, name, dest) in unpack {
            by_zip.entry(li).or_default().push((name, dest));
        }
        for (li, list) in by_zip {
            let mut zip = open_zip(&layers[li].zip)?;
            for (name, dest) in list {
                let mut entry = zip.by_name(&name)?;
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::io::copy(&mut entry, &mut std::fs::File::create(&dest)?)?;
            }
        }
        let mut out = BTreeMap::new();
        for rel in staged {
            let f = std::fs::File::open(stage2.join(&rel))?;
            out.insert(rel, Some(Hash::sha1(hash_reader(std::io::BufReader::new(f), HashAlgo::Sha1)?)));
        }
        Ok(out)
    })
    .await??;
    hashes.extend(fresh);
    Ok(Staged { entries, hashes })
}

// ---------- The switch ----------

/// What a switch did, for the UI.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PackSwitch {
    pub version_name: String,
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    /// Files the player had changed that stay as they are.
    pub kept: Vec<String>,
    /// Copies of the player's changed files that the new version replaced.
    pub backups: Vec<String>,
    /// Website-only files to download (Download all on the instance page).
    pub pending: usize,
    /// A server's website-only files (servers have no Download all; the UI lists them).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pending_files: Vec<PendingDownload>,
    /// Client-only mods left off a server, by name.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dropped: Vec<String>,
}

static SWITCHING: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Marks an instance as switching while it lives.
struct Busy(String);

impl Busy {
    fn claim(id: &str) -> Option<Busy> {
        SWITCHING.lock().unwrap().get_or_insert_with(HashSet::new).insert(id.to_string()).then(|| Busy(id.to_string()))
    }
}

impl Drop for Busy {
    fn drop(&mut self) {
        if let Some(s) = SWITCHING.lock().unwrap().as_mut() {
            s.remove(&self.0);
        }
    }
}

pub fn is_switching(id: &str) -> bool {
    SWITCHING.lock().unwrap().as_ref().is_some_and(|s| s.contains(id))
}

/// The index key of a content file ("mods/x.jar" → "mod/x.jar"); None outside the content folders.
fn content_key(path: &str) -> Option<String> {
    let (dir, name) = path.split_once('/')?;
    if name.contains('/') {
        return None;
    }
    [ContentKind::Mod, ContentKind::Resourcepack, ContentKind::Shader].into_iter().find(|k| k.folder() == dir).map(|k| index_key(k, name))
}

/// What `switch_files` found and did.
pub(crate) struct Switched {
    pub target: PackTarget,
    /// The loader version to install (pinned by the pack, else the newest for its Minecraft version).
    pub loader_version: Option<String>,
    pub plan: Plan,
    pub staged: Staged,
    pub old: Option<BTreeMap<String, Option<Hash>>>,
    /// Client-only mods left off a server, by name.
    pub dropped: Vec<String>,
}

impl Switched {
    /// The new version's website-only files still to fetch.
    pub fn pending(&self) -> Vec<PendingDownload> {
        self.target.pending.iter().filter(|p| self.plan.pending.contains(&pending_path(p))).cloned().collect()
    }

    pub fn summary(&self) -> PackSwitch {
        let mut out =
            PackSwitch { version_name: self.target.version_name.clone(), kept: self.plan.kept.clone(), pending: self.pending().len(), ..Default::default() };
        for step in &self.plan.steps {
            match step {
                Step::Place { to, bak, .. } => {
                    if *bak {
                        out.backups.push(format!("{to}.bak"));
                    }
                    if *bak || self.old.as_ref().is_some_and(|o| o.contains_key(to.strip_suffix(".disabled").unwrap_or(to))) {
                        out.updated += 1;
                    } else {
                        out.added += 1;
                    }
                }
                Step::Remove { .. } => out.removed += 1,
            }
        }
        out
    }

    /// Mod files of the new version (for the duplicate check after an instance switch).
    pub fn listed_mods(&self) -> Vec<String> {
        self.staged.entries.keys().filter_map(|p| p.strip_prefix("mods/")).filter(|n| !n.contains('/')).map(str::to_string).collect()
    }
}

/// The part of a switch shared by instances and servers, inside `guarded`: fetches `version_id`, works out what the
/// installed version (`installed`) put where, stages and plans the difference and applies it. `idx` (an instance's
/// content index) tells which project each mod belongs to.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn switch_files(
    task: &Arc<Task>,
    work: &Path,
    applier: &Applier,
    project_id: &str,
    installed: Option<&str>,
    version_id: &str,
    pack_name: &str,
    side: Side,
    keep: &Keep,
    idx: Option<&super::index::ContentIndex>,
) -> Result<Switched> {
    let (root, game) = (applier.site.root.clone(), applier.site.game.clone());
    let stage = work.join("stage");
    tokio::fs::create_dir_all(&stage).await?;

    task.update(tr!("packswitch.reading", "Getting the new version's details…"), None);
    let unreachable = |e: anyhow::Error| {
        err_tr!(
            "packswitch.unreachable",
            "Couldn't get that version of {name} from its source, so the instance was left as it is. {error}",
            name = pack_name.to_string(),
            error = format!("{e:#}")
        )
    };
    let target_ = target(project_id, version_id, &work.join("src-new"), task, side).await.map_err(unreachable)?;
    let loader_version = match (target_.loader, target_.loader_version.clone()) {
        (LoaderType::Vanilla, _) => None,
        (l, Some(v)) if !v.is_empty() => Some(crate::game::loaders::normalize_loader_version(l, &target_.mc_version, &v)),
        (l, _) => Some(crate::game::loaders::latest_loader(l, &target_.mc_version).await.map_err(unreachable)?),
    };
    let old = installed_files(&root, installed, project_id, &work.join("src-old"), task, side, keep).await;
    let mut staged = stage_new(&target_, &game, &stage, task, keep).await?;

    // A server leaves out the mods Modrinth says don't run on servers (as when it was created).
    let mut dropped = Vec::new();
    if side == Side::Server {
        let mods: Vec<(String, String)> = staged
            .hashes
            .iter()
            .filter(|(p, _)| p.starts_with("mods/") && p.to_lowercase().ends_with(".jar"))
            .filter_map(|(p, h)| h.as_ref().filter(|h| h.algo == HashAlgo::Sha1).map(|h| (p.clone(), h.value.clone())))
            .collect();
        if let Ok(found) = crate::server::create::client_only_sha1s(&mods.iter().map(|m| m.1.clone()).collect::<Vec<_>>()).await {
            for (p, sha1) in mods {
                if let Some(title) = found.get(&sha1) {
                    staged.entries.remove(&p);
                    staged.hashes.remove(&p);
                    dropped.push(title.clone());
                }
            }
        }
    }

    // Which project each mod belongs to, so a mod the player switched off stays off under its new file name.
    let mut project_of: HashMap<String, String> = HashMap::new();
    if let Some(idx) = idx {
        for p in old.iter().flat_map(|o| o.keys()).chain(staged.entries.keys()) {
            if let Some(e) = content_key(p).and_then(|k| idx.get(&k)).filter(|e| !e.project_id.is_empty()) {
                project_of.insert(p.clone(), e.project_id.clone());
            }
        }
    }
    for (p, e) in &target_.index {
        project_of.insert(norm(p), e.project_id.clone());
    }
    if source_of(project_id) == ContentSource::Modrinth {
        let sha1s: Vec<(String, String)> = staged
            .hashes
            .iter()
            .filter(|(p, _)| p.starts_with("mods/") && !project_of.contains_key(*p))
            .filter_map(|(p, h)| h.as_ref().filter(|h| h.algo == HashAlgo::Sha1).map(|h| (p.clone(), h.value.clone())))
            .collect();
        if let Ok(found) = modrinth::projects_by_sha1(&sha1s.iter().map(|x| x.1.clone()).collect::<Vec<_>>()).await {
            for (p, sha1) in sha1s {
                if let Some(pr) = found.get(&sha1) {
                    project_of.insert(p, pr.clone());
                }
            }
        }
    }

    task.update(tr!("packswitch.comparing", "Comparing with your files…"), None);
    let (g, s, o, n, k) = (game.clone(), stage.clone(), old.clone(), staged.entries.clone(), keep.clone());
    let plan = tokio::task::spawn_blocking(move || plan(&g, &s, o.as_ref(), &n, &project_of, &k)).await?;

    task.update(tr!("packswitch.applying", "Backing up and applying the changes…"), None);
    applier.apply(&stage, plan.steps.clone()).await?;
    write_record(&root, &target_.version_id, &staged.hashes).await?;
    Ok(Switched { target: target_, loader_version, plan, staged, old, dropped })
}

/// Switches modpack instance `instance_id` to `version_id` of its pack (see the module docs). A cancellable task;
/// any failure or Cancel leaves the instance exactly as it was.
pub async fn switch_version(instance_id: &str, version_id: &str) -> Result<PackSwitch> {
    let inst = instances::get(instance_id).await?;
    let Some(pack) = inst.modpack.clone() else {
        bail_tr!("packswitch.notModpack", "{name} wasn't installed from a modpack", name = inst.name)
    };
    let Some(project_id) = pack.project_id.clone().filter(|p| !p.is_empty() && source_of(p) != ContentSource::Github) else {
        bail_tr!("packswitch.noSource", "ResinCraft doesn't know where {name} came from, so it can't switch its version", name = pack.name)
    };
    if pack.version_id.as_deref() == Some(version_id) {
        return Ok(PackSwitch::default());
    }
    if crate::game::running_ids().iter().any(|r| r == instance_id) {
        bail_tr!("packswitch.running", "Close {name} before switching its modpack version", name = inst.name);
    }

    let (id, vid) = (instance_id.to_string(), version_id.to_string());
    let title = tr!("packswitch.title", "Switching {name} to another version", name = pack.name.clone());
    let busy = tr!("packswitch.busy", "{name} is already switching versions", name = inst.name.clone());
    let (out, listed) = guarded(Site::instance(instance_id), title, busy, move |task, work, applier| async move {
        let idx = load_index(&id).await;
        let sw = switch_files(&task, &work, &applier, &project_id, pack.version_id.as_deref(), &vid, &pack.name, Side::Client, &Keep::client(), Some(&idx))
            .await?;

        // The content index: drop entries of files that changed or went, add what the source knows.
        let mut idx = load_index(&id).await;
        for step in &sw.plan.steps {
            let (Step::Place { to: p, .. } | Step::Remove { path: p }) = step;
            if let Some(k) = content_key(p) {
                idx.remove(&k);
            }
        }
        for (p, e) in &sw.target.index {
            if let Some(k) = content_key(&norm(p)) {
                idx.insert(k, e.clone());
            }
        }
        save_index(&id, &idx).await?;

        let pending = sw.pending();
        let mut modpack = pack.clone();
        modpack.version_id = Some(sw.target.version_id.clone());
        instances::update(
            &id,
            json!({
                "mcVersion": sw.target.mc_version,
                "loader": sw.target.loader,
                "loaderVersion": sw.loader_version,
                "modpack": modpack,
                "pendingDownloads": if pending.is_empty() { None } else { Some(&pending) },
            }),
        )
        .await?;
        Ok((sw.summary(), sw.listed_mods()))
    })
    .await?;

    // A jar in the new overrides may be another copy of a listed mod, as at install.
    let inst = instances::get(instance_id).await?;
    let task = Task::new(tr!("dupes.checkingPack", "Checking for mods the pack has twice…"), Some(inst.id.clone()));
    dedupe_pack(&inst, &task, &listed).await;
    task.done();
    Ok(out)
}


#[cfg(test)]
#[path = "packswitch_tests.rs"]
mod tests;
