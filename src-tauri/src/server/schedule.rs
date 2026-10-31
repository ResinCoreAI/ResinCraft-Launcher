//! Scheduled tasks: per server, a backup, a restart or a console command, daily, weekly or every N hours, in local
//! time. They're kept in `schedule.json` in the server's folder (so deleting the server deletes them) and never
//! travel to the web console: `remote.rs` doesn't list these channels.
//!
//! One loop looks every TICK for times that fell since its last look, so runs missed while the app was closed, or
//! while the PC slept, simply don't happen. A server's tasks run one at a time, in list order when they fall together.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use chrono::{Datelike, Days, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::events::list_changed;
use crate::core::fsutil::{now_ms, read_json, write_json};
use crate::core::router::json;
use crate::core::types::{ServerKind, ServerMeta};
use crate::core::{backupfiles, Args};

use super::{runtime, store};

const FILE: &str = "schedule.json";
/// How often the loop looks for tasks that are due.
const TICK: Duration = Duration::from_secs(20);
/// A longer gap between looks means the PC slept (or the clock moved): what fell in it counts as missed.
const LATE_MS: i64 = 5 * 60_000;
/// How long players get between the warning and a restart.
const WARNING: Duration = Duration::from_secs(60);
/// Longer than runtime's STOP_TIMEOUT, after which the server is killed.
const STOP_WAIT: Duration = Duration::from_secs(120);
/// How long a restart watches the server come back up, so the next task finds it ready.
const START_WAIT: Duration = Duration::from_secs(600);
const NOT_RUNNING: &str = "The server isn't running";
const IN_TRANSITION: &str = "The server is starting or stopping";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Action {
    /// `keep`: how many of this task's own backups stay; the oldest beyond that are deleted.
    Backup {
        #[serde(default)]
        keep: Option<u32>,
    },
    Restart,
    Command {
        command: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Timing {
    /// `at`: "HH:MM", local time.
    Daily { at: String },
    /// `days`: 0 = Monday … 6 = Sunday.
    Weekly { days: Vec<u8>, at: String },
    /// Counted from the task's `anchor`, in real hours whatever the clocks do.
    Hours { every: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Ok,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastRun {
    /// When it started (ms).
    pub at: i64,
    pub outcome: Outcome,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub enabled: bool,
    pub action: Action,
    pub timing: Timing,
    /// When the task or its timing was last set (ms): nothing before this counts, and `Hours` counts from it.
    pub anchor: i64,
    #[serde(default)]
    pub last_run: Option<LastRun>,
    /// The backups this task made, oldest first: "keep last N" only ever deletes these.
    #[serde(default)]
    pub made: Vec<String>,
}

/// What the UI sends: a new task without `id`, or the task to change.
#[derive(Debug, Deserialize)]
pub struct TaskEdit {
    #[serde(default)]
    id: Option<String>,
    enabled: bool,
    action: Action,
    timing: Timing,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskView {
    #[serde(flatten)]
    task: Task,
    /// The next run (ms); none while it's off.
    next: Option<i64>,
    /// Running, or waiting for the server's other task.
    running: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Schedule {
    #[serde(default)]
    tasks: Vec<Task>,
}

/// Every read-change-write of a schedule file goes through this, so a run's result can't undo an edit.
static FILE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
/// (server, task) running or queued.
static CLAIMS: Mutex<BTreeSet<(String, String)>> = Mutex::new(BTreeSet::new());
/// One gate per server: a task holds it while it runs.
static GATES: Mutex<BTreeMap<String, Arc<tokio::sync::Mutex<()>>>> = Mutex::new(BTreeMap::new());
/// The app is closing: nothing new starts.
static HALTED: AtomicBool = AtomicBool::new(false);

// ---------- When ----------

/// The time zone, apart so the tests can have clocks that change.
trait Zone {
    fn wall_clock(&self, ms: i64) -> Option<NaiveDateTime>;
    fn moments(&self, at: NaiveDateTime) -> LocalResult<i64>;
}

struct Here;

impl Zone for Here {
    fn wall_clock(&self, ms: i64) -> Option<NaiveDateTime> {
        chrono::Local.timestamp_millis_opt(ms).single().map(|t| t.naive_local())
    }
    fn moments(&self, at: NaiveDateTime) -> LocalResult<i64> {
        chrono::Local.from_local_datetime(&at).map(|t| t.timestamp_millis())
    }
}

/// When the clocks show `at`: the first time when they show it twice (so it runs once), and just after the jump when
/// they skip it (so it still runs that day).
fn moment(zone: &impl Zone, at: NaiveDateTime) -> Option<i64> {
    (0..=8).find_map(|quarters| match zone.moments(at.checked_add_signed(TimeDelta::minutes(15 * quarters))?) {
        LocalResult::Single(ms) => Some(ms),
        LocalResult::Ambiguous(a, b) => Some(a.min(b)),
        LocalResult::None => None,
    })
}

fn clock_time(at: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(at, "%H:%M").ok()
}

/// The first day from `after`'s that passes `day`, whose time `at` is past `after`. Starts a day early for a time
/// pushed over midnight by a clock change.
fn next_at(zone: &impl Zone, at: &str, after: i64, day: impl Fn(NaiveDate) -> bool) -> Option<i64> {
    let at = clock_time(at)?;
    let first = zone.wall_clock(after)?.date().checked_sub_days(Days::new(1))?;
    (0..=9u64).filter_map(|n| first.checked_add_days(Days::new(n))).filter(|d| day(*d)).filter_map(|d| moment(zone, d.and_time(at))).find(|ms| *ms > after)
}

/// The first run after `after`. None when the timing can't happen (a hand-edited file).
fn next_run(task: &Task, after: i64, zone: &impl Zone) -> Option<i64> {
    // Never again for a time it already ran for, even if the clock is set back.
    let after = after.max(task.anchor).max(task.last_run.as_ref().map_or(i64::MIN, |l| l.at));
    match &task.timing {
        Timing::Daily { at } => next_at(zone, at, after, |_| true),
        Timing::Weekly { days, at } => next_at(zone, at, after, |d| days.contains(&(d.weekday().num_days_from_monday() as u8))),
        Timing::Hours { every } => {
            let step = i64::from(*every).checked_mul(3_600_000).filter(|s| *s > 0)?;
            let done = after.saturating_sub(task.anchor).div_euclid(step);
            task.anchor.checked_add(done.checked_add(1)?.max(1).checked_mul(step)?)
        }
    }
}

/// Due in the look that spans `prev` to `now`.
fn is_due(task: &Task, prev: i64, now: i64, zone: &impl Zone) -> bool {
    task.enabled && next_run(task, prev, zone).is_some_and(|next| next <= now)
}

/// A gap this long (or a clock set back) means nothing falls due in this look.
fn asleep(prev: i64, now: i64) -> bool {
    !(0..=LATE_MS).contains(&(now - prev))
}

// ---------- Rules ----------

/// Why the action can't run on a server in `state`.
fn skip_reason(action: &Action, state: &str) -> Option<&'static str> {
    match action {
        Action::Backup { .. } => matches!(state, "starting" | "stopping").then_some(IN_TRANSITION),
        Action::Restart | Action::Command { .. } => (state != "running").then_some(NOT_RUNNING),
    }
}

/// Proxies have no `say` command, so nobody can be told; the restart goes ahead without the wait.
fn can_warn(kind: ServerKind) -> bool {
    !kind.proxy()
}

/// Takes the oldest of `made` past the newest `keep` out of it; returns them, to be deleted.
fn past_keep(made: &mut Vec<String>, keep: Option<u32>) -> Vec<String> {
    let extra = keep.map_or(0, |k| made.len().saturating_sub(k as usize));
    made.drain(..extra).collect()
}

fn validate(action: &mut Action, timing: &Timing) -> Result<()> {
    match action {
        Action::Backup { keep: Some(0) } => bail!("Keep at least 1 backup"),
        Action::Backup { .. } | Action::Restart => {}
        Action::Command { command } => {
            *command = command.trim().trim_start_matches('/').to_string();
            if command.is_empty() {
                bail!("Type the command to run");
            }
            if command.contains(['\n', '\r']) {
                bail!("A command must be a single line");
            }
        }
    }
    match timing {
        Timing::Daily { at } if clock_time(at).is_none() => bail!("Choose a time"),
        Timing::Weekly { at, .. } if clock_time(at).is_none() => bail!("Choose a time"),
        Timing::Weekly { days, .. } if days.is_empty() || days.iter().any(|d| *d > 6) => bail!("Choose at least one day"),
        Timing::Hours { every } if !(1..=720).contains(every) => bail!("The hours must be a number from 1 to 720"),
        _ => Ok(()),
    }
}

// ---------- Saved tasks ----------

async fn load(id: &str) -> Schedule {
    let Ok(dir) = store::dir(id) else { return Schedule::default() };
    read_json(&dir.join(FILE)).await.unwrap_or_default()
}

async fn save(id: &str, schedule: &Schedule) -> Result<()> {
    write_json(&store::dir(id)?.join(FILE), schedule).await
}

async fn view(id: &str) -> Vec<TaskView> {
    let now = now_ms();
    let claims = CLAIMS.lock().unwrap().clone();
    load(id)
        .await
        .tasks
        .into_iter()
        .map(|task| TaskView {
            next: if task.enabled { next_run(&task, now, &Here) } else { None },
            running: claims.contains(&(id.to_string(), task.id.clone())),
            task,
        })
        .collect()
}

async fn put(id: &str, edit: TaskEdit) -> Result<()> {
    let TaskEdit { id: task_id, enabled, mut action, timing } = edit;
    validate(&mut action, &timing)?;
    let _file = FILE_LOCK.lock().await;
    let mut schedule = load(id).await;
    let now = now_ms();
    match task_id {
        Some(task_id) => {
            let task = schedule.tasks.iter_mut().find(|t| t.id == task_id).ok_or_else(|| anyhow!("That task no longer exists"))?;
            if task.timing != timing || (enabled && !task.enabled) {
                task.anchor = now;
            }
            (task.enabled, task.action, task.timing) = (enabled, action, timing);
        }
        None => {
            let task_id = uuid::Uuid::new_v4().simple().to_string()[..8].to_string();
            schedule.tasks.push(Task { id: task_id, enabled, action, timing, anchor: now, last_run: None, made: Vec::new() });
        }
    }
    save(id, &schedule).await
}

async fn remove(id: &str, task_id: &str) -> Result<()> {
    let _file = FILE_LOCK.lock().await;
    let mut schedule = load(id).await;
    schedule.tasks.retain(|t| t.id != task_id);
    save(id, &schedule).await
}

/// Saves how a run went and, for a backup, adds the zip and deletes what's past "keep last N". The zips really go
/// (Recycle Bin would keep the disk full), and only names from this task's own list, inside the backups folder.
async fn finish(id: &str, task_id: &str, last: LastRun, made: Option<String>) {
    let _file = FILE_LOCK.lock().await;
    // Never brings back the folder of a server deleted meanwhile.
    if store::get(id).await.is_err() {
        return;
    }
    let mut schedule = load(id).await;
    let Some(task) = schedule.tasks.iter_mut().find(|t| t.id == task_id) else { return };
    task.last_run = Some(last);
    if let (Some(made), Ok(dir)) = (made, store::dir(id)) {
        let backups = dir.join("backups");
        task.made.push(made);
        task.made.retain(|f| backups.join(f).is_file());
        let keep = if let Action::Backup { keep } = task.action { keep } else { None };
        for old in past_keep(&mut task.made, keep) {
            if let Ok(path) = backupfiles::path_of(&backups, &old) {
                if let Err(e) = tokio::fs::remove_file(&path).await {
                    eprintln!("[schedule] couldn't delete {old}: {e}");
                }
            }
        }
    }
    if let Err(e) = save(id, &schedule).await {
        eprintln!("[schedule] couldn't save the result for {id}: {e:#}");
    }
}

// ---------- Running ----------

fn claim(id: &str, task_id: &str) -> bool {
    CLAIMS.lock().unwrap().insert((id.to_string(), task_id.to_string()))
}

fn release(id: &str, task_id: &str) {
    CLAIMS.lock().unwrap().remove(&(id.to_string(), task_id.to_string()));
}

fn gate(id: &str) -> Arc<tokio::sync::Mutex<()>> {
    GATES.lock().unwrap().entry(id.to_string()).or_default().clone()
}

/// A task of this server is running or queued (its restart may have the server stopped for a moment).
pub fn busy(id: &str) -> bool {
    CLAIMS.lock().unwrap().iter().any(|(server, _)| server == id)
}

/// The server was deleted (its tasks went with its folder).
pub fn forget(id: &str) {
    GATES.lock().unwrap().remove(id);
}

/// The app is closing: a restart under way doesn't start its server again.
pub fn halt() {
    HALTED.store(true, Ordering::SeqCst);
}

/// Runs `tasks` in order, each after whatever the server is already running. A task that's still running or queued
/// isn't queued twice.
fn fire(id: String, tasks: Vec<String>, manual: bool) {
    let tasks: Vec<String> = tasks.into_iter().filter(|t| claim(&id, t)).collect();
    if tasks.is_empty() {
        return;
    }
    list_changed("servers");
    tokio::spawn(async move {
        let gate = gate(&id);
        for task in tasks {
            let turn = gate.lock().await;
            run(&id, &task, manual).await;
            release(&id, &task);
            drop(turn);
            list_changed("servers");
        }
    });
}

async fn run(id: &str, task_id: &str, manual: bool) {
    let Some(task) = load(id).await.tasks.into_iter().find(|t| t.id == task_id) else { return };
    // Turned off while it waited for its turn.
    if !task.enabled && !manual {
        return;
    }
    let Ok(meta) = store::get(id).await else { return };
    let at = now_ms();
    let mut made = None;
    let (outcome, message) = match skip_reason(&task.action, runtime::state(id)) {
        Some(reason) => (Outcome::Skipped, Some(reason.to_string())),
        None => match perform(&task.action, &meta).await {
            Ok(Done::Ran(note)) => (Outcome::Ok, note),
            Ok(Done::Backup(file)) => {
                let note = Some(file.clone());
                made = Some(file);
                (Outcome::Ok, note)
            }
            Ok(Done::Skipped(reason)) => (Outcome::Skipped, Some(reason.to_string())),
            Err(e) => (Outcome::Failed, Some(format!("{e:#}"))),
        },
    };
    if outcome != Outcome::Ok {
        eprintln!("[schedule] {}: {outcome:?}: {}", meta.name, message.as_deref().unwrap_or_default());
    }
    finish(id, task_id, LastRun { at, outcome, message }, made).await;
}

enum Done {
    Ran(Option<String>),
    /// The zip's file name.
    Backup(String),
    Skipped(&'static str),
}

async fn perform(action: &Action, meta: &ServerMeta) -> Result<Done> {
    let id = meta.id.as_str();
    match action {
        Action::Backup { .. } => {
            let path = runtime::backup(id).await?;
            let file = std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(path);
            Ok(Done::Backup(file))
        }
        Action::Command { command } => {
            runtime::command(id, command)?;
            Ok(Done::Ran(None))
        }
        Action::Restart => restart(meta).await,
    }
}

async fn restart(meta: &ServerMeta) -> Result<Done> {
    let id = meta.id.as_str();
    if can_warn(meta.kind) {
        runtime::command(id, "say Restarting in 1 minute")?;
        let deadline = tokio::time::Instant::now() + WARNING;
        while tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_secs(1)).await;
            // Someone stopped it meanwhile, or the app is closing.
            if runtime::state(id) != "running" || HALTED.load(Ordering::SeqCst) {
                return Ok(Done::Skipped(NOT_RUNNING));
            }
        }
    }
    runtime::stop(id)?;
    runtime::wait_state(id, &["stopped", "crashed"], STOP_WAIT).await?;
    if HALTED.load(Ordering::SeqCst) {
        return Ok(Done::Skipped("The app was closing"));
    }
    runtime::start(id).await?;
    match runtime::wait_state(id, &["running", "stopped", "crashed"], START_WAIT).await {
        Ok("running") => Ok(Done::Ran(None)),
        Ok(state) => bail!("The server {state} while starting again"),
        Err(_) => Ok(Done::Ran(Some("Still starting".to_string()))),
    }
}

/// The background loop (one per app, spawned at startup).
pub async fn init() {
    let mut prev = now_ms();
    while !HALTED.load(Ordering::SeqCst) {
        tokio::time::sleep(TICK).await;
        let now = now_ms();
        if !asleep(prev, now) {
            for meta in store::list().await.unwrap_or_default() {
                let due = load(&meta.id).await.tasks.into_iter().filter(|t| is_due(t, prev, now, &Here)).map(|t| t.id).collect();
                fire(meta.id, due, false);
            }
        }
        prev = now;
    }
}

/// `server:schedule*`. Each answers with the server's tasks.
pub async fn handle(channel: &str, args: Args) -> Result<Value> {
    let id: String = args.get(0)?;
    store::get(&id).await?;
    match channel {
        "server:schedule" => {}
        "server:scheduleSave" => put(&id, args.get(1)?).await?,
        "server:scheduleDelete" => remove(&id, &args.get::<String>(1)?).await?,
        "server:scheduleRun" => {
            let task: String = args.get(1)?;
            if !load(&id).await.tasks.iter().any(|t| t.id == task) {
                bail!("That task no longer exists");
            }
            if CLAIMS.lock().unwrap().contains(&(id.clone(), task.clone())) {
                bail!("That task is already running");
            }
            fire(id.clone(), vec![task], true);
        }
        _ => bail!("Unknown request: {channel}"),
    }
    json(view(&id).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Central Europe in 2026: UTC+1, UTC+2 from 29 March 02:00 (to 03:00) until 25 October 03:00 (back to 02:00).
    struct Berlin;

    fn date(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, min, 0).unwrap()
    }

    fn utc_ms(at: NaiveDateTime) -> i64 {
        at.and_utc().timestamp_millis()
    }

    fn offset_hours(ms: i64) -> i64 {
        // In UTC: the changes happen at 01:00 both times.
        if (utc_ms(date(2026, 3, 29, 1, 0))..utc_ms(date(2026, 10, 25, 1, 0))).contains(&ms) { 2 } else { 1 }
    }

    impl Zone for Berlin {
        fn wall_clock(&self, ms: i64) -> Option<NaiveDateTime> {
            chrono::DateTime::from_timestamp_millis(ms.checked_add(offset_hours(ms) * 3_600_000)?).map(|t| t.naive_utc())
        }
        fn moments(&self, at: NaiveDateTime) -> LocalResult<i64> {
            let fits: Vec<i64> = [2, 1].iter().map(|h| utc_ms(at) - h * 3_600_000).filter(|ms| [2, 1].contains(&offset_hours(*ms)) && utc_ms(at) - ms == offset_hours(*ms) * 3_600_000).collect();
            match fits[..] {
                [] => LocalResult::None,
                [one] => LocalResult::Single(one),
                [a, b, ..] => LocalResult::Ambiguous(a, b),
            }
        }
    }

    /// The moment Berlin's clocks show `at` (the first, when they show it twice).
    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> i64 {
        moment(&Berlin, date(y, m, d, h, min)).unwrap()
    }

    fn task(timing: Timing) -> Task {
        Task { id: "t".into(), enabled: true, action: Action::Restart, timing, anchor: 0, last_run: None, made: Vec::new() }
    }

    fn daily(time: &str) -> Task {
        task(Timing::Daily { at: time.into() })
    }

    fn weekly(days: &[u8], time: &str) -> Task {
        task(Timing::Weekly { days: days.to_vec(), at: time.into() })
    }

    /// Every run from `from` to `to`, with the loop looking every 20 seconds as the real one does.
    fn runs(mut t: Task, from: i64, to: i64) -> Vec<i64> {
        let mut out = Vec::new();
        let mut prev = from;
        while prev < to {
            let now = prev + 20_000;
            if is_due(&t, prev, now, &Berlin) {
                out.push(now);
                t.last_run = Some(LastRun { at: now, outcome: Outcome::Ok, message: None });
            }
            prev = now;
        }
        out
    }

    #[test]
    fn daily_runs_today_then_tomorrow() {
        let t = daily("04:30");
        assert_eq!(next_run(&t, at(2026, 6, 10, 3, 0), &Berlin), Some(at(2026, 6, 10, 4, 30)));
        assert_eq!(next_run(&t, at(2026, 6, 10, 4, 30), &Berlin), Some(at(2026, 6, 11, 4, 30)), "not again at the same minute");
        assert_eq!(next_run(&t, at(2026, 6, 10, 5, 0), &Berlin), Some(at(2026, 6, 11, 4, 30)));
    }

    #[test]
    fn months_and_years_roll_over() {
        let t = daily("00:15");
        assert_eq!(next_run(&t, at(2026, 6, 30, 23, 0), &Berlin), Some(at(2026, 7, 1, 0, 15)));
        assert_eq!(next_run(&t, at(2026, 12, 31, 23, 59), &Berlin), Some(at(2027, 1, 1, 0, 15)));
        assert_eq!(next_run(&t, at(2028, 2, 28, 23, 0), &Berlin), Some(at(2028, 2, 29, 0, 15)), "a leap day");
        // 31 December 2026 is a Thursday; the next Monday is in 2027.
        assert_eq!(next_run(&weekly(&[0], "12:00"), at(2026, 12, 31, 13, 0), &Berlin), Some(at(2027, 1, 4, 12, 0)));
    }

    #[test]
    fn weekly_runs_on_the_chosen_days() {
        // 8 June 2026 is a Monday.
        let t = weekly(&[0, 3], "20:00");
        assert_eq!(next_run(&t, at(2026, 6, 8, 19, 0), &Berlin), Some(at(2026, 6, 8, 20, 0)));
        assert_eq!(next_run(&t, at(2026, 6, 8, 20, 0), &Berlin), Some(at(2026, 6, 11, 20, 0)), "Thursday");
        assert_eq!(next_run(&t, at(2026, 6, 12, 8, 0), &Berlin), Some(at(2026, 6, 15, 20, 0)), "the next Monday");
        // One day only, already past: a whole week on. Sunday too (6).
        assert_eq!(next_run(&weekly(&[6], "10:00"), at(2026, 6, 14, 11, 0), &Berlin), Some(at(2026, 6, 21, 10, 0)));
    }

    #[test]
    fn every_n_hours_keeps_to_its_own_grid() {
        let start = at(2026, 6, 10, 9, 17);
        let hours = |n: i64| 3_600_000 * n;
        let t = Task { anchor: start, ..task(Timing::Hours { every: 6 }) };
        assert_eq!(next_run(&t, start, &Berlin), Some(start + hours(6)));
        assert_eq!(next_run(&t, start + hours(6), &Berlin), Some(start + hours(12)));
        assert_eq!(next_run(&t, start + hours(7), &Berlin), Some(start + hours(12)));
        // A run by hand doesn't move the grid.
        let by_hand = Task { last_run: Some(LastRun { at: start + hours(1), outcome: Outcome::Ok, message: None }), ..t.clone() };
        assert_eq!(next_run(&by_hand, start + hours(1), &Berlin), Some(start + hours(6)));
        // Not before it was set.
        assert_eq!(next_run(&t, start - hours(100), &Berlin), Some(start + hours(6)));
    }

    #[test]
    fn every_n_hours_ignores_the_clock_change() {
        let start = at(2026, 10, 24, 12, 0);
        let t = Task { anchor: start, ..task(Timing::Hours { every: 6 }) };
        let got = runs(t, start, start + 48 * 3_600_000);
        assert_eq!(got.len(), 8);
        assert!(got.windows(2).all(|w| w[1] - w[0] == 6 * 3_600_000), "real hours apart: {got:?}");
    }

    #[test]
    fn a_time_the_clocks_skip_still_runs_once() {
        let t = daily("02:30");
        // 29 March has no 02:30: it runs when the clocks reach 03:00.
        assert_eq!(next_run(&t, at(2026, 3, 28, 12, 0), &Berlin), Some(at(2026, 3, 29, 3, 0)));
        let got = runs(t, at(2026, 3, 27, 12, 0), at(2026, 3, 31, 12, 0));
        assert_eq!(got.len(), 4, "27→31 March holds four 02:30s, skipped one included: {got:?}");
    }

    #[test]
    fn a_time_the_clocks_show_twice_runs_once() {
        let t = daily("02:30");
        let first = at(2026, 10, 25, 2, 30);
        assert_eq!(next_run(&t, first - 1, &Berlin), Some(first));
        // Between the two 02:30s: the next run is tomorrow's.
        assert_eq!(next_run(&t, first + 1_800_000, &Berlin), Some(at(2026, 10, 26, 2, 30)));
        let got = runs(t, at(2026, 10, 24, 12, 0), at(2026, 10, 27, 12, 0));
        assert_eq!(got.len(), 3, "{got:?}");
    }

    #[test]
    fn every_day_of_a_year_runs_exactly_once() {
        // 366 looks by day would be slow at 20 s a look; the changes are what matters, and runs() walks them above.
        let t = daily("02:30");
        let mut after = at(2026, 1, 1, 0, 0);
        let mut seen = BTreeSet::new();
        for _ in 0..365 {
            let next = next_run(&t, after, &Berlin).unwrap();
            assert!(next > after);
            assert!(seen.insert(Berlin.wall_clock(next).unwrap().date()), "two runs in one day");
            after = next;
        }
        assert_eq!(seen.len(), 365);
    }

    #[test]
    fn nonsense_from_a_hand_edited_file_never_panics() {
        let now = at(2026, 6, 10, 12, 0);
        assert_eq!(next_run(&weekly(&[], "10:00"), now, &Berlin), None);
        assert_eq!(next_run(&weekly(&[9], "10:00"), now, &Berlin), None);
        assert_eq!(next_run(&daily("25:99"), now, &Berlin), None);
        assert_eq!(next_run(&daily(""), now, &Berlin), None);
        assert_eq!(next_run(&task(Timing::Hours { every: 0 }), now, &Berlin), None);
        let far = Task { anchor: i64::MAX, ..task(Timing::Hours { every: u32::MAX }) };
        assert_eq!(next_run(&far, i64::MIN, &Berlin), None);
        assert_eq!(next_run(&daily("10:00"), i64::MAX, &Berlin), None);
        assert!(!is_due(&daily("10:00"), i64::MAX - 1, i64::MAX, &Berlin));
    }

    #[test]
    fn runs_missed_while_closed_or_asleep_arent_caught_up() {
        // Nothing before the app opened is looked at: the first look starts at now.
        let t = daily("04:00");
        let opened = at(2026, 6, 10, 12, 0);
        assert!(!is_due(&t, opened, opened + 20_000, &Berlin));
        // The PC slept through 04:00 tonight.
        assert!(asleep(at(2026, 6, 10, 23, 0), at(2026, 6, 11, 8, 0)));
        assert!(asleep(1_000, 0), "the clock went back");
        assert!(!asleep(0, 20_000));
    }

    #[test]
    fn a_task_set_after_its_time_waits_for_the_next() {
        let at_noon = at(2026, 6, 10, 12, 0);
        let t = Task { anchor: at_noon + 5_000, ..daily("12:00") };
        assert!(!is_due(&t, at_noon - 10_000, at_noon + 10_000, &Berlin));
    }

    #[test]
    fn a_task_that_is_off_never_runs() {
        let t = Task { enabled: false, ..daily("12:00") };
        let noon = at(2026, 6, 10, 12, 0);
        assert!(!is_due(&t, noon - 10_000, noon + 10_000, &Berlin));
    }

    #[test]
    fn the_clock_set_back_doesnt_run_it_twice() {
        let noon = at(2026, 6, 10, 12, 0);
        let t = Task { last_run: Some(LastRun { at: noon + 5_000, outcome: Outcome::Ok, message: None }), ..daily("12:00") };
        assert!(!is_due(&t, noon - 10_000, noon + 10_000, &Berlin));
    }

    #[test]
    fn restart_and_command_need_a_running_server() {
        let command = Action::Command { command: "say hi".into() };
        for action in [Action::Restart, command] {
            assert_eq!(skip_reason(&action, "running"), None);
            for state in ["stopped", "crashed", "starting", "stopping"] {
                assert_eq!(skip_reason(&action, state), Some(NOT_RUNNING), "{state}");
            }
        }
    }

    #[test]
    fn backups_run_except_mid_start_or_stop() {
        let action = Action::Backup { keep: None };
        for state in ["running", "stopped", "crashed"] {
            assert_eq!(skip_reason(&action, state), None, "{state}");
        }
        for state in ["starting", "stopping"] {
            assert_eq!(skip_reason(&action, state), Some(IN_TRANSITION), "{state}");
        }
    }

    #[test]
    fn proxies_get_no_warning() {
        assert!(can_warn(ServerKind::Paper) && can_warn(ServerKind::Bedrock));
        assert!(!can_warn(ServerKind::Velocity) && !can_warn(ServerKind::Bungeecord));
    }

    #[test]
    fn keep_last_n_takes_only_the_oldest_of_the_tasks_own() {
        let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let mut made = names(&["a.zip", "b.zip", "c.zip", "d.zip"]);
        assert_eq!(past_keep(&mut made, Some(3)), names(&["a.zip"]));
        assert_eq!(made, names(&["b.zip", "c.zip", "d.zip"]));
        assert!(past_keep(&mut made, Some(3)).is_empty());
        assert!(past_keep(&mut made, Some(10)).is_empty());
        assert!(past_keep(&mut made, None).is_empty(), "no limit set: nothing goes");
        assert_eq!(made.len(), 3);
    }

    #[test]
    fn bad_tasks_are_refused() {
        let every_day = Timing::Daily { at: "04:00".into() };
        let ok = |mut a: Action, t: &Timing| validate(&mut a, t).is_ok();
        assert!(ok(Action::Restart, &every_day));
        assert!(!ok(Action::Backup { keep: Some(0) }, &every_day));
        assert!(!ok(Action::Command { command: " / ".into() }, &every_day));
        assert!(!ok(Action::Command { command: "say a\nstop".into() }, &every_day));
        assert!(!ok(Action::Restart, &Timing::Daily { at: "24:00".into() }));
        assert!(!ok(Action::Restart, &Timing::Weekly { days: vec![], at: "04:00".into() }));
        assert!(!ok(Action::Restart, &Timing::Weekly { days: vec![7], at: "04:00".into() }));
        assert!(!ok(Action::Restart, &Timing::Hours { every: 0 }));
        let mut slash = Action::Command { command: " /say hi ".into() };
        validate(&mut slash, &every_day).unwrap();
        assert_eq!(slash, Action::Command { command: "say hi".into() });
    }

    #[test]
    fn the_file_keeps_its_shape() {
        let t = Task { made: vec!["a.zip".into()], ..task(Timing::Weekly { days: vec![0, 6], at: "04:00".into() }) };
        let text = serde_json::to_value(&t).unwrap();
        assert_eq!(text["action"], serde_json::json!({ "kind": "restart" }));
        assert_eq!(text["timing"], serde_json::json!({ "kind": "weekly", "days": [0, 6], "at": "04:00" }));
        assert_eq!(serde_json::from_value::<Task>(text).unwrap(), t);
    }
}
