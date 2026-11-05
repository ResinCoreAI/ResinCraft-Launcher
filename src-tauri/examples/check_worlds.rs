//! Downloads a real map from CurseForge's Worlds into a throwaway instance, as Discover's Worlds does, and checks
//! it shows in the instance's Worlds list; installing it again must give a second world, not overwrite the first:
//!   RESINCRAFT_CURSEFORGE_KEY=… cargo run --example check_worlds -- [mcVersion] [search]
//! The data folder is left in the temp folder to look at (its path is printed).

use resincraft_lib::content::{curseforge, sources};
use resincraft_lib::core::types::{ContentKind, ContentSource, LoaderType, NewInstance, SearchQuery};
use resincraft_lib::core::{ctx, instances};
use resincraft_lib::player::worlds;

#[tokio::main]
async fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mc = argv.first().cloned().unwrap_or_else(|| "1.20.1".into());
    let search = argv.get(1).cloned().unwrap_or_default();
    let data = std::env::temp_dir().join(format!("resin-worlds-{}", std::process::id()));
    ctx::init(data.clone(), None);
    println!("data folder: {}", data.display());

    let new = NewInstance { name: "Maps".into(), mc_version: mc.clone(), loader: LoaderType::Vanilla, loader_version: None, color: None, group: None };
    let id = instances::create(new, |_| {}).await.unwrap().id;

    let q = SearchQuery {
        query: search,
        kind: ContentKind::World,
        source: Some(ContentSource::Curseforge),
        game_version: Some(mc.clone()),
        loader: None,
        sort: Some("downloads".into()),
        offset: None,
        limit: Some(10),
    };
    let found = sources::search(&q).await.expect("search");
    println!("{} worlds for Minecraft {mc}; first ten:", found.total);
    for h in &found.hits {
        println!("  {} {:?} by {} ({} downloads)", h.id, h.title, h.author, h.downloads);
        assert_eq!(h.kind, ContentKind::World, "{} came back as {:?}", h.title, h.kind);
    }

    // The first map that installs (an author may only allow downloads from the website).
    let mut installed = None;
    for h in &found.hits {
        let files = sources::versions(&h.id, Some(&id)).await.expect("versions");
        let fits = files.iter().find(|f| f.game_versions.contains(&mc));
        println!("{}: {} files, newest for {mc}: {:?}", h.title, files.len(), fits.map(|f| &f.name));
        match curseforge::install_world(&id, &h.id, fits.map(|f| f.id.clone())).await {
            Ok(w) => {
                println!("installed {:?} as saves/{} ({} bytes, made in {:?})", w.name, w.folder, w.size, w.version);
                installed = Some((h.clone(), w));
                break;
            }
            Err(e) => println!("  skipped: {e:#}"),
        }
    }
    let (hit, first) = installed.expect("no map could be installed");
    let again = curseforge::install_world(&id, &hit.id, None).await.expect("second install");
    println!("installed again as saves/{}", again.folder);
    assert_ne!(first.folder, again.folder, "the second install must not reuse the first one's folder");

    let list = worlds::list_worlds(&id).await.unwrap();
    println!("Worlds tab of {id}:");
    for w in &list {
        println!("  {} — saves/{} {:?} {}", w.name, w.folder, w.version, w.error.as_deref().unwrap_or("ok"));
    }
    for folder in [&first.folder, &again.folder] {
        let w = list.iter().find(|w| &w.folder == folder).expect("world missing from the list");
        assert!(w.error.is_none(), "{folder}: {:?}", w.error);
    }
    let leftovers = std::fs::read_dir(data.join("cache").join("world-downloads")).map(|d| d.count()).unwrap_or(0);
    assert_eq!(leftovers, 0, "the downloaded zip should be deleted");
    println!("PASS");
}
