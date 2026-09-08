use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use futures::future::join_all;
use serde::Deserialize;
use serde_json::Value;

use super::parse_lrc_with_options;
use super::provider::{
    collect_provider_results, version_tags_from_title, LyricsProvider, LyricsSearchInput,
    LyricsSearchResult, ProviderCandidate, ProviderCandidateReport, ProviderCapabilities,
    ProviderError, ProviderErrorKind, ProviderFuture, ProviderSearchReport, QISHUI_DISPLAY_NAME,
};

pub(crate) const QISHUI_PROVIDER_ID: &str = "qishui";

const SEARCH_ENDPOINT: &str = "https://api.qishui.com/luna/search/track";
const DETAIL_ENDPOINT: &str = "https://beta-luna.douyin.com/luna/h5/seo_track";
const SEARCH_USER_AGENT: &str =
    "com.luna.music/100198030 (Linux; U; Android 15; zh_CN_#Hans; ABR-AL80; Build/V417IR;tt-ok/3.12.13.19)";
const WEB_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
const CLIENT_DEVICE_PLATFORM: &str = "android";
const CLIENT_APP_NAME: &str = "luna";
const CLIENT_VERSION_CODE: &str = "100198030";
const CLIENT_VERSION_NAME: &str = "19.8.0";
const CLIENT_PACKAGE: &str = "com.luna.music";
const CLIENT_CHANNEL: &str = "xiaomi_8478_64";
const CLIENT_DEVICE_TYPE: &str = "ABR-AL80";
const CLIENT_DEVICE_BRAND: &str = "HUAWEI";
const CLIENT_OS_VERSION: &str = "15";
const CLIENT_OS_API: &str = "35";
const CLIENT_CARRIER_ID: &str = "386088";
const CLIENT_CDID: &str = "46556f98-1720-4248-83da-62b74b60b46a";

static NEXT_CLIENT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct SearchEnvelope {
    #[serde(default)]
    result_groups: Vec<SearchResultGroup>,
}

#[derive(Debug, Deserialize)]
struct SearchResultGroup {
    #[serde(default)]
    data: Vec<SearchResultItem>,
}

#[derive(Debug, Deserialize)]
struct SearchResultItem {
    meta: Option<SearchResultMeta>,
    entity: Option<SearchEntity>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct SearchResultMeta {
    item_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchEntity {
    track: Option<SodaTrack>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum SodaId {
    String(String),
    Unsigned(u64),
    Signed(i64),
}

impl SodaId {
    fn as_string(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::Unsigned(value) => value.to_string(),
            Self::Signed(value) => value.to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
struct SodaTrack {
    id: Option<SodaId>,
    name: Option<String>,
    #[serde(default)]
    artists: Vec<SodaArtist>,
    album: Option<SodaAlbum>,
    duration: Option<u64>,
    tags: Option<Vec<Value>>,
}

#[derive(Debug, Clone, Deserialize)]
struct SodaArtist {
    name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct SodaAlbum {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct DetailEnvelope {
    lyric: Option<SodaLyric>,
    track: Option<SodaTrack>,
    seo_track: Option<SodaSeoTrack>,
}

#[derive(Debug, Deserialize)]
struct SodaSeoTrack {
    track: Option<SodaTrack>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct SodaLyric {
    content: Option<String>,
    translations: Option<Value>,
    lang_translations: Option<Value>,
}

pub struct QishuiProvider;

impl LyricsProvider for QishuiProvider {
    fn id(&self) -> &'static str {
        QISHUI_PROVIDER_ID
    }

    fn display_name(&self) -> &'static str {
        QISHUI_DISPLAY_NAME
    }

    fn search<'a>(
        &'a self,
        client: &'a reqwest::Client,
        input: &'a LyricsSearchInput,
    ) -> ProviderFuture<'a, ProviderSearchReport> {
        Box::pin(async move {
            let report = self.search_candidates(client, input).await?;
            let candidates = report.candidates.into_iter().collect::<Vec<_>>();
            let fetched = join_all(
                candidates
                    .iter()
                    .map(|candidate| self.fetch(client, input, candidate)),
            )
            .await;
            collect_provider_results(fetched)
        })
    }

    fn search_candidates<'a>(
        &'a self,
        client: &'a reqwest::Client,
        input: &'a LyricsSearchInput,
    ) -> ProviderFuture<'a, ProviderCandidateReport> {
        Box::pin(async move {
            let tracks = self.search_tracks(client, input).await?;
            Ok(ProviderCandidateReport {
                candidates: tracks
                    .into_iter()
                    .filter_map(|track| self.candidate_from_track(track, None))
                    .collect(),
                warning: None,
            })
        })
    }

    fn lookup_by_id<'a>(
        &'a self,
        client: &'a reqwest::Client,
        _input: &'a LyricsSearchInput,
        provider_item_id: &'a str,
    ) -> ProviderFuture<'a, ProviderCandidateReport> {
        Box::pin(async move {
            let detail = self.fetch_detail(client, provider_item_id).await?;
            let Some(track) = detail_track(&detail) else {
                return Ok(ProviderCandidateReport {
                    candidates: Vec::new(),
                    warning: None,
                });
            };
            Ok(ProviderCandidateReport {
                candidates: self
                    .candidate_from_track(track.clone(), Some(provider_item_id))
                    .into_iter()
                    .collect(),
                warning: None,
            })
        })
    }

    fn fetch<'a>(
        &'a self,
        client: &'a reqwest::Client,
        _input: &'a LyricsSearchInput,
        candidate: &'a ProviderCandidate,
    ) -> ProviderFuture<'a, Option<LyricsSearchResult>> {
        Box::pin(async move {
            let detail = self
                .fetch_detail(client, &candidate.provider_item_id)
                .await?;
            Ok(self.result_from_detail(candidate.metadata_result(), &detail))
        })
    }
}

impl QishuiProvider {
    async fn search_tracks(
        &self,
        client: &reqwest::Client,
        input: &LyricsSearchInput,
    ) -> Result<Vec<SodaTrack>, ProviderError> {
        let mut url = reqwest::Url::parse(SEARCH_ENDPOINT)
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?;
        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("device_platform", CLIENT_DEVICE_PLATFORM)
                .append_pair("os", "android")
                .append_pair("ssmix", "a")
                .append_pair("cdid", CLIENT_CDID)
                .append_pair("channel", CLIENT_CHANNEL)
                .append_pair("aid", CLIENT_CARRIER_ID)
                .append_pair("app_name", CLIENT_APP_NAME)
                .append_pair("version_code", CLIENT_VERSION_CODE)
                .append_pair("version_name", CLIENT_VERSION_NAME)
                .append_pair("manifest_version_code", CLIENT_VERSION_CODE)
                .append_pair("update_version_code", CLIENT_VERSION_CODE)
                .append_pair("resolution", "1080*1920")
                .append_pair("dpi", "480")
                .append_pair("device_type", CLIENT_DEVICE_TYPE)
                .append_pair("device_brand", CLIENT_DEVICE_BRAND)
                .append_pair("language", "zh")
                .append_pair("os_api", CLIENT_OS_API)
                .append_pair("os_version", CLIENT_OS_VERSION)
                .append_pair("ac", "wifi")
                .append_pair("device_model", CLIENT_DEVICE_TYPE)
                .append_pair("tz_name", "Asia/Shanghai")
                .append_pair("tz_offset", "28800")
                .append_pair("package", CLIENT_PACKAGE)
                .append_pair("sim_region", "cn")
                .append_pair("iid", &ephemeral_client_id())
                .append_pair("device_id", &ephemeral_client_id())
                .append_pair(
                    "_rticket",
                    &SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis()
                        .to_string(),
                )
                .append_pair(
                    "q",
                    &format!("{} {}", input.title.trim(), input.artist.trim()),
                )
                .append_pair("cursor", "0")
                .append_pair("count", "20");
        }
        let response = client
            .get(url)
            .header(reqwest::header::ACCEPT, "*/*")
            .header(reqwest::header::USER_AGENT, SEARCH_USER_AGENT)
            .send()
            .await
            .map_err(|error| {
                self.error(
                    ProviderErrorKind::Network,
                    format!("汽水音乐搜索失败：{}", error.without_url()),
                )
            })?;
        if !response.status().is_success() {
            return Err(super::provider::response_error(
                self.id(),
                &response,
                "汽水音乐搜索请求失败",
            ));
        }
        let envelope = response.json::<SearchEnvelope>().await.map_err(|error| {
            self.error(
                ProviderErrorKind::InvalidResponse,
                format!("无法解析汽水音乐搜索结果：{error}"),
            )
        })?;
        Ok(envelope
            .result_groups
            .into_iter()
            .flat_map(|group| group.data)
            .filter(|item| {
                item.meta
                    .as_ref()
                    .and_then(|meta| meta.item_type.as_deref())
                    .is_none_or(|item_type| item_type == "track")
            })
            .filter_map(|item| item.entity.and_then(|entity| entity.track))
            .collect())
    }

    async fn fetch_detail(
        &self,
        client: &reqwest::Client,
        provider_item_id: &str,
    ) -> Result<DetailEnvelope, ProviderError> {
        let provider_item_id = provider_item_id.trim();
        if provider_item_id.is_empty() {
            return Err(self.error(ProviderErrorKind::InvalidResponse, "汽水音乐歌曲 ID 为空"));
        }
        let mut url = reqwest::Url::parse(DETAIL_ENDPOINT)
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?;
        url.query_pairs_mut()
            .append_pair("track_id", provider_item_id)
            .append_pair("device_platform", "web");
        let response = client
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::USER_AGENT, WEB_USER_AGENT)
            .send()
            .await
            .map_err(|error| {
                self.error(
                    ProviderErrorKind::Network,
                    format!("汽水音乐歌词获取失败：{}", error.without_url()),
                )
            })?;
        if !response.status().is_success() {
            return Err(super::provider::response_error(
                self.id(),
                &response,
                "汽水音乐详情请求失败",
            ));
        }
        response.json::<DetailEnvelope>().await.map_err(|error| {
            self.error(
                ProviderErrorKind::InvalidResponse,
                format!("无法解析汽水音乐详情结果：{error}"),
            )
        })
    }

    fn candidate_from_track(
        &self,
        track: SodaTrack,
        fallback_id: Option<&str>,
    ) -> Option<ProviderCandidate> {
        let provider_item_id = track
            .id
            .as_ref()
            .map(SodaId::as_string)
            .or_else(|| fallback_id.map(str::to_owned))?;
        if provider_item_id.trim().is_empty() {
            return None;
        }
        let title = track.name.unwrap_or_default();
        if title.trim().is_empty() {
            return None;
        }
        let artists = track
            .artists
            .into_iter()
            .filter_map(|artist| artist.name)
            .map(|artist| artist.trim().to_owned())
            .filter(|artist| !artist.is_empty())
            .collect::<Vec<_>>();
        Some(ProviderCandidate {
            provider_id: self.id().into(),
            provider_item_id,
            title: title.clone(),
            artists,
            album: track.album.and_then(|album| album.name),
            duration_ms: track.duration,
            version_tags: version_tags(&title, track.tags.as_deref()),
            capabilities: ProviderCapabilities {
                metadata_search: true,
                id_lookup: true,
                plain_text: true,
                line_timing: true,
                word_timing: true,
                translation: true,
                romanization: false,
            },
            source: self.display_name().into(),
            lookup_key: None,
            legacy_result: None,
        })
    }

    fn result_from_detail(
        &self,
        mut candidate: LyricsSearchResult,
        detail: &DetailEnvelope,
    ) -> Option<LyricsSearchResult> {
        let lyric = detail.lyric.as_ref()?;
        let original = lyric
            .content
            .as_deref()
            .filter(|value| !value.trim().is_empty())?;
        let translation = lyric
            .translations
            .as_ref()
            .and_then(translation_text)
            .or_else(|| lyric.lang_translations.as_ref().and_then(translation_text))
            .filter(|value| has_timed_text(value));
        let mut lyrics = original.trim().to_owned();
        if let Some(translation) = translation {
            lyrics.push_str("\n[lyrics-plus:translation]\n");
            lyrics.push_str(translation.trim());
        }
        let parsed = parse_lrc_with_options(&lyrics, self.display_name(), false).ok()?;
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

    fn error(&self, kind: ProviderErrorKind, message: impl Into<String>) -> ProviderError {
        ProviderError::new(self.id(), kind, message)
    }
}

fn detail_track(detail: &DetailEnvelope) -> Option<&SodaTrack> {
    detail.track.as_ref().or_else(|| {
        detail
            .seo_track
            .as_ref()
            .and_then(|track| track.track.as_ref())
    })
}

fn version_tags(title: &str, tags: Option<&[Value]>) -> Vec<String> {
    let mut values = version_tags_from_title(title);
    if let Some(tags) = tags {
        for tag in tags {
            let mut texts = Vec::new();
            collect_tag_text(tag, &mut texts);
            for text in texts {
                let text = text.trim();
                if !text.is_empty()
                    && is_version_tag(text)
                    && !values.iter().any(|value| value.eq_ignore_ascii_case(text))
                {
                    values.push(text.to_owned());
                }
            }
        }
    }
    values
}

fn collect_tag_text(value: &Value, texts: &mut Vec<String>) {
    match value {
        Value::String(value) => texts.push(value.clone()),
        Value::Array(values) => values
            .iter()
            .for_each(|value| collect_tag_text(value, texts)),
        Value::Object(object) => {
            for key in ["tag_name", "name", "title", "value"] {
                if let Some(value) = object.get(key) {
                    collect_tag_text(value, texts);
                }
            }
        }
        _ => {}
    }
}

fn is_version_tag(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    [
        "live",
        "remix",
        "acoustic",
        "remaster",
        "edit",
        "clean",
        "explicit",
        "instrumental",
        "karaoke",
        "demo",
        "sped",
        "slowed",
        "version",
    ]
    .iter()
    .any(|tag| value.contains(tag))
}

fn translation_text(value: &Value) -> Option<String> {
    match value {
        Value::String(value) if !value.trim().is_empty() => Some(value.clone()),
        Value::Array(values) => values
            .iter()
            .filter_map(translation_text)
            .find(|value| has_timed_text(value)),
        Value::Object(object) => ["cn", "zh", "content", "text", "lyric", "translation"]
            .iter()
            .filter_map(|key| object.get(*key).and_then(translation_text))
            .find(|value| has_timed_text(value)),
        _ => None,
    }
}

fn has_timed_text(value: &str) -> bool {
    value.lines().any(|line| {
        let line = line.trim_start();
        let Some(rest) = line.strip_prefix('[') else {
            return false;
        };
        rest.find(']')
            .is_some_and(|end| rest[..end].contains(':') || rest[..end].contains('.'))
    })
}

fn ephemeral_client_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let sequence = NEXT_CLIENT_ID.fetch_add(1, Ordering::Relaxed);
    format!(
        "{:08}{:08}",
        timestamp % 100_000_000,
        sequence % 100_000_000
    )
}
