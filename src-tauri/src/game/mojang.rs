//! Mojang's version manifest, version JSONs, OS rules, libraries, client jar, logging config and assets.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::core::fsutil::{read_json, safe_join, write_json};
use crate::core::net::{download_file, fetch_json, hash_file, DownloadItem, Hash};
use crate::core::paths;
use crate::core::types::McVersion;
use crate::err_tr;

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const RESOURCES_URL: &str = "https://resources.download.minecraft.net";
const LIBRARIES_URL: &str = "https://libraries.minecraft.net/";

// ---------- Version JSON shapes (the subset we use; anything else is kept in `extra`) ----------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OsRule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Rule {
    /// "allow" | "disallow"
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os: Option<OsRule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<BTreeMap<String, bool>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArgValue {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Ruled {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rules: Option<Vec<Rule>>,
        value: ArgValue,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Arguments {
    pub game: Vec<Argument>,
    pub jvm: Vec<Argument>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FileRef {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LibDownloads {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<FileRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classifiers: Option<BTreeMap<String, FileRef>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ExtractRule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Library {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<Rule>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub natives: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extract: Option<ExtractRule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub downloads: Option<LibDownloads>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetIndexRef {
    pub id: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct JavaVersion {
    pub major_version: u32,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LogFile {
    pub id: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingClient {
    pub argument: String,
    pub file: LogFile,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Logging {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<LoggingClient>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inherits_from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minecraft_arguments: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Arguments>,
    pub libraries: Vec<Library>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_index: Option<AssetIndexRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub downloads: Option<BTreeMap<String, FileRef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub java_version: Option<JavaVersion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<Logging>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl VersionJson {
    /// The Java major version this game needs (8 when the JSON doesn't say).
    pub fn java_major(&self) -> u32 {
        self.java_version.as_ref().map(|j| j.major_version).filter(|m| *m > 0).unwrap_or(8)
    }
}

// ---------- Platform rules ----------

pub fn os_name() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

/// Node's `os.arch()` names: "x64", "ia32", "arm64"…
pub fn node_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "x86" => "ia32",
        "aarch64" => "arm64",
        "arm" => "arm",
        other => other,
    }
}

/// Node's `os.release()`: "10.0.<build>" on Windows, the kernel version elsewhere.
fn os_release() -> &'static str {
    static REL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    REL.get_or_init(|| {
        let k = sysinfo::System::kernel_version().unwrap_or_default();
        if cfg!(windows) && !k.contains('.') {
            format!("10.0.{k}")
        } else {
            k
        }
    })
}

pub fn rules_allow(rules: Option<&[Rule]>, features: &BTreeMap<String, bool>) -> bool {
    let Some(rules) = rules.filter(|r| !r.is_empty()) else { return true };
    let mut allowed = false;
    for rule in rules {
        let mut matches = true;
        if let Some(os) = &rule.os {
            if let Some(name) = os.name.as_deref().filter(|s| !s.is_empty()) {
                if name != os_name() {
                    matches = false;
                }
            }
            if let Some(arch) = os.arch.as_deref().filter(|s| !s.is_empty()) {
                if !(arch == "x86" && node_arch() == "ia32") {
                    matches = false;
                }
            }
            if let Some(ver) = os.version.as_deref().filter(|s| !s.is_empty()) {
                if !regex::Regex::new(ver).map(|r| r.is_match(os_release())).unwrap_or(false) {
                    matches = false;
                }
            }
        }
        if let Some(f) = &rule.features {
            for (k, v) in f {
                if features.get(k).copied().unwrap_or(false) != *v {
                    matches = false;
                }
            }
        }
        if matches {
            allowed = rule.action == "allow";
        }
    }
    allowed
}

// ---------- Manifest & version JSON ----------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestEntry {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    release_time: String,
    url: String,
    sha1: String,
}

static MANIFEST: Mutex<Option<(Instant, Vec<ManifestEntry>)>> = Mutex::new(None);

async fn manifest() -> Result<Vec<ManifestEntry>> {
    if let Some((at, v)) = MANIFEST.lock().unwrap().as_ref() {
        if at.elapsed() < Duration::from_secs(600) {
            return Ok(v.clone());
        }
    }
    let file = paths::cache().join("version_manifest_v2.json");
    let parse = |v: &Value| -> Vec<ManifestEntry> { v.get("versions").cloned().and_then(|x| serde_json::from_value(x).ok()).unwrap_or_default() };
    let versions = match fetch_json::<Value>(MANIFEST_URL).await {
        Ok(data) => {
            write_json(&file, &data).await?;
            parse(&data)
        }
        Err(e) => {
            // Offline: fall back to the last manifest we saw.
            let data: Value = read_json(&file).await.unwrap_or(Value::Null);
            let v = parse(&data);
            if v.is_empty() {
                return Err(e);
            }
            v
        }
    };
    *MANIFEST.lock().unwrap() = Some((Instant::now(), versions.clone()));
    Ok(versions)
}

pub async fn list_versions() -> Result<Vec<McVersion>> {
    Ok(manifest().await?.into_iter().map(|v| McVersion { id: v.id, kind: v.kind, release_time: v.release_time }).collect())
}

/// A path as handed to Java and shown to people: without the `\\?\` prefix Windows' canonicalize adds
/// (the dev data folder is canonicalized), which Java can't use on a classpath.
pub fn plain(p: &Path) -> String {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\UNC\") {
        Some(rest) => format!(r"\\{rest}"),
        None => s.strip_prefix(r"\\?\").unwrap_or(&s).to_string(),
    }
}

pub(crate) async fn exists(p: &Path) -> bool {
    tokio::fs::metadata(p).await.is_ok()
}

/// Downloads `item` unless a valid copy is already there; a failed download is fine if an old copy exists.
async fn refresh(item: &DownloadItem) -> Result<()> {
    if let (Some(h), true) = (&item.hash, exists(&item.dest).await) {
        if hash_file(&item.dest, h.algo).await.map(|v| v.eq_ignore_ascii_case(&h.value)).unwrap_or(false) {
            return Ok(());
        }
    }
    match download_file(item).await {
        Ok(()) => Ok(()),
        Err(_) if exists(&item.dest).await => Ok(()),
        Err(e) => Err(e),
    }
}

/// Downloads (or re-checks) a version's JSON against the manifest.
async fn fetch_vanilla_version(id: &str, file: &Path) -> Result<()> {
    let entry = manifest().await?.into_iter().find(|v| v.id == id).ok_or_else(|| err_tr!("mojang.unknownVersion", "Minecraft {id} isn't a known version", id = id))?;
    refresh(&DownloadItem::new(entry.url, file).hash(Some(Hash::sha1(entry.sha1)))).await
}

pub async fn get_vanilla_version(id: &str) -> Result<VersionJson> {
    let file = paths::versions().join(id).join(format!("{id}.json"));
    // Saved copy first, so launching doesn't wait on the network (~0.25 s); Mojang rarely re-publishes a
    // version's JSON, and the background check picks that up for the next start.
    if let Ok(text) = tokio::fs::read_to_string(&file).await {
        if let Ok(v) = serde_json::from_str::<VersionJson>(&text) {
            let id = id.to_string();
            tokio::spawn(async move {
                let _ = fetch_vanilla_version(&id, &file).await;
            });
            return Ok(v);
        }
    }
    let fetched = fetch_vanilla_version(id, &file).await;
    if let Err(e) = fetched {
        if !exists(&file).await {
            return Err(e);
        }
    }
    let text = tokio::fs::read_to_string(&file).await?;
    serde_json::from_str(&text).map_err(|_| err_tr!("mojang.damaged", "{file} is damaged", file = file.display().to_string()))
}

/// Layers a mod-loader profile (Fabric/Quilt/Forge) on top of its vanilla parent.
pub fn merge_versions(parent: &VersionJson, child: &VersionJson) -> VersionJson {
    let legacy = parent.minecraft_arguments.as_deref().is_some_and(|s| !s.is_empty());
    let mut game = Vec::new();
    let mut jvm = Vec::new();
    if !legacy {
        if let Some(a) = &parent.arguments {
            game.extend(a.game.iter().cloned());
            jvm.extend(a.jvm.iter().cloned());
        }
    }
    if let Some(a) = &child.arguments {
        game.extend(a.game.iter().cloned());
        jvm.extend(a.jvm.iter().cloned());
    }
    let mut out = parent.clone();
    out.id = child.id.clone();
    out.main_class = child.main_class.clone().or_else(|| parent.main_class.clone());
    // The loader's libraries go first so they win any duplicates.
    out.libraries = child.libraries.iter().chain(parent.libraries.iter()).cloned().collect();
    out.minecraft_arguments = child.minecraft_arguments.clone().or_else(|| parent.minecraft_arguments.clone());
    out.arguments = if game.is_empty() && jvm.is_empty() { None } else { Some(Arguments { game, jvm }) };
    out.inherits_from = None;
    out
}

// ---------- Libraries ----------

#[derive(Debug, Clone)]
pub struct ResolvedLibrary {
    pub item: DownloadItem,
    pub classpath: bool,
    /// Native library archive to unpack into the natives directory.
    pub extract: bool,
    pub exclude: Vec<String>,
}

pub fn maven_path(name: &str) -> String {
    let mut at = name.splitn(2, '@');
    let coords = at.next().unwrap_or_default();
    let ext = at.next().unwrap_or("jar");
    let p: Vec<&str> = coords.split(':').collect();
    let (group, artifact, version) = (p.first().copied().unwrap_or(""), p.get(1).copied().unwrap_or(""), p.get(2).copied().unwrap_or(""));
    let classifier = p.get(3).map(|c| format!("-{c}")).unwrap_or_default();
    format!("{}/{artifact}/{version}/{artifact}-{version}{classifier}.{ext}", group.replace('.', "/"))
}

/// `libraries/<maven path>` for a set of coordinates.
pub fn lib_path(coords: &str) -> PathBuf {
    join_rel(&paths::libraries(), &maven_path(coords))
}

/// Joins a `/`-separated relative path onto `base`.
pub(crate) fn join_rel(base: &Path, rel: &str) -> PathBuf {
    safe_join(base, rel).unwrap_or_else(|_| base.join(rel))
}

fn native_matches_arch(classifier: &str) -> bool {
    let a = node_arch();
    if classifier.ends_with("-arm64") || classifier.ends_with("-aarch_64") {
        return a == "arm64";
    }
    if classifier.ends_with("-x86") {
        return a == "ia32";
    }
    a != "ia32"
}

pub fn resolve_libraries(v: &VersionJson) -> Vec<ResolvedLibrary> {
    resolve_library_list(&v.libraries, true)
}

/// `dedupe = false` keeps several versions of the same artifact (Forge installer tools need that).
pub fn resolve_library_list(libs: &[Library], dedupe: bool) -> Vec<ResolvedLibrary> {
    resolve_library_list_in(libs, dedupe, &paths::libraries())
}

fn resolve_library_list_in(libs: &[Library], dedupe: bool, root: &Path) -> Vec<ResolvedLibrary> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let sha = |s: &Option<String>| s.as_ref().filter(|s| !s.is_empty()).map(|s| Hash::sha1(s.clone()));
    let empty = BTreeMap::new();

    for lib in libs {
        if !rules_allow(lib.rules.as_deref(), &empty) {
            continue;
        }
        let parts: Vec<&str> = lib.name.split('@').next().unwrap_or_default().split(':').collect();
        let classifier = parts.get(3).copied();
        let key = format!("{}:{}:{}", parts.first().unwrap_or(&""), parts.get(1).unwrap_or(&""), classifier.unwrap_or(""));
        // A repeated artifact adds nothing to the classpath, but may still carry natives: Minecraft 1.14–1.18
        // list each LWJGL library twice, once plain and once with its natives.
        let repeated = dedupe && !seen.insert(key);
        if repeated && lib.natives.is_none() {
            continue;
        }

        let is_native = classifier.is_some_and(|c| c.starts_with("natives-"));
        if repeated {
            // Only the natives below.
        } else if let Some(art) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
            let rel = art.path.clone().unwrap_or_else(|| maven_path(&lib.name));
            out.push(ResolvedLibrary {
                item: DownloadItem::new(art.url.clone(), join_rel(root, &rel)).hash(sha(&art.sha1)).size(art.size),
                classpath: true,
                extract: is_native && native_matches_arch(classifier.unwrap_or("")),
                exclude: Vec::new(),
            });
        } else if lib.natives.is_none() {
            // Plain Maven coordinates, as used by Fabric and Quilt profiles.
            let rel = maven_path(&lib.name);
            let mut base = lib.url.clone().unwrap_or_else(|| LIBRARIES_URL.to_string());
            if !base.ends_with('/') {
                base.push('/');
            }
            out.push(ResolvedLibrary {
                item: DownloadItem::new(format!("{base}{rel}"), join_rel(root, &rel)).hash(sha(&lib.sha1)).size(lib.size),
                classpath: true,
                extract: false,
                exclude: Vec::new(),
            });
        }

        // Pre-1.19 style natives live in a "classifiers" map keyed per OS.
        if let Some(natives) = &lib.natives {
            let cls = natives.get(os_name()).map(|c| c.replace("${arch}", if node_arch() == "ia32" { "32" } else { "64" }));
            let file = cls.as_ref().and_then(|c| lib.downloads.as_ref()?.classifiers.as_ref()?.get(c));
            if let (Some(cls), Some(file)) = (cls.as_ref(), file) {
                let rel = file.path.clone().unwrap_or_else(|| maven_path(&format!("{}:{cls}", lib.name)));
                out.push(ResolvedLibrary {
                    item: DownloadItem::new(file.url.clone(), join_rel(root, &rel)).hash(sha(&file.sha1)).size(file.size),
                    classpath: false,
                    extract: true,
                    exclude: lib.extract.as_ref().and_then(|e| e.exclude.clone()).unwrap_or_default(),
                });
            }
        }
    }
    // Classpath jars without a URL are built locally (e.g. by the Forge installer).
    out.into_iter().filter(|l| !l.item.url.is_empty() || l.classpath).collect()
}

pub fn client_jar(v: &VersionJson) -> Result<DownloadItem> {
    let c = v.downloads.as_ref().and_then(|d| d.get("client")).ok_or_else(|| err_tr!("mojang.noClientDownload", "Version {version} has no client download", version = v.id))?;
    let dest = paths::versions().join(&v.id).join(format!("{}.jar", v.id));
    Ok(DownloadItem::new(c.url.clone(), dest).hash(c.sha1.clone().map(Hash::sha1)).size(c.size))
}

pub struct LoggingConfig {
    pub item: DownloadItem,
    pub argument: String,
}

pub fn logging_config(v: &VersionJson) -> Option<LoggingConfig> {
    let l = v.logging.as_ref()?.client.as_ref()?;
    let dest = join_rel(&paths::assets().join("log_configs"), &l.file.id);
    Some(LoggingConfig {
        item: DownloadItem::new(l.file.url.clone(), dest).hash(Some(Hash::sha1(l.file.sha1.clone()))).size(Some(l.file.size)),
        argument: l.argument.clone(),
    })
}

// ---------- Assets ----------

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AssetIndex {
    pub objects: BTreeMap<String, AssetObject>,
    pub r#virtual: bool,
    pub map_to_resources: bool,
}

pub struct Assets {
    pub items: Vec<DownloadItem>,
    pub index: AssetIndex,
    pub id: String,
}

fn object_path(hash: &str) -> PathBuf {
    paths::assets().join("objects").join(hash.get(..2).unwrap_or(hash)).join(hash)
}

pub async fn asset_items(v: &VersionJson) -> Result<Assets> {
    let Some(ai) = &v.asset_index else {
        return Ok(Assets { items: Vec::new(), index: AssetIndex::default(), id: "legacy".into() });
    };
    let file = join_rel(&paths::assets().join("indexes"), &format!("{}.json", ai.id));
    refresh(&DownloadItem::new(ai.url.clone(), &file).hash(Some(Hash::sha1(ai.sha1.clone())))).await?;
    let index: AssetIndex = read_json(&file).await.unwrap_or_default();
    let items = index
        .objects
        .values()
        .map(|o| {
            let mut item = DownloadItem::new(format!("{RESOURCES_URL}/{}/{}", o.hash.get(..2).unwrap_or(&o.hash), o.hash), object_path(&o.hash))
                .hash(Some(Hash::sha1(o.hash.clone())))
                .size(Some(o.size));
            item.trust_size = true;
            item
        })
        .collect();
    Ok(Assets { items, index, id: ai.id.clone() })
}

/// Very old versions read assets from a flat folder instead of the hashed object store.
/// Returns the directory to pass as `game_assets`.
pub async fn materialize_legacy_assets(index: &AssetIndex, id: &str, game_directory: &Path) -> Result<PathBuf> {
    let target = if index.map_to_resources { game_directory.join("resources") } else { paths::assets().join("virtual").join(id) };
    if !index.map_to_resources && !index.r#virtual {
        return Ok(target);
    }
    for (name, obj) in &index.objects {
        let dest = safe_join(&target, name)?;
        if exists(&dest).await {
            continue;
        }
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::copy(object_path(&obj.hash), &dest).await.map_err(|_| err_tr!("mojang.copyAssetFailed", "Couldn't copy asset {name}", name = name))?;
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_libraries_still_bring_their_natives() {
        // Minecraft 1.16.5 lists LWJGL plain first, then again with natives for every OS.
        let libs: Vec<Library> = serde_json::from_str(
            r#"[
              {"name":"org.lwjgl:lwjgl:3.2.2","downloads":{"artifact":{"url":"https://x/lwjgl.jar","path":"org/lwjgl/lwjgl/3.2.2/lwjgl-3.2.2.jar"}}},
              {"name":"org.lwjgl:lwjgl:3.2.2","natives":{"linux":"natives-linux","osx":"natives-macos","windows":"natives-windows"},
               "downloads":{"artifact":{"url":"https://x/lwjgl.jar","path":"org/lwjgl/lwjgl/3.2.2/lwjgl-3.2.2.jar"},
                 "classifiers":{"natives-linux":{"url":"https://x/l.jar","path":"l.jar"},"natives-macos":{"url":"https://x/m.jar","path":"m.jar"},
                                "natives-windows":{"url":"https://x/w.jar","path":"w.jar"}}}}
            ]"#,
        )
        .unwrap();
        let out = resolve_library_list_in(&libs, true, Path::new("libraries"));
        assert_eq!(out.iter().filter(|l| l.classpath).count(), 1, "the jar goes on the classpath once");
        assert_eq!(out.iter().filter(|l| l.extract).count(), 1, "and its natives are still unpacked");
    }
}
