use base64::Engine;
use futures::future::join_all;
use lyrics_crypto::decrypter::krc::decrypter::decrypt_lyrics;
use serde::Deserialize;

use super::parse_lrc_with_options;
use super::provider::{
    collect_provider_results, duration_ms_from_seconds_u64, score_candidate, LyricsProvider,
    LyricsSearchInput, LyricsSearchResult, ProviderError, ProviderErrorKind, ProviderFuture,
    ProviderSearchReport, KUGOU_DISPLAY_NAME,
};

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

    fn search<'a>(
        &'a self,
        client: &'a reqwest::Client,
        input: &'a LyricsSearchInput,
    ) -> ProviderFuture<'a, ProviderSearchReport> {
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
                return Err(super::provider::response_error(
                    self.id(),
                    &response,
                    "搜索请求失败",
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

impl KugouProvider {
    async fn fetch_result(
        &self,
        client: &reqwest::Client,
        input: &LyricsSearchInput,
        song: KugouSong,
    ) -> Result<Option<LyricsSearchResult>, ProviderError> {
        let duration_ms = song.duration.map(duration_ms_from_seconds_u64);
        let Some(lyric_candidate) = self
            .search_lyrics(client, &song.file_hash, song.mix_song_id.as_deref())
            .await?
            .into_iter()
            .next()
        else {
            return Ok(None);
        };
        let lyrics = self
            .download(client, &lyric_candidate.id, &lyric_candidate.accesskey)
            .await?;
        let parsed =
            parse_lrc_with_options(&lyrics, self.display_name(), false).map_err(|error| {
                log::debug!("酷狗 KRC 与普通 LRC 歌词均解析失败：{error}");
                self.error(
                    ProviderErrorKind::InvalidResponse,
                    format!("酷狗歌词解析失败：{error}"),
                )
            })?;
        let has_translation = parsed.tracks.translation.is_some();
        let has_word_timing = parsed
            .tracks
            .original
            .lines
            .iter()
            .any(|line| line.words.as_ref().is_some_and(|words| !words.is_empty()));
        let has_romanization = parsed.tracks.romanization.is_some();
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
        Ok(Some(result))
    }

    async fn search_lyrics(
        &self,
        client: &reqwest::Client,
        hash: &str,
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
            return Err(super::provider::response_error(
                self.id(),
                &response,
                "歌词搜索请求失败",
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
        match self.download_krc(client, id, access_key).await {
            Ok(lyrics) if parse_lrc_with_options(&lyrics, self.display_name(), false).is_ok() => {
                return Ok(lyrics);
            }
            Ok(_) => log::debug!("酷狗 KRC 歌词解析失败，回退普通 LRC"),
            Err(error) => log::debug!("酷狗 KRC 歌词获取失败，回退普通 LRC：{error}"),
        }

        self.download_lrc(client, id, access_key).await
    }

    async fn download_krc(
        &self,
        client: &reqwest::Client,
        id: &str,
        access_key: &str,
    ) -> Result<String, ProviderError> {
        let envelope = self
            .download_envelope(client, id, access_key, "krc")
            .await?;
        decrypt_lyrics(&envelope.content)
            .ok_or_else(|| self.error(ProviderErrorKind::InvalidResponse, "酷狗 KRC 歌词解密失败"))
    }

    async fn download_lrc(
        &self,
        client: &reqwest::Client,
        id: &str,
        access_key: &str,
    ) -> Result<String, ProviderError> {
        let envelope = self
            .download_envelope(client, id, access_key, "lrc")
            .await?;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(envelope.content)
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?;
        String::from_utf8(decoded)
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))
    }

    async fn download_envelope(
        &self,
        client: &reqwest::Client,
        id: &str,
        access_key: &str,
        format: &str,
    ) -> Result<DownloadEnvelope, ProviderError> {
        let mut url = reqwest::Url::parse("https://lyrics.kugou.com/download")
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))?;
        url.query_pairs_mut()
            .append_pair("ver", "1")
            .append_pair("client", "pc")
            .append_pair("id", id)
            .append_pair("fmt", format)
            .append_pair("charset", "utf8")
            .append_pair("accesskey", access_key);
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|error| self.error(ProviderErrorKind::Network, error.to_string()))?;
        if !response.status().is_success() {
            return Err(super::provider::response_error(
                self.id(),
                &response,
                "歌词下载请求失败",
            ));
        }
        response
            .json::<DownloadEnvelope>()
            .await
            .map_err(|error| self.error(ProviderErrorKind::InvalidResponse, error.to_string()))
    }

    fn error(&self, kind: ProviderErrorKind, message: impl Into<String>) -> ProviderError {
        ProviderError::new(self.id(), kind, message)
    }
}

fn metadata_score(input: &LyricsSearchInput, song: &KugouSong) -> f64 {
    let result = LyricsSearchResult {
        id: String::new(),
        provider_id: "kugou".into(),
        title: song.song_name.clone(),
        artist: song.singer_name.clone(),
        album: song.album_name.clone(),
        duration_ms: song.duration.map(duration_ms_from_seconds_u64),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn result(id: &str) -> LyricsSearchResult {
        LyricsSearchResult {
            id: id.into(),
            provider_id: "kugou".into(),
            title: id.into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: None,
            source: KUGOU_DISPLAY_NAME.into(),
            synced: true,
            has_translation: false,
            has_word_timing: false,
            has_romanization: false,
            score: 1.0,
            lyrics: format!("[00:01.00]{id}"),
        }
    }

    fn failure(message: &str) -> ProviderError {
        ProviderError::new("kugou", ProviderErrorKind::Network, message)
    }

    #[test]
    fn preserves_song_order_after_concurrent_pipelines() {
        let report =
            collect_provider_results(vec![Ok(Some(result("first"))), Ok(Some(result("second")))])
                .unwrap();
        assert_eq!(report.results[0].id, "first");
        assert_eq!(report.results[1].id, "second");
    }

    #[test]
    fn partial_download_failure_is_degraded() {
        let report = collect_provider_results(vec![
            Err(failure("download failed")),
            Ok(Some(result("available"))),
        ])
        .unwrap();
        assert_eq!(report.results[0].id, "available");
        assert_eq!(
            report.warning.as_ref().map(|error| error.message.as_str()),
            Some("download failed")
        );
    }

    #[test]
    fn all_pipeline_failures_return_the_first_error() {
        let error = collect_provider_results(vec![
            Err(failure("first failure")),
            Err(failure("second failure")),
        ])
        .unwrap_err();
        assert_eq!(error.message, "first failure");
    }

    #[test]
    fn no_lyric_candidate_is_a_successful_empty_result() {
        let report = collect_provider_results(vec![Ok(None)]).unwrap();
        assert!(report.results.is_empty());
        assert!(report.warning.is_none());
    }
}
