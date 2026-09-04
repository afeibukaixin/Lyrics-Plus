use std::sync::Mutex;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use image::{DynamicImage, ImageFormat};

use super::super::PlaybackArtwork;
use super::palette::extract_artwork_accent_colors;

const MAX_ARTWORK_EDGE_PX: u32 = 192;

pub(super) fn invalidate_cache(
    cache: &Mutex<Option<PlaybackArtwork>>,
    snapshot: &super::super::PlaybackSnapshot,
) {
    let current_id = snapshot.artwork_id.as_deref();
    let mut cache = cache.lock().unwrap_or_else(|error| error.into_inner());
    if cache
        .as_ref()
        .is_some_and(|artwork| Some(artwork.id.as_str()) != current_id)
    {
        *cache = None;
    }
}

pub(super) fn cached(
    cache: &Mutex<Option<PlaybackArtwork>>,
    artwork_id: &str,
) -> Option<PlaybackArtwork> {
    let cache = cache.lock().unwrap_or_else(|error| error.into_inner());
    cache
        .as_ref()
        .filter(|artwork| artwork.id == artwork_id)
        .cloned()
}

pub(super) fn store(cache: &Mutex<Option<PlaybackArtwork>>, artwork: PlaybackArtwork) {
    *cache.lock().unwrap_or_else(|error| error.into_inner()) = Some(artwork);
}

pub(super) fn encode(artwork_id: &str, image: &DynamicImage) -> Result<PlaybackArtwork, String> {
    let thumbnail = image.thumbnail(MAX_ARTWORK_EDGE_PX, MAX_ARTWORK_EDGE_PX);
    let accent_colors = extract_artwork_accent_colors(&thumbnail);
    let mut encoded = std::io::Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut encoded, ImageFormat::Png)
        .map_err(|error| format!("封面编码失败：{error}"))?;
    Ok(PlaybackArtwork {
        id: artwork_id.to_string(),
        mime_type: "image/png".into(),
        data_base64: BASE64.encode(encoded.into_inner()),
        accent_color: accent_colors.primary,
        spectrum_colors: accent_colors.spectrum,
    })
}

// 使用图片实际像素内容计算轻量指纹，避免系统封面更新时复用旧缓存。
pub(super) fn artwork_fingerprint(image: &image::DynamicImage) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001b3;

    let mut fingerprint = FNV_OFFSET_BASIS;
    let mut update = |byte: u8| {
        fingerprint ^= u64::from(byte);
        fingerprint = fingerprint.wrapping_mul(FNV_PRIME);
    };
    for byte in image.width().to_le_bytes() {
        update(byte);
    }
    for byte in image.height().to_le_bytes() {
        update(byte);
    }
    let color = image.color();
    update(color.bytes_per_pixel());
    for byte in color.bits_per_pixel().to_le_bytes() {
        update(byte);
    }
    update(color.has_alpha() as u8);
    update(color.has_color() as u8);
    for &byte in image.as_bytes() {
        update(byte);
    }
    fingerprint
}
