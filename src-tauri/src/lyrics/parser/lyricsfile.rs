use serde::Deserialize;

use super::super::{
    LyricsDocument, LyricsLine, LyricsMetadata, LyricsTrack, LyricsTracks, LyricsWord,
};

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

pub(super) fn parse_lyricsfile(
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
