//! The one-click multi-core setup on servers. Mod servers get the mods from content/multicore.rs. Plugins can't make
//! the tick multi-core, so plugin servers can only move to Folia: a change of server software that only happens on an
//! explicit request, after a world backup, and that can be taken back.

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::content::multicore::{items_for, load_record, nothing_to_offer, remember, save_record, scan, to_remove, Plan, PlanItem, Target};
use crate::core::events::{list_changed, Task};
use crate::core::fsutil::{read_json, write_json};
use crate::core::shell;
use crate::core::types::{LoaderType, ServerKind, ServerLaunch, ServerMeta};

use super::{installers, plugins, properties, runtime, store};

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerPlan {
    #[serde(flatten)]
    pub plan: Plan,
    /// Offered on plugin servers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folia: Option<FoliaOffer>,
    /// On a server that moved to Folia: the way back.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moved: Option<FoliaMove>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FoliaOffer {
    /// Folia has a build for this Minecraft version.
    pub available: bool,
    /// Plugins that say they run on Folia.
    pub ready: Vec<String>,
    /// Plugins that don't, and so won't load there.
    pub blocked: Vec<String>,
}

/// `.resincraft/folia-move.json`: how the server ran before the move.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FoliaMove {
    pub kind: ServerKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loader: Option<LoaderType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loader_version: Option<String>,
    pub launch: ServerLaunch,
    /// The world backup made first (None: the server had no world yet).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<String>,
}

fn record_file(id: &str) -> Result<PathBuf> {
    Ok(store::dir(id)?.join(".resincraft").join("multicore.json"))
}

fn move_file(id: &str) -> Result<PathBuf> {
    Ok(store::dir(id)?.join(".resincraft").join("folia-move.json"))
}

async fn folia_move(id: &str) -> Result<Option<FoliaMove>> {
    Ok(read_json(&move_file(id)?).await)
}

async fn listed(id: &str) -> Result<Vec<(String, String, String)>> {
    let items = plugins::list(id, None).await?;
    Ok(items.into_iter().map(|a| a.item).map(|i| (i.title.clone().unwrap_or_else(|| i.filename.clone()), i)).map(|(t, i)| (i.filename, i.project_id.unwrap_or_default(), t)).collect())
}

/// What the kind of server allows, when it isn't a plain mod server.
fn reason_for(kind: ServerKind) -> Option<&'static str> {
    match kind {
        ServerKind::Fabric | ServerKind::Quilt | ServerKind::Forge | ServerKind::Neoforge => None,
        ServerKind::Vanilla => Some("vanilla"),
        ServerKind::Folia => Some("folia"),
        k if k.bedrock() => Some("bedrock"),
        // C2ME and Lithium patch the code a hybrid has already rewritten for Bukkit.
        k if k.hybrid() => Some("hybrid"),
        k if k.bukkit() => Some("plugins"),
        // Proxies, Sponge and servers written from scratch run no Java mods.
        _ => Some("noJavaMods"),
    }
}

async fn target_of(meta: &ServerMeta) -> Target {
    Target {
        loaders: plugins::mr_loaders(meta).into_iter().map(str::to_string).collect(),
        mc: meta.mc_version.clone(),
        server: true,
        fabric_loader: (meta.kind == ServerKind::Fabric).then(|| meta.loader_version.clone()).flatten(),
        java: installers::required_java(&meta.mc_version).await.ok(),
        optifine: false,
    }
}

pub async fn plan(id: &str) -> Result<ServerPlan> {
    let meta = store::get(id).await?;
    let mut out = ServerPlan::default();
    let record = load_record(&record_file(id)?).await;
    if !record.added.is_empty() {
        out.plan.undo = to_remove(&record, &listed(id).await?).into_iter().map(|r| r.1).collect();
    }
    out.moved = folia_move(id).await?;
    out.plan.reason = reason_for(meta.kind);
    match out.plan.reason {
        None => {
            let known: Vec<(String, String)> = listed(id).await?.into_iter().map(|(f, p, _)| (f, p)).collect();
            let installed = scan(&store::dir(id)?.join("mods"), known).await;
            out.plan.items = items_for(&target_of(&meta).await, &installed).await?;
            out.plan.reason = nothing_to_offer(&out.plan.items);
        }
        Some("plugins") => {
            let versions = installers::software_versions(ServerKind::Folia).await?;
            let (ready, blocked) = plugins::folia_support(id).await?;
            out.folia = Some(FoliaOffer { available: versions.contains(&meta.mc_version), ready, blocked });
        }
        _ => {}
    }
    Ok(out)
}

pub async fn apply(id: &str) -> Result<ServerPlan> {
    let current = plan(id).await?;
    let picks: Vec<&PlanItem> = current.plan.items.iter().filter(|i| i.state == "install").collect();
    let before = listed(id).await?;
    let mut failed = None;
    for item in &picks {
        let (Some(project), Some(version)) = (&item.project_id, &item.version_id) else { continue };
        if let Err(e) = plugins::install(id, project, Some(version.clone()), None).await {
            failed = Some(e);
            break;
        }
    }
    let file = record_file(id)?;
    let mut record = load_record(&file).await;
    let roots: HashSet<String> = picks.iter().filter_map(|i| i.project_id.clone()).collect();
    remember(&mut record, &before, &listed(id).await?, &roots);
    save_record(&file, &record).await?;
    match failed {
        Some(e) => Err(e),
        None => plan(id).await,
    }
}

/// Queued like any other removal while the server runs: `plugins::remove` takes them away when it stops.
pub async fn undo(id: &str) -> Result<ServerPlan> {
    let file = record_file(id)?;
    let record = load_record(&file).await;
    for (name, _) in to_remove(&record, &listed(id).await?) {
        plugins::remove(id, &name, None).await?;
    }
    let _ = tokio::fs::remove_file(&file).await;
    plan(id).await
}

async fn stopped(id: &str, meta: &ServerMeta) -> Result<()> {
    if runtime::is_running(id) {
        bail!("Stop {} first: its server software can only change while it's off.", meta.name);
    }
    Ok(())
}

/// Moves a plugin server to Folia. `confirmed` is the player's answer to the warning; without it nothing happens.
pub async fn move_to_folia(id: &str, confirmed: bool) -> Result<ServerPlan> {
    let meta = store::get(id).await?;
    if !confirmed {
        bail!("Moving {} to Folia needs your confirmation.", meta.name);
    }
    if reason_for(meta.kind) != Some("plugins") {
        bail!("Only Paper, Purpur, Leaf and Spigot servers can move to Folia.");
    }
    stopped(id, &meta).await?;
    if !installers::software_versions(ServerKind::Folia).await?.contains(&meta.mc_version) {
        bail!("Folia isn't available for Minecraft {}.", meta.mc_version);
    }
    let dir = store::dir(id)?;
    let level = properties::get_value(&dir, "level-name").await.filter(|l| !l.trim().is_empty()).unwrap_or_else(|| "world".into());
    // Stops here if the backup fails: the move never starts without one.
    let backup = if dir.join(level.trim()).is_dir() { Some(runtime::backup_as(id, "_before-folia").await?) } else { None };
    let record = FoliaMove { kind: meta.kind, loader: meta.loader, loader_version: meta.loader_version.clone(), launch: meta.launch.clone(), backup };

    // The old jar stays in the folder and server.json only changes once Folia is in place, so a failure here changes nothing.
    let mut moved = meta.clone();
    moved.kind = ServerKind::Folia;
    moved.loader = None;
    moved.loader_version = None;
    let title = format!("Moving {} to Folia", meta.name);
    let folder = dir.clone();
    let moved = Task::run_to_end(title, Some(id.to_string()), |task| async move {
        installers::install(&mut moved, &folder, &task).await?;
        Ok(moved)
    })
    .await?;
    write_json(&move_file(id)?, &record).await?;
    store::save(&moved).await?;
    list_changed("servers");
    plan(id).await
}

/// Takes a server back to the software it had before the move. The world stays as it is: Paper reads Folia's worlds,
/// and the backup from before the move stays in the server's backups.
pub async fn undo_folia(id: &str) -> Result<ServerPlan> {
    let mut meta = store::get(id).await?;
    let Some(before) = folia_move(id).await? else { bail!("{} wasn't moved to Folia by ResinCraft.", meta.name) };
    if meta.kind != ServerKind::Folia {
        bail!("{} isn't running Folia any more.", meta.name);
    }
    stopped(id, &meta).await?;
    let dir = store::dir(id)?;
    let folia_jar = meta.launch.jar.clone();
    meta.kind = before.kind;
    meta.loader = before.loader;
    meta.loader_version = before.loader_version;
    meta.launch = before.launch;
    // Its jar was left in place; a Reinstall since may have taken it.
    if !meta.launch.jar.as_ref().is_some_and(|j| dir.join(j).is_file()) {
        let (folder, mut again) = (dir.clone(), meta.clone());
        meta = Task::run_to_end(format!("Reinstalling {}", meta.kind.label()), Some(id.to_string()), |task| async move {
            installers::install(&mut again, &folder, &task).await?;
            Ok(again)
        })
        .await?;
    }
    store::save(&meta).await?;
    tokio::fs::remove_file(move_file(id)?).await?;
    if let Some(jar) = folia_jar.filter(|j| dir.join(j).is_file() && meta.launch.jar.as_ref() != Some(j)) {
        let _ = shell::trash(&dir.join(jar)).await;
    }
    list_changed("servers");
    plan(id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_mod_servers_get_the_mods() {
        for k in [ServerKind::Fabric, ServerKind::Quilt, ServerKind::Forge, ServerKind::Neoforge] {
            assert_eq!(reason_for(k), None);
        }
        for k in [ServerKind::Paper, ServerKind::Purpur, ServerKind::Leaf, ServerKind::Spigot] {
            assert_eq!(reason_for(k), Some("plugins"));
        }
        assert_eq!(reason_for(ServerKind::Folia), Some("folia"));
        assert_eq!(reason_for(ServerKind::Vanilla), Some("vanilla"));
        assert_eq!(reason_for(ServerKind::Arclight), Some("hybrid"));
        assert_eq!(reason_for(ServerKind::Bedrock), Some("bedrock"));
        assert_eq!(reason_for(ServerKind::Nukkit), Some("bedrock"));
        assert_eq!(reason_for(ServerKind::Mohist), Some("hybrid"));
        assert_eq!(reason_for(ServerKind::Pufferfish), Some("plugins"));
        for k in [ServerKind::Velocity, ServerKind::Sponge, ServerKind::Glowstone, ServerKind::Horizon] {
            assert_eq!(reason_for(k), Some("noJavaMods"));
        }
    }

    #[test]
    fn the_way_back_keeps_how_the_server_ran() {
        let m = FoliaMove {
            kind: ServerKind::Paper,
            loader: None,
            loader_version: Some("123".into()),
            launch: ServerLaunch { jar: Some("paper.jar".into()), ..Default::default() },
            backup: None,
        };
        let back: FoliaMove = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!((back.kind, back.loader_version.as_deref(), back.launch.jar.as_deref()), (ServerKind::Paper, Some("123"), Some("paper.jar")));
    }
}
