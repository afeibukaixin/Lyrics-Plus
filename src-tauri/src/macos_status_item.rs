use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::MainThreadMarker;
use objc2::{define_class, msg_send, sel, AnyThread, DefinedClass};
use objc2_app_kit::{
    NSApplicationDidChangeScreenParametersNotification, NSAttributedStringNSStringDrawing, NSColor,
    NSFont, NSFontAttributeName, NSFontManager, NSFontTraitMask, NSForegroundColorAttributeName,
    NSScreen, NSWindowDidChangeScreenNotification,
};
use objc2_core_foundation::CFRetained;
use objc2_core_graphics::{CGColor, CGDirectDisplayID};
use objc2_core_video::{kCVReturnSuccess, CVDisplayLink, CVOptionFlags, CVReturn, CVTimeStamp};
use objc2_foundation::{
    NSMutableAttributedString, NSNotification, NSNotificationCenter, NSNumber, NSObjectProtocol,
    NSOperationQueue, NSPoint, NSRange, NSRect, NSRunLoop, NSRunLoopCommonModes, NSSize, NSString,
};
use objc2_quartz_core::{CADisplayLink, CALayer, CATextLayer, CATransaction};
use tauri::Manager;
use zhhz::Region;

use crate::config::{ChineseConversion, CompactKaraokeStyle, StatusBarAlignment};
use crate::lyrics::conversion::detect_region;
use crate::lyrics::{LyricsDocument, LyricsLine, LyricsTrack, LyricsWord};
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
const DOUBLE_LINE_ROW_GAP: f64 = 0.0;
const DOUBLE_LINE_TEXT_LAYER_PADDING: f64 = 1.0;
const AUXILIARY_TIMESTAMP_TOLERANCE_MS: u64 = 500;

thread_local! {
    static LAYER_CACHE: RefCell<Option<LayerCache>> = const { RefCell::new(None) };
    static DISPLAY_DRIVER: RefCell<Option<DisplayDriver>> = const { RefCell::new(None) };
    static DISPLAY_OBSERVERS: RefCell<Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>> = const { RefCell::new(Vec::new()) };
    static TRACK_REGION_CACHE: RefCell<Option<TrackRegionCache>> = const { RefCell::new(None) };
}

static FALLBACK_LOOP_STARTED: AtomicBool = AtomicBool::new(false);
static DISPLAY_DRIVER_READY: AtomicBool = AtomicBool::new(false);

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

struct TrackRegionCache {
    document_identity: (usize, usize),
    original: Option<Region>,
    translation: Option<Region>,
}

struct DisplayLinkTargetIvars {
    app: tauri::AppHandle,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[ivars = DisplayLinkTargetIvars]
    struct DisplayLinkTarget;

    unsafe impl NSObjectProtocol for DisplayLinkTarget {}

    impl DisplayLinkTarget {
        #[unsafe(method(displayLinkTick:))]
        fn display_link_tick(&self, link: &CADisplayLink) {
            if should_tick(&self.ivars().app) {
                sync(&self.ivars().app);
            } else {
                link.setPaused(true);
            }
        }
    }
);

impl DisplayLinkTarget {
    fn new(app: tauri::AppHandle) -> Retained<Self> {
        let this = Self::alloc().set_ivars(DisplayLinkTargetIvars { app });
        unsafe { msg_send![super(this), init] }
    }
}

struct CadDisplayLinkState {
    _target: Retained<DisplayLinkTarget>,
    link: Retained<CADisplayLink>,
    display_id: Option<CGDirectDisplayID>,
}

struct CvDisplayLinkContext {
    app: tauri::AppHandle,
    pending: Arc<AtomicBool>,
}

struct CvDisplayLinkState {
    link: CFRetained<CVDisplayLink>,
    _context: Box<CvDisplayLinkContext>,
    display_id: Option<CGDirectDisplayID>,
    running: bool,
}

enum DisplayDriver {
    Cad(CadDisplayLinkState),
    Cv(CvDisplayLinkState),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenderLineKind {
    Empty,
    Fallback,
    Primary,
    Next,
    Translation,
    Romanization,
}

struct RenderLinePayload {
    text: String,
    content_key: String,
    kind: RenderLineKind,
    base_color: String,
    highlight_color: String,
    sweep_progress: Option<f64>,
    scroll_duration: Option<Duration>,
}

impl RenderLinePayload {
    fn empty(content_key: String, inactive_color: String, highlight_color: String) -> Self {
        Self {
            text: String::new(),
            content_key,
            kind: RenderLineKind::Empty,
            base_color: inactive_color,
            highlight_color,
            sweep_progress: None,
            scroll_duration: None,
        }
    }
}

struct RenderPayload {
    lines: [RenderLinePayload; 2],
    double_line: bool,
    cache_key: String,
    width: f64,
    font_family: String,
    font_size: f64,
    vertical_offset: f64,
    font_weight: u16,
    secondary_font_weight: u16,
    alignment: StatusBarAlignment,
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

fn sweep_progress(text: &str, words: &[LyricsWord], position_ms: u64) -> Option<f64> {
    let total_units = text.encode_utf16().count();
    if total_units == 0 {
        return None;
    }

    let mut byte_cursor = 0;
    let mut progress_units: f64 = 0.0;
    let mut matched_word = false;
    for word in words {
        let Some(relative_start) = text[byte_cursor..].find(&word.text) else {
            continue;
        };
        let start = byte_cursor + relative_start;
        let word_end = start + word.text.len();
        byte_cursor = word_end;
        matched_word = true;

        let word_progress = if position_ms <= word.start_ms {
            0.0
        } else if word.end_ms <= word.start_ms || position_ms >= word.end_ms {
            1.0
        } else {
            let elapsed = position_ms.saturating_sub(word.start_ms) as u128;
            let duration = word.end_ms.saturating_sub(word.start_ms) as u128;
            (elapsed as f64 / duration as f64).clamp(0.0, 1.0)
        };
        let prefix_units = text[..start].encode_utf16().count() as f64;
        let word_units = text[start..word_end].encode_utf16().count() as f64;
        progress_units = progress_units.max(prefix_units + word_units * word_progress);
        if position_ms < word.end_ms {
            break;
        }
    }

    matched_word.then(|| (progress_units / total_units as f64).clamp(0.0, 1.0))
}

/// 与歌词窗口保持一致：优先使用相同时间戳，否则取 500ms 内最近的非空辅助行。
fn find_aligned_auxiliary_line<'a>(
    lines: &'a [LyricsLine],
    current_line: &LyricsLine,
) -> Option<&'a LyricsLine> {
    if let Some(exact) = lines
        .iter()
        .find(|line| line.start_ms == current_line.start_ms && !line.text.trim().is_empty())
    {
        return Some(exact);
    }
    lines
        .iter()
        .filter(|line| !line.text.trim().is_empty())
        .min_by_key(|line| line.start_ms.abs_diff(current_line.start_ms))
        .filter(|line| {
            line.start_ms.abs_diff(current_line.start_ms) <= AUXILIARY_TIMESTAMP_TOLERANCE_MS
        })
}

fn line_scroll_duration(lines: &[LyricsLine], index: usize) -> Option<Duration> {
    let line = lines.get(index)?;
    lines
        .get(index + 1)
        .map(|next| next.start_ms)
        .or(line.end_ms)
        .and_then(|end_ms| end_ms.checked_sub(line.start_ms))
        .filter(|duration_ms| *duration_ms > 0)
        .map(Duration::from_millis)
}

fn track_region(track: &LyricsTrack) -> Option<Region> {
    let text = track
        .lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    detect_region(&text)
}

fn cached_track_regions(document: &LyricsDocument) -> (Option<Region>, Option<Region>) {
    let document_identity = (document.raw.as_ptr() as usize, document.raw.len());
    TRACK_REGION_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(regions) = cache
            .as_ref()
            .filter(|regions| regions.document_identity == document_identity)
        {
            return (regions.original, regions.translation);
        }
        let regions = TrackRegionCache {
            document_identity,
            original: track_region(&document.tracks.original),
            translation: document.tracks.translation.as_ref().and_then(track_region),
        };
        let result = (regions.original, regions.translation);
        *cache = Some(regions);
        result
    })
}

fn supporting_line_payload(
    track_key: &str,
    raw_line: &LyricsLine,
    conversion: ChineseConversion,
    source_region: Option<Region>,
    repair_japanese: bool,
    kind: RenderLineKind,
    base_color: String,
    highlight_color: String,
    scroll_duration: Option<Duration>,
) -> RenderLinePayload {
    let line =
        raw_line.converted_for_output_with_region(conversion, source_region, repair_japanese);
    let text = line.text.trim().to_owned();
    RenderLinePayload {
        content_key: format!("{track_key}:{kind:?}:{}:{text}", line.start_ms),
        text,
        kind,
        base_color,
        highlight_color,
        sweep_progress: None,
        scroll_duration,
    }
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

    let fallback_text = playback
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(|title| format!("♪ {title}"))
        .unwrap_or_else(|| "Lyrics Plus".into());
    let track_key = playback_key.as_deref().unwrap_or_default();
    let inactive_color = preferences.appearance.inactive_color.clone();
    let highlight_color = preferences.appearance.highlight_color.clone();
    let translation_color = preferences.appearance.translation_color.clone();
    let romanization_color = preferences.appearance.romanization_color.clone();
    let mut primary = RenderLinePayload {
        text: fallback_text.clone(),
        content_key: format!("{track_key}:fallback:{fallback_text}"),
        kind: RenderLineKind::Fallback,
        base_color: preferences.appearance.text_color.clone(),
        highlight_color: highlight_color.clone(),
        sweep_progress: None,
        scroll_duration: None,
    };
    let mut secondary = RenderLinePayload::empty(
        format!("{track_key}:secondary:empty"),
        inactive_color.clone(),
        highlight_color.clone(),
    );

    if runtime.track_key == playback_key {
        if let Some(document) = runtime.document.as_ref() {
            let adjusted = (position_ms as i128 + document.offset_ms as i128).max(0) as u64;
            let lines = &document.tracks.original.lines;
            let (original_region, translation_region) = cached_track_regions(document);
            let current_index = lines.iter().rposition(|line| line.start_ms <= adjusted);
            if let Some(index) = current_index {
                if let Some(raw_line) = lines.get(index) {
                    let line = raw_line.converted_for_output_with_region(
                        config.lyrics.chinese_conversion,
                        original_region,
                        config.lyrics.repair_simplified_japanese,
                    );
                    primary.text = line.text.trim().to_owned();
                    primary.kind = RenderLineKind::Primary;
                    primary.content_key =
                        format!("{track_key}:primary:{}:{}", line.start_ms, primary.text);
                    primary.scroll_duration = line_scroll_duration(lines, index);
                    if let Some(words) = line.words.as_deref().filter(|words| !words.is_empty()) {
                        match preferences.appearance.karaoke_style {
                            CompactKaraokeStyle::Sweep => {
                                primary.base_color = inactive_color.clone();
                                primary.sweep_progress =
                                    sweep_progress(&primary.text, words, adjusted);
                            }
                            CompactKaraokeStyle::Highlight => {
                                primary.base_color = highlight_color.clone();
                            }
                        }
                    } else {
                        primary.base_color = highlight_color.clone();
                    }
                    if preferences.double_line {
                        let mut supporting = None;
                        if preferences.show_translation {
                            if let Some(track) = document.tracks.translation.as_ref() {
                                if let Some(line) =
                                    find_aligned_auxiliary_line(&track.lines, raw_line)
                                {
                                    supporting = Some((
                                        line,
                                        RenderLineKind::Translation,
                                        translation_region,
                                    ));
                                }
                            }
                        }
                        if supporting.is_none() && preferences.show_romanization {
                            if let Some(track) = document.tracks.romanization.as_ref() {
                                if let Some(line) =
                                    find_aligned_auxiliary_line(&track.lines, raw_line)
                                {
                                    supporting = Some((line, RenderLineKind::Romanization, None));
                                }
                            }
                        }
                        if let Some((raw_supporting, kind, source_region)) = supporting {
                            let color = match kind {
                                RenderLineKind::Translation => translation_color.clone(),
                                RenderLineKind::Romanization => romanization_color.clone(),
                                _ => inactive_color.clone(),
                            };
                            secondary = supporting_line_payload(
                                track_key,
                                raw_supporting,
                                config.lyrics.chinese_conversion,
                                source_region,
                                config.lyrics.repair_simplified_japanese,
                                kind,
                                color,
                                highlight_color.clone(),
                                primary.scroll_duration,
                            );
                        } else if let Some(raw_next) = lines.get(index + 1) {
                            secondary = supporting_line_payload(
                                track_key,
                                raw_next,
                                config.lyrics.chinese_conversion,
                                original_region,
                                config.lyrics.repair_simplified_japanese,
                                RenderLineKind::Next,
                                inactive_color.clone(),
                                highlight_color.clone(),
                                primary.scroll_duration,
                            );
                        }
                    }
                }
            } else if preferences.double_line {
                if let Some(raw_next) = lines.first() {
                    secondary = supporting_line_payload(
                        track_key,
                        raw_next,
                        config.lyrics.chinese_conversion,
                        original_region,
                        config.lyrics.repair_simplified_japanese,
                        RenderLineKind::Next,
                        inactive_color.clone(),
                        highlight_color.clone(),
                        line_scroll_duration(lines, 0),
                    );
                }
            }
        }
    }

    let style_key = format!(
        ":style:{}:{}:{}:{}:{}:{:?}:{}:{:?}:{}:{}:{}:{}:{}:{}:{}:{}:{:?}:{:?}",
        preferences.appearance.width,
        preferences.appearance.font_family,
        preferences.appearance.font_size,
        preferences.appearance.font_weight,
        preferences.appearance.secondary_font_weight,
        preferences.appearance.alignment,
        preferences.appearance.vertical_offset,
        preferences.appearance.karaoke_style,
        primary.base_color,
        preferences.appearance.highlight_color,
        inactive_color,
        translation_color,
        romanization_color,
        preferences.double_line,
        preferences.show_translation,
        preferences.show_romanization,
        primary.sweep_progress.is_some(),
        secondary.kind,
    );
    primary.content_key.push_str(&style_key);
    secondary.content_key.push_str(&style_key);
    secondary.content_key.push_str(":secondary");

    Some(RenderPayload {
        cache_key: format!("{}|{}", primary.content_key, secondary.content_key),
        lines: [primary, secondary],
        double_line: preferences.double_line,
        width: preferences.appearance.width as f64,
        font_family: preferences.appearance.font_family,
        font_size: preferences.appearance.font_size as f64,
        vertical_offset: preferences.appearance.vertical_offset,
        font_weight: preferences.appearance.font_weight,
        secondary_font_weight: preferences.appearance.secondary_font_weight,
        alignment: preferences.appearance.alignment,
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
    font: &NSFont,
    color: &NSColor,
) -> Retained<NSMutableAttributedString> {
    let string = NSString::from_str(text);
    let attributed = NSMutableAttributedString::from_nsstring(&string);
    let full_range = NSRange::new(0, string.length());
    let font_object = <NSFont as AsRef<AnyObject>>::as_ref(font);
    let color_object = <NSColor as AsRef<AnyObject>>::as_ref(color);
    unsafe {
        attributed.addAttribute_value_range(NSFontAttributeName, font_object, full_range);
        attributed.addAttribute_value_range(
            NSForegroundColorAttributeName,
            color_object,
            full_range,
        );
    }
    attributed
}

fn layer_id(layer: &CALayer) -> usize {
    layer as *const CALayer as usize
}

fn build_layer_row(line: &RenderLinePayload, font: &NSFont, host_layer: &CALayer) -> LayerRowCache {
    let base_color = native_color(&line.base_color, (0.96, 0.98, 1.0, 1.0));
    let base_attributed = attributed_text(&line.text, font, &base_color);
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
        let highlight_attributed = attributed_text(&line.text, font, &highlight_color);
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
    let primary_font = resolve_font(&payload.font_family, font_size, payload.font_weight, mtm);
    let secondary_font = resolve_font(
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
            let font = if index == 0 {
                &primary_font
            } else {
                &secondary_font
            };
            build_layer_row(line, font, host_layer)
        })
        .collect();
    LayerCache {
        host_layer_id: layer_id(host_layer),
        cache_key,
        rows,
    }
}

fn render_on_main(payload: RenderPayload, tray: &tauri::tray::TrayIcon) {
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
    });
}

fn menu_bar_display_id(app: &tauri::AppHandle) -> Option<CGDirectDisplayID> {
    let tray_state = app.try_state::<TrayMenuState>()?;
    let display_id = Arc::new(Mutex::new(None));
    let display_id_for_main = display_id.clone();
    let _ = tray_state.lyrics_icon.with_inner_tray_icon(move |inner| {
        let Some(status_item) = inner.ns_status_item() else {
            return;
        };
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let Some(button) = status_item.button(mtm) else {
            return;
        };
        let Some(screen) = button.window().and_then(|window| window.screen()) else {
            return;
        };
        if let Ok(mut target) = display_id_for_main.lock() {
            *target = screen_display_id(&screen);
        }
    });
    display_id.lock().ok().and_then(|target| *target)
}

fn screen_display_id(screen: &NSScreen) -> Option<CGDirectDisplayID> {
    let description = screen.deviceDescription();
    let screen_number_key = NSString::from_str("NSScreenNumber");
    description
        .objectForKey(&screen_number_key)
        .and_then(|value| {
            value
                .downcast_ref::<NSNumber>()
                .map(NSNumber::unsignedIntValue)
        })
}

fn screen_for_display_id(
    display_id: Option<CGDirectDisplayID>,
    mtm: MainThreadMarker,
) -> Option<Retained<NSScreen>> {
    let display_id = display_id?;
    let screens = NSScreen::screens(mtm);
    for index in 0..screens.count() {
        let screen = screens.objectAtIndex(index);
        if screen_display_id(&screen) == Some(display_id) {
            return Some(screen);
        }
    }
    None
}

fn should_tick(app: &tauri::AppHandle) -> bool {
    let Some(state) = app.try_state::<AppState>() else {
        return false;
    };
    let config = state.config.snapshot();
    if !config.lyrics.displays.status_bar.enabled {
        return false;
    }
    let is_playing = state
        .last_snapshot
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .is_playing;
    is_playing
}

impl Drop for CadDisplayLinkState {
    fn drop(&mut self) {
        self.link.invalidate();
    }
}

impl Drop for CvDisplayLinkState {
    #[allow(deprecated)]
    fn drop(&mut self) {
        if self.running {
            let _ = self.link.stop();
            self.running = false;
        }
    }
}

unsafe extern "C-unwind" fn cv_display_link_callback(
    _display_link: NonNull<CVDisplayLink>,
    _in_now: NonNull<CVTimeStamp>,
    _in_output_time: NonNull<CVTimeStamp>,
    _flags_in: CVOptionFlags,
    _flags_out: NonNull<CVOptionFlags>,
    user_info: *mut c_void,
) -> CVReturn {
    let Some(context) = user_info.cast::<CvDisplayLinkContext>().as_ref() else {
        return kCVReturnSuccess;
    };
    if !should_tick(&context.app) || context.pending.swap(true, Ordering::AcqRel) {
        return kCVReturnSuccess;
    }

    let app = context.app.clone();
    let app_for_main = app.clone();
    let pending = context.pending.clone();
    if app
        .run_on_main_thread(move || {
            pending.store(false, Ordering::Release);
            if should_tick(&app_for_main) {
                sync(&app_for_main);
            }
        })
        .is_err()
    {
        context.pending.store(false, Ordering::Release);
    }
    kCVReturnSuccess
}

fn create_cad_display_link(
    app: &tauri::AppHandle,
    screen: Retained<NSScreen>,
    display_id: Option<CGDirectDisplayID>,
) -> DisplayDriver {
    let target = DisplayLinkTarget::new(app.clone());
    let link =
        unsafe { screen.displayLinkWithTarget_selector(target.as_ref(), sel!(displayLinkTick:)) };
    let run_loop = NSRunLoop::mainRunLoop();
    unsafe {
        link.addToRunLoop_forMode(&run_loop, NSRunLoopCommonModes);
    }
    link.setPaused(!should_tick(app));
    DisplayDriver::Cad(CadDisplayLinkState {
        _target: target,
        link,
        display_id,
    })
}

#[allow(deprecated)]
fn create_cv_display_link(
    app: &tauri::AppHandle,
    display_id: Option<CGDirectDisplayID>,
) -> Option<DisplayDriver> {
    let mut raw_link = std::ptr::null_mut::<CVDisplayLink>();
    let output = NonNull::from(&mut raw_link);
    let status = unsafe {
        match display_id {
            Some(display_id) => CVDisplayLink::create_with_cg_display(display_id, output),
            None => CVDisplayLink::create_with_active_cg_displays(output),
        }
    };
    if status != kCVReturnSuccess {
        return None;
    }
    let raw_link = NonNull::new(raw_link)?;
    let link = unsafe { CFRetained::from_raw(raw_link) };
    let pending = Arc::new(AtomicBool::new(false));
    let context = Box::new(CvDisplayLinkContext {
        app: app.clone(),
        pending,
    });
    let user_info = (&*context as *const CvDisplayLinkContext).cast_mut().cast();
    let status = unsafe { link.set_output_callback(Some(cv_display_link_callback), user_info) };
    if status != kCVReturnSuccess {
        return None;
    }
    let mut state = CvDisplayLinkState {
        link,
        _context: context,
        display_id,
        running: false,
    };
    if should_tick(app) && state.link.start() == kCVReturnSuccess {
        state.running = true;
    }
    Some(DisplayDriver::Cv(state))
}

fn install_display_observers(app: &tauri::AppHandle) {
    DISPLAY_OBSERVERS.with(|slot| {
        if !slot.borrow().is_empty() {
            return;
        }
        let center = NSNotificationCenter::defaultCenter();
        let queue = NSOperationQueue::mainQueue();
        let app_for_parameters = app.clone();
        let parameters_block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
            ensure_display_driver(&app_for_parameters);
            update_display_driver_activity(&app_for_parameters);
        });
        let app_for_window = app.clone();
        let window_block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
            ensure_display_driver(&app_for_window);
            update_display_driver_activity(&app_for_window);
        });
        let parameters_observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSApplicationDidChangeScreenParametersNotification),
                None,
                Some(&queue),
                &parameters_block,
            )
        };
        let window_observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSWindowDidChangeScreenNotification),
                None,
                Some(&queue),
                &window_block,
            )
        };
        let mut observers = slot.borrow_mut();
        observers.push(parameters_observer);
        observers.push(window_observer);
    });
}

fn ensure_display_driver(app: &tauri::AppHandle) {
    let display_id = menu_bar_display_id(app);
    let use_cad = objc2::available!(macos = 14.0, ..);
    let needs_rebind = DISPLAY_DRIVER.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|driver| match driver {
                DisplayDriver::Cad(state) => !use_cad || state.display_id != display_id,
                DisplayDriver::Cv(state) => use_cad || state.display_id != display_id,
            })
            .unwrap_or(true)
    });
    if !needs_rebind {
        return;
    }

    DISPLAY_DRIVER.with(|slot| {
        slot.borrow_mut().take();
    });
    DISPLAY_DRIVER_READY.store(false, Ordering::Release);
    if let Some(state) = app.try_state::<AppState>() {
        state.status_bar_wake.notify_one();
    }
    let driver = if use_cad {
        MainThreadMarker::new()
            .and_then(|mtm| screen_for_display_id(display_id, mtm))
            .map(|screen| create_cad_display_link(app, screen, display_id))
    } else {
        create_cv_display_link(app, display_id)
    };
    if let Some(driver) = driver {
        DISPLAY_DRIVER.with(|slot| {
            *slot.borrow_mut() = Some(driver);
        });
        DISPLAY_DRIVER_READY.store(true, Ordering::Release);
    }
}

#[allow(deprecated)]
fn update_display_driver_activity(app: &tauri::AppHandle) {
    let active = should_tick(app);
    DISPLAY_DRIVER.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(driver) = slot.as_mut() else {
            return;
        };
        match driver {
            DisplayDriver::Cad(state) => {
                if state.link.isPaused() == active {
                    state.link.setPaused(!active);
                }
            }
            DisplayDriver::Cv(state) => {
                if active && !state.running {
                    state.running = state.link.start() == kCVReturnSuccess;
                } else if !active && state.running {
                    let _ = state.link.stop();
                    state.running = false;
                }
            }
        }
    });
}

fn start_display_driver(app: &tauri::AppHandle) -> bool {
    install_display_observers(app);
    ensure_display_driver(app);
    update_display_driver_activity(app);
    DISPLAY_DRIVER.with(|slot| slot.borrow().is_some())
}

pub(crate) fn sync(app: &tauri::AppHandle) {
    let Some(tray_state) = app.try_state::<TrayMenuState>() else {
        return;
    };
    let payload = render_payload(app);
    let visible = payload.is_some();
    let _ = tray_state.lyrics_icon.with_inner_tray_icon(move |inner| {
        if let Some(status_item) = inner.ns_status_item() {
            if status_item.isVisible() != visible {
                status_item.setVisible(visible);
            }
        }
    });
    if let Some(payload) = payload {
        render_on_main(payload, &tray_state.lyrics_icon);
    } else {
        reset_scroll();
    }
    update_display_driver_activity(app);
}

pub(crate) fn start(app: tauri::AppHandle) {
    if MainThreadMarker::new().is_some() {
        if start_display_driver(&app) {
            return;
        }
        start_fallback_loop(app);
        return;
    }

    let handle = app.clone();
    if app
        .run_on_main_thread(move || {
            if start_display_driver(&handle) {
                return;
            }
            start_fallback_loop(handle);
        })
        .is_err()
    {
        start_fallback_loop(app);
    }
}

fn start_fallback_loop(app: tauri::AppHandle) {
    if FALLBACK_LOOP_STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        loop {
            if DISPLAY_DRIVER_READY.load(Ordering::Acquire) {
                if let Some(state) = app.try_state::<AppState>() {
                    let wake = state.status_bar_wake.clone();
                    wake.notified().await;
                } else {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                continue;
            }
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
            let handle = app.clone();
            if let Err(error) = app.run_on_main_thread(move || sync(&handle)) {
                log::debug!("Failed to schedule fallback menu bar frame: {error}");
            }
            if is_playing {
                tokio::time::sleep(Duration::from_millis(16)).await;
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
    let handle = app.clone();
    if MainThreadMarker::new().is_some() {
        ensure_display_driver(&handle);
        update_display_driver_activity(&handle);
    } else if let Err(error) = app.run_on_main_thread(move || {
        ensure_display_driver(&handle);
        update_display_driver_activity(&handle);
    }) {
        log::debug!("Failed to wake menu bar display link: {error}");
    }
}
