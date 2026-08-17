use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use futures::future::join_all;
use reqwest::header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use serde::{Deserialize, Serialize};

use super::parse_lrc_with_options;
use super::provider::{
    collect_provider_results, score_candidate, LyricsProvider, LyricsSearchInput,
    LyricsSearchResult, ProviderError, ProviderErrorKind, ProviderFuture, ProviderSearchReport,
    ProviderSettings, AMLL_DISPLAY_NAME,
};

const INDEX_PATHS: [&str; 4] = [
    "ncm-lyrics/index.jsonl",
    "qq-lyrics/index.jsonl",
    "am-lyrics/index.jsonl",
    "spotify-lyrics/index.jsonl",
];

#[derive(Debug, Clone)]
struct AmllEntry {
    id: String,
    title: String,
    artist: String,
    album: Option<String>,
    duration_ms: Option<u64>,
    path: String,
    canonical_key: String,
}

#[derive(Debug, Deserialize)]
struct IndexEntry {
    id: String,
    #[serde(default)]
    metadata: Vec<(String, Vec<String>)>,
    #[serde(default, rename = "rawLyricFile")]
    raw_lyric_file: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct CachedIndex {
    path: String,
    etag: Option<String>,
    last_modified: Option<String>,
    body: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct CachedManifest {
    indexes: Vec<CachedIndex>,
}

enum IndexFetch {
    NotModified,
    Fresh(CachedIndex),
    Failed(String),
}

pub struct AmllTtmlProvider {
    settings: Arc<RwLock<ProviderSettings>>,
    cache_path: Option<PathBuf>,
}

impl AmllTtmlProvider {
    pub fn new(settings: Arc<RwLock<ProviderSettings>>, cache_path: Option<PathBuf>) -> Self {
        Self {
            settings,
            cache_path,
        }
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
            let (indexes, mut warning) = self.load_indexes(client, &base_url).await?;
            let mut latest = HashMap::<String, AmllEntry>::new();
            let mut invalid_lines = 0_u64;
            for (index_path, body) in indexes {
                let folder = index_path
                    .split_once('/')
                    .map(|(folder, _)| folder)
                    .unwrap_or_default();
                for line in body.lines().filter(|line| !line.trim().is_empty()) {
                    match serde_json::from_str::<IndexEntry>(line) {
                        Ok(entry) => {
                            if let Some(entry) = convert_index_entry(folder, entry) {
                                latest.insert(entry.path.clone(), entry);
                            }
                        }
                        Err(_) => invalid_lines = invalid_lines.saturating_add(1),
                    }
                }
            }
            if invalid_lines > 0 {
                let message = format!("有 {invalid_lines} 条索引记录格式无效");
                warning = Some(match warning {
                    Some(existing) => format!("{existing}；{message}"),
                    None => message,
                });
            }
            let mut candidates = latest
                .into_values()
                .filter(|entry| !entry.title.is_empty())
                .map(|entry| (metadata_score(input, &entry), entry))
                .collect::<Vec<_>>();
            candidates.sort_by(|left, right| right.0.total_cmp(&left.0));
            let mut seen = HashSet::new();
            candidates.retain(|(_, entry)| seen.insert(entry.canonical_key.clone()));
            candidates.truncate(5);
            let outcomes = join_all(
                candidates
                    .into_iter()
                    .map(|(_, entry)| self.fetch_result(client, input, &base_url, entry)),
            )
            .await;
            let mut report = collect_provider_results(outcomes)?;
            report.warning = match (report.warning, warning) {
                (Some(provider), Some(index)) => Some(format!("{provider}；{index}")),
                (provider, index) => provider.or(index),
            };
            Ok(report)
        })
    }
}

impl AmllTtmlProvider {
    async fn load_indexes(
        &self,
        client: &reqwest::Client,
        base_url: &str,
    ) -> Result<(Vec<(String, String)>, Option<String>), ProviderError> {
        let mut cache = self.read_cache().unwrap_or_default();
        let mut indexes = Vec::new();
        let mut failures = Vec::new();
        let jobs = INDEX_PATHS.into_iter().map(|path| {
            let cached = cache
                .indexes
                .iter()
                .find(|index| index.path == path)
                .cloned();
            let url = format!("{}/{path}", base_url.trim_end_matches('/'));
            let mut request = client.get(url);
            if let Some(cached) = &cached {
                if let Some(etag) = &cached.etag {
                    request = request.header(IF_NONE_MATCH, etag);
                }
                if let Some(last_modified) = &cached.last_modified {
                    request = request.header(IF_MODIFIED_SINCE, last_modified);
                }
            }
            async move {
                let outcome = match request.send().await {
                    Ok(response) if response.status() == reqwest::StatusCode::NOT_MODIFIED => {
                        IndexFetch::NotModified
                    }
                    Ok(response) if response.status().is_success() => {
                        let etag = header_value(&response, ETAG);
                        let last_modified = header_value(&response, LAST_MODIFIED);
                        match response.text().await {
                            Ok(body) if valid_jsonl_index(&body) => {
                                IndexFetch::Fresh(CachedIndex {
                                    path: path.to_string(),
                                    etag,
                                    last_modified,
                                    body,
                                })
                            }
                            Ok(_) => IndexFetch::Failed(format!("{path} 不是有效索引")),
                            Err(_) => IndexFetch::Failed(format!("{path} 响应读取失败")),
                        }
                    }
                    Ok(response) => {
                        IndexFetch::Failed(format!("{path} 返回 HTTP {}", response.status()))
                    }
                    Err(_) => IndexFetch::Failed(format!("{path} 请求失败")),
                };
                (path, cached, outcome)
            }
        });

        for (path, cached, outcome) in join_all(jobs).await {
            match outcome {
                IndexFetch::NotModified => match cached {
                    Some(cached) => indexes.push((path.to_string(), cached.body)),
                    None => failures.push(format!("{path} 返回了无缓存的 304 响应")),
                },
                IndexFetch::Fresh(index) => {
                    cache.indexes.retain(|cached| cached.path != path);
                    indexes.push((path.to_string(), index.body.clone()));
                    cache.indexes.push(index);
                }
                IndexFetch::Failed(message) => {
                    failures.push(message);
                    if let Some(cached) = cached {
                        indexes.push((path.to_string(), cached.body));
                    }
                }
            }
        }

        if indexes.is_empty() {
            return Err(self.error(
                ProviderErrorKind::Network,
                failures
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| "无法读取 AMLL TTML 索引".into()),
            ));
        }
        self.write_cache(&cache);
        let warning = (!failures.is_empty()).then(|| {
            format!(
                "部分索引更新失败，已使用可用索引或缓存：{}",
                failures.join("；")
            )
        });
        Ok((indexes, warning))
    }

    async fn fetch_result(
        &self,
        client: &reqwest::Client,
        input: &LyricsSearchInput,
        base_url: &str,
        entry: AmllEntry,
    ) -> Result<Option<LyricsSearchResult>, ProviderError> {
        let url = reqwest::Url::parse(&format!(
            "{}/{}",
            base_url.trim_end_matches('/'),
            entry.path.trim_start_matches('/')
        ))
        .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?;
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|error| self.error(ProviderErrorKind::Network, error.to_string()))?;
        if !response.status().is_success() {
            return Err(self.error(
                ProviderErrorKind::Http,
                format!("TTML 返回 HTTP {}", response.status()),
            ));
        }
        let lyrics = response
            .text()
            .await
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?;
        let Some(document) = parse_lrc_with_options(&lyrics, self.display_name(), false).ok()
        else {
            return Ok(None);
        };
        let duration_ms = entry.duration_ms.or_else(|| {
            document
                .tracks
                .original
                .lines
                .last()
                .and_then(|line| line.end_ms)
        });
        let mut result = LyricsSearchResult {
            id: entry.id,
            provider_id: self.id().into(),
            title: entry.title,
            artist: entry.artist,
            album: entry.album,
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

    fn read_cache(&self) -> Option<CachedManifest> {
        let path = self.cache_path.as_ref()?;
        fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
    }

    fn write_cache(&self, cache: &CachedManifest) {
        let Some(path) = &self.cache_path else {
            return;
        };
        let Some(parent) = path.parent() else {
            return;
        };
        if fs::create_dir_all(parent).is_err() {
            return;
        }
        let Ok(raw) = serde_json::to_string(cache) else {
            return;
        };
        let temporary = path.with_extension("tmp");
        if fs::write(&temporary, raw).is_ok() {
            let _ = fs::rename(temporary, path);
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

fn header_value(response: &reqwest::Response, name: reqwest::header::HeaderName) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn valid_jsonl_index(body: &str) -> bool {
    body.lines()
        .find(|line| !line.trim().is_empty())
        .is_some_and(|line| serde_json::from_str::<IndexEntry>(line).is_ok())
}

fn convert_index_entry(folder: &str, entry: IndexEntry) -> Option<AmllEntry> {
    let title = metadata_values(&entry.metadata, "musicName").join(" / ");
    if entry.id.trim().is_empty() || title.is_empty() {
        return None;
    }
    let artist = metadata_values(&entry.metadata, "artists").join(" / ");
    let album = metadata_values(&entry.metadata, "album")
        .first()
        .cloned()
        .filter(|album| !album.is_empty());
    let duration_ms = metadata_values(&entry.metadata, "duration")
        .first()
        .and_then(|duration| parse_duration_ms(duration));
    let path = format!("{folder}/{}.ttml", entry.id);
    let canonical_key = if entry.raw_lyric_file.is_empty() {
        path.clone()
    } else {
        entry.raw_lyric_file
    };
    Some(AmllEntry {
        id: path.clone(),
        title,
        artist,
        album,
        duration_ms,
        path,
        canonical_key,
    })
}

fn metadata_values<'a>(metadata: &'a [(String, Vec<String>)], key: &str) -> &'a [String] {
    metadata
        .iter()
        .find(|(candidate, _)| candidate == key)
        .map(|(_, values)| values.as_slice())
        .unwrap_or_default()
}

fn parse_duration_ms(raw: &str) -> Option<u64> {
    if let Ok(milliseconds) = raw.parse::<u64>() {
        return Some(if milliseconds < 10_000 {
            milliseconds.saturating_mul(1000)
        } else {
            milliseconds
        });
    }
    let parts = raw.split(':').collect::<Vec<_>>();
    let (hours, minutes, seconds): (u64, u64, f64) = match parts.as_slice() {
        [minutes, seconds] => (0_u64, minutes.parse().ok()?, seconds.parse::<f64>().ok()?),
        [hours, minutes, seconds] => (
            hours.parse().ok()?,
            minutes.parse().ok()?,
            seconds.parse::<f64>().ok()?,
        ),
        _ => return None,
    };
    Some(
        hours
            .saturating_mul(3_600_000)
            .saturating_add(minutes.saturating_mul(60_000))
            .saturating_add((seconds * 1000.0).round() as u64),
    )
}

fn metadata_score(input: &LyricsSearchInput, entry: &AmllEntry) -> f64 {
    let result = LyricsSearchResult {
        id: entry.id.clone(),
        provider_id: "amll_ttml".into(),
        title: entry.title.clone(),
        artist: entry.artist.clone(),
        album: entry.album.clone(),
        duration_ms: entry.duration_ms,
        source: AMLL_DISPLAY_NAME.into(),
        synced: true,
        has_translation: false,
        has_word_timing: true,
        has_romanization: false,
        score: 0.0,
        lyrics: String::new(),
    };
    score_candidate(input, &result)
}
