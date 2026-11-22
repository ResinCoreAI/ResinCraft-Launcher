//! A backup's round trip through the engine, as Settings → Move to another PC does it, on a launcher data folder
//! (a dev build's `.dev-data`, with the launcher closed):
//!   cargo run --example check_backup -- <data folder> [backup file]
//! Without a backup file it makes one of each kind first (into the temp folder) and imports the PRAR one. Every
//! instance then comes in again next to itself (as `<id>-2`, since the id is taken), is compared file by file with
//! the original and removed. A second import is cancelled halfway and must leave nothing behind.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use resincraft_lib::core::backup::{self, Format};
use resincraft_lib::core::{ctx, events, paths};

fn files(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let rel = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
        if rel == "natives" {
            continue;
        }
        match e.file_type() {
            Ok(t) if t.is_dir() => files(&e.path(), &rel, out),
            Ok(t) if t.is_file() => out.push((rel, e.path())),
            _ => {}
        }
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    use std::io::Read;
    let (Ok(mut fa), Ok(mut fb)) = (std::fs::File::open(a), std::fs::File::open(b)) else { return false };
    let (mut ba, mut bb) = (vec![0u8; 1 << 20], vec![0u8; 1 << 20]);
    loop {
        let na = fa.read(&mut ba).unwrap_or(usize::MAX);
        let nb = fb.read(&mut bb).unwrap_or(usize::MAX - 1);
        if na != nb {
            // (read sizes can differ in principle; files this test meets are read in full pieces)
            return false;
        }
        if na == 0 {
            return true;
        }
        if ba[..na] != bb[..nb] {
            return false;
        }
    }
}

fn leftovers(before: &[String]) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(paths::instances()).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| !before.contains(n)).collect();
    out.extend(std::fs::read_dir(paths::games()).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.starts_with(".backup-import")));
    out
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let data = PathBuf::from(args.next().expect("usage: check_backup <data folder> [backup file]"));
    ctx::init(data, None);
    let mut ok = true;

    let path = match args.next() {
        Some(p) => p,
        None => {
            let dir = std::env::temp_dir().join(format!("resin-check-backup-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let mut made = String::new();
            for (format, name) in [(Format::Prar, "backup.prar"), (Format::Zip, "backup.resincraft")] {
                let dest = dir.join(name);
                let t = Instant::now();
                let r = backup::export(Some(dest.to_string_lossy().to_string()), false, format).await.unwrap().unwrap();
                println!("export {format:?}: {:.1} MB in {:.1}s ({} instances)", r.bytes as f64 / 1e6, t.elapsed().as_secs_f64(), r.instances);
                if format == Format::Prar {
                    made = r.path;
                }
            }
            made
        }
    };

    let info = backup::inspect(&path).await.unwrap();
    println!("{path}: {:?}, {} instances, {} servers, {:.1} MB inside", info.kind, info.manifest.instances.len(), info.manifest.servers.len(), info.manifest.bytes as f64 / 1e6);
    let before: Vec<String> = std::fs::read_dir(paths::instances()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();

    // Import: everything comes in as copies next to the originals.
    let t = Instant::now();
    let r = backup::import(path.clone(), false).await.unwrap();
    println!("import: {} instances, {} renamed, in {:.1}s", r.instances, r.renamed, t.elapsed().as_secs_f64());
    for item in &info.manifest.instances {
        let (orig, copy) = (paths::instance_dir(&item.id), paths::instance_dir(&format!("{}-2", item.id)));
        let (mut a, mut b) = (Vec::new(), Vec::new());
        files(&orig, "", &mut a);
        files(&copy, "", &mut b);
        a.sort();
        b.sort();
        let names_match = a.iter().map(|f| &f.0).eq(b.iter().map(|f| &f.0));
        let bytes_match = names_match && a.iter().zip(&b).filter(|(x, _)| x.0 != "instance.json").all(|(x, y)| same_file(&x.1, &y.1));
        let meta = std::fs::read_to_string(copy.join("instance.json")).unwrap_or_default();
        let id_moved = meta.contains(&format!("\"{}-2\"", item.id));
        println!("  {} -> {}-2: {} files, names {}, bytes {}, id {}", item.id, item.id, b.len(), names_match, bytes_match, id_moved);
        ok &= names_match && bytes_match && id_moved;
        let _ = std::fs::remove_dir_all(&copy);
    }
    let left = leftovers(&before);
    println!("  left after removing the copies: {left:?}");
    ok &= left.is_empty();

    // Cancel halfway: nothing may stay behind (the unpacking thread cleans up after the task is dropped).
    let mut rx = events::tap();
    let p2 = path.clone();
    let job = tokio::spawn(async move { backup::import(p2, false).await });
    let mut cancelled_at = None;
    while let Ok(Ok((channel, v))) = tokio::time::timeout(Duration::from_secs(120), rx.recv()).await {
        let progress = v.get("progress").and_then(|p| p.as_f64()).unwrap_or(0.0);
        if channel == "task" && v.get("state").and_then(|s| s.as_str()) == Some("running") && progress >= 0.2 {
            let id = v.get("id").and_then(|s| s.as_str()).unwrap_or_default().to_string();
            cancelled_at = Some(progress);
            events::cancel(&id);
            break;
        }
    }
    let result = job.await.unwrap();
    let was_cancelled = result.as_ref().is_err_and(events::is_cancelled);
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut left = leftovers(&before);
    while !left.is_empty() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(500)).await;
        left = leftovers(&before);
    }
    println!("cancel at {:?}: cancelled {}, left {left:?}", cancelled_at.map(|p| format!("{:.0}%", p * 100.0)), was_cancelled);
    ok &= cancelled_at.is_some() && was_cancelled && left.is_empty();

    println!("{}", if ok { "PASS" } else { "FAIL" });
    std::process::exit(if ok { 0 } else { 1 });
}
