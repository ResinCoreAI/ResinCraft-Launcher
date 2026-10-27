//! Server hosting (`server:*`): create Minecraft servers (vanilla, Paper/Purpur/Folia/Spigot, Fabric/Quilt/
//! Forge/NeoForge, modpacks, or from an instance), run them with a live console, edit server.properties and
//! the player lists, install plugins and mods, back up worlds, and open the port on the router so friends
//! can join. Servers live in `<dataDir>/servers/<id>/`.

pub mod antixray;
pub mod cloudflare;
pub mod clientmods;
pub mod create;
pub mod discordbot;
pub mod domain;
pub mod installers;
pub mod network;
pub mod packs;
pub mod playit;
pub mod plugins;
pub mod properties;
pub mod relay;
pub mod requests;
pub mod runtime;
pub mod store;
pub mod tunnel;

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::core::router::{done, json};
use crate::core::types::{NewServer, PropEntry, ServerKind, ServerMeta};
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
        "server:softwareIcons" => json(installers::software_icons().await),
        "server:builds" => json(installers::builds(args.get::<ServerKind>(0)?, &args.get::<String>(1)?).await?),
        "server:create" => json(create::create(args.get::<NewServer>(0)?).await?),
        "server:switchPack" => json(packs::switch_pack(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        "server:origin" => json(packs::origin(&args.get::<String>(0)?).await?),
        "server:syncPreview" => json(packs::sync_preview(&args.get::<String>(0)?).await?),
        "server:syncFromInstance" => json(packs::sync_from_instance(&args.get::<String>(0)?).await?),
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
            if (&meta.join_requests, &meta.name) != (&before.join_requests, &before.name) {
                discordbot::reconcile().await;
                relay::send_join_requests(&id).await;
            }
            if (&meta.domain, meta.dns_auto) != (&before.domain, before.dns_auto) {
                domain::forget(&id);
                runtime::set_domain_ok(&id, None);
                // Cleared: the relay stops routing it (a new domain simply replaces the old one there).
                if let (None, Some(name)) = (&meta.domain, before.relay_name.clone().filter(|_| before.domain.is_some())) {
                    tokio::spawn(relay::domain_detach(id.clone(), name));
                }
                tokio::spawn(domain::on_open(id.clone()));
            }
            json(store::info(meta).await)
        }
        // A picture for the server: [id, data URL | null] (null removes it) → ServerInfo.
        "server:setIcon" => json(store::info(set_icon(&args.get::<String>(0)?, args.get::<Option<String>>(1)?).await?).await),
        "server:remove" => {
            let id: String = args.get(0)?;
            if runtime::is_running(&id) {
                bail!("Stop the server before deleting it");
            }
            store::get(&id).await?;
            runtime::forget_link(&id);
            shell::trash(&store::dir(&id)?).await?;
            let _ = cloudflare::set_token(&id, None).await;
            requests::forget(&id).await;
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
        "server:relayContinue" => {
            runtime::relay_continue(&args.get::<String>(0)?)?;
            done()
        }
        "server:relayTopUp" => json(runtime::relay_top_up(&args.get::<String>(0)?).await?),
        "server:relayRestart" => {
            runtime::relay_restart(&args.get::<String>(0)?).await;
            done()
        }
        "server:log" => json(runtime::log(&args.get::<String>(0)?)),
        "server:launchArgs" => json(runtime::launch_args(&store::get(&args.get::<String>(0)?).await?)?),
        "server:requests" => json(requests::info(&args.get::<String>(0)?).await?),
        "server:requestAccept" => json(requests::accept(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        "server:requestReject" => json(requests::reject(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        "server:requestBlock" => json(requests::reject_and_block(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        "server:requestUnblockAll" => json(requests::unblock_all(&args.get::<String>(0)?).await?),
        "server:setBotToken" => {
            let id: String = args.get(0)?;
            requests::set_bot_token(&id, args.get::<Option<String>>(1)?).await?;
            json(requests::info(&id).await?)
        }
        "server:clientMods" => json(clientmods::disabled(&store::dir(&args.get::<String>(0)?)?).await),
        "server:downloadsCheck" => {
            let id: String = args.get(0)?;
            let meta = store::get(&id).await?;
            let pack = meta.modpack.as_ref().and_then(|m| m.project_id.clone());
            let got = crate::content::manual::collect_for_server(&store::dir(&id)?, meta.pending_downloads.clone(), pack.as_deref()).await?;
            let mut m = store::get(&id).await?;
            m.pending_downloads = got.remaining.clone();
            store::save(&m).await?;
            json(got)
        }
        "server:downloadsForget" => {
            let id: String = args.get(0)?;
            let file: String = args.get(1)?;
            let mut m = store::get(&id).await?;
            m.pending_downloads.retain(|p| p.file_name != file);
            store::save(&m).await?;
            json(m.pending_downloads)
        }
        "server:packFlags" => json(runtime::pack_flags(&store::dir(&args.get::<String>(0)?)?)),
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
        // Installs the server software again (Server → Reinstall), and the modpack's server files.
        // Paper's built-in anti-xray: [id] → { enabled, mode } (null for servers without it); [id, { enabled, mode }].
        "server:antiXray" => {
            let meta = store::get(&args.get::<String>(0)?).await?;
            let dir = store::dir(&meta.id)?;
            json(if antixray::supported(meta.kind) { Some(antixray::get(&meta, &dir).await) } else { None })
        }
        // Settings → Anti-cheat (Grim): [id] → {supported, installed, enabled}; [id, on] → the same, after the change.
        "server:antiCheat" => json(plugins::anticheat_status(&args.get::<String>(0)?).await?),
        "server:setAntiCheat" => {
            let id: String = args.get(0)?;
            plugins::anticheat_set(&id, args.get(1)?).await?;
            json(plugins::anticheat_status(&id).await?)
        }
        "server:setAntiXray" => {
            let meta = store::get(&args.get::<String>(0)?).await?;
            let want: antixray::AntiXray = args.get(1)?;
            antixray::set(&meta, &store::dir(&meta.id)?, &want).await?;
            done()
        }
        "server:reinstall" => {
            packs::reinstall(&args.get::<String>(0)?).await?;
            done()
        }
        "server:reinstallPack" => json(packs::reinstall_pack(&args.get::<String>(0)?).await?),
        "server:backup" => json(runtime::backup(&args.get::<String>(0)?).await?),
        // [id, {backup: bool, seed: null | "" (random) | "…"}] → how many world folders went to the Recycle Bin.
        "server:regenerateWorld" => {
            let opts = args.raw(1);
            let backup = opts.get("backup").and_then(Value::as_bool).unwrap_or(true);
            let seed = opts.get("seed").and_then(Value::as_str).map(str::to_string);
            json(runtime::regenerate_world(&args.get::<String>(0)?, backup, seed).await?)
        }
        // The add-on calls end with an optional area ("plugins" / "mods") for hybrid servers.
        "server:addonSources" => json(plugins::sources(&args.get::<String>(0)?, args.get::<Option<String>>(1)?.as_deref()).await?),
        "server:addons" => json(plugins::list(&args.get::<String>(0)?, args.get::<Option<String>>(1)?.as_deref()).await?),
        "server:addonSearch" => {
            let (id, query, source): (String, String, String) = (args.get(0)?, args.get::<Option<String>>(1)?.unwrap_or_default(), args.get(2)?);
            let offset: Option<usize> = args.get(3)?;
            let area: Option<String> = args.get(4)?;
            json(plugins::search(&id, &query, &source, offset.unwrap_or(0), area.as_deref()).await?)
        }
        "server:addonVersions" => {
            json(plugins::versions(&args.get::<String>(0)?, &args.get::<String>(1)?, args.get::<Option<String>>(2)?.as_deref()).await?)
        }
        "server:addonChangeVersion" => json(
            plugins::change_version(&args.get::<String>(0)?, &args.get::<String>(1)?, &args.get::<String>(2)?, args.get::<Option<String>>(3)?.as_deref()).await?,
        ),
        "server:addonCheckUpdates" => json(plugins::check_updates(&args.get::<String>(0)?, args.get::<Option<String>>(1)?.as_deref()).await?),
        "server:addonCancelPending" => {
            plugins::cancel_pending(&args.get::<String>(0)?, &args.get::<String>(1)?, args.get::<Option<String>>(2)?.as_deref()).await?;
            done()
        }
        "server:addonInstall" => {
            plugins::install(&args.get::<String>(0)?, &args.get::<String>(1)?, args.get(2)?, args.get::<Option<String>>(3)?.as_deref()).await?;
            done()
        }
        "server:addonToggle" => {
            plugins::toggle(&args.get::<String>(0)?, &args.get::<String>(1)?, args.get::<Option<String>>(2)?.as_deref()).await?;
            done()
        }
        "server:addonRemove" => {
            plugins::remove(&args.get::<String>(0)?, &args.get::<String>(1)?, args.get::<Option<String>>(2)?.as_deref()).await?;
            done()
        }
        "server:addonFromLink" => {
            plugins::install_link(&args.get::<String>(0)?, &args.get::<String>(1)?, args.get::<Option<String>>(2)?.as_deref()).await?;
            done()
        }
        "server:bedrockJoin" => json(plugins::bedrock_status(&args.get::<String>(0)?).await?),
        "server:setBedrockJoin" => {
            let (id, on): (String, bool) = (args.get(0)?, args.get(1)?);
            if on {
                plugins::bedrock_enable(&id).await?;
            } else {
                plugins::bedrock_disable(&id).await?;
            }
            json(plugins::bedrock_status(&id).await?)
        }
        "server:domain" => json(domain::info(&args.get::<String>(0)?).await?),
        "server:checkDns" => json(domain::check_now(&args.get::<String>(0)?).await?),
        "server:setDnsToken" => {
            let id: String = args.get(0)?;
            store::get(&id).await?;
            cloudflare::set_token(&id, args.get::<Option<String>>(1)?).await?;
            if runtime::net(&id).is_some_and(|n| n.state == "open") {
                tokio::spawn(domain::on_open(id.clone()));
            }
            json(domain::info(&id).await?)
        }
        "server:relayInfo" => json(relay::info(&args.get::<String>(0)?).await?),
        "server:relayCheck" => json(relay::check(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        "server:relayName" => {
            let (id, name): (String, String) = (args.get(0)?, args.get(1)?);
            let before = store::get(&id).await?;
            let meta = relay::set_name(&id, &name).await?;
            // Online under the old name: come back under the new one.
            if meta.relay_name != before.relay_name && runtime::net(&id).is_some_and(|n| n.via == Some("resin")) {
                runtime::refresh_internet(&id).await;
            }
            json(store::info(meta).await)
        }
        "server:network" => json(json!({ "lanIp": network::lan_ip().map(|ip| ip.to_string()) })),
        "server:play" => {
            let (id, instance): (String, Option<String>) = (args.get(0)?, args.get(1)?);
            let meta = store::get(&id).await?;
            if meta.kind.bedrock() {
                // Minecraft (Bedrock) adds the server to its Servers list and opens there.
                let name: String = meta.name.chars().filter(|c| *c != '|').collect();
                shell::open_app_link(&format!("minecraft://?addExternalServer={}|127.0.0.1:{}", urlencoding::encode(&name), meta.port))?;
                return done();
            }
            let instance = instance.ok_or_else(|| anyhow::anyhow!("Choose an instance to play with"))?;
            crate::game::launch(&instance, Some(format!("localhost:{}", meta.port))).await?;
            done()
        }
        _ => bail!("Unknown request: {channel}"),
    }
}

/// Sets or removes the server's picture. The launcher shows a 128×128 copy; a Java server also gets the 64×64
/// server-icon.png Minecraft shows players in their server list (it's read when the server starts). Any picture the
/// launcher's image picker can open works: it hands over a PNG.
async fn set_icon(id: &str, data_url: Option<String>) -> Result<ServerMeta> {
    let meta = store::get(id).await?;
    let file = store::dir(id)?.join("server-icon.png");
    let Some(url) = data_url.filter(|u| !u.trim().is_empty()) else {
        let _ = tokio::fs::remove_file(&file).await;
        return store::update(id, serde_json::json!({ "iconUrl": null })).await;
    };
    let bytes = {
        use base64::Engine;
        let b64 = url.split_once("base64,").map(|(_, b)| b).ok_or_else(|| anyhow::anyhow!("That picture couldn't be read"))?;
        base64::engine::general_purpose::STANDARD.decode(b64.trim())?
    };
    let java = meta.kind != ServerKind::Bedrock;
    let (small, icon) = tokio::task::spawn_blocking(move || icon_pngs(&bytes, java)).await??;
    if let Some(icon) = icon {
        tokio::fs::write(&file, icon).await?;
    }
    let shown = {
        use base64::Engine;
        format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(small))
    };
    store::update(id, serde_json::json!({ "iconUrl": shown })).await
}

/// The launcher's 128×128 copy and, for Java, the 64×64 server-icon.png Minecraft needs (exactly that size), both PNG.
fn icon_pngs(picture: &[u8], java: bool) -> Result<(Vec<u8>, Option<Vec<u8>>)> {
    let img = image::load_from_memory(picture).map_err(|_| anyhow::anyhow!("That picture couldn't be read"))?;
    let png = |size: u32| -> Result<Vec<u8>> {
        let mut out = std::io::Cursor::new(Vec::new());
        img.resize_to_fill(size, size, image::imageops::FilterType::Lanczos3).write_to(&mut out, image::ImageFormat::Png)?;
        Ok(out.into_inner())
    };
    Ok((png(128)?, if java { Some(png(64)?) } else { None }))
}

#[cfg(test)]
mod icon_tests {
    use super::*;

    #[test]
    fn server_icons_are_exact_squares() {
        // A wide JPEG, as someone might pick.
        let mut jpg = std::io::Cursor::new(Vec::new());
        image::RgbImage::from_pixel(300, 120, image::Rgb([200, 40, 90])).write_to(&mut jpg, image::ImageFormat::Jpeg).unwrap();
        let (small, icon) = icon_pngs(jpg.get_ref(), true).unwrap();
        let dims = |b: &[u8]| {
            let i = image::load_from_memory_with_format(b, image::ImageFormat::Png).unwrap();
            (i.width(), i.height())
        };
        assert_eq!(dims(&small), (128, 128));
        assert_eq!(dims(&icon.unwrap()), (64, 64));
        // Bedrock: only the launcher's copy.
        assert!(icon_pngs(jpg.get_ref(), false).unwrap().1.is_none());
        assert!(icon_pngs(b"not a picture", true).is_err());
    }
}
