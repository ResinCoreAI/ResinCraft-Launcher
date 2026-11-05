//! Horizon's mod registry as a place to get content for Horizon instances: content mods (`.hmod`), modpacks
//! (`.hpack`), resource packs and Horizon shader packs (`.hshader`), as Horizon's own launcher gets them. The
//! registry is one file, `index.json` (horizon-rust/horizon `docs/api/mod-index.md`, schema 1), kept for half an
//! hour. Project ids are "hz:<id>". Native mods (`.hnmod`) are unpacked into `horizon-native-mods/<id>/`; Horizon's
//! own launcher compiles them into the game before the instance plays (`game::horizon_native`).
//!
//! In a Horizon instance's game folder (the one holding `options.txt`): content mods go in `horizon-mods/` and are
//! turned on in `horizon-mods.txt` (`mod.<id>:true`), resource packs in `resourcepacks/`, shader packs in
//! `shaderpacks/`. Every file is checked against the size and SHA-256 the index pins, and files the registry took
//! down (`revoked`) are refused.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::Value;

use super::details;
use super::index::{index_key, load_index, save_index, ContentIndex, IndexEntry};
use crate::core::ctx::http;
use crate::core::events::Task;
use crate::core::fsutil::safe_join;
use crate::core::instances;
use crate::core::net::{download_file, hash_file, DownloadItem, Hash, HashAlgo};
use crate::core::paths;
use crate::core::types::{
    ContentItem, ContentKind, ContentSource, HorizonFacts, Instance, LoaderType, PackMod, ProjectDetails, ProjectHit, ProjectLink, ProjectVersion,
    SearchQuery, SearchResult, UpdateRef, VersionInfo,
};
use crate::{bail_tr, err_tr, tr};

pub const PREFIX: &str = "hz:";
const INDEX_URL: &str = "https://horizon-rust.github.io/horizon-mods/index.json";
/// The index schema this build reads.
pub const SCHEMA: u64 = 1;
const KEEP: Duration = Duration::from_secs(30 * 60);
/// The largest file the registry lists.
const MAX_FILE: u64 = 64 << 20;
/// Where the registry's own releases are.
const REGISTRY_RELEASES: &str = "https://github.com/horizon-rust/horizon-mods/releases/download/";
/// The folder content mods go in, and the file that turns them on.
pub const MODS_DIR: &str = "horizon-mods";
pub const MODS_FILE: &str = "horizon-mods.txt";
/// A mod's saved settings, under `horizon-mods/`.
const STORAGE_DIR: &str = ".horizon-mod-data";
const PACK_MANIFEST: &str = "horizon-modpack.toml";
const PACK_FORMAT: u32 = 1;
const OVERRIDES: &str = "overrides/";
/// Bytes per override file and in all, unpacked.
const MAX_OVERRIDE: u64 = 64 << 20;
const MAX_OVERRIDES: u64 = 256 << 20;
/// The settings files a modpack may replace.
const SETTINGS: [&str; 3] = ["options.txt", "horizon-graphics.txt", "horizon-extras.txt"];

// ---------- The index ----------

#[derive(Debug, Clone, Deserialize)]
pub struct Index {
    pub schema: u64,
    #[serde(default)]
    pub generated: String,
    #[serde(skip)]
    pub projects: Vec<Project>,
    #[serde(skip)]
    pub revoked: Vec<Revoked>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    pub id: String,
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub owners: Vec<String>,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub icon: Option<String>,
    /// The author's minisign key: every version is signed with it.
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub downloads: u64,
    /// Newest first.
    #[serde(default)]
    pub versions: Vec<Version>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Version {
    pub version: String,
    #[serde(default)]
    pub published: String,
    #[serde(default)]
    pub changelog: String,
    /// The Minecraft version it is for.
    #[serde(default)]
    pub game: String,
    pub file: HFile,
    /// A modpack's mods.
    #[serde(default)]
    pub requires: Vec<Requirement>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HFile {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
    #[serde(default)]
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Requirement {
    pub id: String,
    pub version: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Revoked {
    pub id: String,
    /// Without it, every version of `id`.
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub date: String,
}

/// The kinds the registry lists. Others may come without a new schema: they are kept, not offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Mod,
    NativeMod,
    Modpack,
    Resourcepack,
    Shader,
    Unknown,
}

impl Kind {
    pub fn of(kind: &str) -> Kind {
        match kind {
            "mod" => Kind::Mod,
            "native-mod" => Kind::NativeMod,
            "modpack" => Kind::Modpack,
            "resourcepack" => Kind::Resourcepack,
            "shader" => Kind::Shader,
            _ => Kind::Unknown,
        }
    }
    /// The Discover tab it shows in; None for kinds this build doesn't know.
    pub fn content(self) -> Option<ContentKind> {
        match self {
            Kind::Mod | Kind::NativeMod => Some(ContentKind::Mod),
            Kind::Modpack => Some(ContentKind::Modpack),
            Kind::Resourcepack => Some(ContentKind::Resourcepack),
            Kind::Shader => Some(ContentKind::Shader),
            Kind::Unknown => None,
        }
    }
    fn extension(self) -> Option<&'static str> {
        match self {
            Kind::Mod => Some("hmod"),
            Kind::NativeMod => Some("hnmod"),
            Kind::Modpack => Some("hpack"),
            Kind::Resourcepack => Some("zip"),
            Kind::Shader => Some("hshader"),
            Kind::Unknown => None,
        }
    }
    /// The folder of the game folder its files go in.
    fn folder(self) -> Option<&'static str> {
        match self {
            Kind::Mod => Some(MODS_DIR),
            Kind::Resourcepack => Some("resourcepacks"),
            Kind::Shader => Some("shaderpacks"),
            _ => None,
        }
    }
}

impl Project {
    pub fn kind(&self) -> Kind {
        Kind::of(&self.kind)
    }
    pub fn pid(&self) -> String {
        format!("{PREFIX}{}", self.id)
    }
    pub fn signed(&self) -> bool {
        self.key.as_deref().is_some_and(|k| !k.trim().is_empty())
    }
    fn author(&self) -> String {
        if self.authors.is_empty() { self.owners.join(", ") } else { self.authors.join(", ") }
    }
    fn page(&self) -> String {
        format!("https://playhorizon.dev/mods/{}", self.id)
    }
}

/// Reads `index.json`. A schema above [`SCHEMA`] is refused rather than misread; a project that doesn't read (a
/// field of another type) is left out, so one bad entry doesn't hide the rest.
pub fn parse_index(text: &str) -> Result<Index> {
    let raw: Value = serde_json::from_str(text).map_err(|e| anyhow!("Horizon's registry index doesn't read: {e}"))?;
    let schema = raw.get("schema").and_then(Value::as_u64).unwrap_or(0);
    if schema > SCHEMA {
        bail_tr!(
            "horizon.registryNewer",
            "Horizon's registry uses a newer format (schema {schema}) than this ResinCraft reads. Update ResinCraft.",
            schema = schema
        );
    }
    if schema == 0 {
        return Err(anyhow!("Horizon's registry index has no schema"));
    }
    let mut index: Index = serde_json::from_value(raw.clone())?;
    let list = |key: &str| raw.get(key).and_then(Value::as_array).cloned().unwrap_or_default();
    for p in list("projects") {
        match serde_json::from_value::<Project>(p) {
            Ok(p) if valid_id(&p.id) && !index.projects.iter().any(|q| q.id == p.id) => index.projects.push(p),
            Ok(p) => eprintln!("[horizon] left out project {:?}: bad or repeated id", p.id),
            Err(e) => eprintln!("[horizon] left out a project that doesn't read: {e}"),
        }
    }
    index.revoked = list("revoked").into_iter().filter_map(|r| serde_json::from_value(r).ok()).collect();
    Ok(index)
}

impl Index {
    pub fn project(&self, id: &str) -> Option<&Project> {
        self.projects.iter().find(|p| p.id == id)
    }

    /// Why the file of `id` with this hash was taken down, if it was.
    pub fn revoked(&self, id: &str, sha256: &str) -> Option<&Revoked> {
        self.revoked.iter().find(|r| r.id == id && r.sha256.as_ref().is_none_or(|h| h.eq_ignore_ascii_case(sha256)))
    }

    /// A project's newest version for Minecraft `game` that wasn't taken down.
    pub fn installable<'a>(&self, project: &'a Project, game: &str) -> Option<&'a Version> {
        project.versions.iter().find(|v| v.game == game && self.revoked(&project.id, &v.file.sha256).is_none())
    }

    /// What a card needs to say about a project for an instance that plays `game`.
    fn facts(&self, p: &Project, game: Option<&str>) -> HorizonFacts {
        let mut games: Vec<String> = Vec::new();
        for v in &p.versions {
            if !v.game.is_empty() && !games.contains(&v.game) {
                games.push(v.game.clone());
            }
        }
        let open = |v: &&Version| game.is_none_or(|g| v.game == g);
        let candidates: Vec<&Version> = p.versions.iter().filter(open).collect();
        let revoked = if !candidates.is_empty() && candidates.iter().all(|v| self.revoked(&p.id, &v.file.sha256).is_some()) {
            candidates.first().and_then(|v| self.revoked(&p.id, &v.file.sha256)).map(|r| r.reason.clone())
        } else {
            None
        };
        HorizonFacts { native: p.kind() == Kind::NativeMod, signed: p.signed(), games, revoked }
    }
}

/// `[a-z0-9_]{1,64}`, the registry's id rule.
pub fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn hex64(h: &str) -> bool {
    h.len() == 64 && h.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A release tag, a repository's owner or name, or a file name: letters, digits, `.`, `-`, `_` and `+`, and not
/// only dots.
fn plain_part(s: &str) -> bool {
    !s.is_empty() && s.len() <= 200 && !s.bytes().all(|b| b == b'.') && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-+".contains(&b))
}

/// Whether a file's address is one of the two the registry allows: its own release
/// (`…/horizon-mods/releases/download/<id>-<version>/<name>`) or a release asset of a project's repository
/// (`https://github.com/<owner>/<repo>/releases/download/<tag>/<name>`), named as the entry names the file.
pub fn url_allowed(url: &str, project_id: &str, version: &str, name: &str) -> bool {
    if !plain_part(name) {
        return false;
    }
    if let Some(rest) = url.strip_prefix(REGISTRY_RELEASES) {
        return rest == format!("{project_id}-{version}/{name}");
    }
    let Some(rest) = url.strip_prefix("https://github.com/") else { return false };
    let parts: Vec<&str> = rest.split('/').collect();
    matches!(parts.as_slice(), [owner, repo, "releases", "download", tag, file]
        if plain_part(owner) && plain_part(repo) && plain_part(tag) && *file == name)
}

/// What must hold of a version's file before it is downloaded: its address, its name and size, its hash, and that
/// it wasn't taken down.
pub fn check_file(index: &Index, p: &Project, v: &Version) -> Result<()> {
    let f = &v.file;
    if let Some(r) = index.revoked(&p.id, &f.sha256) {
        bail_tr!("horizon.revoked", "{name} {version} was taken down by Horizon's registry: {reason}", name = p.name, version = v.version, reason = r.reason);
    }
    let extension_ok = p.kind().extension().is_some_and(|e| f.name.to_lowercase().ends_with(&format!(".{e}")));
    if !extension_ok || !url_allowed(&f.url, &p.id, &v.version, &f.name) || !hex64(&f.sha256) || f.size == 0 || f.size > MAX_FILE {
        bail_tr!(
            "horizon.badFile",
            "Horizon's registry lists a file for {name} {version} that ResinCraft won't download ({file})",
            name = p.name,
            version = v.version,
            file = f.name
        );
    }
    Ok(())
}

static CACHE: Mutex<Option<(Instant, Arc<Index>)>> = Mutex::new(None);

/// The index, from the cache when it's fresh (a stale copy when the registry can't be reached).
pub async fn load() -> Result<Arc<Index>> {
    let cached = CACHE.lock().unwrap().clone();
    if let Some((at, index)) = &cached {
        if at.elapsed() < KEEP {
            return Ok(index.clone());
        }
    }
    let fetched = async {
        let res = http().get(INDEX_URL).send().await?.error_for_status()?;
        parse_index(&res.text().await?)
    }
    .await;
    match fetched {
        Ok(index) => {
            let index = Arc::new(index);
            *CACHE.lock().unwrap() = Some((Instant::now(), index.clone()));
            Ok(index)
        }
        // A newer schema stays an error: the old copy may say something the new one took back.
        Err(e) if e.downcast_ref::<crate::core::text::Text>().is_some() => Err(e),
        Err(e) => match cached {
            Some((_, index)) => Ok(index),
            None => Err(err_tr!("horizon.unreachable", "Couldn't reach Horizon's registry: {error}", error = e.to_string())),
        },
    }
}

/// "hz:<id>" → `<id>`, checked for shape.
pub fn id_of(project_id: &str) -> Result<&str> {
    let id = project_id.strip_prefix(PREFIX).unwrap_or(project_id);
    if valid_id(id) {
        Ok(id)
    } else {
        Err(err_tr!("horizon.unknownProject", "Horizon's registry has no project {id}", id = id))
    }
}

fn project<'a>(index: &'a Index, project_id: &str) -> Result<&'a Project> {
    let id = id_of(project_id)?;
    index.project(id).ok_or_else(|| err_tr!("horizon.unknownProject", "Horizon's registry has no project {id}", id = id))
}

// ---------- Discover ----------

fn to_hit(index: &Index, p: &Project, game: Option<&str>) -> ProjectHit {
    ProjectHit {
        id: p.pid(),
        slug: p.id.clone(),
        title: p.name.clone(),
        description: p.summary.clone(),
        icon_url: p.icon.clone().filter(|i| i.starts_with("https://")),
        author: p.author(),
        downloads: p.downloads,
        follows: 0,
        categories: Vec::new(),
        kind: p.kind().content().unwrap_or(ContentKind::Mod),
        updated: p.versions.first().map(|v| v.published.clone()).unwrap_or_default(),
        source: ContentSource::Horizon,
        url: p.page(),
        horizon: Some(index.facts(p, game)),
    }
}

/// The projects of `kind` whose name, summary, id or authors hold the query. Content mods come before native mods,
/// which can't be installed from here.
pub fn find<'a>(index: &'a Index, q: &SearchQuery) -> Vec<&'a Project> {
    let words: Vec<String> = q.query.to_lowercase().split_whitespace().map(str::to_string).collect();
    let mut found: Vec<&Project> = index
        .projects
        .iter()
        .filter(|p| p.kind().content() == Some(q.kind) && !p.versions.is_empty())
        .filter(|p| {
            let text = format!("{} {} {} {}", p.name, p.summary, p.id, p.authors.join(" ")).to_lowercase();
            words.iter().all(|w| text.contains(w.as_str()))
        })
        .collect();
    let named = |p: &Project| words.iter().all(|w| p.name.to_lowercase().contains(w.as_str()));
    let first = |p: &Project| p.versions.last().map(|v| v.published.clone()).unwrap_or_default();
    let last = |p: &Project| p.versions.first().map(|v| v.published.clone()).unwrap_or_default();
    found.sort_by(|a, b| {
        let order = match q.sort.as_deref() {
            Some("newest") => first(b).cmp(&first(a)),
            Some("updated") => last(b).cmp(&last(a)),
            Some("downloads") | Some("follows") => b.downloads.cmp(&a.downloads),
            _ => named(b).cmp(&named(a)).then(b.downloads.cmp(&a.downloads)),
        };
        (a.kind() == Kind::NativeMod).cmp(&(b.kind() == Kind::NativeMod)).then(order).then(a.name.cmp(&b.name))
    });
    found
}

pub async fn search_hits(q: &SearchQuery) -> Result<SearchResult> {
    let index = load().await?;
    let offset = q.offset.unwrap_or(0) as usize;
    let limit = q.limit.unwrap_or(20).clamp(1, 100) as usize;
    let found = find(&index, q);
    let game = q.game_version.as_deref().filter(|g| !g.is_empty());
    Ok(SearchResult {
        total: found.len() as u64,
        hits: found.into_iter().skip(offset).take(limit).map(|p| to_hit(&index, p, game)).collect(),
        offset: offset as u64,
    })
}

/// The project's Markdown description, its licence and source.
pub async fn details(project_id: &str) -> Result<ProjectDetails> {
    let index = load().await?;
    let p = project(&index, project_id)?;
    let text = if p.description.trim().is_empty() { p.summary.clone() } else { p.description.clone() };
    let source = Some(p.source.clone()).filter(|s| s.starts_with("https://"));
    let body = match source.as_deref().and_then(github_base) {
        Some((page, raw)) => details::absolutize(&details::markdown(&text, false), &page, &raw),
        None => details::markdown(&text, false),
    };
    Ok(ProjectDetails {
        body,
        gallery: Vec::new(),
        links: ProjectLink::list([("source", source), ("website", Some(p.page()))]),
        license: Some(p.license.clone()).filter(|l| !l.trim().is_empty()),
        changelogs: true,
        dependencies: false,
        hit: to_hit(&index, p, None),
    })
}

/// A GitHub repository's page and raw-file bases, for relative links in a description.
fn github_base(source: &str) -> Option<(String, String)> {
    let rest = source.strip_prefix("https://github.com/")?;
    let mut parts = rest.split('/');
    let (owner, repo) = (parts.next()?, parts.next()?);
    (plain_part(owner) && plain_part(repo))
        .then(|| (format!("https://github.com/{owner}/{repo}/blob/HEAD/"), format!("https://raw.githubusercontent.com/{owner}/{repo}/HEAD/")))
}

pub async fn version_info(project_id: &str, version: &str) -> Result<VersionInfo> {
    let index = load().await?;
    let p = project(&index, project_id)?;
    let changelog = p
        .versions
        .iter()
        .find(|v| v.version == version)
        .map(|v| v.changelog.clone())
        .filter(|c| !c.trim().is_empty())
        .map(|c| details::markdown(&c, false));
    Ok(VersionInfo { changelog, dependencies: Vec::new() })
}

/// A modpack version's mods, as the index lists them.
pub async fn pack_mods(project_id: &str, version: &str) -> Result<Vec<PackMod>> {
    let index = load().await?;
    let p = project(&index, project_id)?;
    let Some(v) = p.versions.iter().find(|v| v.version == version) else { return Ok(Vec::new()) };
    Ok(v.requires
        .iter()
        .map(|r| {
            let m = index.project(&r.id);
            PackMod {
                title: m.map(|m| m.name.clone()).unwrap_or_else(|| r.id.clone()),
                id: m.map(Project::pid),
                source: m.map(|_| ContentSource::Horizon),
                icon_url: m.and_then(|m| m.icon.clone()),
                file: m.and_then(|m| m.versions.iter().find(|w| w.version == r.version)).map(|w| w.file.name.clone()),
            }
        })
        .collect())
}

/// The versions that weren't taken down, newest first.
pub async fn versions_for(project_id: &str) -> Result<Vec<ProjectVersion>> {
    let index = load().await?;
    let p = project(&index, project_id)?;
    let content = p.kind() == Kind::Mod || p.kind() == Kind::NativeMod;
    Ok(p.versions
        .iter()
        .filter(|v| index.revoked(&p.id, &v.file.sha256).is_none())
        .map(|v| ProjectVersion {
            id: v.version.clone(),
            name: v.file.name.clone(),
            version_number: v.version.clone(),
            game_versions: vec![v.game.clone()],
            loaders: if content { vec!["horizon".into()] } else { Vec::new() },
            kind: "release".into(),
            published: v.published.clone(),
            downloads: 0,
        })
        .collect())
}

// ---------- Installing ----------

/// The version of `p` to install into `inst`: `wanted`, else the newest for its Minecraft version.
fn choose<'a>(index: &Index, p: &'a Project, inst: &Instance, wanted: Option<&str>) -> Result<&'a Version> {
    match p.kind() {
        Kind::Unknown => bail_tr!("horizon.unknownKind", "ResinCraft can't install {name} ({kind}) yet", name = p.name, kind = p.kind),
        _ => {}
    }
    let v = match wanted {
        Some(w) => p.versions.iter().find(|v| v.version == w).ok_or_else(|| {
            err_tr!("horizon.noSuchVersion", "Horizon's registry has no version {version} of {name}", version = w, name = p.name)
        })?,
        None => match index.installable(p, &inst.mc_version) {
            Some(v) => v,
            None => {
                if let Some(v) = p.versions.iter().find(|v| v.game == inst.mc_version) {
                    check_file(index, p, v)?;
                }
                bail_tr!("horizon.noVersion", "{name} has no version for Minecraft {mc}", name = p.name, mc = inst.mc_version);
            }
        },
    };
    if v.game != inst.mc_version {
        bail_tr!(
            "horizon.otherGame",
            "{name} {version} is for Minecraft {game}; {instance} plays {mc}",
            name = p.name,
            version = v.version,
            game = v.game,
            instance = inst.name,
            mc = inst.mc_version
        );
    }
    check_file(index, p, v)?;
    Ok(v)
}

fn download(v: &Version, dest: &Path) -> DownloadItem {
    DownloadItem::new(&v.file.url, dest).hash(Some(Hash::sha256(v.file.sha256.clone()))).size(Some(v.file.size))
}

/// Where an install goes: a Horizon game folder, the Minecraft version it plays and its name (for messages).
pub struct Target<'a> {
    pub game: &'a Path,
    pub mc: &'a str,
    pub name: &'a str,
}

/// Installs a project into a Horizon instance, recording what it put where in `idx`.
pub(crate) async fn install_into(inst: &Instance, task: &Task, idx: &mut ContentIndex, project_id: &str, version: Option<String>) -> Result<()> {
    if inst.loader != LoaderType::Horizon {
        bail_tr!("horizon.horizonOnly", "Horizon's registry is for Horizon instances; {name} isn't one", name = inst.name);
    }
    let index = load().await?;
    let p = project(&index, project_id)?;
    let v = choose(&index, p, inst, version.as_deref())?;
    task.update(tr!("content.installingItem", "Installing {name}…", name = p.name), None);
    let game = paths::game_dir(&inst.id);
    let to = Target { game: &game, mc: &inst.mc_version, name: &inst.name };
    if p.kind() == Kind::Modpack {
        let work = paths::instance_dir(&inst.id).join(".horizon-pack");
        let _ = tokio::fs::remove_dir_all(&work).await;
        let result = async {
            let file = safe_join(&work, &v.file.name)?;
            check_file(&index, p, v)?;
            download_file(&download(v, &file)).await?;
            install_pack_file(&index, &to, Some(task), idx, &p.name, &file).await
        }
        .await;
        let _ = tokio::fs::remove_dir_all(&work).await;
        return result;
    }
    if p.kind() == Kind::NativeMod {
        return put_native(&index, &to, idx, p, v).await;
    }
    put_file(&index, &to, idx, p, v, true).await
}

/// The content index's key for an installed native mod (its folder isn't a file the Content tab lists by name).
fn native_key(id: &str) -> String {
    index_key(ContentKind::Mod, &format!("{id}.hnmod"))
}

/// Downloads a native mod and unpacks it into `horizon-native-mods/<id>/`, checked as Horizon's registry checks
/// it, replacing the version that's there. Nothing is compiled here: that waits for the player's go-ahead.
pub async fn put_native(index: &Index, to: &Target<'_>, idx: &mut ContentIndex, p: &Project, v: &Version) -> Result<()> {
    use crate::game::horizon_native::{unpack, NATIVE_DIR};
    check_file(index, p, v)?;
    let dir = to.game.join(NATIVE_DIR);
    let work = dir.join(".downloads");
    tokio::fs::create_dir_all(&work).await?;
    let file = safe_join(&work, &v.file.name)?;
    download_file(&download(v, &file)).await?;
    let sha1 = hash_file(&file, HashAlgo::Sha1).await.unwrap_or_default();
    let (path, into, id) = (file.clone(), dir.clone(), p.id.clone());
    let unpacked = tokio::task::spawn_blocking(move || unpack(&path, &into, Some(&id))).await?;
    let _ = tokio::fs::remove_dir_all(&work).await;
    let m = unpacked?;
    if m.version != v.version || m.game != v.game {
        let _ = tokio::fs::remove_dir_all(dir.join(&p.id)).await;
        bail_tr!(
            "horizon.nativeMismatch",
            "{name}'s file says it is {version} for Minecraft {game}, not what Horizon's registry lists",
            name = p.name,
            version = m.version,
            game = m.game
        );
    }
    idx.insert(
        native_key(&p.id),
        IndexEntry { project_id: p.pid(), version_id: v.version.clone(), version_number: v.version.clone(), title: p.name.clone(), icon_url: p.icon.clone(), sha1 },
    );
    Ok(())
}

/// Downloads one mod, resource pack or shader pack into the game folder (an older copy of it goes), a mod turned
/// on or off in `horizon-mods.txt`.
pub async fn put_file(index: &Index, to: &Target<'_>, idx: &mut ContentIndex, p: &Project, v: &Version, on: bool) -> Result<()> {
    check_file(index, p, v)?;
    let (Some(folder), Some(kind)) = (p.kind().folder(), p.kind().content()) else {
        bail_tr!("horizon.unknownKind", "ResinCraft can't install {name} ({kind}) yet", name = p.name, kind = p.kind);
    };
    let dir = to.game.join(folder);
    tokio::fs::create_dir_all(&dir).await?;
    let dest = safe_join(&dir, &v.file.name)?;
    let prefix = format!("{}/", kind.as_str());
    let old: Vec<String> = idx.iter().filter(|(k, e)| e.project_id == p.pid() && k.starts_with(&prefix)).map(|(k, _)| k.clone()).collect();
    download_file(&download(v, &dest)).await?;
    for key in old {
        let name = &key[prefix.len()..];
        if name != v.file.name {
            if let Ok(path) = safe_join(&dir, name) {
                let _ = tokio::fs::remove_file(&path).await;
            }
        }
        idx.remove(&key);
    }
    if p.kind() == Kind::Mod {
        set_mod(to.game, &p.id, on).await?;
    }
    idx.insert(
        index_key(kind, &v.file.name),
        IndexEntry {
            project_id: p.pid(),
            version_id: v.version.clone(),
            version_number: v.version.clone(),
            title: p.name.clone(),
            icon_url: p.icon.clone(),
            sha1: hash_file(&dest, HashAlgo::Sha1).await.unwrap_or_default(),
        },
    );
    Ok(())
}

/// A downloaded `.hpack` into a Horizon game folder: each of its mods from the index (the very file the pack pins),
/// turned on or off as it says, then its overrides. Everything is looked up and checked before anything lands.
pub async fn install_pack_file(index: &Index, to: &Target<'_>, task: Option<&Task>, idx: &mut ContentIndex, title: &str, file: &Path) -> Result<()> {
    let path = file.to_path_buf();
    let pack = tokio::task::spawn_blocking(move || read_pack(&path)).await??;
    if pack.manifest.game != to.mc {
        bail_tr!(
            "horizon.otherGame",
            "{name} {version} is for Minecraft {game}; {instance} plays {mc}",
            name = title,
            version = pack.manifest.version,
            game = pack.manifest.game,
            instance = to.name,
            mc = to.mc
        );
    }
    let mut mods = Vec::new();
    let mut native = Vec::new();
    for m in &pack.manifest.mods {
        let found = index.project(&m.id).and_then(|q| q.versions.iter().find(|w| w.version == m.version).map(|w| (q, w)));
        let Some((q, w)) = found else {
            bail_tr!("horizon.packMissing", "{pack} needs {id} {version}, which isn't in Horizon's registry", pack = title, id = m.id, version = m.version);
        };
        if !w.file.sha256.eq_ignore_ascii_case(&m.sha256) {
            bail_tr!("horizon.packPin", "{pack} pins another file of {id} {version} than Horizon's registry lists", pack = title, id = m.id, version = m.version);
        }
        match q.kind() {
            Kind::Mod => {
                check_file(index, q, w)?;
                mods.push((q, w, m.on));
            }
            // Always on, as in Horizon: a pack's `on` doesn't apply.
            Kind::NativeMod => {
                check_file(index, q, w)?;
                native.push((q, w));
            }
            _ => bail_tr!("horizon.unknownKind", "ResinCraft can't install {name} ({kind}) yet", name = q.name, kind = q.kind),
        }
    }
    let total = (mods.len() + native.len()).max(1) as f64;
    let content = mods.len() as f64;
    for (n, (q, w, on)) in mods.into_iter().enumerate() {
        if let Some(task) = task {
            task.update(tr!("content.installingItem", "Installing {name}…", name = q.name), Some(n as f64 / total));
        }
        put_file(index, to, idx, q, w, on).await?;
    }
    for (n, (q, w)) in native.into_iter().enumerate() {
        if let Some(task) = task {
            task.update(tr!("content.installingItem", "Installing {name}…", name = q.name), Some((content + n as f64) / total));
        }
        put_native(index, to, idx, q, w).await?;
    }
    let (path, home) = (file.to_path_buf(), to.game.to_path_buf());
    tokio::task::spawn_blocking(move || extract_overrides(&path, &home)).await??;
    Ok(())
}

pub async fn install(instance_id: &str, project_id: &str, version: Option<String>) -> Result<()> {
    let inst = instances::get(instance_id).await?;
    let project_id = project_id.to_string();
    Task::run(tr!("horizon.installing", "Installing from Horizon's registry"), Some(inst.id.clone()), move |task| async move {
        let mut idx = load_index(&inst.id).await;
        let r = install_into(&inst, &task, &mut idx, &project_id, version).await;
        save_index(&inst.id, &idx).await?;
        r
    })
    .await
}

/// Marks Horizon content that has a newer version for the instance's Minecraft.
pub async fn check_updates(inst: &Instance, items: &mut [ContentItem]) -> Result<()> {
    if !items.iter().any(|i| i.project_id.as_deref().is_some_and(|p| p.starts_with(PREFIX))) {
        return Ok(());
    }
    let index = load().await?;
    for item in items.iter_mut() {
        let Some(p) = item.project_id.as_deref().and_then(|p| p.strip_prefix(PREFIX)).and_then(|id| index.project(id)) else { continue };
        if let Some(v) = index.installable(p, &inst.mc_version) {
            if item.version_id.as_deref() != Some(v.version.as_str()) {
                item.update = Some(UpdateRef { version_id: v.version.clone(), version_number: v.version.clone() });
            }
        }
    }
    Ok(())
}

// ---------- horizon-mods.txt ----------

/// `text` (a `horizon-mods.txt`) with mod `id` turned on or off; every other line stays as it was.
pub fn set_mod_line(text: &str, id: &str, on: bool) -> String {
    let key = format!("mod.{id}");
    let line = format!("{key}:{on}");
    let mut found = false;
    let mut out: Vec<String> = Vec::new();
    for l in text.lines() {
        if l.split_once(':').is_some_and(|(k, _)| k.trim() == key) {
            if !found {
                out.push(line.clone());
            }
            found = true;
        } else {
            out.push(l.to_string());
        }
    }
    if !found {
        out.push(line);
    }
    out.join("\n") + "\n"
}

/// `text` without mod `id`'s line.
pub fn drop_mod_line(text: &str, id: &str) -> String {
    let key = format!("mod.{id}");
    let kept: Vec<&str> = text.lines().filter(|l| !l.split_once(':').is_some_and(|(k, _)| k.trim() == key)).collect();
    if kept.is_empty() { String::new() } else { kept.join("\n") + "\n" }
}

/// Whether `horizon-mods.txt` turns mod `id` on (mods are off unless it says so).
pub fn mod_on(text: &str, id: &str) -> bool {
    let key = format!("mod.{id}");
    text.lines().filter_map(|l| l.split_once(':')).filter(|(k, _)| k.trim() == key).last().is_some_and(|(_, v)| v.trim() == "true")
}

async fn read_mods_file(game: &Path) -> String {
    tokio::fs::read_to_string(game.join(MODS_FILE)).await.unwrap_or_default()
}

async fn set_mod(game: &Path, id: &str, on: bool) -> Result<()> {
    let text = read_mods_file(game).await;
    tokio::fs::create_dir_all(game).await?;
    tokio::fs::write(game.join(MODS_FILE), set_mod_line(&text, id, on)).await?;
    Ok(())
}

// ---------- A Horizon instance's content tab ----------

/// A content mod's id and name, from its `horizon-mod.toml`.
fn mod_manifest(path: &Path) -> Option<(String, Option<String>)> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    let mut text = String::new();
    zip.by_name("horizon-mod.toml").ok()?.take(1 << 20).read_to_string(&mut text).ok()?;
    let v: toml::Value = toml::from_str(&text).ok()?;
    let id = v.get("id")?.as_str()?.to_string();
    valid_id(&id).then(|| (id, v.get("name").and_then(|n| n.as_str()).map(str::to_string)))
}

/// The id of an installed mod file: what the index recorded, else its manifest's.
async fn mod_id(game: &Path, filename: &str, idx: &ContentIndex) -> Result<String> {
    let base = filename.strip_suffix(".disabled").unwrap_or(filename);
    if let Some(id) = idx.get(&index_key(ContentKind::Mod, base)).and_then(|e| e.project_id.strip_prefix(PREFIX)).filter(|id| valid_id(id)) {
        return Ok(id.to_string());
    }
    let path = safe_join(&game.join(MODS_DIR), base)?;
    tokio::task::spawn_blocking(move || mod_manifest(&path))
        .await?
        .map(|(id, _)| id)
        .ok_or_else(|| err_tr!("horizon.notAMod", "{file} isn't a Horizon content mod", file = base.to_string()))
}

/// The content mods in `horizon-mods/`, on or off as `horizon-mods.txt` says.
pub async fn list_mods(inst: &Instance) -> Result<Vec<ContentItem>> {
    let game = paths::game_dir(&inst.id);
    let dir = game.join(MODS_DIR);
    tokio::fs::create_dir_all(&dir).await?;
    let text = read_mods_file(&game).await;
    let idx = load_index(&inst.id).await;
    let mut items = Vec::new();
    let mut entries = tokio::fs::read_dir(&dir).await?;
    while let Some(e) = entries.next_entry().await? {
        let name = e.file_name().to_string_lossy().into_owned();
        if !name.to_lowercase().ends_with(".hmod") || !e.file_type().await?.is_file() {
            continue;
        }
        let path = e.path();
        let manifest = tokio::task::spawn_blocking(move || mod_manifest(&path)).await?;
        let entry = idx.get(&index_key(ContentKind::Mod, &name));
        let id = entry.and_then(|e| e.project_id.strip_prefix(PREFIX).map(str::to_string)).or_else(|| manifest.as_ref().map(|m| m.0.clone()));
        items.push(ContentItem {
            filename: name.clone(),
            enabled: id.as_deref().is_some_and(|id| mod_on(&text, id)),
            size: e.metadata().await.map(|m| m.len()).unwrap_or(0),
            kind: ContentKind::Mod,
            title: entry.map(|e| e.title.clone()).or_else(|| manifest.and_then(|m| m.1)),
            project_id: entry.map(|e| e.project_id.clone()),
            version_id: entry.map(|e| e.version_id.clone()),
            version_number: entry.map(|e| e.version_number.clone()),
            icon_url: entry.and_then(|e| e.icon_url.clone()),
            update: None,
            native: false,
        });
    }
    // Native mods: always on, shown as `<id>.hnmod`.
    for m in crate::game::horizon_native::installed(&game) {
        let entry = idx.get(&native_key(&m.id));
        items.push(ContentItem {
            filename: format!("{}.hnmod", m.id),
            enabled: true,
            size: dir_size(&game.join(crate::game::horizon_native::NATIVE_DIR).join(&m.id)).await,
            kind: ContentKind::Mod,
            title: Some(entry.map(|e| e.title.clone()).unwrap_or_else(|| m.id.clone())),
            project_id: Some(entry.map(|e| e.project_id.clone()).unwrap_or_else(|| format!("{PREFIX}{}", m.id))),
            version_id: Some(m.version.clone()),
            version_number: Some(m.version.clone()),
            icon_url: entry.and_then(|e| e.icon_url.clone()),
            update: None,
            native: true,
        });
    }
    items.sort_by_key(|i| i.title.clone().unwrap_or_else(|| i.filename.clone()).to_lowercase());
    Ok(items)
}

async fn dir_size(dir: &Path) -> u64 {
    fn walk(d: &Path) -> u64 {
        let list = std::fs::read_dir(d).into_iter().flatten().flatten();
        list.map(|e| if e.file_type().is_ok_and(|t| t.is_dir()) { walk(&e.path()) } else { e.metadata().map(|m| m.len()).unwrap_or(0) }).sum()
    }
    let dir = dir.to_path_buf();
    tokio::task::spawn_blocking(move || walk(&dir)).await.unwrap_or(0)
}

/// A native mod's id from its Content tab name (`<id>.hnmod`).
fn native_id(filename: &str) -> Option<&str> {
    filename.strip_suffix(".hnmod").filter(|id| valid_id(id))
}

/// Turns a content mod on or off in `horizon-mods.txt` (the file stays where it is).
pub async fn toggle_mod(inst: &Instance, filename: &str) -> Result<()> {
    if native_id(filename).is_some() {
        bail_tr!("horizon.nativeAlwaysOn", "Native mods are always on, as in Horizon: remove one to stop using it.");
    }
    let game = paths::game_dir(&inst.id);
    let idx = load_index(&inst.id).await;
    let id = mod_id(&game, filename, &idx).await?;
    let on = mod_on(&read_mods_file(&game).await, &id);
    set_mod(&game, &id, !on).await
}

/// Moves a content mod to the Recycle Bin and drops its line from `horizon-mods.txt`.
pub async fn remove_mod(inst: &Instance, filename: &str) -> Result<()> {
    let game = paths::game_dir(&inst.id);
    if let Some(id) = native_id(filename) {
        crate::game::horizon_native::remove(&game, id).await?;
        let mut idx = load_index(&inst.id).await;
        idx.remove(&native_key(id));
        return save_index(&inst.id, &idx).await;
    }
    let base = filename.strip_suffix(".disabled").unwrap_or(filename);
    let mut idx = load_index(&inst.id).await;
    let id = mod_id(&game, base, &idx).await.ok();
    crate::core::shell::trash(&safe_join(&game.join(MODS_DIR), base)?).await?;
    if let Some(id) = id {
        let text = read_mods_file(&game).await;
        if !text.is_empty() {
            tokio::fs::write(game.join(MODS_FILE), drop_mod_line(&text, &id)).await?;
        }
    }
    idx.remove(&index_key(ContentKind::Mod, base));
    save_index(&inst.id, &idx).await
}

// ---------- .hpack ----------

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PackManifest {
    pub format: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub authors: Vec<String>,
    pub game: String,
    #[serde(default)]
    pub mods: Vec<PackPin>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PackPin {
    pub id: String,
    pub version: String,
    pub sha256: String,
    #[serde(default = "yes")]
    pub on: bool,
}

fn yes() -> bool {
    true
}

fn bad_pack(why: impl std::fmt::Display) -> anyhow::Error {
    err_tr!("horizon.badPack", "That Horizon modpack can't be installed: {why}", why = why.to_string())
}

pub fn parse_pack_manifest(text: &str) -> Result<PackManifest> {
    let m: PackManifest = toml::from_str(text).map_err(|e| bad_pack(format!("{PACK_MANIFEST}: {e}")))?;
    if m.format != PACK_FORMAT {
        return Err(bad_pack(format!("{PACK_MANIFEST}: format {}", m.format)));
    }
    if !valid_id(&m.id) {
        return Err(bad_pack(format!("{PACK_MANIFEST}: id {:?}", m.id)));
    }
    for (i, pm) in m.mods.iter().enumerate() {
        if !valid_id(&pm.id) || !hex64(&pm.sha256) || pm.version.is_empty() {
            return Err(bad_pack(format!("{PACK_MANIFEST}: mod {:?}", pm.id)));
        }
        if m.mods[..i].iter().any(|o| o.id == pm.id) {
            return Err(bad_pack(format!("{PACK_MANIFEST}: {} is listed twice", pm.id)));
        }
    }
    Ok(m)
}

/// Whether a path under `overrides/` may be copied into the game folder: a settings file, a resource pack zip, or
/// the saved settings of one of the pack's mods. Plain parts only, so nothing climbs out.
pub fn override_allowed(path: &str, mods: &[&str]) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    let plain = |p: &&str| !p.is_empty() && !p.starts_with('.') && p.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-+ ".contains(&b));
    match parts.as_slice() {
        [file] => SETTINGS.contains(file),
        ["resourcepacks", name] => plain(name) && name.to_lowercase().ends_with(".zip"),
        ["horizon-mods", dir, id, rest @ ..] => *dir == STORAGE_DIR && mods.contains(id) && !rest.is_empty() && rest.iter().all(plain),
        _ => false,
    }
}

#[derive(Debug)]
pub struct Pack {
    pub manifest: PackManifest,
    /// Paths under `overrides/`.
    pub overrides: Vec<String>,
}

/// Reads a `.hpack`: its manifest, and its overrides, each checked (anything else in the zip refuses the pack).
pub fn read_pack(path: &Path) -> Result<Pack> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path)?).map_err(bad_pack)?;
    let mut text = String::new();
    zip.by_name(PACK_MANIFEST).map_err(|_| bad_pack(format!("{PACK_MANIFEST} is missing")))?.take(1 << 20).read_to_string(&mut text)?;
    let manifest = parse_pack_manifest(&text)?;
    let ids: Vec<&str> = manifest.mods.iter().map(|m| m.id.as_str()).collect();
    let (mut overrides, mut total) = (Vec::new(), 0u64);
    for i in 0..zip.len() {
        let entry = zip.by_index_raw(i).map_err(bad_pack)?;
        let name = entry.name().to_string();
        if name == PACK_MANIFEST || entry.is_dir() {
            continue;
        }
        let rest = name.strip_prefix(OVERRIDES).filter(|r| override_allowed(r, &ids)).ok_or_else(|| bad_pack(format!("it holds {name}")))?;
        if entry.size() > MAX_OVERRIDE {
            return Err(bad_pack(format!("{name} is over {} MiB", MAX_OVERRIDE >> 20)));
        }
        total += entry.size();
        overrides.push(rest.to_string());
    }
    if total > MAX_OVERRIDES {
        return Err(bad_pack(format!("its overrides are over {} MiB", MAX_OVERRIDES >> 20)));
    }
    overrides.sort();
    Ok(Pack { manifest, overrides })
}

/// Copies a pack's overrides into the game folder `home`, counting the bytes actually unpacked against the limits
/// (a zip's sizes are its own claim). On a failure, what was written is taken out again.
pub fn extract_overrides(path: &Path, home: &Path) -> Result<Vec<PathBuf>> {
    let pack = read_pack(path)?;
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path)?).map_err(bad_pack)?;
    let mut written: Vec<PathBuf> = Vec::new();
    let mut total = 0u64;
    for name in &pack.overrides {
        let r = (|| -> Result<PathBuf> {
            let mut entry = zip.by_name(&format!("{OVERRIDES}{name}")).map_err(bad_pack)?;
            let target = safe_join(home, name)?;
            if let Some(dir) = target.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let tmp = PathBuf::from(format!("{}.part", target.display()));
            let mut out = std::fs::File::create(&tmp)?;
            let copied = std::io::copy(&mut (&mut entry).take(MAX_OVERRIDE + 1), &mut out)?;
            drop(out);
            total += copied;
            if copied > MAX_OVERRIDE || total > MAX_OVERRIDES {
                let _ = std::fs::remove_file(&tmp);
                return Err(bad_pack(format!("{name} unpacks to more than allowed")));
            }
            std::fs::rename(&tmp, &target)?;
            Ok(target)
        })();
        match r {
            Ok(t) => written.push(t),
            Err(e) => {
                for t in &written {
                    let _ = std::fs::remove_file(t);
                }
                return Err(e);
            }
        }
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const SHA_MOD: &str = "e1135345692f6dd8fd4c51920151944b82333e17049c9bb7d756f27b8deefc53";

    /// Shaped as Horizon's own sample (crates/horizon-mods/tests/index/index-v1.json), with a native mod and a
    /// project of a kind this build doesn't know.
    fn sample() -> String {
        format!(
            r#"{{
  "schema": 1, "generated": "2026-10-24T18:00:00Z",
  "projects": [
    {{ "id": "coords_hud", "kind": "mod", "name": "Coordinates HUD", "summary": "Your coordinates.",
      "description": "`/where` puts them in chat.", "authors": ["Horizon"], "owners": ["ludovic111"], "license": "MIT",
      "source": "https://github.com/horizon-rust/horizon", "icon": "https://horizon-rust.github.io/horizon-mods/icons/coords_hud.png",
      "key": "RWTVtuGb/GRXcBf3Re5hzM07EzoStr2sLTJjghJia0xX15ULoZRFFZDb", "downloads": 12,
      "versions": [{{ "version": "0.1.0", "published": "2026-10-24", "api": "0.1", "side": "client", "capabilities": ["hud"], "api_level": 0, "game": "26.3",
        "file": {{ "name": "coords_hud-0.1.0.hmod", "url": "https://github.com/horizon-rust/horizon-mods/releases/download/coords_hud-0.1.0/coords_hud-0.1.0.hmod",
          "size": 438, "sha256": "{SHA_MOD}", "signature": "untrusted comment: x\nRUT\ntrusted comment: y\nIES\n" }} }}] }},
    {{ "id": "cozy", "kind": "modpack", "name": "Cozy", "summary": "Coordinates and a calm render distance.", "authors": ["Horizon"], "license": "MIT",
      "source": "https://github.com/horizon-rust/horizon-mods",
      "versions": [{{ "version": "1.0.0", "published": "2026-10-24", "api": "1", "game": "26.3",
        "requires": [{{ "id": "coords_hud", "version": "0.1.0", "sha256": "{SHA_MOD}" }}],
        "file": {{ "name": "cozy-1.0.0.hpack", "url": "https://github.com/horizon-rust/horizon-mods/releases/download/cozy-1.0.0/cozy-1.0.0.hpack",
          "size": 471, "sha256": "241ac840f9c5ea3f6252d849da10c90d36cb1da4d6cd8d1d268c2760a353cd65" }} }}] }},
    {{ "id": "glow", "kind": "shader", "name": "Soft Glow", "summary": "A post pass.", "license": "MIT", "source": "https://example.org/glow",
      "versions": [{{ "version": "1.0.0", "game": "26.3",
        "file": {{ "name": "glow-1.0.0.hshader", "url": "https://github.com/someone/glow/releases/download/v1.0.0/glow-1.0.0.hshader",
          "size": 689, "sha256": "331cfe29a666a0de804e9a9ff3d19d246eebcfbfa70b4f49bba8482d30ad9f02" }} }}] }},
    {{ "id": "zoom", "kind": "native-mod", "name": "Zoom", "summary": "Zoom in.", "license": "MIT", "source": "https://github.com/horizon-rust/horizon-mods",
      "versions": [{{ "version": "2.0.0", "game": "26.3",
        "file": {{ "name": "zoom-2.0.0.hnmod", "url": "https://github.com/horizon-rust/horizon-mods/releases/download/zoom-2.0.0/zoom-2.0.0.hnmod",
          "size": 5886, "sha256": "{SHA_MOD}" }} }}] }},
    {{ "id": "skyblock_start", "kind": "world", "name": "Skyblock start", "summary": "Kept, not offered.", "license": "CC0-1.0",
      "source": "https://example.org/skyblock", "future_field": true,
      "versions": [{{ "version": "1", "game": "26.3",
        "file": {{ "name": "skyblock.hworld", "url": "https://example.org/skyblock.hworld", "size": 1,
          "sha256": "0000000000000000000000000000000000000000000000000000000000000000" }} }}] }},
    {{ "id": "Broken", "kind": "mod", "name": 5 }}
  ],
  "revoked": [{{ "id": "bad_mod", "sha256": "1111111111111111111111111111111111111111111111111111111111111111",
    "reason": "Sent chat messages it didn't declare.", "date": "2026-10-24" }}, {{ "id": "glow", "reason": "Every version." }}]
}}"#
        )
    }

    fn query(kind: ContentKind, text: &str) -> SearchQuery {
        SearchQuery { query: text.into(), kind, source: None, game_version: Some("26.3".into()), loader: None, sort: None, offset: None, limit: None }
    }

    #[test]
    fn reads_the_sample_index() {
        let index = parse_index(&sample()).unwrap();
        assert_eq!(index.schema, 1);
        let ids: Vec<&str> = index.projects.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["coords_hud", "cozy", "glow", "zoom", "skyblock_start"]);
        let hud = index.project("coords_hud").unwrap();
        assert_eq!(hud.kind(), Kind::Mod);
        assert!(hud.signed());
        assert_eq!(hud.versions[0].file.size, 438);
        assert_eq!(index.project("cozy").unwrap().versions[0].requires[0].id, "coords_hud");
        // A kind this build doesn't know is kept, but offered nowhere.
        assert_eq!(index.project("skyblock_start").unwrap().kind(), Kind::Unknown);
        for kind in [ContentKind::Mod, ContentKind::Modpack, ContentKind::Resourcepack, ContentKind::Shader] {
            assert!(find(&index, &query(kind, "")).iter().all(|p| p.id != "skyblock_start"));
        }
        // Native mods are listed with the mods, after the content ones.
        let mods: Vec<&str> = find(&index, &query(ContentKind::Mod, "")).iter().map(|p| p.id.as_str()).collect();
        assert_eq!(mods, ["coords_hud", "zoom"]);
        assert_eq!(find(&index, &query(ContentKind::Mod, "coordinates")).len(), 1);
        let facts = index.facts(index.project("zoom").unwrap(), Some("26.3"));
        assert!(facts.native && !facts.signed && facts.games == ["26.3"]);
    }

    #[test]
    fn refuses_a_newer_schema() {
        let newer = sample().replacen("\"schema\": 1", "\"schema\": 2", 1);
        let e = parse_index(&newer).unwrap_err();
        assert!(e.to_string().contains("Update ResinCraft"), "{e}");
        assert!(parse_index(r#"{"projects": []}"#).is_err());
    }

    #[test]
    fn file_addresses_have_one_of_two_shapes() {
        let reg = "https://github.com/horizon-rust/horizon-mods/releases/download/coords_hud-0.1.0/coords_hud-0.1.0.hmod";
        assert!(url_allowed(reg, "coords_hud", "0.1.0", "coords_hud-0.1.0.hmod"));
        // The registry's release must be the entry's own id and version.
        assert!(!url_allowed(reg, "other", "0.1.0", "coords_hud-0.1.0.hmod"));
        assert!(!url_allowed(reg, "coords_hud", "0.2.0", "coords_hud-0.1.0.hmod"));
        let own = "https://github.com/someone/glow/releases/download/v1.0.0/glow-1.0.0.hshader";
        assert!(url_allowed(own, "glow", "1.0.0", "glow-1.0.0.hshader"));
        assert!(!url_allowed(own, "glow", "1.0.0", "other.hshader"));
        for bad in [
            "http://github.com/someone/glow/releases/download/v1/glow.hshader",
            "https://github.com.evil.dev/someone/glow/releases/download/v1/glow.hshader",
            "https://github.com/someone/glow/raw/main/glow.hshader",
            "https://github.com/someone/../releases/download/v1/glow.hshader",
            "https://github.com/someone/glow/releases/download/v1/x/glow.hshader",
            "https://github.com/someone/glow/releases/download/v%2F1/glow.hshader",
            "https://example.org/glow.hshader",
        ] {
            assert!(!url_allowed(bad, "glow", "1", "glow.hshader"), "{bad}");
        }
        let index = parse_index(&sample()).unwrap();
        let world = index.project("skyblock_start").unwrap();
        assert!(check_file(&index, world, &world.versions[0]).is_err());
    }

    #[test]
    fn revoked_files_are_refused() {
        let mut index = parse_index(&sample()).unwrap();
        // Every version of a project, without a hash.
        let glow = index.project("glow").unwrap().clone();
        assert!(index.revoked("glow", &glow.versions[0].file.sha256).is_some());
        let e = check_file(&index, &glow, &glow.versions[0]).unwrap_err();
        assert!(e.to_string().contains("taken down"), "{e}");
        assert!(index.installable(&glow, "26.3").is_none());
        assert_eq!(index.facts(&glow, Some("26.3")).revoked.as_deref(), Some("Every version."));
        // One file, by its hash (either case).
        let hud = index.project("coords_hud").unwrap().clone();
        assert!(check_file(&index, &hud, &hud.versions[0]).is_ok());
        assert!(index.installable(&hud, "26.3").is_some());
        assert!(index.installable(&hud, "26.4").is_none());
        index.revoked.push(Revoked { id: "coords_hud".into(), sha256: Some(SHA_MOD.to_uppercase()), reason: "Bad.".into(), date: String::new() });
        assert!(check_file(&index, &hud, &hud.versions[0]).is_err());
        assert!(index.installable(&hud, "26.3").is_none());
        assert!(index.revoked("bad_mod", SHA_MOD).is_none());
    }

    #[test]
    fn mods_file_keeps_other_lines() {
        let text = "mod.other:true\n# a note\nmod.coords_hud:false\nsomething.else:3\n";
        let on = set_mod_line(text, "coords_hud", true);
        assert_eq!(on, "mod.other:true\n# a note\nmod.coords_hud:true\nsomething.else:3\n");
        assert!(mod_on(&on, "coords_hud") && mod_on(&on, "other") && !mod_on(&on, "coords"));
        let added = set_mod_line("mod.other:true", "new_mod", true);
        assert_eq!(added, "mod.other:true\nmod.new_mod:true\n");
        assert_eq!(set_mod_line("", "a", false), "mod.a:false\n");
        assert!(!mod_on("mod.a:false\n", "a") && !mod_on("", "a"));
        assert_eq!(drop_mod_line(&on, "coords_hud"), "mod.other:true\n# a note\nsomething.else:3\n");
        // A mod whose id starts like another's is a different line.
        assert_eq!(set_mod_line("mod.coords_hud_plus:true\n", "coords_hud", true), "mod.coords_hud_plus:true\nmod.coords_hud:true\n");
    }

    const PACK: &str = "format = 1\nid = \"cozy\"\nname = \"Cozy\"\nversion = \"1.0.0\"\ngame = \"26.3\"\n\n[[mods]]\nid = \"coords_hud\"\nversion = \"0.1.0\"\nsha256 = \"e1135345692f6dd8fd4c51920151944b82333e17049c9bb7d756f27b8deefc53\"\n";

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("resincraft-hpack-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_pack(dir: &Path, manifest: &str, files: &[(&str, &[u8])]) -> PathBuf {
        let path = dir.join("cozy.hpack");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file(PACK_MANIFEST, options).unwrap();
        zip.write_all(manifest.as_bytes()).unwrap();
        for (name, bytes) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    #[test]
    fn override_paths_are_filtered() {
        let mods = ["coords_hud"];
        for ok in ["options.txt", "horizon-graphics.txt", "horizon-extras.txt", "resourcepacks/Cozy Look.zip", "horizon-mods/.horizon-mod-data/coords_hud/corner.txt"] {
            assert!(override_allowed(ok, &mods), "{ok}");
        }
        for bad in [
            "../options.txt",
            "../../.bashrc",
            "servers.dat",
            "horizon-mods/evil.hmod",
            "resourcepacks/../options.txt",
            "resourcepacks/.hidden.zip",
            "resourcepacks/pack.jar",
            "resourcepacks/a/b.zip",
            "horizon-mods/.horizon-mod-data/other_mod/x",
            "horizon-mods/.horizon-mod-data/coords_hud",
            "horizon-mods/.horizon-mod-data/coords_hud/../../../x",
            "horizon-mods/.horizon-mod-data/coords_hud/a\\..\\..\\x",
            "mods/evil.jar",
            "",
        ] {
            assert!(!override_allowed(bad, &mods), "{bad}");
        }

        let dir = scratch("filter");
        let storage = "overrides/horizon-mods/.horizon-mod-data/coords_hud/corner.txt";
        let path = write_pack(&dir, PACK, &[("overrides/options.txt", b"renderDistance:8\n"), ("overrides/resourcepacks/Cozy.zip", b"zip"), (storage, b"top-left")]);
        let pack = read_pack(&path).unwrap();
        assert!(pack.manifest.mods[0].on);
        assert_eq!(pack.overrides.len(), 3);
        let home = dir.join("home");
        extract_overrides(&path, &home).unwrap();
        assert_eq!(std::fs::read_to_string(home.join("options.txt")).unwrap(), "renderDistance:8\n");
        assert!(home.join("horizon-mods/.horizon-mod-data/coords_hud/corner.txt").is_file());
        for name in ["mods/evil.jar", "overrides/../../.bashrc", "overrides/servers.dat", "overrides/horizon-mods/evil.hmod"] {
            let path = write_pack(&dir, PACK, &[(name, b"x")]);
            assert!(read_pack(&path).is_err(), "{name} was accepted");
        }
        assert!(parse_pack_manifest(&PACK.replace("format = 1", "format = 2")).is_err());
        assert!(parse_pack_manifest(&format!("{PACK}extra = 1\n")).is_err());
        assert!(parse_pack_manifest(&format!("{PACK}{}", &PACK[PACK.find("[[mods]]").unwrap()..])).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
