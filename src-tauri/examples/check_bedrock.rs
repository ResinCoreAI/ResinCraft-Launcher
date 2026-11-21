//! Real checks of the Bedrock version switch on this PC, through the launcher's own requests:
//!   cargo run --example check_bedrock -- status             what is installed, Developer Mode, Gaming Services
//!   cargo run --example check_bedrock -- versions           the versions the launcher lists
//!   cargo run --example check_bedrock -- plan <version>     what a switch would need (changes nothing)
//!   cargo run --example check_bedrock -- download <version> downloads Microsoft's package and checks it (a UWP one is
//!                                                           unpacked too); installs nothing
//!   cargo run --example check_bedrock -- backup | backups | restore <backup id>
//!   cargo run --example check_bedrock -- install            Minecraft for Windows from the Microsoft Store
//!   cargo run --example check_bedrock -- switch <version>   THE REAL THING: removes the installed game, installs that one
//!   cargo run --example check_bedrock -- back               back to the Store's version
//!   cargo run --example check_bedrock -- uninstall          removes Minecraft for Windows for this Windows user
//!
//! The launcher's data for these runs goes to RC_DATA, or `.dev-data/bedrock-check` in the repository: keep it out
//! of AppData when the shell runs inside a packaged app, because Windows itself has to read the files there.
//! Started with no arguments it reads the command from `<data>/command.txt` and appends what it prints to
//! `<data>/log.txt`, ending with a line `END`: for starting it through explorer.exe, outside a packaged app's
//! redirected view of AppData.

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use resincraft_lib::core::router::{dispatch, Args};
use resincraft_lib::core::{ctx, events};
use serde_json::{json, Value};

static LOG: OnceLock<Option<PathBuf>> = OnceLock::new();
/// The last task line printed, so a long download isn't a thousand lines.
static LAST: Mutex<String> = Mutex::new(String::new());

fn say(line: &str) {
    println!("{line}");
    if let Some(Some(path)) = LOG.get() {
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "{line}");
        }
    }
}

/// One line per change of a task's step, and per tenth of its progress.
fn on_task(v: &Value) {
    let text = |k: &str| v[k].as_str().unwrap_or_default().to_string();
    let step: String = text("detail").chars().filter(|c| !c.is_ascii_digit()).collect();
    let tenth = v["progress"].as_f64().map(|p| (p * 10.0).floor() as i64);
    let key = format!("{}|{}|{step}|{tenth:?}", text("title"), text("state"));
    let mut last = LAST.lock().unwrap();
    if *last != key {
        *last = key;
        let cancel = if v["cancellable"].as_bool() == Some(false) { " (can't be cancelled)" } else { "" };
        say(&format!("  task [{}]{cancel} {} :: {} {}", text("state"), text("title"), text("detail"), tenth.map(|t| format!("{}%", t * 10)).unwrap_or_default()));
        if !text("error").is_empty() {
            say(&format!("  task error: {}", text("error")));
        }
    }
}

async fn call(channel: &str, args: Vec<Value>) {
    say(&format!("> {channel} {}", Value::Array(args.clone())));
    match dispatch(channel, Args::new(args)).await {
        Ok(v) => say(&format!("ok: {}", serde_json::to_string_pretty(&v).unwrap_or_default())),
        Err(e) => say(&format!("FAILED: {e}")),
    }
}

fn main() {
    // The router is a deep future for a debug build's main thread.
    let run = || tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap().block_on(run());
    std::thread::Builder::new().stack_size(64 << 20).spawn(run).unwrap().join().unwrap();
}

async fn run() {
    let data = std::env::var_os("RC_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("the repository").join(".dev-data").join("bedrock-check"));
    std::fs::create_dir_all(&data).expect("data folder");
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let from_file = args.is_empty();
    if from_file {
        args = std::fs::read_to_string(data.join("command.txt")).unwrap_or_default().split_whitespace().map(str::to_string).collect();
    }
    let _ = LOG.set(from_file.then(|| data.join("log.txt")));
    ctx::init(data.clone(), None);
    events::set_sink(|channel, v| {
        if channel == "task" {
            on_task(&v);
        }
    });
    say(&format!("== check_bedrock {} (data: {})", args.join(" "), data.display()));
    let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
    match arg(0).as_str() {
        "status" => call("bedrock:status", vec![]).await,
        "versions" => match dispatch("bedrock:versions", Args::new(vec![])).await {
            Ok(Value::Array(list)) => {
                let of = |format: &str| list.iter().filter(|v| v["format"] == format).count();
                say(&format!("{} versions: {} gdk, {} uwp", list.len(), of("gdk"), of("uwp")));
                for v in list.iter().take(5).chain(list.iter().rev().take(2)) {
                    say(&format!("  {v}"));
                }
            }
            other => say(&format!("FAILED: {other:?}")),
        },
        "plan" => call("bedrock:switchPlan", vec![json!(arg(1))]).await,
        "download" => match resincraft_lib::player::bedrock_switch::download_only(&arg(1)).await {
            Ok(path) => {
                let files = walkdir(&path);
                say(&format!("ok: {} ({} files, {} bytes)", path.display(), files.0, files.1));
            }
            Err(e) => say(&format!("FAILED: {e}")),
        },
        "backup" => call("bedrock:backup", vec![json!("release")]).await,
        "backups" => call("bedrock:backups", vec![]).await,
        "restore" => call("bedrock:restore", vec![json!(arg(1))]).await,
        "install" => call("bedrock:install", vec![json!("release"), json!("install")]).await,
        "switch" => call("bedrock:switch", vec![json!(arg(1))]).await,
        "back" => call("bedrock:backToStore", vec![]).await,
        "uninstall" => call("bedrock:uninstall", vec![json!("release")]).await,
        other => say(&format!("Unknown command: {other:?} (see the top of examples/check_bedrock.rs)")),
    }
    if matches!(arg(0).as_str(), "install" | "switch" | "back" | "uninstall") {
        call("bedrock:status", vec![]).await;
    }
    say("END");
}

/// How many files are under a path (or 1 for a file), and their bytes.
fn walkdir(path: &std::path::Path) -> (usize, u64) {
    let Ok(meta) = std::fs::metadata(path) else { return (0, 0) };
    if meta.is_file() {
        return (1, meta.len());
    }
    std::fs::read_dir(path).map(|rd| rd.flatten().map(|e| walkdir(&e.path())).fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1))).unwrap_or((0, 0))
}
