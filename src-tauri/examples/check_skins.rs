//! Live check of the skin library against the real Mojang and MineSkin APIs: player lookup, gallery search,
//! saving (once per image), listing and removing. Uses a throwaway data folder, removed again at the end.
//!
//! cargo run --example check_skins

use std::sync::atomic::{AtomicUsize, Ordering};

use resincraft_lib::core::ctx;
use resincraft_lib::player::skins;

static FAILS: AtomicUsize = AtomicUsize::new(0);

fn check(name: &str, ok: bool, detail: impl std::fmt::Display) {
    if ok {
        println!("PASS {name}");
    } else {
        FAILS.fetch_add(1, Ordering::SeqCst);
        println!("FAIL {name}: {detail}");
    }
}

#[tokio::main]
async fn main() {
    let dir = std::env::temp_dir().join(format!("resincraft-skins-{}", uuid::Uuid::new_v4().simple()));
    ctx::init(dir.clone(), None);

    let jeb = skins::lookup_player("jeb_").await;
    check("look up jeb_", jeb.as_ref().is_ok_and(|s| s.url.starts_with("https://textures.minecraft.net/texture/")), format!("{jeb:?}"));
    let missing = skins::lookup_player("zz_no_such_plyr").await;
    check("unknown player is an error", missing.is_err(), format!("{missing:?}"));
    if let Err(e) = &missing {
        println!("     {e}");
    }

    let page = skins::search("fox", None).await;
    check("search MineSkin for fox", page.as_ref().is_ok_and(|p| !p.skins.is_empty()), format!("{:?}", page.as_ref().err()));
    let newest = skins::search("", None).await;
    check("newest MineSkin skins", newest.as_ref().is_ok_and(|p| !p.skins.is_empty() && p.next.is_some()), format!("{:?}", newest.as_ref().err()));
    if let Ok(p) = &page {
        println!("     {} skins: {}", p.skins.len(), p.skins.iter().take(5).map(|s| s.name.as_str()).collect::<Vec<_>>().join(", "));
    }

    if let (Ok(jeb), Ok(page)) = (&jeb, &page) {
        let a = skins::save_online(jeb).await;
        let b = skins::save_online(jeb).await;
        check("save a player's skin once", matches!((&a, &b), (Ok(a), Ok(b)) if a.id == b.id && a.source == "player"), format!("{a:?} / {b:?}"));
        let online = skins::save_online(&page.skins[0]).await;
        check("save a MineSkin skin", online.as_ref().is_ok_and(|s| s.source == "online" && s.preview.starts_with("data:image/png")), format!("{online:?}"));
        if let Ok(s) = &online {
            println!("     {} ({})", s.name, s.variant);
        }
        let listed = skins::list().await.map(|l| l.len()).unwrap_or(0);
        check("library lists both", listed == 2, listed);
        if let Ok(a) = &a {
            let renamed = skins::update(&a.id, Some("  Jeb's skin  "), Some("slim")).await;
            check("rename and change arms", renamed.as_ref().is_ok_and(|s| s.name == "Jeb's skin" && s.variant == "slim"), format!("{renamed:?}"));
            let _ = skins::remove(&a.id).await;
            let left = skins::list().await.map(|l| l.len()).unwrap_or(0);
            check("remove", left == 1, left);
        }
    }

    let _ = std::fs::remove_dir_all(&dir);
    let fails = FAILS.load(Ordering::SeqCst);
    println!("{}", if fails == 0 { "ALL PASSED".to_string() } else { format!("{fails} FAILED") });
    std::process::exit(if fails == 0 { 0 } else { 1 });
}
