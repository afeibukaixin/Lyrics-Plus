use crate::config::ChineseConversion;
use crate::lyrics::conversion::{
    convert_text, detect_region, output_config_for_region, repair_simplified_japanese,
};
use zhhz::Region;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsWord {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsLine {
    pub start_ms: u64,
    pub end_ms: Option<u64>,
    pub text: String,
    pub words: Option<Vec<LyricsWord>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsTrack {
    pub lines: Vec<LyricsLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsTracks {
    pub original: LyricsTrack,
    pub translation: Option<LyricsTrack>,
    pub romanization: Option<LyricsTrack>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub source: String,
    pub original_format: String,
    pub manual_selected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsDocument {
    pub metadata: LyricsMetadata,
    pub tracks: LyricsTracks,
    pub offset_ms: i64,
    pub raw: String,
}

impl LyricsDocument {
    /// 根据输出配置生成展示副本；持久化的原始歌词和时间轴不会被修改。
    pub(crate) fn converted_for_output(
        &self,
        conversion: ChineseConversion,
        repair_japanese: bool,
    ) -> Self {
        if matches!(conversion, ChineseConversion::Original) && !repair_japanese {
            return self.clone();
        }
        let mut converted = self.clone();
        convert_track(
            &mut converted.tracks.original,
            conversion,
            repair_japanese,
        );
        if let Some(translation) = converted.tracks.translation.as_mut() {
            convert_track(translation, conversion, repair_japanese);
        }
        converted
    }
}

impl LyricsLine {
    pub(crate) fn converted_for_output_with_region(
        &self,
        conversion: ChineseConversion,
        source_region: Option<Region>,
        repair_japanese: bool,
    ) -> Self {
        if matches!(conversion, ChineseConversion::Original) && !repair_japanese {
            return self.clone();
        }
        let mut converted = self.clone();
        convert_line(&mut converted, conversion, source_region, repair_japanese);
        converted
    }
}

fn track_text(track: &LyricsTrack) -> String {
    track
        .lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn convert_line(
    line: &mut LyricsLine,
    conversion: ChineseConversion,
    source_region: Option<Region>,
    repair_japanese: bool,
) {
    // A Japanese track-level result protects kanji-only lines; Chinese lines
    // still use their own detected region so mixed-region lyrics convert well.
    let source_region = if matches!(source_region, Some(Region::JpN | Region::JpT)) {
        source_region
    } else {
        detect_region(&line.text).or(source_region)
    };
    if repair_japanese && matches!(source_region, Some(Region::JpN | Region::JpT)) {
        repair_line(line);
    }
    let Some(config) =
        source_region.and_then(|region| output_config_for_region(region, conversion))
    else {
        return;
    };

    if let Some(words) = line.words.as_mut().filter(|words| !words.is_empty()) {
        let source_words = words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<String>();
        let converted_words = convert_text(&source_words, config);
        if source_words.chars().count() == converted_words.chars().count() {
            let mut converted_chars = converted_words.chars();
            for word in words.iter_mut() {
                let char_count = word.text.chars().count();
                let converted = converted_chars
                    .by_ref()
                    .take(char_count)
                    .collect::<String>();
                word.text = converted;
            }
        } else {
            for word in words.iter_mut() {
                let converted = convert_text(&word.text, config);
                word.text = converted;
            }
        }
        line.text = words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<String>();
    } else {
        line.text = convert_text(&line.text, config);
    }
}

fn repair_line(line: &mut LyricsLine) {
    if let Some(words) = line.words.as_mut().filter(|words| !words.is_empty()) {
        for word in words.iter_mut() {
            word.text = repair_simplified_japanese(&word.text);
        }
        let text = words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<String>();
        line.text = text;
    } else {
        line.text = repair_simplified_japanese(&line.text);
    }
}

fn convert_track(
    track: &mut LyricsTrack,
    conversion: ChineseConversion,
    repair_japanese: bool,
) {
    let Some(source_region) = detect_region(&track_text(track)) else {
        return;
    };
    for line in &mut track.lines {
        convert_line(line, conversion, Some(source_region), repair_japanese);
    }
}
