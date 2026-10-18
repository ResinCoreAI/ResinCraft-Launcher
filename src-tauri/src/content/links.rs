//! Installing from a link: a project page on Modrinth, CurseForge, dev.bukkit.org, Hangar, SpigotMC or GitHub
//! (with its version, when the link names one), or a .jar from anywhere. Shared by instances (`install_link`
//! here) and hosted servers (server/plugins.rs).

use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;

use super::curseforge::cf_raw;
use super::index::{folder_of, load_index, safe_name, save_index};
use super::{github, modrinth};
use crate::core::events::Task;
use crate::core::fsutil::safe_join;
use crate::core::instances;
use crate::core::net::{download_file, fetch_json, DownloadItem};
use crate::core::settings;
use crate::core::types::ContentKind;
use crate::{bail_tr, err_tr, tr};

const MODRINTH: &str = "https://api.modrinth.com/v2";

/// A project (and maybe a version) named by a web page's address, or a file to download as it is.
#[derive(Debug, PartialEq)]
pub enum Link {
    /// A Modrinth id or slug, "hangar:<slug>", "spigot:<id>" or "gh:<owner>/<repo>".
    Project { id: String, version: Option<String> },
    /// A CurseForge or dev.bukkit.org page: its slug still has to be looked up (CurseForge's class id: 5 Bukkit
    /// plugins, 6 mods, 12 resource packs, 6552 shaders, 4471 modpacks).
    CurseForge { slug: String, class: u32, file: Option<String> },
    File { url: String, name: String },
}

impl Link {
    /// Server plugin sites; an instance can't use them.
    pub fn is_server_plugin(&self) -> bool {
        match self {
            Link::Project { id, .. } => id.starts_with("hangar:") || id.starts_with("spigot:"),
            Link::CurseForge { class, .. } => *class == 5,
            Link::File { .. } => false,
        }
    }
}

fn unknown() -> anyhow::Error {
    err_tr!("hostAddon.unknownLink", "ResinCraft can't tell which plugin or mod that link is")
}

/// Reads a link to a project page on one of the sources, or to a .jar.
pub fn parse_link(url: &str) -> Result<Link> {
    let parsed = reqwest::Url::parse(url.trim()).map_err(|_| err_tr!("hostAddon.badLink", "That isn't a web address"))?;
    if !matches!(parsed.scheme(), "https" | "http") {
        bail_tr!("hostAddon.badLink", "That isn't a web address");
    }
    let host = parsed.host_str().unwrap_or_default().trim_start_matches("www.").to_lowercase();
    let segs: Vec<String> = parsed.path_segments().map(|s| s.filter(|p| !p.is_empty()).map(|p| p.to_string()).collect()).unwrap_or_default();
    let seg = |i: usize| segs.get(i).cloned();
    let after = |name: &str| segs.iter().position(|s| s == name).and_then(|i| segs.get(i + 1).cloned());
    let numeric = |s: Option<String>| s.filter(|f| !f.is_empty() && f.chars().all(|c| c.is_ascii_digit()));
    let project = |id: String, version: Option<String>| Ok(Link::Project { id, version });
    match host.as_str() {
        "modrinth.com" => match (seg(0), seg(1)) {
            (Some(_), Some(slug)) => project(slug, after("version")),
            _ => Err(unknown()),
        },
        "curseforge.com" | "legacy.curseforge.com" => match (seg(0).as_deref(), seg(1).as_deref(), seg(2)) {
            (Some("minecraft"), Some(section), Some(slug)) => {
                let class = match section {
                    "bukkit-plugins" => 5,
                    "mc-mods" => 6,
                    "texture-packs" => 12,
                    "shaders" => 6552,
                    "modpacks" => 4471,
                    _ => return Err(unknown()),
                };
                Ok(Link::CurseForge { slug, class, file: numeric(after("files")) })
            }
            _ => Err(unknown()),
        },
        "dev.bukkit.org" => match (seg(0).as_deref(), seg(1)) {
            (Some("projects"), Some(slug)) => Ok(Link::CurseForge { slug, class: 5, file: numeric(after("files")) }),
            _ => Err(unknown()),
        },
        "hangar.papermc.io" => match seg(1) {
            Some(slug) => project(format!("hangar:{slug}"), after("versions")),
            None => Err(unknown()),
        },
        "spigotmc.org" => {
            // /resources/name.12345/ (or just /resources/12345/)
            let id = numeric(after("resources").and_then(|r| r.rsplit('.').next().map(str::to_string)));
            match id {
                Some(id) => project(format!("spigot:{id}"), None),
                None => Err(unknown()),
            }
        }
        "github.com" => match (seg(0), seg(1)) {
            (Some(owner), Some(repo)) => {
                // A release page or a release download: that release.
                let tag = if seg(2).as_deref() == Some("releases") { after("tag").or_else(|| after("download")) } else { None };
                project(format!("{}{owner}/{}", github::PREFIX, repo.trim_end_matches(".git")), tag)
            }
            _ => Err(unknown()),
        },
        _ => {
            let name = segs.last().cloned().unwrap_or_default();
            if !name.to_lowercase().ends_with(".jar") {
                return Err(unknown());
            }
            let name = urlencoding::decode(&name).map(|n| n.into_owned()).unwrap_or(name);
            Ok(Link::File { url: parsed.to_string(), name })
        }
    }
}

/// CurseForge's numeric id for a project slug in a class.
pub async fn cf_id_of_slug(slug: &str, class: u32) -> Result<String> {
    if settings::curseforge_key().await.is_empty() {
        bail_tr!("curseforge.unavailable", "CurseForge isn't available in this build of ResinCraft.");
    }
    #[derive(Deserialize)]
    struct Found {
        data: Vec<Value>,
    }
    let found: Found = cf_raw(&format!("/mods/search?gameId=432&classId={class}&slug={}", urlencoding::encode(slug)), None).await?;
    found
        .data
        .first()
        .and_then(|m| m.get("id").and_then(Value::as_u64))
        .map(|id| id.to_string())
        .ok_or_else(|| err_tr!("hostAddon.notFound", "Nothing called {name} was found there", name = slug))
}

/// A Modrinth link names a version by its number as often as by its id: the id either way.
pub async fn modrinth_version_id(project: &str, version: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct V {
        id: String,
    }
    Ok(fetch_json::<V>(&format!("{MODRINTH}/project/{}/version/{}", urlencoding::encode(project), urlencoding::encode(version))).await?.id)
}

/// A jar is a zip: it starts with "PK".
pub async fn is_zip(p: &std::path::Path) -> bool {
    use tokio::io::AsyncReadExt;
    let Ok(mut f) = tokio::fs::File::open(p).await else { return false };
    let mut magic = [0u8; 2];
    f.read_exact(&mut magic).await.is_ok() && &magic == b"PK"
}

/// Installs what a link points to into an instance (mods, resource packs or shaders, as `kind` says).
pub async fn install_link(instance_id: &str, url: &str, kind: ContentKind) -> Result<()> {
    let link = parse_link(url)?;
    if link.is_server_plugin() {
        bail_tr!("links.serverPlugin", "That's a server plugin. Add it to a server's Plugins tab instead.");
    }
    match link {
        Link::Project { id, version } if id.starts_with(github::PREFIX) => github::install(instance_id, &id, kind, version).await,
        Link::Project { id, version } => {
            let version = match version {
                Some(v) => Some(modrinth_version_id(&id, &v).await?),
                None => None,
            };
            modrinth::install(instance_id, &id, kind, version).await
        }
        Link::CurseForge { slug, class, file } => {
            let cid = cf_id_of_slug(&slug, class).await?;
            super::curseforge::install(instance_id, &format!("cf:{cid}"), kind, file).await
        }
        Link::File { url, name } => {
            let inst = instances::get(instance_id).await?;
            let folder = folder_of(&inst, kind);
            tokio::fs::create_dir_all(&folder).await?;
            Task::run(tr!("links.downloading", "Downloading {name}", name = name), Some(inst.id.clone()), move |_| async move {
                let dest = safe_join(&folder, safe_name(&name)?)?;
                download_file(&DownloadItem::new(&url, &dest)).await?;
                if !is_zip(&dest).await {
                    let _ = tokio::fs::remove_file(&dest).await;
                    bail_tr!("hostAddon.notJar", "The download for {name} isn't a plugin file", name = name);
                }
                // Left out of the index: the next listing names it by its hash.
                let idx = load_index(&inst.id).await;
                save_index(&inst.id, &idx).await
            })
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_name_their_project() {
        let p = |id: &str, v: Option<&str>| Link::Project { id: id.into(), version: v.map(str::to_string) };
        assert_eq!(parse_link("https://modrinth.com/plugin/luckperms").unwrap(), p("luckperms", None));
        assert_eq!(parse_link("https://modrinth.com/mod/sodium/version/mc1.21.1-0.6.0").unwrap(), p("sodium", Some("mc1.21.1-0.6.0")));
        assert_eq!(parse_link("https://hangar.papermc.io/HelpChat/PlaceholderAPI").unwrap(), p("hangar:PlaceholderAPI", None));
        assert_eq!(parse_link("https://www.spigotmc.org/resources/luckperms.28140/").unwrap(), p("spigot:28140", None));
        assert_eq!(parse_link("https://github.com/EssentialsX/Essentials/releases/tag/2.21.0").unwrap(), p("gh:EssentialsX/Essentials", Some("2.21.0")));
        assert_eq!(parse_link("https://dev.bukkit.org/projects/worldguard/files/123").unwrap(), Link::CurseForge { slug: "worldguard".into(), class: 5, file: Some("123".into()) });
        assert_eq!(parse_link("https://www.curseforge.com/minecraft/mc-mods/jei").unwrap(), Link::CurseForge { slug: "jei".into(), class: 6, file: None });
        assert_eq!(
            parse_link("https://example.com/files/My%20Plugin.jar").unwrap(),
            Link::File { url: "https://example.com/files/My%20Plugin.jar".into(), name: "My Plugin.jar".into() }
        );
        assert!(parse_link("https://example.com/page").is_err());
        assert!(parse_link("file:///C:/x.jar").is_err());
        assert!(parse_link("https://dev.bukkit.org/projects/x").unwrap().is_server_plugin());
        assert!(!parse_link("https://modrinth.com/mod/sodium").unwrap().is_server_plugin());
    }
}
