use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::super::super::amll_ttml::AmllTtmlProvider;
use super::super::super::credentials::ProviderCredentialStore;
use super::super::super::kugou::KugouProvider;
use super::super::super::kuwo::KuwoProvider;
use super::super::super::lrclib::LrcLibProvider;
use super::super::super::migu::MiguProvider;
use super::super::super::musixmatch::MusixmatchProvider;
use super::super::super::netease::NeteaseProvider;
use super::super::super::qqmusic::QqMusicProvider;
use super::super::{
    LyricsProvider, ProviderHealth, ProviderSettings, ProviderStatus, AMLL_DISPLAY_NAME,
    KUGOU_DISPLAY_NAME, KUWO_DISPLAY_NAME, LRCLIB_DISPLAY_NAME, MIGU_DISPLAY_NAME,
    MUSIXMATCH_DISPLAY_NAME, NETEASE_DISPLAY_NAME, QQMUSIC_DISPLAY_NAME,
};

pub(super) fn build_providers(
    settings: &Arc<RwLock<ProviderSettings>>,
    credentials: &Arc<ProviderCredentialStore>,
) -> Vec<Box<dyn LyricsProvider>> {
    vec![
        Box::new(NeteaseProvider),
        Box::new(QqMusicProvider),
        Box::new(KugouProvider),
        Box::new(LrcLibProvider::default()),
        Box::new(KuwoProvider),
        Box::new(AmllTtmlProvider::new(settings.clone())),
        Box::new(MiguProvider),
        Box::new(MusixmatchProvider::new(credentials.clone())),
    ]
}

pub(super) fn initial_statuses(
    providers: &[Box<dyn LyricsProvider>],
) -> HashMap<String, ProviderStatus> {
    providers
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
        .collect()
}

pub(in crate::lyrics::provider) fn provider_definitions() -> [(&'static str, &'static str); 8] {
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
