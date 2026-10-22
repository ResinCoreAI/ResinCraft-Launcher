//! `bedrock:*`: Minecraft Bedrock Edition (Minecraft for Windows, and Minecraft Preview) as installed from the
//! Microsoft Store or the Xbox app. ResinCraft finds the installed version and the worlds, opens the game, and
//! has the Store's install service install, update or repair it in a task (`bedrock_store`), with the Store's own
//! pages as the way out when that fails. Only the Store may install these apps, so ResinCraft never downloads them
//! itself or switches them to another version: Preview is the way to try the next update.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    fn key(self) -> &'static str {
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
    fn data_dirs(self) -> Vec<PathBuf> {
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
}

/// Gaming Services: the Xbox service that installs and runs Store games like Bedrock.
const GAMING_SERVICES: &str = "Microsoft.GamingServices";
const GAMING_SERVICES_ID: &str = "9MWPM2CQNLHN";

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

/// The installed app packages' versions, from PowerShell's Get-AppxPackage.
async fn installed_packages() -> Vec<(String, String)> {
    if !cfg!(windows) {
        return Vec::new();
    }
    // -Name takes one name (with wildcards); the editions are picked out below.
    let script = "@(Get-AppxPackage -Name 'Microsoft.Minecraft*') + @(Get-AppxPackage -Name 'Microsoft.GamingServices') | Select-Object Name, Version | ConvertTo-Json -Compress";
    let out = crate::game::java::hidden_command("powershell").args(["-NoProfile", "-NonInteractive", "-Command", script]).output().await;
    let Ok(out) = out else { return Vec::new() };
    let text = String::from_utf8_lossy(&out.stdout);
    let v: Value = serde_json::from_str(text.trim()).unwrap_or(Value::Null);
    // One package comes back as an object, several as a list.
    let list = match v {
        Value::Array(a) => a,
        Value::Object(_) => vec![v],
        _ => Vec::new(),
    };
    list.iter()
        .filter_map(|p| Some((p.get("Name")?.as_str()?.to_string(), p.get("Version")?.as_str()?.to_string())))
        .collect()
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

fn world_roots(edition: Edition) -> Vec<PathBuf> {
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
        return Ok(BedrockStatus { supported: false, editions: Vec::new(), gaming_services: false });
    }
    let packages = installed_packages().await;
    let editions = Edition::ALL
        .iter()
        .map(|&edition| {
            let version = packages.iter().find(|(n, _)| n == edition.package()).map(|(_, v)| game_version(v));
            BedrockEdition { edition, installed: version.is_some(), version, worlds: worlds(edition) }
        })
        .collect();
    let gaming_services = packages.iter().any(|(n, _)| n == GAMING_SERVICES);
    Ok(BedrockStatus { supported: true, editions, gaming_services })
}

/// A world's folder, found again by its id (never a path from the UI).
fn world_dir(edition: Edition, id: &str) -> Option<PathBuf> {
    if id.is_empty() || id.contains(['/', '\\']) || id == "." || id == ".." {
        return None;
    }
    world_roots(edition).into_iter().map(|r| r.join(id)).find(|p| p.join("level.dat").is_file())
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
        "bedrock:gamingServices" => {
            shell::open_app_link(&store_page(GAMING_SERVICES_ID))?;
            done()
        }
        // The Store's Downloads and updates page, which updates every Store app (Minecraft included).
        "bedrock:update" => {
            shell::open_app_link("ms-windows-store://downloadsandupdates")?;
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
    fn reads_uninstall_results() {
        assert!(uninstall_result("ok").is_ok());
        assert!(uninstall_result("missing").is_ok(), "already gone counts as done");
        let busy = uninstall_result("error 0x80073D02 The package could not be installed because resources it modifies are currently in use.").unwrap_err();
        assert!(busy.to_string().contains("Close Minecraft first"));
        let other = uninstall_result("error 0x80070005 Access is denied.").unwrap_err();
        assert!(other.to_string().contains("Access is denied"));
    }

    #[test]
    fn package_versions_read_as_game_versions() {
        assert_eq!(game_version("1.26.5203.0"), "1.26.52.3");
        assert_eq!(game_version("1.21.12000.0"), "1.21.120");
        assert_eq!(game_version("1.20.8101.0"), "1.20.81.1");
    }
}
