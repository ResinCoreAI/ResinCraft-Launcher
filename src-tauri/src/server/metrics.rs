//! Performance graphs for running servers (`server:metrics`): one sampler reads CPU, RAM and players every few seconds
//! and keeps about the last hour per run, in memory only; a stopped server's history goes with it.
//!
//! CPU is a share of the whole machine (every core together), so a server using 2 of 8 cores reads 25%. RAM is what
//! `core/memory.rs` shows for the server. TPS only exists on Paper-family servers, read with their `tps` command at
//! most every 30 seconds and only after the last one was answered, so a struggling server is never asked twice. The
//! reply is hidden from the console. Spark replaces that command with a longer report, so a Paper server with the
//! spark plugin in plugins/ reports no TPS here (Paper's own bundled spark leaves it alone).

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::{Duration, Instant};

use regex::Regex;
use serde::Serialize;
use sysinfo::{CpuRefreshKind, Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

use crate::core::types::ServerKind;

use super::{runtime, spark, store};

const SAMPLE_EVERY: Duration = Duration::from_secs(5);
/// One hour of samples.
const KEEP: usize = 720;
const TPS_EVERY: Duration = Duration::from_secs(30);
/// An unanswered `tps` is forgotten after this; only then may another be sent.
const TPS_GIVE_UP: Duration = Duration::from_secs(60);
/// A reading older than this isn't drawn as current (the server stopped answering).
const TPS_STALE: Duration = Duration::from_secs(70);

/// Keeps the newest `cap` items.
struct Ring<T> {
    items: VecDeque<T>,
    cap: usize,
}

impl<T: Clone> Ring<T> {
    fn new(cap: usize) -> Self {
        Ring { items: VecDeque::with_capacity(cap), cap }
    }
    fn push(&mut self, item: T) {
        if self.items.len() == self.cap {
            self.items.pop_front();
        }
        self.items.push_back(item);
    }
    fn to_vec(&self) -> Vec<T> {
        self.items.iter().cloned().collect()
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    /// Unix milliseconds.
    pub at: i64,
    /// % of the whole machine; none on a server's first sample (a process needs two readings).
    pub cpu: Option<f32>,
    pub bytes: Option<u64>,
    pub players: i64,
    pub tps: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerMetrics {
    pub interval_secs: u64,
    pub samples: Vec<Sample>,
    /// The server's Java memory limit; none for Bedrock.
    pub max_bytes: Option<u64>,
    /// False: this server has no reliable TPS; show "not available", never a number.
    pub tps_available: bool,
}

/// When to ask a Paper server for its TPS, and the last answer.
#[derive(Default)]
struct Tps {
    available: bool,
    asked: Option<Instant>,
    waiting: bool,
    last: Option<(f32, Instant)>,
}

impl Tps {
    fn due(&self, now: Instant) -> bool {
        let since = self.asked.map(|a| now.duration_since(a));
        self.available && since.is_none_or(|s| s >= if self.waiting { TPS_GIVE_UP } else { TPS_EVERY })
    }
    fn current(&self, now: Instant) -> Option<f32> {
        self.last.filter(|(_, at)| now.duration_since(*at) < TPS_STALE).map(|(v, _)| v)
    }
}

struct Series {
    run: u64,
    samples: Ring<Sample>,
    max_bytes: Option<u64>,
    tps: Tps,
}

static SERIES: LazyLock<Mutex<HashMap<String, Series>>> = LazyLock::new(Default::default);

/// Starts the sampler the first time a server starts.
pub fn ensure_sampler() {
    static STARTED: AtomicBool = AtomicBool::new(false);
    if !STARTED.swap(true, Ordering::Relaxed) {
        tokio::spawn(sampler());
    }
}

async fn sampler() {
    let mut iv = tokio::time::interval(SAMPLE_EVERY);
    iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut sys = System::new_with_specifics(RefreshKind::nothing());
    sys.refresh_cpu_list(CpuRefreshKind::nothing());
    loop {
        iv.tick().await;
        let procs = runtime::server_processes();
        // A stopped server's history goes; a restarted one starts again (a new run).
        SERIES.lock().unwrap().retain(|id, s| runtime::server_run(id) == Some(s.run));
        if procs.is_empty() {
            continue;
        }
        let roots: Vec<(String, u32)> = procs.iter().map(|(id, pid, _)| (id.clone(), *pid)).collect();
        let Ok((back, cpu)) = tokio::task::spawn_blocking(move || {
            let cpu = read_cpu(&mut sys, &roots);
            (sys, cpu)
        })
        .await
        else {
            return;
        };
        sys = back;
        let memory = crate::core::memory::usage().await;
        for (id, _, max_mb) in procs {
            let Some(run) = runtime::server_run(&id) else { continue };
            let known = SERIES.lock().unwrap().get(&id).is_some_and(|s| s.run == run);
            let tps = if known { false } else { tps_readable(&id).await };
            let bytes = memory.servers.iter().find(|m| m.server_id == id).map(|m| m.bytes).filter(|b| *b > 0);
            let players = runtime::online(&id).unwrap_or_else(|| runtime::players(&id).len() as i64);
            let now = Instant::now();
            let ask = {
                let mut all = SERIES.lock().unwrap();
                let s = all.entry(id.clone()).or_insert_with(|| Series {
                    run,
                    samples: Ring::new(KEEP),
                    max_bytes: max_mb.map(|m| m as u64 * 1024 * 1024),
                    tps: Tps { available: tps, ..Default::default() },
                });
                // Its first reading only sets the baseline.
                let cpu = if known { cpu.get(&id).copied() } else { None };
                s.samples.push(Sample { at: now_ms(), cpu, bytes, players, tps: s.tps.current(now) });
                let ask = runtime::state(&id) == "running" && s.tps.due(now);
                if ask {
                    s.tps.asked = Some(now);
                    s.tps.waiting = true;
                }
                ask
            };
            if ask {
                let _ = runtime::quiet_command(&id, "tps");
            }
        }
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

async fn tps_readable(id: &str) -> bool {
    let Ok(meta) = store::get(id).await else { return false };
    let Ok(dir) = store::dir(id) else { return false };
    paper_tps(meta.kind) && !spark::has_jar(&dir.join("plugins")).await
}

/// The servers with Paper's `tps` reply. Folia's answers per region, in another format.
fn paper_tps(kind: ServerKind) -> bool {
    matches!(kind, ServerKind::Paper | ServerKind::Purpur | ServerKind::Leaf | ServerKind::Pufferfish)
}

/// CPU of each server with everything it started, as % of the whole machine.
fn read_cpu(sys: &mut System, roots: &[(String, u32)]) -> HashMap<String, f32> {
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_cpu());
    let cores = sys.cpus().len();
    // Threads (Linux lists them as processes) would count again.
    let procs: Vec<(u32, Option<u32>, f32)> =
        sys.processes().iter().filter(|(_, p)| p.thread_kind().is_none()).map(|(pid, p)| (pid.as_u32(), p.parent().map(Pid::as_u32), p.cpu_usage())).collect();
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for (pid, parent, _) in &procs {
        if let Some(parent) = parent {
            children.entry(*parent).or_default().push(*pid);
        }
    }
    let usage: HashMap<u32, f32> = procs.iter().map(|(pid, _, cpu)| (*pid, *cpu)).collect();
    roots
        .iter()
        .map(|(id, root)| {
            let sum = crate::core::memory::tree(*root, &children, &HashSet::new()).iter().filter_map(|p| usage.get(p)).sum();
            (id.clone(), whole_machine(sum, cores))
        })
        .collect()
}

/// sysinfo counts 100% per core.
fn whole_machine(per_core: f32, cores: usize) -> f32 {
    (per_core / cores.max(1) as f32).clamp(0.0, 100.0)
}

pub fn get(id: &str) -> ServerMetrics {
    let run = runtime::server_run(id);
    let all = SERIES.lock().unwrap();
    let mut out = ServerMetrics { interval_secs: SAMPLE_EVERY.as_secs(), ..Default::default() };
    if let Some(s) = all.get(id).filter(|s| Some(s.run) == run) {
        out.samples = s.samples.to_vec();
        out.max_bytes = s.max_bytes;
        out.tps_available = s.tps.available;
    }
    out
}

/// Takes the reply to our own `tps` out of the console; true when the line is hidden. Only one reply per question,
/// so a `tps` typed by hand still shows.
pub fn hide_line(id: &str, text: &str) -> bool {
    if !text.contains("TPS from last") {
        return false;
    }
    let Some(tps) = parse_tps(text) else { return false };
    let mut all = SERIES.lock().unwrap();
    let Some(s) = all.get_mut(id).filter(|s| s.tps.waiting) else { return false };
    s.tps.waiting = false;
    s.tps.last = Some((tps, Instant::now()));
    true
}

/// The 1-minute average from a `tps` reply: "[12:00:01 INFO]: TPS from last 1m, 5m, 15m: *20.0, 19.98, 18.5" (Purpur
/// adds 5s first). `*` marks a value capped at 20. Anything after a log prefix (a player's chat line) doesn't match.
pub fn parse_tps(line: &str) -> Option<f32> {
    static RE: OnceLock<(Regex, Regex)> = OnceLock::new();
    let (colours, reply) = RE.get_or_init(|| {
        (
            Regex::new(r"§[0-9a-fk-orxA-FK-ORX]|\x1b\[[0-9;]*m").unwrap(),
            Regex::new(r"^(?:\[[^\]]*\]:?\s*)*TPS from last ([^:]+):\s*(.+?)\s*$").unwrap(),
        )
    });
    let plain = colours.replace_all(line, "");
    let c = reply.captures(&plain)?;
    let labels: Vec<&str> = c[1].split(',').map(str::trim).collect();
    let values: Vec<f32> = c[2].split(',').map(|v| v.trim().trim_start_matches('*').parse().ok()).collect::<Option<_>>()?;
    let i = labels.iter().position(|l| *l == "1m")?;
    (labels.len() == values.len()).then(|| values[i]).filter(|v| v.is_finite() && *v >= 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_keeps_the_newest() {
        let mut r = Ring::new(3);
        assert!(r.to_vec().is_empty());
        r.push(1);
        r.push(2);
        assert_eq!(r.to_vec(), [1, 2]);
        r.push(3);
        r.push(4);
        r.push(5);
        assert_eq!(r.to_vec(), [3, 4, 5]);
    }

    #[test]
    fn an_hour_fits() {
        assert_eq!(KEEP as u64 * SAMPLE_EVERY.as_secs(), 3600);
    }

    #[test]
    fn reads_papers_tps_reply() {
        assert_eq!(parse_tps("[12:00:01 INFO]: TPS from last 1m, 5m, 15m: *20.0, *20.0, *20.0"), Some(20.0));
        assert_eq!(parse_tps("TPS from last 1m, 5m, 15m: 18.5, 19.02, 19.99"), Some(18.5));
        assert_eq!(parse_tps("[12:00:01] [Server thread/INFO]: TPS from last 1m, 5m, 15m: 7.12, 19.02, 19.99"), Some(7.12));
    }

    #[test]
    fn reads_colour_codes_in_the_reply() {
        assert_eq!(parse_tps("[12:00:01 INFO]: §6TPS from last 1m, 5m, 15m: §a*20.0§6, §a*20.0§6, §e17.3"), Some(20.0));
        assert_eq!(parse_tps("\x1b[0;33mTPS from last 1m, 5m, 15m: \x1b[0;32;1m19.5\x1b[m, 20.0, 20.0"), Some(19.5));
    }

    #[test]
    fn purpur_lists_5s_first_and_the_1m_value_is_still_read() {
        assert_eq!(parse_tps("[12:00:01 INFO]: TPS from last 5s, 1m, 5m, 15m: *20.0, 15.5, 19.0, 19.5"), Some(15.5));
    }

    #[test]
    fn other_lines_are_not_tps() {
        assert_eq!(parse_tps("[12:00:01 INFO]: <Steve> TPS from last 1m, 5m, 15m: 1.0, 1.0, 1.0"), None);
        assert_eq!(parse_tps("[12:00:01 INFO]: TPS from last 1m, 5m, 15m: soon"), None);
        // spark's own reply puts the values on the next line.
        assert_eq!(parse_tps("[12:00:01 INFO]: TPS from last 5s, 10s, 1m, 5m, 15m:"), None);
        assert_eq!(parse_tps("[12:00:01 INFO]: TPS from last 1m, 5m, 15m: 20.0, 20.0"), None);
    }

    #[test]
    fn asks_at_most_every_30s_and_never_twice_unanswered() {
        let t0 = Instant::now();
        let mut tps = Tps { available: true, ..Default::default() };
        assert!(tps.due(t0));
        tps.asked = Some(t0);
        tps.waiting = true;
        assert!(!tps.due(t0 + Duration::from_secs(45)), "still unanswered");
        assert!(tps.due(t0 + TPS_GIVE_UP), "given up on");
        tps.waiting = false;
        assert!(!tps.due(t0 + Duration::from_secs(29)));
        assert!(tps.due(t0 + TPS_EVERY));
        assert!(!Tps::default().due(t0), "other servers are never asked");
    }

    #[test]
    fn only_a_fresh_reading_counts() {
        let t0 = Instant::now();
        let tps = Tps { last: Some((19.0, t0)), ..Default::default() };
        assert_eq!(tps.current(t0 + Duration::from_secs(30)), Some(19.0));
        assert_eq!(tps.current(t0 + TPS_STALE), None);
    }

    #[test]
    fn cpu_is_a_share_of_the_whole_machine() {
        assert_eq!(whole_machine(200.0, 8), 25.0);
        assert_eq!(whole_machine(900.0, 8), 100.0);
        assert_eq!(whole_machine(50.0, 0), 50.0);
    }

    #[test]
    fn only_paper_family_servers_have_tps() {
        assert!(paper_tps(ServerKind::Paper) && paper_tps(ServerKind::Purpur));
        assert!(!paper_tps(ServerKind::Folia) && !paper_tps(ServerKind::Velocity) && !paper_tps(ServerKind::Fabric) && !paper_tps(ServerKind::Vanilla));
    }
}
