//! Per-instance notes: plain Markdown next to instance.json, so they're readable outside the launcher too.

use std::path::PathBuf;

use anyhow::{bail, Result};

use super::files::{instance_root, js_len, write_atomic};

async fn notes_file(id: &str) -> Result<PathBuf> {
    Ok(instance_root(id).await?.join("notes.md"))
}

pub async fn get_notes(id: &str) -> Result<String> {
    let file = notes_file(id).await?;
    Ok(tokio::fs::read(&file).await.map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default())
}

pub async fn set_notes(id: &str, text: &str) -> Result<()> {
    if js_len(text) > 1_000_000 {
        bail!("Notes are too long");
    }
    let file = notes_file(id).await?;
    if text.trim().is_empty() {
        match tokio::fs::remove_file(&file).await {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
        Ok(())
    } else {
        write_atomic(&file, text.as_bytes()).await
    }
}
