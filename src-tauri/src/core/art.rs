//! Instance pictures the player chooses: the square icon and the wide banner.
//! The UI crops the picture and sends it as a data URL; everything is decoded and re-encoded here,
//! so only clean PNG/JPEG files of a bounded size ever reach the disk.

use anyhow::Result;
use base64::Engine;

use super::instances;
use super::types::Instance;
use crate::err_tr;
use crate::player::files::write_atomic;
use crate::player::screenshots::make_icon;

/// Banners are 16:9 and at most this wide (960×540 keeps the data URL around 100 KB).
const BANNER_WIDTH: u32 = 960;
const BANNER_HEIGHT: u32 = 540;
/// Refuse anything bigger than this before decoding (the UI already shrinks pictures).
const MAX_BYTES: usize = 15 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Icon,
    Banner,
}

impl Kind {
    pub fn parse(s: &str) -> Result<Kind> {
        match s {
            "icon" => Ok(Kind::Icon),
            "banner" => Ok(Kind::Banner),
            _ => anyhow::bail!("Unknown picture kind: {s}"),
        }
    }
}

/// The bytes inside a `data:image/…;base64,` URL.
fn decode_data_url(url: &str) -> Result<Vec<u8>> {
    let bad = || err_tr!("art.unreadable", "That picture couldn't be read. Try a PNG or JPEG file.");
    let rest = url.strip_prefix("data:image/").ok_or_else(bad)?;
    let (_, b64) = rest.split_once(";base64,").ok_or_else(bad)?;
    if b64.len() > MAX_BYTES * 4 / 3 + 4 {
        return Err(err_tr!("art.tooBig", "That picture is too large."));
    }
    base64::engine::general_purpose::STANDARD.decode(b64.trim()).map_err(|_| bad())
}

/// Centre-crops to 16:9 and scales down to at most BANNER_WIDTH×BANNER_HEIGHT, as JPEG.
pub fn make_banner(bytes: &[u8]) -> Result<Vec<u8>> {
    let img = image::load_from_memory(bytes).map_err(|_| err_tr!("art.unreadable", "That picture couldn't be read. Try a PNG or JPEG file."))?;
    let (w, h) = (img.width(), img.height());
    // The largest 16:9 rectangle that fits, centred.
    let (cw, ch) = if w * 9 > h * 16 { (h * 16 / 9, h) } else { (w, w * 9 / 16) };
    let (cw, ch) = (cw.max(1), ch.max(1));
    let cropped = img.crop_imm((w - cw) / 2, (h - ch) / 2, cw, ch);
    let sized = if cw > BANNER_WIDTH { cropped.resize_exact(BANNER_WIDTH, BANNER_HEIGHT, image::imageops::FilterType::Lanczos3) } else { cropped };
    let rgb = image::DynamicImage::ImageRgb8(sized.to_rgb8());
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85).encode_image(&rgb)?;
    Ok(out)
}

/// Sets (Some) or removes (None) an instance's icon or banner.
pub async fn set_image(id: &str, kind: Kind, data_url: Option<String>) -> Result<Instance> {
    instances::get(id).await?;
    match (kind, data_url) {
        (Kind::Icon, Some(url)) => {
            let bytes = decode_data_url(&url)?;
            let png = tokio::task::spawn_blocking(move || make_icon(&bytes)).await??;
            write_atomic(&instances::icon_file(id), &png).await?;
            let url = format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&png));
            instances::set_custom_icon(id, url).await
        }
        (Kind::Icon, None) => instances::reset_icon(id).await,
        (Kind::Banner, Some(url)) => {
            let bytes = decode_data_url(&url)?;
            let jpg = tokio::task::spawn_blocking(move || make_banner(&bytes)).await??;
            write_atomic(&instances::banner_file(id), &jpg).await?;
            let url = format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(&jpg));
            instances::update(id, serde_json::json!({ "bannerUrl": url })).await
        }
        (Kind::Banner, None) => {
            match tokio::fs::remove_file(instances::banner_file(id)).await {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
                _ => {}
            }
            instances::update(id, serde_json::json!({ "bannerUrl": null })).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_of(img: image::RgbImage) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img).write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn banners_are_16_by_9_and_capped() {
        let tall = make_banner(&png_of(image::RgbImage::new(2000, 2000))).unwrap();
        let tall = image::load_from_memory(&tall).unwrap();
        assert_eq!((tall.width(), tall.height()), (BANNER_WIDTH, BANNER_HEIGHT));

        let small = make_banner(&png_of(image::RgbImage::new(320, 100))).unwrap();
        let small = image::load_from_memory(&small).unwrap();
        assert_eq!((small.width(), small.height()), (177, 100), "wide pictures are cropped at the sides, not upscaled");
    }

    #[test]
    fn data_urls_are_checked() {
        let png = png_of(image::RgbImage::new(4, 4));
        let url = format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&png));
        assert_eq!(decode_data_url(&url).unwrap(), png);
        assert!(decode_data_url("https://example.com/a.png").is_err());
        assert!(decode_data_url("data:text/html;base64,PGI+").is_err());
        assert!(decode_data_url("data:image/png;base64,***").is_err());
        assert!(make_banner(b"not a picture").is_err());
    }
}
