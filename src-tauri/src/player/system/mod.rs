use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, OnceLock, RwLock,
};
use std::time::{Duration, Instant, SystemTime};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use image::ImageFormat;
use media_remote::{Controller, NowPlayingInfo, NowPlayingPerl, Subscription};
use serde_json::Value;

use super::{
    normalized_track_component, now_ms, run_with_timeout, PlaybackAction, PlaybackArtwork,
    PlaybackErrorCode, PlaybackSnapshot, PlaybackSpectrumColors, PlaybackSpectrumColumnColors,
    PlayerKind,
};

mod compat;

const MAX_ARTWORK_EDGE_PX: u32 = 192;
const ARTWORK_COLOR_SAMPLE_SIZE: u32 = 32;
const ARTWORK_COLOR_QUANTIZATION_STEP: u8 = 32;
const ARTWORK_COLOR_MIN_ALPHA: u8 = 128;
const ARTWORK_BACKGROUND_BORDER_SIZE: u32 = 2;
const ARTWORK_BACKGROUND_MIN_COVERAGE: f32 = 0.35;
const ARTWORK_BACKGROUND_DISTANCE_START: f32 = 0.03;
const ARTWORK_BACKGROUND_DISTANCE_END: f32 = 0.18;
const ARTWORK_BACKGROUND_WEIGHT_FLOOR: f32 = 0.08;
const ARTWORK_MIN_GLOBAL_FOREGROUND_EVIDENCE: f32 = 24.0;
const ARTWORK_MIN_REGION_FOREGROUND_EVIDENCE: f32 = 6.0;
const ARTWORK_NEUTRAL_SATURATION_THRESHOLD: f32 = 0.18;
const ARTWORK_HUE_BUCKET_COUNT: usize = 12;
const ARTWORK_PRIMARY_MIN_LIGHTNESS: f32 = 0.42;
const ARTWORK_PRIMARY_MAX_LIGHTNESS: f32 = 0.72;
const ARTWORK_SPECTRUM_MIN_LIGHTNESS: f32 = 0.46;
const ARTWORK_SPECTRUM_MAX_LIGHTNESS: f32 = 0.74;
const ARTWORK_SPECTRUM_LOCAL_WEIGHT: f32 = 0.60;
const ARTWORK_SPECTRUM_GLOBAL_WEIGHT: f32 = 0.40;
const ARTWORK_DARK_SATURATION_REDUCTION: f32 = 0.85;
const DEFAULT_ARTWORK_ACCENT_COLOR: &str = "#ffffff";

struct ArtworkAccentColors {
    primary: String,
    spectrum: PlaybackSpectrumColors,
}

struct ArtworkColorBucket {
    count: u32,
    red: f32,
    green: f32,
    blue: f32,
}

#[derive(Default)]
struct ArtworkColorBuckets {
    bucket_indices: HashMap<(u8, u8, u8), usize>,
    buckets: Vec<ArtworkColorBucket>,
}

impl ArtworkColorBuckets {
    fn add(&mut self, red: u8, green: u8, blue: u8) {
        let key = (
            red / ARTWORK_COLOR_QUANTIZATION_STEP,
            green / ARTWORK_COLOR_QUANTIZATION_STEP,
            blue / ARTWORK_COLOR_QUANTIZATION_STEP,
        );
        let bucket_index = if let Some(index) = self.bucket_indices.get(&key) {
            *index
        } else {
            let index = self.buckets.len();
            self.bucket_indices.insert(key, index);
            self.buckets.push(ArtworkColorBucket {
                count: 0,
                red: 0.0,
                green: 0.0,
                blue: 0.0,
            });
            index
        };
        let bucket = &mut self.buckets[bucket_index];
        bucket.count += 1;
        bucket.red += red as f32;
        bucket.green += green as f32;
        bucket.blue += blue as f32;
    }

    fn dominant_bucket(&self) -> Option<&ArtworkColorBucket> {
        self.buckets.iter().max_by_key(|bucket| bucket.count)
    }

    fn dominant_rgb(&self) -> Option<(f32, f32, f32)> {
        let selected = self.dominant_bucket()?;
        let red = selected.red / selected.count as f32;
        let green = selected.green / selected.count as f32;
        let blue = selected.blue / selected.count as f32;
        Some((red, green, blue))
    }

    fn dominant_count(&self) -> Option<u32> {
        self.dominant_bucket().map(|bucket| bucket.count)
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum ArtworkPaletteKey {
    Neutral,
    Hue(usize),
}

struct ArtworkPaletteBucket {
    weight: f32,
    linear_red: f32,
    linear_green: f32,
    linear_blue: f32,
}

#[derive(Default)]
struct ArtworkPalette {
    buckets: HashMap<ArtworkPaletteKey, ArtworkPaletteBucket>,
    foreground_evidence: f32,
}

impl ArtworkPalette {
    fn add(&mut self, red: u8, green: u8, blue: u8, weight: f32, foreground_evidence: f32) {
        let (hue, saturation, _) = rgb_to_hsl(red as f32, green as f32, blue as f32);
        let key = if saturation < ARTWORK_NEUTRAL_SATURATION_THRESHOLD {
            ArtworkPaletteKey::Neutral
        } else {
            ArtworkPaletteKey::Hue(
                (hue * ARTWORK_HUE_BUCKET_COUNT as f32)
                    .floor()
                    .clamp(0.0, (ARTWORK_HUE_BUCKET_COUNT - 1) as f32) as usize,
            )
        };
        let bucket = self
            .buckets
            .entry(key)
            .or_insert_with(|| ArtworkPaletteBucket {
                weight: 0.0,
                linear_red: 0.0,
                linear_green: 0.0,
                linear_blue: 0.0,
            });
        bucket.weight += weight;
        bucket.linear_red += srgb_to_linear(red as f32 / 255.0) * weight;
        bucket.linear_green += srgb_to_linear(green as f32 / 255.0) * weight;
        bucket.linear_blue += srgb_to_linear(blue as f32 / 255.0) * weight;
        self.foreground_evidence += foreground_evidence;
    }

    fn has_enough_foreground(&self, minimum: f32) -> bool {
        self.foreground_evidence >= minimum
    }

    fn dominant_rgb(&self) -> Option<(f32, f32, f32)> {
        let (_, bucket) = self
            .buckets
            .iter()
            .max_by(|(left_key, left), (right_key, right)| {
                left.weight
                    .partial_cmp(&right.weight)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| left_key.cmp(right_key))
            })?;
        if bucket.weight <= 0.0 {
            return None;
        }
        Some((
            linear_to_srgb(bucket.linear_red / bucket.weight) * 255.0,
            linear_to_srgb(bucket.linear_green / bucket.weight) * 255.0,
            linear_to_srgb(bucket.linear_blue / bucket.weight) * 255.0,
        ))
    }
}

pub struct SystemMediaService {
    player: OnceLock<Result<AdapterClient, String>>,
    artwork_cache: Mutex<Option<PlaybackArtwork>>,
}

struct AdapterClient {
    player: NowPlayingPerl,
    latest: Arc<RwLock<Option<TimedInfo>>>,
    resync_requested: Arc<AtomicBool>,
    script_path: PathBuf,
    framework_path: PathBuf,
}

#[derive(Clone)]
struct TimedInfo {
    info: NowPlayingInfo,
    received_at: Instant,
}

impl Default for SystemMediaService {
    fn default() -> Self {
        Self {
            player: OnceLock::new(),
            artwork_cache: Mutex::new(None),
        }
    }
}

impl SystemMediaService {
    fn player(&self) -> Result<&AdapterClient, String> {
        self.player
            .get_or_init(|| {
                let existing = adapter_directories();
                let player = catch_unwind(AssertUnwindSafe(NowPlayingPerl::new))
                    .map_err(|_| "无法启动系统媒体适配器".to_string())?;
                let latest = Arc::new(RwLock::new(None));
                let latest_for_listener = latest.clone();
                let resync_requested = Arc::new(AtomicBool::new(true));
                let resync_for_listener = resync_requested.clone();
                player.subscribe(move |info| {
                    let next = info.as_ref().cloned().and_then(timed_info);
                    *latest_for_listener
                        .write()
                        .unwrap_or_else(|error| error.into_inner()) = next;
                    resync_for_listener.store(true, Ordering::SeqCst);
                });
                // 适配器的 get 脚本仍用于刷新精确进度；固定版本并定位它刚创建的临时目录，避免自行维护资源副本。
                let directory = adapter_directories()
                    .into_iter()
                    .find(|path| !existing.contains(path))
                    .ok_or_else(|| "无法定位系统媒体适配器资源".to_string())?;
                let script_path = directory.join("mediaremote-adapter.pl");
                let framework_path = directory.join("MediaRemoteAdapter.framework");
                if !script_path.is_file() || !framework_path.is_dir() {
                    return Err("系统媒体适配器资源不完整".into());
                }
                let output = run_adapter(
                    &script_path,
                    &framework_path,
                    ["get", "--no-artwork", "--now"],
                )?;
                if !output.status.success() {
                    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    return Err(if detail.is_empty() {
                        "系统媒体适配器自检失败".into()
                    } else {
                        detail
                    });
                }
                Ok(AdapterClient {
                    player,
                    latest,
                    resync_requested,
                    script_path,
                    framework_path,
                })
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    pub fn snapshot(&self) -> PlaybackSnapshot {
        let player = match self.player() {
            Ok(player) => player,
            Err(error) => {
                return PlaybackSnapshot::unavailable_with_code(
                    Some(PlayerKind::System),
                    PlaybackErrorCode::Unavailable,
                    error,
                )
            }
        };
        refresh_elapsed(player);
        let info = player
            .latest
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let Some(info) = info.as_ref() else {
            let snapshot = PlaybackSnapshot::unavailable_with_code(
                Some(PlayerKind::System),
                PlaybackErrorCode::Waiting,
                "未检测到系统正在播放的媒体".into(),
            );
            self.invalidate_artwork_cache(&snapshot);
            return snapshot;
        };
        if info
            .info
            .bundle_id
            .as_deref()
            .is_some_and(|bundle_id| !super::automation::is_application_running(bundle_id))
        {
            let snapshot = PlaybackSnapshot::unavailable_with_code(
                Some(PlayerKind::System),
                PlaybackErrorCode::Waiting,
                "未检测到系统正在播放的媒体".into(),
            );
            self.invalidate_artwork_cache(&snapshot);
            return snapshot;
        }
        let snapshot = snapshot_from_info(info);
        self.invalidate_artwork_cache(&snapshot);
        snapshot
    }

    fn invalidate_artwork_cache(&self, snapshot: &PlaybackSnapshot) {
        let current_id = snapshot.artwork_id.as_deref();
        let mut cache = self
            .artwork_cache
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if cache
            .as_ref()
            .is_some_and(|artwork| Some(artwork.id.as_str()) != current_id)
        {
            *cache = None;
        }
    }

    pub fn control(&self, action: PlaybackAction) -> Result<(), String> {
        let player = self.player()?;
        let accepted = match action {
            PlaybackAction::Play => player.player.play(),
            PlaybackAction::Pause => player.player.pause(),
            PlaybackAction::TogglePlayPause => player.player.toggle(),
            PlaybackAction::Previous => player.player.previous(),
            PlaybackAction::Next => player.player.next(),
        };
        if accepted {
            Ok(())
        } else {
            Err("系统媒体播放器未接受控制命令".into())
        }
    }

    pub fn seek(&self, position_ms: u64) -> Result<(), String> {
        let player = self.player()?;
        let position_micros = position_ms.saturating_mul(1_000);
        let position = position_micros.to_string();
        let output = run_adapter(
            &player.script_path,
            &player.framework_path,
            ["seek", position.as_str()],
        )?;
        if output.status.success() {
            Ok(())
        } else {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(if detail.is_empty() {
                "系统媒体播放器未接受跳转命令".into()
            } else {
                detail
            })
        }
    }

    pub fn artwork(&self, artwork_id: &str) -> Result<Option<PlaybackArtwork>, String> {
        let current = self.snapshot();
        if current.artwork_id.as_deref() != Some(artwork_id) {
            return Ok(None);
        }

        {
            let cache = self
                .artwork_cache
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if cache
                .as_ref()
                .is_some_and(|artwork| artwork.id == artwork_id)
            {
                return Ok((*cache).clone());
            }
        }

        let player = self.player()?;
        let latest = player
            .latest
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let Some(timed) = latest.as_ref() else {
            return Ok(None);
        };
        if snapshot_from_info(timed).artwork_id.as_deref() != Some(artwork_id) {
            return Ok(None);
        }
        let image = timed.info.album_cover.clone();
        let Some(image) = image else {
            return Ok(None);
        };

        let thumbnail = image.thumbnail(MAX_ARTWORK_EDGE_PX, MAX_ARTWORK_EDGE_PX);
        let accent_colors = extract_artwork_accent_colors(&thumbnail);
        let mut encoded = std::io::Cursor::new(Vec::new());
        thumbnail
            .write_to(&mut encoded, ImageFormat::Png)
            .map_err(|error| format!("封面编码失败：{error}"))?;
        let artwork = PlaybackArtwork {
            id: artwork_id.to_string(),
            mime_type: "image/png".into(),
            data_base64: BASE64.encode(encoded.into_inner()),
            accent_color: accent_colors.primary,
            spectrum_colors: accent_colors.spectrum,
        };
        *self
            .artwork_cache
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(artwork.clone());
        Ok(Some(artwork))
    }
}

// 从全图主色和封面分区颜色生成能保持整体统一的频谱渐变。
fn extract_artwork_accent_colors(image: &image::DynamicImage) -> ArtworkAccentColors {
    let sampled = image
        .resize_exact(
            ARTWORK_COLOR_SAMPLE_SIZE,
            ARTWORK_COLOR_SAMPLE_SIZE,
            image::imageops::FilterType::Triangle,
        )
        .to_rgba8();
    let background_rgb = detect_artwork_background(&sampled);
    let mut global_palette = ArtworkPalette::default();
    let mut fallback_global_palette = ArtworkPalette::default();
    let mut region_palettes: [[ArtworkPalette; 3]; 3] =
        std::array::from_fn(|_| std::array::from_fn(|_| ArtworkPalette::default()));

    for (x, y, pixel) in sampled.enumerate_pixels() {
        let [red, green, blue, alpha] = pixel.0;
        if alpha < ARTWORK_COLOR_MIN_ALPHA {
            continue;
        }

        let (weight, foreground_evidence) =
            artwork_pixel_weight((red, green, blue), background_rgb);
        global_palette.add(red, green, blue, weight, foreground_evidence);
        fallback_global_palette.add(red, green, blue, 1.0, 1.0);
        let column = ((x * 3) / sampled.width()).min(2) as usize;
        let row = ((y * 3) / sampled.height()).min(2) as usize;
        region_palettes[column][row].add(red, green, blue, weight, foreground_evidence);
    }

    let global_rgb = if global_palette.has_enough_foreground(ARTWORK_MIN_GLOBAL_FOREGROUND_EVIDENCE)
    {
        global_palette.dominant_rgb()
    } else {
        fallback_global_palette.dominant_rgb()
    };
    let Some(global_rgb) = global_rgb else {
        return ArtworkAccentColors {
            primary: DEFAULT_ARTWORK_ACCENT_COLOR.into(),
            spectrum: PlaybackSpectrumColors {
                left: default_spectrum_column_colors(),
                center: default_spectrum_column_colors(),
                right: default_spectrum_column_colors(),
            },
        };
    };

    let primary = artwork_color_to_hex(
        global_rgb,
        ARTWORK_PRIMARY_MIN_LIGHTNESS,
        ARTWORK_PRIMARY_MAX_LIGHTNESS,
    );
    let spectrum: [PlaybackSpectrumColumnColors; 3] = std::array::from_fn(|column| {
        let colors: [String; 3] = std::array::from_fn(|row| {
            let region_rgb = if region_palettes[column][row]
                .has_enough_foreground(ARTWORK_MIN_REGION_FOREGROUND_EVIDENCE)
            {
                region_palettes[column][row]
                    .dominant_rgb()
                    .unwrap_or(global_rgb)
            } else {
                global_rgb
            };
            let mixed_rgb = blend_artwork_rgb(region_rgb, global_rgb);
            artwork_color_to_hex(
                mixed_rgb,
                ARTWORK_SPECTRUM_MIN_LIGHTNESS,
                ARTWORK_SPECTRUM_MAX_LIGHTNESS,
            )
        });
        PlaybackSpectrumColumnColors {
            top: colors[0].clone(),
            middle: colors[1].clone(),
            bottom: colors[2].clone(),
        }
    });
    let spectrum = PlaybackSpectrumColors {
        left: spectrum[0].clone(),
        center: spectrum[1].clone(),
        right: spectrum[2].clone(),
    };
    ArtworkAccentColors { primary, spectrum }
}

fn detect_artwork_background(image: &image::RgbaImage) -> Option<(f32, f32, f32)> {
    let border_size = ARTWORK_BACKGROUND_BORDER_SIZE
        .min(image.width() / 2)
        .min(image.height() / 2);
    if border_size == 0 {
        return None;
    }

    let mut border_buckets = ArtworkColorBuckets::default();
    let mut valid_border_pixels: u32 = 0;
    for (x, y, pixel) in image.enumerate_pixels() {
        if x >= border_size
            && x < image.width().saturating_sub(border_size)
            && y >= border_size
            && y < image.height().saturating_sub(border_size)
        {
            continue;
        }
        let [red, green, blue, alpha] = pixel.0;
        if alpha < ARTWORK_COLOR_MIN_ALPHA {
            continue;
        }
        valid_border_pixels += 1;
        border_buckets.add(red, green, blue);
    }

    let coverage = border_buckets.dominant_count()? as f32 / valid_border_pixels.max(1) as f32;
    if coverage < ARTWORK_BACKGROUND_MIN_COVERAGE {
        None
    } else {
        border_buckets.dominant_rgb()
    }
}

fn artwork_pixel_weight(rgb: (u8, u8, u8), background_rgb: Option<(f32, f32, f32)>) -> (f32, f32) {
    let Some(background_rgb) = background_rgb else {
        return (1.0, 1.0);
    };
    let distance =
        artwork_color_distance((rgb.0 as f32, rgb.1 as f32, rgb.2 as f32), background_rgb);
    let span =
        (ARTWORK_BACKGROUND_DISTANCE_END - ARTWORK_BACKGROUND_DISTANCE_START).max(f32::EPSILON);
    let normalized = ((distance - ARTWORK_BACKGROUND_DISTANCE_START) / span).clamp(0.0, 1.0);
    let foreground_evidence = normalized * normalized * (3.0 - 2.0 * normalized);
    let weight = ARTWORK_BACKGROUND_WEIGHT_FLOOR
        + (1.0 - ARTWORK_BACKGROUND_WEIGHT_FLOOR) * foreground_evidence;
    (weight, foreground_evidence)
}

fn artwork_color_distance(left: (f32, f32, f32), right: (f32, f32, f32)) -> f32 {
    let red = srgb_to_linear(left.0 / 255.0) - srgb_to_linear(right.0 / 255.0);
    let green = srgb_to_linear(left.1 / 255.0) - srgb_to_linear(right.1 / 255.0);
    let blue = srgb_to_linear(left.2 / 255.0) - srgb_to_linear(right.2 / 255.0);
    (0.2126 * red * red + 0.7152 * green * green + 0.0722 * blue * blue).sqrt()
}

fn artwork_color_to_hex(
    rgb: (f32, f32, f32),
    minimum_lightness: f32,
    maximum_lightness: f32,
) -> String {
    let (hue, saturation, lightness) = rgb_to_hsl(rgb.0, rgb.1, rgb.2);
    let target_lightness = lightness.clamp(minimum_lightness, maximum_lightness);
    // 极暗颜色被提亮时同步降低色度，避免近黑色中微小的通道差异被放大成鲜紫。
    let saturation = if target_lightness > lightness {
        let lift_ratio =
            ((target_lightness - lightness) / target_lightness.max(f32::EPSILON)).clamp(0.0, 1.0);
        saturation * (1.0 - ARTWORK_DARK_SATURATION_REDUCTION * lift_ratio)
    } else {
        saturation
    };
    hsl_to_hex(hue, saturation, target_lightness)
}

fn blend_artwork_rgb(local: (f32, f32, f32), global: (f32, f32, f32)) -> (f32, f32, f32) {
    let blend_channel = |local: f32, global: f32| {
        let local = srgb_to_linear(local / 255.0);
        let global = srgb_to_linear(global / 255.0);
        linear_to_srgb(
            local * ARTWORK_SPECTRUM_LOCAL_WEIGHT + global * ARTWORK_SPECTRUM_GLOBAL_WEIGHT,
        ) * 255.0
    };
    (
        blend_channel(local.0, global.0),
        blend_channel(local.1, global.1),
        blend_channel(local.2, global.2),
    )
}

fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

fn default_spectrum_column_colors() -> PlaybackSpectrumColumnColors {
    PlaybackSpectrumColumnColors {
        top: DEFAULT_ARTWORK_ACCENT_COLOR.into(),
        middle: DEFAULT_ARTWORK_ACCENT_COLOR.into(),
        bottom: DEFAULT_ARTWORK_ACCENT_COLOR.into(),
    }
}

fn rgb_to_hsl(red: f32, green: f32, blue: f32) -> (f32, f32, f32) {
    let red = red / 255.0;
    let green = green / 255.0;
    let blue = blue / 255.0;
    let maximum = red.max(green).max(blue);
    let minimum = red.min(green).min(blue);
    let lightness = (maximum + minimum) / 2.0;

    if maximum == minimum {
        return (0.0, 0.0, lightness);
    }

    let delta = maximum - minimum;
    let saturation = if lightness > 0.5 {
        delta / (2.0 - maximum - minimum)
    } else {
        delta / (maximum + minimum)
    };
    let hue = if maximum == red {
        (green - blue) / delta + if green < blue { 6.0 } else { 0.0 }
    } else if maximum == green {
        (blue - red) / delta + 2.0
    } else {
        (red - green) / delta + 4.0
    };
    (hue / 6.0, saturation, lightness)
}

fn hue_to_rgb(p: f32, q: f32, input: f32) -> f32 {
    let hue = if input < 0.0 {
        input + 1.0
    } else if input > 1.0 {
        input - 1.0
    } else {
        input
    };
    if hue < 1.0 / 6.0 {
        p + (q - p) * 6.0 * hue
    } else if hue < 0.5 {
        q
    } else if hue < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - hue) * 6.0
    } else {
        p
    }
}

fn hsl_to_hex(hue: f32, saturation: f32, lightness: f32) -> String {
    if saturation == 0.0 {
        let value = (lightness * 255.0).round() as u8;
        return format!("#{value:02x}{value:02x}{value:02x}");
    }

    let q = if lightness < 0.5 {
        lightness * (1.0 + saturation)
    } else {
        lightness + saturation - lightness * saturation
    };
    let p = 2.0 * lightness - q;
    let red = (hue_to_rgb(p, q, hue + 1.0 / 3.0) * 255.0).round() as u8;
    let green = (hue_to_rgb(p, q, hue) * 255.0).round() as u8;
    let blue = (hue_to_rgb(p, q, hue - 1.0 / 3.0) * 255.0).round() as u8;
    format!("#{red:02x}{green:02x}{blue:02x}")
}

fn refresh_elapsed(client: &AdapterClient) {
    if client
        .latest
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .is_none()
        || !client.resync_requested.swap(false, Ordering::SeqCst)
    {
        return;
    }
    if let Ok(output) = run_adapter(
        &client.script_path,
        &client.framework_path,
        ["get", "--no-artwork", "--now"],
    ) {
        if output.status.success() {
            sync_elapsed_from_adapter(&client.latest, &output.stdout);
        }
    }
}

fn sync_elapsed_from_adapter(latest: &RwLock<Option<TimedInfo>>, output: &[u8]) -> bool {
    let Ok(payload) = serde_json::from_slice::<Value>(output) else {
        return false;
    };
    let Some(position_ms) = milliseconds(payload.get("elapsedTimeNow").and_then(Value::as_f64))
    else {
        return false;
    };
    let mut latest = latest.write().unwrap_or_else(|error| error.into_inner());
    let Some(timed) = latest.as_mut() else {
        return false;
    };
    let same_track = payload.get("title").and_then(Value::as_str) == timed.info.title.as_deref()
        && payload.get("bundleIdentifier").and_then(Value::as_str)
            == timed.info.bundle_id.as_deref();
    if same_track {
        timed.info.elapsed_time = Some(position_ms as f64 / 1000.0);
        timed.received_at = Instant::now();
    }
    same_track
}

fn run_adapter(
    script_path: &std::path::Path,
    framework_path: &std::path::Path,
    arguments: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
) -> Result<std::process::Output, String> {
    let mut command = Command::new("/usr/bin/perl");
    command.arg(script_path).arg(framework_path).args(arguments);
    run_with_timeout(command, Duration::from_secs(3))
}

fn adapter_directories() -> Vec<PathBuf> {
    std::fs::read_dir(std::env::temp_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            name.to_string_lossy()
                .starts_with("mediaremote-adapter")
                .then(|| entry.path())
        })
        .collect()
}

fn milliseconds(seconds: Option<f64>) -> Option<u64> {
    seconds
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| (value * 1000.0).round() as u64)
}

fn valid_elapsed_time(info: &NowPlayingInfo) -> bool {
    match (info.elapsed_time, info.duration) {
        (Some(elapsed), Some(duration)) => {
            elapsed.is_finite()
                && duration.is_finite()
                && elapsed >= 0.0
                && duration >= 0.0
                && elapsed <= duration + 5.0
        }
        (Some(elapsed), None) => elapsed.is_finite() && elapsed >= 0.0,
        _ => true,
    }
}

fn timed_info(mut info: NowPlayingInfo) -> Option<TimedInfo> {
    // 播放器退出时，系统适配器会发送 null；media-remote 会把它映射为全字段为空的结构体。
    // 这类事件表示媒体已清空，不能作为仍在运行的系统播放器缓存下来。
    if !has_media_identity(&info) {
        return None;
    }
    if !valid_elapsed_time(&info) {
        return None;
    }
    if info.is_playing == Some(true) {
        if let (Some(elapsed), Some(updated_at)) = (info.elapsed_time, info.info_update_time) {
            if let Ok(age) = SystemTime::now().duration_since(updated_at) {
                info.elapsed_time = Some(elapsed + age.as_secs_f64());
            }
        }
    }
    Some(TimedInfo {
        info,
        received_at: Instant::now(),
    })
}

fn has_media_identity(info: &NowPlayingInfo) -> bool {
    info.title
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        || info
            .artist
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        || info
            .album
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        || info.album_cover.is_some()
        || info
            .bundle_id
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        || info
            .bundle_name
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        || info.bundle_icon.is_some()
}

fn normalized_system_metadata(info: &NowPlayingInfo) -> compat::TrackMetadata {
    compat::normalize(
        info.bundle_id.as_deref(),
        compat::TrackMetadata::new(info.title.clone(), info.artist.clone()),
    )
}

#[cfg(test)]
fn system_track_id(info: &NowPlayingInfo) -> Option<String> {
    let metadata = normalized_system_metadata(info);
    system_track_id_from_metadata(info, &metadata)
}

fn system_track_id_from_metadata(
    info: &NowPlayingInfo,
    metadata: &compat::TrackMetadata,
) -> Option<String> {
    let title = metadata.title.as_deref()?;
    let artist = metadata.artist.as_deref().unwrap_or_default();
    Some(format!(
        "system:{}|{}|{}|{}",
        normalized_track_component(info.bundle_id.as_deref().unwrap_or_default()),
        normalized_track_component(title),
        normalized_track_component(artist),
        milliseconds(info.duration).unwrap_or_default(),
    ))
}

// 使用图片实际像素内容计算轻量指纹，避免系统封面更新时复用旧缓存。
fn artwork_fingerprint(image: &image::DynamicImage) -> u64 {
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

fn snapshot_from_info(timed: &TimedInfo) -> PlaybackSnapshot {
    let info = &timed.info;
    let metadata = normalized_system_metadata(info);
    let track_id = system_track_id_from_metadata(info, &metadata);
    let duration_ms = milliseconds(info.duration);
    let elapsed = info.elapsed_time.map(|elapsed| {
        if info.is_playing == Some(true) {
            elapsed + timed.received_at.elapsed().as_secs_f64()
        } else {
            elapsed
        }
    });
    let position_ms = milliseconds(elapsed).map(|position| {
        duration_ms
            .map(|duration| position.min(duration))
            .unwrap_or(position)
    });
    let artwork_id = info.album_cover.as_ref().and_then(|cover| {
        track_id
            .as_ref()
            .map(|track_id| format!("{track_id}|artwork:{:016x}", artwork_fingerprint(cover)))
    });
    PlaybackSnapshot {
        player: Some(PlayerKind::System),
        is_running: true,
        is_playing: info.is_playing.unwrap_or(false),
        track_id,
        title: metadata.title,
        artist: metadata.artist,
        album: info.album.clone(),
        source_app_name: info.bundle_name.clone(),
        source_app_bundle_id: info.bundle_id.clone(),
        artwork_id,
        duration_ms,
        position_ms,
        observed_at_ms: now_ms(),
        error_code: None,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> NowPlayingInfo {
        NowPlayingInfo {
            is_playing: Some(true),
            title: Some(" Test Song ".into()),
            artist: Some("Some Artist".into()),
            album: Some("Album".into()),
            album_cover: None,
            elapsed_time: Some(12.345),
            duration: Some(123.456),
            info_update_time: Some(SystemTime::now()),
            bundle_id: Some("com.example.Player".into()),
            bundle_name: Some("Example Player".into()),
            bundle_icon: None,
        }
    }

    #[test]
    fn converts_system_media_info_to_snapshot() {
        let snapshot = snapshot_from_info(&TimedInfo {
            info: info(),
            received_at: Instant::now(),
        });
        assert_eq!(snapshot.player, Some(PlayerKind::System));
        assert_eq!(snapshot.position_ms, Some(12_345));
        assert_eq!(snapshot.duration_ms, Some(123_456));
        assert_eq!(snapshot.source_app_name.as_deref(), Some("Example Player"));
    }

    #[test]
    fn initial_adapter_snapshot_uses_calculated_current_position() {
        let latest = RwLock::new(Some(TimedInfo {
            info: info(),
            received_at: Instant::now(),
        }));
        assert!(sync_elapsed_from_adapter(
            &latest,
            br#"{"title":" Test Song ","bundleIdentifier":"com.example.Player","elapsedTimeNow":56.86}"#,
        ));
        let timed = latest.read().unwrap();
        assert_eq!(timed.as_ref().unwrap().info.elapsed_time, Some(56.86));
    }

    #[test]
    fn anchors_existing_playback_to_the_media_timestamp() {
        let mut current = info();
        current.info_update_time = Some(SystemTime::now() - Duration::from_secs(30));
        let snapshot = snapshot_from_info(&timed_info(current).unwrap());
        assert!(snapshot
            .position_ms
            .is_some_and(|position| (42_345..43_345).contains(&position)));
    }

    #[test]
    fn system_track_id_includes_source_application() {
        let first = system_track_id(&info()).unwrap();
        let mut other = info();
        other.bundle_id = Some("com.example.Other".into());
        assert_ne!(first, system_track_id(&other).unwrap());
    }

    #[test]
    fn rejects_invalid_times() {
        assert_eq!(milliseconds(Some(f64::NAN)), None);
        assert_eq!(milliseconds(Some(-1.0)), None);
    }

    #[test]
    fn advances_playing_position_from_monotonic_receive_time() {
        let snapshot = snapshot_from_info(&TimedInfo {
            info: info(),
            received_at: Instant::now() - Duration::from_millis(500),
        });
        assert!(snapshot
            .position_ms
            .is_some_and(|value| (12_845..=12_855).contains(&value)));
    }

    #[test]
    fn rejects_dependency_timestamp_overflow() {
        let mut invalid = info();
        invalid.elapsed_time = Some(978_307_212.0);
        assert!(!valid_elapsed_time(&invalid));
    }
}
