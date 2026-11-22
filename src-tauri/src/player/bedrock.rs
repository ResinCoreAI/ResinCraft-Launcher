//! `bedrock:*`: Minecraft Bedrock Edition (Minecraft for Windows, and Minecraft Preview) as installed from the
//! Microsoft Store or the Xbox app. ResinCraft finds the installed version and the worlds, opens the game, and
//! has the Store's install service install, update or repair it in a task (`bedrock_store`), with the Store's own
//! pages as the way out when that fails. It also lists the older versions of Minecraft for Windows
//! (`bedrock_versions`), works out what a switch to one would need (`bedrock_switch`), and backs up and restores
//! worlds (`bedrock_backup`).

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::bedrock_versions::Format;
use crate::core::router::{done, json};
use crate::core::{shell, Args};

/// The two editions: the game, and Minecraft Preview (beta builds of the next update).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edition {
    Release,
    Preview,
}

impl Edition {
    const ALL: [Edition; 2] = [Edition::Release, Edition::Preview];
    /// The app package's name.
    fn package(self) -> &'static str {
        match self {
            Edition::Release => "Microsoft.MinecraftUWP",
            Edition::Preview => "Microsoft.MinecraftWindowsBeta",
        }
    }
    /// The link that opens the game.
    fn protocol(self) -> &'static str {
        match self {
            Edition::Release => "minecraft:",
            Edition::Preview => "minecraft-preview:",
        }
    }
    /// Its Microsoft Store product id.
    pub(crate) fn product_id(self) -> &'static str {
        match self {
            Edition::Release => "9NBLGGH2JHXJ",
            Edition::Preview => "9P5X4QVLC2XR",
        }
    }
    pub(crate) fn key(self) -> &'static str {
        match self {
            Edition::Release => "release",
            Edition::Preview => "preview",
        }
    }
    /// The app's name (the same in every language).
    pub(crate) fn name(self) -> &'static str {
        match self {
            Edition::Release => "Minecraft for Windows",
            Edition::Preview => "Minecraft Preview",
        }
    }
    /// The Microsoft Store's compact install window for it (the "mini" product page).
    fn store(self) -> String {
        store_page(self.product_id())
    }
    /// Where the game keeps its data: `%APPDATA%\Minecraft Bedrock` (and the older app-package folder).
    pub(crate) fn data_dirs(self) -> Vec<PathBuf> {
        let appdata = std::env::var_os("APPDATA").map(PathBuf::from);
        let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        let (name, family) = match self {
            Edition::Release => ("Minecraft Bedrock", "Microsoft.MinecraftUWP_8wekyb3d8bbwe"),
            Edition::Preview => ("Minecraft Bedrock Preview", "Microsoft.MinecraftWindowsBeta_8wekyb3d8bbwe"),
        };
        let mut out = Vec::new();
        if let Some(a) = appdata {
            out.push(a.join(name));
        }
        if let Some(l) = local {
            out.push(l.join("Packages").join(family).join("LocalState"));
        }
        out
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BedrockWorld {
    /// The world's folder name: its id for `bedrock:openFolder`.
    pub id: String,
    pub name: String,
    /// Last played (ms since 1970).
    pub modified: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BedrockEdition {
    pub edition: Edition,
    pub installed: bool,
    /// The game version, e.g. "1.26.52.3".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Worlds, the most recently played first.
    pub worlds: Vec<BedrockWorld>,
    /// How the installed copy is packaged: GDK from 1.21.120, UWP before.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<Format>,
    /// ResinCraft switched it to this version (it isn't the Store's current one).
    pub switched: bool,
}

/// Gaming Services: the Xbox service that installs and runs Store games like Bedrock.
const GAMING_SERVICES: &str = "Microsoft.GamingServices";
const GAMING_SERVICES_ID: &str = "9MWPM2CQNLHN";
/// The Xbox app, where the Windows account games sign in with is chosen.
const XBOX_APP_ID: &str = "9MV0B5HZVK9Z";

fn store_page(product_id: &str) -> String {
    format!("ms-windows-store://pdp/?productid={product_id}&mode=mini")
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BedrockStatus {
    /// Bedrock Edition only exists as an app on Windows.
    pub supported: bool,
    pub editions: Vec<BedrockEdition>,
    /// Gaming Services is installed (Windows only; the Store usually adds it with the game).
    pub gaming_services: bool,
    /// Windows' Developer Mode is on (Settings > System > For developers), which the UWP versions need.
    /// ResinCraft only reads it.
    pub developer_mode: bool,
}

/// "1.26.5203.0" (the app package's version) → "1.26.52.3" (the game's).
fn game_version(package: &str) -> String {
    let parts: Vec<u32> = package.split('.').filter_map(|p| p.parse().ok()).collect();
    match parts.as_slice() {
        [major, minor, build, ..] if *build >= 100 => {
            let (patch, revision) = (build / 100, build % 100);
            if revision > 0 {
                format!("{major}.{minor}.{patch}.{revision}")
            } else {
                format!("{major}.{minor}.{patch}")
            }
        }
        _ => package.to_string(),
    }
}

/// What Windows knows about an installed app package.
#[derive(Debug, Clone, PartialEq)]
struct Installed {
    name: String,
    /// The app package's version ("1.26.6501.0").
    version: String,
    /// Registered from a folder (which takes Developer Mode) instead of installed from a package.
    development: bool,
    location: Option<String>,
}

/// One PowerShell run for both: the installed packages (Get-AppxPackage) and whether Developer Mode is on (the
/// setting's registry value, or the policy that sets it). No double quotes: it passes through -Command as it is.
const WINDOWS_STATE_SCRIPT: &str = "$p = @(Get-AppxPackage -Name 'Microsoft.Minecraft*') + @(Get-AppxPackage -Name 'Microsoft.GamingServices') | Select-Object Name, Version, IsDevelopmentMode, InstallLocation; \
     $k = 'AllowDevelopmentWithoutDevLicense'; \
     $d = (Get-ItemProperty -Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\AppModelUnlock' -Name $k -ErrorAction SilentlyContinue).$k; \
     $g = (Get-ItemProperty -Path 'HKLM:\\SOFTWARE\\Policies\\Microsoft\\Windows\\Appx' -Name $k -ErrorAction SilentlyContinue).$k; \
     @{ packages = @($p); developerMode = (($d -eq 1) -or ($g -eq 1)) } | ConvertTo-Json -Compress -Depth 3";

/// Reads the script's answer: the packages, and Developer Mode.
fn parse_windows_state(text: &str) -> (Vec<Installed>, bool) {
    let v: Value = serde_json::from_str(text.trim()).unwrap_or(Value::Null);
    // One package can come back as an object, several as a list.
    let list = match v.get("packages") {
        Some(Value::Array(a)) => a.clone(),
        Some(p @ Value::Object(_)) => vec![p.clone()],
        _ => Vec::new(),
    };
    let packages = list
        .iter()
        .filter_map(|p| {
            Some(Installed {
                name: p.get("Name")?.as_str()?.to_string(),
                version: p.get("Version")?.as_str()?.to_string(),
                development: p.get("IsDevelopmentMode").and_then(Value::as_bool).unwrap_or(false),
                location: p.get("InstallLocation").and_then(Value::as_str).filter(|l| !l.is_empty()).map(str::to_string),
            })
        })
        .collect();
    (packages, v.get("developerMode").and_then(Value::as_bool).unwrap_or(false))
}

async fn windows_state() -> (Vec<Installed>, bool) {
    if !cfg!(windows) {
        return (Vec::new(), false);
    }
    let out = crate::game::java::hidden_command("powershell").args(["-NoProfile", "-NonInteractive", "-Command", WINDOWS_STATE_SCRIPT]).output().await;
    match out {
        Ok(out) => parse_windows_state(&String::from_utf8_lossy(&out.stdout)),
        Err(_) => (Vec::new(), false),
    }
}

/// Minecraft for Windows as it is installed now, for a version switch.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InstalledGame {
    /// The game version ("1.26.65.1").
    pub version: String,
    /// Registered from a folder (a UWP version someone switched to) instead of installed from a package.
    pub development: bool,
    pub location: Option<String>,
}

pub(crate) async fn installed_release() -> Option<InstalledGame> {
    let (packages, _) = windows_state().await;
    let dir = versions_dir();
    packages.into_iter().find(|p| p.name == Edition::Release.package()).map(|p| InstalledGame { version: version_of(&p, &dir), development: p.development, location: p.location })
}

/// Where ResinCraft unpacks the UWP versions it switches to.
pub(crate) fn versions_dir() -> PathBuf {
    crate::core::paths::root().join("bedrock-versions")
}

/// The GDK version ResinCraft last switched Minecraft for Windows to (`bedrock-switch.json`). A GDK version is
/// installed by Windows like the Store's own copy, so only this note tells them apart.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SwitchRecord {
    pub version: String,
}

pub(crate) fn switch_record_file() -> PathBuf {
    crate::core::paths::root().join("bedrock-switch.json")
}

/// Whether the installed copy is one ResinCraft switched to: a UWP version registered from ResinCraft's own
/// folder, or the GDK version the note names (once the Store updates it, the versions differ and it no longer is).
fn is_switched(installed: &Installed, game: &str, versions_dir: &Path, record: Option<&SwitchRecord>) -> bool {
    unpacked_version(installed, versions_dir).is_some() || record.is_some_and(|r| super::bedrock_versions::same_version(&r.version, game))
}

/// The game version of a UWP copy registered from ResinCraft's folder: its folder there is named after it (an old
/// version's app package version doesn't read back as the game's). None for any other copy.
fn unpacked_version(installed: &Installed, versions_dir: &Path) -> Option<String> {
    if !installed.development {
        return None;
    }
    let tidy = |p: &str| p.replace('/', "\\").trim_end_matches('\\').to_string();
    let (location, dir) = (tidy(installed.location.as_deref()?), tidy(&versions_dir.to_string_lossy()));
    let inside = location.len() > dir.len() && location.is_char_boundary(dir.len()) && location[..dir.len()].eq_ignore_ascii_case(&dir);
    let folder = location.get(dir.len()..).filter(|_| inside)?.strip_prefix('\\')?.split('\\').next()?;
    super::bedrock_versions::parse_version(folder).map(|_| folder.to_string())
}

/// The game version of an installed copy.
fn version_of(installed: &Installed, versions_dir: &Path) -> String {
    unpacked_version(installed, versions_dir).unwrap_or_else(|| game_version(&installed.version))
}

/// Every `minecraftWorlds` folder of an edition (one per signed-in player, plus the older shared one).
/// Worlds kept inside the app package (versions before Minecraft moved them to %APPDATA%): uninstalling deletes
/// these, so they're copied out first.
fn package_world_roots(edition: Edition) -> Vec<PathBuf> {
    let Some(package_dir) = edition.data_dirs().into_iter().find(|d| d.components().any(|c| c.as_os_str() == "Packages")) else { return vec![] };
    world_roots(edition).into_iter().filter(|r| r.starts_with(&package_dir)).collect()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Uninstalled {
    /// Where worlds from inside the app package were copied before it went (none when there were none).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup: Option<String>,
    pub worlds: usize,
}

/// Removes the game for this Windows account, as Settings → Apps → Uninstall does. Worlds in %APPDATA% stay; worlds
/// inside the app package (older versions) are copied to ResinCraft's bedrock-backups folder first.
async fn uninstall(edition: Edition) -> Result<Uninstalled> {
    if !cfg!(windows) {
        bail!("Bedrock Edition only exists on Windows");
    }
    let title = crate::tr!("bedrock.uninstalling", "Uninstalling {name}", name = edition.name());
    crate::core::events::Task::run(title, Some(format!("bedrock:{}", edition.key())), move |task| async move {
        // 1. Worlds that would go with the package.
        let roots = package_world_roots(edition);
        let worlds: usize = roots.iter().map(|r| std::fs::read_dir(r).map(|d| d.flatten().filter(|e| e.path().is_dir()).count()).unwrap_or(0)).sum();
        let mut backup = None;
        if worlds > 0 {
            task.update(crate::tr!("bedrock.backingUp", "Backing up {count} worlds…", count = worlds), None);
            let dest = crate::core::paths::root()
                .join("bedrock-backups")
                .join(format!("{}-{}", edition.key(), chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")));
            let (from, to) = (roots.clone(), dest.clone());
            tokio::task::spawn_blocking(move || -> Result<()> {
                for (i, r) in from.iter().enumerate() {
                    let target = if from.len() > 1 { to.join(i.to_string()) } else { to.clone() };
                    crate::core::instances::copy_dir(r, &target, &[])?;
                }
                Ok(())
            })
            .await??;
            backup = Some(dest.to_string_lossy().into_owned());
        }

        // 2. The app itself.
        task.update(crate::tr!("bedrock.removing", "Removing {name}…", name = edition.name()), None);
        let script = format!(
            "$ErrorActionPreference = 'Stop'; $p = Get-AppxPackage -Name '{}'; if (-not $p) {{ 'missing'; exit }}; try {{ $p | Remove-AppxPackage; 'ok' }} catch {{ 'error ' + ('0x{{0:X8}}' -f $_.Exception.HResult) + ' ' + $_.Exception.Message }}",
            edition.package()
        );
        let out = crate::game::java::hidden_command("powershell.exe").args(["-NoProfile", "-NonInteractive", "-Command", &script]).output().await?;
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        match uninstall_result(&text) {
            Ok(()) => Ok(Uninstalled { backup, worlds }),
            Err(e) => Err(e),
        }
    })
    .await
}

/// Reads the uninstall script's last line.
fn uninstall_result(out: &str) -> Result<()> {
    let last = out.lines().last().unwrap_or_default().trim();
    if last == "ok" || last == "missing" {
        return Ok(());
    }
    // 0x80073D02: the app is open.
    if last.contains("0x80073D02") {
        crate::bail_tr!("bedrock.closeFirst", "Close Minecraft first, then uninstall it.");
    }
    let detail = last.strip_prefix("error ").unwrap_or(last).to_string();
    crate::bail_tr!("bedrock.uninstallFailed", "Windows couldn't uninstall it: {error}", error = detail)
}

pub(crate) fn world_roots(edition: Edition) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for base in edition.data_dirs() {
        let users = base.join("Users");
        if let Ok(rd) = std::fs::read_dir(&users) {
            for e in rd.flatten() {
                let p = e.path().join("games").join("com.mojang").join("minecraftWorlds");
                if p.is_dir() {
                    roots.push(p);
                }
            }
        }
        let p = base.join("games").join("com.mojang").join("minecraftWorlds");
        if p.is_dir() {
            roots.push(p);
        }
    }
    roots
}

fn modified_ms(p: &Path) -> i64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn worlds(edition: Edition) -> Vec<BedrockWorld> {
    let mut out: Vec<BedrockWorld> = Vec::new();
    for root in world_roots(edition) {
        let Ok(rd) = std::fs::read_dir(&root) else { continue };
        for e in rd.flatten() {
            let dir = e.path();
            if !dir.join("level.dat").is_file() {
                continue;
            }
            let id = e.file_name().to_string_lossy().into_owned();
            let name = std::fs::read_to_string(dir.join("levelname.txt")).map(|s| s.trim().to_string()).ok().filter(|s| !s.is_empty()).unwrap_or_else(|| id.clone());
            // The world's files change when it's played; the folder's own time doesn't always.
            let modified = modified_ms(&dir.join("level.dat")).max(modified_ms(&dir));
            if !out.iter().any(|w| w.id == id) {
                out.push(BedrockWorld { id, name, modified });
            }
        }
    }
    out.sort_by_key(|w| std::cmp::Reverse(w.modified));
    out
}

pub async fn status() -> Result<BedrockStatus> {
    if !cfg!(windows) {
        return Ok(BedrockStatus { supported: false, editions: Vec::new(), gaming_services: false, developer_mode: false });
    }
    let (packages, developer_mode) = windows_state().await;
    let record: Option<SwitchRecord> = crate::core::fsutil::read_json(&switch_record_file()).await;
    let versions_dir = versions_dir();
    let editions = Edition::ALL
        .iter()
        .map(|&edition| {
            let package = packages.iter().find(|p| p.name == edition.package());
            let version = package.map(|p| version_of(p, &versions_dir));
            // Only Minecraft for Windows is switched; Preview is always the Store's.
            let switched = edition == Edition::Release && package.zip(version.as_deref()).is_some_and(|(p, v)| is_switched(p, v, &versions_dir, record.as_ref()));
            BedrockEdition { edition, installed: version.is_some(), format: version.as_deref().map(super::bedrock_switch::format_of), switched, version, worlds: worlds(edition) }
        })
        .collect();
    let gaming_services = packages.iter().any(|p| p.name == GAMING_SERVICES);
    Ok(BedrockStatus { supported: true, editions, gaming_services, developer_mode })
}

/// A world's folder, found again by its id (never a path from the UI).
fn world_dir(edition: Edition, id: &str) -> Option<PathBuf> {
    if id.is_empty() || id.contains(['/', '\\']) || id == "." || id == ".." {
        return None;
    }
    world_roots(edition).into_iter().map(|r| r.join(id)).find(|p| p.join("level.dat").is_file())
}

/// The worlds folder the installed game reads, for putting worlds back: `dirs` are the edition's data folders.
/// A GDK version keeps a folder per signed-in player under %APPDATA% (the one used last is taken; "Shared" holds
/// no worlds); a UWP version uses its app data folder, which only Windows may create (a folder made here would
/// lack the app's access rights). None when the game hasn't made its folder yet.
fn restore_target(format: Format, dirs: &[PathBuf]) -> Option<PathBuf> {
    let in_package = |d: &&PathBuf| d.components().any(|c| c.as_os_str() == "Packages");
    let worlds = |base: &Path| base.join("games").join("com.mojang").join("minecraftWorlds");
    match format {
        Format::Uwp => dirs.iter().find(in_package).filter(|d| d.is_dir()).map(|d| worlds(d)),
        Format::Gdk => {
            let users = dirs.iter().find(|d| !in_package(d))?.join("Users");
            let mut players: Vec<PathBuf> = std::fs::read_dir(users).ok()?.flatten().map(|e| e.path()).filter(|p| p.join("games").join("com.mojang").is_dir()).collect();
            if players.len() > 1 {
                players.retain(|p| p.file_name().is_some_and(|n| !n.eq_ignore_ascii_case("Shared")));
            }
            players.into_iter().max_by_key(|p| modified_ms(&p.join("games").join("com.mojang"))).map(|p| worlds(&p))
        }
    }
}

fn task_owner(edition: Edition) -> Option<String> {
    Some(format!("bedrock:{}", edition.key()))
}

/// Backs up every world of an edition now (a task on the edition's card). None when it has no worlds.
async fn back_up(edition: Edition) -> Result<Option<super::bedrock_backup::BackupInfo>> {
    if tokio::task::spawn_blocking(super::bedrock_switch::game_running).await? {
        crate::bail_tr!("bedrock.backup.closeFirst", "Close Minecraft first: worlds can't be copied safely while it is open.");
    }
    let version = status().await.ok().and_then(|s| s.editions.into_iter().find(|e| e.edition == edition).and_then(|e| e.version));
    let title = crate::tr!("bedrock.backup.task", "Backing up {name} worlds", name = edition.name());
    crate::core::events::Task::run(title, task_owner(edition), move |task| async move { super::bedrock_backup::run(edition, version, &task).await }).await
}

/// Copies a backup's worlds into the worlds folder the installed game reads (a task on the edition's card).
/// Worlds that are there already stay as they are.
async fn restore(id: &str) -> Result<super::bedrock_backup::Restored> {
    let dir = super::bedrock_backup::backup_dir(id)?;
    let folder = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let Some(backup) = super::bedrock_backup::list().into_iter().find(|b| b.id == folder) else {
        crate::bail_tr!("bedrock.backup.missing", "That backup isn't there any more.");
    };
    let edition = backup.edition;
    let installed = status().await?.editions.into_iter().find(|e| e.edition == edition).and_then(|e| e.format);
    let Some(format) = installed else {
        crate::bail_tr!("bedrock.restore.notInstalled", "Install {name} first, then restore the worlds.", name = edition.name());
    };
    if tokio::task::spawn_blocking(super::bedrock_switch::game_running).await? {
        crate::bail_tr!("bedrock.backup.closeFirst", "Close Minecraft first: worlds can't be copied safely while it is open.");
    }
    let Some(target) = restore_target(format, &edition.data_dirs()) else {
        crate::bail_tr!("bedrock.restore.playFirst", "Start {name} once so it makes its worlds folder, then restore.", name = edition.name());
    };
    let title = crate::tr!("bedrock.restore.task", "Restoring {name} worlds", name = edition.name());
    crate::core::events::Task::run(title, task_owner(edition), move |task| async move { super::bedrock_backup::run_restore(&folder, target, &task).await }).await
}

// ---------- Which Xbox account Bedrock signs in with ----------
//
// Minecraft for Windows signs in as the Xbox account the Xbox app has for this Windows user; Microsoft offers no way
// for another program to choose it (the game asks Gaming Services for the "default user"). ResinCraft can only see
// which account that is and compare it with its own accounts. Windows keeps it, written by the Xbox app when it
// signs in, under HKCU\Software\Microsoft\XboxLive (Xuid, Gamertag). Microsoft doesn't document that key, so a
// missing or odd one reads as "unknown", never as an error.

/// The Xbox account the Xbox app is signed in with.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XboxUser {
    pub xuid: String,
    pub gamertag: String,
}

/// Reads `reg query HKCU\Software\Microsoft\XboxLive`: lines like `    Xuid    REG_SZ    2535466269577823`.
fn parse_xbox_live_key(text: &str) -> Option<XboxUser> {
    let value = |name: &str| -> Option<String> {
        text.lines().find_map(|line| {
            let mut parts = line.trim().splitn(3, |c: char| c.is_whitespace());
            let (key, _) = (parts.next()?, parts.next()?);
            if !key.eq_ignore_ascii_case(name) {
                return None;
            }
            // `Name    REG_SZ    value`: the value is what follows the type, spaces inside it kept.
            let rest = line.trim()[key.len()..].trim_start();
            let rest = rest.strip_prefix("REG_SZ")?.trim();
            (!rest.is_empty()).then(|| rest.to_string())
        })
    };
    let xuid = value("Xuid").filter(|x| x.len() >= 10 && x.chars().all(|c| c.is_ascii_digit()))?;
    let gamertag = value("UniqueModernGamertag").or_else(|| value("ModernGamertag")).or_else(|| value("Gamertag")).unwrap_or_default();
    Some(XboxUser { xuid, gamertag })
}

async fn xbox_user() -> Option<XboxUser> {
    if !cfg!(windows) {
        return None;
    }
    let out = crate::game::java::hidden_command("reg").args(["query", r"HKCU\Software\Microsoft\XboxLive"]).output().await.ok()?;
    parse_xbox_live_key(&String::from_utf8_lossy(&out.stdout))
}

/// The Xbox app's account next to ResinCraft's accounts, for Bedrock's Play.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XboxSignIn {
    /// Who the Xbox app is signed in as; none when Windows doesn't say.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<XboxUser>,
    /// The ResinCraft account that is that Xbox account, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    /// ResinCraft's Microsoft accounts whose Xbox account isn't known (signed in before it was stored): they can't
    /// be matched.
    pub unknown: Vec<String>,
    /// ResinCraft's accounts that own Minecraft for Windows, and those that don't (unknown ones are in neither).
    pub owners: Vec<String>,
    pub not_owners: Vec<String>,
}

/// Matches the Xbox app's account to ResinCraft's: by Xbox user ID, or for a Bedrock account without one, by its
/// gamertag (it is named after it).
fn match_xbox(user: Option<XboxUser>, accounts: &[super::accounts::XboxIdentity]) -> XboxSignIn {
    let account_id = user.as_ref().and_then(|u| {
        accounts
            .iter()
            .find(|a| a.xuid.as_deref() == Some(u.xuid.as_str()))
            .or_else(|| accounts.iter().find(|a| a.xuid.is_none() && a.kind == "bedrock" && !u.gamertag.is_empty() && a.username.eq_ignore_ascii_case(&u.gamertag)))
            .map(|a| a.id.clone())
    });
    let unknown = accounts.iter().filter(|a| a.xuid.is_none() && a.kind != "bedrock").map(|a| a.id.clone()).collect();
    let owners = accounts.iter().filter(|a| a.owns_bedrock == Some(true)).map(|a| a.id.clone()).collect();
    let not_owners = accounts.iter().filter(|a| a.owns_bedrock == Some(false)).map(|a| a.id.clone()).collect();
    XboxSignIn { user, account_id, unknown, owners, not_owners }
}

pub async fn handle(channel: &str, args: Args) -> Result<Value> {
    match channel {
        "bedrock:status" => json(status().await?),
        "bedrock:play" => {
            shell::open_app_link(args.get::<Edition>(0)?.protocol())?;
            done()
        }
        // Install, update or repair in a task: [edition, "install" | "update" | "repair"] → "done" | "upToDate".
        "bedrock:install" => json(super::bedrock_store::run(args.get(0)?, args.get(1)?).await?),
        // Removes the game from this PC (worlds inside its app package are backed up first) → Uninstalled.
        "bedrock:uninstall" => json(uninstall(args.get(0)?).await?),
        // The Store's own install window: the way out when installing from here fails.
        "bedrock:get" => {
            shell::open_app_link(&args.get::<Edition>(0)?.store())?;
            done()
        }
        // Bedrock signs in as the Xbox account Windows uses (the Xbox app's), never the launcher's: this opens the Xbox
        // app to switch it, or its Store page when it isn't installed.
        "bedrock:xboxAccount" => {
            if shell::open_app_link("msxbox://").is_err() {
                shell::open_app_link(&store_page(XBOX_APP_ID))?;
            }
            done()
        }
        // Who the Xbox app is signed in as (Bedrock plays as that account), matched to ResinCraft's accounts.
        "bedrock:xboxUser" => json(match_xbox(xbox_user().await, &super::accounts::xbox_identities().await)),
        "bedrock:gamingServices" => {
            shell::open_app_link(&store_page(GAMING_SERVICES_ID))?;
            done()
        }
        // The Store's Downloads and updates page, which updates every Store app (Minecraft included).
        "bedrock:update" => {
            shell::open_app_link("ms-windows-store://downloadsandupdates")?;
            done()
        }
        // The older versions Minecraft for Windows can be switched to, the newest first → VersionInfo[].
        "bedrock:versions" => json(super::bedrock_versions::all().await.iter().map(super::bedrock_versions::VersionInfo::from).collect::<Vec<_>>()),
        // What a switch to a version needs and what is in its way, without changing anything: [version] → Plan.
        "bedrock:switchPlan" => json(super::bedrock_switch::plan_for(&args.get::<String>(0)?).await?),
        // Switches Minecraft for Windows to a version (backup first; the Store version is put back if Windows
        // refuses the other one): [version] → Switched.
        "bedrock:switch" => json(super::bedrock_switch::run(&args.get::<String>(0)?).await?),
        // Puts the Store's current version back after a switch → Switched.
        "bedrock:backToStore" => json(super::bedrock_switch::back_to_store().await?),
        // Backs up an edition's worlds now: [edition] → BackupInfo | null (no worlds).
        "bedrock:backup" => json(back_up(args.get(0)?).await?),
        "bedrock:backups" => json(tokio::task::spawn_blocking(super::bedrock_backup::list).await?),
        // Copies a backup's worlds back: [backup id] → Restored.
        "bedrock:restore" => json(restore(&args.get::<String>(0)?).await?),
        "bedrock:openBackup" => {
            shell::open_path(&super::bedrock_backup::backup_dir(&args.get::<String>(0)?)?)?;
            done()
        }
        // Windows' own page with the Developer Mode switch. ResinCraft never changes the setting itself.
        "bedrock:developerSettings" => {
            crate::game::java::hidden_command("explorer.exe").arg("ms-settings:developers").spawn()?;
            done()
        }
        "bedrock:openFolder" => {
            let edition: Edition = args.get(0)?;
            let world: Option<String> = args.get(1)?;
            let dir = match world {
                Some(id) => world_dir(edition, &id),
                None => world_roots(edition).into_iter().next().or_else(|| edition.data_dirs().into_iter().find(|d| d.is_dir())),
            };
            match dir {
                Some(d) => shell::open_path(&d)?,
                None => bail!("Minecraft hasn't made that folder yet. Play once first."),
            }
            done()
        }
        _ => bail!("Unknown request: {channel}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_xbox_apps_account() {
        // As on a real PC (2026-11-22), with the e-mail line left out of the test.
        let key = "\r\nHKEY_CURRENT_USER\\Software\\Microsoft\\XboxLive\r\n    HasFiredSignInEvent    REG_DWORD    0x1\r\n    AccountId    REG_SZ    00037FFE41F1DBC0\r\n    Xuid    REG_SZ    2535466269577823\r\n    Gamertag    REG_SZ    Old Tag\r\n    UniqueModernGamertag    REG_SZ    Puirkle Gamer#123\r\n\r\nHKEY_CURRENT_USER\\Software\\Microsoft\\XboxLive\\TokenBroker\r\n";
        assert_eq!(parse_xbox_live_key(key), Some(XboxUser { xuid: "2535466269577823".into(), gamertag: "Puirkle Gamer#123".into() }));
        let plain = "    Xuid    REG_SZ    2535466269577823\n    Gamertag    REG_SZ    PuirkleGamer\n";
        assert_eq!(parse_xbox_live_key(plain).unwrap().gamertag, "PuirkleGamer");
        // Signed out, the key missing, or something odd: unknown.
        assert_eq!(parse_xbox_live_key("ERROR: The system was unable to find the specified registry key or value."), None);
        assert_eq!(parse_xbox_live_key("    Xuid    REG_SZ    \n"), None);
        assert_eq!(parse_xbox_live_key("    Xuid    REG_SZ    not-a-number\n"), None);
        assert_eq!(parse_xbox_live_key("    XuidOld    REG_SZ    2535466269577823\n"), None);
        assert_eq!(parse_xbox_live_key(""), None);
    }

    /// This PC's Xbox app: `cargo test --lib real_xbox_user -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "reads this PC's Xbox sign-in"]
    async fn real_xbox_user() {
        let user = xbox_user().await;
        println!("gamertag: {:?}, xuid known: {}", user.as_ref().map(|u| &u.gamertag), user.is_some());
    }

    #[test]
    fn matches_the_xbox_account_to_ours() {
        use super::super::accounts::XboxIdentity;
        let acc = |id: &str, kind: &str, name: &str, xuid: Option<&str>| XboxIdentity {
            id: id.into(),
            kind: kind.into(),
            username: name.into(),
            xuid: xuid.map(str::to_string),
            owns_bedrock: match id {
                "a" => Some(true),
                "b" => Some(false),
                _ => None,
            },
        };
        let ours = vec![acc("a", "msa", "Steve", Some("111")), acc("b", "bedrock", "PuirkleGamer", None), acc("c", "msa", "Alex", None)];
        let user = |xuid: &str, tag: &str| Some(XboxUser { xuid: xuid.into(), gamertag: tag.into() });
        assert_eq!(match_xbox(user("111", "Whoever"), &ours).account_id.as_deref(), Some("a"));
        assert_eq!(match_xbox(user("222", "puirklegamer"), &ours).account_id.as_deref(), Some("b"), "a Bedrock account is named after its gamertag");
        assert_eq!(match_xbox(user("333", "Alex"), &ours).account_id, None, "a Java name is not a gamertag");
        let none = match_xbox(None, &ours);
        assert_eq!((none.account_id, none.unknown), (None, vec!["c".to_string()]));
        let v = serde_json::to_value(match_xbox(user("111", "T"), &ours)).unwrap();
        assert_eq!(v, serde_json::json!({"user": {"xuid": "111", "gamertag": "T"}, "accountId": "a", "unknown": ["c"], "owners": ["a"], "notOwners": ["b"]}));
    }

    #[test]
    fn reads_uninstall_results() {
        assert!(uninstall_result("ok").is_ok());
        assert!(uninstall_result("missing").is_ok(), "already gone counts as done");
        let busy = uninstall_result("error 0x80073D02 The package could not be installed because resources it modifies are currently in use.").unwrap_err();
        assert!(busy.to_string().contains("Close Minecraft first"));
        let other = uninstall_result("error 0x80070005 Access is denied.").unwrap_err();
        assert!(other.to_string().contains("Access is denied"));
    }

    #[test]
    fn reads_windows_state() {
        let two = r#"{"developerMode":true,"packages":[{"Name":"Microsoft.MinecraftUWP","Version":"1.26.6501.0","IsDevelopmentMode":false,"InstallLocation":"C:\\XboxGames\\Minecraft for Windows\\Content"},{"Name":"Microsoft.GamingServices","Version":"33.110.6002.0","IsDevelopmentMode":false,"InstallLocation":null}]}"#;
        let (packages, developer) = parse_windows_state(two);
        assert!(developer);
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0], Installed { name: "Microsoft.MinecraftUWP".into(), version: "1.26.6501.0".into(), development: false, location: Some("C:\\XboxGames\\Minecraft for Windows\\Content".into()) });
        assert_eq!(packages[1].location, None);
        // One package as an object, nothing installed, and no answer at all.
        let one = r#"{"packages":{"Name":"Microsoft.MinecraftUWP","Version":"1.20.8101.0","IsDevelopmentMode":true,"InstallLocation":"D:\\x"},"developerMode":false}"#;
        let (packages, developer) = parse_windows_state(one);
        assert!(!developer && packages.len() == 1 && packages[0].development);
        assert_eq!(parse_windows_state(r#"{"packages":[],"developerMode":true}"#), (vec![], true));
        assert_eq!(parse_windows_state(r#"{"packages":null,"developerMode":false}"#), (vec![], false));
        assert_eq!(parse_windows_state(""), (vec![], false));
        assert_eq!(parse_windows_state("WARNING: something"), (vec![], false));
    }

    #[test]
    fn the_state_script_passes_through_the_command_line() {
        // Rust quotes double quotes as \" on Windows, which PowerShell's -Command doesn't read back the same way.
        assert!(!WINDOWS_STATE_SCRIPT.contains('"'));
        assert!(WINDOWS_STATE_SCRIPT.contains("HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\AppModelUnlock"));
        // It only reads.
        for word in ["Set-ItemProperty", "New-ItemProperty", "Remove-", "Add-AppxPackage", "reg add"] {
            assert!(!WINDOWS_STATE_SCRIPT.contains(word), "{word}");
        }
    }

    #[test]
    fn tells_a_switched_copy_from_the_stores() {
        let dir = Path::new("D:\\ResinCraft\\bedrock-versions");
        let package = |development: bool, location: &str| Installed { name: "Microsoft.MinecraftUWP".into(), version: String::new(), development, location: Some(location.into()) };
        let note = SwitchRecord { version: "1.21.130.4".into() };
        // A UWP version registered from ResinCraft's folder.
        assert!(is_switched(&package(true, "D:\\ResinCraft\\bedrock-versions\\1.20.81.1"), "1.20.81.1", dir, None));
        assert!(is_switched(&package(true, "d:/resincraft/bedrock-versions/1.20.81.1/"), "1.20.81.1", dir, None));
        // Registered from somewhere else (another launcher's folder), or a folder that only starts the same.
        assert!(!is_switched(&package(true, "D:\\MCLauncher\\Minecraft-1.20.81.1"), "1.20.81.1", dir, None));
        assert!(!is_switched(&package(true, "D:\\ResinCraft\\bedrock-versions-old\\1.20.81.1"), "1.20.81.1", dir, None));
        // A GDK version: only the note says so, and only while the version is still that one.
        let store = package(false, "C:\\XboxGames\\Minecraft for Windows\\Content");
        assert!(is_switched(&store, "1.21.130.4", dir, Some(&note)));
        assert!(!is_switched(&store, "1.26.65.1", dir, Some(&note)), "the Store updated it since");
        assert!(!is_switched(&store, "1.21.130.4", dir, None));
        // An old version's package version isn't its game version: the folder's name is.
        let old = Installed { version: "0.1400.1.0".into(), ..package(true, "D:\\ResinCraft\\bedrock-versions\\0.14.0.1") };
        assert_eq!(version_of(&old, dir), "0.14.0.1");
        assert_eq!(version_of(&Installed { version: "1.26.6501.0".into(), ..store.clone() }, dir), "1.26.65.1");
        assert_eq!(unpacked_version(&package(true, "D:\\ResinCraft\\bedrock-versions\\not-a-version"), dir), None);
        assert_eq!(unpacked_version(&package(true, "D:\\ResinCraft\\bedrock-versions"), dir), None);
    }

    #[test]
    fn finds_where_restored_worlds_go() {
        let base = std::env::temp_dir().join(format!("resincraft-bedrock-target-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let roaming = base.join("Roaming").join("Minecraft Bedrock");
        let local = base.join("Local").join("Packages").join("Microsoft.MinecraftUWP_8wekyb3d8bbwe").join("LocalState");
        let dirs = vec![roaming.clone(), local.clone()];
        // Nothing made yet: no target for either format.
        assert_eq!(restore_target(Format::Gdk, &dirs), None);
        assert_eq!(restore_target(Format::Uwp, &dirs), None);
        // UWP: the app data folder Windows made, with the worlds folder below it.
        std::fs::create_dir_all(&local).unwrap();
        assert_eq!(restore_target(Format::Uwp, &dirs), Some(local.join("games").join("com.mojang").join("minecraftWorlds")));
        // GDK: the signed-in player's folder, not "Shared".
        let player = |name: &str| roaming.join("Users").join(name).join("games").join("com.mojang");
        std::fs::create_dir_all(player("Shared")).unwrap();
        assert_eq!(restore_target(Format::Gdk, &dirs), Some(player("Shared").join("minecraftWorlds")), "the only one there is");
        std::fs::create_dir_all(player("2535412345678901")).unwrap();
        std::fs::create_dir_all(roaming.join("Users").join("empty")).unwrap();
        assert_eq!(restore_target(Format::Gdk, &dirs), Some(player("2535412345678901").join("minecraftWorlds")));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn package_versions_read_as_game_versions() {
        assert_eq!(game_version("1.26.5203.0"), "1.26.52.3");
        assert_eq!(game_version("1.21.12000.0"), "1.21.120");
        assert_eq!(game_version("1.20.8101.0"), "1.20.81.1");
    }
}
