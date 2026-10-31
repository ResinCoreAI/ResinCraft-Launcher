use std::io::Cursor;

use super::*;
use crate::core::types::ServerKind;

/// A scratch folder of its own, removed afterwards.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("rc-transfer-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn file(&self, rel: &str, text: &str) -> &Self {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
        self
    }
    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.0.join(rel)).unwrap()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn server(s: &Scratch) {
    s.file("eula.txt", "eula=true\n")
        .file("server.json", "{}")
        .file("schedule.json", "{}")
        .file("server.properties", "server-port=25565\nrcon.password=hunter2\nmanagement-server-secret=abc\nmotd=hi\n")
        .file("config/paper-global.yml", "proxies:\n  velocity:\n    enabled: true\n    secret: topsecret\n")
        .file("velocity.toml", "forwarding-secret = \"oldstyle\"\nbind = \"0.0.0.0:25577\"\n")
        .file("forwarding.secret", "supersecret")
        .file("plugins/Geyser/key.pem", "KEY")
        .file("plugins/bStats/config.yml", "serverUuid: 123\n")
        .file("plugins/LuckPerms/config.yml", "password: 'dbpass'\nuse-password: true\nmax-password-length: 12\napi_key: k1\n")
        .file("plugins/x.jar", "jar")
        .file("mods/y.jar", "jar")
        .file("paper.jar", "jar")
        .file("world/level.dat", "lvl")
        .file("world/session.lock", "lock")
        .file("world/region/r.0.0.mca", "r")
        .file("world_nether/level.dat", "lvl")
        .file("creative/level.dat", "lvl")
        .file("logs/latest.log", "log")
        .file("crash-reports/a.txt", "crash")
        .file("cache/mojang.jar", "cache")
        .file("usercache.json", "[]")
        .file("backups/a.zip", "zip")
        .file(".resincraft/pending-addons.json", "[]")
        .file("instance-sync.json", "{}");
}

fn export_rules(dir: &Path) -> Rules {
    Rules { purpose: Purpose::Export, worlds: world_folders(dir, "world"), keep_worlds: true, logs: false, backups: false, caches: false }
}

fn names(listing: &Listing) -> BTreeSet<String> {
    listing.files.iter().map(|(p, _)| p.to_string_lossy().replace('\\', "/")).collect()
}

fn manifest() -> Manifest {
    Manifest {
        format: FORMAT,
        name: "Test".into(),
        kind: ServerKind::Velocity,
        mc_version: "1.21.4".into(),
        loader_version: None,
        memory_mb: 1024,
        launch: ServerLaunch { jar: Some("paper.jar".into()), ..Default::default() },
        optimized_flags: true,
        modpack: None,
        built_pack: false,
        pending_downloads: Vec::new(),
    }
}

fn zip_of(entries: &[(&str, &str)]) -> zip::ZipArchive<Cursor<Vec<u8>>> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, text) in entries {
        w.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(text.as_bytes()).unwrap();
    }
    zip::ZipArchive::new(Cursor::new(w.finish().unwrap().into_inner())).unwrap()
}

fn manifest_text() -> String {
    serde_json::to_string(&manifest()).unwrap()
}

#[test]
fn an_export_leaves_out_logs_caches_backups_and_secrets() {
    let s = Scratch::new();
    server(&s);
    let got = names(&list_files(&s.0, &export_rules(&s.0)));
    for name in ["server.properties", "plugins/x.jar", "mods/y.jar", "paper.jar", "world/level.dat", "world/region/r.0.0.mca", "creative/level.dat", "config/paper-global.yml"] {
        assert!(got.contains(name), "{name} comes along");
    }
    for name in [
        "eula.txt", "server.json", "schedule.json", "forwarding.secret", "plugins/Geyser/key.pem", "plugins/bStats/config.yml", "world/session.lock", "logs/latest.log", "crash-reports/a.txt",
        "cache/mojang.jar", "usercache.json", "backups/a.zip", ".resincraft/pending-addons.json", "instance-sync.json",
    ] {
        assert!(!got.contains(name), "{name} stays behind");
    }
}

#[test]
fn logs_backups_and_caches_are_the_players_choice_but_the_agreement_never_is() {
    let s = Scratch::new();
    server(&s);
    let rules = Rules { logs: true, backups: true, caches: true, ..export_rules(&s.0) };
    let got = names(&list_files(&s.0, &rules));
    assert!(got.contains("logs/latest.log") && got.contains("backups/a.zip") && got.contains("cache/mojang.jar"));
    assert!(!got.contains("eula.txt") && !got.contains("forwarding.secret"));
}

#[test]
fn without_worlds_every_world_folder_stays() {
    let s = Scratch::new();
    server(&s);
    let rules = Rules { keep_worlds: false, ..export_rules(&s.0) };
    let got = names(&list_files(&s.0, &rules));
    assert!(!got.iter().any(|n| n.starts_with("world/") || n.starts_with("world_nether/") || n.starts_with("creative/")), "{got:?}");
    assert!(got.contains("plugins/x.jar"));
}

#[test]
fn a_clone_keeps_its_own_config_but_not_the_agreement_or_history() {
    let s = Scratch::new();
    server(&s);
    let rules = Rules { purpose: Purpose::Clone, ..export_rules(&s.0) };
    let got = names(&list_files(&s.0, &rules));
    assert!(got.contains("forwarding.secret") && got.contains("instance-sync.json"), "still this PC's own setup");
    assert!(!got.contains("eula.txt") && !got.contains("server.json") && !got.contains("schedule.json") && !got.contains("logs/latest.log") && !got.contains("backups/a.zip"));
}

#[test]
fn the_agreement_stays_out_however_its_name_is_written() {
    for name in ["eula.txt", "EULA.TXT", "./eula.txt", ".\\eula.txt", "eula.txt.", "eula.txt ", "Eula.Txt..."] {
        assert!(left_out(&Rules::import(), name), "{name}");
    }
    assert!(!left_out(&Rules::import(), "plugins/eula.txt"), "only the server's own");
}

#[test]
fn config_files_lose_their_secrets_and_keep_the_rest() {
    let out = redact("plugins/a/config.yml", "password: 'dbpass'\nuse-password: true\nmax-password-length: 12\napi_key: k1\nname: me\n# token: example\n").unwrap();
    assert_eq!(out, "password: \"\"\nuse-password: true\nmax-password-length: 12\napi_key: \"\"\nname: me\n# token: example\n");
    assert_eq!(redact("server.properties", "rcon.password=hunter2\r\nmanagement-server-secret=abc\r\nmotd=hi\r\n").unwrap(), "rcon.password=\r\nmanagement-server-secret=\r\nmotd=hi\r\n");
    assert_eq!(redact("velocity.toml", "forwarding-secret = \"x\"\n").unwrap(), "forwarding-secret = \"\"\n");
    assert_eq!(redact("a.json", "{\n  \"token\": \"abc\",\n  \"n\": 1\n}\n").unwrap(), "{\n  \"token\": \"\",\n  \"n\": 1\n}\n");
    assert!(redact("server.properties", "rcon.password=\nmotd=hi\n").is_none(), "nothing to change");
    assert!(redact("a.yml", "token:\n  - a\ntokens: [a]\n").is_none());
}

#[test]
fn a_zip_goes_out_and_comes_back() {
    let (s, out, dst) = (Scratch::new(), Scratch::new(), Scratch::new());
    server(&s);
    let listing = list_files(&s.0, &export_rules(&s.0));
    let target = out.0.join("server.zip");
    let (stop, done) = (AtomicBool::new(false), AtomicU64::new(0));
    let bytes = serde_json::to_vec(&manifest()).unwrap();
    let (skipped, size) = write_zip(&s.0, &listing, &export_rules(&s.0).worlds, &bytes, &target, &stop, &done).unwrap();
    assert!(skipped.is_empty() && size > 0);
    assert!(!out.0.join("server.zip.part").exists());

    let mut zip = open_zip(&target).unwrap();
    let plan = read_plan(&mut zip).unwrap();
    assert_eq!((plan.prefix.as_str(), plan.manifest.name.as_str()), ("", "Test"));
    unpack(&mut zip, &plan.prefix, &dst.0, &stop, |_| {}).unwrap();

    assert_eq!(dst.read("plugins/x.jar"), "jar");
    assert_eq!(dst.read("world/region/r.0.0.mca"), "r");
    assert_eq!(dst.read("server.properties"), "server-port=25565\nrcon.password=\nmanagement-server-secret=\nmotd=hi\n");
    assert!(!dst.read("config/paper-global.yml").contains("topsecret"));
    assert!(dst.read("config/paper-global.yml").contains("enabled: true"));
    assert!(!dst.read("velocity.toml").contains("oldstyle"));
    assert!(!dst.read("plugins/LuckPerms/config.yml").contains("dbpass"));
    assert!(!dst.0.join("eula.txt").exists() && !dst.0.join(MANIFEST).exists() && !dst.0.join("server.json").exists());
    assert_eq!(s.read("eula.txt"), "eula=true\n", "the source is only read");
}

#[test]
fn a_cancelled_export_leaves_nothing() {
    let (s, out) = (Scratch::new(), Scratch::new());
    server(&s);
    let listing = list_files(&s.0, &export_rules(&s.0));
    let target = out.0.join("server.zip");
    let (stop, done) = (AtomicBool::new(true), AtomicU64::new(0));
    assert!(write_zip(&s.0, &listing, &BTreeSet::new(), b"{}", &target, &stop, &done).is_err());
    assert_eq!(std::fs::read_dir(&out.0).unwrap().count(), 0);
}

#[test]
fn a_zip_with_a_way_out_is_refused_whole() {
    let m = manifest_text();
    for bad in ["../evil.txt", "a/../../evil.txt", "..\\evil.txt", "C:\\evil.txt", "C:/evil.txt", "eula.txt:hidden", "a/.. /evil.txt", "a/... /evil.txt"] {
        let mut zip = zip_of(&[(MANIFEST, m.as_str()), (bad, "x")]);
        assert!(read_plan(&mut zip).unwrap_err().to_string().contains("unsafe path"), "{bad}");
    }
    let mut outside = manifest();
    outside.launch.jar = Some("../../java.exe".into());
    let mut zip = zip_of(&[(MANIFEST, &serde_json::to_string(&outside).unwrap())]);
    assert!(read_plan(&mut zip).is_err(), "a launch file outside the folder");
}

#[test]
fn absolute_names_land_inside_and_the_agreement_never_lands() {
    let dst = Scratch::new();
    let m = manifest_text();
    let mut zip = zip_of(&[(MANIFEST, m.as_str()), ("/abs/file.txt", "a"), ("eula.txt", "eula=true\n"), ("EULA.TXT.", "eula=true\n"), ("server.json", "{\"id\":\"x\"}"), ("schedule.json", "{}"), (".resincraft/pending-addons.json", "[]"), ("plugins/a.pem", "k")]);
    let plan = read_plan(&mut zip).unwrap();
    unpack(&mut zip, &plan.prefix, &dst.0, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(dst.read("abs/file.txt"), "a");
    for gone in ["eula.txt", "server.json", "schedule.json", ".resincraft", "plugins/a.pem", MANIFEST] {
        assert!(!dst.0.join(gone).exists(), "{gone}");
    }
}

#[test]
fn a_zip_zipped_again_in_a_folder_still_imports() {
    let dst = Scratch::new();
    let m = manifest_text();
    let name = format!("Exported/{MANIFEST}");
    let mut zip = zip_of(&[(name.as_str(), m.as_str()), ("Exported/plugins/a.jar", "a"), ("Exported/eula.txt", "eula=true\n"), ("other.txt", "not the server's")]);
    let plan = read_plan(&mut zip).unwrap();
    assert_eq!(plan.prefix, "Exported/");
    unpack(&mut zip, &plan.prefix, &dst.0, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(dst.read("plugins/a.jar"), "a");
    assert!(!dst.0.join("eula.txt").exists() && !dst.0.join("other.txt").exists());
}

#[test]
fn other_zips_are_not_servers() {
    assert!(read_plan(&mut zip_of(&[("server.properties", "")])).is_err());
    assert!(read_plan(&mut zip_of(&[(MANIFEST, "not json")])).is_err());
    let mut newer = manifest();
    newer.format = FORMAT + 1;
    assert!(read_plan(&mut zip_of(&[(MANIFEST, &serde_json::to_string(&newer).unwrap())])).unwrap_err().to_string().contains("newer"));
}

fn original() -> ServerMeta {
    serde_json::from_value(serde_json::json!({
        "id": "smp-abc123", "name": "SMP", "type": "paper", "mcVersion": "1.21.4", "memoryMb": 2048, "port": 25565, "createdAt": 1, "lastStarted": 2,
        "javaPath": "C:/java/bin/java.exe", "jvmArgs": "-Dcom.mojang.eula.agree=true -XX:+UseZGC", "public": false,
        "joinRequests": { "enabled": true, "web": true, "discord": true }, "tunnel": "resin", "relayHost": "relay.example", "relaySecret": "s3cret",
        "domain": "play.example.com", "dnsAuto": true, "relayName": "smp", "relayAccount": "uuid",
        "domainTxt": { "name": "_rscraft.play.example.com", "value": "rscraft-verify=abc" }
    }))
    .unwrap()
}

#[test]
fn a_clone_has_nothing_of_the_originals_identity() {
    let m = clone_meta(original(), "copy-1", "SMP copy", 25566);
    assert_eq!((m.id.as_str(), m.name.as_str(), m.port, m.last_started), ("copy-1", "SMP copy", 25566, None));
    assert_eq!((m.relay_name, m.relay_account, m.relay_host, m.relay_secret, m.domain), (None, None, None, None, None));
    assert!(m.domain_txt.is_none() && !m.dns_auto && !m.join_requests.enabled);
    assert_eq!(m.tunnel, TunnelMode::Auto);
    assert_eq!(m.jvm_args.as_deref(), Some("-XX:+UseZGC"), "not the flag that agrees for the player");
    assert_eq!((m.memory_mb, m.java_path.as_deref(), m.public), (2048, Some("C:/java/bin/java.exe"), false), "its own settings stay");
}

#[test]
fn a_manifest_says_nothing_a_stranger_shouldnt_read_or_set() {
    let text = serde_json::to_string(&manifest_of(&original())).unwrap();
    for word in ["s3cret", "relay", "domain", "java", "jvm", "play.example.com", "smp-abc123", "uuid"] {
        assert!(!text.contains(word), "{word} in {text}");
    }
    let m = imported_meta(serde_json::from_str(&text).unwrap(), "new-1", "SMP", 25570);
    assert!(m.java_path.is_none() && m.jvm_args.is_none() && !m.pack_flags && m.relay_name.is_none());
    assert_eq!((m.id.as_str(), m.port, m.kind), ("new-1", 25570, ServerKind::Paper));
}

#[test]
fn a_manifests_pending_downloads_stay_in_the_folder() {
    let pending = |file: &str, folder: &str| PendingDownload { title: "t".into(), file_name: file.into(), url: "u".into(), folder: folder.into(), size: None, sha1: None, project_id: None, version_id: None };
    let mut m = manifest();
    m.pending_downloads = vec![pending("a.jar", "mods"), pending("../b.jar", "mods"), pending("c.jar", "../.."), pending("d.jar", "C:/x")];
    let got = imported_meta(m, "id", "n", 1).pending_downloads;
    assert_eq!(got.iter().map(|p| p.file_name.as_str()).collect::<Vec<_>>(), ["a.jar"]);
}

#[test]
fn names_of_copies() {
    assert_eq!(copy_name("SMP", &[]), "SMP copy");
    assert_eq!(copy_name("SMP", &["SMP copy".into(), "SMP copy 2".into()]), "SMP copy 3");
}

#[test]
fn links_stay_out() {
    let (s, outside) = (Scratch::new(), Scratch::new());
    outside.file("private.txt", "private");
    s.file("server.properties", "");
    #[cfg(windows)]
    let made = std::process::Command::new("cmd").args(["/c", "mklink", "/J"]).arg(s.0.join("link")).arg(&outside.0).output().is_ok_and(|o| o.status.success());
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(&outside.0, s.0.join("link")).is_ok();
    assert!(made);
    let listing = list_files(&s.0, &export_rules(&s.0));
    assert_eq!(names(&listing).into_iter().collect::<Vec<_>>(), ["server.properties"]);
    assert_eq!(listing.skipped.iter().map(|k| k.path.as_str()).collect::<Vec<_>>(), ["link"]);
    let _ = std::fs::remove_dir(s.0.join("link"));
}

#[test]
fn ports_of_a_copy_dont_collide_with_the_originals() {
    let s = Scratch::new();
    s.file("server.properties", "server-port=25565\nquery.port=25565\nenable-rcon=true\nrcon.port=25575\nmotd=hi\n");
    tokio::runtime::Runtime::new().unwrap().block_on(write_ports(&s.0, ServerKind::Paper, 25570)).unwrap();
    let text = s.read("server.properties");
    for line in ["server-port=25570", "query.port=25570", "rcon.port=25580", "motd=hi"] {
        assert!(text.contains(line), "{line} in {text}");
    }
}

#[test]
fn args_files_follow_the_system() {
    let s = Scratch::new();
    let lib = "libraries/net/neoforged/neoforge/21.1.77";
    s.file(&format!("{lib}/win_args.txt"), "").file(&format!("{lib}/unix_args.txt"), "");
    let other = if cfg!(windows) { "unix_args.txt" } else { "win_args.txt" };
    let mine = if cfg!(windows) { "win_args.txt" } else { "unix_args.txt" };
    let got = native_launch(&s.0, ServerLaunch { args_file: Some(format!("{lib}/{other}")), ..Default::default() });
    assert_eq!(got.args_file, Some(format!("{lib}/{mine}")));
}

/// The real thing, on a throwaway data folder: makes a Velocity server, runs it, clones it, exports it, imports the zip,
/// runs both, and checks each stops. No EULA is involved (Velocity has none) and every Java process is the runtime's own.
///   RC_TRANSFER_DATA=<empty folder> cargo test -j 2 --no-default-features --features panel --lib real_velocity -- --ignored --nocapture
#[test]
#[ignore]
fn real_velocity_round_trip() {
    let data = PathBuf::from(std::env::var("RC_TRANSFER_DATA").expect("a scratch folder"));
    std::fs::create_dir_all(&data).unwrap();
    assert_eq!(std::fs::read_dir(&data).unwrap().count(), 0, "an empty folder, so no real server is near this");
    crate::core::ctx::init(data.clone(), None);
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().thread_stack_size(64 << 20).build().unwrap();
    runtime.block_on(async move {
        let call = |c: &'static str, a: Vec<serde_json::Value>| async move { crate::server::handle(c, crate::core::Args::new(a)).await };
        let id_of = |v: &serde_json::Value| v["server"]["id"].as_str().unwrap().to_string();
        let run = |id: String| async move {
            let v = serde_json::json!(id);
            crate::server::handle("server:start", crate::core::Args::new(vec![v.clone()])).await.unwrap();
            for _ in 0..240 {
                if runtime::state(&id) == "running" {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            assert_eq!(runtime::state(&id), "running", "{:?}", runtime::log(&id).iter().rev().take(20).collect::<Vec<_>>());
            let port = store::get(&id).await.unwrap().port;
            assert!(tokio::net::TcpStream::connect(("127.0.0.1", port)).await.is_ok(), "listening on {port}");
            println!("{id} is up on port {port}");
            port
        };
        let stop = |id: String| async move {
            let v = serde_json::json!(id);
            crate::server::handle("server:stop", crate::core::Args::new(vec![v])).await.unwrap();
            for _ in 0..120 {
                if !runtime::is_running(&id) {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            panic!("{id} didn't stop");
        };

        let versions = call("server:versions", vec![serde_json::json!("velocity")]).await.unwrap();
        println!("velocity versions: {}", versions.to_string().chars().take(200).collect::<String>());
        let mc = versions[0]["id"].as_str().or(versions[0].as_str()).unwrap_or("3.4.0").to_string();
        let new = serde_json::json!({ "name": "Scratch proxy", "type": "velocity", "mcVersion": mc });
        let made = call("server:create", vec![new]).await.unwrap();
        let original = id_of(&made);
        let port = run(original.clone()).await;

        // Running: a clone is refused.
        let refused = call("server:clone", vec![serde_json::json!(original)]).await.unwrap_err();
        assert!(refused.to_string().contains("Stop the server"), "{refused}");
        stop(original.clone()).await;

        let dir = store::dir(&original).unwrap();
        let secret = std::fs::read_to_string(dir.join("forwarding.secret")).unwrap();
        let mut m = store::get(&original).await.unwrap();
        (m.relay_name, m.relay_secret, m.domain) = (Some("scratch".into()), Some("relay-s3cret".into()), Some("scratch.example.com".into()));
        store::save(&m).await.unwrap();
        std::fs::write(dir.join("schedule.json"), r#"{"tasks":[{"id":"a","enabled":true,"action":{"kind":"restart"},"timing":{"kind":"hours","every":6},"anchor":1,"lastRun":{"at":5,"outcome":"ok"},"made":["x.zip"]}]}"#).unwrap();

        let cloned = call("server:clone", vec![serde_json::json!(original), serde_json::json!({ "worlds": true })]).await.unwrap();
        let copy = id_of(&cloned);
        println!("clone: {cloned}");
        assert_ne!(copy, original);
        assert_eq!(cloned["server"]["name"], "Scratch proxy copy");
        assert_ne!(cloned["server"]["port"].as_u64().unwrap() as u16, port);
        let cm = store::get(&copy).await.unwrap();
        assert!(cm.relay_name.is_none() && cm.relay_secret.is_none() && cm.domain.is_none());
        let sched = std::fs::read_to_string(store::dir(&copy).unwrap().join("schedule.json")).unwrap();
        assert!(sched.contains("\"enabled\": false") && !sched.contains("x.zip") && !sched.contains("\"outcome\""), "{sched}");

        let zip_path = data.join("out.zip");
        let exported = call("server:exportZip", vec![serde_json::json!(original), serde_json::json!({ "path": zip_path })]).await.unwrap();
        println!("export: {exported}");
        let mut zip = open_zip(&zip_path).unwrap();
        let names: Vec<String> = zip.file_names().map(str::to_string).collect();
        assert!(names.iter().any(|n| n == MANIFEST) && names.iter().any(|n| n == "velocity.jar" || n.ends_with(".jar")));
        for bad in ["server.json", "eula.txt", "schedule.json", "forwarding.secret"] {
            assert!(!names.iter().any(|n| n == bad), "{bad} in the zip");
        }
        let mut all = Vec::new();
        for i in 0..zip.len() {
            zip.by_index(i).unwrap().read_to_end(&mut all).unwrap();
        }
        for needle in [secret.trim().as_bytes(), b"relay-s3cret", b"scratch.example.com"] {
            assert!(!all.windows(needle.len()).any(|w| w == needle), "{} in the zip", String::from_utf8_lossy(needle));
        }

        let imported = call("server:importZip", vec![serde_json::json!(zip_path)]).await.unwrap();
        let back = id_of(&imported);
        println!("import: {imported}");
        assert!(!store::dir(&back).unwrap().join("eula.txt").exists());

        // Both new ones at once, on ports of their own, with the original's secret file gone.
        let (a, b) = (run(copy.clone()).await, run(back.clone()).await);
        assert!(a != b && a != port && b != port, "{a} {b} {port}");
        assert!(store::dir(&back).unwrap().join("forwarding.secret").is_file(), "Velocity makes its own");
        assert_ne!(std::fs::read_to_string(store::dir(&back).unwrap().join("forwarding.secret")).unwrap(), secret);
        stop(copy).await;
        stop(back).await;
    });
}
