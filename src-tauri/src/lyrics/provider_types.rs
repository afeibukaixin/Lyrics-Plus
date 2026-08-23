pub type ProviderFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, ProviderError>> + Send + 'a>>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderHealth {
    Unknown,
    Available,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub provider_id: String,
    pub name: String,
    pub health: ProviderHealth,
    pub message: Option<String>,
    pub checked_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorKind {
    Network,
    Http,
    InvalidResponse,
    Configuration,
    Unauthorized,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderError {
    pub provider_id: String,
    pub kind: ProviderErrorKind,
    pub message: String,
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}：{}", self.provider_id, self.message)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(default, rename_all = "camelCase")]
pub struct MatchWeights {
    pub title: u8,
    pub artist: u8,
    pub album: u8,
    pub duration: u8,
}

impl MatchWeights {
    fn total(self) -> u16 {
        u16::from(self.title)
            + u16::from(self.artist)
            + u16::from(self.album)
            + u16::from(self.duration)
    }
}

impl Default for MatchWeights {
    fn default() -> Self {
        Self {
            title: 39,
            artist: 36,
            album: 8,
            duration: 17,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ScoringSettings {
    title_filter_keywords: Vec<String>,
    match_weights: MatchWeights,
    normalize_chinese: bool,
}

impl Default for ScoringSettings {
    fn default() -> Self {
        Self {
            title_filter_keywords: Vec::new(),
            match_weights: MatchWeights::default(),
            normalize_chinese: default_normalize_chinese(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSearchInput {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    #[serde(skip)]
    pub(crate) scoring: Arc<ScoringSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSearchResult {
    pub id: String,
    pub provider_id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub source: String,
    pub synced: bool,
    pub has_translation: bool,
    pub has_word_timing: bool,
    pub has_romanization: bool,
    pub score: f64,
    pub lyrics: String,
}

#[derive(Debug)]
pub struct ProviderSearchReport {
    pub results: Vec<LyricsSearchResult>,
    pub warning: Option<String>,
}

impl ProviderSearchReport {
    pub fn available(results: Vec<LyricsSearchResult>) -> Self {
        Self {
            results,
            warning: None,
        }
    }
}

pub(crate) fn collect_provider_results(
    outcomes: impl IntoIterator<Item = Result<Option<LyricsSearchResult>, ProviderError>>,
) -> Result<ProviderSearchReport, ProviderError> {
    let mut results = Vec::new();
    let mut first_error = None;
    let mut any_success = false;
    for outcome in outcomes {
        match outcome {
            Ok(result) => {
                any_success = true;
                results.extend(result);
            }
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    if !any_success {
        if let Some(error) = first_error {
            return Err(error);
        }
    }
    Ok(ProviderSearchReport {
        results,
        warning: first_error.map(|error| error.message),
    })
}
