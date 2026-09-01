use futures::future::join_all;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::sync::{Arc, RwLock};

use super::parse_lrc_with_options;
use super::provider::{
    collect_provider_results, score_candidate, LyricsProvider, LyricsSearchInput,
    LyricsSearchResult, ProviderError, ProviderErrorKind, ProviderFuture, ProviderSearchReport,
    ProviderSettings, AMLL_DISPLAY_NAME,
};

#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    status: u16,
    data: Option<T>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchData {
    #[serde(default)]
    items: Vec<SongItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SongItem {
    id: Option<u64>,
    #[serde(default)]
    music_names: Vec<String>,
    #[serde(default)]
    artist_names: Vec<String>,
    #[serde(default)]
    album_names: Vec<String>,
    #[serde(default)]
    lyrics: Option<String>,
}

#[derive(Debug, Clone)]
struct AmllCandidate {
    id: u64,
    title: String,
    artist: String,
    album: Option<String>,
}

pub struct AmllTtmlProvider {
    settings: Arc<RwLock<ProviderSettings>>,
}

impl AmllTtmlProvider {
    pub fn new(settings: Arc<RwLock<ProviderSettings>>) -> Self {
        Self { settings }
    }
}

impl LyricsProvider for AmllTtmlProvider {
    fn id(&self) -> &'static str {
        "amll_ttml"
    }

    fn display_name(&self) -> &'static str {
        AMLL_DISPLAY_NAME
    }

    fn search<'a>(
        &'a self,
        client: &'a reqwest::Client,
        input: &'a LyricsSearchInput,
    ) -> ProviderFuture<'a, ProviderSearchReport> {
        Box::pin(async move {
            let base_url = self
                .settings
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .amll_base_url
                .clone();
            let mut songs = self.fetch_search(client, input, &base_url, false).await?;
            if songs.is_empty() {
                songs = self.fetch_search(client, input, &base_url, true).await?;
            }
            let mut candidates = songs
                .into_iter()
                .filter_map(candidate_from_song)
                .map(|candidate| (metadata_score(input, &candidate), candidate))
                .collect::<Vec<_>>();
            candidates.sort_by(|left, right| right.0.total_cmp(&left.0));
            candidates.truncate(5);
            let outcomes = join_all(
                candidates
                    .into_iter()
                    .map(|(_, candidate)| self.fetch_result(client, input, &base_url, candidate)),
            )
            .await;
            collect_provider_results(outcomes)
        })
    }
}

impl AmllTtmlProvider {
    async fn fetch_search(
        &self,
        client: &reqwest::Client,
        input: &LyricsSearchInput,
        base_url: &str,
        broad: bool,
    ) -> Result<Vec<SongItem>, ProviderError> {
        let mut url = self.api_url(base_url, "/v1/lyrics/search")?;
        {
            let mut query = url.query_pairs_mut();
            if broad {
                query.append_pair("musicName", input.title.trim());
            } else {
                query.append_pair("musicName", input.title.trim());
                if !input.artist.trim().is_empty() {
                    query.append_pair("artistName", input.artist.trim());
                }
            }
            query.append_pair("page", "1").append_pair("pageSize", "20");
        }
        let envelope = self
            .send_json::<ApiResponse<SearchData>>(client, url)
            .await?;
        if envelope.status != 200 {
            return Err(self.api_error(envelope.status, envelope.error, envelope.message));
        }
        Ok(envelope.data.map(|data| data.items).unwrap_or_default())
    }

    async fn fetch_result(
        &self,
        client: &reqwest::Client,
        input: &LyricsSearchInput,
        base_url: &str,
        candidate: AmllCandidate,
    ) -> Result<Option<LyricsSearchResult>, ProviderError> {
        let mut url = self.api_url(base_url, "/v1/lyrics/get")?;
        url.query_pairs_mut()
            .append_pair("id", &candidate.id.to_string());
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|error| self.error(ProviderErrorKind::Network, error.to_string()))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(super::provider::response_error(
                self.id(),
                &response,
                "歌词请求失败",
            ));
        }
        let envelope = response
            .json::<ApiResponse<SongItem>>()
            .await
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?;
        if envelope.status != 200 {
            if envelope.status == 404 {
                return Ok(None);
            }
            return Err(self.api_error(envelope.status, envelope.error, envelope.message));
        }
        let Some(song) = envelope.data else {
            return Ok(None);
        };
        let Some(lyrics) = song.lyrics.filter(|value| !value.trim().is_empty()) else {
            return Ok(None);
        };
        let Some(document) = parse_lrc_with_options(&lyrics, self.display_name(), false).ok()
        else {
            return Ok(None);
        };
        let title = first_value(&song.music_names).unwrap_or(candidate.title);
        let artist = joined_values(&song.artist_names).unwrap_or(candidate.artist);
        let album = first_value(&song.album_names).or(candidate.album);
        let duration_ms = document
            .tracks
            .original
            .lines
            .last()
            .and_then(|line| line.end_ms)
            .or(input.duration_ms);
        let mut result = LyricsSearchResult {
            id: song.id.unwrap_or(candidate.id).to_string(),
            provider_id: self.id().into(),
            title,
            artist,
            album,
            duration_ms,
            source: self.display_name().into(),
            synced: true,
            has_translation: document.tracks.translation.is_some(),
            has_word_timing: document
                .tracks
                .original
                .lines
                .iter()
                .any(|line| line.words.as_ref().is_some_and(|words| !words.is_empty())),
            has_romanization: document.tracks.romanization.is_some(),
            score: 0.0,
            lyrics,
        };
        result.score = score_candidate(input, &result);
        Ok(Some(result))
    }

    async fn send_json<T: DeserializeOwned>(
        &self,
        client: &reqwest::Client,
        url: reqwest::Url,
    ) -> Result<T, ProviderError> {
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|error| self.error(ProviderErrorKind::Network, error.to_string()))?;
        if !response.status().is_success() {
            return Err(super::provider::response_error(
                self.id(),
                &response,
                "服务请求失败",
            ));
        }
        response
            .json::<T>()
            .await
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))
    }

    fn api_url(&self, base_url: &str, path: &str) -> Result<reqwest::Url, ProviderError> {
        reqwest::Url::parse(&format!("{}{}", base_url.trim_end_matches('/'), path))
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))
    }

    fn api_error(
        &self,
        status: u16,
        error: Option<String>,
        message: Option<String>,
    ) -> ProviderError {
        let detail = message.or(error).unwrap_or_else(|| "未知错误".into());
        ProviderError::with_http(
            self.id(),
            status,
            None,
            format!("AMLL API 返回状态 {status}：{detail}"),
        )
    }

    fn error(&self, kind: ProviderErrorKind, message: impl Into<String>) -> ProviderError {
        ProviderError::new(self.id(), kind, message)
    }
}

fn candidate_from_song(song: SongItem) -> Option<AmllCandidate> {
    Some(AmllCandidate {
        id: song.id?,
        title: first_value(&song.music_names)?,
        artist: joined_values(&song.artist_names).unwrap_or_default(),
        album: first_value(&song.album_names),
    })
}

fn first_value(values: &[String]) -> Option<String> {
    values
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(str::to_owned)
}

fn joined_values(values: &[String]) -> Option<String> {
    let values = values
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    (!values.is_empty()).then(|| values.join(" / "))
}

fn metadata_score(input: &LyricsSearchInput, candidate: &AmllCandidate) -> f64 {
    let result = LyricsSearchResult {
        id: candidate.id.to_string(),
        provider_id: "amll_ttml".into(),
        title: candidate.title.clone(),
        artist: candidate.artist.clone(),
        album: candidate.album.clone(),
        duration_ms: None,
        source: AMLL_DISPLAY_NAME.into(),
        synced: true,
        has_translation: false,
        has_word_timing: false,
        has_romanization: false,
        score: 0.0,
        lyrics: String::new(),
    };
    score_candidate(input, &result)
}
