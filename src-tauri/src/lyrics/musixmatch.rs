use std::sync::Arc;

use futures::future::join_all;
use serde::Deserialize;
use tokio::sync::Mutex as AsyncMutex;

use super::credentials::{MusixmatchTokenType, ProviderCredentialStore};
use super::parse_lrc_with_options;
use super::provider::{
    collect_provider_results, duration_ms_from_seconds_u64, score_candidate, LyricsProvider,
    LyricsSearchInput, LyricsSearchResult, ProviderError, ProviderErrorKind, ProviderFuture,
    ProviderSearchReport, MUSIXMATCH_DISPLAY_NAME,
};

const DESKTOP_API_BASE: &str = "https://apic-desktop.musixmatch.com/ws/1.1";
const DEVELOPER_API_BASE: &str = "https://api.musixmatch.com/ws/1.1";
const DESKTOP_APP_ID: &str = "web-desktop-app-v1.0";

#[derive(Debug, Deserialize)]
struct MessageEnvelope {
    message: Message,
}

#[derive(Debug, Deserialize)]
struct Message {
    header: MessageHeader,
    body: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct MessageHeader {
    status_code: u16,
    hint: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenBody {
    user_token: String,
}

#[derive(Debug, Deserialize)]
struct TrackSearchBody {
    #[serde(default)]
    track_list: Vec<TrackItem>,
}

#[derive(Debug, Deserialize)]
struct TrackItem {
    track: MusixmatchTrack,
}

#[derive(Debug, Deserialize)]
struct MusixmatchTrack {
    #[serde(default)]
    track_id: u64,
    #[serde(default)]
    track_name: String,
    #[serde(default)]
    artist_name: String,
    #[serde(default)]
    album_name: Option<String>,
    #[serde(default)]
    track_length: Option<u64>,
    #[serde(default)]
    commontrack_id: Option<u64>,
    #[serde(default)]
    has_richsync: Option<u8>,
}

#[derive(Debug, Deserialize)]
struct SubtitlesBody {
    #[serde(default)]
    subtitle_list: Vec<SubtitleItem>,
}

#[derive(Debug, Deserialize)]
struct SubtitleItem {
    subtitle: Subtitle,
}

#[derive(Debug, Deserialize)]
struct Subtitle {
    subtitle_body: String,
    subtitle_language: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum DesktopTokenSource {
    Anonymous,
    Manual,
}

pub struct MusixmatchProvider {
    credentials: Arc<ProviderCredentialStore>,
    anonymous_token_lock: AsyncMutex<()>,
}

impl MusixmatchProvider {
    pub fn new(credentials: Arc<ProviderCredentialStore>) -> Self {
        Self {
            credentials,
            anonymous_token_lock: AsyncMutex::new(()),
        }
    }
}

impl LyricsProvider for MusixmatchProvider {
    fn id(&self) -> &'static str {
        "musixmatch"
    }

    fn display_name(&self) -> &'static str {
        MUSIXMATCH_DISPLAY_NAME
    }

    fn search<'a>(
        &'a self,
        client: &'a reqwest::Client,
        input: &'a LyricsSearchInput,
    ) -> ProviderFuture<'a, ProviderSearchReport> {
        Box::pin(async move {
            match self.credentials.musixmatch_credentials() {
                Some((MusixmatchTokenType::DeveloperApiKey, token)) => {
                    self.search_developer(client, input, &token).await
                }
                Some((MusixmatchTokenType::DesktopUserToken, token)) => {
                    self.search_desktop(client, input, &token, DesktopTokenSource::Manual)
                        .await
                }
                None => {
                    let token = self.anonymous_token(client).await?;
                    match self
                        .search_desktop(client, input, &token, DesktopTokenSource::Anonymous)
                        .await
                    {
                        Err(error) if error.kind == ProviderErrorKind::Unauthorized => {
                            let refreshed = self.refresh_anonymous_token(client, &token).await?;
                            self.search_desktop(
                                client,
                                input,
                                &refreshed,
                                DesktopTokenSource::Anonymous,
                            )
                            .await
                        }
                        outcome => outcome,
                    }
                }
            }
        })
    }
}

impl MusixmatchProvider {
    async fn search_developer(
        &self,
        client: &reqwest::Client,
        input: &LyricsSearchInput,
        token: &str,
    ) -> Result<ProviderSearchReport, ProviderError> {
        let mut url = self.api_url(DEVELOPER_API_BASE, "track.search")?;
        url.query_pairs_mut()
            .append_pair("q_track", input.title.trim())
            .append_pair("q_artist", input.artist.trim())
            .append_pair("page", "1")
            .append_pair("page_size", "10")
            .append_pair("s_track_rating", "desc")
            .append_pair("apikey", token);
        let envelope = self.send(client.get(url)).await?;
        if envelope.message.header.status_code == 404 {
            return Ok(ProviderSearchReport::available(Vec::new()));
        }
        self.ensure_developer_status(envelope.message.header.status_code)?;
        let tracks = self.ranked_tracks(input, envelope.message.body)?;
        let outcomes = join_all(
            tracks
                .into_iter()
                .map(|track| self.fetch_developer_result(client, input, token, track)),
        )
        .await;
        collect_provider_results(outcomes)
    }

    async fn search_desktop(
        &self,
        client: &reqwest::Client,
        input: &LyricsSearchInput,
        token: &str,
        source: DesktopTokenSource,
    ) -> Result<ProviderSearchReport, ProviderError> {
        let mut url = self.api_url(DESKTOP_API_BASE, "macro.subtitles.get")?;
        url.query_pairs_mut()
            .append_pair("namespace", "lyrics_richsynched")
            .append_pair("subtitle_format", "lrc")
            .append_pair("q_track", input.title.trim())
            .append_pair("q_artist", input.artist.trim())
            .append_pair("app_id", DESKTOP_APP_ID)
            .append_pair("usertoken", token);
        if let Some(album) = input
            .album
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            url.query_pairs_mut().append_pair("q_album", album.trim());
        }
        if let Some(duration_ms) = input.duration_ms {
            url.query_pairs_mut().append_pair(
                "q_duration",
                &(duration_ms as f64 / 1000.0).round().to_string(),
            );
        }
        let envelope = self.send_desktop(client.get(url)).await?;
        if envelope.message.header.status_code == 404 {
            return Ok(ProviderSearchReport::available(Vec::new()));
        }
        self.ensure_desktop_status(envelope.message.header.status_code, source)?;
        if nested_status_code(&envelope.message.body) == Some(401) {
            return Err(self.error(
                ProviderErrorKind::Unauthorized,
                "Musixmatch Desktop Token 已失效",
            ));
        }
        let result = self
            .fetch_desktop_result(client, input, token, source, envelope.message.body)
            .await?;
        Ok(ProviderSearchReport::available(
            result.into_iter().collect(),
        ))
    }

    async fn fetch_developer_result(
        &self,
        client: &reqwest::Client,
        input: &LyricsSearchInput,
        token: &str,
        track: MusixmatchTrack,
    ) -> Result<Option<LyricsSearchResult>, ProviderError> {
        let mut url = self.api_url(DEVELOPER_API_BASE, "track.subtitles.get")?;
        url.query_pairs_mut()
            .append_pair("track_id", &track.track_id.to_string())
            .append_pair("subtitle_format", "lrc")
            .append_pair("apikey", token);
        let envelope = self.send(client.get(url)).await?;
        if envelope.message.header.status_code == 404 {
            return Ok(None);
        }
        self.ensure_developer_status(envelope.message.header.status_code)?;
        let body = serde_json::from_value::<SubtitlesBody>(envelope.message.body)
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?;
        self.result_from_subtitles(
            input,
            track,
            body.subtitle_list
                .into_iter()
                .map(|item| item.subtitle)
                .collect(),
        )
    }

    async fn fetch_desktop_result(
        &self,
        client: &reqwest::Client,
        input: &LyricsSearchInput,
        token: &str,
        source: DesktopTokenSource,
        body: serde_json::Value,
    ) -> Result<Option<LyricsSearchResult>, ProviderError> {
        let Some(track) = macro_track(&body) else {
            return Ok(None);
        };
        let line_lyrics = macro_subtitle_body(&body)
            .filter(|value| has_timed_text(value))
            .map(|value| value.trim().to_string());
        let richsync = match (track.commontrack_id, track.has_richsync == Some(1)) {
            (Some(commontrack_id), true) => match self
                .fetch_richsync(client, token, source, commontrack_id)
                .await
            {
                Ok(value) => value,
                Err(error) if line_lyrics.is_some() => {
                    log::debug!(
                        "Musixmatch RichSync 获取失败，使用逐行歌词：{}",
                        error.message
                    );
                    None
                }
                Err(error) => return Err(error),
            },
            _ => None,
        };
        let Some(lyrics) = richsync.or(line_lyrics) else {
            return Ok(None);
        };
        self.result_from_lyrics(input, track, lyrics)
    }

    async fn fetch_richsync(
        &self,
        client: &reqwest::Client,
        token: &str,
        source: DesktopTokenSource,
        commontrack_id: u64,
    ) -> Result<Option<String>, ProviderError> {
        let mut url = self.api_url(DESKTOP_API_BASE, "track.richsync.get")?;
        url.query_pairs_mut()
            .append_pair("commontrack_id", &commontrack_id.to_string())
            .append_pair("app_id", DESKTOP_APP_ID)
            .append_pair("usertoken", token);
        let envelope = self.send_desktop(client.get(url)).await?;
        if envelope.message.header.status_code == 404 {
            return Ok(None);
        }
        self.ensure_desktop_status(envelope.message.header.status_code, source)?;
        let Some(body) = deep_find(&envelope.message.body, "richsync_body") else {
            return Ok(None);
        };
        Ok(richsync_to_lrc(body))
    }

    fn ranked_tracks(
        &self,
        input: &LyricsSearchInput,
        body: serde_json::Value,
    ) -> Result<Vec<MusixmatchTrack>, ProviderError> {
        let body = serde_json::from_value::<TrackSearchBody>(body)
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?;
        let mut tracks = body
            .track_list
            .into_iter()
            .map(|item| item.track)
            .collect::<Vec<_>>();
        tracks.sort_by(|left, right| {
            metadata_score(input, right).total_cmp(&metadata_score(input, left))
        });
        tracks.truncate(5);
        Ok(tracks)
    }

    fn result_from_subtitles(
        &self,
        input: &LyricsSearchInput,
        track: MusixmatchTrack,
        subtitles: Vec<Subtitle>,
    ) -> Result<Option<LyricsSearchResult>, ProviderError> {
        let subtitles = subtitles
            .into_iter()
            .filter(|subtitle| has_timed_text(&subtitle.subtitle_body))
            .collect::<Vec<_>>();
        let Some(original) = subtitles.first() else {
            return Ok(None);
        };
        let translation = subtitles
            .iter()
            .skip(1)
            .find(|subtitle| subtitle.subtitle_language != original.subtitle_language);
        let lyrics = match translation {
            Some(translation) => format!(
                "{}\n[lyrics-plus:translation]\n{}",
                original.subtitle_body.trim(),
                translation.subtitle_body.trim()
            ),
            None => original.subtitle_body.trim().to_string(),
        };
        self.result_from_lyrics(input, track, lyrics)
    }

    fn result_from_lyrics(
        &self,
        input: &LyricsSearchInput,
        track: MusixmatchTrack,
        lyrics: String,
    ) -> Result<Option<LyricsSearchResult>, ProviderError> {
        let parsed = parse_lrc_with_options(&lyrics, self.display_name(), false).ok();
        let mut result = LyricsSearchResult {
            id: track.track_id.to_string(),
            provider_id: self.id().into(),
            title: track.track_name,
            artist: track.artist_name,
            album: track.album_name.filter(|album| !album.is_empty()),
            duration_ms: track.track_length.map(duration_ms_from_seconds_u64),
            source: self.display_name().into(),
            synced: parsed.is_some(),
            has_translation: parsed
                .as_ref()
                .is_some_and(|document| document.tracks.translation.is_some()),
            has_word_timing: parsed.as_ref().is_some_and(|document| {
                document
                    .tracks
                    .original
                    .lines
                    .iter()
                    .any(|line| line.words.as_ref().is_some_and(|words| !words.is_empty()))
            }),
            has_romanization: parsed
                .as_ref()
                .is_some_and(|document| document.tracks.romanization.is_some()),
            score: 0.0,
            lyrics,
        };
        if !result.synced {
            return Ok(None);
        }
        result.score = score_candidate(input, &result);
        Ok(Some(result))
    }

    async fn anonymous_token(&self, client: &reqwest::Client) -> Result<String, ProviderError> {
        if let Some(token) = self.credentials.musixmatch_anonymous_token() {
            return Ok(token);
        }
        let _guard = self.anonymous_token_lock.lock().await;
        if let Some(token) = self.credentials.musixmatch_anonymous_token() {
            return Ok(token);
        }
        self.fetch_anonymous_token(client).await
    }

    async fn refresh_anonymous_token(
        &self,
        client: &reqwest::Client,
        rejected_token: &str,
    ) -> Result<String, ProviderError> {
        let _guard = self.anonymous_token_lock.lock().await;
        if let Some(token) = self.credentials.musixmatch_anonymous_token() {
            if token != rejected_token {
                return Ok(token);
            }
        }
        self.credentials
            .clear_musixmatch_anonymous_token()
            .map_err(|error| self.error(ProviderErrorKind::Configuration, error))?;
        self.fetch_anonymous_token(client).await
    }

    async fn fetch_anonymous_token(
        &self,
        client: &reqwest::Client,
    ) -> Result<String, ProviderError> {
        let mut url = self.api_url(DESKTOP_API_BASE, "token.get")?;
        url.query_pairs_mut().append_pair("app_id", DESKTOP_APP_ID);
        let envelope = self.send_desktop(client.get(url)).await?;
        let status = envelope.message.header.status_code;
        if status != 200 {
            let captcha = envelope
                .message
                .header
                .hint
                .as_deref()
                .is_some_and(|hint| hint.eq_ignore_ascii_case("captcha"));
            let message = if captcha {
                "Musixmatch 匿名 Token 获取触发 captcha，请稍后重试"
            } else {
                "Musixmatch 匿名 Token 获取失败"
            };
            return Err(self.error(
                if status == 401 || status == 403 {
                    ProviderErrorKind::Unauthorized
                } else {
                    ProviderErrorKind::Http
                },
                format!("{message}（状态码 {status}）"),
            ));
        }
        let body = serde_json::from_value::<TokenBody>(envelope.message.body)
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?;
        let token = body.user_token.trim();
        if token.is_empty() {
            return Err(self.error(
                ProviderErrorKind::InvalidResponse,
                "Musixmatch 匿名 Token 响应为空",
            ));
        }
        self.credentials
            .set_musixmatch_anonymous_token(token.to_string())
            .map_err(|error| self.error(ProviderErrorKind::Configuration, error))?;
        Ok(token.to_string())
    }

    async fn send(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<MessageEnvelope, ProviderError> {
        let response = request.send().await.map_err(|error| {
            self.error(ProviderErrorKind::Network, error.without_url().to_string())
        })?;
        if !response.status().is_success() {
            return Err(self.error(
                if matches!(response.status().as_u16(), 401 | 402 | 403) {
                    ProviderErrorKind::Unauthorized
                } else {
                    ProviderErrorKind::Http
                },
                format!("服务返回 HTTP {}", response.status()),
            ));
        }
        response.json::<MessageEnvelope>().await.map_err(|error| {
            self.error(
                ProviderErrorKind::InvalidResponse,
                error.without_url().to_string(),
            )
        })
    }

    async fn send_desktop(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<MessageEnvelope, ProviderError> {
        self.send(
            request
                .header("Origin", "https://www.musixmatch.com")
                .header("Referer", "https://www.musixmatch.com/"),
        )
        .await
    }

    fn api_url(&self, base: &str, method: &str) -> Result<reqwest::Url, ProviderError> {
        reqwest::Url::parse(&format!("{base}/{method}"))
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))
    }

    fn ensure_developer_status(&self, status: u16) -> Result<(), ProviderError> {
        match status {
            200 => Ok(()),
            401 | 402 | 403 | 429 => Err(self.error(
                ProviderErrorKind::Unauthorized,
                format!("Developer API Key 无效、无歌词权限或额度不足（状态码 {status}）"),
            )),
            _ => Err(self.error(ProviderErrorKind::Http, format!("服务返回状态码 {status}"))),
        }
    }

    fn ensure_desktop_status(
        &self,
        status: u16,
        source: DesktopTokenSource,
    ) -> Result<(), ProviderError> {
        match status {
            200 => Ok(()),
            401 | 402 | 403 => {
                let message = match source {
                    DesktopTokenSource::Anonymous => "匿名 Desktop Token 已失效",
                    DesktopTokenSource::Manual => "Desktop Token 无效或无歌词权限",
                };
                Err(self.error(
                    ProviderErrorKind::Unauthorized,
                    format!("{message}（状态码 {status}）"),
                ))
            }
            429 => Err(self.error(
                ProviderErrorKind::Http,
                "Musixmatch Desktop 接口请求过于频繁（状态码 429）",
            )),
            _ => Err(self.error(ProviderErrorKind::Http, format!("服务返回状态码 {status}"))),
        }
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

fn deep_find<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(value) = object.get(key) {
                return Some(value);
            }
            object.values().find_map(|value| deep_find(value, key))
        }
        serde_json::Value::Array(values) => values.iter().find_map(|value| deep_find(value, key)),
        _ => None,
    }
}

fn nested_status_code(value: &serde_json::Value) -> Option<u16> {
    deep_find(value, "status_code")?.as_u64()?.try_into().ok()
}

fn macro_track(body: &serde_json::Value) -> Option<MusixmatchTrack> {
    let value = body
        .get("macro_calls")
        .and_then(|calls| calls.get("matcher.track.get"))
        .and_then(|call| call.get("message"))
        .and_then(|message| message.get("body"))
        .and_then(|body| body.get("track"))
        .or_else(|| deep_find(body, "track"))?;
    serde_json::from_value(value.clone()).ok()
}

fn macro_subtitle_body(body: &serde_json::Value) -> Option<String> {
    deep_find(body, "subtitle_body")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

fn richsync_to_lrc(value: &serde_json::Value) -> Option<String> {
    let entries = match value {
        serde_json::Value::String(raw) => serde_json::from_str::<serde_json::Value>(raw).ok()?,
        serde_json::Value::Array(_) => value.clone(),
        _ => return None,
    };
    let serde_json::Value::Array(entries) = entries else {
        return None;
    };
    let mut lines = Vec::new();
    let mut has_words = false;
    for entry in entries {
        let Some(timestamp) = entry.get("ts").and_then(serde_json::Value::as_f64) else {
            continue;
        };
        let words = entry
            .get("l")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|word| {
                let text = word.get("c")?.as_str()?;
                if text.is_empty() {
                    return None;
                }
                let offset = word
                    .get("o")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0);
                Some((timestamp + offset, text.to_string()))
            })
            .collect::<Vec<_>>();
        if !words.is_empty() {
            has_words = true;
            let text = words
                .into_iter()
                .map(|(start, text)| format!("<{}>{text}", stamp_seconds(start)))
                .collect::<Vec<_>>()
                .join("");
            lines.push((timestamp, format!("[{}]{text}", stamp_seconds(timestamp))));
        } else if let Some(text) = entry
            .get("x")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
        {
            lines.push((timestamp, format!("[{}]{text}", stamp_seconds(timestamp))));
        }
    }
    if !has_words {
        return None;
    }
    lines.sort_by(|left, right| left.0.total_cmp(&right.0));
    Some(
        lines
            .into_iter()
            .map(|(_, line)| line)
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn stamp_seconds(seconds: f64) -> String {
    let total_centiseconds = (seconds.max(0.0) * 100.0).round() as u64;
    let minutes = total_centiseconds / 6_000;
    let remaining = total_centiseconds % 6_000;
    format!("{minutes:02}:{:02}.{:02}", remaining / 100, remaining % 100)
}

fn metadata_score(input: &LyricsSearchInput, track: &MusixmatchTrack) -> f64 {
    let result = LyricsSearchResult {
        id: track.track_id.to_string(),
        provider_id: "musixmatch".into(),
        title: track.track_name.clone(),
        artist: track.artist_name.clone(),
        album: track.album_name.clone(),
        duration_ms: track.track_length.map(duration_ms_from_seconds_u64),
        source: MUSIXMATCH_DISPLAY_NAME.into(),
        synced: true,
        has_translation: false,
        has_word_timing: false,
        has_romanization: false,
        score: 0.0,
        lyrics: String::new(),
    };
    score_candidate(input, &result)
}
