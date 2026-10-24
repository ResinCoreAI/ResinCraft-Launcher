//! Technic Platform's public API (the one the Technic Launcher uses). A pack is either a single zip
//! ("url") or a Solder server that serves one zip per mod for each build. Either way the zips are
//! unpacked over the game directory, and bin/modpack.jar carries a version.json naming the loader. Packs for
//! Minecraft 1.5.2 and older have no version.json: their modpack.jar is a jar mod (Forge and all) instead.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use futures::stream::{self, StreamExt};
use serde::Deserialize;
use serde_json::Value;

use super::details;
use super::dupes::dedupe_pack;
use super::index::{discard_instance, extract_where, lenient_u64, lenient_u64_zero, natural_cmp, open_zip, safe_name, InstanceGuard, Zip};
use super::packswitch::{Layer, PackTarget};
use crate::core::events::Task;
use crate::core::instances;
use crate::core::net::{download_all, fetch_json, DownloadItem, Hash};
use crate::core::paths;
use crate::core::types::{
    ContentKind, ContentSource, GalleryImage, Instance, LoaderType, ModpackRef, NewInstance, ProjectDetails, ProjectHit, ProjectVersion, SearchQuery, SearchResult,
};
use crate::{bail_tr, err_tr, tr};

const API: &str = "https://api.technicpack.net";
// The API refuses requests without a launcher build number; any recent one works.
const BUILD: u32 = 1170;

fn slug_of(project_id: &str) -> &str {
    project_id.strip_prefix("technic:").unwrap_or(project_id)
}

fn q(path: &str) -> String {
    format!("{API}{path}{}build={BUILD}", if path.contains('?') { '&' } else { '?' })
}

fn enc(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct Icon {
    url: Option<String>,
}

#[derive(Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
struct TechnicPack {
    #[serde(deserialize_with = "lenient_u64_zero")]
    id: u64,
    name: String,
    display_name: String,
    user: Option<String>,
    minecraft: Option<String>,
    #[serde(deserialize_with = "lenient_u64_zero")]
    ratings: u64,
    #[serde(deserialize_with = "lenient_u64_zero")]
    installs: u64,
    description: Option<String>,
    tags: Option<String>,
    version: Option<String>,
    icon: Option<Icon>,
    background: Option<Icon>,
    platform_url: String,
    /// Download of the whole pack, for packs without Solder.
    url: Option<String>,
    solder: Option<String>,
}

impl TechnicPack {
    fn url(&self) -> Option<&str> {
        self.url.as_deref().filter(|u| !u.is_empty())
    }
    fn solder(&self) -> Option<&str> {
        self.solder.as_deref().filter(|u| !u.is_empty())
    }
    fn icon(&self) -> Option<String> {
        self.icon.as_ref().and_then(|i| i.url.clone())
    }
    fn solder_url(&self, path: &str) -> String {
        let s = self.solder().unwrap_or_default();
        let base = if s.ends_with('/') { s.to_string() } else { format!("{s}/") };
        format!("{base}modpack/{}{path}", enc(&self.name))
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SolderPack {
    recommended: Option<String>,
    latest: Option<String>,
    builds: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SolderMod {
    url: String,
    md5: Option<String>,
    #[serde(deserialize_with = "lenient_u64")]
    filesize: Option<u64>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SolderBuild {
    minecraft: String,
    #[serde(deserialize_with = "lenient_u64")]
    memory: Option<u64>,
    mods: Vec<SolderMod>,
}

static PACKS: Mutex<Option<HashMap<String, Arc<TechnicPack>>>> = Mutex::new(None);

async fn pack(slug: &str) -> Result<Arc<TechnicPack>> {
    if let Some(p) = PACKS.lock().unwrap().as_ref().and_then(|m| m.get(slug).cloned()) {
        return Ok(p);
    }
    let p: Arc<TechnicPack> = Arc::new(fetch_json(&q(&format!("/modpack/{}", enc(slug)))).await?);
    PACKS.lock().unwrap().get_or_insert_with(HashMap::new).insert(slug.to_string(), p.clone());
    Ok(p)
}

fn to_hit(p: &TechnicPack) -> ProjectHit {
    let mut categories: Vec<String> = p.minecraft.iter().filter(|m| !m.is_empty()).map(|m| format!("MC {m}")).collect();
    categories.extend(p.tags.as_deref().unwrap_or_default().split(',').filter(|t| !t.is_empty()).map(str::to_string));
    ProjectHit {
        id: format!("technic:{}", p.name),
        slug: p.name.clone(),
        title: p.display_name.clone(),
        description: p.description.clone().unwrap_or_default(),
        icon_url: p.icon(),
        author: p.user.clone().unwrap_or_else(|| "Unknown".into()),
        downloads: p.installs,
        follows: p.ratings,
        categories,
        kind: ContentKind::Modpack,
        // The API doesn't say when a pack was last updated.
        updated: String::new(),
        source: ContentSource::Technic,
        url: p.platform_url.clone(),
    }
}

pub async fn search(sq: &SearchQuery) -> Result<SearchResult> {
    if sq.kind != ContentKind::Modpack {
        return Ok(SearchResult { hits: vec![], total: 0, offset: 0 });
    }
    #[derive(Deserialize)]
    struct Slug {
        slug: String,
    }
    #[derive(Deserialize)]
    struct Res {
        modpacks: Vec<Slug>,
    }
    let term = sq.query.trim();
    // Search returns at most ~15 matches and trending ~20, both without details; fetch those per pack.
    let res: Res = fetch_json(&if term.is_empty() { q("/trending") } else { q(&format!("/search?q={}", enc(term))) }).await?;
    let packs: Vec<Option<Arc<TechnicPack>>> = stream::iter(res.modpacks).map(|h| async move { pack(&h.slug).await.ok() }).buffered(16).collect().await;
    let mut list: Vec<Arc<TechnicPack>> = packs.into_iter().flatten().filter(|p| p.url().is_some() || p.solder().is_some()).collect();
    if let Some(gv) = sq.game_version.as_deref().filter(|v| !v.is_empty()) {
        list.retain(|p| p.minecraft.as_deref() == Some(gv));
    }
    match sq.sort.as_deref() {
        Some("downloads") => list.sort_by_key(|p| std::cmp::Reverse(p.installs)),
        Some("follows") => list.sort_by_key(|p| std::cmp::Reverse(p.ratings)),
        Some("newest") => list.sort_by_key(|p| std::cmp::Reverse(p.id)),
        _ => {}
    }
    let offset = sq.offset.unwrap_or(0) as usize;
    let limit = sq.limit.unwrap_or(20) as usize;
    Ok(SearchResult { hits: list.iter().skip(offset).take(limit).map(|p| to_hit(p)).collect(), total: list.len() as u64, offset: offset as u64 })
}

pub async fn details(project_id: &str) -> Result<ProjectDetails> {
    let p = pack(slug_of(project_id)).await?;
    Ok(ProjectDetails {
        // Plain text; Technic has no long description, changelogs or screenshots, only the pack's background.
        body: details::markdown(p.description.as_deref().unwrap_or_default(), true),
        gallery: p.background.iter().filter_map(|b| b.url.clone()).map(|url| GalleryImage { url, thumb: None, title: None, description: None }).collect(),
        links: Vec::new(),
        license: None,
        changelogs: false,
        dependencies: false,
        hit: to_hit(&p),
    })
}

fn row(id: &str, mc: Option<String>, kind: &str) -> ProjectVersion {
    ProjectVersion {
        id: id.into(),
        name: id.into(),
        version_number: id.into(),
        game_versions: vec![mc.filter(|m| !m.is_empty()).unwrap_or_else(|| "?".into())],
        // Only known after downloading the pack (it's inside bin/modpack.jar).
        loaders: vec![],
        kind: kind.into(),
        published: String::new(),
        downloads: 0,
    }
}

pub async fn versions(project_id: &str) -> Result<Vec<ProjectVersion>> {
    let p = pack(slug_of(project_id)).await?;
    if p.solder().is_none() {
        return Ok(vec![row(p.version.as_deref().unwrap_or("latest"), p.minecraft.clone(), "release")]);
    }
    let s: SolderPack = fetch_json(&p.solder_url("")).await?;
    let mut builds = s.builds.clone();
    builds.sort_by(|a, b| natural_cmp(b, a));
    builds.truncate(40);
    let rec = s.recommended.as_ref().and_then(|r| builds.iter().position(|b| b == r));
    let rows = stream::iter(builds.into_iter().enumerate())
        .map(|(i, b)| {
            let p = p.clone();
            async move {
                let mc = match fetch_json::<SolderBuild>(&p.solder_url(&format!("/{}", enc(&b)))).await {
                    Ok(x) => Some(x.minecraft),
                    Err(_) => p.minecraft.clone(),
                };
                // Builds newer than the author's recommended one are effectively betas.
                let beta = matches!(rec, Some(r) if r > 0 && i < r);
                row(&b, mc, if beta { "beta" } else { "release" })
            }
        })
        .buffered(10)
        .collect::<Vec<_>>()
        .await;
    Ok(rows)
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct VersionArgs {
    game: Vec<Value>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct VersionLib {
    name: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct VersionJson {
    id: Option<String>,
    main_class: Option<String>,
    inherits_from: Option<String>,
    arguments: Option<VersionArgs>,
    libraries: Option<Vec<VersionLib>>,
}

enum Found {
    Json(VersionJson),
    None,
    Unreadable,
    /// bin/modpack.jar without a version.json: the jar itself, merged into the game jar on old packs.
    JarMod(Vec<u8>),
}

/// Reads bin/version.json, or the version.json inside bin/modpack.jar, from whichever zip has it.
fn read_version_json(zips: &mut [Zip]) -> Result<Found> {
    for zip in zips.iter_mut() {
        if let Some(text) = super::index::read_zip_text(zip, "bin/version.json")? {
            return Ok(Found::Json(serde_json::from_str(&text)?));
        }
        let mut jar = Vec::new();
        match zip.by_name("bin/modpack.jar") {
            Ok(mut e) => {
                e.read_to_end(&mut jar)?;
            }
            Err(_) => continue,
        }
        let inner = (|| -> Result<Option<VersionJson>> {
            let mut z = zip::ZipArchive::new(std::io::Cursor::new(&jar))?;
            Ok(match super::index::read_zip_text(&mut z, "version.json")? {
                Some(t) => Some(serde_json::from_str(&t)?),
                None => None,
            })
        })();
        return Ok(match inner {
            Ok(Some(v)) => Found::Json(v),
            Ok(None) => Found::JarMod(jar),
            Err(_) => Found::Unreadable,
        });
    }
    Ok(Found::None)
}

/// Works out the loader from the pack's version.json (the Technic Launcher's own format).
pub fn detect_loader(v: &VersionJson) -> Option<(LoaderType, Option<String>)> {
    let args: Vec<String> = v
        .arguments
        .as_ref()
        .map(|a| a.game.iter().map(|x| x.as_str().map(str::to_string).unwrap_or_else(|| x.to_string())).collect())
        .unwrap_or_default();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned());
    let lib = |prefix: &str| {
        v.libraries.as_ref()?.iter().find(|l| l.name.starts_with(prefix)).and_then(|l| l.name.split(':').nth(2).map(str::to_string))
    };

    if let Some(neo) = arg("--fml.neoForgeVersion").or_else(|| lib("net.neoforged:neoforge:")) {
        return Some((LoaderType::Neoforge, Some(neo)));
    }
    // NeoForge for 1.20.1 still calls itself forge, under the net.neoforged group.
    if let Some(neo) = lib("net.neoforged:forge:") {
        return Some((LoaderType::Neoforge, Some(neo)));
    }
    if let Some(forge) = arg("--fml.forgeVersion").or_else(|| lib("net.minecraftforge:fmlloader:")).or_else(|| lib("net.minecraftforge:forge:")) {
        return Some((LoaderType::Forge, Some(forge)));
    }
    if let Some(fabric) = lib("net.fabricmc:fabric-loader:") {
        return Some((LoaderType::Fabric, Some(fabric)));
    }
    if let Some(quilt) = lib("org.quiltmc:quilt-loader:") {
        return Some((LoaderType::Quilt, Some(quilt)));
    }
    if v.main_class.as_deref() == Some("net.minecraft.client.main.Main") {
        return Some((LoaderType::Vanilla, None));
    }
    None
}

pub async fn install_modpack(project_id: &str, version_id: Option<String>) -> Result<Instance> {
    let p = pack(slug_of(project_id)).await?;
    let mut mc = p.minecraft.clone().filter(|m| !m.is_empty());
    let mut build = p.version.clone().unwrap_or_else(|| "latest".into());
    let mut memory_mb: Option<u32> = None;
    let tmp = paths::cache().join("technic").join(format!("{}-{}", safe_name(&p.name)?, &uuid::Uuid::new_v4().simple().to_string()[..6]));
    let mut files: Vec<DownloadItem> = Vec::new();

    if p.solder().is_some() {
        let s: SolderPack = fetch_json(&p.solder_url("")).await?;
        build = version_id.or(s.recommended.clone()).or(s.latest.clone()).or(s.builds.last().cloned()).unwrap_or_default();
        if build.is_empty() {
            bail_tr!("technic.noBuild", "{name} has no build to install", name = p.display_name);
        }
        let b: SolderBuild = fetch_json(&p.solder_url(&format!("/{}", enc(&build)))).await?;
        mc = Some(b.minecraft.clone()).filter(|m| !m.is_empty());
        memory_mb = b.memory.filter(|m| *m > 0).map(|m| m as u32);
        for (i, m) in b.mods.iter().enumerate() {
            let dest = tmp.join(format!("{i}.zip"));
            let hash = m.md5.as_ref().filter(|s| !s.is_empty()).map(Hash::md5);
            files.push(DownloadItem::new(&m.url, dest).hash(hash).size(m.filesize.filter(|s| *s > 0)));
        }
    } else {
        let url = p.url().ok_or_else(|| err_tr!("technic.noDownload", "{name} has no download on Technic", name = p.display_name))?;
        files.push(DownloadItem::new(url, tmp.join("pack.zip")));
    }
    let mc = mc.ok_or_else(|| err_tr!("modpack.noMcVersion", "{name} doesn't say which Minecraft version it needs", name = p.display_name))?;

    let tmp2 = tmp.clone();
    let res = Task::run(tr!("modpack.installing", "Installing {name}", name = p.display_name), None, move |task| async move {
        let paths_: Vec<PathBuf> = files.iter().map(|f| f.dest.clone()).collect();
        download_all(files, 4, task.counter(tr!("technic.downloading", "Downloading modpack"))).await?;
        task.update(tr!("technic.reading", "Reading modpack…"), None);
        let name = p.display_name.clone();
        let (zips, found) = tokio::task::spawn_blocking(move || -> Result<(Vec<Zip>, Found)> {
            let mut zips = Vec::new();
            for f in &paths_ {
                zips.push(open_zip(f).map_err(|_| {
                    err_tr!("technic.notZip", "{name}'s download isn't a zip file. The author's download link may be broken.", name = name)
                })?);
            }
            let found = read_version_json(&mut zips)?;
            Ok((zips, found))
        })
        .await??;
        let mut jar_mod = None;
        let (vj, (loader, loader_version)) = match found {
            // Before Minecraft 1.6, the jar is the game's patch itself (usually Forge and the pack's base mods).
            Found::JarMod(bytes) if is_jar_mod_era(&mc) => {
                jar_mod = Some(bytes);
                (None, (LoaderType::Vanilla, None))
            }
            Found::Unreadable | Found::JarMod(_) => {
                bail_tr!(
                    "technic.customModpackJar",
                    "{name} ships a custom bin/modpack.jar that ResinCraft can't read, so it can't tell which mod loader to install.",
                    name = p.display_name
                )
            }
            Found::None => (None, (LoaderType::Vanilla, None)),
            Found::Json(v) => {
                let Some(l) = detect_loader(&v) else {
                    if let Some(id) = v.id.as_ref().filter(|i| !i.is_empty()) {
                        bail_tr!("technic.unknownLoaderId", "{name} uses a mod loader ResinCraft doesn't recognise ({id}).", name = p.display_name, id = id);
                    }
                    bail_tr!("technic.unknownLoader", "{name} uses a mod loader ResinCraft doesn't recognise.", name = p.display_name);
                };
                (Some(v), l)
            }
        };

        // The loader's version.json is more reliable than the version typed into the Technic website.
        let mc_version = vj.as_ref().and_then(|v| v.inherits_from.clone()).filter(|m| !m.is_empty()).unwrap_or(mc);
        let modpack = ModpackRef { name: p.display_name.clone(), project_id: Some(format!("technic:{}", p.name)), version_id: Some(build.clone()) };
        let icon = p.icon();
        let new = NewInstance { name: p.display_name.clone(), mc_version, loader, loader_version, color: None, group: None };
        let inst = instances::create(new, move |i| {
            i.icon_url = icon;
            i.memory_mb = memory_mb;
            i.modpack = Some(modpack);
        })
        .await?;
        let guard = InstanceGuard::new(&inst.id);
        // On the Library at once, with this task's progress on its card.
        task.set_instance(&inst.id);
        crate::core::events::list_changed("instances");
        if let Some(bytes) = jar_mod {
            if let Err(e) = add_modpack_jar(&inst.id, bytes).await {
                discard_instance(&inst.id).await;
                return Err(e);
            }
        }
        let dir = paths::game_dir(&inst.id);
        task.update(tr!("technic.unpacking", "Unpacking modpack…"), None);
        let unpacked = tokio::task::spawn_blocking(move || -> Result<()> {
            for mut zip in zips {
                // bin/ holds the Technic Launcher's own loader jar; ResinCraft installs the loader itself.
                extract_where(&mut zip, &dir, |name| (!name.get(..4).is_some_and(|p| p.eq_ignore_ascii_case("bin/"))).then(|| name.to_string()))?;
            }
            Ok(())
        })
        .await
        .map_err(anyhow::Error::from)
        .and_then(|r| r);
        if let Err(e) = unpacked {
            discard_instance(&inst.id).await;
            return Err(e);
        }
        // Two of the pack's zips may carry the same mod; Technic lists none of them as such.
        dedupe_pack(&inst, &task, &[]).await;
        guard.keep();
        let inst = instances::get(&inst.id).await?;
        super::packswitch::record_install(&inst).await;
        Ok(inst)
    })
    .await;
    let _ = tokio::fs::remove_dir_all(&tmp2).await;
    res
}

/// Minecraft 1.5.2 and older, where packs patch the game jar.
fn is_jar_mod_era(mc: &str) -> bool {
    crate::game::loaders::is_jar_mod_forge(mc)
}

/// Keeps an old pack's bin/modpack.jar as the instance's jar mod.
async fn add_modpack_jar(id: &str, bytes: Vec<u8>) -> Result<()> {
    let dir = crate::game::jarmods::jar_dir(id);
    tokio::fs::create_dir_all(&dir).await?;
    tokio::fs::write(dir.join("modpack.jar"), bytes).await?;
    let mut inst = instances::get(id).await?;
    inst.jar_mods = vec![crate::core::types::JarMod { file: "modpack.jar".into(), name: "modpack.jar".into(), disabled: false }];
    instances::save(&inst).await
}

/// Build `version_id` of Technic pack `project_id`, for switching an instance to it (see packswitch.rs). Downloads
/// the build's zips into `work`. A pack without Solder only has its current download.
pub(crate) async fn pack_target(project_id: &str, version_id: &str, work: &Path, task: &Arc<Task>) -> Result<PackTarget> {
    let p = pack(slug_of(project_id)).await?;
    let mut mc = p.minecraft.clone().filter(|m| !m.is_empty());
    let mut items = Vec::new();
    if p.solder().is_some() {
        let b: SolderBuild = fetch_json(&p.solder_url(&format!("/{}", enc(version_id)))).await?;
        mc = Some(b.minecraft.clone()).filter(|m| !m.is_empty()).or(mc);
        for (i, m) in b.mods.iter().enumerate() {
            let hash = m.md5.as_ref().filter(|s| !s.is_empty()).map(Hash::md5);
            items.push(DownloadItem::new(&m.url, work.join(format!("{i}.zip"))).hash(hash).size(m.filesize.filter(|s| *s > 0)));
        }
    } else {
        let current = p.version.clone().unwrap_or_else(|| "latest".into());
        if version_id != current {
            bail_tr!("technic.onlyLatest", "{name} only offers its current version ({version}) for download", name = p.display_name.clone(), version = current);
        }
        let url = p.url().ok_or_else(|| err_tr!("technic.noDownload", "{name} has no download on Technic", name = p.display_name.clone()))?;
        items.push(DownloadItem::new(url, work.join("pack.zip")));
    }
    let zips: Vec<PathBuf> = items.iter().map(|i| i.dest.clone()).collect();
    download_all(items, 4, task.counter(tr!("technic.downloading", "Downloading modpack"))).await?;
    let name = p.display_name.clone();
    let paths_ = zips.clone();
    let found = tokio::task::spawn_blocking(move || -> Result<Found> {
        let mut opened = Vec::new();
        for f in &paths_ {
            opened.push(open_zip(f).map_err(|_| err_tr!("technic.notZip", "{name}'s download isn't a zip file. The author's download link may be broken.", name = name.clone()))?);
        }
        read_version_json(&mut opened)
    })
    .await??;
    let (vj, (loader, loader_version)) = match found {
        Found::JarMod(_) if mc.as_deref().is_some_and(is_jar_mod_era) => bail_tr!(
            "technic.jarModSwitch",
            "{name} patches the game jar, so ResinCraft can't switch it to another version. Install that version as a new instance instead.",
            name = p.display_name.clone()
        ),
        Found::Unreadable | Found::JarMod(_) => bail_tr!(
            "technic.customModpackJar",
            "{name} ships a custom bin/modpack.jar that ResinCraft can't read, so it can't tell which mod loader to install.",
            name = p.display_name.clone()
        ),
        Found::None => (None, (LoaderType::Vanilla, None)),
        Found::Json(v) => {
            let l = detect_loader(&v).ok_or_else(|| err_tr!("technic.unknownLoader", "{name} uses a mod loader ResinCraft doesn't recognise.", name = p.display_name.clone()))?;
            (Some(v), l)
        }
    };
    let mc_version = vj
        .and_then(|v| v.inherits_from)
        .filter(|m| !m.is_empty())
        .or(mc)
        .ok_or_else(|| err_tr!("modpack.noMcVersion", "{name} doesn't say which Minecraft version it needs", name = p.display_name.clone()))?;
    Ok(PackTarget {
        version_id: version_id.to_string(),
        version_name: version_id.to_string(),
        mc_version,
        loader,
        loader_version,
        files: vec![],
        layers: zips.into_iter().map(|zip| Layer { zip, prefix: String::new(), to: String::new(), skip_bin: true }).collect(),
        pending: vec![],
        index: vec![],
        server_pack: false,
    })
}

/// The build a Technic pack recommends now (its only download for a pack without Solder), for the update check.
pub(crate) async fn recommended_build(project_id: &str) -> Result<Option<String>> {
    let p = pack(slug_of(project_id)).await?;
    if p.solder().is_none() {
        return Ok(p.version.clone().filter(|v| !v.is_empty()));
    }
    let s: SolderPack = fetch_json(&p.solder_url("")).await?;
    Ok(s.recommended.or(s.latest).filter(|b| !b.is_empty()))
}

/// Drops a cached pack, so the update check sees builds published since it was fetched.
pub(crate) fn forget_cache(project_id: &str) {
    if let Some(m) = PACKS.lock().unwrap().as_mut() {
        m.remove(slug_of(project_id));
    }
}

/// The mods in a Technic pack build: a Solder pack lists them (as zips named after the mod); others don't.
pub async fn pack_mods(project_id: &str, version_id: &str) -> Result<Vec<crate::core::types::PackMod>> {
    use crate::core::types::PackMod;
    let p = pack(slug_of(project_id)).await?;
    if p.solder().is_none() {
        return Ok(Vec::new());
    }
    let b: SolderBuild = fetch_json(&p.solder_url(&format!("/{}", enc(version_id)))).await?;
    Ok(b.mods
        .iter()
        .filter_map(|m| {
            let name = m.url.rsplit('/').next()?.split('?').next()?;
            // "jei-1.12.2-4.16.zip" → "jei 1.12.2 4.16"
            let title = name.strip_suffix(".zip").unwrap_or(name).replace(['-', '_'], " ");
            Some(PackMod { title, id: None, source: None, icon_url: None, file: Some(name.to_string()) })
        })
        .collect())
}
