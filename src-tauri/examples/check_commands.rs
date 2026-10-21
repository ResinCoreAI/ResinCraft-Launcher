//! Headless check of custom commands and environment variables (no window needed):
//!   cargo run --example check_commands -- [mcVersion]
//! Creates a throwaway vanilla instance (default 1.21.1) and launches it four ways:
//!   1. an environment variable name that can't be used is refused when saving;
//!   2. a pre-launch command that exits with 3 stops the launch, and its output is in the error;
//!   3. a wrapper (this example itself) runs the game and exits with 7: the pre-launch command, the wrapper and the
//!      post-exit command all see the variables, the post-exit command gets code 7, secret-looking values stay hidden;
//!   4. the instance's own commands replace the ones in Settings, and its variables reach Java itself
//!      (JAVA_TOOL_OPTIONS, which the JVM reports); Stop ends Java behind the wrapper too.
//! RESIN_DATA_DIR picks the data folder (a folder with the version already downloaded is quickest).
//!
//! `check_commands --wrap [--exit N] <program> <args…>` is the wrapper: it prints what it sees, then runs the
//! program (or exits with N).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use resincraft_lib::core::types::{LaunchCommands, LoaderType, NewInstance};
use resincraft_lib::core::{ctx, events, instances, paths, settings};
use resincraft_lib::game::launch;
use resincraft_lib::player::accounts::LaunchAuth;
use serde_json::json;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.first().map(String::as_str) == Some("--wrap") {
        std::process::exit(wrap(&argv[1..]));
    }
    let code = match run(argv.first().cloned().unwrap_or_else(|| "1.21.1".into())).await {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(e) => {
            eprintln!("CHECK FAILED: {e:#}");
            1
        }
    };
    std::process::exit(code);
}

fn wrap(args: &[String]) -> i32 {
    let var = |n: &str| std::env::var(n).unwrap_or_default();
    println!("RESIN WRAPPER RAN check={} inst={} args={}", var("RESIN_CHECK"), var("INST_NAME"), args.len());
    if args.first().map(String::as_str) == Some("--exit") {
        return args.get(1).and_then(|n| n.parse().ok()).unwrap_or(1);
    }
    let Some((program, rest)) = args.split_first() else { return 2 };
    match std::process::Command::new(program).args(rest).spawn() {
        Ok(mut child) => {
            println!("RESIN WRAPPER CHILD pid={}", child.id());
            child.wait().ok().and_then(|s| s.code()).unwrap_or(1)
        }
        Err(e) => {
            eprintln!("wrapper couldn't start {program}: {e}");
            2
        }
    }
}

#[derive(Default)]
struct Seen {
    lines: Vec<String>,
    running: bool,
}

async fn run(mc: String) -> anyhow::Result<bool> {
    let data_dir = std::env::var_os("RESIN_DATA_DIR").map(Into::into).unwrap_or_else(|| paths::default_data_dir(None));
    std::fs::create_dir_all(&data_dir)?;
    ctx::init(data_dir, None);

    let inst = instances::create(NewInstance { name: "Check commands".into(), mc_version: mc.clone(), loader: LoaderType::Vanilla, loader_version: None, color: None, group: None }, |_| {}).await?;
    println!("instance {} ({mc})", inst.id);
    let result = checks(&inst.id).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let _ = tokio::fs::remove_dir_all(paths::instance_dir(&inst.id)).await;
    result
}

async fn checks(id: &str) -> anyhow::Result<bool> {
    let seen: Arc<Mutex<Seen>> = Arc::default();
    let (stop_tx, mut stop_rx) = mpsc::unbounded_channel::<(String, Option<i64>)>();
    {
        let (seen, id) = (seen.clone(), id.to_string());
        events::set_sink(move |channel, payload| match channel {
            "task" if payload["state"] == "error" => println!("[task] error {}", payload["error"].as_str().unwrap_or_default()),
            "log" => {
                for l in payload["lines"].as_array().into_iter().flatten() {
                    let text = l["text"].as_str().unwrap_or_default().to_string();
                    println!("[game:{}] {}", l["level"].as_str().unwrap_or_default(), text.chars().take(240).collect::<String>());
                    if text.contains("Picked up JAVA_TOOL_OPTIONS") && text.contains("resin.check") {
                        launch::kill_instance(&id, None);
                    }
                    seen.lock().unwrap().lines.push(text);
                }
            }
            "game" => {
                let state = payload["state"].as_str().unwrap_or_default().to_string();
                println!("[game] {state} {}", payload["code"]);
                if state == "running" {
                    seen.lock().unwrap().running = true;
                }
                if state == "stopped" || state == "crashed" {
                    let _ = stop_tx.send((state, payload["code"].as_i64()));
                }
            }
            _ => {}
        });
    }
    let auth = || LaunchAuth {
        account_id: "offline-test".into(),
        username: "ResinTester".into(),
        uuid: "00000000-0000-0000-0000-000000000000".into(),
        access_token: "0".into(),
        user_type: "legacy",
        xuid: None,
    };
    let reset = || std::mem::take(&mut *seen.lock().unwrap());
    let has = |lines: &[String], want: &str| lines.iter().any(|l| l.contains(want));
    let mut ok = true;
    let mut check = |pass: bool, what: &str| {
        println!("{} {what}", if pass { "PASS" } else { "FAIL" });
        ok &= pass;
    };

    // 1. Names a shell can't use are refused.
    let bad = instances::update(id, json!({ "env": [{ "name": "1BAD", "value": "x" }] })).await;
    check(bad.is_err(), "an environment variable named 1BAD is refused");
    instances::update(id, json!({ "env": [{ "name": "RESIN_CHECK", "value": "from-instance" }] })).await?;

    let wrapper_exe = std::env::current_exe()?.to_string_lossy().into_owned();
    let mut s = settings::get().await;
    s.memory_mb = 2048;
    s.env = serde_json::from_value(json!([
        { "name": "RESIN_CHECK", "value": "global" },
        { "name": "RESIN_API_TOKEN", "value": "s3cr3t-value-123" }
    ]))?;

    // 2. A failing pre-launch command stops the launch.
    s.commands = LaunchCommands { pre_launch: "echo resin-pre-failed&& exit 3".into(), ..Default::default() };
    reset();
    let failed = launch::launch_instance(id, &s, auth(), launch::LaunchHooks::default()).await;
    let _ = tokio::time::timeout(Duration::from_secs(5), stop_rx.recv()).await;
    let message = failed.as_ref().err().map(|e| e.to_string()).unwrap_or_default();
    println!("launch error: {message}");
    check(failed.is_err() && message.contains("resin-pre-failed") && message.contains("exit code 3"), "a pre-launch command exiting with 3 stops the launch and shows its output");
    check(!reset().running, "the game never started after the failed pre-launch command");

    // 3. Wrapper, variables and the post-exit command's exit code.
    s.commands = LaunchCommands {
        pre_launch: "echo pre-ok $INST_NAME / $RESIN_CHECK".into(),
        wrapper: format!("\"{wrapper_exe}\" --wrap --exit 7 {{java}}"),
        post_exit: "echo post-exit code=$INST_EXIT_CODE id=$INST_ID".into(),
    };
    launch::launch_instance(id, &s, auth(), launch::LaunchHooks::default()).await?;
    let stopped = tokio::time::timeout(Duration::from_secs(120), stop_rx.recv()).await?;
    let lines = reset().lines;
    check(has(&lines, "pre-ok Check commands / from-instance"), "the pre-launch command sees $INST_NAME and the instance's own value of a variable");
    check(has(&lines, "RESIN WRAPPER RAN check=from-instance inst=Check commands"), "the game runs through the wrapper, which sees the variables");
    check(stopped == Some(("crashed".into(), Some(7))), "the game's exit code (7) is reported");
    check(has(&lines, &format!("post-exit code=7 id={id}")), "the post-exit command gets the exit code in $INST_EXIT_CODE");
    check(has(&lines, "RESIN_API_TOKEN=<hidden>") && !lines.iter().any(|l| l.contains("s3cr3t")), "a secret-looking value never reaches the log");

    // 4. The instance's own commands win; its variables reach Java; Stop ends Java behind the wrapper.
    s.commands.pre_launch = "exit 9".into();
    instances::update(
        id,
        json!({
            "env": [{ "name": "RESIN_CHECK", "value": "from-instance" }, { "name": "JAVA_TOOL_OPTIONS", "value": "-Dresin.check=from-instance" }],
            "commands": { "preLaunch": "", "wrapper": format!("\"{wrapper_exe}\" --wrap"), "postExit": "" }
        }),
    )
    .await?;
    launch::launch_instance(id, &s, auth(), launch::LaunchHooks::default()).await?;
    let stopped = tokio::time::timeout(Duration::from_secs(180), stop_rx.recv()).await;
    if stopped.is_err() {
        launch::kill_instance(id, None);
        let _ = tokio::time::timeout(Duration::from_secs(30), stop_rx.recv()).await;
    }
    let lines = reset().lines;
    check(has(&lines, "RESIN WRAPPER RAN check=from-instance"), "the instance's wrapper replaces the Settings commands (whose pre-launch would fail)");
    check(has(&lines, "Picked up JAVA_TOOL_OPTIONS: -Dresin.check=from-instance"), "the instance's variables reach Java itself");
    let java_pid = lines.iter().find_map(|l| l.split("RESIN WRAPPER CHILD pid=").nth(1)).and_then(|p| p.trim().parse::<u32>().ok());
    tokio::time::sleep(Duration::from_secs(1)).await;
    let java_alive = java_pid.is_some_and(|pid| {
        let mut sys = sysinfo::System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[sysinfo::Pid::from_u32(pid)]), true);
        sys.process(sysinfo::Pid::from_u32(pid)).is_some()
    });
    check(java_pid.is_some() && !java_alive, "Stop ends Java behind the wrapper");

    println!("{}", if ok { "CHECK OK" } else { "CHECK FAILED" });
    Ok(ok)
}
