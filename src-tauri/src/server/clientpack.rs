//! Client packs for the server list (relay protocol 8): a hosted modded server's mod set as a Modrinth .mrpack,
//! exported from an instance (the one the server was made from, or another the owner picks) and uploaded to
//! ResinCraft Relay; players install it from Discover → Servers as an instance of its own and join.
//!
//! Mods from Modrinth or CurseForge go in as download links; the owner's own jars go in as files. The relay checks
//! the pack, keeps it while the listing is listed, and serves it at `https://<base domain>/packs/<id>.mrpack`.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use resincraft_relay::proto::ListingPack;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::content::{export, sources};
use crate::core::fsutil::{read_json, write_json};
use crate::core::instances;
use crate::core::paths;
use crate::core::types::ExportOptions;
use crate::err_tr;
use crate::player::accounts;

use super::{listing, packs, relay, store};

/// What the Settings tab shows about a server's client pack.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackInfo {
    /// The server runs mods (a client pack makes sense).
    pub modded: bool,
    /// Instances it could come from: same Minecraft version and loader as the server.
    pub instances: Vec<PackSource>,
    /// The instance the server was made from, when it still exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_instance: Option<String>,
    /// What's on the relay now.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published: Option<ListingPack>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackSource {
    pub id: String,
    pub name: String,
}

pub async fn info(id: &str) -> Result<PackInfo> {
    let meta = store::get(id).await?;
    let modded = meta.kind.modded();
    let mut list = Vec::new();
    if modded {
        for i in instances::list().await.unwrap_or_default() {
            let loader_ok = meta.loader.is_none_or(|l| l == i.loader) && (meta.kind.loader().is_none() || meta.kind.loader() == Some(i.loader));
            if i.mc_version == meta.mc_version && loader_ok {
                list.push(PackSource { id: i.id, name: i.name });
            }
        }
    }
    let default_instance = packs::source_instance(id).await.filter(|(_, _, exists)| *exists).map(|(iid, _, _)| iid);
    let listing_id = meta.relay_name.as_ref().map(|n| format!("rc-{n}"));
    let published = match listing_id {
        Some(lid) => listing::mine().await.ok().and_then(|all| all.into_iter().find(|m| m.status.id == lid)).and_then(|m| m.pack),
        None => None,
    };
    Ok(PackInfo { modded, instances: list, default_instance, published })
}

/// Exports `instance_id` as a .mrpack and uploads it as the server's client pack.
pub async fn publish(id: &str, instance_id: &str) -> Result<ListingPack> {
    let meta = store::get(id).await?;
    let Some(name) = meta.relay_name.clone() else {
        return Err(err_tr!("clientPack.needsRelay", "Start the server once with ResinCraft Relay to get its address, then publish its pack."));
    };
    let inst = instances::get(instance_id).await?;
    let Some(account) = accounts::microsoft_account(meta.relay_account.as_deref()).await else { return Err(relay::needs_microsoft()) };
    let include = export::export_preview(instance_id).await?.into_iter().filter(|e| e.suggested).map(|e| e.name).collect();
    let dir = paths::root().join("tmp");
    tokio::fs::create_dir_all(&dir).await?;
    let file = dir.join(format!("client-pack-{name}.mrpack"));
    let opts = ExportOptions {
        format: "mrpack".into(),
        name: meta.name.clone(),
        version: chrono::Utc::now().format("%Y.%m.%d-%H%M").to_string(),
        author: account.username.clone(),
        summary: format!("Client pack for {}", meta.name),
        include,
        bump_on_export: None,
    };
    export::export_modpack(&inst.id, opts, &file).await?;
    let body = tokio::fs::read(&file).await?;
    let _ = tokio::fs::remove_file(&file).await;
    let (token, base) = relay::session_token(&account).await?;
    let res = crate::core::ctx::http()
        .put(format!("https://{base}/api/packs/rc-{name}"))
        .bearer_auth(token)
        .header("Content-Type", "application/x-modrinth-modpack+zip")
        .timeout(Duration::from_secs(300))
        .body(body)
        .send()
        .await
        .map_err(|e| err_tr!("clientPack.unreachable", "Couldn't upload the pack to ResinCraft Relay ({error}).", error = e.to_string()))?;
    let status = res.status();
    if status.is_success() {
        return Ok(res.json().await?);
    }
    let why = res.text().await.unwrap_or_default();
    Err(match status.as_u16() {
        404 => err_tr!("clientPack.notYet", "ResinCraft Relay doesn't take client packs yet."),
        413 => err_tr!("clientPack.tooBig", "The pack is bigger than ResinCraft Relay allows. Leave out big files (shaders, resource packs) and try again."),
        _ => err_tr!("clientPack.refused", "ResinCraft Relay didn't take the pack: {error}", error = why.chars().take(300).collect::<String>()),
    })
}

pub async fn remove(id: &str) -> Result<()> {
    let meta = store::get(id).await?;
    let Some(name) = meta.relay_name.clone() else { return Ok(()) };
    let Some(account) = accounts::microsoft_account(meta.relay_account.as_deref()).await else { return Err(relay::needs_microsoft()) };
    let (token, base) = relay::session_token(&account).await?;
    let res = crate::core::ctx::http().delete(format!("https://{base}/api/packs/rc-{name}")).bearer_auth(token).timeout(Duration::from_secs(30)).send().await?;
    if res.status().is_success() || res.status().as_u16() == 404 {
        Ok(())
    } else {
        Err(err_tr!("clientPack.refused", "ResinCraft Relay didn't take the pack: {error}", error = res.status().to_string()))
    }
}

// ---------- Players: install a listing's pack ----------

/// Listing id → the instance its pack was installed as (and which pack), so Install & join reuses it.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Installed {
    #[serde(default)]
    packs: HashMap<String, (String, String)>,
}

fn installed_file() -> std::path::PathBuf {
    paths::root().join("server-list-packs.json")
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallReq {
    /// The listing's id.
    pub id: String,
    pub url: String,
    pub sha256: String,
}

/// The instance for a listing's pack: the one installed before while it's the same pack, else a fresh install.
pub async fn install(req: InstallReq) -> Result<String> {
    let mut known: Installed = read_json(&installed_file()).await.unwrap_or_default();
    if let Some((iid, sha)) = known.packs.get(&req.id) {
        if sha.eq_ignore_ascii_case(&req.sha256) && instances::get(iid).await.is_ok() {
            return Ok(iid.clone());
        }
    }
    let Some(s) = relay::settings() else { return Err(err_tr!("serverList.noRelay", "This build has no ResinCraft Relay, so there's no server list.")) };
    // Only the relay's own packs: a listing can't point the launcher at some other download.
    if !req.url.starts_with(&format!("https://{}/packs/", s.domain)) {
        return Err(err_tr!("clientPack.badUrl", "That pack doesn't come from ResinCraft Relay."));
    }
    let bytes = crate::core::ctx::http()
        .get(&req.url)
        .timeout(Duration::from_secs(600))
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| err_tr!("clientPack.download", "Couldn't download the server's pack ({error}).", error = e.to_string()))?
        .bytes()
        .await?;
    let sha = hex::encode(Sha256::digest(&bytes));
    if !sha.eq_ignore_ascii_case(&req.sha256) {
        return Err(err_tr!("clientPack.changed", "The server's pack changed while downloading. Try again."));
    }
    let dir = paths::root().join("tmp");
    tokio::fs::create_dir_all(&dir).await?;
    let file = dir.join(format!("{}.mrpack", req.id.replace(|c: char| !c.is_ascii_alphanumeric() && c != '-', "_")));
    tokio::fs::write(&file, &bytes).await?;
    let inst = sources::import_modpack_file(&file).await;
    let _ = tokio::fs::remove_file(&file).await;
    let inst = inst?;
    known.packs.insert(req.id, (inst.id.clone(), sha));
    write_json(&installed_file(), &known).await?;
    Ok(inst.id)
}
