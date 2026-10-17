//! Headless check of the `player` group: NBT, worlds, servers, Server List Ping, screenshots, notes,
//! accounts and token sealing. Everything it creates is removed again; the real accounts.json is never touched.
//!
//! cargo run --example check_player

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use resincraft_lib::core::types::{LoaderType, NewInstance, ServerEntry};
use resincraft_lib::core::{ctx, instances, paths, settings};
use resincraft_lib::player::nbt::{self, Compound, NbtFile, Tag};
use resincraft_lib::player::{accounts, notes, screenshots, servers, worlds};

static FAILS: AtomicUsize = AtomicUsize::new(0);

fn check(name: &str, ok: bool, detail: impl std::fmt::Display) {
    if ok {
        println!("PASS {name}");
    } else {
        FAILS.fetch_add(1, Ordering::SeqCst);
        println!("FAIL {name}: {detail}");
    }
}

fn check_res<T>(name: &str, r: &anyhow::Result<T>) -> bool {
    match r {
        Ok(_) => {
            println!("PASS {name}");
            true
        }
        Err(e) => {
            FAILS.fetch_add(1, Ordering::SeqCst);
            println!("FAIL {name}: {e:#}");
            false
        }
    }
}

fn check_err<T>(name: &str, r: anyhow::Result<T>, contains: &str) {
    match r {
        Ok(_) => check(name, false, "expected an error"),
        Err(e) => check(name, e.to_string().contains(contains), format!("unexpected error: {e}")),
    }
}

fn gunzip_if_needed(data: &[u8]) -> Vec<u8> {
    use std::io::Read;
    if data.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        flate2::read::MultiGzDecoder::new(data).read_to_end(&mut out).unwrap();
        out
    } else {
        data.to_vec()
    }
}

fn synthetic_level(name: &str) -> NbtFile {
    let mut data = Compound::new();
    data.set("LevelName", Tag::String(name.into()));
    data.set("GameType", Tag::Int(1));
    data.set("hardcore", Tag::Byte(0));
    data.set("allowCommands", Tag::Byte(1));
    data.set("LastPlayed", Tag::Long(1_700_000_000_000));
    data.set("RandomSeed", Tag::Long(-4_242_424_242_424_242));
    let mut version = Compound::new();
    version.set("Name", Tag::String("1.21.1".into()));
    data.set("Version", Tag::Compound(version));
    data.set("Short", Tag::Short(-12));
    data.set("Float", Tag::Float(1.5));
    data.set("Double", Tag::Double(-2.25));
    data.set("Bytes", Tag::ByteArray(vec![-1, 0, 1, 127]));
    data.set("Ints", Tag::IntArray(vec![i32::MIN, 0, i32::MAX]));
    data.set("Longs", Tag::LongArray(vec![i64::MIN, 7]));
    data.set("EmptyList", Tag::list(nbt::STRING, vec![]));
    data.set("Strings", Tag::list(nbt::STRING, vec![Tag::String("Grüße ✓ 🎮".into()), Tag::String("nul\0char".into())]));
    let mut root = Compound::new();
    root.set("Data", Tag::Compound(data));
    NbtFile { name: String::new(), root, compression: nbt::Compression::Gzip }
}

fn find_real_level_dat(data_dir: &Path) -> Option<PathBuf> {
    for inst in std::fs::read_dir(data_dir.join("instances")).ok()?.flatten() {
        let saves = inst.path().join("minecraft").join("saves");
        for world in std::fs::read_dir(saves).into_iter().flatten().flatten() {
            let p = world.path().join("level.dat");
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

async fn test_nbt(data_dir: &Path) -> Vec<u8> {
    // Synthetic file with every tag type.
    let syn = synthetic_level("Synthetic");
    let bytes = nbt::write(&syn).unwrap();
    let back = nbt::parse(&bytes);
    check("nbt: synthetic round-trip", back.as_ref().is_ok_and(|b| *b == syn), "parsed value differs");
    for comp in [nbt::Compression::None, nbt::Compression::Zlib] {
        let f = NbtFile { compression: comp, ..syn.clone() };
        let ok = nbt::parse(&nbt::write(&f).unwrap()).is_ok_and(|b| b == f);
        check(&format!("nbt: {comp:?} round-trip"), ok, "differs");
    }
    check_err("nbt: truncated data rejected", nbt::parse(&nbt::write(&NbtFile { compression: nbt::Compression::None, ..syn.clone() }).unwrap()[..40]), "ends unexpectedly");

    match find_real_level_dat(data_dir) {
        Some(p) => {
            let original = std::fs::read(&p).unwrap();
            match nbt::parse(&original) {
                Ok(level) => {
                    let rewritten = nbt::write(&NbtFile { compression: nbt::Compression::None, ..level.clone() }).unwrap();
                    check(
                        &format!("nbt: real level.dat byte-identical after rewrite ({})", p.parent().unwrap().file_name().unwrap().to_string_lossy()),
                        rewritten == gunzip_if_needed(&original),
                        "bytes differ",
                    );
                    let again = nbt::parse(&nbt::write(&level).unwrap());
                    check("nbt: real level.dat re-parse equal", again.is_ok_and(|a| a == level), "differs");
                    original
                }
                Err(e) => {
                    check("nbt: parse real level.dat", false, e);
                    nbt::write(&syn).unwrap()
                }
            }
        }
        None => {
            println!("INFO no real world found; using a synthetic level.dat");
            nbt::write(&syn).unwrap()
        }
    }
}

async fn test_worlds(id: &str, level_dat: &[u8]) {
    let saves = paths::game_dir(id).join("saves");
    std::fs::create_dir_all(saves.join("TestWorld")).unwrap();
    std::fs::write(saves.join("TestWorld").join("level.dat"), level_dat).unwrap();
    std::fs::write(saves.join("TestWorld").join("session.lock"), b"x").unwrap();
    std::fs::create_dir_all(saves.join("Broken")).unwrap();

    let list = worlds::list_worlds(id).await;
    if check_res("worlds: list", &list) {
        let list = list.unwrap();
        check("worlds: two worlds listed", list.len() == 2, format!("{} worlds", list.len()));
        let broken = list.iter().find(|w| w.folder == "Broken");
        check("worlds: missing level.dat reported", broken.is_some_and(|w| w.error.as_deref() == Some("No level.dat found")), format!("{broken:?}"));
        let tw = list.iter().find(|w| w.folder == "TestWorld");
        check("worlds: TestWorld readable", tw.is_some_and(|w| w.error.is_none() && w.size > 0), format!("{tw:?}"));
        if let Some(w) = tw {
            println!("INFO TestWorld: name={:?} mode={:?} version={:?} seed={:?} lastPlayed={:?}", w.name, w.game_mode, w.version, w.seed, w.last_played);
        }
    }

    let r = worlds::rename_world(id, "TestWorld", "  Renamed ✓ World  ").await;
    if check_res("worlds: rename", &r) {
        let list = worlds::list_worlds(id).await.unwrap();
        let tw = list.iter().find(|w| w.folder == "TestWorld");
        check("worlds: rename reads back", tw.is_some_and(|w| w.name == "Renamed ✓ World"), format!("{tw:?}"));
        check("worlds: level.dat_old written", saves.join("TestWorld").join("level.dat_old").is_file(), "missing");
    }
    check_err("worlds: empty name rejected", worlds::rename_world(id, "TestWorld", "   ").await, "Enter a name");
    check_err("worlds: '..' folder rejected", worlds::rename_world(id, "..", "x").await, "Invalid name");
    check_err("worlds: nested path rejected", worlds::world_path(id, "a/../../x").await, "Invalid name");
    check_err("worlds: bad instance id rejected", worlds::list_worlds("../..").await, "Invalid instance id");

    let backup = worlds::backup_world(id, "TestWorld").await;
    if check_res("worlds: backup", &backup) {
        let zip_path = PathBuf::from(backup.unwrap());
        let names: Vec<String> = {
            let z = zip::ZipArchive::new(std::fs::File::open(&zip_path).unwrap()).unwrap();
            z.file_names().map(String::from).collect()
        };
        check(
            "worlds: backup has level.dat, no session.lock",
            names.iter().any(|n| n == "TestWorld/level.dat") && !names.iter().any(|n| n.ends_with("session.lock")),
            format!("{names:?}"),
        );
        let imported = worlds::import_world(id, &zip_path).await;
        if check_res("worlds: import from zip", &imported) {
            let w = imported.unwrap();
            check("worlds: imported as a new folder", w.folder == "TestWorld (2)" && w.name == "Renamed ✓ World" && w.error.is_none(), format!("{w:?}"));
        }
    }
    check_err("worlds: import needs .zip", worlds::import_world(id, Path::new("C:/nope/world.rar")).await, "Choose a .zip file");
}

async fn test_servers(id: &str) {
    let list = vec![
        ServerEntry { name: "Hypixel".into(), ip: " mc.hypixel.net ".into(), icon: Some("iVBORw0KGgo=".into()), accept_textures: Some(true), hidden: None },
        ServerEntry { name: "Local ✓".into(), ip: "localhost:25566".into(), icon: None, accept_textures: None, hidden: Some(true) },
    ];
    check_res("servers: save (new file)", &servers::save_servers(id, &list).await);
    check_res("servers: save (existing file)", &servers::save_servers(id, &list).await);
    let back = servers::list_servers(id).await;
    if check_res("servers: list", &back) {
        let back = back.unwrap();
        let ok = back.len() == 2
            && back[0].name == "Hypixel"
            && back[0].ip == "mc.hypixel.net"
            && back[0].icon.as_deref() == Some("iVBORw0KGgo=")
            && back[0].accept_textures == Some(true)
            && back[0].hidden.is_none()
            && back[1].name == "Local ✓"
            && back[1].hidden == Some(true)
            && back[1].accept_textures.is_none();
        check("servers: round-trip", ok, format!("{back:?}"));
    }
    let file = paths::game_dir(id).join("servers.dat");
    check("servers: servers.dat_old backup", paths::game_dir(id).join("servers.dat_old").is_file(), "missing");
    let raw = nbt::parse(&std::fs::read(&file).unwrap()).unwrap();
    check("servers: uncompressed NBT like the game writes", raw.compression == nbt::Compression::None, format!("{:?}", raw.compression));
    let bad = vec![ServerEntry { name: "x".into(), ip: "a".into(), icon: Some("not base64!".into()), accept_textures: None, hidden: None }];
    check_err("servers: invalid icon rejected", servers::save_servers(id, &bad).await, "Invalid server icon");
    let empty = vec![ServerEntry { name: "x".into(), ip: "  ".into(), icon: None, accept_textures: None, hidden: None }];
    check_err("servers: empty address rejected", servers::save_servers(id, &empty).await, "needs an address");
    check_err("servers: bad port rejected", servers::ping_server("example.com:70000", Duration::from_secs(1)).await, "Invalid port");
    check_err("servers: whitespace host rejected", servers::ping_server("bad host", Duration::from_secs(1)).await, "Invalid server address");

    match servers::ping_server("mc.hypixel.net", Duration::from_secs(5)).await {
        Ok(s) => check(
            "servers: ping mc.hypixel.net",
            !s.version.is_empty() && s.players.max > 0,
            format!("version={:?} players={:?}", s.version, s.players),
        ),
        Err(e) => println!("WARN servers: ping mc.hypixel.net failed (network?): {e}"),
    }
    match servers::ping_server("127.0.0.1:1", Duration::from_secs(3)).await {
        Ok(_) => check("servers: closed port fails", false, "unexpected success"),
        Err(e) => check("servers: closed port fails", true, e),
    }
}

async fn test_screenshots(id: &str) {
    let dir = paths::game_dir(id).join("screenshots");
    std::fs::create_dir_all(&dir).unwrap();
    let img = image::RgbaImage::from_fn(1000, 600, |x, y| image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255]));
    img.save(dir.join("2026-01-01_12.00.00.png")).unwrap();
    std::fs::write(dir.join("notes.txt"), b"not an image").unwrap();

    let list = screenshots::list_screenshots(id).await;
    if check_res("screenshots: list", &list) {
        let list = list.unwrap();
        check("screenshots: only images listed", list.len() == 1 && list[0].name == "2026-01-01_12.00.00.png" && list[0].size > 0, format!("{list:?}"));
    }
    let thumb = screenshots::screenshot_thumbnail(id, "2026-01-01_12.00.00.png").await;
    if check_res("screenshots: thumbnail", &thumb) {
        use base64::Engine;
        let url = thumb.unwrap();
        let ok = url.strip_prefix("data:image/jpeg;base64,").and_then(|b| base64::engine::general_purpose::STANDARD.decode(b).ok()).and_then(|b| image::load_from_memory(&b).ok());
        check("screenshots: thumbnail is a 384x230 JPEG", ok.as_ref().is_some_and(|i| i.width() == 384 && i.height() == 230), format!("{:?}", ok.map(|i| (i.width(), i.height()))));
        let again = screenshots::screenshot_thumbnail(id, "2026-01-01_12.00.00.png").await;
        check("screenshots: cached thumbnail identical", again.is_ok_and(|a| a == url), "differs");
    }
    let full = screenshots::screenshot_image(id, "2026-01-01_12.00.00.png").await;
    check("screenshots: full image PNG data URL", full.as_ref().is_ok_and(|u| u.starts_with("data:image/png;base64,iVBOR")), "wrong prefix");
    check_err("screenshots: non-image rejected", screenshots::screenshot_path(id, "notes.txt").await, "Not a screenshot");
    check_err("screenshots: traversal rejected", screenshots::screenshot_path(id, "../options.png").await, "Invalid name");
    let png = screenshots::screenshot_png(id, "2026-01-01_12.00.00.png").await;
    check("screenshots: PNG bytes for clipboard", png.is_ok_and(|b| b.starts_with(b"\x89PNG")), "not a PNG");
    image::DynamicImage::ImageRgba8(img).to_rgb8().save(dir.join("shot.jpg")).unwrap();
    let png = screenshots::screenshot_png(id, "shot.jpg").await;
    check("screenshots: JPEG converted to PNG for clipboard", png.is_ok_and(|b| b.starts_with(b"\x89PNG")), "not a PNG");
}

async fn test_notes(id: &str) {
    let text = "# Notes ✓\n\nRemember the diamonds at -120, 12, 340.\n";
    check_res("notes: set", &notes::set_notes(id, text).await);
    check("notes: get round-trip", notes::get_notes(id).await.is_ok_and(|t| t == text), "differs");
    check("notes: stored as notes.md", paths::instance_dir(id).join("notes.md").is_file(), "missing");
    check_res("notes: clear", &notes::set_notes(id, "  \n ").await);
    check("notes: blank removes the file", !paths::instance_dir(id).join("notes.md").exists(), "still there");
    check("notes: get after clear is empty", notes::get_notes(id).await.is_ok_and(|t| t.is_empty()), "not empty");
}

/// Runs in a child process whose data dir is a temporary folder (ctx can only be initialised once).
async fn accounts_child(dir: PathBuf) {
    ctx::init(dir.clone(), None);
    let events: Arc<Mutex<Vec<String>>> = Arc::default();
    let ev = events.clone();
    resincraft_lib::core::events::set_sink(move |ch, v| ev.lock().unwrap().push(format!("{ch}: {v}")));

    // Sealing.
    let sealed = accounts::seal("secret-token ✓");
    #[cfg(windows)]
    check("accounts: DPAPI seal format", sealed.starts_with("dpapi:"), &sealed);
    check("accounts: seal/unseal round-trip", accounts::unseal(Some(&sealed)).is_ok_and(|s| s == "secret-token ✓"), "differs");
    check("accounts: raw: tokens readable", accounts::unseal(Some("raw:aGVsbG8=")).is_ok_and(|s| s == "hello"), "differs");
    check("accounts: enc: tokens unreadable", accounts::unseal(Some("enc:AAAA")).is_err(), "decoded?");
    check("accounts: offline UUID matches Java", accounts::offline_uuid("Notch") == "b50ad385-829d-3141-a216-7e7d7539ba7f", accounts::offline_uuid("Notch"));

    check_err("accounts: no account → sign in first", accounts::get_launch_auth().await, "Sign in with your Microsoft account");

    // An account written by the Electron build (safeStorage tokens), with an extra field to keep.
    let legacy = serde_json::json!([{
        "id": "legacy-1", "type": "msa", "username": "OldMsaUser", "uuid": "0123456789abcdef0123456789abcdef",
        "active": true, "refreshToken": "enc:djEwAAAA", "mcToken": "enc:djEwBBBB", "mcTokenExpires": 4102444800000i64,
        "xuid": "2535400000000000", "futureField": {"keep": true}
    }]);
    std::fs::write(paths::accounts_file(), serde_json::to_vec_pretty(&legacy).unwrap()).unwrap();
    let list = accounts::list_accounts().await.unwrap();
    check("accounts: Electron-era account still listed", list.len() == 1 && list[0].username == "OldMsaUser" && list[0].active, format!("{list:?}"));
    check_err("accounts: Electron-era tokens → sign in again", accounts::get_launch_auth().await, "Please sign in to OldMsaUser again");

    check_err("accounts: bad offline name rejected", accounts::add_offline_account("no spaces!").await, "Usernames are 3–16");
    let off = accounts::add_offline_account("  Steve_Test ").await;
    if check_res("accounts: add offline (dev build)", &off) {
        let off = off.unwrap();
        check("accounts: offline view", off.kind == "offline" && off.username == "Steve_Test" && off.active && off.uuid == accounts::offline_uuid("Steve_Test"), format!("{off:?}"));
        let auth = accounts::get_launch_auth().await;
        check(
            "accounts: offline launch auth",
            auth.as_ref().is_ok_and(|a| a.user_type == "legacy" && a.access_token == "0" && a.username == "Steve_Test" && a.xuid.is_none()),
            format!("{auth:?}"),
        );
        // Adding the same name again replaces it rather than duplicating.
        accounts::add_offline_account("Steve_Test").await.unwrap();
        let list = accounts::list_accounts().await.unwrap();
        check("accounts: upsert de-duplicates", list.len() == 2 && list[0].username == "Steve_Test" && !list[1].active, format!("{list:?}"));
        accounts::set_active_account("legacy-1").await.unwrap();
        let list = accounts::list_accounts().await.unwrap();
        check("accounts: set active", list.iter().filter(|a| a.active).count() == 1 && list.iter().any(|a| a.id == "legacy-1" && a.active), format!("{list:?}"));
        accounts::remove_account("legacy-1").await.unwrap();
        let list = accounts::list_accounts().await.unwrap();
        check("accounts: remove re-activates another", list.len() == 1 && list[0].active, format!("{list:?}"));
    }
    // File format: same keys as Electron; unknown fields survive a rewrite.
    accounts::add_offline_account("Alex_Test").await.unwrap();
    std::fs::write(paths::accounts_file(), serde_json::to_vec_pretty(&legacy).unwrap()).unwrap();
    accounts::add_offline_account("Alex_Test").await.unwrap();
    let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(paths::accounts_file()).unwrap()).unwrap();
    let off = &raw[0];
    let old = &raw[1];
    check(
        "accounts: accounts.json shape",
        off["type"] == "offline" && off.get("skinUrl").is_none() && off.get("refreshToken").is_none() && off["active"] == true
            && old["mcTokenExpires"] == 4102444800000i64 && old["refreshToken"] == "enc:djEwAAAA" && old["futureField"]["keep"] == true && old["active"] == false,
        &raw,
    );

    // Microsoft sign-in without a Client ID gives the friendly message.
    settings::set(serde_json::json!({ "msClientId": "" })).await.unwrap();
    check_err("accounts: missing Client ID message", accounts::begin_microsoft_login().await, "needs an Azure Client ID");
    // With the real Client ID from .dev-data: request a device code, then cancel before polling.
    let real: Option<serde_json::Value> = std::env::var("CHECK_PLAYER_CLIENT_ID").ok().map(serde_json::Value::String);
    if let Some(cid) = real.filter(|c| c.as_str().is_some_and(|s| !s.is_empty())) {
        settings::set(serde_json::json!({ "msClientId": cid })).await.unwrap();
        match accounts::begin_microsoft_login().await {
            Ok(dc) => {
                check("accounts: device code issued", !dc.user_code.is_empty() && dc.verification_uri.starts_with("https://") && dc.expires_in > 0, format!("{dc:?}"));
                accounts::cancel_microsoft_login();
                tokio::time::sleep(Duration::from_secs(6)).await;
                let evs = events.lock().unwrap().clone();
                check("accounts: cancelled login emits nothing", !evs.iter().any(|e| e.starts_with("login")), format!("{evs:?}"));
            }
            Err(e) => println!("WARN accounts: device code request failed (network/client id?): {e}"),
        }
    } else {
        println!("INFO no msClientId in .dev-data/settings.json; skipping device-code request");
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--accounts-child") {
        accounts_child(PathBuf::from(&args[2])).await;
        std::process::exit(if FAILS.load(Ordering::SeqCst) == 0 { 0 } else { 1 });
    }

    let data_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(".dev-data");
    let data_dir = std::fs::canonicalize(&data_dir).unwrap_or(data_dir);
    ctx::init(data_dir.clone(), None);
    println!("INFO data dir: {}", data_dir.display());

    let level_dat = test_nbt(&data_dir).await;

    let inst = instances::create(
        NewInstance { name: "check-player temp".into(), mc_version: "1.21.1".into(), loader: LoaderType::Vanilla, loader_version: None, color: None, group: None },
        |_| {},
    )
    .await
    .expect("create temp instance");
    let id = inst.id.clone();
    println!("INFO temp instance: {id}");

    test_worlds(&id, &level_dat).await;
    test_servers(&id).await;
    test_screenshots(&id).await;
    test_notes(&id).await;

    let inst_dir = paths::instance_dir(&id);
    let removed = std::fs::remove_dir_all(&inst_dir);
    check("cleanup: temp instance removed", removed.is_ok() && !inst_dir.exists(), format!("{removed:?}"));

    // Accounts run in a child process against a throwaway data dir.
    let tmp = std::env::temp_dir().join(format!("resincraft-check-player-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let client_id = settings::get().await.ms_client_id;
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--accounts-child")
        .arg(&tmp)
        .env("CHECK_PLAYER_CLIENT_ID", client_id)
        .status()
        .expect("run accounts child");
    if !status.success() {
        FAILS.fetch_add(1, Ordering::SeqCst);
        println!("FAIL accounts: child process reported failures ({status})");
    }
    let removed = std::fs::remove_dir_all(&tmp);
    check("cleanup: temp accounts dir removed", removed.is_ok(), format!("{removed:?}"));

    let fails = FAILS.load(Ordering::SeqCst);
    println!("{}", if fails == 0 { "ALL PASSED".to_string() } else { format!("{fails} FAILED") });
    std::process::exit(if fails == 0 { 0 } else { 1 });
}
