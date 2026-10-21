//! Headless end-to-end check of server hosting (no window needed):
//!   cargo run --example check_server -- [type[@version]…] [--mc 1.21.1] [--keep] [--upnp] [--tunnel auto|playit|relay] [--relay host[:port]] [--relay-secret s] [--managed-java] [--update-test] [--geyser] [--port 25590] [--verbose]
//! Types: vanilla paper purpur folia spigot leaf fabric quilt forge neoforge arclight bedrock, or
//! modpack:<Modrinth id or slug, cf:<CurseForge id> or ftb:<id>> (default: vanilla paper fabric). "type@version"
//! picks that type's Minecraft version (Bedrock: its server version; without one, the newest). Classic Forge:
//! forge@1.12.2 forge@1.7.10 modpack:cf:285109 (RLCraft); these also check that Forge Mod Loader came up.
//! "type@version#build" picks a loader build too (forge@1.12.2#14.23.5.2847: the older installer format). Each server is created, given a plugin
//! (plugin servers and hybrids: LuckPerms from Modrinth) or a server-side mod (Fabric/Quilt: Lithium), started
//! until it's ready, asked `list`, backed up and stopped with `stop`. --geyser also lets Bedrock players join
//! (Geyser + Floodgate) and checks Geyser starts. Servers are deleted afterwards unless --keep is given.

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
    /// Plugin servers: start with an older ViaVersion from Modrinth and update it while the server runs.
    update_test: bool,
    /// Install Geyser and Floodgate ("Bedrock players can join") and check Geyser starts.
    geyser: bool,
    /// Plugin servers and hybrids: also install from a Hangar link and from GitHub; hybrids: a mod in their mods.
    sources: bool,
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
    let opts = Opts { tunnel, relay, relay_secret, keep: flag(&mut argv, "--keep"), upnp: flag(&mut argv, "--upnp"), verbose: flag(&mut argv, "--verbose"), managed_java: flag(&mut argv, "--managed-java"), update_test: flag(&mut argv, "--update-test"), geyser: flag(&mut argv, "--geyser"), sources: flag(&mut argv, "--sources"), mc: {
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
    let (t, version) = t.split_once('@').map(|(a, b)| (a, Some(b.to_string()))).unwrap_or((t, None));
    // "forge@1.12.2#14.23.5.2847": a specific loader build.
    let (version, loader_version) = match version.as_deref().and_then(|v| v.split_once('#')) {
        Some((v, l)) => (Some(v.to_string()), Some(l.to_string())),
        None => (version, None),
    };
    let (kind, source) = match t.strip_prefix("modpack:") {
        Some(pack) => (None, Some(ServerSource::Modpack { project_id: pack.to_string(), version_id: None })),
        None => (Some(serde_json::from_value::<ServerKind>(json!(t)).map_err(|_| anyhow!("unknown server type {t}"))?), None),
    };
    let mc = match (version, kind) {
        (Some(v), _) => v,
        // Bedrock's versions are its own: take the newest.
        (None, Some(k)) if k.bedrock() => resincraft_lib::server::installers::software_versions(k).await?.first().cloned().ok_or_else(|| anyhow!("no Bedrock versions"))?,
        _ => opts.mc.clone(),
    };
    if let Some(k) = kind {
        let builds = resincraft_lib::server::installers::builds(k, &mc).await?;
        if !builds.is_empty() {
            println!("  builds: {}", builds.iter().map(|b| format!("{}{}", b.label, if b.stable { "" } else { " (dev)" })).collect::<Vec<_>>().join(", "));
        }
    }
    let req = NewServer {
        name: format!("check-server {t}"),
        kind,
        mc_version: source.is_none().then(|| mc.clone()),
        loader_version,
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
    let bedrock = meta.kind.bedrock();
    // One add-on per kind of server, from Modrinth (hybrids: a plugin).
    let addon = match meta.kind {
        k if k.plugins() || k.hybrid() => Some("luckperms"),
        ServerKind::Fabric | ServerKind::Quilt => Some("lithium"),
        _ => None,
    };
    if let (Some(a), None) = (addon, &meta.modpack) {
        plugins::install(id, a, None, None).await?;
        let list = plugins::list(id, None).await?;
        let names: Vec<String> = list.iter().map(|c| c.item.title.clone().unwrap_or_else(|| c.item.filename.clone())).collect();
        println!("  installed add-ons: {}", names.join(", "));
        if list.is_empty() {
            bail!("{a} wasn't installed");
        }
    }
    if opts.sources && (meta.kind.plugins() || meta.kind.hybrid()) {
        plugins::install_link(id, "https://hangar.papermc.io/HelpChat/PlaceholderAPI", Some("plugins")).await?;
        plugins::install(id, "gh:EssentialsX/Essentials", None, Some("plugins")).await?;
        let list = plugins::list(id, Some("plugins")).await?;
        let shown: Vec<String> = list.iter().map(|a| format!("{} [{}]", a.item.title.clone().unwrap_or_default(), a.source.unwrap_or("?"))).collect();
        println!("  plugins: {}", shown.join(", "));
        for want in ["hangar", "github"] {
            if !list.iter().any(|a| a.source == Some(want)) {
                bail!("nothing from {want} in the plugins");
            }
        }
        if meta.kind.hybrid() {
            plugins::install(id, "spark", None, Some("mods")).await?;
            let mods = plugins::list(id, Some("mods")).await?;
            println!("  mods: {}", mods.iter().map(|a| a.item.filename.clone()).collect::<Vec<_>>().join(", "));
            if mods.iter().any(|a| a.item.filename.to_lowercase().contains("luckperms")) || mods.is_empty() {
                bail!("the plugins and mods areas got mixed up");
            }
        }
    }
    if opts.geyser {
        plugins::bedrock_enable(id).await?;
        let status = plugins::bedrock_status(id).await?;
        let names: Vec<String> = plugins::list(id, Some("plugins")).await?.iter().map(|c| c.item.filename.clone()).collect();
        println!("  Bedrock players: {status} ({})", names.join(", "));
        if status.get("geyser") != Some(&json!(true)) || status.get("floodgate") != Some(&json!(true)) {
            bail!("Geyser and Floodgate weren't installed");
        }
    }
    let mut old_via = None;
    if opts.update_test && meta.kind.plugins() {
        let versions = plugins::versions(id, "viaversion", None).await?;
        let compatible: Vec<_> = versions.iter().filter(|v| v.compatible && v.version.kind == "release").collect();
        let old = compatible.get(3).or(compatible.last()).ok_or_else(|| anyhow!("no compatible ViaVersion"))?;
        plugins::install(id, "viaversion", Some(old.version.id.clone()), None).await?;
        println!("  installed ViaVersion {} ({} compatible releases, newest {})", old.version.version_number, compatible.len(), compatible[0].version.version_number);
        old_via = Some((old.version.version_number.clone(), compatible[0].version.version_number.clone()));
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
    if !bedrock {
        println!("  java {}", runtime::launch_args(&store::get(id).await?)?.join(" "));
    }
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
    let done = runtime::log(id).into_iter().find(|l| l.text.contains("Done (") || l.text.contains("Server started.")).map(|l| l.text).unwrap_or_default();
    println!("  {done}");
    if meta.loader == Some(resincraft_lib::core::types::LoaderType::Forge) && resincraft_lib::game::loaders::is_legacy_mc(&meta.mc_version) {
        // Forge 1.7.10–1.12.2 runs from its universal jar: make sure that was Forge, not plain vanilla.
        // Big packs push it out of the console buffer, so look in the log files too.
        let files = ["logs/fml-server-latest.log", "logs/latest.log"].map(|f| std::fs::read_to_string(store::dir(id).unwrap().join(f)).unwrap_or_default());
        let lines = runtime::log(id).into_iter().map(|l| l.text).chain(files.iter().flat_map(|f| f.lines().map(str::to_string)));
        let fml = lines.into_iter().find(|l| l.contains("Forge Mod Loader version"));
        println!("  {}", fml.as_deref().ok_or_else(|| anyhow!("Forge Mod Loader never started"))?);
    }
    if opts.geyser {
        // Geyser starts once the Java server is up.
        let geyser_re = Regex::new(r"(?i)started geyser|geyser.*(?:listening|started) on")?;
        let started = runtime::log(id).into_iter().find(|l| geyser_re.is_match(&l.text)).map(|l| l.text);
        let started = match started {
            Some(l) => l,
            None => runtime::wait_line(id, &geyser_re, Duration::from_secs(90)).await?,
        };
        println!("  {started}");
    }

    // Java: "There are 0 of a max of 20 players online"; Bedrock: "There are 0/10 players online".
    let list_re = Regex::new(r"There are \d+(?: (?:of a max(?:imum)? of|out of maximum) |/)\d+ players online")?;
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

    let mut expect_via = None;
    if let Some((old, newest)) = &old_via {
        let checked = plugins::check_updates(id, None).await?;
        let via = checked.iter().find(|a| a.item.title.as_deref() == Some("ViaVersion")).ok_or_else(|| anyhow!("ViaVersion missing"))?;
        let up = via.item.update.clone().ok_or_else(|| anyhow!("no update found for ViaVersion {old}"))?;
        println!("  update check: ViaVersion {old} → {} ({})", up.version_number, via.update_url.clone().unwrap_or_default());
        if &up.version_number != newest {
            bail!("expected the newest compatible version {newest}, got {}", up.version_number);
        }
        let queued = plugins::change_version(id, &via.item.filename, &up.version_id, None).await?;
        let pending = plugins::list(id, None).await?.into_iter().find(|a| a.item.title.as_deref() == Some("ViaVersion")).and_then(|a| a.pending);
        println!("  update queued while running: {queued} (pending: {pending:?})");
        if !queued {
            bail!("a running server's jar was replaced directly");
        }
        expect_via = Some(up.version_number);
    }

    // Bedrock worlds are backed up while the server is off (below).
    if !bedrock {
        let zip = runtime::backup(id).await?;
        let size = std::fs::metadata(&zip).map(|m| m.len()).unwrap_or(0);
        println!("  backup {zip} ({} KB)", size / 1024);
        if size == 0 {
            bail!("the backup is empty");
        }
    }

    runtime::stop(id)?;
    let end = runtime::wait_state(id, &["stopped", "crashed"], Duration::from_secs(120)).await?;
    if end != "stopped" {
        print_tail(id, 25);
        bail!("the server {end} while stopping");
    }
    let tail = runtime::log(id).last().map(|l| l.text.clone()).unwrap_or_default();
    println!("  stopped cleanly ({tail})");
    if bedrock {
        let zip = runtime::backup(id).await?;
        let size = std::fs::metadata(&zip).map(|m| m.len()).unwrap_or(0);
        println!("  backup {zip} ({} KB)", size / 1024);
        if size == 0 {
            bail!("the backup is empty");
        }
    }
    if let Some(want) = expect_via {
        let applied: Vec<String> = runtime::log(id).iter().filter(|l| l.text.starts_with("Updated ")).map(|l| l.text.clone()).collect();
        let list = plugins::list(id, None).await?;
        let vias: Vec<_> = list.iter().filter(|a| a.item.title.as_deref() == Some("ViaVersion")).collect();
        let now = vias.first().and_then(|a| a.item.version_number.clone()).unwrap_or_default();
        println!("  after stop: {applied:?}; ViaVersion is {now} ({} jar(s), pending {:?})", vias.len(), vias.first().and_then(|a| a.pending));
        if now != want || vias.len() != 1 || vias[0].pending.is_some() {
            bail!("the queued ViaVersion update wasn't applied cleanly");
        }
    }
    Ok(())
}

fn print_tail(id: &str, n: usize) {
    let log = runtime::log(id);
    for l in log.iter().skip(log.len().saturating_sub(n)) {
        println!("    | {}", l.text);
    }
}
