//! Content lists: an instance's mods (or resource packs, or shaders) saved to a file, and installed again from one,
//! into the same instance or another. A list is JSON (below), or plain text with one link or Modrinth name per line.
//! Hand-added files have no project to download from, so a list names them but can't bring them back.

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::dupes::{self, ModPick};
use super::{links, sources};
use crate::core::fsutil::slugify;
use crate::core::instances;
use crate::core::shell;
use crate::core::text::error_payload;
use crate::core::types::{source_of, ContentKind, ContentSource, LoaderType};
use crate::{bail_tr, tr};

const FORMAT: &str = "resincraft-content-list";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContentList {
    format: String,
    version: u32,
    kind: ContentKind,
    minecraft: String,
    loader: LoaderType,
    #[serde(default)]
    instance: String,
    items: Vec<ListItem>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListItem {
    title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    version_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(default)]
    file: String,
    /// The project's page, for people reading the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

/// What installing a list did.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListImport {
    pub kind: Option<ContentKind>,
    pub installed: Vec<String>,
    /// Already in the instance.
    pub skipped: Vec<String>,
    /// Added by hand where the list came from: nothing to download them from.
    pub missing: Vec<String>,
    pub failed: Vec<ListFailure>,
}

#[derive(Debug, Serialize)]
pub struct ListFailure {
    pub title: String,
    /// `{message, key?, params?}`, like a rejected engine call.
    pub error: Value,
}

fn source_name(id: &str) -> &'static str {
    match source_of(id) {
        ContentSource::Curseforge => "curseforge",
        ContentSource::Github => "github",
        ContentSource::Ftb => "ftb",
        ContentSource::Atlauncher => "atlauncher",
        ContentSource::Technic => "technic",
        ContentSource::Modrinth => "modrinth",
    }
}

fn page_of(id: &str) -> Option<String> {
    match source_of(id) {
        ContentSource::Modrinth => Some(format!("https://modrinth.com/project/{id}")),
        ContentSource::Curseforge => id.strip_prefix("cf:").map(|n| format!("https://www.curseforge.com/projects/{n}")),
        ContentSource::Github => id.strip_prefix("gh:").map(|r| format!("https://github.com/{r}")),
        _ => None,
    }
}

fn kind_word(kind: ContentKind) -> &'static str {
    match kind {
        ContentKind::Mod => "mods",
        ContentKind::Resourcepack => "resource packs",
        ContentKind::Shader => "shaders",
        ContentKind::Modpack => "modpacks",
    }
}

/// Saves the instance's `kind` content as a list. Returns where, or None if the save dialog was cancelled.
pub async fn export_list(instance_id: &str, kind: ContentKind) -> Result<Option<String>> {
    let inst = instances::get(instance_id).await?;
    let items = sources::list_content(instance_id, kind).await?;
    if items.is_empty() {
        bail_tr!("lists.empty", "{name} has no {kind} to list", name = inst.name, kind = kind_word(kind));
    }
    let list = ContentList {
        format: FORMAT.into(),
        version: 1,
        kind,
        minecraft: inst.mc_version.clone(),
        loader: inst.loader,
        instance: inst.name.clone(),
        items: items
            .iter()
            .map(|i| ListItem {
                title: i.title.clone().unwrap_or_else(|| i.filename.trim_end_matches(".disabled").to_string()),
                source: i.project_id.as_deref().map(|p| source_name(p).to_string()),
                url: i.project_id.as_deref().and_then(page_of),
                project_id: i.project_id.clone(),
                version_id: i.version_id.clone(),
                version: i.version_number.clone(),
                file: i.filename.clone(),
            })
            .collect(),
    };
    let name = format!("{} {}.json", slugify(&inst.name), kind_word(kind).replace(' ', "-"));
    let title = tr!("lists.saveTitle", "Save the list of {kind}", kind = kind_word(kind)).text;
    let Some(path) = shell::save_file(&title, &shell::downloads_dir().join(name), &[("ResinCraft list", &["json"])]).await? else {
        return Ok(None);
    };
    crate::core::fsutil::write_json(&path, &list).await?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// Installs a list into an instance: picks the file when there's no `path`. None if the dialog was cancelled.
pub async fn import_list(instance_id: &str, kind: ContentKind, path: Option<String>) -> Result<Option<ListImport>> {
    let path = match path {
        Some(p) => PathBuf::from(p),
        None => match shell::pick_file(&tr!("lists.openTitle", "Choose a list").text, &[("ResinCraft list or text", &["json", "txt"])]).await? {
            Some(p) => p,
            None => return Ok(None),
        },
    };
    let text = tokio::fs::read_to_string(&path).await?;
    let inst = instances::get(instance_id).await?;
    let present: HashSet<String> = sources::list_content(instance_id, kind).await.unwrap_or_default().into_iter().filter_map(|i| i.project_id).collect();

    // JSON from "Export list".
    if let Ok(list) = serde_json::from_str::<ContentList>(&text) {
        if list.format != FORMAT {
            bail_tr!("lists.unknown", "That file isn't a ResinCraft list");
        }
        let present: HashSet<String> = if list.kind == kind {
            present
        } else {
            sources::list_content(instance_id, list.kind).await.unwrap_or_default().into_iter().filter_map(|i| i.project_id).collect()
        };
        // The same game and loader: the exact versions; otherwise each project's newest that fits.
        let same = list.minecraft == inst.mc_version && list.loader == inst.loader;
        let mut out = ListImport { kind: Some(list.kind), ..Default::default() };
        let mut picks = Vec::new();
        for item in list.items {
            match item.project_id {
                None => out.missing.push(item.title),
                Some(id) if present.contains(&id) => out.skipped.push(item.title),
                Some(id) => picks.push(ModPick {
                    id,
                    kind: list.kind,
                    title: item.title,
                    author: String::new(),
                    slug: String::new(),
                    version_id: item.version_id.filter(|_| same),
                }),
            }
        }
        if !picks.is_empty() {
            let titles: Vec<(String, String)> = picks.iter().map(|p| (p.id.clone(), p.title.clone())).collect();
            let done = dupes::install_many(instance_id, picks, Vec::new()).await?;
            let title_of = |id: &str| titles.iter().find(|(i, _)| i == id).map(|(_, t)| t.clone()).unwrap_or_else(|| id.to_string());
            out.installed = done.installed.iter().map(|id| title_of(id)).collect();
            out.failed = done.failed.into_iter().map(|f| ListFailure { title: title_of(&f.id), error: f.error }).collect();
        }
        return Ok(Some(out));
    }

    // Plain text: one link (or Modrinth name) per line; # starts a comment.
    let mut out = ListImport { kind: Some(kind), ..Default::default() };
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
    if lines.is_empty() {
        bail_tr!("lists.unknown", "That file isn't a ResinCraft list");
    }
    for line in lines {
        let result = if line.starts_with("http://") || line.starts_with("https://") {
            links::install_link(instance_id, line, kind).await
        } else if present.contains(line) {
            out.skipped.push(line.to_string());
            continue;
        } else {
            sources::install(instance_id, line, kind, None).await
        };
        match result {
            Ok(()) => out.installed.push(line.to_string()),
            Err(e) => out.failed.push(ListFailure { title: line.to_string(), error: error_payload(&e) }),
        }
    }
    Ok(Some(out))
}
