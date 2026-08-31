#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderOrderMode {
    #[default]
    Smart,
    Strict,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPreference {
    pub id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(default, rename_all = "camelCase")]
pub struct ProviderSettings {
    pub mode: ProviderOrderMode,
    pub providers: Vec<ProviderPreference>,
    #[serde(default = "default_auto_apply_threshold")]
    pub auto_apply_threshold: u8,
    #[serde(default = "default_prefer_capabilities")]
    pub prefer_capabilities: bool,
    #[serde(default)]
    pub match_weights: MatchWeights,
    #[serde(default = "default_normalize_chinese")]
    pub normalize_chinese: bool,
    #[serde(default = "default_title_filter_keywords")]
    pub title_filter_keywords: Vec<String>,
    #[serde(default = "default_amll_base_url")]
    pub amll_base_url: String,
}

const MAX_TITLE_FILTER_KEYWORDS: usize = 32;
const MAX_TITLE_FILTER_KEYWORD_LENGTH: usize = 64;

const fn default_auto_apply_threshold() -> u8 {
    60
}

const fn default_normalize_chinese() -> bool {
    true
}

const fn default_prefer_capabilities() -> bool {
    true
}

fn default_title_filter_keywords() -> Vec<String> {
    [
        "feat",
        "ft",
        "featuring",
        "主题曲",
        "片头曲",
        "片尾曲",
        "插曲",
        "电影",
        "电视剧",
        "动画",
        "游戏",
        "ost",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn default_amll_base_url() -> String {
    DEFAULT_AMLL_BASE_URL.into()
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            mode: ProviderOrderMode::Smart,
            providers: provider_definitions()
                .into_iter()
                .map(|(id, _)| ProviderPreference {
                    id: id.into(),
                    enabled: default_provider_enabled(id),
                })
                .collect(),
            auto_apply_threshold: default_auto_apply_threshold(),
            prefer_capabilities: default_prefer_capabilities(),
            match_weights: MatchWeights::default(),
            normalize_chinese: default_normalize_chinese(),
            title_filter_keywords: default_title_filter_keywords(),
            amll_base_url: default_amll_base_url(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettingsView {
    pub settings: ProviderSettings,
    pub statuses: Vec<ProviderStatus>,
}

#[derive(Clone)]
pub struct ProviderSearchOutcome {
    pub results: Vec<LyricsSearchResult>,
    pub statuses: Vec<ProviderStatus>,
    pub auto_apply_threshold: u8,
    pub prefer_capabilities: bool,
    pub error: Option<String>,
}

type SearchFlight = tokio::sync::OnceCell<Result<ProviderSearchOutcome, String>>;

#[derive(Clone, PartialEq, Eq, Hash)]
struct SearchKey {
    title: String,
    artist: String,
    album: Option<String>,
    duration_ms: Option<u64>,
    settings: ProviderSettings,
    revision: u64,
}

impl SearchKey {
    fn new(input: &LyricsSearchInput, settings: ProviderSettings, revision: u64) -> Self {
        Self {
            title: input.title.trim().into(),
            artist: input.artist.trim().into(),
            album: input.album.as_deref().map(str::trim).map(str::to_owned),
            duration_ms: input.duration_ms,
            settings,
            revision,
        }
    }
}

pub trait LyricsProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn search<'a>(
        &'a self,
        client: &'a reqwest::Client,
        input: &'a LyricsSearchInput,
    ) -> ProviderFuture<'a, ProviderSearchReport>;
}
