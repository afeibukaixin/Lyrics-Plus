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
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderStatusDetail {
    NotTested,
    NotParticipated,
    Success {
        #[serde(rename = "resultCount")]
        result_count: usize,
    },
    PartialFailure {
        #[serde(rename = "resultCount")]
        result_count: usize,
        #[serde(rename = "errorKind")]
        error_kind: ProviderErrorKind,
    },
    Failure {
        #[serde(rename = "errorKind")]
        error_kind: ProviderErrorKind,
        #[serde(rename = "statusCode")]
        status_code: Option<u16>,
    },
    Timeout,
    Cooldown {
        #[serde(rename = "retryAfterMs")]
        retry_after_ms: Option<u64>,
        #[serde(rename = "requiresConfiguration")]
        requires_configuration: bool,
    },
}

impl ProviderStatusDetail {
    pub(crate) fn result_count(&self) -> usize {
        match self {
            Self::Success { result_count } | Self::PartialFailure { result_count, .. } => {
                *result_count
            }
            _ => 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub provider_id: String,
    pub name: String,
    pub health: ProviderHealth,
    pub detail: ProviderStatusDetail,
    pub checked_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderTier {
    Stable,
    ExactId,
    Advanced,
    Experimental,
    Dormant,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAutoBindingPolicy {
    Allowed,
    ExactIdOnly,
    UserOnly,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderPersistencePolicy {
    DownloadLibrary,
    CacheOnly,
    MemoryOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub metadata_search: bool,
    pub id_lookup: bool,
    pub plain_text: bool,
    pub line_timing: bool,
    pub word_timing: bool,
    pub translation: bool,
    pub romanization: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderManifest {
    pub id: String,
    pub display_name: String,
    pub implementation_key: String,
    pub tier: ProviderTier,
    pub enabled_by_default: bool,
    pub auto_binding: ProviderAutoBindingPolicy,
    pub capabilities: ProviderCapabilities,
    pub requires_credentials: bool,
    pub persistence: ProviderPersistencePolicy,
    pub request_timeout_ms: u64,
    pub notice_key: Option<String>,
}

/// 对外公开的 Provider 描述，同时携带当前用户策略和最近状态。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDescriptor {
    pub manifest: ProviderManifest,
    pub enabled: bool,
    pub status: Option<ProviderStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCandidate {
    pub provider_id: String,
    pub provider_item_id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub version_tags: Vec<String>,
    pub capabilities: ProviderCapabilities,
    pub source: String,
    pub lookup_key: Option<String>,
    #[serde(skip)]
    pub(crate) legacy_result: Option<LyricsSearchResult>,
}

#[derive(Debug)]
pub struct ProviderCandidateReport {
    pub candidates: Vec<ProviderCandidate>,
    pub warning: Option<ProviderError>,
}

impl ProviderCandidate {
    pub(crate) fn from_result(result: LyricsSearchResult) -> Self {
        let artists = result
            .artist
            .split(" / ")
            .map(str::trim)
            .filter(|artist| !artist.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        Self {
            provider_id: result.provider_id.clone(),
            provider_item_id: result.id.clone(),
            title: result.title.clone(),
            artists,
            album: result.album.clone(),
            duration_ms: result.duration_ms,
            version_tags: version_tags_from_title(&result.title),
            capabilities: ProviderCapabilities {
                metadata_search: true,
                id_lookup: true,
                plain_text: true,
                line_timing: result.synced,
                word_timing: result.has_word_timing,
                translation: result.has_translation,
                romanization: result.has_romanization,
            },
            source: result.source.clone(),
            lookup_key: None,
            legacy_result: Some(result),
        }
    }

    pub(crate) fn metadata_result(&self) -> LyricsSearchResult {
        LyricsSearchResult {
            id: self.provider_item_id.clone(),
            provider_id: self.provider_id.clone(),
            title: self.title.clone(),
            artist: self.artists.join(" / "),
            album: self.album.clone(),
            duration_ms: self.duration_ms,
            source: self.source.clone(),
            synced: self.capabilities.line_timing,
            has_translation: self.capabilities.translation,
            has_word_timing: self.capabilities.word_timing,
            has_romanization: self.capabilities.romanization,
            score: 0.0,
            lyrics: String::new(),
        }
    }
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
    #[serde(default, skip_serializing)]
    pub status_code: Option<u16>,
    #[serde(default, skip_serializing)]
    pub retry_after_ms: Option<u64>,
}

impl ProviderError {
    pub(crate) fn new(
        provider_id: impl Into<String>,
        kind: ProviderErrorKind,
        message: impl Into<String>,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            kind,
            message: message.into(),
            status_code: None,
            retry_after_ms: None,
        }
    }

    pub(crate) fn with_http(
        provider_id: impl Into<String>,
        status_code: u16,
        retry_after_ms: Option<u64>,
        message: impl Into<String>,
    ) -> Self {
        let kind = if matches!(status_code, 401 | 402 | 403) {
            ProviderErrorKind::Unauthorized
        } else {
            ProviderErrorKind::Http
        };
        Self {
            provider_id: provider_id.into(),
            kind,
            message: message.into(),
            status_code: Some(status_code),
            retry_after_ms,
        }
    }
}

pub(crate) fn response_error(
    provider_id: &str,
    response: &reqwest::Response,
    context: impl Into<String>,
) -> ProviderError {
    let status_code = response.status().as_u16();
    let retry_after_ms = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_retry_after_ms);
    ProviderError::with_http(
        provider_id,
        status_code,
        retry_after_ms,
        format!("{}：HTTP {}", context.into(), response.status()),
    )
}

fn parse_retry_after_ms(value: &str) -> Option<u64> {
    let value = value.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(seconds.saturating_mul(1_000));
    }
    let timestamp = parse_http_date(value)?;
    let now = SystemTime::now();
    Some(
        timestamp
            .duration_since(now)
            .unwrap_or_default()
            .as_millis() as u64,
    )
}

/// 解析 HTTP-date 的三种标准写法，无需引入额外运行时依赖。
fn parse_http_date(value: &str) -> Option<SystemTime> {
    let fields = value
        .split(|character: char| character == ',' || character == ' ' || character == '-')
        .filter(|field| !field.is_empty())
        .collect::<Vec<_>>();
    let (day, month, year, time): (u32, u32, u32, &str) =
        if fields.len() >= 5 && fields[1].parse::<u32>().is_ok() && fields[3].len() == 4 {
            // RFC 1123: Sun, 06 Nov 1994 08:49:37 GMT
            (
                fields[1].parse().ok()?,
                month_number(fields[2])?,
                fields[3].parse().ok()?,
                fields[4],
            )
        } else if fields.len() >= 6 && fields[1].parse::<u32>().is_ok() && fields[3].len() <= 2 {
            // RFC 850: Sunday, 06-Nov-94 08:49:37 GMT
            let short_year: u32 = fields[3].parse().ok()?;
            let year = if short_year >= 70 {
                1_900 + short_year
            } else {
                2_000 + short_year
            };
            (
                fields[1].parse().ok()?,
                month_number(fields[2])?,
                year,
                fields[4],
            )
        } else {
            // asctime: Sun Nov  6 08:49:37 1994
            let fields = value.split_whitespace().collect::<Vec<_>>();
            if fields.len() != 5 {
                return None;
            }
            (
                fields[2].parse().ok()?,
                month_number(fields[1])?,
                fields[4].parse().ok()?,
                fields[3],
            )
        };
    let mut time_parts = time.split(':');
    let hour: u64 = time_parts.next()?.parse().ok()?;
    let minute: u64 = time_parts.next()?.parse().ok()?;
    let second: u64 = time_parts.next()?.parse().ok()?;
    if time_parts.next().is_some() || hour >= 24 || minute >= 60 || second >= 60 {
        return None;
    }
    let days = days_from_civil(year as i64, month as i64, day as i64)?;
    let seconds = days
        .checked_mul(86_400)?
        .checked_add((hour * 3_600 + minute * 60 + second) as i64)?;
    if seconds < 0 {
        return None;
    }
    UNIX_EPOCH.checked_add(Duration::from_secs(seconds as u64))
}

fn month_number(value: &str) -> Option<u32> {
    [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .iter()
    .position(|month| month.eq_ignore_ascii_case(value))
    .map(|index| index as u32 + 1)
}

fn days_from_civil(year: i64, month: i64, day: i64) -> Option<i64> {
    if !(1..=12).contains(&month) || day == 0 || day > 31 {
        return None;
    }
    let year = year - if month <= 2 { 1 } else { 0 };
    let era = (if year >= 0 { year } else { year - 399 }) / 400;
    let year_of_era = year - era * 400;
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(era * 146_097 + day_of_era - 719_468)
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
    pub version: u8,
}

impl MatchWeights {
    fn total(self) -> u16 {
        u16::from(self.title)
            + u16::from(self.artist)
            + u16::from(self.album)
            + u16::from(self.duration)
            + u16::from(self.version)
    }
}

impl Default for MatchWeights {
    fn default() -> Self {
        Self {
            title: 64,
            artist: 16,
            album: 5,
            duration: 10,
            version: 5,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ScoringSettings {
    title_filter_keywords: Vec<String>,
    match_weights: MatchWeights,
    normalize_chinese: bool,
    confirmed_artist_aliases: Vec<(String, String)>,
}

impl Default for ScoringSettings {
    fn default() -> Self {
        Self {
            title_filter_keywords: Vec::new(),
            match_weights: MatchWeights::default(),
            normalize_chinese: default_normalize_chinese(),
            confirmed_artist_aliases: Vec::new(),
        }
    }
}

impl ScoringSettings {
    pub(crate) fn with_confirmed_artist_aliases(mut self, aliases: Vec<(String, String)>) -> Self {
        self.confirmed_artist_aliases = aliases;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSearchInput {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    /// 播放器平台上下文；用于处理平台返回的署名字段不完整问题。
    #[serde(default)]
    pub platform: Option<String>,
    /// 当前播放器曲目的平台内 ID；为空时只进行普通元数据搜索。
    #[serde(default)]
    pub platform_item_id: Option<String>,
    #[serde(skip)]
    pub(crate) scoring: Arc<ScoringSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSearchResult {
    /// 来源内稳定标识，必须与 provider_id 组合使用，不能跨歌词源比较。
    pub id: String,
    /// 产生该 ID 的歌词源标识。
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
    pub warning: Option<ProviderError>,
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
        warning: first_error,
    })
}
