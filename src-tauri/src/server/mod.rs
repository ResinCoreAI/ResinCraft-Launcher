//! Server hosting (`server:*`): create Minecraft servers (vanilla, Paper/Purpur/Folia/Spigot, Fabric/Quilt/
//! Forge/NeoForge, modpacks, or from an instance), run them with a live console, edit server.properties and
//! the player lists, install plugins and mods, back up worlds, and open the port on the router so friends
//! can join. Servers live in `<dataDir>/servers/<id>/`.

pub mod create;
pub mod installers;
pub mod network;
pub mod playit;
pub mod plugins;
pub mod properties;
pub mod runtime;
pub mod store;
pub mod tunnel;

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::core::router::{done, json};
use crate::core::types::{NewServer, PropEntry, ServerKind};
use crate::core::{shell, Args};

pub async fn handle(channel: &str, args: Args) -> Result<Value> {
    match channel {
        "server:list" => {
            let mut out = Vec::new();
            for m in store::list().await? {
                out.push(store::info(m).await);
            }
            json(out)
        }
        "server:get" => json(store::info(store::get(&args.get::<String>(0)?).await?).await),
        "server:versions" => json(installers::software_versions(args.get::<ServerKind>(0)?).await?),
        "server:create" => json(create::create(args.get::<NewServer>(0)?).await?),
        "server:update" => {
            let id: String = args.get(0)?;
            let patch = args.raw(1);
            let public = patch.get("public").and_then(Value::as_bool);
            let before = store::get(&id).await?;
            let meta = store::update(&id, patch).await?;
            if let Some(p) = public {
                runtime::set_public(&id, p).await;
            }
            if (meta.tunnel, &meta.relay_host, &meta.relay_secret) != (before.tunnel, &before.relay_host, &before.relay_secret) {
                runtime::refresh_internet(&id).await;
            }
            json(store::info(meta).await)
        }
        "server:remove" => {
            let id: String = args.get(0)?;
            if runtime::is_running(&id) {
                bail!("Stop the server before deleting it");
            }
            store::get(&id).await?;
            runtime::forget_link(&id);
            shell::trash(&store::dir(&id)?).await?;
            done()
        }
        "server:openFolder" => {
            let id: String = args.get(0)?;
            let sub: Option<String> = args.get(1)?;
            let mut dir = store::dir(&id)?;
            if let Some(s) = sub.filter(|s| !s.is_empty()) {
                dir = crate::core::fsutil::safe_join(&dir, &s)?;
                tokio::fs::create_dir_all(&dir).await?;
            }
            shell::open_path(&dir)?;
            done()
        }
        "server:acceptEula" => {
            store::accept_eula(&args.get::<String>(0)?).await?;
            done()
        }
        "server:start" => {
            runtime::start(&args.get::<String>(0)?).await?;
            done()
        }
        "server:stop" => {
            runtime::stop(&args.get::<String>(0)?)?;
            done()
        }
        "server:kill" => {
            runtime::kill(&args.get::<String>(0)?);
            done()
        }
        "server:command" => {
            runtime::command(&args.get::<String>(0)?, &args.get::<String>(1)?)?;
            done()
        }
        "server:log" => json(runtime::log(&args.get::<String>(0)?)),
        "server:launchArgs" => json(runtime::launch_args(&store::get(&args.get::<String>(0)?).await?)?),
        "server:properties" => json(properties::read(&store::dir(&args.get::<String>(0)?)?).await),
        "server:setProperties" => {
            properties::write_all(&args.get::<String>(0)?, &args.get::<Vec<PropEntry>>(1)?).await?;
            done()
        }
        "server:playerList" => json(properties::list_entries(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        "server:playerAdd" => {
            properties::add(&args.get::<String>(0)?, &args.get::<String>(1)?, &args.get::<String>(2)?).await?;
            done()
        }
        "server:playerRemove" => {
            properties::remove(&args.get::<String>(0)?, &args.get::<String>(1)?, &args.get::<String>(2)?).await?;
            done()
        }
        "server:backup" => json(runtime::backup(&args.get::<String>(0)?).await?),
        "server:addonSources" => json(plugins::sources(store::get(&args.get::<String>(0)?).await?.kind)),
        "server:addons" => json(plugins::list(&args.get::<String>(0)?).await?),
        "server:addonSearch" => {
            let (id, query, source): (String, String, String) = (args.get(0)?, args.get::<Option<String>>(1)?.unwrap_or_default(), args.get(2)?);
            let offset: Option<usize> = args.get(3)?;
            json(plugins::search(&id, &query, &source, offset.unwrap_or(0)).await?)
        }
        "server:addonVersions" => json(plugins::versions(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        "server:addonInstall" => {
            plugins::install(&args.get::<String>(0)?, &args.get::<String>(1)?, args.get(2)?).await?;
            done()
        }
        "server:addonToggle" => {
            plugins::toggle(&args.get::<String>(0)?, &args.get::<String>(1)?).await?;
            done()
        }
        "server:addonRemove" => {
            plugins::remove(&args.get::<String>(0)?, &args.get::<String>(1)?).await?;
            done()
        }
        "server:network" => json(json!({ "lanIp": network::lan_ip().map(|ip| ip.to_string()) })),
        "server:play" => {
            let (id, instance): (String, String) = (args.get(0)?, args.get(1)?);
            let meta = store::get(&id).await?;
            crate::game::launch(&instance, Some(format!("localhost:{}", meta.port))).await?;
            done()
        }
        _ => bail!("Unknown request: {channel}"),
    }
}
