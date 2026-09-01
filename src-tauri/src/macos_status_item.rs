use std::cell::RefCell;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSAttributedStringNSStringDrawing, NSColor, NSFont, NSFontAttributeName, NSFontManager,
    NSFontTraitMask, NSForegroundColorAttributeName,
};
use objc2_foundation::{NSMutableAttributedString, NSPoint, NSRange, NSRect, NSSize, NSString};
use objc2_quartz_core::{CATextLayer, CATransaction};
use tauri::Manager;
use unicode_segmentation::UnicodeSegmentation;

use crate::config::{CompactKaraokeStyle, StatusBarAlignment};
use crate::lyrics::LyricsWord;
use crate::AppState;
use crate::TrayMenuState;

const CONTENT_INSET: f64 = 6.0;
const SCROLL_SPEED_POINTS_PER_SECOND: f64 = 35.0;
const DEFAULT_SCROLL_DURATION_SECONDS: f64 = 4.0;
const MIN_SCROLL_DURATION_SECONDS: f64 = 0.1;
const SCROLL_START_HOLD_PROGRESS: f64 = 0.12;
const SCROLL_END_HOLD_PROGRESS: f64 = 0.88;
const STATUS_BAR_FONT_SIZE_MAX: f64 = 18.0;
const TEXT_LAYER_HEIGHT_PADDING: f64 = 4.0;

thread_local! {
    static TEXT_LAYER: RefCell<Option<Retained<CATextLayer>>> = const { RefCell::new(None) };
}

#[derive(Default)]
struct ScrollState {
    content_key: String,
    changed_at: Option<Instant>,
}

#[derive(Clone)]
struct HighlightRange {
    start: usize,
    length: usize,
}

struct RenderPayload {
    text: String,
    content_key: String,
    width: f64,
    font_family: String,
    font_size: f64,
    vertical_offset: f64,
    font_weight: u16,
    alignment: StatusBarAlignment,
    base_color: String,
    highlight_color: String,
    highlight_ranges: Vec<HighlightRange>,
    scroll_duration: Option<Duration>,
    is_playing: bool,
}

fn scroll_state() -> &'static Mutex<ScrollState> {
    static STATE: OnceLock<Mutex<ScrollState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(ScrollState::default()))
}

fn reset_scroll() {
    *scroll_state()
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = ScrollState::default();
}

pub(crate) fn sync_app_icon_visibility(app: &tauri::AppHandle, visible: bool) -> tauri::Result<()> {
    let Some(tray_state) = app.try_state::<TrayMenuState>() else {
        return Ok(());
    };

    // Tauri 在 macOS 上通过移除 NSStatusItem 实现 set_visible(false)。固定应用图标只使用
    // AppKit 的可见性开关，确保最后一个 WebView 销毁后仍保留状态项及其菜单栏位置。
    if visible {
        tray_state.icon.set_visible(true)?;
    }
    let autosave_name = format!("{}.app-menu", app.config().identifier);
    tray_state.icon.with_inner_tray_icon(move |inner| {
        if let Some(status_item) = inner.ns_status_item() {
            status_item.setAutosaveName(Some(&NSString::from_str(&autosave_name)));
            status_item.setVisible(visible);
        }
    })?;
    Ok(())
}

pub(crate) fn configure_lyrics_icon_identity(app: &tauri::AppHandle) -> tauri::Result<()> {
    let Some(tray_state) = app.try_state::<TrayMenuState>() else {
        return Ok(());
    };
    let autosave_name = format!("{}.lyrics-status-item", app.config().identifier);
    tray_state.lyrics_icon.with_inner_tray_icon(move |inner| {
        if let Some(status_item) = inner.ns_status_item() {
            status_item.setAutosaveName(Some(&NSString::from_str(&autosave_name)));
        }
    })?;
    Ok(())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn current_position_ms(snapshot: &crate::player::PlaybackSnapshot) -> u64 {
    let position = snapshot.position_ms.unwrap_or_default();
    if snapshot.is_playing {
        position.saturating_add(now_ms().saturating_sub(snapshot.observed_at_ms))
    } else {
        position
    }
}

fn highlight_ranges(text: &str, words: &[LyricsWord], position_ms: u64) -> Vec<HighlightRange> {
    let mut byte_cursor = 0;
    let mut ranges = Vec::new();
    for word in words {
        let Some(relative_start) = text[byte_cursor..].find(&word.text) else {
            continue;
        };
        let start = byte_cursor + relative_start;
        let word_end = start + word.text.len();
        byte_cursor = word_end;
        if position_ms <= word.start_ms {
            continue;
        }
        let highlighted_bytes = if word.end_ms <= word.start_ms || position_ms >= word.end_ms {
            word.text.len()
        } else {
            let grapheme_ends = word
                .text
                .grapheme_indices(true)
                .map(|(index, grapheme)| index + grapheme.len())
                .collect::<Vec<_>>();
            let elapsed = position_ms.saturating_sub(word.start_ms) as u128;
            let duration = word.end_ms.saturating_sub(word.start_ms) as u128;
            let sung_count = (elapsed * grapheme_ends.len() as u128 / duration) as usize;
            sung_count
                .checked_sub(1)
                .and_then(|index| grapheme_ends.get(index))
                .copied()
                .unwrap_or_default()
        };
        if highlighted_bytes == 0 {
            continue;
        }
        let end = start + highlighted_bytes;
        ranges.push(HighlightRange {
            start: text[..start].encode_utf16().count(),
            length: text[start..end].encode_utf16().count(),
        });
    }
    ranges
}

fn render_payload(app: &tauri::AppHandle) -> Option<RenderPayload> {
    let state = app.try_state::<AppState>()?;
    let config = state.config.snapshot();
    let preferences = config.lyrics.displays.status_bar;
    if !preferences.enabled {
        return None;
    }

    let playback = state
        .last_snapshot
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    if preferences.hide_when_not_playing && !playback.is_playing {
        return None;
    }
    let playback_key = crate::commands::playback_track_key(&playback);
    let position_ms = current_position_ms(&playback);
    let runtime = state
        .lyrics_runtime
        .read()
        .unwrap_or_else(|error| error.into_inner());

    let mut text = playback
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(|title| format!("♪ {title}"))
        .unwrap_or_else(|| "Lyrics Plus".into());
    let track_key = playback_key.as_deref().unwrap_or_default();
    let mut content_key = format!("{track_key}:fallback:{text}");
    let mut base_color = preferences.appearance.text_color.clone();
    let mut highlighted_ranges = Vec::new();
    let mut scroll_duration = None;

    if runtime.track_key == playback_key {
        if let Some(document) = runtime.document.as_ref() {
            let adjusted = (position_ms as i128 + document.offset_ms as i128).max(0) as u64;
            let lines = &document.tracks.original.lines;
            let current_index = lines.iter().rposition(|line| line.start_ms <= adjusted);
            if let Some(index) = current_index {
                if let Some(raw_line) = lines.get(index) {
                    let line = raw_line.converted_for_output(config.lyrics.chinese_conversion);
                    text = line.text.trim().to_owned();
                    content_key = format!("{track_key}:line:{}:{text}", line.start_ms);
                    scroll_duration = lines
                        .get(index + 1)
                        .map(|next| next.start_ms)
                        .or(raw_line.end_ms)
                        .and_then(|end_ms| end_ms.checked_sub(raw_line.start_ms))
                        .filter(|duration_ms| *duration_ms > 0)
                        .map(Duration::from_millis);
                    if let Some(words) = line.words.as_deref().filter(|words| !words.is_empty()) {
                        match preferences.appearance.karaoke_style {
                            CompactKaraokeStyle::Sweep => {
                                base_color = preferences.appearance.inactive_color.clone();
                                highlighted_ranges = highlight_ranges(&text, words, adjusted);
                            }
                            CompactKaraokeStyle::Highlight => {
                                base_color = preferences.appearance.highlight_color.clone();
                            }
                        }
                    } else {
                        base_color = preferences.appearance.highlight_color.clone();
                    }
                }
            }
        }
    }
    content_key.push_str(&format!(
        ":style:{}:{}:{}:{}:{:?}:{}",
        preferences.appearance.width,
        preferences.appearance.font_family,
        preferences.appearance.font_size,
        preferences.appearance.font_weight,
        preferences.appearance.alignment,
        preferences.appearance.vertical_offset,
    ));

    Some(RenderPayload {
        text,
        content_key,
        width: preferences.appearance.width as f64,
        font_family: preferences.appearance.font_family,
        font_size: preferences.appearance.font_size as f64,
        vertical_offset: preferences.appearance.vertical_offset,
        font_weight: preferences.appearance.font_weight,
        alignment: preferences.appearance.alignment,
        base_color,
        highlight_color: preferences.appearance.highlight_color,
        highlight_ranges: highlighted_ranges,
        scroll_duration,
        is_playing: playback.is_playing,
    })
}

fn system_font_weight(weight: u16) -> f64 {
    match weight {
        0..=449 => 0.0,
        450..=549 => 0.23,
        550..=649 => 0.30,
        650..=749 => 0.40,
        _ => 0.56,
    }
}

fn font_manager_weight(weight: u16) -> isize {
    match weight {
        0..=449 => 5,
        450..=549 => 7,
        550..=649 => 9,
        650..=749 => 11,
        _ => 13,
    }
}

fn resolve_font(
    family_stack: &str,
    size: f64,
    weight: u16,
    mtm: MainThreadMarker,
) -> Retained<NSFont> {
    let manager = NSFontManager::sharedFontManager(mtm);
    for candidate in family_stack.split(',') {
        let family = candidate.trim().trim_matches(['\'', '"']);
        if family.is_empty() {
            continue;
        }
        if matches!(
            family.to_ascii_lowercase().as_str(),
            "-apple-system" | "system-ui" | "sans-serif"
        ) {
            break;
        }
        if let Some(font) = manager.fontWithFamily_traits_weight_size(
            &NSString::from_str(family),
            NSFontTraitMask::empty(),
            font_manager_weight(weight),
            size,
        ) {
            return font;
        }
    }
    NSFont::systemFontOfSize_weight(size, system_font_weight(weight))
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
            if let (Some(red), Some(green), Some(blue)) =
                (component(0), component(2), component(4))
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
                    values.get(3).and_then(|alpha| parse_alpha(alpha)).unwrap_or(1.0),
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

fn scroll_elapsed(content_key: &str, is_playing: bool) -> Duration {
    let now = Instant::now();
    let mut state = scroll_state()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if state.content_key != content_key || !is_playing {
        state.content_key = content_key.to_owned();
        state.changed_at = Some(now);
        return Duration::ZERO;
    }
    let changed_at = *state.changed_at.get_or_insert(now);
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

    let preferred_duration = DEFAULT_SCROLL_DURATION_SECONDS
        .max(maximum / SCROLL_SPEED_POINTS_PER_SECOND);
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

fn render_on_main(payload: RenderPayload, tray: &tauri::tray::TrayIcon) {
    let elapsed = scroll_elapsed(&payload.content_key, payload.is_playing);
    let _ = tray.with_inner_tray_icon(move |inner| {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let Some(status_item) = inner.ns_status_item() else {
            return;
        };
        status_item.setLength(payload.width);
        let Some(button) = status_item.button(mtm) else {
            return;
        };
        let button_height = button.bounds().size.height;
        let font_size = payload
            .font_size
            .min(STATUS_BAR_FONT_SIZE_MAX)
            .min((button_height - TEXT_LAYER_HEIGHT_PADDING).max(10.0));

        button.setTitle(&NSString::from_str(""));
        button.setWantsLayer(true);
        let Some(host_layer) = button.layer() else {
            return;
        };
        host_layer.setMasksToBounds(true);

        let font = resolve_font(
            &payload.font_family,
            font_size,
            payload.font_weight,
            mtm,
        );
        let base_color = native_color(&payload.base_color, (0.96, 0.98, 1.0, 1.0));
        let highlight_color = native_color(&payload.highlight_color, (0.64, 0.90, 0.21, 1.0));
        let string = NSString::from_str(&payload.text);
        let attributed = NSMutableAttributedString::from_nsstring(&string);
        let full_range = NSRange::new(0, string.length());
        let font_object = <NSFont as AsRef<AnyObject>>::as_ref(&font);
        let base_color_object = <NSColor as AsRef<AnyObject>>::as_ref(&base_color);
        unsafe {
            attributed.addAttribute_value_range(NSFontAttributeName, font_object, full_range);
            attributed.addAttribute_value_range(
                NSForegroundColorAttributeName,
                base_color_object,
                full_range,
            );
        }
        let highlight_object = <NSColor as AsRef<AnyObject>>::as_ref(&highlight_color);
        for range in payload.highlight_ranges {
            unsafe {
                attributed.addAttribute_value_range(
                    NSForegroundColorAttributeName,
                    highlight_object,
                    NSRange::new(range.start, range.length),
                );
            }
        }

        let content_width = attributed.size().width.ceil();
        let available_width = (payload.width - CONTENT_INSET * 2.0).max(1.0);
        let overflowing = content_width > available_width;
        let offset = scroll_offset(
            content_width,
            available_width,
            elapsed,
            payload.scroll_duration,
        );
        let layer_height = (font_size + TEXT_LAYER_HEIGHT_PADDING).min(button_height);
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
        // AppKit 可能为状态栏按钮使用翻转坐标系，统一让正值表示视觉上移。
        let vertical_offset = if host_layer.isGeometryFlipped() {
            -payload.vertical_offset
        } else {
            payload.vertical_offset
        };
        let origin_y = (button_height - layer_height) / 2.0 + vertical_offset;

        CATransaction::begin();
        CATransaction::setDisableActions(true);
        TEXT_LAYER.with(|slot| {
            let mut slot = slot.borrow_mut();
            let text_layer = slot.get_or_insert_with(|| {
                let layer = CATextLayer::layer();
                layer.setWrapped(false);
                layer.setContentsScale(2.0);
                host_layer.addSublayer(&layer);
                layer
            });
            let attributed_object =
                <NSMutableAttributedString as AsRef<AnyObject>>::as_ref(&attributed);
            unsafe {
                text_layer.setString(Some(attributed_object));
            }
            text_layer.setFrame(NSRect::new(
                NSPoint::new(origin_x, origin_y),
                NSSize::new(content_width.max(1.0), layer_height),
            ));
        });
        CATransaction::commit();
    });
}

pub(crate) fn sync(app: &tauri::AppHandle) {
    let Some(tray_state) = app.try_state::<TrayMenuState>() else {
        return;
    };
    let payload = render_payload(app);
    let visible = payload.is_some();
    let _ = tray_state.lyrics_icon.with_inner_tray_icon(move |inner| {
        if let Some(status_item) = inner.ns_status_item() {
            status_item.setVisible(visible);
        }
    });
    if let Some(payload) = payload {
        render_on_main(payload, &tray_state.lyrics_icon);
    } else {
        reset_scroll();
    }
}

pub(crate) fn start(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let Some(state) = app.try_state::<AppState>() else {
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            };
            let enabled = state.config.snapshot().lyrics.displays.status_bar.enabled;
            let is_playing = state
                .last_snapshot
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .is_playing;
            let wake = state.status_bar_wake.clone();
            if !enabled {
                wake.notified().await;
                continue;
            }
            sync(&app);
            if is_playing {
                tokio::time::sleep(Duration::from_millis(50)).await;
            } else {
                wake.notified().await;
            }
        }
    });
}

pub(crate) fn wake(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        state.status_bar_wake.notify_one();
    }
}
