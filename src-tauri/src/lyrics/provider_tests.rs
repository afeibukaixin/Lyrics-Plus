#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn defaults_to_all_providers_enabled() {
        let settings = ProviderSettings::default();
        assert!(settings.providers.iter().all(|provider| provider.enabled));
    }

    struct MockProvider {
        id: &'static str,
        score: f64,
        fails: bool,
        warning: Option<&'static str>,
        empty: bool,
        lyrics: &'static str,
        calls: Option<Arc<AtomicUsize>>,
        delay: Duration,
    }

    impl LyricsProvider for MockProvider {
        fn id(&self) -> &'static str {
            self.id
        }

        fn display_name(&self) -> &'static str {
            self.id
        }

        fn search<'a>(
            &'a self,
            _client: &'a reqwest::Client,
            _input: &'a LyricsSearchInput,
        ) -> ProviderFuture<'a, ProviderSearchReport> {
            Box::pin(async move {
                if let Some(calls) = &self.calls {
                    calls.fetch_add(1, Ordering::SeqCst);
                }
                if !self.delay.is_zero() {
                    tokio::time::sleep(self.delay).await;
                }
                if self.fails {
                    return Err(ProviderError {
                        provider_id: self.id.into(),
                        kind: ProviderErrorKind::Network,
                        message: "mock failure".into(),
                    });
                }
                Ok(ProviderSearchReport {
                    results: (!self.empty)
                        .then(|| result(self.id, self.score, self.lyrics))
                        .into_iter()
                        .collect(),
                    warning: self.warning.map(str::to_owned),
                })
            })
        }
    }

    fn result(provider_id: &str, score: f64, lyrics: &str) -> LyricsSearchResult {
        LyricsSearchResult {
            id: format!("{provider_id}:1"),
            provider_id: provider_id.into(),
            title: "Hello".into(),
            artist: "Adele".into(),
            album: Some("25".into()),
            duration_ms: Some(295_100),
            source: provider_id.into(),
            synced: true,
            has_translation: false,
            has_word_timing: false,
            has_romanization: false,
            score,
            lyrics: lyrics.into(),
        }
    }

    #[test]
    fn exact_synced_result_scores_highly() {
        let input = LyricsSearchInput {
            title: "Hello".into(),
            artist: "Adele".into(),
            album: Some("25".into()),
            duration_ms: Some(295_000),
            scoring: Arc::default(),
        };
        assert!(score_candidate(&input, &result("lrclib", 0.0, "line")) > 0.98);
    }

    #[test]
    fn normalise_treats_traditional_and_simplified_chinese_as_equal() {
        assert_eq!(normalise("愛上你不是我決定"), normalise("爱上你不是我决定"));
        assert_eq!(normalise("蕭敬騰"), normalise("萧敬腾"));
    }

    #[test]
    fn traditional_metadata_scores_the_same_as_simplified_metadata() {
        let traditional = LyricsSearchInput {
            title: "愛上你不是我決定".into(),
            artist: "蕭敬騰".into(),
            album: Some("愛的時刻".into()),
            duration_ms: Some(295_000),
            scoring: Arc::default(),
        };
        let simplified = LyricsSearchInput {
            title: "爱上你不是我决定".into(),
            artist: "萧敬腾".into(),
            album: Some("爱的时刻".into()),
            duration_ms: Some(295_000),
            scoring: Arc::default(),
        };
        let mut candidate = result("lrclib", 0.0, "line");
        candidate.title = simplified.title.clone();
        candidate.artist = simplified.artist.clone();
        candidate.album = simplified.album.clone();
        candidate.duration_ms = simplified.duration_ms;

        let traditional_score = score_candidate(&traditional, &candidate);
        assert_eq!(traditional_score, score_candidate(&simplified, &candidate));
        assert!(traditional_score > 0.98);
    }

    #[test]
    fn default_title_filters_remove_only_matching_metadata() {
        let keywords = prepare_title_filter_keywords(&default_title_filter_keywords()).unwrap();
        assert_eq!(
            filter_title("All For You - 《蜘蛛人：重生日》電影片尾曲", &keywords),
            "all for you"
        );
        assert_eq!(
            filter_title("愛上你不是我決定 (feat. A-Lin)", &keywords),
            "爱上你不是我决定"
        );
        assert_eq!(
            filter_title("All For You 《蜘蛛人：重生日》電影片尾曲", &keywords),
            "all for you"
        );
        assert_eq!(filter_title("Song featuring Artist", &keywords), "song");
        for title in [
            "A-B",
            "Song (Live)",
            "Song - Remix",
            "Song (Acoustic)",
            "伴奏",
            "Soft Landing",
            "Most Wanted",
        ] {
            assert_eq!(filter_title(title, &keywords), simplify(title));
        }
        assert_eq!(filter_title("Song Demo", &["demo".into()]), "song");
        assert_eq!(filter_title("Song (Live)", &["live".into()]), "song");
        assert_eq!(filter_title("伴奏", &["伴奏".into()]), "");
    }

    #[test]
    fn title_filters_apply_to_both_sides_of_scoring() {
        let keywords =
            Arc::new(prepare_title_filter_keywords(&default_title_filter_keywords()).unwrap());
        let mut input = LyricsSearchInput {
            title: "All For You - 《蜘蛛人：重生日》電影片尾曲".into(),
            artist: "OneRepublic".into(),
            album: None,
            duration_ms: Some(240_000),
            scoring: Arc::new(ScoringSettings {
                title_filter_keywords: keywords.as_ref().clone(),
                ..ScoringSettings::default()
            }),
        };
        let mut candidate = result("lrclib", 0.0, "line");
        candidate.title = "All For You".into();
        candidate.artist = input.artist.clone();
        candidate.album = None;
        candidate.duration_ms = input.duration_ms;
        assert!(score_candidate(&input, &candidate) > 0.98);

        input.title = "愛上你不是我決定".into();
        candidate.title = "爱上你不是我决定 (feat. A-Lin)".into();
        candidate.artist = "蕭敬騰".into();
        input.artist = "萧敬腾".into();
        assert!(score_candidate(&input, &candidate) > 0.98);
    }

    #[test]
    fn title_filter_validation_rejects_invalid_lists() {
        assert!(prepare_title_filter_keywords(&[]).is_ok());
        for keywords in [
            vec![" ".into()],
            vec!["same".into(), "same".into()],
            vec!["OST".into(), "ost".into()],
            vec!["电影".into(), "電影".into()],
            vec!["x".repeat(MAX_TITLE_FILTER_KEYWORD_LENGTH + 1)],
            vec!["x".into(); MAX_TITLE_FILTER_KEYWORDS + 1],
        ] {
            assert!(prepare_title_filter_keywords(&keywords).is_err());
        }

        let mut settings = ProviderSettings {
            title_filter_keywords: vec!["  Live  ".into()],
            ..ProviderSettings::default()
        };
        normalize_settings(&mut settings).unwrap();
        assert_eq!(settings.title_filter_keywords, ["Live"]);
    }

    #[test]
    fn auto_apply_uses_configured_threshold_and_requires_synced_lyrics() {
        assert!(can_auto_apply(
            &[result("a", 0.94, "one"), result("b", 0.84, "two")],
            60
        ));
        assert!(can_auto_apply(
            &[result("a", 0.94, "one"), result("b", 0.90, "two")],
            60
        ));
        assert!(can_auto_apply(&[result("a", 0.60, "one")], 60));
        assert!(!can_auto_apply(&[result("a", 0.59, "one")], 60));
        assert!(can_auto_apply(&[result("a", 0.0, "one")], 0));
        assert!(can_auto_apply(&[result("a", 1.0, "one")], 100));
        assert!(!can_auto_apply(&[result("a", 0.99, "one")], 100));
        let mut unsynced = result("a", 0.99, "one");
        unsynced.synced = false;
        assert!(!can_auto_apply(&[unsynced], 60));
    }

    #[test]
    fn duplicate_lyrics_are_removed_across_sources() {
        let mut results = vec![
            result("netease", 0.9, "[00:01] Hello"),
            result("qqmusic", 0.8, "[00:01]Hello"),
        ];
        deduplicate(&mut results);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn invalid_settings_are_rejected() {
        assert!(validate_settings(&ProviderSettings {
            mode: ProviderOrderMode::Smart,
            providers: vec![ProviderPreference {
                id: "unknown".into(),
                enabled: true,
            }],
            auto_apply_threshold: 60,
            ..ProviderSettings::default()
        })
        .is_err());
        assert!(validate_settings(&ProviderSettings {
            mode: ProviderOrderMode::Smart,
            providers: vec![ProviderPreference {
                id: "lrclib".into(),
                enabled: false,
            }],
            auto_apply_threshold: 60,
            ..ProviderSettings::default()
        })
        .is_err());
        assert!(validate_settings(&ProviderSettings {
            auto_apply_threshold: 101,
            ..ProviderSettings::default()
        })
        .is_err());
    }

    #[test]
    fn provider_display_names_are_stable_brand_names() {
        let registry = ProviderRegistry::default();
        let names = registry
            .settings_view()
            .statuses
            .into_iter()
            .map(|status| (status.provider_id, status.name))
            .collect::<HashMap<_, _>>();

        for (provider_id, expected_name) in provider_definitions() {
            assert_eq!(
                names.get(provider_id).map(String::as_str),
                Some(expected_name)
            );
        }
    }

    #[test]
    fn default_settings_use_current_smart_priority() {
        let settings = ProviderSettings::default();
        assert_eq!(settings.mode, ProviderOrderMode::Smart);
        assert_eq!(settings.auto_apply_threshold, 60);
        assert_eq!(settings.title_filter_keywords.len(), 12);
        assert_eq!(
            settings
                .providers
                .iter()
                .map(|provider| provider.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "lrclib",
                "kugou",
                "qqmusic",
                "netease",
                "kuwo",
                "amll_ttml",
                "migu",
                "musixmatch",
            ]
        );
    }

    #[test]
    fn explicitly_saved_legacy_order_is_preserved() {
        let registry = ProviderRegistry::default();
        let settings = ProviderSettings {
            mode: ProviderOrderMode::Smart,
            auto_apply_threshold: 60,
            prefer_capabilities: false,
            match_weights: MatchWeights::default(),
            normalize_chinese: true,
            providers: ["netease", "qqmusic", "kugou", "lrclib"]
                .into_iter()
                .map(|id| ProviderPreference {
                    id: id.into(),
                    enabled: true,
                })
                .collect(),
            title_filter_keywords: default_title_filter_keywords(),
            amll_base_url: default_amll_base_url(),
        };

        let view = registry.set_settings(settings.clone()).unwrap();
        let provider_ids = view
            .settings
            .providers
            .iter()
            .map(|provider| provider.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            &provider_ids[..settings.providers.len()],
            ["netease", "qqmusic", "kugou", "lrclib"]
        );
        assert_eq!(
            &provider_ids[settings.providers.len()..],
            ["kuwo", "amll_ttml", "migu", "musixmatch"]
        );
        assert!(view.settings.providers[settings.providers.len()..]
            .iter()
            .all(|provider| provider.enabled));
    }

    fn mock_registry(mode: ProviderOrderMode, netease_fails: bool) -> ProviderRegistry {
        let settings = ProviderSettings {
            mode,
            auto_apply_threshold: 60,
            prefer_capabilities: false,
            match_weights: MatchWeights::default(),
            normalize_chinese: true,
            providers: vec![
                ProviderPreference {
                    id: "lrclib".into(),
                    enabled: true,
                },
                ProviderPreference {
                    id: "netease".into(),
                    enabled: true,
                },
            ],
            title_filter_keywords: default_title_filter_keywords(),
            amll_base_url: default_amll_base_url(),
        };
        let statuses = ["lrclib", "netease"]
            .into_iter()
            .map(|id| {
                (
                    id.into(),
                    ProviderStatus {
                        provider_id: id.into(),
                        name: id.into(),
                        health: ProviderHealth::Unknown,
                        message: None,
                        checked_at_ms: None,
                    },
                )
            })
            .collect();
        ProviderRegistry {
            providers: vec![
                Box::new(MockProvider {
                    id: "lrclib",
                    score: 0.70,
                    fails: false,
                    warning: None,
                    empty: false,
                    lyrics: "[00:01]Same",
                    calls: None,
                    delay: Duration::ZERO,
                }),
                Box::new(MockProvider {
                    id: "netease",
                    score: 0.98,
                    fails: netease_fails,
                    warning: None,
                    empty: false,
                    lyrics: "[00:01]Same",
                    calls: None,
                    delay: Duration::ZERO,
                }),
            ],
            settings: Arc::new(RwLock::new(settings)),
            credentials: Arc::new(ProviderCredentialStore::memory()),
            statuses: RwLock::new(statuses),
            in_flight: Mutex::new(HashMap::new()),
            timeout: Duration::from_millis(100),
        }
    }

    fn single_mock_registry(warning: Option<&'static str>, empty: bool) -> ProviderRegistry {
        ProviderRegistry {
            providers: vec![Box::new(MockProvider {
                id: "lrclib",
                score: 0.70,
                fails: false,
                warning,
                empty,
                lyrics: "[00:01]lrclib",
                calls: None,
                delay: Duration::ZERO,
            })],
            settings: Arc::new(RwLock::new(ProviderSettings {
                mode: ProviderOrderMode::Smart,
                auto_apply_threshold: 60,
                prefer_capabilities: false,
                match_weights: MatchWeights::default(),
                normalize_chinese: true,
                providers: vec![ProviderPreference {
                    id: "lrclib".into(),
                    enabled: true,
                }],
                title_filter_keywords: default_title_filter_keywords(),
                amll_base_url: default_amll_base_url(),
            })),
            credentials: Arc::new(ProviderCredentialStore::memory()),
            statuses: RwLock::new(HashMap::from([(
                "lrclib".into(),
                ProviderStatus {
                    provider_id: "lrclib".into(),
                    name: "lrclib".into(),
                    health: ProviderHealth::Unknown,
                    message: None,
                    checked_at_ms: None,
                },
            )])),
            in_flight: Mutex::new(HashMap::new()),
            timeout: Duration::from_millis(100),
        }
    }

    fn counting_registry(calls: Arc<AtomicUsize>) -> ProviderRegistry {
        ProviderRegistry {
            providers: vec![Box::new(MockProvider {
                id: "lrclib",
                score: 0.90,
                fails: false,
                warning: None,
                empty: false,
                lyrics: "[00:01]Hello",
                calls: Some(calls),
                delay: Duration::from_millis(20),
            })],
            settings: Arc::new(RwLock::new(ProviderSettings {
                providers: vec![ProviderPreference {
                    id: "lrclib".into(),
                    enabled: true,
                }],
                ..ProviderSettings::default()
            })),
            credentials: Arc::new(ProviderCredentialStore::memory()),
            statuses: RwLock::new(HashMap::from([(
                "lrclib".into(),
                ProviderStatus {
                    provider_id: "lrclib".into(),
                    name: "lrclib".into(),
                    health: ProviderHealth::Unknown,
                    message: None,
                    checked_at_ms: None,
                },
            )])),
            in_flight: Mutex::new(HashMap::new()),
            timeout: Duration::from_millis(100),
        }
    }

    #[test]
    fn one_provider_failure_does_not_hide_other_results() {
        tauri::async_runtime::block_on(async {
            let client = reqwest::Client::new();
            let outcome = mock_registry(ProviderOrderMode::Smart, true)
                .search(
                    &client,
                    &LyricsSearchInput {
                        title: "Hello".into(),
                        artist: "Adele".into(),
                        album: None,
                        duration_ms: None,
                        scoring: Arc::default(),
                    },
                )
                .await
                .unwrap();
            assert_eq!(outcome.results.len(), 1);
            assert_eq!(outcome.results[0].provider_id, "lrclib");
            assert_eq!(
                outcome
                    .statuses
                    .iter()
                    .find(|status| status.provider_id == "netease")
                    .unwrap()
                    .health,
                ProviderHealth::Unavailable
            );
        });
    }

    #[test]
    fn partial_provider_failure_is_reported_as_degraded() {
        tauri::async_runtime::block_on(async {
            let outcome = single_mock_registry(Some("detail failed"), false)
                .search(
                    &reqwest::Client::new(),
                    &LyricsSearchInput {
                        title: "Hello".into(),
                        artist: "Adele".into(),
                        album: None,
                        duration_ms: None,
                        scoring: Arc::default(),
                    },
                )
                .await
                .unwrap();
            let status = &outcome.statuses[0];
            assert_eq!(status.health, ProviderHealth::Degraded);
            assert_eq!(
                status.message.as_deref(),
                Some("部分请求失败：detail failed")
            );
        });
    }

    #[test]
    fn successful_empty_provider_is_available() {
        tauri::async_runtime::block_on(async {
            let outcome = single_mock_registry(None, true)
                .search(
                    &reqwest::Client::new(),
                    &LyricsSearchInput {
                        title: "Missing".into(),
                        artist: "Artist".into(),
                        album: None,
                        duration_ms: None,
                        scoring: Arc::default(),
                    },
                )
                .await
                .unwrap();
            assert!(outcome.results.is_empty());
            assert_eq!(outcome.statuses[0].health, ProviderHealth::Available);
            assert_eq!(
                outcome.statuses[0].message.as_deref(),
                Some("连接正常，未找到同步歌词")
            );
        });
    }

    #[test]
    fn sorting_mode_switches_between_priority_and_score() {
        tauri::async_runtime::block_on(async {
            let client = reqwest::Client::new();
            let input = LyricsSearchInput {
                title: "Hello".into(),
                artist: "Adele".into(),
                album: None,
                duration_ms: None,
                scoring: Arc::default(),
            };
            let strict = mock_registry(ProviderOrderMode::Strict, false)
                .search(&client, &input)
                .await
                .unwrap();
            assert_eq!(strict.results.len(), 1);
            assert_eq!(strict.results[0].provider_id, "lrclib");
            let smart = mock_registry(ProviderOrderMode::Smart, false)
                .search(&client, &input)
                .await
                .unwrap();
            assert_eq!(smart.results.len(), 1);
            assert_eq!(smart.results[0].provider_id, "netease");
        });
    }

    #[test]
    fn concurrent_identical_searches_share_only_in_flight_work() {
        tauri::async_runtime::block_on(async {
            let calls = Arc::new(AtomicUsize::new(0));
            let registry = counting_registry(calls.clone());
            let client = reqwest::Client::new();
            let input = LyricsSearchInput {
                title: "Hello".into(),
                artist: "Adele".into(),
                album: None,
                duration_ms: None,
                scoring: Arc::default(),
            };

            let (first, second) = tokio::join!(
                registry.search(&client, &input),
                registry.search(&client, &input),
            );
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert_eq!(first.unwrap().results[0].id, second.unwrap().results[0].id);

            registry.search(&client, &input).await.unwrap();
            assert_eq!(calls.load(Ordering::SeqCst), 2);

            let mut different = input.clone();
            different.title = "World".into();
            let _ = tokio::join!(
                registry.search(&client, &input),
                registry.search(&client, &different),
            );
            assert_eq!(calls.load(Ordering::SeqCst), 4);
        });
    }

    #[test]
    #[ignore = "需要访问外部歌词服务"]
    fn live_each_provider_returns_candidates() {
        tauri::async_runtime::block_on(async {
            let client = reqwest::Client::builder()
                .user_agent("Lyrics Plus integration test")
                .timeout(Duration::from_secs(8))
                .build()
                .unwrap();
            let input = LyricsSearchInput {
                title: "晴天".into(),
                artist: "周杰伦".into(),
                album: Some("叶惠美".into()),
                duration_ms: Some(269_000),
                scoring: Arc::default(),
            };
            for target in ["netease", "qqmusic", "kugou", "lrclib"] {
                let settings = ProviderSettings {
                    mode: ProviderOrderMode::Smart,
                    auto_apply_threshold: 60,
                    prefer_capabilities: false,
                    match_weights: MatchWeights::default(),
                    normalize_chinese: true,
                    providers: provider_definitions()
                        .into_iter()
                        .map(|(id, _)| ProviderPreference {
                            id: id.into(),
                            enabled: id == target,
                        })
                        .collect(),
                    title_filter_keywords: default_title_filter_keywords(),
                    amll_base_url: default_amll_base_url(),
                };
                let registry = ProviderRegistry::new(settings);
                let outcome = registry.search(&client, &input).await.unwrap();
                assert!(
                    !outcome.results.is_empty(),
                    "{target} did not return candidates"
                );
                assert!(outcome
                    .results
                    .iter()
                    .all(|result| result.provider_id == target));
            }
        });
    }
}
