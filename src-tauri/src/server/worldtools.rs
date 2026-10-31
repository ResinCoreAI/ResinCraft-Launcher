//! World tools for Java servers (Settings → World tools, `server:world*` and `server:chunk*`): several worlds in one
//! server, chunks reset so they generate again, and a small player data editor. Everything that writes refuses while
//! the server runs, and a backup comes first unless the host turns that off.
//!
//! On this PC only: these calls are not on remote.rs's list, so the web console, other PCs and the panel's website
//! never reach them.
//!
//! Where a world's files are depends on the game and the server:
//!   - Minecraft 26.1 on: `<world>/dimensions/<namespace>/<path>` and `<world>/players/data`.
//!   - Before that, vanilla and mods: `<world>` for the overworld, `<world>/DIM-1`, `<world>/DIM1`, `<world>/playerdata`.
//!   - Before that, Paper and Spigot: `<world>_nether/DIM-1` and `<world>_the_end/DIM1` as well.
//!
//! An Anvil region (`r.X.Z.mca`) holds 32×32 chunks. Sector 0 holds where each chunk is (3 bytes of sector offset,
//! 1 of length), sector 1 holds when it was saved, and the chunks follow. Removing a chunk is zeroing its 8 header
//! bytes: nothing moves and no data sector is touched, so its neighbours can't be damaged. The game reuses the space.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, bail, Context, Result};
use flate2::read::{MultiGzDecoder, ZlibDecoder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::core::router::json;
use crate::core::types::{ServerKind, ServerMeta};
use crate::core::Args;
use crate::player::files::write_atomic;
use crate::player::nbt::{self, compound, num, str, Compression, NbtFile, Tag};

use super::{properties, runtime, schedule, store};

const SECTOR: usize = 4096;
const HEADER: usize = SECTOR * 2;
/// The game's own limits: chunks outside the world border's maximum can't exist.
const MAX_BLOCK: i32 = 30_000_000;
/// What the level.dat of an Anvil world says in `version` (19132 is the older McRegion).
const ANVIL: i64 = 19133;

// ---------- One tool at a time, and no start while one works ----------

static BUSY: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

struct Claim(String);

impl Drop for Claim {
    fn drop(&mut self) {
        BUSY.lock().unwrap().remove(&self.0);
    }
}

fn claim(id: &str) -> Result<Claim> {
    if !BUSY.lock().unwrap().insert(id.to_string()) {
        bail!("Another world tool is still working on this server. Try again when it's done.");
    }
    Ok(Claim(id.to_string()))
}

/// For whatever starts the server or replaces its world while a tool may be writing.
pub fn check_free(id: &str) -> Result<()> {
    if BUSY.lock().unwrap().contains(id) {
        bail!("A world tool is working on this server. Try again when it's done.");
    }
    Ok(())
}

/// Why this software can't use the tools: their worlds aren't files these tools can read safely.
fn unsupported(kind: ServerKind) -> Option<&'static str> {
    if kind.bedrock() {
        return Some("World tools work on Java worlds. Bedrock worlds use a different format.");
    }
    if kind.proxy() {
        return Some("A proxy has no world of its own.");
    }
    match kind {
        ServerKind::Glowstone | ServerKind::Cuberite | ServerKind::Pumpkin | ServerKind::Horizon => Some("This server software keeps its worlds its own way, so world tools can't be used on them safely."),
        _ => None,
    }
}

/// The checks every tool that writes starts with. The claim is taken first, so a start can't slip in behind them.
async fn begin(id: &str) -> Result<(Claim, ServerMeta, PathBuf)> {
    let claim = claim(id)?;
    let meta = store::get(id).await?;
    if let Some(why) = unsupported(meta.kind) {
        bail!("{why}");
    }
    if runtime::is_running(id) {
        bail!("Stop {} first: world tools only work while it's off.", meta.name);
    }
    if schedule::busy(id) {
        bail!("A scheduled task is running on this server. Try again when it's done.");
    }
    if crate::content::packswitch::is_switching(&format!("server:{id}")) {
        crate::bail_tr!("packswitch.wait", "Wait for the modpack version switch to finish first");
    }
    let dir = store::dir(id)?;
    Ok((claim, meta, dir))
}

async fn active_level(dir: &Path) -> String {
    properties::get_value(dir, "level-name").await.map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).unwrap_or_else(|| "world".into())
}

// ---------- Worlds ----------

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Dimension {
    /// Relative to the server folder; the id of this dimension in requests.
    pub path: String,
    /// "minecraft:the_nether".
    pub name: String,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct World {
    pub name: String,
    pub active: bool,
    /// False for the world `level-name` names before the server made it.
    pub generated: bool,
    pub size: u64,
    /// The world's folders: itself and any `_nether` and `_the_end` beside it.
    pub folders: Vec<String>,
    pub dimensions: Vec<Dimension>,
    pub players: usize,
    pub data_version: Option<i64>,
    /// Why the tools refuse this world.
    pub problem: Option<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WorldTools {
    /// Set when this server can't use the tools at all.
    pub unsupported: Option<String>,
    pub active: String,
    pub worlds: Vec<World>,
}

fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else { return 0 };
    entries
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

/// Reads a world's level.dat and says why it can't be touched, if so. The version number when it can.
fn check_level(world: &Path) -> Result<Option<i64>> {
    if world.join("db").is_dir() || world.join("levelname.txt").is_file() {
        bail!("This is a Bedrock world.");
    }
    let data = std::fs::read(world.join("level.dat")).context("It has no level.dat.")?;
    let level = nbt::parse(&data).map_err(|_| anyhow!("Its level.dat can't be read."))?;
    let Some(info) = compound(level.root.get("Data")) else { bail!("Its level.dat can't be read.") };
    if let Some(version) = nbt::long(info.get("version")) {
        if version != ANVIL {
            bail!("It's saved in an older format than Anvil. Open it once in Minecraft 1.2 or newer first.");
        }
    }
    Ok(nbt::long(info.get("DataVersion")))
}

fn rel(parts: &[&str]) -> String {
    parts.join("/")
}

/// Custom dimensions: every folder under `<root>` holding a `region`, named after its path.
fn custom_dimensions(root: &str, dir: &Path, skip: &[&str], out: &mut Vec<Dimension>) {
    let Ok(namespaces) = std::fs::read_dir(dir.join(root)) else { return };
    for ns in namespaces.flatten().filter(|e| e.path().is_dir()) {
        let ns_name = ns.file_name().to_string_lossy().into_owned();
        let mut stack = vec![(ns.path(), String::new())];
        while let Some((folder, sub)) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&folder) else { continue };
            for e in entries.flatten().filter(|e| e.path().is_dir()) {
                let name = e.file_name().to_string_lossy().into_owned();
                let path = if sub.is_empty() { name } else { format!("{sub}/{name}") };
                if e.path().join("region").is_dir() {
                    let full = format!("{ns_name}:{path}");
                    if !skip.contains(&full.as_str()) {
                        out.push(Dimension { path: format!("{root}/{ns_name}/{path}"), name: full });
                    }
                } else if path.matches('/').count() < 4 {
                    stack.push((e.path(), path));
                }
            }
        }
    }
}

/// Every place this world keeps chunks. When a dimension is in two places (a world moved from vanilla to Paper, or
/// from before 26.1), both are listed with their paths, the one this software reads first.
pub fn find_dimensions(dir: &Path, world: &str, bukkit: bool) -> Vec<Dimension> {
    const OVERWORLD: &str = "minecraft:overworld";
    const NETHER: &str = "minecraft:the_nether";
    const END: &str = "minecraft:the_end";
    let new_dim = |folder: &str, name: &str| rel(&[folder, "dimensions/minecraft", name]);
    let (nether, end) = (format!("{world}_nether"), format!("{world}_the_end"));
    let mut places: Vec<(String, &str)> = vec![(new_dim(world, "overworld"), OVERWORLD), (world.to_string(), OVERWORLD), (new_dim(world, "the_nether"), NETHER), (new_dim(&nether, "the_nether"), NETHER)];
    let (split_nether, plain_nether) = (rel(&[&nether, "DIM-1"]), rel(&[world, "DIM-1"]));
    if bukkit {
        places.extend([(split_nether, NETHER), (plain_nether, NETHER), (nether.clone(), NETHER)]);
    } else {
        places.extend([(plain_nether, NETHER), (split_nether, NETHER), (nether.clone(), NETHER)]);
    }
    places.extend([(new_dim(world, "the_end"), END), (new_dim(&end, "the_end"), END)]);
    let (split_end, plain_end) = (rel(&[&end, "DIM1"]), rel(&[world, "DIM1"]));
    if bukkit {
        places.extend([(split_end, END), (plain_end, END), (end.clone(), END)]);
    } else {
        places.extend([(plain_end, END), (split_end, END), (end.clone(), END)]);
    }
    let mut out: Vec<Dimension> = places.into_iter().filter(|(p, _)| dir.join(p).join("region").is_dir()).map(|(path, name)| Dimension { path, name: name.to_string() }).collect();
    custom_dimensions(&rel(&[world, "dimensions"]), dir, &[OVERWORLD, NETHER, END], &mut out);
    // Forge's numbered dimensions, from before dimensions had names.
    if let Ok(entries) = std::fs::read_dir(dir.join(world)) {
        let mut numbered: Vec<Dimension> = entries
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let n: i32 = name.strip_prefix("DIM")?.parse().ok()?;
                (n != -1 && n != 1 && e.path().join("region").is_dir()).then(|| Dimension { path: rel(&[world, &name]), name: format!("dimension {n}") })
            })
            .collect();
        numbered.sort_by(|a, b| a.path.cmp(&b.path));
        out.extend(numbered);
    }
    out
}

/// Where this world keeps its players: `players/data` from 26.1, `playerdata` before.
fn player_dir(world: &Path) -> Option<PathBuf> {
    [world.join("players").join("data"), world.join("playerdata")].into_iter().find(|p| p.is_dir())
}

fn player_files(folder: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder).map(|d| d.flatten().map(|e| e.path()).filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "dat")).collect()).unwrap_or_default();
    files.sort();
    files
}

/// A folder is a world if it holds a level.dat. `x_nether` and `x_the_end` belong to world `x` when that exists.
pub fn scan_worlds(dir: &Path, active: &str, bukkit: bool) -> Vec<World> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|d| {
            d.flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| !n.starts_with('.') && dir.join(n).join("level.dat").is_file())
                .collect()
        })
        .unwrap_or_default();
    let all = names.clone();
    names.retain(|n| !["_nether", "_the_end"].iter().any(|s| n.strip_suffix(s).is_some_and(|base| all.iter().any(|a| a == base))));
    names.sort();
    let mut worlds: Vec<World> = names
        .iter()
        .map(|name| {
            let folder = dir.join(name);
            let folders: Vec<String> = [name.clone(), format!("{name}_nether"), format!("{name}_the_end")].into_iter().filter(|f| dir.join(f).is_dir()).collect();
            let (data_version, problem) = match check_level(&folder) {
                Ok(v) => (v, None),
                Err(e) => (None, Some(e.to_string())),
            };
            World {
                name: name.clone(),
                active: name == active,
                generated: true,
                size: folders.iter().map(|f| dir_size(&dir.join(f))).sum(),
                dimensions: if problem.is_none() { find_dimensions(dir, name, bukkit) } else { Vec::new() },
                players: player_dir(&folder).map_or(0, |p| player_files(&p).len()),
                folders,
                data_version,
                problem,
            }
        })
        .collect();
    if !worlds.iter().any(|w| w.active) {
        worlds.insert(0, World { name: active.to_string(), active: true, generated: false, size: 0, folders: Vec::new(), dimensions: Vec::new(), players: 0, data_version: None, problem: None });
    }
    worlds
}

pub async fn info(id: &str) -> Result<WorldTools> {
    let meta = store::get(id).await?;
    if let Some(why) = unsupported(meta.kind) {
        return Ok(WorldTools { unsupported: Some(why.to_string()), active: String::new(), worlds: Vec::new() });
    }
    let dir = store::dir(id)?;
    let active = active_level(&dir).await;
    let (a, bukkit) = (active.clone(), meta.kind.bukkit());
    let worlds = tokio::task::spawn_blocking(move || scan_worlds(&dir, &a, bukkit)).await?;
    Ok(WorldTools { unsupported: None, active, worlds })
}

/// A name for a new world, or one to write in `level-name`: plain, so it means the same in server.properties and on
/// every system, and never one of the server's own folders.
pub fn check_world_name(name: &str) -> Result<()> {
    let plain = !name.is_empty() && name.len() <= 64 && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-'));
    if !plain || name.starts_with([' ', '-']) || name.ends_with(' ') {
        bail!("Use letters, numbers, spaces, - and _ in a world's name (64 at most).");
    }
    let low = name.to_ascii_lowercase();
    const TAKEN: &[&str] = &["backups", "cache", "config", "configs", "defaultconfigs", "kubejs", "libraries", "logs", "mods", "plugins", "versions", "worlds", "con", "prn", "aux", "nul"];
    let device = ["com", "lpt"].iter().any(|d| low.strip_prefix(d).is_some_and(|n| n.len() == 1 && n.as_bytes()[0].is_ascii_digit()));
    if TAKEN.contains(&low.as_str()) || device {
        bail!("{name} can't be a world's name: the server needs that name for something else.");
    }
    if low.ends_with("_nether") || low.ends_with("_the_end") {
        bail!("Names ending in _nether or _the_end are used by a world's other dimensions.");
    }
    Ok(())
}

async fn world_of(dir: &Path, name: &str, bukkit: bool) -> Result<World> {
    crate::content::index::safe_name(name)?;
    let (d, n) = (dir.to_path_buf(), active_level(dir).await);
    let worlds = tokio::task::spawn_blocking(move || scan_worlds(&d, &n, bukkit)).await?;
    worlds.into_iter().find(|w| w.name == name && w.generated).ok_or_else(|| anyhow!("There's no world called {name} on this server."))
}

fn usable(world: World) -> Result<World> {
    match &world.problem {
        Some(why) => bail!("{}: {why}", world.name),
        None => Ok(world),
    }
}

/// The world's folders into a zip in the server's backups, before anything changes. Its path for the result.
async fn backup_first(id: &str, level: &str, wanted: bool) -> Result<Option<String>> {
    if !wanted {
        return Ok(None);
    }
    Ok(Some(runtime::backup_level(id, level, "").await?))
}

#[derive(Deserialize)]
pub struct SwitchWorld {
    name: String,
    backup: bool,
}

pub async fn switch_world(id: &str, req: SwitchWorld) -> Result<Option<String>> {
    let (_claim, meta, dir) = begin(id).await?;
    check_world_name(&req.name)?;
    let world = usable(world_of(&dir, &req.name, meta.kind.bukkit()).await?)?;
    let active = active_level(&dir).await;
    if world.name == active {
        return Ok(None);
    }
    let saved = if dir.join(&active).is_dir() { backup_first(id, &active, req.backup).await? } else { None };
    properties::set_values(&dir, &[("level-name", req.name)]).await?;
    Ok(saved)
}

#[derive(Deserialize)]
pub struct CreateWorld {
    name: String,
    /// "" picks a random one.
    seed: String,
    backup: bool,
}

/// The new world is made by the server on its next start: `level-name` points at a folder that isn't there yet.
pub async fn create_world(id: &str, req: CreateWorld) -> Result<Option<String>> {
    let (_claim, _meta, dir) = begin(id).await?;
    check_world_name(&req.name)?;
    let seed = req.seed.trim().to_string();
    if seed.len() > 64 || seed.contains(['\n', '\r']) {
        bail!("That seed is too long.");
    }
    let exists = |n: &str| std::fs::read_dir(&dir).map(|d| d.flatten().any(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(n))).unwrap_or(false);
    if [req.name.clone(), format!("{}_nether", req.name), format!("{}_the_end", req.name)].iter().any(|n| exists(n)) {
        bail!("This server already has a folder called {}.", req.name);
    }
    let active = active_level(&dir).await;
    let saved = if dir.join(&active).is_dir() { backup_first(id, &active, req.backup).await? } else { None };
    properties::set_values(&dir, &[("level-name", req.name), ("level-seed", seed)]).await?;
    Ok(saved)
}

#[derive(Deserialize)]
pub struct DeleteWorld {
    name: String,
    /// The world's name again: the request says what it means, whatever the page did.
    confirm: String,
    backup: bool,
}

/// To the Recycle Bin, like Regenerate world. Not the world in use: that's what Regenerate world is for.
pub async fn delete_world(id: &str, req: DeleteWorld) -> Result<Option<String>> {
    let (_claim, meta, dir) = begin(id).await?;
    if req.confirm != req.name {
        bail!("Type the world's name to delete it.");
    }
    let world = world_of(&dir, &req.name, meta.kind.bukkit()).await?;
    if world.name == active_level(&dir).await {
        bail!("{} is the world in use. Switch to another first, or use Regenerate world.", world.name);
    }
    let saved = backup_first(id, &world.name, req.backup).await?;
    for f in &world.folders {
        crate::core::shell::trash(&dir.join(f)).await?;
    }
    Ok(saved)
}

// ---------- Chunks ----------

/// An area on the map, as the host typed it: corners in any order, in blocks or in chunks.
#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct Area {
    pub x1: i32,
    pub z1: i32,
    pub x2: i32,
    pub z2: i32,
    /// Blocks, or chunks when false.
    pub blocks: bool,
}

/// Chunks from (x1, z1) to (x2, z2), both included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x1: i32,
    pub z1: i32,
    pub x2: i32,
    pub z2: i32,
}

impl Rect {
    pub fn of(a: Area) -> Result<Rect> {
        let limit = if a.blocks { MAX_BLOCK } else { MAX_BLOCK >> 4 };
        if [a.x1, a.z1, a.x2, a.z2].iter().any(|v| v.unsigned_abs() > limit as u32) {
            bail!("That's outside the world: coordinates stop at {limit}.");
        }
        // Blocks to chunks rounds down: block -1 is in chunk -1, not 0.
        let at = |v: i32| if a.blocks { v >> 4 } else { v };
        Ok(Rect { x1: at(a.x1.min(a.x2)), z1: at(a.z1.min(a.z2)), x2: at(a.x1.max(a.x2)), z2: at(a.z1.max(a.z2)) })
    }
    fn holds(&self, x: i32, z: i32) -> bool {
        (self.x1..=self.x2).contains(&x) && (self.z1..=self.z2).contains(&z)
    }
    fn touches_region(&self, rx: i32, rz: i32) -> bool {
        let (x, z) = ((rx as i64) << 5, (rz as i64) << 5);
        x <= self.x2 as i64 && x + 31 >= self.x1 as i64 && z <= self.z2 as i64 && z + 31 >= self.z1 as i64
    }
}

/// `r.-1.2.mca` → (-1, 2).
fn region_name(name: &str) -> Option<(i32, i32)> {
    let mut parts = name.strip_prefix("r.")?.strip_suffix(".mca")?.split('.');
    let (x, z) = (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
    parts.next().is_none().then_some((x, z))
}

/// `c.-1.2.mcc` → (-1, 2): one chunk too big for its region, kept beside it.
fn external_name(name: &str) -> Option<(i32, i32)> {
    let mut parts = name.strip_prefix("c.")?.strip_suffix(".mcc")?.split('.');
    let (x, z) = (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
    parts.next().is_none().then_some((x, z))
}

fn location(header: &[u8], i: usize) -> u32 {
    u32::from_be_bytes(header[i * 4..i * 4 + 4].try_into().expect("four bytes"))
}

/// Every chunk the header lists must lie inside the file, after the header, and not on another chunk's sectors.
/// A file that fails isn't a region this code understands, so nothing in it is touched.
pub fn check_header(header: &[u8], file_len: u64, name: &str) -> Result<()> {
    let sectors = file_len.div_ceil(SECTOR as u64) as usize;
    let mut used = vec![false; sectors];
    for i in 0..1024 {
        let loc = location(header, i);
        if loc == 0 {
            continue;
        }
        let (offset, count) = ((loc >> 8) as usize, (loc & 0xff) as usize);
        if offset < 2 || count == 0 || offset + count > sectors {
            bail!("{name} has a damaged header, so nothing was changed. Chunk {i} doesn't fit in the file.");
        }
        for s in &mut used[offset..offset + count] {
            if std::mem::replace(s, true) {
                bail!("{name} has a damaged header, so nothing was changed. Two chunks share a sector.");
            }
        }
    }
    Ok(())
}

fn read_header(file: &mut std::fs::File, name: &str) -> Result<Option<Vec<u8>>> {
    let len = file.metadata()?.len();
    if len == 0 {
        return Ok(None);
    }
    if len < HEADER as u64 {
        bail!("{name} is shorter than a region header, so nothing was changed.");
    }
    let mut header = vec![0u8; HEADER];
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut header)?;
    check_header(&header, len, name)?;
    Ok(Some(header))
}

/// Region-relative slots of the chunks of `(rx, rz)` that are in `rect` and in the file.
fn slots(header: &[u8], rx: i32, rz: i32, rect: &Rect) -> Vec<usize> {
    let mut found = Vec::new();
    for lz in 0..32i64 {
        for lx in 0..32i64 {
            let (x, z) = (((rx as i64) << 5) + lx, ((rz as i64) << 5) + lz);
            let i = (lx + lz * 32) as usize;
            if x >= rect.x1 as i64 && x <= rect.x2 as i64 && z >= rect.z1 as i64 && z <= rect.z2 as i64 && location(header, i) != 0 {
                found.push(i);
            }
        }
    }
    found
}

/// Removes the chunks in `rect` from one region by zeroing their header entries. Every other byte stays.
pub fn clear_chunks(path: &Path, rx: i32, rz: i32, rect: &Rect) -> Result<usize> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut file = std::fs::OpenOptions::new().read(true).write(true).open(path)?;
    let Some(mut header) = read_header(&mut file, &name)? else { return Ok(0) };
    let found = slots(&header, rx, rz, rect);
    if found.is_empty() {
        return Ok(0);
    }
    for &i in &found {
        header[i * 4..i * 4 + 4].fill(0);
        header[SECTOR + i * 4..SECTOR + i * 4 + 4].fill(0);
    }
    // Locations first: the timestamp of a chunk that's gone means nothing, so a stop between the two is harmless.
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&header[..SECTOR])?;
    file.write_all(&header[SECTOR..])?;
    file.sync_all()?;
    let mut back = vec![0u8; HEADER];
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut back)?;
    if back != header {
        bail!("{name} didn't save correctly. Restore the backup.");
    }
    Ok(found.len())
}

/// What a reset of one folder (`region`, `entities` or `poi`) will touch.
#[derive(Debug, Default)]
struct FolderPlan {
    regions: Vec<(PathBuf, i32, i32)>,
    externals: Vec<PathBuf>,
    chunks: u64,
}

/// Reads one folder and refuses any format it doesn't know, before anything changes.
fn plan_folder(folder: &Path, rect: &Rect) -> Result<FolderPlan> {
    let mut plan = FolderPlan::default();
    let Ok(entries) = std::fs::read_dir(folder) else { return Ok(plan) };
    let mut anvil = false;
    let mut old = false;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let ext = name.rsplit('.').next().unwrap_or_default().to_ascii_lowercase();
        match ext.as_str() {
            "mca" => {
                let Some((rx, rz)) = region_name(&name) else { bail!("{name} isn't a region file name this tool knows, so nothing was changed.") };
                anvil = true;
                if rect.touches_region(rx, rz) {
                    let mut file = std::fs::File::open(e.path())?;
                    if let Some(header) = read_header(&mut file, &name)? {
                        let n = slots(&header, rx, rz, rect).len() as u64;
                        if n > 0 {
                            plan.chunks += n;
                            plan.regions.push((e.path(), rx, rz));
                        }
                    }
                }
            }
            "mcc" => {
                let Some((x, z)) = external_name(&name) else { bail!("{name} isn't a chunk file name this tool knows, so nothing was changed.") };
                if rect.holds(x, z) {
                    plan.externals.push(e.path());
                }
            }
            "mcr" => old = true,
            // Paper's sidecars for oversized chunks; left alone, a chunk saved there later replaces them.
            "nbt" => {}
            _ => bail!("{name} isn't an Anvil region file. This world uses a format (a Linear or similar plugin, perhaps) this tool can't edit safely, so nothing was changed."),
        }
    }
    if old && !anvil {
        bail!("This world is in the old McRegion format. Open it once in Minecraft 1.2 or newer first.");
    }
    Ok(plan)
}

#[derive(Serialize, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChunkCount {
    /// Chunks of terrain.
    pub chunks: u64,
    pub entities: u64,
    pub poi: u64,
    pub regions: usize,
    pub external: usize,
    /// The chunks' size in blocks across, for the page's summary.
    pub width: i64,
    pub depth: i64,
    pub backup: Option<String>,
}

fn plan_dimension(dimension: &Path, rect: &Rect) -> Result<[FolderPlan; 3]> {
    Ok([plan_folder(&dimension.join("region"), rect)?, plan_folder(&dimension.join("entities"), rect)?, plan_folder(&dimension.join("poi"), rect)?])
}

fn count_of(plans: &[FolderPlan; 3], rect: &Rect) -> ChunkCount {
    ChunkCount {
        chunks: plans[0].chunks,
        entities: plans[1].chunks,
        poi: plans[2].chunks,
        regions: plans[0].regions.len(),
        external: plans.iter().map(|p| p.externals.len()).sum(),
        width: (rect.x2 as i64 - rect.x1 as i64 + 1) * 16,
        depth: (rect.z2 as i64 - rect.z1 as i64 + 1) * 16,
        backup: None,
    }
}

/// A chunk is its terrain, plus its entities and points of interest in their own regions (Minecraft 1.17 and 1.14
/// on): leaving those would put old mobs and villager beds on the new terrain.
pub fn reset_dimension(dimension: &Path, rect: &Rect) -> Result<ChunkCount> {
    let plans = plan_dimension(dimension, rect)?;
    let count = count_of(&plans, rect);
    for plan in &plans {
        for (path, rx, rz) in &plan.regions {
            clear_chunks(path, *rx, *rz, rect)?;
        }
        for file in &plan.externals {
            std::fs::remove_file(file)?;
        }
    }
    Ok(count)
}

#[derive(Deserialize)]
pub struct ChunkTarget {
    world: String,
    dimension: String,
    area: Area,
    /// Only for the reset itself; the preview never writes.
    #[serde(default)]
    backup: Option<bool>,
}

/// The dimension's folder, if it's one of this world's own and the world is one these tools may touch.
async fn target(dir: &Path, meta: &ServerMeta, world: &str, dimension: &str) -> Result<PathBuf> {
    let found = usable(world_of(dir, world, meta.kind.bukkit()).await?)?;
    if !found.dimensions.iter().any(|d| d.path == dimension) {
        bail!("{world} has no such dimension.");
    }
    crate::core::fsutil::safe_join(dir, dimension)
}

pub async fn chunk_preview(id: &str, req: ChunkTarget) -> Result<ChunkCount> {
    let meta = store::get(id).await?;
    if let Some(why) = unsupported(meta.kind) {
        bail!("{why}");
    }
    let dir = store::dir(id)?;
    let rect = Rect::of(req.area)?;
    let folder = target(&dir, &meta, &req.world, &req.dimension).await?;
    tokio::task::spawn_blocking(move || Ok(count_of(&plan_dimension(&folder, &rect)?, &rect))).await?
}

pub async fn chunk_reset(id: &str, req: ChunkTarget) -> Result<ChunkCount> {
    let (_claim, meta, dir) = begin(id).await?;
    let Some(backup) = req.backup else { bail!("Choose whether to back up first.") };
    let rect = Rect::of(req.area)?;
    let folder = target(&dir, &meta, &req.world, &req.dimension).await?;
    // Checked before the backup too: a world that will be refused shouldn't cost one.
    let f = folder.clone();
    let before = tokio::task::spawn_blocking(move || Ok::<_, anyhow::Error>(count_of(&plan_dimension(&f, &rect)?, &rect))).await??;
    if before.chunks + before.entities + before.poi + before.external as u64 == 0 {
        bail!("There are no saved chunks in that area.");
    }
    let saved = backup_first(id, &req.world, backup).await?;
    let mut count = tokio::task::spawn_blocking(move || reset_dimension(&folder, &rect)).await??;
    count.backup = saved;
    Ok(count)
}

// ---------- Players ----------

#[derive(Serialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    pub file: String,
    pub uuid: String,
    pub name: Option<String>,
    pub pos: Option<[f64; 3]>,
    pub dimension: Option<String>,
    pub game_mode: Option<i32>,
    pub health: Option<f64>,
    pub food: Option<i32>,
    pub xp_level: Option<i32>,
    pub xp_progress: Option<f64>,
    pub xp_total: Option<i32>,
    /// Riding something: the game puts them where it is, whatever the file's position says.
    pub vehicle: bool,
    /// Why this file can't be edited.
    pub error: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PlayerEdit {
    pub pos: Option<[f64; 3]>,
    pub game_mode: Option<i32>,
    pub health: Option<f64>,
    pub food: Option<i32>,
    pub xp_level: Option<i32>,
    pub xp_progress: Option<f64>,
    pub xp_total: Option<i32>,
}

fn inflate(data: &[u8], compression: Compression) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    match compression {
        Compression::Gzip => MultiGzDecoder::new(data).read_to_end(&mut out).map(drop)?,
        Compression::Zlib => ZlibDecoder::new(data).read_to_end(&mut out).map(drop)?,
        Compression::None => out = data.to_vec(),
    }
    Ok(out)
}

/// Parses a file and proves it writes back byte for byte (before compression), so whatever an edit leaves alone stays
/// exactly as it was. Anything this NBT reader would change (broken text, data after the end) is refused.
pub fn read_exact_nbt(data: &[u8]) -> Result<NbtFile> {
    let parsed = nbt::parse(data)?;
    let plain = NbtFile { compression: Compression::None, ..parsed.clone() };
    if nbt::write(&plain)? != inflate(data, parsed.compression)? {
        bail!("This file holds data the editor can't keep exactly as it is, so it won't be changed.");
    }
    Ok(parsed)
}

fn int_of(tag: Option<&Tag>) -> Option<i32> {
    num(tag).map(|v| v as i32)
}

pub fn summary(file: &NbtFile, player: &mut Player) {
    let root = &file.root;
    if let Some(Tag::List(_, items)) = root.get("Pos") {
        if let [x, y, z] = items.as_slice() {
            if let (Tag::Double(x), Tag::Double(y), Tag::Double(z)) = (x, y, z) {
                player.pos = Some([*x, *y, *z]);
            }
        }
    }
    player.dimension = match root.get("Dimension") {
        Some(Tag::String(s)) => Some(s.clone()),
        Some(Tag::Int(0)) => Some("minecraft:overworld".into()),
        Some(Tag::Int(-1)) => Some("minecraft:the_nether".into()),
        Some(Tag::Int(1)) => Some("minecraft:the_end".into()),
        _ => None,
    };
    player.game_mode = int_of(root.get("playerGameType"));
    player.health = num(root.get("Health"));
    player.food = int_of(root.get("foodLevel"));
    player.xp_level = int_of(root.get("XpLevel"));
    player.xp_progress = num(root.get("XpP"));
    player.xp_total = int_of(root.get("XpTotal"));
    player.vehicle = root.get("RootVehicle").is_some();
    if player.name.is_none() {
        player.name = str(compound(root.get("bukkit")).and_then(|b| b.get("lastKnownName"))).map(str::to_string);
    }
}

/// Sets a number where the file already has it, in the type it already has. A file without the field stays without.
fn put(root: &mut nbt::Compound, key: &str, value: f64) -> Result<()> {
    let slot = root.get_mut(key).ok_or_else(|| anyhow!("This player's file has no {key}, so it can't be changed."))?;
    *slot = match slot {
        Tag::Byte(_) => Tag::Byte(value as i8),
        Tag::Short(_) => Tag::Short(value as i16),
        Tag::Int(_) => Tag::Int(value as i32),
        Tag::Long(_) => Tag::Long(value as i64),
        Tag::Float(_) => Tag::Float(value as f32),
        Tag::Double(_) => Tag::Double(value),
        _ => bail!("This player's {key} isn't a number, so it can't be changed."),
    };
    Ok(())
}

fn in_range(what: &str, value: f64, low: f64, high: f64) -> Result<f64> {
    if !value.is_finite() || value < low || value > high {
        bail!("{what} must be between {low} and {high}.");
    }
    Ok(value)
}

/// The changed file, or an error with the original untouched. Only the fields `edit` names differ.
pub fn edit_player(data: &[u8], edit: &PlayerEdit) -> Result<Vec<u8>> {
    let mut file = read_exact_nbt(data)?;
    let root = &mut file.root;
    if let Some([x, y, z]) = edit.pos {
        let (x, y, z) = (in_range("X", x, -30_000_000.0, 30_000_000.0)?, in_range("Y", y, -20_000_000.0, 20_000_000.0)?, in_range("Z", z, -30_000_000.0, 30_000_000.0)?);
        match root.get_mut("Pos") {
            Some(Tag::List(nbt::DOUBLE, items)) if items.len() == 3 => *items = vec![Tag::Double(x), Tag::Double(y), Tag::Double(z)],
            _ => bail!("This player's file has no position, so it can't be changed."),
        }
    }
    if let Some(v) = edit.game_mode {
        put(root, "playerGameType", in_range("Game mode", v as f64, 0.0, 3.0)?)?;
    }
    if let Some(v) = edit.health {
        put(root, "Health", in_range("Health", v, 0.0, 1024.0)?)?;
    }
    if let Some(v) = edit.food {
        put(root, "foodLevel", in_range("Food", v as f64, 0.0, 20.0)?)?;
    }
    if let Some(v) = edit.xp_level {
        put(root, "XpLevel", in_range("XP level", v as f64, 0.0, i32::MAX as f64)?)?;
    }
    if let Some(v) = edit.xp_progress {
        put(root, "XpP", in_range("XP progress", v, 0.0, 1.0)?)?;
    }
    if let Some(v) = edit.xp_total {
        put(root, "XpTotal", in_range("Total XP", v as f64, 0.0, i32::MAX as f64)?)?;
    }
    let out = nbt::write(&file)?;
    read_exact_nbt(&out).context("The edited file didn't read back")?;
    Ok(out)
}

/// usercache.json: lower-case UUID → the name it last had.
pub fn user_names(text: &str) -> BTreeMap<String, String> {
    #[derive(Deserialize)]
    struct Entry {
        uuid: String,
        name: String,
    }
    serde_json::from_str::<Vec<Entry>>(text).unwrap_or_default().into_iter().map(|e| (e.uuid.to_ascii_lowercase(), e.name)).collect()
}

pub fn list_players(dir: &Path, world: &str) -> Vec<Player> {
    let names = user_names(&std::fs::read_to_string(dir.join("usercache.json")).unwrap_or_default());
    let Some(folder) = player_dir(&dir.join(world)) else { return Vec::new() };
    let mut out: Vec<Player> = player_files(&folder)
        .iter()
        .map(|path| {
            let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let uuid = file.trim_end_matches(".dat").to_string();
            let mut player = Player { name: names.get(&uuid.to_ascii_lowercase()).cloned(), uuid, file, ..Player::default() };
            match std::fs::read(path).map_err(anyhow::Error::from).and_then(|d| read_exact_nbt(&d)) {
                Ok(nbt) => summary(&nbt, &mut player),
                Err(e) => player.error = Some(e.to_string()),
            }
            player
        })
        .collect();
    out.sort_by_key(|p| (p.name.is_none(), p.name.as_deref().unwrap_or(&p.uuid).to_ascii_lowercase()));
    out
}

pub async fn players(id: &str, world: &str) -> Result<Vec<Player>> {
    let meta = store::get(id).await?;
    if let Some(why) = unsupported(meta.kind) {
        bail!("{why}");
    }
    let dir = store::dir(id)?;
    let world = usable(world_of(&dir, world, meta.kind.bukkit()).await?)?.name;
    Ok(tokio::task::spawn_blocking(move || list_players(&dir, &world)).await?)
}

#[derive(Deserialize)]
pub struct SavePlayer {
    world: String,
    file: String,
    edit: PlayerEdit,
    backup: bool,
}

pub async fn save_player(id: &str, req: SavePlayer) -> Result<Option<String>> {
    let (_claim, meta, dir) = begin(id).await?;
    let world = usable(world_of(&dir, &req.world, meta.kind.bukkit()).await?)?;
    crate::content::index::safe_name(&req.file)?;
    if !req.file.ends_with(".dat") {
        bail!("That isn't a player's file.");
    }
    let Some(folder) = player_dir(&dir.join(&world.name)) else { bail!("{} has no player files.", world.name) };
    let path = folder.join(&req.file);
    let data = tokio::fs::read(&path).await.with_context(|| format!("{} is gone", req.file))?;
    // Built before the backup, so a refused edit doesn't cost one.
    let out = edit_player(&data, &req.edit)?;
    let saved = backup_first(id, &world.name, req.backup).await?;
    write_atomic(&path, &out).await?;
    Ok(saved)
}

/// `server:worldTools`, `server:world*` and `server:chunk*`.
pub async fn handle(channel: &str, args: Args) -> Result<Value> {
    let id: String = args.get(0)?;
    match channel {
        "server:worldTools" => json(info(&id).await?),
        "server:worldSwitch" => json(json!({ "backup": switch_world(&id, args.get(1)?).await? })),
        "server:worldCreate" => json(json!({ "backup": create_world(&id, args.get(1)?).await? })),
        "server:worldDelete" => json(json!({ "backup": delete_world(&id, args.get(1)?).await? })),
        "server:chunkPreview" => json(chunk_preview(&id, args.get(1)?).await?),
        "server:chunkReset" => json(chunk_reset(&id, args.get(1)?).await?),
        "server:worldPlayers" => json(players(&id, &args.get::<String>(1)?).await?),
        "server:worldPlayerSave" => json(json!({ "backup": save_player(&id, args.get(1)?).await? })),
        _ => bail!("Unknown request: {channel}"),
    }
}

#[cfg(test)]
#[path = "worldtools_tests.rs"]
mod tests;
