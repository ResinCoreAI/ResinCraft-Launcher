//! Finds the required mods and plugins a folder doesn't have: jars added by hand, or a dependency removed later.
//! Installing from a site already pulls in dependencies; this reads what is really in the folder.
//!
//! Each jar is read as its loader reads it, so a jar built for several loaders only counts its own metadata.
//! Only ids are compared: a dependency that's there in the wrong version isn't reported.
//!
//! The fix looks each missing id up on Modrinth by exact slug or id, then reads the candidate's jar and installs it
//! only if that jar really declares the id. Anything else is reported by name.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::future::Future;
use std::io::{Cursor, Read, Seek};
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::index::{folder_of, loaders_for, open_zip, read_zip_text};
use super::jarmeta::{json_of, parse_mcmod, remote_zip_texts, toml_string, usable};
use super::modrinth;
use crate::core::instances;
use crate::core::net::{fetch_json, status_of};
use crate::core::types::{ContentKind, LoaderType, ServerKind};
use crate::server::plugins;

const API: &str = "https://api.modrinth.com/v2";
/// How deep jars inside jars are followed, and the largest nested jar read into memory.
const MAX_DEPTH: usize = 4;
const MAX_NESTED: u64 = 64 << 20;

/// Which metadata a folder's jars are read by.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Family {
    Fabric,
    Quilt,
    Forge,
    Neoforge,
    Bukkit,
    Paper,
}

impl Family {
    /// The files that loader reads, first found wins. mcmod.info (old Forge) names mods but not their dependencies.
    fn files(self) -> &'static [&'static str] {
        match self {
            Family::Fabric => &["fabric.mod.json"],
            Family::Quilt => &["quilt.mod.json", "fabric.mod.json"],
            Family::Forge => &["META-INF/mods.toml", "mcmod.info"],
            Family::Neoforge => &["META-INF/neoforge.mods.toml", "META-INF/mods.toml"],
            Family::Bukkit => &["plugin.yml"],
            Family::Paper => &["paper-plugin.yml", "plugin.yml"],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Side {
    Client,
    Server,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    pub family: Family,
    pub side: Side,
}

/// What a jar's metadata says.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Parsed {
    /// Ids this jar answers to: its own, every `provides`, and those of the jars nested in it.
    pub ids: Vec<String>,
    pub name: Option<String>,
    /// Required ids, nested jars' included.
    pub needs: Vec<String>,
    /// Nested jars named in the metadata.
    pub nested: Vec<String>,
    /// Made for the other side (a client-only mod on a server): the loader skips the jar.
    pub skipped: bool,
}

fn clean_id(id: &str) -> Option<String> {
    let id = id.trim().to_lowercase();
    usable(&id).then_some(id)
}

impl Parsed {
    fn add_id(&mut self, id: &str) {
        if let Some(id) = clean_id(id).filter(|i| !self.ids.contains(i)) {
            self.ids.push(id);
        }
    }
    fn need(&mut self, id: &str) {
        if let Some(id) = clean_id(id).filter(|i| !self.needs.contains(i)) {
            self.needs.push(id);
        }
    }
    fn merge(&mut self, other: Parsed) {
        other.ids.iter().for_each(|i| self.add_id(i));
        other.needs.iter().for_each(|i| self.need(i));
    }
}

/// Ids the game or the loader answers to itself, never a separate jar.
fn built_in(id: &str) -> bool {
    matches!(id, "minecraft" | "java" | "fabricloader" | "fabric-loader" | "quilt_loader" | "forge" | "neoforge" | "fml" | "javafml" | "mixinextras")
}

// ---------- Metadata formats ----------

/// JSON as Fabric's lenient reader takes it: trailing commas and raw line breaks are common in real jars.
fn lenient_json(text: &str) -> Option<Value> {
    json_of(text).or_else(|| {
        let flat = text.replace(['\r', '\n', '\t'], " ");
        let re = regex::Regex::new(r",\s*([}\]])").ok()?;
        json_of(&re.replace_all(&flat, "$1"))
    })
}

fn strings<'a>(v: Option<&'a Value>) -> impl Iterator<Item = &'a str> {
    v.and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str)
}

/// fabric.mod.json: `id`, `provides`, the keys of `depends`, `jars[].file` and `environment`.
pub fn parse_fabric(text: &str, side: Side) -> Option<Parsed> {
    let v = lenient_json(text)?;
    let mut p = Parsed { name: v["name"].as_str().map(str::to_string), ..Default::default() };
    p.add_id(v["id"].as_str()?);
    strings(v.get("provides")).for_each(|i| p.add_id(i));
    v["depends"].as_object().into_iter().flatten().for_each(|(k, _)| p.need(k));
    p.nested = v["jars"].as_array().into_iter().flatten().filter_map(|j| j["file"].as_str().map(str::to_string)).collect();
    p.skipped = matches!((v["environment"].as_str(), side), (Some("client"), Side::Server) | (Some("server"), Side::Client));
    Some(p)
}

/// Quilt names mods `id` or `group:id`, as a string or `{ "id": ... }`.
fn quilt_id(v: &Value) -> Option<&str> {
    let id = v.as_str().or_else(|| v["id"].as_str())?;
    id.rsplit(':').next()
}

/// quilt.mod.json: under `quilt_loader`, `id`, `provides`, `depends` and `jars`. Optional dependencies, ones with an
/// `unless` and any-of lists (a nested array) are choices, so none of them count as required.
pub fn parse_quilt(text: &str, side: Side) -> Option<Parsed> {
    let v = lenient_json(text)?;
    let q = v.get("quilt_loader")?;
    let mut p = Parsed { name: q["metadata"]["name"].as_str().map(str::to_string), ..Default::default() };
    p.add_id(quilt_id(q)?);
    q["provides"].as_array().into_iter().flatten().filter_map(quilt_id).for_each(|i| p.add_id(i));
    for d in q["depends"].as_array().into_iter().flatten() {
        if d.is_array() || d["optional"].as_bool() == Some(true) || d.get("unless").is_some() {
            continue;
        }
        if let Some(id) = quilt_id(d) {
            p.need(id);
        }
    }
    p.nested = strings(q.get("jars")).map(str::to_string).collect();
    p.skipped = matches!((v["minecraft"]["environment"].as_str(), side), (Some("client"), Side::Server) | (Some("dedicated_server"), Side::Client));
    Some(p)
}

/// One `[[dependencies.x]]` table of a mods.toml.
#[derive(Default)]
struct TomlDep {
    id: Option<String>,
    mandatory: Option<bool>,
    kind: Option<String>,
    side: Option<String>,
}

impl TomlDep {
    /// Forge says `mandatory`, NeoForge `type` (required unless it says otherwise).
    fn needed(&self, neoforge: bool, side: Side) -> bool {
        let required = match (&self.kind, self.mandatory) {
            (Some(k), _) => k.eq_ignore_ascii_case("required"),
            (None, Some(m)) => m,
            (None, None) => neoforge,
        };
        let here = match (self.side.as_deref().map(str::to_uppercase).as_deref(), side) {
            (Some("CLIENT"), Side::Server) | (Some("SERVER"), Side::Client) => false,
            _ => true,
        };
        required && here
    }
}

fn toml_list(v: &str) -> Vec<String> {
    let re = regex::Regex::new(r#"["']([^"']+)["']"#).expect("valid pattern");
    re.captures_iter(v).map(|c| c[1].to_string()).collect()
}

/// mods.toml / neoforge.mods.toml: every `[[mods]]` (its `modId` and `provides`) and `[[dependencies.x]]` table.
pub fn parse_mods_toml(text: &str, neoforge: bool, side: Side) -> Option<Parsed> {
    #[derive(PartialEq)]
    enum Table {
        Mods,
        Dep,
        Other,
    }
    let mut p = Parsed::default();
    let mut table = Table::Other;
    let mut dep = TomlDep::default();
    let mut deps = Vec::new();
    for raw in text.trim_start_matches('\u{feff}').lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            let header: String = line.split('#').next().unwrap_or_default().chars().filter(|c| !c.is_whitespace()).collect();
            deps.push(std::mem::take(&mut dep));
            table = if header == "[[mods]]" {
                Table::Mods
            } else if header.starts_with("[[dependencies.") {
                Table::Dep
            } else {
                Table::Other
            };
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let key = key.trim().trim_matches('"');
        match (&table, key) {
            (Table::Mods, "modId") => {
                if let Some(id) = toml_string(value) {
                    p.add_id(&id);
                }
            }
            (Table::Mods, "displayName") if p.name.is_none() => p.name = toml_string(value),
            (Table::Mods, "provides") => toml_list(value).iter().for_each(|i| p.add_id(i)),
            (Table::Dep, "modId") => dep.id = toml_string(value),
            (Table::Dep, "mandatory") => dep.mandatory = value.split('#').next().map(|b| b.trim() == "true"),
            (Table::Dep, "type") => dep.kind = toml_string(value),
            (Table::Dep, "side") => dep.side = toml_string(value),
            _ => {}
        }
    }
    deps.push(dep);
    for d in deps.iter().filter(|d| d.needed(neoforge, side)) {
        if let Some(id) = &d.id {
            p.need(id);
        }
    }
    (!p.ids.is_empty()).then_some(p)
}

fn yaml_scalar(v: &str) -> &str {
    let v = v.trim();
    let v = if v.starts_with(['"', '\'']) { v } else { v.split(" #").next().unwrap_or(v).trim_end() };
    v.trim_matches(['"', '\''])
}

/// A top-level key's value lines: the text after the colon, then the indented or `- ` lines under it.
fn yaml_block<'a>(text: &'a str, key: &str) -> Option<(&'a str, Vec<&'a str>)> {
    let mut lines = text.trim_start_matches('\u{feff}').lines();
    let first = lines.find_map(|l| l.strip_prefix(key)?.trim_start().strip_prefix(':'))?;
    let block = lines
        .take_while(|l| l.trim().is_empty() || l.starts_with([' ', '\t', '-', '#']))
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .collect();
    Some((first.trim(), block))
}

/// A plugin.yml list: `depend: [A, B]`, a `- A` list, or one name.
fn yaml_list(text: &str, key: &str) -> Vec<String> {
    let Some((first, block)) = yaml_block(text, key) else { return Vec::new() };
    if first.starts_with('[') {
        // A flow list may run over several lines.
        let all = std::iter::once(first).chain(block).collect::<Vec<_>>().join(" ");
        let inner = all.trim_start_matches('[').split(']').next().unwrap_or_default();
        return inner.split(',').map(yaml_scalar).filter(|s| !s.is_empty()).map(str::to_string).collect();
    }
    if !yaml_scalar(first).is_empty() {
        return vec![yaml_scalar(first).to_string()];
    }
    block.iter().filter_map(|l| l.trim_start().strip_prefix('-')).map(yaml_scalar).filter(|s| !s.is_empty()).map(str::to_string).collect()
}

/// paper-plugin.yml `dependencies`, required unless they say `required: false`: `bootstrap:` and `server:` maps of
/// plugin names, or the older list of `- name: X`.
fn paper_deps(text: &str) -> Vec<String> {
    let Some((_, block)) = yaml_block(text, "dependencies") else { return Vec::new() };
    let mut out: Vec<(String, bool)> = Vec::new();
    for line in block {
        let t = line.trim();
        if t == "bootstrap:" || t == "server:" {
            continue;
        }
        if let Some(name) = t.strip_prefix('-').map(str::trim_start).and_then(|r| r.strip_prefix("name:")) {
            out.push((yaml_scalar(name).to_string(), true));
        } else if let Some(name) = t.strip_suffix(':') {
            out.push((yaml_scalar(name).to_string(), true));
        } else if let (Some(last), Some(v)) = (out.last_mut(), t.strip_prefix('-').unwrap_or(t).trim_start().strip_prefix("required:")) {
            last.1 = yaml_scalar(v) != "false";
        }
    }
    out.into_iter().filter(|(_, required)| *required).map(|(n, _)| n).collect()
}

/// plugin.yml / paper-plugin.yml. Bukkit turns spaces in plugin names into underscores.
pub fn parse_plugin_yml(text: &str, paper: bool) -> Option<Parsed> {
    let name = yaml_block(text, "name").map(|(n, _)| yaml_scalar(n).to_string()).filter(|n| !n.is_empty())?;
    let mut p = Parsed::default();
    p.add_id(&name.replace(' ', "_"));
    p.name = Some(name);
    yaml_list(text, "provides").iter().for_each(|i| p.add_id(&i.replace(' ', "_")));
    let needs = if paper { paper_deps(text) } else { yaml_list(text, "depend") };
    needs.iter().for_each(|i| p.need(&i.replace(' ', "_")));
    Some(p)
}

fn parse_file(file: &str, text: &str, t: Target) -> Option<Parsed> {
    match file {
        "fabric.mod.json" => parse_fabric(text, t.side),
        "quilt.mod.json" => parse_quilt(text, t.side),
        "paper-plugin.yml" => parse_plugin_yml(text, true),
        "plugin.yml" => parse_plugin_yml(text, false),
        "mcmod.info" => {
            let (id, name, _) = parse_mcmod(text);
            let mut p = Parsed { name, ..Default::default() };
            p.add_id(&id?);
            Some(p)
        }
        _ => parse_mods_toml(text, file.contains("neoforge"), t.side),
    }
}

// ---------- Jars ----------

fn in_nested_folder(name: &str) -> bool {
    (name.starts_with("META-INF/jars/") || name.starts_with("META-INF/jarjar/")) && name.ends_with(".jar")
}

fn read_nested<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, name: &str) -> Option<Vec<u8>> {
    let entry = zip.by_name(name).ok()?;
    if entry.size() > MAX_NESTED {
        return None;
    }
    let mut all = Vec::with_capacity(entry.size() as usize);
    entry.take(MAX_NESTED).read_to_end(&mut all).ok()?;
    Some(all)
}

/// A jar's metadata and that of the jars nested in it. None when it has none for this loader.
pub fn read_jar<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, t: Target, depth: usize) -> Option<Parsed> {
    let mut p = t.family.files().iter().find_map(|f| parse_file(f, &read_zip_text(zip, f).ok().flatten()?, t))?;
    if depth >= MAX_DEPTH {
        return Some(p);
    }
    let mut nested = std::mem::take(&mut p.nested);
    nested.extend(zip.file_names().filter(|n| in_nested_folder(n)).map(str::to_string));
    nested.sort();
    nested.dedup();
    for name in nested {
        let Some(bytes) = read_nested(zip, &name) else { continue };
        let Ok(mut inner) = zip::ZipArchive::new(Cursor::new(bytes)) else { continue };
        if let Some(n) = read_jar(&mut inner, t, depth + 1).filter(|n| !n.skipped) {
            p.merge(n);
        }
    }
    Some(p)
}

/// One jar in the folder. `parsed` is None when nothing in it could be read for this loader.
pub struct Scanned {
    pub filename: String,
    pub enabled: bool,
    pub parsed: Option<Parsed>,
}

/// Every jar in a folder, turned off or not (blocking: call from spawn_blocking).
fn scan(folder: &Path, t: Target) -> Vec<Scanned> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(folder).into_iter().flatten().flatten() {
        let filename = e.file_name().to_string_lossy().into_owned();
        let lower = filename.to_lowercase();
        let enabled = lower.ends_with(".jar");
        if (enabled || lower.ends_with(".jar.disabled")) && e.path().is_file() {
            let parsed = open_zip(&e.path()).ok().and_then(|mut z| read_jar(&mut z, t, 0));
            out.push(Scanned { filename, enabled, parsed });
        }
    }
    out.sort_by(|a, b| a.filename.cmp(&b.filename));
    out
}

// ---------- The check ----------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingDep {
    pub id: String,
    /// The mods that need it, by name.
    pub required_by: Vec<String>,
    /// The jar that has it but is turned off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DepReport {
    /// Enabled jars that were read.
    pub checked: usize,
    pub missing: Vec<MissingDep>,
    /// Enabled jars with no metadata for this loader: libraries, jars for another loader, damaged files.
    pub unread: Vec<String>,
}

pub fn report(jars: &[Scanned]) -> DepReport {
    let mut out = DepReport::default();
    let loaded: Vec<(&Scanned, &Parsed)> = jars.iter().filter(|j| j.enabled).filter_map(|j| Some((j, j.parsed.as_ref()?))).filter(|(_, p)| !p.skipped).collect();
    let have: HashSet<&str> = loaded.iter().flat_map(|(_, p)| p.ids.iter().map(String::as_str)).collect();
    let mut missing: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for (j, p) in &loaded {
        for need in p.needs.iter().filter(|n| !built_in(n) && !have.contains(n.as_str())) {
            let by = p.name.clone().filter(|n| usable(n)).unwrap_or_else(|| j.filename.clone());
            let list = missing.entry(need).or_default();
            if !list.contains(&by) {
                list.push(by);
            }
        }
    }
    out.checked = jars.iter().filter(|j| j.enabled && j.parsed.is_some()).count();
    out.unread = jars.iter().filter(|j| j.enabled && j.parsed.is_none()).map(|j| j.filename.clone()).collect();
    out.missing = missing
        .into_iter()
        .map(|(id, required_by)| {
            let off = jars.iter().find(|j| !j.enabled && j.parsed.as_ref().is_some_and(|p| p.ids.iter().any(|i| i == id)));
            MissingDep { id: id.to_string(), required_by, disabled: off.map(|j| j.filename.clone()) }
        })
        .collect();
    out
}

// ---------- A game instance or a server's area ----------

struct Scope {
    target: Target,
    folder: PathBuf,
    loaders: Vec<String>,
    mc: String,
}

async fn instance_scope(id: &str) -> Result<Scope> {
    let inst = instances::get(id).await?;
    let family = match inst.loader {
        LoaderType::Fabric => Family::Fabric,
        LoaderType::Quilt => Family::Quilt,
        LoaderType::Forge => Family::Forge,
        LoaderType::Neoforge => Family::Neoforge,
        _ => bail!("{} has no mod loader, so it has no mods to check.", inst.name),
    };
    Ok(Scope { target: Target { family, side: Side::Client }, folder: folder_of(&inst, ContentKind::Mod), loaders: loaders_for(&inst), mc: inst.mc_version })
}

async fn server_scope(id: &str, area: Option<&str>) -> Result<Scope> {
    let (kind, folder, loaders, mc) = plugins::dep_scope(id, area).await?;
    let family = match kind {
        ServerKind::Fabric => Family::Fabric,
        ServerKind::Quilt => Family::Quilt,
        ServerKind::Forge => Family::Forge,
        ServerKind::Neoforge => Family::Neoforge,
        ServerKind::Paper | ServerKind::Purpur | ServerKind::Folia | ServerKind::Leaf | ServerKind::Pufferfish => Family::Paper,
        ServerKind::Spigot | ServerKind::Craftbukkit => Family::Bukkit,
        k => bail!("The dependency check doesn't read {} plugins yet.", k.label()),
    };
    Ok(Scope { target: Target { family, side: Side::Server }, folder, loaders: loaders.into_iter().map(str::to_string).collect(), mc })
}

async fn check(scope: &Scope) -> Result<DepReport> {
    let (folder, target) = (scope.folder.clone(), scope.target);
    Ok(report(&tokio::task::spawn_blocking(move || scan(&folder, target)).await?))
}

pub async fn check_instance(id: &str) -> Result<DepReport> {
    check(&instance_scope(id).await?).await
}

pub async fn check_server(id: &str, area: Option<&str>) -> Result<DepReport> {
    check(&server_scope(id, area).await?).await
}

// ---------- The one-click fix ----------

#[derive(Deserialize)]
struct MrProject {
    id: String,
    #[serde(default)]
    slug: String,
    title: String,
    #[serde(default)]
    project_type: String,
}

#[derive(Deserialize)]
struct MrFile {
    url: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    size: u64,
}

#[derive(Deserialize)]
struct MrVersion {
    id: String,
    #[serde(default)]
    version_type: String,
    #[serde(default)]
    files: Vec<MrFile>,
}

/// Why a missing id wasn't installed.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NotFound {
    /// No Modrinth project has exactly this slug or id.
    NotOnModrinth,
    /// It has, but no version for this loader and game version.
    NoVersion,
    /// It has, but its jar doesn't declare this id, so it may be another mod.
    NotSameMod,
    /// A jar in the folder has it, turned off: installing would make a second copy.
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DepNotFound {
    pub id: String,
    pub reason: NotFound,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DepFailed {
    pub id: String,
    pub error: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DepInstallResult {
    /// Project names.
    pub installed: Vec<String>,
    pub not_found: Vec<DepNotFound>,
    pub failed: Vec<DepFailed>,
}

/// The project a slug-or-id lookup returned, if it is exactly that id: Modrinth also answers to old slugs.
fn exact(p: &MrProject, id: &str) -> bool {
    (p.slug.eq_ignore_ascii_case(id) || p.id.eq_ignore_ascii_case(id)) && matches!(p.project_type.as_str(), "mod" | "plugin")
}

/// The file an install of this version downloads, as `install_version` picks it.
fn file_of(v: &MrVersion) -> Option<&MrFile> {
    v.files.iter().find(|f| f.primary).or(v.files.first())
}

/// The newest release, or the newest of anything when there are only betas (Modrinth lists newest first).
fn pick(list: &[MrVersion]) -> Option<&MrVersion> {
    list.iter().filter(|v| file_of(v).is_some()).find(|v| v.version_type == "release").or_else(|| list.iter().find(|v| file_of(v).is_some()))
}

/// Whether a jar's metadata files declare `id`, read as this loader would.
fn declares(texts: &BTreeMap<&'static str, String>, id: &str, t: Target) -> bool {
    t.family.files().iter().find_map(|f| parse_file(f, texts.get(f)?, t)).is_some_and(|p| p.ids.iter().any(|i| i == id))
}

/// The project and version to install for `id`, or why there is none.
async fn resolve(id: &str, scope: &Scope) -> Result<std::result::Result<(MrProject, String), NotFound>> {
    let enc = |s: &str| urlencoding::encode(s).into_owned();
    let project: MrProject = match fetch_json(&format!("{API}/project/{}", enc(id))).await {
        Ok(p) => p,
        Err(e) if status_of(&e) == Some(404) => return Ok(Err(NotFound::NotOnModrinth)),
        Err(e) => return Err(e),
    };
    if !exact(&project, id) {
        return Ok(Err(NotFound::NotOnModrinth));
    }
    let url = format!(
        "{API}/project/{}/version?loaders={}&game_versions={}",
        enc(&project.id),
        enc(&serde_json::to_string(&scope.loaders)?),
        enc(&serde_json::to_string(&[&scope.mc])?)
    );
    let versions: Vec<MrVersion> = fetch_json(&url).await?;
    let Some(version) = pick(&versions) else { return Ok(Err(NotFound::NoVersion)) };
    let file = file_of(version).expect("picked versions have a file");
    match remote_zip_texts(&file.url, file.size, scope.target.family.files()).await {
        Some(texts) if declares(&texts, id, scope.target) => Ok(Ok((project, version.id.clone()))),
        _ => Ok(Err(NotFound::NotSameMod)),
    }
}

/// Installs what `check` finds missing, one at a time through `install(project id, version id)`.
async fn fix<F, Fut>(scope: &Scope, install: F) -> Result<DepInstallResult>
where
    F: Fn(String, String) -> Fut,
    Fut: Future<Output = Result<()>>,
{
    let mut out = DepInstallResult::default();
    let mut done: HashMap<String, String> = HashMap::new();
    for dep in check(scope).await?.missing {
        if dep.disabled.is_some() {
            out.not_found.push(DepNotFound { id: dep.id, reason: NotFound::Disabled });
            continue;
        }
        match resolve(&dep.id, scope).await {
            Err(e) => out.failed.push(DepFailed { id: dep.id, error: e.to_string() }),
            Ok(Err(reason)) => out.not_found.push(DepNotFound { id: dep.id, reason }),
            // Two ids can be one project (slug and id).
            Ok(Ok((project, _))) if done.contains_key(&project.id) => {}
            Ok(Ok((project, version))) => match install(project.id.clone(), version).await {
                Ok(()) => {
                    out.installed.push(project.title.clone());
                    done.insert(project.id, project.title);
                }
                Err(e) => out.failed.push(DepFailed { id: dep.id, error: e.to_string() }),
            },
        }
    }
    Ok(out)
}

pub async fn install_instance(id: &str) -> Result<DepInstallResult> {
    let scope = instance_scope(id).await?;
    fix(&scope, |pid, vid| async move { modrinth::install(id, &pid, ContentKind::Mod, Some(vid)).await }).await
}

pub async fn install_server(id: &str, area: Option<&str>) -> Result<DepInstallResult> {
    let scope = server_scope(id, area).await?;
    fix(&scope, |pid, vid| async move { plugins::install(id, &pid, Some(vid), area).await }).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const FABRIC: Target = Target { family: Family::Fabric, side: Side::Client };
    const NEO: Target = Target { family: Family::Neoforge, side: Side::Client };
    const PAPER: Target = Target { family: Family::Paper, side: Side::Server };

    fn jar(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (name, bytes) in files {
            w.start_file(*name, opts).unwrap();
            w.write_all(bytes).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    fn read(bytes: Vec<u8>, t: Target) -> Option<Parsed> {
        read_jar(&mut zip::ZipArchive::new(Cursor::new(bytes)).unwrap(), t, 0)
    }

    fn scanned(filename: &str, enabled: bool, parsed: Option<Parsed>) -> Scanned {
        Scanned { filename: filename.into(), enabled, parsed }
    }

    fn fabric(text: &str) -> Option<Parsed> {
        parse_fabric(text, Side::Client)
    }

    #[test]
    fn reads_fabric_depends_and_provides() {
        let p = fabric(
            r#"{"id": "Create", "name": "Create", "provides": ["create_api"],
            "depends": {"fabricloader": ">=0.15", "fabric-api": "*", "Porting_Lib": ["1.0", "2.0"]},
            "recommends": {"jei": "*"}, "suggests": {"emi": "*"}, "breaks": {"optifine": "*"}}"#,
        )
        .unwrap();
        assert_eq!(p.ids, vec!["create", "create_api"]);
        let mut needs = p.needs.clone();
        needs.sort();
        assert_eq!(needs, vec!["fabric-api", "fabricloader", "porting_lib"]);
        // As Fabric's reader takes it: a trailing comma and a raw line break in a string.
        let loose = fabric("{\"id\": \"x\", \"description\": \"a\nb\", \"depends\": {\"y\": \"*\",},}").unwrap();
        assert_eq!(loose.needs, vec!["y"]);
        assert!(fabric("not json").is_none());
    }

    #[test]
    fn fabric_mods_for_the_other_side_are_skipped() {
        let client = r#"{"id": "hud", "environment": "client", "depends": {"lib": "*"}}"#;
        assert!(parse_fabric(client, Side::Server).unwrap().skipped);
        assert!(!parse_fabric(client, Side::Client).unwrap().skipped);
        assert!(parse_fabric(r#"{"id": "s", "environment": "server"}"#, Side::Client).unwrap().skipped);
    }

    #[test]
    fn reads_quilt_depends_and_provides() {
        let p = parse_quilt(
            r#"{"quilt_loader": {"group": "org.x", "id": "qmod", "version": "1", "metadata": {"name": "Q Mod"},
              "provides": ["qalias", {"id": "org.y:qother", "version": "2"}],
              "depends": ["quilt_loader", {"id": "org.z:required_one", "versions": ">=1"},
                {"id": "maybe", "optional": true}, {"id": "unlessed", "unless": "other"}, [{"id": "a"}, {"id": "b"}]],
              "jars": ["META-INF/jars/inner.jar"]}}"#,
            Side::Client,
        )
        .unwrap();
        assert_eq!(p.ids, vec!["qmod", "qalias", "qother"]);
        assert_eq!(p.needs, vec!["quilt_loader", "required_one"]);
        assert_eq!(p.nested, vec!["META-INF/jars/inner.jar"]);
        assert_eq!(p.name.as_deref(), Some("Q Mod"));
    }

    #[test]
    fn reads_forge_mandatory_dependencies() {
        let t = r#"
modLoader="javafml"
[[mods]]
modId="alpha"
displayName="Alpha"
[[mods]]
modId="beta" # a second mod in one jar
[[dependencies.alpha]]
    modId="forge"
    mandatory=true
[[dependencies.alpha]]
    modId="geckolib"
    mandatory=true # yes
    side="BOTH"
[[dependencies.alpha]]
    modId="jei"
    mandatory=false
[[dependencies.alpha]]
    modId="clientlib"
    mandatory=true
    side="CLIENT"
[[dependencies."beta"]]
    modId="serverlib"
    mandatory=true
    side="SERVER"
"#;
        let client = parse_mods_toml(t, false, Side::Client).unwrap();
        assert_eq!(client.ids, vec!["alpha", "beta"]);
        assert_eq!(client.needs, vec!["forge", "geckolib", "clientlib"]);
        assert_eq!(parse_mods_toml(t, false, Side::Server).unwrap().needs, vec!["forge", "geckolib", "serverlib"]);
    }

    #[test]
    fn reads_neoforge_types() {
        let t = r#"
[[mods]]
modId="gamma"
provides=["gamma_api", 'gamma2']
[[dependencies.gamma]]
    modId="neoforge"
    type="required"
[[dependencies.gamma]]
    modId="curios"
    type="required"
[[dependencies.gamma]]
    modId="jei"
    type="optional"
[[dependencies.gamma]]
    modId="badmod"
    type="incompatible"
[[dependencies.gamma]]
    modId="unsaid"
[[dependencies.gamma]]
    modId="legacy"
    mandatory=true
"#;
        let p = parse_mods_toml(t, true, Side::Client).unwrap();
        assert_eq!(p.ids, vec!["gamma", "gamma_api", "gamma2"]);
        assert_eq!(p.needs, vec!["neoforge", "curios", "unsaid", "legacy"]);
        // A dependency saying nothing is only required in NeoForge's own file.
        assert!(!parse_mods_toml(t, false, Side::Client).unwrap().needs.contains(&"unsaid".to_string()));
    }

    #[test]
    fn reads_plugin_yml() {
        let flow = "name: My Plugin\nversion: 1\nmain: x.Y\ndepend: [Vault, \"Luck Perms\"] # needed\nsoftdepend: [Other]\nprovides: [OldName]\n";
        let p = parse_plugin_yml(flow, false).unwrap();
        assert_eq!(p.ids, vec!["my_plugin", "oldname"]);
        assert_eq!(p.needs, vec!["vault", "luck_perms"]);
        assert_eq!(p.name.as_deref(), Some("My Plugin"));

        let block = "name: 'B'\ndepend:\n  - One\n  # a comment\n  - 'Two'\nsoftdepend:\n  - Three\nloadbefore: []\n";
        assert_eq!(parse_plugin_yml(block, false).unwrap().needs, vec!["one", "two"]);
        let flush = "name: C\ndepend:\n- One\n- Two\nmain: x\n";
        assert_eq!(parse_plugin_yml(flush, false).unwrap().needs, vec!["one", "two"]);
        assert_eq!(parse_plugin_yml("name: D\ndepend: Solo\n", false).unwrap().needs, vec!["solo"]);
        let many = "name: E\ndepend: [A,\n  B]\n";
        assert_eq!(parse_plugin_yml(many, false).unwrap().needs, vec!["a", "b"]);
        assert!(parse_plugin_yml("main: x.Y\n", false).is_none());
        // Keys only count at the top level.
        assert!(parse_plugin_yml("name: F\ncommands:\n  depend:\n    - Nope\n", false).unwrap().needs.is_empty());
    }

    #[test]
    fn reads_paper_plugin_yml() {
        let t = "name: Pap\nmain: x.Y\ndependencies:\n  bootstrap:\n    Early:\n      load: BEFORE\n      required: true\n  server:\n    Must:\n      load: BEFORE\n    Maybe:\n      required: false\n      join-classpath: true\nauthors: [x]\n";
        assert_eq!(parse_plugin_yml(t, true).unwrap().needs, vec!["early", "must"]);
        let old = "name: Pap\ndependencies:\n  - name: Must\n    required: true\n    bootstrap: false\n  - name: Maybe\n    required: false\n";
        assert_eq!(parse_plugin_yml(old, true).unwrap().needs, vec!["must"]);
    }

    #[test]
    fn a_jar_counts_its_own_loaders_metadata_only() {
        let both = jar(&[
            ("fabric.mod.json", br#"{"id": "multi", "depends": {"fabric-only": "*"}}"#),
            ("META-INF/neoforge.mods.toml", b"[[mods]]\nmodId=\"multi\"\n[[dependencies.multi]]\nmodId=\"neo-only\"\ntype=\"required\"\n"),
        ]);
        assert_eq!(read(both.clone(), FABRIC).unwrap().needs, vec!["fabric-only"]);
        assert_eq!(read(both, NEO).unwrap().needs, vec!["neo-only"]);
        // Quilt prefers its own file and runs Fabric jars.
        let quilt = Target { family: Family::Quilt, side: Side::Client };
        assert_eq!(read(jar(&[("fabric.mod.json", br#"{"id": "f"}"#)]), quilt).unwrap().ids, vec!["f"]);
        // Paper reads paper-plugin.yml first; Spigot never does.
        let two = jar(&[("paper-plugin.yml", b"name: P\ndependencies:\n  server:\n    A:\n"), ("plugin.yml", b"name: P\ndepend: [B]\n")]);
        assert_eq!(read(two.clone(), PAPER).unwrap().needs, vec!["a"]);
        assert_eq!(read(two, Target { family: Family::Bukkit, side: Side::Server }).unwrap().needs, vec!["b"]);
        // Nothing for this loader: not read.
        assert!(read(jar(&[("assets/x.txt", b"hi")]), FABRIC).is_none());
    }

    #[test]
    fn nested_jars_provide_ids() {
        let inner2 = jar(&[("fabric.mod.json", br#"{"id": "deep", "depends": {"needed-by-deep": "*"}}"#)]);
        let inner = jar(&[("fabric.mod.json", br#"{"id": "lib", "provides": ["lib-alias"], "jars": [{"file": "nested/deep.jar"}]}"#), ("nested/deep.jar", &inner2)]);
        let outer = jar(&[("fabric.mod.json", br#"{"id": "big", "jars": [{"file": "META-INF/jars/lib.jar"}]}"#), ("META-INF/jars/lib.jar", &inner)]);
        let p = read(outer, FABRIC).unwrap();
        assert_eq!(p.ids, vec!["big", "lib", "lib-alias", "deep"]);
        assert_eq!(p.needs, vec!["needed-by-deep"]);

        // Forge's jar-in-jar folder, which fabric.mod.json doesn't list.
        let lib = jar(&[("META-INF/neoforge.mods.toml", b"[[mods]]\nmodId=\"jarred\"\n")]);
        let outer = jar(&[("META-INF/neoforge.mods.toml", b"[[mods]]\nmodId=\"host\"\n"), ("META-INF/jarjar/lib.jar", &lib), ("META-INF/jarjar/metadata.json", b"{}")]);
        assert_eq!(read(outer, NEO).unwrap().ids, vec!["host", "jarred"]);
        // A nested file that isn't a zip is left alone.
        let bad = jar(&[("fabric.mod.json", br#"{"id": "ok"}"#), ("META-INF/jars/broken.jar", b"nope")]);
        assert_eq!(read(bad, FABRIC).unwrap().ids, vec!["ok"]);
    }

    fn mod_needing(id: &str, needs: &[&str]) -> Parsed {
        Parsed { ids: vec![id.into()], name: Some(id.to_uppercase()), needs: needs.iter().map(|s| s.to_string()).collect(), ..Default::default() }
    }

    #[test]
    fn reports_what_is_missing_and_who_needs_it() {
        let provider = Parsed { ids: vec!["real".into(), "alias".into(), "nested-module".into()], ..Default::default() };
        let r = report(&[
            scanned("a.jar", true, Some(mod_needing("a", &["minecraft", "java", "fabricloader", "quilt_loader", "forge", "neoforge", "real", "alias", "nested-module", "gone"]))),
            scanned("b.jar", true, Some(mod_needing("b", &["a", "gone", "other"]))),
            scanned("real.jar", true, Some(provider)),
            scanned("lib.jar", true, None),
            scanned("ignored.jar.disabled", false, None),
        ]);
        assert_eq!(r.checked, 3);
        assert_eq!(r.unread, vec!["lib.jar"]);
        let names: Vec<_> = r.missing.iter().map(|m| (m.id.as_str(), m.required_by.join(","))).collect();
        assert_eq!(names, vec![("gone", "A,B".to_string()), ("other", "B".to_string())]);
    }

    #[test]
    fn turned_off_jars_need_and_provide_nothing() {
        let r = report(&[
            scanned("a.jar", true, Some(mod_needing("a", &["lib"]))),
            scanned("lib.jar.disabled", false, Some(mod_needing("lib", &[]))),
            scanned("c.jar.disabled", false, Some(mod_needing("c", &["never-asked"]))),
        ]);
        assert_eq!(r.missing.len(), 1);
        assert_eq!((r.missing[0].id.as_str(), r.missing[0].disabled.as_deref()), ("lib", Some("lib.jar.disabled")));
        // A client-only jar on a server: the loader skips it, so it provides nothing either.
        let mut skipped = mod_needing("hud", &["client-lib"]);
        skipped.skipped = true;
        let r = report(&[scanned("hud.jar", true, Some(skipped)), scanned("b.jar", true, Some(mod_needing("b", &["hud"])))]);
        assert_eq!(r.missing.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["hud"]);
    }

    #[test]
    fn only_exact_projects_and_real_ids_pass() {
        let p = |slug: &str, kind: &str| MrProject { id: "AANobbMI".into(), slug: slug.into(), title: "T".into(), project_type: kind.into() };
        assert!(exact(&p("sodium", "mod"), "sodium") && exact(&p("Sodium", "plugin"), "sodium") && exact(&p("x", "mod"), "aanobbmi"));
        assert!(!exact(&p("sodium-extra", "mod"), "sodium") && !exact(&p("sodium", "modpack"), "sodium"));

        let texts: BTreeMap<&'static str, String> = [("fabric.mod.json", r#"{"id": "real", "provides": ["alias"]}"#.to_string())].into();
        assert!(declares(&texts, "real", FABRIC) && declares(&texts, "alias", FABRIC));
        assert!(!declares(&texts, "rea", FABRIC) && !declares(&texts, "real", NEO));
    }

    #[test]
    fn picks_a_release_with_a_file() {
        let v = |id: &str, kind: &str, files: usize| MrVersion {
            id: id.into(),
            version_type: kind.into(),
            files: (0..files).map(|i| MrFile { url: format!("u{i}"), primary: i == 1, size: 1 }).collect(),
        };
        assert_eq!(pick(&[v("a", "release", 0), v("b", "beta", 1), v("c", "release", 2)]).unwrap().id, "c");
        assert_eq!(pick(&[v("b", "beta", 1), v("c", "alpha", 1)]).unwrap().id, "b");
        assert!(pick(&[v("a", "release", 0)]).is_none());
        assert_eq!(file_of(&v("c", "release", 2)).unwrap().url, "u1");
    }

    #[test]
    fn reads_a_real_folder_and_turned_off_jars() {
        let dir = std::env::temp_dir().join(format!("resincraft-depcheck-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.jar"), jar(&[("fabric.mod.json", br#"{"id": "a", "name": "Mod A", "depends": {"b": "*", "c": "*"}}"#)])).unwrap();
        std::fs::write(dir.join("b.jar.disabled"), jar(&[("fabric.mod.json", br#"{"id": "b"}"#)])).unwrap();
        std::fs::write(dir.join("notes.txt"), "not a jar").unwrap();
        std::fs::write(dir.join("broken.jar"), "not a zip").unwrap();
        let r = report(&scan(&dir, FABRIC));
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(r.checked, 1);
        assert_eq!(r.unread, vec!["broken.jar"]);
        assert_eq!(r.missing.iter().map(|m| (m.id.as_str(), m.disabled.is_some())).collect::<Vec<_>>(), vec![("b", true), ("c", false)]);
        assert_eq!(r.missing[0].required_by, vec!["Mod A"]);
    }
}
