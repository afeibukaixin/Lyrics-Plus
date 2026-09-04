mod cache;
mod catalog;
mod health;
mod search;

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};
use std::time::Duration;

use super::super::credentials::{
    MusixmatchTokenType, ProviderCredentialStore, ProviderCredentialView,
};
#[cfg(test)]
use super::ProviderSearchOutcome;
use super::{
    LyricsProvider, LyricsSearchInput, ProviderSettings, ProviderSettingsView, ProviderStatus,
};
use cache::{CachedSearch, SearchFlight, SearchKey};
pub(super) use catalog::provider_definitions;
use health::ProviderCooldown;

pub struct ProviderRegistry {
    pub(super) providers: Vec<Box<dyn LyricsProvider>>,
    pub(super) settings: Arc<RwLock<ProviderSettings>>,
    pub(super) credentials: Arc<ProviderCredentialStore>,
    pub(super) statuses: RwLock<std::collections::HashMap<String, ProviderStatus>>,
    pub(super) in_flight: Mutex<std::collections::HashMap<SearchKey, Weak<SearchFlight>>>,
    pub(super) cache: Mutex<std::collections::HashMap<SearchKey, CachedSearch>>,
    pub(super) cooldowns: Mutex<std::collections::HashMap<String, ProviderCooldown>>,
    pub(super) revision: AtomicU64,
    pub(super) timeout: Duration,
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
        let providers = catalog::build_providers(&settings, &credentials);
        let statuses = catalog::initial_statuses(&providers);
        Self {
            providers,
            settings,
            credentials,
            statuses: RwLock::new(statuses),
            in_flight: Mutex::new(std::collections::HashMap::new()),
            cache: Mutex::new(std::collections::HashMap::new()),
            cooldowns: Mutex::new(std::collections::HashMap::new()),
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
        super::normalize_settings(&mut settings)?;
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
        super::deduplicate(&mut outcome.results);
        outcome.results.truncate(24);
        Ok(outcome)
    }

    pub(crate) fn local_search_context(
        &self,
        input: &LyricsSearchInput,
    ) -> Result<(LyricsSearchInput, u8, bool, u8), String> {
        let settings = self
            .settings
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        Ok((
            search::with_scoring_settings(input, &settings)?,
            settings.auto_apply_threshold,
            settings.auto_apply_duration_guard_enabled,
            settings.auto_apply_duration_tolerance_seconds,
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
}
