use futures::future::join_all;
use serde::Deserialize;

use super::provider::{
    collect_provider_results, score_candidate, LyricsProvider, LyricsSearchInput,
    LyricsSearchResult, ProviderError, ProviderErrorKind, ProviderFuture, ProviderSearchReport,
    QQMUSIC_DISPLAY_NAME,
};

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

    fn search<'a>(
        &'a self,
        client: &'a reqwest::Client,
        input: &'a LyricsSearchInput,
    ) -> ProviderFuture<'a, ProviderSearchReport> {
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

            let details = join_all(
                candidates
                    .iter()
                    .map(|candidate| self.fetch_detail(client, &candidate.id)),
            )
            .await;
            collect_provider_results(candidates.into_iter().zip(details).map(
                |(candidate, detail)| detail.map(|detail| result_from_detail(candidate, detail)),
            ))
        })
    }
}

fn result_from_detail(
    mut candidate: LyricsSearchResult,
    detail: LyricsEnvelope,
) -> Option<LyricsSearchResult> {
    let original = detail.lyric.filter(|value| has_timed_text(value))?;
    let translation = detail.trans.filter(|value| has_timed_text(value));
    candidate.has_translation = translation.is_some();
    candidate.lyrics = merge_tracks(&original, translation.as_deref());
    Some(candidate)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str) -> LyricsSearchResult {
        LyricsSearchResult {
            id: id.into(),
            provider_id: "qqmusic".into(),
            title: id.into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: None,
            source: QQMUSIC_DISPLAY_NAME.into(),
            synced: true,
            has_translation: false,
            has_word_timing: false,
            has_romanization: false,
            score: 1.0,
            lyrics: String::new(),
        }
    }

    fn detail(lyrics: Option<&str>) -> LyricsEnvelope {
        LyricsEnvelope {
            lyric: lyrics.map(str::to_owned),
            trans: None,
        }
    }

    fn failure(message: &str) -> ProviderError {
        ProviderError {
            provider_id: "qqmusic".into(),
            kind: ProviderErrorKind::Network,
            message: message.into(),
        }
    }

    #[test]
    fn preserves_order_and_translation_capability() {
        let mut translated = detail(Some("[00:01.00]First"));
        translated.trans = Some("[00:01.00]第一".into());
        let report = collect_provider_results(vec![
            Ok(result_from_detail(candidate("first"), translated)),
            Ok(result_from_detail(
                candidate("second"),
                detail(Some("[00:01.00]Second")),
            )),
        ])
        .unwrap();

        assert_eq!(report.results[0].id, "first");
        assert_eq!(report.results[1].id, "second");
        assert!(report.results[0].has_translation);
    }

    #[test]
    fn partial_failure_is_degraded_but_keeps_results() {
        let report = collect_provider_results(vec![
            Err(failure("temporary failure")),
            Ok(result_from_detail(
                candidate("available"),
                detail(Some("[00:01.00]Available")),
            )),
        ])
        .unwrap();

        assert_eq!(report.results[0].id, "available");
        assert_eq!(report.warning.as_deref(), Some("temporary failure"));
    }

    #[test]
    fn all_failures_return_the_first_error() {
        let error = collect_provider_results(vec![
            Err(failure("first failure")),
            Err(failure("second failure")),
        ])
        .unwrap_err();
        assert_eq!(error.message, "first failure");
    }

    #[test]
    fn successful_empty_detail_is_available() {
        let report = collect_provider_results(vec![Ok(result_from_detail(
            candidate("empty"),
            detail(None),
        ))])
        .unwrap();
        assert!(report.results.is_empty());
        assert!(report.warning.is_none());
    }
}
