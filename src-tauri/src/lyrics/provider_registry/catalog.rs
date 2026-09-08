use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::super::super::amll_ttml::AmllTtmlProvider;
use super::super::super::credentials::ProviderCredentialStore;
use super::super::super::kugou::KugouProvider;
use super::super::super::lrclib::LrcLibProvider;
use super::super::super::musixmatch::MusixmatchProvider;
use super::super::super::netease::NeteaseProvider;
use super::super::super::qishui::QishuiProvider;
use super::super::super::qqmusic::QqMusicProvider;
use super::super::{
    LyricsProvider, ProviderAutoBindingPolicy, ProviderCapabilities, ProviderHealth,
    ProviderManifest, ProviderPersistencePolicy, ProviderSettings, ProviderStatus,
    ProviderStatusDetail, ProviderTier, AMLL_DISPLAY_NAME, KUGOU_DISPLAY_NAME, KUWO_DISPLAY_NAME,
    LRCLIB_DISPLAY_NAME, MIGU_DISPLAY_NAME, MUSIXMATCH_DISPLAY_NAME, NETEASE_DISPLAY_NAME,
    QISHUI_DISPLAY_NAME, QQMUSIC_DISPLAY_NAME,
};

pub(super) fn build_providers(
    settings: &Arc<RwLock<ProviderSettings>>,
    credentials: &Arc<ProviderCredentialStore>,
) -> Vec<Box<dyn LyricsProvider>> {
    let mut providers: Vec<Box<dyn LyricsProvider>> = vec![
        Box::new(NeteaseProvider),
        Box::new(QqMusicProvider),
        Box::new(KugouProvider),
        Box::new(LrcLibProvider::default()),
        Box::new(AmllTtmlProvider::new(settings.clone())),
        Box::new(MusixmatchProvider::new(credentials.clone())),
        Box::new(QishuiProvider),
    ];
    // 休眠来源保留实现代码，但不进入实例注册表，因此旧设置即使仍为 enabled
    // 也不会创建 Provider、更不会发起网络请求。
    let dormant_ids = provider_manifests()
        .into_iter()
        .filter(|manifest| manifest.tier == ProviderTier::Dormant)
        .map(|manifest| manifest.id)
        .collect::<std::collections::HashSet<_>>();
    providers.retain(|provider| !dormant_ids.contains(provider.id()));
    validate_manifest(&providers);
    providers
}

fn validate_manifest(providers: &[Box<dyn LyricsProvider>]) {
    let manifests = provider_manifests();
    let manifest_ids = manifests
        .iter()
        .map(|manifest| manifest.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let implementation_ids = providers
        .iter()
        .map(|provider| provider.id())
        .collect::<std::collections::HashSet<_>>();
    for id in implementation_ids.difference(&manifest_ids) {
        log::error!("歌词 Provider 缺少 Manifest：{id}");
    }
    for id in manifest_ids.difference(&implementation_ids) {
        if manifests
            .iter()
            .find(|manifest| manifest.id == *id)
            .is_some_and(|manifest| manifest.tier != ProviderTier::Dormant)
        {
            log::error!("歌词 Manifest 缺少可用实现：{id}");
        }
    }
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
                    detail: ProviderStatusDetail::NotTested,
                    checked_at_ms: None,
                },
            )
        })
        .collect()
}

pub(in crate::lyrics::provider) fn provider_manifests() -> Vec<ProviderManifest> {
    vec![
        manifest(
            "qqmusic",
            QQMUSIC_DISPLAY_NAME,
            ProviderTier::Stable,
            true,
            ProviderAutoBindingPolicy::Allowed,
            capabilities(true, true, true, true, true, true, true),
            false,
            ProviderPersistencePolicy::DownloadLibrary,
        ),
        manifest(
            "netease",
            NETEASE_DISPLAY_NAME,
            ProviderTier::Stable,
            true,
            ProviderAutoBindingPolicy::Allowed,
            capabilities(true, true, true, true, true, true, true),
            false,
            ProviderPersistencePolicy::DownloadLibrary,
        ),
        manifest(
            "kugou",
            KUGOU_DISPLAY_NAME,
            ProviderTier::Stable,
            true,
            ProviderAutoBindingPolicy::Allowed,
            capabilities(true, true, true, true, true, false, false),
            false,
            ProviderPersistencePolicy::DownloadLibrary,
        ),
        manifest(
            "lrclib",
            LRCLIB_DISPLAY_NAME,
            ProviderTier::Stable,
            true,
            ProviderAutoBindingPolicy::Allowed,
            capabilities(true, true, true, true, false, false, false),
            false,
            ProviderPersistencePolicy::DownloadLibrary,
        ),
        manifest(
            "amll_ttml",
            AMLL_DISPLAY_NAME,
            ProviderTier::ExactId,
            true,
            ProviderAutoBindingPolicy::Allowed,
            capabilities(true, true, false, true, true, true, true),
            false,
            ProviderPersistencePolicy::DownloadLibrary,
        ),
        manifest(
            "musixmatch",
            MUSIXMATCH_DISPLAY_NAME,
            ProviderTier::Advanced,
            false,
            ProviderAutoBindingPolicy::Allowed,
            capabilities(true, true, true, true, false, true, false),
            true,
            ProviderPersistencePolicy::DownloadLibrary,
        ),
        manifest(
            "qishui",
            QISHUI_DISPLAY_NAME,
            ProviderTier::Experimental,
            false,
            ProviderAutoBindingPolicy::Allowed,
            capabilities(true, true, true, true, true, true, false),
            false,
            ProviderPersistencePolicy::DownloadLibrary,
        ),
        manifest(
            "kuwo",
            KUWO_DISPLAY_NAME,
            ProviderTier::Dormant,
            false,
            ProviderAutoBindingPolicy::Disabled,
            capabilities(true, true, true, true, false, false, false),
            false,
            ProviderPersistencePolicy::CacheOnly,
        ),
        manifest(
            "migu",
            MIGU_DISPLAY_NAME,
            ProviderTier::Dormant,
            false,
            ProviderAutoBindingPolicy::Disabled,
            capabilities(true, true, true, true, false, true, false),
            false,
            ProviderPersistencePolicy::CacheOnly,
        ),
    ]
}

fn manifest(
    id: &str,
    display_name: &str,
    tier: ProviderTier,
    enabled_by_default: bool,
    auto_binding: ProviderAutoBindingPolicy,
    capabilities: ProviderCapabilities,
    requires_credentials: bool,
    persistence: ProviderPersistencePolicy,
) -> ProviderManifest {
    ProviderManifest {
        id: id.into(),
        display_name: display_name.into(),
        implementation_key: id.into(),
        tier,
        enabled_by_default,
        auto_binding,
        capabilities,
        requires_credentials,
        persistence,
        request_timeout_ms: 8_000,
        notice_key: (id == "qishui").then(|| "qishui".to_owned()),
    }
}

fn capabilities(
    metadata_search: bool,
    id_lookup: bool,
    plain_text: bool,
    line_timing: bool,
    word_timing: bool,
    translation: bool,
    romanization: bool,
) -> ProviderCapabilities {
    ProviderCapabilities {
        metadata_search,
        id_lookup,
        plain_text,
        line_timing,
        word_timing,
        translation,
        romanization,
    }
}

pub(in crate::lyrics::provider) fn provider_definitions() -> [(&'static str, &'static str); 9] {
    [
        ("lrclib", LRCLIB_DISPLAY_NAME),
        ("kugou", KUGOU_DISPLAY_NAME),
        ("qqmusic", QQMUSIC_DISPLAY_NAME),
        ("netease", NETEASE_DISPLAY_NAME),
        ("kuwo", KUWO_DISPLAY_NAME),
        ("amll_ttml", AMLL_DISPLAY_NAME),
        ("migu", MIGU_DISPLAY_NAME),
        ("musixmatch", MUSIXMATCH_DISPLAY_NAME),
        ("qishui", QISHUI_DISPLAY_NAME),
    ]
}
