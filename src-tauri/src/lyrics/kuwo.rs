use futures::future::join_all;
use serde::Deserialize;

use super::parse_lrc_with_options;
use super::provider::{
    collect_provider_results, parse_duration_text_ms, score_candidate, DurationUnit,
    LyricsProvider, LyricsSearchInput, LyricsSearchResult, ProviderError, ProviderErrorKind,
    ProviderFuture, ProviderSearchReport, KUWO_DISPLAY_NAME,
};

#[derive(Debug, Deserialize)]
struct SearchEnvelope {
    #[serde(default, rename = "abslist")]
    songs: Vec<KuwoSong>,
}

#[derive(Debug, Deserialize)]
struct KuwoSong {
    #[serde(default, rename = "MUSICRID")]
    music_rid: String,
    #[serde(default, rename = "SONGNAME")]
    song_name: String,
    #[serde(default, rename = "ARTIST")]
    artist: String,
    #[serde(default, rename = "ALBUM")]
    album: String,
    #[serde(default, rename = "DURATION")]
    duration: String,
}

#[derive(Debug, Deserialize)]
struct LyricsEnvelope {
    data: Option<LyricsData>,
}

#[derive(Debug, Deserialize)]
struct LyricsData {
    #[serde(default)]
    lrclist: Vec<KuwoLine>,
}

#[derive(Debug, Deserialize)]
struct KuwoLine {
    #[serde(default)]
    time: String,
    #[serde(default, rename = "lineLyric")]
    line_lyrics: String,
}

pub struct KuwoProvider;

impl LyricsProvider for KuwoProvider {
    fn id(&self) -> &'static str {
        "kuwo"
    }

    fn display_name(&self) -> &'static str {
        KUWO_DISPLAY_NAME
    }

    fn search<'a>(
        &'a self,
        client: &'a reqwest::Client,
        input: &'a LyricsSearchInput,
    ) -> ProviderFuture<'a, ProviderSearchReport> {
        Box::pin(async move {
            let mut url = reqwest::Url::parse("https://search.kuwo.cn/r.s").map_err(|error| {
                self.error(ProviderErrorKind::InvalidResponse, error.to_string())
            })?;
            url.query_pairs_mut()
                .append_pair(
                    "all",
                    &format!("{} {}", input.title.trim(), input.artist.trim()),
                )
                .append_pair("ft", "music")
                .append_pair("itemset", "web_2013")
                .append_pair("client", "kt")
                .append_pair("pn", "0")
                .append_pair("rn", "10")
                .append_pair("rformat", "json")
                .append_pair("encoding", "utf8")
                .append_pair("pcjson", "1");
            let response = client
                .get(url)
                .header("Referer", "https://www.kuwo.cn/")
                .send()
                .await
                .map_err(|error| self.error(ProviderErrorKind::Network, error.to_string()))?;
            if !response.status().is_success() {
                return Err(super::provider::response_error(
                    self.id(),
                    &response,
                    "搜索请求失败",
                ));
            }
            let mut songs = response
                .json::<SearchEnvelope>()
                .await
                .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?
                .songs;
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

impl KuwoProvider {
    async fn fetch_result(
        &self,
        client: &reqwest::Client,
        input: &LyricsSearchInput,
        song: KuwoSong,
    ) -> Result<Option<LyricsSearchResult>, ProviderError> {
        let id = song
            .music_rid
            .split_once('_')
            .map(|(_, id)| id)
            .unwrap_or(song.music_rid.as_str())
            .to_string();
        if id.is_empty() {
            return Ok(None);
        }
        let mut url = reqwest::Url::parse("https://kuwo.cn/openapi/v1/www/lyric/getlyric")
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?;
        url.query_pairs_mut().append_pair("musicId", &id);
        let response = client
            .get(url)
            .header("Referer", "https://kuwo.cn/")
            .send()
            .await
            .map_err(|error| self.error(ProviderErrorKind::Network, error.to_string()))?;
        if !response.status().is_success() {
            return Err(super::provider::response_error(
                self.id(),
                &response,
                "歌词请求失败",
            ));
        }
        let envelope = response
            .json::<LyricsEnvelope>()
            .await
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?;
        let lyrics = envelope
            .data
            .map(|data| {
                data.lrclist
                    .into_iter()
                    .filter(|line| !line.time.is_empty() && !line.line_lyrics.trim().is_empty())
                    .map(|line| format!("[{}]{}", normalize_time(&line.time), line.line_lyrics))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        if lyrics.is_empty() {
            return Ok(None);
        }
        let parsed = parse_lrc_with_options(&lyrics, self.display_name(), false).ok();
        let mut result = LyricsSearchResult {
            id,
            provider_id: self.id().into(),
            title: song.song_name,
            artist: song.artist,
            album: (!song.album.is_empty()).then_some(song.album),
            duration_ms: parse_duration_text_ms(&song.duration, DurationUnit::Seconds),
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

    fn error(&self, kind: ProviderErrorKind, message: impl Into<String>) -> ProviderError {
        ProviderError::new(self.id(), kind, message)
    }
}

fn normalize_time(raw: &str) -> String {
    let seconds = raw.trim().parse::<f64>().unwrap_or(0.0).max(0.0);
    format!(
        "{:02}:{:05.2}",
        (seconds / 60.0).floor() as u64,
        seconds % 60.0
    )
}

fn metadata_score(input: &LyricsSearchInput, song: &KuwoSong) -> f64 {
    let result = LyricsSearchResult {
        id: String::new(),
        provider_id: "kuwo".into(),
        title: song.song_name.clone(),
        artist: song.artist.clone(),
        album: (!song.album.is_empty()).then_some(song.album.clone()),
        duration_ms: parse_duration_text_ms(&song.duration, DurationUnit::Seconds),
        source: KUWO_DISPLAY_NAME.into(),
        synced: true,
        has_translation: false,
        has_word_timing: false,
        has_romanization: false,
        score: 0.0,
        lyrics: String::new(),
    };
    score_candidate(input, &result)
}
