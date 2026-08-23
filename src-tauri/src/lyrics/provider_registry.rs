pub struct ProviderRegistry {
    providers: Vec<Box<dyn LyricsProvider>>,
    settings: Arc<RwLock<ProviderSettings>>,
    credentials: Arc<ProviderCredentialStore>,
    statuses: RwLock<HashMap<String, ProviderStatus>>,
    in_flight: Mutex<HashMap<SearchKey, Weak<SearchFlight>>>,
    revision: AtomicU64,
    timeout: Duration,
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
        Self::build(settings, Arc::new(ProviderCredentialStore::memory()), None)
    }

    pub fn new_with_app_dir(settings: ProviderSettings, app_dir: &Path) -> Result<Self, String> {
        let credentials = Arc::new(ProviderCredentialStore::load(app_dir)?);
        Ok(Self::build(
            settings,
            credentials,
            Some(app_dir.join("cache").join("amll-index.json")),
        ))
    }

    fn build(
        settings: ProviderSettings,
        credentials: Arc<ProviderCredentialStore>,
        amll_cache_path: Option<std::path::PathBuf>,
    ) -> Self {
        let settings = Arc::new(RwLock::new(settings));
        let providers: Vec<Box<dyn LyricsProvider>> = vec![
            Box::new(NeteaseProvider),
            Box::new(QqMusicProvider),
            Box::new(KugouProvider),
            Box::new(LrcLibProvider),
            Box::new(KuwoProvider),
            Box::new(AmllTtmlProvider::new(settings.clone(), amll_cache_path)),
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

    pub async fn search(
        &self,
        client: &reqwest::Client,
        input: &LyricsSearchInput,
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
                in_flight.insert(key, Arc::downgrade(&flight));
                flight
            }
        };
        flight
            .get_or_init(|| self.search_once(client, input, settings))
            .await
            .clone()
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

        let scoring_input = with_scoring_settings(input, &settings)?;
        let scoring_input = &scoring_input;
        let jobs = enabled.iter().map(|provider| async move {
            let outcome =
                tokio::time::timeout(self.timeout, provider.search(client, scoring_input)).await;
            (*provider, outcome)
        });
        let outcomes = join_all(jobs).await;
        let mut results = Vec::new();
        let mut errors = Vec::new();
        let mut any_success = false;
        for (provider, outcome) in outcomes {
            match outcome {
                Ok(Ok(mut report)) => {
                    any_success = true;
                    let (health, message) = report_status(&report);
                    self.record_status(provider, health, message);
                    results.append(&mut report.results);
                }
                Ok(Err(error)) => {
                    errors.push(error.to_string());
                    self.record_status(provider, ProviderHealth::Unavailable, Some(error.message));
                }
                Err(_) => {
                    let message = "搜索超时".to_string();
                    errors.push(format!("{}：{message}", provider.display_name()));
                    self.record_status(provider, ProviderHealth::Unavailable, Some(message));
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
                results.sort_by(|left, right| right.score.total_cmp(&left.score));
                let mut band_start = 0;
                while band_start < results.len() {
                    let band_score = results[band_start].score;
                    let band_len = results[band_start..]
                        .iter()
                        .take_while(|result| band_score - result.score <= 0.035)
                        .count();
                    let band_end = band_start + band_len;
                    results[band_start..band_end].sort_by(|left, right| {
                        priority
                            .get(left.provider_id.as_str())
                            .cmp(&priority.get(right.provider_id.as_str()))
                            .then_with(|| right.score.total_cmp(&left.score))
                    });
                    band_start = band_end;
                }
            }
        }
        deduplicate(&mut results);
        results.truncate(24);
        Ok(ProviderSearchOutcome {
            results,
            statuses: self.statuses_for(&enabled_ids),
            auto_apply_threshold: settings.auto_apply_threshold,
            prefer_capabilities: settings.prefer_capabilities,
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
        match tokio::time::timeout(self.timeout, provider.search(client, &input)).await {
            Ok(Ok(report)) => {
                let (health, message) = report_status(&report);
                self.record_status(provider.as_ref(), health, message);
            }
            Ok(Err(error)) => self.record_status(
                provider.as_ref(),
                ProviderHealth::Unavailable,
                Some(error.message),
            ),
            Err(_) => self.record_status(
                provider.as_ref(),
                ProviderHealth::Unavailable,
                Some("测试超时".into()),
            ),
        }
        self.statuses
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .get(provider_id)
            .cloned()
            .ok_or_else(|| "无法读取歌词源状态".into())
    }

    fn statuses(&self) -> Vec<ProviderStatus> {
        let settings = self
            .settings
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let statuses = self
            .statuses
            .read()
            .unwrap_or_else(|error| error.into_inner());
        settings
            .providers
            .iter()
            .filter_map(|preference| statuses.get(&preference.id).cloned())
            .collect()
    }

    fn statuses_for(&self, provider_ids: &[String]) -> Vec<ProviderStatus> {
        let statuses = self
            .statuses
            .read()
            .unwrap_or_else(|error| error.into_inner());
        provider_ids
            .iter()
            .filter_map(|provider_id| statuses.get(provider_id).cloned())
            .collect()
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
            Some(format!("部分请求失败：{warning}")),
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
