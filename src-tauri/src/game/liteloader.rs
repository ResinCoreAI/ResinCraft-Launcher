//! LiteLoader (Minecraft 1.5.2–1.12.2), from LiteLoader's own version list. It starts through launchwrapper as
//! a tweaker: on its own (loader `liteloader`) or next to Forge (an instance's `liteloader` version).

use std::collections::HashMap;

use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;

use crate::core::net::{download_file, DownloadItem, Hash};
use crate::core::types::LoaderVersion;
use crate::{bail_tr, err_tr, tr};

use super::loaders::{cached_text, ResolveContext};
use super::mojang::{exists, lib_path, maven_path, Argument, FileRef, LibDownloads, Library, VersionJson};

const VERSIONS_URL: &str = "https://dl.liteloader.com/versions/versions.json";
/// Release builds (LiteLoader's "ivy" repository).
const RELEASES: &str = "https://dl.liteloader.com/versions/";
/// Snapshot builds (1.8.9 and later are only published as snapshots).
const SNAPSHOTS: &str = "https://repo.mumfrey.com/content/repositories/snapshots/";
const ARTIFACT: &str = "com.mumfrey:liteloader";
pub(crate) const TWEAKER: &str = "com.mumfrey.liteloader.launch.LiteLoaderTweaker";
const LAUNCHWRAPPER_MAIN: &str = "net.minecraft.launchwrapper.Launch";

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct Lib {
    name: String,
    url: Option<String>,
}

#[derive(Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
struct Build {
    tweak_class: Option<String>,
    libraries: Vec<Lib>,
    stream: String,
    file: String,
    version: String,
    md5: String,
    timestamp: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct McEntry {
    artefacts: HashMap<String, HashMap<String, Build>>,
    snapshots: HashMap<String, Value>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Meta {
    versions: HashMap<String, McEntry>,
}

/// Every LiteLoader build for a Minecraft version, newest first.
async fn builds(mc: &str) -> Result<Vec<Build>> {
    let meta: Meta = serde_json::from_str(&cached_text(VERSIONS_URL, "liteloader-versions.json").await?)?;
    let Some(entry) = meta.versions.get(mc) else { return Ok(Vec::new()) };
    let mut list: Vec<Build> = entry.artefacts.get(ARTIFACT).into_iter().flat_map(|m| m.iter()).filter(|(k, _)| *k != "latest").map(|(_, b)| b.clone()).collect();
    // The snapshot map also holds the snapshot's shared "libraries" list, which isn't a build.
    if let Some(Value::Object(m)) = entry.snapshots.get(ARTIFACT) {
        list.extend(m.iter().filter(|(k, _)| *k != "latest").filter_map(|(_, v)| serde_json::from_value::<Build>(v.clone()).ok()));
    }
    let mut seen = std::collections::HashSet::new();
    list.retain(|b| !b.version.is_empty() && seen.insert(b.version.clone()));
    list.sort_by_key(|b| std::cmp::Reverse(b.timestamp.parse::<u64>().unwrap_or(0)));
    Ok(list)
}

pub async fn versions(mc: &str) -> Result<Vec<LoaderVersion>> {
    Ok(builds(mc).await?.into_iter().map(|b| LoaderVersion { stable: b.stream == "RELEASE", version: b.version }).collect())
}

/// Where a build's jar is: a release by its file name; a snapshot by the newest timestamped `-release` jar in
/// the snapshot repository (the one LiteLoader's list has the checksum of).
async fn jar_url(mc: &str, b: &Build) -> Result<String> {
    if b.stream != "SNAPSHOT" {
        return Ok(format!("{RELEASES}com/mumfrey/liteloader/{mc}/{}", b.file));
    }
    let dir = format!("{SNAPSHOTS}com/mumfrey/liteloader/{}/", b.version);
    let xml = crate::core::net::fetch_text(&format!("{dir}maven-metadata.xml")).await?;
    let tag = |t: &str| regex::Regex::new(&format!("<{t}>([^<]+)</{t}>")).unwrap().captures(&xml).map(|c| c[1].trim().to_string());
    let (Some(ts), Some(n)) = (tag("timestamp"), tag("buildNumber")) else {
        bail_tr!("liteloader.notFound", "LiteLoader {version} couldn't be found", version = b.version.clone());
    };
    let base = b.version.trim_end_matches("-SNAPSHOT");
    Ok(format!("{dir}liteloader-{base}-{ts}-{n}-release.jar"))
}

/// Maven bases for one of LiteLoader's libraries: its own (LiteLoader's repository mirrors Maven Central), then
/// Mojang's and Maven Central.
fn lib_bases(url: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(u) = url.map(str::trim).filter(|u| !u.is_empty() && !u.contains("liteloader.com")) {
        out.push(if u.ends_with('/') { u.replace("http://", "https://") } else { format!("{}/", u.replace("http://", "https://")) });
    }
    for b in ["https://libraries.minecraft.net/", "https://repo1.maven.org/maven2/"] {
        if !out.iter().any(|o| o == b) {
            out.push(b.to_string());
        }
    }
    out
}

/// A library that's on disk (downloaded from the first host that has it).
async fn fetch_lib(lib: &Lib) -> Result<Library> {
    let dest = lib_path(&lib.name);
    let rel = maven_path(&lib.name);
    let local = |url: String| Library {
        name: lib.name.clone(),
        downloads: Some(LibDownloads { artifact: Some(FileRef { url, path: Some(rel.clone()), ..Default::default() }), classifiers: None }),
        ..Default::default()
    };
    let bases = lib_bases(lib.url.as_deref());
    if exists(&dest).await {
        return Ok(local(format!("{}{rel}", bases[0])));
    }
    let mut last = None;
    for base in bases {
        let url = format!("{base}{rel}");
        match download_file(&DownloadItem::new(url.clone(), &dest)).await {
            Ok(()) => return Ok(local(url)),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| anyhow::anyhow!("{} couldn't be downloaded", lib.name)))
}

/// Adds LiteLoader `ver` to a resolved version: its jar and libraries go first on the classpath, the game starts
/// through launchwrapper, and LiteLoader's tweaker is added after any the version already has (Forge's).
pub async fn apply(version: &mut VersionJson, mc: &str, ver: &str, ctx: &ResolveContext) -> Result<()> {
    let all = builds(mc).await?;
    if all.is_empty() {
        bail_tr!("loader.notAvailable", "{loader} isn't available for Minecraft {mc} yet", loader = "LiteLoader", mc = mc);
    }
    let Some(b) = all.into_iter().find(|b| b.version == ver) else {
        bail_tr!("liteloader.notFound", "LiteLoader {version} couldn't be found", version = ver.to_string());
    };
    let coords = format!("{ARTIFACT}:{}", b.version);
    let jar = lib_path(&coords);
    if !exists(&jar).await {
        if let Some(t) = &ctx.task {
            t.update(tr!("liteloader.downloading", "Downloading LiteLoader {version}…", version = b.version.clone()), None);
        }
        let url = jar_url(mc, &b).await?;
        let hash = Some(b.md5.clone()).filter(|m| !m.is_empty()).map(Hash::md5);
        download_file(&DownloadItem::new(url, &jar).hash(hash)).await.map_err(|e| err_tr!("liteloader.downloadFailed", "LiteLoader {version} couldn't be downloaded: {error}", version = b.version.clone(), error = e.to_string()))?;
    }
    let mut libs = vec![Library {
        name: coords.clone(),
        downloads: Some(LibDownloads { artifact: Some(FileRef { url: String::new(), path: Some(maven_path(&coords)), ..Default::default() }), classifiers: None }),
        ..Default::default()
    }];
    for l in &b.libraries {
        libs.push(fetch_lib(l).await?);
    }
    libs.append(&mut version.libraries);
    version.libraries = libs;
    version.main_class = Some(LAUNCHWRAPPER_MAIN.into());
    let tweaker = b.tweak_class.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| TWEAKER.into());
    let game = &mut version.arguments.get_or_insert_with(Default::default).game;
    if !game.iter().any(|a| matches!(a, Argument::Plain(s) if *s == tweaker)) {
        game.extend([Argument::Plain("--tweakClass".into()), Argument::Plain(tweaker)]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_hosts() {
        assert_eq!(lib_bases(Some("http://repo.liteloader.com/"))[0], "https://libraries.minecraft.net/");
        assert_eq!(lib_bases(Some("https://repo.spongepowered.org/maven"))[0], "https://repo.spongepowered.org/maven/");
        assert_eq!(lib_bases(None).len(), 2);
    }

    #[test]
    fn reads_the_version_list() {
        let entry: McEntry = serde_json::from_value(serde_json::json!({
            "artefacts": { "com.mumfrey:liteloader": {
                "latest": { "version": "1.7.10_04", "stream": "RELEASE", "file": "liteloader-1.7.10.jar", "timestamp": "3" },
                "aa": { "version": "1.7.10_04", "stream": "RELEASE", "file": "liteloader-1.7.10_04.jar", "timestamp": "3" },
                "bb": { "version": "1.7.10_03", "stream": "RELEASE", "file": "liteloader-1.7.10_03.jar", "timestamp": "2" }
            }},
            "snapshots": { "libraries": [], "com.mumfrey:liteloader": { "cc": { "version": "1.7.10-SNAPSHOT", "stream": "SNAPSHOT", "timestamp": "4" } } }
        }))
        .unwrap();
        let n = entry.artefacts.get(ARTIFACT).unwrap().len();
        assert_eq!(n, 3);
        assert!(matches!(entry.snapshots.get(ARTIFACT), Some(Value::Object(_))));
    }
}
