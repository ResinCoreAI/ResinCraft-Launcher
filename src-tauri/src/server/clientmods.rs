//! Keeps client-only mods off modded servers. Before each start, jars Modrinth marks as client-only move from mods/ to
//! mods-disabled/; when a start still crashes because a mod reached for the game client (one Modrinth doesn't know),
//! the crash names it and it moves out the same way.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::core::fsutil::{read_json, write_json};
use crate::core::net::{hash_file, HashAlgo};

/// Where client-only mods go: the server doesn't load it, and the files stay for anyone who wants them back.
pub const DISABLED_DIR: &str = "mods-disabled";
/// Jars already looked up, so a start only asks Modrinth about new or changed ones.
const CHECKED_FILE: &str = ".resincraft-client-mods.json";

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Checked {
    /// File name → "size:modified" of jars Modrinth didn't call client-only.
    ok: BTreeMap<String, String>,
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

/// Before a start: moves the jars Modrinth marks as client-only out of mods/; returns their names. Offline or when
/// Modrinth can't be reached, nothing moves (the crash check below still catches what breaks the start).
pub async fn sweep(dir: &Path) -> Vec<String> {
    let mods = dir.join("mods");
    let checked_path = dir.join(CHECKED_FILE);
    let mut checked: Checked = read_json(&checked_path).await.unwrap_or_default();
    let all = jars(&mods);
    let mut by_hash: BTreeMap<String, (String, PathBuf, String)> = BTreeMap::new();
    for (name, path, meta) in &all {
        let st = stamp(meta);
        if checked.ok.get(name) == Some(&st) {
            continue;
        }
        if let Ok(h) = hash_file(path, HashAlgo::Sha1).await {
            by_hash.insert(h, (name.clone(), path.clone(), st));
        }
    }
    let present: BTreeSet<&String> = all.iter().map(|(n, _, _)| n).collect();
    checked.ok.retain(|n, _| present.contains(n));
    let mut moved = Vec::new();
    if !by_hash.is_empty() {
        let hashes: Vec<String> = by_hash.keys().cloned().collect();
        let lookup = tokio::time::timeout(std::time::Duration::from_secs(15), super::create::client_only_sha1s(&hashes)).await;
        let Ok(Ok(client)) = lookup else { return moved };
        let flagged: Vec<PathBuf> = by_hash.iter().filter(|(h, _)| client.contains_key(*h)).map(|(_, (_, p, _))| p.clone()).collect();
        let removable: BTreeSet<PathBuf> = not_needed(&mods, flagged).into_iter().collect();
        for (hash, (name, path, st)) in by_hash {
            match client.get(&hash) {
                Some(title) if removable.contains(&path) => {
                    if move_out(dir, &path).await {
                        moved.push(title.clone());
                    }
                }
                // Not client-only, or client-only but another mod here needs it: it stays.
                _ => {
                    checked.ok.insert(name, st);
                }
            }
        }
    }
    let _ = write_json(&checked_path, &checked).await;
    moved
}

/// Moves one jar from mods/ into mods-disabled/ (replacing an older copy there).
pub async fn move_out(dir: &Path, jar: &Path) -> bool {
    let Some(name) = jar.file_name() else { return false };
    let to = dir.join(DISABLED_DIR);
    if tokio::fs::create_dir_all(&to).await.is_err() {
        return false;
    }
    let dest = to.join(name);
    let _ = tokio::fs::remove_file(&dest).await;
    tokio::fs::rename(jar, &dest).await.is_ok()
}

struct Patterns {
    client_class: Regex,
    fatal: Regex,
    mixin_owner: Regex,
    mods_toml_id: Regex,
    fabric_id: Regex,
    fabric_depends: Regex,
    json_key: Regex,
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
