use futures::future::join_all;
use serde::Deserialize;

use super::parse_lrc_with_options;
use super::provider::{
    collect_provider_results, score_candidate, LyricsProvider, LyricsSearchInput,
    LyricsSearchResult, ProviderError, ProviderErrorKind, ProviderFuture, ProviderSearchReport,
    MIGU_DISPLAY_NAME,
};

#[derive(Debug, Deserialize)]
struct SearchEnvelope {
    #[serde(default, rename = "songResultData")]
    song_result_data: SongResultData,
}

#[derive(Debug, Default, Deserialize)]
struct SongResultData {
    #[serde(default)]
    result: Vec<MiguSong>,
}

#[derive(Debug, Deserialize)]
struct MiguSong {
    #[serde(default, rename = "copyrightId")]
    copyright_id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    singers: Vec<NamedEntity>,
    #[serde(default)]
    albums: Vec<NamedEntity>,
    #[serde(default)]
    duration: String,
    #[serde(default, rename = "lyricUrl")]
    lrc_url: String,
    #[serde(default, rename = "trcUrl")]
    trc_url: String,
}

#[derive(Debug, Deserialize)]
struct NamedEntity {
    #[serde(default)]
    name: String,
}

pub struct MiguProvider;

impl LyricsProvider for MiguProvider {
    fn id(&self) -> &'static str {
        "migu"
    }

    fn display_name(&self) -> &'static str {
        MIGU_DISPLAY_NAME
    }

    fn search<'a>(
        &'a self,
        client: &'a reqwest::Client,
        input: &'a LyricsSearchInput,
    ) -> ProviderFuture<'a, ProviderSearchReport> {
        Box::pin(async move {
            let mut url = reqwest::Url::parse(
                "https://c.musicapp.migu.cn/MIGUM3.0/v1.0/content/search_all.do",
            )
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?;
            url.query_pairs_mut()
                .append_pair(
                    "text",
                    &format!("{} {}", input.title.trim(), input.artist.trim()),
                )
                .append_pair("pageNo", "1")
                .append_pair("pageSize", "10")
                .append_pair("searchSwitch", r#"{"song":1}"#);
            let response = client
                .get(url)
                .send()
                .await
                .map_err(|error| self.error(ProviderErrorKind::Network, error.to_string()))?;
            if !response.status().is_success() {
                return Err(self.error(
                    ProviderErrorKind::Http,
                    format!("搜索返回 HTTP {}", response.status()),
                ));
            }
            let mut songs = response
                .json::<SearchEnvelope>()
                .await
                .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?
                .song_result_data
                .result;
            songs.sort_by(|left, right| {
                metadata_score(input, right).total_cmp(&metadata_score(input, left))
            });
            songs.truncate(5);
            let outcomes = join_all(
                songs
                    .into_iter()
                    .map(|song| self.fetch_result(client, input, song)),
            )
            .await;
            collect_provider_results(outcomes)
        })
    }
}

impl MiguProvider {
    async fn fetch_result(
        &self,
        client: &reqwest::Client,
        input: &LyricsSearchInput,
        song: MiguSong,
    ) -> Result<Option<LyricsSearchResult>, ProviderError> {
        let mut selected = None;
        let mut last_error = None;
        for (index, lyric_url) in [song.trc_url.as_str(), song.lrc_url.as_str()]
            .into_iter()
            .enumerate()
        {
            if lyric_url.is_empty()
                || selected.is_some()
                || index == 1 && lyric_url == song.trc_url.as_str()
            {
                continue;
            }
            match self.download_lyrics(client, lyric_url).await {
                Ok(lyrics) => {
                    if let Ok(document) =
                        parse_lrc_with_options(&lyrics, self.display_name(), false)
                    {
                        selected = Some((lyrics, document));
                    }
                }
                Err(error) => last_error = Some(error),
            }
        }
        let Some((lyrics, document)) = selected else {
            return match last_error {
                Some(error) => Err(error),
                None => Ok(None),
            };
        };
        let artist = song
            .singers
            .iter()
            .map(|singer| singer.name.as_str())
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>()
            .join(" / ");
        let mut result = LyricsSearchResult {
            id: song.copyright_id,
            provider_id: self.id().into(),
            title: song.name,
            artist,
            album: song
                .albums
                .into_iter()
                .map(|album| album.name)
                .find(|album| !album.is_empty()),
            duration_ms: parse_duration_ms(&song.duration),
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

    async fn download_lyrics(
        &self,
        client: &reqwest::Client,
        raw_url: &str,
    ) -> Result<String, ProviderError> {
        let url = normalize_resource_url(raw_url)
            .ok_or_else(|| self.error(ProviderErrorKind::InvalidResponse, "歌词地址无效"))?;
        let response = client
            .get(url)
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
            .text()
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

fn normalize_resource_url(raw: &str) -> Option<reqwest::Url> {
    let normalized = if raw.starts_with("//") {
        format!("https:{raw}")
    } else {
        raw.to_string()
    };
    reqwest::Url::parse(&normalized).ok()
}

fn parse_duration_ms(raw: &str) -> Option<u64> {
    if let Ok(milliseconds) = raw.parse::<u64>() {
        return Some(if milliseconds < 10_000 {
            milliseconds.saturating_mul(1000)
        } else {
            milliseconds
        });
    }
    let (minutes, seconds) = raw.split_once(':')?;
    Some(
        minutes.trim().parse::<u64>().ok()?.saturating_mul(60_000)
            + seconds.trim().parse::<u64>().ok()?.saturating_mul(1000),
    )
}

fn metadata_score(input: &LyricsSearchInput, song: &MiguSong) -> f64 {
    let artist = song
        .singers
        .iter()
        .map(|singer| singer.name.as_str())
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>()
        .join(" / ");
    let result = LyricsSearchResult {
        id: song.copyright_id.clone(),
        provider_id: "migu".into(),
        title: song.name.clone(),
        artist,
        album: song
            .albums
            .iter()
            .map(|album| album.name.clone())
            .find(|album| !album.is_empty()),
        duration_ms: parse_duration_ms(&song.duration),
        source: MIGU_DISPLAY_NAME.into(),
        synced: true,
        has_translation: false,
        has_word_timing: false,
        has_romanization: false,
        score: 0.0,
        lyrics: String::new(),
    };
    score_candidate(input, &result)
}
