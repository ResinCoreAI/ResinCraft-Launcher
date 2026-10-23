//! The crash fixer: when Minecraft exits with an error, reads the new crash report, logs/latest.log and the game's
//! last console lines, says what went wrong in plain words, and offers a fix for what it knows how to fix
//! (installing a missing mod, turning off a mod that crashed or won't load, Java back to automatic, more memory).
//! Nothing changes until the player presses Fix.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::SystemTime;

use anyhow::{bail, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::content::jarmeta;
use crate::core::text::Text;
use crate::core::types::{ContentKind, Instance};
use crate::core::{instances, paths, settings};
use crate::tr;

/// What a Fix button does.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Fix {
    /// Installs a mod (a Modrinth project id or slug) that another mod needs.
    #[serde(rename_all = "camelCase")]
    InstallMod { project_id: String, name: String },
    /// Turns off a mod in mods/ (renamed to .disabled; the Content tab turns it back on).
    #[serde(rename_all = "camelCase")]
    DisableMod { file: String },
    /// Lets ResinCraft pick the Java this Minecraft needs (instead of the one set for the instance).
    JavaAuto,
    /// Gives the game more memory.
    #[serde(rename_all = "camelCase")]
    Memory { mb: u32 },
}

/// One thing that went wrong.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    pub title: Text,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<Text>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<Fix>,
}

/// What the crash fixer found after a crash (the `gameCrash` event).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashReport {
    pub instance_id: String,
    pub run_id: String,
    pub problems: Vec<Problem>,
    /// The crash report file it read, if Minecraft wrote one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crash_report: Option<String>,
}

struct Patterns {
    fabric_missing: Regex,
    fabric_incompatible: Regex,
    forge_missing: Regex,
    forge_failed: Regex,
    mixin_mod: Regex,
    entrypoint: Regex,
    class_version: Regex,
    out_of_memory: Regex,
    driver: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        // Fabric: "Mod 'Sodium Extra' (sodium-extra) 0.5.1 requires any version of mod sodium, which is missing!" and
        // newer "… requires version 0.15.x of 'Fabric API' (fabric-api), which is missing!".
        fabric_missing: Regex::new(
            r"Mod '(?P<who>[^']+)' \([\w\-.]+\) \S+ requires (?:any version|version [^,!']+?) of (?:mod (?P<id1>[\w\-.]+)|'(?P<name>[^']+)' \((?P<id2>[\w\-.]+)\)),? which is missing",
        )
        .unwrap(),
        // "Mod 'Iris' (iris) 1.6.4 is incompatible with any version of mod 'OptiFabric' (optifabric), but …"
        fabric_incompatible: Regex::new(r"Mod '(?P<who>[^']+)' \((?P<whoid>[\w\-.]+)\) \S+ is incompatible with (?:any version|version [^,!']+?) of (?:mod )?'?(?P<other>[^',(]+?)'? ?\((?P<otherid>[\w\-.]+)\)").unwrap(),
        // Forge/NeoForge: "Mod ID: 'geckolib', Requested by: 'mowziesmobs', Expected range: '[4.4,)'"
        forge_missing: Regex::new(r"Mod ID: '(?P<id>[\w\-.]+)', Requested by: '(?P<by>[\w\-.]+)'").unwrap(),
        // "Failed to create mod instance. ModID: citresewn, class …"
        forge_failed: Regex::new(r"Failed to create mod instance\. ModID: (?P<id>[\w\-.]+)").unwrap(),
        // "Mixin apply for mod sodium failed" / "… from mod connectorextras …" in a mixin error.
        // Fabric's "A mod crashed on startup!": "Could not execute entrypoint stage 'preLaunch' due to errors, provided by
        // 'sodium'" (also "Exception while loading entries for entrypoint 'main' provided by 'x'").
        entrypoint: Regex::new(r"(?:Could not execute entrypoint stage '\w+' due to errors, provided by|entrypoint '\w+' provided by) '(?P<id>[\w\-.]+)'").unwrap(),
        mixin_mod: Regex::new(r"(?:Mixin apply for mod (?P<a>[\w\-.]+) failed|MixinApplyError.*?from mod (?P<b>[\w\-.]+)|MixinTransformerError.*?from mod (?P<c>[\w\-.]+))").unwrap(),
        // "UnsupportedClassVersionError: … compiled by a more recent version of the Java Runtime (class file version 65.0)"
        class_version: Regex::new(r"UnsupportedClassVersionError.*class file version (?P<v>\d+)").unwrap(),
        out_of_memory: Regex::new(r"java\.lang\.OutOfMemoryError").unwrap(),
        // The JVM's own crash in a graphics driver.
        driver: Regex::new(r"(?i)EXCEPTION_ACCESS_VIOLATION|Problematic frame:.*(?:atio6axx|atig6pxx|nvoglv|ig\d+icd|amdvlk)").unwrap(),
    })
}

/// A console line saying the loader gave up and is showing its own error window while the game process stays open
/// (Fabric's "A mod crashed on startup!", "Incompatible mods found!").
pub fn loader_gave_up(line: &str) -> bool {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"A mod crashed on startup|Could not execute entrypoint stage|Incompatible mods found|Mod resolution failed|incompatible mod set").unwrap()).is_match(line)
}

/// The crash report Minecraft (or the loader) wrote during this run, if any.
fn newest_report(game: &Path, since: SystemTime) -> Option<(PathBuf, String)> {
    let mut best: Option<(SystemTime, PathBuf)> = None;
    for e in std::fs::read_dir(game.join("crash-reports")).ok()?.flatten() {
        let Ok(modified) = e.metadata().and_then(|m| m.modified()) else { continue };
        if modified >= since && best.as_ref().is_none_or(|(t, _)| modified > *t) {
            best = Some((modified, e.path()));
        }
    }
    let path = best?.1;
    let text = std::fs::read_to_string(&path).ok()?;
    Some((path, text))
}

/// The end of logs/latest.log, when it was written during this run.
fn latest_log(game: &Path, since: SystemTime) -> String {
    let path = game.join("logs").join("latest.log");
    let fresh = std::fs::metadata(&path).and_then(|m| m.modified()).is_ok_and(|t| t >= since);
    if !fresh {
        return String::new();
    }
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    // The last 400 KB holds the failure; a long session's start doesn't matter.
    let cut = text.len().saturating_sub(400 * 1024);
    text[text.char_indices().map(|(i, _)| i).find(|i| *i >= cut).unwrap_or(0)..].to_string()
}

/// The jar in mods/ that holds mod `id` (by the ids in its metadata), as its file name.
fn jar_of(mods: &Path, id: &str) -> Option<String> {
    let id = id.to_lowercase();
    for e in std::fs::read_dir(mods).ok()?.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.to_lowercase().ends_with(".jar") && jarmeta::read_jar(&e.path()).ids.iter().any(|m| *m == id) {
            return Some(name);
        }
    }
    None
}

/// What went wrong in a run that crashed. `console` is the game's last output lines (a wrong Java fails before
/// Minecraft writes any log).
pub async fn diagnose(inst: &Instance, since: SystemTime, console: &[String]) -> (Vec<Problem>, Option<String>) {
    let game = paths::game_dir(&inst.id);
    let mods = game.join("mods");
    let report = newest_report(&game, since);
    let mut text = latest_log(&game, since);
    for l in console {
        text.push('\n');
        text.push_str(l);
    }
    if let Some((_, r)) = &report {
        text.push('\n');
        text.push_str(r);
    }
    let p = patterns();
    let mut out: Vec<Problem> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut add = |key: String, problem: Problem| {
        if seen.insert(key) {
            out.push(problem);
        }
    };

    // A mod needs another that isn't installed.
    let mut missing: Vec<(String, String, String)> = Vec::new();
    for c in p.fabric_missing.captures_iter(&text) {
        let id = c.name("id1").or(c.name("id2")).map(|m| m.as_str().to_string()).unwrap_or_default();
        let name = c.name("name").map(|m| m.as_str().to_string()).unwrap_or_else(|| id.clone());
        missing.push((id, name, c["who"].to_string()));
    }
    for c in p.forge_missing.captures_iter(&text) {
        missing.push((c["id"].to_string(), c["id"].to_string(), c["by"].to_string()));
    }
    for (id, name, who) in missing {
        if matches!(id.as_str(), "minecraft" | "java" | "fabricloader" | "forge" | "neoforge") {
            // Needs another game or loader version: nothing to install.
            add(format!("needs:{id}"), Problem { title: tr!("crash.needsLoader", "{who} needs a different version of {what}", who = who, what = name), detail: None, fix: None });
            continue;
        }
        let project = modrinth_project(&id).await;
        add(
            format!("missing:{id}"),
            Problem {
                title: tr!("crash.missingMod", "{who} needs {dep}, which isn't installed", who = who, dep = name.clone()),
                detail: project.is_none().then(|| tr!("crash.missingModManual", "Find {dep} in Discover and install it.", dep = name.clone())),
                fix: project.map(|project_id| Fix::InstallMod { project_id, name }),
            },
        );
    }

    // Two mods that don't work together.
    for c in p.fabric_incompatible.captures_iter(&text) {
        let file = jar_of(&mods, &c["whoid"]);
        add(
            format!("incompatible:{}", &c["whoid"]),
            Problem {
                title: tr!("crash.incompatible", "{who} doesn't work together with {other}", who = c["who"].to_string(), other = c["other"].trim().to_string()),
                detail: Some(tr!("crash.incompatibleHint", "Turn one of them off.")),
                fix: file.map(|file| Fix::DisableMod { file }),
            },
        );
    }

    // A mod that crashed while loading: Forge's crash report sections, its console line, and mixin failures.
    let mut crashed: Vec<String> = Vec::new();
    if let Some((_, r)) = &report {
        for part in r.split("-- MOD ").skip(1) {
            let id = part.split_whitespace().next().unwrap_or("").trim_end_matches('-').to_string();
            if part.contains("has failed to load correctly") && !id.is_empty() {
                crashed.push(id);
            }
        }
    }
    crashed.extend(p.forge_failed.captures_iter(&text).map(|c| c["id"].to_string()));
    crashed.extend(p.entrypoint.captures_iter(&text).map(|c| c["id"].to_string()));
    for c in p.mixin_mod.captures_iter(&text) {
        if let Some(m) = c.name("a").or(c.name("b")).or(c.name("c")) {
            crashed.push(m.as_str().to_string());
        }
    }
    for id in crashed {
        let Some(file) = jar_of(&mods, &id) else { continue };
        add(
            format!("crashed:{file}"),
            Problem {
                title: tr!("crash.modCrashed", "{who} crashed the game", who = id),
                detail: Some(tr!("crash.modCrashedHint", "Turning it off lets the game start; look for an update of it later.")),
                fix: Some(Fix::DisableMod { file }),
            },
        );
    }

    // The wrong Java.
    if let Some(c) = p.class_version.captures(&text) {
        let need = c["v"].parse::<u32>().unwrap_or(0).saturating_sub(44);
        add(
            "java".into(),
            Problem {
                title: tr!("crash.java", "This needs Java {version} or newer", version = need),
                detail: inst.java_path.is_none().then(|| tr!("crash.javaHint", "A mod needs a newer Java than this Minecraft version uses.")),
                fix: inst.java_path.is_some().then_some(Fix::JavaAuto),
            },
        );
    }

    // Out of memory.
    if p.out_of_memory.is_match(&text) {
        let now = match inst.memory_mb {
            Some(mb) => mb,
            None => settings::get().await.memory_mb,
        };
        let most = (settings::total_memory_mb() * 3 / 4) as u32;
        let more = ((now + 2048) / 512 * 512).min(most);
        add(
            "memory".into(),
            Problem {
                title: tr!("crash.memory", "Minecraft ran out of memory"),
                detail: Some(tr!("crash.memoryHint", "It has {now} MB; more lets bigger modpacks run.", now = now)),
                fix: (more > now).then_some(Fix::Memory { mb: more }),
            },
        );
    }

    // The graphics driver itself crashed (only said when nothing else explains the crash).
    drop(add);
    if out.is_empty() && p.driver.is_match(&text) {
        out.push(
            Problem {
                title: tr!("crash.driver", "The graphics driver crashed"),
                detail: Some(tr!("crash.driverHint", "Update your graphics driver. Shader mods (Iris, Oculus, OptiFine) can also cause this: try turning them off.")),
                fix: None,
            },
        );
    }
    (out, report.map(|(path, _)| path.to_string_lossy().into_owned()))
}

/// A Modrinth project for mod id `id` (mod ids and Modrinth slugs usually match; `_` is often `-`).
async fn modrinth_project(id: &str) -> Option<String> {
    let mut tries = vec![id.to_string()];
    if id.contains('_') {
        tries.push(id.replace('_', "-"));
    }
    for slug in tries {
        let url = format!("https://api.modrinth.com/v2/project/{}", urlencoding::encode(&slug));
        if let Ok(v) = crate::core::net::fetch_json::<serde_json::Value>(&url).await {
            if v["project_type"] == "mod" {
                return v["id"].as_str().map(str::to_string);
            }
        }
    }
    None
}

/// Does what a Fix button says, for instance `id`.
pub async fn apply(id: &str, fix: &Fix) -> Result<()> {
    match fix {
        Fix::InstallMod { project_id, .. } => crate::content::sources::install(id, project_id, ContentKind::Mod, None).await,
        Fix::DisableMod { file } => {
            if file.contains(['/', '\\']) || !file.to_lowercase().ends_with(".jar") {
                bail!("Not a mod file: {file}");
            }
            crate::content::modrinth::toggle_content(id, ContentKind::Mod, file).await
        }
        Fix::JavaAuto => instances::update(id, json!({ "javaPath": null })).await.map(|_| ()),
        Fix::Memory { mb } => instances::update(id, json!({ "memoryMb": mb })).await.map(|_| ()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_fabric_and_forge_messages() {
        let p = patterns();
        let fabric = "\t - Mod 'Sodium Extra' (sodium-extra) 0.5.1 requires any version of mod sodium, which is missing!\n\t - Mod 'Iris' (iris) 1.7.0 requires version 0.15.0 or later of 'Fabric API' (fabric-api), which is missing!";
        let found: Vec<(String, String)> = p
            .fabric_missing
            .captures_iter(fabric)
            .map(|c| (c["who"].to_string(), c.name("id1").or(c.name("id2")).unwrap().as_str().to_string()))
            .collect();
        assert_eq!(found, [("Sodium Extra".into(), "sodium".into()), ("Iris".into(), "fabric-api".into())]);

        let forge = "Missing or unsupported mandatory dependencies:\n\tMod ID: 'geckolib', Requested by: 'mowziesmobs', Expected range: '[4.4,)', Actual version: '[MISSING]'";
        let c = p.forge_missing.captures(forge).unwrap();
        assert_eq!((&c["id"], &c["by"]), ("geckolib", "mowziesmobs"));

        let java = "java.lang.UnsupportedClassVersionError: net/fabricmc/x has been compiled by a more recent version of the Java Runtime (class file version 65.0), this version of the Java Runtime only recognizes class file versions up to 61.0";
        assert_eq!(&p.class_version.captures(java).unwrap()["v"], "65");

        let mixin = "org.spongepowered.asm.mixin.transformer.throwables.MixinTransformerError: An unexpected critical error was encountered\nCaused by: org.spongepowered.asm.mixin.throwables.MixinApplyError: Mixin [sodium.mixins.json:core.MixinWindow] from mod sodium failed";
        let c = p.mixin_mod.captures(mixin).unwrap();
        assert_eq!(c.name("b").or(c.name("c")).unwrap().as_str(), "sodium");

        // Fabric's "A mod crashed on startup!" window.
        let entry = "java.lang.RuntimeException: Could not execute entrypoint stage 'preLaunch' due to errors, provided by 'sodium' at 'net.caffeinemc.mods.sodium.fabric.SodiumPreLaunch'!";
        assert_eq!(&p.entrypoint.captures(entry).unwrap()["id"], "sodium");
        let entry2 = "Caused by: net.fabricmc.loader.api.EntrypointException: Exception while loading entries for entrypoint 'preLaunch' provided by 'sodium'";
        assert_eq!(&p.entrypoint.captures(entry2).unwrap()["id"], "sodium");

        let inc = "\t - Mod 'Iris' (iris) 1.7.0 is incompatible with any version of mod 'OptiFabric' (optifabric), but a matching version is present: 1.14.3!";
        let c = p.fabric_incompatible.captures(inc).unwrap();
        assert_eq!((&c["whoid"], c["other"].trim(), &c["otherid"]), ("iris", "OptiFabric", "optifabric"));
    }
}
