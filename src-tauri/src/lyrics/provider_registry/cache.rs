use std::collections::HashMap;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use super::super::{LyricsSearchInput, ProviderSearchOutcome, ProviderSettings};
use super::ProviderRegistry;

pub(super) const SEARCH_CACHE_TTL: Duration = Duration::from_secs(120);
pub(super) const SEARCH_CACHE_CAPACITY: usize = 128;

#[derive(Clone)]
pub(in crate::lyrics::provider) struct CachedSearch {
    pub(super) outcome: ProviderSearchOutcome,
    pub(super) expires_at: Instant,
}

pub(in crate::lyrics::provider) type SearchFlight =
    tokio::sync::OnceCell<Result<ProviderSearchOutcome, String>>;

#[derive(Clone, PartialEq, Eq, Hash)]
pub(in crate::lyrics::provider) struct SearchKey {
    title: String,
    artist: String,
    album: Option<String>,
    duration_ms: Option<u64>,
    settings: ProviderSettings,
    revision: u64,
}

impl SearchKey {
    pub(super) fn new(
        input: &LyricsSearchInput,
        settings: ProviderSettings,
        revision: u64,
    ) -> Self {
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

impl ProviderRegistry {
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
            self.revision.load(std::sync::atomic::Ordering::SeqCst),
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
        let mut cache: std::sync::MutexGuard<'_, HashMap<SearchKey, CachedSearch>> =
            self.cache.lock().unwrap_or_else(|error| error.into_inner());
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
}
