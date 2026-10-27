//! Client packs for the server list (relay protocol 8): what players need to join a hosted modded server, made from
//! an instance (the one the server was made from, or another the owner picks). Players install it from Discover →
//! Servers as an instance of its own and join.
//!
//! Published as a pack plan (clientplan.rs): the modpack it came from, then every other file as a link to Modrinth or
//! CurseForge, or as the owner's own file, which stays on the owner's PC and reaches players through the relay
//! (clientfiles.rs). Packs published by older builds are a Modrinth .mrpack; players can still install those.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use resincraft_relay::proto::ListingPack;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::content::sources;
use crate::core::fsutil::{read_json, write_json};
use crate::core::instances;
use crate::core::paths;
use crate::err_tr;
use crate::player::accounts;

use super::{clientfiles, clientplan, listing, packs, relay, store};

const PLAN_FORMAT: u32 = 2;

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

/// What the owner picked to send along besides the mods.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishOpts {
    #[serde(default)]
    pub resourcepacks: Vec<String>,
    #[serde(default)]
    pub shaderpacks: Vec<String>,
}

/// Makes the pack plan for `instance_id` and publishes it as the server's client pack. The owner's own files stay on
/// this PC; players download them through the relay while this launcher is running.
pub async fn publish(id: &str, instance_id: &str, opts: PublishOpts) -> Result<ListingPack> {
    let meta = store::get(id).await?;
    let Some(name) = meta.relay_name.clone() else {
        return Err(err_tr!("clientPack.needsRelay", "Start the server once with ResinCraft Relay to get its address, then publish its pack."));
    };
    let Some(account) = accounts::microsoft_account(meta.relay_account.as_deref()).await else { return Err(relay::needs_microsoft()) };
    let version = chrono::Utc::now().format("%Y.%m.%d-%H%M").to_string();
    let (mut plan, owned) = clientplan::build(instance_id, &opts.resourcepacks, &opts.shaderpacks, &version).await?;
    plan.name = meta.name.chars().take(64).collect();
    let body = serde_json::to_vec(&plan)?;
    let (token, base) = relay::session_token(&account).await?;
    let res = crate::core::ctx::http()
        .put(format!("https://{base}/api/packs/rc-{name}"))
        .bearer_auth(token)
        .header("Content-Type", "application/json")
        .timeout(Duration::from_secs(60))
        .body(body)
        .send()
        .await
        .map_err(|e| err_tr!("clientPack.unreachable", "Couldn't upload the pack to ResinCraft Relay ({error}).", error = e.to_string()))?;
    let status = res.status();
    if status.is_success() {
        clientfiles::set(&format!("rc-{name}"), &account.uuid, owned).await?;
        return Ok(res.json().await?);
    }
    let why = res.text().await.unwrap_or_default();
    Err(match status.as_u16() {
        404 => err_tr!("clientPack.notYet", "ResinCraft Relay doesn't take client packs yet."),
        413 => err_tr!("clientPack.tooBig", "The pack lists more files than ResinCraft Relay allows. Leave some out and try again."),
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
        clientfiles::set(&format!("rc-{name}"), &account.uuid, HashMap::new()).await?;
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
    /// The pack's `format`: 2 for a plan, else a .mrpack.
    #[serde(default)]
    pub format: u32,
}

/// The instance for a listing's pack: the one installed before while it's the same pack, else a fresh install.
pub async fn install(req: InstallReq) -> Result<String> {
    if req.format == PLAN_FORMAT {
        return clientplan::install(&req.id, &req.url).await;
    }
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
