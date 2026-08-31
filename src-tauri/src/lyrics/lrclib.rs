use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Deserialize;

use super::parse_lrc_with_options;
use super::provider::{
    duration_ms_from_seconds, score_candidate, LyricsProvider, LyricsSearchInput,
    LyricsSearchResult, ProviderError, ProviderErrorKind, ProviderFuture, ProviderSearchReport,
    LRCLIB_DISPLAY_NAME,
};

const DEFAULT_COOLDOWN_SECS: u64 = 60;
const MAX_COOLDOWN_SECS: u64 = 300;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrcLibItem {
    id: i64,
    track_name: String,
    artist_name: String,
    album_name: Option<String>,
    duration: Option<f64>,
    synced_lyrics: Option<String>,
    #[serde(default, alias = "lyricsFile")]
    lyricsfile: Option<String>,
}

pub struct LrcLibProvider {
    cooldown_until: Mutex<Option<Instant>>,
}

impl Default for LrcLibProvider {
    fn default() -> Self {
        Self {
            cooldown_until: Mutex::new(None),
        }
    }
}

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
            if let Ok(Some(item)) = self.fetch_exact(client, input).await {
                if let Some(result) = self.result_from_item(input, item) {
                    return Ok(ProviderSearchReport::available(vec![result]));
                }
            }

            let items = self.fetch_broad(client, input).await?;
            let mut results = items
                .into_iter()
                .filter_map(|item| self.result_from_item(input, item))
                .collect::<Vec<_>>();
            results.sort_by(|left, right| right.score.total_cmp(&left.score));
            results.truncate(8);
            Ok(ProviderSearchReport::available(results))
        })
    }
}

impl LrcLibProvider {
    async fn fetch_exact(
        &self,
        client: &reqwest::Client,
        input: &LyricsSearchInput,
    ) -> Result<Option<LrcLibItem>, ProviderError> {
        self.ensure_not_rate_limited()?;
        let mut url = reqwest::Url::parse("https://lrclib.net/api/get")
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?;
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
            if let Some(duration_ms) = input.duration_ms {
                query.append_pair(
                    "duration",
                    &(duration_ms as f64 / 1000.0).round().to_string(),
                );
            }
        }

        let response = client.get(url).send().await.map_err(|error| {
            self.error(
                ProviderErrorKind::Network,
                format!("精确歌词查询失败：{error}"),
            )
        })?;
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(self.rate_limit_error(&response));
        }
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(self.error(
                ProviderErrorKind::Http,
                format!("歌词服务返回 HTTP {}", response.status()),
            ));
        }
        response
            .json::<LrcLibItem>()
            .await
            .map(Some)
            .map_err(|error| {
                self.error(
                    ProviderErrorKind::InvalidResponse,
                    format!("无法解析精确歌词结果：{error}"),
                )
            })
    }

    async fn fetch_broad(
        &self,
        client: &reqwest::Client,
        input: &LyricsSearchInput,
    ) -> Result<Vec<LrcLibItem>, ProviderError> {
        self.ensure_not_rate_limited()?;
        let mut url = reqwest::Url::parse("https://lrclib.net/api/search")
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?;
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
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(self.rate_limit_error(&response));
        }
        if !response.status().is_success() {
            return Err(self.error(
                ProviderErrorKind::Http,
                format!("歌词服务返回 HTTP {}", response.status()),
            ));
        }
        response.json::<Vec<LrcLibItem>>().await.map_err(|error| {
            self.error(
                ProviderErrorKind::InvalidResponse,
                format!("无法解析歌词搜索结果：{error}"),
            )
        })
    }

    fn ensure_not_rate_limited(&self) -> Result<(), ProviderError> {
        let now = Instant::now();
        let mut cooldown = self
            .cooldown_until
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(until) = *cooldown else {
            return Ok(());
        };
        if until <= now {
            *cooldown = None;
            return Ok(());
        }
        let remaining = until.duration_since(now);
        let seconds = remaining
            .as_secs()
            .saturating_add(if remaining.subsec_nanos() > 0 { 1 } else { 0 })
            .max(1);
        Err(self.error(
            ProviderErrorKind::Http,
            format!("LRCLIB 请求冷却中，请约 {seconds} 秒后重试"),
        ))
    }

    fn rate_limit_error(&self, response: &reqwest::Response) -> ProviderError {
        let seconds = self.record_rate_limit(response);
        self.error(
            ProviderErrorKind::Http,
            format!("LRCLIB 请求过于频繁，请约 {seconds} 秒后重试"),
        )
    }

    fn record_rate_limit(&self, response: &reqwest::Response) -> u64 {
        let requested_seconds = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<u64>().ok())
            .map(|value| value.clamp(1, MAX_COOLDOWN_SECS))
            .unwrap_or(DEFAULT_COOLDOWN_SECS);
        let now = Instant::now();
        let requested_until = now + Duration::from_secs(requested_seconds);
        let mut cooldown = self
            .cooldown_until
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let until = match *cooldown {
            Some(existing) if existing > requested_until => existing,
            _ => requested_until,
        };
        *cooldown = Some(until);
        let remaining = until.duration_since(now);
        remaining
            .as_secs()
            .saturating_add(if remaining.subsec_nanos() > 0 { 1 } else { 0 })
            .max(1)
    }

    fn result_from_item(
        &self,
        input: &LyricsSearchInput,
        item: LrcLibItem,
    ) -> Option<LyricsSearchResult> {
        let lyrics = item
            .lyricsfile
            .filter(|value| !value.trim().is_empty())
            .filter(|value| parse_lrc_with_options(value, LRCLIB_DISPLAY_NAME, false).is_ok())
            .or(item.synced_lyrics)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())?;
        let (has_translation, has_word_timing, has_romanization) = capabilities(&lyrics);
        let mut result = LyricsSearchResult {
            id: item.id.to_string(),
            provider_id: self.id().into(),
            title: item.track_name,
            artist: item.artist_name,
            album: item.album_name,
            duration_ms: item.duration.and_then(duration_ms_from_seconds),
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
