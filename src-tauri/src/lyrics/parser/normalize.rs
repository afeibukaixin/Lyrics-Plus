use super::super::LyricsDocument;
use super::basic_lrc::timestamp_ms;
use super::platform::parse_integer_list;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LyricsQualityReport {
    pub has_valid_synced_original: bool,
    pub degraded_word_lines: usize,
    pub last_valid_time_ms: Option<u64>,
    pub auto_applicable: bool,
}

/// 对所有解析分支执行同一套时间轴收尾校验，保持公开解析函数的返回类型不变。
/// 发生降级时仅记录时间轴字段，不记录歌词正文。
pub(super) fn normalize_document(document: &mut LyricsDocument) {
    let source = document.metadata.source.clone();
    let format = document.metadata.original_format.clone();
    for line in &mut document.tracks.original.lines {
        let Some(words) = line.words.as_mut() else {
            continue;
        };
        let mut previous_start = None;
        let mut invalid_reason = None;
        let mut invalid_overflow_ms = None;
        let mut invalid_word = None;
        for word in words.iter_mut() {
            if previous_start.is_some_and(|start| word.start_ms < start) {
                invalid_reason = Some("word_start_decreased");
                invalid_word = Some((word.start_ms, word.end_ms));
                break;
            }
            if word.end_ms < word.start_ms {
                invalid_reason = Some("word_end_before_start");
                invalid_word = Some((word.start_ms, word.end_ms));
                break;
            }
            if word.start_ms < line.start_ms {
                let overflow = line.start_ms - word.start_ms;
                if overflow <= 250 {
                    word.start_ms = line.start_ms;
                } else {
                    invalid_reason = Some("word_start_before_line");
                    invalid_overflow_ms = Some(overflow);
                    invalid_word = Some((word.start_ms, word.end_ms));
                    break;
                }
            }
            if let Some(line_end) = line.end_ms {
                if word.end_ms > line_end {
                    let overflow = word.end_ms - line_end;
                    if overflow <= 250 {
                        word.end_ms = line_end;
                    } else {
                        invalid_reason = Some("word_end_after_line");
                        invalid_overflow_ms = Some(overflow);
                        invalid_word = Some((word.start_ms, word.end_ms));
                        break;
                    }
                }
                if word.start_ms > line_end {
                    let overflow = word.start_ms - line_end;
                    if overflow <= 250 {
                        word.start_ms = line_end;
                        word.end_ms = word.end_ms.max(word.start_ms);
                    } else {
                        invalid_reason = Some("word_start_after_line");
                        invalid_overflow_ms = Some(overflow);
                        invalid_word = Some((word.start_ms, word.end_ms));
                        break;
                    }
                }
            }
            if word.end_ms < word.start_ms {
                invalid_reason = Some("word_end_before_start");
                invalid_word = Some((word.start_ms, word.end_ms));
                break;
            }
            previous_start = Some(word.start_ms);
        }
        if let Some(reason) = invalid_reason {
            let (word_start_ms, word_end_ms) = invalid_word.unwrap_or((0, 0));
            log::debug!(
                "lyrics.parse degraded source={source:?} format={format} line_start_ms={} line_end_ms={:?} reason={reason} overflow_ms={} word_start_ms={word_start_ms} word_end_ms={word_end_ms}",
                line.start_ms,
                line.end_ms,
                invalid_overflow_ms.unwrap_or(0),
            );
            line.words = None;
        }
    }
}

pub(crate) fn lyrics_quality_report(
    document: &LyricsDocument,
    duration_ms: Option<u64>,
    duration_tolerance_ms: Option<u64>,
) -> LyricsQualityReport {
    let original = &document.tracks.original.lines;
    let has_valid_synced_original = original.iter().any(|line| {
        !line.text.trim().is_empty() && line.start_ms <= line.end_ms.unwrap_or(u64::MAX)
    });
    let last_valid_time_ms = original
        .iter()
        .filter(|line| {
            !line.text.trim().is_empty() && line.start_ms <= line.end_ms.unwrap_or(u64::MAX)
        })
        .map(|line| {
            line.end_ms.unwrap_or_else(|| {
                line.words
                    .as_ref()
                    .and_then(|words| words.last())
                    .map(|word| word.end_ms)
                    .unwrap_or(line.start_ms)
            })
        })
        .max();
    let attempted_word_lines = attempted_word_line_starts(document);
    let degraded_word_lines = original
        .iter()
        .filter(|line| {
            !line.text.trim().is_empty()
                && line.words.is_none()
                && attempted_word_lines.contains(&line.start_ms)
        })
        .count();
    let auto_applicable = has_valid_synced_original
        && match (duration_ms, duration_tolerance_ms) {
            (Some(duration), Some(tolerance)) => {
                last_valid_time_ms.is_none_or(|last| last <= duration.saturating_add(tolerance))
            }
            _ => true,
        };
    LyricsQualityReport {
        has_valid_synced_original,
        degraded_word_lines,
        last_valid_time_ms,
        auto_applicable,
    }
}

pub(crate) fn semantic_fingerprint(document: &LyricsDocument) -> String {
    document
        .tracks
        .original
        .lines
        .iter()
        .flat_map(|line| line.text.chars())
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn attempted_word_line_starts(document: &LyricsDocument) -> std::collections::HashSet<u64> {
    let mut starts = std::collections::HashSet::new();
    match document.metadata.original_format.as_str() {
        "enhanced_lrc" => {
            let mut track_kind = 0_u8;
            for source_line in document.raw.lines() {
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
                if track_kind != 0 {
                    continue;
                }
                let mut remaining = source_line.trim_start();
                let mut timestamps = Vec::new();
                while let Some(after_open) = remaining.strip_prefix('[') {
                    let Some(end) = after_open.find(']') else {
                        break;
                    };
                    let tag = &after_open[..end];
                    remaining = &after_open[end + 1..];
                    if let Some(time_ms) = timestamp_ms(tag) {
                        timestamps.push(time_ms);
                    }
                }
                if remaining.contains('<') {
                    starts.extend(timestamps);
                }
            }
        }
        "qrc" | "yrc" | "krc" => {
            let format = document.metadata.original_format.as_str();
            let mut track_kind = 0_u8;
            for source_line in document.raw.lines() {
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
                if track_kind != 0 {
                    continue;
                }
                let line = source_line.trim();
                let Some(after_open) = line.strip_prefix('[') else {
                    continue;
                };
                let Some(close) = after_open.find(']') else {
                    continue;
                };
                let values = parse_integer_list(&after_open[..close]);
                let content = &after_open[close + 1..];
                let has_word_timing = match format {
                    "qrc" => contains_qrc_word_tag(content),
                    "yrc" => content.contains('('),
                    "krc" => content.contains('<'),
                    _ => false,
                };
                if values.len() == 2 && has_word_timing {
                    starts.insert(values[0]);
                }
            }
        }
        _ => {}
    }
    starts
}

/// QRC 的逐字标签是 `(开始时间,时长)`；普通括号文本不应被当作逐字行。
fn contains_qrc_word_tag(raw: &str) -> bool {
    let mut cursor = 0;
    while let Some(relative_open) = raw[cursor..].find('(') {
        let open = cursor + relative_open;
        let Some(relative_close) = raw[open + 1..].find(')') else {
            return false;
        };
        let close = open + 1 + relative_close;
        if parse_integer_list(&raw[open + 1..close]).len() == 2 {
            return true;
        }
        cursor = close + 1;
        if cursor >= raw.len() {
            break;
        }
    }
    false
}
