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
