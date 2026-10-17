//! Talking to the operating system: opening folders and links, the Recycle Bin, file dialogs, clipboard.
//! All of these need the running app; in headless runs they fail with a clear error.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use super::ctx;
use crate::{bail_tr, err_tr};

fn app() -> Result<&'static tauri::AppHandle> {
    ctx::app().ok_or_else(|| anyhow!("Not available without the app window"))
}

/// Opens a folder (or file) with the system's default handler.
pub fn open_path(path: &Path) -> Result<()> {
    app()?.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(|e| err_tr!("shell.openFailed", "Couldn't open {path}: {error}", path = path.display().to_string(), error = e.to_string()))
}

/// Shows a file selected in Explorer / Finder.
pub fn reveal(path: &Path) -> Result<()> {
    app()?.opener().reveal_item_in_dir(path).map_err(|e| err_tr!("shell.showFailed", "Couldn't show {path}: {error}", path = path.display().to_string(), error = e.to_string()))
}

/// Opens an https link in the user's browser.
pub fn open_url(url: &str) -> Result<()> {
    if !url.starts_with("https://") {
        bail_tr!("shell.httpsOnly", "Only https links can be opened");
    }
    app()?.opener().open_url(url, None::<&str>).map_err(|e| err_tr!("shell.openLinkFailed", "Couldn't open the link: {error}", error = e.to_string()))
}

/// Moves a file or folder to the Recycle Bin / Trash (never a permanent delete).
pub async fn trash(path: &Path) -> Result<()> {
    let p = path.to_path_buf();
    tokio::task::spawn_blocking(move || trash::delete(&p)).await?.map_err(|e| {
        err_tr!("shell.trashFailed", "Couldn't move {path} to the Recycle Bin: {error}", path = path.display().to_string(), error = e.to_string())
    })
}

/// A file-type filter for dialogs: ("Modrinth modpack", &["mrpack"]).
pub type Filter<'a> = (&'a str, &'a [&'a str]);

/// Asks the user to pick one file. None if cancelled.
pub async fn pick_file(title: &str, filters: &[Filter<'_>]) -> Result<Option<PathBuf>> {
    let mut dialog = app()?.dialog().file().set_title(title);
    for (name, exts) in filters {
        dialog = dialog.add_filter(*name, exts);
    }
    let picked = tokio::task::spawn_blocking(move || dialog.blocking_pick_file()).await?;
    Ok(picked.and_then(|p| p.into_path().ok()))
}

/// Asks where to save a file. None if cancelled.
pub async fn save_file(title: &str, default_path: &Path, filters: &[Filter<'_>]) -> Result<Option<PathBuf>> {
    let mut dialog = app()?.dialog().file().set_title(title);
    if let Some(dir) = default_path.parent() {
        dialog = dialog.set_directory(dir);
    }
    if let Some(name) = default_path.file_name() {
        dialog = dialog.set_file_name(name.to_string_lossy());
    }
    for (name, exts) in filters {
        dialog = dialog.add_filter(*name, exts);
    }
    let picked = tokio::task::spawn_blocking(move || dialog.blocking_save_file()).await?;
    Ok(picked.and_then(|p| p.into_path().ok()))
}

/// Copies a PNG image to the clipboard.
pub fn copy_png(png: &[u8]) -> Result<()> {
    let image = tauri::image::Image::from_bytes(png).map_err(|e| err_tr!("shell.imageUnreadable", "Couldn't read the image: {error}", error = e.to_string()))?;
    app()?.clipboard().write_image(&image).map_err(|e| err_tr!("shell.copyImageFailed", "Couldn't copy the image: {error}", error = e.to_string()))
}

/// The user's Downloads folder (for default save locations).
pub fn downloads_dir() -> PathBuf {
    use tauri::Manager;
    ctx::app().and_then(|a| a.path().download_dir().ok()).unwrap_or_else(std::env::temp_dir)
}
