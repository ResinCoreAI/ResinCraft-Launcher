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
        "app:pendingOpen" => json(crate::openfile::take()),
        "app:readThemeFile" => json(crate::openfile::read_theme(&args.get::<String>(0)?).await?),
        // Settings → Appearance → Export: [file name, the theme file's JSON text] → where it was saved, or null.
        "app:exportTheme" => json(export_theme(&args.get::<String>(0)?, &args.get::<String>(1)?).await?),
        // A few textures for the holiday decorations, from the player's own Minecraft (never shipped with ResinCraft).
        "app:gameTexture" => json(game_texture(&args.get::<String>(0)?).await),

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
        "instances:create" => json(instances::create(args.get::<NewInstance>(0)?, |_| {}).await?),
        "instances:update" => json(instances::update(&args.get::<String>(0)?, args.raw(1)).await?),
        // Files to download by hand: pick up the ones that arrived in Downloads.
        "instances:collectPending" => json(crate::content::manual::collect(&args.get::<String>(0)?).await?),
        "instances:addWatchFolder" => json(crate::content::manual::add_watch_folder().await?),
        // [id, Minecraft version, loader, loader version | null (the newest)] → Instance.
        "instances:setVersion" => json(
            instances::set_version(&args.get::<String>(0)?, &args.get::<String>(1)?, args.get(2)?, args.get::<Option<String>>(3)?).await?,
        ),
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