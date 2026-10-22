//! Starts a server's Discord bot in a fresh data folder and prints its state as it changes:
//!   cargo run --example check_discordbot -- [bot token]
//! Without a token it uses a made-up one, which Discord must refuse (the bot ends up in "error").

use std::time::Duration;

use resincraft_lib::core::types::ServerMeta;
use resincraft_lib::core::{ctx, paths};
use resincraft_lib::server::{discordbot, requests, store};
use serde_json::json;

#[tokio::main]
async fn main() {
    let token = std::env::args().nth(1).unwrap_or_else(|| "MTIzNDU2Nzg5MDEyMzQ1Njc4.made-up.token-for-testing".into());
    let data = std::env::temp_dir().join(format!("resin-bot-{}", std::process::id()));
    ctx::init(data.clone(), None);
    let meta: ServerMeta = serde_json::from_value(json!({
        "id": "smp", "name": "Test SMP", "type": "paper", "mcVersion": "1.21.1", "memoryMb": 2048, "port": 25599,
        "createdAt": 0, "launch": { "jar": "server.jar" }, "public": false, "optimizedFlags": true, "tunnel": "auto",
        "dnsAuto": false, "joinRequests": { "enabled": true, "web": false, "discord": true }
    }))
    .unwrap();
    std::fs::create_dir_all(paths::servers().join("smp")).unwrap();
    store::save(&meta).await.unwrap();
    requests::set_bot_token("smp", Some(token)).await.unwrap();

    let mut last = None;
    for _ in 0..40 {
        let s = discordbot::status("smp");
        if last.as_ref() != Some(&s) {
            println!("{} {}{}", s.state, s.name.clone().unwrap_or_default(), s.message.clone().map(|m| format!(" ({m})")).unwrap_or_default());
            if let Some(url) = &s.invite_url {
                println!("  invite: {url}");
            }
            last = Some(s.clone());
        }
        if s.state == "error" || (s.state == "online" && std::env::args().nth(1).is_none()) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    let _ = std::fs::remove_dir_all(&data);
}
