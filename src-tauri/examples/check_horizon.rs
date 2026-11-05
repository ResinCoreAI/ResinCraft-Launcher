//! Live, headless check of Horizon's registry as a content source: reads the real index, makes a Horizon instance
//! in a scratch launcher folder and installs a real content mod, resource pack and shader pack into it through the
//! engine's own calls, turns the mod off and on, and installs a modpack. The registry lists no modpack yet, so the
//! pack is one made here that pins two of the registry's real content mods, with overrides. Prints what landed on
//! disk.
//!
//!   cargo run --example check_horizon [scratch folder]     (default: C:\zgc-target\horizon-live, or the temp folder)

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{anyhow, bail, Result};
use resincraft_lib::content::horizon::{self, Kind, Target};
use resincraft_lib::content::{details, index, modrinth, sources};
use resincraft_lib::core::types::{ContentKind, ContentSource, LoaderType, NewInstance, SearchQuery};
use resincraft_lib::core::{ctx, events, instances, paths};

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

fn query(kind: ContentKind, mc: &str) -> SearchQuery {
    serde_json::from_value(serde_json::json!({ "query": "", "kind": kind, "source": ContentSource::Horizon, "gameVersion": mc, "sort": "downloads" })).unwrap()
}

fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
    let Ok(list) = std::fs::read_dir(dir) else { return };
    for e in list.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, base, out);
        } else {
            out.push(format!("{} ({} bytes)", p.strip_prefix(base).unwrap().display(), e.metadata().map(|m| m.len()).unwrap_or(0)));
        }
    }
}

fn write_pack(path: &Path, manifest: &str, files: &[(&str, &[u8])]) -> Result<()> {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path)?);
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("horizon-modpack.toml", options)?;
    zip.write_all(manifest.as_bytes())?;
    for (name, bytes) in files {
        zip.start_file(*name, options)?;
        zip.write_all(bytes)?;
    }
    zip.finish()?;
    Ok(())
}

#[tokio::main]
async fn main() {
    let root = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        let c = PathBuf::from(r"C:\zgc-target");
        if c.is_dir() { c.join("horizon-live") } else { std::env::temp_dir().join("resincraft-horizon-live") }
    });
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    println!("scratch launcher folder: {}", root.display());
    ctx::init(root.clone(), None);
    events::set_sink(|channel, payload| {
        if channel == "task" && payload["state"] == "error" {
            println!("     [task failed] {}: {}", payload["title"].as_str().unwrap_or(""), payload["error"].as_str().unwrap_or(""));
        }
    });

    let Some(index) = report("read the live index", horizon::load().await) else { return finish() };
    println!("     schema {}, generated {}, {} projects, {} revoked", index.schema, index.generated, index.projects.len(), index.revoked.len());

    let new = NewInstance { name: "check-horizon".into(), mc_version: String::new(), loader: LoaderType::Horizon, loader_version: None, color: None, group: None };
    let Some(inst) = report("make a Horizon instance", instances::create(new, |_| {}).await) else { return finish() };
    let mc = inst.mc_version.clone();
    println!("     {} plays Minecraft {mc}", inst.id);
    let game = paths::game_dir(&inst.id);
    std::fs::create_dir_all(&game).unwrap();
    std::fs::write(game.join(horizon::MODS_FILE), "mod.someone_else:true\n").unwrap();

    // One of each installable kind: the most downloaded with a version for this Minecraft.
    for kind in [ContentKind::Mod, ContentKind::Resourcepack, ContentKind::Shader] {
        let hits = report(&format!("search {kind:?}"), sources::search(&query(kind, &mc)).await);
        let Some(hit) = hits.and_then(|r| {
            println!("     {} results", r.total);
            r.hits.into_iter().find(|h| h.horizon.as_ref().is_some_and(|f| !f.native && f.revoked.is_none() && f.games.contains(&mc)))
        }) else {
            report::<()>(&format!("find an installable {kind:?}"), Err(anyhow!("none")));
            continue;
        };
        report(&format!("details of {}", hit.id), details::details(&hit.id).await.map(|d| println!("     licence {:?}, links {:?}, {} bytes of description", d.license, d.links.iter().map(|l| &l.kind).collect::<Vec<_>>(), d.body.len())));
        report(&format!("versions of {}", hit.id), sources::versions(&hit.id, Some(&inst.id)).await.map(|v| println!("     {:?}", v.iter().map(|v| &v.id).collect::<Vec<_>>())));
        report(&format!("install {} ({kind:?})", hit.id), sources::install(&inst.id, &hit.id, kind, None).await);
    }

    // Native mods and revoked files are refused.
    let native = index.projects.iter().find(|p| p.kind() == Kind::NativeMod).unwrap();
    let r = sources::install(&inst.id, &native.pid(), ContentKind::Mod, None).await;
    report("a native mod is refused", match r {
        Ok(()) => Err(anyhow!("it installed")),
        Err(e) => {
            println!("     {e}");
            Ok(())
        }
    });
    if let Some(rv) = index.revoked.iter().find(|r| r.sha256.is_some()) {
        let p = index.project(&rv.id).unwrap();
        let v = p.versions.iter().find(|v| Some(&v.file.sha256) == rv.sha256.as_ref()).unwrap();
        report("a revoked file is refused", match horizon::check_file(&index, p, v) {
            Ok(()) => Err(anyhow!("it was allowed")),
            Err(e) => {
                println!("     {e}");
                Ok(())
            }
        });
    }

    // The Content tab: list, turn off and on, as the UI does.
    let mods = report("list the instance's Horizon mods", modrinth::list_content(&inst.id, ContentKind::Mod).await).unwrap_or_default();
    for m in &mods {
        println!("     {} · {:?} {:?} · on {}", m.filename, m.title, m.version_number, m.enabled);
    }
    if let Some(m) = mods.first() {
        report("turn a mod off", async {
            modrinth::toggle_content(&inst.id, ContentKind::Mod, &m.filename).await?;
            let after = modrinth::list_content(&inst.id, ContentKind::Mod).await?;
            if after.iter().any(|x| x.filename == m.filename && x.enabled) {
                bail!("still on");
            }
            println!("     {}", std::fs::read_to_string(game.join(horizon::MODS_FILE))?.trim().replace('\n', " | "));
            modrinth::toggle_content(&inst.id, ContentKind::Mod, &m.filename).await
        }
        .await);
    }
    for kind in [ContentKind::Resourcepack, ContentKind::Shader] {
        let list = modrinth::list_content(&inst.id, kind).await.unwrap_or_default();
        println!("     {kind:?}: {:?}", list.iter().map(|i| (&i.filename, &i.title)).collect::<Vec<_>>());
    }

    // A modpack pinning two real content mods (the second off), with every kind of override.
    let content: Vec<&horizon::Project> = index.projects.iter().filter(|p| p.kind() == Kind::Mod && index.installable(p, &mc).is_some()).take(2).collect();
    let mut manifest = format!("format = 1\nid = \"check_pack\"\nname = \"Check pack\"\nversion = \"1.0.0\"\ngame = \"{mc}\"\n");
    for (i, p) in content.iter().enumerate() {
        let v = index.installable(p, &mc).unwrap();
        manifest += &format!("\n[[mods]]\nid = \"{}\"\nversion = \"{}\"\nsha256 = \"{}\"\non = {}\n", p.id, v.version, v.file.sha256, i == 0);
    }
    let storage = format!("overrides/horizon-mods/.horizon-mod-data/{}/settings.txt", content[0].id);
    let pack = root.join("check_pack.hpack");
    write_pack(&pack, &manifest, &[("overrides/options.txt", b"renderDistance:8\n"), ("overrides/resourcepacks/Check Look.zip", b"PK"), (&storage, b"corner=top-left")]).unwrap();
    let to = Target { game: &game, mc: &mc, name: &inst.name };
    let mut idx = index::load_index(&inst.id).await;
    report("install a modpack", horizon::install_pack_file(&index, &to, None, &mut idx, "Check pack", &pack).await);
    let bad = root.join("bad.hpack");
    write_pack(&bad, &manifest, &[("overrides/../evil.txt", b"x")]).unwrap();
    report("a pack with a path it may not hold is refused", match horizon::install_pack_file(&index, &to, None, &mut idx, "Bad pack", &bad).await {
        Ok(()) => Err(anyhow!("it installed")),
        Err(e) => {
            println!("     {e}");
            Ok(())
        }
    });
    index::save_index(&inst.id, &idx).await.unwrap();
    let text = std::fs::read_to_string(game.join(horizon::MODS_FILE)).unwrap_or_default();
    report("horizon-mods.txt keeps other lines and the pack's on/off", {
        let ok = text.contains("mod.someone_else:true") && horizon::mod_on(&text, &content[0].id) && !horizon::mod_on(&text, &content[1].id);
        if ok { Ok(()) } else { Err(anyhow!("{text}")) }
    });

    let mut files = Vec::new();
    walk(&game, &game, &mut files);
    files.sort();
    println!("\ngame folder {}:\n  {}", game.display(), files.join("\n  "));
    println!("\n{}:\n{text}", horizon::MODS_FILE);
    finish()
}

fn finish() {
    let (p, f) = (PASSED.load(Ordering::Relaxed), FAILED.load(Ordering::Relaxed));
    println!("\n{p} passed, {f} failed");
    if f > 0 {
        std::process::exit(1);
    }
}
