//! Installing server software into a server folder: the official vanilla server.jar, Paper / Purpur / Folia /
//! Leaf from their download APIs, Fabric's server launcher, Quilt's installer, the Forge / NeoForge installers
//! (`--installServer`; Forge 1.7.10–1.12.2 is laid out from its installer instead), the Arclight hybrid (one
//! jar; it fetches its libraries on first start), Mohist, Pufferfish, SpongeVanilla, the proxies (Velocity, BungeeCord,
//! Waterfall, Travertine, WaterdogPE), Geyser on its own, Glowstone, Cuberite, Pumpkin, Bedrock Dedicated Server, the
//! other Bedrock servers (PocketMine-MP, Nukkit and its forks) and the other hybrids (Magma, CatServer,
//! Crucible, SpongeForge, Cardboard). Each fills in `ServerMeta::launch` (how to start it) and the loader/build version.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::content::github;
use crate::content::index::natural_cmp;
use crate::core::events::Task;
use crate::core::net::{download_all, download_file, fetch_json, fetch_text, status_of, DownloadItem, Hash};
use crate::core::types::{LoaderType, ServerBuild, ServerKind, ServerLaunch, ServerMeta};
use crate::core::{paths, settings};
use crate::game::java::{find_java, hidden_command, install_java};
use crate::game::forge::legacy_lib_bases;
use crate::game::loaders::{full_forge_version, installer_url, is_ancient_forge, is_legacy_forge, latest_loader, legacy_forge_error, normalize_loader_version};
use crate::game::mojang::{asset_items, client_jar, get_vanilla_version, maven_path, plain, resolve_libraries};
use crate::{err_tr, tr};

const FILL: &str = "https://fill.papermc.io/v3/projects";
const PURPUR: &str = "https://api.purpurmc.org/v2/purpur";
const FABRIC_META: &str = "https://meta.fabricmc.net/v2";
const SPIGOT_VERSIONS: &str = "https://hub.spigotmc.org/versions/";
const BUILDTOOLS: &str = "https://hub.spigotmc.org/jenkins/job/BuildTools/lastSuccessfulBuild/artifact/target/BuildTools.jar";
const QUILT_INSTALLER: &str = "https://maven.quiltmc.org/repository/release/org/quiltmc/quilt-installer";
const LEAF: &str = "https://api.leafmc.one/v2/projects/leaf";
const MOHIST: &str = "https://api.mohistmc.com/project/mohist";
const SPONGE: &str = "https://dl-api.spongepowered.org/v2/groups/org.spongepowered/artifacts/spongevanilla";
const SPONGE_FORGE: &str = "https://dl-api.spongepowered.org/v2/groups/org.spongepowered/artifacts/spongeforge";
const MODRINTH: &str = "https://api.modrinth.com/v2";
/// One repository per Minecraft version, newest first.
const MAGMA: [(&str, &str); 4] = [("1.20.1", "magmamaintained/Magma-1.20.1"), ("1.19.3", "magmamaintained/Magma-1.19.3"), ("1.18.2", "magmamaintained/Magma-1.18.2"), ("1.12.2", "magmamaintained/Magma-1.12.2")];
const CATSERVER: &str = "Luohuayu/CatServer";
const CRUCIBLE: &str = "CrucibleMC/Crucible";
const CRUCIBLE_MC: &str = "1.7.10";
const PUMPKIN: &str = "Pumpkin-MC/Pumpkin";
const POCKETMINE: &str = "pmmp/PocketMine-MP";
const POWERNUKKITX: &str = "PowerNukkitX/PowerNukkitX";
const NUKKIT_PM1: &str = "PetteriM1/NukkitPetteriM1Edition";
const WATERDOGPE: &str = "WaterdogPE/WaterdogPE";
/// Cloudburst publishes Nukkit as a Maven snapshot.
const NUKKIT: &str = "https://repo.opencollab.dev/maven-snapshots/cn/nukkit/nukkit/1.0-SNAPSHOT";
const GEYSER: &str = "https://download.geysermc.org/v2/projects/geyser";
const LATEST: &str = "latest";
const HORIZON: &str = "horizon-rust/horizon";
/// Pufferfish's Jenkins: one job per Minecraft series ("Pufferfish-1.21").
const PUFFERFISH: &str = "https://ci.pufferfish.host";
const BUNGEECORD: &str = "https://ci.md-5.net/job/BungeeCord/lastSuccessfulBuild/artifact/bootstrap/target/BungeeCord.jar";
/// Its last release (2021), for Minecraft 1.12.2.
const GLOWSTONE: &str = "https://github.com/GlowstoneMC/Glowstone/releases/download/2021.8.0/glowstone.jar";
const CUBERITE: &str = "https://download.cuberite.org/linux-x86_64/Cuberite.tar.gz";
/// What Glowstone and Cuberite speak; they have one version each.
const OLD_MC: &str = "1.12.2";
/// The newest Velocity needs it, and the older proxies run on it.
const PROXY_JAVA: u32 = 21;
/// Velocity 4 is compiled for Java 25.
const VELOCITY_4_JAVA: u32 = 25;
const GLOWSTONE_JAVA: u32 = 11;
/// Arclight's own file server: one folder per Minecraft version (some are aliases of the same branch).
const ARCLIGHT: &str = "https://files.hypertention.cn/v1/files/arclight/minecraft";
/// Every Bedrock Dedicated Server version, kept by the Bedrock OSS community (the downloads are Mojang's).
const BEDROCK_VERSIONS: &str = "https://raw.githubusercontent.com/Bedrock-OSS/BDS-Versions/main/versions.json";
/// Mojang's own download links (the newest release), in case that list hasn't caught up yet.
const BEDROCK_LINKS: &str = "https://net-secondary.web.minecraft-services.net/api/v1.0/download/links";
const BEDROCK_CDN: &str = "https://www.minecraft.net/bedrockdedicatedserver";

fn enc(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

// ---------- Java ----------

/// The Java major version a Minecraft version needs (from Mojang's version JSON).
pub async fn required_java(mc: &str) -> Result<u32> {
    Ok(get_vanilla_version(mc).await?.java_major())
}

fn non_empty(s: Option<&str>) -> Option<String> {
    s.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

/// The server's own Java, else the global one from Settings, else a detected install, else Temurin
/// downloaded into the launcher's Java folder (when automatic Java downloads are on).
pub async fn choose_java(meta: &ServerMeta) -> Result<String> {
    let settings = settings::get().await;
    if let Some(custom) = non_empty(meta.java_path.as_deref()).or_else(|| non_empty(Some(&settings.java_path))) {
        return Ok(custom);
    }
    let required = match meta.kind {
        ServerKind::Nukkit | ServerKind::Nukkitpm1 | ServerKind::Powernukkitx => PROXY_JAVA,
        ServerKind::Velocity if meta.mc_version.split('.').next().and_then(|v| v.parse::<u32>().ok()).is_some_and(|major| major >= 4) => VELOCITY_4_JAVA,
        k if k.proxy() => PROXY_JAVA,
        ServerKind::Glowstone => GLOWSTONE_JAVA,
        _ => required_java(&meta.mc_version).await?,
    };
    java_of(required, meta).await
}

/// A detected Java of this major version, else Temurin downloaded into the launcher's Java folder.
async fn java_of(required: u32, meta: &ServerMeta) -> Result<String> {
    if let Some(found) = find_java(required).await? {
        return Ok(found.path);
    }
    if settings::get().await.auto_java {
        let installed = Task::run(format!("Downloading Java {required}"), Some(meta.id.clone()), |t| async move { install_java(required, &t).await }).await?;
        return Ok(installed.path);
    }
    bail!("Minecraft {} needs Java {required}. Install it, or turn on automatic Java downloads in Settings.", meta.mc_version)
}

// ---------- Version lists ----------

/// Minecraft versions a server type can be created for. Empty for vanilla and mod loaders (the UI uses
/// Mojang's list and the loader lists for those).
pub async fn software_versions(kind: ServerKind) -> Result<Vec<String>> {
    let mut list: Vec<String> = match kind {
        ServerKind::Paper | ServerKind::Folia | ServerKind::Velocity | ServerKind::Waterfall | ServerKind::Travertine => {
            #[derive(Deserialize)]
            struct Project {
                versions: HashMap<String, Vec<String>>,
            }
            let p: Project = fetch_json(&format!("{FILL}/{}", project_of(kind))).await?;
            p.versions.into_values().flatten().collect()
        }
        ServerKind::Purpur => {
            #[derive(Deserialize)]
            struct Project {
                versions: Vec<String>,
            }
            fetch_json::<Project>(PURPUR).await?.versions
        }
        ServerKind::Spigot | ServerKind::Craftbukkit => {
            // BuildTools' version list is a plain directory listing of <version>.json files.
            let html = fetch_text(SPIGOT_VERSIONS).await?;
            let re = regex::Regex::new(r#"href="(\d+\.\d+(?:\.\d+)?)\.json""#)?;
            re.captures_iter(&html).map(|c| c[1].to_string()).collect()
        }
        ServerKind::Leaf => {
            #[derive(Deserialize)]
            struct Project {
                versions: Vec<String>,
            }
            fetch_json::<Project>(LEAF).await?.versions
        }
        ServerKind::Arclight => arclight_versions().await?,
        ServerKind::Mohist => mohist_versions().await?,
        ServerKind::Pufferfish => pufferfish_versions().await?,
        ServerKind::Sponge => sponge_versions().await?,
        ServerKind::Magma => MAGMA.iter().map(|(mc, _)| mc.to_string()).collect(),
        ServerKind::Catserver => catserver_versions().await?,
        ServerKind::Crucible => vec![CRUCIBLE_MC.to_string()],
        ServerKind::Spongeforge => spongeforge_versions().await?,
        ServerKind::Cardboard => cardboard_versions().await?,
        ServerKind::Bungeecord | ServerKind::Nukkit | ServerKind::Waterdogpe => vec![LATEST.to_string()],
        ServerKind::Geyser => geyser_versions().await?,
        ServerKind::Glowstone | ServerKind::Cuberite => vec![OLD_MC.to_string()],
        ServerKind::Pumpkin => pumpkin_versions().await?,
        ServerKind::Horizon => horizon_versions().await?,
        ServerKind::Bedrock => bedrock_versions().await?,
        ServerKind::Pocketmine => release_tags(POCKETMINE, "PocketMine-MP.phar").await?,
        ServerKind::Powernukkitx => release_tags(POWERNUKKITX, "powernukkitx.jar").await?,
        ServerKind::Nukkitpm1 => release_tags(NUKKIT_PM1, "Nukkit-PM1E.jar").await?,
        _ => return Ok(Vec::new()),
    };
    // Releases only; pre-releases and release candidates are experimental builds.
    list.retain(|v| !v.contains('-'));
    list.sort_by(|a, b| natural_cmp(b, a));
    list.dedup();
    Ok(list)
}

/// Server software logos: Modrinth's loader icons (SVG), by loader name. Fetched once per run; empty when offline
/// (the UI has its own icons then).
pub async fn software_icons() -> HashMap<String, String> {
    static ICONS: tokio::sync::OnceCell<HashMap<String, String>> = tokio::sync::OnceCell::const_new();
    #[derive(Deserialize)]
    struct Tag {
        name: String,
        #[serde(default)]
        icon: String,
    }
    let wanted = ["vanilla", "paper", "purpur", "folia", "spigot", "fabric", "quilt", "forge", "neoforge", "geyser"];
    let got = ICONS
        .get_or_try_init(|| async {
            let tags: Vec<Tag> = fetch_json("https://api.modrinth.com/v2/tag/loader").await?;
            Ok::<_, anyhow::Error>(tags.into_iter().filter(|t| wanted.contains(&t.name.as_str()) && t.icon.trim_start().starts_with("<svg")).map(|t| (t.name, t.icon)).collect())
        })
        .await;
    got.cloned().unwrap_or_default()
}

/// The builds a server type offers for a Minecraft version, when there's a choice to make: Arclight comes for
/// Forge, NeoForge or Fabric. Empty for the other types (they take the newest build).
pub async fn builds(kind: ServerKind, mc: &str) -> Result<Vec<ServerBuild>> {
    match kind {
        ServerKind::Arclight => arclight_builds(mc).await,
        _ => Ok(Vec::new()),
    }
}

fn project_of(kind: ServerKind) -> &'static str {
    match kind {
        ServerKind::Folia => "folia",
        ServerKind::Velocity => "velocity",
        ServerKind::Waterfall => "waterfall",
        ServerKind::Travertine => "travertine",
        _ => "paper",
    }
}

// ---------- Install ----------

/// Installs the server software for `meta` into `dir` and records how to start it.
pub async fn install(meta: &mut ServerMeta, dir: &Path, task: &Arc<Task>) -> Result<()> {
    tokio::fs::create_dir_all(dir).await?;
    let mc = meta.mc_version.clone();
    let wanted = meta.loader_version.clone().filter(|v| !v.is_empty());
    let mut loader = meta.kind.loader();
    let (launch, version) = match meta.kind {
        ServerKind::Vanilla => {
            vanilla_jar(&mc, &dir.join("server.jar"), task).await?;
            (jar("server.jar"), None)
        }
        ServerKind::Leaf => {
            let build = leaf(&mc, wanted.as_deref(), dir, task).await?;
            (jar("leaf.jar"), Some(build))
        }
        ServerKind::Arclight => {
            let (on, build) = arclight(&mc, wanted.as_deref(), dir, task).await?;
            loader = Some(on);
            (jar("arclight.jar"), Some(build))
        }
        ServerKind::Bedrock => {
            let exe = bedrock(&mc, dir, task).await?;
            (ServerLaunch { exe: Some(exe), ..Default::default() }, None)
        }
        ServerKind::Mohist => {
            let (on, build) = mohist(&mc, dir, task).await?;
            loader = Some(on);
            (jar("mohist.jar"), Some(build))
        }
        ServerKind::Pufferfish => {
            let build = pufferfish(&mc, dir, task).await?;
            (jar("pufferfish.jar"), Some(build))
        }
        ServerKind::Sponge => {
            let build = sponge(&mc, dir, task).await?;
            (jar("sponge.jar"), Some(build))
        }
        ServerKind::Magma => {
            let build = magma(&mc, dir, task).await?;
            loader = Some(LoaderType::Forge);
            (jar("magma.jar"), Some(build))
        }
        ServerKind::Catserver => {
            let build = catserver(&mc, dir, task).await?;
            loader = Some(LoaderType::Forge);
            (jar("catserver.jar"), Some(build))
        }
        ServerKind::Crucible => {
            let build = crucible(dir, task).await?;
            loader = Some(LoaderType::Forge);
            (jar("crucible.jar"), Some(build))
        }
        ServerKind::Spongeforge => {
            let java = choose_java(meta).await?;
            let (launch, build) = spongeforge(&mc, dir, &java, task).await?;
            loader = Some(LoaderType::Forge);
            (launch, Some(build))
        }
        ServerKind::Cardboard => {
            let build = cardboard(&mc, dir, task).await?;
            loader = Some(LoaderType::Fabric);
            (jar("fabric-server-launch.jar"), Some(build))
        }
        ServerKind::Waterdogpe => {
            let build = github_jar(WATERDOGPE, None, "Waterdog.jar", "waterdogpe.jar", dir, task).await?;
            (jar("waterdogpe.jar"), Some(build))
        }
        ServerKind::Geyser => {
            let build = geyser(&mc, dir, task).await?;
            (jar("geyser.jar"), Some(build))
        }
        ServerKind::Pumpkin => {
            let (exe, build) = pumpkin(&mc, dir, task).await?;
            (ServerLaunch { exe: Some(exe), ..Default::default() }, Some(build))
        }
        ServerKind::Horizon => {
            let (exe, exe_args) = horizon(meta, dir, task).await?;
            (ServerLaunch { exe: Some(exe), exe_args, ..Default::default() }, None)
        }
        ServerKind::Pocketmine => {
            let exe = pocketmine(&mc, dir, task).await?;
            (ServerLaunch { exe: Some(exe), exe_args: vec!["PocketMine-MP.phar".to_string(), "--no-wizard".to_string()], ..Default::default() }, None)
        }
        ServerKind::Nukkit => {
            let build = nukkit(dir, task).await?;
            skip_language_question(dir).await?;
            (jar("nukkit.jar"), Some(build))
        }
        ServerKind::Powernukkitx => {
            github_jar(POWERNUKKITX, Some(&mc), "powernukkitx.jar", "powernukkitx.jar", dir, task).await?;
            skip_language_question(dir).await?;
            (jar("powernukkitx.jar"), None)
        }
        ServerKind::Nukkitpm1 => {
            github_jar(NUKKIT_PM1, Some(&mc), "Nukkit-PM1E.jar", "nukkit.jar", dir, task).await?;
            skip_language_question(dir).await?;
            (jar("nukkit.jar"), None)
        }
        ServerKind::Bungeecord => {
            download_all(vec![DownloadItem::new(BUNGEECORD, dir.join("bungeecord.jar"))], 1, task.counter("Downloading BungeeCord")).await?;
            (jar("bungeecord.jar"), None)
        }
        ServerKind::Glowstone => {
            download_all(vec![DownloadItem::new(GLOWSTONE, dir.join("glowstone.jar"))], 1, task.counter("Downloading Glowstone")).await?;
            (jar("glowstone.jar"), None)
        }
        ServerKind::Cuberite => {
            cuberite(dir, task).await?;
            (ServerLaunch { exe: Some("Cuberite".to_string()), ..Default::default() }, None)
        }
        ServerKind::Paper | ServerKind::Folia | ServerKind::Velocity | ServerKind::Waterfall | ServerKind::Travertine => {
            let build = paper_like(meta.kind, &mc, wanted.as_deref(), dir, task).await?;
            (jar(&format!("{}.jar", project_of(meta.kind))), Some(build))
        }
        ServerKind::Purpur => {
            let build = purpur(&mc, wanted.as_deref(), dir, task).await?;
            (jar("purpur.jar"), Some(build))
        }
        ServerKind::Spigot => {
            let java = choose_java(meta).await?;
            spigot(&mc, dir, &java, "spigot", task).await?;
            (jar("spigot.jar"), None)
        }
        ServerKind::Craftbukkit => {
            let java = choose_java(meta).await?;
            spigot(&mc, dir, &java, "craftbukkit", task).await?;
            (jar("craftbukkit.jar"), None)
        }
        ServerKind::Fabric => {
            let loader = match wanted {
                Some(v) => v,
                None => latest_loader(LoaderType::Fabric, &mc).await?,
            };
            fabric(&mc, &loader, dir, task).await?;
            (jar("fabric-server-launch.jar"), Some(loader))
        }
        ServerKind::Quilt => {
            let loader = match wanted {
                Some(v) => v,
                None => latest_loader(LoaderType::Quilt, &mc).await?,
            };
            let java = choose_java(meta).await?;
            quilt(&mc, &loader, dir, &java, task).await?;
            (jar("quilt-server-launch.jar"), Some(loader))
        }
        ServerKind::Forge | ServerKind::Neoforge => {
            let loader = meta.kind.loader().unwrap_or(LoaderType::Forge);
            let ver = match wanted {
                Some(v) => normalize_loader_version(loader, &mc, &v),
                None => latest_loader(loader, &mc).await?,
            };
            let java = choose_java(meta).await?;
            let launch = forge_like(loader, &mc, &ver, dir, &java, task).await?;
            (launch, Some(ver))
        }
    };
    meta.launch = launch;
    meta.loader = loader;
    meta.loader_version = version;
    Ok(())
}

fn jar(name: &str) -> ServerLaunch {
    ServerLaunch { jar: Some(name.to_string()), ..Default::default() }
}

/// The official server.jar from the version JSON's `downloads.server`.
async fn vanilla_jar(mc: &str, dest: &Path, task: &Arc<Task>) -> Result<()> {
    task.update(format!("Downloading the Minecraft {mc} server…"), None);
    let v = get_vanilla_version(mc).await?;
    let f = v.downloads.as_ref().and_then(|d| d.get("server")).ok_or_else(|| anyhow!("Minecraft {mc} has no official server download"))?;
    let item = DownloadItem::new(&f.url, dest).hash(f.sha1.clone().map(Hash::sha1)).size(f.size);
    download_all(vec![item], 1, task.counter("Downloading the server")).await
}

#[derive(Deserialize)]
struct FillChecksums {
    sha256: String,
}
#[derive(Deserialize)]
struct FillDownload {
    url: String,
    checksums: FillChecksums,
    #[serde(default)]
    size: Option<u64>,
}
#[derive(Deserialize)]
struct FillBuild {
    id: u64,
    #[serde(default)]
    channel: String,
    #[serde(default)]
    downloads: HashMap<String, FillDownload>,
}

/// Paper or Folia from PaperMC's download API. Returns the build number.
async fn paper_like(kind: ServerKind, mc: &str, build: Option<&str>, dir: &Path, task: &Arc<Task>) -> Result<String> {
    let project = project_of(kind);
    task.update(format!("Finding {} for Minecraft {mc}…", kind.label()), None);
    let builds: Vec<FillBuild> = match fetch_json(&format!("{FILL}/{project}/versions/{}/builds", enc(mc))).await {
        Err(e) if status_of(&e) == Some(404) => bail!("{} isn't available for Minecraft {mc}", kind.label()),
        r => r?,
    };
    let pick = match build {
        Some(b) => builds.iter().find(|x| x.id.to_string() == b),
        None => builds.iter().find(|x| x.channel.eq_ignore_ascii_case("stable")).or(builds.first()),
    };
    let pick = pick.ok_or_else(|| anyhow!("{} has no build for Minecraft {mc}", kind.label()))?;
    // Projects that stopped before Fill's "server:default" kept the old name.
    let dl = pick.downloads.get("server:default").or(pick.downloads.get("application")).ok_or_else(|| anyhow!("{} build {} has no server download", kind.label(), pick.id))?;
    let item = DownloadItem::new(&dl.url, dir.join(format!("{project}.jar"))).hash(Some(Hash::sha256(&dl.checksums.sha256))).size(dl.size);
    download_all(vec![item], 1, task.counter(&format!("Downloading {}", kind.label()))).await?;
    Ok(pick.id.to_string())
}

/// Purpur from its download API. Returns the build number.
async fn purpur(mc: &str, build: Option<&str>, dir: &Path, task: &Arc<Task>) -> Result<String> {
    #[derive(Deserialize)]
    struct Build {
        build: String,
        #[serde(default)]
        md5: Option<String>,
    }
    task.update(format!("Finding Purpur for Minecraft {mc}…"), None);
    let b: Build = match fetch_json(&format!("{PURPUR}/{}/{}", enc(mc), enc(build.unwrap_or("latest")))).await {
        Err(e) if matches!(status_of(&e), Some(400 | 404)) => bail!("Purpur isn't available for Minecraft {mc}"),
        r => r?,
    };
    let url = format!("{PURPUR}/{}/{}/download", enc(mc), enc(&b.build));
    let item = DownloadItem::new(url, dir.join("purpur.jar")).hash(b.md5.map(Hash::md5));
    download_all(vec![item], 1, task.counter("Downloading Purpur")).await?;
    Ok(b.build)
}

/// Leaf from its download API (the same shape as PaperMC's old one). Returns the build number.
async fn leaf(mc: &str, build: Option<&str>, dir: &Path, task: &Arc<Task>) -> Result<String> {
    #[derive(Deserialize)]
    struct Download {
        name: String,
        sha256: String,
    }
    #[derive(Deserialize)]
    struct Build {
        build: u64,
        #[serde(default)]
        channel: String,
        downloads: HashMap<String, Download>,
    }
    #[derive(Deserialize)]
    struct Builds {
        builds: Vec<Build>,
    }
    task.update(format!("Finding Leaf for Minecraft {mc}…"), None);
    let list = match fetch_json::<Builds>(&format!("{LEAF}/versions/{}/builds", enc(mc))).await {
        Err(e) if matches!(status_of(&e), Some(400 | 404)) => bail!("Leaf isn't available for Minecraft {mc}"),
        r => r?.builds,
    };
    // Oldest first; "default" is the stable channel.
    let pick = match build {
        Some(b) => list.iter().find(|x| x.build.to_string() == b),
        None => list.iter().rev().find(|x| x.channel == "default").or(list.last()),
    };
    let pick = pick.ok_or_else(|| anyhow!("Leaf has no build for Minecraft {mc}"))?;
    let dl = pick.downloads.get("primary").ok_or_else(|| anyhow!("Leaf build {} has no server download", pick.build))?;
    let url = format!("{LEAF}/versions/{}/builds/{}/downloads/{}", enc(mc), pick.build, enc(&dl.name));
    let item = DownloadItem::new(url, dir.join("leaf.jar")).hash(Some(Hash::sha256(&dl.sha256)));
    download_all(vec![item], 1, task.counter("Downloading Leaf")).await?;
    Ok(pick.build.to_string())
}

/// One entry of an Arclight file-server listing.
#[derive(Deserialize, Clone)]
struct ArcFile {
    name: String,
    key: String,
    #[serde(default)]
    permlink: Option<String>,
}
#[derive(Deserialize)]
struct ArcListing {
    #[serde(default)]
    files: Vec<ArcFile>,
    #[serde(default)]
    key: String,
}

/// Minecraft versions with an Arclight build. Some names are aliases of the same branch ("1.21" is 1.21.1):
/// only the newest name of each branch is kept.
async fn arclight_versions() -> Result<Vec<String>> {
    let names: Vec<String> = fetch_json::<ArcListing>(ARCLIGHT).await?.files.into_iter().map(|f| f.name).collect();
    let checks = names.into_iter().map(|v| async move {
        // A listing's own key names the branch behind the alias.
        let branch = fetch_json::<ArcListing>(&format!("{ARCLIGHT}/{}/loaders", enc(&v))).await.ok().map(|l| l.key);
        (v, branch)
    });
    let mut newest: HashMap<String, String> = HashMap::new();
    for (v, branch) in futures::future::join_all(checks).await {
        let branch = branch.filter(|b| !b.is_empty()).unwrap_or_else(|| v.clone());
        match newest.get(&branch) {
            Some(have) if natural_cmp(have, &v).is_ge() => {}
            _ => {
                newest.insert(branch, v);
            }
        }
    }
    Ok(newest.into_values().collect())
}

fn arc_loader(name: &str) -> Option<LoaderType> {
    match name {
        "forge" => Some(LoaderType::Forge),
        "neoforge" => Some(LoaderType::Neoforge),
        "fabric" => Some(LoaderType::Fabric),
        _ => None,
    }
}

/// The build version is the folder above the loader's file: `.../versions-stable/1.0.1-8ec9529/neoforge`.
fn arc_version(f: &ArcFile) -> String {
    f.key.rsplit('/').nth(1).unwrap_or_default().to_string()
}

/// The newest stable Arclight for each loader, then the newer development builds. Ids are "loader@version".
async fn arclight_builds(mc: &str) -> Result<Vec<ServerBuild>> {
    let mut out: Vec<ServerBuild> = Vec::new();
    for (folder, stable) in [("latest-stable", true), ("latest-snapshot", false)] {
        let Ok(list) = fetch_json::<ArcListing>(&format!("{ARCLIGHT}/{}/{folder}", enc(mc))).await else { continue };
        let mut files = list.files;
        // NeoForge first, then Forge, then Fabric.
        files.sort_by_key(|f| ["neoforge", "forge", "fabric"].iter().position(|n| *n == f.name).unwrap_or(9));
        for f in files {
            let Some(loader) = arc_loader(&f.name) else { continue };
            let id = format!("{}@{}", f.name, arc_version(&f));
            if out.iter().any(|b| b.id == id) {
                continue;
            }
            out.push(ServerBuild { label: format!("{} · {}", loader.label(), arc_version(&f)), id, stable });
        }
    }
    Ok(out)
}

/// Arclight for `mc`: the build chosen in the UI ("loader@version"), else the newest stable one.
/// Returns the mod loader it runs on and the build version.
async fn arclight(mc: &str, build: Option<&str>, dir: &Path, task: &Arc<Task>) -> Result<(LoaderType, String)> {
    task.update(format!("Finding Arclight for Minecraft {mc}…"), None);
    let (name, version) = match build.and_then(|b| b.split_once('@')) {
        Some((l, v)) => (l.to_string(), v.to_string()),
        None => {
            let first = arclight_builds(mc).await?.into_iter().next().ok_or_else(|| anyhow!("Arclight isn't available for Minecraft {mc}"))?;
            let (l, v) = first.id.split_once('@').unwrap_or_default();
            (l.to_string(), v.to_string())
        }
    };
    let loader = arc_loader(&name).ok_or_else(|| anyhow!("Arclight doesn't come for {name}"))?;
    let folders = ["latest-stable".to_string(), "latest-snapshot".to_string(), format!("versions-stable/{version}"), format!("versions-snapshot/{version}")];
    let mut found = None;
    for folder in folders {
        let Ok(list) = fetch_json::<ArcListing>(&format!("{ARCLIGHT}/{}/{folder}", enc(mc))).await else { continue };
        found = list.files.into_iter().find(|f| f.name == name && arc_version(f) == version && f.permlink.is_some());
        if found.is_some() {
            break;
        }
    }
    let url = found.and_then(|f| f.permlink).ok_or_else(|| anyhow!("Arclight {version} for {} {mc} couldn't be found", loader.label()))?;
    // The object's name is the SHA-1 of the file.
    let sha1 = url.rsplit('/').next().filter(|h| h.len() == 40 && h.chars().all(|c| c.is_ascii_hexdigit())).map(Hash::sha1);
    download_all(vec![DownloadItem::new(&url, dir.join("arclight.jar")).hash(sha1)], 1, task.counter("Downloading Arclight")).await?;
    Ok((loader, version))
}

/// This computer's name in Mojang's download links and in the version list.
fn bedrock_platform() -> (&'static str, &'static str, &'static str) {
    if cfg!(windows) {
        ("windows", "bin-win", "serverBedrockWindows")
    } else {
        ("linux", "bin-linux", "serverBedrockLinux")
    }
}

/// Bedrock Dedicated Server versions (releases), from the community list plus Mojang's newest.
async fn bedrock_versions() -> Result<Vec<String>> {
    let (key, _, link) = bedrock_platform();
    let mut list: Vec<String> = match fetch_json::<Value>(BEDROCK_VERSIONS).await {
        Ok(v) => v.get(key).and_then(|p| p.get("versions")).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default(),
        Err(e) => {
            eprintln!("[server] Bedrock version list unavailable: {e:#}");
            Vec::new()
        }
    };
    if let Ok(v) = fetch_json::<Value>(BEDROCK_LINKS).await {
        let newest = v
            .pointer("/result/links")
            .and_then(Value::as_array)
            .and_then(|a| a.iter().find(|l| l.get("downloadType").and_then(Value::as_str) == Some(link)))
            .and_then(|l| l.get("downloadUrl"))
            .and_then(Value::as_str)
            .and_then(|u| u.rsplit("bedrock-server-").next())
            .and_then(|f| f.strip_suffix(".zip"))
            .map(str::to_string);
        if let Some(n) = newest.filter(|n| !list.contains(n)) {
            list.push(n);
        }
    }
    if list.is_empty() {
        bail!("Couldn't load the Bedrock server versions. Check the internet connection");
    }
    Ok(list)
}

/// Bedrock Dedicated Server from minecraft.net, unpacked into the server folder. Mojang publishes no checksums,
/// so the download is checked by unpacking it and finding the server program inside. Returns the program's name.
async fn bedrock(version: &str, dir: &Path, task: &Arc<Task>) -> Result<String> {
    if !cfg!(any(windows, all(target_os = "linux", target_arch = "x86_64"))) {
        bail!("Mojang's Bedrock server only runs on Windows and x64 Linux");
    }
    let (_, folder, _) = bedrock_platform();
    let zip = paths::cache().join("server-installers").join(format!("bedrock-server-{version}.zip"));
    let url = format!("{BEDROCK_CDN}/{folder}/bedrock-server-{}.zip", enc(version));
    task.update(format!("Downloading the Bedrock server {version}…"), None);
    if let Err(e) = download_all(vec![DownloadItem::new(url, &zip)], 1, task.counter("Downloading the Bedrock server")).await {
        if matches!(status_of(&e), Some(403 | 404)) {
            bail!("Bedrock server {version} isn't available from minecraft.net");
        }
        return Err(e);
    }
    task.update("Unpacking the Bedrock server…", None);
    let (zip2, dir2) = (zip.clone(), dir.to_path_buf());
    let unpacked = tokio::task::spawn_blocking(move || -> Result<()> {
        let mut archive = crate::content::index::open_zip(&zip2)?;
        crate::content::index::extract_where(&mut archive, &dir2, |name| Some(name.to_string()))
    })
    .await?;
    let _ = tokio::fs::remove_file(&zip).await;
    unpacked.context("The Bedrock server download is damaged")?;
    let exe = if cfg!(windows) { "bedrock_server.exe" } else { "bedrock_server" };
    if !dir.join(exe).is_file() {
        bail!("The Bedrock server download didn't contain {exe}");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(dir.join(exe), std::fs::Permissions::from_mode(0o755)).await?;
    }
    Ok(exe.to_string())
}

/// The Minecraft versions Mohist has a build for (its list also names versions without any).
async fn mohist_versions() -> Result<Vec<String>> {
    #[derive(Deserialize)]
    struct Version {
        name: String,
    }
    let all: Vec<Version> = fetch_json(&format!("{MOHIST}/versions")).await?;
    let checks = all.into_iter().map(|v| async move { fetch_text(&format!("{MOHIST}/{}/builds/latest", enc(&v.name))).await.ok().map(|_| v.name) });
    Ok(futures::future::join_all(checks).await.into_iter().flatten().collect())
}

/// Mohist's newest build. Returns the loader it runs on and the build number.
async fn mohist(mc: &str, dir: &Path, task: &Arc<Task>) -> Result<(LoaderType, String)> {
    #[derive(Deserialize)]
    struct Loader {
        #[serde(default)]
        forge_version: Option<String>,
    }
    #[derive(Deserialize)]
    struct Build {
        id: u64,
        file_sha256: String,
        loader: Loader,
    }
    task.update(format!("Finding Mohist for Minecraft {mc}…"), None);
    let b: Build = match fetch_json(&format!("{MOHIST}/{}/builds/latest", enc(mc))).await {
        Err(e) if status_of(&e) == Some(404) => bail!("Mohist isn't available for Minecraft {mc}"),
        r => r?,
    };
    let url = format!("{MOHIST}/{}/builds/{}/download", enc(mc), b.id);
    let item = DownloadItem::new(url, dir.join("mohist.jar")).hash(Some(Hash::sha256(&b.file_sha256)));
    download_all(vec![item], 1, task.counter("Downloading Mohist")).await?;
    let on = if b.loader.forge_version.is_some_and(|v| !v.is_empty()) { LoaderType::Forge } else { LoaderType::Neoforge };
    Ok((on, b.id.to_string()))
}

/// The Minecraft series Pufferfish has a job for.
async fn pufferfish_versions() -> Result<Vec<String>> {
    let text = fetch_text(&format!("{PUFFERFISH}/api/json?tree=jobs%5Bname%5D")).await?;
    let re = regex::Regex::new(r#""name":"Pufferfish-(\d+\.\d+)""#)?;
    Ok(re.captures_iter(&text).map(|c| c[1].to_string()).collect())
}

/// The series' newest build (its paperclip jar). Returns the build number.
async fn pufferfish(mc: &str, dir: &Path, task: &Arc<Task>) -> Result<String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Artifact {
        relative_path: String,
    }
    #[derive(Deserialize)]
    struct Build {
        number: u64,
        artifacts: Vec<Artifact>,
    }
    task.update(format!("Finding Pufferfish for Minecraft {mc}…"), None);
    let job = format!("{PUFFERFISH}/job/Pufferfish-{}/lastSuccessfulBuild", enc(mc));
    let b: Build = match fetch_json(&format!("{job}/api/json?tree=number,artifacts%5BrelativePath%5D")).await {
        Err(e) if status_of(&e) == Some(404) => bail!("Pufferfish isn't available for Minecraft {mc}"),
        r => r?,
    };
    let art = b.artifacts.iter().find(|a| a.relative_path.contains("paperclip")).or(b.artifacts.iter().find(|a| a.relative_path.ends_with(".jar")));
    let art = art.ok_or_else(|| anyhow!("Pufferfish build {} has no server jar", b.number))?;
    let url = format!("{job}/artifact/{}", art.relative_path);
    download_all(vec![DownloadItem::new(url, dir.join("pufferfish.jar"))], 1, task.counter("Downloading Pufferfish")).await?;
    Ok(b.number.to_string())
}

/// The Minecraft releases SpongeVanilla has builds for.
async fn sponge_versions() -> Result<Vec<String>> {
    let v: Value = fetch_json(SPONGE).await?;
    let list = v["tags"]["minecraft"].as_array().cloned().unwrap_or_default();
    let re = regex::Regex::new(r"^\d+\.\d+(\.\d+)?$")?;
    Ok(list.iter().filter_map(Value::as_str).filter(|m| re.is_match(m)).map(str::to_string).collect())
}

/// SpongeVanilla's recommended build, else its newest. Returns the build.
async fn sponge(mc: &str, dir: &Path, task: &Arc<Task>) -> Result<String> {
    task.update(format!("Finding Sponge for Minecraft {mc}…"), None);
    let v: Value = fetch_json(&format!("{SPONGE}/versions?tags=minecraft:{}&offset=0&limit=25", enc(mc))).await?;
    let builds = v["artifacts"].as_object().cloned().unwrap_or_default();
    let newest = |list: Vec<&String>| list.into_iter().max_by(|a, b| natural_cmp(a, b)).cloned();
    let pick = newest(builds.iter().filter(|(_, b)| b["recommended"] == true).map(|(k, _)| k).collect()).or_else(|| newest(builds.keys().collect()));
    let pick = pick.ok_or_else(|| anyhow!("Sponge isn't available for Minecraft {mc}"))?;
    let detail: Value = fetch_json(&format!("{SPONGE}/versions/{}", enc(&pick))).await?;
    let assets = detail["assets"].as_array().cloned().unwrap_or_default();
    let asset = assets.iter().find(|a| a["classifier"] == "universal" && a["extension"] == "jar").ok_or_else(|| anyhow!("Sponge {pick} has no server jar"))?;
    let url = asset["downloadUrl"].as_str().ok_or_else(|| anyhow!("Sponge {pick} has no server jar"))?;
    let item = DownloadItem::new(url, dir.join("sponge.jar")).hash(asset["sha1"].as_str().map(Hash::sha1));
    download_all(vec![item], 1, task.counter("Downloading Sponge")).await?;
    Ok(pick)
}

// ---------- Software published as GitHub releases ----------

/// The tags of the full releases that hold `file`: each is a version of that software.
async fn release_tags(repo: &str, file: &str) -> Result<Vec<String>> {
    let list = github::all_releases(repo).await?;
    Ok(list.into_iter().filter(|r| !r.prerelease && r.assets.iter().any(|a| a.name == file)).map(|r| r.tag_name).collect())
}

/// The release `tag`, else the newest full release that holds `file`.
async fn release_with(repo: &str, tag: Option<&str>, file: impl Fn(&str) -> bool) -> Result<github::Release> {
    let list = github::all_releases(repo).await?;
    let pick = list.into_iter().find(|r| tag.map_or(!r.prerelease, |t| r.tag_name == t) && r.assets.iter().any(|a| file(&a.name)));
    pick.ok_or_else(|| anyhow!("{repo} has no download for that version"))
}

fn release_item(asset: &github::Asset, dest: PathBuf) -> DownloadItem {
    DownloadItem::new(&asset.url, dest).hash(asset.sha256().map(Hash::sha256)).size(Some(asset.size))
}

/// The file `name` of a release, saved as `save`. Returns the release's tag.
async fn github_jar(repo: &str, tag: Option<&str>, name: &str, save: &str, dir: &Path, task: &Arc<Task>) -> Result<String> {
    task.update(format!("Finding {name}…"), None);
    let release = release_with(repo, tag, |n| n == name).await?;
    let asset = release.assets.iter().find(|a| a.name == name).ok_or_else(|| anyhow!("{repo} has no {name}"))?;
    download_all(vec![release_item(asset, dir.join(save))], 1, task.counter(format!("Downloading {name}"))).await?;
    Ok(release.tag_name)
}

// ---------- Other hybrids ----------

/// The newest Magma server jar for `mc`. Returns the build (the commit it's tagged with).
async fn magma(mc: &str, dir: &Path, task: &Arc<Task>) -> Result<String> {
    task.update(format!("Finding Magma for Minecraft {mc}…"), None);
    let repo = MAGMA.iter().find(|(v, _)| *v == mc).map(|(_, r)| *r).ok_or_else(|| anyhow!("Magma isn't available for Minecraft {mc}"))?;
    let is_server = |n: &str| n.to_lowercase().ends_with("-server.jar");
    let release = release_with(repo, None, is_server).await?;
    let asset = release.assets.iter().find(|a| is_server(&a.name)).ok_or_else(|| anyhow!("Magma {mc} has no server jar"))?;
    download_all(vec![release_item(asset, dir.join("magma.jar"))], 1, task.counter("Downloading Magma")).await?;
    Ok(release.tag_name)
}

/// The Minecraft version a CatServer jar is for: "CatServer-1.16.5-1d8d6313-server.jar"; the 1.12.2 builds are
/// "CatServer-4168d848-universal.jar".
fn catserver_mc(file: &str) -> Option<String> {
    let rest = file.strip_prefix("CatServer-")?;
    if file.ends_with("-universal.jar") {
        return Some(OLD_MC.to_string());
    }
    let version = rest.strip_suffix("-server.jar")?.split('-').next()?;
    version.starts_with("1.").then(|| version.to_string())
}

async fn catserver_versions() -> Result<Vec<String>> {
    let list = github::all_releases(CATSERVER).await?;
    Ok(list.iter().flat_map(|r| &r.assets).filter_map(|a| catserver_mc(&a.name)).collect())
}

/// The newest CatServer for `mc`. Returns the build (the release's tag).
async fn catserver(mc: &str, dir: &Path, task: &Arc<Task>) -> Result<String> {
    task.update(format!("Finding CatServer for Minecraft {mc}…"), None);
    let for_mc = |n: &str| catserver_mc(n).as_deref() == Some(mc);
    let release = release_with(CATSERVER, None, for_mc).await.map_err(|_| anyhow!("CatServer isn't available for Minecraft {mc}"))?;
    let asset = release.assets.iter().find(|a| for_mc(&a.name)).ok_or_else(|| anyhow!("CatServer isn't available for Minecraft {mc}"))?;
    download_all(vec![release_item(asset, dir.join("catserver.jar"))], 1, task.counter("Downloading CatServer")).await?;
    Ok(release.tag_name)
}

/// Crucible's newest full release: the server jar and the libraries it doesn't carry. Returns the build.
async fn crucible(dir: &Path, task: &Arc<Task>) -> Result<String> {
    task.update("Finding Crucible…", None);
    let is_jar = |n: &str| n.starts_with("Crucible-") && n.ends_with(".jar");
    let release = release_with(CRUCIBLE, None, is_jar).await?;
    let asset = release.assets.iter().find(|a| is_jar(&a.name)).ok_or_else(|| anyhow!("Crucible has no server jar"))?;
    let mut items = vec![release_item(asset, dir.join("crucible.jar"))];
    let libs = release.assets.iter().find(|a| a.name == "libraries.zip");
    let zip = dir.join("libraries.zip");
    if let Some(l) = libs {
        items.push(release_item(l, zip.clone()));
    }
    download_all(items, 2, task.counter("Downloading Crucible")).await?;
    if libs.is_some() {
        unpack_zip(&zip, &dir.join("libraries"), "Crucible's libraries").await?;
        let _ = tokio::fs::remove_file(&zip).await;
    }
    Ok(release.tag_name)
}

/// Unpacks a download into the server folder, as it is laid out in the zip.
pub async fn unpack_zip(zip: &Path, dir: &Path, what: &str) -> Result<()> {
    let (zip, dir) = (zip.to_path_buf(), dir.to_path_buf());
    let unpacked = tokio::task::spawn_blocking(move || -> Result<()> {
        let mut archive = crate::content::index::open_zip(&zip)?;
        crate::content::index::extract_where(&mut archive, &dir, |name| Some(name.to_string()))
    })
    .await?;
    unpacked.with_context(|| format!("The download of {what} is damaged"))
}

pub async fn unpack_tar(archive: &Path, dir: &Path) -> Result<()> {
    let status = tokio::process::Command::new("tar").arg("-xzf").arg(archive).arg("-C").arg(dir).status().await.context("Couldn't run tar")?;
    if !status.success() {
        bail!("{} couldn't be unpacked", archive.display());
    }
    Ok(())
}

/// SpongeForge's list goes further back, to Forge versions the launcher can't set up.
const SPONGEFORGE_OLDEST: &str = "1.12.2";

fn spongeforge_lists(mc: &str) -> bool {
    mc.split('.').all(|p| p.parse::<u32>().is_ok()) && natural_cmp(mc, SPONGEFORGE_OLDEST).is_ge()
}

async fn spongeforge_versions() -> Result<Vec<String>> {
    let v: Value = fetch_json(SPONGE_FORGE).await?;
    let list = v["tags"]["minecraft"].as_array().cloned().unwrap_or_default();
    Ok(list.iter().filter_map(Value::as_str).filter(|m| spongeforge_lists(m)).map(str::to_string).collect())
}

/// The Forge version a SpongeForge build asks for. The 1.12.2 builds give only Forge's build number.
fn sponge_forge_version(mc: &str, forge: &str) -> String {
    if mc == OLD_MC && !forge.contains('.') {
        format!("14.23.5.{forge}")
    } else {
        forge.to_string()
    }
}

/// The Forge server SpongeForge asks for, with SpongeForge in its mods. Returns how to start it and SpongeForge's build.
async fn spongeforge(mc: &str, dir: &Path, java: &str, task: &Arc<Task>) -> Result<(ServerLaunch, String)> {
    task.update(format!("Finding SpongeForge for Minecraft {mc}…"), None);
    let v: Value = fetch_json(&format!("{SPONGE_FORGE}/versions?tags=minecraft:{}&offset=0&limit=25", enc(mc))).await?;
    let builds = v["artifacts"].as_object().cloned().unwrap_or_default();
    let newest = |list: Vec<&String>| list.into_iter().max_by(|a, b| natural_cmp(a, b)).cloned();
    let pick = newest(builds.iter().filter(|(_, b)| b["recommended"] == true).map(|(k, _)| k).collect()).or_else(|| newest(builds.keys().collect()));
    let pick = pick.ok_or_else(|| anyhow!("SpongeForge isn't available for Minecraft {mc}"))?;
    let forge = builds[&pick]["tagValues"]["forge"].as_str().ok_or_else(|| anyhow!("SpongeForge {pick} doesn't say which Forge it needs"))?;
    // SpongeForge 1.16.5 names a Forge that no longer starts on today's Java 8.
    let forge = if mc == "1.16.5" { latest_loader(LoaderType::Forge, mc).await? } else { sponge_forge_version(mc, forge) };
    let detail: Value = fetch_json(&format!("{SPONGE_FORGE}/versions/{}", enc(&pick))).await?;
    let assets = detail["assets"].as_array().cloned().unwrap_or_default();
    // 1.12.2's mod jar has no classifier.
    let asset = assets.iter().find(|a| matches!(a["classifier"].as_str(), Some("universal" | "")) && a["extension"] == "jar").ok_or_else(|| anyhow!("SpongeForge {pick} has no mod jar"))?;
    let url = asset["downloadUrl"].as_str().ok_or_else(|| anyhow!("SpongeForge {pick} has no mod jar"))?;
    let launch = forge_like(LoaderType::Forge, mc, &forge, dir, java, task).await?;
    let item = DownloadItem::new(url, dir.join("mods").join("spongeforge.jar")).hash(asset["sha1"].as_str().map(Hash::sha1));
    download_all(vec![item], 1, task.counter("Downloading SpongeForge")).await?;
    Ok((launch, pick))
}

#[derive(Deserialize)]
struct MrFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    hashes: HashMap<String, String>,
}
#[derive(Deserialize)]
struct MrVersion {
    version_number: String,
    #[serde(default)]
    files: Vec<MrFile>,
}

/// The newest Fabric build of a Modrinth project for `mc`, into mods/. Returns its version.
async fn modrinth_mod(project: &str, mc: &str, dir: &Path, task: &Arc<Task>) -> Result<String> {
    let url = format!("{MODRINTH}/project/{}/version?loaders={}&game_versions={}", enc(project), enc(r#"["fabric"]"#), enc(&format!(r#"["{mc}"]"#)));
    let version = fetch_json::<Vec<MrVersion>>(&url).await?.into_iter().next().ok_or_else(|| anyhow!("{project} isn't available for Minecraft {mc}"))?;
    let file = version.files.iter().find(|f| f.primary).or(version.files.first()).ok_or_else(|| anyhow!("{project} {} has no file", version.version_number))?;
    let item = DownloadItem::new(&file.url, dir.join("mods").join(crate::content::index::safe_name(&file.filename)?)).hash(file.hashes.get("sha1").cloned().map(Hash::sha1));
    download_all(vec![item], 1, task.counter(format!("Downloading {project}"))).await?;
    Ok(version.version_number)
}

async fn modrinth_mc_versions(project: &str) -> Result<Vec<String>> {
    let v: Value = fetch_json(&format!("{MODRINTH}/project/{project}")).await?;
    let list = v["game_versions"].as_array().cloned().unwrap_or_default();
    let re = regex::Regex::new(r"^\d+\.\d+(\.\d+)?$")?;
    Ok(list.iter().filter_map(Value::as_str).filter(|m| re.is_match(m)).map(str::to_string).collect())
}

/// Minecraft versions with both Cardboard and iCommon, which it can't run without.
async fn cardboard_versions() -> Result<Vec<String>> {
    let common = modrinth_mc_versions("icommon").await?;
    Ok(modrinth_mc_versions("cardboard").await?.into_iter().filter(|v| common.contains(v)).collect())
}

/// A Fabric server with Cardboard and the two mods it needs (its Modrinth page doesn't list them). Returns Cardboard's version.
async fn cardboard(mc: &str, dir: &Path, task: &Arc<Task>) -> Result<String> {
    task.update(format!("Finding Cardboard for Minecraft {mc}…"), None);
    let version = modrinth_mod("cardboard", mc, dir, task).await?;
    modrinth_mod("fabric-api", mc, dir, task).await?;
    modrinth_mod("icommon", mc, dir, task).await?;
    let loader = latest_loader(LoaderType::Fabric, mc).await?;
    fabric(mc, &loader, dir, task).await?;
    Ok(version)
}

// ---------- Geyser, Pumpkin, Bedrock servers ----------

async fn geyser_versions() -> Result<Vec<String>> {
    #[derive(Deserialize)]
    struct Project {
        versions: Vec<String>,
    }
    Ok(fetch_json::<Project>(GEYSER).await?.versions)
}

/// The newest build of Geyser's standalone program. Returns the build number.
async fn geyser(version: &str, dir: &Path, task: &Arc<Task>) -> Result<String> {
    #[derive(Deserialize)]
    struct Download {
        sha256: String,
    }
    #[derive(Deserialize)]
    struct Build {
        build: u64,
        downloads: HashMap<String, Download>,
    }
    task.update(format!("Finding Geyser {version}…"), None);
    let builds = format!("{GEYSER}/versions/{}/builds/latest", enc(version));
    let b: Build = fetch_json(&builds).await?;
    let dl = b.downloads.get("standalone").ok_or_else(|| anyhow!("Geyser {version} has no standalone build"))?;
    let item = DownloadItem::new(format!("{builds}/downloads/standalone"), dir.join("geyser.jar")).hash(Some(Hash::sha256(&dl.sha256)));
    download_all(vec![item], 1, task.counter("Downloading Geyser")).await?;
    Ok(b.build.to_string())
}

/// The Minecraft version a Pumpkin release speaks: "0.2.1+26.3-26.51" is Pumpkin 0.2.1 for Java 26.3 and Bedrock
/// 26.51. Canary and nightly builds have none.
fn pumpkin_mc(tag: &str) -> Option<String> {
    let (_, target) = tag.split_once('+')?;
    target.split('-').next().filter(|v| !v.is_empty()).map(str::to_string)
}

fn pumpkin_asset(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("windows", "x86_64") => Some("pumpkin-X64-Windows.exe"),
        ("windows", "aarch64") => Some("pumpkin-ARM64-Windows.exe"),
        ("linux", "x86_64") => Some("pumpkin-X64-Linux"),
        ("linux", "aarch64") => Some("pumpkin-ARM64-Linux"),
        ("macos", "aarch64") => Some("pumpkin-ARM64-macOS"),
        _ => None,
    }
}

async fn pumpkin_versions() -> Result<Vec<String>> {
    let list = github::all_releases(PUMPKIN).await?;
    Ok(list.iter().filter(|r| !r.prerelease).filter_map(|r| pumpkin_mc(&r.tag_name)).collect())
}

/// Pumpkin's newest release for `mc`, the program for this computer. Returns its name and Pumpkin's version.
async fn pumpkin(mc: &str, dir: &Path, task: &Arc<Task>) -> Result<(String, String)> {
    task.update(format!("Finding Pumpkin for Minecraft {mc}…"), None);
    let name = pumpkin_asset(std::env::consts::OS, std::env::consts::ARCH).ok_or_else(|| anyhow!("Pumpkin has no build for this computer"))?;
    let list = github::all_releases(PUMPKIN).await?;
    let release = list.iter().find(|r| !r.prerelease && pumpkin_mc(&r.tag_name).as_deref() == Some(mc)).ok_or_else(|| anyhow!("Pumpkin isn't available for Minecraft {mc}"))?;
    let asset = release.assets.iter().find(|a| a.name == name).ok_or_else(|| anyhow!("Pumpkin {} has no {name}", release.tag_name))?;
    let exe = if cfg!(windows) { "pumpkin.exe" } else { "pumpkin" };
    download_all(vec![release_item(asset, dir.join(exe))], 1, task.counter("Downloading Pumpkin")).await?;
    make_executable(&dir.join(exe)).await?;
    Ok((exe.to_string(), release.tag_name.split('+').next().unwrap_or_default().to_string()))
}

pub async fn make_executable(_file: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(_file, std::fs::Permissions::from_mode(0o755)).await?;
    }
    Ok(())
}

/// Horizon's archive for this kind of computer and the game program in it. Windows and macOS have only the full game
/// (it runs without a window as a server); Linux has a build without its sound and controller libraries.
fn horizon_archive(os: &str, arch: &str) -> Option<(&'static str, &'static str)> {
    match (os, arch) {
        ("windows", "x86_64") => Some(("horizon-windows-x64.zip", "horizon-client.exe")),
        ("linux", "x86_64") => Some(("horizon-server-linux-x86_64.tar.gz", "horizon-engine")),
        ("macos", "aarch64") => Some(("horizon-macos-arm64.zip", "horizon-client")),
        _ => None,
    }
}

fn horizon_here() -> Result<(&'static str, &'static str)> {
    horizon_archive(std::env::consts::OS, std::env::consts::ARCH).ok_or_else(|| anyhow!("Horizon has no build for this computer"))
}

async fn horizon_versions() -> Result<Vec<String>> {
    let (archive, _) = horizon_here()?;
    let tags = release_tags(HORIZON, archive).await?;
    Ok(tags.into_iter().filter(|t| t.starts_with('v')).collect())
}

/// The Minecraft version a Horizon release is pinned to, from its research-lock.json.
fn horizon_mc(lock: &str) -> Option<String> {
    serde_json::from_str::<Value>(lock).ok()?.pointer("/minecraft/version")?.as_str().map(str::to_string)
}

/// A Horizon release, prepared for hosting: horizon-prepare builds its resources from the client and server of the
/// Minecraft version it's pinned to. Those come from Mojang into the launcher's own files, which are laid out like
/// the official launcher's, where horizon-prepare reads them. Returns the program and what it's started with.
async fn horizon(meta: &ServerMeta, dir: &Path, task: &Arc<Task>) -> Result<(String, Vec<String>)> {
    let (archive, program) = horizon_here()?;
    let tag = meta.mc_version.clone();
    github_jar(HORIZON, Some(&tag), archive, archive, dir, task).await?;
    let file = dir.join(archive);
    task.update("Unpacking Horizon…", None);
    if archive.ends_with(".zip") {
        unpack_zip(&file, dir, "Horizon").await?;
    } else {
        unpack_tar(&file, dir).await?;
    }
    let _ = tokio::fs::remove_file(&file).await;
    let mut locks = Vec::new();
    find_files(dir, "research-lock.json", 2, &mut locks);
    let lock = locks.into_iter().next().ok_or_else(|| anyhow!("Horizon {tag} has no research-lock.json"))?;
    let mc = horizon_mc(&tokio::fs::read_to_string(&lock).await?).ok_or_else(|| anyhow!("Couldn't read which Minecraft version Horizon {tag} needs"))?;
    let root = lock.parent().unwrap_or(dir).to_path_buf();

    task.update(format!("Downloading Minecraft {mc}…"), None);
    let vanilla = get_vanilla_version(&mc).await?;
    let mut items = vec![client_jar(&vanilla)?];
    items.extend(resolve_libraries(&vanilla).into_iter().map(|l| l.item));
    download_all(items, 8, task.counter("Downloading Minecraft")).await?;
    download_all(asset_items(&vanilla).await?.items, 16, task.counter("Downloading assets")).await?;
    let server_jar = paths::cache().join("server-installers").join(format!("horizon-server-{mc}.jar"));
    vanilla_jar(&mc, &server_jar, task).await?;

    let java = java_of(vanilla.java_major(), meta).await?;
    let java_home = Path::new(&java).parent().and_then(Path::parent).map(Path::to_path_buf);
    let prepare = root.join(if cfg!(windows) { "horizon-prepare.exe" } else { "horizon-prepare" });
    make_executable(&prepare).await?;
    make_executable(&root.join(program)).await?;
    let args = vec!["--prepare".to_string(), "--server-jar".into(), plain(&server_jar), "--minecraft-dir".into(), plain(&paths::root()), "--root".into(), plain(&root)];
    let java_env = java_home.as_deref().map(|h| ("JAVA_HOME", h));
    run_tool(&plain(&prepare), &args, &root, task, "Horizon", java_env).await?;
    let _ = tokio::fs::remove_file(&server_jar).await;

    let relative = |p: &Path| p.strip_prefix(dir).unwrap_or(p).to_string_lossy().replace('\\', "/");
    let resources = root.join(".local").join("horizon-client").join(&mc);
    if !resources.is_dir() {
        bail!("Preparing Horizon finished without its resources");
    }
    Ok((relative(&root.join(program)), vec!["--resources".to_string(), relative(&resources), "--dedicated".to_string(), "--nogui".to_string()]))
}

/// The PHP build a server's release asks for, on this kind of computer. `series`: "PM5".
fn php_asset(php: &str, series: &str, os: &str, arch: &str) -> Option<String> {
    let platform = match (os, arch) {
        ("windows", "x86_64") => "Windows-x64",
        ("linux", "x86_64") => "Linux-x86_64",
        ("macos", "aarch64") => "MacOS-arm64",
        ("macos", "x86_64") => "MacOS-x86_64",
        _ => return None,
    };
    let extension = if os == "windows" { "zip" } else { "tar.gz" };
    Some(format!("PHP-{php}-{platform}-{series}.{extension}"))
}

fn php_for_computer(php: &str, series: &str) -> Result<String> {
    php_asset(php, series, std::env::consts::OS, std::env::consts::ARCH).ok_or_else(|| anyhow!("PocketMine's PHP has no build for this computer"))
}

/// php.ini pointing at its extensions, through an absolute path that's only known once PHP is unpacked.
fn php_ini_with_extensions(ini: &str, dir: &str) -> String {
    let mut lines: Vec<String> = ini.lines().filter(|l| !l.trim_start().starts_with("extension_dir")).map(str::to_string).collect();
    lines.push(format!("extension_dir=\"{dir}\""));
    lines.join("\n") + "\n"
}

/// PocketMine's PHP has the extensions the server needs. It's unpacked into bin/ of the server folder, where
/// PocketMine's own start scripts look too. Returns the program, relative to the server folder.
async fn php_runtime(url: &str, dir: &Path, task: &Arc<Task>) -> Result<String> {
    let archive = dir.join(url.rsplit('/').next().unwrap_or("php"));
    download_all(vec![DownloadItem::new(url, &archive)], 1, task.counter("Downloading PHP")).await?;
    task.update("Unpacking PHP…", None);
    if cfg!(windows) {
        unpack_zip(&archive, dir, "PHP").await?;
        let _ = tokio::fs::remove_file(&archive).await;
        return Ok("bin/php/php.exe".to_string());
    }
    let unpacked = unpack_tar(&archive, dir).await;
    let _ = tokio::fs::remove_file(&archive).await;
    unpacked?;
    let bin = dir.join("bin");
    let root = bin.clone();
    let extensions = tokio::task::spawn_blocking(move || {
        let mut found = Vec::new();
        find_files(&root, "phar.so", 8, &mut found);
        found.into_iter().next().and_then(|p| p.parent().map(Path::to_path_buf))
    })
    .await?;
    if let Some(ext) = extensions {
        let ini = bin.join("php7/bin/php.ini");
        let text = tokio::fs::read_to_string(&ini).await.unwrap_or_default();
        tokio::fs::write(&ini, php_ini_with_extensions(&text, &plain(&ext))).await?;
    }
    Ok("bin/php7/bin/php".to_string())
}

/// PocketMine-MP `version` and the PHP its release asks for. Returns the program that starts it.
async fn pocketmine(version: &str, dir: &Path, task: &Arc<Task>) -> Result<String> {
    #[derive(Deserialize)]
    struct BuildInfo {
        php_version: String,
    }
    task.update(format!("Finding PocketMine-MP {version}…"), None);
    let base = format!("https://github.com/{POCKETMINE}/releases/download/{}", enc(version));
    let info: BuildInfo = fetch_json(&format!("{base}/build_info.json")).await?;
    let series = version.split('.').next().unwrap_or("5");
    let name = php_for_computer(&info.php_version, &format!("PM{series}"))?;
    download_all(vec![DownloadItem::new(format!("{base}/PocketMine-MP.phar"), dir.join("PocketMine-MP.phar"))], 1, task.counter("Downloading PocketMine-MP")).await?;
    php_runtime(&format!("https://github.com/pmmp/PHP-Binaries/releases/download/pm{series}-php-{}-latest/{name}", info.php_version), dir, task).await
}

/// The newest build in Maven's snapshot metadata ("1.0-20261027.214338-1252"): the jar without a classifier.
fn nukkit_snapshot(xml: &str) -> Option<String> {
    let block = regex::Regex::new(r"(?s)<snapshotVersion>(.*?)</snapshotVersion>").ok()?;
    let value = regex::Regex::new(r"<value>([^<]+)</value>").ok()?;
    let jar = block.captures_iter(xml).map(|c| c[1].to_string()).find(|v| v.contains("<extension>jar</extension>") && !v.contains("<classifier>"))?;
    value.captures(&jar).map(|c| c[1].to_string())
}

/// With no nukkit.yml, Nukkit asks for a language in the console and waits. It fills in the rest of the file itself.
async fn skip_language_question(dir: &Path) -> Result<()> {
    Ok(tokio::fs::write(dir.join("nukkit.yml"), "settings:
  language: eng
").await?)
}

/// Cloudburst's newest Nukkit build. Returns the build number.
async fn nukkit(dir: &Path, task: &Arc<Task>) -> Result<String> {
    task.update("Finding Nukkit…", None);
    let xml = fetch_text(&format!("{NUKKIT}/maven-metadata.xml")).await?;
    let build = nukkit_snapshot(&xml).ok_or_else(|| anyhow!("Couldn't read Nukkit's newest build"))?;
    let url = format!("{NUKKIT}/nukkit-{build}.jar");
    let sha1 = fetch_text(&format!("{url}.sha1")).await.ok().map(|s| s.trim().to_string()).filter(|s| s.len() == 40);
    download_all(vec![DownloadItem::new(url, dir.join("nukkit.jar")).hash(sha1.map(Hash::sha1))], 1, task.counter("Downloading Nukkit")).await?;
    Ok(build.rsplit('-').next().unwrap_or(&build).to_string())
}

/// Cuberite's nightly for 64-bit Linux, the only build it still publishes.
async fn cuberite(dir: &Path, task: &Arc<Task>) -> Result<()> {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        bail!("Cuberite only has a build for 64-bit Linux");
    }
    if crate::game::java::on_musl() {
        bail!("Cuberite's Linux build needs glibc, so it can't run on Alpine. Use a Java server here, or another Linux");
    }
    let archive = dir.join("cuberite.tar.gz");
    download_all(vec![DownloadItem::new(CUBERITE, &archive)], 1, task.counter("Downloading Cuberite")).await?;
    task.update("Unpacking Cuberite…", None);
    let status = tokio::process::Command::new("tar").arg("-xzf").arg(&archive).arg("-C").arg(dir).status().await.context("Couldn't run tar")?;
    let _ = tokio::fs::remove_file(&archive).await;
    if !status.success() || !dir.join("Cuberite").is_file() {
        bail!("Cuberite's download couldn't be unpacked");
    }
    Ok(())
}

/// Spigot can't be redistributed as a ready-made jar, so it's compiled locally with Spigot's official
/// BuildTools (which fetches Git and the sources itself). Takes a few minutes the first time. `what`: "spigot" or
/// "craftbukkit".
async fn spigot(mc: &str, dir: &Path, java: &str, what: &str, task: &Arc<Task>) -> Result<()> {
    let work = paths::cache().join("buildtools");
    tokio::fs::create_dir_all(&work).await?;
    let tool = work.join("BuildTools.jar");
    task.update("Downloading Spigot BuildTools…", None);
    // Always fetch the newest BuildTools: older ones can't build new Minecraft versions.
    crate::core::net::download_file(&DownloadItem::new(BUILDTOOLS, &tool)).await?;
    let args = vec![
        "-jar".to_string(),
        plain(&tool),
        "--rev".into(),
        mc.into(),
        "--compile".into(),
        what.into(),
        "--output-dir".into(),
        plain(dir),
        "--nogui".into(),
    ];
    run_installer(java, &args, &work, task, "Spigot BuildTools").await?;
    let built = dir.join(format!("{what}-{mc}.jar"));
    if !built.is_file() {
        bail!("BuildTools finished without producing {what}-{mc}.jar");
    }
    tokio::fs::rename(&built, dir.join(format!("{what}.jar"))).await?;
    Ok(())
}

/// Fabric's server launcher (it fetches its libraries on first start) plus the vanilla server.jar it wraps.
async fn fabric(mc: &str, loader: &str, dir: &Path, task: &Arc<Task>) -> Result<()> {
    #[derive(Deserialize)]
    struct Installer {
        version: String,
        #[serde(default)]
        stable: bool,
    }
    task.update("Finding the Fabric server launcher…", None);
    let installers: Vec<Installer> = fetch_json(&format!("{FABRIC_META}/versions/installer")).await?;
    let installer = installers.iter().find(|i| i.stable).or(installers.first()).ok_or_else(|| anyhow!("Fabric's installer list is empty"))?;
    let url = format!("{FABRIC_META}/versions/loader/{}/{}/{}/server/jar", enc(mc), enc(loader), enc(&installer.version));
    download_all(vec![DownloadItem::new(url, dir.join("fabric-server-launch.jar"))], 1, task.counter("Downloading Fabric")).await?;
    vanilla_jar(mc, &dir.join("server.jar"), task).await
}

/// Quilt's installer in server mode, which writes quilt-server-launch.jar, its libraries and server.jar.
async fn quilt(mc: &str, loader: &str, dir: &Path, java: &str, task: &Arc<Task>) -> Result<()> {
    task.update("Finding the Quilt installer…", None);
    let xml = fetch_text(&format!("{QUILT_INSTALLER}/maven-metadata.xml")).await?;
    let ver = regex::Regex::new(r"<release>([^<]+)</release>")?
        .captures(&xml)
        .map(|c| c[1].to_string())
        .ok_or_else(|| anyhow!("Couldn't read Quilt's installer version"))?;
    let installer = paths::cache().join("server-installers").join(format!("quilt-installer-{ver}.jar"));
    let url = format!("{QUILT_INSTALLER}/{ver}/quilt-installer-{ver}.jar");
    download_all(vec![DownloadItem::new(url, &installer)], 1, task.counter("Downloading the Quilt installer")).await?;
    let args = vec![
        "-jar".to_string(),
        plain(&installer),
        "install".into(),
        "server".into(),
        mc.into(),
        loader.into(),
        "--download-server".into(),
        format!("--install-dir={}", plain(dir)),
    ];
    run_installer(java, &args, dir, task, "Quilt").await?;
    if !dir.join("quilt-server-launch.jar").is_file() {
        bail!("The Quilt installer finished without creating quilt-server-launch.jar");
    }
    Ok(())
}

/// Runs the Forge / NeoForge installer with `--installServer` and works out how to start the result:
/// the @argument file of modern versions (1.17+), or the forge jar of older ones.
async fn forge_like(loader: LoaderType, mc: &str, ver: &str, dir: &Path, java: &str, task: &Arc<Task>) -> Result<ServerLaunch> {
    let legacy = loader == LoaderType::Forge && is_legacy_forge(mc);
    if legacy && is_ancient_forge(mc) {
        return Err(legacy_forge_error(mc));
    }
    // Old builds are published with a branch suffix ("10.13.4.1614-1.7.10").
    let ver = &if legacy { full_forge_version(mc, ver).await } else { ver.to_string() };
    let name = format!("{} {ver}", loader.label());
    task.update(format!("Downloading the {name} installer…"), None);
    let installer = paths::cache().join("server-installers").join(format!("{}-{mc}-{ver}-installer.jar", loader.as_str()));
    if let Err(e) = download_all(vec![DownloadItem::new(installer_url(loader, mc, ver), &installer)], 1, |_, _| {}).await {
        if status_of(&e) == Some(404) {
            bail!("{name} for Minecraft {mc} couldn't be found");
        }
        return Err(e);
    }
    if legacy {
        if let Some(launch) = legacy_forge(mc, &name, &installer, dir, task).await? {
            return Ok(launch);
        }
    }
    let args = vec!["-jar".to_string(), plain(&installer), "--installServer".into(), plain(dir)];
    run_installer(java, &args, dir, task, &name).await?;

    let wanted = if cfg!(windows) { "win_args.txt" } else { "unix_args.txt" };
    let found = {
        let (root, ver, wanted) = (dir.join("libraries"), ver.to_string(), wanted.to_string());
        tokio::task::spawn_blocking(move || {
            let mut hits = Vec::new();
            find_files(&root, &wanted, 8, &mut hits);
            hits.sort_by_key(|p| !p.to_string_lossy().contains(&ver));
            hits.into_iter().next()
        })
        .await?
    };
    if let Some(args_file) = found {
        let rel = args_file.strip_prefix(dir).unwrap_or(&args_file).to_string_lossy().replace('\\', "/");
        return Ok(ServerLaunch { args_file: Some(rel), ..Default::default() });
    }
    // 1.13–1.16: the installer leaves forge-<mc>-<ver>.jar in the server folder.
    let mut rd = tokio::fs::read_dir(dir).await?;
    let mut jars = Vec::new();
    while let Some(e) = rd.next_entry().await? {
        let n = e.file_name().to_string_lossy().into_owned();
        let lower = n.to_lowercase();
        if (lower.starts_with("forge-") || lower.starts_with("neoforge-")) && lower.ends_with(".jar") && !lower.contains("installer") {
            jars.push(n);
        }
    }
    jars.sort_by_key(|n| !n.contains(ver));
    match jars.into_iter().next() {
        Some(j) => Ok(jar(&j)),
        None => bail!("The {name} installer finished, but ResinCraft couldn't find how to start the server"),
    }
}

/// Forge for Minecraft 1.7.10–1.12.2. Its installer's `--installServer` downloads from hosts that are gone (or
/// too slow to finish), so this does the same job: the universal jar from the installer, the libraries its
/// manifest's Class-Path lists (the launcher's own copies, else the moved Maven hosts) and the vanilla server jar.
/// The universal jar starts the server itself (FML's ServerLaunchWrapper: launchwrapper with the FML server
/// tweaker). Both installer formats are handled: install + versionInfo (up to 1.12.2's 14.23.5.2847) and the
/// processor-less modern one of the last 1.12.2 builds. None for anything else, which runs the installer.
async fn legacy_forge(mc: &str, name: &str, installer: &Path, dir: &Path, task: &Arc<Task>) -> Result<Option<ServerLaunch>> {
    task.update(tr!("forge.unpacking", "Unpacking {name}…", name = name), None);
    let found = {
        let (installer, dir, name) = (installer.to_path_buf(), dir.to_path_buf(), name.to_string());
        tokio::task::spawn_blocking(move || -> Result<Option<(String, Vec<String>, HashMap<String, String>)>> {
            let missing = |file: &str| err_tr!("forge.installerMissing", "The {name} installer is missing {file}", name = name, file = file);
            let mut zip = crate::content::index::open_zip(&installer)?;
            let text = crate::content::index::read_zip_text(&mut zip, "install_profile.json")?.ok_or_else(|| missing("install_profile.json"))?;
            let profile: Value = serde_json::from_str(&text)?;
            let libraries = |v: &Value| v.get("libraries").and_then(Value::as_array).cloned().unwrap_or_default();
            // The universal jar inside the installer, and each library's own Maven host by its path in the
            // libraries folder.
            let (entry, libs, base_of): (String, Vec<Value>, fn(&Value, &str) -> String) = match profile.get("install") {
                Some(install) => {
                    let file = install.get("filePath").and_then(Value::as_str).filter(|f| !f.is_empty()).ok_or_else(|| missing("install.filePath"))?;
                    let libs = profile.get("versionInfo").map(libraries).unwrap_or_default();
                    (file.to_string(), libs, |l, _| l.get("url").and_then(Value::as_str).unwrap_or_default().to_string())
                }
                None => {
                    let has_processors = profile.get("processors").and_then(Value::as_array).is_some_and(|p| !p.is_empty());
                    let (Some(path), Some(json)) = (profile.get("path").and_then(Value::as_str), profile.get("json").and_then(Value::as_str)) else { return Ok(None) };
                    if has_processors {
                        return Ok(None);
                    }
                    let version = crate::content::index::read_zip_text(&mut zip, json.trim_start_matches('/'))?.ok_or_else(|| missing(json))?;
                    let libs = libraries(&serde_json::from_str(&version)?);
                    let base = |l: &Value, rel: &str| {
                        let url = l.pointer("/downloads/artifact/url").and_then(Value::as_str).unwrap_or_default();
                        url.strip_suffix(rel).unwrap_or_default().to_string()
                    };
                    (format!("maven/{}", maven_path(path)), libs, base)
                }
            };
            let urls = libs
                .iter()
                .filter_map(|l| {
                    let rel = maven_path(l.get("name")?.as_str()?);
                    let base = base_of(l, &rel);
                    Some((rel, base))
                })
                .collect();
            let jar_name = crate::content::index::safe_name(entry.rsplit('/').next().unwrap_or(&entry))?.to_string();
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut zip.by_name(&entry).map_err(|_| missing(&entry))?, &mut bytes)?;
            let mut universal = zip::ZipArchive::new(std::io::Cursor::new(&bytes))?;
            let manifest = crate::content::index::read_zip_text(&mut universal, "META-INF/MANIFEST.MF")?.unwrap_or_default();
            // Manifest lines wrap at 72 bytes; continuation lines start with a space.
            let unfolded = manifest.replace("\r\n ", "").replace("\n ", "");
            let class_path: Vec<String> =
                unfolded.lines().find_map(|l| l.strip_prefix("Class-Path:")).map(|c| c.split_whitespace().map(str::to_string).collect()).unwrap_or_default();
            if class_path.is_empty() {
                return Err(missing("Class-Path"));
            }
            std::fs::write(dir.join(&jar_name), &bytes)?;
            Ok(Some((jar_name, class_path, urls)))
        })
        .await??
    };
    let Some((jar_name, class_path, urls)) = found else { return Ok(None) };

    let server_jar = format!("minecraft_server.{mc}.jar");
    vanilla_jar(mc, &dir.join(&server_jar), task).await?;

    let libs: Vec<String> = class_path.into_iter().filter_map(|c| c.strip_prefix("libraries/").map(str::to_string)).collect();
    let total = libs.len();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let progress = task.counter(tr!("forge.downloadingLibraries", "Downloading {name} libraries", name = name));
    let results = futures::stream::iter(libs.into_iter().map(|rel| {
        let (done, progress, urls) = (&done, &progress, &urls);
        async move {
            let r = legacy_forge_lib(&rel, urls.get(&rel).map(String::as_str), dir).await;
            progress(done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1, total);
            r
        }
    }))
    .buffer_unordered(8)
    .collect::<Vec<_>>()
    .await;
    results.into_iter().collect::<Result<Vec<()>>>()?;
    Ok(Some(jar(&jar_name)))
}

/// One library for an old Forge server: already there, copied from the launcher's libraries, or downloaded from
/// the first host that still has it.
async fn legacy_forge_lib(rel: &str, url: Option<&str>, dir: &Path) -> Result<()> {
    let dest = crate::core::fsutil::safe_join(&dir.join("libraries"), rel)?;
    if dest.is_file() {
        return Ok(());
    }
    let shared = crate::core::fsutil::safe_join(&paths::libraries(), rel)?;
    if shared.is_file() {
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        if tokio::fs::copy(&shared, &dest).await.is_ok() {
            return Ok(());
        }
    }
    let mut last = None;
    for base in legacy_lib_bases(url) {
        match download_file(&DownloadItem::new(format!("{base}{rel}"), &dest)).await {
            Ok(()) => return Ok(()),
            Err(e) => last = Some(e),
        }
    }
    let file = rel.rsplit('/').next().unwrap_or(rel);
    Err(last.unwrap_or_else(|| anyhow!("{file}")).context(err_tr!("forge.serverLibMissing", "Couldn't download {file}, which this Forge server needs", file = file)))
}

fn find_files(dir: &Path, name: &str, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            if depth > 0 {
                find_files(&p, name, depth - 1, out);
            }
        } else if e.file_name() == name {
            out.push(p);
        }
    }
}

/// Runs an installer jar, showing its progress lines in the task; on failure the last lines become the error.
async fn run_installer(java: &str, args: &[String], cwd: &Path, task: &Arc<Task>, name: &str) -> Result<()> {
    run_tool(java, args, cwd, task, name, None).await
}

async fn run_tool(program: &str, args: &[String], cwd: &Path, task: &Arc<Task>, name: &str, env: Option<(&str, &Path)>) -> Result<()> {
    task.update(format!("Running the {name} installer…"), None);
    let mut cmd = hidden_command(program);
    if let Some((key, value)) = env {
        cmd.env(key, value);
    }
    cmd.args(args)
        .current_dir(cwd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let mut child = cmd.spawn().with_context(|| format!("Couldn't start {program}"))?;
    let tail = Arc::new(std::sync::Mutex::new(VecDeque::<String>::new()));
    let mut readers = Vec::new();
    for stream in [child.stdout.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Send + Unpin>), child.stderr.take().map(|s| Box::new(s) as _)]
        .into_iter()
        .flatten()
    {
        let (tail, task, name) = (tail.clone(), task.clone(), name.to_string());
        readers.push(tokio::spawn(async move {
            let mut lines = BufReader::new(stream).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                task.update(format!("{name}: {}", line.chars().take(120).collect::<String>()), None);
                let mut t = tail.lock().unwrap();
                t.push_back(line);
                if t.len() > 25 {
                    t.pop_front();
                }
            }
        }));
    }
    let status = child.wait().await?;
    for r in readers {
        let _ = r.await;
    }
    if !status.success() {
        let tail: Vec<String> = tail.lock().unwrap().iter().cloned().collect();
        let last = tail.iter().rev().find(|l| l.to_lowercase().contains("error") || l.to_lowercase().contains("exception")).or(tail.last());
        bail!("The {name} installer failed ({status}){}", last.map(|l| format!(": {l}")).unwrap_or_default());
    }
    Ok(())
}

/// The version-specific loader info packs and instances carry, as the UI shows it.
pub fn describe(meta: &ServerMeta) -> String {
    match (&meta.loader_version, meta.loader.filter(|_| meta.kind.hybrid())) {
        (Some(v), Some(on)) => format!("{} {} ({}, {v})", meta.kind.label(), meta.mc_version, on.label()),
        (Some(v), None) => format!("{} {} ({v})", meta.kind.label(), meta.mc_version),
        (None, _) => format!("{} {}", meta.kind.label(), meta.mc_version),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catserver_files_name_their_version() {
        assert_eq!(catserver_mc("CatServer-1.16.5-1d8d6313-server.jar").as_deref(), Some("1.16.5"));
        assert_eq!(catserver_mc("CatServer-1.18.2-6c3f5965-server.jar").as_deref(), Some("1.18.2"));
        assert_eq!(catserver_mc("CatServer-4168d848-universal.jar").as_deref(), Some("1.12.2"));
        assert_eq!(catserver_mc("CatServer-4168d848-sources.jar"), None);
    }

    #[test]
    fn pumpkin_tags_name_the_minecraft_version() {
        assert_eq!(pumpkin_mc("0.2.1+26.3-26.51").as_deref(), Some("26.3"));
        assert_eq!(pumpkin_mc("0.1.0-dev+26.2-26.45").as_deref(), Some("26.2"));
        assert_eq!(pumpkin_mc("canary"), None);
        assert_eq!(pumpkin_mc("nightly"), None);
    }

    #[test]
    fn pumpkin_builds_per_computer() {
        assert_eq!(pumpkin_asset("windows", "x86_64"), Some("pumpkin-X64-Windows.exe"));
        assert_eq!(pumpkin_asset("linux", "aarch64"), Some("pumpkin-ARM64-Linux"));
        assert_eq!(pumpkin_asset("macos", "x86_64"), None);
    }

    #[test]
    fn horizon_builds_per_computer() {
        assert_eq!(horizon_archive("windows", "x86_64"), Some(("horizon-windows-x64.zip", "horizon-client.exe")));
        assert_eq!(horizon_archive("linux", "x86_64"), Some(("horizon-server-linux-x86_64.tar.gz", "horizon-engine")));
        assert_eq!(horizon_archive("linux", "aarch64"), None);
    }

    #[test]
    fn horizon_lock_names_its_minecraft() {
        assert_eq!(horizon_mc(r#"{"minecraft":{"version":"26.3","manifest_sha1":"x"}}"#).as_deref(), Some("26.3"));
        assert_eq!(horizon_mc("{}"), None);
    }

    #[test]
    fn php_builds_per_computer() {
        assert_eq!(php_asset("8.2", "PM5", "windows", "x86_64").as_deref(), Some("PHP-8.2-Windows-x64-PM5.zip"));
        assert_eq!(php_asset("8.2", "PM5", "linux", "x86_64").as_deref(), Some("PHP-8.2-Linux-x86_64-PM5.tar.gz"));
        assert_eq!(php_asset("8.2", "PM5", "macos", "aarch64").as_deref(), Some("PHP-8.2-MacOS-arm64-PM5.tar.gz"));
        assert_eq!(php_asset("8.2", "PM5", "linux", "aarch64"), None);
    }

    #[test]
    fn php_ini_gets_one_extension_folder() {
        assert_eq!(php_ini_with_extensions("a=1\nextension_dir=\"/old\"\n", "/srv/ext"), "a=1\nextension_dir=\"/srv/ext\"\n");
        assert_eq!(php_ini_with_extensions("", "/srv/ext"), "extension_dir=\"/srv/ext\"\n");
    }

    #[test]
    fn nukkit_build_is_the_plain_jar() {
        let xml = "<snapshotVersions>
            <snapshotVersion><extension>pom</extension><value>1.0-2-5</value></snapshotVersion>
            <snapshotVersion><classifier>dev</classifier><extension>jar</extension><value>1.0-2-4</value></snapshotVersion>
            <snapshotVersion><extension>jar</extension><value>1.0-2-3</value></snapshotVersion>
        </snapshotVersions>";
        assert_eq!(nukkit_snapshot(xml).as_deref(), Some("1.0-2-3"));
        assert_eq!(nukkit_snapshot("<metadata/>"), None);
    }

    #[test]
    fn spongeforge_versions_start_at_1_12_2() {
        assert!(spongeforge_lists("1.12.2") && spongeforge_lists("1.21.11") && spongeforge_lists("26.3"));
        assert!(!spongeforge_lists("1.12.1") && !spongeforge_lists("1.8"));
    }

    #[test]
    fn spongeforge_build_numbers_become_forge_versions() {
        assert_eq!(sponge_forge_version("1.12.2", "2838"), "14.23.5.2838");
        assert_eq!(sponge_forge_version("1.12.2", "14.23.5.2768"), "14.23.5.2768");
        assert_eq!(sponge_forge_version("1.16.5", "36.2.5"), "36.2.5");
    }
}
