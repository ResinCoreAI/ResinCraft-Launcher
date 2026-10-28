//! Whitelist requests: players ask to join a hosted server through the host's own Discord bot (discordbot.rs) or,
//! with ResinCraft Relay, a web page at the server's relay address; the host accepts or rejects them in the server's
//! Players tab. Accepting puts the player on the whitelist (and the bot tells them on Discord).
//!
//! Pending requests are kept in `<dataDir>/servers/join-requests.json`, who the host blocked in `join-blocks.json`;
//! a server's bot token is kept sealed (like account tokens) in `<dataDir>/servers/discord-bots.json`, apart from
//! server.json, and never sent to the UI. A request from the server list (a signed-in player) tells the relay what
//! the host decided, so the player's launcher can let them know.

use std::collections::HashMap;
use std::sync::OnceLock;

use anyhow::{bail, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::core::events::emit;
use crate::core::fsutil::{now_ms, read_json, write_json};
use crate::core::types::{JoinRequest, ServerMeta};
use crate::player::accounts::{seal, unseal};

use super::{discordbot, properties, store};

/// At most this many requests wait per server; more are turned away until the host answers some.
const MAX_PENDING: usize = 100;

/// Where a request came from.
pub enum Via {
    /// The relay's id for the request; `listing`: from the server list rather than the web page.
    Web { relay_id: u64, listing: bool },
    /// The Discord user's id (to message them the answer) and the name they go by.
    Discord { user_id: String, name: String },
}

/// What happened to a request (the relay's JoinOutcome has the same cases).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Received,
    AlreadyListed,
    AlreadyRequested,
    /// The server isn't taking requests this way (switched off, or too many waiting).
    Closed,
    Invalid,
    /// On the server's ban list.
    Banned,
    /// The host blocked this Discord user from asking.
    Blocked,
}

/// The Players tab's view of requests (`server:requests`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestsInfo {
    pub requests: Vec<JoinRequest>,
    pub has_bot_token: bool,
    /// Discord users the host blocked from asking (Reject and block).
    pub blocked_on_discord: usize,
    /// Everyone the host blocked (Reject and block), newest first.
    pub blocked: Vec<Blocked>,
    pub bot: discordbot::BotStatus,
    /// Whether server.properties has the whitelist on (requests only matter then).
    pub whitelist_on: bool,
    /// The web page players ask on, once the server has a ResinCraft Relay name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_url: Option<String>,
}

fn lock() -> &'static Mutex<()> {
    static L: OnceLock<Mutex<()>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(()))
}

fn requests_file() -> std::path::PathBuf {
    store::root().join("join-requests.json")
}

/// Someone the host blocked from asking.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Blocked {
    /// A Discord user id, or a Minecraft name in lowercase (web page and server list requests).
    pub key: String,
    /// "discord" | "name"
    pub kind: String,
    /// What to show: the Minecraft name they asked with (and their Discord name).
    pub name: String,
    pub at: i64,
}

fn blocks_file() -> std::path::PathBuf {
    store::root().join("join-blocks.json")
}

/// Before blocks had names: Discord user ids per server.
fn old_blocked_file() -> std::path::PathBuf {
    store::root().join("join-blocked.json")
}

/// Who is blocked, per server.
async fn blocked() -> HashMap<String, Vec<Blocked>> {
    if let Some(b) = read_json(&blocks_file()).await {
        return b;
    }
    let old: HashMap<String, Vec<String>> = read_json(&old_blocked_file()).await.unwrap_or_default();
    old.into_iter()
        .map(|(id, users)| (id, users.into_iter().map(|u| Blocked { name: format!("Discord user {u}"), key: u, kind: "discord".into(), at: 0 }).collect()))
        .collect()
}

async fn save_blocked(all: &HashMap<String, Vec<Blocked>>) -> Result<()> {
    write_json(&blocks_file(), all).await?;
    let _ = tokio::fs::remove_file(old_blocked_file()).await;
    Ok(())
}

fn tokens_file() -> std::path::PathBuf {
    store::root().join("discord-bots.json")
}

async fn all() -> HashMap<String, Vec<JoinRequest>> {
    read_json(&requests_file()).await.unwrap_or_default()
}

/// A Java username (3 to 16 letters, digits or _), or a Floodgate Bedrock name (with its leading dot).
pub fn valid_username(name: &str) -> bool {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^\.?[A-Za-z0-9_]{3,16}$").unwrap()).is_match(name)
}

pub async fn list(id: &str) -> Vec<JoinRequest> {
    all().await.remove(id).unwrap_or_default()
}

pub async fn info(id: &str) -> Result<RequestsInfo> {
    let meta = store::get(id).await?;
    let dir = store::dir(id)?;
    let whitelist_on = properties::read(&dir).await.iter().any(|p| p.key == "white-list" && p.value.trim() == "true");
    let address = (!meta.kind.bedrock()).then(|| super::relay::address_of(&meta)).flatten();
    let mut blocks = blocked().await.remove(id).unwrap_or_default();
    blocks.sort_by_key(|b| std::cmp::Reverse(b.at));
    Ok(RequestsInfo {
        requests: list(id).await,
        has_bot_token: bot_token(id).await.is_some(),
        blocked_on_discord: blocks.iter().filter(|b| b.kind == "discord").count(),
        blocked: blocks,
        bot: discordbot::status(id),
        whitelist_on,
        // The player's verified domain, the name's own address, or /join/<name> on the relay (relay::join_page).
        web_url: match address {
            Some(_) => super::relay::join_page(&meta).await,
            None => None,
        },
    })
}

/// Tells the UI the server's requests changed.
async fn changed(id: &str) {
    if let Ok(i) = info(id).await {
        emit("serverRequests", serde_json::json!({ "serverId": id, "info": i }));
    }
}

/// Whether `meta` takes requests coming `via` this way.
fn open_for(meta: &ServerMeta, via: &Via) -> bool {
    let r = &meta.join_requests;
    r.enabled
        && match via {
            Via::Web { .. } => r.web,
            Via::Discord { .. } => r.discord,
        }
}

/// A player asks to be whitelisted on server `id`.
pub async fn submit(id: &str, username: &str, via: Via) -> Outcome {
    let username = username.trim();
    let Ok(meta) = store::get(id).await else { return Outcome::Closed };
    if !open_for(&meta, &via) {
        return Outcome::Closed;
    }
    if !valid_username(username) {
        return Outcome::Invalid;
    }
    let key = match &via {
        Via::Discord { user_id, .. } => user_id.clone(),
        Via::Web { .. } => username.to_ascii_lowercase(),
    };
    if blocked().await.get(id).is_some_and(|b| b.iter().any(|b| b.key == key)) {
        return Outcome::Blocked;
    }
    // Banned players never reach the host (a Bedrock name keeps Floodgate's leading dot, as the list does).
    let banned = properties::list_entries(id, "bans").await.unwrap_or_default();
    if banned.iter().any(|e| e.name.eq_ignore_ascii_case(username)) {
        return Outcome::Banned;
    }
    let listed = properties::list_entries(id, "whitelist").await.unwrap_or_default();
    if listed.iter().any(|e| e.name.eq_ignore_ascii_case(username)) {
        return Outcome::AlreadyListed;
    }
    {
        let _guard = lock().lock().await;
        let mut every = all().await;
        let pending = every.entry(id.to_string()).or_default();
        if pending.iter().any(|r| r.username.eq_ignore_ascii_case(username)) {
            return Outcome::AlreadyRequested;
        }
        if pending.len() >= MAX_PENDING {
            return Outcome::Closed;
        }
        let (source, from, discord_user, relay_id) = match via {
            Via::Web { relay_id, listing } => (if listing { "list" } else { "web" }, None, None, Some(relay_id)),
            Via::Discord { user_id, name } => ("discord", Some(name), Some(user_id), None),
        };
        pending.push(JoinRequest {
            id: uuid::Uuid::new_v4().to_string(),
            username: username.to_string(),
            source: source.into(),
            from,
            discord_user,
            relay_id,
            at: now_ms(),
        });
        if write_json(&requests_file(), &every).await.is_err() {
            return Outcome::Closed;
        }
    }
    changed(id).await;
    Outcome::Received
}

/// Takes request `request` off server `id`'s list.
async fn take(id: &str, request: &str) -> Result<JoinRequest> {
    let _guard = lock().lock().await;
    let mut every = all().await;
    let pending = every.entry(id.to_string()).or_default();
    let Some(at) = pending.iter().position(|r| r.id == request) else { bail!("That request was already answered") };
    let r = pending.remove(at);
    if pending.is_empty() {
        every.remove(id);
    }
    write_json(&requests_file(), &every).await?;
    Ok(r)
}

/// Puts the player on the whitelist and lets them know on Discord.
pub async fn accept(id: &str, request: &str) -> Result<RequestsInfo> {
    let r = list(id).await.into_iter().find(|r| r.id == request);
    let Some(r) = r else { bail!("That request was already answered") };
    properties::add(id, "whitelist", &r.username).await?;
    take(id, request).await?;
    if r.source == "list" {
        super::relay::send_join_decided(id, &r.username, true);
    }
    if let Some(user) = r.discord_user.as_deref() {
        let name = store::get(id).await.map(|m| m.name).unwrap_or_default();
        discordbot::tell(id, user, &format!("You're on the whitelist of **{name}** now as `{}`. Have fun!", r.username)).await;
    }
    changed(id).await;
    info(id).await
}

pub async fn reject(id: &str, request: &str) -> Result<RequestsInfo> {
    let r = take(id, request).await?;
    if r.source == "list" {
        super::relay::send_join_decided(id, &r.username, false);
    }
    if let Some(user) = r.discord_user.as_deref() {
        let name = store::get(id).await.map(|m| m.name).unwrap_or_default();
        discordbot::tell(id, user, &format!("Your request to join **{name}** as `{}` wasn't accepted.", r.username)).await;
    }
    changed(id).await;
    info(id).await
}

/// Rejects a request and blocks whoever sent it until Unblock: that Discord user, or that Minecraft name (web page
/// and server list requests).
pub async fn reject_and_block(id: &str, request: &str) -> Result<RequestsInfo> {
    let r = take(id, request).await?;
    if r.source == "list" {
        super::relay::send_join_decided(id, &r.username, false);
    }
    let (key, kind, name) = match (&r.discord_user, &r.from) {
        (Some(user), from) => (user.clone(), "discord", match from {
            Some(f) => format!("{} ({f})", r.username),
            None => r.username.clone(),
        }),
        (None, _) => (r.username.to_ascii_lowercase(), "name", r.username.clone()),
    };
    {
        let _guard = lock().lock().await;
        let mut every = blocked().await;
        let list = every.entry(id.to_string()).or_default();
        if !list.iter().any(|b| b.key == key) {
            list.push(Blocked { key, kind: kind.into(), name, at: now_ms() });
        }
        save_blocked(&every).await?;
    }
    if let Some(user) = r.discord_user.as_deref() {
        let name = store::get(id).await.map(|m| m.name).unwrap_or_default();
        discordbot::tell(id, user, &format!("Your request to join **{name}** as `{}` wasn't accepted.", r.username)).await;
    }
    changed(id).await;
    info(id).await
}

/// Lets everyone blocked on server `id` ask again.
pub async fn unblock_all(id: &str) -> Result<RequestsInfo> {
    {
        let _guard = lock().lock().await;
        let mut every = blocked().await;
        if every.remove(id).is_some() {
            save_blocked(&every).await?;
        }
    }
    changed(id).await;
    info(id).await
}

/// Lets one blocked player (`key` from [`Blocked`]) ask again.
pub async fn unblock(id: &str, key: &str) -> Result<RequestsInfo> {
    {
        let _guard = lock().lock().await;
        let mut every = blocked().await;
        if let Some(list) = every.get_mut(id) {
            list.retain(|b| b.key != key);
            if list.is_empty() {
                every.remove(id);
            }
            save_blocked(&every).await?;
        }
    }
    changed(id).await;
    info(id).await
}

/// Drops a deleted server's requests and bot token.
pub async fn forget(id: &str) {
    {
        let _guard = lock().lock().await;
        let mut every = all().await;
        if every.remove(id).is_some() {
            let _ = write_json(&requests_file(), &every).await;
        }
        let mut blocks = blocked().await;
        if blocks.remove(id).is_some() {
            let _ = save_blocked(&blocks).await;
        }
    }
    let _ = set_bot_token(id, None).await;
}

// ---------- The bot token ----------

async fn tokens() -> HashMap<String, String> {
    read_json(&tokens_file()).await.unwrap_or_default()
}

/// Saves (or with None/empty, removes) the server's Discord bot token, then starts or stops bots to match.
pub async fn set_bot_token(id: &str, token: Option<String>) -> Result<()> {
    let mut every = tokens().await;
    match token.map(|t| t.trim().trim_start_matches("Bot ").trim().to_string()).filter(|t| !t.is_empty()) {
        Some(t) => every.insert(id.to_string(), seal(&t)),
        None => every.remove(id),
    };
    write_json(&tokens_file(), &every).await?;
    discordbot::reconcile().await;
    Ok(())
}

pub async fn bot_token(id: &str) -> Option<String> {
    let sealed = tokens().await.remove(id)?;
    unseal(Some(&sealed)).ok().filter(|t| !t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usernames() {
        for ok in ["Steve", "Puirkle_Gamer", "abc", "ABCDEFGHIJKLMNOP", ".PuirkleBE"] {
            assert!(valid_username(ok), "{ok}");
        }
        for bad in ["ab", "ABCDEFGHIJKLMNOPQ", "has space", "bad-dash", "..dots", "", "Steve!"] {
            assert!(!valid_username(bad), "{bad}");
        }
    }
}
