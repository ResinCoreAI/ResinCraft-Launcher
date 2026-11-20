//! Times downloads, modpack version switches and the duplicate-mod check, for before/after comparisons, and
//! prints what each one produced so the results can be compared too (no window needed):
//!
//!   cargo run --release --example speed -- local [small files] [big files] [at once] [latency ms]
//!       the downloader against a server on this computer (no internet in the way)
//!   cargo run --release --example speed -- game <mcVersion> [at once]
//!       Minecraft's own files (client jar, libraries, assets) into the data folder
//!   cargo run --release --example speed -- pack <project id> <version id>
//!       a modpack install; prints the new instance's id
//!   cargo run --release --example speed -- switch <instance id> <version id> [<version id>…]
//!       switches the instance's modpack version, once per version given
//!   cargo run --release --example speed -- unknown <instance id> <count>
//!       adds jars to mods/ that no site knows (always the same ones)
//!   cargo run --release --example speed -- picks <instance id> <out.json>
//!       finds a fixed list of mods on Modrinth and CurseForge and saves them as picks
//!   cargo run --release --example speed -- dupes <instance id> <picks.json> [times] [out.json]
//!       the duplicate check Discover runs before installing those picks
//!   cargo run --release --example speed -- tree <instance id> [out.txt]
//!       every file of the instance with its SHA-1, plus its modpack record and content index
//!   cargo run --release --example speed -- steps <instance id>
//!       what a modpack install does after its downloads, one step at a time
//!   cargo run --release --example speed -- make <mcVersion> <vanilla|fabric|quilt> [name]
//!       a new empty instance; prints its id
//!   cargo run --release --example speed -- launch <instance id> [times] [command.txt]
//!       presses Play: the time until the game's process is running, then stops the game again
//!   cargo run --release --example speed -- ready <instance id> [<instance id>…] [play]
//!       gets the instances' files ready in the background, as making an instance does, and waits for it; with
//!       `play`, presses Play on the first one two seconds in and says how long that Play took
//!   cargo run --release --example speed -- prep <instance id> [times]
//!       what Play does before Java starts, one step at a time
//!   cargo run --release --example speed -- cdn <Modrinth pack version id> [times] [at once]
//!       the pack's files through the launcher's downloader into a throwaway folder, with the slowest files each time
//!       (RESIN_SPEED_SPLIT=1 fetches each file by hand instead and says how long the network and the disk took)
//!   cargo run --release --example speed -- faults
//!       downloads that go wrong (cut-off connections, wrong hashes, missing files) against a server on this computer
//!
//! RESIN_DATA_DIR picks the data folder (use an empty one for a first-ever download).

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use resincraft_lib::content::dupes::{self, ModPick};
use resincraft_lib::content::{packswitch, sources};
use resincraft_lib::core::net::{download_all, hash_bytes, DownloadItem, Hash, HashAlgo};
use resincraft_lib::core::types::{ContentKind, ContentSource, LoaderType, NewInstance, SearchQuery};
use resincraft_lib::core::{ctx, events, instances, paths, settings};
use resincraft_lib::game::loaders::{resolve_version, ResolveContext};
use resincraft_lib::game::{java, launch, mojang, versionpatch};
use resincraft_lib::player::accounts::LaunchAuth;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn secs(d: Duration) -> String {
    format!("{:>7.2}s", d.as_secs_f64())
}

fn mb(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1e6)
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("SPEED FAILED: {e:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let data_dir: PathBuf = std::env::var_os("RESIN_DATA_DIR").map(Into::into).unwrap_or_else(|| paths::default_data_dir(None));
    std::fs::create_dir_all(&data_dir)?;
    ctx::init(data_dir, None);
    watch_tasks();

    let argv: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| argv.get(i).cloned();
    let need = |i: usize, what: &str| arg(i).ok_or_else(|| anyhow!("missing {what}"));
    let num = |i: usize, default: usize| arg(i).and_then(|v| v.parse().ok()).unwrap_or(default);
    match arg(0).as_deref() {
        Some("local") => local(num(1, 3000), num(2, 150), num(3, 16), num(4, 0) as u64).await,
        Some("game") => game(&need(1, "a Minecraft version")?, arg(2).and_then(|v| v.parse().ok())).await,
        Some("pack") => pack(&need(1, "a project id")?, &need(2, "a version id")?).await,
        Some("switch") => switch(&need(1, "an instance id")?, &argv[2..]).await,
        Some("unknown") => unknown(&need(1, "an instance id")?, num(2, 20)),
        Some("picks") => picks(&need(1, "an instance id")?, Path::new(&need(2, "a file to write")?)).await,
        Some("dupes") => dupes_check(&need(1, "an instance id")?, Path::new(&need(2, "a picks file")?), num(3, 1), arg(4)).await,
        Some("tree") => tree(&need(1, "an instance id")?, arg(2)),
        Some("steps") => steps(&need(1, "an instance id")?).await,
        Some("make") => make(&need(1, "a Minecraft version")?, &need(2, "a loader")?, arg(3)).await,
        Some("launch") => launch_times(&need(1, "an instance id")?, num(2, 1), arg(3)).await,
        Some("prep") => prep(&need(1, "an instance id")?, num(2, 1)).await,
        Some("ready") => ready(&argv[1..]).await,
        Some("faults") => faults().await,
        Some("cdn") => cdn(&need(1, "a Modrinth version id")?, num(2, 3), num(3, 16)).await,
        _ => bail!("say what to time: local, game, pack, switch, unknown, picks, dupes, steps, make, launch or tree (see the top of examples/speed.rs)"),
    }
}

/// The Java command of the last game started, from its log.
static COMMAND: Mutex<Option<String>> = Mutex::new(None);
/// Rung when a game stops.
static STOPPED: tokio::sync::Notify = tokio::sync::Notify::const_new();
/// How many tasks have ended.
static TASKS_ENDED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Prints each task's steps as they start, with the time since this program started, and notes what games do.
fn watch_tasks() {
    let start = Instant::now();
    let last: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    events::set_sink(move |channel, payload| match channel {
        "task" => {
            let id = payload["id"].as_str().unwrap_or_default().to_string();
            let state = payload["state"].as_str().unwrap_or_default();
            let detail = payload["detail"].as_str().unwrap_or_default();
            // "Downloading modpack files (12/155)" is one step.
            let step = format!("{} | {}", payload["title"].as_str().unwrap_or_default(), detail.split(" (").next().unwrap_or_default());
            let step = if state == "running" { step } else { format!("{step} | {state}") };
            let mut last = last.lock().unwrap();
            if state != "running" && !last.get(&id).is_some_and(|s| s.ends_with(state)) {
                TASKS_ENDED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
            if last.get(&id) != Some(&step) {
                println!("  [{}] {step}", secs(start.elapsed()));
                last.insert(id, step);
            }
        }
        "game" => {
            if matches!(payload["state"].as_str(), Some("stopped" | "crashed")) {
                STOPPED.notify_one();
            }
        }
        "log" => {
            for l in payload["lines"].as_array().into_iter().flatten() {
                if let Some(c) = l["text"].as_str().and_then(|t| t.strip_prefix("Command: ")) {
                    *COMMAND.lock().unwrap() = Some(c.to_string());
                }
            }
        }
        _ => {}
    });
}

// ---------- The downloader against a server on this computer ----------

/// Always the same bytes for the same `seed`.
fn noise(seed: u64, len: usize) -> Vec<u8> {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut out = Vec::with_capacity(len + 8);
    while out.len() < len {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        out.extend_from_slice(&x.to_le_bytes());
    }
    out.truncate(len);
    out
}

/// Answers `GET /<name>` with that file, keeping connections open like a download site does.
async fn serve(listener: tokio::net::TcpListener, files: Arc<HashMap<String, Arc<Vec<u8>>>>, latency: Duration) {
    loop {
        let Ok((mut sock, _)) = listener.accept().await else { return };
        let _ = sock.set_nodelay(true);
        let files = files.clone();
        tokio::spawn(async move {
            let mut buf: Vec<u8> = Vec::new();
            let mut tmp = [0u8; 4096];
            loop {
                let end = loop {
                    if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        break i + 4;
                    }
                    match sock.read(&mut tmp).await {
                        Ok(n) if n > 0 => buf.extend_from_slice(&tmp[..n]),
                        _ => return,
                    }
                };
                let head = String::from_utf8_lossy(&buf[..end]).to_string();
                buf.drain(..end);
                let path = head.split_whitespace().nth(1).unwrap_or("/").trim_start_matches('/').to_string();
                if !latency.is_zero() {
                    tokio::time::sleep(latency).await;
                }
                let sent = match files.get(&path) {
                    Some(body) => {
                        let h = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n\r\n", body.len());
                        sock.write_all(h.as_bytes()).await.is_ok() && sock.write_all(body).await.is_ok()
                    }
                    None => sock.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n").await.is_ok(),
                };
                if !sent {
                    return;
                }
            }
        });
    }
}

async fn local(small: usize, big: usize, at_once: usize, latency_ms: u64) -> Result<()> {
    // Asset-sized files (2–64 KB) and mod-sized ones (0.3–4 MB).
    let mut files: HashMap<String, Arc<Vec<u8>>> = HashMap::new();
    for i in 0..small {
        files.insert(format!("s{i}.bin"), Arc::new(noise(i as u64 + 1, 2_000 + (i * 7919) % 62_000)));
    }
    for i in 0..big {
        files.insert(format!("b{i}.jar"), Arc::new(noise(1_000_000 + i as u64, 300_000 + (i * 104_729) % 3_700_000)));
    }
    let total: u64 = files.values().map(|f| f.len() as u64).sum();
    let hashes: HashMap<String, String> = files.iter().map(|(n, b)| (n.clone(), hash_bytes(b, HashAlgo::Sha1))).collect();
    let files = Arc::new(files);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    tokio::spawn(serve(listener, files.clone(), Duration::from_millis(latency_ms)));

    let dir = paths::cache().join("speed-local");
    let _ = tokio::fs::remove_dir_all(&dir).await;
    let mut names: Vec<&String> = files.keys().collect();
    names.sort();
    let items: Vec<DownloadItem> = names
        .iter()
        .map(|n| {
            // Spread over folders, like assets and libraries are.
            let sub = &hashes[*n][..2];
            DownloadItem::new(format!("http://127.0.0.1:{port}/{n}"), dir.join(sub).join(n)).hash(Some(Hash::sha1(hashes[*n].clone()))).size(Some(files[*n].len() as u64))
        })
        .collect();
    println!("local: {} files, {}, {at_once} at once, {latency_ms} ms before each answer", items.len(), mb(total));
    let t = Instant::now();
    download_all(items.clone(), at_once, |_, _| {}).await?;
    let first = t.elapsed();
    println!("RESULT local download      {}  ({:.0} MB/s)", secs(first), total as f64 / 1e6 / first.as_secs_f64());
    // What landed is what was served.
    for n in &names {
        let got = std::fs::read(dir.join(&hashes[*n][..2]).join(n))?;
        if hash_bytes(&got, HashAlgo::Sha1) != hashes[*n] {
            bail!("{n} isn't what was served");
        }
    }
    let t = Instant::now();
    download_all(items, at_once, |_, _| {}).await?;
    println!("RESULT local check again   {}  (everything already there)", secs(t.elapsed()));
    let _ = tokio::fs::remove_dir_all(&dir).await;
    Ok(())
}

// ---------- Downloads that go wrong ----------

/// A server that cuts every answer short: each connection gets at most `piece` more bytes of the file, from where
/// the request's `Range` says (or from the start), then closes. `ranges` false: it ignores `Range` and always starts over.
async fn serve_in_pieces(listener: tokio::net::TcpListener, body: Arc<Vec<u8>>, piece: usize, ranges: bool, requests: Arc<Mutex<Vec<String>>>) {
    loop {
        let Ok((mut sock, _)) = listener.accept().await else { return };
        let (body, requests) = (body.clone(), requests.clone());
        tokio::spawn(async move {
            let mut buf: Vec<u8> = Vec::new();
            let mut tmp = [0u8; 4096];
            while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                match sock.read(&mut tmp).await {
                    Ok(n) if n > 0 => buf.extend_from_slice(&tmp[..n]),
                    _ => return,
                }
            }
            let head = String::from_utf8_lossy(&buf).to_lowercase();
            let from: usize = head.lines().find_map(|l| l.strip_prefix("range: bytes=")).and_then(|r| r.trim().trim_end_matches('-').parse().ok()).filter(|_| ranges).unwrap_or(0);
            requests.lock().unwrap().push(format!("from {from}"));
            let rest = &body[from.min(body.len())..];
            let h = match from {
                0 => format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", rest.len()),
                _ => format!("HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {from}-{}/{}\r\n\r\n", rest.len(), body.len() - 1, body.len()),
            };
            let _ = sock.write_all(h.as_bytes()).await;
            let _ = sock.write_all(&rest[..rest.len().min(piece)]).await;
            let _ = sock.shutdown().await;
        });
    }
}

async fn faults() -> Result<()> {
    use resincraft_lib::core::net::download_file;
    let dir = paths::cache().join("speed-faults");
    let _ = tokio::fs::remove_dir_all(&dir).await;
    let mut failed = 0;
    let mut check = |name: &str, ok: bool, detail: String| {
        println!("{} {name}{}", if ok { "PASS" } else { "FAIL" }, if detail.is_empty() { String::new() } else { format!(": {detail}") });
        if !ok {
            failed += 1;
        }
    };

    // A file that only ever arrives in pieces, small (kept in memory) and big (written as it arrives).
    for (label, len, piece) in [("small", 300_000usize, 70_000usize), ("big", 3_000_000, 700_000)] {
        // (A host that cuts every answer at the same place and ignores Range is retried for ever, before and after
        // these changes, so that case isn't run here.)
        for ranges in [true] {
            let body = Arc::new(noise(len as u64, len));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let port = listener.local_addr()?.port();
            let requests = Arc::new(Mutex::new(Vec::new()));
            let server = tokio::spawn(serve_in_pieces(listener, body.clone(), piece, ranges, requests.clone()));
            let dest = dir.join(format!("{label}-{ranges}.bin"));
            let item = DownloadItem::new(format!("http://127.0.0.1:{port}/f"), &dest).hash(Some(Hash::sha1(hash_bytes(&body, HashAlgo::Sha1)))).size(Some(len as u64));
            let res = download_file(&item).await;
            let asked = requests.lock().unwrap().clone();
            let part = std::path::PathBuf::from(format!("{}.part", dest.display()));
            if ranges {
                // Each try goes on from where the last one stopped, and the pieces make the whole file.
                let whole = std::fs::read(&dest).map(|b| b == *body).unwrap_or(false);
                let resumed = asked.len() == len.div_ceil(piece) && asked.iter().enumerate().all(|(i, r)| *r == format!("from {}", i * piece));
                check(&format!("{label} file cut off again and again arrives whole"), res.is_ok() && whole && resumed && !part.exists(), format!("{:?} {asked:?}", res.err().map(|e| e.to_string())));
            } else {
                // A host that starts over every time never gets further: three tries, then an error and nothing left.
                check(&format!("{label} file from a host that always starts over fails after its tries"), res.is_err() && !dest.exists() && !part.exists(), format!("{} requests", asked.len()));
            }
            server.abort();
        }
    }

    // A whole file with the wrong hash, a missing one, and no address.
    let mut files: HashMap<String, Arc<Vec<u8>>> = HashMap::new();
    files.insert("small.bin".into(), Arc::new(noise(1, 50_000)));
    files.insert("big.bin".into(), Arc::new(noise(2, 2_500_000)));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    tokio::spawn(serve(listener, Arc::new(files.clone()), Duration::ZERO));
    for name in ["small.bin", "big.bin"] {
        let dest = dir.join(format!("wrong-{name}"));
        let part = std::path::PathBuf::from(format!("{}.part", dest.display()));
        let res = download_file(&DownloadItem::new(format!("http://127.0.0.1:{port}/{name}"), &dest).hash(Some(Hash::sha1("0".repeat(40))))).await;
        let said = res.as_ref().err().map(|e| e.to_string()).unwrap_or_default();
        check(&format!("{name} with the wrong hash is refused"), res.is_err() && said.contains("Checksum mismatch") && !dest.exists() && !part.exists(), said);
        // The same file with no hash to check, and with each kind of hash.
        let body = &files[name];
        for hash in [None, Some(Hash::md5(hash_bytes(body, HashAlgo::Md5))), Some(Hash::sha256(hash_bytes(body, HashAlgo::Sha256))), Some(Hash::sha512(hash_bytes(body, HashAlgo::Sha512)))] {
            let kind = hash.as_ref().map(|h| format!("{:?}", h.algo)).unwrap_or_else(|| "no hash".into());
            let dest = dir.join(format!("{kind}-{name}"));
            let res = download_file(&DownloadItem::new(format!("http://127.0.0.1:{port}/{name}"), &dest).hash(hash)).await;
            check(&format!("{name} checked with {kind}"), res.is_ok() && std::fs::read(&dest).map(|b| b == **body).unwrap_or(false), String::new());
        }
    }
    let dest = dir.join("missing.bin");
    let res = download_file(&DownloadItem::new(format!("http://127.0.0.1:{port}/nothing-here"), &dest)).await;
    check("a missing file fails at once", res.is_err() && !dest.exists(), res.err().map(|e| e.to_string()).unwrap_or_default());
    let res = download_file(&DownloadItem::new("", dir.join("nowhere.bin"))).await;
    check("no address is an error", res.is_err(), String::new());

    // Many files where one is wrong: the others that started still arrive, the error comes back.
    let mut items: Vec<DownloadItem> = ["small.bin", "big.bin"].iter().map(|n| DownloadItem::new(format!("http://127.0.0.1:{port}/{n}"), dir.join("all").join(n)).hash(Some(Hash::sha1(hash_bytes(&files[*n], HashAlgo::Sha1))))).collect();
    items.push(DownloadItem::new(format!("http://127.0.0.1:{port}/nothing-here"), dir.join("all").join("missing.bin")));
    let seen = Mutex::new(Vec::new());
    let res = download_all(items.clone(), 4, |done, total| seen.lock().unwrap().push((done, total))).await;
    check("one missing file among many fails the lot", res.is_err() && dir.join("all").join("big.bin").exists(), format!("{:?}", seen.lock().unwrap()));
    items.pop();
    let seen = Mutex::new(Vec::new());
    let res = download_all(items, 4, |done, total| seen.lock().unwrap().push((done, total))).await;
    let seen = seen.into_inner().unwrap();
    check("files already there are counted without downloading", res.is_ok() && seen.first() == Some(&(0, 2)) && seen.last() == Some(&(2, 2)), format!("{seen:?}"));

    let _ = tokio::fs::remove_dir_all(&dir).await;
    if failed > 0 {
        bail!("{failed} download checks failed");
    }
    println!("all download checks passed");
    Ok(())
}

// ---------- A pack's files from the real site ----------

async fn cdn(version: &str, times: usize, at_once: usize) -> Result<()> {
    use resincraft_lib::core::net::{download_file, fetch_json};
    let v: Value = fetch_json(&format!("https://api.modrinth.com/v2/version/{version}")).await?;
    let files = v["files"].as_array().cloned().unwrap_or_default();
    let pack = files.iter().find(|f| f["primary"].as_bool() == Some(true)).or(files.first()).ok_or_else(|| anyhow!("that version has no file"))?;
    let dir = paths::cache().join("speed-cdn");
    let _ = tokio::fs::remove_dir_all(&dir).await;
    let zip_path = dir.join("pack.mrpack");
    download_file(&DownloadItem::new(pack["url"].as_str().unwrap_or_default(), &zip_path)).await?;
    let text = tokio::task::spawn_blocking(move || -> Result<String> {
        use std::io::Read;
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&zip_path)?)?;
        let mut text = String::new();
        zip.by_name("modrinth.index.json")?.read_to_string(&mut text)?;
        Ok(text)
    })
    .await??;
    let index: Value = serde_json::from_str(&text)?;
    let items: Vec<DownloadItem> = index["files"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(i, f)| {
            DownloadItem::new(f["downloads"][0].as_str().unwrap_or_default(), dir.join("files").join(format!("{i}.bin")))
                .hash(f["hashes"]["sha1"].as_str().map(Hash::sha1))
                .size(f["fileSize"].as_u64())
        })
        .collect();
    let bytes: u64 = items.iter().filter_map(|i| i.size).sum();
    println!("cdn: {} files, {}, {at_once} at once", items.len(), mb(bytes));
    for n in 0..times.max(1) {
        let _ = tokio::fs::remove_dir_all(dir.join("files")).await;
        let t = Instant::now();
        let mut waiting = items.clone().into_iter();
        let mut running = tokio::task::JoinSet::new();
        let mut took: Vec<(Duration, u64, String, Option<String>)> = Vec::new();
        loop {
            while running.len() < at_once.max(1) {
                let Some(item) = waiting.next() else { break };
                running.spawn(async move {
                    let t = Instant::now();
                    let name = item.url.rsplit('/').next().unwrap_or_default().to_string();
                    if std::env::var_os("RESIN_SPEED_SPLIT").is_some() {
                        let note = fetch_split(&item).await.unwrap_or_else(|e| format!("FAILED {e}"));
                        return (t.elapsed(), item.size.unwrap_or(0), format!("{name}  [{note}]"), None);
                    }
                    let res = download_file(&item).await;
                    (t.elapsed(), item.size.unwrap_or(0), name, res.err().map(|e| e.to_string()))
                });
            }
            let Some(done) = running.join_next().await else { break };
            took.push(done?);
        }
        let total = t.elapsed();
        took.sort_by(|a, b| b.0.cmp(&a.0));
        let errors = took.iter().filter(|x| x.3.is_some()).count();
        println!("RESULT cdn #{}  {}  ({:.0} MB/s, {errors} failed)", n + 1, secs(total), bytes as f64 / 1e6 / total.as_secs_f64());
        for (dt, size, name, err) in took.iter().take(3) {
            println!("    {}  {:>8}  {}{}", secs(*dt), mb(*size), &name[..name.len().min(220)], err.as_ref().map(|e| format!("  {e}")).unwrap_or_default());
        }
    }
    let _ = tokio::fs::remove_dir_all(&dir).await;
    Ok(())
}

/// One file fetched by hand with the launcher's HTTP client, to tell where its time goes: until the first byte,
/// the rest of the body (and the longest wait for a piece of it), and writing, closing and renaming the file.
async fn fetch_split(item: &DownloadItem) -> Result<String> {
    let t = Instant::now();
    let mut res = ctx::http().get(&item.url).send().await?;
    let first = t.elapsed();
    let mut body: Vec<u8> = Vec::new();
    let mut last = Instant::now();
    let mut silence = Duration::ZERO;
    while let Some(chunk) = res.chunk().await? {
        silence = silence.max(last.elapsed());
        last = Instant::now();
        body.extend_from_slice(&chunk);
    }
    let received = t.elapsed();
    let dest = item.dest.clone();
    let t = Instant::now();
    let (hashing, writing, closing) = tokio::task::spawn_blocking(move || -> Result<(Duration, Duration, Duration)> {
        use std::io::Write;
        let t = Instant::now();
        let _ = hash_bytes(&body, HashAlgo::Sha1);
        let hashing = t.elapsed();
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = std::path::PathBuf::from(format!("{}.part", dest.display()));
        let t = Instant::now();
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(&body)?;
        let writing = t.elapsed();
        let t = Instant::now();
        drop(f);
        std::fs::rename(&tmp, &dest)?;
        Ok((hashing, writing, t.elapsed()))
    })
    .await??;
    Ok(format!(
        "first byte {:.1}s, body {:.1}s (longest wait {:.1}s), hash {:.2}s, write {:.2}s, close+rename {:.2}s, disk step in all {:.2}s",
        first.as_secs_f64(),
        (received - first).as_secs_f64(),
        silence.as_secs_f64(),
        hashing.as_secs_f64(),
        writing.as_secs_f64(),
        closing.as_secs_f64(),
        t.elapsed().as_secs_f64()
    ))
}

// ---------- Minecraft's own files ----------

async fn game(mc: &str, at_once: Option<usize>) -> Result<()> {
    let at_once = match at_once {
        Some(n) => n,
        None => settings::get().await.concurrency,
    };
    let total = Instant::now();
    let ctx = ResolveContext { task: None, concurrency: Some(at_once), java: None };
    let version = resolve_version(mc, LoaderType::Vanilla, None, &ctx).await?;
    let mut items = vec![mojang::client_jar(&version)?];
    items.extend(mojang::resolve_libraries(&version).into_iter().map(|l| l.item));
    items.extend(mojang::logging_config(&version).into_iter().map(|l| l.item));
    let items: Vec<DownloadItem> = items.into_iter().filter(|i| !i.url.is_empty()).collect();
    let assets = mojang::asset_items(&version).await?;
    println!("game {mc}: {} libraries and jars, {} assets, {at_once} at once ({} for assets)", items.len(), assets.items.len(), at_once * 2);

    let size = |list: &[DownloadItem]| -> u64 { list.iter().filter_map(|i| std::fs::metadata(&i.dest).ok()).map(|m| m.len()).sum() };
    let t = Instant::now();
    download_all(items.clone(), at_once, |_, _| {}).await?;
    println!("RESULT game libraries      {}  ({})", secs(t.elapsed()), mb(size(&items)));
    let t = Instant::now();
    download_all(assets.items.clone(), at_once * 2, |_, _| {}).await?;
    println!("RESULT game assets         {}  ({})", secs(t.elapsed()), mb(size(&assets.items)));
    println!("RESULT game total          {}", secs(total.elapsed()));
    let t = Instant::now();
    download_all(items, at_once, |_, _| {}).await?;
    download_all(assets.items, at_once * 2, |_, _| {}).await?;
    println!("RESULT game check again    {}  (everything already there)", secs(t.elapsed()));
    Ok(())
}

// ---------- Modpacks ----------

async fn pack(project: &str, version: &str) -> Result<()> {
    let t = Instant::now();
    let inst = sources::install_modpack(project, Some(version.to_string())).await?;
    println!("RESULT pack install        {}  ({} {})", secs(t.elapsed()), inst.name, version);
    println!("INSTANCE {}", inst.id);
    Ok(())
}

async fn switch(id: &str, versions: &[String]) -> Result<()> {
    if versions.is_empty() {
        bail!("missing a version id");
    }
    for v in versions {
        let t = Instant::now();
        let out = packswitch::switch_version(id, v).await?;
        println!(
            "RESULT pack switch → {v}  {}  ({}: {} added, {} updated, {} removed, kept {:?}, backups {:?})",
            secs(t.elapsed()),
            out.version_name,
            out.added,
            out.updated,
            out.removed,
            out.kept,
            out.backups
        );
    }
    Ok(())
}

// ---------- The duplicate check ----------

/// A jar that names its mod `id`, with `len` bytes of filler no site has ever seen.
fn make_jar(path: &Path, id: &str, name: &str, version: &str, seed: u64, len: usize) -> Result<()> {
    let mut w = zip::ZipWriter::new(std::fs::File::create(path)?);
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let packed = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    w.start_file("data/filler.bin", stored)?;
    w.write_all(&noise(seed, len))?;
    w.start_file("META-INF/neoforge.mods.toml", packed)?;
    w.write_all(format!("modLoader=\"javafml\"\nloaderVersion=\"[1,)\"\n[[mods]]\nmodId=\"{id}\"\nversion=\"{version}\"\ndisplayName=\"{name}\"\n").as_bytes())?;
    w.start_file("fabric.mod.json", packed)?;
    w.write_all(json!({ "schemaVersion": 1, "id": id, "name": name, "version": version }).to_string().as_bytes())?;
    w.finish()?;
    Ok(())
}

fn unknown(id: &str, count: usize) -> Result<()> {
    let mods = paths::game_dir(id).join("mods");
    std::fs::create_dir_all(&mods)?;
    let mut total = 0;
    for i in 0..count {
        // The first one is a copy of a mod the picks include (JEI), as a player's own build of it would be.
        let (mod_id, name) = if i == 0 { ("jei".to_string(), "Just Enough Items (own build)".to_string()) } else { (format!("handmade_{i}"), format!("Handmade {i}")) };
        let len = 400_000 + (i * 611_953) % 2_600_000;
        make_jar(&mods.join(format!("handmade-{i:02}.jar")), &mod_id, &name, &format!("1.0.{i}"), 7_000 + i as u64, len)?;
        total += len;
    }
    println!("added {count} jars no site knows to {} ({})", mods.display(), mb(total as u64));
    Ok(())
}

/// Mods that are on both sites, by the name people search for.
const WANTED: [&str; 12] = [
    "Just Enough Items",
    "Jade",
    "Mouse Tweaks",
    "AppleSkin",
    "Clumps",
    "Xaero's Minimap",
    "Farmer's Delight",
    "Sophisticated Backpacks",
    "Waystones",
    "Iron Chests",
    "Carry On",
    "Controlling",
];

async fn picks(id: &str, out: &Path) -> Result<()> {
    let inst = instances::get(id).await?;
    let mut list: Vec<Value> = Vec::new();
    for (i, name) in WANTED.iter().enumerate() {
        // Half from each site, and the first four from both (the same mod picked twice).
        let sources: &[ContentSource] = match i {
            0..=3 => &[ContentSource::Modrinth, ContentSource::Curseforge],
            _ if i % 2 == 0 => &[ContentSource::Modrinth],
            _ => &[ContentSource::Curseforge],
        };
        for source in sources {
            let q = SearchQuery {
                query: name.to_string(),
                kind: ContentKind::Mod,
                source: Some(*source),
                game_version: Some(inst.mc_version.clone()),
                loader: Some(inst.loader),
                sort: None,
                offset: None,
                limit: Some(5),
            };
            let hits = sources::search(&q).await?.hits;
            let Some(h) = hits.iter().find(|h| h.title.to_lowercase().starts_with(&name.to_lowercase())).or(hits.first()) else {
                println!("  nothing for {name} on {source:?}");
                continue;
            };
            println!("  {name}: {} ({}) by {}", h.title, h.id, h.author);
            list.push(json!({ "id": h.id, "kind": "mod", "title": h.title, "author": h.author, "slug": h.slug }));
        }
    }
    std::fs::write(out, serde_json::to_vec_pretty(&list)?)?;
    println!("saved {} picks to {}", list.len(), out.display());
    Ok(())
}

async fn dupes_check(id: &str, picks: &Path, times: usize, out: Option<String>) -> Result<()> {
    let picks: Vec<ModPick> = serde_json::from_slice(&std::fs::read(picks)?)?;
    let mods = std::fs::read_dir(paths::game_dir(id).join("mods")).map(|d| d.flatten().count()).unwrap_or(0);
    println!("dupes: {} picks against {mods} files in mods/", picks.len());
    for n in 0..times.max(1) {
        let t = Instant::now();
        let found = dupes::check(id, picks.clone()).await?;
        println!("RESULT duplicate check #{}  {}  ({} groups, {} already installed)", n + 1, secs(t.elapsed()), found.groups.len(), found.installed.len());
        if n == 0 {
            if let Some(out) = &out {
                std::fs::write(out, serde_json::to_vec_pretty(&found)?)?;
            }
        }
    }
    Ok(())
}

// ---------- The steps after a modpack's files are in place ----------

/// Times what a modpack install does once its files are downloaded, one step at a time, on an installed pack:
/// the check for mods the pack has twice (as at install: nothing known about the files yet) and the record of the
/// pack's files. Leaves the instance as an install would.
async fn steps(id: &str) -> Result<()> {
    let inst = instances::get(id).await?;
    let mods = paths::game_dir(id).join("mods");
    let jars: Vec<PathBuf> = std::fs::read_dir(&mods)?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "jar")).collect();
    let bytes: u64 = jars.iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len()).sum();
    println!("steps: {} jars, {}", jars.len(), mb(bytes));

    let t = Instant::now();
    for p in &jars {
        resincraft_lib::core::net::hash_file(p, HashAlgo::Sha1).await?;
    }
    println!("RESULT sha1 of every jar, one by one   {}  ({:.0} MB/s)", secs(t.elapsed()), bytes as f64 / 1e6 / t.elapsed().as_secs_f64());

    let t = Instant::now();
    let list = jars.clone();
    let named = tokio::task::spawn_blocking(move || list.iter().filter(|p| !resincraft_lib::content::jarmeta::read_jar(p).ids.is_empty()).count()).await?;
    println!("RESULT mod ids read from every jar     {}  ({named} name a mod)", secs(t.elapsed()));

    // As at install: the launcher hasn't looked at any of the files yet.
    let index = paths::instance_dir(id).join("content-index.json");
    let _ = std::fs::remove_file(&index);
    let t = Instant::now();
    let items = sources::list_content(id, ContentKind::Mod).await?;
    println!("RESULT mods list, nothing known yet    {}  ({} of {} recognised)", secs(t.elapsed()), items.iter().filter(|i| i.project_id.is_some()).count(), items.len());
    let t = Instant::now();
    sources::list_content(id, ContentKind::Mod).await?;
    println!("RESULT mods list again                 {}", secs(t.elapsed()));

    let _ = std::fs::remove_file(&index);
    let listed: Vec<String> = jars.iter().filter_map(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).collect();
    let task = events::Task::new("Checking for mods the pack has twice…", Some(id.to_string()));
    let t = Instant::now();
    dupes::dedupe_pack(&inst, &task, &listed).await;
    task.done();
    println!("RESULT check for mods the pack has twice {}", secs(t.elapsed()));

    let t = Instant::now();
    packswitch::record_install(&inst).await;
    println!("RESULT record of the pack's files      {}", secs(t.elapsed()));
    Ok(())
}

// ---------- Play ----------

async fn make(mc: &str, loader: &str, name: Option<String>) -> Result<()> {
    let loader: LoaderType = serde_json::from_value(Value::String(loader.to_string())).map_err(|_| anyhow!("loader must be vanilla, fabric or quilt"))?;
    let new = NewInstance { name: name.unwrap_or_else(|| format!("Speed {mc} {}", loader.as_str())), mc_version: mc.to_string(), loader, loader_version: None, color: None, group: None };
    let inst = instances::create(new, |_| {}).await?;
    println!("INSTANCE {}", inst.id);
    Ok(())
}

/// Presses Play `times` times: each time, how long until the game's process is running (everything the launcher
/// does first: version files, libraries, assets, natives, Java). The game is stopped again as soon as it runs.
/// The last start's Java command line (with the data folder's path taken out) goes to `command_out`, to compare builds.
async fn launch_times(id: &str, times: usize, command_out: Option<String>) -> Result<()> {
    let mut s = settings::get().await;
    s.memory_mb = 2048;
    for n in 0..times.max(1) {
        let auth = LaunchAuth { account_id: "offline-test".into(), username: "ResinTester".into(), uuid: "00000000-0000-0000-0000-000000000000".into(), access_token: "0".into(), user_type: "legacy", xuid: None };
        let t = Instant::now();
        launch::launch_instance(id, &s, auth, launch::LaunchHooks::default()).await?;
        println!("RESULT launch #{}  Play to game process {}", n + 1, secs(t.elapsed()));
        // Long enough for Java to open its files, as a real start does; then stop it.
        tokio::time::sleep(Duration::from_millis(2500)).await;
        launch::kill_instance(id, None);
        let _ = tokio::time::timeout(Duration::from_secs(30), STOPPED.notified()).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    if let (Some(out), Some(c)) = (command_out, COMMAND.lock().unwrap().clone()) {
        // The same command from another data folder differs only in that folder's path.
        std::fs::write(out, c.replace(&mojang::plain(&paths::root()), "<data>").replace('\\', "/").replace(" -", "\n-"))?;
    }
    Ok(())
}

/// The background step that gets instances ready (what making an instance starts), timed until it's done.
async fn ready(args: &[String]) -> Result<()> {
    let play = args.last().is_some_and(|a| a == "play");
    let ids: Vec<&String> = args.iter().filter(|a| *a != "play").collect();
    let Some(first) = ids.first() else { bail!("missing an instance id") };
    let t = Instant::now();
    for id in &ids {
        launch::get_ready(id);
    }
    let mut ended_by_play = 0;
    if play {
        // Play pressed while the files are still arriving: it waits for them, then starts the game.
        tokio::time::sleep(Duration::from_secs(2)).await;
        let mut s = settings::get().await;
        s.memory_mb = 2048;
        let auth = LaunchAuth { account_id: "offline-test".into(), username: "ResinTester".into(), uuid: "00000000-0000-0000-0000-000000000000".into(), access_token: "0".into(), user_type: "legacy", xuid: None };
        let p = Instant::now();
        launch::launch_instance(first, &s, auth, launch::LaunchHooks::default()).await?;
        println!("RESULT Play pressed 2 s into getting ready, until the game process {}", secs(p.elapsed()));
        tokio::time::sleep(Duration::from_millis(2500)).await;
        launch::kill_instance(first, None);
        let _ = tokio::time::timeout(Duration::from_secs(30), STOPPED.notified()).await;
        ended_by_play = 1;
    }
    // Every get-ready task (and Play's own) has ended.
    while TASKS_ENDED.load(std::sync::atomic::Ordering::SeqCst) < ids.len() + ended_by_play {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    println!("RESULT {} instance(s) ready in the background {}", ids.len(), secs(t.elapsed()));
    Ok(())
}

/// What Play does before Java starts, one step at a time (as launch.rs `prepare` does them; unpacking the natives is
/// inside `prepare`, so it only shows in `launch`).
async fn prep(id: &str, times: usize) -> Result<()> {
    let inst = instances::get(id).await?;
    let at_once = settings::get().await.concurrency;
    for n in 0..times.max(1) {
        let total = Instant::now();
        let mut line = format!("RESULT prep #{}", n + 1);
        let t = Instant::now();
        let ctx = ResolveContext { task: None, concurrency: Some(at_once), java: None };
        let version = versionpatch::resolve_instance(&inst, &ctx).await?;
        line += &format!("  version {}", secs(t.elapsed()));

        let t = Instant::now();
        let mut items = vec![mojang::client_jar(&version)?];
        items.extend(mojang::resolve_libraries(&version).into_iter().map(|l| l.item));
        items.extend(mojang::logging_config(&version).into_iter().map(|l| l.item));
        let items: Vec<DownloadItem> = items.into_iter().filter(|i| !i.url.is_empty()).collect();
        let assets = mojang::asset_items(&version).await?;
        line += &format!("  lists {}", secs(t.elapsed()));

        let t = Instant::now();
        download_all(items, at_once, |_, _| {}).await?;
        line += &format!("  libraries {}", secs(t.elapsed()));

        let t = Instant::now();
        let count = assets.items.len();
        download_all(assets.items, at_once * 2, |_, _| {}).await?;
        line += &format!("  {count} assets {}", secs(t.elapsed()));

        let t = Instant::now();
        let found = java::find_java(version.java_major()).await?;
        line += &format!("  Java {}", secs(t.elapsed()));
        println!("{line}  total {}  ({})", secs(total.elapsed()), found.map(|j| j.path).unwrap_or_else(|| "no Java".into()));
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    Ok(())
}

// ---------- What ended up on disk ----------

fn sha1_of(p: &Path) -> String {
    std::fs::read(p).map(|b| hash_bytes(&b, HashAlgo::Sha1)).unwrap_or_else(|_| "unreadable".into())
}

fn tree(id: &str, out: Option<String>) -> Result<()> {
    fn go(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                go(root, &p, out);
            } else {
                out.insert(packswitch::norm(&p.strip_prefix(root).unwrap_or(&p).to_string_lossy()), sha1_of(&p));
            }
        }
    }
    let mut files = BTreeMap::new();
    let game = paths::game_dir(id);
    go(&game, &game, &mut files);
    let mut text = String::new();
    for (p, h) in &files {
        text.push_str(&format!("{h}  {p}\n"));
    }
    // The launcher's own notes on the instance: what the pack installed, and where each file came from.
    for name in ["modpack-files.json", "content-index.json"] {
        let v: Value = serde_json::from_str(&std::fs::read_to_string(paths::instance_dir(id).join(name)).unwrap_or_default()).unwrap_or(Value::Null);
        text.push_str(&format!("{}  <{name}>\n", hash_bytes(serde_json::to_string(&v)?.as_bytes(), HashAlgo::Sha1)));
    }
    println!("{} files in {}", files.len(), game.display());
    match out {
        Some(out) => std::fs::write(out, text)?,
        None => print!("{text}"),
    }
    Ok(())
}
