mod basic_lrc;
mod lyricsfile;
mod normalize;
mod platform;
mod ttml;

use super::{LyricsDocument, LyricsMetadata, LyricsTrack, LyricsTracks};

pub(crate) use normalize::{lyrics_quality_report, semantic_fingerprint, LyricsQualityReport};
pub(in crate::lyrics) use platform::parse_integer_list;
pub(in crate::lyrics) use ttml::decode_xml_text;

#[allow(dead_code)]
pub fn parse_lrc(raw: &str, source: impl Into<String>) -> Result<LyricsDocument, String> {
    parse_lrc_with_options(raw, source, false)
}

pub fn parse_lrc_with_options(
    raw: &str,
    source: impl Into<String>,
    manual_selected: bool,
) -> Result<LyricsDocument, String> {
    let source = source.into();
    if let Some(document) = lyricsfile::parse_lyricsfile(raw, &source, manual_selected)? {
        let mut document = document;
        normalize::normalize_document(&mut document);
        return Ok(document);
    }
    if let Some(tracks) = ttml::parse_ttml_lyrics(raw) {
        let mut document = LyricsDocument {
            metadata: LyricsMetadata {
                title: None,
                artist: None,
                album: None,
                source,
                original_format: "ttml".into(),
                manual_selected,
            },
            tracks,
            offset_ms: 0,
            raw: raw.to_string(),
        };
        normalize::normalize_document(&mut document);
        return Ok(document);
    }
    if let Some((format, lines)) = platform::parse_platform_word_lyrics(raw) {
        let (title, artist, album, embedded_offset) = platform::metadata_tags(raw);
        let (mut translation, mut romanization) = platform::parse_auxiliary_lrc_tracks(raw);
        if format == "krc" && (translation.is_empty() || romanization.is_empty()) {
            let (language_translation, language_romanization) =
                platform::parse_krc_language_tracks(raw);
            if translation.is_empty() {
                translation = language_translation;
            }
            if romanization.is_empty() {
                romanization = language_romanization;
            }
        }
        let mut document = LyricsDocument {
            metadata: LyricsMetadata {
                title,
                artist,
                album,
                source,
                original_format: format.into(),
                manual_selected,
            },
            tracks: LyricsTracks {
                original: LyricsTrack { lines },
                translation: (!translation.is_empty())
                    .then_some(LyricsTrack { lines: translation }),
                romanization: (!romanization.is_empty()).then_some(LyricsTrack {
                    lines: romanization,
                }),
            },
            offset_ms: embedded_offset,
            raw: raw.to_string(),
        };
        normalize::normalize_document(&mut document);
        return Ok(document);
    }
    let mut document = basic_lrc::parse_basic_lrc(raw, source, manual_selected)?;
    normalize::normalize_document(&mut document);
    Ok(document)
}
