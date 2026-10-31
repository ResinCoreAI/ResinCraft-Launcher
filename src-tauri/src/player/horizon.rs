//! Horizon (playhorizon.dev), a Rust rewrite of the Minecraft: Java Edition client. Its own launcher picks a Horizon
//! version, prepares it from the player's Minecraft files and starts it, so this only downloads that launcher from
//! Horizon's GitHub releases into the data folder and opens it. It isn't an instance: the game runs outside ResinCraft.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use serde_json::Value;

use crate::content::github;
use crate::core::events::Task;
use crate::core::net::{download_all, DownloadItem, Hash};
use crate::core::paths;
use crate::core::router::{done, json};
use crate::core::Args;
use crate::server::installers::{make_executable, unpack_tar, unpack_zip};

const REPO: &str = "horizon-rust/horizon";
const TAG_PREFIX: &str = "launcher@";
const VERSION_FILE: &str = "version.txt";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    /// Horizon publishes a launcher for this computer.
    supported: bool,
    /// The Minecraft version Horizon plays now; a new Horizon instance gets it.
    minecraft: Option<String>,
    installed: Option<String>,
    /// None when the release list can't be read.
    latest: Option<String>,
}

fn folder() -> PathBuf {
    paths::root().join("horizon-launcher")
}

fn asset(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("windows", "x86_64") => Some("horizon-launcher-windows-x64.zip"),
        ("linux", "x86_64") => Some("horizon-launcher-linux-x86_64.tar.gz"),
        ("macos", "aarch64") => Some("horizon-launcher-macos-arm64.zip"),
        _ => None,
    }
}

fn this_computer() -> Option<&'static str> {
    asset(std::env::consts::OS, std::env::consts::ARCH)
}

/// "launcher@0.2.30" → "0.2.30"; the game's own releases ("v0.2.97") aren't the launcher's.
fn launcher_version(tag: &str) -> Option<&str> {
    tag.strip_prefix(TAG_PREFIX)
}

async fn latest() -> Result<github::Release> {
    let list = github::all_releases(REPO).await?;
    list.into_iter().find(|r| !r.prerelease && launcher_version(&r.tag_name).is_some()).ok_or_else(|| anyhow!("Horizon has no launcher release"))
}

async fn status() -> Status {
    let installed = tokio::fs::read_to_string(folder().join(VERSION_FILE)).await.ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    let latest = latest().await.ok().and_then(|r| launcher_version(&r.tag_name).map(str::to_string));
    Status { supported: this_computer().is_some(), minecraft: crate::game::horizon::minecraft().await.ok(), installed, latest }
}

/// The newest Horizon launcher, replacing the one there. Returns its version.
async fn install() -> Result<String> {
    let name = this_computer().ok_or_else(|| anyhow!("Horizon has no launcher for this computer"))?;
    Task::run("Installing Horizon", Some("horizon".to_string()), |task| async move {
        task.update("Finding the Horizon launcher…", None);
        let release = latest().await?;
        let version = launcher_version(&release.tag_name).unwrap_or_default().to_string();
        let file = release.assets.iter().find(|a| a.name == name).ok_or_else(|| anyhow!("Horizon {version} has no {name}"))?;
        let archive = paths::cache().join(name);
        let item = DownloadItem::new(&file.url, &archive).hash(file.sha256().map(Hash::sha256)).size(Some(file.size));
        download_all(vec![item], 1, task.counter("Downloading Horizon")).await?;
        task.update("Unpacking Horizon…", None);
        let dir = folder();
        if dir.is_dir() {
            tokio::fs::remove_dir_all(&dir).await.map_err(|e| anyhow!("Close Horizon first ({e})"))?;
        }
        tokio::fs::create_dir_all(&dir).await?;
        if name.ends_with(".zip") {
            unpack_zip(&archive, &dir, "Horizon").await?;
        } else {
            unpack_tar(&archive, &dir).await?;
        }
        let _ = tokio::fs::remove_file(&archive).await;
        make_executable(&program(&dir)).await?;
        tokio::fs::write(dir.join(VERSION_FILE), &version).await?;
        Ok(version)
    })
    .await
}

fn program(dir: &Path) -> PathBuf {
    if cfg!(windows) {
        dir.join("horizon-launcher.exe")
    } else if cfg!(target_os = "macos") {
        dir.join("Horizon.app/Contents/MacOS/horizon-launcher")
    } else {
        dir.join("horizon-launcher")
    }
}

/// Starts Horizon's launcher on its own: it stays open when ResinCraft closes.
fn open() -> Result<()> {
    let exe = program(&folder());
    if !exe.is_file() {
        bail!("Install Horizon first");
    }
    std::process::Command::new(&exe).current_dir(folder()).spawn().map_err(|e| anyhow!("Couldn't start Horizon: {e}"))?;
    Ok(())
}

pub async fn handle(channel: &str, _args: Args) -> Result<Value> {
    match channel {
        "horizon:status" => json(status().await),
        "horizon:install" => json(install().await?),
        "horizon:open" => {
            open()?;
            done()
        }
        _ => bail!("Unknown request: {channel}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_tags_are_told_from_game_releases() {
        assert_eq!(launcher_version("launcher@0.2.30"), Some("0.2.30"));
        assert_eq!(launcher_version("v0.2.97"), None);
        assert_eq!(launcher_version("nightly"), None);
    }

    #[test]
    fn launcher_builds_per_computer() {
        assert_eq!(asset("windows", "x86_64"), Some("horizon-launcher-windows-x64.zip"));
        assert_eq!(asset("linux", "x86_64"), Some("horizon-launcher-linux-x86_64.tar.gz"));
        assert_eq!(asset("macos", "aarch64"), Some("horizon-launcher-macos-arm64.zip"));
        assert_eq!(asset("linux", "aarch64"), None);
    }
}
