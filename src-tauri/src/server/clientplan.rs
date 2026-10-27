//! Client packs as a plan (format 2): what players need for a hosted server, without shipping a big pack file.
//!
//! - A server made from a modpack: players install that modpack from its own site (same version), then only what the
//!   owner added or changed.
//! - Otherwise: an instance with the server's Minecraft version and loader, then the mods.
//! - Resource packs and shaders the owner picks go along too.
//!
//! Each file comes from where it lives: Modrinth (a CDN link), CurseForge (project and file ids, downloaded by each
//! player's launcher), or the owner's own PC, served by the owner's launcher through ResinCraft Relay (clientfiles.rs).
//! The relay keeps only the small plan (`PUT /api/packs/<listing id>`, JSON) and serves it at
//! `https://<base>/packs/<id>.json`.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::content::index::{index_key, load_index};
use crate::content::packswitch::{self, Keep, Side};
use crate::content::{curseforge, modrinth, sources};
use crate::core::events::Task;
use crate::core::fsutil::{read_json, safe_join, write_json};
use crate::core::instances;
use crate::core::net::{download_all, hash_bytes, DownloadItem, Hash, HashAlgo};
use crate::core::paths;
use crate::core::types::{ContentKind, Instance, LoaderType, NewInstance};
use crate::{err_tr, tr};

use super::relay;

/// Where one file of the plan comes from.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum From {
    /// A Modrinth CDN link.
    Modrinth(String),
    /// CurseForge project and file ids (each player's launcher downloads it from CurseForge).
    Curseforge(CfRef),
    /// The owner's own file, served by their launcher through the relay.
    Owner(bool),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CfRef {
    pub project: u64,
    pub file: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanFile {
    /// Relative to the game directory: mods/, resourcepacks/ or shaderpacks/.
    pub path: String,
    pub sha1: String,
    pub size: u64,
    pub from: From,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanModpack {
    /// "modrinth", "curseforge", "ftb", "atlauncher", "technic".
    pub source: String,
    /// The site's own id (no ResinCraft prefix).
    pub project_id: String,
    pub version_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Plan {
    pub format: u32,
    pub name: String,
    pub version: String,
    pub mc_version: String,
    pub loader: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loader_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modpack: Option<PlanModpack>,
    #[serde(default)]
    pub files: Vec<PlanFile>,
    /// Files of the modpack the owner took out (players delete them after installing it).
    #[serde(default)]
    pub remove: Vec<String>,
}

/// ResinCraft's project id ("cf:123", "ftb:5"…) → (site, the site's own id).
fn split_project(id: &str) -> (&'static str, String) {
    for (prefix, site) in [("cf:", "curseforge"), ("ftb:", "ftb"), ("atl:", "atlauncher"), ("technic:", "technic")] {
        if let Some(rest) = id.strip_prefix(prefix) {
            return (site, rest.to_string());
        }
    }
    ("modrinth", id.to_string())
}

/// The other way round, for installing.
fn join_project(m: &PlanModpack) -> String {
    match m.source.as_str() {
        "curseforge" => format!("cf:{}", m.project_id),
        "ftb" => format!("ftb:{}", m.project_id),
        "atlauncher" => format!("atl:{}", m.project_id),
        "technic" => format!("technic:{}", m.project_id),
        _ => m.project_id.clone(),
    }
}

fn loader_name(l: LoaderType) -> &'static str {
    match l {
        LoaderType::Fabric => "fabric",
        LoaderType::Quilt => "quilt",
        LoaderType::Forge | LoaderType::Liteloader => "forge",
        LoaderType::Neoforge => "neoforge",
        LoaderType::Vanilla => "vanilla",
    }
}

fn loader_of(name: &str) -> LoaderType {
    match name {
        "fabric" => LoaderType::Fabric,
        "quilt" => LoaderType::Quilt,
        "forge" => LoaderType::Forge,
        "neoforge" => LoaderType::Neoforge,
        _ => LoaderType::Vanilla,
    }
}

/// What the owner can send along: the instance's resource packs and shaders (mods always go).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Choices {
    pub resourcepacks: Vec<String>,
    pub shaderpacks: Vec<String>,
    /// Made from this modpack (players install it from its site).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modpack: Option<String>,
}

fn list_folder(dir: &Path, exts: &[&str]) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<String> = rd
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.ends_with(".disabled") && exts.iter().any(|x| n.to_lowercase().ends_with(x)))
        .collect();
    out.sort();
    out
}

pub async fn choices(instance_id: &str) -> Result<Choices> {
    let inst = instances::get(instance_id).await?;
    let game = paths::game_dir(&inst.id);
    Ok(Choices {
        resourcepacks: list_folder(&game.join("resourcepacks"), &[".zip"]),
        shaderpacks: list_folder(&game.join("shaderpacks"), &[".zip"]),
        modpack: inst.modpack.as_ref().filter(|m| m.version_id.is_some() && m.project_id.is_some()).map(|m| m.name.clone()),
    })
}

/// One file of the instance that goes into the plan.
struct Local {
    rel: String,
    abs: PathBuf,
    sha1: String,
    size: u64,
}

fn read_local(game: &Path, rel: &str) -> Option<Local> {
    let abs = safe_join(game, rel).ok()?;
    let data = std::fs::read(&abs).ok()?;
    Some(Local { rel: rel.to_string(), sha1: hash_bytes(&data, HashAlgo::Sha1), size: data.len() as u64, abs })
}

/// The plan for `instance_id`, and the owner's own files it names (sha1 → path on this PC).
pub async fn build(instance_id: &str, resourcepacks: &[String], shaderpacks: &[String], version: &str) -> Result<(Plan, HashMap<String, PathBuf>)> {
    let inst = instances::get(instance_id).await?;
    let game = paths::game_dir(&inst.id);
    let modpack = inst
        .modpack
        .as_ref()
        .and_then(|m| Some((m.project_id.clone()?, m.version_id.clone()?)))
        .map(|(pid, vid)| {
            let (source, project_id) = split_project(&pid);
            (pid, PlanModpack { source: source.to_string(), project_id, version_id: vid })
        });

    // What the modpack itself installs (path → hash), to send only what differs.
    let base: BTreeMap<String, Option<Hash>> = match &modpack {
        Some((pid, m)) => {
            let work = paths::cache().join("client-plan").join(&inst.id);
            let (root, vid, pid) = (paths::instance_dir(&inst.id), m.version_id.clone(), pid.clone());
            Task::run_to_end(tr!("clientPack.readingPack", "Reading the modpack's files…"), None, |t| async move {
                Ok(packswitch::installed_files(&root, Some(&vid), &pid, &work, &t, Side::Client, &Keep::client()).await.unwrap_or_default())
            })
            .await?
        }
        None => BTreeMap::new(),
    };

    // The instance's mods, and the resource packs and shaders the owner picked.
    let g = game.clone();
    let (rp, sp) = (resourcepacks.to_vec(), shaderpacks.to_vec());
    let locals: Vec<Local> = tokio::task::spawn_blocking(move || {
        let mut names: Vec<String> = list_folder(&g.join("mods"), &[".jar"]).into_iter().map(|n| format!("mods/{n}")).collect();
        names.extend(rp.iter().filter(|n| !n.contains(['/', '\\'])).map(|n| format!("resourcepacks/{n}")));
        names.extend(sp.iter().filter(|n| !n.contains(['/', '\\'])).map(|n| format!("shaderpacks/{n}")));
        names.iter().filter_map(|rel| read_local(&g, rel)).collect()
    })
    .await?;

    let same = |l: &Local| base.get(&l.rel).is_some_and(|h| h.as_ref().is_some_and(|h| h.value.eq_ignore_ascii_case(&l.sha1)));
    let extras: Vec<Local> = locals.into_iter().filter(|l| !same(l)).collect();
    // Mods the owner took out of the pack: players remove them too.
    let present: std::collections::HashSet<String> = list_folder(&game.join("mods"), &[".jar"]).into_iter().map(|n| format!("mods/{n}")).collect();
    let remove: Vec<String> = base.keys().filter(|p| p.starts_with("mods/") && p.ends_with(".jar") && !present.contains(*p)).cloned().collect();

    // Where each extra comes from: Modrinth by hash, CurseForge from what ResinCraft installed, else the owner's PC.
    let found = modrinth::lookup_hashes(&extras.iter().map(|l| l.sha1.clone()).collect::<Vec<_>>()).await.unwrap_or_default();
    let idx = load_index(&inst.id).await;
    let mut owned = HashMap::new();
    let files = extras
        .into_iter()
        .map(|l| {
            let (folder, name) = l.rel.split_once('/').unwrap_or_default();
            let kind = match folder {
                "mods" => ContentKind::Mod,
                "resourcepacks" => ContentKind::Resourcepack,
                _ => ContentKind::Shader,
            };
            let from = if let Some(hit) = found.get(&l.sha1).filter(|h| h.url.starts_with("https://cdn.modrinth.com/")) {
                From::Modrinth(hit.url.clone())
            } else if let Some(e) = idx.get(&index_key(kind, name)).filter(|e| e.project_id.starts_with("cf:") && e.sha1.eq_ignore_ascii_case(&l.sha1)) {
                From::Curseforge(CfRef { project: curseforge::cf_id(&e.project_id), file: e.version_id.parse().unwrap_or(0) })
            } else {
                owned.insert(l.sha1.to_lowercase(), l.abs.clone());
                From::Owner(true)
            };
            PlanFile { path: l.rel, sha1: l.sha1.to_lowercase(), size: l.size, from }
        })
        .collect();

    let plan = Plan {
        format: 2,
        name: inst.name.clone(),
        version: version.to_string(),
        mc_version: inst.mc_version.clone(),
        loader: loader_name(inst.loader).to_string(),
        loader_version: inst.loader_version.clone().filter(|v| !v.is_empty()),
        modpack: modpack.map(|(_, m)| m),
        files,
        remove,
    };
    Ok((plan, owned))
}

// ---------- Players: installing a plan ----------

#[derive(Debug, Default, Serialize, Deserialize)]
struct Installed {
    /// Listing id → (instance id, plan hash).
    #[serde(default)]
    plans: HashMap<String, (String, String)>,
}

fn installed_file() -> PathBuf {
    paths::root().join("server-list-plans.json")
}

/// The instance for a listing's plan: the one installed before while the plan is the same, else a fresh install.
pub async fn install(listing_id: &str, plan_url: &str) -> Result<String> {
    let Some(s) = relay::settings() else { return Err(err_tr!("serverList.noRelay", "This build has no ResinCraft Relay, so there's no server list.")) };
    let base = format!("https://{}/", s.domain);
    if !plan_url.starts_with(&format!("{base}packs/")) {
        return Err(err_tr!("clientPack.badUrl", "That pack doesn't come from ResinCraft Relay."));
    }
    let bytes = crate::core::ctx::http()
        .get(plan_url)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| err_tr!("clientPack.download", "Couldn't download the server's pack ({error}).", error = e.to_string()))?
        .bytes()
        .await?;
    let plan: Plan = serde_json::from_slice(&bytes).map_err(|e| err_tr!("clientPack.badPlan", "The server's pack can't be read ({error}).", error = e.to_string()))?;
    let plan_hash = hash_bytes(&bytes, HashAlgo::Sha1);
    let mut known: Installed = read_json(&installed_file()).await.unwrap_or_default();
    if let Some((iid, h)) = known.plans.get(listing_id) {
        if *h == plan_hash && instances::get(iid).await.is_ok() {
            return Ok(iid.clone());
        }
    }

    // The base: the modpack from its own site, or an instance with the server's version and loader.
    let inst: Instance = match &plan.modpack {
        Some(m) => sources::install_modpack(&join_project(m), Some(m.version_id.clone())).await?,
        None => {
            let new = NewInstance {
                name: plan.name.clone(),
                mc_version: plan.mc_version.clone(),
                loader: loader_of(&plan.loader),
                loader_version: plan.loader_version.clone(),
                color: None,
                group: None,
            };
            instances::create(new, |_| {}).await?
        }
    };
    let game = paths::game_dir(&inst.id);

    // What the owner took out of the modpack.
    for rel in &plan.remove {
        if let Ok(p) = safe_join(&game, rel) {
            let _ = tokio::fs::remove_file(p).await;
        }
    }

    // Modrinth and the owner's own files: straight downloads, checked by SHA-1.
    let listing = listing_id.to_string();
    let items: Vec<DownloadItem> = plan
        .files
        .iter()
        .filter_map(|f| {
            let url = match &f.from {
                From::Modrinth(u) if u.starts_with("https://cdn.modrinth.com/") => u.clone(),
                From::Owner(_) => format!("{base}files/{listing}/{}", f.sha1),
                _ => return None,
            };
            let dest = safe_join(&game, &f.path).ok()?;
            Some(DownloadItem { url, dest, hash: Some(Hash::sha1(f.sha1.clone())), size: Some(f.size), trust_size: false })
        })
        .collect();
    let task_name = tr!("clientPack.installing", "Installing {name}'s files", name = plan.name.clone());
    Task::run_to_end(task_name, Some(inst.id.clone()), |t| async move {
        download_all(items, 4, move |done, all| t.update(tr!("clientPack.files", "Files ({done}/{total})", done = done, total = all), Some(done as f64 / all.max(1) as f64))).await
    })
    .await
    .map_err(|e| err_tr!("clientPack.filesFailed", "Some of the server's files didn't download ({error}). If they come from the owner's PC, the owner may be offline.", error = e.to_string()))?;

    // CurseForge files through CurseForge itself (its rules: no re-hosting).
    for f in plan.files.iter() {
        if let From::Curseforge(cf) = &f.from {
            let kind = match f.path.split('/').next().unwrap_or_default() {
                "mods" => ContentKind::Mod,
                "resourcepacks" => ContentKind::Resourcepack,
                _ => ContentKind::Shader,
            };
            curseforge::install(&inst.id, &format!("cf:{}", cf.project), kind, Some(cf.file.to_string())).await?;
        }
    }

    known.plans.insert(listing_id.to_string(), (inst.id.clone(), plan_hash));
    write_json(&installed_file(), &known).await?;
    Ok(inst.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_round_trip() {
        for id in ["cf:285109", "1KVo5zza", "ftb:23", "atl:SkyFactory", "technic:tekkit"] {
            let (source, project_id) = split_project(id);
            let m = PlanModpack { source: source.into(), project_id, version_id: "1".into() };
            assert_eq!(join_project(&m), id);
        }
    }

    #[test]
    fn plan_wire_shape() {
        let plan = Plan {
            format: 2,
            name: "SMP".into(),
            version: "1".into(),
            mc_version: "1.20.1".into(),
            loader: "forge".into(),
            loader_version: Some("47.4.10".into()),
            modpack: None,
            files: vec![
                PlanFile { path: "mods/a.jar".into(), sha1: "a".repeat(40), size: 1, from: From::Modrinth("https://cdn.modrinth.com/x".into()) },
                PlanFile { path: "mods/b.jar".into(), sha1: "b".repeat(40), size: 2, from: From::Curseforge(CfRef { project: 1, file: 2 }) },
                PlanFile { path: "mods/c.jar".into(), sha1: "c".repeat(40), size: 3, from: From::Owner(true) },
            ],
            remove: vec![],
        };
        let text = serde_json::to_string(&plan).unwrap();
        assert!(text.contains(r#""from":{"modrinth":"https://cdn.modrinth.com/x"}"#), "{text}");
        assert!(text.contains(r#""from":{"curseforge":{"project":1,"file":2}}"#), "{text}");
        assert!(text.contains(r#""from":{"owner":true}"#), "{text}");
        assert_eq!(serde_json::from_str::<Plan>(&text).unwrap(), plan);
    }
}
