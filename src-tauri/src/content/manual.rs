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
use crate::core::{instances, paths, settings, shell};

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

/// Asks for a folder to watch too (besides Downloads) and saves it. The updated list; None if cancelled.
pub async fn add_watch_folder() -> Result<Option<Vec<String>>> {
    let title = crate::tr!("manual.watchFolder", "Choose a folder where your browser saves downloads").to_string();
    let Some(dir) = shell::pick_folder(&title).await? else { return Ok(None) };
    let dir = dir.to_string_lossy().to_string();
    let mut list = settings::get().await.watch_folders;
    if !list.iter().any(|d| d.eq_ignore_ascii_case(&dir)) {
        list.push(dir);
    }
    settings::set(json!({ "watchFolders": list.clone() })).await?;
    Ok(Some(list))
}

/// Puts the pending files found in the kept copies, `extra` folders, Downloads or the watch folders into `root`
/// (each under its own folder). Returns them with where each went, and the ones still missing.
async fn place(pending: Vec<PendingDownload>, root: &Path, extra: Vec<PathBuf>) -> Result<(Vec<(PendingDownload, PathBuf)>, Vec<PendingDownload>)> {
    let kept = kept_dir();
    // Kept copies first, then the caller's folders, Downloads and the player's own watch folders.
    let mut dirs = vec![kept.clone()];
    dirs.extend(extra);
    dirs.push(shell::downloads_dir());
    dirs.extend(settings::get().await.watch_folders.iter().map(PathBuf::from).filter(|d| !d.as_os_str().is_empty()));
    let mut listed: Vec<(PathBuf, Vec<String>)> = Vec::new();
    for d in dirs {
        let names = file_names(&d).await;
        listed.push((d, names));
    }

    let (mut placed, mut remaining) = (Vec::new(), Vec::new());
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
            remaining.push(p);
            continue;
        };
        let name = safe_name(&p.file_name)?;
        let dest = safe_join(&safe_join(root, &p.folder)?, name)?;
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::copy(&source, &dest).await?;
        // Keep a copy for next time (the one in Downloads stays where the browser put it).
        if !source.starts_with(&kept) {
            let _ = tokio::fs::create_dir_all(&kept).await;
            let _ = tokio::fs::copy(&source, kept.join(name)).await;
        }
        placed.push((p, dest));
    }
    Ok((placed, remaining))
}

/// Looks for the instance's missing files (kept copies first, then Downloads) and puts the ones it finds in place.
pub async fn collect(instance_id: &str) -> Result<Collected> {
    let inst = instances::get(instance_id).await?;
    let pending = inst.pending_downloads.clone().unwrap_or_default();
    if pending.is_empty() {
        return Ok(Collected::default());
    }
    let (placed, remaining) = place(pending, &paths::game_dir(instance_id), Vec::new()).await?;
    let mut out = Collected { found: Vec::new(), remaining };
    let mut idx = None;
    for (p, dest) in placed {
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

/// The same for a server (in `dir`): also looks in the game folders of instances of the same modpack, where the
/// player may have downloaded these files already. Files already in place count as found.
pub async fn collect_for_server(dir: &Path, pending: Vec<PendingDownload>, pack: Option<&str>) -> Result<Collected> {
    let mut out = Collected::default();
    let mut missing = Vec::new();
    for p in pending {
        let here = safe_join(&safe_join(dir, &p.folder)?, safe_name(&p.file_name)?)?;
        if here.is_file() || here.with_file_name(format!("{}.disabled", p.file_name)).is_file() {
            out.found.push(p.file_name);
        } else {
            missing.push(p);
        }
    }
    if missing.is_empty() {
        return Ok(out);
    }
    let mut extra = Vec::new();
    if let Some(pack) = pack.filter(|p| !p.is_empty()) {
        for inst in instances::list().await.unwrap_or_default() {
            if inst.modpack.as_ref().and_then(|m| m.project_id.as_deref()) == Some(pack) {
                let game = paths::game_dir(&inst.id);
                for folder in missing.iter().map(|p| p.folder.clone()).collect::<std::collections::BTreeSet<_>>() {
                    extra.push(game.join(folder));
                }
            }
        }
    }
    let (placed, remaining) = place(missing, dir, extra).await?;
    out.found.extend(placed.into_iter().map(|(p, _)| p.file_name));
    out.remaining = remaining;
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
    async fn server_files_already_in_place_count_as_found() {
        let dir = std::env::temp_dir().join(format!("resincraft-manual-srv-{}", std::process::id()));
        tokio::fs::create_dir_all(dir.join("mods")).await.unwrap();
        tokio::fs::write(dir.join("mods/species-3.5.jar"), b"x").await.unwrap();
        tokio::fs::write(dir.join("mods/off.jar.disabled"), b"x").await.unwrap();
        let p = |f: &str| PendingDownload { title: f.into(), file_name: f.into(), folder: "mods".into(), ..Default::default() };
        let got = collect_for_server(&dir, vec![p("species-3.5.jar"), p("off.jar")], None).await.unwrap();
        assert_eq!(got.found, ["species-3.5.jar", "off.jar"]);
        assert!(got.remaining.is_empty());
        let _ = tokio::fs::remove_dir_all(&dir).await;
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
