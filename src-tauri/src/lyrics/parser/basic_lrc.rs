use std::collections::BTreeMap;

use super::super::{
    LyricsDocument, LyricsLine, LyricsMetadata, LyricsTrack, LyricsTracks, LyricsWord,
};
use super::platform::parse_enhanced_words;

pub(super) fn timestamp_ms(tag: &str) -> Option<u64> {
    let mut parts = tag.trim().split(':');
    let minutes: u64 = parts.next()?.trim().parse().ok()?;
    let seconds_part = parts.next()?.trim();
    let milliseconds = if let Some(fraction_part) = parts.next() {
        if parts.next().is_some() {
            return None;
        }
        let seconds: u64 = seconds_part.parse().ok()?;
        if seconds >= 60 {
            return None;
        }
        let fraction_part = fraction_part.trim();
        if fraction_part.is_empty()
            || fraction_part.len() > 3
            || !fraction_part
                .chars()
                .all(|character| character.is_ascii_digit())
        {
            return None;
        }
        let fraction: u64 = fraction_part.parse().ok()?;
        let multiplier = 10_u64.pow((3 - fraction_part.len()) as u32);
        seconds
            .saturating_mul(1_000)
            .saturating_add(fraction.saturating_mul(multiplier))
    } else {
        let seconds: f64 = seconds_part.parse().ok()?;
        if !(0.0..60.0).contains(&seconds) {
            return None;
        }
        (seconds * 1_000.0).round() as u64
    };
    Some(minutes.saturating_mul(60_000).saturating_add(milliseconds))
}
pub(super) fn finish_lines(entries: impl IntoIterator<Item = (u64, String)>) -> Vec<LyricsLine> {
    let mut entries = entries.into_iter().collect::<Vec<_>>();
    entries.sort_by_key(|(start_ms, _)| *start_ms);
    (0..entries.len())
        .map(|index| LyricsLine {
            start_ms: entries[index].0,
            end_ms: entries.get(index + 1).map(|entry| entry.0),
            text: entries[index].1.clone(),
            words: None,
        })
        .collect()
}

pub(super) fn text_at_column(texts: &[String], column: usize) -> Option<String> {
    texts
        .iter()
        .filter(|text| !text.trim().is_empty())
        .nth(column)
        .cloned()
}

/// 只有覆盖整首歌词且分布足够广的重复列，才视为普通 LRC 的隐式辅助轨。
/// 片头制作信息即使在多个时间点重复，也不会因为局部数量形成翻译或音译。
pub(super) fn implicit_column_is_stable(
    timed_text: &BTreeMap<u64, Vec<String>>,
    column: usize,
) -> bool {
    let entries = timed_text
        .iter()
        .filter(|(_, texts)| text_at_column(texts, 0).is_some())
        .collect::<Vec<_>>();
    let total = entries.len();
    if total == 0 {
        return false;
    }

    let candidate_times = entries
        .iter()
        .filter_map(|(time, texts)| text_at_column(texts, column).map(|_| **time))
        .collect::<Vec<_>>();
    if candidate_times.len().saturating_mul(2) <= total {
        return false;
    }

    let Some(overall_first) = entries.first().map(|(time, _)| **time) else {
        return false;
    };
    let Some(overall_last) = entries.last().map(|(time, _)| **time) else {
        return false;
    };
    let Some(candidate_first) = candidate_times.first().copied() else {
        return false;
    };
    let Some(candidate_last) = candidate_times.last().copied() else {
        return false;
    };

    let overall_span = overall_last.saturating_sub(overall_first);
    let candidate_span = candidate_last.saturating_sub(candidate_first);
    overall_span == 0 || candidate_span.saturating_mul(2) >= overall_span
}

pub(super) fn select_original_text(
    texts: &[String],
    word_text: Option<&str>,
    prefer_last: bool,
) -> Option<String> {
    if let Some(word_text) = word_text {
        if let Some(text) = texts.iter().find(|text| text.as_str() == word_text) {
            return Some(text.clone());
        }
    }
    if prefer_last {
        texts
            .iter()
            .rev()
            .find(|text| !text.trim().is_empty())
            .cloned()
            .or_else(|| texts.last().cloned())
    } else {
        texts
            .iter()
            .find(|text| !text.trim().is_empty())
            .cloned()
            .or_else(|| texts.first().cloned())
    }
}

pub(super) fn parse_basic_lrc(
    raw: &str,
    source: String,
    manual_selected: bool,
) -> Result<LyricsDocument, String> {
    let mut title = None;
    let mut artist = None;
    let mut album = None;
    let mut embedded_offset = 0_i64;
    let mut timed_text = BTreeMap::<u64, Vec<String>>::new();
    let mut explicit_translation = BTreeMap::<u64, Vec<String>>::new();
    let mut explicit_romanization = BTreeMap::<u64, Vec<String>>::new();
    let mut explicit_translation_seen = false;
    let mut explicit_romanization_seen = false;
    let mut track_kind = 0_u8;
    let mut word_timings = BTreeMap::<u64, (String, Vec<LyricsWord>)>::new();
    let mut saw_enhanced_words = false;

    for source_line in raw.lines() {
        match source_line.trim() {
            "[lyrics-plus:translation]" => {
                track_kind = 1;
                explicit_translation_seen = true;
                continue;
            }
            "[lyrics-plus:romanization]" => {
                track_kind = 2;
                explicit_romanization_seen = true;
                continue;
            }
            _ => {}
        }
        let mut remaining = source_line.trim_start();
        let mut timestamps = Vec::new();

        while let Some(after_open) = remaining.strip_prefix('[') {
            let Some(end) = after_open.find(']') else {
                break;
            };
            let tag = &after_open[..end];
            remaining = &after_open[end + 1..];
            if let Some(time) = timestamp_ms(tag) {
                timestamps.push(time);
                continue;
            }

            if let Some((key, value)) = tag.split_once(':') {
                let value = value.trim().to_string();
                match key.trim().to_ascii_lowercase().as_str() {
                    "ti" => title = Some(value),
                    "ar" => artist = Some(value),
                    "al" => album = Some(value),
                    "offset" => embedded_offset = value.parse().unwrap_or(0),
                    _ => {}
                }
            }
        }

        let parsed_words = parse_enhanced_words(remaining);
        saw_enhanced_words |= parsed_words.has_word_tags;
        let malformed_words = parsed_words.malformed;
        let text = parsed_words.text;
        let words = parsed_words.words;
        for time_ms in timestamps {
            let texts = match track_kind {
                1 => explicit_translation.entry(time_ms).or_default(),
                2 => explicit_romanization.entry(time_ms).or_default(),
                _ => timed_text.entry(time_ms).or_default(),
            };
            if !texts.contains(&text) {
                texts.push(text.clone());
            }
            if track_kind == 0 && !malformed_words && !words.is_empty() {
                word_timings
                    .entry(time_ms)
                    .or_insert_with(|| (text.clone(), words.clone()));
            }
        }
    }

    if timed_text.is_empty() {
        return Err("没有找到带时间标签的歌词行".into());
    }

    let implicit_translation =
        !explicit_translation_seen && implicit_column_is_stable(&timed_text, 1);
    let implicit_romanization = !explicit_romanization_seen
        && (explicit_translation_seen || implicit_translation)
        && implicit_column_is_stable(&timed_text, 2);
    let implicit_width = if implicit_romanization {
        3
    } else if implicit_translation {
        2
    } else {
        1
    };
    let prefer_last_on_collision = !explicit_translation_seen && !explicit_romanization_seen;

    let mut original = finish_lines(timed_text.iter().filter_map(|(time, texts)| {
        let word_text = word_timings.get(time).map(|(text, _)| text.as_str());
        let has_extra_column = text_at_column(texts, implicit_width).is_some();
        let prefer_last = prefer_last_on_collision && has_extra_column;
        select_original_text(texts, word_text, prefer_last).map(|text| (*time, text))
    }));
    for line in &mut original {
        if let Some((_, mut words)) = word_timings.remove(&line.start_ms) {
            if let (Some(last), Some(line_end)) = (words.last_mut(), line.end_ms) {
                last.end_ms = last.end_ms.max(line_end);
            }
            line.words = Some(words);
        }
    }
    let translations = if explicit_translation_seen {
        finish_lines(
            explicit_translation
                .iter()
                .filter_map(|(time, texts)| texts.first().map(|text| (*time, text.clone()))),
        )
    } else if implicit_translation {
        finish_lines(timed_text.iter().filter_map(|(time, texts)| {
            if text_at_column(texts, implicit_width).is_some() {
                return None;
            }
            text_at_column(texts, 1).map(|text| (*time, text))
        }))
    } else {
        Vec::new()
    };
    let romanization = if explicit_romanization_seen {
        finish_lines(
            explicit_romanization
                .iter()
                .filter_map(|(time, texts)| texts.first().map(|text| (*time, text.clone()))),
        )
    } else if implicit_romanization {
        finish_lines(timed_text.iter().filter_map(|(time, texts)| {
            if text_at_column(texts, implicit_width).is_some() {
                return None;
            }
            text_at_column(texts, 2).map(|text| (*time, text))
        }))
    } else {
        Vec::new()
    };

    let document = LyricsDocument {
        metadata: LyricsMetadata {
            title,
            artist,
            album,
            source,
            original_format: if saw_enhanced_words
                || original.iter().any(|line| line.words.is_some())
            {
                "enhanced_lrc".into()
            } else {
                "lrc".into()
            },
            manual_selected,
        },
        tracks: LyricsTracks {
            original: LyricsTrack { lines: original },
            translation: (!translations.is_empty()).then_some(LyricsTrack {
                lines: translations,
            }),
            romanization: (!romanization.is_empty()).then_some(LyricsTrack {
                lines: romanization,
            }),
        },
        offset_ms: embedded_offset,
        raw: raw.to_string(),
    };

    Ok(document)
}
