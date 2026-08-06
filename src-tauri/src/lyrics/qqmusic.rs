use serde::Deserialize;

use super::parse_lrc_with_options;
use super::provider::{
    score_candidate, LyricsProvider, LyricsSearchInput, LyricsSearchResult, ProviderCapabilities,
    ProviderError, ProviderErrorKind, ProviderFuture, QQMUSIC_DISPLAY_NAME,
};
use super::LyricsDocument;

#[derive(Debug, Deserialize)]
struct SearchEnvelope {
    data: Option<SearchData>,
}

#[derive(Debug, Deserialize)]
struct SearchData {
    song: Option<SongData>,
}

#[derive(Debug, Deserialize)]
struct SongData {
    #[serde(default)]
    list: Vec<QqSong>,
}

#[derive(Debug, Deserialize)]
struct QqSong {
    songmid: String,
    songname: String,
    albumname: Option<String>,
    interval: Option<u64>,
    #[serde(default)]
    singer: Vec<QqSinger>,
}

#[derive(Debug, Deserialize)]
struct QqSinger {
    name: String,
}

#[derive(Debug, Deserialize)]
struct LyricsEnvelope {
    lyric: Option<String>,
    trans: Option<String>,
}

pub struct QqMusicProvider;

impl LyricsProvider for QqMusicProvider {
    fn id(&self) -> &'static str {
        "qqmusic"
    }

    fn display_name(&self) -> &'static str {
        QQMUSIC_DISPLAY_NAME
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            synced: true,
            translation: true,
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
            let mut url = reqwest::Url::parse("https://c.y.qq.com/soso/fcgi-bin/client_search_cp")
                .map_err(|error| {
                    self.error(ProviderErrorKind::InvalidResponse, error.to_string())
                })?;
            url.query_pairs_mut()
                .append_pair(
                    "w",
                    &format!("{} {}", input.title.trim(), input.artist.trim()),
                )
                .append_pair("p", "1")
                .append_pair("n", "12")
                .append_pair("format", "json");
            let response = client
                .get(url)
                .header("Referer", "https://y.qq.com/")
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
                .data
                .and_then(|data| data.song)
                .map(|song| song.list)
                .unwrap_or_default()
                .into_iter()
                .map(|song| {
                    let mut result = LyricsSearchResult {
                        id: song.songmid,
                        provider_id: self.id().into(),
                        title: song.songname,
                        artist: song
                            .singer
                            .into_iter()
                            .map(|singer| singer.name)
                            .collect::<Vec<_>>()
                            .join(" / "),
                        album: song.albumname.filter(|album| !album.is_empty()),
                        duration_ms: song.interval.map(|seconds| seconds * 1000),
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
                    let Some(original) = detail.lyric.filter(|value| has_timed_text(value)) else {
                        continue;
                    };
                    let translation = detail.trans.filter(|value| has_timed_text(value));
                    candidate.has_translation = translation.is_some();
                    candidate.lyrics = merge_tracks(&original, translation.as_deref());
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
            let original = detail
                .lyric
                .filter(|value| has_timed_text(value))
                .ok_or_else(|| self.error(ProviderErrorKind::NotFound, "没有同步歌词"))?;
            let translation = detail.trans.filter(|value| has_timed_text(value));
            Ok(merge_tracks(&original, translation.as_deref()))
        })
    }

    fn parse(&self, raw: &str, manual_selected: bool) -> Result<LyricsDocument, ProviderError> {
        parse_lrc_with_options(raw, self.display_name(), manual_selected)
            .map_err(|message| self.error(ProviderErrorKind::Parse, message))
    }
}

impl QqMusicProvider {
    async fn fetch_detail(
        &self,
        client: &reqwest::Client,
        song_mid: &str,
    ) -> Result<LyricsEnvelope, ProviderError> {
        let mut url =
            reqwest::Url::parse("https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg")
                .map_err(|error| {
                    self.error(ProviderErrorKind::InvalidResponse, error.to_string())
                })?;
        url.query_pairs_mut()
            .append_pair("songmid", song_mid)
            .append_pair("format", "json")
            .append_pair("nobase64", "1");
        let response = client
            .get(url)
            .header("Referer", "https://y.qq.com/")
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

fn merge_tracks(original: &str, translation: Option<&str>) -> String {
    match translation {
        Some(translation) => format!("{}\n{}", original.trim(), translation.trim()),
        None => original.trim().to_string(),
    }
}
