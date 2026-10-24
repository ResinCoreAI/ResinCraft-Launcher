//! Rust mirror of `src/shared/types.ts`. Field names serialise to the same camelCase JSON the UI expects,
//! so keep the two files in step: change a type here, change it there.

use serde::{Deserialize, Serialize};

pub use super::text::Text;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoaderType {
    Vanilla,
    Fabric,
    Quilt,
    Forge,
    Neoforge,
    Liteloader,
}

impl LoaderType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
            Self::Forge => "forge",
            Self::Neoforge => "neoforge",
            Self::Liteloader => "liteloader",
        }
    }
    /// Human name, for messages.
    pub fn label(self) -> &'static str {
        match self {
            Self::Vanilla => "Vanilla",
            Self::Fabric => "Fabric",
            Self::Quilt => "Quilt",
            Self::Forge => "Forge",
            Self::Neoforge => "NeoForge",
            Self::Liteloader => "LiteLoader",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentKind {
    Mod,
    Resourcepack,
    Shader,
    Modpack,
}

impl ContentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mod => "mod",
            Self::Resourcepack => "resourcepack",
            Self::Shader => "shader",
            Self::Modpack => "modpack",
        }
    }
    /// Folder inside the game directory where this kind of content lives.
    pub fn folder(self) -> &'static str {
        match self {
            Self::Resourcepack => "resourcepacks",
            Self::Shader => "shaderpacks",
            _ => "mods",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentSource {
    Modrinth,
    Curseforge,
    Ftb,
    Atlauncher,
    Technic,
    /// Mods published as GitHub releases ("gh:owner/repo").
    Github,
}

/// Project ids carry their source as a prefix ("cf:238222", "ftb:100", "atl:Foo", "technic:bar", "gh:owner/repo");
/// bare ids are Modrinth.
pub fn source_of(project_id: &str) -> ContentSource {
    if project_id.starts_with("cf:") {
        ContentSource::Curseforge
    } else if project_id.starts_with("ftb:") {
        ContentSource::Ftb
    } else if project_id.starts_with("atl:") {
        ContentSource::Atlauncher
    } else if project_id.starts_with("technic:") {
        ContentSource::Technic
    } else if project_id.starts_with("gh:") {
        ContentSource::Github
    } else {
        ContentSource::Modrinth
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PendingDownload {
    pub title: String,
    pub file_name: String,
    pub url: String,
    pub folder: String,
    /// Size and SHA-1, when the source says them: a file found in Downloads must match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    /// "cf:<id>" and the file id, so the file is tracked (updates, content list) once it's in place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModpackRef {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    pub loader: LoaderType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loader_version: Option<String>,
    pub color: String,
    /// A web URL (modpack icon) or, for a custom icon picked from a screenshot, a data URL of `icon.png`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    /// The modpack's own icon while a custom one replaces it, so resetting the icon brings it back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack_icon_url: Option<String>,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_played: Option<i64>,
    #[serde(default)]
    pub playtime: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_mb: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jvm_args: Option<String>,
    /// ResinCraft's G1GC flags for this instance; None follows Settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optimized_flags: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modpack: Option<ModpackRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_downloads: Option<Vec<PendingDownload>>,
    /// Library group; None = Ungrouped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// A data URL of banner.jpg (16:9), shown on Library cards and the instance page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner_url: Option<String>,
    /// OptiFine the player downloaded and imported (see game/optifine.rs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optifine: Option<OptifineRef>,
    /// Set when the player made this instance as their own modpack (New instance → Create a modpack).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub own_pack: Option<OwnPack>,
    /// Environment variables for the game, added to the ones in Settings (a name set here wins).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<EnvVar>,
    /// Commands run around a launch instead of the ones in Settings (see game/commands.rs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commands: Option<LaunchCommands>,
    /// LiteLoader version loaded next to Forge (a LiteLoader-only instance has loader `liteloader` instead).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liteloader: Option<String>,
    /// Zips merged into the game jar, in order (later ones win); files in the instance's jarmods/ folder.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub jar_mods: Vec<JarMod>,
    /// The player's own changes to the version the game starts with (instance settings → Version).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_patch: Option<VersionPatch>,
}

/// One environment variable the player set for the game and its commands.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EnvVar {
    pub name: String,
    #[serde(default)]
    pub value: String,
}

/// The player's own commands around a launch, as Prism Launcher has them. Empty means none.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct LaunchCommands {
    /// Runs before Java starts; a non-zero exit stops the launch.
    pub pre_launch: String,
    /// The game runs through it (`prime-run`); `{java}` marks where the Java command goes, else it goes last.
    pub wrapper: String,
    /// Runs after the game closes, with its exit code in `$INST_EXIT_CODE`.
    pub post_exit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JarMod {
    /// File name in the instance's jarmods/ folder.
    pub file: String,
    /// Shown name (the file the player picked).
    pub name: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
}

/// Changes applied over the loader's version JSON at launch (see game/versionpatch.rs). Empty values change nothing.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct VersionPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_class: Option<String>,
    /// Extra libraries, put in front of the classpath.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub libraries: Vec<PatchLibrary>,
    /// launchwrapper tweak classes (`--tweakClass`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tweakers: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub jvm_args: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub game_args: Vec<String>,
}

/// A library: Maven coordinates ("group:artifact:version[:classifier]") with the repository to get it from, or a
/// jar file kept in the instance's libraries/ folder (`file`).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PatchLibrary {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

/// The details of a modpack the player is making; the export dialog starts from them and saves them back.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OwnPack {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub summary: String,
    /// Suggest the next version (1.0.0 → 1.0.1) when exporting again after `version` was exported.
    #[serde(default)]
    pub bump_on_export: bool,
    /// The version last exported, and when (ms); none until the first export.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exported_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exported_at: Option<i64>,
}

/// An imported OptiFine jar: `version` is "<mc>_<edition>" ("1.20.1_HD_U_I6"), `file` the jar's name in mods/
/// (Forge, Fabric, Quilt) or in the instance's optifine/ folder (vanilla).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OptifineRef {
    pub version: String,
    pub file: String,
}

/// What `optifine:inspect` found in a jar.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OptifineJar {
    pub version: String,
    pub mc_version: String,
    /// "HD U I6", for display.
    pub edition: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewInstance {
    pub name: String,
    pub mc_version: String,
    pub loader: LoaderType,
    #[serde(default)]
    pub loader_version: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McVersion {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub release_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoaderVersion {
    pub version: String,
    pub stable: bool,
}

/// A build of server software to choose from (Arclight: one per mod loader it runs on).
#[derive(Debug, Clone, Serialize)]
pub struct ServerBuild {
    pub id: String,
    pub label: String,
    pub stable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub username: String,
    pub uuid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skin_url: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
}

/// A Microsoft account's Minecraft profile, for the Accounts page's Manage view.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerProfile {
    pub id: String,
    pub name: String,
    /// None means Minecraft's default skin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skin: Option<SkinInfo>,
    pub capes: Vec<CapeInfo>,
    /// None when Mojang didn't say (the lookup is best effort).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_change: Option<NameChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinInfo {
    pub url: String,
    /// "classic" | "slim"
    pub variant: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapeInfo {
    pub id: String,
    pub alias: String,
    pub url: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NameChange {
    pub allowed: bool,
    /// When the name was last changed (ISO 8601), if ever.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed_at: Option<String>,
}

/// A new skin: a PNG (base64, optionally as a data: URL) or a public image link, plus the arm style.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinUpload {
    /// "classic" | "slim"
    pub variant: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub png: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// A skin in the launcher's skin library (Accounts → Manage → Skin).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSkin {
    pub id: String,
    pub name: String,
    /// "classic" | "slim"
    pub variant: String,
    /// "upload" | "player" | "online" | "history"
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    pub added_at: i64,
    /// data: URL of the PNG.
    pub preview: String,
}

/// A skin found online (MineSkin's gallery) or on a player, not saved yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineSkin {
    pub id: String,
    pub name: String,
    /// The texture on textures.minecraft.net.
    pub url: String,
    /// "classic" | "slim"; None when the source didn't say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    /// "player" | "mineskin"
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineSkinPage {
    pub skins: Vec<OnlineSkin>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginEvent {
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<AccountView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// `error` in a form the UI can translate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_text: Option<Text>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaInstall {
    pub path: String,
    pub version: String,
    pub major: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    pub managed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub platform: String,
    pub total_mem_mb: u64,
    pub dev: bool,
    /// This build can sign in with Microsoft (has a Client ID).
    pub microsoft: bool,
    /// This build can use CurseForge (has an API key).
    pub curseforge: bool,
    /// This build can show Discord Rich Presence (has a Discord app ID).
    pub discord: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInfo {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
    /// "running" | "done" | "error" | "cancelled"
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    /// `title`, `detail` and `error` in a form the UI can translate (absent for plain text).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_text: Option<Text>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail_text: Option<Text>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_text: Option<Text>,
    /// The dock offers Cancel.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub cancellable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameEvent {
    pub instance_id: String,
    /// One start of the instance: the instance id for the first, `{id}~2`… for more at once (other accounts).
    pub run_id: String,
    pub account_id: String,
    pub username: String,
    /// "preparing" | "running" | "stopped" | "crashed"
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogLine {
    pub text: String,
    /// "info" | "warn" | "error" | "debug"
    pub level: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEvent {
    pub instance_id: String,
    pub run_id: String,
    pub lines: Vec<LogLine>,
}

/// A game being prepared or running (`instances:runs`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunView {
    pub instance_id: String,
    pub run_id: String,
    pub account_id: String,
    pub username: String,
    /// "preparing" | "running"
    pub state: &'static str,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub query: String,
    pub kind: ContentKind,
    #[serde(default)]
    pub source: Option<ContentSource>,
    #[serde(default)]
    pub game_version: Option<String>,
    #[serde(default)]
    pub loader: Option<LoaderType>,
    /// "relevance" | "downloads" | "follows" | "newest" | "updated"
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub offset: Option<u32>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHit {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    pub author: String,
    pub downloads: u64,
    pub follows: u64,
    pub categories: Vec<String>,
    pub kind: ContentKind,
    pub updated: String,
    pub source: ContentSource,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub hits: Vec<ProjectHit>,
    pub total: u64,
    pub offset: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectVersion {
    pub id: String,
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    /// "release" | "beta" | "alpha"
    #[serde(rename = "type")]
    pub kind: String,
    pub published: String,
    pub downloads: u64,
}

/// A project's page inside Discover. HTML here comes straight from the source (or from its Markdown) and is
/// sanitized by the UI before it's shown.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetails {
    #[serde(flatten)]
    pub hit: ProjectHit,
    /// The long description as HTML; empty when the source only has the short one.
    pub body: String,
    pub gallery: Vec<GalleryImage>,
    pub links: Vec<ProjectLink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Whether `version_info` can tell a version's changelog / dependencies for this source.
    pub changelogs: bool,
    pub dependencies: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryImage {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumb: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectLink {
    /// "website" | "source" | "issues" | "wiki" | "discord" | "donate"
    pub kind: String,
    pub url: String,
}

impl ProjectLink {
    /// The links that are set, in the order given.
    pub fn list<'a>(links: impl IntoIterator<Item = (&'a str, Option<String>)>) -> Vec<ProjectLink> {
        let mut out: Vec<ProjectLink> = Vec::new();
        for (kind, url) in links {
            if let Some(url) = url.map(|u| u.trim().to_string()).filter(|u| u.starts_with("http")) {
                if !out.iter().any(|l| l.url == url) {
                    out.push(ProjectLink { kind: kind.into(), url });
                }
            }
        }
        out
    }
}

/// One version's changelog (HTML) and what it needs.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changelog: Option<String>,
    pub dependencies: Vec<ProjectDependency>,
}

/// One mod in a modpack, for its details page (content:packMods).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PackMod {
    pub title: String,
    /// Its project, when the pack says which (opens its own details page).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<ContentSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    /// The jar's file name, when the pack lists it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDependency {
    /// The project's id on the same source, when known (opens its own details page).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    /// "required" | "optional" | "embedded" | "incompatible"
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRef {
    pub version_id: String,
    pub version_number: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentItem {
    pub filename: String,
    pub enabled: bool,
    pub size: u64,
    pub kind: ContentKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update: Option<UpdateRef>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportEntry {
    pub name: String,
    pub is_dir: bool,
    pub suggested: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    /// "mrpack" | "curseforge"
    pub format: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub summary: String,
    pub include: Vec<String>,
    /// For the player's own modpack: whether the next export should suggest a bumped version.
    #[serde(default)]
    pub bump_on_export: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub path: String,
    pub linked: u64,
    pub bundled: Vec<String>,
    pub skipped_disabled: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldInfo {
    pub folder: String,
    pub name: String,
    /// "survival" | "creative" | "adventure" | "spectator"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_mode: Option<String>,
    pub hardcore: bool,
    pub cheats: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_played: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// `error` in a form the UI can translate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_text: Option<Text>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerEntry {
    pub name: String,
    pub ip: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accept_textures: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerPlayers {
    pub online: i64,
    pub max: i64,
    pub sample: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerStatus {
    pub motd: String,
    pub players: ServerPlayers,
    pub version: String,
    pub latency: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favicon: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScreenshotInfo {
    pub name: String,
    pub size: u64,
    pub mtime: i64,
}

// ---------- Server hosting ----------

/// The server software a hosted server runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerKind {
    Vanilla,
    Paper,
    Purpur,
    Folia,
    /// Built locally with Spigot's BuildTools (Spigot can't be downloaded ready-made).
    Spigot,
    /// A Paper fork tuned for performance.
    Leaf,
    Fabric,
    Quilt,
    Forge,
    Neoforge,
    /// Mods and Bukkit plugins together, on Forge, NeoForge or Fabric (which one is in `ServerMeta::loader`).
    Arclight,
    /// Bedrock Dedicated Server from minecraft.net: its own program, UDP port, worlds and packs (no Java).
    Bedrock,
}

impl ServerKind {
    /// The mod loader behind a mod server (None for vanilla and plugin servers).
    pub fn loader(self) -> Option<LoaderType> {
        match self {
            Self::Fabric => Some(LoaderType::Fabric),
            Self::Quilt => Some(LoaderType::Quilt),
            Self::Forge => Some(LoaderType::Forge),
            Self::Neoforge => Some(LoaderType::Neoforge),
            _ => None,
        }
    }
    pub fn from_loader(l: LoaderType) -> ServerKind {
        match l {
            LoaderType::Vanilla | LoaderType::Liteloader => Self::Vanilla,
            LoaderType::Fabric => Self::Fabric,
            LoaderType::Quilt => Self::Quilt,
            LoaderType::Forge => Self::Forge,
            LoaderType::Neoforge => Self::Neoforge,
        }
    }
    /// Bukkit-style plugin servers (plugins/ folder).
    pub fn plugins(self) -> bool {
        matches!(self, Self::Paper | Self::Purpur | Self::Folia | Self::Spigot | Self::Leaf)
    }
    /// Hybrids (Arclight) run mods (mods/) and Bukkit plugins (plugins/) together.
    pub fn hybrid(self) -> bool {
        matches!(self, Self::Arclight)
    }
    /// Runs mods, as a mod loader or a hybrid.
    pub fn modded(self) -> bool {
        self.loader().is_some() || self.hybrid()
    }
    pub fn bedrock(self) -> bool {
        self == Self::Bedrock
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Vanilla => "Vanilla",
            Self::Paper => "Paper",
            Self::Purpur => "Purpur",
            Self::Folia => "Folia",
            Self::Spigot => "Spigot",
            Self::Leaf => "Leaf",
            Self::Fabric => "Fabric",
            Self::Quilt => "Quilt",
            Self::Forge => "Forge",
            Self::Neoforge => "NeoForge",
            Self::Arclight => "Arclight",
            Self::Bedrock => "Bedrock",
        }
    }
}

/// How a server is started: `-jar <jar>`, a Java @argument file (modern Forge/NeoForge), or a program of its own
/// (Bedrock). Paths are relative to the server folder.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerLaunch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jar: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exe: Option<String>,
}

fn yes() -> bool {
    true
}

/// `<dataDir>/servers/<id>/server.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerMeta {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: ServerKind,
    pub mc_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loader: Option<LoaderType>,
    /// Mod loader version, or the build of Paper, Purpur, Folia, Leaf or Arclight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loader_version: Option<String>,
    pub memory_mb: u32,
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jvm_args: Option<String>,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_started: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modpack: Option<ModpackRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub launch: ServerLaunch,
    /// Open the port on the router automatically (UPnP) while the server runs.
    #[serde(default = "yes")]
    pub public: bool,
    /// Start Java with Aikar's tuned G1GC flags (and -Xms = -Xmx).
    #[serde(default = "yes")]
    pub optimized_flags: bool,
    /// Whitelist requests through the host's Discord bot or the relay's web page (server/requests.rs).
    #[serde(default)]
    pub join_requests: JoinRequestsConfig,
    /// Start Java with the flags the server pack sets for itself (user_jvm_args.txt, variables.txt, its start
    /// script) instead of ResinCraft's; ResinCraft's are used when the pack sets none.
    #[serde(default)]
    pub pack_flags: bool,
    /// Files the server needs whose authors only allow downloads from their website. Before each start ResinCraft
    /// looks for them (kept copies, the same pack's instances, Downloads) and puts the ones it finds in place.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_downloads: Vec<PendingDownload>,
    /// Made from a modpack that has no official server pack: ResinCraft built the server from the pack's own
    /// file list, client-only mods included (the server page says so).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub built_pack: bool,
    /// How the server is shared over the internet when `public` is on.
    #[serde(default)]
    pub tunnel: TunnelMode,
    /// A bore relay to use instead of bore.pub ("host" or "host:port"), e.g. a self-hosted one nearby.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_host: Option<String>,
    /// The secret of that relay (`bore server --secret`), if it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_secret: Option<String>,
    /// The player's own domain for this server (normalised: lowercase ASCII/punycode, no scheme or port).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    /// Keep the domain's DNS records up to date through the Cloudflare API (token kept apart, sealed).
    #[serde(default)]
    pub dns_auto: bool,
    /// The server's ResinCraft Relay name (friends join `<name>.<relay domain>`), once chosen or reserved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_name: Option<String>,
    /// The Minecraft account (UUID) that name is reserved for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_account: Option<String>,
}

/// How a public server reaches the internet. The bore relay (bore.pub or `relay_host`) is always the last resort.
/// New servers start on playit.gg; `Auto` is what older server.json files without the field mean.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TunnelMode {
    /// Open the port on the router (UPnP, direct), else ResinCraft Relay (with a Microsoft account), else
    /// playit.gg (once linked), else the bore relay.
    #[default]
    Auto,
    /// ResinCraft Relay: `<name>.rscraft.resincore.tech`, stable, needs a Microsoft account; else the bore relay.
    Resin,
    /// playit.gg: a stable *.joinmc.link address after linking a playit account once, else the relay.
    Playit,
    /// Always the relay.
    Relay,
    /// The player forwarded the port on the router themselves: share the public IP and port directly.
    Manual,
}

/// Internet access for a running server (UPnP).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServerNet {
    /// "off" | "opening" | "open" | "failed"
    pub state: &'static str,
    /// Public "ip:port" to share once the router opened the port.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// "router" (port opened with UPnP, direct), "resin" (ResinCraft Relay), "playit" (playit.gg), "relay"
    /// (the bore relay, no setup) or "manual" (the player's own port forward).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via: Option<&'static str>,
    /// `error` as translatable text, when it is one (the UI shows this instead).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_text: Option<Text>,
    /// While opening through playit.gg for the first time: the page where the player links this launcher.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_url: Option<String>,
    /// Whether the server's own domain points at this address (None: no domain, or not checked yet).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_ok: Option<bool>,
    /// Automatic fell back to the relay because playit.gg isn't linked yet: suggest linking it.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub playit_hint: bool,
    /// Java servers with Geyser: where Bedrock players join (UDP), through the router or playit.gg.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bedrock_address: Option<String>,
    /// ResinCraft Relay: the name, traffic and connection state while `via` is "resin", or why it wasn't used
    /// when another route took over.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resin: Option<ResinNet>,
    /// Automatic skipped ResinCraft Relay because no Microsoft account is signed in: suggest signing in.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub resin_hint: bool,
}

fn is_zero_u64(v: &u64) -> bool {
    *v == 0
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

/// ResinCraft Relay's part of a server's internet state.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResinNet {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The connection to the relay dropped; reconnecting (the address stays the same).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub reconnecting: bool,
    /// The account's relay traffic this month, and its monthly limit with top-ups (0: none).
    pub used_bytes: u64,
    pub limit_bytes: u64,
    /// The relay's admin gave the account unlimited use (no data cap, no session time limit).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub unlimited: bool,
    /// "low" (less than the relay's threshold left, 1 GB by default) or "out" (used up) this month.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<&'static str>,
    /// What one top-up adds (0: the relay has none), how many are left this month, and whether one would be
    /// granted now.
    #[serde(skip_serializing_if = "is_zero_u64")]
    pub topup_bytes: u64,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub topups_left: u32,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub can_top_up: bool,
    /// Data the relay's admin added while this server was online (the app says so when it grows).
    #[serde(skip_serializing_if = "is_zero_u64")]
    pub admin_added_bytes: u64,
    /// The host may press Continue once this little time is left (seconds).
    #[serde(skip_serializing_if = "is_zero_u64")]
    pub continue_secs: u64,
    /// The session is within that time (the app reminds the host once).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub soon: bool,
    /// Why ResinCraft Relay wasn't used (another route took over).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Text>,
    /// Local time (Unix ms) when the relay session ends (None: no limit).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ends_at: Option<i64>,
    /// Minutes left at the last warning, until the host continues (the Console banner shows it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<u32>,
    pub continues: u32,
    /// The relay's admin paused it or switched it off: their message (empty: none given).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paused: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub off: bool,
    /// The session time ran out (the card offers to start it again).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub ended: bool,
    /// The relay's admin banned the account: `error` says until when and why. The launcher doesn't reconnect.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub banned: bool,
}

/// The ResinCraft Relay part of a server's Settings, before asking the relay anything.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayInfo {
    /// This build includes a relay.
    pub available: bool,
    /// Its base domain, e.g. "rscraft.resincore.tech".
    pub domain: String,
    /// The Microsoft account it would use (None: none signed in).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    /// The server's name, once chosen.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// A name made from the server's name, for an empty field.
    pub suggested: String,
}

/// Whether a ResinCraft Relay name can be used.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayNameCheck {
    pub name: String,
    pub available: bool,
    /// Already reserved for this account.
    pub mine: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<Text>,
}

/// A DNS record the player should have for their domain.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DnsRecord {
    /// "A" | "AAAA" | "CNAME" | "SRV"
    pub kind: &'static str,
    /// The full record name, e.g. "play.example.com" or "_minecraft._tcp.play.example.com".
    pub name: String,
    /// A/AAAA: the IP address. CNAME, SRV: the target host.
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<u16>,
}

/// One record of a DNS check: what was found against what's expected.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsRecordCheck {
    pub record: DnsRecord,
    pub ok: bool,
    /// What the public resolver returned for that name and type ("1.2.3.4", "0 5 25566 host.example.com").
    pub found: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsCheck {
    pub at: i64,
    pub ok: bool,
    /// The public resolver that answered ("1.1.1.1" or "8.8.8.8").
    pub resolver: String,
    pub records: Vec<DnsRecordCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Text>,
}

/// The server's own domain as the Settings tab shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    /// How the server is reached right now: "direct" | "resin" | "playit" | "relay" | "none" (not running or no address).
    pub route: &'static str,
    pub records: Vec<DnsRecord>,
    /// Records that work instead of `records` (ResinCraft Relay: an SRV record, for a domain without a subdomain).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub alt_records: Vec<DnsRecord>,
    /// Direct route only: the line players can put in their own device's hosts file instead of DNS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hosts_line: Option<String>,
    /// playit.gg on a guest (unclaimed) account: the address may change.
    pub playit_guest: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check: Option<DnsCheck>,
    /// A Cloudflare API token is saved for this server.
    pub cloudflare_token: bool,
    /// What the last automatic Cloudflare update did (or why it failed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloudflare_status: Option<Text>,
}

/// A server as the UI sees it: its metadata plus live state.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    #[serde(flatten)]
    pub meta: ServerMeta,
    pub eula_accepted: bool,
    /// "stopped" | "starting" | "running" | "stopping" | "crashed"
    pub state: &'static str,
    pub players: Vec<String>,
    /// How many players the server says are online (its list can hide names, or hold only 12).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub net: Option<ServerNet>,
}

/// How a hosted server takes whitelist requests.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct JoinRequestsConfig {
    pub enabled: bool,
    /// From the web page at the server's ResinCraft Relay address (while it's online there).
    pub web: bool,
    /// From the host's own Discord bot (`/whitelist`), while ResinCraft runs.
    pub discord: bool,
}

/// A player waiting to be whitelisted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JoinRequest {
    pub id: String,
    pub username: String,
    /// "web" | "discord"
    pub source: String,
    /// Their Discord name, for requests from the bot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Their Discord user id, to tell them the answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discord_user: Option<String>,
    /// The relay's id for a web request (Reject and block names it; the relay knows who sent it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_id: Option<u64>,
    pub at: i64,
}

/// Where a new server's content comes from (besides plain server software).
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", rename_all_fields = "camelCase")]
pub enum ServerSource {
    /// A Modrinth, CurseForge ("cf:") or FTB ("ftb:") modpack.
    Modpack { project_id: String, version_id: Option<String> },
    /// A ResinCraft instance (its mods minus client-only ones, plus configs).
    Instance { instance_id: String },
    /// A local .mrpack or CurseForge .zip; a file picker opens when there's no path.
    File { path: Option<String> },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewServer {
    pub name: String,
    #[serde(default, rename = "type")]
    pub kind: Option<ServerKind>,
    #[serde(default)]
    pub mc_version: Option<String>,
    #[serde(default)]
    pub loader_version: Option<String>,
    #[serde(default)]
    pub memory_mb: Option<u32>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub source: Option<ServerSource>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCreated {
    pub server: ServerInfo,
    /// Client-only mods that were left out.
    pub dropped: Vec<String>,
    /// Files the pack author only allows from their website.
    pub pending: Vec<PendingDownload>,
    /// The pack has no official server pack, so ResinCraft built the server from its file list.
    pub built_pack: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerEvent {
    pub server_id: String,
    pub state: &'static str,
    pub players: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub net: Option<ServerNet>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerLogEvent {
    pub server_id: String,
    pub lines: Vec<LogLine>,
}

/// One server.properties line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropEntry {
    pub key: String,
    pub value: String,
}

/// An entry of whitelist.json, ops.json, banned-players.json or banned-ips.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerListEntry {
    /// Player name, or the address for banned-ips.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// A plugin or mod found for a hosted server.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerAddonHit {
    /// Bare id for Modrinth, "hangar:<slug>", "spigot:<id>", "cf:<id>", "gh:<owner>/<repo>".
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    pub author: String,
    pub downloads: u64,
    /// GitHub: stars instead of downloads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stars: Option<u64>,
    /// "modrinth" | "hangar" | "spigot" | "bukkit" | "curseforge" | "github"
    pub source: &'static str,
    /// Can't be installed from the launcher (premium, or hosted on another website): open `url` instead.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub external: bool,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerAddonResult {
    pub hits: Vec<ServerAddonHit>,
    pub total: u64,
}

/// An installed plugin or mod on a hosted server, with where it came from and any change waiting for a restart.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerAddon {
    #[serde(flatten)]
    pub item: ContentItem,
    /// "modrinth" | "hangar" | "spigot" | "curseforge"; None = unknown source (no updates).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<&'static str>,
    /// A change queued until the server stops: "update" | "enable" | "disable" | "remove".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending: Option<&'static str>,
    /// For a queued update: the version it switches to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_version: Option<String>,
    /// Release notes of the available update (set by an update check).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_url: Option<String>,
}

/// One version of a server plugin or mod, marked for whether it fits the server.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerAddonVersion {
    #[serde(flatten)]
    pub version: ProjectVersion,
    /// Made for this server's Minecraft version and platform/loader.
    pub compatible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changelog_url: Option<String>,
}
