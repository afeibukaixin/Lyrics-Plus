use std::collections::HashMap;

use super::super::PlaybackSpectrumColors;
use super::super::PlaybackSpectrumColumnColors;

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

pub(super) struct ArtworkAccentColors {
    pub(super) primary: String,
    pub(super) spectrum: PlaybackSpectrumColors,
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

// 从全图主色和封面分区颜色生成能保持整体统一的频谱渐变。
pub(super) fn extract_artwork_accent_colors(image: &image::DynamicImage) -> ArtworkAccentColors {
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
