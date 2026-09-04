use std::collections::BTreeMap;

use base64::Engine;
use serde::Deserialize;

use super::super::{LyricsLine, LyricsWord};
use super::basic_lrc::{finish_lines, timestamp_ms};

pub(super) fn metadata_tags(raw: &str) -> (Option<String>, Option<String>, Option<String>, i64) {
    let mut title = None;
    let mut artist = None;
    let mut album = None;
    let mut offset = 0;
    for line in raw.lines() {
        let Some(tag) = line
            .trim()
            .strip_prefix('[')
            .and_then(|line| line.split_once(']').map(|value| value.0))
        else {
            continue;
        };
        let Some((key, value)) = tag.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim().to_ascii_lowercase().as_str() {
            "ti" => title = Some(value.into()),
            "ar" => artist = Some(value.into()),
            "al" => album = Some(value.into()),
            "offset" => offset = value.parse().unwrap_or(0),
            _ => {}
        }
    }
    (title, artist, album, offset)
}

pub(super) struct ParsedEnhancedWords {
    pub(super) text: String,
    pub(super) words: Vec<LyricsWord>,
    pub(super) malformed: bool,
    pub(super) has_word_tags: bool,
}

pub(super) fn parse_enhanced_words(raw: &str) -> ParsedEnhancedWords {
    let mut words: Vec<LyricsWord> = Vec::new();
    let mut malformed = false;
    let has_word_tags = raw.contains('<');
    let mut cursor = 0;
    while let Some(relative_open) = raw[cursor..].find('<') {
        let open = cursor + relative_open;
        let Some(relative_close) = raw[open + 1..].find('>') else {
            malformed = true;
            break;
        };
        let close = open + 1 + relative_close;
        let Some(start_ms) = timestamp_ms(&raw[open + 1..close]) else {
            malformed = true;
            cursor = close + 1;
            continue;
        };
        let text_start = close + 1;
        let text_end = raw[text_start..]
            .find('<')
            .map(|value| text_start + value)
            .unwrap_or(raw.len());
        let text = raw[text_start..text_end].trim_start().to_string();
        if !text.is_empty() {
            if let Some(previous) = words.last_mut() {
                previous.end_ms = start_ms.max(previous.start_ms);
            }
            words.push(LyricsWord {
                start_ms,
                end_ms: start_ms,
                text,
            });
        }
        cursor = text_end;
        if cursor >= raw.len() {
            break;
        }
    }
    if malformed {
        return ParsedEnhancedWords {
            text: recover_platform_text(raw),
            words: Vec::new(),
            malformed,
            has_word_tags,
        };
    }
    if words.is_empty() {
        return ParsedEnhancedWords {
            text: raw.trim().to_string(),
            words,
            malformed,
            has_word_tags,
        };
    }
    let text = words
        .iter()
        .map(|word| word.text.as_str())
        .collect::<String>()
        .trim()
        .to_string();
    ParsedEnhancedWords {
        text,
        words,
        malformed,
        has_word_tags,
    }
}

pub(super) fn parse_platform_word_lyrics(raw: &str) -> Option<(&'static str, Vec<LyricsLine>)> {
    let mut format = None;
    let mut lines = Vec::new();
    for raw_line in raw.lines() {
        let line = raw_line.trim();
        let Some(after_open) = line.strip_prefix('[') else {
            continue;
        };
        let Some(close) = after_open.find(']') else {
            continue;
        };
        let values = parse_integer_list(&after_open[..close]);
        if values.len() != 2 {
            continue;
        }
        let start_ms = values[0];
        let duration_ms = values[1];
        let content = &after_open[close + 1..];
        let content = content.trim_start();
        let (detected, parsed_words) = if content.starts_with('(') {
            ("yrc", parse_yrc_words(content))
        } else if content.starts_with('<') {
            ("krc", parse_krc_words(content, start_ms))
        } else {
            ("qrc", parse_qrc_words(content))
        };
        let text = recover_platform_text(content);
        if parsed_words.malformed {
            if !text.is_empty() {
                format.get_or_insert(detected);
                lines.push(LyricsLine {
                    start_ms,
                    end_ms: Some(start_ms.saturating_add(duration_ms)),
                    text,
                    words: None,
                });
            }
            continue;
        }
        let words = parsed_words.words;
        if words.is_empty() {
            if content.trim().is_empty() {
                lines.push(LyricsLine {
                    start_ms,
                    end_ms: Some(start_ms.saturating_add(duration_ms)),
                    text: String::new(),
                    words: None,
                });
            } else {
                if !text.is_empty() {
                    format.get_or_insert(detected);
                    lines.push(LyricsLine {
                        start_ms,
                        end_ms: Some(start_ms.saturating_add(duration_ms)),
                        text,
                        words: None,
                    });
                }
            }
            continue;
        }
        format.get_or_insert(detected);
        let text = if text.is_empty() {
            words
                .iter()
                .map(|word| word.text.as_str())
                .collect::<String>()
        } else {
            text
        };
        lines.push(LyricsLine {
            start_ms,
            end_ms: Some(start_ms.saturating_add(duration_ms)),
            text: text.trim().to_string(),
            words: Some(words),
        });
    }
    lines.sort_by_key(|line| line.start_ms);
    if matches!(format, Some("krc" | "qrc")) {
        repair_platform_line_ends(&mut lines);
    }
    format.map(|format| (format, lines))
}

/// KRC/QRC 的声明行时长偶尔早于最后一个字的结束时间；仅在不越过下一行时修正行尾。
fn repair_platform_line_ends(lines: &mut [LyricsLine]) {
    for index in 0..lines.len() {
        let line = &lines[index];
        let Some(words) = line.words.as_ref() else {
            continue;
        };
        let Some(line_end) = line.end_ms else {
            continue;
        };
        let mut previous_start = None;
        let mut valid = true;
        for word in words {
            if previous_start.is_some_and(|start| word.start_ms < start)
                || word.start_ms < line.start_ms
                || word.end_ms < word.start_ms
            {
                valid = false;
                break;
            }
            previous_start = Some(word.start_ms);
        }
        if !valid {
            continue;
        }
        let Some(last_word_end) = words.last().map(|word| word.end_ms) else {
            continue;
        };
        if last_word_end <= line_end {
            continue;
        }
        if lines
            .get(index + 1)
            .is_some_and(|next| last_word_end > next.start_ms)
        {
            continue;
        }
        lines[index].end_ms = Some(last_word_end);
    }
}

struct ParsedPlatformWords {
    words: Vec<LyricsWord>,
    malformed: bool,
}

fn recover_platform_text(raw: &str) -> String {
    let mut text = String::new();
    let mut cursor = 0;
    let bytes = raw.as_bytes();
    while cursor < bytes.len() {
        let Some(relative_open) = raw[cursor..].find(|character| matches!(character, '(' | '<'))
        else {
            text.push_str(&raw[cursor..]);
            break;
        };
        let open = cursor + relative_open;
        text.push_str(&raw[cursor..open]);
        let opener = bytes[open];
        let closer = if opener == b'(' { b')' } else { b'>' };
        let Some(relative_close) = raw[open + 1..].find(closer as char) else {
            // 不完整标签后面的可见文本仍应保留，避免整行丢失。
            let tail = &raw[open + 1..];
            if tail
                .chars()
                .any(|character| !character.is_ascii_digit() && !",.-".contains(character))
            {
                text.push_str(tail);
            }
            break;
        };
        cursor = open + 1 + relative_close + 1;
    }
    text.trim().to_string()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KrcLanguagePayload {
    #[serde(default)]
    content: Vec<KrcLanguageTrack>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KrcLanguageTrack {
    #[serde(default)]
    lyric_content: Vec<Vec<String>>,
    #[serde(rename = "type", default)]
    kind: u8,
}

pub(super) fn parse_krc_language_tracks(raw: &str) -> (Vec<LyricsLine>, Vec<LyricsLine>) {
    let Some(encoded) = raw.lines().find_map(krc_language_value) else {
        return (Vec::new(), Vec::new());
    };
    let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(encoded) else {
        return (Vec::new(), Vec::new());
    };
    let Ok(payload) = serde_json::from_slice::<KrcLanguagePayload>(&decoded) else {
        return (Vec::new(), Vec::new());
    };
    let timings = krc_line_timings(raw);
    let mut translation = None;
    let mut romanization = None;
    for track in payload.content {
        let target = match track.kind {
            0 if romanization.is_none() => &mut romanization,
            1 if translation.is_none() => &mut translation,
            _ => continue,
        };
        let lines = language_lines(track.lyric_content, &timings);
        if !lines.is_empty() {
            *target = Some(lines);
        }
    }
    (
        translation.unwrap_or_default(),
        romanization.unwrap_or_default(),
    )
}

fn krc_language_value(source_line: &str) -> Option<&str> {
    let tag = source_line.trim();
    let tag = tag.strip_prefix('[')?.strip_suffix(']')?;
    let (key, value) = tag.split_once(':')?;
    key.trim()
        .eq_ignore_ascii_case("language")
        .then_some(value.trim())
        .filter(|value| !value.is_empty())
}

fn krc_line_timings(raw: &str) -> Vec<(u64, u64)> {
    raw.lines()
        .filter_map(|source_line| {
            let line = source_line.trim();
            let after_open = line.strip_prefix('[')?;
            let close = after_open.find(']')?;
            let values = parse_integer_list(&after_open[..close]);
            if values.len() != 2 || !after_open[close + 1..].trim_start().starts_with('<') {
                return None;
            }
            Some((values[0], values[1]))
        })
        .collect()
}

fn language_lines(content: Vec<Vec<String>>, timings: &[(u64, u64)]) -> Vec<LyricsLine> {
    let mut lines = content
        .into_iter()
        .enumerate()
        .filter_map(|(index, fragments)| {
            let (start_ms, duration_ms) = *timings.get(index)?;
            let text = fragments
                .iter()
                .map(String::as_str)
                .collect::<String>()
                .trim()
                .to_string();
            (!text.is_empty()).then_some(LyricsLine {
                start_ms,
                end_ms: Some(start_ms.saturating_add(duration_ms)),
                text,
                words: None,
            })
        })
        .collect::<Vec<_>>();
    lines.sort_by_key(|line| line.start_ms);
    lines
}

pub(super) fn parse_auxiliary_lrc_tracks(raw: &str) -> (Vec<LyricsLine>, Vec<LyricsLine>) {
    let mut timed_text = BTreeMap::<u64, Vec<String>>::new();
    let mut explicit_translation = BTreeMap::<u64, Vec<String>>::new();
    let mut explicit_romanization = BTreeMap::<u64, Vec<String>>::new();
    let mut explicit_translation_seen = false;
    let mut explicit_romanization_seen = false;
    let mut track_kind = 0_u8;
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
            let Some(close) = after_open.find(']') else {
                break;
            };
            let tag = &after_open[..close];
            remaining = &after_open[close + 1..];
            if let Some(start_ms) = timestamp_ms(tag) {
                timestamps.push(start_ms);
            } else {
                break;
            }
        }
        let text = remaining.trim().to_string();
        if text.is_empty() || text.contains('<') {
            continue;
        }
        for start_ms in timestamps {
            let values = match track_kind {
                1 => explicit_translation.entry(start_ms).or_default(),
                2 => explicit_romanization.entry(start_ms).or_default(),
                _ => timed_text.entry(start_ms).or_default(),
            };
            if !values.contains(&text) {
                values.push(text.clone());
            }
        }
    }
    let translation_source = if explicit_translation_seen {
        &explicit_translation
    } else {
        &timed_text
    };
    let translation = finish_lines(
        translation_source
            .iter()
            .filter_map(|(time, texts)| texts.first().map(|text| (*time, text.clone()))),
    );
    let romanization = if explicit_romanization_seen {
        finish_lines(
            explicit_romanization
                .iter()
                .filter_map(|(time, texts)| texts.first().map(|text| (*time, text.clone()))),
        )
    } else {
        finish_lines(
            timed_text
                .iter()
                .filter_map(|(time, texts)| texts.get(1).map(|text| (*time, text.clone()))),
        )
    };
    (translation, romanization)
}

fn parse_yrc_words(raw: &str) -> ParsedPlatformWords {
    let mut words: Vec<LyricsWord> = Vec::new();
    let mut malformed = false;
    let mut cursor = 0;
    while let Some(relative_open) = raw[cursor..].find('(') {
        let open = cursor + relative_open;
        let Some(relative_close) = raw[open + 1..].find(')') else {
            malformed = true;
            break;
        };
        let close = open + 1 + relative_close;
        let values = parse_integer_list(&raw[open + 1..close]);
        if values.len() != 3 {
            malformed = true;
            cursor = close + 1;
            continue;
        }
        let text_start = close + 1;
        let text_end = raw[text_start..]
            .find('(')
            .map(|value| text_start + value)
            .unwrap_or(raw.len());
        let text = raw[text_start..text_end].to_string();
        if !text.is_empty() {
            words.push(LyricsWord {
                start_ms: values[0],
                end_ms: values[0].saturating_add(values[1]),
                text,
            });
        }
        cursor = text_end;
        if cursor >= raw.len() {
            break;
        }
    }
    ParsedPlatformWords { words, malformed }
}

fn parse_qrc_words(raw: &str) -> ParsedPlatformWords {
    let mut words = Vec::new();
    let mut malformed = false;
    let mut cursor = 0;
    while let Some(relative_open) = raw[cursor..].find('(') {
        let open = cursor + relative_open;
        let Some(relative_close) = raw[open + 1..].find(')') else {
            malformed = true;
            break;
        };
        let close = open + 1 + relative_close;
        let values = parse_integer_list(&raw[open + 1..close]);
        if values.len() != 2 {
            malformed = true;
            cursor = close + 1;
            continue;
        }
        let text = raw[cursor..open].to_string();
        if !text.is_empty() {
            words.push(LyricsWord {
                start_ms: values[0],
                end_ms: values[0].saturating_add(values[1]),
                text,
            });
        }
        cursor = close + 1;
    }
    ParsedPlatformWords { words, malformed }
}

fn parse_krc_words(raw: &str, line_start_ms: u64) -> ParsedPlatformWords {
    let mut words = Vec::new();
    let mut malformed = false;
    let mut cursor = 0;
    while let Some(relative_open) = raw[cursor..].find('<') {
        let open = cursor + relative_open;
        let Some(relative_close) = raw[open + 1..].find('>') else {
            malformed = true;
            break;
        };
        let close = open + 1 + relative_close;
        let values = parse_integer_list(&raw[open + 1..close]);
        if values.len() != 3 {
            malformed = true;
            cursor = close + 1;
            continue;
        }
        let text_start = close + 1;
        let text_end = raw[text_start..]
            .find('<')
            .map(|value| text_start + value)
            .unwrap_or(raw.len());
        let text = raw[text_start..text_end].to_string();
        if !text.is_empty() {
            let start_ms = line_start_ms.saturating_add(values[0]);
            words.push(LyricsWord {
                start_ms,
                end_ms: start_ms.saturating_add(values[1]),
                text,
            });
        }
        cursor = text_end;
        if cursor >= raw.len() {
            break;
        }
    }
    ParsedPlatformWords { words, malformed }
}

pub(in crate::lyrics) fn parse_integer_list(raw: &str) -> Vec<u64> {
    raw.split(',')
        .map(|value| value.trim().parse::<u64>().ok())
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default()
}
