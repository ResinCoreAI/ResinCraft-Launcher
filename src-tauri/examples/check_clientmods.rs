//! Copies a server's jars into a scratch folder and runs the client-only checks on them, as a server start does:
//!   cargo run --example check_clientmods -- <folder of jars> [crash report]
//! Prints which jars the check turns off (and why), then which ones the crash report names.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use resincraft_lib::core::{ctx, paths};
use resincraft_lib::server::clientmods::{culprits, disabled, sweep};

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let jars = PathBuf::from(args.next().expect("folder of jars"));
    let report = args.next().map(PathBuf::from);
    ctx::init(paths::default_data_dir(None), None);

    let dir = std::env::temp_dir().join(format!("resin-clientmods-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("mods")).unwrap();
    for e in std::fs::read_dir(&jars).unwrap().flatten() {
        std::fs::copy(e.path(), dir.join("mods").join(e.file_name())).unwrap();
    }
    let moved = sweep(&dir).await;
    println!("turned off: {moved:?}");
    let left: Vec<String> = std::fs::read_dir(dir.join("mods")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".jar")).collect();
    println!("still on: {left:?}");
    println!("recorded: {:?}", disabled(&dir).await.iter().map(|d| format!("{} ({:?})", d.file, d.reason)).collect::<Vec<_>>());
    let again = sweep(&dir).await;
    println!("second start turns off: {again:?}");

    if let Some(report) = report {
        // Put the moved jars back so the crash check has them to find.
        for e in std::fs::read_dir(dir.join("mods")).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if let Some(jar) = name.strip_suffix(".disabled") {
                std::fs::rename(e.path(), dir.join("mods").join(jar)).unwrap();
            }
        }
        std::fs::create_dir_all(dir.join("crash-reports")).unwrap();
        // Written fresh, as the server would (a copy keeps the old modified time).
        std::fs::write(dir.join("crash-reports/crash.txt"), std::fs::read(&report).unwrap()).unwrap();
        let found = culprits(&dir, &[], SystemTime::now() - Duration::from_secs(60));
        println!("named by the crash report: {:?}", found.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect::<Vec<_>>());
    }
    let _ = std::fs::remove_dir_all(&dir);
}
