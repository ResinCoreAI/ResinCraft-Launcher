//! The host's own Discord bot for whitelist requests (requests.rs). While ResinCraft runs, each bot token in use
//! keeps one connection to Discord's gateway and offers `/whitelist <username>`; a server picker is added when one
//! bot serves several servers. Players get a private answer, and a direct message when the host accepts or rejects.
//! The bot needs no privileged intents and no permissions: slash commands arrive without either.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use anyhow::{anyhow, Result};
use futures::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async_tls_with_config, Connector};

use crate::core::ctx::http;
use crate::core::events::emit;

use super::requests::{self, Outcome, Via};
use super::store;

const API: &str = "https://discord.com/api/v10";
const GATEWAY: &str = "wss://gateway.discord.gg/?v=10&encoding=json";

/// A server's bot, as the Players tab shows it.
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BotStatus {
    /// "off" | "connecting" | "online" | "error"
    pub state: &'static str,
    /// The bot's Discord name once connected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Adds the bot to a Discord server (with its slash commands).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invite_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

struct Running {
    servers: Vec<(String, String)>,
    stop: watch::Sender<bool>,
}

/// Running bots by token digest.
fn bots() -> &'static Mutex<HashMap<String, Running>> {
    static B: OnceLock<Mutex<HashMap<String, Running>>> = OnceLock::new();
    B.get_or_init(Default::default)
}

fn statuses() -> &'static Mutex<HashMap<String, BotStatus>> {
    static S: OnceLock<Mutex<HashMap<String, BotStatus>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

pub fn status(id: &str) -> BotStatus {
    statuses().lock().unwrap().get(id).cloned().unwrap_or(BotStatus { state: "off", ..Default::default() })
}

fn set_status(servers: &[(String, String)], s: BotStatus) {
    {
        let mut all = statuses().lock().unwrap();
        for (id, _) in servers {
            all.insert(id.clone(), s.clone());
        }
    }
    for (id, _) in servers {
        let id = id.clone();
        tokio::spawn(async move {
            if let Ok(info) = requests::info(&id).await {
                emit("serverRequests", json!({ "serverId": id, "info": info }));
            }
        });
    }
}

fn digest(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// Starts the bots servers want (requests on, through Discord, with a token) and stops the rest. Called at start
/// and whenever a server's request settings or token change.
pub async fn reconcile() {
    let mut want: BTreeMap<String, (String, Vec<(String, String)>)> = BTreeMap::new();
    let mut off: Vec<(String, String)> = Vec::new();
    for m in store::list().await.unwrap_or_default() {
        let r = &m.join_requests;
        match (r.enabled && r.discord).then_some(()).and(requests::bot_token(&m.id).await) {
            Some(token) => want.entry(digest(&token)).or_insert_with(|| (token, Vec::new())).1.push((m.id.clone(), m.name.clone())),
            None => off.push((m.id.clone(), m.name.clone())),
        }
    }
    let mut start = Vec::new();
    {
        let mut running = bots().lock().unwrap();
        running.retain(|d, r| {
            let keep = want.get(d).is_some_and(|(_, s)| *s == r.servers);
            if !keep {
                let _ = r.stop.send(true);
            }
            keep
        });
        for (d, (token, servers)) in want {
            if !running.contains_key(&d) {
                let (stop, rx) = watch::channel(false);
                running.insert(d, Running { servers: servers.clone(), stop });
                start.push((token, servers, rx));
            }
        }
    }
    set_status(&off, BotStatus { state: "off", ..Default::default() });
    for (token, servers, rx) in start {
        tokio::spawn(run(token, servers, rx));
    }
}

/// Starts the bots that should run (at launch).
pub async fn init() {
    reconcile().await;
}

enum End {
    Stopped,
    /// Discord refused the token: no point retrying until it changes.
    BadToken,
    Again(String),
}

async fn run(token: String, servers: Vec<(String, String)>, mut stop: watch::Receiver<bool>) {
    let mut wait = Duration::from_secs(3);
    loop {
        set_status(&servers, BotStatus { state: "connecting", ..Default::default() });
        match session(&token, &servers, &mut stop).await {
            End::Stopped => return,
            End::BadToken => {
                set_status(&servers, BotStatus { state: "error", message: Some("Discord refused this bot token. Copy it again from the Developer Portal (Bot → Reset Token).".into()), ..Default::default() });
                return;
            }
            End::Again(why) => {
                set_status(&servers, BotStatus { state: "connecting", message: Some(why), ..Default::default() });
                tokio::select! {
                    _ = tokio::time::sleep(wait) => {}
                    _ = stop.changed() => return,
                }
                wait = (wait * 2).min(Duration::from_secs(60));
            }
        }
        if *stop.borrow() {
            return;
        }
    }
}

fn tls() -> Result<Arc<rustls::ClientConfig>> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    // An explicit provider: the launcher links crates that enable other rustls providers too.
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    Ok(Arc::new(
        rustls::ClientConfig::builder_with_provider(provider).with_safe_default_protocol_versions()?.with_root_certificates(roots).with_no_client_auth(),
    ))
}

/// One gateway connection, until it ends.
async fn session(token: &str, servers: &[(String, String)], stop: &mut watch::Receiver<bool>) -> End {
    let config = match tls() {
        Ok(c) => c,
        Err(e) => return End::Again(e.to_string()),
    };
    let ws = match connect_async_tls_with_config(GATEWAY, None, false, Some(Connector::Rustls(config))).await {
        Ok((ws, _)) => ws,
        Err(e) => return End::Again(format!("Couldn't reach Discord: {e}")),
    };
    let (mut tx, mut rx) = ws.split();
    // Hello: how often to heartbeat.
    let interval = match rx.next().await {
        Some(Ok(Message::Text(t))) => serde_json::from_str::<Value>(&t).ok().and_then(|v| v["d"]["heartbeat_interval"].as_u64()).unwrap_or(41_250),
        _ => return End::Again("Discord closed the connection".into()),
    };
    let identify = json!({
        "op": 2,
        "d": { "token": token, "intents": 0, "properties": { "os": std::env::consts::OS, "browser": "ResinCraft", "device": "ResinCraft" } }
    });
    if tx.send(Message::Text(identify.to_string().into())).await.is_err() {
        return End::Again("Couldn't sign in to Discord".into());
    }
    let mut beat = tokio::time::interval(Duration::from_millis(interval));
    beat.tick().await;
    let mut seq: Option<i64> = None;
    let mut acked = true;
    loop {
        tokio::select! {
            _ = stop.changed() => {
                let _ = tx.send(Message::Close(None)).await;
                return End::Stopped;
            }
            _ = beat.tick() => {
                if !acked {
                    return End::Again("Discord stopped answering".into());
                }
                acked = false;
                if tx.send(Message::Text(json!({ "op": 1, "d": seq }).to_string().into())).await.is_err() {
                    return End::Again("The connection to Discord dropped".into());
                }
            }
            msg = rx.next() => {
                let text = match msg {
                    Some(Ok(Message::Text(t))) => t.to_string(),
                    Some(Ok(Message::Close(frame))) => {
                        let code = frame.map(|f| u16::from(f.code)).unwrap_or(0);
                        // 4004: authentication failed; 4014: disallowed intents (none are asked for, but the same fix).
                        return if code == 4004 || code == 4014 { End::BadToken } else { End::Again(format!("Discord closed the connection ({code})")) };
                    }
                    Some(Ok(_)) => continue,
                    Some(Err(e)) => return End::Again(format!("The connection to Discord dropped: {e}")),
                    None => return End::Again("The connection to Discord dropped".into()),
                };
                let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
                match v["op"].as_i64() {
                    Some(0) => {
                        seq = v["s"].as_i64().or(seq);
                        match v["t"].as_str() {
                            Some("READY") => {
                                let app = v["d"]["application"]["id"].as_str().unwrap_or_default().to_string();
                                let name = v["d"]["user"]["username"].as_str().map(str::to_string);
                                let invite = format!("https://discord.com/oauth2/authorize?client_id={app}&scope=bot%20applications.commands&permissions=0");
                                let registered = register(token, &app, servers).await;
                                set_status(servers, BotStatus {
                                    state: "online",
                                    name,
                                    invite_url: Some(invite),
                                    message: registered.err().map(|e| format!("Couldn't add /whitelist to Discord: {e}")),
                                });
                            }
                            Some("INTERACTION_CREATE") => {
                                let servers = servers.to_vec();
                                tokio::spawn(async move { answer(v["d"].clone(), &servers).await });
                            }
                            _ => {}
                        }
                    }
                    Some(1) => {
                        let _ = tx.send(Message::Text(json!({ "op": 1, "d": seq }).to_string().into())).await;
                    }
                    Some(7) => return End::Again("Discord asked to reconnect".into()),
                    Some(9) => return End::Again("Discord ended the session".into()),
                    Some(11) => acked = true,
                    _ => {}
                }
            }
        }
    }
}

/// The `/whitelist` command, with a server picker when one bot serves several servers.
fn command(servers: &[(String, String)]) -> Value {
    let mut options = vec![json!({
        "type": 3, "name": "username", "description": "Your Minecraft username", "required": true, "min_length": 3, "max_length": 17
    })];
    if servers.len() > 1 {
        let choices: Vec<Value> = servers.iter().take(25).map(|(id, name)| json!({ "name": name.chars().take(100).collect::<String>(), "value": id })).collect();
        options.push(json!({ "type": 3, "name": "server", "description": "Which Minecraft server", "required": true, "choices": choices }));
    }
    json!([{
        "name": "whitelist",
        "description": "Ask to be whitelisted on the Minecraft server",
        "options": options,
        // In servers the bot is in (not in DMs).
        "contexts": [0],
        "integration_types": [0]
    }])
}

async fn register(token: &str, app: &str, servers: &[(String, String)]) -> Result<()> {
    let res = http().put(format!("{API}/applications/{app}/commands")).header("Authorization", format!("Bot {token}")).json(&command(servers)).send().await?;
    if !res.status().is_success() {
        return Err(anyhow!("Discord said {}", res.status()));
    }
    Ok(())
}

/// Answers a `/whitelist` use, privately.
async fn answer(d: Value, servers: &[(String, String)]) {
    if d["type"].as_i64() != Some(2) || d["data"]["name"].as_str() != Some("whitelist") {
        return;
    }
    let option = |name: &str| d["data"]["options"].as_array().and_then(|o| o.iter().find(|x| x["name"] == name)).and_then(|x| x["value"].as_str()).map(str::to_string);
    let user = if d["member"]["user"].is_object() { &d["member"]["user"] } else { &d["user"] };
    let user_id = user["id"].as_str().unwrap_or_default().to_string();
    let who = user["global_name"].as_str().or(user["username"].as_str()).unwrap_or("someone").to_string();
    let username = option("username").unwrap_or_default();
    let server = match servers {
        [(id, _)] => Some(id.clone()),
        _ => option("server").filter(|s| servers.iter().any(|(id, _)| id == s)),
    };
    let text = match server {
        None => "Pick a server.".to_string(),
        Some(id) => {
            let name = servers.iter().find(|(s, _)| *s == id).map(|(_, n)| n.clone()).unwrap_or_default();
            match requests::submit(&id, &username, Via::Discord { user_id, name: who }).await {
                Outcome::Received => format!("Thanks! The host of **{name}** will look at your request for `{username}`. I'll message you when they answer."),
                Outcome::AlreadyListed => format!("`{username}` is already on the whitelist of **{name}**. You can join."),
                Outcome::AlreadyRequested => format!("There's already a request for `{username}` on **{name}**. The host will answer it."),
                Outcome::Closed => format!("**{name}** isn't taking whitelist requests right now."),
                Outcome::Invalid => format!("`{username}` isn't a valid Minecraft username."),
            }
        }
    };
    let (id, token) = (d["id"].as_str().unwrap_or_default(), d["token"].as_str().unwrap_or_default());
    // Type 4: a message; flag 64: only the player sees it.
    let _ = http().post(format!("{API}/interactions/{id}/{token}/callback")).json(&json!({ "type": 4, "data": { "content": text, "flags": 64 } })).send().await;
}

/// Messages a Discord user through server `id`'s bot (the answer to their request). Fails quietly: they may have
/// direct messages off.
pub async fn tell(id: &str, user_id: &str, text: &str) {
    let Some(token) = requests::bot_token(id).await else { return };
    let auth = format!("Bot {token}");
    let dm = http().post(format!("{API}/users/@me/channels")).header("Authorization", &auth).json(&json!({ "recipient_id": user_id })).send().await;
    let Ok(dm) = dm else { return };
    let Ok(channel) = dm.json::<Value>().await else { return };
    let Some(channel) = channel["id"].as_str() else { return };
    let _ = http().post(format!("{API}/channels/{channel}/messages")).header("Authorization", &auth).json(&json!({ "content": text })).send().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_server_needs_no_picker() {
        let one = command(&[("a".into(), "SMP".into())]);
        assert_eq!(one[0]["options"].as_array().unwrap().len(), 1);
        let two = command(&[("a".into(), "SMP".into()), ("b".into(), "Creative".into())]);
        let picker = &two[0]["options"][1];
        assert_eq!(picker["name"], "server");
        assert_eq!(picker["choices"][1], json!({ "name": "Creative", "value": "b" }));
    }
}
