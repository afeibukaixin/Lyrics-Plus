use base64::Engine;
use serde::Deserialize;

use super::provider::{
    score_candidate, LyricsProvider, LyricsSearchInput, LyricsSearchResult, ProviderCapabilities,
    ProviderError, ProviderErrorKind, ProviderFuture, KUGOU_DISPLAY_NAME,
};
use super::{parse_lrc_with_options, LyricsDocument};

#[derive(Debug, Deserialize)]
struct SearchEnvelope {
    data: Option<SearchData>,
}

#[derive(Debug, Deserialize)]
struct SearchData {
    #[serde(default)]
    lists: Vec<KugouSong>,
}

#[derive(Debug, Deserialize)]
struct KugouSong {
    #[serde(rename = "FileHash")]
    file_hash: String,
    #[serde(rename = "SongName")]
    song_name: String,
    #[serde(rename = "SingerName")]
    singer_name: String,
    #[serde(rename = "AlbumName")]
    album_name: Option<String>,
    #[serde(rename = "Duration")]
    duration: Option<u64>,
    #[serde(rename = "MixSongID")]
    mix_song_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LyricSearchEnvelope {
    #[serde(default)]
    candidates: Vec<KugouLyricCandidate>,
}

#[derive(Debug, Deserialize)]
struct KugouLyricCandidate {
    id: String,
    accesskey: String,
}

#[derive(Debug, Deserialize)]
struct DownloadEnvelope {
    content: String,
}

pub struct KugouProvider;

impl LyricsProvider for KugouProvider {
    fn id(&self) -> &'static str {
        "kugou"
    }

    fn display_name(&self) -> &'static str {
        KUGOU_DISPLAY_NAME
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            synced: true,
            translation: true,
            word_timing: true,
            romanization: false,
        }
    }

    fn search<'a>(
        &'a self,
        client: &'a reqwest::Client,
        input: &'a LyricsSearchInput,
    ) -> ProviderFuture<'a, Vec<LyricsSearchResult>> {
        Box::pin(async move {
            let mut url = reqwest::Url::parse("https://songsearch.kugou.com/song_search_v2")
                .map_err(|error| {
                    self.error(ProviderErrorKind::InvalidResponse, error.to_string())
                })?;
            url.query_pairs_mut()
                .append_pair(
                    "keyword",
                    &format!("{} {}", input.title.trim(), input.artist.trim()),
                )
                .append_pair("page", "1")
                .append_pair("pagesize", "10")
                .append_pair("platform", "WebFilter");
            let response = client
                .get(url)
                .header("Referer", "https://www.kugou.com/")
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
            let mut songs = envelope.data.map(|data| data.lists).unwrap_or_default();
            songs.sort_by(|left, right| {
                let left_score = metadata_score(input, left);
                let right_score = metadata_score(input, right);
                right_score.total_cmp(&left_score)
            });
            songs.truncate(4);

            let mut results = Vec::new();
            for song in songs {
                let duration_ms = song.duration.map(|duration| duration * 1000);
                let lyric_candidates = match self
                    .search_lyrics(
                        client,
                        &song.file_hash,
                        duration_ms,
                        song.mix_song_id.as_deref(),
                    )
                    .await
                {
                    Ok(candidates) => candidates,
                    Err(_) => continue,
                };
                let Some(lyric_candidate) = lyric_candidates.into_iter().next() else {
                    continue;
                };
                let lyrics = match self
                    .download(client, &lyric_candidate.id, &lyric_candidate.accesskey)
                    .await
                {
                    Ok(lyrics) => lyrics,
                    Err(_) => continue,
                };
                let parsed = parse_lrc_with_options(&lyrics, self.display_name(), false).ok();
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
                let mut result = LyricsSearchResult {
                    id: format!("{}|{}", lyric_candidate.id, lyric_candidate.accesskey),
                    provider_id: self.id().into(),
                    title: song.song_name,
                    artist: song.singer_name,
                    album: song.album_name.filter(|album| !album.is_empty()),
                    duration_ms,
                    source: self.display_name().into(),
                    synced: true,
                    has_translation,
                    has_word_timing,
                    has_romanization,
                    score: 0.0,
                    lyrics,
                };
                result.score = score_candidate(input, &result);
                results.push(result);
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
            let (id, access_key) = result
                .id
                .split_once('|')
                .ok_or_else(|| self.error(ProviderErrorKind::InvalidResponse, "候选标识无效"))?;
            self.download(client, id, access_key).await
        })
    }

    fn parse(&self, raw: &str, manual_selected: bool) -> Result<LyricsDocument, ProviderError> {
        parse_lrc_with_options(raw, self.display_name(), manual_selected)
            .map_err(|message| self.error(ProviderErrorKind::Parse, message))
    }
}

impl KugouProvider {
    async fn search_lyrics(
        &self,
        client: &reqwest::Client,
        hash: &str,
        duration_ms: Option<u64>,
        mix_song_id: Option<&str>,
    ) -> Result<Vec<KugouLyricCandidate>, ProviderError> {
        let mut url = reqwest::Url::parse("https://lyrics.kugou.com/search")
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?;
        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("ver", "1")
                .append_pair("man", "yes")
                .append_pair("client", "pc")
                .append_pair("hash", hash);
            if let Some(duration_ms) = duration_ms {
                query.append_pair("duration", &duration_ms.to_string());
            }
            if let Some(mix_song_id) = mix_song_id {
                query.append_pair("album_audio_id", mix_song_id);
            }
        }
        let response = client
            .get(url)
            .header("Referer", "https://www.kugou.com/")
            .send()
            .await
            .map_err(|error| self.error(ProviderErrorKind::Network, error.to_string()))?;
        if !response.status().is_success() {
            return Err(self.error(
                ProviderErrorKind::Http,
                format!("歌词搜索返回 HTTP {}", response.status()),
            ));
        }
        response
            .json::<LyricSearchEnvelope>()
            .await
            .map(|envelope| envelope.candidates)
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))
    }

    async fn download(
        &self,
        client: &reqwest::Client,
        id: &str,
        access_key: &str,
    ) -> Result<String, ProviderError> {
        let mut url = reqwest::Url::parse("https://lyrics.kugou.com/download")
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?;
        url.query_pairs_mut()
            .append_pair("ver", "1")
            .append_pair("client", "pc")
            .append_pair("id", id)
            .append_pair("fmt", "lrc")
            .append_pair("charset", "utf8")
            .append_pair("accesskey", access_key);
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|error| self.error(ProviderErrorKind::Network, error.to_string()))?;
        if !response.status().is_success() {
            return Err(self.error(
                ProviderErrorKind::Http,
                format!("歌词下载返回 HTTP {}", response.status()),
            ));
        }
        let envelope = response
            .json::<DownloadEnvelope>()
            .await
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(envelope.content)
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?;
        String::from_utf8(decoded)
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

fn metadata_score(input: &LyricsSearchInput, song: &KugouSong) -> f64 {
    let result = LyricsSearchResult {
        id: String::new(),
        provider_id: "kugou".into(),
        title: song.song_name.clone(),
        artist: song.singer_name.clone(),
        album: song.album_name.clone(),
        duration_ms: song.duration.map(|duration| duration * 1000),
        source: KUGOU_DISPLAY_NAME.into(),
        synced: true,
        has_translation: false,
        has_word_timing: false,
        has_romanization: false,
        score: 0.0,
        lyrics: String::new(),
    };
    score_candidate(input, &result)
}
