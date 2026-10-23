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

    let web = || Via::Web { relay_id: 1 };
    check("closed while off", requests::submit("smp", "Steve", web()).await == Outcome::Closed);
    meta.join_requests.enabled = true;
    meta.join_requests.web = true;
    store::save(&meta).await.unwrap();
    check("web request received", requests::submit("smp", "Steve", web()).await == Outcome::Received);
    check("same name again", requests::submit("smp", "steve", web()).await == Outcome::AlreadyRequested);
    check("bad name", requests::submit("smp", "no spaces", web()).await == Outcome::Invalid);
    check("discord off for this server", requests::submit("smp", "Alex", discord()).await == Outcome::Closed);
    check("second web request", requests::submit("smp", "Notch", web()).await == Outcome::Received);

    let waiting = requests::list("smp").await;
    check("two waiting", waiting.iter().map(|r| r.username.as_str()).collect::<Vec<_>>() == ["Steve", "Notch"]);
    let info = requests::accept("smp", &waiting[0].id).await.unwrap();
    check("accept leaves one", info.requests.len() == 1);
    let whitelist = std::fs::read_to_string(paths::servers().join("smp").join("whitelist.json")).unwrap_or_default();
    println!("     whitelist.json: {}", whitelist.trim());
    check("Steve on whitelist.json", whitelist.contains("\"Steve\""));
    check("listed now", requests::submit("smp", "Steve", web()).await == Outcome::AlreadyListed);
    let info = requests::reject("smp", &waiting[1].id).await.unwrap();
    check("reject empties the list", info.requests.is_empty());
    check("Notch not whitelisted", !std::fs::read_to_string(paths::servers().join("smp").join("whitelist.json")).unwrap_or_default().contains("Notch"));
    check("answering twice fails", requests::reject("smp", &waiting[1].id).await.is_err());

    // Banned players never reach the host; Reject and block stops a Discord user asking again.
    std::fs::write(paths::servers().join("smp").join("banned-players.json"), r#"[{"name":"Herobrine","uuid":"f84c6a79-0a4e-45e0-879b-cd49ebd4c4e2"}]"#).unwrap();
    check("banned name refused", requests::submit("smp", "herobrine", web()).await == Outcome::Banned);
    meta.join_requests.discord = true;
    store::save(&meta).await.unwrap();
    check("discord request received", requests::submit("smp", "Alex", discord()).await == Outcome::Received);
    let alex = requests::list("smp").await.into_iter().find(|r| r.username == "Alex").unwrap();
    let info = requests::reject_and_block("smp", &alex.id).await.unwrap();
    check("blocked one Discord user", info.blocked_on_discord == 1 && info.requests.is_empty());
    check("blocked user can't ask", requests::submit("smp", "Alex2", discord()).await == Outcome::Blocked);
    check("others still can", requests::submit("smp", "Steve2", Via::Discord { user_id: "2".into(), name: "sam".into() }).await == Outcome::Received);
    let info = requests::unblock_all("smp").await.unwrap();
    check("unblock all", info.blocked_on_discord == 0);
    check("unblocked user can ask again", requests::submit("smp", "Alex2", discord()).await == Outcome::Received);

    let _ = std::fs::remove_dir_all(&data);
    println!("{}", if ok { "PASS" } else { "FAIL" });
    std::process::exit(if ok { 0 } else { 1 });
}
