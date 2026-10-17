//! Live, headless check of the content group: searches every source, installs Modrinth mods with a
//! dependency, exports .mrpack / CurseForge .zip, imports the .mrpack back, and checks the CurseForge
//! fingerprint against FTB's published `cfMurmur` values. Cleans up everything it creates.
//!
//!   cargo run --example check_content            (add `--modpack` to also install a small Modrinth modpack)

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{anyhow, bail, Result};
use resincraft_lib::content::{curseforge, export, modrinth, sources};
use resincraft_lib::core::types::{ContentKind, ContentSource, ExportOptions, LoaderType, NewInstance, SearchQuery};
use resincraft_lib::core::{ctx, events, instances, net, paths, settings};
use serde_json::Value;

static PASSED: AtomicUsize = AtomicUsize::new(0);
static FAILED: AtomicUsize = AtomicUsize::new(0);

fn report<T>(name: &str, r: Result<T>) -> Option<T> {
    match r {
        Ok(v) => {
            PASSED.fetch_add(1, Ordering::Relaxed);
            println!("PASS {name}");
            Some(v)
        }
        Err(e) => {
            FAILED.fetch_add(1, Ordering::Relaxed);
            println!("FAIL {name}: {e:#}");
            None
        }
    }
}

fn query(q: &str, kind: ContentKind, source: ContentSource) -> SearchQuery {
    serde_json::from_value(serde_json::json!({ "query": q, "kind": kind, "source": source })).unwrap()
}

async fn check_source(name: &str, mut q: SearchQuery) -> Option<String> {
    q.limit = Some(5);
    let hits = report(&format!("{name} search \"{}\"", q.query), async {
        let r = sources::search(&q).await?;
        if r.hits.is_empty() {
            bail!("no results");
        }
        println!("     {} total; {}", r.total, r.hits.iter().map(|h| format!("{} [{}]", h.title, h.id)).collect::<Vec<_>>().join(" | "));
        Ok(r.hits)
    }.await)?;
    let id = hits[0].id.clone();
    report(&format!("{name} versions of {}", hits[0].title), async {
        let v = sources::versions(&id, None).await?;
        if v.is_empty() {
            bail!("no versions");
        }
        println!(
            "     {} versions; {}",
            v.len(),
            v.iter().take(3).map(|x| format!("{} MC{} {:?} {}", x.version_number, x.game_versions.first().cloned().unwrap_or_default(), x.loaders, x.kind)).collect::<Vec<_>>().join(" | ")
        );
        Ok(())
    }.await);
    Some(id)
}

fn mod_names(id: &str) -> BTreeSet<String> {
    std::fs::read_dir(paths::game_dir(id).join("mods"))
        .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".jar")).collect())
        .unwrap_or_default()
}

#[tokio::main]
async fn main() {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(".dev-data");
    let data = std::fs::canonicalize(&data).unwrap_or(data);
    ctx::init(data, None);
    events::set_sink(|channel, payload| {
        if channel == "task" && payload["state"] == "error" {
            println!("     [task failed] {}: {}", payload["title"].as_str().unwrap_or(""), payload["error"].as_str().unwrap_or(""));
        }
    });
    let has_cf = !settings::get().await.curseforge_key.trim().is_empty();
    println!("CurseForge key configured: {has_cf}");

    let mut created: Vec<String> = Vec::new();
    let work = paths::cache().join("check-content");
    let _ = std::fs::create_dir_all(&work);

    // ---------- Search & versions on every source ----------
    let mut q = query("sodium", ContentKind::Mod, ContentSource::Modrinth);
    q.game_version = Some("1.21.1".into());
    q.loader = Some(LoaderType::Fabric);
    check_source("modrinth", q).await;
    check_source("modrinth", query("", ContentKind::Modpack, ContentSource::Modrinth)).await;
    check_source("ftb", query("stoneblock", ContentKind::Modpack, ContentSource::Ftb)).await;
    check_source("ftb", query("", ContentKind::Modpack, ContentSource::Ftb)).await;
    check_source("atlauncher", query("skyfactory", ContentKind::Modpack, ContentSource::Atlauncher)).await;
    check_source("technic", query("cobblemon", ContentKind::Modpack, ContentSource::Technic)).await;
    report("technic solder builds", async {
        let v = sources::versions("technic:apex-pixelmon-reforged", None).await?;
        println!("     {}", v.iter().take(4).map(|x| format!("{} MC{} {}", x.version_number, x.game_versions[0], x.kind)).collect::<Vec<_>>().join(" | "));
        if v.is_empty() {
            bail!("no builds");
        }
        Ok(())
    }.await);
    if has_cf {
        let mut q = query("jei", ContentKind::Mod, ContentSource::Curseforge);
        q.game_version = Some("1.21.1".into());
        q.loader = Some(LoaderType::Neoforge);
        check_source("curseforge", q).await;
        check_source("curseforge", query("all the mods", ContentKind::Modpack, ContentSource::Curseforge)).await;
    } else {
        println!("SKIP curseforge search (no API key)");
        report("curseforge friendly no-key error", async {
            match sources::search(&query("jei", ContentKind::Mod, ContentSource::Curseforge)).await {
                Err(e) if e.to_string().contains("needs an API key") => Ok(()),
                other => Err(anyhow!("unexpected: {:?}", other.map(|r| r.total))),
            }
        }.await);
    }

    // ---------- CurseForge fingerprint vs FTB's published cfMurmur ----------
    report("curseforge fingerprint matches FTB cfMurmur", async {
        let v: Value = net::fetch_json("https://api.feed-the-beast.com/v1/modpacks/public/modpack/100/100359").await?;
        let files = v["files"].as_array().ok_or_else(|| anyhow!("no files"))?;
        let mut samples: Vec<&Value> = files.iter().filter(|f| f["type"] == "mod").take(3).collect();
        samples.extend(files.iter().take(2));
        let mut ok = 0;
        for f in &samples {
            let bytes = ctx::http().get(f["url"].as_str().unwrap_or_default()).send().await?.error_for_status()?.bytes().await?;
            let mine = curseforge::fingerprint(&bytes);
            let theirs = f["hashes"]["cfMurmur"].as_u64().unwrap_or(0);
            println!("     {} {} mine={mine} cf={theirs}", if mine as u64 == theirs { "MATCH" } else { "DIFF " }, f["name"].as_str().unwrap_or(""));
            if mine as u64 == theirs {
                ok += 1;
            }
        }
        if ok != samples.len() {
            bail!("{ok}/{} matched", samples.len());
        }
        Ok(())
    }.await);

    // ---------- Instance with Modrinth mods ----------
    let inst = report("create Fabric 1.21.1 instance", async {
        let new = NewInstance { name: "check-content test".into(), mc_version: "1.21.1".into(), loader: LoaderType::Fabric, loader_version: Some("0.16.5".into()), color: None };
        instances::create(new, |_| {}).await
    }.await);
    let Some(inst) = inst else { return finish(&created, &work).await };
    created.push(inst.id.clone());
    let id = inst.id.clone();

    report("vanilla instance refuses mods", async {
        let new = NewInstance { name: "check-content vanilla".into(), mc_version: "1.21.1".into(), loader: LoaderType::Vanilla, loader_version: None, color: None };
        let v = instances::create(new, |_| {}).await?;
        created.push(v.id.clone());
        match sources::install(&v.id, "lithium", ContentKind::Mod, None).await {
            Err(e) if e.to_string().contains("vanilla instance") => Ok(()),
            other => Err(anyhow!("unexpected: {other:?}")),
        }
    }.await);

    report("modrinth install lithium", sources::install(&id, "lithium", ContentKind::Mod, None).await);
    report("modrinth install iris (pulls in sodium)", sources::install(&id, "iris", ContentKind::Mod, None).await);
    let listed = report("content list identifies mods", async {
        let items = sources::list_content(&id, ContentKind::Mod).await?;
        for i in &items {
            println!("     {} -> {:?} {:?} {:?}", i.filename, i.title, i.project_id, i.version_number);
        }
        let titles: Vec<String> = items.iter().filter_map(|i| i.title.clone()).collect();
        for want in ["Lithium", "Iris", "Sodium"] {
            if !titles.iter().any(|t| t.contains(want)) {
                bail!("{want} missing from {titles:?}");
            }
        }
        Ok(items)
    }.await);
    report("content-index.json uses mod/<filename> keys", async {
        let text = std::fs::read_to_string(paths::instance_dir(&id).join("content-index.json"))?;
        let v: Value = serde_json::from_str(&text)?;
        let keys: Vec<&String> = v.as_object().ok_or_else(|| anyhow!("not an object"))?.keys().collect();
        if keys.len() < 3 || !keys.iter().all(|k| k.starts_with("mod/")) || !v[keys[0]]["projectId"].is_string() || !v[keys[0]]["sha1"].is_string() {
            bail!("unexpected index: {text}");
        }
        Ok(())
    }.await);

    // Hand-made files and configs.
    let mods_dir = paths::game_dir(&id).join("mods");
    let _ = std::fs::create_dir_all(paths::game_dir(&id).join("config"));
    let _ = std::fs::write(paths::game_dir(&id).join("config").join("check-content.json"), b"{ \"hello\": \"resin\" }\n");
    let _ = std::fs::write(mods_dir.join("handmade-test.jar"), format!("not really a jar {}", uuid::Uuid::new_v4()));
    report("hand-added jar listed without a project", async {
        let items = sources::list_content(&id, ContentKind::Mod).await?;
        let h = items.iter().find(|i| i.filename == "handmade-test.jar").ok_or_else(|| anyhow!("not listed"))?;
        if h.project_id.is_some() {
            bail!("unexpectedly identified");
        }
        Ok(())
    }.await);

    // Toggle twice.
    if let Some(lith) = listed.as_ref().and_then(|l| l.iter().find(|i| i.title.as_deref() == Some("Lithium")).cloned()) {
        report("toggle disables and re-enables", async {
            sources::list_content(&id, ContentKind::Mod).await?;
            modrinth::toggle_content(&id, ContentKind::Mod, &lith.filename).await?;
            let items = sources::list_content(&id, ContentKind::Mod).await?;
            let d = items.iter().find(|i| i.filename == format!("{}.disabled", lith.filename)).ok_or_else(|| anyhow!("no .disabled file"))?;
            if d.enabled || d.title.as_deref() != Some("Lithium") {
                bail!("disabled file not tracked");
            }
            modrinth::toggle_content(&id, ContentKind::Mod, &d.filename).await?;
            if !mods_dir.join(&lith.filename).exists() {
                bail!("not re-enabled");
            }
            Ok(())
        }.await);

        report("changeVersion to an older lithium keeps disabled state", async {
            let vs = sources::versions("lithium", Some(&id)).await?;
            let older = vs.get(1).ok_or_else(|| anyhow!("only one version"))?;
            modrinth::toggle_content(&id, ContentKind::Mod, &lith.filename).await?;
            sources::change_version(&id, ContentKind::Mod, &format!("{}.disabled", lith.filename), &older.id).await?;
            let items = sources::list_content(&id, ContentKind::Mod).await?;
            let now = items.iter().find(|i| i.project_id == lith.project_id).ok_or_else(|| anyhow!("lithium gone"))?;
            println!("     {} -> {} (enabled: {})", lith.filename, now.filename, now.enabled);
            if now.version_id.as_deref() != Some(older.id.as_str()) || now.enabled {
                bail!("expected disabled {} got {:?} enabled={}", older.version_number, now.version_number, now.enabled);
            }
            if items.iter().filter(|i| i.project_id == lith.project_id).count() != 1 {
                bail!("old copy left behind");
            }
            modrinth::toggle_content(&id, ContentKind::Mod, &now.filename).await?;
            Ok(())
        }.await);

        report("checkUpdates offers the newer lithium", async {
            let mods = sources::check_updates(&id).await?;
            let l = mods.iter().find(|i| i.project_id == lith.project_id).ok_or_else(|| anyhow!("lithium gone"))?;
            let u = l.update.as_ref().ok_or_else(|| anyhow!("no update offered"))?;
            println!("     lithium {:?} -> {}", l.version_number, u.version_number);
            sources::update_content(&id, &l.filename).await?;
            let after = sources::list_content(&id, ContentKind::Mod).await?;
            let l2 = after.iter().find(|i| i.project_id == lith.project_id).ok_or_else(|| anyhow!("lithium gone"))?;
            if l2.version_id.as_deref() != Some(u.version_id.as_str()) {
                bail!("update not applied");
            }
            Ok(())
        }.await);
    }

    // CurseForge install into the same instance.
    if has_cf {
        report("curseforge install a Fabric mod (appleskin)", async {
            let mut q = query("appleskin", ContentKind::Mod, ContentSource::Curseforge);
            q.game_version = Some("1.21.1".into());
            q.loader = Some(LoaderType::Fabric);
            let hit = sources::search(&q).await?.hits.into_iter().next().ok_or_else(|| anyhow!("no hit"))?;
            sources::install(&id, &hit.id, ContentKind::Mod, None).await?;
            let items = sources::list_content(&id, ContentKind::Mod).await?;
            let a = items.iter().find(|i| i.project_id.as_deref() == Some(hit.id.as_str())).ok_or_else(|| anyhow!("not listed"))?;
            println!("     {} -> {:?} {:?}", a.filename, a.project_id, a.version_number);
            let mods = sources::check_updates(&id).await?;
            println!("     updates offered: {}", mods.iter().filter(|m| m.update.is_some()).count());
            Ok(())
        }.await);
    }

    report("exportPreview suggests config", async {
        let p = export::export_preview(&id).await?;
        println!("     {}", p.iter().map(|e| format!("{}{}{}", e.name, if e.is_dir { "/" } else { "" }, if e.suggested { "*" } else { "" })).collect::<Vec<_>>().join(" "));
        if !p.iter().any(|e| e.name == "config" && e.suggested && e.is_dir) || p.iter().any(|e| e.name == "mods") {
            bail!("unexpected preview");
        }
        Ok(())
    }.await);

    let opts = |format: &str| -> ExportOptions {
        serde_json::from_value(serde_json::json!({
            "format": format, "name": "Check Content", "version": "1.0.0", "author": "ResinCraft", "summary": "test", "include": ["config"]
        }))
        .unwrap()
    };
    let mrpack = work.join("check-content.mrpack");
    let mr = report("export .mrpack", async {
        let r = export::export_modpack(&id, opts("mrpack"), &mrpack).await?;
        println!("     linked {} bundled {:?} skippedDisabled {}", r.linked, r.bundled, r.skipped_disabled);
        if r.linked < 3 || r.bundled != vec!["mods/handmade-test.jar".to_string()] {
            bail!("unexpected result");
        }
        let names = zip_names(&mrpack)?;
        for want in ["modrinth.index.json", "overrides/config/check-content.json", "overrides/mods/handmade-test.jar"] {
            if !names.contains(want) {
                bail!("{want} missing from {names:?}");
            }
        }
        Ok(r)
    }.await);
    let cfzip = work.join("check-content-cf.zip");
    report("export CurseForge .zip", async {
        let r = export::export_modpack(&id, opts("curseforge"), &cfzip).await?;
        println!("     linked {} bundled {:?}", r.linked, r.bundled);
        let names = zip_names(&cfzip)?;
        if !names.contains("manifest.json") || !names.contains("overrides/config/check-content.json") || !names.contains("overrides/mods/handmade-test.jar") {
            bail!("unexpected contents {names:?}");
        }
        if has_cf && r.linked == 0 {
            bail!("nothing linked although a key is configured");
        }
        Ok(())
    }.await);

    // Import the .mrpack back and compare.
    if mr.is_some() {
        report("import .mrpack reproduces the mod list", async {
            let imported = sources::import_modpack_file(&mrpack).await?;
            created.push(imported.id.clone());
            let (a, b) = (mod_names(&id), mod_names(&imported.id));
            println!("     original {a:?}\n     imported {b:?}");
            if a != b {
                bail!("mod lists differ");
            }
            if !paths::game_dir(&imported.id).join("config").join("check-content.json").exists() {
                bail!("config override missing");
            }
            if imported.loader != LoaderType::Fabric || imported.mc_version != "1.21.1" || imported.loader_version.as_deref() != Some("0.16.5") {
                bail!("wrong loader/version {:?} {} {:?}", imported.loader, imported.mc_version, imported.loader_version);
            }
            let items = sources::list_content(&imported.id, ContentKind::Mod).await?;
            if items.iter().filter(|i| i.project_id.is_some()).count() < 3 {
                bail!("imported mods not identified");
            }
            Ok(())
        }.await);
    }
    if has_cf {
        report("import CurseForge .zip reproduces the mod list", async {
            let imported = sources::import_modpack_file(&cfzip).await?;
            created.push(imported.id.clone());
            let (a, b) = (mod_names(&id), mod_names(&imported.id));
            println!("     imported {b:?}");
            if a != b {
                bail!("mod lists differ: original {a:?}");
            }
            Ok(())
        }.await);
    }
    report("import rejects other files", async {
        let bad = work.join("not-a-pack.zip");
        std::fs::write(&bad, b"nope")?;
        match sources::import_modpack_file(&bad).await {
            Err(e) if e.to_string().contains("isn't a Modrinth or CurseForge modpack") => Ok(()),
            other => Err(anyhow!("unexpected: {:?}", other.map(|i| i.id))),
        }
    }.await);

    report("remove sends the hand-made jar to the Recycle Bin", async {
        sources::list_content(&id, ContentKind::Mod).await?;
        modrinth::remove_content(&id, ContentKind::Mod, "handmade-test.jar").await?;
        if mods_dir.join("handmade-test.jar").exists() {
            bail!("still there");
        }
        Ok(())
    }.await);

    if std::env::args().any(|a| a == "--modpack") {
        report("modrinth modpack install (fabulously-optimized)", async {
            let inst = sources::install_modpack("fabulously-optimized", None).await?;
            created.push(inst.id.clone());
            println!("     {} {} {:?} {:?}: {} mods", inst.name, inst.mc_version, inst.loader, inst.loader_version, mod_names(&inst.id).len());
            Ok(())
        }.await);
    }

    finish(&created, &work).await
}

fn zip_names(p: &Path) -> Result<BTreeSet<String>> {
    let z = zip::ZipArchive::new(std::fs::File::open(p)?)?;
    Ok(z.file_names().map(str::to_string).collect())
}

async fn finish(created: &[String], work: &Path) {
    for id in created {
        let _ = tokio::fs::remove_dir_all(paths::instance_dir(id)).await;
    }
    let _ = tokio::fs::remove_dir_all(work).await;
    let (p, f) = (PASSED.load(Ordering::Relaxed), FAILED.load(Ordering::Relaxed));
    println!("\n{p} passed, {f} failed; cleaned up {} instance(s)", created.len());
    if f > 0 {
        std::process::exit(1);
    }
}
