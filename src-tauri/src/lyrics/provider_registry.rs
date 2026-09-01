pub struct ProviderRegistry {
    providers: Vec<Box<dyn LyricsProvider>>,
    settings: Arc<RwLock<ProviderSettings>>,
    credentials: Arc<ProviderCredentialStore>,
    statuses: RwLock<HashMap<String, ProviderStatus>>,
    in_flight: Mutex<HashMap<SearchKey, Weak<SearchFlight>>>,
    cache: Mutex<HashMap<SearchKey, CachedSearch>>,
    cooldowns: Mutex<HashMap<String, ProviderCooldown>>,
    revision: AtomicU64,
    timeout: Duration,
}

const SEARCH_CACHE_TTL: Duration = Duration::from_secs(120);
const SEARCH_CACHE_CAPACITY: usize = 128;

#[derive(Clone)]
struct CachedSearch {
    outcome: ProviderSearchOutcome,
    expires_at: Instant,
}

struct ProviderCooldown {
    until: Instant,
    consecutive_failures: u8,
    sticky: bool,
    revision: u64,
}

fn with_scoring_settings(
    input: &LyricsSearchInput,
    settings: &ProviderSettings,
) -> Result<LyricsSearchInput, String> {
    let mut scoring_input = input.clone();
    scoring_input.scoring = Arc::new(ScoringSettings {
        title_filter_keywords: prepare_title_filter_keywords_with_normalization(
            &settings.title_filter_keywords,
            settings.normalize_chinese,
        )?,
        match_weights: settings.match_weights,
        normalize_chinese: settings.normalize_chinese,
    });
    Ok(scoring_input)
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new(ProviderSettings::default())
    }
}

impl ProviderRegistry {
    pub fn new(settings: ProviderSettings) -> Self {
        Self::build(settings, Arc::new(ProviderCredentialStore::memory()))
    }

    pub fn new_with_app_dir(settings: ProviderSettings, app_dir: &Path) -> Result<Self, String> {
        let credentials = Arc::new(ProviderCredentialStore::load(app_dir)?);
        let legacy_cache = app_dir.join("cache").join("amll-index.json");
        if legacy_cache.is_file() {
            if let Err(error) = std::fs::remove_file(&legacy_cache) {
                log::debug!("清理旧 AMLL 索引缓存失败：{error}");
            }
        }
        Ok(Self::build(settings, credentials))
    }

    fn build(settings: ProviderSettings, credentials: Arc<ProviderCredentialStore>) -> Self {
        let settings = Arc::new(RwLock::new(settings));
        let providers: Vec<Box<dyn LyricsProvider>> = vec![
            Box::new(NeteaseProvider),
            Box::new(QqMusicProvider),
            Box::new(KugouProvider),
            Box::new(LrcLibProvider::default()),
            Box::new(KuwoProvider),
            Box::new(AmllTtmlProvider::new(settings.clone())),
            Box::new(MiguProvider),
            Box::new(MusixmatchProvider::new(credentials.clone())),
        ];
        let statuses = providers
            .iter()
            .map(|provider| {
                (
                    provider.id().into(),
                    ProviderStatus {
                        provider_id: provider.id().into(),
                        name: provider.display_name().into(),
                        health: ProviderHealth::Unknown,
                        message: Some("尚未测试".into()),
                        checked_at_ms: None,
                    },
                )
            })
            .collect();
        Self {
            providers,
            settings,
            credentials,
            statuses: RwLock::new(statuses),
            in_flight: Mutex::new(HashMap::new()),
            cache: Mutex::new(HashMap::new()),
            cooldowns: Mutex::new(HashMap::new()),
            revision: AtomicU64::new(0),
            timeout: Duration::from_secs(8),
        }
    }

    pub fn settings_view(&self) -> ProviderSettingsView {
        ProviderSettingsView {
            settings: self
                .settings
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .clone(),
            statuses: self.statuses(),
        }
    }

    pub fn set_settings(
        &self,
        mut settings: ProviderSettings,
    ) -> Result<ProviderSettingsView, String> {
        normalize_settings(&mut settings)?;
        *self
            .settings
            .write()
            .unwrap_or_else(|error| error.into_inner()) = settings;
        self.revision.fetch_add(1, Ordering::SeqCst);
        Ok(self.settings_view())
    }

    pub fn credential_view(&self) -> ProviderCredentialView {
        self.credentials.view()
    }

    pub fn set_musixmatch_token(
        &self,
        token_type: MusixmatchTokenType,
        token: String,
    ) -> Result<(ProviderCredentialView, ProviderSettingsView), String> {
        let credentials = self.credentials.set_musixmatch_token(token_type, token)?;
        let mut settings = self
            .settings
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(provider) = settings
            .providers
            .iter_mut()
            .find(|provider| provider.id == "musixmatch")
        {
            provider.enabled = true;
        }
        drop(settings);
        self.revision.fetch_add(1, Ordering::SeqCst);
        Ok((credentials, self.settings_view()))
    }

    pub fn clear_musixmatch_token(
        &self,
    ) -> Result<(ProviderCredentialView, ProviderSettingsView), String> {
        let credentials = self.credentials.clear_musixmatch_token()?;
        self.revision.fetch_add(1, Ordering::SeqCst);
        Ok((credentials, self.settings_view()))
    }

    #[cfg(test)]
    pub async fn search(
        &self,
        client: &reqwest::Client,
        input: &LyricsSearchInput,
    ) -> Result<ProviderSearchOutcome, String> {
        let mut outcome = self.search_with_cache(client, input, false).await?;
        // 保留 ProviderRegistry 直接调用方的旧批量边界；跨本地/在线的语义去重
        // 在命令层完成，避免这里提前丢失 Smart 模式所需的质量信息。
        deduplicate(&mut outcome.results);
        outcome.results.truncate(24);
        Ok(outcome)
    }

    pub async fn search_with_cache(
        &self,
        client: &reqwest::Client,
        input: &LyricsSearchInput,
        bypass_cache: bool,
    ) -> Result<ProviderSearchOutcome, String> {
        let settings = self
            .settings
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let key = SearchKey::new(
            input,
            settings.clone(),
            self.revision.load(Ordering::SeqCst),
        );
        if !bypass_cache {
            if let Some(outcome) = self.cached_search(&key) {
                log::debug!(
                    "歌词搜索命中缓存：title={} artist={}",
                    input.title,
                    input.artist
                );
                return Ok(outcome);
            }
        }
        let flight = {
            let mut in_flight = self
                .in_flight
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            in_flight.retain(|_, flight| flight.strong_count() > 0);
            if let Some(flight) = in_flight.get(&key).and_then(Weak::upgrade) {
                flight
            } else {
                let flight = Arc::new(SearchFlight::new());
                in_flight.insert(key.clone(), Arc::downgrade(&flight));
                flight
            }
        };
        let result = flight
            .get_or_init(|| self.search_once(client, input, settings))
            .await
            .clone();
        if let Ok(outcome) = &result {
            if outcome.error.is_none() {
                self.store_cached_search(key, outcome.clone());
            }
        }
        result
    }

    fn cached_search(&self, key: &SearchKey) -> Option<ProviderSearchOutcome> {
        let mut cache = self.cache.lock().unwrap_or_else(|error| error.into_inner());
        let (expires_at, mut outcome) = cache
            .get(key)
            .map(|entry| (entry.expires_at, entry.outcome.clone()))?;
        if expires_at <= Instant::now() {
            cache.remove(key);
            return None;
        }
        let provider_ids = outcome
            .statuses
            .iter()
            .map(|status| status.provider_id.clone())
            .collect::<Vec<_>>();
        outcome.statuses = self.statuses_for(&provider_ids);
        Some(outcome)
    }

    fn store_cached_search(&self, key: SearchKey, outcome: ProviderSearchOutcome) {
        let mut cache = self.cache.lock().unwrap_or_else(|error| error.into_inner());
        cache.retain(|_, entry| entry.expires_at > Instant::now());
        if cache.len() >= SEARCH_CACHE_CAPACITY && !cache.contains_key(&key) {
            if let Some(oldest_key) = cache
                .iter()
                .min_by_key(|(_, entry)| entry.expires_at)
                .map(|(key, _)| key.clone())
            {
                cache.remove(&oldest_key);
            }
        }
        cache.insert(
            key,
            CachedSearch {
                outcome,
                expires_at: Instant::now() + SEARCH_CACHE_TTL,
            },
        );
    }

    pub(crate) fn local_search_context(
        &self,
        input: &LyricsSearchInput,
    ) -> Result<(LyricsSearchInput, u8), String> {
        let settings = self
            .settings
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        Ok((
            with_scoring_settings(input, &settings)?,
            settings.auto_apply_threshold,
        ))
    }

    pub(crate) fn auto_search_debounce(&self) -> Duration {
        Duration::from_millis(
            self.settings
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .auto_search_debounce_ms,
        )
    }

    async fn search_once(
        &self,
        client: &reqwest::Client,
        input: &LyricsSearchInput,
        settings: ProviderSettings,
    ) -> Result<ProviderSearchOutcome, String> {
        let priority = settings
            .providers
            .iter()
            .enumerate()
            .map(|(index, preference)| (preference.id.as_str(), index))
            .collect::<HashMap<_, _>>();
        let enabled = settings
            .providers
            .iter()
            .filter(|preference| preference.enabled)
            .filter_map(|preference| {
                self.providers
                    .iter()
                    .find(|provider| provider.id() == preference.id)
                    .map(|provider| provider.as_ref())
            })
            .collect::<Vec<_>>();
        if enabled.is_empty() {
            return Err("请至少启用一个歌词源".into());
        }
        let enabled_ids = enabled
            .iter()
            .map(|provider| provider.id().to_owned())
            .collect::<Vec<_>>();

        let mut active = Vec::with_capacity(enabled.len());
        let mut errors = Vec::new();
        for provider in &enabled {
            if let Some(error) = self.cooldown_error(provider.id()) {
                log::debug!(
                    "歌词源处于冷却中，跳过请求：{}：{}",
                    provider.id(),
                    error.message
                );
                errors.push(error.to_string());
                self.record_cooldown_status(*provider, &error.message);
            } else {
                active.push(*provider);
            }
        }
        let scoring_input = with_scoring_settings(input, &settings)?;
        let scoring_input = &scoring_input;
        let jobs = active.iter().map(|provider| async move {
            let outcome =
                tokio::time::timeout(self.timeout, provider.search(client, scoring_input)).await;
            (*provider, outcome)
        });
        let outcomes = join_all(jobs).await;
        let mut results = Vec::new();
        let mut any_success = false;
        for (provider, outcome) in outcomes {
            match outcome {
                Ok(Ok(mut report)) => {
                    any_success = true;
                    let (health, message) = report_status(&report);
                    if let Some(warning) = &report.warning {
                        self.record_failure(provider, warning);
                    } else {
                        self.record_success(provider.id());
                    }
                    self.record_status(provider, health, message);
                    results.append(&mut report.results);
                }
                Ok(Err(error)) => {
                    errors.push(error.to_string());
                    self.record_failure(provider, &error);
                    self.record_status(provider, ProviderHealth::Unavailable, Some(error.message));
                }
                Err(_) => {
                    let error =
                        ProviderError::new(provider.id(), ProviderErrorKind::Network, "搜索超时");
                    errors.push(error.to_string());
                    self.record_failure(provider, &error);
                    self.record_status(provider, ProviderHealth::Unavailable, Some(error.message));
                }
            }
        }

        match settings.mode {
            ProviderOrderMode::Strict => results.sort_by(|left, right| {
                priority
                    .get(left.provider_id.as_str())
                    .cmp(&priority.get(right.provider_id.as_str()))
                    .then_with(|| right.score.total_cmp(&left.score))
            }),
            ProviderOrderMode::Smart => {
                let score_band = if settings.prefer_capabilities {
                    f64::from(settings.capability_preference_tolerance) / 100.0
                } else {
                    f64::from(DEFAULT_CAPABILITY_PREFERENCE_TOLERANCE) / 100.0
                };
                results.sort_by(|left, right| right.score.total_cmp(&left.score));
                if let Some(top_score) = results.first().map(|result| result.score) {
                    let band_len = results
                        .iter()
                        .take_while(|result| top_score - result.score <= score_band + f64::EPSILON)
                        .count();
                    results[..band_len].sort_by(|left, right| {
                        priority
                            .get(left.provider_id.as_str())
                            .cmp(&priority.get(right.provider_id.as_str()))
                            .then_with(|| right.score.total_cmp(&left.score))
                    });
                }
            }
        }
        Ok(ProviderSearchOutcome {
            results,
            statuses: self.statuses_for(&enabled_ids),
            auto_apply_threshold: settings.auto_apply_threshold,
            prefer_capabilities: settings.prefer_capabilities,
            capability_preference_tolerance: settings.capability_preference_tolerance,
            mode: settings.mode,
            provider_order: settings
                .providers
                .iter()
                .map(|provider| provider.id.clone())
                .collect(),
            error: (!any_success && !errors.is_empty()).then(|| errors.join("；")),
        })
    }

    pub async fn test_provider(
        &self,
        client: &reqwest::Client,
        provider_id: &str,
    ) -> Result<ProviderStatus, String> {
        let provider = self
            .providers
            .iter()
            .find(|provider| provider.id() == provider_id)
            .ok_or_else(|| "未知歌词源".to_string())?;
        let input = LyricsSearchInput {
            title: "晴天".into(),
            artist: "周杰伦".into(),
            album: None,
            duration_ms: Some(269_000),
            scoring: Arc::default(),
        };
        if let Some(error) = self.cooldown_error(provider.id()) {
            log::debug!("歌词源测试因冷却跳过：{}：{}", provider.id(), error.message);
            self.record_cooldown_status(provider.as_ref(), &error.message);
            return self
                .statuses
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .get(provider_id)
                .cloned()
                .ok_or_else(|| "无法读取歌词源状态".into());
        }
        match tokio::time::timeout(self.timeout, provider.search(client, &input)).await {
            Ok(Ok(report)) => {
                let (health, message) = report_status(&report);
                if let Some(warning) = &report.warning {
                    self.record_failure(provider.as_ref(), warning);
                } else {
                    self.record_success(provider.id());
                }
                self.record_status(provider.as_ref(), health, message);
            }
            Ok(Err(error)) => {
                self.record_failure(provider.as_ref(), &error);
                self.record_status(
                    provider.as_ref(),
                    ProviderHealth::Unavailable,
                    Some(error.message),
                )
            }
            Err(_) => {
                let error =
                    ProviderError::new(provider.id(), ProviderErrorKind::Network, "测试超时");
                self.record_failure(provider.as_ref(), &error);
                self.record_status(
                    provider.as_ref(),
                    ProviderHealth::Unavailable,
                    Some(error.message),
                )
            }
        }
        self.statuses_for(&[provider_id.to_string()])
            .into_iter()
            .next()
            .ok_or_else(|| "无法读取歌词源状态".into())
    }

    fn statuses(&self) -> Vec<ProviderStatus> {
        let provider_ids = self
            .settings
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let provider_ids = provider_ids
            .providers
            .iter()
            .map(|preference| preference.id.clone())
            .collect::<Vec<_>>();
        self.statuses_for(&provider_ids)
    }

    fn statuses_for(&self, provider_ids: &[String]) -> Vec<ProviderStatus> {
        let statuses = self
            .statuses
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        provider_ids
            .iter()
            .filter_map(|provider_id| {
                let mut status = statuses.get(provider_id).cloned()?;
                if let Some(error) = self.cooldown_error(provider_id) {
                    status.health = ProviderHealth::Unavailable;
                    status.message = Some(error.message);
                }
                Some(status)
            })
            .collect()
    }

    fn cooldown_error(&self, provider_id: &str) -> Option<ProviderError> {
        let revision = self.cooldown_revision(provider_id);
        let now = Instant::now();
        let mut cooldowns = self
            .cooldowns
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let (cooldown_revision, sticky, until) = cooldowns
            .get(provider_id)
            .map(|cooldown| (cooldown.revision, cooldown.sticky, cooldown.until))?;
        if cooldown_revision != revision {
            cooldowns.remove(provider_id);
            return None;
        }
        if sticky {
            return Some(ProviderError::new(
                provider_id,
                ProviderErrorKind::Configuration,
                "歌词源因鉴权或配置错误保持冷却，请修改相关设置或凭据后重试",
            ));
        }
        if until <= now {
            return None;
        }
        let remaining = until.duration_since(now);
        let seconds = remaining
            .as_secs()
            .saturating_add(if remaining.subsec_nanos() > 0 { 1 } else { 0 })
            .max(1);
        Some(ProviderError::new(
            provider_id,
            ProviderErrorKind::Http,
            format!("歌词源请求冷却中，剩余约 {seconds} 秒"),
        ))
    }

    fn record_success(&self, provider_id: &str) {
        self.cooldowns
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(provider_id);
    }

    fn record_failure(&self, provider: &dyn LyricsProvider, error: &ProviderError) {
        let revision = self.cooldown_revision(provider.id());
        let mut cooldowns = self
            .cooldowns
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let state = cooldowns
            .entry(provider.id().to_string())
            .or_insert(ProviderCooldown {
                until: Instant::now(),
                consecutive_failures: 0,
                sticky: false,
                revision,
            });
        if state.revision != revision {
            state.consecutive_failures = 0;
            state.sticky = false;
            state.revision = revision;
        }
        let duration = if matches!(
            &error.kind,
            ProviderErrorKind::Unauthorized | ProviderErrorKind::Configuration
        ) {
            state.sticky = true;
            Duration::from_secs(365 * 24 * 60 * 60)
        } else if error.status_code == Some(429) {
            state.consecutive_failures = 0;
            Duration::from_millis(error.retry_after_ms.unwrap_or(60_000))
        } else if error.kind == ProviderErrorKind::Network
            || error.status_code.is_some_and(|status| status >= 500)
        {
            let backoff = [15_u64, 30, 60, 120, 300];
            let index = usize::from(state.consecutive_failures).min(backoff.len() - 1);
            state.consecutive_failures = state.consecutive_failures.saturating_add(1);
            Duration::from_secs(backoff[index])
        } else {
            state.consecutive_failures = 0;
            Duration::from_secs(60)
        };
        state.until = Instant::now() + duration;
        log::debug!(
            "歌词源进入冷却：provider={} kind={:?} status={:?} seconds={} sticky={}",
            provider.id(),
            error.kind,
            error.status_code,
            duration.as_secs(),
            state.sticky
        );
    }

    /// 鉴权/配置冷却只随该歌词源自己的设置或凭据变化而失效，避免调整全局排序规则时误清除。
    fn cooldown_revision(&self, provider_id: &str) -> u64 {
        let settings = self
            .settings
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let mut hasher = DefaultHasher::new();
        provider_id.hash(&mut hasher);
        settings
            .providers
            .iter()
            .find(|provider| provider.id == provider_id)
            .map(|provider| provider.enabled)
            .hash(&mut hasher);
        if provider_id == "amll_ttml" {
            settings.amll_base_url.hash(&mut hasher);
        }
        drop(settings);
        if provider_id == "musixmatch" {
            if let Some((token_type, token)) = self.credentials.musixmatch_credentials() {
                format!("{token_type:?}").hash(&mut hasher);
                token.hash(&mut hasher);
            }
            self.credentials
                .musixmatch_anonymous_token()
                .hash(&mut hasher);
        }
        hasher.finish()
    }

    fn record_cooldown_status(&self, provider: &dyn LyricsProvider, message: &str) {
        let mut statuses = self
            .statuses
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(status) = statuses.get_mut(provider.id()) {
            status.health = ProviderHealth::Unavailable;
            status.message = Some(message.to_string());
        }
    }

    fn record_status(
        &self,
        provider: &dyn LyricsProvider,
        health: ProviderHealth,
        message: Option<String>,
    ) {
        self.statuses
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .insert(
                provider.id().into(),
                ProviderStatus {
                    provider_id: provider.id().into(),
                    name: provider.display_name().into(),
                    health,
                    message,
                    checked_at_ms: Some(now_ms()),
                },
            );
    }
}

fn report_status(report: &ProviderSearchReport) -> (ProviderHealth, Option<String>) {
    if let Some(warning) = &report.warning {
        return (
            ProviderHealth::Degraded,
            Some(format!("部分请求失败：{}", warning.message)),
        );
    }
    let message = if report.results.is_empty() {
        "连接正常，未找到同步歌词".into()
    } else {
        format!("连接正常，返回 {} 个候选", report.results.len())
    };
    (ProviderHealth::Available, Some(message))
}

fn provider_definitions() -> [(&'static str, &'static str); 8] {
    [
        ("lrclib", LRCLIB_DISPLAY_NAME),
        ("kugou", KUGOU_DISPLAY_NAME),
        ("qqmusic", QQMUSIC_DISPLAY_NAME),
        ("netease", NETEASE_DISPLAY_NAME),
        ("kuwo", KUWO_DISPLAY_NAME),
        ("amll_ttml", AMLL_DISPLAY_NAME),
        ("migu", MIGU_DISPLAY_NAME),
        ("musixmatch", MUSIXMATCH_DISPLAY_NAME),
    ]
}
