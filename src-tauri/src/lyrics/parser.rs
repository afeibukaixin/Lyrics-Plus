use base64::Engine;

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct LyricsFilePayload {
    version: String,
    metadata: LyricsFileMetadata,
    #[serde(default)]
    lines: Vec<LyricsFileLine>,
    plain: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct LyricsFileMetadata {
    title: String,
    artist: String,
    album: Option<String>,
    duration_ms: Option<i64>,
    #[serde(default)]
    instrumental: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct LyricsFileLine {
    text: String,
    start_ms: i64,
    end_ms: Option<i64>,
    #[serde(default)]
    words: Vec<LyricsFileWord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct LyricsFileWord {
    text: String,
    start_ms: i64,
    end_ms: Option<i64>,
}

fn looks_like_lyricsfile(raw: &str) -> bool {
    raw.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#') && *line != "---")
        .is_some_and(|line| {
            line == "version: '1.0'" || line == "version: \"1.0\"" || line.starts_with("version:")
        })
}

fn parse_lyricsfile(
    raw: &str,
    source: &str,
    manual_selected: bool,
) -> Result<Option<LyricsDocument>, String> {
    if !looks_like_lyricsfile(raw) {
        return Ok(None);
    }
    if raw.len() > 5 * 1024 * 1024 {
        return Err("Lyricsfile 超过 5 MB".into());
    }
    let payload = serde_saphyr::from_str::<LyricsFilePayload>(raw)
        .map_err(|error| format!("无法解析 Lyricsfile：{error}"))?;
    if payload.version != "1.0" {
        return Err(format!("不支持的 Lyricsfile 版本：{}", payload.version));
    }
    if payload.metadata.duration_ms.is_some_and(|value| value < 0) {
        return Err("Lyricsfile 的 duration_ms 不能为负数".into());
    }
    if payload.metadata.instrumental
        && (!payload.lines.is_empty()
            || payload
                .plain
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()))
    {
        return Err("纯音乐 Lyricsfile 不能包含歌词内容".into());
    }
    if payload.lines.is_empty() {
        return Err("Lyricsfile 没有同步歌词行".into());
    }

    let mut lines = Vec::with_capacity(payload.lines.len());
    for line in payload.lines {
        let start_ms = u64::try_from(line.start_ms)
            .map_err(|_| "Lyricsfile 的行开始时间不能为负数".to_string())?;
        let end_ms = match line.end_ms {
            Some(end_ms) => {
                let end_ms = u64::try_from(end_ms)
                    .map_err(|_| "Lyricsfile 的行结束时间不能为负数".to_string())?;
                if end_ms < start_ms {
                    return Err("Lyricsfile 的行结束时间不能早于开始时间".into());
                }
                Some(end_ms)
            }
            None => None,
        };
        let mut words = Vec::with_capacity(line.words.len());
        for word in line.words {
            let word_start_ms = u64::try_from(word.start_ms)
                .map_err(|_| "Lyricsfile 的单词开始时间不能为负数".to_string())?;
            let word_end_ms = match word.end_ms {
                Some(end_ms) => {
                    let end_ms = u64::try_from(end_ms)
                        .map_err(|_| "Lyricsfile 的单词结束时间不能为负数".to_string())?;
                    if end_ms < word_start_ms {
                        return Err("Lyricsfile 的单词结束时间不能早于开始时间".into());
                    }
                    Some(end_ms)
                }
                None => None,
            };
            words.push((word_start_ms, word_end_ms, word.text));
        }
        words.sort_by_key(|(start_ms, _, _)| *start_ms);
        let words = (0..words.len())
            .map(|index| {
                let (start_ms, explicit_end_ms, text) = &words[index];
                LyricsWord {
                    start_ms: *start_ms,
                    end_ms: explicit_end_ms
                        .to_owned()
                        .or_else(|| words_next_start(&words, index))
                        .or(end_ms)
                        .unwrap_or(*start_ms)
                        .max(*start_ms),
                    text: text.clone(),
                }
            })
            .collect::<Vec<_>>();
        lines.push(LyricsLine {
            start_ms,
            end_ms,
            text: line.text,
            words: (!words.is_empty()).then_some(words),
        });
    }
    lines.sort_by_key(|line| line.start_ms);

    Ok(Some(LyricsDocument {
        metadata: LyricsMetadata {
            title: Some(payload.metadata.title),
            artist: Some(payload.metadata.artist),
            album: payload.metadata.album,
            source: source.into(),
            original_format: "lyricsfile".into(),
            manual_selected,
        },
        tracks: LyricsTracks {
            original: LyricsTrack { lines },
            translation: None,
            romanization: None,
        },
        offset_ms: 0,
        raw: raw.to_string(),
    }))
}

fn words_next_start(words: &[(u64, Option<u64>, String)], index: usize) -> Option<u64> {
    words.get(index + 1).map(|(start_ms, _, _)| *start_ms)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LyricsQualityReport {
    pub has_valid_synced_original: bool,
    pub degraded_word_lines: usize,
    pub last_valid_time_ms: Option<u64>,
    pub auto_applicable: bool,
}

/// 对所有解析分支执行同一套时间轴收尾校验，保持公开解析函数的返回类型不变。
/// 发生降级时仅记录时间轴字段，不记录歌词正文。
fn normalize_document(document: &mut LyricsDocument) {
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
        && duration_ms.is_none_or(|duration| {
            last_valid_time_ms.is_none_or(|last| last <= duration.saturating_add(12_000))
        });
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

pub fn parse_lrc_with_options(
    raw: &str,
    source: impl Into<String>,
    manual_selected: bool,
) -> Result<LyricsDocument, String> {
    let source = source.into();
    if let Some(document) = parse_lyricsfile(raw, &source, manual_selected)? {
        let mut document = document;
        normalize_document(&mut document);
        return Ok(document);
    }
    if let Some(tracks) = parse_ttml_lyrics(raw) {
        let mut document = LyricsDocument {
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
        };
        normalize_document(&mut document);
        return Ok(document);
    }
    if let Some((format, lines)) = parse_platform_word_lyrics(raw) {
        let (title, artist, album, embedded_offset) = metadata_tags(raw);
        let (mut translation, mut romanization) = parse_auxiliary_lrc_tracks(raw);
        if format == "krc" && (translation.is_empty() || romanization.is_empty()) {
            let (language_translation, language_romanization) =
                parse_krc_language_tracks(raw);
            if translation.is_empty() {
                translation = language_translation;
            }
            if romanization.is_empty() {
                romanization = language_romanization;
            }
        }
        let mut document = LyricsDocument {
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
        };
        normalize_document(&mut document);
        return Ok(document);
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

    let mut document = LyricsDocument {
        metadata: LyricsMetadata {
            title,
            artist,
            album,
            source,
            original_format: if saw_enhanced_words || original.iter().any(|line| line.words.is_some()) {
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
    normalize_document(&mut document);
    Ok(document)
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

struct ParsedEnhancedWords {
    text: String,
    words: Vec<LyricsWord>,
    malformed: bool,
    has_word_tags: bool,
}

fn parse_enhanced_words(raw: &str) -> ParsedEnhancedWords {
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

fn parse_krc_language_tracks(raw: &str) -> (Vec<LyricsLine>, Vec<LyricsLine>) {
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

fn parse_integer_list(raw: &str) -> Vec<u64> {
    raw.split(',')
        .map(|value| value.trim().parse::<u64>().ok())
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default()
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
