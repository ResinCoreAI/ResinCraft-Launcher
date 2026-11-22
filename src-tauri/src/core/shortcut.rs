//! A desktop shortcut that starts one instance (Library → right-click an instance → Create desktop shortcut).
//!
//! The shortcut runs ResinCraft with `--play=<instance id>`. Only a command line can carry that: openfile.rs turns
//! it into the UI's internal `resincraft://play/<id>` link, and refuses that kind of link when it arrives as a real
//! `resincraft://` URL, so a web page can never start a game.
//!
//! Windows gets a `.lnk` (written by the Windows Script Host, which every Windows has), Linux a `.desktop` file.

use std::path::{Path, PathBuf};

use anyhow::Result;

use super::{ctx, instances, paths};
use crate::{bail_tr, err_tr};

/// What a shortcut starts ResinCraft with, followed by the instance's id.
pub const PLAY_ARG: &str = "--play=";

/// Whether `id` could be an instance's id (its folder name): short, and nothing a path or a command line gives
/// meaning to. The UI still looks it up among the instances that exist.
pub fn plausible_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id != "."
        && id != ".."
        && id.chars().all(|c| !c.is_control() && !c.is_whitespace() && !matches!(c, '/' | '\\' | '"' | '\'' | ':' | '*' | '?' | '<' | '>' | '|' | '%' | '&'))
}

/// The instance's name as a file name: without what Windows forbids in one, and not too long.
fn file_stem(name: &str) -> String {
    let clean: String = name.chars().filter(|c| !c.is_control() && !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')).collect();
    let clean: String = clean.trim().trim_end_matches('.').trim().chars().take(60).collect();
    if clean.is_empty() {
        return "ResinCraft".into();
    }
    // Names Windows keeps for devices (CON, NUL, COM1...) can't be files, whatever follows the first dot.
    let base = clean.split('.').next().unwrap_or("").trim().to_ascii_uppercase();
    let reserved = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (base.len() == 4 && (base.starts_with("COM") || base.starts_with("LPT")) && base.as_bytes()[3].is_ascii_digit());
    if reserved {
        format!("{clean} (ResinCraft)")
    } else {
        clean
    }
}

fn desktop() -> Result<PathBuf> {
    use tauri::Manager;
    ctx::app().and_then(|a| a.path().desktop_dir().ok()).filter(|d| d.is_dir()).ok_or_else(|| err_tr!("shortcut.noDesktop", "Couldn't find your desktop folder."))
}

/// The instance's own picture as `shortcut.ico` next to it: a 256 px PNG in an ICO wrapper, which Windows reads.
/// The picture is the one the UI sent (it can read a modpack's logo, whatever format its site serves), else a custom
/// icon kept as `icon.png`. None when there is neither.
#[cfg(windows)]
async fn icon_ico(id: &str, sent: Option<Vec<u8>>) -> Option<PathBuf> {
    let bytes = match sent {
        Some(png) => png,
        None => tokio::fs::read(instances::icon_file(id)).await.ok()?,
    };
    let png = tokio::task::spawn_blocking(move || -> Option<Vec<u8>> {
        let img = image::load_from_memory(&bytes).ok()?.resize_exact(256, 256, image::imageops::FilterType::Lanczos3).to_rgba8();
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut out, image::ImageFormat::Png).ok()?;
        Some(out.into_inner())
    })
    .await
    .ok()??;
    // ICONDIR (reserved, type 1 = icon, one image), then its ICONDIRENTRY: 0 means 256 for width and height.
    let mut ico = vec![0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 1, 0, 32, 0];
    ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes());
    ico.extend_from_slice(&png);
    let path = paths::instance_dir(id).join("shortcut.ico");
    tokio::fs::write(&path, ico).await.ok()?;
    Some(path)
}

#[cfg(windows)]
async fn write(dir: &Path, stem: &str, exe: &Path, id: &str, name: &str, icon: Option<Vec<u8>>) -> Result<PathBuf> {
    use std::os::windows::process::CommandExt;
    let lnk = dir.join(format!("{stem}.lnk"));
    let icon = match icon_ico(id, icon).await {
        Some(ico) => format!("{},0", ico.display()),
        None => format!("{},0", exe.display()),
    };
    // Every value goes in through the environment, so nothing in a name or a path is ever read as script.
    let script = "$s=(New-Object -ComObject WScript.Shell).CreateShortcut($env:RC_LNK);$s.TargetPath=$env:RC_EXE;$s.Arguments=$env:RC_ARGS;\
                  $s.WorkingDirectory=$env:RC_DIR;$s.IconLocation=$env:RC_ICON;$s.Description=$env:RC_DESC;$s.Save()";
    let mut cmd = std::process::Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("RC_LNK", &lnk)
        .env("RC_EXE", exe)
        .env("RC_ARGS", format!("{PLAY_ARG}{id}"))
        .env("RC_DIR", exe.parent().unwrap_or(dir))
        .env("RC_ICON", icon)
        .env("RC_DESC", name)
        .creation_flags(crate::game::java::CREATE_NO_WINDOW);
    let out = tokio::task::spawn_blocking(move || cmd.output()).await??;
    if !out.status.success() || !lnk.is_file() {
        let why = String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or_default().trim().to_string();
        bail_tr!("shortcut.failed", "Couldn't make the shortcut. {error}", error = why);
    }
    Ok(lnk)
}

#[cfg(target_os = "linux")]
async fn write(dir: &Path, stem: &str, exe: &Path, id: &str, name: &str, icon: Option<Vec<u8>>) -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let file = dir.join(format!("{stem}.desktop"));
    // The picture the UI sent is kept next to the instance, for the entry to point at.
    let sent = paths::instance_dir(id).join("shortcut.png");
    let icon_png = match icon {
        Some(png) if tokio::fs::write(&sent, &png).await.is_ok() => sent,
        _ => instances::icon_file(id),
    };
    let icon = if icon_png.is_file() { icon_png.display().to_string() } else { "resincraft".to_string() };
    // A .desktop value is one line; the name is the only free text in it.
    let name: String = name.chars().filter(|c| !c.is_control()).collect();
    let text = format!("[Desktop Entry]\nType=Application\nName={name}\nExec=\"{}\" {PLAY_ARG}{id}\nIcon={icon}\nTerminal=false\nCategories=Game;\n", exe.display());
    tokio::fs::write(&file, text).await?;
    tokio::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).await?;
    Ok(file)
}

#[cfg(not(any(windows, target_os = "linux")))]
async fn write(_dir: &Path, _stem: &str, _exe: &Path, _id: &str, _name: &str, _icon: Option<Vec<u8>>) -> Result<PathBuf> {
    bail_tr!("shortcut.unsupported", "Desktop shortcuts aren't available on this system yet.");
}

/// `instances:shortcut`: puts a shortcut for the instance on the desktop (replacing one of the same name) and
/// answers with where it is. `icon`: the instance's picture as PNG bytes, when the UI could read it.
pub async fn create(id: &str, icon: Option<Vec<u8>>) -> Result<String> {
    let inst = instances::get(id).await?;
    if !plausible_id(&inst.id) {
        bail_tr!("shortcut.failed", "Couldn't make the shortcut. {error}", error = "This instance's folder name can't be used in a shortcut.");
    }
    let exe = std::env::current_exe()?;
    let path = write(&desktop()?, &file_stem(&inst.name), &exe, &inst.id, &inst.name, icon).await?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_a_shortcut_may_carry() {
        for ok in ["cozy-survival-a1b2c3", "x", "โลกของฉัน-0f9e8d", "tech_and.automation-112233"] {
            assert!(plausible_id(ok), "{ok}");
        }
        for bad in ["", ".", "..", "a/b", "a\\b", "a b", "a\"b", "c:evil", "a&b", "100%", &"x".repeat(129)] {
            assert!(!plausible_id(bad), "{bad}");
        }
    }

    #[test]
    fn names_become_file_names() {
        assert_eq!(file_stem("  My: World?  "), "My World");
        assert_eq!(file_stem("..."), "ResinCraft");
        assert_eq!(file_stem("con"), "con (ResinCraft)");
        assert_eq!(file_stem("NUL.world"), "NUL.world (ResinCraft)");
        assert_eq!(file_stem("COM1"), "COM1 (ResinCraft)");
        assert_eq!(file_stem("Console"), "Console");
        assert_eq!(file_stem("COMB"), "COMB");
        assert_eq!(file_stem("a/b\\c"), "abc");
        assert_eq!(file_stem(&"n".repeat(200)).chars().count(), 60);
    }
}
