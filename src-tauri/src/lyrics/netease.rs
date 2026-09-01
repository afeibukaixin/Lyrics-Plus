use futures::future::join_all;
use serde::Deserialize;

use super::parse_lrc_with_options;
use super::provider::{
    collect_provider_results, score_candidate, LyricsProvider, LyricsSearchInput,
    LyricsSearchResult, ProviderError, ProviderErrorKind, ProviderFuture, ProviderSearchReport,
    NETEASE_DISPLAY_NAME,
};

#[derive(Debug, Deserialize)]
struct SearchEnvelope {
    result: Option<SearchData>,
}

#[derive(Debug, Deserialize)]
struct SearchData {
    #[serde(default)]
    songs: Vec<NeteaseSong>,
}

#[derive(Debug, Deserialize)]
struct NeteaseSong {
    id: i64,
    name: String,
    #[serde(default)]
    artists: Vec<NeteaseArtist>,
    album: Option<NeteaseAlbum>,
    duration: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct NeteaseArtist {
    name: String,
}

#[derive(Debug, Deserialize)]
struct NeteaseAlbum {
    name: String,
}

#[derive(Debug, Deserialize)]
struct LyricsEnvelope {
    #[serde(default)]
    lrc: Option<LyricValue>,
    #[serde(default)]
    tlyric: Option<LyricValue>,
    #[serde(default)]
    yrc: Option<LyricValue>,
    #[serde(default)]
    romalrc: Option<LyricValue>,
    #[serde(default)]
    ytlrc: Option<LyricValue>,
    #[serde(default)]
    yromalrc: Option<LyricValue>,
}

#[derive(Debug, Deserialize)]
struct LyricValue {
    lyric: String,
}

pub struct NeteaseProvider;

impl LyricsProvider for NeteaseProvider {
    fn id(&self) -> &'static str {
        "netease"
    }

    fn display_name(&self) -> &'static str {
        NETEASE_DISPLAY_NAME
    }

    fn search<'a>(
        &'a self,
        client: &'a reqwest::Client,
        input: &'a LyricsSearchInput,
    ) -> ProviderFuture<'a, ProviderSearchReport> {
        Box::pin(async move {
            let mut url = reqwest::Url::parse("https://music.163.com/api/cloudsearch/pc").map_err(
                |error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()),
            )?;
            url.query_pairs_mut()
                .append_pair(
                    "s",
                    &format!("{} {}", input.title.trim(), input.artist.trim()),
                )
                .append_pair("type", "1")
                .append_pair("offset", "0")
                .append_pair("total", "true")
                // 未登录搜索会把部分翻唱置顶，扩大候选池后再由本地元数据评分。
                .append_pair("limit", "100");
            let response = client
                .get(url)
                .header("Referer", "https://music.163.com/")
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
                .result
                .map(|result| result.songs)
                .unwrap_or_default()
                .into_iter()
                .map(|song| {
                    let mut result = LyricsSearchResult {
                        id: song.id.to_string(),
                        provider_id: self.id().into(),
                        title: song.name,
                        artist: song
                            .artists
                            .into_iter()
                            .map(|artist| artist.name)
                            .collect::<Vec<_>>()
                            .join(" / "),
                        album: song.album.map(|album| album.name),
                        duration_ms: song.duration,
                        source: self.display_name().into(),
                        synced: true,
                        has_translation: false,
                        has_word_timing: false,
                        has_romanization: false,
                        score: 0.0,
                        lyrics: String::new(),
                    };
                    result.score = score_candidate(input, &result);
                    result
                })
                .collect::<Vec<_>>();
            candidates.sort_by(|left, right| right.score.total_cmp(&left.score));
            candidates.truncate(5);

            let details = join_all(
                candidates
                    .iter()
                    .map(|candidate| self.fetch_detail(client, &candidate.id)),
            )
            .await;
            collect_provider_results(candidates.into_iter().zip(details).map(
                |(candidate, detail)| detail.map(|detail| result_from_detail(candidate, detail)),
            ))
        })
    }
}

fn result_from_detail(
    mut candidate: LyricsSearchResult,
    detail: LyricsEnvelope,
) -> Option<LyricsSearchResult> {
    let line_lyrics = detail.lrc.map(|value| value.lyric)?;
    if !has_timed_text(&line_lyrics) {
        return None;
    }
    let word_lyrics = detail
        .yrc
        .map(|value| value.lyric)
        .filter(|value| !value.trim().is_empty());
    let translation = detail
        .ytlrc
        .or(detail.tlyric)
        .map(|value| value.lyric)
        .filter(|value| has_timed_text(value));
    let romanization = detail
        .yromalrc
        .or(detail.romalrc)
        .map(|value| value.lyric)
        .filter(|value| has_timed_text(value));
    let lyrics = merge_tracks(
        word_lyrics.as_deref().unwrap_or(&line_lyrics),
        translation.as_deref(),
        romanization.as_deref(),
    );
    let parsed = parse_lrc_with_options(&lyrics, NETEASE_DISPLAY_NAME, false).ok()?;
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

impl NeteaseProvider {
    async fn fetch_detail(
        &self,
        client: &reqwest::Client,
        id: &str,
    ) -> Result<LyricsEnvelope, ProviderError> {
        match self.fetch_detail_at(client, id, "api/song/lyric/v1").await {
            Ok(detail) => Ok(detail),
            Err(error)
                if matches!(
                    &error.kind,
                    ProviderErrorKind::Network
                        | ProviderErrorKind::Http
                        | ProviderErrorKind::InvalidResponse
                ) && error.status_code != Some(429) =>
            {
                self.fetch_detail_at(client, id, "api/song/lyric").await
            }
            Err(error) => Err(error),
        }
    }

    async fn fetch_detail_at(
        &self,
        client: &reqwest::Client,
        id: &str,
        endpoint: &str,
    ) -> Result<LyricsEnvelope, ProviderError> {
        let mut url = reqwest::Url::parse(&format!("https://music.163.com/{endpoint}"))
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?;
        url.query_pairs_mut()
            .append_pair("id", id)
            .append_pair("lv", "1")
            .append_pair("kv", "1")
            .append_pair("tv", "1")
            .append_pair("yv", "1")
            .append_pair("rv", "1");
        let response = client
            .get(url)
            .header("Referer", "https://music.163.com/")
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

fn has_timed_text(raw: &str) -> bool {
    raw.lines().any(|line| {
        line.find(']')
            .is_some_and(|end| line[..end].contains(':') && !line[end + 1..].trim().is_empty())
    })
}

fn merge_tracks(original: &str, translation: Option<&str>, romanization: Option<&str>) -> String {
    let mut sections = vec![original.trim().to_string()];
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
            provider_id: "netease".into(),
            title: id.into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: None,
            source: NETEASE_DISPLAY_NAME.into(),
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
            lrc: lyrics.map(|lyric| LyricValue {
                lyric: lyric.into(),
            }),
            tlyric: None,
            yrc: None,
            romalrc: None,
            ytlrc: None,
            yromalrc: None,
        }
    }

    fn failure(message: &str) -> ProviderError {
        ProviderError::new("netease", ProviderErrorKind::Network, message)
    }

    #[test]
    fn ignores_timestamp_only_translation() {
        assert!(!has_timed_text("[00:01.00]\n[00:02.00]"));
        assert!(has_timed_text("[00:01.00]Hello"));
    }

    #[test]
    fn preserves_candidate_order_when_collecting_details() {
        let report = collect_provider_results(vec![
            Ok(result_from_detail(
                candidate("first"),
                detail(Some("[00:01.00]First")),
            )),
            Ok(result_from_detail(
                candidate("second"),
                detail(Some("[00:01.00]Second")),
            )),
        ])
        .unwrap();

        assert_eq!(report.results[0].id, "first");
        assert_eq!(report.results[1].id, "second");
    }

    #[test]
    fn keeps_successful_details_when_another_request_fails() {
        let report = collect_provider_results(vec![
            Err(failure("temporary failure")),
            Ok(result_from_detail(
                candidate("available"),
                detail(Some("[00:01.00]Available")),
            )),
        ])
        .unwrap();

        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].id, "available");
        assert_eq!(
            report.warning.as_ref().map(|error| error.message.as_str()),
            Some("temporary failure")
        );
    }

    #[test]
    fn returns_first_error_when_every_detail_request_fails() {
        let error = collect_provider_results(vec![
            Err(failure("first failure")),
            Err(failure("second failure")),
        ])
        .unwrap_err();

        assert_eq!(error.message, "first failure");
    }

    #[test]
    fn successful_empty_detail_is_not_a_connection_error() {
        let report = collect_provider_results(vec![Ok(result_from_detail(
            candidate("empty"),
            detail(None),
        ))])
        .unwrap();
        assert!(report.results.is_empty());
        assert!(report.warning.is_none());
    }
}
