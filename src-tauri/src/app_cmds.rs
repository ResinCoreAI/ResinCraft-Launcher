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
            discord: crate::discord::available(),
        }),
        "app:openExternal" => {
            shell::open_url(&args.get::<String>(0)?)?;
            done()
        }
        "app:showItem" => {
            shell::reveal(std::path::Path::new(&args.get::<String>(0)?))?;
            done()
        }
        // Kept for older front ends: the window controls are drawn in the UI, so there's nothing to recolour.
        "app:setTitleBar" => done(),
        "app:memory" => json(crate::core::memory::usage().await),
        "app:rest" => {
            crate::webview::rest().await;
            done()
        }
        "app:cancelTask" => json(crate::core::events::cancel(&args.get::<String>(0)?)),
        // A custom theme's background picture: [theme id, data URL | null (remove)], and reading it back.
        "app:saveThemeImage" => {
            save_theme_image(&args.get::<String>(0)?, args.get::<Option<String>>(1)?).await?;
            done()
        }
        "app:themeImage" => json(theme_image(&args.get::<String>(0)?).await),
        // A file Windows asked ResinCraft to open at start (once), and a theme file's text (openfile.rs).
        // Settings → Uninstall ResinCraft: "uninstaller" (it started; ResinCraft quits) or "settings" (Windows' Installed
        // apps page, for MSI and Store installs).
        "app:uninstall" => json(uninstall()?),
        // The window moved to https: the old origin's localStorage, read from WebView2's files once (webstorage.rs).
        "app:storageImport" => json(crate::webstorage::import().await),
        "app:pendingOpen" => json(crate::openfile::take()),
        "app:pendingLink" => json(crate::openfile::take_link()),
        // A picture the UI drew (an instance's overview card), as a PNG data URL: to the clipboard, or to a file the
        // player picks ([suggested name, data URL] → where it was saved, or null).
        "app:copyPng" => {
            shell::copy_png(&png_from_data_url(&args.get::<String>(0)?)?)?;
            done()
        }
        "app:savePng" => json(save_png(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        "app:mcpInfo" => json(crate::mcp::info().await?),
        "app:mcpResetToken" => json(crate::mcp::reset_token().await?),
        "app:readThemeFile" => json(crate::openfile::read_theme(&args.get::<String>(0)?).await?),
        // Settings → Appearance → Export: [file name, the theme file's JSON text] → where it was saved, or null.
        "app:exportTheme" => json(export_theme(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        // A QR code as SVG, made here (the 2FA setup key never leaves the PC).
        "app:qrSvg" => json(qr_svg(&args.get::<String>(0)?)?),
        // [file name, text] → a save dialog (recovery codes); the path, or null when cancelled.
        "app:saveText" => json(save_text(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        // A few textures for the holiday decorations, from the player's own Minecraft (never shipped with ResinCraft).
        "app:gameTexture" => json(game_texture(&args.get::<String>(0)?).await),

        // "Report a problem" from inside the launcher (report.rs), to the same team inbox as the report website.
        "app:sendReport" => json(crate::report::send(serde_json::from_value(args.raw(0))?).await?),
        "app:serviceStatus" => json(crate::report::status().await),
        "app:checkServices" => json(crate::status::check_all().await),
        "app:statusNotices" => json(crate::status::notices().await),
        // A picture from the web as a data: URL, for pictures the UI draws itself (the overview card): a page can't
        // read pixels from sites that don't allow it, the engine can.
        "app:imageData" => json(image_data(&args.get::<String>(0)?).await?),
        "app:artData" => json(art_data(&args.get::<String>(0)?).await?),

        // The page the player is on, for Discord's "in the launcher" presence (discord.rs `page_lines`).
        "app:discordPage" => {
            let page: String = args.get(0)?;
            let kind: Option<String> = args.get(1).ok().flatten();
            let name: Option<String> = args.get(2).ok().flatten();
            crate::discord::page(&page, kind.as_deref(), name.as_deref()).await;
            done()
        }

        "settings:get" => json(settings::redacted(settings::get().await)),
        "settings:set" => {
            let s = settings::set(args.raw(0)).await?;
            crate::discord::refresh().await;
            json(settings::redacted(s))
        }

        "settings:reset" => {
            let s = settings::reset().await?;
            crate::discord::refresh().await;
            json(settings::redacted(s))
        }

        "instances:list" => json(instances::list().await?),
        "instances:create" => {
            let inst = instances::create(args.get::<NewInstance>(0)?, |_| {}).await?;
            // Its game files start arriving now, not at the first Play.
            crate::game::launch::get_ready(&inst.id);
            json(inst)
        }
        "instances:update" => json(instances::update(&args.get::<String>(0)?, args.raw(1)).await?),
        // Files to download by hand: pick up the ones that arrived in Downloads.
        "instances:collectPending" => json(crate::content::manual::collect(&args.get::<String>(0)?).await?),
        "instances:addWatchFolder" => json(crate::content::manual::add_watch_folder().await?),
        // [id, Minecraft version, loader, loader version | null (the newest)] → Instance.
        "instances:setVersion" => {
            let inst = instances::set_version(&args.get::<String>(0)?, &args.get::<String>(1)?, args.get(2)?, args.get::<Option<String>>(3)?).await?;
            crate::game::launch::get_ready(&inst.id);
            json(inst)
        }
        "instances:duplicate" => json(instances::duplicate(&args.get::<String>(0)?).await?),
        "instances:copyFrom" => json(
            crate::player::carry::copy_from(&args.get::<String>(0)?, &args.get::<String>(1)?, args.get::<crate::player::carry::CopyPicks>(2)?).await?,
        ),
        "instances:setImage" => {
            let id: String = args.get(0)?;
            crate::player::files::instance_root(&id).await?;
            let kind = crate::core::art::Kind::parse(&args.get::<String>(1)?)?;
            json(crate::core::art::set_image(&id, kind, args.get::<Option<String>>(2)?).await?)
        }
        "instances:resetIcon" => {
            let id: String = args.get(0)?;
            crate::player::files::instance_root(&id).await?;
            json(instances::reset_icon(&id).await?)
        }
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
        // Library → right-click → Create desktop shortcut: where the shortcut was put.
        "instances:shortcut" => {
            // [id, the instance's picture as a PNG data URL (optional)]
            let icon = match args.get::<Option<String>>(1)? {
                Some(url) => Some(png_from_data_url(&url)?),
                None => None,
            };
            json(crate::core::shortcut::create(&args.get::<String>(0)?, icon).await?)
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

/// Biggest picture a theme may use (the creator says 8 MB too).
const THEME_IMAGE_MAX: usize = 8 * 1024 * 1024;

/// `<data>/themes/<id>.<ext>`: ids come from the UI, so only letters, digits and `-` make it into a file name.
fn theme_image_stem(id: &str) -> Result<std::path::PathBuf> {
    if id.is_empty() || id.len() > 64 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        bail!("That theme can't have a picture.");
    }
    Ok(paths::root().join("themes").join(id))
}

const THEME_IMAGE_TYPES: [(&str, &str); 5] =
    [("png", "image/png"), ("jpg", "image/jpeg"), ("webp", "image/webp"), ("gif", "image/gif"), ("avif", "image/avif")];

async fn save_theme_image(id: &str, data_url: Option<String>) -> Result<()> {
    let stem = theme_image_stem(id)?;
    for (ext, _) in THEME_IMAGE_TYPES {
        let _ = tokio::fs::remove_file(stem.with_extension(ext)).await;
    }
    let Some(url) = data_url.filter(|u| !u.trim().is_empty()) else { return Ok(()) };
    let (head, b64) = url.split_once(";base64,").ok_or_else(|| anyhow::anyhow!("That picture couldn't be read"))?;
    let mime = head.trim_start_matches("data:");
    let Some((ext, _)) = THEME_IMAGE_TYPES.iter().find(|(_, m)| *m == mime) else {
        bail!("Use a PNG, JPG, WebP, GIF or AVIF picture.");
    };
    let bytes = {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.decode(b64.trim())?
    };
    if bytes.len() > THEME_IMAGE_MAX {
        bail!("That picture is over 8 MB.");
    }
    tokio::fs::create_dir_all(stem.parent().unwrap()).await?;
    tokio::fs::write(stem.with_extension(ext), bytes).await?;
    Ok(())
}

async fn theme_image(id: &str) -> Option<String> {
    let stem = theme_image_stem(id).ok()?;
    for (ext, mime) in THEME_IMAGE_TYPES {
        if let Ok(bytes) = tokio::fs::read(stem.with_extension(ext)).await {
            use base64::Engine;
            return Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)));
        }
    }
    None
}

/// The textures the holiday decorations (ThemeDecor.tsx) draw with. They come from a Minecraft game file the player
/// already downloaded, so ResinCraft never ships Mojang's art; with none downloaded yet, the UI draws its own.
const GAME_TEXTURES: [&str; 6] = [
    "block/carved_pumpkin",
    "block/jack_o_lantern",
    "block/pumpkin_side",
    "block/pumpkin_top",
    "entity/zombie/zombie",
    "entity/chest/christmas",
];

static GAME_TEXTURE_CACHE: std::sync::Mutex<Option<std::collections::HashMap<&'static str, Option<String>>>> =
    std::sync::Mutex::new(None);

async fn game_texture(name: &str) -> Option<String> {
    let name: &'static str = GAME_TEXTURES.iter().find(|n| **n == name)?;
    if let Some(hit) = GAME_TEXTURE_CACHE.lock().unwrap().as_ref().and_then(|c| c.get(name)) {
        return hit.clone();
    }
    let found = tokio::task::spawn_blocking(move || read_game_texture(name)).await.ok().flatten();
    GAME_TEXTURE_CACHE.lock().unwrap().get_or_insert_with(Default::default).insert(name, found.clone());
    found
}

/// The texture as a PNG data URL, from the newest vanilla client jar in the shared versions folder.
fn read_game_texture(name: &str) -> Option<String> {
    use std::io::Read;
    // Vanilla client jars are several MB; loader profiles (Fabric, Forge) have tiny or empty ones.
    let mut jars: Vec<(std::time::SystemTime, std::path::PathBuf)> = std::fs::read_dir(paths::versions())
        .ok()?
        .flatten()
        .filter_map(|e| {
            let dir = e.path();
            let jar = dir.join(format!("{}.jar", dir.file_name()?.to_str()?));
            let meta = jar.metadata().ok()?;
            if meta.len() < 1_000_000 {
                return None;
            }
            Some((meta.modified().ok()?, jar))
        })
        .collect();
    jars.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, jar) in jars {
        let Ok(file) = std::fs::File::open(&jar) else { continue };
        let Ok(mut zip) = zip::ZipArchive::new(file) else { continue };
        let Ok(entry) = zip.by_name(&format!("assets/minecraft/textures/{name}.png")) else { continue };
        let mut bytes = Vec::new();
        if entry.take(256 * 1024).read_to_end(&mut bytes).is_err() || bytes.is_empty() {
            continue;
        }
        use base64::Engine;
        return Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)));
    }
    None
}
/// Saves a theme someone can import elsewhere (`<name>.rctheme`, JSON with the picture inside). None if cancelled.
async fn export_theme(name: &str, contents: &str) -> Result<Option<String>> {
    let base: String = name.chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_')).collect();
    let base = if base.trim().is_empty() { "theme".to_string() } else { base.trim().to_string() };
    let title = crate::tr!("app.exportThemeTitle", "Save the theme").text;
    let default = shell::downloads_dir().join(format!("{base}.rctheme"));
    let Some(path) = shell::save_file(&title, &default, &[("ResinCraft theme", &["rctheme"])]).await? else { return Ok(None) };
    tokio::fs::write(&path, contents).await?;
    Ok(Some(path.to_string_lossy().into_owned()))
}
/// `text` as a black-on-white QR code (SVG, with a quiet zone), for an authenticator app to scan.
fn qr_svg(text: &str) -> Result<String> {
    let code = qrcode::QrCode::with_error_correction_level(text.as_bytes(), qrcode::EcLevel::M)?;
    Ok(code.render::<qrcode::render::svg::Color>().min_dimensions(200, 200).quiet_zone(true).build())
}

/// The PNG inside a `data:image/png;base64,` URL the UI sends; anything else, or anything huge, is refused.
fn png_from_data_url(url: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    const MAX: usize = 12 * 1024 * 1024;
    let b64 = url.strip_prefix("data:image/png;base64,").filter(|b| b.len() <= MAX);
    let png = b64.and_then(|b| base64::engine::general_purpose::STANDARD.decode(b).ok());
    match png {
        Some(p) if p.starts_with(&[0x89, b'P', b'N', b'G']) => Ok(p),
        _ => bail_tr!("shell.imageUnreadable", "Couldn't read the image: {error}", error = "not a PNG"),
    }
}

/// Saves a PNG the UI drew where the player picks (Downloads by default). None if cancelled.
async fn save_png(name: &str, data_url: &str) -> Result<Option<String>> {
    let png = png_from_data_url(data_url)?;
    let base: String = name.chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_')).collect();
    let base = if base.trim().is_empty() { "ResinCraft".to_string() } else { base.trim().to_string() };
    let title = crate::tr!("app.saveImageTitle", "Save the image").text;
    let default = shell::downloads_dir().join(format!("{base}.png"));
    let Some(path) = shell::save_file(&title, &default, &[("PNG image", &["png"])]).await? else { return Ok(None) };
    tokio::fs::write(&path, png).await?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// Saves `text` as a .txt file the player picks (Downloads by default).
async fn save_text(name: &str, text: &str) -> Result<Option<String>> {
    let base: String = name.chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.')).collect();
    let base = if base.trim().is_empty() { "ResinCraft.txt".to_string() } else { base.trim().to_string() };
    let title = crate::tr!("app.saveTextTitle", "Save the file").text;
    let default = shell::downloads_dir().join(&base);
    let Some(path) = shell::save_file(&title, &default, &[("Text", &["txt"])]).await? else { return Ok(None) };
    tokio::fs::write(&path, text).await?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// Starts ResinCraft's own uninstaller (installed with the setup: `uninstall.exe` next to the program) and quits so it
/// can remove the files; it asks whether to delete the player's data. MSI and Microsoft Store installs are removed
/// through Windows' Installed apps page instead. Refused while a game or hosted server runs.
fn uninstall() -> Result<&'static str> {
    if !crate::game::running_ids().is_empty() || !crate::server::runtime::running_ids().is_empty() {
        crate::bail_tr!("app.uninstallBusy", "Close your games and stop your servers first.");
    }
    #[cfg(windows)]
    {
        let uninstaller = std::env::current_exe()?.parent().map(|d| d.join("uninstall.exe")).filter(|p| p.is_file());
        if let Some(uninstaller) = uninstaller {
            std::process::Command::new(uninstaller).spawn()?;
            if let Some(app) = ctx::app().cloned() {
                // A moment for the answer to reach the UI; the uninstaller waits for ResinCraft to close.
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
                    crate::tray::quit(&app);
                });
            }
            return Ok("uninstaller");
        }
        std::process::Command::new("explorer.exe").arg("ms-settings:appsfeatures").spawn()?;
        Ok("settings")
    }
    #[cfg(not(windows))]
    crate::bail_tr!("app.uninstallElsewhere", "Remove ResinCraft the way your system removes apps.");
}
/** At most this big: a modpack's gallery picture, not a download. */
const MAX_IMAGE: usize = 12 * 1024 * 1024;

/// What a picture file is, from its first bytes: PNG, JPEG, GIF or WebP (never SVG, which can carry scripts).
fn picture_kind(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF8") {
        Some("image/gif")
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// An https picture as a data: URL, at most MAX_IMAGE. Told apart by its own bytes: some servers (FTB's) send
/// pictures as plain files.
async fn image_data(url: &str) -> Result<String> {
    use base64::Engine;
    if !url.starts_with("https://") {
        bail!("only https pictures");
    }
    let res = ctx::http().get(url).timeout(std::time::Duration::from_secs(20)).send().await?.error_for_status()?;
    if res.content_length().is_some_and(|n| n as usize > MAX_IMAGE) {
        bail!("picture too big");
    }
    let bytes = res.bytes().await?;
    if bytes.len() > MAX_IMAGE {
        bail!("picture too big");
    }
    let Some(kind) = picture_kind(&bytes) else { bail!("not a picture") };
    Ok(format!("data:{kind};base64,{}", base64::engine::general_purpose::STANDARD.encode(&bytes)))
}

/// Widest a modpack's picture is kept for the backdrops behind Home, big screen mode and the launch screen. A
/// gallery's original (often 1920–2560 wide) decodes to 8–15 MB of graphics memory; at this width it's under 4 MB,
/// and it's shown darkened behind text anyway.
const ART_WIDTH: u32 = 1280;

/// A modpack's gallery picture, shrunk to ART_WIDTH as a JPEG data: URL and kept in the cache folder, so it's
/// fetched and shrunk once. A WebP (which isn't decoded here) or anything that can't be read comes back as it is.
async fn art_data(url: &str) -> Result<String> {
    use base64::Engine;
    use sha2::Digest;
    let dir = paths::cache().join("art");
    let file = dir.join(format!("{}-{ART_WIDTH}.jpg", &hex::encode(sha2::Sha256::digest(url.as_bytes()))[..24]));
    if let Ok(bytes) = tokio::fs::read(&file).await {
        return Ok(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(&bytes)));
    }
    let original = image_data(url).await?;
    let Some(b64) = original.split_once(',').map(|(_, b)| b.to_owned()) else { return Ok(original) };
    let shrunk = tokio::task::spawn_blocking(move || -> Option<Vec<u8>> {
        let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
        let img = image::load_from_memory(&bytes).ok()?;
        let img = if img.width() > ART_WIDTH { img.resize(ART_WIDTH, u32::MAX, image::imageops::FilterType::Triangle) } else { img };
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 84).encode_image(&img.to_rgb8()).ok()?;
        Some(out)
    })
    .await?;
    let Some(jpeg) = shrunk else { return Ok(original) };
    let _ = tokio::fs::create_dir_all(&dir).await;
    let _ = tokio::fs::write(&file, &jpeg).await;
    Ok(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(&jpeg)))
}

#[cfg(test)]
mod picture_tests {
    use super::picture_kind;

    #[test]
    fn pictures_are_known_by_their_bytes() {
        assert_eq!(picture_kind(&[0x89, b'P', b'N', b'G', 13, 10]), Some("image/png"));
        assert_eq!(picture_kind(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(picture_kind(b"RIFF\x00\x00\x00\x00WEBPVP8 "), Some("image/webp"));
        assert_eq!(picture_kind(b"<svg xmlns="), None);
        assert_eq!(picture_kind(b"<html>"), None);
    }
}
