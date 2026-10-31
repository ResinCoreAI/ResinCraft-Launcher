//! One-click lag reports with spark (`server:spark*`): runs `spark profiler start --timeout 60` in the server's console,
//! follows its replies and picks up the link to the report, which spark uploads to its public viewer.
//!
//! Spark on Modrinth has mod builds only (Fabric, Quilt, Forge, NeoForge), so those servers can get it from here. Paper
//! 1.21+ bundles it. Anything else needs a spark jar put there by hand; without one there's no report to offer.
//! Hybrids stay out: nothing shows spark's plugin or mod builds work on them.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::Duration;

use anyhow::{bail, Result};
use regex::Regex;
use serde::Serialize;
use tokio::sync::broadcast::error::RecvError;

use crate::core::types::{ServerKind, ServerMeta};

use super::{plugins, runtime, store};

/// spark's Modrinth project.
const PROJECT: &str = "l6YH9Als";
const PROFILE_SECS: u64 = 60;
/// On top of the profile: spark starting, then uploading.
const GRACE: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SparkReport {
    /// "starting" | "profiling" | "uploading" | "done" | "failed"
    pub state: &'static str,
    /// When the report was asked for (Unix milliseconds).
    pub started_at: i64,
    pub seconds: u64,
    pub url: Option<String>,
    /// When failed: "notLoaded" | "busy" | "upload" | "failed" | "timeout" | "stopped"
    pub error: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SparkStatus {
    /// False: no report on this server, the button is hidden.
    pub available: bool,
    /// Installing it from Modrinth comes first.
    pub needs_install: bool,
    /// Installed while the server runs: it loads at the next start.
    pub needs_restart: bool,
    pub report: Option<SparkReport>,
}

/// What this run of each server has: its report, and whether spark was installed during it.
struct Run {
    run: u64,
    report: Option<SparkReport>,
    installed: bool,
}

static RUNS: LazyLock<Mutex<HashMap<String, Run>>> = LazyLock::new(Default::default);

fn with_run<R>(id: &str, f: impl FnOnce(&mut Run) -> R) -> Option<R> {
    let run = runtime::server_run(id)?;
    let mut all = RUNS.lock().unwrap();
    let e = all.entry(id.to_string()).or_insert(Run { run, report: None, installed: false });
    if e.run != run {
        *e = Run { run, report: None, installed: false };
    }
    Some(f(e))
}

/// A spark jar in plugins/ or mods/ (not one turned off).
pub async fn has_jar(folder: &Path) -> bool {
    let Ok(mut entries) = tokio::fs::read_dir(folder).await else { return false };
    while let Ok(Some(e)) = entries.next_entry().await {
        let name = e.file_name().to_string_lossy().to_ascii_lowercase();
        if (name.starts_with("spark-") || name.starts_with("spark_")) && name.ends_with(".jar") {
            return true;
        }
    }
    false
}

/// The console command of spark on this server software; none where it can't run.
fn command_base(kind: ServerKind) -> Option<&'static str> {
    match kind {
        ServerKind::Velocity => Some("sparkv"),
        ServerKind::Bungeecord | ServerKind::Waterfall | ServerKind::Travertine => Some("sparkb"),
        ServerKind::Sponge | ServerKind::Fabric | ServerKind::Quilt | ServerKind::Forge | ServerKind::Neoforge => Some("spark"),
        k if k.bukkit() => Some("spark"),
        _ => None,
    }
}

fn on_modrinth(kind: ServerKind) -> bool {
    matches!(kind, ServerKind::Fabric | ServerKind::Quilt | ServerKind::Forge | ServerKind::Neoforge)
}

/// Paper 1.21+ and its forks, which carry Paper's bundled copy. Newer Minecraft numbers years (26.1).
fn bundled(kind: ServerKind, mc: &str) -> bool {
    let mut parts = mc.split('.').map_while(|p| p.parse::<u32>().ok());
    let version = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    matches!(kind, ServerKind::Paper | ServerKind::Purpur | ServerKind::Leaf | ServerKind::Pufferfish) && version >= (1, 21)
}

async fn jar_there(meta: &ServerMeta) -> Result<bool> {
    let dir = store::dir(&meta.id)?;
    Ok(has_jar(&dir.join(if meta.kind.plugins() { "plugins" } else { "mods" })).await)
}

pub async fn status(id: &str) -> Result<SparkStatus> {
    let meta = store::get(id).await?;
    let jar = command_base(meta.kind).is_some() && jar_there(&meta).await?;
    let ready = jar || bundled(meta.kind, &meta.mc_version);
    let (report, installed) = with_run(id, |r| (r.report.clone(), r.installed)).unwrap_or((None, false));
    Ok(SparkStatus { available: ready || on_modrinth(meta.kind), needs_install: !ready, needs_restart: jar && installed, report })
}

/// Installs spark from Modrinth (it loads at the server's next start).
pub async fn install(id: &str) -> Result<SparkStatus> {
    let meta = store::get(id).await?;
    if !on_modrinth(meta.kind) {
        bail!("Modrinth has no spark for {}", meta.kind.label());
    }
    if !jar_there(&meta).await? {
        plugins::install(id, PROJECT, None, None).await?;
        with_run(id, |r| r.installed = true);
    }
    status(id).await
}

/// Starts a report and returns; `status` follows it.
pub async fn run(id: &str) -> Result<SparkStatus> {
    let meta = store::get(id).await?;
    let now = status(id).await?;
    let Some(base) = command_base(meta.kind).filter(|_| now.available && !now.needs_install) else { bail!("This server has no spark") };
    if now.needs_restart {
        bail!("Restart the server first: spark loads when it starts");
    }
    if runtime::state(id) != "running" {
        bail!("Start the server first");
    }
    if now.report.is_some_and(|r| r.error.is_none() && r.url.is_none()) {
        bail!("A lag report is already running");
    }
    // Listen before typing, so no reply is missed.
    let Some(lines) = runtime::subscribe_lines(id) else { bail!("The server isn't running") };
    let started_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    with_run(id, |r| r.report = Some(SparkReport { state: "starting", started_at, seconds: PROFILE_SECS, url: None, error: None }));
    runtime::command(id, &format!("{base} profiler start --timeout {PROFILE_SECS}"))?;
    tokio::spawn(follow(id.to_string(), lines));
    status(id).await
}

async fn follow(id: String, mut lines: tokio::sync::broadcast::Receiver<String>) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(PROFILE_SECS) + GRACE;
    let run = runtime::server_run(&id);
    let end = loop {
        match tokio::time::timeout_at(deadline, lines.recv()).await {
            Err(_) => break Reply::Failed("timeout"),
            Ok(Err(RecvError::Closed)) => break Reply::Failed("stopped"),
            Ok(Err(RecvError::Lagged(_))) => {}
            Ok(Ok(line)) => match read_reply(&line) {
                Some(Reply::Running) => set(&id, run, |r| r.state = "profiling"),
                Some(Reply::Uploading) => set(&id, run, |r| r.state = "uploading"),
                // Any server's "unknown command" before spark answered at all, not one someone else typed later.
                Some(Reply::Failed("notLoaded")) if !still_starting(&id, run) => {}
                Some(end) => break end,
                None => {}
            },
        }
    };
    set(&id, run, |r| match end {
        Reply::Url(url) => {
            r.state = "done";
            r.url = Some(url);
        }
        Reply::Failed(why) => {
            r.state = "failed";
            r.error = Some(why);
        }
        _ => {}
    });
}

fn still_starting(id: &str, run: Option<u64>) -> bool {
    run == runtime::server_run(id) && with_run(id, |r| r.report.as_ref().is_some_and(|x| x.state == "starting")).unwrap_or(false)
}

/// Changes the report of run `run`; a report outlives neither its run nor a newer report.
fn set(id: &str, run: Option<u64>, f: impl FnOnce(&mut SparkReport)) {
    if run.is_some() && run == runtime::server_run(id) {
        with_run(id, |r| r.report.as_mut().map(f));
    } else if let Some(r) = RUNS.lock().unwrap().get_mut(id).filter(|r| Some(r.run) == run) {
        // The server ended under the report.
        r.report.as_mut().map(f);
    }
}

#[derive(Debug, PartialEq)]
enum Reply {
    Running,
    Uploading,
    Url(String),
    Failed(&'static str),
}

/// What a console line says about the report. Log prefixes and spark's own "[⚡]" are skipped, so a link or a phrase in
/// a player's chat ("<Steve> https://…") doesn't count.
fn read_reply(line: &str) -> Option<Reply> {
    static RE: OnceLock<(Regex, Regex)> = OnceLock::new();
    let (prefix, link) = RE.get_or_init(|| (Regex::new(r"^(?:\[[^\]]*\]:?\s*)+").unwrap(), Regex::new(r"^https://spark\.lucko\.me/[A-Za-z0-9_-]+$").unwrap()));
    let body = prefix.replace(line, "");
    let body = body.trim();
    if link.is_match(body) {
        return Some(Reply::Url(body.to_string()));
    }
    let says = |s: &str| body.starts_with(s);
    if says("Profiler is now running") {
        Some(Reply::Running)
    } else if says("The active profiler has completed") || says("Stopping the profiler & uploading") {
        Some(Reply::Uploading)
    } else if says("Profiler is already running") {
        Some(Reply::Failed("busy"))
    } else if says("An error occurred whilst uploading") {
        Some(Reply::Failed("upload"))
    } else if says("Profiler operation failed") {
        Some(Reply::Failed("failed"))
    } else if says("Unknown command") || says("Unknown or incomplete command") {
        Some(Reply::Failed("notLoaded"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_the_report_link() {
        let want = Some(Reply::Url("https://spark.lucko.me/AbC123xYz".into()));
        assert_eq!(read_reply("https://spark.lucko.me/AbC123xYz"), want);
        assert_eq!(read_reply("[12:00:59 INFO]: https://spark.lucko.me/AbC123xYz"), want);
        assert_eq!(read_reply("[12:00:59] [spark-worker-pool-1-thread-2/INFO]: https://spark.lucko.me/AbC123xYz"), want);
        assert_eq!(read_reply("[12:00:59] [Server thread/INFO] [spark/]: [⚡] https://spark.lucko.me/AbC123xYz  "), want);
    }

    #[test]
    fn a_link_in_chat_or_the_bare_viewer_is_not_a_report() {
        assert_eq!(read_reply("[12:00:59 INFO]: <Steve> https://spark.lucko.me/fake"), None);
        assert_eq!(read_reply("[12:00:59 INFO]: https://spark.lucko.me/fake/../evil"), None);
        assert_eq!(read_reply("[12:00:59 INFO]: https://evil.example/spark.lucko.me/x"), None);
        assert_eq!(read_reply("[⚡] You can view the profile file using the web app @ https://spark.lucko.me/"), None);
        assert_eq!(read_reply("https://spark.lucko.me/"), None);
    }

    #[test]
    fn follows_sparks_replies() {
        assert_eq!(read_reply("[12:00:00 INFO]: [⚡] Starting a new profiler, please wait..."), None);
        assert_eq!(read_reply("[12:00:00 INFO]: [⚡] Profiler is now running! (async)"), Some(Reply::Running));
        assert_eq!(read_reply("[12:01:00 INFO]: [⚡] The active profiler has completed! Uploading results..."), Some(Reply::Uploading));
        assert_eq!(read_reply("[⚡] Profiler is already running!"), Some(Reply::Failed("busy")));
        assert_eq!(read_reply("[⚡] An error occurred whilst uploading the results. Attempting to save to disk instead."), Some(Reply::Failed("upload")));
        assert_eq!(read_reply("[12:00:00 INFO]: Unknown command. Type \"/help\" for help."), Some(Reply::Failed("notLoaded")));
        assert_eq!(read_reply("[12:00:00 INFO]: <Steve> Profiler is already running!"), None);
    }

    #[test]
    fn the_command_follows_the_platform() {
        assert_eq!(command_base(ServerKind::Paper), Some("spark"));
        assert_eq!(command_base(ServerKind::Fabric), Some("spark"));
        assert_eq!(command_base(ServerKind::Velocity), Some("sparkv"));
        assert_eq!(command_base(ServerKind::Waterfall), Some("sparkb"));
        assert_eq!(command_base(ServerKind::Vanilla), None);
        assert_eq!(command_base(ServerKind::Bedrock), None);
        assert_eq!(command_base(ServerKind::Mohist), None);
    }

    #[test]
    fn paper_bundles_it_from_1_21() {
        assert!(bundled(ServerKind::Paper, "1.21.1") && bundled(ServerKind::Purpur, "1.21") && bundled(ServerKind::Paper, "26.1"));
        assert!(!bundled(ServerKind::Paper, "1.20.6") && !bundled(ServerKind::Spigot, "1.21.1") && !bundled(ServerKind::Fabric, "1.21.1") && !bundled(ServerKind::Paper, "1.8.8"));
    }
}
