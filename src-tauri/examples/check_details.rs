//! Live check of Discover's details page on every source: description, gallery, links, and the newest
//! version's changelog and dependencies.
//!
//!   cargo run --example check_details

use std::path::PathBuf;

use anyhow::{bail, Result};
use resincraft_lib::content::{details, github, sources};
use resincraft_lib::core::{ctx, settings};

async fn check(id: &str) -> Result<()> {
    let d = details::details(id).await?;
    println!(
        "  {} by {} ({:?}): body {} chars, {} images, links [{}], license {:?}",
        d.hit.title,
        d.hit.author,
        d.hit.kind,
        d.body.len(),
        d.gallery.len(),
        d.links.iter().map(|l| l.kind.as_str()).collect::<Vec<_>>().join(", "),
        d.license
    );
    if d.hit.title.is_empty() {
        bail!("no title");
    }
    let versions = sources::versions(id, None).await?;
    let Some(v) = versions.first() else { bail!("no versions") };
    println!("  {} versions; newest {} MC{:?} {:?} {}", versions.len(), v.version_number, v.game_versions, v.loaders, v.kind);
    let info = details::version_info(id, &v.id).await?;
    println!(
        "  changelog {} chars; dependencies: {}",
        info.changelog.as_deref().map_or(0, str::len),
        info.dependencies.iter().map(|x| format!("{} ({}, {})", x.title, x.kind, x.id.as_deref().unwrap_or("-"))).collect::<Vec<_>>().join(", ")
    );
    if d.changelogs != info.changelog.is_some() {
        println!("  note: changelogs={} but this version's changelog is {}", d.changelogs, if info.changelog.is_some() { "there" } else { "empty" });
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(".dev-data");
    let data = std::fs::canonicalize(&data).unwrap_or(data);
    ctx::init(data, None);
    let has_cf = !settings::get().await.curseforge_key.trim().is_empty();
    let mut ids: Vec<String> = ["sodium", "fabric-api", "1KVo5zza", "ftb:126", "atl:SkyFactory4", "technic:tekkit"].map(String::from).to_vec();
    if has_cf {
        // JEI (a mod with dependencies on some files) and All the Mods 9 (a modpack).
        ids.extend(["cf:238222", "cf:715572"].map(String::from));
    }
    // A GitHub mod that publishes jars in its releases.
    if let Ok((repos, _)) = github::search("", "minecraft-mod", 0, 10).await {
        for r in repos {
            if github::releases(&r.id()).await.is_ok_and(|l| !l.is_empty()) {
                ids.push(r.id());
                break;
            }
        }
    }
    let mut failed = 0;
    for id in ids {
        println!("{id}");
        if let Err(e) = check(&id).await {
            failed += 1;
            println!("  FAIL {e:#}");
        }
    }
    if !has_cf {
        println!("(no CurseForge key in .dev-data; CurseForge skipped)");
    }
    std::process::exit(if failed > 0 { 1 } else { 0 });
}
