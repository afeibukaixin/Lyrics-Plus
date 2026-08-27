fn timestamp_ms(tag: &str) -> Option<u64> {
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
            || !fraction_part.chars().all(|character| character.is_ascii_digit())
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
    Some(
        minutes
            .saturating_mul(60_000)
            .saturating_add(milliseconds),
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
    if let Some(tracks) = parse_ttml_lyrics(raw) {
        return Ok(LyricsDocument {
            metadata: LyricsMetadata {
                title: None,
                artist: None,
                album: None,
                source,
                original_format: "ttml".into(),
                manual_selected,
            },
            tracks,
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
    let mut explicit_translation = BTreeMap::<u64, Vec<String>>::new();
    let mut explicit_romanization = BTreeMap::<u64, Vec<String>>::new();
    let mut explicit_translation_seen = false;
    let mut explicit_romanization_seen = false;
    let mut track_kind = 0_u8;
    let mut word_timings = BTreeMap::<u64, Vec<LyricsWord>>::new();

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

        let (text, words) = parse_enhanced_words(remaining);
        for time_ms in timestamps {
            let texts = match track_kind {
                1 => explicit_translation.entry(time_ms).or_default(),
                2 => explicit_romanization.entry(time_ms).or_default(),
                _ => timed_text.entry(time_ms).or_default(),
            };
            if !texts.contains(&text) {
                texts.push(text.clone());
            }
            if track_kind == 0 && !words.is_empty() {
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
    let translations = if explicit_translation_seen {
        finish_lines(
            explicit_translation
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
                .filter_map(|(time, texts)| texts.get(2).map(|text| (*time, text.clone()))),
        )
    };

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
            if content.trim().is_empty() {
                lines.push(LyricsLine {
                    start_ms,
                    end_ms: Some(start_ms.saturating_add(duration_ms)),
                    text: String::new(),
                    words: None,
                });
            }
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

fn parse_ttml_lyrics(raw: &str) -> Option<LyricsTracks> {
    if !raw.contains("<tt") || !raw.contains("<p") {
        return None;
    }
    let mut lines = Vec::new();
    let mut translation_lines = Vec::new();
    let mut romanization_lines = Vec::new();
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
        let main_body = strip_ttml_auxiliary_spans(body);
        let words = parse_ttml_spans(&main_body);
        let text = if words.is_empty() {
            decode_xml_text(&strip_xml_tags(&main_body))
                .trim()
                .to_string()
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
        if let Some(text) = ttml_role_text(body, "x-translation") {
            translation_lines.push(LyricsLine {
                start_ms,
                end_ms,
                text,
                words: None,
            });
        }
        if let Some(text) = ttml_role_text(body, "x-roman") {
            romanization_lines.push(LyricsLine {
                start_ms,
                end_ms,
                text,
                words: None,
            });
        }
        cursor = close + 4;
    }
    lines.sort_by_key(|line| line.start_ms);
    translation_lines.sort_by_key(|line| line.start_ms);
    romanization_lines.sort_by_key(|line| line.start_ms);
    (!lines.is_empty()).then_some(LyricsTracks {
        original: LyricsTrack { lines },
        translation: (!translation_lines.is_empty()).then_some(LyricsTrack {
            lines: translation_lines,
        }),
        romanization: (!romanization_lines.is_empty()).then_some(LyricsTrack {
            lines: romanization_lines,
        }),
    })
}

fn strip_ttml_auxiliary_spans(raw: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    while let Some(relative_open) = raw[cursor..].find("<span") {
        let open = cursor + relative_open;
        output.push_str(&raw[cursor..open]);
        let Some(relative_header_end) = raw[open..].find('>') else {
            output.push_str(&raw[open..]);
            return output;
        };
        let header_end = open + relative_header_end;
        let header = &raw[open..=header_end];
        let Some(relative_close) = raw[header_end + 1..].find("</span>") else {
            output.push_str(&raw[open..]);
            return output;
        };
        let close = header_end + 1 + relative_close + 7;
        if header.contains("x-translation") || header.contains("x-roman") {
            cursor = close;
        } else {
            output.push_str(&raw[open..close]);
            cursor = close;
        }
    }
    output.push_str(&raw[cursor..]);
    output
}

fn ttml_role_text(raw: &str, role: &str) -> Option<String> {
    let mut cursor = 0;
    while let Some(relative_open) = raw[cursor..].find("<span") {
        let open = cursor + relative_open;
        let header_end = open + raw[open..].find('>')?;
        let header = &raw[open..=header_end];
        let close = header_end + 1 + raw[header_end + 1..].find("</span>")?;
        if header.contains(role) {
            let text = decode_xml_text(&strip_xml_tags(&raw[header_end + 1..close]))
                .trim()
                .to_string();
            return (!text.is_empty()).then_some(text);
        }
        cursor = close + 7;
    }
    None
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
    if !raw.contains(':') {
        return raw
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|value| *value >= 0.0)
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
