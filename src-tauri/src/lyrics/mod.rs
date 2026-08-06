use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub mod kugou;
pub mod lrclib;
pub mod netease;
pub mod provider;
pub mod qqmusic;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsWord {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsLine {
    pub start_ms: u64,
    pub end_ms: Option<u64>,
    pub text: String,
    pub words: Option<Vec<LyricsWord>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsTrack {
    pub lines: Vec<LyricsLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsTracks {
    pub original: LyricsTrack,
    pub translation: Option<LyricsTrack>,
    pub romanization: Option<LyricsTrack>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub source: String,
    pub original_format: String,
    pub manual_selected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsDocument {
    pub metadata: LyricsMetadata,
    pub tracks: LyricsTracks,
    pub offset_ms: i64,
    pub raw: String,
}

fn timestamp_ms(tag: &str) -> Option<u64> {
    let (minutes, seconds) = tag.split_once(':')?;
    let minutes: u64 = minutes.trim().parse().ok()?;
    let seconds: f64 = seconds.trim().parse().ok()?;
    if !(0.0..60.0).contains(&seconds) {
        return None;
    }
    Some(
        minutes
            .saturating_mul(60_000)
            .saturating_add((seconds * 1000.0).round() as u64),
    )
}

fn finish_lines(entries: impl IntoIterator<Item = (u64, String)>) -> Vec<LyricsLine> {
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

#[allow(dead_code)]
pub fn parse_lrc(raw: &str, source: impl Into<String>) -> Result<LyricsDocument, String> {
    parse_lrc_with_options(raw, source, false)
}

pub fn parse_lrc_with_options(
    raw: &str,
    source: impl Into<String>,
    manual_selected: bool,
) -> Result<LyricsDocument, String> {
    let source = source.into();
    if let Some(lines) = parse_ttml_lyrics(raw) {
        return Ok(LyricsDocument {
            metadata: LyricsMetadata {
                title: None,
                artist: None,
                album: None,
                source,
                original_format: "ttml".into(),
                manual_selected,
            },
            tracks: LyricsTracks {
                original: LyricsTrack { lines },
                translation: None,
                romanization: None,
            },
            offset_ms: 0,
            raw: raw.to_string(),
        });
    }
    if let Some((format, lines)) = parse_platform_word_lyrics(raw) {
        let (title, artist, album, embedded_offset) = metadata_tags(raw);
        let (translation, romanization) = parse_auxiliary_lrc_tracks(raw);
        return Ok(LyricsDocument {
            metadata: LyricsMetadata {
                title,
                artist,
                album,
                source,
                original_format: format.into(),
                manual_selected,
            },
            tracks: LyricsTracks {
                original: LyricsTrack { lines },
                translation: (!translation.is_empty())
                    .then_some(LyricsTrack { lines: translation }),
                romanization: (!romanization.is_empty()).then_some(LyricsTrack {
                    lines: romanization,
                }),
            },
            offset_ms: embedded_offset,
            raw: raw.to_string(),
        });
    }
    let mut title = None;
    let mut artist = None;
    let mut album = None;
    let mut embedded_offset = 0_i64;
    let mut timed_text = BTreeMap::<u64, Vec<String>>::new();
    let mut word_timings = BTreeMap::<u64, Vec<LyricsWord>>::new();

    for source_line in raw.lines() {
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

        let (text, words) = parse_enhanced_words(remaining);
        for time_ms in timestamps {
            let texts = timed_text.entry(time_ms).or_default();
            if !texts.contains(&text) {
                texts.push(text.clone());
            }
            if !words.is_empty() {
                word_timings.entry(time_ms).or_insert_with(|| words.clone());
            }
        }
    }

    if timed_text.is_empty() {
        return Err("没有找到带时间标签的歌词行".into());
    }

    let mut original = finish_lines(
        timed_text
            .iter()
            .filter_map(|(time, texts)| texts.first().map(|text| (*time, text.clone()))),
    );
    for line in &mut original {
        if let Some(mut words) = word_timings.remove(&line.start_ms) {
            if let (Some(last), Some(line_end)) = (words.last_mut(), line.end_ms) {
                last.end_ms = last.end_ms.max(line_end);
            }
            line.words = Some(words);
        }
    }
    let translations = finish_lines(
        timed_text
            .iter()
            .filter_map(|(time, texts)| texts.get(1).map(|text| (*time, text.clone()))),
    );
    let romanization = finish_lines(
        timed_text
            .iter()
            .filter_map(|(time, texts)| texts.get(2).map(|text| (*time, text.clone()))),
    );

    Ok(LyricsDocument {
        metadata: LyricsMetadata {
            title,
            artist,
            album,
            source,
            original_format: if original.iter().any(|line| line.words.is_some()) {
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
    })
}

fn metadata_tags(raw: &str) -> (Option<String>, Option<String>, Option<String>, i64) {
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

fn parse_enhanced_words(raw: &str) -> (String, Vec<LyricsWord>) {
    let mut words: Vec<LyricsWord> = Vec::new();
    let mut cursor = 0;
    while let Some(relative_open) = raw[cursor..].find('<') {
        let open = cursor + relative_open;
        let Some(relative_close) = raw[open + 1..].find('>') else {
            break;
        };
        let close = open + 1 + relative_close;
        let Some(start_ms) = timestamp_ms(&raw[open + 1..close]) else {
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
    if words.is_empty() {
        return (raw.trim().to_string(), words);
    }
    let text = words
        .iter()
        .map(|word| word.text.as_str())
        .collect::<String>()
        .trim()
        .to_string();
    (text, words)
}

fn parse_platform_word_lyrics(raw: &str) -> Option<(&'static str, Vec<LyricsLine>)> {
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
        let (detected, words) = if content.trim_start().starts_with('(') {
            ("yrc", parse_yrc_words(content))
        } else {
            ("qrc", parse_qrc_words(content))
        };
        if words.is_empty() {
            continue;
        }
        format.get_or_insert(detected);
        let text = words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<String>();
        lines.push(LyricsLine {
            start_ms,
            end_ms: Some(start_ms.saturating_add(duration_ms)),
            text: text.trim().to_string(),
            words: Some(words),
        });
    }
    lines.sort_by_key(|line| line.start_ms);
    format.map(|format| (format, lines))
}

fn parse_auxiliary_lrc_tracks(raw: &str) -> (Vec<LyricsLine>, Vec<LyricsLine>) {
    let mut timed_text = BTreeMap::<u64, Vec<String>>::new();
    let mut explicit_translation = BTreeMap::<u64, Vec<String>>::new();
    let mut explicit_romanization = BTreeMap::<u64, Vec<String>>::new();
    let mut track_kind = 0_u8;
    for source_line in raw.lines() {
        match source_line.trim() {
            "[lyrics-plus:translation]" => {
                track_kind = 1;
                continue;
            }
            "[lyrics-plus:romanization]" => {
                track_kind = 2;
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
    let translation_source = if explicit_translation.is_empty() {
        &timed_text
    } else {
        &explicit_translation
    };
    let translation = finish_lines(
        translation_source
            .iter()
            .filter_map(|(time, texts)| texts.first().map(|text| (*time, text.clone()))),
    );
    let romanization = if explicit_romanization.is_empty() {
        finish_lines(
            timed_text
                .iter()
                .filter_map(|(time, texts)| texts.get(1).map(|text| (*time, text.clone()))),
        )
    } else {
        finish_lines(
            explicit_romanization
                .iter()
                .filter_map(|(time, texts)| texts.first().map(|text| (*time, text.clone()))),
        )
    };
    (translation, romanization)
}

fn parse_yrc_words(raw: &str) -> Vec<LyricsWord> {
    let mut words: Vec<LyricsWord> = Vec::new();
    let mut cursor = 0;
    while let Some(relative_open) = raw[cursor..].find('(') {
        let open = cursor + relative_open;
        let Some(relative_close) = raw[open + 1..].find(')') else {
            break;
        };
        let close = open + 1 + relative_close;
        let values = parse_integer_list(&raw[open + 1..close]);
        if values.len() != 3 {
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
    words
}

fn parse_qrc_words(raw: &str) -> Vec<LyricsWord> {
    let mut words = Vec::new();
    let mut cursor = 0;
    while let Some(relative_open) = raw[cursor..].find('(') {
        let open = cursor + relative_open;
        let Some(relative_close) = raw[open + 1..].find(')') else {
            break;
        };
        let close = open + 1 + relative_close;
        let values = parse_integer_list(&raw[open + 1..close]);
        if values.len() != 2 {
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
    words
}

fn parse_integer_list(raw: &str) -> Vec<u64> {
    raw.split(',')
        .filter_map(|value| value.trim().parse().ok())
        .collect()
}

fn parse_ttml_lyrics(raw: &str) -> Option<Vec<LyricsLine>> {
    if !raw.contains("<tt") || !raw.contains("<p") {
        return None;
    }
    let mut lines = Vec::new();
    let mut cursor = 0;
    while let Some(relative_open) = raw[cursor..].find("<p") {
        let open = cursor + relative_open;
        let Some(relative_header_end) = raw[open..].find('>') else {
            break;
        };
        let header_end = open + relative_header_end;
        let header = &raw[open..=header_end];
        let Some(relative_close) = raw[header_end + 1..].find("</p>") else {
            break;
        };
        let close = header_end + 1 + relative_close;
        let body = &raw[header_end + 1..close];
        let Some(start_ms) = xml_time_attribute(header, "begin") else {
            cursor = close + 4;
            continue;
        };
        let end_ms = xml_time_attribute(header, "end");
        let words = parse_ttml_spans(body);
        let text = if words.is_empty() {
            decode_xml_text(&strip_xml_tags(body)).trim().to_string()
        } else {
            words
                .iter()
                .map(|word| word.text.as_str())
                .collect::<String>()
                .trim()
                .to_string()
        };
        if !text.is_empty() {
            lines.push(LyricsLine {
                start_ms,
                end_ms,
                text,
                words: (!words.is_empty()).then_some(words),
            });
        }
        cursor = close + 4;
    }
    lines.sort_by_key(|line| line.start_ms);
    (!lines.is_empty()).then_some(lines)
}

fn parse_ttml_spans(raw: &str) -> Vec<LyricsWord> {
    let mut words = Vec::new();
    let mut cursor = 0;
    while let Some(relative_open) = raw[cursor..].find("<span") {
        let open = cursor + relative_open;
        let Some(relative_header_end) = raw[open..].find('>') else {
            break;
        };
        let header_end = open + relative_header_end;
        let header = &raw[open..=header_end];
        let Some(relative_close) = raw[header_end + 1..].find("</span>") else {
            break;
        };
        let close = header_end + 1 + relative_close;
        let (Some(start_ms), Some(end_ms)) = (
            xml_time_attribute(header, "begin"),
            xml_time_attribute(header, "end"),
        ) else {
            cursor = close + 7;
            continue;
        };
        let text = decode_xml_text(&strip_xml_tags(&raw[header_end + 1..close]));
        if !text.is_empty() && end_ms >= start_ms {
            words.push(LyricsWord {
                start_ms,
                end_ms,
                text,
            });
        }
        cursor = close + 7;
    }
    words
}

fn xml_time_attribute(raw: &str, name: &str) -> Option<u64> {
    let marker = format!("{name}=");
    let start = raw.find(&marker)? + marker.len();
    let quote = raw[start..].chars().next()?;
    if !matches!(quote, '\'' | '"') {
        return None;
    }
    let value_start = start + quote.len_utf8();
    let value_end = value_start + raw[value_start..].find(quote)?;
    parse_ttml_time(&raw[value_start..value_end])
}

fn parse_ttml_time(raw: &str) -> Option<u64> {
    if let Some(value) = raw.strip_suffix("ms") {
        return value.trim().parse().ok();
    }
    if let Some(value) = raw.strip_suffix('s') {
        return value
            .trim()
            .parse::<f64>()
            .ok()
            .map(|value| (value * 1000.0).round() as u64);
    }
    let parts = raw.split(':').collect::<Vec<_>>();
    let (hours, minutes, seconds): (u64, u64, f64) = match parts.as_slice() {
        [minutes, seconds] => (0_u64, minutes.parse().ok()?, seconds.parse::<f64>().ok()?),
        [hours, minutes, seconds] => (
            hours.parse().ok()?,
            minutes.parse().ok()?,
            seconds.parse::<f64>().ok()?,
        ),
        _ => return None,
    };
    Some(
        hours
            .saturating_mul(3_600_000)
            .saturating_add(minutes.saturating_mul(60_000))
            .saturating_add((seconds * 1000.0).round() as u64),
    )
}

fn strip_xml_tags(raw: &str) -> String {
    let mut output = String::new();
    let mut inside = false;
    for character in raw.chars() {
        match character {
            '<' => inside = true,
            '>' => inside = false,
            _ if !inside => output.push(character),
            _ => {}
        }
    }
    output
}

fn decode_xml_text(raw: &str) -> String {
    raw.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_metadata_multiple_timestamps_and_offset() {
        let raw = "[ti:Song]\n[ar:Artist]\n[offset:120]\n[00:01.00][00:02.500]Hello\n[00:03]World";
        let result = parse_lrc(raw, "test").unwrap();
        assert_eq!(result.metadata.title.as_deref(), Some("Song"));
        assert_eq!(result.metadata.artist.as_deref(), Some("Artist"));
        assert_eq!(result.offset_ms, 120);
        assert_eq!(result.tracks.original.lines.len(), 3);
        assert_eq!(result.tracks.original.lines[1].start_ms, 2500);
        assert_eq!(result.tracks.original.lines[1].end_ms, Some(3000));
    }

    #[test]
    fn separates_translation_lines_with_matching_timestamps() {
        let raw = "[00:01.00]Hello\n[00:01.00]你好\n[00:03.00]World\n[00:03.00]世界";
        let result = parse_lrc_with_options(raw, "test", true).unwrap();
        assert_eq!(result.tracks.original.lines[0].text, "Hello");
        assert_eq!(
            result.tracks.translation.as_ref().unwrap().lines[1].text,
            "世界"
        );
        assert!(result.metadata.manual_selected);
    }

    #[test]
    fn duplicate_text_is_not_misclassified_as_translation() {
        let raw = "[00:01.00]Hello\n[00:01.00]Hello";
        let result = parse_lrc(raw, "test").unwrap();
        assert!(result.tracks.translation.is_none());
    }

    #[test]
    fn rejects_unsynchronised_text() {
        assert!(parse_lrc("hello\nworld", "test").is_err());
    }

    #[test]
    fn parses_enhanced_lrc_word_timestamps_without_exposing_tags() {
        let raw = "[00:01.00]<00:01.00>Hello <00:01.50>world\n[00:03.00]Next";
        let result = parse_lrc(raw, "test").unwrap();
        let line = &result.tracks.original.lines[0];
        assert_eq!(result.metadata.original_format, "enhanced_lrc");
        assert_eq!(line.text, "Hello world");
        assert_eq!(line.words.as_ref().unwrap()[0].end_ms, 1500);
        assert_eq!(line.words.as_ref().unwrap()[1].end_ms, 3000);
    }

    #[test]
    fn parses_yrc_absolute_word_ranges() {
        let raw =
            "[ti:Song]\n[1000,1200](1000,400,0)你(1400,800,0)好\n[00:01.00]Hello\n[00:01.00]ni hao";
        let result = parse_lrc(raw, "test").unwrap();
        let line = &result.tracks.original.lines[0];
        assert_eq!(result.metadata.original_format, "yrc");
        assert_eq!(line.text, "你好");
        assert_eq!(line.end_ms, Some(2200));
        assert_eq!(line.words.as_ref().unwrap()[1].start_ms, 1400);
        assert_eq!(line.words.as_ref().unwrap()[1].end_ms, 2200);
        assert_eq!(
            result.tracks.translation.as_ref().unwrap().lines[0].text,
            "Hello"
        );
        assert_eq!(
            result.tracks.romanization.as_ref().unwrap().lines[0].text,
            "ni hao"
        );
    }

    #[test]
    fn parses_qrc_trailing_word_ranges() {
        let raw = "[1000,1200]Hello (1000,400)world(1400,800)";
        let result = parse_lrc(raw, "test").unwrap();
        let words = result.tracks.original.lines[0].words.as_ref().unwrap();
        assert_eq!(result.metadata.original_format, "qrc");
        assert_eq!(words[0].text, "Hello ");
        assert_eq!(words[1].start_ms, 1400);
    }

    #[test]
    fn third_same_timestamp_line_is_reserved_as_romanization() {
        let raw = "[00:01]今日は\n[00:01]今天\n[00:01]kyou wa";
        let result = parse_lrc(raw, "test").unwrap();
        assert_eq!(
            result.tracks.translation.as_ref().unwrap().lines[0].text,
            "今天"
        );
        assert_eq!(
            result.tracks.romanization.as_ref().unwrap().lines[0].text,
            "kyou wa"
        );
    }

    #[test]
    fn parses_ttml_explicit_word_ranges() {
        let raw = r#"<tt xmlns="http://www.w3.org/ns/ttml"><body><div>
          <p begin="00:00:01.000" end="00:00:03.000"><span begin="1s" end="1.5s">Hello </span><span begin="1500ms" end="3s">&amp; world</span></p>
        </div></body></tt>"#;
        let result = parse_lrc(raw, "test").unwrap();
        let line = &result.tracks.original.lines[0];
        assert_eq!(result.metadata.original_format, "ttml");
        assert_eq!(line.text, "Hello & world");
        assert_eq!(line.words.as_ref().unwrap()[1].start_ms, 1500);
        assert_eq!(line.words.as_ref().unwrap()[1].end_ms, 3000);
    }
}
