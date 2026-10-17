//! Headless end-to-end check of server hosting (no window needed):
//!   cargo run --example check_server -- [type…] [--mc 1.21.1] [--keep] [--upnp] [--tunnel auto|playit|relay] [--relay host[:port]] [--relay-secret s] [--managed-java] [--port 25590] [--verbose]
//! Types: vanilla paper purpur folia spigot fabric quilt forge neoforge, or modpack:<Modrinth id or slug>
//! (default: vanilla paper fabric). Each server is created, given a plugin (Paper/Purpur: LuckPerms from
//! Modrinth) or a server-side mod (Fabric/Quilt: Lithium), started until "Done (…)! For help, type "help"",
//! asked `list`, backed up and stopped with `stop`. Servers are deleted afterwards unless --keep is given.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use regex::Regex;
use resincraft_lib::core::types::{NewServer, ServerKind, ServerSource};
use resincraft_lib::core::{ctx, events, paths};
use resincraft_lib::server::{create, plugins, runtime, store};
use serde_json::{json, Value};

#[tokio::main]
async fn main() {
    let code = match run().await {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(e) => {
            eprintln!("CHECK FAILED: {e:#}");
            1
        }
    };
    std::process::exit(code);
}

struct Opts {
    mc: String,
    keep: bool,
    upnp: bool,
    verbose: bool,
    managed_java: bool,
    /// With --upnp: how to share the server (TunnelMode), and a bore relay instead of bore.pub.
    tunnel: String,
    relay: Option<String>,
    relay_secret: Option<String>,
}

async fn run() -> Result<bool> {
    let data_dir = std::env::var_os("RESIN_DATA_DIR").map(Into::into).unwrap_or_else(|| paths::default_data_dir(None));
    std::fs::create_dir_all(&data_dir)?;
    ctx::init(data_dir, None);

    let mut argv: Vec<String> = std::env::args().skip(1).collect();
    let flag = |argv: &mut Vec<String>, name: &str| {
        let had = argv.iter().any(|a| a == name);
        argv.retain(|a| a != name);
        had
    };
    let mut value = |name: &str| -> Result<Option<String>> {
        let Some(i) = argv.iter().position(|a| a == name) else { return Ok(None) };
        let v = argv.get(i + 1).cloned().ok_or_else(|| anyhow!("{name} needs a value"))?;
        argv.drain(i..=i + 1);
        Ok(Some(v))
    };
    let (tunnel, relay, relay_secret) = (value("--tunnel")?.unwrap_or_else(|| "auto".into()), value("--relay")?, value("--relay-secret")?);
    let opts = Opts { tunnel, relay, relay_secret, keep: flag(&mut argv, "--keep"), upnp: flag(&mut argv, "--upnp"), verbose: flag(&mut argv, "--verbose"), managed_java: flag(&mut argv, "--managed-java"), mc: {
        let mut mc = "1.21.1".to_string();
        if let Some(i) = argv.iter().position(|a| a == "--mc") {
            mc = argv.get(i + 1).cloned().ok_or_else(|| anyhow!("--mc needs a version"))?;
            argv.drain(i..=i + 1);
        }
        mc
    } };
    // --port N: first port to use (each type gets the next one), so parallel runs don't collide.
    let mut base_port: u16 = 25590;
    if let Some(i) = argv.iter().position(|a| a == "--port") {
        base_port = argv.get(i + 1).and_then(|p| p.parse().ok()).ok_or_else(|| anyhow!("--port needs a number"))?;
        argv.drain(i..=i + 1);
    }
    let types = if argv.is_empty() { vec!["vanilla".to_string(), "paper".into(), "fabric".into()] } else { argv };

    // Print task progress (deduplicated) and, with --verbose, every console line.
    let last = Arc::new(Mutex::new(String::new()));
    let verbose = opts.verbose;
    events::set_sink(move |channel, payload| match channel {
        "task" => {
            let line = format!(
                "  · {}{}",
                payload.get("title").and_then(Value::as_str).unwrap_or_default(),
                payload.get("detail").and_then(Value::as_str).map(|d| format!(": {d}")).unwrap_or_default()
            );
            let mut l = last.lock().unwrap();
            if *l != line && !line.contains('(') {
                println!("{line}");
                *l = line;
            }
        }
        "serverLog" if verbose => {
            for l in payload.get("lines").and_then(Value::as_array).cloned().unwrap_or_default() {
                println!("    | {}", l.get("text").and_then(Value::as_str).unwrap_or_default());
            }
        }
        _ => {}
    });

    let mut ok = true;
    for (i, t) in types.iter().enumerate() {
        let started = Instant::now();
        println!("== {t} (Minecraft {}) ==", opts.mc);
        match check(t, &opts, base_port + i as u16).await {
            Ok(()) => println!("PASS {t} ({}s)", started.elapsed().as_secs()),
            Err(e) => {
                ok = false;
                println!("FAIL {t}: {e:#}");
            }
        }
    }
    Ok(ok)
}

async fn check(t: &str, opts: &Opts, port: u16) -> Result<()> {
    let (kind, source) = match t.strip_prefix("modpack:") {
        Some(pack) => (None, Some(ServerSource::Modpack { project_id: pack.to_string(), version_id: None })),
        None => (Some(serde_json::from_value::<ServerKind>(json!(t)).map_err(|_| anyhow!("unknown server type {t}"))?), None),
    };
    let req = NewServer {
        name: format!("check-server {t}"),
        kind,
        mc_version: source.is_none().then(|| opts.mc.clone()),
        loader_version: None,
        memory_mb: Some(2048),
        port: Some(port),
        source,
    };
    let created = create::create(req).await?;
    let id = created.server.meta.id.clone();
    println!("  created {id}: {} {} {:?}", created.server.meta.kind.label(), created.server.meta.mc_version, created.server.meta.launch);
    if !created.dropped.is_empty() {
        println!("  left out client-only: {}", created.dropped.join(", "));
    }
    let result = exercise(&id, opts).await;
    if runtime::is_running(&id) {
        runtime::kill(&id);
        let _ = runtime::wait_state(&id, &["stopped", "crashed"], Duration::from_secs(20)).await;
    }
    if !opts.keep {
        let _ = tokio::fs::remove_dir_all(store::dir(&id)?).await;
    }
    result
}

async fn exercise(id: &str, opts: &Opts) -> Result<()> {
    let meta = store::get(id).await?;
    // One add-on per kind of server, from Modrinth.
    let addon = match meta.kind {
        ServerKind::Paper | ServerKind::Purpur | ServerKind::Spigot => Some("luckperms"),
        ServerKind::Fabric | ServerKind::Quilt => Some("lithium"),
        _ => None,
    };
    if let (Some(a), None) = (addon, &meta.modpack) {
        plugins::install(id, a, None).await?;
        let list = plugins::list(id).await?;
        let names: Vec<String> = list.iter().map(|c| c.title.clone().unwrap_or_else(|| c.filename.clone())).collect();
        println!("  installed add-ons: {}", names.join(", "));
        if list.is_empty() {
            bail!("{a} wasn't installed");
        }
    }
    store::update(id, json!({ "public": opts.upnp, "tunnel": opts.tunnel, "relayHost": opts.relay, "relaySecret": opts.relay_secret })).await?;
    if opts.managed_java {
        // The Temurin runtime the launcher downloads itself (reused when already there).
        let major = resincraft_lib::server::installers::required_java(&meta.mc_version).await?;
        let found = resincraft_lib::game::java::detect_java().await?.into_iter().find(|j| j.managed && j.major == major);
        let java = match found {
            Some(j) => j,
            None => {
                let task = events::Task::new(format!("Installing Java {major}"), None);
                let j = resincraft_lib::game::java::install_java(major, &task).await?;
                task.done();
                j
            }
        };
        println!("  managed Java {} at {}", java.version, java.path);
        store::update(id, json!({ "javaPath": java.path })).await?;
    }
    println!("  java {}", runtime::launch_args(&store::get(id).await?)?.join(" "));
    if runtime::start(id).await.is_ok() {
        bail!("the server started without the EULA being accepted");
    }
    store::accept_eula(id).await?;
    runtime::start(id).await?;
    let state = runtime::wait_state(id, &["running", "stopped", "crashed"], Duration::from_secs(900)).await?;
    if state != "running" {
        print_tail(id, 25);
        bail!("the server {state} before it was ready");
    }
    let done = runtime::log(id).into_iter().find(|l| l.text.contains("Done (")).map(|l| l.text).unwrap_or_default();
    println!("  {done}");

    let list_re = Regex::new(r"There are \d+ (?:of a max(?:imum)? of|out of maximum) \d+ players online")?;
    let waiter = runtime::wait_line(id, &list_re, Duration::from_secs(30));
    runtime::command(id, "list")?;
    println!("  list → {}", waiter.await?);
    if let Some(a) = addon {
        let enabled = runtime::log(id).into_iter().any(|l| l.text.to_lowercase().contains(&format!("enabling {a}")) || l.text.to_lowercase().contains(a));
        println!("  {a} mentioned in the console: {enabled}");
    }
    if opts.upnp {
        // Router (UPnP), playit.gg or the relay (--tunnel); then ping the server the way a friend's game would.
        // Linking playit.gg the first time waits for the browser, so that gets longer.
        let deadline = Instant::now() + Duration::from_secs(if opts.tunnel == "playit" { 16 * 60 } else { 40 });
        let mut claim_shown = false;
        while runtime::net(id).is_none_or(|n| n.state == "opening") && Instant::now() < deadline {
            if let Some(url) = runtime::net(id).and_then(|n| n.claim_url).filter(|_| !claim_shown) {
                claim_shown = true;
                println!("  link playit.gg in the browser: {url}");
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        let net = runtime::net(id).ok_or_else(|| anyhow!("internet access never started"))?;
        println!("  internet: {} via {:?} {} {}", net.state, net.via, net.address.clone().unwrap_or_default(), net.error.clone().unwrap_or_default());
        let address = net.address.ok_or_else(|| anyhow!("no public address"))?;
        let status = resincraft_lib::player::servers::ping_server(&address, Duration::from_secs(15)).await?;
        println!("  pinged {address}: \"{}\" {} ({}/{} players, {} ms)", status.motd, status.version, status.players.online, status.players.max, status.latency);
    }

    let zip = runtime::backup(id).await?;
    let size = std::fs::metadata(&zip).map(|m| m.len()).unwrap_or(0);
    println!("  backup {zip} ({} KB)", size / 1024);
    if size == 0 {
        bail!("the backup is empty");
    }

    runtime::stop(id)?;
    let end = runtime::wait_state(id, &["stopped", "crashed"], Duration::from_secs(120)).await?;
    if end != "stopped" {
        print_tail(id, 25);
        bail!("the server {end} while stopping");
    }
    let tail = runtime::log(id).last().map(|l| l.text.clone()).unwrap_or_default();
    println!("  stopped cleanly ({tail})");
    Ok(())
}

fn print_tail(id: &str, n: usize) {
    let log = runtime::log(id);
    for l in log.iter().skip(log.len().saturating_sub(n)) {
        println!("    | {}", l.text);
    }
}
