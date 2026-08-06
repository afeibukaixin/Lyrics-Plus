use serde::Deserialize;

use super::parse_lrc_with_options;
use super::provider::{
    score_candidate, LyricsProvider, LyricsSearchInput, LyricsSearchResult, ProviderCapabilities,
    ProviderError, ProviderErrorKind, ProviderFuture, LRCLIB_DISPLAY_NAME,
};
use super::LyricsDocument;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrcLibItem {
    id: i64,
    track_name: String,
    artist_name: String,
    album_name: Option<String>,
    duration: Option<f64>,
    synced_lyrics: Option<String>,
}

pub struct LrcLibProvider;

impl LyricsProvider for LrcLibProvider {
    fn id(&self) -> &'static str {
        "lrclib"
    }

    fn display_name(&self) -> &'static str {
        LRCLIB_DISPLAY_NAME
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            synced: true,
            translation: false,
            word_timing: false,
            romanization: false,
        }
    }

    fn search<'a>(
        &'a self,
        client: &'a reqwest::Client,
        input: &'a LyricsSearchInput,
    ) -> ProviderFuture<'a, Vec<LyricsSearchResult>> {
        Box::pin(async move {
            let mut url =
                reqwest::Url::parse("https://lrclib.net/api/search").map_err(|error| {
                    self.error(ProviderErrorKind::InvalidResponse, error.to_string())
                })?;
            {
                let mut query = url.query_pairs_mut();
                query.append_pair("track_name", input.title.trim());
                query.append_pair("artist_name", input.artist.trim());
                if let Some(album) = input
                    .album
                    .as_deref()
                    .filter(|album| !album.trim().is_empty())
                {
                    query.append_pair("album_name", album.trim());
                }
            }

            let response = client.get(url).send().await.map_err(|error| {
                self.error(ProviderErrorKind::Network, format!("歌词搜索失败：{error}"))
            })?;
            if !response.status().is_success() {
                return Err(self.error(
                    ProviderErrorKind::Http,
                    format!("歌词服务返回 HTTP {}", response.status()),
                ));
            }

            let items = response.json::<Vec<LrcLibItem>>().await.map_err(|error| {
                self.error(
                    ProviderErrorKind::InvalidResponse,
                    format!("无法解析歌词搜索结果：{error}"),
                )
            })?;
            let capabilities = self.capabilities();
            let mut results = items
                .into_iter()
                .filter_map(|item| {
                    let lyrics = item.synced_lyrics?.trim().to_string();
                    if lyrics.is_empty() {
                        return None;
                    }
                    let mut result = LyricsSearchResult {
                        id: item.id.to_string(),
                        provider_id: self.id().into(),
                        title: item.track_name,
                        artist: item.artist_name,
                        album: item.album_name,
                        duration_ms: item
                            .duration
                            .map(|seconds| (seconds * 1000.0).round() as u64),
                        source: self.display_name().into(),
                        synced: capabilities.synced,
                        has_translation: capabilities.translation,
                        has_word_timing: capabilities.word_timing,
                        has_romanization: capabilities.romanization,
                        score: 0.0,
                        lyrics,
                    };
                    result.score = score_candidate(input, &result);
                    Some(result)
                })
                .collect::<Vec<_>>();
            results.sort_by(|left, right| right.score.total_cmp(&left.score));
            results.truncate(8);
            Ok(results)
        })
    }

    fn fetch<'a>(
        &'a self,
        _client: &'a reqwest::Client,
        result: &'a LyricsSearchResult,
    ) -> ProviderFuture<'a, String> {
        Box::pin(async move {
            if result.lyrics.trim().is_empty() {
                Err(self.error(ProviderErrorKind::NotFound, "候选没有同步歌词"))
            } else {
                Ok(result.lyrics.clone())
            }
        })
    }

    fn parse(&self, raw: &str, manual_selected: bool) -> Result<LyricsDocument, ProviderError> {
        parse_lrc_with_options(raw, self.display_name(), manual_selected)
            .map_err(|message| self.error(ProviderErrorKind::Parse, message))
    }
}

impl LrcLibProvider {
    fn error(&self, kind: ProviderErrorKind, message: impl Into<String>) -> ProviderError {
        ProviderError {
            provider_id: self.id().into(),
            kind,
            message: message.into(),
        }
    }
}
