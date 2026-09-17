use std::cell::RefCell;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::payload::{RenderLinePayload, RenderPayload};
use crate::config::StatusBarAlignment;
use crate::font_weight::{font_manager_weight, system_font_weight};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSAttributedStringNSStringDrawing, NSColor, NSFont, NSFontAttributeName, NSFontManager,
    NSFontTraitMask, NSForegroundColorAttributeName,
};
use objc2_core_graphics::CGColor;
use objc2_foundation::{NSMutableAttributedString, NSPoint, NSRange, NSRect, NSSize, NSString};
use objc2_quartz_core::{CALayer, CATextLayer, CATransaction};
use unicode_segmentation::UnicodeSegmentation;

const CONTENT_INSET: f64 = 6.0;
const SCROLL_SPEED_POINTS_PER_SECOND: f64 = 35.0;
const DEFAULT_SCROLL_DURATION_SECONDS: f64 = 4.0;
const MIN_SCROLL_DURATION_SECONDS: f64 = 0.1;
const SCROLL_START_HOLD_PROGRESS: f64 = 0.12;
const SCROLL_END_HOLD_PROGRESS: f64 = 0.88;
const STATUS_BAR_FONT_SIZE_MAX: f64 = 18.0;
const TEXT_LAYER_HEIGHT_PADDING: f64 = 4.0;
const DOUBLE_LINE_ROW_GAP: f64 = 0.0;
const DOUBLE_LINE_TEXT_LAYER_PADDING: f64 = 1.0;

thread_local! {
    static LAYER_CACHE: RefCell<Option<LayerCache>> = const { RefCell::new(None) };
}

struct LayerCache {
    host_layer_id: usize,
    cache_key: String,
    rows: Vec<LayerRowCache>,
}

struct LayerRowCache {
    base_layer: Retained<CATextLayer>,
    highlight_layer: Option<Retained<CATextLayer>>,
    highlight_mask: Option<Retained<CALayer>>,
    content_width: f64,
}

#[derive(Default)]
struct ScrollState {
    rows: [ScrollRowState; 2],
}

#[derive(Default)]
struct ScrollRowState {
    content_key: String,
    changed_at: Option<Instant>,
}

fn scroll_state() -> &'static Mutex<ScrollState> {
    static STATE: OnceLock<Mutex<ScrollState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(ScrollState::default()))
}

pub(super) fn reset_scroll() {
    *scroll_state()
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = ScrollState::default();
}

pub(super) fn reset() {
    super::payload::reset_position();
    LAYER_CACHE.with(|slot| {
        let Some(cache) = slot.borrow_mut().take() else {
            return;
        };
        for row in cache.rows {
            row.base_layer.removeFromSuperlayer();
            if let Some(layer) = row.highlight_layer {
                layer.removeFromSuperlayer();
            }
        }
    });
    reset_scroll();
}

fn split_font_family_stack(value: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' && quote.is_some() {
            escaped = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if character == active_quote {
                quote = None;
            } else {
                current.push(character);
            }
        } else if matches!(character, '\'' | '"') {
            quote = Some(character);
        } else if character == ',' {
            candidates.push(current.trim().to_owned());
            current.clear();
        } else {
            current.push(character);
        }
    }
    if escaped {
        current.push('\\');
    }
    candidates.push(current.trim().to_owned());
    candidates
}

fn is_system_font_family(family: &str) -> bool {
    matches!(
        family.trim().to_ascii_lowercase().as_str(),
        "-apple-system" | "system-ui" | "blinkmacsystemfont" | "sans-serif"
    )
}

fn resolve_fonts(
    family_stack: &str,
    size: f64,
    weight: u16,
    mtm: MainThreadMarker,
) -> Vec<Retained<NSFont>> {
    let manager = NSFontManager::sharedFontManager(mtm);
    let mut fonts = Vec::new();
    let mut seen_families = Vec::new();
    let mut system_font_added = false;
    for candidate in split_font_family_stack(family_stack) {
        let family = candidate.trim();
        if family.is_empty() {
            continue;
        }
        if is_system_font_family(family) {
            if !system_font_added {
                fonts.push(NSFont::systemFontOfSize_weight(
                    size,
                    system_font_weight(weight),
                ));
                system_font_added = true;
            }
            continue;
        }
        if seen_families
            .iter()
            .any(|seen: &String| seen.eq_ignore_ascii_case(family))
        {
            continue;
        }
        seen_families.push(family.to_owned());
        if let Some(font) = manager.fontWithFamily_traits_weight_size(
            &NSString::from_str(family),
            NSFontTraitMask::empty(),
            font_manager_weight(weight),
            size,
        ) {
            fonts.push(font);
        }
    }
    if !system_font_added {
        fonts.push(NSFont::systemFontOfSize_weight(
            size,
            system_font_weight(weight),
        ));
    }
    fonts
}

fn parse_channel(value: &str) -> Option<f64> {
    let value = value.trim();
    if let Some(percent) = value.strip_suffix('%') {
        return Some(percent.trim().parse::<f64>().ok()?.clamp(0.0, 100.0) / 100.0);
    }
    Some(value.parse::<f64>().ok()?.clamp(0.0, 255.0) / 255.0)
}

fn parse_alpha(value: &str) -> Option<f64> {
    let value = value.trim();
    if let Some(percent) = value.strip_suffix('%') {
        return Some(percent.trim().parse::<f64>().ok()?.clamp(0.0, 100.0) / 100.0);
    }
    Some(value.parse::<f64>().ok()?.clamp(0.0, 1.0))
}

fn parse_color(value: &str, fallback: (f64, f64, f64, f64)) -> (f64, f64, f64, f64) {
    let value = value.trim();
    if value.eq_ignore_ascii_case("transparent") {
        return (0.0, 0.0, 0.0, 0.0);
    }
    if let Some(hex) = value.strip_prefix('#') {
        let expanded = match hex.len() {
            3 | 4 => hex
                .chars()
                .flat_map(|character| [character, character])
                .collect::<String>(),
            6 | 8 => hex.to_owned(),
            _ => String::new(),
        };
        if matches!(expanded.len(), 6 | 8) {
            let component = |start| u8::from_str_radix(&expanded[start..start + 2], 16).ok();
            if let (Some(red), Some(green), Some(blue)) = (component(0), component(2), component(4))
            {
                let alpha = if expanded.len() == 8 {
                    component(6).unwrap_or(255)
                } else {
                    255
                };
                return (
                    red as f64 / 255.0,
                    green as f64 / 255.0,
                    blue as f64 / 255.0,
                    alpha as f64 / 255.0,
                );
            }
        }
    }

    let lower = value.to_ascii_lowercase();
    if (lower.starts_with("rgb(") || lower.starts_with("rgba(")) && lower.ends_with(')') {
        let start = lower.find('(').unwrap_or_default() + 1;
        let values = lower[start..lower.len() - 1]
            .replace('/', ",")
            .split(|character: char| character == ',' || character.is_ascii_whitespace())
            .filter(|part| !part.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if values.len() >= 3 {
            if let (Some(red), Some(green), Some(blue)) = (
                parse_channel(&values[0]),
                parse_channel(&values[1]),
                parse_channel(&values[2]),
            ) {
                return (
                    red,
                    green,
                    blue,
                    values
                        .get(3)
                        .and_then(|alpha| parse_alpha(alpha))
                        .unwrap_or(1.0),
                );
            }
        }
    }
    fallback
}

fn native_color(value: &str, fallback: (f64, f64, f64, f64)) -> Retained<NSColor> {
    let (red, green, blue, alpha) = parse_color(value, fallback);
    NSColor::colorWithSRGBRed_green_blue_alpha(red, green, blue, alpha)
}

fn scroll_elapsed(row_index: usize, content_key: &str, is_playing: bool) -> Duration {
    let now = Instant::now();
    let mut state = scroll_state()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let row = &mut state.rows[row_index.min(1)];
    if row.content_key != content_key || !is_playing {
        row.content_key = content_key.to_owned();
        row.changed_at = Some(now);
        return Duration::ZERO;
    }
    let changed_at = *row.changed_at.get_or_insert(now);
    now.saturating_duration_since(changed_at)
}

fn scroll_offset(
    content_width: f64,
    available_width: f64,
    elapsed: Duration,
    maximum_duration: Option<Duration>,
) -> f64 {
    let maximum = (content_width - available_width).max(0.0);
    if maximum <= 0.0 {
        return 0.0;
    }

    let preferred_duration =
        DEFAULT_SCROLL_DURATION_SECONDS.max(maximum / SCROLL_SPEED_POINTS_PER_SECOND);
    let duration = maximum_duration
        .map(|limit| preferred_duration.min(limit.as_secs_f64()))
        .unwrap_or(preferred_duration)
        .max(MIN_SCROLL_DURATION_SECONDS);
    let progress = (elapsed.as_secs_f64() / duration).clamp(0.0, 1.0);
    if progress <= SCROLL_START_HOLD_PROGRESS {
        return 0.0;
    }
    if progress >= SCROLL_END_HOLD_PROGRESS {
        return maximum;
    }

    let travel_progress = (progress - SCROLL_START_HOLD_PROGRESS)
        / (SCROLL_END_HOLD_PROGRESS - SCROLL_START_HOLD_PROGRESS);
    let eased_progress = travel_progress * travel_progress * (3.0 - 2.0 * travel_progress);
    maximum * eased_progress
}

fn attributed_text(
    text: &str,
    fonts: &[Retained<NSFont>],
    color: &NSColor,
) -> Retained<NSMutableAttributedString> {
    let string = NSString::from_str(text);
    let attributed = NSMutableAttributedString::from_nsstring(&string);
    let full_range = NSRange::new(0, string.length());
    let color_object = <NSColor as AsRef<AnyObject>>::as_ref(color);
    unsafe {
        attributed.addAttribute_value_range(
            NSForegroundColorAttributeName,
            color_object,
            full_range,
        );
    }

    if fonts.is_empty() || text.is_empty() {
        return attributed;
    }

    let coverage = fonts
        .iter()
        .map(|font| font.coveredCharacterSet())
        .collect::<Vec<_>>();
    let font_for_grapheme = |grapheme: &str| {
        coverage
            .iter()
            .position(|characters| {
                grapheme
                    .chars()
                    .all(|character| characters.longCharacterIsMember(character as u32))
            })
            .unwrap_or(coverage.len() - 1)
    };
    let mut runs = Vec::new();
    let mut utf16_offset = 0;
    let mut run_start = 0;
    let mut run_font = None;
    // Keep combining marks and emoji sequences in one font run. NSRange uses
    // UTF-16 code units, so a supplementary character advances by two units.
    for grapheme in text.graphemes(true) {
        let character_font = font_for_grapheme(grapheme);
        if let Some(previous_font) = run_font {
            if previous_font != character_font {
                runs.push((run_start, utf16_offset, previous_font));
                run_start = utf16_offset;
            }
        } else {
            run_start = utf16_offset;
        }
        run_font = Some(character_font);
        utf16_offset += grapheme.encode_utf16().count();
    }
    if let Some(last_font) = run_font {
        runs.push((run_start, utf16_offset, last_font));
    }
    for (start, end, font_index) in runs {
        let font_object = <NSFont as AsRef<AnyObject>>::as_ref(&*fonts[font_index]);
        unsafe {
            attributed.addAttribute_value_range(
                NSFontAttributeName,
                font_object,
                NSRange::new(start, end - start),
            );
        }
    }
    attributed
}

fn layer_id(layer: &CALayer) -> usize {
    layer as *const CALayer as usize
}

fn build_layer_row(
    line: &RenderLinePayload,
    fonts: &[Retained<NSFont>],
    host_layer: &CALayer,
) -> LayerRowCache {
    let base_color = native_color(&line.base_color, (0.96, 0.98, 1.0, 1.0));
    let base_attributed = attributed_text(&line.text, fonts, &base_color);
    let content_width = base_attributed.size().width.ceil();

    let base_layer = CATextLayer::layer();
    base_layer.setWrapped(false);
    base_layer.setContentsScale(2.0);
    let base_object = <NSMutableAttributedString as AsRef<AnyObject>>::as_ref(&base_attributed);
    unsafe {
        base_layer.setString(Some(base_object));
    }
    host_layer.addSublayer(&base_layer);

    let (highlight_layer, highlight_mask) = if line.sweep_progress.is_some() {
        let highlight_color = native_color(&line.highlight_color, (0.64, 0.90, 0.21, 1.0));
        let highlight_attributed = attributed_text(&line.text, fonts, &highlight_color);
        let layer = CATextLayer::layer();
        layer.setWrapped(false);
        layer.setContentsScale(2.0);
        let highlight_object =
            <NSMutableAttributedString as AsRef<AnyObject>>::as_ref(&highlight_attributed);
        unsafe {
            layer.setString(Some(highlight_object));
        }
        let mask = CALayer::layer();
        let mask_color = CGColor::new_generic_gray(1.0, 1.0);
        mask.setBackgroundColor(Some(&mask_color));
        unsafe {
            layer.setMask(Some(&mask));
        }
        host_layer.addSublayer(&layer);
        (Some(layer), Some(mask))
    } else {
        (None, None)
    };

    LayerRowCache {
        base_layer,
        highlight_layer,
        highlight_mask,
        content_width,
    }
}

fn build_layer_cache(
    payload: &RenderPayload,
    host_layer: &CALayer,
    font_size: f64,
    mtm: MainThreadMarker,
    cache_key: String,
) -> LayerCache {
    let primary_fonts = resolve_fonts(&payload.font_family, font_size, payload.font_weight, mtm);
    let secondary_fonts = resolve_fonts(
        &payload.font_family,
        font_size,
        payload.secondary_font_weight,
        mtm,
    );
    let line_count = if payload.double_line { 2 } else { 1 };
    let rows = payload
        .lines
        .iter()
        .take(line_count)
        .enumerate()
        .map(|(index, line)| {
            let fonts = if index == 0 {
                &primary_fonts
            } else {
                &secondary_fonts
            };
            build_layer_row(line, fonts, host_layer)
        })
        .collect();
    LayerCache {
        host_layer_id: layer_id(host_layer),
        cache_key,
        rows,
    }
}

pub(super) fn render_on_main(payload: RenderPayload, tray: &tauri::tray::TrayIcon) {
    let _ = tray.with_inner_tray_icon(move |inner| {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let Some(status_item) = inner.ns_status_item() else {
            return;
        };
        let Some(button) = status_item.button(mtm) else {
            return;
        };
        let button_height = button.bounds().size.height.max(1.0);
        let line_count = if payload.double_line { 2 } else { 1 };
        let row_gap = if payload.double_line {
            DOUBLE_LINE_ROW_GAP
        } else {
            0.0
        };
        let (font_size, row_height, total_height) = if payload.double_line {
            let row_height = ((button_height - row_gap).max(1.0)) / 2.0;
            let max_font_size = (row_height - DOUBLE_LINE_TEXT_LAYER_PADDING).max(1.0);
            let font_size = payload
                .font_size
                .min(STATUS_BAR_FONT_SIZE_MAX)
                .min(max_font_size);
            (font_size, row_height, row_height * 2.0 + row_gap)
        } else {
            let font_size = payload
                .font_size
                .min(STATUS_BAR_FONT_SIZE_MAX)
                .min((button_height - TEXT_LAYER_HEIGHT_PADDING).max(10.0));
            let row_height = (font_size + TEXT_LAYER_HEIGHT_PADDING).min(button_height);
            (font_size, row_height, row_height)
        };

        let host_layer = if let Some(layer) = button.layer() {
            layer
        } else {
            button.setWantsLayer(true);
            let Some(layer) = button.layer() else {
                return;
            };
            layer
        };
        let cache_key = format!(
            "{}:{font_size:.3}:{row_height:.3}:{line_count}",
            payload.cache_key
        );
        let geometry_flipped = host_layer.isGeometryFlipped();

        CATransaction::begin();
        CATransaction::setDisableActions(true);
        LAYER_CACHE.with(|slot| {
            let mut slot = slot.borrow_mut();
            let rebuild = slot
                .as_ref()
                .map(|cache| {
                    cache.host_layer_id != layer_id(&host_layer) || cache.cache_key != cache_key
                })
                .unwrap_or(true);
            if rebuild {
                status_item.setLength(payload.width);
                button.setTitle(&NSString::from_str(""));
                button.setWantsLayer(true);
                host_layer.setMasksToBounds(true);
                if let Some(old_cache) = slot.take() {
                    for row in old_cache.rows {
                        row.base_layer.removeFromSuperlayer();
                        if let Some(layer) = row.highlight_layer {
                            layer.removeFromSuperlayer();
                        }
                    }
                }
                *slot = Some(build_layer_cache(
                    &payload,
                    &host_layer,
                    font_size,
                    mtm,
                    cache_key,
                ));
            }

            let Some(cache) = slot.as_mut() else {
                return;
            };
            let available_width = (payload.width - CONTENT_INSET * 2.0).max(1.0);
            let block_origin_y = (button_height - total_height) / 2.0
                + if geometry_flipped {
                    -payload.vertical_offset
                } else {
                    payload.vertical_offset
                };
            for index in 0..line_count {
                let row = &mut cache.rows[index];
                let line = &payload.lines[index];
                let elapsed = scroll_elapsed(index, &line.content_key, payload.is_playing);
                let content_width = row.content_width;
                let overflowing = content_width > available_width;
                let offset = scroll_offset(
                    content_width,
                    available_width,
                    elapsed,
                    line.scroll_duration,
                );
                let origin_x = if overflowing {
                    CONTENT_INSET - offset
                } else {
                    match payload.alignment {
                        StatusBarAlignment::Left => CONTENT_INSET,
                        StatusBarAlignment::Center => {
                            ((payload.width - content_width) / 2.0).max(CONTENT_INSET)
                        }
                        StatusBarAlignment::Right => {
                            (payload.width - CONTENT_INSET - content_width).max(CONTENT_INSET)
                        }
                    }
                };
                let coordinate_index = if geometry_flipped {
                    index
                } else {
                    line_count - 1 - index
                };
                let origin_y = block_origin_y + coordinate_index as f64 * (row_height + row_gap);
                let frame = NSRect::new(
                    NSPoint::new(origin_x, origin_y),
                    NSSize::new(content_width.max(1.0), row_height),
                );
                row.base_layer.setFrame(frame);
                if let (Some(layer), Some(mask)) =
                    (row.highlight_layer.as_ref(), row.highlight_mask.as_ref())
                {
                    layer.setFrame(frame);
                    let progress = line.sweep_progress.unwrap_or_default().clamp(0.0, 1.0);
                    mask.setFrame(NSRect::new(
                        NSPoint::new(0.0, 0.0),
                        NSSize::new(content_width.max(1.0) * progress, row_height),
                    ));
                }
            }
        });
        CATransaction::commit();
        if let Some(position) = payload.presented_position {
            super::payload::commit_position(position);
        } else {
            super::payload::reset_position();
        }
    });
}
