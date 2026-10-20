//! Reads other launchers' instance folders: Prism Launcher / MultiMC, the Modrinth App, the CurseForge app,
//! ATLauncher, GDLauncher and the official launcher's `.minecraft`. Every reader only looks; nothing here
//! writes to another launcher's folder. All of it is blocking file I/O (call from `spawn_blocking`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::content::index::parse_loader;
use crate::core::text::Text;
use crate::core::types::{LoaderType, ModpackRef, ServerKind, ServerLaunch};
use crate::tr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Launcher {
    Prism,
    Multimc,
    Modrinth,
    Curseforge,
    Atlauncher,
    Gdlauncher,
    Official,
    /// A folder the player picked that no launcher owns (a server folder, a loose `.minecraft`).
    Folder,
}

impl Launcher {
    pub fn label(self) -> &'static str {
        match self {
            Self::Prism => "Prism Launcher",
            Self::Multimc => "MultiMC",
            Self::Modrinth => "Modrinth App",
            Self::Curseforge => "CurseForge",
            Self::Atlauncher => "ATLauncher",
            Self::Gdlauncher => "GDLauncher",
            Self::Official => "Minecraft Launcher",
            Self::Folder => "Folder",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Icon {
    File(PathBuf),
    Bytes(Vec<u8>),
    /// A web address (a modpack's icon), kept as it is.
    Url(String),
}

/// One instance (or server) found in another launcher, ready to be copied.
#[derive(Debug, Clone)]
pub struct Found {
    pub launcher: Launcher,
    pub server: bool,
    pub name: String,
    /// The folder that identifies it (the instance's own folder, or `.minecraft` for official profiles).
    pub path: PathBuf,
    /// The official launcher's profile key (several profiles can share one `.minecraft`).
    pub profile: Option<String>,
    /// What gets copied into the new instance's game folder (or the server folder).
    pub game_dir: PathBuf,
    /// Top-level names in `game_dir` that are left behind (the other launcher's own files, caches, logs).
    /// `name*` matches a prefix, `*.ext` a suffix.
    pub skip: Vec<String>,
    pub mc_version: Option<String>,
    pub loader: LoaderType,
    pub loader_version: Option<String>,
    pub memory_mb: Option<u32>,
    pub jvm_args: Option<String>,
    pub icon: Option<Icon>,
    pub modpack: Option<ModpackRef>,
    pub server_kind: Option<ServerKind>,
    pub launch: Option<ServerLaunch>,
    pub port: Option<u16>,
    /// Why it can't be imported (shown instead of a checkbox).
    pub problem: Option<Text>,
}

impl Found {
    pub fn new(launcher: Launcher, name: impl Into<String>, path: &Path, game_dir: PathBuf) -> Found {
        Found {
            launcher,
            server: false,
            name: name.into(),
            path: path.to_path_buf(),
            profile: None,
            game_dir,
            skip: INSTANCE_SKIP.iter().map(|s| s.to_string()).collect(),
            mc_version: None,
            loader: LoaderType::Vanilla,
            loader_version: None,
            memory_mb: None,
            jvm_args: None,
            icon: None,
            modpack: None,
            server_kind: None,
            launch: None,
            port: None,
            problem: None,
        }
    }

    fn skipping(mut self, names: &[&str]) -> Found {
        self.skip.extend(names.iter().map(|s| s.to_string()));
        self
    }

    /// Marks it as not importable when no Minecraft version could be found.
    pub fn check(mut self) -> Found {
        if self.problem.is_none() && self.mc_version.as_deref().is_none_or(str::is_empty) {
            self.problem = Some(tr!("migrate.noVersion", "Couldn't tell which Minecraft version it uses"));
        }
        self
    }
}

/// Left behind in every instance: per-launch natives and old logs.
const INSTANCE_SKIP: [&str; 2] = ["natives", "logs"];

/// Whether `name` (a top-level entry of the game folder) is left behind.
pub fn is_skipped(name: &str, skip: &[String]) -> bool {
    let lower = name.to_lowercase();
    skip.iter().any(|rule| {
        let rule = rule.to_lowercase();
        if let Some(prefix) = rule.strip_suffix('*') {
            lower.starts_with(prefix)
        } else if let Some(suffix) = rule.strip_prefix('*') {
            lower.ends_with(suffix)
        } else {
            lower == rule
        }
    })
}

// ---------- Small helpers ----------

pub fn read_json_file(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

fn text_at<'a>(v: &'a Value, pointer: &str) -> Option<&'a str> {
    v.pointer(pointer).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty())
}

fn num_at(v: &Value, pointer: &str) -> Option<u64> {
    let x = v.pointer(pointer)?;
    x.as_u64().or_else(|| x.as_f64().map(|f| f.max(0.0) as u64)).or_else(|| x.as_str().and_then(|s| s.trim().parse().ok()))
}

fn id_at(v: &Value, pointer: &str) -> Option<String> {
    let x = v.pointer(pointer)?;
    match x {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Value::Number(n) if n.as_u64().is_some_and(|n| n > 0) => Some(n.to_string()),
        _ => None,
    }
}

pub fn folder_name(dir: &Path) -> String {
    dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn nonempty(s: Option<&str>) -> Option<String> {
    s.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

/// Memory in MB from a Java size ("4G", "4096M", "4096m", "4194304k").
pub fn parse_mem(value: &str) -> Option<u32> {
    let v = value.trim();
    let (num, unit) = v.split_at(v.find(|c: char| !c.is_ascii_digit()).unwrap_or(v.len()));
    let n: u64 = num.parse().ok()?;
    let mb = match unit.to_ascii_lowercase().as_str() {
        "g" | "gb" => n * 1024,
        "m" | "mb" => n,
        "k" | "kb" => n / 1024,
        "" => n / (1024 * 1024),
        _ => return None,
    };
    u32::try_from(mb).ok().filter(|m| *m >= 256)
}

/// Splits `-Xmx`/`-Xms` out of Java arguments: (max memory, the remaining arguments).
pub fn split_memory(args: &str) -> (Option<u32>, Option<String>) {
    let mut mem = None;
    let mut rest = Vec::new();
    for a in args.split_whitespace() {
        if let Some(v) = a.strip_prefix("-Xmx") {
            mem = parse_mem(v);
        } else if !a.starts_with("-Xms") {
            rest.push(a);
        }
    }
    (mem, Some(rest.join(" ")).filter(|s| !s.is_empty()))
}

/// The modpack an instance came from, as ResinCraft keeps it ("cf:<id>" for CurseForge).
fn pack_ref(name: &str, project: Option<String>, version: Option<String>, curseforge: bool) -> Option<ModpackRef> {
    let project = project?;
    let project = if curseforge && !project.starts_with("cf:") { format!("cf:{project}") } else { project };
    Some(ModpackRef { project_id: Some(project), version_id: version, name: name.to_string() })
}

/// Loader name as other launchers write it ("Forge", "fabric", "NeoForge", "LegacyFabric"…).
fn loader_named(name: &str) -> Option<LoaderType> {
    let n = name.trim().to_lowercase().replace(['-', '_', ' '], "");
    match n.as_str() {
        "neoforge" | "neoforged" => Some(LoaderType::Neoforge),
        "forge" | "minecraftforge" => Some(LoaderType::Forge),
        "fabric" | "fabricloader" => Some(LoaderType::Fabric),
        "quilt" | "quiltloader" => Some(LoaderType::Quilt),
        "vanilla" | "" => Some(LoaderType::Vanilla),
        _ => parse_loader(&n),
    }
}

// ---------- Prism Launcher / MultiMC ----------

/// Qt-style INI (instance.cfg, prismlauncher.cfg): `key=value` lines, values possibly quoted. Sections are ignored
/// ([General] is the only one that matters).
pub fn read_ini(text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(['#', ';', '[']) {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let mut v = v.trim().to_string();
        if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
            v = v[1..v.len() - 1].replace("\\\"", "\"").replace("\\\\", "\\");
        }
        out.insert(k.trim().to_string(), v);
    }
    out
}

/// The folder holding a Prism/MultiMC install's instances (`InstanceDir` in its config, else `instances`).
pub fn mmc_instances_dir(root: &Path) -> PathBuf {
    for cfg in ["prismlauncher.cfg", "polymc.cfg", "multimc.cfg"] {
        if let Ok(text) = std::fs::read_to_string(root.join(cfg)) {
            if let Some(dir) = read_ini(&text).get("InstanceDir").filter(|d| !d.is_empty()) {
                let p = PathBuf::from(dir);
                return if p.is_absolute() { p } else { root.join(p) };
            }
        }
    }
    root.join("instances")
}

/// One Prism/MultiMC instance folder (has instance.cfg). `root` is the launcher's folder, for its icons.
pub fn mmc_instance(dir: &Path, root: Option<&Path>, launcher: Launcher) -> Option<Found> {
    let cfg = read_ini(&std::fs::read_to_string(dir.join("instance.cfg")).ok()?);
    let name = cfg.get("name").filter(|n| !n.is_empty()).cloned().unwrap_or_else(|| folder_name(dir));
    let game = [".minecraft", "minecraft"].iter().map(|g| dir.join(g)).find(|p| p.is_dir()).unwrap_or_else(|| dir.join(".minecraft"));
    let mut f = Found::new(launcher, name, dir, game);
    if let Some(pack) = read_json_file(&dir.join("mmc-pack.json")) {
        for c in pack.get("components").and_then(Value::as_array).into_iter().flatten() {
            let version = text_at(c, "/version").or_else(|| text_at(c, "/cachedVersion")).map(str::to_string);
            match c.get("uid").and_then(Value::as_str).unwrap_or_default() {
                "net.minecraft" => f.mc_version = version,
                "net.fabricmc.fabric-loader" => (f.loader, f.loader_version) = (LoaderType::Fabric, version),
                "org.quiltmc.quilt-loader" => (f.loader, f.loader_version) = (LoaderType::Quilt, version),
                "net.minecraftforge" => (f.loader, f.loader_version) = (LoaderType::Forge, version),
                "net.neoforged" => (f.loader, f.loader_version) = (LoaderType::Neoforge, version),
                _ => {}
            }
        }
    }
    // MultiMC before mmc-pack.json.
    if f.mc_version.is_none() {
        f.mc_version = nonempty(cfg.get("IntendedVersion").map(String::as_str));
    }
    let on = |k: &str| cfg.get(k).is_some_and(|v| v.eq_ignore_ascii_case("true"));
    if on("OverrideMemory") {
        f.memory_mb = cfg.get("MaxMemAlloc").and_then(|m| m.parse().ok()).filter(|m| *m >= 256);
    }
    if on("OverrideJavaArgs") {
        f.jvm_args = nonempty(cfg.get("JvmArgs").map(String::as_str));
    }
    if let (Some(key), Some(root)) = (cfg.get("iconKey").filter(|k| !k.is_empty()), root) {
        f.icon = find_icon(&root.join("icons"), key).map(Icon::File);
    }
    if on("ManagedPack") {
        let name = cfg.get("ManagedPackName").cloned().unwrap_or_else(|| f.name.clone());
        let id = nonempty(cfg.get("ManagedPackID").map(String::as_str));
        let version = nonempty(cfg.get("ManagedPackVersionID").map(String::as_str));
        match cfg.get("ManagedPackType").map(String::as_str) {
            Some("modrinth") => f.modpack = pack_ref(&name, id, version, false),
            Some("flame") => f.modpack = pack_ref(&name, id, version, true),
            _ => {}
        }
    }
    Some(f.check())
}

/// `<icons>/<key>.<any extension>` (Prism keeps custom icons there; built-in keys have no file).
fn find_icon(icons: &Path, key: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(icons).ok()?;
    entries.flatten().map(|e| e.path()).find(|p| {
        p.is_file()
            && p.file_stem().is_some_and(|s| s.to_string_lossy() == key)
            && p.extension().is_some_and(|e| matches!(e.to_string_lossy().to_lowercase().as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp"))
    })
}

// ---------- Modrinth App ----------

/// Every profile of a Modrinth App install: from its database (app.db, current versions) and from
/// `profiles/<name>/profile.json` (versions before 0.8). `profiles` is where profile folders live.
pub fn modrinth_profiles(root: &Path) -> Vec<Found> {
    let mut rows = read_modrinth_db(&root.join("app.db")).unwrap_or_default();
    let profiles = rows
        .iter()
        .find_map(|r| r.get("__custom_dir").and_then(Value::as_str).map(|d| PathBuf::from(d).join("profiles")))
        .unwrap_or_else(|| root.join("profiles"));
    rows.retain(|r| r.get("__custom_dir").is_none());
    let mut out: Vec<Found> = rows.iter().filter_map(|r| modrinth_row(r, &profiles)).collect();
    if let Ok(entries) = std::fs::read_dir(&profiles) {
        for e in entries.flatten() {
            let dir = e.path();
            if dir.is_dir() && !out.iter().any(|f| f.path == dir) {
                if let Some(f) = modrinth_profile_json(&dir) {
                    out.push(f);
                }
            }
        }
    }
    out
}

/// The `profiles` table as JSON objects (column name → value), plus a `{"__custom_dir": …}` row when the
/// app keeps its data elsewhere. The database is copied first: the app may have it open, and reading must
/// never touch it.
pub fn read_modrinth_db(db: &Path) -> Option<Vec<serde_json::Map<String, Value>>> {
    if !db.is_file() {
        return None;
    }
    let tmp = std::env::temp_dir().join(format!("resincraft-modrinth-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&tmp).ok()?;
    let copy = tmp.join("app.db");
    let result = (|| {
        std::fs::copy(db, &copy).ok()?;
        for ext in ["-wal", "-shm"] {
            let side = PathBuf::from(format!("{}{ext}", db.display()));
            if side.is_file() {
                let _ = std::fs::copy(&side, tmp.join(format!("app.db{ext}")));
            }
        }
        let conn = rusqlite::Connection::open(&copy).ok()?;
        let mut rows = query_rows(&conn, "SELECT * FROM profiles").unwrap_or_default();
        if let Some(dir) = query_rows(&conn, "SELECT custom_dir FROM settings").ok().and_then(|r| r.into_iter().next()) {
            if let Some(d) = dir.get("custom_dir").and_then(Value::as_str).filter(|d| !d.is_empty()) {
                let mut m = serde_json::Map::new();
                m.insert("__custom_dir".into(), Value::String(d.to_string()));
                rows.push(m);
            }
        }
        Some(rows)
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

fn query_rows(conn: &rusqlite::Connection, sql: &str) -> rusqlite::Result<Vec<serde_json::Map<String, Value>>> {
    use rusqlite::types::ValueRef;
    let mut stmt = conn.prepare(sql)?;
    let names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let mut m = serde_json::Map::new();
        for (i, name) in names.iter().enumerate() {
            let v = match row.get_ref(i)? {
                ValueRef::Null | ValueRef::Blob(_) => Value::Null,
                ValueRef::Integer(n) => Value::from(n),
                ValueRef::Real(f) => Value::from(f),
                ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
            };
            m.insert(name.clone(), v);
        }
        out.push(m);
    }
    Ok(out)
}

fn modrinth_row(r: &serde_json::Map<String, Value>, profiles: &Path) -> Option<Found> {
    let row = Value::Object(r.clone());
    let rel = text_at(&row, "/path")?;
    let dir = profiles.join(rel);
    let name = text_at(&row, "/name").unwrap_or(rel).to_string();
    let mut f = Found::new(Launcher::Modrinth, name, &dir, dir.clone()).skipping(&["profile.json"]);
    f.mc_version = text_at(&row, "/game_version").map(str::to_string);
    f.loader = text_at(&row, "/mod_loader").and_then(loader_named).unwrap_or(LoaderType::Vanilla);
    f.loader_version = text_at(&row, "/mod_loader_version").map(str::to_string).filter(|_| f.loader != LoaderType::Vanilla);
    f.memory_mb = num_at(&row, "/override_mc_memory_max").and_then(|m| u32::try_from(m).ok()).filter(|m| *m >= 256);
    if let Some(args) = text_at(&row, "/override_extra_launch_args") {
        let list: Vec<String> = serde_json::from_str(args).unwrap_or_default();
        f.jvm_args = Some(list.join(" ")).filter(|s| !s.trim().is_empty());
    }
    f.icon = text_at(&row, "/icon_path").map(PathBuf::from).filter(|p| p.is_file()).map(Icon::File);
    f.modpack = pack_ref(&f.name, text_at(&row, "/linked_project_id").map(str::to_string), text_at(&row, "/linked_version_id").map(str::to_string), false);
    if !dir.is_dir() {
        f.problem = Some(tr!("migrate.folderMissing", "Its folder is missing"));
    }
    Some(f.check())
}

/// A profile folder of the Modrinth App before 0.8 (profile.json next to the game files).
pub fn modrinth_profile_json(dir: &Path) -> Option<Found> {
    let j = read_json_file(&dir.join("profile.json"))?;
    let meta = j.get("metadata")?;
    let name = text_at(meta, "/name").map(str::to_string).unwrap_or_else(|| folder_name(dir));
    let mut f = Found::new(Launcher::Modrinth, name, dir, dir.to_path_buf()).skipping(&["profile.json"]);
    f.mc_version = text_at(meta, "/game_version").map(str::to_string);
    f.loader = text_at(meta, "/loader").and_then(loader_named).unwrap_or(LoaderType::Vanilla);
    f.loader_version = text_at(meta, "/loader_version/id").map(str::to_string).filter(|_| f.loader != LoaderType::Vanilla);
    f.memory_mb = num_at(&j, "/memory/maximum").and_then(|m| u32::try_from(m).ok()).filter(|m| *m >= 256);
    if let Some(args) = j.pointer("/java/extra_arguments").and_then(Value::as_array) {
        let args: Vec<&str> = args.iter().filter_map(Value::as_str).collect();
        f.jvm_args = Some(args.join(" ")).filter(|s| !s.is_empty());
    }
    f.icon = text_at(meta, "/icon").map(PathBuf::from).filter(|p| p.is_file()).map(Icon::File);
    f.modpack =
        pack_ref(&f.name, text_at(meta, "/linked_data/project_id").map(str::to_string), text_at(meta, "/linked_data/version_id").map(str::to_string), false);
    Some(f.check())
}

// ---------- CurseForge app ----------

#[derive(Deserialize, Serialize)]
struct CfInstance {
    name: Option<Value>,
    #[serde(rename = "gameVersion")]
    game_version: Option<Value>,
    #[serde(rename = "baseModLoader")]
    base_mod_loader: Option<Value>,
    #[serde(rename = "allocatedMemory")]
    allocated_memory: Option<Value>,
    #[serde(rename = "javaArgsOverride")]
    java_args_override: Option<Value>,
    #[serde(rename = "profileImagePath")]
    profile_image_path: Option<Value>,
    #[serde(rename = "projectID")]
    project_id: Option<Value>,
    #[serde(rename = "fileID")]
    file_id: Option<Value>,
    #[serde(rename = "installedModpack")]
    installed_modpack: Option<Value>,
}

/// One CurseForge instance folder (has minecraftinstance.json); the folder itself is the game folder.
pub fn curseforge_instance(dir: &Path) -> Option<Found> {
    let text = std::fs::read_to_string(dir.join("minecraftinstance.json")).ok()?;
    // Only the fields used here: the file also lists every installed add-on and can be megabytes long.
    let slim: CfInstance = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let j = serde_json::to_value(slim).ok()?;
    let name = text_at(&j, "/name").map(str::to_string).unwrap_or_else(|| folder_name(dir));
    let mut f = Found::new(Launcher::Curseforge, name, dir, dir.to_path_buf()).skipping(&["minecraftinstance.json", ".curseclient", "Install"]);
    let mc = text_at(&j, "/gameVersion").or_else(|| text_at(&j, "/baseModLoader/minecraftVersion")).map(str::to_string);
    f.mc_version = mc.clone();
    // "forge-47.2.0", "neoforge-21.1.77", "fabric-0.15.7-1.20.1", "quilt-0.21.0-1.20.1".
    if let Some(loader_name) = text_at(&j, "/baseModLoader/name") {
        if let Some((kind, rest)) = loader_name.split_once('-') {
            if let Some(loader) = loader_named(kind).filter(|l| *l != LoaderType::Vanilla) {
                let rest = mc.as_deref().and_then(|mc| rest.strip_suffix(&format!("-{mc}"))).unwrap_or(rest);
                f.loader = loader;
                f.loader_version = Some(rest.to_string());
            }
        }
    }
    f.memory_mb = num_at(&j, "/allocatedMemory").and_then(|m| u32::try_from(m).ok()).filter(|m| *m >= 256);
    f.jvm_args = text_at(&j, "/javaArgsOverride").map(str::to_string);
    f.icon = text_at(&j, "/profileImagePath")
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .map(Icon::File)
        .or_else(|| text_at(&j, "/installedModpack/thumbnailUrl").filter(|u| u.starts_with("https://")).map(|u| Icon::Url(u.to_string())));
    let pack_name = text_at(&j, "/installedModpack/name").unwrap_or(&f.name).to_string();
    let project = id_at(&j, "/projectID").or_else(|| id_at(&j, "/installedModpack/addonID"));
    let file = id_at(&j, "/fileID").or_else(|| id_at(&j, "/installedModpack/installedFile/id"));
    f.modpack = pack_ref(&pack_name, project, file, true);
    Some(f.check())
}

// ---------- ATLauncher ----------

/// One ATLauncher instance folder (instance.json with a `launcher` section); the folder is the game folder.
pub fn atlauncher_instance(dir: &Path) -> Option<Found> {
    let j = read_json_file(&dir.join("instance.json"))?;
    let l = j.get("launcher")?;
    let name = text_at(l, "/name").map(str::to_string).unwrap_or_else(|| folder_name(dir));
    let mut f = Found::new(Launcher::Atlauncher, name, dir, dir.to_path_buf()).skipping(&["instance.json", "bin", "libraries", "versions", "assets"]);
    f.mc_version = text_at(&j, "/id").or_else(|| text_at(&j, "/minecraftVersion")).or_else(|| text_at(l, "/minecraftVersion")).map(str::to_string);
    if let Some(lv) = l.get("loaderVersion").filter(|v| v.is_object()) {
        if let Some(loader) = text_at(lv, "/type").and_then(loader_named) {
            f.loader = loader;
            f.loader_version = text_at(lv, "/version").map(str::to_string);
        }
    }
    f.memory_mb = num_at(l, "/maximumMemory").and_then(|m| u32::try_from(m).ok()).filter(|m| *m >= 256);
    f.jvm_args = text_at(l, "/javaArguments").map(str::to_string);
    f.icon = Some(dir.join("instance.png")).filter(|p| p.is_file()).map(Icon::File);
    let pack = text_at(l, "/pack").unwrap_or(&f.name).to_string();
    f.modpack = pack_ref(&pack, id_at(l, "/modrinthProject/id"), id_at(l, "/modrinthVersion/id"), false)
        .or_else(|| pack_ref(&pack, id_at(l, "/curseForgeProject/id"), id_at(l, "/curseForgeFile/id"), true));
    Some(f.check())
}

// ---------- GDLauncher ----------

/// A GDLauncher instance: the classic app (config.json in the game folder) or GDLauncher Carbon
/// (instance.json, game files in `instance/`).
pub fn gdlauncher_instance(dir: &Path) -> Option<Found> {
    if let Some(j) = read_json_file(&dir.join("config.json")).filter(|j| j.get("loader").is_some()) {
        let mut f = Found::new(Launcher::Gdlauncher, folder_name(dir), dir, dir.to_path_buf()).skipping(&["config.json"]);
        f.mc_version = text_at(&j, "/loader/mcVersion").map(str::to_string);
        f.loader = text_at(&j, "/loader/loaderType").and_then(loader_named).unwrap_or(LoaderType::Vanilla);
        f.loader_version = text_at(&j, "/loader/loaderVersion").map(str::to_string).filter(|_| f.loader != LoaderType::Vanilla);
        f.memory_mb = num_at(&j, "/javaMemory").and_then(|m| u32::try_from(m).ok()).filter(|m| *m >= 256);
        f.jvm_args = text_at(&j, "/javaArgs").map(str::to_string);
        f.icon = text_at(&j, "/background").map(|b| dir.join(b)).filter(|p| p.is_file()).map(Icon::File);
        let curseforge = text_at(&j, "/loader/source").is_some_and(|s| s.eq_ignore_ascii_case("curseforge"));
        f.modpack = pack_ref(&f.name, id_at(&j, "/loader/projectID"), id_at(&j, "/loader/fileID"), curseforge);
        return Some(f.check());
    }
    let j = read_json_file(&dir.join("instance.json")).filter(|j| j.get("game_configuration").is_some())?;
    let game = dir.join("instance");
    let name = text_at(&j, "/name").map(str::to_string).unwrap_or_else(|| folder_name(dir));
    let mut f = Found::new(Launcher::Gdlauncher, name, dir, game);
    let version = j.pointer("/game_configuration/version/Standard").or_else(|| j.pointer("/game_configuration/version"));
    if let Some(v) = version {
        f.mc_version = text_at(v, "/release").map(str::to_string);
        if let Some(ml) = v.get("modloaders").and_then(Value::as_array).and_then(|a| a.first()) {
            if let Some(loader) = text_at(ml, "/type_").or_else(|| text_at(ml, "/type")).and_then(loader_named) {
                f.loader = loader;
                f.loader_version = text_at(ml, "/version").map(str::to_string);
            }
        }
    }
    if let Some(max) = j.pointer("/game_configuration/memory").and_then(Value::as_array).and_then(|a| a.get(1)).and_then(Value::as_u64) {
        f.memory_mb = u32::try_from(max).ok().filter(|m| *m >= 256);
    }
    f.jvm_args = text_at(&j, "/game_configuration/extra_java_args").map(str::to_string);
    f.icon = text_at(&j, "/icon/RelativePath").or_else(|| text_at(&j, "/icon")).map(|p| dir.join(p)).filter(|p| p.is_file()).map(Icon::File);
    Some(f.check())
}

// ---------- Official launcher (.minecraft) ----------

/// The official launcher's own files and downloads in `.minecraft`, none of which belong in an instance
/// (and `launcher_accounts*.json` / `launcher_msa_credentials*` hold sign-ins: never copied).
const OFFICIAL_SKIP: [&str; 14] = [
    "assets",
    "libraries",
    "versions",
    "runtime",
    "bin",
    "webcache",
    "webcache2",
    "cachedImages",
    "launcher*",
    "treatment_tags.json",
    "*.exe",
    "*.dll",
    "*.log",
    "clientId.txt",
];

/// What a version folder name says: ("1.20.1", Fabric, "0.15.0") for `fabric-loader-0.15.0-1.20.1`.
pub fn parse_version_id(id: &str) -> (Option<String>, LoaderType, Option<String>) {
    let id = id.trim();
    for (prefix, loader) in [("fabric-loader-", LoaderType::Fabric), ("quilt-loader-", LoaderType::Quilt)] {
        if let Some(rest) = id.strip_prefix(prefix) {
            return match rest.split_once('-') {
                Some((l, mc)) => (Some(mc.to_string()), loader, Some(l.to_string())),
                None => (None, loader, Some(rest.to_string())),
            };
        }
    }
    if let Some(v) = id.strip_prefix("neoforge-") {
        return (neoforge_mc(v), LoaderType::Neoforge, Some(v.to_string()));
    }
    let lower = id.to_lowercase();
    if let Some(at) = lower.find("-forge") {
        let mc = &id[..at];
        // "1.20.1-forge-47.2.0", "1.12.2-forge1.12.2-14.23.5.2860", "1.7.10-Forge10.13.4.1614-1.7.10".
        let rest = id[at + "-forge".len()..].trim_start_matches('-');
        let rest = rest.strip_prefix(&format!("{mc}-")).unwrap_or(rest);
        let rest = rest.strip_suffix(&format!("-{mc}")).unwrap_or(rest);
        return (Some(mc.to_string()), LoaderType::Forge, Some(rest.to_string()));
    }
    // "1.20.1-OptiFine_HD_U_I6" and other add-ons on top of a plain version.
    let mc = id.split('-').next().unwrap_or(id);
    (Some(mc.to_string()).filter(|m| !m.is_empty()), LoaderType::Vanilla, None)
}

/// NeoForge 21.1.77 is for Minecraft 1.21.1 (20.4.x → 1.20.4, 21.0.x → 1.21).
fn neoforge_mc(v: &str) -> Option<String> {
    let mut parts = v.split('.');
    let (major, minor): (u32, u32) = (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
    Some(if minor == 0 { format!("1.{major}") } else { format!("1.{major}.{minor}") })
}

/// Reads a version's JSON (`versions/<id>/<id>.json`) to fill in what the id alone can't tell.
fn refine_from_version_json(mc_dir: &Path, id: &str, f: &mut Found) {
    let Some(j) = read_json_file(&mc_dir.join("versions").join(id).join(format!("{id}.json"))) else {
        return;
    };
    if let Some(parent) = text_at(&j, "/inheritsFrom") {
        f.mc_version = Some(parent.to_string());
    }
    for lib in j.get("libraries").and_then(Value::as_array).into_iter().flatten() {
        let Some(name) = text_at(lib, "/name") else {
            continue;
        };
        let parts: Vec<&str> = name.split(':').collect();
        let [group, artifact, version, ..] = parts.as_slice() else {
            continue;
        };
        match (*group, *artifact) {
            ("net.fabricmc", "fabric-loader") => (f.loader, f.loader_version) = (LoaderType::Fabric, Some(version.to_string())),
            ("org.quiltmc", "quilt-loader") => (f.loader, f.loader_version) = (LoaderType::Quilt, Some(version.to_string())),
            _ => {}
        }
    }
}

/// The newest plain release installed in `.minecraft/versions` (what "Latest release" last ran).
fn newest_installed_release(mc_dir: &Path) -> Option<String> {
    let mut best: Option<(String, String)> = None;
    for e in std::fs::read_dir(mc_dir.join("versions")).ok()?.flatten() {
        let id = e.file_name().to_string_lossy().into_owned();
        let Some(j) = read_json_file(&e.path().join(format!("{id}.json"))) else {
            continue;
        };
        if text_at(&j, "/type") != Some("release") || j.get("inheritsFrom").is_some() {
            continue;
        }
        let time = text_at(&j, "/releaseTime").unwrap_or_default().to_string();
        if best.as_ref().is_none_or(|(t, _)| time > *t) {
            best = Some((time, id));
        }
    }
    best.map(|(_, id)| id)
}

/// The official launcher's profiles (launcher_profiles.json in `.minecraft`), each one becoming an instance.
/// Without that file, a `.minecraft`-like folder is offered as a single instance.
pub fn official_profiles(mc_dir: &Path) -> Vec<Found> {
    let Some(j) = read_json_file(&mc_dir.join("launcher_profiles.json")) else {
        return loose_minecraft(mc_dir).into_iter().collect();
    };
    let mut out = Vec::new();
    let Some(profiles) = j.get("profiles").and_then(Value::as_object) else {
        return out;
    };
    for (key, p) in profiles {
        let kind = text_at(p, "/type").unwrap_or("custom");
        if kind == "latest-snapshot" {
            continue; // shares .minecraft with "Latest release"; snapshots are rarely what's wanted
        }
        let game = text_at(p, "/gameDir").map(PathBuf::from).unwrap_or_else(|| mc_dir.to_path_buf());
        let name = match text_at(p, "/name") {
            Some(n) => n.to_string(),
            None if kind == "latest-release" => "Latest release".to_string(),
            None => key.clone(),
        };
        let mut f = Found::new(Launcher::Official, name, &game, game.clone());
        f.skip.extend(OFFICIAL_SKIP.iter().map(|s| s.to_string()));
        f.profile = Some(key.clone());
        let version = text_at(p, "/lastVersionId").unwrap_or("latest-release");
        if version == "latest-release" || kind == "latest-release" {
            // Resolved to the current release at import when nothing is installed yet.
            f.mc_version = Some(newest_installed_release(mc_dir).unwrap_or_else(|| "latest-release".to_string()));
        } else {
            let (mc, loader, lv) = parse_version_id(version);
            (f.mc_version, f.loader, f.loader_version) = (mc, loader, lv);
            refine_from_version_json(mc_dir, version, &mut f);
        }
        if let Some(args) = text_at(p, "/javaArgs") {
            (f.memory_mb, f.jvm_args) = split_memory(args);
        }
        if let Some(icon) = text_at(p, "/icon").and_then(|i| i.split_once(";base64,")).map(|(_, b64)| b64.to_string()) {
            use base64::Engine;
            f.icon = base64::engine::general_purpose::STANDARD.decode(icon.trim()).ok().map(Icon::Bytes);
        }
        if !game.is_dir() {
            f.problem = Some(tr!("migrate.folderMissing", "Its folder is missing"));
        }
        out.push(f.check());
    }
    out.sort_by_key(|f| f.name.to_lowercase());
    out
}

/// A game folder with no launcher data (mods/saves/options.txt): one instance, using the version in its
/// `versions` folder when there is exactly one hint.
pub fn loose_minecraft(dir: &Path) -> Option<Found> {
    let looks = ["mods", "saves", "options.txt", "resourcepacks"].iter().any(|n| dir.join(n).exists());
    if !looks {
        return None;
    }
    let mut f = Found::new(Launcher::Folder, folder_name(dir), dir, dir.to_path_buf());
    f.skip.extend(OFFICIAL_SKIP.iter().map(|s| s.to_string()));
    if let Some(id) = newest_installed_release(dir) {
        f.mc_version = Some(id);
    }
    Some(f.check())
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A throwaway folder for fixtures, removed when dropped.
    pub struct Fixture(pub PathBuf);

    impl Default for Fixture {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Fixture {
        pub fn new() -> Fixture {
            let dir = std::env::temp_dir().join(format!("resincraft-migrate-test-{}", uuid::Uuid::new_v4().simple()));
            std::fs::create_dir_all(&dir).unwrap();
            Fixture(dir)
        }
        /// Writes `text` at `rel` (folders are created).
        pub fn file(&self, rel: &str, text: &str) -> &Self {
            let p = self.0.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
            self
        }
        pub fn path(&self, rel: &str) -> PathBuf {
            self.0.join(rel)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn prism_instance() {
        let fx = Fixture::new();
        fx.file("prismlauncher.cfg", "[General]\nInstanceDir=instances\n")
            .file(
                "instances/Cozy/instance.cfg",
                "[General]\nname=\"Cozy, Survival\"\niconKey=cozy\nOverrideMemory=true\nMaxMemAlloc=6144\nOverrideJavaArgs=true\nJvmArgs=-XX:+UseZGC\nManagedPack=true\nManagedPackType=modrinth\nManagedPackID=1KVo5zza\nManagedPackVersionID=abc123\nManagedPackName=Fabulously Optimized\n",
            )
            .file(
                "instances/Cozy/mmc-pack.json",
                r#"{"components":[{"uid":"org.lwjgl3","version":"3.3.3"},{"uid":"net.minecraft","version":"1.21.1"},{"uid":"net.fabricmc.intermediary","version":"1.21.1"},{"uid":"net.fabricmc.fabric-loader","version":"0.16.5"}],"formatVersion":1}"#,
            )
            .file("instances/Cozy/minecraft/mods/sodium.jar", "jar")
            .file("icons/cozy.png", "png");
        let dir = mmc_instances_dir(&fx.0);
        assert_eq!(dir, fx.path("instances"));
        let f = mmc_instance(&dir.join("Cozy"), Some(&fx.0), Launcher::Prism).unwrap();
        assert_eq!(f.name, "Cozy, Survival");
        assert_eq!(f.mc_version.as_deref(), Some("1.21.1"));
        assert_eq!((f.loader, f.loader_version.as_deref()), (LoaderType::Fabric, Some("0.16.5")));
        assert_eq!(f.game_dir, fx.path("instances/Cozy/minecraft"), "Prism's newer `minecraft` folder");
        assert_eq!(f.memory_mb, Some(6144));
        assert_eq!(f.jvm_args.as_deref(), Some("-XX:+UseZGC"));
        assert_eq!(f.icon, Some(Icon::File(fx.path("icons/cozy.png"))));
        let pack = f.modpack.unwrap();
        assert_eq!((pack.project_id.as_deref(), pack.version_id.as_deref()), (Some("1KVo5zza"), Some("abc123")));
        assert!(f.problem.is_none());
    }

    #[test]
    fn multimc_forge_and_legacy() {
        let fx = Fixture::new();
        fx.file("instances/Pack/instance.cfg", "InstanceType=OneSix\nname=Pack\nOverrideMemory=false\nMaxMemAlloc=8192\n")
            .file(
                "instances/Pack/mmc-pack.json",
                r#"{"components":[{"uid":"net.minecraft","cachedVersion":"1.20.1"},{"uid":"net.minecraftforge","version":"47.2.0"}]}"#,
            )
            .file("instances/Pack/.minecraft/options.txt", "")
            .file("instances/Old/instance.cfg", "name=Old\nIntendedVersion=1.8.9\n")
            .file("instances/Broken/instance.cfg", "name=Broken\n");
        let f = mmc_instance(&fx.path("instances/Pack"), None, Launcher::Multimc).unwrap();
        assert_eq!(f.mc_version.as_deref(), Some("1.20.1"));
        assert_eq!((f.loader, f.loader_version.as_deref()), (LoaderType::Forge, Some("47.2.0")));
        assert_eq!(f.game_dir, fx.path("instances/Pack/.minecraft"));
        assert_eq!(f.memory_mb, None, "memory only when the instance overrides it");
        let old = mmc_instance(&fx.path("instances/Old"), None, Launcher::Multimc).unwrap();
        assert_eq!(old.mc_version.as_deref(), Some("1.8.9"));
        let broken = mmc_instance(&fx.path("instances/Broken"), None, Launcher::Multimc).unwrap();
        assert!(broken.problem.is_some());
        assert!(mmc_instance(&fx.path("instances"), None, Launcher::Multimc).is_none());
    }

    #[test]
    fn modrinth_app_database_and_old_profiles() {
        let fx = Fixture::new();
        fx.file("profiles/Better Adventures/mods/a.jar", "jar").file("icon.png", "png");
        let conn = rusqlite::Connection::open(fx.path("app.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE profiles (path TEXT PRIMARY KEY, name TEXT, icon_path TEXT, game_version TEXT, mod_loader TEXT, mod_loader_version TEXT,
                linked_project_id TEXT, linked_version_id TEXT, override_extra_launch_args JSONB, override_mc_memory_max INTEGER);
             CREATE TABLE settings (id INTEGER, custom_dir TEXT);
             INSERT INTO settings VALUES (0, NULL);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO profiles VALUES ('Better Adventures', 'Better Adventures+', ?1, '1.20.1', 'neoforge', '47.1.106', 'abcd', 'v1', '[\"-XX:+UseG1GC\",\"-Dx=1\"]', 8192)",
            [fx.path("icon.png").to_string_lossy()],
        )
        .unwrap();
        conn.execute("INSERT INTO profiles VALUES ('Gone', 'Gone', NULL, '1.21', 'vanilla', NULL, NULL, NULL, '[]', NULL)", []).unwrap();
        drop(conn);
        fx.file(
            "profiles/Legacy/profile.json",
            r#"{"path":"Legacy","metadata":{"name":"Legacy Pack","game_version":"1.19.2","loader":"fabric","loader_version":{"id":"0.14.21","url":"","stable":true}},"memory":{"maximum":3072},"java":{"extra_arguments":["-Dfoo"]}}"#,
        );
        let found = modrinth_profiles(&fx.0);
        assert_eq!(found.len(), 3);
        let ba = found.iter().find(|f| f.name == "Better Adventures+").unwrap();
        assert_eq!(ba.game_dir, fx.path("profiles/Better Adventures"));
        assert_eq!((ba.loader, ba.loader_version.as_deref(), ba.mc_version.as_deref()), (LoaderType::Neoforge, Some("47.1.106"), Some("1.20.1")));
        assert_eq!(ba.memory_mb, Some(8192));
        assert_eq!(ba.jvm_args.as_deref(), Some("-XX:+UseG1GC -Dx=1"));
        assert_eq!(ba.icon, Some(Icon::File(fx.path("icon.png"))));
        assert_eq!(ba.modpack.as_ref().and_then(|m| m.project_id.as_deref()), Some("abcd"));
        let gone = found.iter().find(|f| f.name == "Gone").unwrap();
        assert!(gone.problem.is_some(), "a profile whose folder is missing can't be imported");
        let legacy = found.iter().find(|f| f.name == "Legacy Pack").unwrap();
        assert_eq!((legacy.loader, legacy.loader_version.as_deref()), (LoaderType::Fabric, Some("0.14.21")));
        assert_eq!(legacy.memory_mb, Some(3072));
        assert!(is_skipped("profile.json", &legacy.skip));
    }

    #[test]
    fn curseforge_instances() {
        let fx = Fixture::new();
        fx.file(
            "Instances/All the Mods 9/minecraftinstance.json",
            r#"{"name":"All the Mods 9","gameVersion":"1.20.1","baseModLoader":{"name":"forge-47.2.0","forgeVersion":"47.2.0","minecraftVersion":"1.20.1","type":1},
               "allocatedMemory":10240,"javaArgsOverride":null,"projectID":715572,"fileID":5458414,
               "installedModpack":{"addonID":715572,"name":"All the Mods 9 - ATM9","thumbnailUrl":"https://media.forgecdn.net/avatars/thumbnails/1.png","installedFile":{"id":5458414}}}"#,
        )
        .file(
            "Instances/Fabric One/minecraftinstance.json",
            "\u{feff}{\"name\":\"Fabric One\",\"gameVersion\":\"1.20.1\",\"baseModLoader\":{\"name\":\"fabric-0.15.7-1.20.1\",\"type\":4},\"projectID\":0}",
        )
        .file("Instances/NeoForge/minecraftinstance.json", r#"{"name":"NeoForge","gameVersion":"1.21.1","baseModLoader":{"name":"neoforge-21.1.77","type":6}}"#);
        let atm = curseforge_instance(&fx.path("Instances/All the Mods 9")).unwrap();
        assert_eq!((atm.loader, atm.loader_version.as_deref()), (LoaderType::Forge, Some("47.2.0")));
        assert_eq!(atm.memory_mb, Some(10240));
        assert_eq!(atm.game_dir, fx.path("Instances/All the Mods 9"));
        assert!(is_skipped("minecraftinstance.json", &atm.skip));
        assert!(matches!(atm.icon, Some(Icon::Url(_))));
        let pack = atm.modpack.unwrap();
        assert_eq!((pack.project_id.as_deref(), pack.version_id.as_deref()), (Some("cf:715572"), Some("5458414")));
        let fabric = curseforge_instance(&fx.path("Instances/Fabric One")).unwrap();
        assert_eq!((fabric.loader, fabric.loader_version.as_deref()), (LoaderType::Fabric, Some("0.15.7")));
        assert!(fabric.modpack.is_none());
        let neo = curseforge_instance(&fx.path("Instances/NeoForge")).unwrap();
        assert_eq!((neo.loader, neo.loader_version.as_deref()), (LoaderType::Neoforge, Some("21.1.77")));
    }

    #[test]
    fn atlauncher_instance_json() {
        let fx = Fixture::new();
        fx.file(
            "instances/MyPack/instance.json",
            r#"{"id":"1.20.1","type":"release","launcher":{"name":"My Pack","pack":"Cool Pack","version":"1.2","loaderVersion":{"version":"0.15.11","type":"Fabric"},
               "maximumMemory":6000,"javaArguments":"-XX:+UseG1GC","modrinthProject":{"id":"P7dR8mSH"},"modrinthVersion":{"id":"v9"}}}"#,
        )
        .file("instances/MyPack/instance.png", "png")
        .file("instances/Plain/instance.json", r#"{"id":"1.21.4","launcher":{"name":"Plain"}}"#)
        .file("instances/NotAtl/instance.json", r#"{"id":"1.21.4"}"#);
        let f = atlauncher_instance(&fx.path("instances/MyPack")).unwrap();
        assert_eq!(f.name, "My Pack");
        assert_eq!((f.mc_version.as_deref(), f.loader, f.loader_version.as_deref()), (Some("1.20.1"), LoaderType::Fabric, Some("0.15.11")));
        assert_eq!(f.memory_mb, Some(6000));
        assert_eq!(f.icon, Some(Icon::File(fx.path("instances/MyPack/instance.png"))));
        assert_eq!(f.modpack.and_then(|m| m.project_id), Some("P7dR8mSH".to_string()));
        let plain = atlauncher_instance(&fx.path("instances/Plain")).unwrap();
        assert_eq!(plain.loader, LoaderType::Vanilla);
        assert!(atlauncher_instance(&fx.path("instances/NotAtl")).is_none());
    }

    #[test]
    fn gdlauncher_classic_and_carbon() {
        let fx = Fixture::new();
        fx.file(
            "instances/Classic/config.json",
            r#"{"loader":{"loaderType":"forge","mcVersion":"1.18.2","loaderVersion":"1.18.2-40.2.0","fileID":123,"projectID":456,"source":"curseforge"},"javaMemory":4096,"background":"icon.png"}"#,
        )
        .file("instances/Classic/icon.png", "png")
        .file(
            "data/instances/Carbon/instance.json",
            r#"{"name":"Carbon Pack","icon":{"RelativePath":"icon.png"},"game_configuration":{"version":{"Standard":{"release":"1.20.4","modloaders":[{"type_":"Quilt","version":"0.25.0"}]}},"memory":[1024,5120],"extra_java_args":"-Dz"}}"#,
        )
        .file("data/instances/Carbon/icon.png", "png")
        .file("data/instances/Carbon/instance/saves/World/level.dat", "");
        let c = gdlauncher_instance(&fx.path("instances/Classic")).unwrap();
        assert_eq!(c.name, "Classic");
        assert_eq!((c.mc_version.as_deref(), c.loader, c.loader_version.as_deref()), (Some("1.18.2"), LoaderType::Forge, Some("1.18.2-40.2.0")));
        assert_eq!(c.memory_mb, Some(4096));
        assert_eq!(c.modpack.and_then(|m| m.project_id), Some("cf:456".to_string()));
        assert!(is_skipped("config.json", &c.skip));
        let n = gdlauncher_instance(&fx.path("data/instances/Carbon")).unwrap();
        assert_eq!(n.name, "Carbon Pack");
        assert_eq!(n.game_dir, fx.path("data/instances/Carbon/instance"));
        assert_eq!((n.mc_version.as_deref(), n.loader, n.loader_version.as_deref()), (Some("1.20.4"), LoaderType::Quilt, Some("0.25.0")));
        assert_eq!(n.memory_mb, Some(5120));
        assert_eq!(n.icon, Some(Icon::File(fx.path("data/instances/Carbon/icon.png"))));
    }

    #[test]
    fn version_ids() {
        assert_eq!(parse_version_id("fabric-loader-0.15.0-1.20.1"), (Some("1.20.1".into()), LoaderType::Fabric, Some("0.15.0".into())));
        assert_eq!(parse_version_id("quilt-loader-0.21.0-1.20.1"), (Some("1.20.1".into()), LoaderType::Quilt, Some("0.21.0".into())));
        assert_eq!(parse_version_id("1.20.1-forge-47.2.0"), (Some("1.20.1".into()), LoaderType::Forge, Some("47.2.0".into())));
        assert_eq!(parse_version_id("1.12.2-forge1.12.2-14.23.5.2860"), (Some("1.12.2".into()), LoaderType::Forge, Some("14.23.5.2860".into())));
        assert_eq!(parse_version_id("1.7.10-Forge10.13.4.1614-1.7.10"), (Some("1.7.10".into()), LoaderType::Forge, Some("10.13.4.1614".into())));
        assert_eq!(parse_version_id("neoforge-21.1.77"), (Some("1.21.1".into()), LoaderType::Neoforge, Some("21.1.77".into())));
        assert_eq!(parse_version_id("neoforge-21.0.10-beta").0.as_deref(), Some("1.21"));
        assert_eq!(parse_version_id("1.20.1-OptiFine_HD_U_I6"), (Some("1.20.1".into()), LoaderType::Vanilla, None));
        assert_eq!(parse_version_id("1.21.4"), (Some("1.21.4".into()), LoaderType::Vanilla, None));
    }

    #[test]
    fn memory_from_java_args() {
        assert_eq!(split_memory("-Xmx4G -XX:+UseG1GC -Xms1G"), (Some(4096), Some("-XX:+UseG1GC".into())));
        assert_eq!(split_memory("-Xmx2048m"), (Some(2048), None));
        assert_eq!(parse_mem("4194304k"), Some(4096));
        assert_eq!(parse_mem("lots"), None);
    }

    #[test]
    fn official_launcher_profiles() {
        let fx = Fixture::new();
        let mc = fx.path(".minecraft");
        let elsewhere = fx.path("Modded");
        fx.file(
            ".minecraft/launcher_profiles.json",
            &serde_json::json!({ "profiles": {
                "a1": { "type": "latest-release", "lastVersionId": "latest-release", "name": "" },
                "a2": { "type": "latest-snapshot", "lastVersionId": "latest-snapshot", "name": "" },
                "b1": { "type": "custom", "name": "Fabric Fun", "lastVersionId": "fabric-loader-0.15.0-1.20.1", "gameDir": elsewhere,
                        "javaArgs": "-Xmx6G -XX:+UseG1GC", "icon": "data:image/png;base64,iVBORw0K" },
                "c1": { "type": "custom", "name": "Neo", "lastVersionId": "neoforge-21.1.77" }
            }})
            .to_string(),
        )
        .file(".minecraft/versions/1.21.4/1.21.4.json", r#"{"id":"1.21.4","type":"release","releaseTime":"2024-12-03T10:12:57+00:00"}"#)
        .file(".minecraft/versions/1.20.1/1.20.1.json", r#"{"id":"1.20.1","type":"release","releaseTime":"2023-06-12T13:25:51+00:00"}"#)
        .file(".minecraft/versions/neoforge-21.1.77/neoforge-21.1.77.json", r#"{"id":"neoforge-21.1.77","inheritsFrom":"1.21.1","libraries":[]}"#)
        .file(".minecraft/launcher_accounts_microsoft_store.json", "{}")
        .file("Modded/mods/x.jar", "jar");
        let found = official_profiles(&mc);
        assert_eq!(found.len(), 3, "the snapshot profile is left out");
        let latest = found.iter().find(|f| f.profile.as_deref() == Some("a1")).unwrap();
        assert_eq!((latest.name.as_str(), latest.mc_version.as_deref()), ("Latest release", Some("1.21.4")));
        assert!(is_skipped("launcher_accounts_microsoft_store.json", &latest.skip), "sign-ins are never copied");
        assert!(is_skipped("versions", &latest.skip) && is_skipped("assets", &latest.skip) && !is_skipped("saves", &latest.skip));
        let fabric = found.iter().find(|f| f.name == "Fabric Fun").unwrap();
        assert_eq!(fabric.game_dir, elsewhere);
        assert_eq!((fabric.mc_version.as_deref(), fabric.loader, fabric.loader_version.as_deref()), (Some("1.20.1"), LoaderType::Fabric, Some("0.15.0")));
        assert_eq!((fabric.memory_mb, fabric.jvm_args.as_deref()), (Some(6144), Some("-XX:+UseG1GC")));
        assert!(matches!(fabric.icon, Some(Icon::Bytes(_))));
        let neo = found.iter().find(|f| f.name == "Neo").unwrap();
        assert_eq!((neo.mc_version.as_deref(), neo.loader), (Some("1.21.1"), LoaderType::Neoforge));
    }

    #[test]
    fn skip_rules() {
        let rules: Vec<String> = ["launcher*", "*.exe", "assets"].iter().map(|s| s.to_string()).collect();
        assert!(is_skipped("launcher_profiles.json", &rules));
        assert!(is_skipped("MinecraftLauncher.EXE", &rules));
        assert!(is_skipped("Assets", &rules));
        assert!(!is_skipped("saves", &rules));
    }
}
