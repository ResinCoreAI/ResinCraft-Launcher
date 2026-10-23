//! Runs the crash fixer on a throwaway instance whose logs say a mod is missing, a mod failed to load, the Java is
//! too old and memory ran out, then applies the fixes:
//!   cargo run --example check_crashfix

use std::io::Write;
use std::time::{Duration, SystemTime};

use resincraft_lib::core::types::{LoaderType, NewInstance};
use resincraft_lib::core::{ctx, instances, paths};
use resincraft_lib::game::crashfix::{apply, diagnose, Fix};

fn jar(path: &std::path::Path, toml_id: &str) {
    let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    z.start_file("META-INF/mods.toml", zip::write::SimpleFileOptions::default()).unwrap();
    write!(z, "modLoader=\"javafml\"\n[[mods]]\nmodId=\"{toml_id}\"\n").unwrap();
    z.finish().unwrap();
}

#[tokio::main]
async fn main() {
    let data = std::env::temp_dir().join(format!("resin-crashfix-{}", std::process::id()));
    ctx::init(data.clone(), None);
    let new = NewInstance { name: "Crashy".into(), mc_version: "1.20.1".into(), loader: LoaderType::Vanilla, loader_version: None, color: None, group: None };
    let inst = instances::create(new, |i| i.java_path = Some("C:/old/java8/bin/javaw.exe".into())).await.unwrap();
    let game = paths::game_dir(&inst.id);
    std::fs::create_dir_all(game.join("mods")).unwrap();
    std::fs::create_dir_all(game.join("logs")).unwrap();
    std::fs::create_dir_all(game.join("crash-reports")).unwrap();
    jar(&game.join("mods/citresewn-1.20.1-5.jar"), "citresewn");
    jar(&game.join("mods/mowziesmobs-1.6.4.jar"), "mowziesmobs");

    let since = SystemTime::now() - Duration::from_secs(5);
    std::fs::write(
        game.join("logs/latest.log"),
        "[main/ERROR]: Missing or unsupported mandatory dependencies:\n\tMod ID: 'geckolib', Requested by: 'mowziesmobs', Expected range: '[4.4,)', Actual version: '[MISSING]'\n\
         [modloading-worker-0/ERROR] [ne.mi.fm.ja.FMLModContainer/LOADING]: Failed to create mod instance. ModID: citresewn, class shcm.CITResewn\n\
         java.lang.OutOfMemoryError: Java heap space\n",
    )
    .unwrap();
    let console = vec!["Exception in thread \"main\" java.lang.UnsupportedClassVersionError: net/minecraft/x has been compiled by a more recent version of the Java Runtime (class file version 61.0), this version of the Java Runtime only recognizes class file versions up to 52.0".to_string()];

    let fresh = instances::get(&inst.id).await.unwrap();
    let (problems, report) = diagnose(&fresh, since, &console).await;
    for p in &problems {
        println!("- {}  ->  {:?}", p.title.text, p.fix);
    }
    println!("crash report: {report:?}");

    let fixes: Vec<Fix> = problems.iter().filter_map(|p| p.fix.clone()).collect();
    let mut ok = fixes.iter().any(|f| matches!(f, Fix::InstallMod { name, .. } if name == "geckolib"))
        && fixes.contains(&Fix::DisableMod { file: "citresewn-1.20.1-5.jar".into() })
        && fixes.contains(&Fix::JavaAuto)
        && fixes.iter().any(|f| matches!(f, Fix::Memory { .. }));

    // The fixes that need no download.
    for f in fixes.iter().filter(|f| !matches!(f, Fix::InstallMod { .. })) {
        apply(&inst.id, f).await.unwrap();
    }
    let after = instances::get(&inst.id).await.unwrap();
    ok &= game.join("mods/citresewn-1.20.1-5.jar.disabled").is_file();
    ok &= after.java_path.is_none();
    ok &= after.memory_mb.is_some();
    println!("after fixes: java {:?}, memory {:?}, citresewn off: {}", after.java_path, after.memory_mb, game.join("mods/citresewn-1.20.1-5.jar.disabled").is_file());

    let _ = std::fs::remove_dir_all(&data);
    println!("{}", if ok { "PASS" } else { "FAIL" });
    std::process::exit(if ok { 0 } else { 1 });
}
