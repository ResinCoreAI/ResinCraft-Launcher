//! Keeps client-only mods off modded servers. Before each start, new or changed jars are checked: Modrinth's
//! server-side flag, a bundled list of known crashers (client_only_mods.txt), and the client-only marker of pure
//! Fabric/Quilt jars. Client-only ones are turned off (renamed to `.jar.disabled`, so the Mods tab shows them with a
//! switch), unless another mod on the server needs them. When a start still crashes because a mod reached for the
//! game client, the crash names it and it's turned off the same way. A mod the host turns back on stays on.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::core::fsutil::{read_json, write_json};
use crate::core::net::{hash_file, HashAlgo};

/// What ResinCraft remembers per server (in the server's folder).
const CHECKED_FILE: &str = ".resincraft-client-mods.json";

/// Mod ids known to crash a dedicated server (see the file's own notes).
const KNOWN: &str = include_str!("client_only_mods.txt");

fn known_ids() -> &'static BTreeSet<String> {
    static K: OnceLock<BTreeSet<String>> = OnceLock::new();
    K.get_or_init(|| KNOWN.lines().map(|l| l.split('#').next().unwrap_or("").trim().to_lowercase()).filter(|l| !l.is_empty()).collect())
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Checked {
    /// File name → "size:modified" of jars found fine (not client-only, or needed by another mod).
    ok: BTreeMap<String, String>,
    /// Jars ResinCraft turned off (by their .jar name), and why.
    disabled: BTreeMap<String, Reason>,
    /// Jars the host turned back on after ResinCraft turned them off: left alone from then on.
    kept: BTreeSet<String>,
    /// The bundled list the `ok` entries were checked against (a longer list checks them again).
    known: String,
}

/// Why a mod was turned off.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Reason {
    /// Modrinth says it doesn't run on servers.
    Modrinth,
    /// On ResinCraft's list of mods that crash servers.
    KnownList,
    /// A Fabric/Quilt mod that declares itself client-only.
    ClientMarker,
    /// It crashed the server reaching for the game client.
    Crash,
}

/// A mod ResinCraft turned off on a server (`server:clientMods`).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DisabledMod {
    /// The file as it is now (`….jar.disabled`).
    pub file: String,
    pub reason: Reason,
}

fn stamp(meta: &std::fs::Metadata) -> String {
    let modified = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
    format!("{}:{modified}", meta.len())
}

fn jars(mods: &Path) -> Vec<(String, PathBuf, std::fs::Metadata)> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(mods) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if let Ok(meta) = e.metadata() {
                if meta.is_file() && name.to_lowercase().ends_with(".jar") {
                    out.push((name, e.path(), meta));
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Before a start: turns off the client-only jars in mods/ (see the module notes); returns their names. Offline,
/// only the bundled list and the jars' own markers count (the crash check still catches what breaks the start).
pub async fn sweep(dir: &Path) -> Vec<String> {
    let mods = dir.join("mods");
    let checked_path = dir.join(CHECKED_FILE);
    let mut checked: Checked = read_json(&checked_path).await.unwrap_or_default();
    if checked.known != KNOWN {
        checked.ok.clear();
        checked.known = KNOWN.to_string();
    }
    let all = jars(&mods);
    // Turned off by ResinCraft, then back on by the host: theirs to decide now.
    for (name, _, _) in &all {
        if checked.disabled.remove(name).is_some() {
            checked.kept.insert(name.clone());
        }
    }
    let present: BTreeSet<&String> = all.iter().map(|(n, _, _)| n).collect();
    checked.ok.retain(|n, _| present.contains(n));

    // Each new or changed jar: its own metadata first, then (together) Modrinth.
    let mut flagged: BTreeMap<PathBuf, (String, String, Reason)> = BTreeMap::new();
    let mut unknown: BTreeMap<String, (String, PathBuf, String)> = BTreeMap::new();
    for (name, path, meta) in &all {
        let st = stamp(meta);
        if checked.kept.contains(name) || checked.ok.get(name) == Some(&st) {
            continue;
        }
        let jm = jar_meta(path);
        let title = jm.ids.first().cloned().unwrap_or_else(|| name.clone());
        if jm.ids.iter().any(|id| known_ids().contains(&id.to_lowercase())) {
            flagged.insert(path.clone(), (name.clone(), title, Reason::KnownList));
        } else if jm.client_marker && !jm.forge {
            // Multi-loader jars carry a Fabric marker too, while their Forge side may well run on servers.
            flagged.insert(path.clone(), (name.clone(), title, Reason::ClientMarker));
        } else if let Ok(h) = hash_file(path, HashAlgo::Sha1).await {
            unknown.insert(h, (name.clone(), path.clone(), st));
        }
    }
    if !unknown.is_empty() {
        let hashes: Vec<String> = unknown.keys().cloned().collect();
        let lookup = tokio::time::timeout(std::time::Duration::from_secs(15), super::create::client_only_sha1s(&hashes)).await;
        match lookup {
            Ok(Ok(client)) => {
                for (hash, (name, path, st)) in unknown {
                    match client.get(&hash) {
                        Some(title) => {
                            flagged.insert(path, (name, title.clone(), Reason::Modrinth));
                        }
                        None => {
                            checked.ok.insert(name, st);
                        }
                    }
                }
            }
            // Offline: these get looked at again next start.
            _ => {}
        }
    }

    let mut moved = Vec::new();
    if !flagged.is_empty() {
        let removable: BTreeSet<PathBuf> = not_needed(&mods, flagged.keys().cloned().collect()).into_iter().collect();
        for (path, (name, title, reason)) in flagged {
            if removable.contains(&path) {
                if turn_off(&path).await {
                    checked.disabled.insert(name, reason);
                    moved.push(title);
                }
            } else if let Ok(meta) = std::fs::metadata(&path) {
                // Client-only, but another mod here needs it: it stays.
                checked.ok.insert(name, stamp(&meta));
            }
        }
    }
    let _ = write_json(&checked_path, &checked).await;
    moved
}

/// Renames `x.jar` to `x.jar.disabled` (replacing an older disabled copy).
async fn turn_off(jar: &Path) -> bool {
    let mut off = jar.as_os_str().to_owned();
    off.push(".disabled");
    let off = PathBuf::from(off);
    let _ = tokio::fs::remove_file(&off).await;
    tokio::fs::rename(jar, &off).await.is_ok()
}

/// Turns off a jar the crash named, and remembers why (even if the host had turned it back on: it crashed).
pub async fn disable_crashed(dir: &Path, jar: &Path) -> bool {
    if !turn_off(jar).await {
        return false;
    }
    let checked_path = dir.join(CHECKED_FILE);
    let mut checked: Checked = read_json(&checked_path).await.unwrap_or_default();
    if let Some(name) = jar.file_name().map(|n| n.to_string_lossy().into_owned()) {
        checked.kept.remove(&name);
        checked.ok.remove(&name);
        checked.disabled.insert(name, Reason::Crash);
    }
    let _ = write_json(&checked_path, &checked).await;
    true
}

/// Records mods turned off while the server was made (Modrinth's check), so they show with the others.
pub async fn record_disabled(dir: &Path, names: &[String], reason: Reason) {
    let checked_path = dir.join(CHECKED_FILE);
    let mut checked: Checked = read_json(&checked_path).await.unwrap_or_default();
    for n in names {
        checked.disabled.insert(n.clone(), reason);
    }
    let _ = write_json(&checked_path, &checked).await;
}

/// Turns off a jar in mods/ (while the server is made) and returns its .jar name.
pub async fn turn_off_named(jar: &Path) -> Option<String> {
    let name = jar.file_name()?.to_string_lossy().into_owned();
    turn_off(jar).await.then_some(name)
}

/// The mods ResinCraft turned off on a server and still off.
pub async fn disabled(dir: &Path) -> Vec<DisabledMod> {
    let checked: Checked = read_json(&dir.join(CHECKED_FILE)).await.unwrap_or_default();
    let mods = dir.join("mods");
    checked
        .disabled
        .into_iter()
        .map(|(name, reason)| DisabledMod { file: format!("{name}.disabled"), reason })
        .filter(|d| mods.join(&d.file).is_file())
        .collect()
}

struct Patterns {
    client_class: Regex,
    fatal: Regex,
    mixin_owner: Regex,
    mods_toml_id: Regex,
    fabric_id: Regex,
    fabric_depends: Regex,
    json_key: Regex,
    fabric_client: Regex,
    failed_mod: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        client_class: Regex::new(r"net[/.]minecraft[/.]client[/.]|com[/.]mojang[/.]blaze3d[/.]|for invalid dist DEDICATED_SERVER").unwrap(),
        fatal: Regex::new(r"Caused by:.*(?:invalid dist DEDICATED_SERVER|NoClassDefFoundError: net/minecraft/client/|ClassNotFoundException: net\.minecraft\.client\.)").unwrap(),
        // Mixin names injected methods handler$<id>$<modid>$<name> (also redirect$, modify$, wrapOperation$…).
        mixin_owner: Regex::new(r"\.(?:handler|redirect|modify|localvar|wrapOperation|wrapWithCondition|modifyExpressionValue|modifyReturnValue)\$[0-9a-z]+\$([a-z][a-z0-9_]{1,63})\$").unwrap(),
        mods_toml_id: Regex::new(r#"(?m)^\s*modId\s*=\s*"([^"]+)""#).unwrap(),
        fabric_id: Regex::new(r#""id"\s*:\s*"([^"]+)""#).unwrap(),
        fabric_depends: Regex::new(r#""depends"\s*:\s*\{([^}]*)\}"#).unwrap(),
        json_key: Regex::new(r#""([^"]+)"\s*:"#).unwrap(),
        fabric_client: Regex::new(r#""environment"\s*:\s*"client""#).unwrap(),
        // Forge's own line when a mod can't be built: "Failed to create mod instance. ModID: citresewn, class …".
        failed_mod: Regex::new(r"Failed to create mod instance\. ModID: ([a-z][a-z0-9_]{1,63})").unwrap(),
    })
}

/// The jars in mods/ that a crashed start shows reaching for the game client: those a crash report written since
/// `since` lists as failing on a client class, and those whose mixins sit in a fatal client-class stack trace in `log`.
pub fn culprits(dir: &Path, log: &[String], since: SystemTime) -> Vec<PathBuf> {
    let mods = dir.join("mods");
    let mut out: BTreeSet<PathBuf> = BTreeSet::new();
    for report in recent_reports(&dir.join("crash-reports"), since) {
        out.extend(report_culprits(&report, &mods));
    }
    let p = patterns();
    // "Failed to create mod instance. ModID: x" followed (within its stack trace) by a client class.
    let mut failed: BTreeSet<String> = BTreeSet::new();
    for (i, line) in log.iter().enumerate() {
        if let Some(c) = p.failed_mod.captures(line) {
            if log.iter().skip(i).take(4).any(|l| p.client_class.is_match(l) && !l.contains("[mixin/]") && !l.contains("RuntimeDistCleaner")) {
                failed.insert(c[1].to_string());
            }
        }
    }
    if !failed.is_empty() {
        for (id, jar) in mod_ids(&mods) {
            if failed.contains(&id) {
                out.insert(jar);
            }
        }
    }
    if log.iter().any(|l| p.fatal.is_match(l)) {
        let ids: BTreeSet<String> = log.iter().filter_map(|l| p.mixin_owner.captures(l)).map(|c| c[1].to_string()).collect();
        if !ids.is_empty() {
            for (id, jar) in mod_ids(&mods) {
                if ids.contains(&id) {
                    out.insert(jar);
                }
            }
        }
    }
    out.into_iter().filter(|j| j.is_file()).collect()
}

fn recent_reports(dir: &Path, since: SystemTime) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let fresh = e.metadata().and_then(|m| m.modified()).is_ok_and(|t| t >= since);
            if fresh && e.file_name().to_string_lossy().ends_with(".txt") {
                if let Ok(text) = std::fs::read_to_string(e.path()) {
                    out.push(text);
                }
            }
        }
    }
    out
}

/// Jars in `mods` a Forge crash report lists ("Mod File: …") with a failure on a client class.
fn report_culprits(report: &str, mods: &Path) -> Vec<PathBuf> {
    let p = patterns();
    let mut out = Vec::new();
    let mut parts = report.split("Mod File:").skip(1);
    for part in &mut parts {
        let mut lines = part.lines();
        let file = lines.next().unwrap_or("").trim();
        // The block runs until the next mod's section.
        let body: String = lines.take_while(|l| !l.starts_with("-- ")).collect::<Vec<_>>().join("\n");
        if !p.client_class.is_match(&body) {
            continue;
        }
        let Some(name) = Path::new(file.trim_start_matches('/')).file_name() else { continue };
        let jar = mods.join(name);
        if jar.is_file() {
            out.push(jar);
        }
    }
    out
}

/// Mod ids declared by each jar in `mods` (Forge/NeoForge mods.toml, Fabric/Quilt metadata).
fn mod_ids(mods: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for (_, path, _) in jars(mods) {
        out.extend(jar_meta(&path).ids.into_iter().map(|id| (id, path.clone())));
    }
    out
}

/// What a jar says about itself: the mods it holds, and the mods they need on a server.
#[derive(Default)]
struct JarMods {
    ids: Vec<String>,
    needs: Vec<String>,
    /// fabric.mod.json / quilt.mod.json says `"environment": "client"`.
    client_marker: bool,
    /// Has a mods.toml (a Forge or NeoForge mod, maybe besides a Fabric one).
    forge: bool,
}

/// The game and its loaders, which every mod lists as a dependency.
const NOT_MODS: [&str; 7] = ["minecraft", "forge", "neoforge", "java", "fabricloader", "quilt_loader", "fml"];

fn jar_meta(path: &Path) -> JarMods {
    let p = patterns();
    let mut out = JarMods::default();
    let Ok(file) = std::fs::File::open(path) else { return out };
    let Ok(mut zip) = zip::ZipArchive::new(file) else { return out };
    let mut read = |name: &str| -> Option<String> {
        let mut s = String::new();
        zip.by_name(name).ok()?.take(256 * 1024).read_to_string(&mut s).ok()?;
        Some(s)
    };
    for toml in ["META-INF/mods.toml", "META-INF/neoforge.mods.toml"] {
        if let Some(text) = read(toml) {
            // Ids before the first dependency table are the jar's own mods.
            let own = text.split("[[dependencies").next().unwrap_or("");
            out.ids.extend(p.mods_toml_id.captures_iter(own).map(|c| c[1].to_string()));
            out.needs.extend(toml_needs(&text));
            out.forge = true;
        }
    }
    for json in ["fabric.mod.json", "quilt.mod.json"] {
        if let Some(text) = read(json) {
            if let Some(c) = p.fabric_id.captures(&text) {
                out.ids.push(c[1].to_string());
            }
            if let Some(c) = p.fabric_depends.captures(&text) {
                out.needs.extend(p.json_key.captures_iter(&c[1]).map(|k| k[1].to_string()));
            }
            out.client_marker |= p.fabric_client.is_match(&text);
        }
    }
    out.needs.retain(|n| !NOT_MODS.contains(&n.as_str()));
    out
}

/// The mods a mods.toml requires on a server: its [[dependencies.*]] tables that are mandatory (or of type
/// "required") and not for the client side only.
fn toml_needs(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut in_dep, mut id, mut required, mut client) = (false, None::<String>, false, false);
    let mut flush = |id: &mut Option<String>, required: &mut bool, client: &mut bool| {
        if let Some(i) = id.take() {
            if *required && !*client {
                out.push(i);
            }
        }
        *required = false;
        *client = false;
    };
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            flush(&mut id, &mut required, &mut client);
            in_dep = l.starts_with("[[dependencies");
            continue;
        }
        if !in_dep {
            continue;
        }
        let Some((k, v)) = l.split_once('=') else { continue };
        let v = v.split('#').next().unwrap_or("").trim().trim_matches('"').to_lowercase();
        match k.trim() {
            "modId" => id = Some(v),
            "mandatory" => required = v == "true",
            "type" => required = v == "required",
            "side" => client = v == "client",
            _ => {}
        }
    }
    flush(&mut id, &mut required, &mut client);
    out
}

/// Of `candidates` (jars in `mods` to take off a server), those no mod staying there needs. A mod another mod
/// requires stays even when Modrinth calls it client-only (a library a server-side mod builds on).
pub fn not_needed(mods: &Path, candidates: Vec<PathBuf>) -> Vec<PathBuf> {
    let all: Vec<(PathBuf, JarMods)> = jars(mods).into_iter().map(|(_, path, _)| (path.clone(), jar_meta(&path))).collect();
    let mut going: BTreeSet<PathBuf> = candidates.into_iter().collect();
    loop {
        let needed: BTreeSet<&String> = all.iter().filter(|(p, _)| !going.contains(p)).flat_map(|(_, m)| m.needs.iter()).collect();
        let keep: Vec<PathBuf> =
            all.iter().filter(|(p, m)| going.contains(p) && m.ids.iter().any(|id| needed.contains(id))).map(|(p, _)| p.clone()).collect();
        if keep.is_empty() {
            break;
        }
        for p in keep {
            going.remove(&p);
        }
    }
    going.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = "---- Minecraft Crash Report ----
-- Mod loading issue for: zume --
Details:
	Mod File: /C:/srv/mods/zume-1.2.0.jar
	Failure message: Zume (zume) has failed to load correctly
		java.lang.NoClassDefFoundError: net/minecraft/client/CameraType
-- Mod loading issue for: tacz --
Details:
	Mod File: /C:/srv/mods/tacz.jar
	Failure message: TACZ has failed to load correctly
		java.lang.IllegalStateException: something on the server side
";

    /// A jar with just a mods.toml (`forge`) or a fabric.mod.json for `id`.
    fn jar(dir: &Path, file: &str, id: &str, fabric_client: bool) -> PathBuf {
        use std::io::Write;
        let path = dir.join(file);
        let mut z = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        if fabric_client {
            z.start_file("fabric.mod.json", opts).unwrap();
            write!(z, r#"{{"schemaVersion":1,"id":"{id}","environment":"client"}}"#).unwrap();
        } else {
            z.start_file("META-INF/mods.toml", opts).unwrap();
            write!(z, "modLoader=\"javafml\"
[[mods]]
modId=\"{id}\"
").unwrap();
        }
        z.finish().unwrap();
        path
    }

    /// The real console of a Connector server that CIT Resewn crashed, cut down to the lines that matter.
    const CITRESEWN_LOG: &str = "[main/ERROR] [ne.mi.fm.lo.RuntimeDistCleaner/DISTXFORM]: Attempted to load class net/minecraft/client/gui/screens/Screen for invalid dist DEDICATED_SERVER
[main/WARN] [mixin/]: @Mixin target net.minecraft.client.gui.screens.Screen was not found fabric-screen-api-v1.mixins.json:ScreenAccessor from mod fabric_screen_api_v1
[main/WARN] [mixin/]: @Mixin target net.minecraft.client.main.Main was not found mixins.emibridge.json:MainEntrypointMixin from mod connectorextras_emi_bridge
[modloading-worker-0/ERROR] [ne.mi.fm.ja.FMLModContainer/LOADING]: Failed to create mod instance. ModID: citresewn, class shcm.shsupercm.forge.citresewn.CITResewn
java.lang.NoClassDefFoundError: net/minecraft/client/renderer/texture/atlas/SpriteSource
	at shcm.shsupercm.forge.citresewn.CITResewn.<init>(CITResewn.java:28) ~[citresewn-1.20.1-5.jar%23243!/:?]
[main/ERROR] [minecraft/Main]: Failed to start the minecraft server";

    #[test]
    fn the_citresewn_crash_names_only_citresewn() {
        let dir = std::env::temp_dir().join(format!("resin-citresewn-{}", std::process::id()));
        let mods = dir.join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        let cit = jar(&mods, "citresewn-1.20.1-5.jar", "citresewn", false);
        jar(&mods, "fabric-screen-api.jar", "fabric_screen_api_v1", false);
        jar(&mods, "connectorextras.jar", "connectorextras_emi_bridge", false);
        let log: Vec<String> = CITRESEWN_LOG.lines().map(str::to_string).collect();
        // No crash report: the console alone names it.
        let found = culprits(&dir, &log, SystemTime::now());
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(found, vec![cit]);
    }

    #[test]
    fn bundled_list_and_client_markers() {
        assert!(known_ids().contains("citresewn"));
        let dir = std::env::temp_dir().join(format!("resin-markers-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fabric = jar_meta(&jar(&dir, "chatanimation.jar", "chatanimation", true));
        assert!(fabric.client_marker && !fabric.forge);
        let forge = jar_meta(&jar(&dir, "tacz.jar", "tacz", false));
        assert!(!forge.client_marker && forge.forge && forge.ids == ["tacz"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn crash_reports_name_the_client_only_jars() {
        let dir = std::env::temp_dir().join(format!("resin-clientmods-{}", std::process::id()));
        let mods = dir.join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("zume-1.2.0.jar"), "x").unwrap();
        std::fs::write(mods.join("tacz.jar"), "x").unwrap();
        let found = report_culprits(REPORT, &mods);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(found, vec![mods.join("zume-1.2.0.jar")]);
    }

    #[test]
    fn mods_toml_dependencies_needed_on_a_server() {
        let toml = r#"
[[mods]]
modId="tacz"
[[dependencies.tacz]]
    modId="forge"
    mandatory=true
    side="BOTH"
[[dependencies.tacz]]
    modId="searchables" # a library
    mandatory=true
    side="BOTH"
[[dependencies.tacz]]
    modId="jei"
    mandatory=false
    side="BOTH"
[[dependencies.tacz]]
    modId="embeddium"
    type="required"
    side="CLIENT"
"#;
        assert_eq!(toml_needs(toml), vec!["forge".to_string(), "searchables".to_string()]);
    }

    #[test]
    fn mixin_handlers_name_their_mod() {
        let line = "at TRANSFORMER/minecraft@1.20.1/net.minecraft.resources.ResourceLocation.handler$bbf000$entity_texture_features$etf$illegalPathOverride(ResourceLocation.java:525)";
        assert_eq!(&patterns().mixin_owner.captures(line).unwrap()[1], "entity_texture_features");
        assert!(patterns().fatal.is_match(
            "[STDERR/]: Caused by: java.lang.RuntimeException: Attempted to load class net/minecraft/client/gui/screens/Screen for invalid dist DEDICATED_SERVER"
        ));
        // A warning about a client class isn't a crash.
        assert!(!patterns().fatal.is_match("[mixin/]: Error loading class: net/minecraft/client/MouseHandler (java.lang.RuntimeException: Attempted to load class net/minecraft/client/MouseHandler for invalid dist DEDICATED_SERVER)"));
    }
}
