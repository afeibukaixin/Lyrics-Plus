use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use zhhz::{detect_text, Config, Converter, Region};

use crate::config::ChineseConversion;

/// Detect the script/region of a text block using the signatures bundled with
/// zhhz. Japanese detection takes precedence when kana is present.
pub(crate) fn detect_region(text: &str) -> Option<Region> {
    detect_text(text).map(|detection| detection.region)
}

pub(crate) fn is_japanese(text: &str) -> bool {
    matches!(detect_region(text), Some(Region::JpN | Region::JpT))
}

pub(crate) fn output_config_for_region(
    region: Region,
    conversion: ChineseConversion,
) -> Option<Config> {
    match (conversion, region) {
        (ChineseConversion::Original, _)
        | (ChineseConversion::Simplified, Region::CnS)
        | (ChineseConversion::Simplified, Region::JpN | Region::JpT)
        | (ChineseConversion::Traditional, Region::CnT | Region::CnTw | Region::CnHk)
        | (ChineseConversion::Traditional, Region::JpN | Region::JpT) => None,
        (ChineseConversion::Simplified, Region::CnT) => Some(Config::Tw2sp),
        (ChineseConversion::Simplified, Region::CnTw) => Some(Config::Tw2sp),
        (ChineseConversion::Simplified, Region::CnHk) => Some(Config::Hk2sp),
        (ChineseConversion::Traditional, Region::CnS) => Some(Config::S2t),
    }
}

/// Convert a text fragment while reusing the expensive zhhz dictionaries.
pub(crate) fn convert_text(text: &str, config: Config) -> String {
    let Some(converter) = converter_for(config) else {
        return text.to_owned();
    };
    converter
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .convert(text)
}

/// Repair only unambiguous Simplified Chinese characters that can represent
/// Japanese kanji.  The map combines the Simplified-to-Traditional character
/// table with zhhz's `T2jp` converter (backed by its embedded
/// `JPShinjitaiCharacters` table), so it stays aligned with the converter
/// version used by the application.
pub(crate) fn repair_simplified_japanese(text: &str) -> String {
    let mappings = japanese_repair_mappings();
    text.chars()
        .map(|character| mappings.get(&character).copied().unwrap_or(character))
        .collect()
}

fn japanese_repair_mappings() -> &'static HashMap<char, char> {
    static MAPPINGS: OnceLock<HashMap<char, char>> = OnceLock::new();
    MAPPINGS.get_or_init(|| {
        let Some(dictionary) = zhhz::data::dict_text("STCharacters") else {
            return HashMap::new();
        };
        let japanese_converter = converter_for(Config::T2jp)
            .expect("Japanese converter must be available")
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut candidates: HashMap<char, HashSet<char>> = HashMap::new();

        for line in dictionary.lines() {
            let line = line.trim_end_matches('\r');
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((source, values)) = line.split_once('\t') else {
                continue;
            };
            let mut source_chars = source.chars();
            let Some(simplified) = source_chars.next() else {
                continue;
            };
            if source_chars.next().is_some() {
                continue;
            }

            let values = values.split_whitespace().collect::<Vec<_>>();
            let Some(traditional) = values.first().copied() else {
                continue;
            };
            // OpenCC's multiple candidates mean the source character cannot
            // be restored safely without Japanese word-level context.
            if values.iter().any(|value| *value != traditional) {
                continue;
            }

            let mut traditional_chars = traditional.chars();
            let Some(traditional) = traditional_chars.next() else {
                continue;
            };
            if traditional_chars.next().is_some() {
                continue;
            }

            let repaired = japanese_converter.convert(&traditional.to_string());
            let mut repaired_chars = repaired.chars();
            let Some(japanese) = repaired_chars.next() else {
                continue;
            };
            if repaired_chars.next().is_some() || japanese == simplified {
                continue;
            }
            candidates.entry(simplified).or_default().insert(japanese);
        }

        candidates
            .into_iter()
            .filter_map(|(simplified, candidates)| {
                let mut candidates = candidates.into_iter();
                let japanese = candidates.next()?;
                candidates
                    .next()
                    .is_none()
                    .then_some((simplified, japanese))
            })
            .collect()
    })
}

fn converter_for(config: Config) -> Option<&'static Mutex<Converter>> {
    static SIMPLIFIED_CONVERTER: OnceLock<Mutex<Converter>> = OnceLock::new();
    static TAIWAN_SIMPLIFIED_CONVERTER: OnceLock<Mutex<Converter>> = OnceLock::new();
    static HONG_KONG_SIMPLIFIED_CONVERTER: OnceLock<Mutex<Converter>> = OnceLock::new();
    static TRADITIONAL_CONVERTER: OnceLock<Mutex<Converter>> = OnceLock::new();
    static JAPANESE_CONVERTER: OnceLock<Mutex<Converter>> = OnceLock::new();

    match config {
        Config::T2s => {
            Some(SIMPLIFIED_CONVERTER.get_or_init(|| Mutex::new(Converter::new(Config::T2s))))
        }
        Config::Tw2sp => Some(
            TAIWAN_SIMPLIFIED_CONVERTER.get_or_init(|| Mutex::new(Converter::new(Config::Tw2sp))),
        ),
        Config::Hk2sp => Some(
            HONG_KONG_SIMPLIFIED_CONVERTER
                .get_or_init(|| Mutex::new(Converter::new(Config::Hk2sp))),
        ),
        Config::S2t => {
            Some(TRADITIONAL_CONVERTER.get_or_init(|| Mutex::new(Converter::new(Config::S2t))))
        }
        Config::T2jp => {
            Some(JAPANESE_CONVERTER.get_or_init(|| Mutex::new(Converter::new(Config::T2jp))))
        }
        _ => None,
    }
}
