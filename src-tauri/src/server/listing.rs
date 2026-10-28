//! The public server list on ResinCraft Relay (relay protocol 8): a server's own listing (opted in per server, off
//! by default), servers hosted elsewhere that the player submits, and what players do in Discover → Servers
//! (browse, like, report, ask to be whitelisted under their own Minecraft account).
//!
//! What a server's listing says is kept here too (`server-list.json`), so the Settings tab shows it even while the
//! relay can't be reached, and so it can be listed again when the relay forgot it (a renamed or expired name).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Duration;

use anyhow::Result;
use resincraft_relay::agent::Connection;
use resincraft_relay::proto::{Edition, JoinOutcome, ListingInfo, ListingModpack, ListingState, ListingStatus, MyListing, CATEGORIES};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::events::emit;
use crate::core::fsutil::{now_ms, read_json, write_json};
use crate::core::paths;
use crate::core::text::Text;
use crate::core::types::ServerMeta;
use crate::player::accounts;

use crate::err_tr;

use super::{relay, store};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Saved {
    /// Server id → its listing.
    #[serde(default)]
    servers: HashMap<String, SavedListing>,
    /// Listings this launcher's players liked (the public list doesn't say).
    #[serde(default)]
    liked: BTreeSet<String>,
    /// Whitelist requests this launcher's players sent from the list (listing id → the newest), until 30 days after.
    #[serde(default)]
    asked: BTreeMap<String, Asked>,
}

/// A whitelist request sent from the list, and what the host decided.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asked {
    /// The Microsoft account (UUID) it was sent as.
    account: String,
    /// The server's name in the list.
    pub title: String,
    /// "waiting", "accepted" or "rejected".
    pub state: String,
    /// When it was sent or decided (ms).
    pub at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedListing {
    enabled: bool,
    info: ListingInfo,
}

fn file() -> std::path::PathBuf {
    paths::root().join("server-list.json")
}

async fn load() -> Saved {
    read_json(&file()).await.unwrap_or_default()
}

async fn save(s: &Saved) -> Result<()> {
    write_json(&file(), s).await
}

/// A server's listing as its Settings tab shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerListing {
    /// The server has a ResinCraft Relay name (it was online through the relay at least once).
    pub available: bool,
    pub enabled: bool,
    pub info: ListingInfo,
    /// Where the relay has it (None while off, or when the relay couldn't be asked).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
    /// The relay is too old for the list (protocol < 8).
    pub unsupported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Text>,
    /// The fixed categories, for the tag picker.
    pub categories: &'static [&'static str],
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub id: String,
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

fn status(st: ListingStatus) -> Status {
    Status {
        id: st.id,
        state: match st.state {
            ListingState::Listed => "listed",
            ListingState::Pending => "pending",
            ListingState::Rejected => "rejected",
            ListingState::Hidden => "hidden",
            ListingState::Off => "off",
            ListingState::Unknown => "unknown",
        },
        message: st.message,
    }
}

/// What a new listing starts with: the server's name, edition, version and whether it runs mods.
fn defaults(meta: &ServerMeta) -> ListingInfo {
    ListingInfo {
        title: meta.name.chars().take(48).collect(),
        version: meta.mc_version.clone(),
        edition: if meta.kind.bedrock() { Edition::Bedrock } else { Edition::Java },
        modded: meta.kind.modded(),
        tags: if meta.kind.modded() { vec!["modded".into()] } else { vec!["survival".into()] },
        ..Default::default()
    }
}

/// What the server itself says about who can join (players see it in the list, so it's never typed by hand):
/// Bedrock players through Geyser, and offline profiles when `online-mode=false`.
async fn with_facts(id: &str, meta: &ServerMeta, mut info: ListingInfo) -> ListingInfo {
    info.crossplay = !meta.kind.bedrock()
        && super::plugins::bedrock_status(id).await.ok().and_then(|v| v.get("geyser").and_then(Value::as_bool)) == Some(true);
    info.offline = match store::dir(id) {
        Ok(dir) => super::properties::get_value(&dir, "online-mode").await.is_some_and(|v| v.trim() == "false"),
        Err(_) => false,
    };
    // The relay knows Geyser's port on it; an address typed here would only be stale.
    info.bedrock_address = None;
    // A server made from a modpack says which one (the owner can name a custom pack only when there's none).
    if let Some(pack) = meta.modpack.as_ref() {
        info.modpack = Some(listing_modpack(pack));
    }
    if info.modpack.is_some() {
        info.modded = true;
    }
    info
}

/// A ResinCraft modpack reference as the server list has it: the site comes from the project id's prefix.
fn listing_modpack(pack: &crate::core::types::ModpackRef) -> ListingModpack {
    let id = pack.project_id.clone().unwrap_or_default();
    let (source, project_id) = [("cf:", "curseforge"), ("ftb:", "ftb"), ("atl:", "atlauncher"), ("technic:", "technic")]
        .iter()
        .find_map(|(prefix, site)| id.strip_prefix(prefix).map(|rest| (site.to_string(), rest.to_string())))
        .unwrap_or_else(|| if id.is_empty() || id.contains(':') { (String::new(), String::new()) } else { ("modrinth".into(), id.clone()) });
    ListingModpack { name: pack.name.chars().take(64).collect(), source, project_id, version: None }
}

/// Signs in as the account the server's name belongs to (`owner`) or the active Microsoft account.
async fn connect(owner: Option<&str>) -> Result<Connection> {
    let Some(account) = accounts::microsoft_account(owner).await else { return Err(relay::needs_microsoft()) };
    relay::connect_for_list(&account).await
}

fn not_yet() -> anyhow::Error {
    err_tr!("serverList.notYet", "ResinCraft Relay doesn't have the server list yet.")
}

fn err_text(e: &anyhow::Error) -> Text {
    relay::as_text(e)
}

/// The server's listing, asking the relay where it stands (and listing it again when the relay forgot it).
pub async fn get(id: &str) -> Result<ServerListing> {
    let meta = store::get(id).await?;
    let saved = load().await.servers.get(id).cloned();
    let enabled = saved.as_ref().is_some_and(|s| s.enabled);
    let saved_info = saved.map(|s| s.info);
    let info = with_facts(id, &meta, saved_info.clone().unwrap_or_else(|| defaults(&meta))).await;
    // Geyser or online-mode changed since it was listed: the list should say so.
    let changed = saved_info.as_ref().is_some_and(|s| (s.crossplay, s.offline, &s.modpack) != (info.crossplay, info.offline, &info.modpack));
    let mut out = ServerListing {
        available: meta.relay_name.is_some() && relay::settings().is_some(),
        enabled,
        info,
        status: None,
        unsupported: false,
        error: None,
        categories: CATEGORIES,
    };
    let (Some(name), true) = (meta.relay_name.clone(), out.available && enabled) else { return Ok(out) };
    let asked = async {
        let mut conn = connect(meta.relay_account.as_deref()).await?;
        if !conn.has_listings() {
            return Ok(None);
        }
        let st = conn.listing_get(&name).await.map_err(|e| relay::describe(&e, &name))?;
        // Listed here but not there (the name was renamed or expired since), or who can join changed: list it again.
        if st.state == ListingState::Off || changed {
            let st = conn.listing_set(&name, Some(out.info.clone())).await.map_err(|e| relay::describe(&e, &name))?;
            let mut all = load().await;
            if let Some(s) = all.servers.get_mut(id) {
                s.info = out.info.clone();
            }
            save(&all).await?;
            return Ok(Some(st));
        }
        Ok::<_, anyhow::Error>(Some(st))
    };
    match asked.await {
        Ok(Some(st)) => out.status = Some(status(st)),
        Ok(None) => out.unsupported = true,
        Err(e) => out.error = Some(err_text(&e)),
    }
    Ok(out)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetListing {
    pub enabled: bool,
    pub info: ListingInfo,
}

/// Saves the server's listing and tells the relay (listing it, updating it, or taking it off).
pub async fn set(id: &str, req: SetListing) -> Result<ServerListing> {
    let meta = store::get(id).await?;
    let mut info = with_facts(id, &meta, req.info).await;
    info.title = info.title.trim().chars().take(48).collect();
    info.description = info.description.trim().chars().take(1000).collect();
    if info.title.is_empty() {
        info.title = meta.name.chars().take(48).collect();
    }
    let mut all = load().await;
    let was_on = all.servers.get(id).is_some_and(|s| s.enabled);
    all.servers.insert(id.to_string(), SavedListing { enabled: req.enabled, info: info.clone() });
    save(&all).await?;
    let Some(name) = meta.relay_name.clone() else { return get(id).await };
    if req.enabled || was_on {
        let mut conn = connect(meta.relay_account.as_deref()).await?;
        if !conn.has_listings() {
            return get(id).await;
        }
        let listing = req.enabled.then_some(info);
        if let Err(e) = conn.listing_set(&name, listing).await {
            // Keep the switch where it was when the relay refused (e.g. a bad link): the player fixes it and tries again.
            let mut all = load().await;
            if let Some(s) = all.servers.get_mut(id) {
                s.enabled = was_on;
            }
            save(&all).await?;
            return Err(relay::describe(&e, &name));
        }
    }
    get(id).await
}

/// Takes a deleted server off the list. Best effort.
pub async fn forget(id: &str, meta: &ServerMeta) {
    let mut all = load().await;
    if all.servers.remove(id).is_some_and(|s| s.enabled) {
        if let (Some(name), Ok(mut conn)) = (meta.relay_name.clone(), connect(meta.relay_account.as_deref()).await) {
            if conn.has_listings() {
                let _ = conn.listing_set(&name, None).await;
            }
        }
    }
    let _ = save(&all).await;
}

// ---------- Discover → Servers ----------

/// Browse filters, as the UI sends them (all optional).
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Browse {
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub edition: Option<String>,
    pub version: Option<String>,
    pub region: Option<String>,
    pub lang: Option<String>,
    pub modded: Option<bool>,
    #[serde(default)]
    pub online: bool,
    /// Only servers Bedrock players can join (Bedrock servers, and Java ones with Geyser).
    #[serde(default)]
    pub crossplay: bool,
    /// Only servers offline profiles can join.
    #[serde(default)]
    pub offline: bool,
    /// "<source>:<project id>", or part of a modpack's name.
    pub modpack: Option<String>,
    pub sort: Option<String>,
    #[serde(default)]
    pub page: u32,
}

/// One page of the public list (`GET https://<base domain>/api/servers`), with `liked` filled in from this launcher.
pub async fn browse(f: Browse) -> Result<Value> {
    let Some(s) = relay::settings() else { return Err(err_tr!("serverList.noRelay", "This build has no ResinCraft Relay, so there's no server list.")) };
    let mut query: Vec<(&str, String)> = vec![("page", f.page.to_string())];
    let mut opt = |k: &'static str, v: Option<String>| {
        if let Some(v) = v.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
            query.push((k, v));
        }
    };
    opt("q", Some(f.q));
    opt("tags", Some(f.tags.join(",")));
    opt("edition", f.edition);
    opt("version", f.version);
    opt("region", f.region);
    opt("lang", f.lang);
    opt("modded", f.modded.map(|m| m.to_string()));
    opt("online", f.online.then(|| "true".to_string()));
    opt("crossplay", f.crossplay.then(|| "true".to_string()));
    opt("offline", f.offline.then(|| "true".to_string()));
    opt("modpack", f.modpack);
    opt("sort", f.sort);
    let res = crate::core::ctx::http()
        .get(format!("https://{}/api/servers", s.domain))
        .query(&query)
        .timeout(Duration::from_secs(15))
        .send()
        .await;
    let res = match res {
        Ok(r) => r,
        Err(e) => return Err(err_tr!("serverList.unreachable", "Couldn't reach the server list ({error}).", error = e.to_string())),
    };
    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(not_yet());
    }
    if !res.status().is_success() {
        return Err(err_tr!("serverList.httpError", "The server list didn't answer (HTTP {status}).", status = res.status().as_u16().to_string()));
    }
    let mut page: Value = res.json().await?;
    let liked = load().await.liked;
    for key in ["servers", "featured"] {
        let Some(list) = page.get_mut(key).and_then(Value::as_array_mut) else { continue };
        for s in list {
            let id = s.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
            s["liked"] = Value::Bool(liked.contains(&id));
        }
    }
    Ok(page)
}

/// Every modpack among listed servers, for the Modpack filter (`GET /api/servers/modpacks`).
pub async fn modpacks() -> Result<Value> {
    let Some(s) = relay::settings() else { return Err(err_tr!("serverList.noRelay", "This build has no ResinCraft Relay, so there's no server list.")) };
    let res = crate::core::ctx::http().get(format!("https://{}/api/servers/modpacks", s.domain)).timeout(Duration::from_secs(15)).send().await;
    match res {
        Ok(r) if r.status().is_success() => Ok(r.json().await?),
        // An older list without the endpoint: no modpack filter, not an error.
        Ok(_) => Ok(serde_json::json!({ "modpacks": [] })),
        Err(e) => Err(err_tr!("serverList.unreachable", "Couldn't reach the server list ({error}).", error = e.to_string())),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Submit {
    pub address: String,
    pub info: ListingInfo,
}

pub async fn submit(req: Submit) -> Result<Status> {
    let address = req.address.trim().to_string();
    if address.is_empty() {
        return Err(err_tr!("serverList.noAddress", "Type the server's address."));
    }
    let mut conn = connect(None).await?;
    if !conn.has_listings() {
        return Err(not_yet());
    }
    let st = conn.listing_submit(&address, req.info).await.map_err(|e| relay::describe(&e, ""))?;
    Ok(status(st))
}

/// The player's own listings: submitted servers and hosted ones.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mine {
    pub status: Status,
    pub address: String,
    pub info: ListingInfo,
    pub likes: u32,
    pub resincraft: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pack: Option<resincraft_relay::proto::ListingPack>,
}

pub async fn mine() -> Result<Vec<Mine>> {
    let mut conn = connect(None).await?;
    if !conn.has_listings() {
        return Ok(Vec::new());
    }
    let list = conn.listing_mine().await.map_err(|e| relay::describe(&e, ""))?;
    Ok(list
        .into_iter()
        .map(|m: MyListing| Mine { status: status(m.status), address: m.address, info: m.listing, likes: m.likes, resincraft: m.resincraft, pack: m.pack })
        .collect())
}

pub async fn remove(id: &str) -> Result<Status> {
    let mut conn = connect(None).await?;
    let st = conn.listing_remove(id).await.map_err(|e| relay::describe(&e, ""))?;
    Ok(status(st))
}

#[derive(Debug, Serialize)]
pub struct Liked {
    pub likes: u32,
    pub liked: bool,
}

pub async fn like(id: &str, on: bool) -> Result<Liked> {
    let mut conn = connect(None).await?;
    let (likes, liked) = conn.listing_like(id, on).await.map_err(|e| relay::describe(&e, ""))?;
    let mut all = load().await;
    if liked {
        all.liked.insert(id.to_string());
    } else {
        all.liked.remove(id);
    }
    save(&all).await?;
    Ok(Liked { likes, liked })
}

pub async fn report(id: &str, reason: &str) -> Result<()> {
    let reason: String = reason.trim().chars().take(500).collect();
    if reason.is_empty() {
        return Err(err_tr!("serverList.noReason", "Say what's wrong with this server."));
    }
    let mut conn = connect(None).await?;
    conn.listing_report(id, &reason).await.map_err(|e| relay::describe(&e, ""))
}

/// Asks a ResinCraft server's host to whitelist the active account; the outcome as the UI words it.
pub async fn ask_join(id: &str, title: Option<String>) -> Result<&'static str> {
    let Some(account) = accounts::microsoft_account(None).await else { return Err(relay::needs_microsoft()) };
    let mut conn = relay::connect_for_list(&account).await?;
    let outcome = conn.listing_join(id).await.map_err(|e| relay::describe(&e, ""))?;
    // Waiting for the host: remembered, and checked until they decide (watch_joins).
    let state = match outcome {
        JoinOutcome::Received | JoinOutcome::AlreadyRequested => Some("waiting"),
        JoinOutcome::AlreadyListed => Some("accepted"),
        _ => None,
    };
    if let Some(state) = state {
        let mut all = load().await;
        let title = title.filter(|t| !t.trim().is_empty()).unwrap_or_else(|| id.trim_start_matches("rc-").to_string());
        all.asked.insert(id.to_string(), Asked { account: account.uuid.clone(), title, state: state.into(), at: now_ms() });
        save(&all).await?;
        emit("joinStates", join_states_of(&all));
        if state == "waiting" {
            WATCH.notify_one();
        }
    }
    Ok(match outcome {
        JoinOutcome::Received => "received",
        JoinOutcome::AlreadyListed => "alreadyListed",
        JoinOutcome::AlreadyRequested => "alreadyRequested",
        JoinOutcome::Closed => "closed",
        JoinOutcome::Invalid => "invalid",
        JoinOutcome::Banned => "banned",
        JoinOutcome::Unknown => "unknown",
    })
}

/// Requests asked or decided longer ago than this are forgotten (the relay keeps them as long).
const ASKED_KEPT_MS: i64 = 30 * 86_400_000;

fn join_states_of(all: &Saved) -> BTreeMap<String, String> {
    all.asked.iter().map(|(id, a)| (id.clone(), a.state.clone())).collect()
}

/// Where this launcher's whitelist requests from the list stand (listing id → "waiting", "accepted" or
/// "rejected"), for Discover → Servers.
pub async fn join_states() -> BTreeMap<String, String> {
    join_states_of(&load().await)
}

/// Wakes the watcher early (a request was just sent, or Discover → Servers opened).
static WATCH: tokio::sync::Notify = tokio::sync::Notify::const_new();

pub fn check_joins_now() {
    WATCH.notify_one();
}

/// Asks the relay about requests still waiting, once; tells the UI about the ones the host decided.
async fn check_joins() -> bool {
    let mut all = load().await;
    let now = now_ms();
    all.asked.retain(|_, a| now - a.at < ASKED_KEPT_MS);
    let accounts: BTreeSet<String> = all.asked.values().filter(|a| a.state == "waiting").map(|a| a.account.clone()).collect();
    let mut changed = false;
    let mut answered = false;
    for uuid in accounts {
        let Some(acc) = accounts::microsoft_account(Some(&uuid)).await.filter(|a| a.uuid.replace('-', "").eq_ignore_ascii_case(&uuid.replace('-', ""))) else { continue };
        let Ok(mut conn) = relay::connect_for_list(&acc).await else { continue };
        // A relay that can't say (older than this) or doesn't know the request any more: forgotten, so the player
        // can ask again (the host's launcher says "already asked" while the old one still waits).
        let joins = conn.listing_joins().await.unwrap_or_default();
        let before = all.asked.len();
        all.asked.retain(|id, a| a.account != uuid || a.state != "waiting" || joins.iter().any(|j| &j.id == id));
        changed |= all.asked.len() != before;
        for j in joins {
            let Some(a) = all.asked.get_mut(&j.id) else { continue };
            if a.account != uuid || a.state != "waiting" || j.state == "waiting" || !(j.state == "accepted" || j.state == "rejected") {
                continue;
            }
            a.state = j.state.clone();
            a.at = now;
            changed = true;
            answered = true;
            emit("joinAnswered", serde_json::json!({ "id": j.id, "title": a.title, "accepted": j.state == "accepted" }));
        }
    }
    if changed {
        let _ = save(&all).await;
        emit("joinStates", join_states_of(&all));
        // Flash the taskbar when the launcher isn't in front, so the player sees the notice.
        use tauri::Manager;
        if let Some(w) = answered.then(crate::core::ctx::app).flatten().and_then(|a| a.get_webview_window("main")) {
            let _ = w.request_user_attention(Some(tauri::UserAttentionType::Informational));
        }
    }
    all.asked.values().any(|a| a.state == "waiting")
}

/// While the launcher runs: checks requests that wait for the host every minute (every 10 minutes when nothing waits,
/// sooner when woken).
pub async fn watch_joins() {
    tokio::time::sleep(Duration::from_secs(20)).await;
    loop {
        let waiting = check_joins().await;
        let pause = if waiting { 60 } else { 600 };
        let _ = tokio::time::timeout(Duration::from_secs(pause), WATCH.notified()).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_listing_reads_back() {
        let mut s = Saved::default();
        s.servers.insert("a".into(), SavedListing { enabled: true, info: ListingInfo { title: "SMP".into(), ..Default::default() } });
        s.liked.insert("rc-smp".into());
        let back: Saved = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert!(back.servers["a"].enabled);
        assert_eq!(back.servers["a"].info.title, "SMP");
        assert!(back.liked.contains("rc-smp"));
        // An empty file is an empty list.
        let empty: Saved = serde_json::from_str("{}").unwrap();
        assert!(empty.servers.is_empty());
    }

    #[test]
    fn modpack_sites_come_from_the_prefix() {
        let pack = |id: Option<&str>| crate::core::types::ModpackRef { project_id: id.map(str::to_string), version_id: None, name: "ATM 9".into() };
        let site = |id| {
            let m = listing_modpack(&pack(id));
            (m.source, m.project_id)
        };
        assert_eq!(site(Some("cf:715572")), ("curseforge".to_string(), "715572".to_string()));
        assert_eq!(site(Some("1KVo5zza")), ("modrinth".to_string(), "1KVo5zza".to_string()));
        assert_eq!(site(Some("ftb:23")), ("ftb".to_string(), "23".to_string()));
        assert_eq!(site(Some("gh:owner/repo")), (String::new(), String::new()));
        assert_eq!(site(None), (String::new(), String::new()));
    }
}
