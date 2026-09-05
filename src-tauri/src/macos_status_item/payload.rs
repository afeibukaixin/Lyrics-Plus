use std::cell::RefCell;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::Manager;
use zhhz::Region;

use crate::config::{
    ChineseConversion, CompactKaraokeStyle, StatusBarAlignment, SupportingLyricsPriority,
};
use crate::lyrics::conversion::detect_region;
use crate::lyrics::{LyricsDocument, LyricsLine, LyricsTrack, LyricsWord};
use crate::overlay_model::{DoubleLineMode, OverlayLayout};
use crate::AppState;

const AUXILIARY_TIMESTAMP_TOLERANCE_MS: u64 = 500;

thread_local! {
    static TRACK_REGION_CACHE: RefCell<Option<TrackRegionCache>> = const { RefCell::new(None) };
}

struct TrackRegionCache {
    document_identity: (usize, usize),
    original: Option<Region>,
    translation: Option<Region>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RenderLineKind {
    Empty,
    Fallback,
    Primary,
    Next,
    Translation,
    Romanization,
}

pub(super) struct RenderLinePayload {
    pub(super) text: String,
    pub(super) content_key: String,
    pub(super) kind: RenderLineKind,
    pub(super) base_color: String,
    pub(super) highlight_color: String,
    pub(super) sweep_progress: Option<f64>,
    pub(super) scroll_duration: Option<Duration>,
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

pub(super) struct RenderPayload {
    pub(super) lines: [RenderLinePayload; 2],
    pub(super) double_line: bool,
    pub(super) cache_key: String,
    pub(super) width: f64,
    pub(super) font_family: String,
    pub(super) font_size: f64,
    pub(super) vertical_offset: f64,
    pub(super) font_weight: u16,
    pub(super) secondary_font_weight: u16,
    pub(super) alignment: StatusBarAlignment,
    pub(super) is_playing: bool,
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

pub(super) fn render_payload(app: &tauri::AppHandle) -> Option<RenderPayload> {
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
    let presentation = &preferences.presentation;
    let compact = &presentation.compact;
    let double_line = compact.layout == OverlayLayout::Double;
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
        .map(str::to_owned)
        .unwrap_or_else(|| "Lyrics Plus".into());
    let fallback_artist = playback
        .artist
        .as_deref()
        .map(str::trim)
        .filter(|artist| !artist.is_empty())
        .unwrap_or_default()
        .to_owned();
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
    let mut secondary = if double_line && !fallback_artist.is_empty() {
        RenderLinePayload {
            text: fallback_artist.clone(),
            content_key: format!("{track_key}:artist:{fallback_artist}"),
            kind: RenderLineKind::Fallback,
            base_color: inactive_color.clone(),
            highlight_color: highlight_color.clone(),
            sweep_progress: None,
            scroll_duration: None,
        }
    } else {
        RenderLinePayload::empty(
            format!("{track_key}:secondary:empty"),
            inactive_color.clone(),
            highlight_color.clone(),
        )
    };
    let mut current_line_index = None;

    if runtime.track_key == playback_key {
        if let Some(document) = runtime.document.as_ref() {
            let adjusted = position_ms as i128 + document.offset_ms as i128;
            let lines = &document.tracks.original.lines;
            let (original_region, translation_region) = cached_track_regions(document);
            let current_index = lines
                .iter()
                .rposition(|line| line.start_ms as i128 <= adjusted);
            if let Some(index) = current_index {
                if let Some(raw_line) = lines.get(index) {
                    current_line_index = Some(index);
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
                                    sweep_progress(&primary.text, words, adjusted.max(0) as u64);
                            }
                            CompactKaraokeStyle::Highlight => {
                                primary.base_color = highlight_color.clone();
                            }
                        }
                    } else {
                        primary.base_color = highlight_color.clone();
                    }
                    if double_line {
                        secondary = RenderLinePayload::empty(
                            format!("{track_key}:secondary:empty:{}", raw_line.start_ms),
                            inactive_color.clone(),
                            highlight_color.clone(),
                        );
                        let mut supporting = None;
                        let mut try_translation = || {
                            if !compact.show_translation {
                                return None;
                            }
                            document
                                .tracks
                                .translation
                                .as_ref()
                                .and_then(|track| {
                                    find_aligned_auxiliary_line(&track.lines, raw_line)
                                })
                                .map(|line| (line, RenderLineKind::Translation, translation_region))
                        };
                        let mut try_romanization = || {
                            if !compact.show_romanization {
                                return None;
                            }
                            document
                                .tracks
                                .romanization
                                .as_ref()
                                .and_then(|track| {
                                    find_aligned_auxiliary_line(&track.lines, raw_line)
                                })
                                .map(|line| (line, RenderLineKind::Romanization, None))
                        };
                        supporting = match compact.supporting_priority {
                            SupportingLyricsPriority::Translation => {
                                try_translation().or_else(try_romanization)
                            }
                            SupportingLyricsPriority::Romanization => {
                                try_romanization().or_else(try_translation)
                            }
                        };
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
            }
        }
    }

    if double_line
        && compact.double_line_mode == DoubleLineMode::Alternating
        && current_line_index.is_some_and(|index| index % 2 == 1)
        && secondary.kind == RenderLineKind::Next
    {
        std::mem::swap(&mut primary, &mut secondary);
    }

    let style_key = format!(
        ":style:{}:{}:{}:{}:{}:{:?}:{}:{:?}:{}:{}:{}:{}:{}:{}:{}:{}:{:?}:{:?}:{:?}:{:?}",
        preferences.appearance.width,
        &preferences.appearance.font_family,
        preferences.appearance.font_size,
        preferences.appearance.font_weight,
        preferences.appearance.secondary_font_weight,
        presentation.alignment,
        preferences.appearance.vertical_offset,
        preferences.appearance.karaoke_style,
        &primary.base_color,
        &preferences.appearance.highlight_color,
        &inactive_color,
        &translation_color,
        &romanization_color,
        double_line,
        compact.show_translation,
        compact.show_romanization,
        compact.double_line_mode,
        compact.supporting_priority,
        primary.sweep_progress.is_some(),
        secondary.kind,
    );
    primary.content_key.push_str(&style_key);
    secondary.content_key.push_str(&style_key);
    secondary.content_key.push_str(":secondary");

    Some(RenderPayload {
        cache_key: format!("{}|{}", primary.content_key, secondary.content_key),
        lines: [primary, secondary],
        double_line,
        width: preferences.appearance.width as f64,
        font_family: preferences.appearance.font_family,
        font_size: preferences.appearance.font_size as f64,
        vertical_offset: preferences.appearance.vertical_offset,
        font_weight: preferences.appearance.font_weight,
        secondary_font_weight: preferences.appearance.secondary_font_weight,
        alignment: presentation.alignment,
        is_playing: playback.is_playing,
    })
}
