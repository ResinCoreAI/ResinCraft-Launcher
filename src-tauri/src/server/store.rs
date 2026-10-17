//! Hosted servers on disk: `<dataDir>/servers/<id>/` is the server's own folder (its working directory),
//! with ResinCraft's metadata in `server.json` next to the server files.

use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};
use serde_json::Value;

use crate::core::fsutil::{now_ms, read_json, safe_join, slugify, write_json};
use crate::core::paths;
use crate::core::types::{ServerInfo, ServerMeta};

use super::runtime;

pub fn root() -> PathBuf {
    paths::root().join("servers")
}

/// A server's folder. Ids come from the UI, so only a single plain folder name is accepted.
pub fn dir(id: &str) -> Result<PathBuf> {
    if id.is_empty() || id.starts_with('.') || id.contains(['/', '\\', ':']) {
        bail!("That server no longer exists");
    }
    safe_join(&root(), id)
}

fn meta_file(id: &str) -> Result<PathBuf> {
    Ok(dir(id)?.join("server.json"))
}

pub async fn get(id: &str) -> Result<ServerMeta> {
    read_json(&meta_file(id)?).await.ok_or_else(|| anyhow!("That server no longer exists"))
}

pub async fn save(meta: &ServerMeta) -> Result<()> {
    write_json(&meta_file(&meta.id)?, meta).await
}

pub async fn list() -> Result<Vec<ServerMeta>> {
    let root = root();
    tokio::fs::create_dir_all(&root).await?;
    let mut out = Vec::new();
    let mut rd = tokio::fs::read_dir(&root).await?;
    while let Some(e) = rd.next_entry().await? {
        if e.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
            if let Some(m) = read_json::<ServerMeta>(&e.path().join("server.json")).await {
                out.push(m);
            }
        }
    }
    out.sort_by_key(|m| std::cmp::Reverse(m.last_started.unwrap_or(m.created_at)));
    Ok(out)
}

/// A new, unused folder name for a server called `name`.
pub fn new_id(name: &str) -> String {
    let hex = uuid::Uuid::new_v4().simple().to_string();
    let base = slugify(name);
    let base = if base == "instance" { "server".to_string() } else { base };
    format!("{base}-{}", &hex[..6])
}

/// Applies the settings the UI may change. Unknown keys are ignored.
pub async fn update(id: &str, patch: Value) -> Result<ServerMeta> {
    let mut m = get(id).await?;
    let Value::Object(p) = patch else { bail!("Invalid server settings") };
    let text = |v: &Value| v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    for (k, v) in p {
        match k.as_str() {
            "name" => {
                if let Some(n) = text(&v) {
                    m.name = n;
                }
            }
            "memoryMb" => {
                let mb = v.as_u64().ok_or_else(|| anyhow!("Invalid memory amount"))?;
                if mb < 512 {
                    bail!("A server needs at least 512 MB of memory");
                }
                m.memory_mb = mb as u32;
            }
            "port" => {
                let port = v.as_u64().filter(|p| (1..=65535).contains(p)).ok_or_else(|| anyhow!("The port must be a number from 1 to 65535"))?;
                m.port = port as u16;
            }
            "javaPath" => m.java_path = text(&v),
            "jvmArgs" => m.jvm_args = text(&v),
            "public" => m.public = v.as_bool().unwrap_or(m.public),
            "optimizedFlags" => m.optimized_flags = v.as_bool().unwrap_or(m.optimized_flags),
            "tunnel" => m.tunnel = serde_json::from_value(v).map_err(|_| anyhow!("Unknown way to share the server"))?,
            "relayHost" => m.relay_host = text(&v),
            "relaySecret" => m.relay_secret = text(&v),
            _ => {}
        }
    }
    save(&m).await?;
    Ok(m)
}

/// True once the player agreed to the Minecraft EULA for this server (eula.txt says so).
pub async fn eula_accepted(id: &str) -> bool {
    let Ok(dir) = dir(id) else { return false };
    let text = tokio::fs::read_to_string(dir.join("eula.txt")).await.unwrap_or_default();
    text.lines().any(|l| l.trim().replace(' ', "").eq_ignore_ascii_case("eula=true"))
}

/// Writes eula.txt. Only called after the player ticked "I agree to the Minecraft EULA".
pub async fn accept_eula(id: &str) -> Result<()> {
    get(id).await?;
    let date = chrono::Local::now().format("%a %b %d %H:%M:%S %Y");
    let text = format!(
        "#By changing the setting below to TRUE you are indicating your agreement to our EULA (https://aka.ms/MinecraftEULA).\n#Agreed in ResinCraft Launcher on {date}\neula=true\n"
    );
    tokio::fs::write(dir(id)?.join("eula.txt"), text).await?;
    Ok(())
}

pub async fn info(meta: ServerMeta) -> ServerInfo {
    let id = meta.id.clone();
    ServerInfo { eula_accepted: eula_accepted(&id).await, state: runtime::state(&id), players: runtime::players(&id), net: runtime::net(&id), meta }
}

pub async fn touch_started(id: &str) {
    if let Ok(mut m) = get(id).await {
        m.last_started = Some(now_ms());
        let _ = save(&m).await;
    }
}
