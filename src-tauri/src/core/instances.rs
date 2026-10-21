use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;

use super::fsutil::{now_ms, read_json, slugify, write_json};
use super::paths;
use super::types::{Instance, LoaderType, NewInstance};
use crate::err_tr;

pub const INSTANCE_COLORS: [&str; 8] = ["#b26bff", "#22d3ee", "#39e58c", "#6d7bff", "#f472b6", "#2dd4bf", "#a3e635", "#fb7185"];

fn meta_file(id: &str) -> PathBuf {
    paths::instance_dir(id).join("instance.json")
}

/// A custom icon (picked from a screenshot) is kept next to instance.json. `icon_url` carries it as a data URL,
/// since the webview can't load files from disk (the CSP allows `data:` images).
pub fn icon_file(id: &str) -> PathBuf {
    paths::instance_dir(id).join("icon.png")
}

pub fn banner_file(id: &str) -> PathBuf {
    paths::instance_dir(id).join("banner.jpg")
}

pub fn is_custom_icon(url: &str) -> bool {
    url.starts_with("data:")
}

pub async fn list() -> Result<Vec<Instance>> {
    let root = paths::instances();
    tokio::fs::create_dir_all(&root).await?;
    let mut out = Vec::new();
    let mut dir = tokio::fs::read_dir(&root).await?;
    while let Some(entry) = dir.next_entry().await? {
        if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
            if let Some(inst) = read_json::<Instance>(&entry.path().join("instance.json")).await {
                out.push(inst);
            }
        }
    }
    out.sort_by_key(|i| std::cmp::Reverse(i.last_played.unwrap_or(i.created_at)));
    Ok(out)
}

pub async fn get(id: &str) -> Result<Instance> {
    read_json(&meta_file(id)).await.ok_or_else(|| err_tr!("instance.missing", "That instance no longer exists"))
}

pub async fn save(inst: &Instance) -> Result<()> {
    write_json(&meta_file(&inst.id), inst).await
}

fn random_hex(bytes: usize) -> String {
    uuid::Uuid::new_v4().simple().to_string()[..bytes * 2].to_string()
}

/// Creates an instance folder and metadata. `extra` can fill in fields like icon or modpack info.
pub async fn create(data: NewInstance, extra: impl FnOnce(&mut Instance)) -> Result<Instance> {
    let name = if data.name.trim().is_empty() { format!("Minecraft {}", data.mc_version) } else { data.name.trim().to_string() };
    let id = format!("{}-{}", slugify(&name), random_hex(3));
    let loader_version = match (data.loader, data.loader_version) {
        (LoaderType::Vanilla, _) => None,
        (loader, Some(v)) if !v.is_empty() => Some(crate::game::loaders::normalize_loader_version(loader, &data.mc_version, &v)),
        (loader, _) => Some(crate::game::loaders::latest_loader(loader, &data.mc_version).await?),
    };
    let color = data.color.unwrap_or_else(|| INSTANCE_COLORS[(now_ms() as usize) % INSTANCE_COLORS.len()].to_string());
    let mut inst = Instance {
        id: id.clone(),
        name,
        mc_version: data.mc_version,
        loader: data.loader,
        loader_version,
        color,
        icon_url: None,
        pack_icon_url: None,
        created_at: now_ms(),
        last_played: None,
        playtime: 0,
        memory_mb: None,
        java_path: None,
        jvm_args: None,
        modpack: None,
        pending_downloads: None,
        group: data.group.map(|g| g.trim().to_string()).filter(|g| !g.is_empty()),
        banner_url: None,
        optifine: None,
        own_pack: None,
        env: Vec::new(),
        commands: None,
    };
    extra(&mut inst);
    tokio::fs::create_dir_all(paths::game_dir(&id)).await?;
    save(&inst).await?;
    Ok(inst)
}

/// Merges a partial instance (JSON object from the UI, or built in Rust) and saves. `null` clears a field.
pub async fn update(id: &str, patch: Value) -> Result<Instance> {
    let mut current = serde_json::to_value(get(id).await?)?;
    if let (Value::Object(cur), Value::Object(p)) = (&mut current, patch) {
        for (k, v) in p {
            if k == "id" {
                continue;
            }
            if v.is_null() {
                cur.remove(&k);
            } else {
                cur.insert(k, v);
            }
        }
    }
    let inst: Instance = serde_json::from_value(current)?;
    crate::game::commands::check_env(&inst.env)?;
    save(&inst).await?;
    Ok(inst)
}

/// Shows `url` (a data URL of icon.png) as the instance's icon; a modpack's own icon is kept for [`reset_icon`].
pub async fn set_custom_icon(id: &str, url: String) -> Result<Instance> {
    let mut inst = get(id).await?;
    if let Some(pack) = inst.icon_url.take().filter(|u| !is_custom_icon(u)) {
        inst.pack_icon_url = Some(pack);
    }
    inst.icon_url = Some(url);
    save(&inst).await?;
    Ok(inst)
}

/// Drops the custom icon: back to the modpack's icon, or the generated block art.
pub async fn reset_icon(id: &str) -> Result<Instance> {
    let mut inst = get(id).await?;
    inst.icon_url = inst.pack_icon_url.take();
    save(&inst).await?;
    match tokio::fs::remove_file(icon_file(id)).await {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(inst),
    }
}

pub async fn duplicate(id: &str) -> Result<Instance> {
    let src = get(id).await?;
    let new = NewInstance {
        name: format!("{} (copy)", src.name),
        mc_version: src.mc_version.clone(),
        loader: src.loader,
        loader_version: src.loader_version.clone(),
        color: Some(src.color.clone()),
        group: src.group.clone(),
    };
    let s = src.clone();
    let copy = create(new, move |i| {
        i.memory_mb = s.memory_mb;
        i.java_path = s.java_path;
        i.jvm_args = s.jvm_args;
        i.icon_url = s.icon_url;
        i.pack_icon_url = s.pack_icon_url;
        i.banner_url = s.banner_url;
        i.modpack = s.modpack;
        i.optifine = s.optifine;
        i.own_pack = s.own_pack;
    })
    .await?;
    // Copy everything except the metadata and per-launch natives (worlds, mods, notes, content index…).
    let (from, to) = (paths::instance_dir(id), paths::instance_dir(&copy.id));
    tokio::task::spawn_blocking(move || copy_dir(&from, &to, &["instance.json", "natives"])).await??;
    Ok(copy)
}

/// Recursively copies `from` into `to`, skipping top-level names in `skip` (several files at a time).
pub fn copy_dir(from: &Path, to: &Path, skip: &[&str]) -> Result<()> {
    use std::sync::atomic::{AtomicBool, AtomicU64};
    let jobs = crate::core::fsutil::list_copy(from, to, skip)?;
    match crate::core::fsutil::copy_many(&jobs, &AtomicBool::new(false), &AtomicU64::new(0)).into_iter().next() {
        Some((_, e)) => Err(e.into()),
        None => Ok(()),
    }
}
