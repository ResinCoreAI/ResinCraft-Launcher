//! Headless end-to-end check of the log browser in a scratch data folder, through the engine's real channels:
//!   cargo run --no-default-features --features panel --example check_logs -- <empty folder> [size in MB, default 300]
//! Plants a server folder with sample logs (no server runs, so there's no EULA), a big log and its .gz, and a file
//! outside the logs folder that nothing may reach. On Windows, also a junction from crash-reports to another folder.
//! Prints how long the big file takes and the process's peak memory where the OS tells it.

use std::io::Write;
use std::path::Path;
use std::time::Instant;

use anyhow::{bail, ensure, Result};
use flate2::write::GzEncoder;
use flate2::Compression;
use resincraft_lib::core::router::dispatch;
use resincraft_lib::core::{ctx, Args};
use resincraft_lib::server::store;
use serde_json::{json, Value};

const ID: &str = "logs-check";
/// The engine's futures are big in a debug build.
const STACK: usize = 16 * 1024 * 1024;

async fn call(channel: &str, args: Vec<Value>) -> Result<Value> {
    dispatch(channel, Args::new(args)).await
}

async fn timed(what: &str, channel: &str, args: Vec<Value>) -> Result<Value> {
    let start = Instant::now();
    let out = call(channel, args).await?;
    println!("{what}: {:?}", start.elapsed());
    Ok(out)
}

fn plant(dir: &Path, mb: usize) -> Result<()> {
    std::fs::create_dir_all(dir.join("logs"))?;
    std::fs::create_dir_all(dir.join("crash-reports"))?;
    std::fs::write(dir.join("server.json"), serde_json::to_vec(&json!({ "id": ID, "name": "Logs check", "type": "velocity", "mcVersion": "1.21.4", "memoryMb": 512, "port": 25599, "createdAt": 0 }))?)?;
    std::fs::write(dir.join("server.properties"), "rcon.password=not-for-the-log-browser\n")?;
    std::fs::write(dir.join("logs/latest.log"), "[12:00:00] [main/INFO]: Xin chào LỖI\n[12:00:01] [main/WARN]: Careful\n[12:00:02] [main/ERROR]: Boom\n\tat a.b(c)\n[12:00:03] [main/INFO]: <Bob> ERROR\n")?;
    let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
    gz.write_all("[11:00:00] [main/INFO]: Привет из gzip\n".as_bytes())?;
    std::fs::write(dir.join("logs/2026-10-30-1.log.gz"), gz.finish()?)?;
    std::fs::write(dir.join("crash-reports/crash-1.txt"), "---- Minecraft Crash Report ----\nDescription: Exception\n\nA detailed walkthrough\nJava: 21\n")?;

    // The big one: the match sits on the very last line, in both forms.
    let line = "[12:00:00] [Server thread/INFO]: Steve moved too quickly! 日本語 12.3,4.5,6.7\n";
    let count = mb * 1024 * 1024 / line.len();
    let mut plain = std::io::BufWriter::new(std::fs::File::create(dir.join("logs/big.log"))?);
    let mut packed = GzEncoder::new(std::io::BufWriter::new(std::fs::File::create(dir.join("logs/big2.log.gz"))?), Compression::fast());
    for _ in 0..count {
        plain.write_all(line.as_bytes())?;
        packed.write_all(line.as_bytes())?;
    }
    plain.write_all(b"[12:00:01] [Server thread/ERROR]: NEEDLE at the end\n")?;
    packed.write_all(b"[12:00:01] [Server thread/ERROR]: NEEDLE at the end\n")?;
    plain.flush()?;
    packed.finish()?.flush()?;
    Ok(())
}

/// cmd takes a forward slash for a switch.
#[cfg(windows)]
fn win(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\")
}

async fn run(folder: &Path, mb: usize) -> Result<()> {
    let dir = store::dir(ID)?;
    plant(&dir, mb)?;

    let files = call("server:logFiles", vec![json!(ID)]).await?;
    let names: Vec<&str> = files.as_array().unwrap().iter().filter_map(|f| f["path"].as_str()).collect();
    println!("listed {names:?}");
    ensure!(names.len() == 5, "listed {}", names.len());

    let page = call("server:logRead", vec![json!(ID), json!("logs/2026-10-30-1.log.gz"), json!(1)]).await?;
    ensure!(page["lines"][0]["text"] == "[11:00:00] [main/INFO]: Привет из gzip", "gz page: {page}");

    let q = |path: Option<&str>, query: &str, problems: bool| vec![json!(ID), json!({ "path": path, "query": query, "problems": problems })];
    let lower = call("server:logSearch", q(Some("logs/latest.log"), "lỗi", false)).await?;
    ensure!(lower["hits"].as_array().unwrap().len() == 1, "{lower}");
    let russian = call("server:logSearch", q(None, "ПРИВЕТ", false)).await?;
    ensure!(russian["hits"][0]["path"] == "logs/2026-10-30-1.log.gz", "{russian}");
    let bad = call("server:logSearch", q(Some("logs/latest.log"), "", true)).await?;
    let lines: Vec<u64> = bad["hits"].as_array().unwrap().iter().filter_map(|h| h["line"].as_u64()).collect();
    ensure!(lines == [2, 3, 4], "problems: {lines:?}");

    for path in ["../server.properties", "logs/../server.properties", "logs\\..\\server.properties", "server.properties", "../../../server.json", "C:\\Windows\\win.ini"] {
        for (channel, args) in [("server:logRead", vec![json!(ID), json!(path), json!(1)]), ("server:logSave", vec![json!(ID), json!(path)]), ("server:logSearch", q(Some(path), "password", false))] {
            let Err(e) = call(channel, args).await else { bail!("{channel} let {path} through") };
            ensure!(e.to_string().contains("log files"), "{channel} {path}: {e}");
        }
    }
    println!("traversal refused on all three channels");

    #[cfg(windows)]
    {
        // A junction: `rmdir` (not remove_dir_all) takes it away, the target stays.
        let outside = folder.join("outside");
        std::fs::create_dir_all(&outside)?;
        std::fs::write(outside.join("secret.txt"), "token")?;
        let link = dir.join("crash-reports");
        std::fs::remove_dir_all(&link)?;
        let made = std::process::Command::new("cmd").args(["/c", "mklink", "/J"]).arg(win(&link)).arg(win(&outside)).output()?;
        ensure!(made.status.success(), "mklink: {made:?}");
        let list = call("server:logFiles", vec![json!(ID)]).await?;
        let read = call("server:logRead", vec![json!(ID), json!("crash-reports/secret.txt"), json!(1)]).await;
        std::process::Command::new("cmd").args(["/c", "rmdir"]).arg(win(&link)).status()?;
        ensure!(!list.to_string().contains("secret"), "a junction was listed");
        ensure!(read.is_err(), "read through a junction");
        ensure!(outside.join("secret.txt").exists(), "the junction's target must survive");
        println!("junction refused");
    }

    for path in ["logs/big.log", "logs/big2.log.gz"] {
        let end = timed(&format!("{path} last page"), "server:logRead", vec![json!(ID), json!(path), json!(0)]).await?;
        let lines = end["lines"].as_array().unwrap();
        ensure!(lines.len() == 2000 && lines[1999]["text"].as_str().unwrap().contains("NEEDLE"), "end page of {path}");
        let found = timed(&format!("{path} search"), "server:logSearch", q(Some(path), "needle AT", false)).await?;
        ensure!(found["hits"].as_array().unwrap().len() == 1, "{found}");
        ensure!(found["hits"][0]["level"] == "error", "{found}");
        let many = timed(&format!("{path} too many"), "server:logSearch", q(Some(path), "日本語", false)).await?;
        ensure!(many["hits"].as_array().unwrap().len() == 1000 && many["more"] == true, "limit on {path}");
    }

    // Peak memory, from the OS (the example's own PID, nothing else).
    if let Ok(out) = std::process::Command::new("powershell").args(["-NoProfile", "-Command", &format!("(Get-Process -Id {}).PeakWorkingSet64 / 1MB", std::process::id())]).output() {
        println!("peak working set MB: {}", String::from_utf8_lossy(&out.stdout).trim());
    }
    println!("all checks passed");
    Ok(())
}

fn main() {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let Some(folder) = words.first() else {
        eprintln!("Give it an empty scratch folder");
        std::process::exit(2);
    };
    let folder = std::path::PathBuf::from(folder);
    if !folder.is_dir() || std::fs::read_dir(&folder).map(|mut d| d.next().is_some()).unwrap_or(true) {
        eprintln!("That isn't an empty folder");
        std::process::exit(2);
    }
    let mb = words.get(1).and_then(|m| m.parse().ok()).unwrap_or(300);
    ctx::init(folder.clone(), None);
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().thread_stack_size(STACK).build().unwrap();
    let main = std::thread::Builder::new().stack_size(STACK).spawn(move || runtime.block_on(run(&folder, mb))).unwrap();
    match main.join() {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            eprintln!("FAILED: {e:#}");
            std::process::exit(1);
        }
        Err(_) => std::process::exit(1),
    }
}
