//! Quick join (`quickjoin:*`): every multiplayer server the player has, one click from playing. The player's own
//! entries and their groups live in `<data>/quickjoin.json` (the launcher's, not an instance's); servers saved in
//! the game come from each instance's servers.dat. Pinging reuses `servers:ping`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use super::files::js_len;
use super::servers::list_servers;
use crate::core::fsutil::{now_ms, read_json, write_json};
use crate::core::types::ServerEntry;
use crate::core::{instances, paths, shell};
use crate::bail_tr;

const MAX_GROUPS: usize = 200;
const MAX_ENTRIES: usize = 1000;
const MAX_RECENT: usize = 20;
const GROUP_NAME_MAX: usize = 60;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edition {
    #[default]
    Java,
    /// Joins through Minecraft for Windows' `minecraft://` link instead of an instance.
    Bedrock,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub collapsed: bool,
}

/// One server the player added to Quick join.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub address: String,
    #[serde(default)]
    pub edition: Edition,
    /// The instance to play it with; None = ask each time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    /// None = Ungrouped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(default)]
    pub pinned: bool,
    /// The server's last icon (base64 PNG, as in servers.dat).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default)]
    pub added_at: i64,
}

/// A server the player joined through the launcher lately (any source).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recent {
    pub address: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub edition: Edition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    pub at: i64,
}

/// Everything in quickjoin.json. Groups and entries are in the player's order.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickJoin {
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(default)]
    pub entries: Vec<Entry>,
    #[serde(default)]
    pub recent: Vec<Recent>,
}

/// A server from the instances' in-game lists, once per address, with every instance that has it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedServer {
    pub name: String,
    pub address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub instances: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinRequest {
    pub address: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub edition: Edition,
    #[serde(default)]
    pub instance_id: Option<String>,
    #[serde(default)]
    pub account_id: Option<String>,
}

/// Reads and writes go one at a time, so a join's "recent" can't be lost to a save.
static LOCK: Mutex<()> = Mutex::const_new(());

fn file() -> PathBuf {
    paths::root().join("quickjoin.json")
}

/// The same server however it was typed: case-insensitive host, default port dropped.
pub fn address_key(address: &str) -> String {
    let a = address.trim().to_ascii_lowercase();
    a.strip_suffix(":25565").filter(|h| !h.is_empty() && !h.ends_with(':')).map(str::to_string).unwrap_or(a)
}

fn base64_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9+/]+=*$").unwrap())
}

fn check_address(address: &str) -> Result<String> {
    let a = address.trim();
    if a.is_empty() {
        bail_tr!("servers.addressMissing", "Every server needs an address");
    }
    if js_len(a) > 300 || a.chars().any(|c| c.is_whitespace() || c.is_control()) {
        bail_tr!("servers.invalidAddress", "Invalid server address");
    }
    Ok(a.to_string())
}

fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..12].to_string()
}

/// Validates what the UI sends: trims names, gives missing or repeated ids a fresh one, drops empty groups' names
/// and links to groups that no longer exist. `recent` is kept from `current` (only joins change it).
pub fn normalize(mut data: QuickJoin, current_recent: Vec<Recent>) -> Result<QuickJoin> {
    if data.groups.len() > MAX_GROUPS || data.entries.len() > MAX_ENTRIES {
        bail_tr!("quickjoin.tooMany", "Too many Quick join entries");
    }
    let mut ids = HashSet::new();
    let mut groups = Vec::with_capacity(data.groups.len());
    for mut g in data.groups.drain(..) {
        g.name = g.name.trim().to_string();
        if g.name.is_empty() {
            continue;
        }
        if js_len(&g.name) > GROUP_NAME_MAX {
            bail_tr!("quickjoin.groupNameTooLong", "Group names can be at most {max} characters", max = GROUP_NAME_MAX);
        }
        if g.id.is_empty() || !ids.insert(g.id.clone()) {
            g.id = new_id();
            ids.insert(g.id.clone());
        }
        groups.push(g);
    }
    let group_ids: HashSet<&str> = groups.iter().map(|g| g.id.as_str()).collect();
    let mut entry_ids = HashSet::new();
    let mut entries = Vec::with_capacity(data.entries.len());
    for mut e in data.entries.drain(..) {
        e.address = check_address(&e.address)?;
        e.name = e.name.trim().to_string();
        if e.name.is_empty() {
            e.name = e.address.clone();
        }
        if js_len(&e.name) > 200 {
            bail_tr!("servers.tooLong", "Server name or address is too long");
        }
        if e.id.is_empty() || !entry_ids.insert(e.id.clone()) {
            e.id = new_id();
            entry_ids.insert(e.id.clone());
        }
        e.group_id = e.group_id.filter(|g| group_ids.contains(g.as_str()));
        e.instance_id = e.instance_id.filter(|i| !i.trim().is_empty());
        if e.edition == Edition::Bedrock {
            e.instance_id = None;
        }
        e.icon = e.icon.filter(|i| !i.is_empty() && i.len() <= 1_000_000 && base64_re().is_match(i));
        if e.added_at <= 0 {
            e.added_at = now_ms();
        }
        entries.push(e);
    }
    Ok(QuickJoin { groups, entries, recent: current_recent })
}

/// Puts a join at the front of `recent` (once per address), keeping the newest few.
pub fn push_recent(recent: &mut Vec<Recent>, item: Recent) {
    let key = address_key(&item.address);
    recent.retain(|r| address_key(&r.address) != key);
    recent.insert(0, item);
    recent.truncate(MAX_RECENT);
}

/// One entry per address across instances, in the order they first appear.
pub fn merge_saved(lists: Vec<(String, Vec<ServerEntry>)>) -> Vec<SavedServer> {
    let mut out: Vec<SavedServer> = Vec::new();
    for (instance, list) in lists {
        for s in list.into_iter().filter(|s| s.hidden != Some(true) && !s.ip.trim().is_empty()) {
            let key = address_key(&s.ip);
            match out.iter_mut().find(|o| address_key(&o.address) == key) {
                Some(o) => {
                    if !o.instances.contains(&instance) {
                        o.instances.push(instance.clone());
                    }
                    if o.icon.is_none() {
                        o.icon = s.icon;
                    }
                }
                None => out.push(SavedServer {
                    name: if s.name.trim().is_empty() { s.ip.trim().to_string() } else { s.name },
                    address: s.ip.trim().to_string(),
                    icon: s.icon,
                    instances: vec![instance.clone()],
                }),
            }
        }
    }
    out
}

pub async fn load_from(path: &Path) -> QuickJoin {
    read_json(path).await.unwrap_or_default()
}

pub async fn save_to(path: &Path, data: QuickJoin) -> Result<QuickJoin> {
    let current = load_from(path).await;
    // A file that exists but no longer parses is kept aside instead of being overwritten.
    if current == QuickJoin::default() && tokio::fs::metadata(path).await.is_ok_and(|m| m.len() > 2) && read_json::<QuickJoin>(path).await.is_none() {
        let _ = tokio::fs::copy(path, path.with_extension("json.bad")).await;
    }
    let data = normalize(data, current.recent)?;
    write_json(path, &data).await?;
    Ok(data)
}

pub async fn get() -> QuickJoin {
    let _g = LOCK.lock().await;
    load_from(&file()).await
}

pub async fn save(data: QuickJoin) -> Result<QuickJoin> {
    let _g = LOCK.lock().await;
    save_to(&file(), data).await
}

pub async fn clear_recent() -> Result<QuickJoin> {
    let _g = LOCK.lock().await;
    let mut data = load_from(&file()).await;
    data.recent.clear();
    write_json(&file(), &data).await?;
    Ok(data)
}

/// Servers saved in any instance's in-game list.
pub async fn saved() -> Result<Vec<SavedServer>> {
    let mut lists = Vec::new();
    for inst in instances::list().await? {
        // An instance with an unreadable servers.dat just contributes nothing.
        let list = list_servers(&inst.id).await.unwrap_or_default();
        lists.push((inst.id, list));
    }
    Ok(merge_saved(lists))
}

/// The `minecraft://` link that adds a server to Bedrock's list and opens it there.
pub fn bedrock_link(name: &str, address: &str) -> String {
    let name: String = name.chars().filter(|c| *c != '|').collect();
    let address = if address.contains(':') { address.to_string() } else { format!("{address}:19132") };
    format!("minecraft://?addExternalServer={}|{}", urlencoding::encode(&name), address)
}

/// Starts the game straight into the server (or opens Bedrock with it), and remembers the join.
pub async fn join(req: JoinRequest) -> Result<()> {
    let address = check_address(&req.address)?;
    let name = req.name.as_deref().map(str::trim).filter(|n| !n.is_empty()).unwrap_or(&address).to_string();
    let instance = req.instance_id.filter(|i| !i.is_empty());
    if req.edition == Edition::Java && instance.is_none() {
        bail_tr!("quickjoin.chooseInstance", "Choose an instance to play with");
    }
    {
        let _g = LOCK.lock().await;
        let mut data = load_from(&file()).await;
        push_recent(&mut data.recent, Recent { address: address.clone(), name: name.clone(), edition: req.edition, instance_id: instance.clone(), at: now_ms() });
        write_json(&file(), &data).await?;
    }
    match (req.edition, instance) {
        (Edition::Bedrock, _) => shell::open_app_link(&bedrock_link(&name, &address)),
        (Edition::Java, Some(id)) => crate::game::launch_as(&id, req.account_id.as_deref(), Some(crate::game::launch::Target::Server(address))).await,
        (Edition::Java, None) => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, address: &str, group: Option<&str>) -> Entry {
        Entry { id: id.into(), name: String::new(), address: address.into(), group_id: group.map(String::from), added_at: 1, ..Default::default() }
    }

    fn server(name: &str, ip: &str) -> ServerEntry {
        ServerEntry { name: name.into(), ip: ip.into(), icon: None, accept_textures: None, hidden: None }
    }

    #[test]
    fn address_keys_ignore_case_and_default_port() {
        assert_eq!(address_key(" Play.Example.org:25565 "), "play.example.org");
        assert_eq!(address_key("play.example.org"), "play.example.org");
        assert_eq!(address_key("play.example.org:25566"), "play.example.org:25566");
        assert_eq!(address_key("[::1]:25565"), "[::1]");
    }

    #[test]
    fn normalize_fixes_ids_names_and_groups() {
        let data = QuickJoin {
            groups: vec![
                Group { id: "g1".into(), name: "  Friends ".into(), collapsed: true },
                Group { id: "g1".into(), name: "Dupe id".into(), collapsed: false },
                Group { id: "g3".into(), name: "   ".into(), collapsed: false },
            ],
            entries: vec![entry("a", " mc.example.org ", Some("g1")), entry("a", "other.org", Some("gone")), entry("", "x.org", Some("g3"))],
            recent: vec![Recent { address: "ignored".into(), at: 1, ..Default::default() }],
        };
        let kept = vec![Recent { address: "kept.org".into(), at: 2, ..Default::default() }];
        let out = normalize(data, kept.clone()).unwrap();
        assert_eq!(out.groups.len(), 2, "a group without a name is dropped");
        assert_eq!(out.groups[0].name, "Friends");
        assert!(out.groups[0].collapsed);
        assert_ne!(out.groups[1].id, "g1", "a repeated id gets a fresh one");
        assert_eq!(out.entries[0].address, "mc.example.org");
        assert_eq!(out.entries[0].name, "mc.example.org", "no name: the address");
        assert_eq!(out.entries[0].group_id.as_deref(), Some("g1"));
        assert_ne!(out.entries[1].id, "a");
        assert_eq!(out.entries[1].group_id, None, "a missing group means Ungrouped");
        assert!(!out.entries[2].id.is_empty());
        assert_eq!(out.entries[2].group_id, None);
        assert_eq!(out.recent, kept, "saving never changes the recent joins");
    }

    #[test]
    fn normalize_refuses_bad_addresses() {
        for bad in ["", "   ", "has space.org", "tab\t.org"] {
            let data = QuickJoin { entries: vec![entry("a", bad, None)], ..Default::default() };
            assert!(normalize(data, Vec::new()).is_err(), "{bad:?} should be refused");
        }
        let long_group = QuickJoin { groups: vec![Group { id: "g".into(), name: "x".repeat(61), collapsed: false }], ..Default::default() };
        assert!(normalize(long_group, Vec::new()).is_err());
    }

    #[test]
    fn bedrock_entries_have_no_instance_and_icons_are_checked() {
        let mut e = entry("b", "bedrock.example.org", None);
        e.edition = Edition::Bedrock;
        e.instance_id = Some("vanilla".into());
        let mut f = entry("c", "java.example.org", None);
        f.instance_id = Some("vanilla".into());
        f.icon = Some("not base64!".into());
        let out = normalize(QuickJoin { entries: vec![e, f], ..Default::default() }, Vec::new()).unwrap();
        assert_eq!(out.entries[0].instance_id, None);
        assert_eq!(out.entries[1].instance_id.as_deref(), Some("vanilla"));
        assert_eq!(out.entries[1].icon, None);
    }

    #[test]
    fn recent_keeps_one_per_address_newest_first() {
        let mut recent = Vec::new();
        for i in 0..25 {
            push_recent(&mut recent, Recent { address: format!("s{i}.org"), at: i, ..Default::default() });
        }
        assert_eq!(recent.len(), MAX_RECENT);
        assert_eq!(recent[0].address, "s24.org");
        push_recent(&mut recent, Recent { address: "S20.ORG:25565".into(), at: 99, ..Default::default() });
        assert_eq!(recent.len(), MAX_RECENT);
        assert_eq!(recent[0].at, 99);
        assert_eq!(recent.iter().filter(|r| address_key(&r.address) == "s20.org").count(), 1);
    }

    #[test]
    fn saved_servers_merge_across_instances() {
        let mut hidden = server("Hidden", "hidden.org");
        hidden.hidden = Some(true);
        let mut with_icon = server("Hub", "hub.example.org:25565");
        with_icon.icon = Some("AAAA".into());
        let lists = vec![
            ("one".to_string(), vec![server("Hub", "hub.example.org"), server("", "solo.org"), hidden]),
            ("two".to_string(), vec![with_icon, server("Hub again", "HUB.example.org")]),
        ];
        let out = merge_saved(lists);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].address, "hub.example.org");
        assert_eq!(out[0].instances, ["one", "two"]);
        assert_eq!(out[0].icon.as_deref(), Some("AAAA"), "an icon from any instance is used");
        assert_eq!(out[1].name, "solo.org", "no name: the address");
    }

    #[test]
    fn bedrock_links_add_the_default_port() {
        assert_eq!(bedrock_link("My | Realm", "play.example.org"), "minecraft://?addExternalServer=My%20%20Realm|play.example.org:19132");
        assert_eq!(bedrock_link("X", "1.2.3.4:19140"), "minecraft://?addExternalServer=X|1.2.3.4:19140");
    }

    #[tokio::test]
    async fn storage_round_trip_keeps_recent() {
        let dir = std::env::temp_dir().join(format!("resin-quickjoin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("quickjoin.json");
        assert_eq!(load_from(&path).await, QuickJoin::default(), "no file yet: empty");

        // A join wrote this earlier.
        let recent = vec![Recent { address: "hub.example.org".into(), name: "Hub".into(), at: 5, ..Default::default() }];
        write_json(&path, &QuickJoin { recent: recent.clone(), ..Default::default() }).await.unwrap();

        let data = QuickJoin {
            groups: vec![Group { id: String::new(), name: "SMP".into(), collapsed: false }],
            entries: vec![entry("", "smp.example.org", None)],
            recent: Vec::new(),
        };
        let saved = save_to(&path, data).await.unwrap();
        assert_eq!(saved.recent, recent);
        let back = load_from(&path).await;
        assert_eq!(back, saved);
        assert!(!back.groups[0].id.is_empty());

        std::fs::write(&path, "{ broken").unwrap();
        assert_eq!(load_from(&path).await, QuickJoin::default(), "an unreadable file reads as empty");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
