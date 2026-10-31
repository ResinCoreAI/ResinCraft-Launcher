//! One-click multi-core setup: the mods that move work off Minecraft's single tick thread, picked for an instance's
//! or a server's loader and Minecraft version. The tick itself stays on one thread (only Folia changes that, see
//! server/multicore.rs); these mods do chunk meshing, chunk loading and generation on other cores and lighten the tick.
//!
//! The list below is the only place that names mods. Everything else comes from Modrinth (what exists, which side it
//! runs on, what it's incompatible with) and from the mod's own fabric.mod.json (what it breaks, what it needs).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::index::{folder_of, load_index, loaders_for, natural_cmp, safe_name, ContentIndex};
use super::jarmeta::{read_jar, remote_zip_texts};
use super::modrinth;
use crate::bail_tr;
use crate::core::fsutil::{read_json, write_json};
use crate::core::instances;
use crate::core::net::{fetch_json, status_of};
use crate::core::paths;
use crate::core::types::{ContentKind, LoaderType};

const API: &str = "https://api.modrinth.com/v2";

/// A mod the setup can install: its Modrinth slug and the mod ids its jar carries.
pub struct Mod {
    slug: &'static str,
    ids: &'static [&'static str],
}

/// One job, done by the first of `mods` that exists for the target. `blocked_by` are mod ids that can't run with it.
pub struct Slot {
    key: &'static str,
    mods: &'static [Mod],
    blocked_by: &'static [&'static str],
}

/// ResinCraft's own multi-core mod, once it's on Modrinth: add it here, e.g.
/// `Mod { slug: "resincraft-multicore", ids: &["resincraft_multicore"] }`, and it's offered first wherever it has a
/// build. Nothing else needs changing; the UI needs one string, `servers.multicore.slot.own`.
const OWN_MODS: &[Mod] = &[];

const SLOTS: &[Slot] = &[
    Slot { key: "own", mods: OWN_MODS, blocked_by: &[] },
    // Client only (Modrinth says so), so servers skip this one.
    Slot {
        key: "render",
        mods: &[Mod { slug: "sodium", ids: &["sodium"] }, Mod { slug: "embeddium", ids: &["embeddium"] }],
        blocked_by: &["optifine", "optifabric", "vulkanmod", "canvas", "rubidium"],
    },
    Slot {
        key: "chunks",
        mods: &[Mod { slug: "c2me-fabric", ids: &["c2me"] }, Mod { slug: "c2me-neoforge", ids: &["c2me"] }],
        blocked_by: &["optifine", "optifabric", "tic_tacs", "dynview", "betterchunkloading"],
    },
    Slot {
        key: "logic",
        // Canary is Lithium for Forge.
        mods: &[Mod { slug: "lithium", ids: &["lithium"] }, Mod { slug: "canary", ids: &["canary"] }],
        blocked_by: &["optifine", "optifabric", "radium"],
    },
];

// ---------- What the setup would do ----------

/// What to pick mods for.
#[derive(Debug, Clone, Default)]
pub struct Target {
    /// Modrinth loader tags it can run.
    pub loaders: Vec<String>,
    pub mc: String,
    pub server: bool,
    /// Fabric itself, with this loader version (other loaders don't go through `requires`).
    pub fabric_loader: Option<String>,
    /// The Java this Minecraft version runs on.
    pub java: Option<u32>,
    /// OptiFine added through the instance's own setting (not a jar in mods/).
    pub optifine: bool,
}

/// A file in mods/.
#[derive(Debug, Clone, Default)]
pub struct Installed {
    pub file: String,
    pub on: bool,
    pub ids: Vec<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub project_id: Option<String>,
}

impl Installed {
    fn label(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.file.clone())
    }
}

/// One slot's outcome. `state`: install | have | off (there, but disabled) | conflict | oldLoader | oldJava | none.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    pub slot: &'static str,
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Only alpha or beta builds exist for this version.
    pub test: bool,
    /// What it conflicts with, or the version it needs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub with: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
}

impl PlanItem {
    fn new(slot: &'static str, state: &'static str) -> PlanItem {
        PlanItem { slot, state, title: None, version: None, test: false, with: None, project_id: None, version_id: None }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// Why nothing can be offered: vanilla | noLoader | noMods, and on servers plugins | hybrid | bedrock | folia.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<&'static str>,
    pub items: Vec<PlanItem>,
    /// The mods Undo would take away (they were added by the setup).
    pub undo: Vec<String>,
}

#[derive(Deserialize)]
struct MrProject {
    id: String,
    title: String,
    #[serde(default)]
    client_side: String,
    #[serde(default)]
    server_side: String,
}

#[derive(Deserialize, Clone)]
struct MrFile {
    url: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    size: u64,
}

#[derive(Deserialize, Clone)]
struct MrDependency {
    #[serde(default)]
    project_id: Option<String>,
    dependency_type: String,
}

#[derive(Deserialize, Clone)]
struct MrVersion {
    id: String,
    version_number: String,
    #[serde(default)]
    version_type: String,
    #[serde(default)]
    files: Vec<MrFile>,
    #[serde(default)]
    dependencies: Vec<MrDependency>,
}

/// A mod's newest build for the target.
#[derive(Debug, Clone, Default)]
struct Found {
    project_id: String,
    title: String,
    version_id: String,
    version: String,
    test: bool,
    incompatible: Vec<String>,
    rules: Rules,
}

/// What a mod's fabric.mod.json asks of the rest of the game.
#[derive(Debug, Clone, Default, PartialEq)]
struct Rules {
    /// Mod id → the version ranges it can't run with.
    breaks: Vec<(String, Vec<String>)>,
    fabric_loader: Vec<String>,
    java: Vec<String>,
}

fn strings(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => vec![s.clone()],
        Value::Array(a) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}

fn rules_of(fabric_mod_json: &str) -> Rules {
    let Ok(v) = serde_json::from_str::<Value>(fabric_mod_json.trim_start_matches('\u{feff}')) else { return Rules::default() };
    let mut breaks = Vec::new();
    for field in ["breaks", "conflicts"] {
        if let Some(map) = v.get(field).and_then(Value::as_object) {
            breaks.extend(map.iter().map(|(id, r)| (id.to_lowercase(), strings(r))));
        }
    }
    let depends = v.get("depends");
    let need = |id: &str| depends.and_then(|d| d.get(id)).map(strings).unwrap_or_default();
    Rules { breaks, fabric_loader: need("fabricloader"), java: need("java") }
}

/// Whether `version` is inside a Fabric version range. A range this can't read counts as `unknown`.
fn in_range(version: &str, range: &str, unknown: bool) -> bool {
    use std::cmp::Ordering::{Equal, Greater, Less};
    let range = range.trim();
    if range == "*" {
        return true;
    }
    let (ops, want): (&[std::cmp::Ordering], &str) = if let Some(r) = range.strip_prefix(">=") {
        (&[Greater, Equal], r)
    } else if let Some(r) = range.strip_prefix("<=") {
        (&[Less, Equal], r)
    } else if let Some(r) = range.strip_prefix('>') {
        (&[Greater], r)
    } else if let Some(r) = range.strip_prefix('<') {
        (&[Less], r)
    } else if range.starts_with(['^', '~']) {
        return unknown;
    } else {
        (&[Equal], range.strip_prefix('=').unwrap_or(range))
    };
    ops.contains(&natural_cmp(version, want.trim()))
}

fn in_any_range(version: &str, ranges: &[String], unknown: bool) -> bool {
    ranges.is_empty() || ranges.iter().any(|r| in_range(version, r, unknown))
}

/// The newest stable build (the list is newest first), else the newest of any kind: C2ME only has alphas.
fn newest(list: Vec<MrVersion>) -> Option<MrVersion> {
    let release = list.iter().position(|v| v.version_type == "release").unwrap_or(0);
    list.into_iter().nth(release)
}

async fn lookup(slug: &str, target: &Target) -> Result<Option<Found>> {
    let project: MrProject = match fetch_json(&format!("{API}/project/{slug}")).await {
        Err(e) if status_of(&e) == Some(404) => return Ok(None),
        r => r?,
    };
    let side = if target.server { &project.server_side } else { &project.client_side };
    if side == "unsupported" {
        return Ok(None);
    }
    let loaders = urlencoding::encode(&serde_json::to_string(&target.loaders)?).into_owned();
    let games = urlencoding::encode(&serde_json::to_string(&[&target.mc])?).into_owned();
    let list: Vec<MrVersion> = fetch_json(&format!("{API}/project/{slug}/version?loaders={loaders}&game_versions={games}")).await?;
    let Some(v) = newest(list) else { return Ok(None) };
    // Only a Fabric jar has these rules, and a jar that can't be read just means fewer checks.
    let mut rules = Rules::default();
    if target.loaders.iter().any(|l| l == "fabric") {
        if let Some(f) = v.files.iter().find(|f| f.primary).or(v.files.first()) {
            if let Some(text) = remote_zip_texts(&f.url, f.size, &["fabric.mod.json"]).await.and_then(|mut t| t.remove("fabric.mod.json")) {
                rules = rules_of(&text);
            }
        }
    }
    Ok(Some(Found {
        project_id: project.id,
        title: project.title,
        version_id: v.id,
        version: v.version_number,
        test: v.version_type != "release",
        incompatible: v.dependencies.iter().filter(|d| d.dependency_type == "incompatible").filter_map(|d| d.project_id.clone()).collect(),
        rules,
    }))
}

/// Decides one slot from what's installed and the build that was found.
fn judge(slot: &Slot, found: Option<&Found>, installed: &[Installed], target: &Target) -> PlanItem {
    let carries = |i: &Installed, ids: &[&str]| i.ids.iter().any(|x| ids.contains(&x.as_str()));
    let mut item = PlanItem::new(slot.key, "none");
    // Already there, under any of its mods; another of them would be a second copy of the same job.
    let have = installed.iter().filter(|i| slot.mods.iter().any(|m| carries(i, m.ids))).max_by_key(|i| i.on);
    if let Some(i) = have {
        item.state = if i.on { "have" } else { "off" };
        item.title = Some(i.label());
        item.version = i.version.clone();
        return item;
    }
    let Some(f) = found else { return item };
    item.title = Some(f.title.clone());
    item.version = Some(f.version.clone());
    item.test = f.test;
    item.project_id = Some(f.project_id.clone());
    item.version_id = Some(f.version_id.clone());

    let conflict = if target.optifine && slot.blocked_by.contains(&"optifine") {
        Some("OptiFine".to_string())
    } else {
        installed
            .iter()
            .filter(|i| i.on)
            .find(|i| {
                carries(i, slot.blocked_by)
                    || i.project_id.as_ref().is_some_and(|p| f.incompatible.contains(p))
                    || f.rules.breaks.iter().any(|(id, ranges)| i.ids.contains(id) && in_any_range(i.version.as_deref().unwrap_or(""), ranges, true))
            })
            .map(Installed::label)
    };
    if conflict.is_some() {
        item.state = "conflict";
        item.with = conflict;
    } else if target.fabric_loader.as_deref().is_some_and(|v| !in_any_range(v, &f.rules.fabric_loader, true)) {
        item.state = "oldLoader";
        item.with = Some(f.rules.fabric_loader.join(", "));
    } else if target.java.is_some_and(|j| !in_any_range(&j.to_string(), &f.rules.java, true)) {
        item.state = "oldJava";
        item.with = Some(f.rules.java.join(", "));
    } else {
        item.state = "install";
    }
    item
}

/// The setup for `target`, slot by slot. A slot nothing can fill for this side (Sodium on a server) is left out when
/// its mods exist but not for this side; `none` means there's simply no build for this version.
pub async fn items_for(target: &Target, installed: &[Installed]) -> Result<Vec<PlanItem>> {
    let mut out = Vec::new();
    for slot in SLOTS {
        if slot.mods.is_empty() || (target.server && slot.key == "render") {
            continue;
        }
        let mut found = None;
        for m in slot.mods {
            found = lookup(m.slug, target).await?;
            if found.is_some() {
                break;
            }
        }
        out.push(judge(slot, found.as_ref(), installed, target));
    }
    Ok(out)
}

/// Why a plan can't do anything, when that's the case.
pub fn nothing_to_offer(items: &[PlanItem]) -> Option<&'static str> {
    items.iter().all(|i| i.state == "none").then_some("noMods")
}

/// The jars in a mods folder with what they say about themselves. `known` is (file, project id) from the content index.
pub async fn scan(folder: &Path, known: Vec<(String, String)>) -> Vec<Installed> {
    let folder = folder.to_path_buf();
    let known: HashMap<String, String> = known.into_iter().collect();
    tokio::task::spawn_blocking(move || {
        let Ok(rd) = std::fs::read_dir(&folder) else { return Vec::new() };
        let mut out = Vec::new();
        for e in rd.flatten() {
            let file = e.file_name().to_string_lossy().into_owned();
            let lower = file.to_lowercase();
            let on = lower.ends_with(".jar");
            if !on && !lower.ends_with(".jar.disabled") {
                continue;
            }
            let meta = read_jar(&e.path());
            let mut ids = meta.ids;
            // OptiFine has no metadata file of its own.
            if lower.starts_with("optifine") && !ids.iter().any(|i| i == "optifine") {
                ids.push("optifine".into());
            }
            out.push(Installed { project_id: known.get(&file).cloned(), file, on, ids, name: meta.name, version: meta.version });
        }
        out
    })
    .await
    .unwrap_or_default()
}

// ---------- Undo ----------

/// `multicore.json`: what the setup added, so Undo takes away those files and nothing else.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Record {
    pub added: Vec<Added>,
    /// The folder's files before the setup, to see what came later.
    pub before: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Added {
    pub file: String,
    pub project_id: String,
    pub title: String,
    /// A mod of the setup itself, not a dependency pulled in for one.
    pub root: bool,
}

pub async fn load_record(file: &Path) -> Record {
    read_json(file).await.unwrap_or_default()
}

pub async fn save_record(file: &Path, record: &Record) -> Result<()> {
    if record.added.is_empty() {
        let _ = tokio::fs::remove_file(file).await;
        return Ok(());
    }
    write_json(file, record).await
}

/// Adds what appeared in the folder since `before` (file, project id, title) to `record`. Files `roots` doesn't name
/// are dependencies. Works after a partly failed install too: what did arrive can be undone.
pub fn remember(record: &mut Record, before: &[(String, String, String)], after: &[(String, String, String)], roots: &HashSet<String>) {
    if record.added.is_empty() {
        record.before = before.iter().map(|b| b.0.clone()).collect();
    }
    for (file, project_id, title) in after {
        if !before.iter().any(|b| &b.0 == file) && !record.added.iter().any(|a| &a.file == file) {
            record.added.push(Added { file: file.clone(), project_id: project_id.clone(), title: title.clone(), root: roots.contains(project_id) });
        }
    }
}

/// What's still there of what the setup added, as (file now, title). An updated mod is found by its project id.
/// Dependencies go too, unless mods have been added since: those may be using them.
pub fn to_remove(record: &Record, now: &[(String, String, String)]) -> Vec<(String, String)> {
    let same = |a: &str, b: &str| a.strip_suffix(".disabled").unwrap_or(a) == b.strip_suffix(".disabled").unwrap_or(b);
    let added_since = now.iter().any(|(file, project_id, _)| {
        !record.before.iter().any(|b| same(b, file)) && !record.added.iter().any(|a| same(&a.file, file) || (!a.project_id.is_empty() && &a.project_id == project_id))
    });
    let mut out = Vec::new();
    for a in record.added.iter().filter(|a| a.root || !added_since) {
        for (file, project_id, _) in now {
            if same(&a.file, file) || (!a.project_id.is_empty() && &a.project_id == project_id) {
                out.push((file.clone(), a.title.clone()));
            }
        }
    }
    out.dedup();
    out
}

// ---------- Instances ----------

fn record_file(id: &str) -> PathBuf {
    paths::instance_dir(id).join("multicore.json")
}

async fn listed(id: &str) -> Result<Vec<(String, String, String)>> {
    let items = modrinth::list_content(id, ContentKind::Mod).await?;
    Ok(items.into_iter().map(|i| (i.title.clone().unwrap_or_else(|| i.filename.clone()), i)).map(|(t, i)| (i.filename, i.project_id.unwrap_or_default(), t)).collect())
}

fn index_pairs(idx: &ContentIndex) -> Vec<(String, String)> {
    idx.iter().filter_map(|(k, e)| k.strip_prefix("mod/").map(|f| (f.to_string(), e.project_id.clone()))).flat_map(|(f, p)| [(format!("{f}.disabled"), p.clone()), (f, p)]).collect()
}

pub async fn instance_plan(id: &str) -> Result<Plan> {
    let inst = instances::get(id).await?;
    let record = load_record(&record_file(id)).await;
    let mut plan = Plan::default();
    if !record.added.is_empty() {
        plan.undo = to_remove(&record, &listed(id).await?).into_iter().map(|r| r.1).collect();
    }
    match inst.loader {
        LoaderType::Vanilla => {
            plan.reason = Some("vanilla");
            return Ok(plan);
        }
        LoaderType::Liteloader => {
            plan.reason = Some("noLoader");
            return Ok(plan);
        }
        _ => {}
    }
    // Names the jars dropped in by hand first, so their Modrinth incompatibilities count.
    listed(id).await.ok();
    let installed = scan(&folder_of(&inst, ContentKind::Mod), index_pairs(&load_index(id).await)).await;
    let target = Target {
        loaders: loaders_for(&inst),
        mc: inst.mc_version.clone(),
        server: false,
        fabric_loader: (inst.loader == LoaderType::Fabric).then(|| inst.loader_version.clone()).flatten(),
        java: crate::server::installers::required_java(&inst.mc_version).await.ok(),
        optifine: inst.optifine.is_some(),
    };
    plan.items = items_for(&target, &installed).await?;
    plan.reason = nothing_to_offer(&plan.items);
    Ok(plan)
}

pub async fn instance_apply(id: &str) -> Result<Plan> {
    if crate::game::running_ids().iter().any(|r| r == id) {
        bail_tr!("multicore.gameRunning", "Close the game first: mods can't change while it's running.");
    }
    let plan = instance_plan(id).await?;
    let picks: Vec<&PlanItem> = plan.items.iter().filter(|i| i.state == "install").collect();
    let before = listed(id).await?;
    let mut failed = None;
    for item in &picks {
        let (Some(project), Some(version)) = (&item.project_id, &item.version_id) else { continue };
        if let Err(e) = modrinth::install(id, project, ContentKind::Mod, Some(version.clone())).await {
            failed = Some(e);
            break;
        }
    }
    let file = record_file(id);
    let mut record = load_record(&file).await;
    let roots: HashSet<String> = picks.iter().filter_map(|i| i.project_id.clone()).collect();
    remember(&mut record, &before, &listed(id).await?, &roots);
    save_record(&file, &record).await?;
    match failed {
        Some(e) => Err(e),
        None => instance_plan(id).await,
    }
}

pub async fn instance_undo(id: &str) -> Result<Plan> {
    if crate::game::running_ids().iter().any(|r| r == id) {
        bail_tr!("multicore.gameRunning", "Close the game first: mods can't change while it's running.");
    }
    let file = record_file(id);
    let record = load_record(&file).await;
    for (name, _) in to_remove(&record, &listed(id).await?) {
        modrinth::remove_content(id, ContentKind::Mod, safe_name(&name)?).await?;
    }
    let _ = tokio::fs::remove_file(&file).await;
    instance_plan(id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fabric() -> Target {
        Target { loaders: vec!["fabric".into()], mc: "1.21.1".into(), fabric_loader: Some("0.18.4".into()), java: Some(21), ..Default::default() }
    }
    fn jar(id: &str, version: &str) -> Installed {
        Installed { file: format!("{id}.jar"), on: true, ids: vec![id.into()], version: Some(version.into()), ..Default::default() }
    }
    fn found() -> Found {
        Found { project_id: "P".into(), title: "Mod".into(), version_id: "V".into(), version: "1".into(), ..Default::default() }
    }
    fn slot(key: &str) -> &'static Slot {
        SLOTS.iter().find(|s| s.key == key).unwrap()
    }
    fn file(f: &str, p: &str) -> (String, String, String) {
        (f.into(), p.into(), f.into())
    }

    #[test]
    fn a_free_slot_installs() {
        let item = judge(slot("render"), Some(&found()), &[], &fabric());
        assert_eq!((item.state, item.version_id.as_deref()), ("install", Some("V")));
    }

    #[test]
    fn what_is_there_is_never_installed_twice() {
        assert_eq!(judge(slot("render"), Some(&found()), &[jar("sodium", "0.6")], &fabric()).state, "have");
        // Embeddium does Sodium's job: no second renderer.
        assert_eq!(judge(slot("render"), Some(&found()), &[jar("embeddium", "1")], &fabric()).state, "have");
        let mut off = jar("lithium", "1");
        off.on = false;
        assert_eq!(judge(slot("logic"), Some(&found()), &[off], &fabric()).state, "off");
        // Even with nothing to download for this version.
        assert_eq!(judge(slot("logic"), None, &[jar("canary", "1")], &fabric()).state, "have");
    }

    #[test]
    fn conflicts_are_skipped_and_named() {
        let mut vulkan = jar("vulkanmod", "1");
        vulkan.name = Some("VulkanMod".into());
        let item = judge(slot("render"), Some(&found()), &[vulkan], &fabric());
        assert_eq!((item.state, item.with.as_deref()), ("conflict", Some("VulkanMod")));
        // OptiFine set on the instance, no jar in mods/.
        let t = Target { optifine: true, ..fabric() };
        assert_eq!(judge(slot("chunks"), Some(&found()), &[], &t).with.as_deref(), Some("OptiFine"));
        // Modrinth's incompatible list goes by project.
        let f = Found { incompatible: vec!["X".into()], ..found() };
        let mut other = jar("other", "1");
        other.project_id = Some("X".into());
        assert_eq!(judge(slot("render"), Some(&f), &[other], &fabric()).state, "conflict");
        // A disabled mod can't clash.
        let mut off = jar("vulkanmod", "1");
        off.on = false;
        assert_eq!(judge(slot("render"), Some(&found()), &[off], &fabric()).state, "install");
    }

    #[test]
    fn a_mods_own_breaks_list_counts_versions() {
        let rules = rules_of(r#"{"breaks":{"iris":"<1.8.13","embeddium":"*"},"conflicts":{"dynview":"*"},"depends":{"fabricloader":">=0.18.3","java":">=21"}}"#);
        let f = Found { rules, ..found() };
        assert_eq!(judge(slot("logic"), Some(&f), &[jar("iris", "1.8.0")], &fabric()).state, "conflict");
        assert_eq!(judge(slot("logic"), Some(&f), &[jar("iris", "1.8.13+mc1.21.1")], &fabric()).state, "install");
        assert_eq!(judge(slot("logic"), Some(&f), &[jar("dynview", "1")], &fabric()).state, "conflict");
    }

    #[test]
    fn old_loaders_and_java_are_refused_rather_than_crash() {
        let f = Found { rules: rules_of(r#"{"depends":{"fabricloader":">=0.18.3","java":">=21"}}"#), ..found() };
        let old = Target { fabric_loader: Some("0.16.9".into()), ..fabric() };
        let item = judge(slot("chunks"), Some(&f), &[], &old);
        assert_eq!((item.state, item.with.as_deref()), ("oldLoader", Some(">=0.18.3")));
        let java17 = Target { java: Some(17), ..fabric() };
        assert_eq!(judge(slot("chunks"), Some(&f), &[], &java17).state, "oldJava");
        assert_eq!(judge(slot("chunks"), Some(&f), &[], &fabric()).state, "install");
        // A range nobody can read doesn't block.
        let g = Found { rules: rules_of(r#"{"depends":{"fabricloader":"^0.18"}}"#), ..found() };
        assert_eq!(judge(slot("chunks"), Some(&g), &[], &fabric()).state, "install");
    }

    #[test]
    fn prefers_a_release_but_takes_an_alpha() {
        let v = |id: &str, kind: &str| MrVersion { id: id.into(), version_number: id.into(), version_type: kind.into(), files: vec![], dependencies: vec![] };
        assert_eq!(newest(vec![v("a", "beta"), v("b", "release"), v("c", "release")]).unwrap().id, "b");
        assert_eq!(newest(vec![v("a", "alpha"), v("b", "alpha")]).unwrap().id, "a");
        assert!(newest(vec![]).is_none());
    }

    #[test]
    fn nothing_available_is_said_once() {
        assert_eq!(nothing_to_offer(&[PlanItem::new("render", "none"), PlanItem::new("logic", "none")]), Some("noMods"));
        assert_eq!(nothing_to_offer(&[PlanItem::new("render", "none"), PlanItem::new("logic", "have")]), None);
    }

    #[test]
    fn undo_takes_exactly_what_the_setup_added() {
        let mut r = Record::default();
        let before = [file("mine.jar", "M")];
        let after = [file("mine.jar", "M"), file("sodium.jar", "S"), file("api.jar", "D")];
        remember(&mut r, &before, &after, &["S".to_string()].into_iter().collect());
        assert_eq!(r.before, ["mine.jar"]);
        assert_eq!(r.added.len(), 2);
        assert_eq!(to_remove(&r, &after), [("sodium.jar".to_string(), "sodium.jar".to_string()), ("api.jar".to_string(), "api.jar".to_string())]);
    }

    #[test]
    fn undo_follows_updates_and_disabled_files() {
        let mut r = Record::default();
        remember(&mut r, &[], &[file("sodium-1.jar", "S")], &["S".to_string()].into_iter().collect());
        assert_eq!(to_remove(&r, &[file("sodium-2.jar", "S")])[0].0, "sodium-2.jar");
        assert_eq!(to_remove(&r, &[file("sodium-1.jar.disabled", "")])[0].0, "sodium-1.jar.disabled");
        assert!(to_remove(&r, &[]).is_empty());
    }

    #[test]
    fn undo_keeps_dependencies_newer_mods_may_use() {
        let mut r = Record::default();
        let after = [file("sodium.jar", "S"), file("api.jar", "D")];
        remember(&mut r, &[], &after, &["S".to_string()].into_iter().collect());
        let later = [file("sodium.jar", "S"), file("api.jar", "D"), file("new.jar", "N")];
        assert_eq!(to_remove(&r, &later).len(), 1);
        assert_eq!(to_remove(&r, &later)[0].0, "sodium.jar");
    }

    #[test]
    fn a_second_run_keeps_the_first_snapshot() {
        let roots = ["S".to_string(), "L".to_string()].into_iter().collect();
        let mut r = Record::default();
        remember(&mut r, &[file("mine.jar", "M")], &[file("mine.jar", "M"), file("sodium.jar", "S")], &roots);
        remember(&mut r, &[file("mine.jar", "M"), file("sodium.jar", "S")], &[file("mine.jar", "M"), file("sodium.jar", "S"), file("lithium.jar", "L")], &roots);
        assert_eq!(r.before, ["mine.jar"]);
        assert_eq!(r.added.iter().map(|a| a.file.as_str()).collect::<Vec<_>>(), ["sodium.jar", "lithium.jar"]);
    }
}
