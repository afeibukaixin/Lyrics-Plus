use base64::Engine;
use futures::future::join_all;
use lyrics_crypto::decrypter::qrc::decrypter::decrypt_lyrics;
use serde::Deserialize;

use super::provider::{
    collect_provider_results, duration_ms_from_seconds_u64, score_candidate, LyricsProvider,
    LyricsSearchInput, LyricsSearchResult, ProviderError, ProviderErrorKind, ProviderFuture,
    ProviderSearchReport, QQMUSIC_DISPLAY_NAME,
};

pub(crate) const QQMUSIC_PROVIDER_ID: &str = "qqmusic";
pub(crate) const QQMUSIC_PLAY_LYRIC_VERSION_TAG: &str =
    "[lyrics-plus:provider-version:qqmusic-play-lyric-v2]";

#[derive(Debug, Deserialize)]
struct SearchEnvelope {
    data: Option<SearchData>,
}

#[derive(Debug, Deserialize)]
struct SearchData {
    song: Option<SongData>,
}

#[derive(Debug, Deserialize)]
struct SongData {
    #[serde(default)]
    list: Vec<QqSong>,
}

#[derive(Debug, Deserialize)]
struct QqSong {
    songmid: String,
    songname: String,
    albumname: Option<String>,
    interval: Option<u64>,
    #[serde(default)]
    songid: Option<serde_json::Value>,
    #[serde(default)]
    singer: Vec<QqSinger>,
}

#[derive(Debug, Deserialize)]
struct QqSinger {
    name: String,
}

#[derive(Debug, Deserialize)]
struct LyricsEnvelope {
    lyric: Option<String>,
    trans: Option<String>,
}

#[derive(Debug)]
struct QqCandidate {
    result: LyricsSearchResult,
    song_mid: String,
    song_id: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
struct QqRichData {
    lyric: Option<String>,
    trans: Option<String>,
    lrc: Option<String>,
    roma: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct QqLyricsPayload {
    lyric: Option<String>,
    trans: Option<String>,
    romanization: Option<String>,
    rich_verified: bool,
}

pub struct QqMusicProvider;

impl LyricsProvider for QqMusicProvider {
    fn id(&self) -> &'static str {
        QQMUSIC_PROVIDER_ID
    }

    fn display_name(&self) -> &'static str {
        QQMUSIC_DISPLAY_NAME
    }

    fn search<'a>(
        &'a self,
        client: &'a reqwest::Client,
        input: &'a LyricsSearchInput,
    ) -> ProviderFuture<'a, ProviderSearchReport> {
        Box::pin(async move {
            let mut url = reqwest::Url::parse("https://c.y.qq.com/soso/fcgi-bin/client_search_cp")
                .map_err(|error| {
                    self.error(ProviderErrorKind::InvalidResponse, error.to_string())
                })?;
            url.query_pairs_mut()
                .append_pair(
                    "w",
                    &format!("{} {}", input.title.trim(), input.artist.trim()),
                )
                .append_pair("p", "1")
                .append_pair("n", "12")
                .append_pair("format", "json");
            let response = client
                .get(url)
                .header("Referer", "https://y.qq.com/")
                .send()
                .await
                .map_err(|error| self.error(ProviderErrorKind::Network, error.to_string()))?;
            if !response.status().is_success() {
                return Err(super::provider::response_error(
                    self.id(),
                    &response,
                    "搜索请求失败",
                ));
            }
            let envelope = response.json::<SearchEnvelope>().await.map_err(|error| {
                self.error(ProviderErrorKind::InvalidResponse, error.to_string())
            })?;
            let mut candidates = envelope
                .data
                .and_then(|data| data.song)
                .map(|song| song.list)
                .unwrap_or_default()
                .into_iter()
                .map(|song| {
                    let song_id = song.songid.as_ref().and_then(value_as_u64);
                    let song_mid = song.songmid;
                    let mut result = LyricsSearchResult {
                        id: song_mid.clone(),
                        provider_id: self.id().into(),
                        title: song.songname,
                        artist: song
                            .singer
                            .into_iter()
                            .map(|singer| singer.name)
                            .collect::<Vec<_>>()
                            .join(" / "),
                        album: song.albumname.filter(|album| !album.is_empty()),
                        duration_ms: song.interval.map(duration_ms_from_seconds_u64),
                        source: self.display_name().into(),
                        synced: true,
                        has_translation: false,
                        has_word_timing: false,
                        has_romanization: false,
                        score: 0.0,
                        lyrics: String::new(),
                    };
                    result.score = score_candidate(input, &result);
                    QqCandidate {
                        result,
                        song_mid,
                        song_id,
                    }
                })
                .collect::<Vec<_>>();
            candidates.sort_by(|left, right| right.result.score.total_cmp(&left.result.score));
            candidates.truncate(5);

            let details = join_all(
                candidates
                    .iter()
                    .map(|candidate| self.fetch_detail(client, candidate)),
            )
            .await;
            collect_provider_results(candidates.into_iter().zip(details).map(
                |(candidate, detail)| {
                    detail.map(|detail| result_from_payload(candidate.result, detail))
                },
            ))
        })
    }
}

#[cfg(test)]
fn result_from_detail(
    candidate: LyricsSearchResult,
    detail: LyricsEnvelope,
) -> Option<LyricsSearchResult> {
    result_from_payload(
        candidate,
        QqLyricsPayload {
            lyric: detail.lyric,
            trans: detail.trans,
            romanization: None,
            rich_verified: false,
        },
    )
}

fn result_from_payload(
    mut candidate: LyricsSearchResult,
    detail: QqLyricsPayload,
) -> Option<LyricsSearchResult> {
    let original = detail.lyric.filter(|value| has_timed_text(value))?;
    let translation = detail.trans.filter(|value| has_timed_text(value));
    let romanization = detail.romanization.filter(|value| has_timed_text(value));
    let lyrics = merge_tracks(
        &original,
        translation.as_deref(),
        romanization.as_deref(),
        detail.rich_verified,
    );
    let parsed = super::parse_lrc_with_options(&lyrics, QQMUSIC_DISPLAY_NAME, false).ok()?;
    candidate.synced = true;
    candidate.has_translation = parsed.tracks.translation.is_some();
    candidate.has_word_timing = parsed
        .tracks
        .original
        .lines
        .iter()
        .any(|line| line.words.as_ref().is_some_and(|words| !words.is_empty()));
    candidate.has_romanization = parsed.tracks.romanization.is_some();
    candidate.lyrics = lyrics;
    Some(candidate)
}

fn value_as_u64(value: &serde_json::Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|value| value.parse::<u64>().ok()))
}

fn encode_base64(value: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(value.as_bytes())
}

fn rich_data(value: &serde_json::Value) -> Result<QqRichData, String> {
    if value_code(value).is_some_and(|code| code != 0) {
        return Err("QQMusic 新歌词接口返回错误".into());
    }
    let request = value
        .get("request")
        .or_else(|| value.get("req_0"))
        .or_else(|| {
            value
                .as_object()
                .and_then(|object| object.values().find(|value| value.get("data").is_some()))
        })
        .ok_or_else(|| "QQMusic 新歌词接口缺少请求结果".to_string())?;
    if value_code(request).is_some_and(|code| code != 0) {
        return Err("QQMusic 新歌词接口请求失败".into());
    }
    let data = request
        .get("data")
        .ok_or_else(|| "QQMusic 新歌词接口缺少歌词数据".to_string())?;
    serde_json::from_value(data.clone()).map_err(|error| format!("歌词数据格式无效：{error}"))
}

fn value_code(value: &serde_json::Value) -> Option<i64> {
    value
        .get("code")
        .and_then(|code| code.as_i64().or_else(|| code.as_str()?.parse().ok()))
}

fn decode_base64_lyrics(raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("QQMusic 歌词内容为空".into());
    }
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(raw)
        .map_err(|error| format!("QQMusic Base64 歌词解码失败：{error}"))?;
    String::from_utf8(decoded).map_err(|error| format!("QQMusic 歌词文本编码无效：{error}"))
}

fn decode_rich_lyric(raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("QQMusic 歌词内容为空".into());
    }
    if raw.starts_with('[') || raw.starts_with('<') {
        return Ok(raw.to_string());
    }
    if let Some(decoded) = decrypt_lyrics(raw) {
        return Ok(decoded);
    }
    decode_base64_lyrics(raw)
}

fn extract_qrc_lyric_content(raw: &str) -> Option<String> {
    let marker = "LyricContent=";
    let start = raw.find(marker)? + marker.len();
    let quote = raw[start..].chars().next()?;
    if !matches!(quote, '\'' | '"') {
        return None;
    }
    let value_start = start + quote.len_utf8();
    let value_end = value_start + raw[value_start..].find(quote)?;
    let value = super::decode_xml_text(&raw[value_start..value_end]);
    (!value.trim().is_empty()).then_some(value)
}

fn has_qrc_timing(raw: &str) -> bool {
    raw.lines().any(|line| {
        let line = line.trim_start();
        let Some(after_open) = line.strip_prefix('[') else {
            return false;
        };
        let Some(close) = after_open.find(']') else {
            return false;
        };
        super::parse_integer_list(&after_open[..close]).len() == 2
    })
}

fn qrc_to_lrc(raw: &str) -> Option<String> {
    let parsed = super::parse_lrc_with_options(raw, QQMUSIC_DISPLAY_NAME, false).ok()?;
    let lines = parsed
        .tracks
        .original
        .lines
        .into_iter()
        .filter(|line| !line.text.trim().is_empty())
        .map(|line| {
            let minutes = line.start_ms / 60_000;
            let seconds = (line.start_ms % 60_000) / 1_000;
            let milliseconds = line.start_ms % 1_000;
            format!(
                "[{minutes:02}:{seconds:02}.{milliseconds:03}]{}",
                line.text.trim()
            )
        })
        .collect::<Vec<_>>();
    (!lines.is_empty()).then_some(lines.join("\n"))
}

// 翻译和音译可能返回普通 LRC，也可能沿用 QRC XML 包装；统一还原成可合并的 LRC。
fn normalize_rich_auxiliary(raw: &str) -> Option<String> {
    let raw = extract_qrc_lyric_content(raw).unwrap_or_else(|| raw.to_string());
    if has_qrc_timing(&raw) {
        qrc_to_lrc(&raw)
    } else {
        clean_qq_auxiliary(&raw)
    }
}

impl QqMusicProvider {
    async fn fetch_detail(
        &self,
        client: &reqwest::Client,
        candidate: &QqCandidate,
    ) -> Result<QqLyricsPayload, ProviderError> {
        if candidate.song_id.is_some() {
            match self.fetch_rich(client, candidate).await {
                Ok(payload)
                    if payload.lyric.as_deref().is_some_and(has_timed_text)
                        && result_from_payload(candidate.result.clone(), payload.clone())
                            .is_some() =>
                {
                    return Ok(payload);
                }
                Ok(_) => log::debug!("QQMusic 新歌词接口未返回可用原文，回退旧接口"),
                Err(error) => log::debug!("QQMusic 新歌词接口失败，回退旧接口：{error}"),
            }
        }

        self.fetch_legacy(client, &candidate.song_mid)
            .await
            .map(|detail| QqLyricsPayload {
                lyric: detail.lyric,
                trans: detail.trans.and_then(|value| clean_qq_auxiliary(&value)),
                romanization: None,
                rich_verified: false,
            })
    }

    async fn fetch_rich(
        &self,
        client: &reqwest::Client,
        candidate: &QqCandidate,
    ) -> Result<QqLyricsPayload, ProviderError> {
        let song_id = candidate.song_id.ok_or_else(|| {
            self.error(
                ProviderErrorKind::InvalidResponse,
                "QQMusic 歌曲缺少数字 ID",
            )
        })?;
        let album = candidate.result.album.as_deref().unwrap_or_default();
        let body = serde_json::json!({
            "comm": {
                "ct": 11,
                "cv": "1003006",
                "v": "1003006",
                "os_ver": "15",
                "phonetype": "24122RKC7C",
                "tmeAppID": "qqmusiclight",
                "nettype": "NETWORK_WIFI",
                "udid": "0"
            },
            "req_0": {
                "method": "GetPlayLyricInfo",
                "module": "music.musichallSong.PlayLyricInfo",
                "param": {
                    "albumName": encode_base64(album),
                    "crypt": 1,
                    "ct": 19,
                    "cv": 2111,
                    "interval": candidate.result.duration_ms.map(|value| value / 1000).unwrap_or(0),
                    "lrc_t": 0,
                    "qrc": 1,
                    "qrc_t": 0,
                    "roma": 1,
                    "roma_t": 0,
                    "singerName": encode_base64(&candidate.result.artist),
                    "songID": song_id,
                    "songName": encode_base64(&candidate.result.title),
                    "trans": 1,
                    "trans_t": 0,
                    "type": 0
                }
            }
        });
        let response = client
            .post("https://u.y.qq.com/cgi-bin/musicu.fcg")
            .header("Content-Type", "application/json")
            .header("User-Agent", "okhttp/3.14.9")
            .header("Cookie", "tmeLoginType=-1;")
            .json(&body)
            .send()
            .await
            .map_err(|error| self.error(ProviderErrorKind::Network, error.to_string()))?;
        if !response.status().is_success() {
            return Err(super::provider::response_error(
                self.id(),
                &response,
                "新歌词接口请求失败",
            ));
        }
        let value = response
            .json::<serde_json::Value>()
            .await
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?;
        let data = rich_data(&value)
            .map_err(|message| self.error(ProviderErrorKind::InvalidResponse, message))?;
        let original = data
            .lyric
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| data.lrc.as_deref().filter(|value| !value.trim().is_empty()))
            .ok_or_else(|| self.error(ProviderErrorKind::InvalidResponse, "新歌词接口原文为空"))
            .and_then(|value| {
                decode_rich_lyric(value)
                    .map_err(|message| self.error(ProviderErrorKind::InvalidResponse, message))
            })
            .map(|value| extract_qrc_lyric_content(&value).unwrap_or(value))?;
        let mut rich_verified = true;
        let trans = match data
            .trans
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            Some(value) => match decode_rich_lyric(value) {
                Ok(value) => normalize_rich_auxiliary(&value),
                Err(error) => {
                    log::debug!("QQMusic 翻译歌词解码失败，保留原文并等待重试：{error}");
                    rich_verified = false;
                    None
                }
            },
            None => None,
        };
        if data
            .trans
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
            && trans.is_none()
        {
            rich_verified = false;
        }
        let romanization = match data
            .roma
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            Some(value) => match decode_rich_lyric(value) {
                Ok(value) => normalize_rich_auxiliary(&value),
                Err(error) => {
                    log::debug!("QQMusic 音译歌词解码失败，保留其他轨道：{error}");
                    rich_verified = false;
                    None
                }
            },
            None => None,
        };
        if data
            .roma
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
            && romanization.is_none()
        {
            rich_verified = false;
        }
        Ok(QqLyricsPayload {
            lyric: Some(original),
            trans,
            romanization,
            rich_verified,
        })
    }

    async fn fetch_legacy(
        &self,
        client: &reqwest::Client,
        song_mid: &str,
    ) -> Result<LyricsEnvelope, ProviderError> {
        let mut url =
            reqwest::Url::parse("https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg")
                .map_err(|error| {
                    self.error(ProviderErrorKind::InvalidResponse, error.to_string())
                })?;
        url.query_pairs_mut()
            .append_pair("songmid", song_mid)
            .append_pair("format", "json")
            .append_pair("nobase64", "1");
        let response = client
            .get(url)
            .header("Referer", "https://y.qq.com/")
            .send()
            .await
            .map_err(|error| self.error(ProviderErrorKind::Network, error.to_string()))?;
        if !response.status().is_success() {
            return Err(super::provider::response_error(
                self.id(),
                &response,
                "歌词请求失败",
            ));
        }
        response
            .json()
            .await
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))
    }

    fn error(&self, kind: ProviderErrorKind, message: impl Into<String>) -> ProviderError {
        ProviderError::new(self.id(), kind, message)
    }
}

fn clean_qq_auxiliary(raw: &str) -> Option<String> {
    let cleaned = raw
        .lines()
        .filter(|line| {
            line.find(']')
                .map(|close| line[close + 1..].trim() != "//")
                .unwrap_or(true)
        })
        .collect::<Vec<_>>()
        .join("\n");
    has_timed_text(&cleaned).then_some(cleaned)
}

fn has_timed_text(raw: &str) -> bool {
    raw.lines().any(|line| {
        let line = line.trim_start();
        line.find(']').is_some_and(|end| {
            let tag = line
                .strip_prefix('[')
                .and_then(|value| value.get(..end.saturating_sub(1)));
            let timed = tag
                .is_some_and(|tag| tag.contains(':') || super::parse_integer_list(tag).len() == 2);
            timed && !line[end + 1..].trim().is_empty()
        })
    })
}

fn merge_tracks(
    original: &str,
    translation: Option<&str>,
    romanization: Option<&str>,
    rich_verified: bool,
) -> String {
    let mut sections = Vec::with_capacity(4);
    if rich_verified {
        sections.push(QQMUSIC_PLAY_LYRIC_VERSION_TAG.to_string());
    }
    sections.push(original.trim().to_string());
    if let Some(value) = translation.filter(|value| !value.trim().is_empty()) {
        sections.push(format!("[lyrics-plus:translation]\n{}", value.trim()));
    }
    if let Some(value) = romanization.filter(|value| !value.trim().is_empty()) {
        sections.push(format!("[lyrics-plus:romanization]\n{}", value.trim()));
    }
    sections.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str) -> LyricsSearchResult {
        LyricsSearchResult {
            id: id.into(),
            provider_id: "qqmusic".into(),
            title: id.into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: None,
            source: QQMUSIC_DISPLAY_NAME.into(),
            synced: true,
            has_translation: false,
            has_word_timing: false,
            has_romanization: false,
            score: 1.0,
            lyrics: String::new(),
        }
    }

    fn detail(lyrics: Option<&str>) -> LyricsEnvelope {
        LyricsEnvelope {
            lyric: lyrics.map(str::to_owned),
            trans: None,
        }
    }

    fn failure(message: &str) -> ProviderError {
        ProviderError::new("qqmusic", ProviderErrorKind::Network, message)
    }

    #[test]
    fn preserves_order_and_translation_capability() {
        let mut translated = detail(Some("[00:01.00]First"));
        translated.trans = Some("[00:01.00]第一".into());
        let report = collect_provider_results(vec![
            Ok(result_from_detail(candidate("first"), translated)),
            Ok(result_from_detail(
                candidate("second"),
                detail(Some("[00:01.00]Second")),
            )),
        ])
        .unwrap();

        assert_eq!(report.results[0].id, "first");
        assert_eq!(report.results[1].id, "second");
        assert!(report.results[0].has_translation);
    }

    #[test]
    fn partial_failure_is_degraded_but_keeps_results() {
        let report = collect_provider_results(vec![
            Err(failure("temporary failure")),
            Ok(result_from_detail(
                candidate("available"),
                detail(Some("[00:01.00]Available")),
            )),
        ])
        .unwrap();

        assert_eq!(report.results[0].id, "available");
        assert_eq!(
            report.warning.as_ref().map(|error| error.message.as_str()),
            Some("temporary failure")
        );
    }

    #[test]
    fn all_failures_return_the_first_error() {
        let error = collect_provider_results(vec![
            Err(failure("first failure")),
            Err(failure("second failure")),
        ])
        .unwrap_err();
        assert_eq!(error.message, "first failure");
    }

    #[test]
    fn successful_empty_detail_is_available() {
        let report = collect_provider_results(vec![Ok(result_from_detail(
            candidate("empty"),
            detail(None),
        ))])
        .unwrap();
        assert!(report.results.is_empty());
        assert!(report.warning.is_none());
    }
}
