//! `app:*`, `settings:*` and the instance CRUD part of `instances:*`.

use anyhow::{bail, Result};
use serde_json::Value;

use crate::core::router::{done, json};
use crate::core::types::{AppInfo, NewInstance};
use crate::core::{ctx, instances, paths, settings, shell, Args};
use crate::bail_tr;

pub async fn handle(channel: &str, args: Args) -> Result<Value> {
    match channel {
        "app:info" => json(AppInfo {
            version: env!("CARGO_PKG_VERSION").into(),
            data_dir: paths::root().to_string_lossy().into(),
            platform: match std::env::consts::OS {
                "windows" => "win32".into(),
                "macos" => "darwin".into(),
                other => other.into(),
            },
            total_mem_mb: settings::total_memory_mb(),
            dev: ctx::ctx().dev,
            microsoft: !settings::ms_client_id().await.is_empty(),
            curseforge: !settings::curseforge_key().await.is_empty(),
        }),
        "app:openExternal" => {
            shell::open_url(&args.get::<String>(0)?)?;
            done()
        }
        "app:showItem" => {
            shell::reveal(std::path::Path::new(&args.get::<String>(0)?))?;
            done()
        }
        // Electron recoloured its native title-bar buttons; Tauri draws its own controls in the UI.
        "app:setTitleBar" => done(),

        "settings:get" => json(settings::redacted(settings::get().await)),
        "settings:set" => json(settings::redacted(settings::set(args.raw(0)).await?)),

        "instances:list" => json(instances::list().await?),
        "instances:create" => json(instances::create(args.get::<NewInstance>(0)?, |_| {}).await?),
        "instances:update" => json(instances::update(&args.get::<String>(0)?, args.raw(1)).await?),
        "instances:duplicate" => json(instances::duplicate(&args.get::<String>(0)?).await?),
        "instances:remove" => {
            let id: String = args.get(0)?;
            if crate::game::running_ids().contains(&id) {
                bail_tr!("instance.stopBeforeDelete", "Stop the game before deleting this instance");
            }
            instances::get(&id).await?;
            // Recycle Bin, not a permanent delete: worlds are precious.
            shell::trash(&paths::instance_dir(&id)).await?;
            done()
        }
        "instances:openFolder" => {
            let id: String = args.get(0)?;
            let sub: Option<String> = args.get(1)?;
            let dir = match sub {
                Some(s) => crate::core::fsutil::safe_join(&paths::game_dir(&id), &s)?,
                None => paths::game_dir(&id),
            };
            tokio::fs::create_dir_all(&dir).await?;
            shell::open_path(&dir)?;
            done()
        }
        _ => bail!("Unknown request: {channel}"),
    }
}
