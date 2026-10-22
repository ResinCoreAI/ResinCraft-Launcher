//! Content and modpacks: Modrinth, CurseForge, FTB, ATLauncher and Technic, plus modpack import & export.
//! Handles every `content:*` channel (argument order as in `ResinApi` in src/shared/types.ts).

pub mod atlauncher;
pub mod curseforge;
pub mod details;
pub mod dupes;
pub mod export;
pub mod ftb;
pub mod github;
pub mod index;
pub mod jarmeta;
pub mod links;
pub mod lists;
pub mod manual;
pub mod modrinth;
pub mod packswitch;
pub mod packupdates;
pub mod sources;
pub mod technic;

use std::path::PathBuf;

use anyhow::{bail, Result};
use serde_json::Value;

use crate::core::router::{done, json};
use crate::core::shell;
use crate::core::types::{ContentKind, ExportOptions, SearchQuery};
use crate::core::Args;

pub async fn handle(channel: &str, args: Args) -> Result<Value> {
    match channel {
        "content:search" => json(sources::search(&args.get::<SearchQuery>(0)?).await?),
        "content:versions" => {
            let instance: Option<String> = args.get(1)?;
            json(sources::versions(&args.get::<String>(0)?, instance.as_deref()).await?)
        }
        "content:details" => json(details::details(&args.get::<String>(0)?).await?),
        "content:versionInfo" => json(details::version_info(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        // The mods in a modpack version: [projectId, versionId] → PackMod[].
        "content:packMods" => json(details::pack_mods(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        "content:install" => {
            sources::install(&args.get::<String>(0)?, &args.get::<String>(1)?, args.get::<ContentKind>(2)?, args.get(3)?).await?;
            done()
        }
        "content:exportList" => json(lists::export_list(&args.get::<String>(0)?, args.get::<ContentKind>(1)?).await?),
        "content:importList" => json(lists::import_list(&args.get::<String>(0)?, args.get::<ContentKind>(1)?, args.get(2)?).await?),
        "content:installFromLink" => {
            links::install_link(&args.get::<String>(0)?, &args.get::<String>(1)?, args.get::<ContentKind>(2)?).await?;
            done()
        }
        "content:duplicates" => json(dupes::check(&args.get::<String>(0)?, args.get(1)?).await?),
        "content:installMany" => json(dupes::install_many(&args.get::<String>(0)?, args.get(1)?, args.get::<Option<_>>(2)?.unwrap_or_default()).await?),
        "content:list" => json(sources::list_content(&args.get::<String>(0)?, args.get(1)?).await?),
        "content:toggle" => {
            modrinth::toggle_content(&args.get::<String>(0)?, args.get(1)?, &args.get::<String>(2)?).await?;
            done()
        }
        "content:remove" => {
            // Recycle Bin, not a permanent delete.
            modrinth::remove_content(&args.get::<String>(0)?, args.get(1)?, &args.get::<String>(2)?).await?;
            done()
        }
        "content:checkUpdates" => json(sources::check_updates(&args.get::<String>(0)?).await?),
        "content:update" => {
            sources::update_content(&args.get::<String>(0)?, &args.get::<String>(1)?).await?;
            done()
        }
        "content:changeVersion" => {
            sources::change_version(&args.get::<String>(0)?, args.get(1)?, &args.get::<String>(2)?, &args.get::<String>(3)?).await?;
            done()
        }
        "content:packUpdates" => json(packupdates::check(args.get::<Option<bool>>(0)?.unwrap_or(false)).await?),
        "content:switchPack" => json(packswitch::switch_version(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        "content:installModpack" => dupes::pack_result(sources::install_modpack(&args.get::<String>(0)?, args.get(1)?).await?),
        "content:importModpack" => {
            let picked = shell::pick_file("Import a modpack", &[("Modrinth or CurseForge modpack", &["mrpack", "zip"])]).await?;
            match picked {
                Some(file) => dupes::pack_result(sources::import_modpack_file(&file).await?),
                None => done(),
            }
        }
        "content:importModpackFile" => dupes::pack_result(sources::import_modpack_file(&PathBuf::from(args.get::<String>(0)?)).await?),
        "content:exportPreview" => json(export::export_preview(&args.get::<String>(0)?).await?),
        "content:exportModpack" => {
            let id: String = args.get(0)?;
            let opts: ExportOptions = args.get(1)?;
            let mrpack = opts.format == "mrpack";
            let ext = if mrpack { "mrpack" } else { "zip" };
            let default = shell::downloads_dir().join(export::pack_file_name(&opts));
            let filter_name = if mrpack { "Modrinth modpack" } else { "CurseForge modpack" };
            match shell::save_file("Export modpack", &default, &[(filter_name, &[ext])]).await? {
                Some(dest) => json(export::export_modpack(&id, opts, &dest).await?),
                None => done(),
            }
        }
        // Exporting several instances at once (Library selection): one folder, a file per instance.
        "content:pickExportFolder" => json(shell::pick_folder("Choose a folder for the modpacks").await?.map(|p| p.to_string_lossy().into_owned())),
        "content:exportModpackTo" => {
            let id: String = args.get(0)?;
            let opts: ExportOptions = args.get(1)?;
            let dest = export::free_path(&PathBuf::from(args.get::<String>(2)?), &export::pack_file_name(&opts));
            json(export::export_modpack(&id, opts, &dest).await?)
        }
        _ => bail!("Unknown request: {channel}"),
    }
}
