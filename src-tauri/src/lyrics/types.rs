use std::sync::{Mutex, OnceLock};

use zhhz::{Config, Converter};

use crate::config::ChineseConversion;

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
    pub(crate) fn converted_for_output(&self, conversion: ChineseConversion) -> Self {
        let converter_config = match conversion {
            ChineseConversion::Original => return self.clone(),
            ChineseConversion::Simplified => Config::T2s,
            ChineseConversion::Traditional => Config::S2t,
        };
        let mut converted = self.clone();
        convert_track(&mut converted.tracks.original, converter_config);
        if let Some(translation) = converted.tracks.translation.as_mut() {
            convert_track(translation, converter_config);
        }
        converted
    }
}

impl LyricsLine {
    /// 仅转换一行展示文本，适用于高频刷新且不需要复制整份文档的展示边界。
    pub(crate) fn converted_for_output(&self, conversion: ChineseConversion) -> Self {
        let config = match conversion {
            ChineseConversion::Original => return self.clone(),
            ChineseConversion::Simplified => Config::T2s,
            ChineseConversion::Traditional => Config::S2t,
        };
        let mut converted = self.clone();
        convert_line(&mut converted, config);
        converted
    }
}

fn converter_for(config: Config) -> Option<&'static Mutex<Converter>> {
    static SIMPLIFIED_CONVERTER: OnceLock<Mutex<Converter>> = OnceLock::new();
    static TRADITIONAL_CONVERTER: OnceLock<Mutex<Converter>> = OnceLock::new();
    match config {
        Config::T2s => {
            Some(SIMPLIFIED_CONVERTER.get_or_init(|| Mutex::new(Converter::new(Config::T2s))))
        }
        Config::S2t => {
            Some(TRADITIONAL_CONVERTER.get_or_init(|| Mutex::new(Converter::new(Config::S2t))))
        }
        _ => None,
    }
}

fn convert_line(line: &mut LyricsLine, config: Config) {
    let Some(converter) = converter_for(config) else {
        return;
    };
    let converter = converter.lock().unwrap_or_else(|error| error.into_inner());
    line.text = converter.convert(&line.text);
    if let Some(words) = line.words.as_mut() {
        for word in words {
            word.text = converter.convert(&word.text);
        }
    }
}

fn convert_track(track: &mut LyricsTrack, config: Config) {
    for line in &mut track.lines {
        convert_line(line, config);
    }
}
