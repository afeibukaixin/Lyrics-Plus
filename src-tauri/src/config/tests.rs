#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("lyrics-plus-{name}-{stamp}"))
    }

    #[test]
    fn jsonc_supports_comments_trailing_commas_and_partial_values() {
        let parsed = parse_config_draft(
            r##"{
              // only override two fields
              "app": { "theme": "light", },
              /* keep everything else default */
              "overlay": { "appearance": { "activeColor": "#ff0000", }, },
            }"##,
        )
        .unwrap();
        assert_eq!(parsed.config.app.theme, ThemePreference::Light);
        assert_eq!(parsed.config.overlay.appearance.active_color, "#ff0000");
        assert_eq!(parsed.config.app.player_selection, PlayerSelection::Auto);
    }

    #[test]
    fn accepts_system_player_selection() {
        let parsed = parse_config_draft(r#"{"app":{"playerSelection":"system"}}"#).unwrap();
        assert_eq!(parsed.config.app.player_selection, PlayerSelection::System);
        let serialized = serde_json::to_value(parsed.config).unwrap();
        assert_eq!(
            serialized
                .pointer("/app/playerSelection")
                .and_then(Value::as_str),
            Some("system")
        );
    }

    #[test]
    fn system_media_filter_mode_defaults_validates_and_round_trips() {
        let default = parse_config_draft(r#"{"app":{}}"#).unwrap();
        assert_eq!(
            default.config.app.system_media_filter_mode,
            SystemMediaFilterMode::Allowlist
        );

        let blocklist = parse_config_draft(
            r#"{"app":{"systemMediaFilterMode":"blocklist","systemMediaApplications":[{"name":"Browser","bundleId":"org.example.Browser"}]}}"#,
        )
        .unwrap();
        assert_eq!(
            blocklist.config.app.system_media_filter_mode,
            SystemMediaFilterMode::Blocklist
        );
        assert_eq!(
            serde_json::to_value(blocklist.config)
                .unwrap()
                .pointer("/app/systemMediaFilterMode")
                .and_then(Value::as_str),
            Some("blocklist")
        );
        assert!(parse_config_draft(r#"{"app":{"systemMediaFilterMode":"unsupported"}}"#).is_err());
    }

    #[test]
    fn schema_twenty_three_preserves_system_media_filter_behavior() {
        let empty =
            parse_config_draft(r#"{"schemaVersion":23,"app":{"systemMediaApplications":[]}}"#)
                .unwrap();
        assert_eq!(
            empty.config.app.system_media_filter_mode,
            SystemMediaFilterMode::Blocklist
        );

        let listed = parse_config_draft(
            r#"{"schemaVersion":23,"app":{"systemMediaApplications":[{"name":"Player","bundleId":"org.example.Player"}]}}"#,
        )
        .unwrap();
        assert_eq!(
            listed.config.app.system_media_filter_mode,
            SystemMediaFilterMode::Allowlist
        );
        assert_eq!(listed.config.app.system_media_applications.len(), 1);
    }

    #[test]
    fn provided_provider_arrays_are_completed_with_disabled_entries() {
        let parsed = parse_config_draft(
            r#"{
              "lyrics": {
                "providers": {
                  "providers": [{ "id": "lrclib", "enabled": true }]
                }
              }
            }"#,
        )
        .unwrap();
        assert_eq!(parsed.config.lyrics.providers.providers.len(), 8);
        assert_eq!(parsed.config.lyrics.providers.providers[0].id, "lrclib");
        assert!(parsed.config.lyrics.providers.providers[0].enabled);
        assert!(parsed.config.lyrics.providers.providers[1..]
            .iter()
            .all(|provider| !provider.enabled));
        let default_lines = canonical_config_jsonc(&AppConfig::default(), UiLanguage::ZhCn)
            .unwrap()
            .lines()
            .count();
        assert_eq!(parsed.normalized_json.lines().count(), default_lines);
    }

    #[test]
    fn unknown_fields_are_rejected_with_location() {
        let validation = validate_config_draft("{\n  \"app\": { \"fontScael\": 120 }\n}");
        assert!(!validation.valid);
        let error = validation.error.unwrap();
        assert_eq!(error.line, 2);
        assert!(error.message.contains("fontScael"));
    }

    #[test]
    fn invalid_draft_falls_back_to_default() {
        let validation = validate_config_draft("{ nope }");
        assert!(!validation.valid);
        assert_eq!(validation.effective_config.app.theme, ThemePreference::Dark);
    }

    #[test]
    fn default_template_matches_runtime_default() {
        let default_jsonc =
            canonical_config_jsonc(&AppConfig::default(), UiLanguage::ZhCn).unwrap();
        let parsed = parse_config_draft(&default_jsonc).unwrap();
        assert!(!parsed.config.app.silent_startup);
        assert_eq!(parsed.config.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(parsed.normalized_json, default_jsonc);
        assert!(parsed.normalized_json.contains("// 配置结构版本"));
        assert!(parsed.normalized_json.contains("// 仅在本地匹配评分前"));
    }

    #[test]
    fn localized_templates_have_the_same_effective_config() {
        let zh = canonical_config_jsonc(&AppConfig::default(), UiLanguage::ZhCn).unwrap();
        let en = canonical_config_jsonc(&AppConfig::default(), UiLanguage::EnUs).unwrap();
        assert!(zh.contains("// 配置结构版本"));
        assert!(en.contains("// Configuration schema version"));
        assert_eq!(
            serde_json::to_value(parse_config_draft(&zh).unwrap().config).unwrap(),
            serde_json::to_value(parse_config_draft(&en).unwrap().config).unwrap(),
        );
    }

    #[test]
    fn schema_eight_adds_shortcuts_and_independent_overlay_controls() {
        let parsed = parse_config_draft(r#"{"schemaVersion":8}"#).unwrap();
        assert!(parsed.migrated);
        assert_eq!(parsed.config.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(
            parsed.config.app.shortcuts,
            GlobalShortcutSettings::default()
        );
        assert_eq!(parsed.config.overlay.appearance.secondary_font_scale, 1.0);
        assert_eq!(parsed.config.overlay.appearance.background_opacity, 0.6);
        assert_eq!(parsed.config.overlay.appearance.background_blur, 18.0);
    }

    #[test]
    fn schema_nine_adds_background_blur() {
        let parsed = parse_config_draft(r#"{"schemaVersion":9}"#).unwrap();
        assert!(parsed.migrated);
        assert_eq!(parsed.config.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(parsed.config.overlay.appearance.background_blur, 18.0);
    }

    #[test]
    fn schema_ten_transparent_background_becomes_transparent_mode() {
        let parsed = parse_config_draft(
            r#"{"schemaVersion":10,"overlay":{"appearance":{"background":"transparent","backgroundOpacity":0.75}}}"#,
        )
        .unwrap();
        assert!(parsed.migrated);
        assert_eq!(
            parsed.config.overlay.appearance.background,
            OverlayBackground::Solid
        );
        assert_eq!(
            parsed.config.overlay.appearance.background_mode,
            OverlayBackgroundMode::Transparent
        );
        assert_eq!(parsed.config.overlay.appearance.background_opacity, 0.75);
    }

    #[test]
    fn schema_ten_glass_and_solid_backgrounds_become_solid_mode() {
        for background in ["glass", "solid"] {
            let raw = format!(
                r#"{{"schemaVersion":10,"overlay":{{"appearance":{{"background":"{background}","backgroundOpacity":0.35,"backgroundBlur":26}}}}}}"#
            );
            let parsed = parse_config_draft(&raw).unwrap();
            assert!(parsed.migrated);
            assert_eq!(
                parsed.config.overlay.appearance.background_mode,
                OverlayBackgroundMode::Solid
            );
            assert_eq!(parsed.config.overlay.appearance.background_opacity, 0.35);
            assert_eq!(parsed.config.overlay.appearance.background_blur, 26.0);
        }
    }

    #[test]
    fn current_background_opacity_is_preserved() {
        let parsed = parse_config_draft(
            r#"{"schemaVersion":14,"overlay":{"appearance":{"backgroundOpacity":0.85}}}"#,
        )
        .unwrap();
        assert_eq!(parsed.config.overlay.appearance.background_opacity, 0.85);
    }

    #[test]
    fn schema_eleven_adds_disabled_hide_when_not_playing() {
        let parsed = parse_config_draft(r#"{"schemaVersion":11}"#).unwrap();
        assert!(parsed.migrated);
        assert_eq!(parsed.config.schema_version, CONFIG_SCHEMA_VERSION);
        assert!(!parsed.config.overlay.hide_when_not_playing);
    }

    #[test]
    fn schema_twelve_adds_system_language_preference() {
        let parsed = parse_config_draft(r#"{"schemaVersion":12}"#).unwrap();
        assert!(parsed.migrated);
        assert_eq!(parsed.config.app.language, LanguagePreference::default());
        assert!(parsed.normalized_json.contains("\"language\": \"system\""));
    }

    #[test]
    fn schema_thirteen_migrates_only_the_old_provider_default() {
        let old_default = parse_config_draft(
            r#"{
              "schemaVersion": 13,
              "lyrics": { "providers": {
                "mode": "smart",
                "autoApplyThreshold": 60,
                "providers": [
                  { "id": "lrclib", "enabled": true },
                  { "id": "kugou", "enabled": true },
                  { "id": "qqmusic", "enabled": true },
                  { "id": "netease", "enabled": true }
                ]
              } }
            }"#,
        )
        .unwrap();
        assert!(old_default
            .config
            .lyrics
            .providers
            .providers
            .iter()
            .all(|provider| provider.enabled));

        let customized = parse_config_draft(
            r#"{
              "schemaVersion": 13,
              "lyrics": { "providers": {
                "mode": "smart",
                "autoApplyThreshold": 61,
                "providers": [
                  { "id": "lrclib", "enabled": true },
                  { "id": "kugou", "enabled": true },
                  { "id": "qqmusic", "enabled": true },
                  { "id": "netease", "enabled": true }
                ]
              } }
            }"#,
        )
        .unwrap();
        assert!(customized
            .config
            .lyrics
            .providers
            .providers
            .iter()
            .all(|provider| provider.enabled));
    }

    #[test]
    fn schema_fifteen_adds_default_title_keywords_but_preserves_an_empty_list() {
        let migrated = parse_config_draft(r#"{"schemaVersion":15}"#).unwrap();
        assert!(migrated.migrated);
        assert_eq!(
            migrated.config.lyrics.providers.title_filter_keywords,
            ProviderSettings::default().title_filter_keywords
        );

        let disabled = parse_config_draft(
            r#"{"schemaVersion":15,"lyrics":{"providers":{"titleFilterKeywords":[]}}}"#,
        )
        .unwrap();
        assert!(disabled
            .config
            .lyrics
            .providers
            .title_filter_keywords
            .is_empty());
    }

    #[test]
    fn previous_schema_discards_removed_app_fields() {
        let parsed = parse_config_draft(
            r#"{"schemaVersion":20,"app":{"removedField":true,"systemMediaApplications":[{"name":"Player","bundleId":"org.example.Player"}]}}"#,
        )
        .unwrap();
        assert!(parsed.migrated);
        assert_eq!(
            parsed.config.app.system_media_applications,
            [RegisteredApplication {
                name: "Player".into(),
                bundle_id: "org.example.Player".into(),
            }]
        );
        assert!(!parsed.normalized_json.contains("removedField"));
    }

    #[test]
    fn system_media_apps_reject_dedicated_players() {
        let applications = vec![
            RegisteredApplication {
                name: "Spotify".into(),
                bundle_id: "com.spotify.client".into(),
            },
            RegisteredApplication {
                name: "Player".into(),
                bundle_id: "org.example.Player".into(),
            },
        ];
        assert_eq!(
            normalize_system_media_applications(vec![applications[1].clone()]).unwrap(),
            [RegisteredApplication {
                name: "Player".into(),
                bundle_id: "org.example.Player".into(),
            }]
        );
        assert!(normalize_system_media_applications(vec![applications[0].clone()]).is_err());
    }

    #[test]
    fn language_preference_accepts_supported_and_future_bcp_47_values() {
        for (raw, expected) in [
            (r#"{"app":{"language":"system"}}"#, "system"),
            (r#"{"app":{"language":"zh-CN"}}"#, "zh-CN"),
            (r#"{"app":{"language":"zh-TW"}}"#, "zh-TW"),
            (r#"{"app":{"language":"en-US"}}"#, "en-US"),
            (r#"{"app":{"language":"fr-FR"}}"#, "fr-FR"),
        ] {
            let parsed = parse_config_draft(raw).unwrap();
            assert_eq!(parsed.config.app.language.0, expected);
        }
    }

    #[test]
    fn language_preference_rejects_invalid_language_tags() {
        for language in ["", "zh_TW", "-zh", "1", "zh--TW", "zh-繁體"] {
            let raw = format!(r#"{{"app":{{"language":"{language}"}}}}"#);
            let validation = validate_config_draft(&raw);
            assert!(!validation.valid, "{language} should be rejected");
            assert!(validation.error.unwrap().message.contains("language"));
        }
    }

    #[test]
    fn only_simplified_chinese_uses_chinese_native_copy() {
        assert_eq!(
            configured_comment_language(&LanguagePreference::from("zh-CN")),
            UiLanguage::ZhCn
        );
        for language in ["system", "zh-TW", "en-US", "ja-JP"] {
            assert_eq!(
                configured_comment_language(&LanguagePreference::from(language)),
                UiLanguage::EnUs
            );
        }
    }

    #[test]
    fn hide_when_not_playing_requires_boolean_value() {
        let validation = validate_config_draft(r#"{"overlay":{"hideWhenNotPlaying":"yes"}}"#);
        assert!(!validation.valid);
        assert!(validation
            .error
            .unwrap()
            .message
            .contains("hideWhenNotPlaying 必须是布尔值"));
    }

    #[test]
    fn shortcuts_require_modifiers_and_unique_combinations() {
        let missing_modifier =
            validate_config_draft(r#"{"app":{"shortcuts":{"toggleOverlay":"KeyL"}}}"#);
        assert!(!missing_modifier.valid);
        assert!(missing_modifier.error.unwrap().message.contains("修饰键"));

        let duplicate = validate_config_draft(
            r#"{"app":{"shortcuts":{"toggleOverlay":"Control+KeyL","unlockOverlay":"Control+KeyL"}}}"#,
        );
        assert!(!duplicate.valid);
        assert!(duplicate.error.unwrap().message.contains("不能重复"));
    }

    #[test]
    fn schema_five_migrates_compound_overlay_layouts() {
        for (legacy, layout, orientation) in [
            (
                "single",
                OverlayLayout::Single,
                OverlayOrientation::Horizontal,
            ),
            (
                "stacked",
                OverlayLayout::Double,
                OverlayOrientation::Horizontal,
            ),
            (
                "side_by_side",
                OverlayLayout::Double,
                OverlayOrientation::Horizontal,
            ),
            (
                "vertical_single",
                OverlayLayout::Single,
                OverlayOrientation::Vertical,
            ),
            (
                "vertical_double",
                OverlayLayout::Double,
                OverlayOrientation::Vertical,
            ),
        ] {
            let raw = format!(
                r#"{{"schemaVersion":5,"overlay":{{"appearance":{{"layout":"{legacy}"}}}}}}"#
            );
            let parsed = parse_config_draft(&raw).unwrap();
            assert!(parsed.migrated);
            assert_eq!(parsed.config.overlay.appearance.layout, layout);
            assert_eq!(parsed.config.overlay.appearance.orientation, orientation);
            assert!(parsed
                .normalized_json
                .contains(&format!("\"schemaVersion\": {CONFIG_SCHEMA_VERSION}")));
        }
    }

    #[test]
    fn current_schema_rejects_compound_overlay_layouts() {
        for layout in [
            "stacked",
            "side_by_side",
            "vertical_single",
            "vertical_double",
        ] {
            let raw = format!(
                r#"{{"schemaVersion":{CONFIG_SCHEMA_VERSION},"overlay":{{"appearance":{{"layout":"{layout}"}}}}}}"#
            );
            let validation = validate_config_draft(&raw);
            assert!(
                !validation.valid,
                "{layout} should be invalid in the current schema"
            );
            assert!(validation.error.unwrap().message.contains("orientation"));
        }
    }

    #[test]
    fn schema_four_migrates_legacy_provider_order_once() {
        let parsed = parse_config_draft(
            r#"{
              "schemaVersion": 4,
              "lyrics": { "providers": {
                "mode": "smart",
                "providers": [
                  { "id": "netease", "enabled": true },
                  { "id": "qqmusic", "enabled": true },
                  { "id": "kugou", "enabled": true },
                  { "id": "lrclib", "enabled": true }
                ]
              } }
            }"#,
        )
        .unwrap();

        assert!(parsed.migrated);
        assert_eq!(parsed.config.lyrics.providers, ProviderSettings::default());
    }

    #[test]
    fn current_schema_preserves_explicit_legacy_provider_order() {
        let parsed = parse_config_draft(
            r#"{
              "schemaVersion": 27,
              "lyrics": { "providers": {
                "mode": "smart",
                "providers": [
                  { "id": "netease", "enabled": true },
                  { "id": "qqmusic", "enabled": true },
                  { "id": "kugou", "enabled": true },
                  { "id": "lrclib", "enabled": true }
                ]
              } }
            }"#,
        )
        .unwrap();

        assert!(!parsed.migrated);
        assert_eq!(
            parsed
                .config
                .lyrics
                .providers
                .providers
                .iter()
                .map(|provider| provider.id.as_str())
                .collect::<Vec<_>>(),
            vec!["netease", "qqmusic", "kugou", "lrclib"]
        );
    }

    #[test]
    fn schema_sixteen_migrates_to_current_schema() {
        let parsed = parse_config_draft(r#"{"schemaVersion":16}"#).unwrap();
        assert!(parsed.migrated);
        assert_eq!(parsed.config.schema_version, CONFIG_SCHEMA_VERSION);
    }

    #[test]
    fn newer_schema_is_rejected() {
        let validation = validate_config_draft(r#"{"schemaVersion":99}"#);
        assert!(!validation.valid);
        assert!(validation.error.unwrap().message.contains("高于当前支持"));
    }

    #[test]
    fn oversized_schema_version_is_rejected_without_wrapping() {
        let validation = validate_config_draft(r#"{"schemaVersion":65538}"#);
        assert!(!validation.valid);
        assert!(validation.error.unwrap().message.contains("超出支持范围"));
    }

    #[test]
    fn invalid_types_enums_colors_and_ranges_report_the_field() {
        for (raw, field) in [
            (r#"{"app":{"hideDockIcon":"yes"}}"#, "hideDockIcon"),
            (r#"{"app":{"playerSelection":"music"}}"#, "playerSelection"),
            (
                r#"{"lyrics":{"providers":{"autoApplyThreshold":101}}}"#,
                "autoApplyThreshold",
            ),
            (
                r#"{"lyrics":{"providers":{"autoApplyThreshold":60.5}}}"#,
                "autoApplyThreshold",
            ),
            (
                r#"{"overlay":{"appearance":{"layout":"triple"}}}"#,
                "layout",
            ),
            (
                r#"{"overlay":{"appearance":{"orientation":"diagonal"}}}"#,
                "orientation",
            ),
            (
                r#"{"overlay":{"appearance":{"activeColor":"not a color"}}}"#,
                "activeColor",
            ),
        ] {
            let validation = validate_config_draft(raw);
            assert!(!validation.valid, "{raw} should be invalid");
            let error = validation.error.unwrap();
            assert!(error.message.contains(field), "{}", error.message);
            assert_eq!(error.line, 1);
            assert!(error.column > 1);
        }
    }

    #[test]
    fn duplicate_and_unknown_providers_are_rejected() {
        for raw in [
            r#"{"lyrics":{"providers":{"providers":[{"id":"lrclib","enabled":true},{"id":"lrclib","enabled":true}]}}}"#,
            r#"{"lyrics":{"providers":{"providers":[{"id":"unknown","enabled":true}]}}}"#,
        ] {
            let validation = validate_config_draft(raw);
            assert!(!validation.valid);
            assert!(validation.error.unwrap().message.contains("歌词源"));
        }
    }

    #[test]
    fn appearance_does_not_serialize_geometry() {
        let raw = serde_json::to_string(&AppConfig::default()).unwrap();
        assert!(!raw.contains("horizontalMaxWidth"));
        assert!(!raw.contains("verticalMaxHeight"));
    }

    #[test]
    fn schema_six_defaults_auto_center_to_disabled() {
        let parsed = parse_config_draft(r#"{"schemaVersion":6}"#).unwrap();
        assert!(parsed.migrated);
        assert!(
            !parsed
                .config
                .overlay
                .appearance
                .auto_center_with_translation_or_romanization
        );
        assert!(parsed
            .normalized_json
            .contains("\"autoCenterWithTranslationOrRomanization\": false"));
    }

    #[test]
    fn legacy_config_defaults_auto_apply_threshold_to_sixty() {
        let parsed = parse_config_draft(r#"{"schemaVersion":7}"#).unwrap();
        assert!(parsed.migrated);
        assert_eq!(parsed.config.lyrics.providers.auto_apply_threshold, 60);
        assert!(parsed
            .normalized_json
            .contains("\"autoApplyThreshold\": 60"));
    }

    #[test]
    fn auto_apply_threshold_round_trips_at_boundaries() {
        for threshold in [0, 100] {
            let raw =
                format!(r#"{{"lyrics":{{"providers":{{"autoApplyThreshold":{threshold}}}}}}}"#);
            let parsed = parse_config_draft(&raw).unwrap();
            assert_eq!(
                parsed.config.lyrics.providers.auto_apply_threshold,
                threshold
            );
        }
    }

    #[test]
    fn auto_center_preference_round_trips() {
        let parsed = parse_config_draft(
            r#"{"overlay":{"appearance":{"autoCenterWithTranslationOrRomanization":true}}}"#,
        )
        .unwrap();
        assert!(
            parsed
                .config
                .overlay
                .appearance
                .auto_center_with_translation_or_romanization
        );
        assert!(parsed
            .normalized_json
            .contains("\"autoCenterWithTranslationOrRomanization\": true"));
    }

    #[test]
    fn dock_icon_preference_round_trips() {
        let parsed = parse_config_draft(r#"{"app":{"hideDockIcon":true}}"#).unwrap();
        assert!(parsed.config.app.hide_dock_icon);
        assert!(parsed.normalized_json.contains("\"hideDockIcon\": true"));
    }

    #[test]
    fn silent_startup_migrates_round_trips_and_validates() {
        let migrated = parse_config_draft(r#"{"schemaVersion":21}"#).unwrap();
        assert!(migrated.migrated);
        assert!(!migrated.config.app.silent_startup);

        let enabled = parse_config_draft(r#"{"app":{"silentStartup":true}}"#).unwrap();
        assert!(enabled.config.app.silent_startup);
        assert!(enabled.normalized_json.contains("\"silentStartup\": true"));

        let invalid = validate_config_draft(r#"{"app":{"silentStartup":"yes"}}"#);
        assert!(!invalid.valid);
        assert!(invalid
            .error
            .unwrap()
            .message
            .contains("silentStartup 必须是布尔值"));
    }

    #[test]
    fn update_preference_migrates_round_trips_and_validates() {
        let migrated = parse_config_draft(r#"{"schemaVersion":14,"app":{}}"#).unwrap();
        assert!(migrated.migrated);
        assert!(migrated.config.app.auto_check_updates);

        let disabled = parse_config_draft(r#"{"app":{"autoCheckUpdates":false}}"#).unwrap();
        assert!(!disabled.config.app.auto_check_updates);
        assert!(disabled
            .normalized_json
            .contains("\"autoCheckUpdates\": false"));

        let invalid = match parse_config_draft(r#"{"app":{"autoCheckUpdates":"yes"}}"#) {
            Ok(_) => panic!("string update preference should be rejected"),
            Err(error) => error,
        };
        assert!(invalid.message.contains("autoCheckUpdates 必须是布尔值"));
    }

    #[test]
    fn failed_atomic_replace_keeps_in_memory_value() {
        let root = test_root("config-rollback");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("config.json")).unwrap();
        let store = ConfigStore {
            path: root.join("config.json"),
            state: RwLock::new(ConfigStoreState {
                value: AppConfig::default(),
                revision: 1,
                source_raw: "{}".into(),
                source_error: None,
                comment_language: UiLanguage::ZhCn,
            }),
        };
        let result = store.update(|config| config.app.theme = ThemePreference::Light);
        assert!(result.is_err());
        assert_eq!(store.snapshot().app.theme, ThemePreference::Dark);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_disk_config_uses_default_without_overwrite() {
        let root = test_root("invalid-disk-config");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("config.json"), "{ broken }").unwrap();
        let storage = Storage::open(root.clone(), root.join("library")).unwrap();
        let (store, migrated) = ConfigStore::load(&root, &storage).unwrap();
        assert!(!migrated);
        assert_eq!(store.snapshot().app.theme, ThemePreference::Dark);
        assert_eq!(
            fs::read_to_string(root.join("config.json")).unwrap(),
            "{ broken }"
        );
        assert!(!store.editor_data().validation.valid);
        let revision = store.revision();
        assert!(store.set_comment_language(UiLanguage::EnUs).unwrap());
        assert_eq!(store.revision(), revision + 1);
        assert_eq!(
            fs::read_to_string(root.join("config.json")).unwrap(),
            "{ broken }"
        );
        assert!(store
            .editor_data()
            .default_jsonc
            .contains("// Configuration schema version"));
        drop(storage);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn valid_disk_config_is_rewritten_as_complete_commented_jsonc() {
        let root = test_root("canonical-disk-config");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("config.json"), r#"{"app":{"theme":"light"}}"#).unwrap();
        let storage = Storage::open(root.clone(), root.join("library")).unwrap();
        let (store, migrated) = ConfigStore::load(&root, &storage).unwrap();
        assert!(!migrated);
        assert_eq!(store.snapshot().app.theme, ThemePreference::Light);
        let persisted = fs::read_to_string(root.join("config.json")).unwrap();
        assert!(persisted.contains("// Application theme"));
        assert!(persisted.contains("\"theme\": \"light\""));
        assert_eq!(persisted, store.editor_data().user_json);
        drop(storage);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn updates_keep_canonical_comments() {
        let root = test_root("commented-update");
        fs::create_dir_all(&root).unwrap();
        let storage = Storage::open(root.clone(), root.join("library")).unwrap();
        let (store, _) = ConfigStore::load(&root, &storage).unwrap();
        let revision = store.revision();
        let before_language_change = serde_json::to_value(store.snapshot()).unwrap();
        assert!(store.set_comment_language(UiLanguage::ZhCn).unwrap());
        assert_eq!(store.revision(), revision + 1);
        assert_eq!(
            serde_json::to_value(store.snapshot()).unwrap(),
            before_language_change
        );
        assert!(store.set_comment_language(UiLanguage::EnUs).unwrap());
        assert_eq!(store.revision(), revision + 2);
        assert!(!store.set_comment_language(UiLanguage::EnUs).unwrap());
        assert_eq!(store.revision(), revision + 2);
        store
            .update(|config| config.app.theme = ThemePreference::Light)
            .unwrap();
        let persisted = fs::read_to_string(root.join("config.json")).unwrap();
        assert!(persisted.contains("// Application theme"));
        assert!(persisted.contains("\"theme\": \"light\""));
        assert!(store
            .export_json()
            .unwrap()
            .contains("// Application theme"));
        drop(storage);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn revision_conflict_does_not_overwrite_newer_config() {
        let root = test_root("revision-conflict");
        fs::create_dir_all(&root).unwrap();
        let storage = Storage::open(root.clone(), root.join("library")).unwrap();
        let (store, _) = ConfigStore::load(&root, &storage).unwrap();
        let revision = store.revision();
        store
            .update(|config| config.app.theme = ThemePreference::Light)
            .unwrap();
        let mut stale = store.snapshot();
        stale.app.theme = ThemePreference::Dark;
        let error = store.replace_at_revision(stale, revision).unwrap_err();
        assert!(error.starts_with("config.conflict:"));
        assert_eq!(store.snapshot().app.theme, ThemePreference::Light);
        drop(storage);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn system_media_applications_validate_deduplicate_and_round_trip() {
        let parsed = parse_config_draft(
            r#"{"app":{"systemMediaApplications":[{"name":"Player","bundleId":"org.example.Player"},{"name":"Duplicate","bundleId":"org.example.Player"}]}}"#,
        )
        .unwrap();
        assert_eq!(parsed.config.app.system_media_applications.len(), 1);
        assert_eq!(
            parsed.config.app.system_media_applications[0].bundle_id,
            "org.example.Player"
        );
        assert!(parse_config_draft(
            r#"{"app":{"systemMediaApplications":[{"name":"Broken","bundleId":""}]}}"#
        )
        .is_err());
        assert!(parse_config_draft(
            r#"{"app":{"systemMediaApplications":[{"name":"Broken","bundleId":"not valid"}]}}"#
        )
        .is_err());
        for bundle_id in ["com.apple.Music", "com.spotify.client"] {
            let raw = format!(
                r#"{{"app":{{"systemMediaApplications":[{{"name":"Dedicated","bundleId":"{bundle_id}"}}]}}}}"#
            );
            assert!(parse_config_draft(&raw).is_err());
        }
        assert!(parse_config_draft(r#"{"app":{"systemMediaApplications":{}}}"#).is_err());
        let multiple = parse_config_draft(
            r#"{"app":{"systemMediaApplications":[{"name":"First","bundleId":"org.example.First"},{"name":"Second","bundleId":"org.example.Second"}]}}"#,
        )
        .unwrap();
        assert_eq!(multiple.config.app.system_media_applications.len(), 2);

        let migrated = parse_config_draft(
            r#"{"schemaVersion":22,"app":{"systemMediaApplications":[{"name":"First","bundleId":"org.example.First"},{"name":"Second","bundleId":"org.example.Second"}]}}"#,
        )
        .unwrap();
        assert_eq!(migrated.config.app.system_media_applications.len(), 2);
        assert_eq!(migrated.config.app.player_follower_application, None);
    }

    #[test]
    fn player_follower_is_a_separate_optional_application() {
        for bundle_id in [
            "com.apple.Music",
            "com.spotify.client",
            "org.example.Player",
        ] {
            let raw = format!(
                r#"{{"app":{{"playerFollowerApplication":{{"name":"Player","bundleId":"{bundle_id}"}}}}}}"#
            );
            let parsed = parse_config_draft(&raw).unwrap();
            assert_eq!(
                parsed
                    .config
                    .app
                    .player_follower_application
                    .as_ref()
                    .map(|application| application.bundle_id.as_str()),
                Some(bundle_id)
            );
        }
        assert!(parse_config_draft(
            r#"{"app":{"playerFollowerApplication":{"name":"Broken","bundleId":""}}}"#
        )
        .is_err());
        assert!(parse_config_draft(r#"{"app":{"playerFollowerApplication":[]}}"#).is_err());
    }
}
