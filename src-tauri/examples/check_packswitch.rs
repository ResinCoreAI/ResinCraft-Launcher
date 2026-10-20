//! Live check of switching a modpack instance's version (content/packswitch.rs) on real packs: installs an older
//! version, makes the changes a player would, switches up to the newest version and back down, and checks each
//! time that the pack's files are the new version's while the player's files and changes survive. Also cancels a
//! switch halfway and checks the instance is exactly as before. Removes the instances it creates.
//!
//!   cargo run --example check_packswitch                 (Modrinth; CurseForge too when a key is configured)
//!   cargo run --example check_packswitch -- <project id> <old version id> <new version id>

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use anyhow::{bail, Result};
use resincraft_lib::content::{packswitch, sources};
use resincraft_lib::core::types::{Instance, NewServer, ServerSource};
use resincraft_lib::server::{create, packs, store};
use resincraft_lib::core::{ctx, events, instances, paths, settings};
use serde_json::Value;

static PASSED: AtomicUsize = AtomicUsize::new(0);
static FAILED: AtomicUsize = AtomicUsize::new(0);
/// While set, the switch task is cancelled once it starts downloading.
static CANCEL_ON_DOWNLOAD: Mutex<bool> = Mutex::new(false);

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

fn sha1_of(p: &Path) -> String {
    use sha1::Digest;
    hex::encode(sha1::Sha1::digest(std::fs::read(p).unwrap_or_default()))
}

/// Every file of the game directory with its SHA-1.
fn tree(dir: &Path) -> BTreeMap<String, String> {
    fn go(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                go(root, &p, out);
            } else {
                out.insert(packswitch::norm(&p.strip_prefix(root).unwrap().to_string_lossy()), sha1_of(&p));
            }
        }
    }
    let mut out = BTreeMap::new();
    go(dir, dir, &mut out);
    out
}

/// The pack's own files per the record (path → "sha1:…").
fn record(id: &str) -> BTreeMap<String, String> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(paths::instance_dir(id).join("modpack-files.json")).unwrap_or_default()).unwrap_or(Value::Null);
    v["files"].as_object().map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect()).unwrap_or_default()
}

struct PlayerChanges {
    config: Option<String>,
    disabled: Option<String>,
}

/// Does what a player does between versions: adds a mod and a world, edits options.txt and a pack config,
/// switches a pack mod off.
fn play(inst: &Instance) -> PlayerChanges {
    let game = paths::game_dir(&inst.id);
    let rec = record(&inst.id);
    std::fs::write(game.join("mods").join("players-own-mod.jar"), b"not really a jar").unwrap();
    std::fs::create_dir_all(game.join("saves").join("My World")).unwrap();
    std::fs::write(game.join("saves").join("My World").join("level.dat"), b"my world").unwrap();
    std::fs::write(game.join("options.txt"), b"key_key.jump:key.keyboard.k\n").unwrap();
    let config = rec.keys().find(|p| p.starts_with("config/") && game.join(p).is_file()).cloned();
    if let Some(c) = &config {
        let mut text = std::fs::read(game.join(c)).unwrap();
        text.extend_from_slice(b"\n# changed by the player\n");
        std::fs::write(game.join(c), text).unwrap();
    }
    let disabled = rec.keys().filter(|p| p.starts_with("mods/") && p.ends_with(".jar") && game.join(p).is_file()).nth(1).cloned();
    if let Some(d) = &disabled {
        std::fs::rename(game.join(d), game.join(format!("{d}.disabled"))).unwrap();
    }
    PlayerChanges { config, disabled }
}

/// After a switch to `version`: the instance names it, every pack file is as the pack ships it (or deliberately the
/// player's), and the player's files are untouched.
fn verify(inst_id: &str, version: &str, before: &BTreeMap<String, String>, out: &packswitch::PackSwitch) -> Result<()> {
    let inst = serde_json::from_str::<Value>(&std::fs::read_to_string(paths::instance_dir(inst_id).join("instance.json"))?)?;
    if inst["modpack"]["versionId"].as_str() != Some(version) {
        bail!("instance says version {:?}, wanted {version}", inst["modpack"]["versionId"]);
    }
    let game = paths::game_dir(inst_id);
    let now = tree(&game);
    let rec = record(inst_id);
    let pending: Vec<String> = inst["pendingDownloads"].as_array().map(|a| a.iter().map(|p| format!("{}/{}", p["folder"].as_str().unwrap_or(""), p["fileName"].as_str().unwrap_or(""))).collect()).unwrap_or_default();
    let mut wrong = Vec::new();
    for (p, h) in &rec {
        let want = h.strip_prefix("sha1:").unwrap_or("");
        let player = p == "options.txt" || p.starts_with("saves/") || out.kept.contains(p) || pending.contains(p);
        if player || want.is_empty() {
            continue;
        }
        let got = now.get(p).or_else(|| now.get(&format!("{p}.disabled")));
        if got.map(String::as_str) != Some(want) {
            wrong.push(p.clone());
        }
    }
    if !wrong.is_empty() {
        bail!("{} pack files aren't the new version's: {:?}", wrong.len(), &wrong[..wrong.len().min(5)]);
    }
    for p in ["mods/players-own-mod.jar", "saves/My World/level.dat", "options.txt"] {
        if now.get(p) != before.get(p) {
            bail!("the player's {p} changed");
        }
    }
    // Old pack files that aren't in the new version are gone (unless the player changed them).
    let stray: Vec<&String> = now.keys().filter(|p| p.starts_with("mods/") && !rec.contains_key(*p) && !rec.contains_key(p.trim_end_matches(".disabled")) && !before_is_players(p)).collect();
    if !stray.is_empty() {
        bail!("mods left over from the other version: {stray:?}");
    }
    println!(
        "     → {}: {} added, {} updated, {} removed, kept {:?}, backups {:?}, {} to download",
        out.version_name, out.added, out.updated, out.removed, out.kept, out.backups, out.pending
    );
    Ok(())
}

fn before_is_players(p: &str) -> bool {
    p == "mods/players-own-mod.jar" || p.ends_with(".bak")
}

async fn check_pack(label: &str, project: &str, old: &str, new: &str) {
    println!("--- {label}: {project} {old} ⇄ {new}");
    let Some(inst) = report(&format!("{label} install old version"), sources::install_modpack(project, Some(old.to_string())).await) else { return };
    let id = inst.id.clone();
    report(&format!("{label} install recorded the pack's files"), async {
        if record(&id).is_empty() {
            bail!("no modpack-files.json");
        }
        Ok(())
    }.await);
    let changes = play(&inst);
    let game = paths::game_dir(&id);

    // Cancel halfway: nothing may change.
    let before = tree(&game);
    let meta_before = std::fs::read_to_string(paths::instance_dir(&id).join("instance.json")).unwrap_or_default();
    *CANCEL_ON_DOWNLOAD.lock().unwrap() = true;
    let r = packswitch::switch_version(&id, new).await;
    *CANCEL_ON_DOWNLOAD.lock().unwrap() = false;
    report(&format!("{label} cancelled switch leaves the instance as it was"), async {
        match r {
            Ok(_) => bail!("the switch finished before it could be cancelled"),
            Err(e) if !events::is_cancelled(&e) => bail!("failed instead of cancelling: {e:#}"),
            Err(_) => {}
        }
        if tree(&game) != before {
            bail!("files changed");
        }
        if std::fs::read_to_string(paths::instance_dir(&id).join("instance.json")).unwrap_or_default() != meta_before {
            bail!("instance.json changed");
        }
        if paths::instance_dir(&id).join(".pack-switch").exists() {
            bail!("work folder left behind");
        }
        Ok(())
    }.await);

    for (dir, to) in [("up", new), ("down", old)] {
        let before = tree(&game);
        report(&format!("{label} switch {dir} to {to}"), async {
            let out = packswitch::switch_version(&id, to).await?;
            verify(&id, to, &before, &out)?;
            if let Some(c) = &changes.config {
                let now = tree(&game);
                let theirs = now.get(c) == before.get(c);
                let backed = now.get(&format!("{c}.bak")) == before.get(c);
                if !theirs && !backed {
                    bail!("the player's change to {c} was lost");
                }
            }
            if let Some(d) = &changes.disabled {
                let enabled = tree(&game).keys().any(|p| p == d);
                if enabled {
                    bail!("{d} was switched back on");
                }
            }
            Ok(())
        }.await);
    }
    let _ = tokio::fs::remove_dir_all(paths::instance_dir(&id)).await;
}

/// What a host does on a server: a world, settings, the player lists, an own mod, a changed pack config.
fn host(dir: &Path) -> Option<String> {
    std::fs::create_dir_all(dir.join("world")).unwrap();
    std::fs::write(dir.join("world").join("level.dat"), b"the host's world").unwrap();
    std::fs::write(dir.join("server.properties"), b"motd=Host's server\nlevel-name=world\n").unwrap();
    std::fs::write(dir.join("whitelist.json"), b"[{\"name\":\"Puirkle\"}]").unwrap();
    std::fs::create_dir_all(dir.join("mods")).unwrap();
    std::fs::write(dir.join("mods").join("hosts-own-mod.jar"), b"not really a jar").unwrap();
    let config = tree(dir).keys().find(|p| p.starts_with("config/")).cloned();
    if let Some(c) = &config {
        let mut text = std::fs::read(dir.join(c)).unwrap();
        text.extend_from_slice(b"\n# changed by the host\n");
        std::fs::write(dir.join(c), text).unwrap();
    }
    config
}

fn host_kept(dir: &Path, before: &BTreeMap<String, String>) -> Result<()> {
    let now = tree(dir);
    for p in ["world/level.dat", "server.properties", "whitelist.json", "mods/hosts-own-mod.jar"] {
        if now.get(p) != before.get(p) {
            bail!("the host's {p} changed");
        }
    }
    Ok(())
}

fn new_server(name: &str, source: ServerSource) -> NewServer {
    NewServer { name: name.into(), kind: None, mc_version: None, loader_version: None, memory_mb: Some(1024), port: None, source: Some(source) }
}

/// A server made from a modpack: switch its pack up and down, cancel one switch.
async fn check_server_pack(project: &str, old: &str, new: &str) {
    println!("--- server from {project} {old} ⇄ {new}");
    let src = ServerSource::Modpack { project_id: project.into(), version_id: Some(old.into()) };
    let Some(created) = report("server create from old version", create::create(new_server("Switch test server", src)).await) else { return };
    let id = created.server.meta.id.clone();
    let dir = store::dir(&id).unwrap();
    let config = host(&dir);

    let before = tree(&dir);
    *CANCEL_ON_DOWNLOAD.lock().unwrap() = true;
    let r = packs::switch_pack(&id, new).await;
    *CANCEL_ON_DOWNLOAD.lock().unwrap() = false;
    report("server cancelled switch leaves it as it was", async {
        match r {
            Ok(_) => println!("     (finished before it could be cancelled)"),
            Err(e) if !events::is_cancelled(&e) => bail!("failed instead of cancelling: {e:#}"),
            Err(_) => {
                if tree(&dir) != before {
                    bail!("files changed");
                }
            }
        }
        Ok(())
    }.await);
    if store::get(&id).await.ok().and_then(|m| m.modpack).and_then(|m| m.version_id).as_deref() != Some(old) {
        report::<()>("server still on the old version", Err(anyhow::anyhow!("version changed")));
    }

    for (dir_name, to) in [("up", new), ("down", old)] {
        let before = tree(&dir);
        report(&format!("server switch {dir_name} to {to}"), async {
            let out = packs::switch_pack(&id, to).await?;
            let meta = store::get(&id).await?;
            if meta.modpack.and_then(|m| m.version_id).as_deref() != Some(to) {
                bail!("server.json doesn't name the new version");
            }
            host_kept(&dir, &before)?;
            let now = tree(&dir);
            let mut wrong = Vec::new();
            for (p, h) in record_at(&dir) {
                let want = h.strip_prefix("sha1:").unwrap_or("");
                if !want.is_empty() && !out.kept.contains(&p) && now.get(&p).map(String::as_str) != Some(want) {
                    wrong.push(p);
                }
            }
            if !wrong.is_empty() {
                bail!("{} server files aren't the new version's: {:?}", wrong.len(), &wrong[..wrong.len().min(5)]);
            }
            if let Some(c) = &config {
                if now.get(c) != before.get(c) && now.get(&format!("{c}.bak")) != before.get(c) {
                    bail!("the host's change to {c} was lost");
                }
            }
            if !meta.launch.jar.clone().or(meta.launch.args_file.clone()).is_some_and(|f| dir.join(f).exists()) {
                bail!("the server software isn't installed");
            }
            println!("     → {} {}: {} added, {} updated, {} removed, dropped {:?}", out.version_name, meta.mc_version, out.added, out.updated, out.removed, out.dropped);
            Ok(())
        }.await);
    }
    let _ = tokio::fs::remove_dir_all(&dir).await;
}

fn record_at(dir: &Path) -> BTreeMap<String, String> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("modpack-files.json")).unwrap_or_default()).unwrap_or(Value::Null);
    v["files"].as_object().map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect()).unwrap_or_default()
}

/// A server made from an instance: nothing to sync at first; after the instance changes, sync, then nothing again;
/// once the instance is deleted, the server says so.
async fn check_server_sync(project: &str, old: &str, new: &str) {
    println!("--- server synced from an instance");
    let Some(inst) = report("sync: install the instance", sources::install_modpack(project, Some(old.to_string())).await) else { return };
    let src = ServerSource::Instance { instance_id: inst.id.clone() };
    let Some(created) = report("sync: server from the instance", create::create(new_server("Sync test server", src)).await) else { return };
    let id = created.server.meta.id.clone();
    let dir = store::dir(&id).unwrap();
    host(&dir);

    report("sync: remembers its instance, nothing to sync yet", async {
        let o = packs::origin(&id).await?;
        if o.instance_id.as_deref() != Some(inst.id.as_str()) || o.sync_available {
            bail!("origin {o:?}");
        }
        Ok(())
    }.await);
    report("sync: instance changes are noticed and synced", async {
        packswitch::switch_version(&inst.id, new).await?;
        if !packs::origin(&id).await?.sync_available {
            bail!("no sync available after the instance changed");
        }
        let p = packs::sync_preview(&id).await?;
        println!("     preview: +{} ~{} -{} kept {:?} mc {:?} dropped {}", p.added.len(), p.updated.len(), p.removed.len(), p.kept, p.minecraft, p.dropped.len());
        if p.is_empty() {
            bail!("the preview shows no changes");
        }
        let before = tree(&dir);
        packs::sync_from_instance(&id).await?;
        host_kept(&dir, &before)?;
        let again = packs::sync_preview(&id).await?;
        if !again.added.is_empty() || !again.removed.is_empty() || again.minecraft.is_some() {
            bail!("still differs after syncing: {again:?}");
        }
        if packs::origin(&id).await?.sync_available {
            bail!("still says sync available");
        }
        let meta = store::get(&id).await?;
        let inst_now = instances::get(&inst.id).await?;
        if meta.mc_version != inst_now.mc_version {
            bail!("server is on {}, the instance on {}", meta.mc_version, inst_now.mc_version);
        }
        Ok(())
    }.await);
    let _ = tokio::fs::remove_dir_all(paths::instance_dir(&inst.id)).await;
    report("sync: a deleted instance is reported", async {
        let o = packs::origin(&id).await?;
        if !o.instance_gone || !packs::sync_preview(&id).await?.missing {
            bail!("origin {o:?}");
        }
        Ok(())
    }.await);
    let _ = tokio::fs::remove_dir_all(&dir).await;
}

#[tokio::main]
async fn main() {
    let data = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(".dev-data");
    let data = std::fs::canonicalize(&data).unwrap_or(data);
    ctx::init(data, None);
    events::set_sink(|channel, payload| {
        if channel != "task" {
            return;
        }
        if payload["state"] == "error" {
            println!("     [task failed] {}: {}", payload["title"].as_str().unwrap_or(""), payload["error"].as_str().unwrap_or(""));
        }
        let downloading = payload["detail"].as_str().is_some_and(|d| d.contains("Downloading modpack files"));
        if downloading && payload["cancellable"] == true && *CANCEL_ON_DOWNLOAD.lock().unwrap() {
            if let Some(id) = payload["id"].as_str() {
                events::cancel(id);
            }
        }
    });

    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let only_servers = args.first().is_some_and(|a| a == "--servers");
    if only_servers {
        args.remove(0);
    }
    if args.len() == 3 {
        check_pack("given", &args[0], &args[1], &args[2]).await;
    } else {
        // Two releases of Fabulously Optimized a few versions apart (configs, overrides and renamed mods change).
        let list = sources::versions("1KVo5zza", None).await.unwrap_or_default();
        let rel: Vec<_> = list.iter().filter(|v| v.kind == "release").collect();
        if rel.len() > 4 {
            if !only_servers {
                check_pack("modrinth", "1KVo5zza", &rel[4].id, &rel[0].id).await;
            }
            check_server_pack("1KVo5zza", &rel[4].id, &rel[0].id).await;
            check_server_sync("1KVo5zza", &rel[4].id, &rel[0].id).await;
        } else {
            report::<()>("modrinth versions", Err(anyhow::anyhow!("couldn't list versions")));
        }
        if !only_servers && !settings::curseforge_key().await.trim().is_empty() {
            let list = sources::versions("cf:396246", None).await.unwrap_or_default();
            let rel: Vec<_> = list.iter().filter(|v| v.kind == "release").collect();
            if rel.len() > 4 {
                check_pack("curseforge", "cf:396246", &rel[4].id, &rel[0].id).await;
            } else {
                report::<()>("curseforge versions", Err(anyhow::anyhow!("couldn't list versions")));
            }
        } else {
            println!("SKIP curseforge (no API key)");
        }
    }
    let _ = instances::list().await;
    println!("\n{} passed, {} failed", PASSED.load(Ordering::Relaxed), FAILED.load(Ordering::Relaxed));
    if FAILED.load(Ordering::Relaxed) > 0 {
        std::process::exit(1);
    }
}
