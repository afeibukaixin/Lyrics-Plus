use serde::{Deserialize, Serialize};

pub mod amll_ttml;
pub(crate) mod conversion;
pub mod credentials;
pub(crate) mod encoding;
pub mod kugou;
pub mod kuwo;
pub mod lrclib;
pub mod migu;
pub mod musixmatch;
pub mod netease;
pub mod provider;
pub mod qqmusic;

include!("types.rs");
mod parser;
#[cfg(test)]
use parser::parse_lrc;
pub use parser::parse_lrc_with_options;
use parser::{decode_xml_text, parse_integer_list};
pub(crate) use parser::{lyrics_quality_report, semantic_fingerprint, LyricsQualityReport};
pub(crate) mod runtime;
pub(crate) use runtime::LyricsSearchSession;
pub use runtime::{
    LyricsLoadResponse, LyricsLoadStatus, LyricsMonitor, LyricsRuntimeSnapshot, LyricsSearchIntent,
    SaveLyricsInput, SearchResponse,
};

#[cfg(test)]
include!("tests.rs");
