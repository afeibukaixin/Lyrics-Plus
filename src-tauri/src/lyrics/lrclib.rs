use serde::Deserialize;

use super::parse_lrc_with_options;
use super::provider::{
    score_candidate, LyricsProvider, LyricsSearchInput, LyricsSearchResult, ProviderError,
    ProviderErrorKind, ProviderFuture, ProviderSearchReport, LRCLIB_DISPLAY_NAME,
};

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

    fn search<'a>(
        &'a self,
        client: &'a reqwest::Client,
        input: &'a LyricsSearchInput,
    ) -> ProviderFuture<'a, ProviderSearchReport> {
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
            let mut results = items
                .into_iter()
                .filter_map(|item| {
                    let lyrics = item.synced_lyrics?.trim().to_string();
                    if lyrics.is_empty() {
                        return None;
                    }
                    let (has_translation, has_word_timing, has_romanization) =
                        capabilities(&lyrics);
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
                        synced: true,
                        has_translation,
                        has_word_timing,
                        has_romanization,
                        score: 0.0,
                        lyrics,
                    };
                    result.score = score_candidate(input, &result);
                    Some(result)
                })
                .collect::<Vec<_>>();
            results.sort_by(|left, right| right.score.total_cmp(&left.score));
            results.truncate(8);
            Ok(ProviderSearchReport::available(results))
        })
    }
}

fn capabilities(lyrics: &str) -> (bool, bool, bool) {
    let parsed = parse_lrc_with_options(lyrics, LRCLIB_DISPLAY_NAME, false).ok();
    let has_translation = parsed
        .as_ref()
        .is_some_and(|document| document.tracks.translation.is_some());
    let has_word_timing = parsed.as_ref().is_some_and(|document| {
        document
            .tracks
            .original
            .lines
            .iter()
            .any(|line| line.words.as_ref().is_some_and(|words| !words.is_empty()))
    });
    let has_romanization = parsed
        .as_ref()
        .is_some_and(|document| document.tracks.romanization.is_some());
    (has_translation, has_word_timing, has_romanization)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_plain_synced_lyrics_capabilities() {
        assert_eq!(capabilities("[00:01.00]Hello"), (false, false, false));
    }

    #[test]
    fn detects_embedded_translation_and_romanization() {
        assert_eq!(
            capabilities("[00:01.00]今日は\n[00:01.00]今天\n[00:01.00]kyou wa"),
            (true, false, true)
        );
    }

    #[test]
    fn detects_embedded_word_timing() {
        assert_eq!(
            capabilities("[00:01.00]<00:01.00>Hello <00:01.50>world"),
            (false, true, false)
        );
    }
}
