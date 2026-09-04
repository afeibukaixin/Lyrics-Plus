use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::super::{
    LyricsProvider, LyricsSearchInput, ProviderError, ProviderErrorKind, ProviderHealth,
    ProviderStatus,
};
use super::search::report_status;
use super::ProviderRegistry;

pub(in crate::lyrics::provider) struct ProviderCooldown {
    until: Instant,
    consecutive_failures: u8,
    sticky: bool,
    revision: u64,
}

impl ProviderRegistry {
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

    pub(super) fn statuses(&self) -> Vec<ProviderStatus> {
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

    pub(super) fn statuses_for(&self, provider_ids: &[String]) -> Vec<ProviderStatus> {
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

    pub(super) fn cooldown_error(&self, provider_id: &str) -> Option<ProviderError> {
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

    pub(super) fn record_success(&self, provider_id: &str) {
        self.cooldowns
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(provider_id);
    }

    pub(super) fn record_failure(&self, provider: &dyn LyricsProvider, error: &ProviderError) {
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
    pub(super) fn cooldown_revision(&self, provider_id: &str) -> u64 {
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

    pub(super) fn record_cooldown_status(&self, provider: &dyn LyricsProvider, message: &str) {
        let mut statuses = self
            .statuses
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(status) = statuses.get_mut(provider.id()) {
            status.health = ProviderHealth::Unavailable;
            status.message = Some(message.to_string());
        }
    }

    pub(super) fn record_status(
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
                    checked_at_ms: Some(super::super::now_ms()),
                },
            );
    }
}
