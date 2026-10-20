//! Looking for newer versions of the modpacks instances and servers were installed from, and for instances that
//! changed since a server was synced from them. The UI asks at start-up and every few hours (Settings → Updates
//! can turn it off); results are cached for a few hours, and sources are asked one pack at a time, each pack once
//! however many instances use it.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{atlauncher, curseforge, ftb, sources, technic};
use crate::core::fsutil::{now_ms, read_json, write_json};
use crate::core::types::{source_of, ContentSource, ModpackRef, ProjectVersion};
use crate::core::{instances, paths, settings};
use crate::server::{packs, store};

/// How long a check's result is reused.
const FRESH_MS: i64 = 3 * 3600 * 1000;
/// Pause between two packs' requests, to go easy on the sources' rate limits.
const SPACING: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackUpdate {
    /// "instance" | "server"
    pub target: String,
    pub id: String,
    pub name: String,
    /// "pack": a newer version of its modpack; "sync": its instance changed since the server was synced.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack_name: Option<String>,
    /// The installed version's id and number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PackUpdates {
    /// When the sources were last asked (ms); 0 = never.
    pub checked_at: i64,
    pub updates: Vec<PackUpdate>,
    /// False when Settings turned the automatic check off.
    pub enabled: bool,
}

fn cache_file() -> std::path::PathBuf {
    paths::cache().join("pack-updates.json")
}

static CHECKING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The newest release of a pack and what the installed version is, from the pack's version list (newest first).
/// None when the installed version is the newest release or newer, or isn't listed.
pub fn newer_release<'a>(list: &'a [ProjectVersion], current: &str) -> Option<(&'a ProjectVersion, Option<&'a ProjectVersion>)> {
    let at = list.iter().position(|v| v.id == current)?;
    let newest = list[..at].iter().find(|v| v.kind == "release")?;
    Some((newest, list.get(at)))
}

/// A pack's version list for the check, fresh from its source; Technic only says which build it recommends.
async fn latest_for(project_id: &str, current: &str) -> Option<(String, String, Option<String>)> {
    match source_of(project_id) {
        ContentSource::Technic => {
            technic::forget_cache(project_id);
            let rec = technic::recommended_build(project_id).await.ok()??;
            let newer = super::index::natural_cmp(&rec, current) == std::cmp::Ordering::Greater;
            newer.then(|| (rec.clone(), rec, Some(current.to_string())))
        }
        source => {
            match source {
                ContentSource::Ftb => ftb::forget_cache(project_id),
                ContentSource::Atlauncher => atlauncher::forget_cache().await,
                ContentSource::Curseforge if !curseforge::available().await => return None,
                _ => {}
            }
            let list = sources::versions(project_id, None).await.ok()?;
            let (newest, cur) = newer_release(&list, current)?;
            Some((newest.id.clone(), newest.version_number.clone(), cur.map(|c| c.version_number.clone())))
        }
    }
}

struct Target {
    target: &'static str,
    id: String,
    name: String,
    pack: ModpackRef,
}

/// Asks the sources (`force`: even when the last check is recent or the setting is off).
pub async fn check(force: bool) -> Result<PackUpdates> {
    let enabled = settings::get().await.auto_check_packs;
    if !enabled && !force {
        return Ok(PackUpdates { enabled, ..Default::default() });
    }
    let _one = CHECKING.lock().await;
    let cached: Option<PackUpdates> = read_json(&cache_file()).await;
    if let Some(c) = cached.filter(|c| !force && now_ms() - c.checked_at < FRESH_MS) {
        return Ok(PackUpdates { enabled, ..still_valid(c).await });
    }

    let mut targets = Vec::new();
    for i in instances::list().await.unwrap_or_default() {
        if let (Some(pack), None) = (i.modpack.clone(), &i.own_pack) {
            targets.push(Target { target: "instance", id: i.id.clone(), name: i.name.clone(), pack });
        }
    }
    let mut updates = Vec::new();
    for s in store::list().await.unwrap_or_default() {
        // A server made from an instance follows that instance, not the pack.
        if packs::source_instance(&s.id).await.is_some() {
            if packs::sync_available(&s.id).await == Some(true) {
                updates.push(PackUpdate {
                    target: "server".into(),
                    id: s.id.clone(),
                    name: s.name.clone(),
                    kind: "sync".into(),
                    pack_name: None,
                    current_id: None,
                    current: None,
                    latest_id: None,
                    latest: None,
                });
            }
            continue;
        }
        if let Some(pack) = s.modpack.clone() {
            targets.push(Target { target: "server", id: s.id.clone(), name: s.name.clone(), pack });
        }
    }

    // Each pack once, one at a time.
    let mut seen: HashMap<(String, String), Option<(String, String, Option<String>)>> = HashMap::new();
    for t in targets {
        let (Some(project), Some(current)) = (t.pack.project_id.clone(), t.pack.version_id.clone()) else { continue };
        if source_of(&project) == ContentSource::Github {
            continue;
        }
        let key = (project.clone(), current.clone());
        if !seen.contains_key(&key) {
            if !seen.is_empty() {
                tokio::time::sleep(SPACING).await;
            }
            seen.insert(key.clone(), latest_for(&project, &current).await);
        }
        if let Some(Some((latest_id, latest, cur))) = seen.get(&key) {
            updates.push(PackUpdate {
                target: t.target.into(),
                id: t.id,
                name: t.name,
                kind: "pack".into(),
                pack_name: Some(t.pack.name.clone()),
                current_id: Some(current),
                current: cur.clone(),
                latest_id: Some(latest_id.clone()),
                latest: Some(latest.clone()),
            });
        }
    }
    let out = PackUpdates { checked_at: now_ms(), updates, enabled };
    let _ = write_json(&cache_file(), &out).await;
    Ok(out)
}

/// A cached result without the entries that no longer hold (switched or synced since, or deleted).
async fn still_valid(mut c: PackUpdates) -> PackUpdates {
    let insts = instances::list().await.unwrap_or_default();
    let servers = store::list().await.unwrap_or_default();
    let mut keep = Vec::new();
    for u in c.updates {
        let installed = match u.target.as_str() {
            "instance" => insts.iter().find(|i| i.id == u.id).map(|i| i.modpack.as_ref().and_then(|m| m.version_id.clone())),
            _ => servers.iter().find(|s| s.id == u.id).map(|s| s.modpack.as_ref().and_then(|m| m.version_id.clone())),
        };
        let ok = match (u.kind.as_str(), installed) {
            (_, None) => false,
            ("sync", Some(_)) => packs::sync_available(&u.id).await == Some(true),
            (_, Some(v)) => v == u.current_id,
        };
        if ok {
            keep.push(u);
        }
    }
    c.updates = keep;
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(id: &str, kind: &str) -> ProjectVersion {
        ProjectVersion {
            id: id.into(),
            name: id.into(),
            version_number: id.into(),
            game_versions: vec![],
            loaders: vec![],
            kind: kind.into(),
            published: String::new(),
            downloads: 0,
        }
    }

    #[test]
    fn newer_release_ignores_betas_and_older() {
        let list = vec![v("5-beta", "beta"), v("4", "release"), v("3", "release"), v("2-beta", "beta")];
        assert_eq!(newer_release(&list, "3").map(|(n, c)| (n.id.as_str(), c.map(|c| c.id.as_str()))), Some(("4", Some("3"))));
        assert!(newer_release(&list, "4").is_none(), "newest release installed");
        assert!(newer_release(&list, "5-beta").is_none(), "a beta newer than every release");
        assert_eq!(newer_release(&list, "2-beta").map(|(n, _)| n.id.as_str()), Some("4"));
        assert!(newer_release(&list, "gone").is_none(), "unlisted version");
    }
}
