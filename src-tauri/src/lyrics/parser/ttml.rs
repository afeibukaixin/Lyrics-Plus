use super::super::{LyricsLine, LyricsTrack, LyricsTracks, LyricsWord};

pub(super) fn parse_ttml_lyrics(raw: &str) -> Option<LyricsTracks> {
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

pub(in crate::lyrics) fn decode_xml_text(raw: &str) -> String {
    raw.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}
