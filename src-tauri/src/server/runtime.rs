//! Running hosted servers: start (Java + the server's launch spec, or Bedrock's own program), the live console
//! (log lines out, commands in on stdin), players from the log, graceful stop (`stop`, then kill after a timeout),
//! the router port while it runs (UDP for Bedrock and Geyser), world backups and stopping everything when the
//! launcher quits.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use regex::Regex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{broadcast, mpsc};

use crate::core::events::{emit, Task};
use crate::core::text::{text_of, Text};
use crate::core::types::{LogLine, ResinNet, ServerEvent, ServerKind, ServerLogEvent, ServerMeta, ServerNet, TunnelMode};
use crate::game::java::hidden_command;
use crate::game::launch::{split_args, LogParser};
use crate::player::files::unique_name;

use super::{installers, network, playit, properties, relay, store, tunnel};

/// How long a server gets to save and shut down after `stop` before it's killed.
const STOP_TIMEOUT: Duration = Duration::from_secs(90);
/// Console lines kept per server (also after it stops, until the next start).
const LOG_KEEP: usize = 3000;

struct Proc {
    /// Distinguishes runs, so a stop timeout from an earlier run never kills a restarted server.
    run: u64,
    stdin: mpsc::UnboundedSender<String>,
    kill: mpsc::UnboundedSender<()>,
    state: &'static str,
    players: BTreeSet<String>,
    stopping: bool,
    net: Option<ServerNet>,
    lines: broadcast::Sender<String>,
    /// Geyser next to a Java server: where Bedrock players join, and the UDP port opened on the router for it.
    bedrock: Option<String>,
    bedrock_port: Option<u16>,
}

#[derive(Default)]
struct World {
    procs: HashMap<String, Proc>,
    logs: HashMap<String, VecDeque<LogLine>>,
    /// How each server last ended ("stopped" / "crashed"), for the list.
    last: HashMap<String, &'static str>,
    /// Open relay connections (see tunnel.rs).
    tunnels: HashMap<String, tokio::task::JoinHandle<()>>,
}

static WORLD: Mutex<Option<World>> = Mutex::new(None);
static RUNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn with<R>(f: impl FnOnce(&mut World) -> R) -> R {
    f(WORLD.lock().unwrap().get_or_insert_with(World::default))
}

pub fn is_running(id: &str) -> bool {
    with(|w| w.procs.contains_key(id))
}

pub fn running_ids() -> Vec<String> {
    with(|w| w.procs.keys().cloned().collect())
}

pub fn state(id: &str) -> &'static str {
    with(|w| w.procs.get(id).map(|p| p.state).or_else(|| w.last.get(id).copied()).unwrap_or("stopped"))
}

pub fn players(id: &str) -> Vec<String> {
    with(|w| w.procs.get(id).map(|p| p.players.iter().cloned().collect()).unwrap_or_default())
}

pub fn net(id: &str) -> Option<ServerNet> {
    with(|w| {
        let p = w.procs.get(id)?;
        let mut n = p.net.clone()?;
        n.bedrock_address = p.bedrock.clone();
        Some(n)
    })
}

/// The console so far (the current run, or the last one).
pub fn log(id: &str) -> Vec<LogLine> {
    with(|w| w.logs.get(id).map(|l| l.iter().cloned().collect()).unwrap_or_default())
}

fn emit_state(id: &str, code: Option<i32>) {
    let event = ServerEvent { server_id: id.to_string(), state: state(id), players: players(id), net: net(id), code };
    emit("server", event);
}

fn set_net(id: &str, n: Option<ServerNet>) {
    let open_at = |n: &Option<ServerNet>| n.as_ref().filter(|n| n.state == "open").map(|n| n.address.clone());
    let now = open_at(&n);
    let before = with(|w| {
        let p = w.procs.get_mut(id)?;
        let was = open_at(&p.net);
        p.net = n;
        Some(was)
    });
    emit_state(id, None);
    // Newly reachable, or at a new address (not every traffic update): re-check (and with Cloudflare, update)
    // the server's own domain for this address.
    if now.is_some() && before.flatten() != now {
        tokio::spawn(super::domain::on_open(id.to_string()));
    }
}

/// Whether the server's own domain points at its current address (see domain.rs).
pub fn set_domain_ok(id: &str, ok: Option<bool>) {
    let changed = with(|w| match w.procs.get_mut(id).and_then(|p| p.net.as_mut()) {
        Some(n) if n.domain_ok != ok => {
            n.domain_ok = ok;
            true
        }
        _ => false,
    });
    if changed {
        emit_state(id, None);
    }
}

// ---------- Log patterns ----------

struct Patterns {
    ansi: Regex,
    done: Regex,
    joined: Regex,
    left: Regex,
    list: Regex,
    /// Bedrock Dedicated Server: "Player connected: Name, xuid: …" (gamertags may have spaces).
    bedrock_joined: Regex,
    bedrock_left: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        ansi: Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b[()][A-Za-z0-9]").unwrap(),
        done: Regex::new(r#"Done \(\d+[.,]?\d*m?s\)! For help, type|INFO\] Server started\."#).unwrap(),
        joined: Regex::new(r"(?:\]|INFO)\]?: ([A-Za-z0-9_.]{1,17}) joined the game\s*$").unwrap(),
        left: Regex::new(r"(?:\]|INFO)\]?: ([A-Za-z0-9_.]{1,17}) left the game\s*$").unwrap(),
        list: Regex::new(r"There are \d+ (?:of a max(?:imum)? of|out of maximum) \d+ players online[.:]?\s*(.*)$").unwrap(),
        bedrock_joined: Regex::new(r"Player connected: (.+?), xuid").unwrap(),
        bedrock_left: Regex::new(r"Player disconnected: (.+?), xuid").unwrap(),
    })
}

/// Reacts to one console line: "Done" means the server is up; joins/leaves/`list` keep the player list.
fn on_line(id: &str, text: &str) -> bool {
    let p = patterns();
    let mut changed = false;
    let mut became_running = false;
    with(|w| {
        let Some(proc) = w.procs.get_mut(id) else { return };
        if proc.state == "starting" && p.done.is_match(text) {
            proc.state = "running";
            changed = true;
            became_running = true;
        }
        if let Some(c) = p.joined.captures(text).or_else(|| p.bedrock_joined.captures(text)) {
            changed |= proc.players.insert(c[1].to_string());
        } else if let Some(c) = p.left.captures(text).or_else(|| p.bedrock_left.captures(text)) {
            changed |= proc.players.remove(&c[1]);
        } else if let Some(c) = p.list.captures(text) {
            let names: BTreeSet<String> = c[1].split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            if names != proc.players {
                proc.players = names;
                changed = true;
            }
        }
    });
    if changed {
        emit_state(id, None);
    }
    became_running
}

// ---------- Start / stop ----------

/// Aikar's G1GC flags (https://mcflags.emc.gs), tuned for Minecraft servers. G1RSetUpdatingPauseIntervalMillis
/// is left out on purpose: newer JDKs reject it, and an unknown -XX option stops Java from starting.
pub fn optimized_flags(memory_mb: u32) -> Vec<String> {
    // Bigger heaps (over 12 GB) get a larger young generation and regions.
    let big = memory_mb > 12 * 1024;
    let (new_size, max_new, region, reserve, ihop) = if big { (40, 50, "16M", 15, 20) } else { (30, 40, "8M", 20, 15) };
    [
        "-XX:+UseG1GC".to_string(),
        "-XX:+ParallelRefProcEnabled".into(),
        "-XX:MaxGCPauseMillis=200".into(),
        "-XX:+UnlockExperimentalVMOptions".into(),
        "-XX:+DisableExplicitGC".into(),
        "-XX:+AlwaysPreTouch".into(),
        format!("-XX:G1NewSizePercent={new_size}"),
        format!("-XX:G1MaxNewSizePercent={max_new}"),
        format!("-XX:G1HeapRegionSize={region}"),
        format!("-XX:G1ReservePercent={reserve}"),
        "-XX:G1HeapWastePercent=5".into(),
        "-XX:G1MixedGCCountTarget=4".into(),
        format!("-XX:InitiatingHeapOccupancyPercent={ihop}"),
        "-XX:G1MixedGCLiveThresholdPercent=90".into(),
        "-XX:SurvivorRatio=32".into(),
        "-XX:+PerfDisableSharedMem".into(),
        "-XX:MaxTenuringThreshold=1".into(),
        "-Dusing.aikars.flags=https://mcflags.emc.gs".into(),
        "-Daikars.new.flags=true".into(),
    ]
    .to_vec()
}

/// The Java arguments a server starts with: memory, optimised flags (if on), the player's own JVM arguments
/// (after ours, so they can override), then the server jar or @argument file and `nogui`.
pub fn launch_args(meta: &ServerMeta) -> Result<Vec<String>> {
    let memory = meta.memory_mb.max(512);
    let mut args = if meta.optimized_flags {
        let mut a = vec![format!("-Xms{memory}M"), format!("-Xmx{memory}M")];
        a.extend(optimized_flags(memory));
        a
    } else {
        vec![format!("-Xmx{memory}M"), format!("-Xms{}M", memory.min(1024))]
    };
    if let Some(extra) = meta.jvm_args.as_deref() {
        args.extend(split_args(extra));
    }
    match (&meta.launch.jar, &meta.launch.args_file) {
        (_, Some(file)) => args.push(format!("@{file}")),
        (Some(jar), None) => {
            args.push("-jar".into());
            args.push(jar.clone());
        }
        (None, None) => bail!("{} isn't fully installed. Create it again.", meta.name),
    }
    if !args.iter().any(|a| a == "nogui" || a == "--nogui") {
        args.push("nogui".into());
    }
    Ok(args)
}

/// Starts a server. Returns once Java is running; the server is watched in the background and reports
/// through "server" / "serverLog" events.
pub async fn start(id: &str) -> Result<()> {
    crate::core::storage::ensure_not_moving()?;
    if is_running(id) {
        bail!("This server is already running");
    }
    if crate::content::packswitch::is_switching(&format!("server:{id}")) {
        crate::bail_tr!("packswitch.wait", "Wait for the modpack version switch to finish first");
    }
    let meta = store::get(id).await?;
    let dir = store::dir(id)?;
    if !store::eula_accepted(id).await {
        bail!("Agree to the Minecraft EULA before starting {}.", meta.name);
    }
    let bedrock = meta.kind.bedrock();
    let port_free = if bedrock { network::udp_port_free(meta.port) } else { network::port_free(meta.port) };
    if !port_free {
        bail!("Port {} is already in use (maybe another server is running). Pick a different port in the server's settings.", meta.port);
    }
    let args = if bedrock { Vec::new() } else { launch_args(&meta)? };
    if bedrock && meta.launch.exe.is_none() {
        bail!("{} isn't fully installed. Create it again.", meta.name);
    }
    for (_, file) in [("jar", &meta.launch.jar), ("args", &meta.launch.args_file), ("exe", &meta.launch.exe)] {
        if let Some(f) = file {
            if !crate::core::fsutil::safe_join(&dir, f)?.is_file() {
                bail!("{f} is missing from the server folder. Create the server again.");
            }
        }
    }

    // Register before the slow part (Java lookup) so a second click can't start it twice.
    let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<String>();
    let (kill_tx, mut kill_rx) = mpsc::unbounded_channel::<()>();
    let (lines_tx, _) = broadcast::channel::<String>(512);
    with(|w| {
        w.procs.insert(
            id.to_string(),
            Proc {
                run: RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                stdin: stdin_tx,
                kill: kill_tx,
                state: "starting",
                players: BTreeSet::new(),
                stopping: false,
                net: None,
                lines: lines_tx.clone(),
                bedrock: None,
                bedrock_port: None,
            },
        );
        w.logs.insert(id.to_string(), VecDeque::new());
        w.last.remove(id);
    });
    emit_state(id, None);

    let fail = |e: anyhow::Error| {
        with(|w| w.procs.remove(id));
        emit_state(id, None);
        e
    };
    // Bedrock runs its own program; everything else runs Java.
    let program = match &meta.launch.exe {
        Some(exe) if bedrock => crate::game::mojang::plain(&crate::core::fsutil::safe_join(&dir, exe)?),
        _ => installers::choose_java(&meta).await.map_err(fail)?,
    };
    let mut ports = vec![("server-port", meta.port.to_string())];
    if bedrock {
        ports.push(("server-portv6", (meta.port + 1).to_string()));
    }
    properties::set_values(&dir, &ports).await.map_err(fail)?;
    // Plugin/mod changes queued while the server last ran (normally applied when it stopped).
    let applied = super::plugins::apply_pending(id).await.unwrap_or_default();

    // Only Paperclip, Spigot and Forge 1.7.10–1.12.2 need it (old log4j cuts jar URLs at the first '!' and then
    // scans the whole drive); other loaders resolve real paths and break across drives.
    let legacy_forge = meta.kind == ServerKind::Forge && crate::game::loaders::is_legacy_mc(&meta.mc_version);
    let run_dir = if meta.kind.plugins() || legacy_forge {
        launch_dir(id, &dir).await
    } else {
        dir.clone()
    };
    let mut cmd = hidden_command(&program);
    cmd.args(&args)
        .current_dir(&run_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| fail(anyhow!("Couldn't start {program}: {e}")))?;
    store::touch_started(id).await;

    // Batch console lines so a chatty server doesn't flood the UI.
    let pending: Arc<Mutex<Vec<LogLine>>> = Arc::new(Mutex::new(Vec::new()));
    let flush = {
        let (pending, id) = (pending.clone(), id.to_string());
        move || {
            let lines = std::mem::take(&mut *pending.lock().unwrap());
            if !lines.is_empty() {
                emit("serverLog", ServerLogEvent { server_id: id.clone(), lines });
            }
        }
    };
    let push = {
        let (pending, id, lines_tx, public) = (pending.clone(), id.to_string(), lines_tx.clone(), meta.public);
        move |l: LogLine| {
            let text = patterns().ansi.replace_all(&l.text, "").trim_end().to_string();
            let text = text.trim_start_matches("> ").to_string();
            if text.trim().is_empty() {
                return;
            }
            let line = LogLine { text: text.clone(), level: l.level };
            with(|w| {
                let log = w.logs.entry(id.clone()).or_default();
                log.push_back(line.clone());
                while log.len() > LOG_KEEP {
                    log.pop_front();
                }
            });
            pending.lock().unwrap().push(line);
            let _ = lines_tx.send(text.clone());
            if on_line(&id, &text) && public {
                tokio::spawn(open_internet(id.clone()));
            }
        }
    };
    push(LogLine { text: format!("ResinCraft is starting {} ({})", meta.name, installers::describe(&meta)), level: "info" });
    for text in applied {
        push(LogLine { text, level: "info" });
    }
    if !bedrock {
        push(LogLine { text: format!("Java: {program}"), level: "info" });
        push(LogLine { text: format!("Command: java {}", args.join(" ")), level: "debug" });
    }

    let mut readers = Vec::new();
    if let Some(mut out) = child.stdout.take() {
        let push = push.clone();
        readers.push(tokio::spawn(async move {
            let mut parser = LogParser::new(push);
            let mut buf = vec![0u8; 16 * 1024];
            while let Ok(n) = out.read(&mut buf).await {
                if n == 0 {
                    break;
                }
                parser.feed_bytes(&buf[..n]);
            }
            parser.feed("\n");
        }));
    }
    if let Some(mut err) = child.stderr.take() {
        let push = push.clone();
        readers.push(tokio::spawn(async move {
            let mut parser = LogParser::new(move |l: LogLine| push(LogLine { level: if l.level == "info" { "warn" } else { l.level }, ..l }));
            let mut buf = vec![0u8; 16 * 1024];
            while let Ok(n) = err.read(&mut buf).await {
                if n == 0 {
                    break;
                }
                parser.feed_bytes(&buf[..n]);
            }
            parser.feed("\n");
        }));
    }
    if let Some(mut stdin) = child.stdin.take() {
        tokio::spawn(async move {
            while let Some(line) = stdin_rx.recv().await {
                if stdin.write_all(format!("{line}\n").as_bytes()).await.is_err() || stdin.flush().await.is_err() {
                    break;
                }
            }
        });
    }
    let ticker = {
        let flush = flush.clone();
        tokio::spawn(async move {
            let mut iv = tokio::time::interval(Duration::from_millis(120));
            loop {
                iv.tick().await;
                flush();
            }
        })
    };

    let id = id.to_string();
    let port = meta.port;
    tokio::spawn(async move {
        let mut killed = false;
        let status = loop {
            tokio::select! {
                s = child.wait() => break s,
                Some(()) = kill_rx.recv(), if !killed => {
                    killed = true;
                    let _ = child.start_kill();
                }
            }
        };
        for r in readers {
            let _ = tokio::time::timeout(Duration::from_secs(5), r).await;
        }
        let code = status.as_ref().ok().and_then(|s| s.code());
        let stopping = with(|w| w.procs.get(&id).map(|p| p.stopping).unwrap_or(false));
        let crashed = !killed && !stopping && code != Some(0);
        close_internet(&id, port).await;
        // Swap in plugin/mod changes that waited for the server to let go of its jars.
        match super::plugins::apply_pending(&id).await {
            Ok(lines) => lines.into_iter().for_each(|text| push(LogLine { text, level: "info" })),
            Err(e) => push(LogLine { text: format!("Couldn't apply queued plugin changes: {e}"), level: "warn" }),
        }
        let shown = if killed { "killed".to_string() } else { code.map(|c| c.to_string()).unwrap_or_else(|| "unknown".into()) };
        push(LogLine { text: format!("Server exited ({shown})"), level: if crashed { "error" } else { "info" } });
        flush();
        ticker.abort();
        with(|w| {
            w.procs.remove(&id);
            w.last.insert(id.clone(), if crashed { "crashed" } else { "stopped" });
        });
        emit_state(&id, code);
    });
    Ok(())
}

/// The folder the server runs in. Paperclip (Paper, Purpur, Folia) and Spigot refuse paths containing '!' or '+'
/// (Java jar URLs use "!/" as a separator), so such folders are reached through a link at a clean location.
async fn launch_dir(id: &str, dir: &std::path::Path) -> PathBuf {
    let bad = |p: &std::path::Path| p.to_string_lossy().contains(['!', '+']);
    if !bad(dir) {
        return dir.to_path_buf();
    }
    for root in link_roots(dir).into_iter().filter(|r| !bad(r)) {
        let link = root.join(id);
        let (link2, target) = (link.clone(), dir.to_path_buf());
        let made = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            std::fs::create_dir_all(link2.parent().unwrap_or(&link2))?;
            // Removing a junction/symlink removes only the link, never the server files.
            if std::fs::symlink_metadata(&link2).is_ok() {
                std::fs::remove_dir(&link2).or_else(|_| std::fs::remove_file(&link2))?;
            }
            make_link(&link2, &target)
        })
        .await;
        if matches!(made, Ok(Ok(()))) && link.join("server.json").is_file() {
            return link;
        }
    }
    dir.to_path_buf()
}

/// Where '!'-free links to server folders go.
fn link_roots(dir: &std::path::Path) -> Vec<PathBuf> {
    // Same drive first, so paths between the link and the real folder stay comparable.
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(drive) = dir.ancestors().last() {
        roots.push(drive.join(".resincraft-server-links"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        roots.push(PathBuf::from(local).join("ResinCraft").join("server-links"));
    }
    roots.push(std::env::temp_dir().join("resincraft-server-links"));
    roots.retain(|r| !r.to_string_lossy().contains('!'));
    roots
}

/// Removes the link made by `launch_dir` (only the link; the server folder is untouched).
pub fn forget_link(id: &str) {
    let Ok(dir) = store::dir(id) else { return };
    for root in link_roots(&dir) {
        let link = root.join(id);
        if std::fs::symlink_metadata(&link).is_ok() {
            let _ = std::fs::remove_dir(&link).or_else(|_| std::fs::remove_file(&link));
        }
    }
}

#[cfg(windows)]
fn make_link(link: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    // A directory junction needs no admin rights or developer mode (unlike a symlink).
    use std::os::windows::process::CommandExt;
    let status = std::process::Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(link)
        .arg(target)
        .creation_flags(crate::game::java::CREATE_NO_WINDOW)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("mklink failed"))
    }
}

#[cfg(not(windows))]
fn make_link(link: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

fn net_state(state: &'static str, address: Option<String>, error: Option<String>, via: Option<&'static str>) -> ServerNet {
    ServerNet {
        state,
        address,
        error,
        via,
        error_text: None,
        claim_url: None,
        domain_ok: None,
        playit_hint: false,
        resin: None,
        resin_hint: false,
        bedrock_address: None,
    }
}

/// Why ResinCraft Relay wasn't used, for the card (out of data: it offers a top-up).
fn resin_failed(e: &anyhow::Error) -> ResinNet {
    let error = text_of(e).cloned().unwrap_or_else(|| Text::plain(e.to_string()));
    let data = (error.key == "hostRelay.overQuota").then_some("out");
    ResinNet { error: Some(error), data, banned: relay::is_banned(e), ..ResinNet::default() }
}

/// What the account's data looks like now: used, its cap, top-ups, and whether it's low or out.
fn apply_usage(resin: &mut ResinNet, usage: &resincraft_relay::proto::Usage) {
    resin.used_bytes = usage.bytes;
    // Relays before protocol 4 don't send the account's cap; the one from going online stays.
    if usage.cap_bytes > 0 || usage.unlimited {
        resin.limit_bytes = usage.cap_bytes;
    }
    resin.unlimited = usage.unlimited;
    resin.topup_bytes = usage.topup_bytes;
    resin.topups_left = usage.topups_left;
    resin.can_top_up = usage.can_top_up();
    let left = (resin.limit_bytes > 0 && !usage.unlimited).then(|| resin.limit_bytes.saturating_sub(usage.bytes));
    resin.data = match left {
        Some(0) => Some("out"),
        Some(l) if l < usage.low_bytes => Some("low"),
        _ => None,
    };
    if usage.unlimited {
        resin.ends_at = None;
        resin.warning = None;
        resin.soon = false;
    }
}

/// Brings the server online through ResinCraft Relay (`<name>.rscraft.resincore.tech`). Ok also when the server
/// stopped meanwhile: nothing more to do then.
async fn open_resin(id: &str) -> Result<()> {
    let Ok(meta) = store::get(id).await else { return Ok(()) };
    let status_id = id.to_string();
    // A Java server with Geyser asks the relay for a port for its Bedrock players too.
    let geyser = if meta.kind.bedrock() { None } else { geyser_port(id).await };
    let opened = relay::open(id, meta.port, meta.tunnel == TunnelMode::Resin, meta.kind.bedrock(), geyser, move |s| on_resin_status(&status_id, s)).await?;
    if !is_running(id) || net(id).is_none() {
        opened.task.abort();
        return Ok(());
    }
    with(|w| {
        if let Some(old) = w.tunnels.insert(id.to_string(), opened.task) {
            old.abort();
        }
    });
    let mut resin = ResinNet {
        name: Some(opened.name),
        limit_bytes: opened.limits.monthly_bytes,
        continue_secs: relay::continue_window(&opened.limits),
        paused: opened.paused.as_ref().map(|(_, m)| m.clone().unwrap_or_default()),
        off: opened.paused.as_ref().is_some_and(|(off, _)| *off),
        ..ResinNet::default()
    };
    apply_usage(&mut resin, &opened.usage);
    resin.ends_at = opened.secs_left.map(ends_at);
    if let Some(address) = opened.geyser_address.clone() {
        set_geyser_address(id, address);
    }
    if resin.data.is_some() {
        attention();
    }
    set_net(id, Some(ServerNet { resin: Some(resin), ..net_state("open", Some(opened.address), None, Some("resin")) }));
    Ok(())
}

/// Flashes the launcher in the taskbar (when it isn't in front) so the host sees a relay warning.
fn attention() {
    use tauri::Manager;
    if let Some(w) = crate::core::ctx::app().and_then(|a| a.get_webview_window("main")) {
        let _ = w.request_user_attention(Some(tauri::UserAttentionType::Critical));
    }
}

/// Another full ResinCraft Relay session (the Continue button, or `continue` typed in the console).
pub fn relay_continue(id: &str) -> Result<()> {
    relay::continue_session(id)
}

/// More relay data this month (the "Get 30 GB more" button). After the data ran out and the tunnel couldn't
/// start, sharing starts again with the new data. Returns the bytes added.
pub async fn relay_top_up(id: &str) -> Result<u64> {
    let added = relay::top_up(id).await?;
    let stopped = net(id).is_some_and(|n| n.via != Some("resin") && n.resin.as_ref().is_some_and(|r| r.data == Some("out")));
    if stopped && is_running(id) {
        refresh_internet(id).await;
    }
    Ok(added)
}

/// Starts sharing again after the relay session ended (the "Start again" button).
pub async fn relay_restart(id: &str) {
    refresh_internet(id).await;
}

/// Local Unix ms when a relay session with `secs_left` ends.
fn ends_at(secs_left: u64) -> i64 {
    crate::core::fsutil::now_ms() + secs_left as i64 * 1000
}

/// Updates from a running ResinCraft Relay tunnel: traffic, reconnecting, back online, or given up.
fn on_resin_status(id: &str, status: relay::Status) {
    let Some(mut n) = net(id).filter(|n| n.via == Some("resin")) else { return };
    let before = n.clone();
    let resin = n.resin.get_or_insert_with(ResinNet::default);
    let data_before = resin.data;
    match status {
        relay::Status::Usage(u) => apply_usage(resin, &u),
        relay::Status::TopUp { added_bytes, by_admin, usage } => {
            apply_usage(resin, &usage);
            if by_admin {
                resin.admin_added_bytes += added_bytes;
            }
        }
        relay::Status::ContinueSoon { secs_left } => {
            resin.soon = true;
            resin.ends_at = Some(ends_at(secs_left));
            attention();
        }
        relay::Status::Reconnecting => {
            if resin.reconnecting {
                return;
            }
            resin.reconnecting = true;
        }
        relay::Status::Online { address, geyser_address, limits, usage, secs_left } => {
            if let Some(g) = geyser_address {
                set_geyser_address(id, g);
            }
            resin.reconnecting = false;
            resin.limit_bytes = limits.monthly_bytes;
            resin.continue_secs = relay::continue_window(&limits);
            apply_usage(resin, &usage);
            resin.ends_at = secs_left.map(ends_at);
            resin.soon = false;
            resin.warning = None;
            resin.paused = None;
            resin.off = false;
            n.address = Some(address);
        }
        relay::Status::Warning { secs_left } => {
            let first = resin.warning.is_none();
            let minutes = secs_left.div_ceil(60).max(1);
            resin.warning = Some(minutes as u32);
            resin.ends_at = Some(ends_at(secs_left));
            if first {
                // Players can't be shown a disconnect reason through the relay, so warn them in chat now.
                let plural = if minutes == 1 { "" } else { "s" };
                if let Ok(tx) = sender(id) {
                    let _ = tx.send(format!("say ResinCraft Relay closes in {minutes} minute{plural} unless the host continues."));
                }
                attention();
            }
        }
        relay::Status::Extended { secs_left, continues } => {
            resin.warning = None;
            resin.soon = false;
            resin.ends_at = Some(ends_at(secs_left));
            resin.continues = continues;
        }
        relay::Status::Paused { message, off, .. } => {
            resin.paused = Some(message.unwrap_or_default());
            resin.off = off;
        }
        relay::Status::Resumed => {
            resin.paused = None;
            resin.off = false;
        }
        relay::Status::SessionEnded => {
            let name = resin.name.clone();
            let e = crate::err_tr!("hostRelay.sessionEnded", "The ResinCraft Relay session ended.");
            n = ServerNet {
                error_text: text_of(&e).cloned(),
                resin: Some(ResinNet { name, ended: true, ..ResinNet::default() }),
                ..net_state("failed", None, Some(e.to_string()), None)
            };
        }
        relay::Status::Failed(e) => {
            eprintln!("[server] ResinCraft Relay stopped: {e:#}");
            // Banned: the card and the banner say until when and why (and nothing reconnects).
            let resin = relay::is_banned(&e).then(|| ResinNet { name: resin.name.clone(), ..resin_failed(&e) });
            if resin.is_some() {
                attention();
            }
            n = ServerNet { error_text: text_of(&e).cloned(), resin, ..net_state("failed", None, Some(e.to_string()), None) };
        }
    }
    // Running low or out of data: flash the taskbar once per step (the app shows a notification too).
    if n.resin.as_ref().is_some_and(|r| r.data.is_some() && r.data != data_before) {
        attention();
    }
    if n == before {
        return;
    }
    set_net(id, Some(n));
}

/// Makes a running server reachable from the internet the way its settings say (TunnelMode): the router
/// (UPnP, a direct connection; the lease is renewed while it runs), ResinCraft Relay (a stable name, needs a
/// Microsoft account), playit.gg (a stable address), or the bore relay (no setup at all), which is also the
/// fallback when the others fail. Automatic and playit.gg try ResinCraft Relay first when a Microsoft account is
/// signed in; then Automatic tries playit.gg if this launcher is already linked to it (never a claim page there),
/// then the router, then bore.
async fn open_internet(id: String) {
    let Ok(meta) = store::get(&id).await else { return };
    if meta.kind.bedrock() {
        return open_bedrock(id, meta).await;
    }
    tokio::spawn(open_geyser(id.clone(), meta.name.clone()));
    open_java(id, meta).await
}

/// Keeps a router mapping alive (it has a lease) while the server runs and still uses it.
async fn renew_mapping(id: &str, udp: bool, port: u16, name: &str, in_use: impl Fn() -> bool) {
    loop {
        tokio::time::sleep(Duration::from_secs(network::LEASE_SECS as u64 / 2)).await;
        if !is_running(id) || !in_use() {
            break;
        }
        let _ = if udp { network::open_udp(port, name.to_string()).await } else { network::open_port(port, name.to_string()).await };
    }
}

/// How trying ResinCraft Relay first went.
enum ResinFirst {
    /// Online through it (or the server stopped meanwhile): nothing more to do.
    Done,
    Failed(ResinNet),
    /// The relay could be used, but no Microsoft account is signed in.
    NoAccount,
    /// This build has no relay.
    Unavailable,
}

/// Tries ResinCraft Relay before the other ways.
async fn resin_first(id: &str, meta: &ServerMeta) -> ResinFirst {
    if !relay::usable(meta).await {
        return if relay::settings().is_some() { ResinFirst::NoAccount } else { ResinFirst::Unavailable };
    }
    match open_resin(id).await {
        Ok(()) => ResinFirst::Done,
        Err(_) if !is_running(id) || net(id).is_none() => ResinFirst::Done,
        Err(e) => {
            eprintln!("[server] ResinCraft Relay didn't work, trying the next way: {e:#}");
            ResinFirst::Failed(resin_failed(&e))
        }
    }
}

/// Bedrock servers talk UDP, which bore can't carry: the router (UPnP), then playit.gg (when linked, or chosen),
/// then ResinCraft Relay (a port of its own on the relay). Set to ResinCraft Relay, only the relay is used.
async fn open_bedrock(id: String, meta: ServerMeta) {
    let port = meta.port;
    set_net(&id, Some(net_state("opening", None, None, None)));
    if meta.tunnel == TunnelMode::Manual {
        match network::public_ipv4().await {
            Ok(ip) if is_running(&id) => set_net(&id, Some(net_state("open", Some(format!("{ip}:{port}")), None, Some("manual")))),
            Ok(_) => {}
            Err(e) => set_net(&id, Some(net_state("failed", None, Some(format!("{e}.")), None))),
        }
        return;
    }
    if meta.tunnel == TunnelMode::Resin {
        if let Err(e) = open_resin(&id).await {
            if is_running(&id) && net(&id).is_some() {
                let resin = resin_failed(&e);
                set_net(&id, Some(ServerNet { error_text: resin.error.clone(), resin: Some(resin), ..net_state("failed", None, Some(format!("{e}")), None) }));
            }
        }
        return;
    }
    // ResinCraft Relay first whenever it can be used, then playit.gg, then the router.
    let mut resin_note = None;
    let mut resin_hint = false;
    if matches!(meta.tunnel, TunnelMode::Auto | TunnelMode::Playit) {
        match resin_first(&id, &meta).await {
            ResinFirst::Done => return,
            ResinFirst::Failed(note) => resin_note = Some(note),
            ResinFirst::NoAccount => resin_hint = true,
            ResinFirst::Unavailable => {}
        }
    }
    let mut first_err = None;
    let linked = playit::linked().await;
    if meta.tunnel != TunnelMode::Playit && !linked {
        match network::open_udp(port, meta.name.clone()).await {
            Ok(address) => {
                if !is_running(&id) {
                    network::close_udp(port).await;
                    return;
                }
                set_net(&id, Some(net_state("open", Some(address), None, Some("router"))));
                let check = id.clone();
                renew_mapping(&id, true, port, &meta.name, move || net(&check).map(|n| n.via) == Some(Some("router"))).await;
                return;
            }
            Err(e) => first_err = Some(e),
        }
    }
    if meta.tunnel == TunnelMode::Playit || linked {
        let (claim_id, wanted_id) = (id.clone(), id.clone());
        let opened = playit::open(
            &id,
            port,
            &meta.name,
            true,
            move |url| set_net(&claim_id, Some(ServerNet { claim_url: Some(url), ..net_state("opening", None, None, None) })),
            move || is_running(&wanted_id) && net(&wanted_id).is_some_and(|n| n.state == "opening"),
        )
        .await;
        match opened {
            Ok(address) if is_running(&id) => {
                set_net(&id, Some(net_state("open", Some(address), None, Some("playit"))));
                return;
            }
            Ok(_) => {
                playit::close(&id).await;
                return;
            }
            Err(_) if !is_running(&id) || net(&id).is_none() => return,
            Err(e) => first_err = Some(first_err.map(|f| anyhow!("{f} {e}")).unwrap_or(e)),
        }
    }
    // Linked to playit.gg but it didn't work: the router as the last way.
    if meta.tunnel != TunnelMode::Playit && linked && first_err.is_some() {
        if let Ok(address) = network::open_udp(port, meta.name.clone()).await {
            if !is_running(&id) {
                network::close_udp(port).await;
                return;
            }
            set_net(&id, Some(net_state("open", Some(address), None, Some("router"))));
            let check = id.clone();
            renew_mapping(&id, true, port, &meta.name, move || net(&check).map(|n| n.via) == Some(Some("router"))).await;
            return;
        }
    }
    let msg = first_err.map(|e| format!("{e}.")).unwrap_or_default();
    set_net(&id, Some(ServerNet { playit_hint: !playit::linked().await, resin: resin_note, resin_hint, ..net_state("failed", None, Some(msg), None) }));
}

/// Geyser next to a Java server: opens its UDP port for Bedrock players (the router, else playit.gg once
/// linked). Nothing to do when Geyser isn't installed.
/// Geyser's local UDP port when this Java server has Geyser installed.
async fn geyser_port(id: &str) -> Option<u16> {
    let status = super::plugins::bedrock_status(id).await.ok()?;
    if status.get("geyser").and_then(serde_json::Value::as_bool) != Some(true) {
        return None;
    }
    Some(status.get("port").and_then(serde_json::Value::as_u64).and_then(|p| u16::try_from(p).ok()).unwrap_or(19132))
}

/// Where Bedrock players join this Java server through ResinCraft Relay (Geyser's port there).
fn set_geyser_address(id: &str, address: String) {
    with(|w| {
        if let Some(p) = w.procs.get_mut(id) {
            p.bedrock = Some(address);
            // Not a router mapping: nothing to close on the router later.
            p.bedrock_port = None;
        }
    });
    emit_state(id, None);
}

async fn open_geyser(id: String, name: String) {
    let Some(port) = geyser_port(&id).await else { return };
    // ResinCraft Relay carries Geyser along with the Java server when it can: wait until sharing has settled, and
    // use the router or playit.gg only if the relay didn't give Bedrock players an address.
    for _ in 0..120 {
        if !is_running(&id) {
            return;
        }
        if net(&id).is_none_or(|n| n.state != "opening") {
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    if with(|w| w.procs.get(&id).is_some_and(|p| p.bedrock.is_some())) {
        return;
    }
    let set = |address: Option<String>, mapped: Option<u16>| {
        with(|w| {
            if let Some(p) = w.procs.get_mut(&id) {
                p.bedrock = address;
                p.bedrock_port = mapped;
            }
        });
        emit_state(&id, None);
    };
    match network::open_udp(port, format!("{name} (Bedrock)")).await {
        Ok(address) if is_running(&id) => {
            set(Some(address), Some(port));
            let check = id.clone();
            renew_mapping(&id, true, port, &format!("{name} (Bedrock)"), move || with(|w| w.procs.get(&check).is_some_and(|p| p.bedrock_port == Some(port)))).await;
        }
        Ok(_) => network::close_udp(port).await,
        Err(_) if playit::linked().await => {
            let key = format!("{id}#bedrock");
            let wanted = id.clone();
            match playit::open(&key, port, &format!("{name} Bedrock"), true, |_| {}, move || is_running(&wanted)).await {
                Ok(address) if is_running(&id) => set(Some(address), None),
                Ok(_) => playit::close(&key).await,
                Err(e) => eprintln!("[server] Geyser's port couldn't be shared through playit.gg: {e:#}"),
            }
        }
        Err(e) => eprintln!("[server] Geyser's port couldn't be opened on the router: {e:#}"),
    }
}

/// Java servers: the router, playit.gg, ResinCraft Relay or the bore relay, as the server is set up.
async fn open_java(id: String, meta: ServerMeta) {
    let port = meta.port;
    set_net(&id, Some(net_state("opening", None, None, None)));
    // Automatic only: playit.gg isn't linked, so the relay's card suggests linking it.
    let mut playit_hint = false;
    // Automatic only: no Microsoft account for ResinCraft Relay, so the card suggests signing in.
    let mut resin_hint = false;
    // Why ResinCraft Relay wasn't used, when it was tried.
    let mut resin_note: Option<ResinNet> = None;
    // ResinCraft Relay first whenever it can be used (a Microsoft account is signed in), then playit.gg.
    if matches!(meta.tunnel, TunnelMode::Auto | TunnelMode::Playit) {
        match resin_first(&id, &meta).await {
            ResinFirst::Done => return,
            ResinFirst::Failed(note) => resin_note = Some(note),
            ResinFirst::NoAccount => resin_hint = true,
            ResinFirst::Unavailable => {}
        }
    }
    let earlier_err = match meta.tunnel {
        // The player forwarded the port themselves: share the public IP with that port.
        TunnelMode::Manual => {
            match network::public_ipv4().await {
                Ok(ip) if is_running(&id) => set_net(&id, Some(net_state("open", Some(format!("{ip}:{port}")), None, Some("manual")))),
                Ok(_) => {}
                Err(e) => set_net(&id, Some(net_state("failed", None, Some(format!("{e}.")), None))),
            }
            return;
        }
        TunnelMode::Auto => {
            // playit.gg second (ResinCraft Relay was tried above), when linked already, so no claim page. If playit.gg
            // asks to link again (the agent was removed there), give up on it at once and move on.
            if playit::linked().await {
                let needs_claim = Arc::new(std::sync::atomic::AtomicBool::new(false));
                let (flag, wanted_id) = (needs_claim.clone(), id.clone());
                let wanted_flag = needs_claim.clone();
                let opened = playit::open(
                    &id,
                    port,
                    &meta.name,
                    false,
                    move |_| flag.store(true, std::sync::atomic::Ordering::Relaxed),
                    move || {
                        !wanted_flag.load(std::sync::atomic::Ordering::Relaxed) && is_running(&wanted_id) && net(&wanted_id).is_some_and(|n| n.state == "opening")
                    },
                )
                .await;
                match opened {
                    Ok(address) if is_running(&id) => {
                        set_net(&id, Some(net_state("open", Some(address), None, Some("playit"))));
                        return;
                    }
                    Ok(_) => {
                        playit::close(&id).await;
                        return;
                    }
                    Err(_) if !is_running(&id) || net(&id).is_none() => return,
                    Err(e) => {
                        eprintln!("[server] playit.gg didn't work, trying the next way: {e:#}");
                        playit_hint = needs_claim.load(std::sync::atomic::Ordering::Relaxed);
                    }
                }
            } else {
                playit_hint = true;
            }
            // Then the router (UPnP).
            match network::open_port(port, meta.name.clone()).await {
                Ok(address) => {
                    if !is_running(&id) {
                        network::close_port(port).await;
                        return;
                    }
                    set_net(&id, Some(net_state("open", Some(address), None, Some("router"))));
                    loop {
                        tokio::time::sleep(Duration::from_secs(network::LEASE_SECS as u64 / 2)).await;
                        if !is_running(&id) || net(&id).map(|n| n.via) != Some(Some("router")) {
                            break;
                        }
                        let _ = network::open_port(port, meta.name.clone()).await;
                    }
                    return;
                }
                Err(router_err) => Some(router_err),
            }
        }
        TunnelMode::Playit => {
            let (claim_id, wanted_id) = (id.clone(), id.clone());
            let opened = playit::open(
                &id,
                port,
                &meta.name,
                false,
                // First time: show the page where the player links the launcher to playit.gg.
                move |url| set_net(&claim_id, Some(ServerNet { claim_url: Some(url), ..net_state("opening", None, None, None) })),
                move || is_running(&wanted_id) && net(&wanted_id).is_some_and(|n| n.state == "opening"),
            )
            .await;
            match opened {
                Ok(address) => {
                    if !is_running(&id) {
                        playit::close(&id).await;
                    } else {
                        set_net(&id, Some(net_state("open", Some(address), None, Some("playit"))));
                    }
                    return;
                }
                // Stopped, or internet access was turned off meanwhile.
                Err(_) if !is_running(&id) || net(&id).is_none() => return,
                Err(e) => Some(e),
            }
        }
        TunnelMode::Resin => match open_resin(&id).await {
            Ok(()) => return,
            Err(_) if !is_running(&id) || net(&id).is_none() => return,
            Err(e) => {
                resin_note = Some(resin_failed(&e));
                Some(e)
            }
        },
        TunnelMode::Relay => None,
    };
    let relay = tunnel::Relay::for_server(&meta);
    let closed_id = id.clone();
    match tunnel::open(relay, port, move || {
        // The relay went away while the server still runs: say so instead of showing a dead address.
        if net(&closed_id).is_some_and(|n| n.via == Some("relay")) {
            set_net(&closed_id, Some(net_state("failed", None, Some("The relay connection was lost. Restart the server to get a new address.".into()), None)));
        }
    })
    .await
    {
        Ok(t) => {
            if !is_running(&id) {
                t.task.abort();
                return;
            }
            with(|w| {
                if let Some(old) = w.tunnels.insert(id.clone(), t.task) {
                    old.abort();
                }
            });
            // When playit.gg was chosen, say why the relay is used instead.
            let why = earlier_err.filter(|_| meta.tunnel == TunnelMode::Playit).map(|e| format!("{e}."));
            set_net(&id, Some(ServerNet { playit_hint, resin_hint, resin: resin_note, ..net_state("open", Some(t.address), why, Some("relay")) }));
        }
        Err(e) => {
            let msg = match earlier_err {
                Some(first) => format!("{first} {e}."),
                None => format!("{e}."),
            };
            // With ResinCraft Relay chosen, its (translated) reason is the one to show.
            let error_text = resin_note.as_ref().filter(|_| meta.tunnel == TunnelMode::Resin).and_then(|r| r.error.clone());
            set_net(&id, Some(ServerNet { error_text, resin: resin_note, ..net_state("failed", None, Some(msg), None) }));
        }
    }
}

/// Closes whatever made the server reachable from the internet.
async fn close_internet(id: &str, port: u16) {
    let current = net(id);
    if let Some(t) = with(|w| w.tunnels.remove(id)) {
        t.abort();
    }
    playit::close(id).await;
    playit::close(&format!("{id}#bedrock")).await;
    let bedrock = store::get(id).await.is_ok_and(|m| m.kind.bedrock());
    if current.as_ref().is_some_and(|n| n.state == "open" && n.via == Some("router")) {
        if bedrock {
            network::close_udp(port).await;
        } else {
            network::close_port(port).await;
        }
    }
    // Geyser's port.
    let geyser = with(|w| {
        w.procs.get_mut(id).and_then(|p| {
            p.bedrock = None;
            p.bedrock_port.take()
        })
    });
    if let Some(p) = geyser {
        network::close_udp(p).await;
    }
}

/// Opens (or closes) internet access for a running server right away, e.g. after the player flipped the switch.
pub async fn set_public(id: &str, public: bool) {
    let Some(state) = with(|w| w.procs.get(id).map(|p| p.state)) else { return };
    let current = net(id).map(|n| n.state);
    let Ok(meta) = store::get(id).await else { return };
    if public && state == "running" && !matches!(current, Some("open" | "opening")) {
        tokio::spawn(open_internet(id.to_string()));
    } else if !public && current.is_some() {
        close_internet(id, meta.port).await;
        set_net(id, None);
    }
}

/// Re-opens internet access after the way of sharing changed (playit.gg, relay…) while the server runs.
pub async fn refresh_internet(id: &str) {
    let Ok(meta) = store::get(id).await else { return };
    if !meta.public || with(|w| w.procs.get(id).map(|p| p.state)) != Some("running") {
        return;
    }
    close_internet(id, meta.port).await;
    set_net(id, None);
    tokio::spawn(open_internet(id.to_string()));
}

fn sender(id: &str) -> Result<mpsc::UnboundedSender<String>> {
    with(|w| w.procs.get(id).map(|p| p.stdin.clone())).ok_or_else(|| anyhow!("The server isn't running"))
}

/// Types a command into the server console.
pub fn command(id: &str, cmd: &str) -> Result<()> {
    let cmd = cmd.trim().trim_start_matches('/');
    if cmd.is_empty() {
        return Ok(());
    }
    if cmd.contains(['\n', '\r']) {
        bail!("A command must be a single line");
    }
    let warned = net(id).and_then(|n| n.resin).is_some_and(|r| r.warning.is_some());
    if warned && cmd.eq_ignore_ascii_case("continue") {
        relay::continue_session(id)?;
        let note = LogLine { text: "> continue  (sent to ResinCraft Relay: the session goes on, it wasn't typed into the server)".into(), level: "info" };
        with(|w| w.logs.entry(id.to_string()).or_default().push_back(note.clone()));
        emit("serverLog", ServerLogEvent { server_id: id.to_string(), lines: vec![note] });
        return Ok(());
    }
    sender(id)?.send(cmd.to_string()).map_err(|_| anyhow!("The server isn't accepting commands"))?;
    let echo = LogLine { text: format!("> {cmd}"), level: "debug" };
    with(|w| w.logs.entry(id.to_string()).or_default().push_back(echo.clone()));
    emit("serverLog", ServerLogEvent { server_id: id.to_string(), lines: vec![echo] });
    Ok(())
}

/// Asks the server to save and stop; it's killed if it hasn't exited after STOP_TIMEOUT.
pub fn stop(id: &str) -> Result<()> {
    let tx = sender(id)?;
    let run = with(|w| {
        let p = w.procs.get_mut(id)?;
        p.stopping = true;
        p.state = "stopping";
        Some(p.run)
    });
    emit_state(id, None);
    let _ = tx.send("stop".into());
    let id = id.to_string();
    tokio::spawn(async move {
        tokio::time::sleep(STOP_TIMEOUT).await;
        if run.is_some() && with(|w| w.procs.get(&id).map(|p| p.run)) == run {
            kill(&id);
        }
    });
    Ok(())
}

/// Ends the server process right away (unsaved progress is lost).
pub fn kill(id: &str) {
    with(|w| {
        if let Some(p) = w.procs.get_mut(id) {
            p.stopping = true;
            let _ = p.kill.send(());
        }
    });
}

/// Waits until a console line matches `re`; returns the line.
pub async fn wait_line(id: &str, re: &Regex, timeout: Duration) -> Result<String> {
    let mut rx = with(|w| w.procs.get(id).map(|p| p.lines.subscribe())).ok_or_else(|| anyhow!("The server isn't running"))?;
    tokio::time::timeout(timeout, async {
        loop {
            match rx.recv().await {
                Ok(line) if re.is_match(&line) => return Ok(line),
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => bail!("The server stopped"),
            }
        }
    })
    .await
    .map_err(|_| anyhow!("Timed out waiting for the server"))?
}

/// Waits until the server reaches one of `states` ("running", "stopped"…).
pub async fn wait_state(id: &str, states: &[&str], timeout: Duration) -> Result<&'static str> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let s = state(id);
        if states.contains(&s) {
            return Ok(s);
        }
        if tokio::time::Instant::now() > deadline {
            bail!("Timed out: the server is still {s}");
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// Stops every running server (used when the launcher quits): `stop` first, then kill whatever is left.
pub async fn shutdown_all(grace: Duration) {
    let ids = running_ids();
    if ids.is_empty() {
        return;
    }
    for id in &ids {
        let _ = stop(id);
    }
    let deadline = tokio::time::Instant::now() + grace;
    while !running_ids().is_empty() && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    for id in running_ids() {
        kill(&id);
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !running_ids().is_empty() && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

// ---------- Backups ----------

/// Zips the world (and Bukkit-style _nether/_the_end folders) into `<server>/backups`. A running server
/// is told to flush and pause saving while the zip is written.
pub async fn backup(id: &str) -> Result<String> {
    let meta = store::get(id).await?;
    let dir = store::dir(id)?;
    let level = properties::get_value(&dir, "level-name").await.filter(|l| !l.trim().is_empty()).unwrap_or_else(|| "world".into());
    let level = crate::content::index::safe_name(level.trim())?.to_string();
    if meta.kind.bedrock() {
        if is_running(id) {
            bail!("Stop {} first: a Bedrock world is backed up while the server is off.", meta.name);
        }
        let world = format!("worlds/{level}");
        if !dir.join(&world).is_dir() {
            bail!("{} has no world yet. Start the server once to create it.", meta.name);
        }
        let backups = dir.join("backups");
        tokio::fs::create_dir_all(&backups).await?;
        let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        let target = backups.join(unique_name(&backups, &format!("{stamp}_{level}"), ".zip").await);
        let t2 = target.clone();
        Task::run_to_end(format!("Backing up {}", meta.name), Some(id.to_string()), |t| async move {
            t.update("Compressing the world…", None);
            zip_worlds(dir, vec![world], t2).await
        })
        .await?;
        return Ok(target.to_string_lossy().into_owned());
    }
    // Flush first: a running server may not have written its world files yet.
    let running = state(id) == "running";
    if running {
        command(id, "save-off")?;
        let saved = Regex::new(r"(?i)saved the game|all chunks are saved|saving is already turned off")?;
        let waiter = wait_line(id, &saved, Duration::from_secs(60));
        command(id, "save-all flush")?;
        let _ = waiter.await;
    }
    let worlds: Vec<String> = [level.clone(), format!("{level}_nether"), format!("{level}_the_end")].into_iter().filter(|w| dir.join(w).is_dir()).collect();
    if worlds.is_empty() {
        if running {
            let _ = command(id, "save-on");
        }
        bail!("{} has no world yet. Start the server once to create it.", meta.name);
    }
    let backups = dir.join("backups");
    tokio::fs::create_dir_all(&backups).await?;
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let target = backups.join(unique_name(&backups, &format!("{stamp}_{level}"), ".zip").await);

    let result = Task::run_to_end(format!("Backing up {}", meta.name), Some(id.to_string()), |t| {
        let (dir, target) = (dir.clone(), target.clone());
        async move {
            t.update("Compressing the world…", None);
            zip_worlds(dir, worlds, target.clone()).await?;
            Ok(target.to_string_lossy().into_owned())
        }
    })
    .await;
    if running && is_running(id) {
        let _ = command(id, "save-on");
    }
    result
}

async fn zip_worlds(dir: PathBuf, worlds: Vec<String>, target: PathBuf) -> Result<()> {
    let tmp = PathBuf::from(format!("{}.part", target.display()));
    let tmp2 = tmp.clone();
    let res = tokio::task::spawn_blocking(move || -> Result<()> {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&tmp2)?);
        for w in &worlds {
            crate::player::worlds::zip_folder(&mut zip, &dir.join(w), w)?;
        }
        zip.finish()?;
        Ok(())
    })
    .await
    .map_err(anyhow::Error::from)
    .and_then(|r| r);
    if let Err(e) = res {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e);
    }
    tokio::fs::rename(&tmp, &target).await?;
    Ok(())
}

