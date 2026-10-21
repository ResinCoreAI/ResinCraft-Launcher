//! Fabric / Quilt / Forge / NeoForge version lists and version resolution
//! (the Forge/NeoForge installer itself lives in `forge.rs`).

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use futures::future::BoxFuture;
use serde::Deserialize;

use crate::core::events::Task;
use crate::core::fsutil::{read_json, write_json};
use crate::core::net::{fetch_json, HttpError};
use crate::core::paths;
use crate::core::types::{LoaderType, LoaderVersion};
use crate::{bail_tr, err_tr};

use super::mojang::{get_vanilla_version, merge_versions, VersionJson};

pub(crate) const FORGE_MAVEN: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge";
const FORGE_PROMOS: &str = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
pub(crate) const NEOFORGE_MAVEN: &str = "https://maven.neoforged.net/releases/net/neoforged";

fn meta(loader: LoaderType) -> &'static str {
    match loader {
        LoaderType::Quilt => "https://meta.quiltmc.org/v3",
        _ => "https://meta.fabricmc.net/v2",
    }
}

fn enc(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

#[derive(Deserialize)]
struct MetaLoader {
    version: String,
    #[serde(default)]
    stable: Option<bool>,
}
#[derive(Deserialize)]
struct MetaEntry {
    loader: MetaLoader,
}

fn prerelease_re() -> regex::Regex {
    regex::Regex::new(r"(?i)-(beta|alpha|pre|rc)").unwrap()
}

pub async fn loader_versions(loader: LoaderType, mc_version: &str) -> Result<Vec<LoaderVersion>> {
    match loader {
        LoaderType::Vanilla => Ok(Vec::new()),
        LoaderType::Forge => forge_versions(mc_version).await,
        LoaderType::Neoforge => neoforge_versions(mc_version).await,
        LoaderType::Fabric | LoaderType::Quilt => {
            let mut list: Vec<MetaEntry> = fetch_json(&format!("{}/versions/loader/{}", meta(loader), enc(mc_version))).await?;
            // Quilt's meta isn't sorted (e.g. 0.20.0-beta.9 comes before 0.30.1); list newest first like Fabric's.
            if loader == LoaderType::Quilt {
                list.sort_by(|a, b| compare_versions(&b.loader.version, &a.loader.version));
            }
            let pre = prerelease_re();
            Ok(list
                .into_iter()
                .map(|e| LoaderVersion {
                    // Quilt's meta has no "stable" flag; pre-releases carry a suffix instead.
                    stable: e.loader.stable.unwrap_or_else(|| !pre.is_match(&e.loader.version)),
                    version: e.loader.version,
                })
                .collect())
        }
    }
}

pub(crate) fn legacy_forge_error(mc: &str) -> anyhow::Error {
    err_tr!(
        "loader.tooOldForge",
        "ResinCraft supports Forge for Minecraft 1.7.10 and newer. Minecraft {mc} needs an older Forge that isn't supported.",
        mc = mc
    )
}

/// Newest stable loader version for a game version.
pub async fn latest_loader(loader: LoaderType, mc_version: &str) -> Result<String> {
    if loader == LoaderType::Forge && is_ancient_forge(mc_version) {
        return Err(legacy_forge_error(mc_version));
    }
    let versions = loader_versions(loader, mc_version).await?;
    let pick = versions.iter().find(|v| v.stable).or(versions.first());
    match pick {
        Some(v) => Ok(v.version.clone()),
        None => bail_tr!("loader.notAvailable", "{loader} isn't available for Minecraft {mc} yet", loader = loader.label(), mc = mc_version),
    }
}

/// Accepts loader versions as other launchers and pack formats write them ("forge-47.2.0", "1.20.1-47.2.0",
/// "neoforge-21.1.77") and returns the form ResinCraft stores ("47.2.0", "21.1.77").
pub fn normalize_loader_version(loader: LoaderType, mc_version: &str, version: &str) -> String {
    let mut v = version.trim().to_string();
    let forgeish = matches!(loader, LoaderType::Forge | LoaderType::Neoforge);
    if forgeish {
        let lower = v.to_ascii_lowercase();
        for p in ["neoforge-", "forge-"] {
            if lower.starts_with(p) {
                v = v[p.len()..].to_string();
                break;
            }
        }
    }
    if loader == LoaderType::Forge || (loader == LoaderType::Neoforge && mc_version == "1.20.1") {
        if let Some(rest) = v.strip_prefix(&format!("{mc_version}-")) {
            v = rest.to_string();
        }
    }
    v
}

/// Picks the Java used to run Forge/NeoForge installer processors, given the required major version.
pub type JavaPicker = Box<dyn Fn(u32) -> BoxFuture<'static, Result<String>> + Send + Sync>;

#[derive(Default)]
pub struct ResolveContext {
    pub task: Option<Arc<Task>>,
    pub concurrency: Option<usize>,
    pub java: Option<JavaPicker>,
}

/// Returns the full, launchable version JSON for an instance's game + loader combination.
pub async fn resolve_version(mc_version: &str, loader: LoaderType, loader_version: Option<&str>, ctx: &ResolveContext) -> Result<VersionJson> {
    let loader_version = loader_version.filter(|v| !v.is_empty());
    if matches!(loader, LoaderType::Forge | LoaderType::Neoforge) {
        let ver = match loader_version {
            Some(v) => normalize_loader_version(loader, mc_version, v),
            None => latest_loader(loader, mc_version).await?,
        };
        return super::forge::resolve_forge_like(mc_version, loader, &ver, ctx).await;
    }

    let vanilla = get_vanilla_version(mc_version).await?;
    if loader == LoaderType::Vanilla {
        return Ok(vanilla);
    }

    let ver = match loader_version {
        Some(v) => v.to_string(),
        None => latest_loader(loader, mc_version).await?,
    };
    let file = paths::versions().join(format!("{}-{mc_version}-{ver}", loader.as_str())).join("profile.json");
    let url = format!("{}/versions/loader/{}/{}/profile/json", meta(loader), enc(mc_version), enc(&ver));
    // Saved profile first, so launching doesn't wait on the network; a background fetch keeps it current
    // for the next start.
    let profile: VersionJson = match read_json::<VersionJson>(&file).await {
        Some(p) => {
            tokio::spawn(async move {
                if let Ok(p) = fetch_json::<VersionJson>(&url).await {
                    let _ = write_json(&file, &p).await;
                }
            });
            p
        }
        None => {
            let p = fetch_json::<VersionJson>(&url).await?;
            write_json(&file, &p).await?;
            p
        }
    };
    Ok(merge_versions(&vanilla, &profile))
}

// ---------- Forge & NeoForge version lists ----------

/// Splits "1.2.3-beta" into ("1.2.3", Some("beta")).
fn split_suffix(v: &str) -> (&str, Option<&str>) {
    match v.split_once('-') {
        Some((a, b)) => (a, Some(b)),
        None => (v, None),
    }
}

/// Like JS `localeCompare(…, {numeric: true})`: digit runs compare as numbers.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let mut n1 = String::new();
                while let Some(c) = x.peek().copied().filter(char::is_ascii_digit) {
                    n1.push(c);
                    x.next();
                }
                let mut n2 = String::new();
                while let Some(d) = y.peek().copied().filter(char::is_ascii_digit) {
                    n2.push(d);
                    y.next();
                }
                let (t1, t2) = (n1.trim_start_matches('0'), n2.trim_start_matches('0'));
                let o = t1.len().cmp(&t2.len()).then_with(|| t1.cmp(t2));
                if o != Ordering::Equal {
                    return o;
                }
            }
            (Some(c), Some(d)) => {
                let o = c.to_lowercase().cmp(d.to_lowercase()).then(c.cmp(&d));
                if o != Ordering::Equal {
                    return o;
                }
                x.next();
                y.next();
            }
        }
    }
}

/// JS `Number(s)`: blank is 0, junk is NaN (None).
fn js_number(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        Some(0.0)
    } else {
        t.parse().ok()
    }
}

/// Compares dotted versions numerically; a pre-release suffix sorts below the plain release.
pub(crate) fn compare_versions(a: &str, b: &str) -> Ordering {
    let (ca, sa) = split_suffix(a);
    let (cb, sb) = split_suffix(b);
    let pa: Vec<&str> = ca.split('.').collect();
    let pb: Vec<&str> = cb.split('.').collect();
    for i in 0..pa.len().max(pb.len()) {
        // Missing parts count as 0; parts that aren't numbers are skipped (as NaN is in the TS version).
        let x = pa.get(i).map_or(Some(0.0), |s| js_number(s));
        let y = pb.get(i).map_or(Some(0.0), |s| js_number(s));
        if let (Some(x), Some(y)) = (x, y) {
            if x != y {
                return x.partial_cmp(&y).unwrap_or(Ordering::Equal);
            }
        }
    }
    let (sa, sb) = (sa.filter(|s| !s.is_empty()), sb.filter(|s| !s.is_empty()));
    match (sa, sb) {
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        _ => natural_cmp(sa.unwrap_or(""), sb.unwrap_or("")),
    }
}

fn minor_of(mc: &str) -> Option<u32> {
    let rest = mc.strip_prefix("1.")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Forge for Minecraft ≤1.12 ships the old installer format (install + versionInfo, no processors).
pub(crate) fn is_legacy_forge(mc: &str) -> bool {
    minor_of(mc).is_some_and(|n| n < 13)
}

/// Minecraft 1.12.2 and older (for tools that check how far an old game got).
pub fn is_legacy_mc(mc: &str) -> bool {
    is_legacy_forge(mc)
}

/// Forge for Minecraft ≤1.6 loads its libraries from servers that are gone; not supported.
pub(crate) fn is_ancient_forge(mc: &str) -> bool {
    minor_of(mc).is_some_and(|n| n < 7)
}

/// Old Forge builds carry a branch suffix in their Maven version ("10.13.4.1614-1.7.10"), which pack
/// formats leave out ("forge-10.13.4.1614"). Finds the published name; unchanged when nothing matches.
pub(crate) async fn full_forge_version(mc: &str, ver: &str) -> String {
    if !is_legacy_forge(mc) {
        return ver.to_string();
    }
    let Ok(xml) = cached_text(&format!("{FORGE_MAVEN}/maven-metadata.xml"), "forge-metadata.xml").await else {
        return ver.to_string();
    };
    pick_full_forge_version(&maven_versions(&xml), mc, ver)
}

fn pick_full_forge_version(published: &[String], mc: &str, ver: &str) -> String {
    let exact = format!("{mc}-{ver}");
    if published.iter().any(|v| *v == exact) {
        return ver.to_string();
    }
    let branch = format!("{exact}-");
    published.iter().find(|v| v.starts_with(&branch)).and_then(|v| v.strip_prefix(&format!("{mc}-"))).map(str::to_string).unwrap_or_else(|| ver.to_string())
}

static TEXT_CACHE: Mutex<Option<HashMap<String, (Instant, String)>>> = Mutex::new(None);

async fn fetch_text(url: &str) -> Result<String> {
    let res = crate::core::ctx::http().get(url).send().await?;
    if !res.status().is_success() {
        return Err(HttpError { status: res.status().as_u16(), url: url.to_string(), body: String::new() }.into());
    }
    Ok(res.text().await?)
}

/// Fetches a (large) metadata file, keeping it for 10 minutes and on disk for offline use.
async fn cached_text(url: &str, name: &str) -> Result<String> {
    if let Some((at, text)) = TEXT_CACHE.lock().unwrap().as_ref().and_then(|m| m.get(url)) {
        if at.elapsed() < Duration::from_secs(600) {
            return Ok(text.clone());
        }
    }
    let file = paths::cache().join(name);
    let text = match fetch_text(url).await {
        Ok(t) => {
            tokio::fs::create_dir_all(paths::cache()).await?;
            tokio::fs::write(&file, &t).await?;
            t
        }
        Err(e) => {
            let t = tokio::fs::read_to_string(&file).await.unwrap_or_default();
            if t.is_empty() {
                return Err(e);
            }
            t
        }
    };
    TEXT_CACHE.lock().unwrap().get_or_insert_with(HashMap::new).insert(url.to_string(), (Instant::now(), text.clone()));
    Ok(text)
}

fn maven_versions(xml: &str) -> Vec<String> {
    let re = regex::Regex::new(r"<version>([^<]+)</version>").unwrap();
    re.captures_iter(xml).map(|c| c[1].trim().to_string()).collect()
}

#[derive(Deserialize)]
struct Promotions {
    #[serde(default)]
    promos: HashMap<String, String>,
}

async fn forge_versions(mc: &str) -> Result<Vec<LoaderVersion>> {
    if is_ancient_forge(mc) {
        return Ok(Vec::new());
    }
    let metadata_url = format!("{FORGE_MAVEN}/maven-metadata.xml");
    let (xml, promos) = tokio::try_join!(cached_text(&metadata_url, "forge-metadata.xml"), async {
        let t = cached_text(FORGE_PROMOS, "forge-promotions.json").await?;
        Ok::<_, anyhow::Error>(serde_json::from_str::<Promotions>(&t)?.promos)
    })?;
    let prefix = format!("{mc}-");
    let mut list: Vec<String> = maven_versions(&xml).into_iter().filter_map(|v| v.strip_prefix(&prefix).map(str::to_string)).collect();
    list.sort_by(|a, b| compare_versions(b, a));
    // Builds newer than the promoted "recommended" one haven't been vetted yet.
    let recommended = promos.get(&format!("{mc}-recommended"));
    Ok(list
        .into_iter()
        .map(|version| LoaderVersion {
            stable: recommended.map(|r| compare_versions(&version, r) != Ordering::Greater).unwrap_or(true),
            version,
        })
        .collect())
}

/// NeoForge versions encode the game version: 21.1.x → 1.21.1, 26.1.0.x → 26.1, 26.1.1.x → 26.1.1.
fn neoforge_game_version(v: &str) -> Option<String> {
    if v.contains('+') {
        return None; // snapshot builds
    }
    let p: Vec<&str> = v.split('-').next().unwrap_or_default().split('.').collect();
    match p.len() {
        4 => Some(if p[2] == "0" { format!("{}.{}", p[0], p[1]) } else { format!("{}.{}.{}", p[0], p[1], p[2]) }),
        3 => Some(if p[1] == "0" { format!("1.{}", p[0]) } else { format!("1.{}.{}", p[0], p[1]) }),
        _ => None,
    }
}

async fn neoforge_versions(mc: &str) -> Result<Vec<LoaderVersion>> {
    // NeoForge's first release (for 1.20.1) was still published as a Forge fork.
    let legacy = mc == "1.20.1";
    let xml = if legacy {
        cached_text(&format!("{NEOFORGE_MAVEN}/forge/maven-metadata.xml"), "neoforge-1.20.1-metadata.xml").await?
    } else {
        cached_text(&format!("{NEOFORGE_MAVEN}/neoforge/maven-metadata.xml"), "neoforge-metadata.xml").await?
    };
    let mut list: Vec<String> = if legacy {
        maven_versions(&xml).into_iter().filter_map(|v| v.strip_prefix("1.20.1-").map(str::to_string)).collect()
    } else {
        maven_versions(&xml).into_iter().filter(|v| neoforge_game_version(v).as_deref() == Some(mc)).collect()
    };
    list.sort_by(|a, b| compare_versions(b, a));
    let pre = regex::Regex::new(r"(?i)-(beta|alpha)").unwrap();
    Ok(list.into_iter().map(|version| LoaderVersion { stable: !pre.is_match(&version), version }).collect())
}

pub(crate) fn installer_url(loader: LoaderType, mc: &str, ver: &str) -> String {
    if loader == LoaderType::Forge {
        return format!("{FORGE_MAVEN}/{mc}-{ver}/forge-{mc}-{ver}-installer.jar");
    }
    if mc == "1.20.1" {
        return format!("{NEOFORGE_MAVEN}/forge/{mc}-{ver}/forge-{mc}-{ver}-installer.jar");
    }
    format!("{NEOFORGE_MAVEN}/neoforge/{ver}/neoforge-{ver}-installer.jar")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare() {
        assert_eq!(compare_versions("47.2.0", "47.10.0"), Ordering::Less);
        assert_eq!(compare_versions("21.1.0-beta", "21.1.0"), Ordering::Less);
        assert_eq!(compare_versions("21.1.0-beta.10", "21.1.0-beta.9"), Ordering::Greater);
    }

    #[test]
    fn normalize() {
        assert_eq!(normalize_loader_version(LoaderType::Forge, "1.20.1", "forge-1.20.1-47.2.0"), "47.2.0");
        assert_eq!(normalize_loader_version(LoaderType::Neoforge, "1.21.1", "NeoForge-21.1.77"), "21.1.77");
        assert_eq!(normalize_loader_version(LoaderType::Neoforge, "1.20.1", "1.20.1-47.1.106"), "47.1.106");
        assert_eq!(normalize_loader_version(LoaderType::Fabric, "1.21.1", " 0.16.5 "), "0.16.5");
        assert_eq!(neoforge_game_version("26.1.0.3"), Some("26.1".into()));
        assert_eq!(neoforge_game_version("21.0.5-beta"), Some("1.21".into()));
        assert!(is_legacy_forge("1.12.2") && !is_legacy_forge("1.13"));
        assert!(is_ancient_forge("1.6.4") && !is_ancient_forge("1.7.10") && !is_ancient_forge("1.12.2"));
    }

    #[test]
    fn finds_old_forge_branch_names() {
        let published: Vec<String> =
            ["1.7.10-10.13.4.1614-1.7.10", "1.7.10-10.13.4.1558-1.7.10", "1.12.2-14.23.5.2860", "1.7.2-10.12.2.1161-mc172"].iter().map(|s| s.to_string()).collect();
        assert_eq!(pick_full_forge_version(&published, "1.7.10", "10.13.4.1614"), "10.13.4.1614-1.7.10");
        assert_eq!(pick_full_forge_version(&published, "1.7.10", "10.13.4.1614-1.7.10"), "10.13.4.1614-1.7.10");
        assert_eq!(pick_full_forge_version(&published, "1.12.2", "14.23.5.2860"), "14.23.5.2860");
        assert_eq!(pick_full_forge_version(&published, "1.7.2", "10.12.2.1161"), "10.12.2.1161-mc172");
        assert_eq!(pick_full_forge_version(&published, "1.7.10", "10.13.0.1"), "10.13.0.1");
    }
}
