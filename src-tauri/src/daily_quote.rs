use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::config::{DailyQuoteCategory, DailyQuoteMode, DailyQuoteSettings};
use crate::AppState;

const QUOTE_URL: &str = "https://v1.hitokoto.cn/";
const DEFAULT_RATE_LIMIT_COOLDOWN: Duration = Duration::from_secs(60);

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CachedQuote {
    text: String,
    // 旧缓存没有出处字段，继续显示原句，等下一次更新时补齐。
    #[serde(default)]
    source: Option<String>,
    fetched_at_ms: u64,
    #[serde(default)]
    categories: Vec<DailyQuoteCategory>,
}

#[derive(Deserialize)]
struct HitokotoResponse {
    hitokoto: String,
    #[serde(rename = "from")]
    source: Option<String>,
}

/// 缓存属于应用进程，不随主窗口的 WebView 销毁而丢失。
pub(crate) struct DailyQuoteService {
    cache_path: Option<PathBuf>,
    cached: Mutex<Option<CachedQuote>>,
    request_in_flight: AtomicBool,
    retry_not_before: Mutex<Option<Instant>>,
}

impl DailyQuoteService {
    pub(crate) fn new(cache_dir: Option<PathBuf>) -> Self {
        let cache_path = cache_dir.map(|directory| directory.join("daily-quote.json"));
        let cached = cache_path
            .as_ref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|raw| serde_json::from_slice::<CachedQuote>(&raw).ok())
            .filter(|quote| !quote.text.trim().is_empty());
        Self {
            cache_path,
            cached: Mutex::new(cached),
            request_in_flight: AtomicBool::new(false),
            retry_not_before: Mutex::new(None),
        }
    }

    pub(crate) fn window_title(&self, enabled: bool) -> String {
        if enabled {
            let cached = self
                .cached
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if let Some(quote) = cached.as_ref() {
                if let Some(source) = quote
                    .source
                    .as_deref()
                    .map(str::trim)
                    .filter(|source| !source.is_empty())
                {
                    return format!("{} · {} - {}", base_window_title(), quote.text, source);
                }
                return format!("{} · {}", base_window_title(), quote.text);
            }
        }
        base_window_title().to_owned()
    }

    fn is_expired(&self, settings: &DailyQuoteSettings) -> bool {
        let cached = self
            .cached
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(quote) = cached.as_ref() else {
            return true;
        };
        // 来源集合变化时，下次打开窗口立即更新，不等待原有小时间隔。
        let mut cached_categories = quote.categories.clone();
        cached_categories.sort_unstable();
        cached_categories.dedup();
        if cached_categories != settings.normalized_categories() {
            return true;
        }
        // 以成功获取时间计算间隔；系统时钟回拨时也允许重新获取。
        let now = now_ms();
        now < quote.fetched_at_ms
            || now - quote.fetched_at_ms >= u64::from(settings.update_interval_hours) * 3_600_000
    }

    fn should_refresh(&self, settings: &DailyQuoteSettings) -> bool {
        if self
            .retry_not_before
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_some_and(|until| Instant::now() < until)
        {
            return false;
        }
        match settings.mode {
            DailyQuoteMode::Off => false,
            DailyQuoteMode::EveryOpen => true,
            DailyQuoteMode::Hourly => self.is_expired(settings),
        }
    }

    async fn refresh(
        &self,
        client: &reqwest::Client,
        app: &tauri::AppHandle,
        settings: &DailyQuoteSettings,
    ) -> Result<bool, String> {
        let categories = settings.normalized_categories();
        // 重复的 c 参数由接口在所选分类中抽取句子，不在客户端随机选分类。
        let mut query = vec![("max_length", "40")];
        query.extend(categories.iter().map(|category| ("c", category.as_str())));
        let response = client
            .get(QUOTE_URL)
            .query(&query)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            // Retry-After 支持秒数和 HTTP 日期；缺失或无效时冷却 60 秒。
            // 冷却结束后仍等下一次打开窗口，不安排自动重试。
            let delay = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(retry_after_delay)
                .unwrap_or(DEFAULT_RATE_LIMIT_COOLDOWN);
            let now = Instant::now();
            *self
                .retry_not_before
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = Some(
                now.checked_add(delay)
                    .unwrap_or(now + DEFAULT_RATE_LIMIT_COOLDOWN),
            );
            return Err("一言接口限流，冷却期间继续显示缓存".into());
        }
        let response = response
            .error_for_status()
            .map_err(|error| error.to_string())?
            .json::<HitokotoResponse>()
            .await
            .map_err(|error| error.to_string())?;
        if response.hitokoto.trim().is_empty() {
            return Err("一言接口返回了空正文".into());
        }
        // 请求返回后重新核对来源，避免过时结果覆盖当前缓存。
        let current = app.state::<AppState>().config.snapshot().app.daily_quote;
        if !current.is_enabled() || current.normalized_categories() != categories {
            return Ok(false);
        }
        let quote = CachedQuote {
            text: response.hitokoto,
            source: response
                .source
                .map(|source| source.trim().to_owned())
                .filter(|source| !source.is_empty()),
            fetched_at_ms: now_ms(),
            categories,
        };
        *self
            .cached
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(quote.clone());
        if let Some(path) = self.cache_path.as_ref() {
            let save = || -> Result<(), String> {
                if let Some(directory) = path.parent() {
                    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
                }
                let raw = serde_json::to_vec(&quote).map_err(|error| error.to_string())?;
                std::fs::write(path, raw).map_err(|error| error.to_string())
            };
            if let Err(error) = save() {
                log::warn!("Failed to persist the daily quote cache: {error}");
            }
        }
        Ok(true)
    }
}

pub(crate) fn base_window_title() -> &'static str {
    if cfg!(debug_assertions) {
        "Lyrics Plus Dev"
    } else {
        "Lyrics Plus"
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn retry_after_delay(value: &str) -> Option<Duration> {
    let value = value.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let date = chrono::DateTime::parse_from_rfc2822(value).ok()?;
    let timestamp = u64::try_from(date.timestamp_millis()).ok()?;
    Some(Duration::from_millis(timestamp.saturating_sub(now_ms())))
}

/// 在主线程重新读取开关和窗口，避免请求完成时写入已销毁窗口或覆盖关闭操作。
pub(crate) fn sync_main_window_title(app: &tauri::AppHandle) {
    let handle = app.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        let Some(state) = handle.try_state::<AppState>() else {
            return;
        };
        if let Some(window) = handle.get_webview_window("main") {
            let enabled = state.config.snapshot().app.daily_quote.is_enabled();
            if let Err(error) = window.set_title(&state.daily_quote.window_title(enabled)) {
                log::warn!("Failed to update the main window title: {error}");
            }
        }
    }) {
        log::warn!("Failed to schedule the main window title update: {error}");
    }
}

struct InFlightRequest<'a>(&'a AtomicBool);

impl Drop for InFlightRequest<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// 仅由主窗口真正打开的入口调用；不绑定焦点变化，也不安装刷新定时器。
pub(crate) fn on_main_window_opened(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let settings = state.config.snapshot().app.daily_quote;
    let service = state.daily_quote.clone();
    if !service.should_refresh(&settings)
        || service
            .request_in_flight
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        return;
    }
    let client = state.http.clone();
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let _request = InFlightRequest(&service.request_in_flight);
        // 异步任务开始前可能已经关闭了语录开关。
        let settings = handle.state::<AppState>().config.snapshot().app.daily_quote;
        if !service.should_refresh(&settings) {
            return;
        }
        match service.refresh(&client, &handle, &settings).await {
            Ok(true) => sync_main_window_title(&handle),
            Ok(false) => {}
            Err(error) => {
                log::warn!("Failed to fetch the daily quote; keeping cached text: {error}")
            }
        }
    });
}
