//! Files whose authors only allow downloads from their own website (`Instance.pending_downloads`). The player
//! downloads each one in their own browser; ResinCraft then finds it in the Downloads folder, checks it's the
//! right file, and puts it where it belongs. A copy is kept, so another instance or modpack that needs the same
//! file gets it without another download. ResinCraft itself never downloads these files.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;
use serde_json::json;

use super::index::{index_key, load_index, safe_name, save_index, IndexEntry};
use crate::core::fsutil::safe_join;
use crate::core::net::{hash_file, HashAlgo};
use crate::core::types::{ContentKind, PendingDownload};
use crate::core::{instances, paths, shell};

/// What one look found.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Collected {
    /// File names put into the instance just now.
    pub found: Vec<String>,
    /// Still missing.
    pub remaining: Vec<PendingDownload>,
}

/// Where ResinCraft keeps the files players downloaded by hand.
fn kept_dir() -> PathBuf {
    paths::cache().join("manual-downloads")
}

/// `wanted` itself, or the name a browser gives a second copy: "name (1).jar".
fn same_file_name(found: &str, wanted: &str) -> bool {
    if found == wanted {
        return true;
    }
    let (stem, ext) = match wanted.rsplit_once('.') {
        Some((s, e)) => (s, format!(".{e}")),
        None => (wanted, String::new()),
    };
    let Some(rest) = found.strip_prefix(stem).and_then(|r| r.strip_suffix(ext.as_str())) else { return false };
    rest.strip_prefix(" (").and_then(|r| r.strip_suffix(')')).is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Size and SHA-1 match when the source told us them (a half-written download never does).
async fn is_the_file(path: &Path, p: &PendingDownload) -> bool {
    let Ok(meta) = tokio::fs::metadata(path).await else { return false };
    if !meta.is_file() || meta.len() == 0 || p.size.is_some_and(|s| s != meta.len()) {
        return false;
    }
    match &p.sha1 {
        Some(want) => hash_file(path, HashAlgo::Sha1).await.is_ok_and(|got| got.eq_ignore_ascii_case(want)),
        None => true,
    }
}

async fn file_names(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(mut rd) = tokio::fs::read_dir(dir).await {
        while let Ok(Some(e)) = rd.next_entry().await {
            if let Some(name) = e.file_name().to_str() {
                out.push(name.to_string());
            }
        }
    }
    out
}

fn kind_of_folder(folder: &str) -> ContentKind {
    match folder {
        "resourcepacks" => ContentKind::Resourcepack,
        "shaderpacks" => ContentKind::Shader,
        _ => ContentKind::Mod,
    }
}

/// Looks for the instance's missing files (kept copies first, then Downloads) and puts the ones it finds in place.
pub async fn collect(instance_id: &str) -> Result<Collected> {
    let inst = instances::get(instance_id).await?;
    let pending = inst.pending_downloads.clone().unwrap_or_default();
    if pending.is_empty() {
        return Ok(Collected::default());
    }
    let game = paths::game_dir(instance_id);
    let kept = kept_dir();
    let dirs = [kept.clone(), shell::downloads_dir()];
    let listed: Vec<(PathBuf, Vec<String>)> = {
        let mut v = Vec::new();
        for d in dirs {
            let names = file_names(&d).await;
            v.push((d, names));
        }
        v
    };

    let mut out = Collected::default();
    let mut idx = None;
    for p in pending {
        let mut source = None;
        'search: for (dir, names) in &listed {
            for name in names.iter().filter(|n| same_file_name(n, &p.file_name)) {
                let path = dir.join(name);
                if is_the_file(&path, &p).await {
                    source = Some(path);
                    break 'search;
                }
            }
        }
        let Some(source) = source else {
            out.remaining.push(p);
            continue;
        };
        let name = safe_name(&p.file_name)?;
        let dest = safe_join(&safe_join(&game, &p.folder)?, name)?;
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::copy(&source, &dest).await?;
        // Keep a copy for next time (the one in Downloads stays where the browser put it).
        if !source.starts_with(&kept) {
            let _ = tokio::fs::create_dir_all(&kept).await;
            let _ = tokio::fs::copy(&source, kept.join(name)).await;
        }
        // Tracked like any CurseForge file, so update checks and the content list know it.
        if let (Some(project_id), Some(version_id)) = (&p.project_id, &p.version_id) {
            let idx = match idx.as_mut() {
                Some(i) => i,
                None => idx.insert(load_index(instance_id).await),
            };
            let sha1 = match &p.sha1 {
                Some(s) => s.clone(),
                None => hash_file(&dest, HashAlgo::Sha1).await.unwrap_or_default(),
            };
            idx.insert(
                index_key(kind_of_folder(&p.folder), &p.file_name),
                IndexEntry {
                    project_id: project_id.clone(),
                    version_id: version_id.clone(),
                    version_number: p.file_name.clone(),
                    title: p.title.clone(),
                    icon_url: None,
                    sha1,
                },
            );
        }
        out.found.push(p.file_name);
    }
    if !out.found.is_empty() {
        if let Some(idx) = &idx {
            save_index(instance_id, idx).await?;
        }
        instances::update(instance_id, json!({ "pendingDownloads": out.remaining })).await?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_browser_copies_only() {
        let want = "species-3.5.jar";
        assert!(same_file_name("species-3.5.jar", want));
        assert!(same_file_name("species-3.5 (1).jar", want));
        assert!(same_file_name("species-3.5 (12).jar", want));
        for bad in ["species-3.5.jar.crdownload", "species-3.5.jar.part", "species-3.6.jar", "species-3.5 (a).jar", "species-3.5 ().jar", "xspecies-3.5.jar"] {
            assert!(!same_file_name(bad, want), "{bad}");
        }
        assert!(same_file_name("TFC_Ambiental_Refreshed_1.18-1.21.1 (2).zip", "TFC_Ambiental_Refreshed_1.18-1.21.1.zip"));
    }

    #[tokio::test]
    async fn checks_size_and_sha1() {
        let dir = std::env::temp_dir().join(format!("resincraft-manual-{}", std::process::id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let f = dir.join("a.jar");
        tokio::fs::write(&f, b"hello").await.unwrap();
        let p = |size, sha1: Option<&str>| PendingDownload {
            title: "A".into(),
            file_name: "a.jar".into(),
            url: String::new(),
            folder: "mods".into(),
            size,
            sha1: sha1.map(str::to_string),
            ..Default::default()
        };
        assert!(is_the_file(&f, &p(None, None)).await);
        assert!(is_the_file(&f, &p(Some(5), Some("aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d"))).await);
        assert!(!is_the_file(&f, &p(Some(6), None)).await);
        assert!(!is_the_file(&f, &p(Some(5), Some("0000000000000000000000000000000000000000"))).await);
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
}
