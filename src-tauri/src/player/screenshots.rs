//! The instance page's Screenshots tab.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use anyhow::{bail, Result};
use base64::Engine;

use super::files::{child_path, game_subdir};
use crate::core::types::ScreenshotInfo;
use crate::err_tr;

const IMAGE_EXTS: [&str; 3] = ["png", "jpg", "jpeg"];
const THUMB_WIDTH: u32 = 384;
const JPEG_QUALITY: u8 = 82;

fn ext_of(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

fn is_image(p: &Path) -> bool {
    IMAGE_EXTS.contains(&ext_of(p).as_str())
}

fn mtime_ms(meta: &std::fs::Metadata) -> i64 {
    meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Path of one screenshot, checked to be an image directly inside the instance's screenshots folder.
pub async fn screenshot_path(id: &str, name: &str) -> Result<PathBuf> {
    let p = child_path(&game_subdir(id, "screenshots").await?, name)?;
    if !is_image(&p) {
        bail!("Not a screenshot");
    }
    Ok(p)
}

pub async fn list_screenshots(id: &str) -> Result<Vec<ScreenshotInfo>> {
    let dir = game_subdir(id, "screenshots").await?;
    let mut rd = tokio::fs::read_dir(&dir).await?;
    let mut shots = Vec::new();
    while let Some(e) = rd.next_entry().await? {
        let is_file = e.file_type().await.map(|t| t.is_file()).unwrap_or(false);
        if !is_file || !is_image(Path::new(&e.file_name())) {
            continue;
        }
        let meta = tokio::fs::metadata(e.path()).await?;
        shots.push(ScreenshotInfo { name: e.file_name().to_string_lossy().into_owned(), size: meta.len(), mtime: mtime_ms(&meta) });
    }
    shots.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    Ok(shots)
}

/// Decoding full-size PNGs is the slow part, so keep recent thumbnails around.
struct ThumbCache {
    map: HashMap<String, String>,
    order: VecDeque<String>,
}

static THUMBS: Mutex<Option<ThumbCache>> = Mutex::new(None);

fn decode(bytes: &[u8]) -> Result<image::DynamicImage> {
    image::load_from_memory(bytes).map_err(|_| err_tr!("screenshots.unreadable", "Could not read image"))
}

/// Builds a small JPEG data URL (at most 384 px wide) from image bytes.
pub fn make_thumbnail(bytes: &[u8]) -> Result<String> {
    let img = decode(bytes)?;
    let img = if img.width() > THUMB_WIDTH {
        let h = ((img.height() as f64 * THUMB_WIDTH as f64 / img.width() as f64).round() as u32).max(1);
        img.resize_exact(THUMB_WIDTH, h, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let rgb = image::DynamicImage::ImageRgb8(img.to_rgb8());
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY).encode_image(&rgb)?;
    Ok(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(out)))
}

pub async fn screenshot_thumbnail(id: &str, name: &str) -> Result<String> {
    let p = screenshot_path(id, name).await?;
    let meta = tokio::fs::metadata(&p).await?;
    let key = format!("{}|{}", p.display(), mtime_ms(&meta));
    if let Some(hit) = THUMBS.lock().unwrap().as_ref().and_then(|c| c.map.get(&key).cloned()) {
        return Ok(hit);
    }
    let bytes = tokio::fs::read(&p).await?;
    let url = tokio::task::spawn_blocking(move || make_thumbnail(&bytes)).await??;
    let mut guard = THUMBS.lock().unwrap();
    let cache = guard.get_or_insert_with(|| ThumbCache { map: HashMap::new(), order: VecDeque::new() });
    if cache.map.len() > 400 {
        if let Some(old) = cache.order.pop_front() {
            cache.map.remove(&old);
        }
    }
    if cache.map.insert(key.clone(), url.clone()).is_none() {
        cache.order.push_back(key);
    }
    Ok(url)
}

/// The full image as a data URL (the file's own bytes).
pub async fn screenshot_image(id: &str, name: &str) -> Result<String> {
    let p = screenshot_path(id, name).await?;
    let mime = if ext_of(&p) == "png" { "image/png" } else { "image/jpeg" };
    let bytes = tokio::fs::read(&p).await?;
    Ok(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

/// The screenshot as PNG bytes (JPEGs are converted).
pub async fn screenshot_png(id: &str, name: &str) -> Result<Vec<u8>> {
    let p = screenshot_path(id, name).await?;
    let bytes = tokio::fs::read(&p).await?;
    tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        // PNGs go as they are (the clipboard code decodes them anyway); JPEGs are re-encoded.
        if ext_of(&p) == "png" {
            return Ok(bytes);
        }
        let img = decode(&bytes)?;
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png)?;
        Ok(out.into_inner())
    })
    .await?
}

pub async fn copy_screenshot(id: &str, name: &str) -> Result<()> {
    let png = screenshot_png(id, name).await?;
    crate::core::shell::copy_png(&png)
}

pub async fn show_screenshot(id: &str, name: &str) -> Result<()> {
    crate::core::shell::reveal(&screenshot_path(id, name).await?)
}

pub async fn remove_screenshot(id: &str, name: &str) -> Result<()> {
    crate::core::shell::trash(&screenshot_path(id, name).await?).await
}
