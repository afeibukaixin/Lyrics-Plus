use std::path::{Path, PathBuf};

use crate::lyrics::parse_lrc_with_options;

use super::discovery::{lyric_filename_stem, strip_lyricsfile_suffix};

pub(super) fn collision_filename_parts(path: &Path) -> Option<(PathBuf, String, String)> {
    let file_name = path.file_name()?.to_str()?;
    let lyricsfile_stem = strip_lyricsfile_suffix(file_name);
    let is_lyricsfile = lyricsfile_stem.is_some();
    let stem = lyricsfile_stem.or_else(|| path.file_stem()?.to_str())?;
    let suffix_start = stem.rfind(" (")?;
    let suffix = stem.get(suffix_start + 2..stem.len().checked_sub(1)?)?;
    if suffix.is_empty() || !suffix.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    let base_stem = stem.get(..suffix_start)?.trim_end();
    let (artist, title) = base_stem.split_once(" - ")?;
    let extension = if is_lyricsfile {
        ".lyricsfile.yaml"
    } else {
        ".lrc"
    };
    Some((
        path.with_file_name(format!("{base_stem}{extension}")),
        artist.to_owned(),
        title.to_owned(),
    ))
}

pub(super) struct ParsedMetadata {
    pub(super) title: String,
    pub(super) artist: String,
    pub(super) source: String,
    pub(super) format: String,
    pub(super) duration_ms: Option<u64>,
    pub(super) has_translation: bool,
    pub(super) has_word_timing: bool,
    pub(super) has_romanization: bool,
}

pub(super) fn lyric_metadata(path: &Path, raw: &str, source: &str) -> ParsedMetadata {
    let extension_format = if path
        .file_name()
        .and_then(|value| value.to_str())
        .and_then(strip_lyricsfile_suffix)
        .is_some()
    {
        "lyricsfile".into()
    } else {
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("lrc")
            .to_ascii_lowercase()
    };
    let document = parse_lrc_with_options(raw, source, false).ok();
    let format = document
        .as_ref()
        .map(|value| value.metadata.original_format.clone())
        .unwrap_or(extension_format);
    let (filename_artist, filename_title) = lyric_filename_stem(path)
        .and_then(|value| value.split_once(" - "))
        .map(|(artist, title)| (Some(artist.to_string()), Some(title.to_string())))
        .unwrap_or((None, None));
    let title = document
        .as_ref()
        .and_then(|value| value.metadata.title.clone())
        .or_else(|| lrc_tag(raw, "ti"))
        .or(filename_title)
        .unwrap_or_else(|| "未知歌曲".into());
    let artist = document
        .as_ref()
        .and_then(|value| value.metadata.artist.clone())
        .or_else(|| lrc_tag(raw, "ar"))
        .or(filename_artist)
        .unwrap_or_else(|| "未知歌手".into());
    let duration_ms = document.as_ref().and_then(|value| {
        value
            .tracks
            .original
            .lines
            .last()
            .map(|line| line.end_ms.unwrap_or(line.start_ms))
    });
    let has_translation = document
        .as_ref()
        .is_some_and(|value| value.tracks.translation.is_some());
    let has_word_timing = document.as_ref().is_some_and(|value| {
        value
            .tracks
            .original
            .lines
            .iter()
            .any(|line| line.words.as_ref().is_some_and(|words| !words.is_empty()))
    });
    let has_romanization = document
        .as_ref()
        .is_some_and(|value| value.tracks.romanization.is_some());
    ParsedMetadata {
        title,
        artist,
        source: source.into(),
        format,
        duration_ms,
        has_translation,
        has_word_timing,
        has_romanization,
    }
}

pub(super) fn lrc_tag(raw: &str, key: &str) -> Option<String> {
    let prefix = format!("[{key}:");
    raw.lines().find_map(|line| {
        line.strip_prefix(&prefix)
            .and_then(|value| value.strip_suffix(']'))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}
