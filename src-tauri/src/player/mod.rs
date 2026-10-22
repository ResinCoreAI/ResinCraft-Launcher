//! Accounts (and their Minecraft profiles), the instance page's Worlds / Servers / Screenshots / Notes tabs, Quick
//! join, and Bedrock Edition as installed on Windows.

pub mod accounts;
pub mod bedrock;
pub mod bedrock_store;
pub mod carry;
pub mod files;
pub mod nbt;
pub mod notes;
pub mod profile;
pub mod quickjoin;
pub mod screenshots;
pub mod servers;
pub mod skins;
pub mod worlds;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Result};
use serde_json::Value;

use crate::core::router::{done, json};
use crate::core::types::{OnlineSkin, ServerEntry, SkinUpload};
use crate::core::{shell, Args};

pub async fn handle(channel: &str, args: Args) -> Result<Value> {
    match channel {
        "accounts:list" => json(accounts::list_accounts().await?),
        "accounts:beginMicrosoft" => json(accounts::begin_microsoft_login().await?),
        "accounts:beginMicrosoftWindow" => json(accounts::begin_microsoft_window_login().await?),
        "accounts:cancelMicrosoft" => {
            accounts::cancel_microsoft_login();
            done()
        }
        "accounts:addOffline" => json(accounts::add_offline_account(&args.get::<String>(0)?).await?),
        "accounts:setActive" => {
            accounts::set_active_account(&args.get::<String>(0)?).await?;
            done()
        }
        "accounts:remove" => {
            accounts::remove_account(&args.get::<String>(0)?).await?;
            done()
        }
        "accounts:profile" => json(profile::get_profile(&args.get::<String>(0)?).await?),
        "accounts:setSkin" => json(profile::set_skin(&args.get::<String>(0)?, &args.get::<SkinUpload>(1)?).await?),
        "accounts:resetSkin" => json(profile::reset_skin(&args.get::<String>(0)?).await?),
        "accounts:setCape" => json(profile::set_cape(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        "accounts:hideCape" => json(profile::hide_cape(&args.get::<String>(0)?).await?),
        "accounts:nameAvailable" => json(profile::name_available(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        "accounts:changeName" => json(profile::change_name(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),

        "skins:list" => json(skins::list().await?),
        "skins:importPng" => json(skins::import_png(&args.get::<String>(0)?, &args.get::<String>(1)?, &args.get::<String>(2)?).await?),
        "skins:update" => {
            let (id, name, variant): (String, Option<String>, Option<String>) = (args.get(0)?, args.get(1)?, args.get(2)?);
            json(skins::update(&id, name.as_deref(), variant.as_deref()).await?)
        }
        "skins:remove" => {
            skins::remove(&args.get::<String>(0)?).await?;
            done()
        }
        "skins:apply" => json(skins::apply(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        "skins:lookupPlayer" => json(skins::lookup_player(&args.get::<String>(0)?).await?),
        "skins:search" => json(skins::search(&args.get::<String>(0)?, args.get::<Option<String>>(1)?.as_deref()).await?),
        "skins:saveOnline" => json(skins::save_online(&args.get::<OnlineSkin>(0)?).await?),

        "notes:get" => json(notes::get_notes(&args.get::<String>(0)?).await?),
        "notes:set" => {
            let id: String = args.get(0)?;
            let Value::String(text) = args.raw(1) else { bail!("Notes must be text") };
            notes::set_notes(&id, &text).await?;
            done()
        }

        "worlds:list" => json(worlds::list_worlds(&args.get::<String>(0)?).await?),
        // The most recently played worlds across every instance: [limit] → RecentWorld[].
        "worlds:recent" => json(worlds::recent_worlds(args.get::<Option<usize>>(0)?.unwrap_or(8).min(50)).await?),
        "worlds:rename" => {
            let (id, folder, name): (String, String, Option<String>) = (args.get(0)?, args.get(1)?, args.get(2)?);
            worlds::rename_world(&id, &folder, name.as_deref().unwrap_or_default()).await?;
            done()
        }
        "worlds:backup" => json(worlds::backup_world(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        "worlds:remove" => {
            worlds::remove_world(&args.get::<String>(0)?, &args.get::<String>(1)?).await?;
            done()
        }
        "worlds:play" => {
            let (id, folder): (String, String) = (args.get(0)?, args.get(1)?);
            // Only one of the instance's own worlds (and never a path out of saves/).
            if !worlds::world_path(&id, &folder).await?.join("level.dat").is_file() {
                crate::bail_tr!("worlds.notFound", "{folder} isn't a world in this instance any more.", folder = folder);
            }
            crate::game::launch_as(&id, args.get::<Option<String>>(2)?.as_deref(), Some(crate::game::launch::Target::World(folder))).await?;
            done()
        }
        "worlds:openFolder" => {
            let path = worlds::world_path(&args.get::<String>(0)?, &args.get::<String>(1)?).await?;
            shell::open_path(&path)?;
            done()
        }
        "worlds:import" => {
            let id: String = args.get(0)?;
            let zip_path = match args.get::<Option<String>>(1)?.filter(|p| !p.is_empty()) {
                Some(p) => PathBuf::from(p),
                None => match shell::pick_file("Import a world", &[("World zip", &["zip"])]).await? {
                    Some(p) => p,
                    None => return Ok(Value::Null),
                },
            };
            json(worlds::import_world(&id, &zip_path).await?)
        }

        "servers:list" => json(servers::list_servers(&args.get::<String>(0)?).await?),
        "servers:save" => {
            let id: String = args.get(0)?;
            if !args.raw(1).is_array() {
                bail!("Invalid server list");
            }
            let list: Vec<ServerEntry> = args.get(1)?;
            servers::save_servers(&id, &list).await?;
            done()
        }
        "servers:ping" => {
            let Value::String(address) = args.raw(0) else { bail!("Invalid server address") };
            json(servers::ping_server(&address, Duration::from_millis(5000)).await?)
        }

        "quickjoin:get" => json(quickjoin::get().await),
        "quickjoin:save" => json(quickjoin::save(args.get::<quickjoin::QuickJoin>(0)?).await?),
        "quickjoin:saved" => json(quickjoin::saved().await?),
        "quickjoin:clearRecent" => json(quickjoin::clear_recent().await?),
        "quickjoin:join" => {
            quickjoin::join(args.get::<quickjoin::JoinRequest>(0)?).await?;
            done()
        }

        "screenshots:list" => json(screenshots::list_screenshots(&args.get::<String>(0)?).await?),
        "screenshots:thumbnail" => json(screenshots::screenshot_thumbnail(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        "screenshots:image" => json(screenshots::screenshot_image(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        "screenshots:copy" => {
            screenshots::copy_screenshot(&args.get::<String>(0)?, &args.get::<String>(1)?).await?;
            done()
        }
        "screenshots:show" => {
            screenshots::show_screenshot(&args.get::<String>(0)?, &args.get::<String>(1)?).await?;
            done()
        }
        "screenshots:remove" => {
            screenshots::remove_screenshot(&args.get::<String>(0)?, &args.get::<String>(1)?).await?;
            done()
        }
        "screenshots:setIcon" => json(screenshots::set_as_icon(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),

        _ => bail!("Unknown request: {channel}"),
    }
}
