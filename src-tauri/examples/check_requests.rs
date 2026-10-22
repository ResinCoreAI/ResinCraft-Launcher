//! Whitelist requests on a throwaway server in a fresh data folder, as the Players tab and the bot use them:
//!   cargo run --example check_requests
//! Turns requests on (web only), sends some, accepts one and rejects one, and checks whitelist.json.

use resincraft_lib::core::types::ServerMeta;
use resincraft_lib::core::{ctx, paths};
use resincraft_lib::server::requests::{self, Outcome, Via};
use resincraft_lib::server::store;
use serde_json::json;

#[tokio::main]
async fn main() {
    let data = std::env::temp_dir().join(format!("resin-requests-{}", std::process::id()));
    ctx::init(data.clone(), None);
    let mut meta: ServerMeta = serde_json::from_value(json!({
        "id": "smp", "name": "Test SMP", "type": "paper", "mcVersion": "1.21.1", "memoryMb": 2048, "port": 25599,
        "createdAt": 0, "launch": { "jar": "server.jar" }, "public": false, "optimizedFlags": true, "tunnel": "auto", "dnsAuto": false
    }))
    .unwrap();
    std::fs::create_dir_all(paths::servers().join("smp")).unwrap();
    store::save(&meta).await.unwrap();

    let mut ok = true;
    let mut check = |what: &str, good: bool| {
        println!("{} {what}", if good { "ok  " } else { "FAIL" });
        ok &= good;
    };
    let discord = || Via::Discord { user_id: "1".into(), name: "alex".into() };

    check("closed while off", requests::submit("smp", "Steve", Via::Web).await == Outcome::Closed);
    meta.join_requests.enabled = true;
    meta.join_requests.web = true;
    store::save(&meta).await.unwrap();
    check("web request received", requests::submit("smp", "Steve", Via::Web).await == Outcome::Received);
    check("same name again", requests::submit("smp", "steve", Via::Web).await == Outcome::AlreadyRequested);
    check("bad name", requests::submit("smp", "no spaces", Via::Web).await == Outcome::Invalid);
    check("discord off for this server", requests::submit("smp", "Alex", discord()).await == Outcome::Closed);
    check("second web request", requests::submit("smp", "Notch", Via::Web).await == Outcome::Received);

    let waiting = requests::list("smp").await;
    check("two waiting", waiting.iter().map(|r| r.username.as_str()).collect::<Vec<_>>() == ["Steve", "Notch"]);
    let info = requests::accept("smp", &waiting[0].id).await.unwrap();
    check("accept leaves one", info.requests.len() == 1);
    let whitelist = std::fs::read_to_string(paths::servers().join("smp").join("whitelist.json")).unwrap_or_default();
    println!("     whitelist.json: {}", whitelist.trim());
    check("Steve on whitelist.json", whitelist.contains("\"Steve\""));
    check("listed now", requests::submit("smp", "Steve", Via::Web).await == Outcome::AlreadyListed);
    let info = requests::reject("smp", &waiting[1].id).await.unwrap();
    check("reject empties the list", info.requests.is_empty());
    check("Notch not whitelisted", !std::fs::read_to_string(paths::servers().join("smp").join("whitelist.json")).unwrap_or_default().contains("Notch"));
    check("answering twice fails", requests::reject("smp", &waiting[1].id).await.is_err());

    let _ = std::fs::remove_dir_all(&data);
    println!("{}", if ok { "PASS" } else { "FAIL" });
    std::process::exit(if ok { 0 } else { 1 });
}
