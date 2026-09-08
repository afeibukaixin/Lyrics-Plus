use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::lyrics::provider::ProviderStatus;
use crate::lyrics::runtime::LyricsSearchIntent;
use crate::storage::Storage;

const TELEMETRY_ENABLED_PREFERENCE: &str = "telemetry.enabled";
const TELEMETRY_INSTALL_ID_PREFERENCE: &str = "telemetry.installId";
const TELEMETRY_LAST_ACTIVE_DAY_PREFERENCE: &str = "telemetry.lastActiveDay";
const TELEMETRY_ENDPOINT: Option<&str> = option_env!("LYRICS_PLUS_TELEMETRY_ENDPOINT");
const MAX_QUEUE_SIZE: usize = 500;
const BATCH_SIZE: usize = 50;
const FLUSH_INTERVAL: Duration = Duration::from_secs(5 * 60);

#[derive(Default)]
struct FeatureUsageCounters {
    searches: u64,
    manual_searches: u64,
    automatic_searches: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TelemetryEvent {
    event: String,
    timestamp_ms: u64,
    install_id: String,
    app_version: &'static str,
    platform: &'static str,
    #[serde(flatten)]
    properties: Map<String, Value>,
}

/// 匿名产品统计服务：只保存短期内存队列，发送失败时丢弃或有限重试，不影响主流程。
pub struct TelemetryService {
    storage: Arc<Storage>,
    http: reqwest::Client,
    queue: Mutex<VecDeque<TelemetryEvent>>,
    feature_usage: Mutex<FeatureUsageCounters>,
    flushing: AtomicBool,
    activated: AtomicBool,
}

impl TelemetryService {
    pub fn new(storage: Arc<Storage>, http: reqwest::Client) -> Self {
        Self {
            storage,
            http,
            queue: Mutex::new(VecDeque::new()),
            feature_usage: Mutex::new(FeatureUsageCounters::default()),
            flushing: AtomicBool::new(false),
            activated: AtomicBool::new(false),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.storage
            .get_preference(TELEMETRY_ENABLED_PREFERENCE)
            .ok()
            .flatten()
            .as_deref()
            != Some("false")
    }

    pub fn set_enabled(self: &Arc<Self>, enabled: bool) -> Result<(), String> {
        self.storage.set_preference(
            TELEMETRY_ENABLED_PREFERENCE,
            if enabled { "true" } else { "false" },
        )?;
        if !enabled {
            self.clear_local_state()?;
        } else if self.activated.load(Ordering::Acquire) {
            self.track("app_session_started", Map::new());
            self.track_daily_active();
        }
        Ok(())
    }

    pub fn activate(self: &Arc<Self>) {
        if self.activated.swap(true, Ordering::AcqRel) {
            return;
        }
        if self.is_enabled() {
            self.track("app_session_started", Map::new());
            self.track_daily_active();
        }

        let service = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(FLUSH_INTERVAL).await;
                service.flush().await;
            }
        });
    }

    pub fn track_search(
        self: &Arc<Self>,
        intent: LyricsSearchIntent,
        response: &crate::lyrics::runtime::SearchResponse,
        duration_ms: u64,
    ) {
        if !self.is_enabled() {
            return;
        }
        let intent = match intent {
            LyricsSearchIntent::Automatic => "automatic",
            LyricsSearchIntent::Refresh => "refresh",
            LyricsSearchIntent::Manual => "manual",
        };
        {
            let mut counters = self
                .feature_usage
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            counters.searches = counters.searches.saturating_add(1);
            if intent == "manual" {
                counters.manual_searches = counters.manual_searches.saturating_add(1);
            } else {
                counters.automatic_searches = counters.automatic_searches.saturating_add(1);
            }
        }
        let mut properties = Map::new();
        properties.insert("intent".into(), Value::String(intent.into()));
        properties.insert(
            "resultCount".into(),
            Value::from(response.results.len() as u64),
        );
        properties.insert(
            "providerCount".into(),
            Value::from(response.provider_statuses.len() as u64),
        );
        properties.insert("autoApplied".into(), Value::Bool(response.auto_apply));
        properties.insert("succeeded".into(), Value::Bool(response.error.is_none()));
        properties.insert("durationMs".into(), Value::from(duration_ms));
        self.track("lyrics_search_completed", properties);

        for status in &response.provider_statuses {
            self.track_provider(
                status,
                response
                    .provider_elapsed_ms
                    .get(&status.provider_id)
                    .copied(),
            );
        }
    }

    fn track_provider(self: &Arc<Self>, status: &ProviderStatus, duration_ms: Option<u64>) {
        let mut properties = Map::new();
        properties.insert(
            "providerId".into(),
            Value::String(status.provider_id.clone()),
        );
        properties.insert(
            "health".into(),
            Value::String(format_health(&status.health)),
        );
        properties.insert(
            "resultCount".into(),
            Value::from(status.detail.result_count() as u64),
        );
        if let Some(duration_ms) = duration_ms {
            properties.insert("durationMs".into(), Value::from(duration_ms));
        }
        self.track("lyrics_provider_completed", properties);
    }

    fn track_daily_active(self: &Arc<Self>) {
        let day = (unix_seconds() / 86_400).to_string();
        let stored = self
            .storage
            .get_preference(TELEMETRY_LAST_ACTIVE_DAY_PREFERENCE)
            .ok()
            .flatten();
        if stored.as_deref() == Some(day.as_str()) {
            return;
        }
        if let Err(error) = self
            .storage
            .set_preference(TELEMETRY_LAST_ACTIVE_DAY_PREFERENCE, &day)
        {
            log::debug!("保存统计活跃日期失败：{error}");
        }
        self.track("app_daily_active", Map::new());
    }

    fn track(self: &Arc<Self>, event: &str, properties: Map<String, Value>) {
        if !self.is_enabled() {
            return;
        }
        let Some(install_id) = self.install_id().ok().flatten() else {
            return;
        };
        let item = TelemetryEvent {
            event: event.to_owned(),
            timestamp_ms: unix_millis(),
            install_id,
            app_version: env!("CARGO_PKG_VERSION"),
            platform: std::env::consts::OS,
            properties,
        };
        let should_flush = {
            let mut queue = self.queue.lock().unwrap_or_else(|error| error.into_inner());
            queue.push_back(item);
            while queue.len() > MAX_QUEUE_SIZE {
                queue.pop_front();
            }
            queue.len() >= BATCH_SIZE
        };
        if should_flush {
            let service = Arc::clone(self);
            tauri::async_runtime::spawn(async move { service.flush().await });
        }
    }

    fn install_id(&self) -> Result<Option<String>, String> {
        if let Some(value) = self
            .storage
            .get_preference(TELEMETRY_INSTALL_ID_PREFERENCE)?
            .filter(|value| !value.trim().is_empty())
        {
            return Ok(Some(value));
        }
        let value = generate_install_id();
        self.storage
            .set_preference(TELEMETRY_INSTALL_ID_PREFERENCE, &value)?;
        Ok(Some(value))
    }

    fn clear_local_state(&self) -> Result<(), String> {
        self.storage
            .remove_preference(TELEMETRY_INSTALL_ID_PREFERENCE)?;
        self.storage
            .remove_preference(TELEMETRY_LAST_ACTIVE_DAY_PREFERENCE)?;
        self.queue
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clear();
        *self
            .feature_usage
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = FeatureUsageCounters::default();
        Ok(())
    }

    async fn flush(self: &Arc<Self>) {
        if !self.is_enabled()
            || self
                .flushing
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return;
        }

        if let Some(properties) = self.take_feature_usage() {
            // 该分支只在后台 flush 中执行；事件入队后仍由当前 flush 统一发送。
            // 这里不直接递归调用 flush，避免统计上报任务互相等待。
            self.track("feature_usage_rollup", properties);
        }

        let batch = {
            let mut queue = self.queue.lock().unwrap_or_else(|error| error.into_inner());
            let count = queue.len().min(BATCH_SIZE);
            queue.drain(..count).collect::<Vec<_>>()
        };
        if batch.is_empty() {
            self.flushing.store(false, Ordering::Release);
            return;
        }

        let body = match serde_json::to_vec(&batch) {
            Ok(body) => body,
            Err(error) => {
                log::debug!("统计事件序列化失败：{error}");
                self.requeue(batch);
                self.flushing.store(false, Ordering::Release);
                return;
            }
        };
        let sent = self.send_batch(&body).await;
        if !sent {
            self.requeue(batch);
        }
        self.flushing.store(false, Ordering::Release);
    }

    fn take_feature_usage(&self) -> Option<Map<String, Value>> {
        let mut counters = self
            .feature_usage
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if counters.searches == 0 {
            return None;
        }
        let mut properties = Map::new();
        properties.insert("searches".into(), Value::from(counters.searches));
        properties.insert(
            "manualSearches".into(),
            Value::from(counters.manual_searches),
        );
        properties.insert(
            "automaticSearches".into(),
            Value::from(counters.automatic_searches),
        );
        *counters = FeatureUsageCounters::default();
        Some(properties)
    }

    async fn send_batch(&self, body: &[u8]) -> bool {
        let Some(endpoint) = TELEMETRY_ENDPOINT else {
            // 发布构建未配置 Worker 地址时，安全地丢弃本地匿名队列，不向未知域名发请求。
            return true;
        };
        for attempt in 0..2 {
            let result = self
                .http
                .post(endpoint)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.to_owned())
                .send()
                .await;
            match result {
                Ok(response) if response.status().is_success() => return true,
                Ok(response) => {
                    log::debug!("统计上报失败：HTTP {}", response.status());
                }
                Err(error) => log::debug!("统计上报失败：{error}"),
            }
            if attempt == 0 {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
        false
    }

    fn requeue(&self, batch: Vec<TelemetryEvent>) {
        let mut queue = self.queue.lock().unwrap_or_else(|error| error.into_inner());
        for item in batch.into_iter().rev() {
            queue.push_front(item);
        }
        while queue.len() > MAX_QUEUE_SIZE {
            queue.pop_back();
        }
    }
}

fn format_health(health: &crate::lyrics::provider::ProviderHealth) -> String {
    match health {
        crate::lyrics::provider::ProviderHealth::Unknown => "unknown",
        crate::lyrics::provider::ProviderHealth::Available => "available",
        crate::lyrics::provider::ProviderHealth::Degraded => "degraded",
        crate::lyrics::provider::ProviderHealth::Unavailable => "unavailable",
    }
    .into()
}

fn generate_install_id() -> String {
    let seed = format!("{}:{}", unix_millis(), std::process::id());
    let digest = Sha256::digest(seed.as_bytes());
    digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
