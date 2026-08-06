use serde::Deserialize;

use super::parse_lrc_with_options;
use super::provider::{
    score_candidate, LyricsProvider, LyricsSearchInput, LyricsSearchResult, ProviderCapabilities,
    ProviderError, ProviderErrorKind, ProviderFuture, NETEASE_DISPLAY_NAME,
};
use super::LyricsDocument;

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
    lrc: Option<LyricValue>,
    tlyric: Option<LyricValue>,
    yrc: Option<LyricValue>,
    romalrc: Option<LyricValue>,
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

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            synced: true,
            translation: true,
            word_timing: true,
            romanization: true,
        }
    }

    fn search<'a>(
        &'a self,
        client: &'a reqwest::Client,
        input: &'a LyricsSearchInput,
    ) -> ProviderFuture<'a, Vec<LyricsSearchResult>> {
        Box::pin(async move {
            let mut url = reqwest::Url::parse("https://music.163.com/api/search/get/web").map_err(
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
                return Err(self.error(
                    ProviderErrorKind::Http,
                    format!("搜索返回 HTTP {}", response.status()),
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

            let mut results = Vec::new();
            for mut candidate in candidates {
                if let Ok(detail) = self.fetch_detail(client, &candidate.id).await {
                    let Some(line_lyrics) = detail.lrc.map(|value| value.lyric) else {
                        continue;
                    };
                    if !has_timed_text(&line_lyrics) {
                        continue;
                    }
                    let word_lyrics = detail
                        .yrc
                        .map(|value| value.lyric)
                        .filter(|value| !value.trim().is_empty());
                    let translation = detail
                        .tlyric
                        .map(|value| value.lyric)
                        .filter(|value| has_timed_text(value));
                    let romanization = detail
                        .romalrc
                        .map(|value| value.lyric)
                        .filter(|value| has_timed_text(value));
                    candidate.has_translation = translation.is_some();
                    candidate.has_word_timing = word_lyrics.is_some();
                    candidate.has_romanization = romanization.is_some();
                    candidate.lyrics = merge_tracks(
                        word_lyrics.as_deref().unwrap_or(&line_lyrics),
                        translation.as_deref(),
                        romanization.as_deref(),
                    );
                    results.push(candidate);
                }
            }
            Ok(results)
        })
    }

    fn fetch<'a>(
        &'a self,
        client: &'a reqwest::Client,
        result: &'a LyricsSearchResult,
    ) -> ProviderFuture<'a, String> {
        Box::pin(async move {
            let detail = self.fetch_detail(client, &result.id).await?;
            let line_lyrics = detail
                .lrc
                .map(|value| value.lyric)
                .ok_or_else(|| self.error(ProviderErrorKind::NotFound, "没有同步歌词"))?;
            let word_lyrics = detail
                .yrc
                .map(|value| value.lyric)
                .filter(|value| !value.trim().is_empty());
            let translation = detail
                .tlyric
                .map(|value| value.lyric)
                .filter(|value| has_timed_text(value));
            let romanization = detail
                .romalrc
                .map(|value| value.lyric)
                .filter(|value| has_timed_text(value));
            Ok(merge_tracks(
                word_lyrics.as_deref().unwrap_or(&line_lyrics),
                translation.as_deref(),
                romanization.as_deref(),
            ))
        })
    }

    fn parse(&self, raw: &str, manual_selected: bool) -> Result<LyricsDocument, ProviderError> {
        parse_lrc_with_options(raw, self.display_name(), manual_selected)
            .map_err(|message| self.error(ProviderErrorKind::Parse, message))
    }
}

impl NeteaseProvider {
    async fn fetch_detail(
        &self,
        client: &reqwest::Client,
        id: &str,
    ) -> Result<LyricsEnvelope, ProviderError> {
        let mut url = reqwest::Url::parse("https://music.163.com/api/song/lyric")
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
            return Err(self.error(
                ProviderErrorKind::Http,
                format!("歌词返回 HTTP {}", response.status()),
            ));
        }
        response
            .json()
            .await
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))
    }

    fn error(&self, kind: ProviderErrorKind, message: impl Into<String>) -> ProviderError {
        ProviderError {
            provider_id: self.id().into(),
            kind,
            message: message.into(),
        }
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

    #[test]
    fn ignores_timestamp_only_translation() {
        assert!(!has_timed_text("[00:01.00]\n[00:02.00]"));
        assert!(has_timed_text("[00:01.00]Hello"));
    }
}
