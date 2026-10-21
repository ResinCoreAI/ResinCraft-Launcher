//! Shows what ResinCraft sends to Discord for Rich Presence:
//!   cargo run --example discord_frames            prints the frames for a few sample games
//!   cargo run --example discord_frames -- --fake  also plays Discord on its IPC socket (only while Discord is
//!                                                 closed) and prints what the real client sends over it
//!   cargo run --example discord_frames -- --live  sends the samples to the running Discord, 10 s each
//!                                                 (needs RESINCRAFT_DISCORD_APP_ID, built in or in the environment)

use std::time::Duration;

use resincraft_lib::core::fsutil::now_ms;
use resincraft_lib::core::types::Instance;
use resincraft_lib::discord::{self, activity, Client, Options, Playing};
use resincraft_lib::game::launch::Target;
use serde_json::{json, Value};

fn samples() -> Vec<(&'static str, Option<Value>)> {
    let inst = |v: Value| -> Instance { serde_json::from_value(v).unwrap() };
    let fabric = inst(json!({ "id": "a", "name": "Survival with friends", "mcVersion": "1.21.1", "loader": "fabric", "loaderVersion": "0.16.5", "color": "#5b8", "createdAt": 0 }));
    let pack = inst(json!({
        "id": "b", "name": "ATM10 (2)", "mcVersion": "1.21.1", "loader": "neoforge", "loaderVersion": "21.1.77", "color": "#a6f", "createdAt": 0,
        "modpack": { "projectId": "925200", "name": "All the Mods 10" },
        "iconUrl": "https://media.forgecdn.net/avatars/thumbnails/1015/447/256/256/638551196442463380.png"
    }));
    let start = now_ms();
    let opts = Options { enabled: true, show_server: false, in_launcher: true };
    vec![
        ("In the launcher", activity(None, opts, start)),
        ("Fabric instance", activity(Some(&Playing::new("a", &fabric, None, start)), opts, start)),
        ("Modpack, straight into a world", activity(Some(&Playing::new("b", &pack, Some(&Target::World("New World".into())), start)), opts, start)),
        ("Server, address hidden (default)", activity(Some(&Playing::new("a", &fabric, Some(&Target::Server("play.example.org".into())), start)), opts, start)),
        (
            "Server, address shown",
            activity(Some(&Playing::new("a", &fabric, Some(&Target::Server("play.example.org:25570".into())), start)), Options { show_server: true, ..opts }, start),
        ),
        ("Cleared (game closed, launcher presence off)", None),
    ]
}

fn print_frame(label: &str, bytes: &[u8]) {
    let (op, v) = discord::read_frame(&mut &bytes[..]).unwrap();
    let head: Vec<String> = bytes[..8].iter().map(|b| format!("{b:02x}")).collect();
    println!("  {label}: op {op}, header [{}]\n  {}", head.join(" "), serde_json::to_string_pretty(&v).unwrap().replace('\n', "\n  "));
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let app_id = discord::app_id().unwrap_or_else(|| "<RESINCRAFT_DISCORD_APP_ID>".into());
    println!("== Frames ResinCraft writes (app id: {app_id}) ==\n");
    print_frame("handshake", &discord::handshake(&app_id));
    for (label, a) in samples() {
        println!();
        print_frame(&format!("SET_ACTIVITY — {label}"), &discord::set_activity(a.as_ref(), std::process::id(), "nonce"));
    }

    if args.iter().any(|a| a == "--fake") {
        fake(&app_id).await;
    }
    if args.iter().any(|a| a == "--live") {
        let Some(id) = discord::app_id() else {
            eprintln!("\n--live needs RESINCRAFT_DISCORD_APP_ID");
            std::process::exit(1);
        };
        println!("\n== Sending to Discord ==");
        tokio::task::spawn_blocking(move || {
            let mut c = Client::connect(&id).unwrap_or_else(|e| panic!("Couldn't reach Discord: {e}"));
            for (label, a) in samples() {
                println!("{label}: {}", c.set_activity(a.as_ref()).map(|v| v["evt"].to_string()).unwrap_or_else(|e| e.to_string()));
                std::thread::sleep(Duration::from_secs(10));
            }
            c.close();
        })
        .await
        .unwrap();
    }
}

/// Listens where Discord would and answers like it, so the real client's bytes can be seen without Discord.
async fn fake(app_id: &str) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    println!("\n== Fake Discord on the IPC socket ==");
    #[cfg(windows)]
    let server = {
        let name = r"\\.\pipe\discord-ipc-0";
        match tokio::net::windows::named_pipe::ServerOptions::new().first_pipe_instance(true).create(name) {
            Ok(s) => s,
            Err(e) => return eprintln!("Couldn't listen on {name} (is Discord running?): {e}"),
        }
    };
    #[cfg(unix)]
    let (listener, path) = {
        let dir = std::env::var_os("XDG_RUNTIME_DIR").or_else(|| std::env::var_os("TMPDIR")).map(std::path::PathBuf::from).unwrap_or("/tmp".into());
        let path = dir.join("discord-ipc-0");
        match tokio::net::UnixListener::bind(&path) {
            Ok(l) => (l, path),
            Err(e) => return eprintln!("Couldn't listen on {} (is Discord running?): {e}", path.display()),
        }
    };

    let id = app_id.to_string();
    let client = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        let mut c = Client::connect(&id)?;
        for (_, a) in samples().into_iter().take(3) {
            c.set_activity(a.as_ref())?;
        }
        c.set_activity(None)?;
        c.close();
        Ok(())
    });

    #[cfg(windows)]
    let mut pipe = {
        server.connect().await.unwrap();
        server
    };
    #[cfg(unix)]
    let mut pipe = listener.accept().await.unwrap().0;

    loop {
        let mut head = [0u8; 8];
        if pipe.read_exact(&mut head).await.is_err() {
            break;
        }
        let len = u32::from_le_bytes(head[4..].try_into().unwrap()) as usize;
        let mut body = vec![0u8; len];
        pipe.read_exact(&mut body).await.unwrap();
        let frame = [&head[..], &body[..]].concat();
        let (op, v) = discord::read_frame(&mut &frame[..]).unwrap();
        print_frame("received", &frame);
        let answer = match op {
            discord::OP_HANDSHAKE => json!({ "cmd": "DISPATCH", "evt": "READY", "data": { "v": 1 } }),
            discord::OP_FRAME => json!({ "cmd": v["cmd"], "evt": null, "data": v["args"]["activity"], "nonce": v["nonce"] }),
            _ => break,
        };
        let _ = pipe.write_all(&discord::encode(discord::OP_FRAME, &answer)).await;
    }
    match client.await.unwrap() {
        Ok(()) => println!("\nThe client connected, sent 3 activities and a clear, and closed: OK"),
        Err(e) => println!("\nClient error: {e}"),
    }
    #[cfg(unix)]
    let _ = std::fs::remove_file(path);
}
