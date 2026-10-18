//! GitHub as a place to get mods and plugins. Projects are repositories found by topic (`minecraft-mod`,
//! `minecraft-plugin`), versions are releases, and the download is a release's .jar. Without an account GitHub
//! allows 10 searches a minute and 60 other requests an hour, so answers are kept for 10 minutes and hitting the
//! limit is reported plainly. Project ids are "gh:owner/repo". Hosted servers use it through server/plugins.rs;
//! instances (mods only) through the functions at the end.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

use super::index::{folder_of, index_key, load_index, loaders_for, remove_project_files, safe_name, save_index, ContentIndex, IndexEntry};
use crate::core::ctx::http;
use crate::core::events::Task;
use crate::core::fsutil::safe_join;
use crate::core::instances;
use crate::core::net::{download_file, hash_file, DownloadItem, Hash, HashAlgo, HttpError};
use crate::core::types::{ContentItem, ContentKind, ContentSource, Instance, LoaderType, ProjectHit, ProjectVersion, SearchQuery, SearchResult, UpdateRef};
use crate::{bail_tr, err_tr, tr};

const API: &str = "https://api.github.com";
pub const PREFIX: &str = "gh:";
const KEEP: Duration = Duration::from_secs(600);

/// Cheat clients and their add-ons ("utility mods") have no place in a mod search. Matched by whole topic words,
/// so "hacktoberfest" doesn't count.
fn cheat_topic(topic: &str) -> bool {
    let words: Vec<&str> = topic.split('-').collect();
    words.iter().any(|w| matches!(*w, "hack" | "hacks" | "hacked" | "cheat" | "cheats" | "anarchy" | "killaura" | "bleachhack"))
        || topic.contains("utility-mod")
        || topic.contains("ghost-client")
        || topic.starts_with("meteor-client")
}

#[derive(Deserialize, Clone)]
struct Owner {
    login: String,
    #[serde(default)]
    avatar_url: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct Repo {
    pub full_name: String,
    pub name: String,
    #[serde(default)]
    description: Option<String>,
    owner: Owner,
    #[serde(default)]
    pub stargazers_count: u64,
    pub html_url: String,
    #[serde(default)]
    topics: Vec<String>,
    #[serde(default)]
    pushed_at: Option<String>,
}

impl Repo {
    pub fn id(&self) -> String {
        format!("{PREFIX}{}", self.full_name)
    }
    pub fn description(&self) -> String {
        self.description.clone().unwrap_or_default()
    }
    pub fn owner(&self) -> String {
        self.owner.login.clone()
    }
    pub fn avatar(&self) -> Option<String> {
        self.owner.avatar_url.clone()
    }
    fn wanted(&self) -> bool {
        let about = self.description().to_lowercase();
        !self.topics.iter().any(|t| cheat_topic(t)) && !about.contains("hacked client") && !about.contains("utility mod")
    }
}

#[derive(Deserialize, Clone)]
pub struct Asset {
    pub name: String,
    #[serde(rename = "browser_download_url")]
    pub url: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub download_count: u64,
    /// "sha256:<hex>" on assets uploaded since mid-2025.
    #[serde(default)]
    digest: Option<String>,
}

impl Asset {
    pub fn sha256(&self) -> Option<String> {
        self.digest.as_deref().and_then(|d| d.strip_prefix("sha256:")).map(str::to_string)
    }
}

#[derive(Deserialize, Clone)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    draft: bool,
    pub html_url: String,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

impl Release {
    pub fn title(&self) -> String {
        self.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| self.tag_name.clone())
    }
    pub fn downloads(&self) -> u64 {
        self.assets.iter().map(|a| a.download_count).sum()
    }
    /// The .jar files that are the mod or plugin itself (not sources, docs or API jars).
    pub fn jars(&self) -> Vec<&Asset> {
        self.assets
            .iter()
            .filter(|a| {
                let n = a.name.to_lowercase();
                n.ends_with(".jar") && !["-sources", "-javadoc", "-api", "-dev", "-plain", "-slim"].iter().any(|x| n.contains(x))
            })
            .collect()
    }
}

static CACHE: Mutex<Option<HashMap<String, (Instant, Value)>>> = Mutex::new(None);

/// GETs a GitHub API path, from the cache when it's fresh.
async fn get<T: DeserializeOwned>(path: &str) -> Result<T> {
    let url = format!("{API}{path}");
    let cached = CACHE.lock().unwrap().as_ref().and_then(|c| c.get(&url).filter(|(at, _)| at.elapsed() < KEEP).map(|(_, v)| v.clone()));
    let value = match cached {
        Some(v) => v,
        None => {
            let res = http().get(&url).header("Accept", "application/vnd.github+json").header("X-GitHub-Api-Version", "2022-11-28").send().await?;
            let status = res.status().as_u16();
            let out_of_requests = res.headers().get("x-ratelimit-remaining").and_then(|v| v.to_str().ok()) == Some("0");
            if (status == 403 || status == 429) && out_of_requests {
                bail!("GitHub's limit for use without an account was reached. Try again in a few minutes.");
            }
            if !(200..300).contains(&status) {
                let body = res.text().await.unwrap_or_default();
                return Err(HttpError { status, url, body }.into());
            }
            let v: Value = res.json().await?;
            CACHE.lock().unwrap().get_or_insert_with(HashMap::new).insert(url, (Instant::now(), v.clone()));
            v
        }
    };
    Ok(serde_json::from_value(value)?)
}

fn enc(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

/// `owner/repo` from a "gh:owner/repo" id (or a bare `owner/repo`), checked for shape.
pub fn repo_of(id: &str) -> Result<&str> {
    let full = id.strip_prefix(PREFIX).unwrap_or(id);
    let ok = full.split('/').count() == 2
        && full.split('/').all(|p| !p.is_empty() && p != "." && p != ".." && p.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)));
    if ok {
        Ok(full)
    } else {
        Err(anyhow!("Unknown GitHub project"))
    }
}

/// Repositories with `topic` matching `query` (the most starred first when there's no query). `offset` should
/// be a multiple of `limit`. Returns the page and the total GitHub reports (searches stop at 1000).
pub async fn search(query: &str, topic: &str, offset: usize, limit: usize) -> Result<(Vec<Repo>, u64)> {
    #[derive(Deserialize)]
    struct Found {
        total_count: u64,
        items: Vec<Repo>,
    }
    let q = format!("{} topic:{topic} archived:false", query.trim());
    let page = offset / limit.max(1) + 1;
    let found: Found = get(&format!("/search/repositories?q={}&sort=stars&order=desc&per_page={limit}&page={page}", enc(q.trim()))).await?;
    Ok((found.items.into_iter().filter(Repo::wanted).collect(), found.total_count.min(1000)))
}

pub async fn repo(id: &str) -> Result<Repo> {
    get(&format!("/repos/{}", repo_of(id)?)).await
}

/// Published releases that have a .jar to download, newest first.
pub async fn releases(id: &str) -> Result<Vec<Release>> {
    let list: Vec<Release> = get(&format!("/repos/{}/releases?per_page=30", repo_of(id)?)).await?;
    Ok(list.into_iter().filter(|r| !r.draft && !r.jars().is_empty()).collect())
}

/// A release by its tag.
pub async fn release(id: &str, tag: &str) -> Result<Release> {
    get(&format!("/repos/{}/releases/tags/{}", repo_of(id)?, enc(tag))).await
}

/// A file name's parts: "sodium-fabric-0.6.0+mc1.21.1.jar" → sodium, fabric, 0.6.0, mc1.21.1.
fn parts(name: &str) -> Vec<String> {
    let n = name.to_lowercase();
    n.trim_end_matches(".jar").split(['-', '_', '+', ' ']).filter(|p| !p.is_empty()).map(str::to_string).collect()
}

/// The Minecraft version a file-name part names: "mc1.21.1", a version from 1.7 up ("1.21", "1.20.4") or a
/// year-based one ("26.3"). Lower "1.x" numbers and longer ones ("1.4.147") are usually the mod's own version.
fn mc_part(p: &str) -> Option<&str> {
    let v = p.strip_prefix("mc").unwrap_or(p);
    // Snapshots: "25w45a".
    let b = v.as_bytes();
    if b.len() == 6 && b[..2].iter().all(u8::is_ascii_digit) && b[2] == b'w' && b[3..5].iter().all(u8::is_ascii_digit) && b[5].is_ascii_lowercase() {
        return Some(v);
    }
    let nums: Vec<&str> = v.split('.').collect();
    if !(2..=3).contains(&nums.len()) || !nums.iter().all(|n| !n.is_empty() && n.len() <= 2 && n.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    let (major, minor): (u32, u32) = (nums[0].parse().ok()?, nums[1].parse().ok()?);
    (p.starts_with("mc") || (major == 1 && (7..=29).contains(&minor)) || (26..=99).contains(&major)).then_some(v)
}

/// Whether a file name fits a Minecraft version and loaders. Names that don't say count as fitting: plenty of
/// plugins are called just "Name-1.2.jar".
pub fn fits(name: &str, mc: &str, loaders: &[&str]) -> bool {
    let parts = parts(name);
    let known = ["fabric", "quilt", "neoforge", "forge", "paper", "spigot", "bukkit", "folia", "velocity"];
    let named: Vec<&str> = known.iter().copied().filter(|l| parts.iter().any(|p| p == l)).collect();
    let loader_ok = named.is_empty() || named.iter().any(|l| loaders.contains(l));
    let versions: Vec<&str> = parts.iter().filter_map(|p| mc_part(p)).collect();
    let mc_ok = versions.is_empty() || versions.iter().any(|v| *v == mc || mc.starts_with(&format!("{v}.")));
    loader_ok && mc_ok
}

/// The jar of a release to install: one that fits (see `fits`), preferring names that say the loader or the
/// Minecraft version, then the shortest (the main file of releases that ship several modules).
pub fn pick<'a>(release: &'a Release, mc: &str, loaders: &[&str]) -> Option<&'a Asset> {
    release.jars().into_iter().filter(|a| fits(&a.name, mc, loaders)).min_by_key(|a| {
        let p = parts(&a.name);
        let says_loader = p.iter().any(|x| loaders.contains(&x.as_str()));
        let says_mc = p.iter().any(|x| x.trim_start_matches("mc") == mc);
        (!says_loader, !says_mc, a.name.len())
    })
}

// ---------- Instances (mods) ----------

/// Discover's GitHub results: mods only (the other kinds have no common topic there).
pub async fn search_hits(q: &SearchQuery) -> Result<SearchResult> {
    let offset = q.offset.unwrap_or(0) as usize;
    if q.kind != ContentKind::Mod {
        return Ok(SearchResult { hits: Vec::new(), total: 0, offset: offset as u64 });
    }
    let limit = q.limit.unwrap_or(20).clamp(1, 50) as usize;
    let (repos, total) = search(&q.query, "minecraft-mod", offset, limit).await?;
    Ok(SearchResult {
        hits: repos
            .into_iter()
            .map(|r| ProjectHit {
                id: r.id(),
                slug: r.name.clone(),
                description: r.description(),
                icon_url: r.avatar(),
                author: r.owner(),
                downloads: 0,
                follows: r.stargazers_count,
                categories: Vec::new(),
                kind: ContentKind::Mod,
                updated: r.pushed_at.clone().unwrap_or_default(),
                source: ContentSource::Github,
                url: r.html_url.clone(),
                title: r.name,
            })
            .collect(),
        total,
        offset: offset as u64,
    })
}

fn loader_names(inst: &Instance) -> Vec<String> {
    loaders_for(inst)
}

/// A project's releases as versions. With an instance, the ones with a file that fits it say so (the version
/// picker marks the others).
pub async fn versions_for(project_id: &str, instance_id: Option<&str>) -> Result<Vec<ProjectVersion>> {
    let inst = match instance_id {
        Some(id) => Some(instances::get(id).await?),
        None => None,
    };
    let loaders = inst.as_ref().map(loader_names).unwrap_or_default();
    let loaders: Vec<&str> = loaders.iter().map(String::as_str).collect();
    Ok(releases(project_id)
        .await?
        .into_iter()
        .map(|r| {
            let fit = inst.as_ref().and_then(|i| pick(&r, &i.mc_version, &loaders).map(|a| (i, a.name.clone())));
            ProjectVersion {
                id: r.tag_name.clone(),
                name: fit.as_ref().map(|(_, n)| n.clone()).unwrap_or_else(|| r.title()),
                version_number: r.title(),
                game_versions: fit.as_ref().map(|(i, _)| vec![i.mc_version.clone()]).unwrap_or_default(),
                loaders: fit.as_ref().map(|(i, _)| vec![i.loader.as_str().to_string()]).unwrap_or_default(),
                kind: if r.prerelease { "beta".into() } else { "release".into() },
                published: r.published_at.clone().unwrap_or_default(),
                downloads: r.downloads(),
            }
        })
        .collect())
}

/// The release and file to install into an instance: `tag`, else the newest full release with a file that fits
/// (else the newest pre-release that has one).
async fn choose(inst: &Instance, project_id: &str, tag: Option<&str>) -> Result<(Repo, Release, Asset)> {
    let repo = repo(project_id).await?;
    let loaders = loader_names(inst);
    let loaders: Vec<&str> = loaders.iter().map(String::as_str).collect();
    let target = format!("{} {}", inst.loader.label(), inst.mc_version);
    let found = match tag {
        Some(tag) => {
            let r = release(project_id, tag).await?;
            let a = pick(&r, &inst.mc_version, &loaders).or_else(|| r.jars().into_iter().min_by_key(|a| a.name.len())).cloned();
            a.map(|a| (r, a))
        }
        None => {
            let list = releases(project_id).await?;
            if list.is_empty() {
                bail_tr!("hostAddon.noReleases", "{name} doesn't publish its files as GitHub releases. Get it from its page: {link}", name = repo.name, link = repo.html_url);
            }
            let fit = |pre: bool| list.iter().filter(|r| r.prerelease == pre).find_map(|r| pick(r, &inst.mc_version, &loaders).map(|a| (r.clone(), a.clone())));
            fit(false).or_else(|| fit(true))
        }
    };
    let (release, asset) = found.ok_or_else(|| err_tr!("hostAddon.noVersion", "{name} has no version for {target}", name = repo.name, target = target))?;
    Ok((repo, release, asset))
}

/// Installs a GitHub mod into an instance, recording it in `idx` (an older copy of it is replaced).
pub(crate) async fn install_into(inst: &Instance, task: &Task, idx: &mut ContentIndex, project_id: &str, kind: ContentKind, tag: Option<String>) -> Result<()> {
    if kind != ContentKind::Mod {
        bail_tr!("github.modsOnly", "GitHub only offers mods here");
    }
    let (repo, release, asset) = choose(inst, project_id, tag.as_deref()).await?;
    task.update(tr!("content.installingItem", "Installing {name}…", name = repo.name), None);
    let folder = folder_of(inst, kind);
    tokio::fs::create_dir_all(&folder).await?;
    remove_project_files(inst, kind, &repo.id(), idx).await?;
    let dest = safe_join(&folder, safe_name(&asset.name)?)?;
    download_file(&DownloadItem::new(&asset.url, &dest).hash(asset.sha256().map(Hash::sha256)).size(Some(asset.size).filter(|s| *s > 0))).await?;
    idx.insert(
        index_key(kind, &asset.name),
        IndexEntry {
            project_id: repo.id(),
            version_id: release.tag_name.clone(),
            version_number: release.title(),
            title: repo.name.clone(),
            icon_url: repo.avatar(),
            sha1: hash_file(&dest, HashAlgo::Sha1).await.unwrap_or_default(),
        },
    );
    Ok(())
}

pub async fn install(instance_id: &str, project_id: &str, kind: ContentKind, tag: Option<String>) -> Result<()> {
    let inst = instances::get(instance_id).await?;
    if kind == ContentKind::Mod && inst.loader == LoaderType::Vanilla {
        bail_tr!(
            "modrinth.vanillaInstance",
            "{name} is a vanilla instance. Mods need a mod loader such as Fabric or NeoForge; create a modded instance first.",
            name = inst.name
        );
    }
    let project_id = project_id.to_string();
    Task::run(tr!("github.installing", "Installing from GitHub"), Some(inst.id.clone()), move |task| async move {
        let mut idx = load_index(&inst.id).await;
        install_into(&inst, &task, &mut idx, &project_id, kind, tag).await?;
        save_index(&inst.id, &idx).await
    })
    .await
}

/// Marks GitHub mods that have a newer release with a file that fits the instance.
pub async fn check_updates(inst: &Instance, mods: &mut [ContentItem]) -> Result<()> {
    let tracked: Vec<usize> = (0..mods.len()).filter(|&i| mods[i].project_id.as_deref().is_some_and(|p| p.starts_with(PREFIX))).collect();
    for i in tracked {
        let pid = mods[i].project_id.clone().unwrap_or_default();
        match choose(inst, &pid, None).await {
            Ok((_, release, _)) if mods[i].version_id.as_deref() != Some(release.tag_name.as_str()) => {
                mods[i].update = Some(UpdateRef { version_id: release.tag_name.clone(), version_number: release.title() });
            }
            Ok(_) => {}
            Err(e) => eprintln!("[content] update check for {} failed: {e:#}", mods[i].filename),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_fit_their_target() {
        assert!(fits("EssentialsX-2.22.0.jar", "1.21.1", &["paper", "spigot"]));
        assert!(fits("sodium-fabric-0.6.0+mc1.21.1.jar", "1.21.1", &["fabric"]));
        assert!(!fits("sodium-fabric-0.6.0+mc1.20.4.jar", "1.21.1", &["fabric"]));
        assert!(!fits("mod-neoforge-1.0.jar", "1.21.1", &["fabric"]));
        assert!(fits("mod-1.0.jar", "1.21.1", &["fabric"]));
        // Minecraft versions without "mc": 1.7 and up, and the year-based ones.
        assert!(!fits("fabric-carpet-26.3+v260915.jar", "1.21.1", &["fabric"]));
        assert!(fits("fabric-carpet-26.3+v260915.jar", "26.3", &["fabric"]));
        assert!(fits("fabric-carpet-1.21-1.4.147+v240613.jar", "1.21.1", &["fabric"]));
        assert!(!fits("fabric-carpet-1.21.2-pre3-1.4.157+v241014.jar", "1.21.1", &["fabric"]));
        assert!(fits("MyMod-1.2.0.jar", "1.21.1", &["fabric"]));
        assert!(fits("LuckPerms-Bukkit-5.5.71.jar", "1.21.8", &["paper", "spigot", "bukkit"]));
        assert!(!fits("fabric-carpet-25w45a-1.4.191+v251104.jar", "1.21.1", &["fabric"]));
    }

    #[test]
    fn cheat_clients_are_left_out() {
        for t in ["hacked-client", "utility-mod", "meteor-client-addon", "anarchy", "minecraft-hack"] {
            assert!(cheat_topic(t), "{t}");
        }
        for t in ["hacktoberfest", "minecraft-mod", "fabricmc", "minecraft-utilities"] {
            assert!(!cheat_topic(t), "{t}");
        }
    }

    #[test]
    fn ids_are_checked() {
        assert_eq!(repo_of("gh:EssentialsX/Essentials").unwrap(), "EssentialsX/Essentials");
        assert!(repo_of("gh:../x").is_err());
        assert!(repo_of("gh:a/b/c").is_err());
    }
}
