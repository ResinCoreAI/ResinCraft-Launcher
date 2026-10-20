//! `bedrock:*`: Minecraft Bedrock Edition (Minecraft for Windows, and Minecraft Preview) as installed from the
//! Microsoft Store or the Xbox app. ResinCraft finds the installed version and the worlds, opens the game, and
//! opens the Store's install window to get it or its updates page to update it. Only the Store (or the Xbox app)
//! may install these paid apps (winget's Store source doesn't carry them either), so ResinCraft can't download
//! them itself or switch them to another version: Preview is the way to try the next update.

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
    /// The Microsoft Store's compact install window for it (the "mini" product page).
    fn store(self) -> &'static str {
        match self {
            Edition::Release => "ms-windows-store://pdp/?productid=9NBLGGH2JHXJ&mode=mini",
            Edition::Preview => "ms-windows-store://pdp/?productid=9P5X4QVLC2XR&mode=mini",
        }
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

#[derive(Debug, Clone, Serialize)]
pub struct BedrockStatus {
    /// Bedrock Edition only exists as an app on Windows.
    pub supported: bool,
    pub editions: Vec<BedrockEdition>,
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
    let script = "Get-AppxPackage -Name 'Microsoft.Minecraft*' | Select-Object Name, Version | ConvertTo-Json -Compress";
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
        return Ok(BedrockStatus { supported: false, editions: Vec::new() });
    }
    let packages = installed_packages().await;
    let editions = Edition::ALL
        .iter()
        .map(|&edition| {
            let version = packages.iter().find(|(n, _)| n == edition.package()).map(|(_, v)| game_version(v));
            BedrockEdition { edition, installed: version.is_some(), version, worlds: worlds(edition) }
        })
        .collect();
    Ok(BedrockStatus { supported: true, editions })
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
            shell::open_url(args.get::<Edition>(0)?.protocol())?;
            done()
        }
        "bedrock:get" => {
            shell::open_url(args.get::<Edition>(0)?.store())?;
            done()
        }
        // The Store's Downloads and updates page, which updates every Store app (Minecraft included).
        "bedrock:update" => {
            shell::open_url("ms-windows-store://downloadsandupdates")?;
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
    fn package_versions_read_as_game_versions() {
        assert_eq!(game_version("1.26.5203.0"), "1.26.52.3");
        assert_eq!(game_version("1.21.12000.0"), "1.21.120");
        assert_eq!(game_version("1.20.8101.0"), "1.20.81.1");
    }
}
