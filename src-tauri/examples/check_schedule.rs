//! Headless end-to-end check of scheduled tasks in a scratch data folder:
//!   cargo run --no-default-features --features panel --example check_schedule -- <empty folder> [pumpkin]
//! Makes a Velocity server (no EULA) and checks skip rules, a backup with "keep last 2" next to a backup of the
//! player's, a queue of tasks that never overlap, and a command and a restart fired by the real loop at the next
//! minutes of the clock. A world folder is planted, since a proxy has none. With `pumpkin`, instead, a restart with the
//! warning on a server that has `say` (also no EULA). Servers are stopped and deleted afterwards.

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use chrono::Timelike;
use resincraft_lib::core::router::dispatch;
use resincraft_lib::core::{ctx, Args};
use resincraft_lib::server::{runtime, schedule, store};
use serde_json::{json, Value};

async fn call(channel: &str, args: Vec<Value>) -> Result<Value> {
    dispatch(channel, Args::new(args)).await
}

async fn add(id: &str, action: Value, timing: Value) -> Result<String> {
    let list = call("server:scheduleSave", vec![json!(id), json!({ "enabled": true, "action": action, "timing": timing })]).await?;
    list.as_array().and_then(|l| l.last()).and_then(|t| t["id"].as_str()).map(str::to_string).context("no task came back")
}

async fn task(id: &str, task: &str) -> Result<Value> {
    let list = call("server:schedule", vec![json!(id)]).await?;
    list.as_array().and_then(|l| l.iter().find(|t| t["id"] == task)).cloned().context("task gone")
}

/// Runs it by hand and returns its `lastRun`.
async fn run_now(id: &str, t: &str) -> Result<Value> {
    call("server:scheduleRun", vec![json!(id), json!(t)]).await?;
    settled(id, t, Duration::from_secs(150)).await
}

async fn settled(id: &str, t: &str, wait: Duration) -> Result<Value> {
    let end = Instant::now() + wait;
    while Instant::now() < end {
        let now = task(id, t).await?;
        if now["running"] == false {
            return Ok(now["lastRun"].clone());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    bail!("task {t} still running")
}

async fn wait_ran(id: &str, t: &str, after: i64, wait: Duration) -> Result<Value> {
    let end = Instant::now() + wait;
    while Instant::now() < end {
        let now = task(id, t).await?;
        if now["running"] == false && now["lastRun"]["at"].as_i64().is_some_and(|at| at > after) {
            return Ok(now["lastRun"].clone());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    bail!("the loop never ran task {t}")
}

fn expect(what: &str, ok: bool, detail: impl std::fmt::Debug) -> Result<()> {
    println!("{} {what}: {detail:?}", if ok { "ok  " } else { "FAIL" });
    if ok {
        Ok(())
    } else {
        Err(anyhow!("{what}"))
    }
}

fn zips(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir.join("backups")).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    out.sort();
    out
}

/// "HH:MM" of a minute at least 25 seconds away.
fn next_minute(extra: i64) -> String {
    let at = chrono::Local::now() + chrono::TimeDelta::seconds(85 - i64::from(chrono::Local::now().second())) + chrono::TimeDelta::minutes(extra);
    at.format("%H:%M").to_string()
}

async fn make(name: &str, kind: &str, port: u16) -> Result<String> {
    let versions = call("server:versions", vec![json!(kind)]).await?;
    let version = versions.as_array().and_then(|v| v.first()).map(|v| v.get("id").cloned().unwrap_or_else(|| v.clone())).context("no versions")?;
    let made = call("server:create", vec![json!({ "name": name, "type": kind, "mcVersion": version, "port": port, "memoryMb": 512 })]).await?;
    made["server"]["id"].as_str().map(str::to_string).context("no id")
}

async fn start(id: &str) -> Result<()> {
    call("server:start", vec![json!(id)]).await?;
    let state = runtime::wait_state(id, &["running", "crashed", "stopped"], Duration::from_secs(180)).await?;
    expect("server comes up", state == "running", state)
}

async fn velocity() -> Result<()> {
    let id = make("sched-velocity", "velocity", 25621).await?;
    let dir = store::dir(&id)?;
    println!("made {id} in {}", dir.display());

    // Stopped: restart and command are skipped, and say why.
    let (restart, command) = (add(&id, json!({ "kind": "restart" }), json!({ "kind": "hours", "every": 1 })).await?, add(&id, json!({ "kind": "command", "command": "/velocity info" }), json!({ "kind": "hours", "every": 1 })).await?);
    for t in [&restart, &command] {
        let last = run_now(&id, t).await?;
        expect("skipped while stopped", last["outcome"] == "skipped", &last)?;
    }

    std::fs::create_dir_all(dir.join("world"))?;
    std::fs::write(dir.join("world/level.dat"), "planted")?;
    start(&id).await?;

    // Backups: keep last 2, and the player's own zip stays.
    std::fs::create_dir_all(dir.join("backups"))?;
    std::fs::write(dir.join("backups/2020-01-01_00-00-00_world.zip"), "mine")?;
    let backup = add(&id, json!({ "kind": "backup", "keep": 2 }), json!({ "kind": "daily", "at": "03:00" })).await?;
    let mut last = Value::Null;
    for _ in 0..4 {
        last = run_now(&id, &backup).await?;
        expect("backup while running", last["outcome"] == "ok", &last)?;
        tokio::time::sleep(Duration::from_millis(1100)).await;
    }
    let held = zips(&dir);
    expect("keeps 2 of its own and the player's", held.len() == 3 && held[0].starts_with("2020") && held.last() == last["message"].as_str().map(str::to_string).as_ref(), &held)?;

    // Queued together: they run one after the other.
    let (a, b) = (add(&id, json!({ "kind": "restart" }), json!({ "kind": "hours", "every": 1 })).await?, add(&id, json!({ "kind": "backup" }), json!({ "kind": "hours", "every": 1 })).await?);
    let before = runtime::server_processes().into_iter().find(|p| p.0 == id).map(|p| p.1);
    call("server:scheduleRun", vec![json!(id), json!(a)]).await?;
    call("server:scheduleRun", vec![json!(id), json!(b)]).await?;
    expect("a task can't be queued twice", call("server:scheduleRun", vec![json!(id), json!(b)]).await.is_err(), "refused")?;
    expect("deleting waits for a running task", call("server:remove", vec![json!(id)]).await.is_err(), "refused")?;
    let (first, second) = (settled(&id, &a, Duration::from_secs(150)).await?, settled(&id, &b, Duration::from_secs(150)).await?);
    let after = runtime::server_processes().into_iter().find(|p| p.0 == id).map(|p| p.1);
    expect("restart: a new process", first["outcome"] == "ok" && before.is_some() && after.is_some() && before != after && runtime::state(&id) == "running", (&first, before, after))?;
    expect("backup found the server up again", second["outcome"] == "ok", &second)?;
    expect("not at the same time", second["at"].as_i64() > first["at"].as_i64(), (first["at"].clone(), second["at"].clone()))?;

    // The real loop, on the clock.
    tokio::spawn(schedule::init());
    let since = resincraft_lib::core::fsutil::now_ms();
    let timed = add(&id, json!({ "kind": "command", "command": "velocity info" }), json!({ "kind": "daily", "at": next_minute(0) })).await?;
    let view = task(&id, &timed).await?;
    println!("next run {} ({} ms away)", view["next"], view["next"].as_i64().unwrap_or(0) - since);
    let last = wait_ran(&id, &timed, since, Duration::from_secs(150)).await?;
    let fell_due = view["next"].as_i64().unwrap_or(0);
    expect("the loop ran the command within a tick", last["outcome"] == "ok" && last["at"].as_i64().is_some_and(|at| (fell_due..fell_due + 25_000).contains(&at)), &last)?;
    let lines = serde_json::to_string(&runtime::log(&id))?;
    expect("the command reached the console", lines.contains("> velocity info"), "echoed")?;

    let since = resincraft_lib::core::fsutil::now_ms();
    let before = runtime::server_processes().into_iter().find(|p| p.0 == id).map(|p| p.1);
    let timed = add(&id, json!({ "kind": "restart" }), json!({ "kind": "weekly", "days": [0, 1, 2, 3, 4, 5, 6], "at": next_minute(0) })).await?;
    let last = wait_ran(&id, &timed, since, Duration::from_secs(200)).await?;
    let after = runtime::server_processes().into_iter().find(|p| p.0 == id).map(|p| p.1);
    expect("the loop restarted it", last["outcome"] == "ok" && before != after && runtime::state(&id) == "running", (&last, before, after))?;

    // It stays once for that minute.
    tokio::time::sleep(Duration::from_secs(45)).await;
    expect("ran once", task(&id, &timed).await?["lastRun"] == last, "same result")?;

    // A task that was off, then deleted with the server.
    call("server:stop", vec![json!(id)]).await?;
    runtime::wait_state(&id, &["stopped", "crashed"], Duration::from_secs(100)).await?;
    let file = dir.join("schedule.json");
    expect("tasks are saved with the server", file.is_file(), &file)?;
    call("server:remove", vec![json!(id)]).await?;
    expect("deleting the server deletes its tasks", !file.exists() && !dir.exists(), "gone")?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    expect("nothing brings the folder back", !dir.exists(), "gone")
}

async fn with_warning() -> Result<()> {
    let id = make("sched-pumpkin", "pumpkin", 25623).await?;
    start(&id).await?;
    let t = add(&id, json!({ "kind": "restart" }), json!({ "kind": "hours", "every": 1 })).await?;
    let began = Instant::now();
    call("server:scheduleRun", vec![json!(id), json!(t)]).await?;
    tokio::time::sleep(Duration::from_secs(5)).await;
    expect("players are warned", serde_json::to_string(&runtime::log(&id))?.contains("> say Restarting in 1 minute"), "echoed")?;
    let last = settled(&id, &t, Duration::from_secs(200)).await?;
    expect("restart a minute after the warning", last["outcome"] == "ok" && began.elapsed() >= Duration::from_secs(60) && runtime::state(&id) == "running", (&last, began.elapsed()))?;

    // Stopped by hand during the warning: it doesn't start the server again.
    call("server:scheduleRun", vec![json!(id), json!(t)]).await?;
    tokio::time::sleep(Duration::from_secs(5)).await;
    call("server:stop", vec![json!(id)]).await?;
    let last = settled(&id, &t, Duration::from_secs(100)).await?;
    runtime::wait_state(&id, &["stopped", "crashed"], Duration::from_secs(100)).await?;
    expect("stopped during the warning: skipped, stays off", last["outcome"] == "skipped" && !runtime::is_running(&id), &last)?;
    call("server:remove", vec![json!(id)]).await?;
    Ok(())
}

async fn all(pumpkin: bool) -> Result<()> {
    if pumpkin {
        with_warning().await
    } else {
        velocity().await
    }
}

/// The engine's request handler is one big future: more stack than a thread gets by default.
const STACK: usize = 64 * 1024 * 1024;

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().thread_stack_size(STACK).build().expect("Tokio runtime");
    let main = std::thread::Builder::new().stack_size(STACK).spawn(move || runtime.block_on(check())).expect("main thread");
    let _ = main.join();
}

async fn check() {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let Some(folder) = words.first() else {
        eprintln!("usage: check_schedule <empty folder> [pumpkin]");
        std::process::exit(2);
    };
    std::fs::create_dir_all(folder).unwrap();
    ctx::init(std::path::PathBuf::from(folder), None);
    let done = all(words.iter().any(|w| w == "pumpkin")).await;
    runtime::shutdown_all(Duration::from_secs(30)).await;
    if let Err(e) = done {
        eprintln!("CHECK FAILED: {e:#}");
        std::process::exit(1);
    }
    println!("all good");
}
