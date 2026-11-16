//! Paid plugins from sites without a download API (SpigotMC premium resources, MythicCraft, Polymart, any plugin
//! store): the launcher opens the plugin's page in a ResinCraft browser window, the player signs in to their own
//! account there and presses the site's Download button, and the launcher catches that download and installs it on
//! the server (or swaps it in for the plugin it updates, after the server stops when it's running). ResinCraft never
//! sees the password and never clicks for the player; site checks (Cloudflare and the like) are the player's to pass.
//!
//! The window keeps its own browser profile (`site-browser/` in the data folder), so the player stays signed in to
//! those sites for the next update.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use super::plugins;
use crate::core::paths;
use crate::content::index::IndexEntry;
use crate::{bail_tr, err_tr, tr};

const WINDOW: &str = "plugin-site";

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SiteOptions {
    /// The installed jar this download replaces (an update).
    #[serde(default)]
    pub replace: Option<String>,
    /// What the plugin is, when known ("spigot:<id>"); otherwise "site:<page>".
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteResult {
    pub file: String,
    pub title: String,
    /// Waits for the server to stop before it's swapped in.
    pub queued: bool,
}

fn close_window() {
    use tauri::Manager;
    if let Some(w) = crate::core::ctx::app().and_then(|a| a.get_webview_window(WINDOW)) {
        let _ = w.close();
    }
}

/// A SpigotMC resource id from its page: `https://www.spigotmc.org/resources/name.12345/` → 12345.
pub fn spigot_id(url: &str) -> Option<u64> {
    let u = tauri::Url::parse(url).ok()?;
    if !u.host_str()?.ends_with("spigotmc.org") {
        return None;
    }
    let mut seg = u.path_segments()?;
    if seg.next()? != "resources" {
        return None;
    }
    let part = seg.next()?;
    part.rsplit('.').next()?.parse().ok().or_else(|| part.parse().ok())
}

/// Plugin files, by what they carry at the top: plugin.yml (Bukkit/Spigot/Paper), paper-plugin.yml, bungee.yml,
/// velocity-plugin.json, or a mod's metadata.
fn is_plugin(names: &[String]) -> bool {
    const MARKS: [&str; 7] = ["plugin.yml", "paper-plugin.yml", "bungee.yml", "velocity-plugin.json", "fabric.mod.json", "META-INF/mods.toml", "META-INF/neoforge.mods.toml"];
    names.iter().any(|n| MARKS.contains(&n.as_str()))
}

/// The downloaded file as a plugin jar: itself, or the one jar inside a zip the site packed it in.
fn plugin_jar(path: &Path) -> Result<PathBuf> {
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| err_tr!("siteDownload.notPlugin", "That download isn't a plugin (.jar). Pick the plugin's main download on the site."))?;
    let names: Vec<String> = zip.file_names().map(str::to_string).collect();
    if is_plugin(&names) {
        return Ok(path.to_path_buf());
    }
    let jars: Vec<String> = names.iter().filter(|n| n.to_lowercase().ends_with(".jar") && !n.contains("__MACOSX")).cloned().collect();
    if jars.len() != 1 {
        bail_tr!(
            "siteDownload.zipChoice",
            "The download is a .zip with {count} jar files in it. Unpack it and add the right one with Add file.",
            count = jars.len()
        );
    }
    let inner = jars[0].rsplit('/').next().unwrap_or(&jars[0]).to_string();
    let out = path.with_file_name(format!("{}-{}", uuid::Uuid::new_v4().simple(), super::super::content::index::safe_name(&inner)?));
    let mut entry = zip.by_name(&jars[0])?;
    let mut w = std::fs::File::create(&out)?;
    std::io::copy(&mut entry, &mut w)?;
    drop(w);
    drop(entry);
    let _ = std::fs::remove_file(path);
    Ok(out)
}

/// Opens `url` for the player to sign in and download; installs what they download. Resolves once it's in (or
/// queued), or fails when the window is closed first.
pub async fn get(id: &str, url: &str, opts: SiteOptions, area: Option<&str>) -> Result<SiteResult> {
    use tauri::webview::DownloadEvent;
    use tauri::{WebviewUrl, WebviewWindowBuilder, WindowEvent};
    let page: tauri::Url = url.parse().map_err(|_| anyhow!("Not a web address: {url}"))?;
    if page.scheme() != "https" {
        bail_tr!("siteDownload.https", "Only https:// pages can be opened for a download.");
    }
    let staged_dir = plugins::staged_dir(id, area).await?;
    tokio::fs::create_dir_all(&staged_dir).await?;

    close_window();
    let app = crate::core::ctx::app().ok_or_else(|| anyhow!("Not available without the app window"))?;
    let (tx, rx) = tokio::sync::oneshot::channel::<Option<Result<PathBuf>>>();
    let tx = Arc::new(Mutex::new(Some(tx)));
    let (dl_tx, close_tx) = (tx.clone(), tx.clone());
    let dir = staged_dir.clone();
    let host = page.host_str().unwrap_or("the site").to_string();
    let title = opts.title.clone().unwrap_or_else(|| host.clone());
    let builder = WebviewWindowBuilder::new(app, WINDOW, WebviewUrl::External(page.clone()));
    #[cfg(windows)]
    let builder = builder.additional_browser_args(crate::webview::BROWSER_ARGS);
    let window = builder
        .title(tr!("siteDownload.window", "{name}: sign in and press Download", name = title).to_string())
        .inner_size(1100.0, 800.0)
        .center()
        .data_directory(paths::root().join("site-browser"))
        .on_download(move |_, event| {
            match event {
                DownloadEvent::Requested { url, destination } => {
                    let name = destination
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .or_else(|| url.path_segments().and_then(|s| s.last().map(str::to_string)))
                        .filter(|n| !n.is_empty())
                        .unwrap_or_else(|| "download.jar".into());
                    let safe: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || "-_.+ ".contains(c) { c } else { '-' }).collect();
                    *destination = dir.join(format!("{}-{}", &uuid::Uuid::new_v4().simple().to_string()[..8], safe.trim()));
                }
                DownloadEvent::Finished { path, success, .. } => {
                    if let Some(tx) = dl_tx.lock().unwrap().take() {
                        let _ = tx.send(Some(match (success, path) {
                            (true, Some(p)) => Ok(p),
                            _ => Err(err_tr!("siteDownload.failed", "The download didn't finish. Try pressing Download again.")),
                        }));
                    }
                }
                _ => {}
            }
            true
        })
        .build()?;
    window.on_window_event(move |e| {
        if matches!(e, WindowEvent::Destroyed) {
            if let Some(tx) = close_tx.lock().unwrap().take() {
                let _ = tx.send(None);
            }
        }
    });

    let got = match rx.await {
        Ok(Some(r)) => r,
        _ => Err(err_tr!("siteDownload.closed", "The window was closed before a download finished.")),
    };
    close_window();
    let file = got?;
    let jar = tokio::task::spawn_blocking(move || plugin_jar(&file)).await??;

    // What it is: the SpigotMC resource (so updates are found), else the page it came from.
    let yml = plugins::plugin_info(&jar).await;
    let mut entry = IndexEntry {
        project_id: opts.project_id.clone().unwrap_or_else(|| match spigot_id(url) {
            Some(rid) => format!("spigot:{rid}"),
            None => format!("site:{url}"),
        }),
        version_id: String::new(),
        version_number: yml.as_ref().map(|(_, v)| v.clone()).unwrap_or_default(),
        title: opts.title.clone().or_else(|| yml.as_ref().map(|(n, _)| n.clone())).unwrap_or_else(|| title.clone()),
        icon_url: None,
        sha1: crate::core::net::hash_file(&jar, crate::core::net::HashAlgo::Sha1).await.unwrap_or_default(),
    };
    if let Some(rid) = entry.project_id.strip_prefix("spigot:") {
        if let Ok((icon, vid, number)) = plugins::spigot_version(rid).await {
            entry.icon_url = Some(icon);
            entry.version_id = vid;
            if entry.version_number.is_empty() {
                entry.version_number = number;
            }
        }
    }
    let base: String = entry.title.chars().map(|c| if c.is_ascii_alphanumeric() || "-_.".contains(c) { c } else { '-' }).collect();
    let target = format!("{}.jar", base.trim_matches('-'));
    let staged = jar.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let title = entry.title.clone();
    let queued = plugins::place_staged(id, area, staged, target.clone(), opts.replace, entry).await?;
    Ok(SiteResult { file: target, title, queued })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_spigot_pages() {
        assert_eq!(spigot_id("https://www.spigotmc.org/resources/luckperms.28140/"), Some(28140));
        assert_eq!(spigot_id("https://www.spigotmc.org/resources/28140/updates"), Some(28140));
        assert_eq!(spigot_id("https://mythiccraft.io/index.php?resources/mythicmobs.1/"), None);
    }

    #[test]
    fn finds_the_plugin_jar() {
        let dir = std::env::temp_dir().join(format!("rc-site-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let write = |name: &str, entries: &[(&str, &[u8])]| {
            let p = dir.join(name);
            let mut w = zip::ZipWriter::new(std::fs::File::create(&p).unwrap());
            for (n, b) in entries {
                w.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
                std::io::Write::write_all(&mut w, b).unwrap();
            }
            w.finish().unwrap();
            p
        };
        let jar = write("a.jar", &[("plugin.yml", b"name: A\nversion: 1.0\n")]);
        assert_eq!(plugin_jar(&jar).unwrap(), jar);
        let inner = write("inner.jar", &[("plugin.yml", b"name: B\n")]);
        let bytes = std::fs::read(&inner).unwrap();
        let packed = write("pack.zip", &[("B/B-2.0.jar", &bytes), ("B/readme.txt", b"hi")]);
        let out = plugin_jar(&packed).unwrap();
        assert!(out.to_string_lossy().ends_with("B-2.0.jar"));
        let two = write("two.zip", &[("x.jar", &bytes), ("y.jar", &bytes)]);
        assert!(plugin_jar(&two).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
