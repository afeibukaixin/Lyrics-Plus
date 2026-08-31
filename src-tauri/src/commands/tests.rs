#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_style_deserializes_old_saved_shape_with_defaults() {
        let style: OverlayStyleSettings = serde_json::from_str(
            r##"{"fontSize":42,"activeColor":"#ff0000","inactiveColor":"#888888","opacity":0.7}"##,
        )
        .unwrap();
        let style = style.normalized();
        assert_eq!(style.font_size, 42);
        assert_eq!(style.background, OverlayBackground::Glass);
        assert_eq!(style.background_blur, 18.0);
        assert_eq!(style.layout, OverlayLayout::Single);
        assert_eq!(style.orientation, OverlayOrientation::Horizontal);
        assert_eq!(style.secondary_display, SecondaryDisplayMode::Translation);
        assert!(!style.auto_center_with_translation_or_romanization);
    }

    #[test]
    fn legacy_transparent_background_normalizes_to_transparent_mode() {
        let style: OverlayStyleSettings =
            serde_json::from_str(r##"{"background":"transparent","backgroundOpacity":0.75}"##)
                .unwrap();
        let style = style.normalized();
        assert_eq!(style.background, OverlayBackground::Solid);
        assert_eq!(style.background_mode, OverlayBackgroundMode::Transparent);
        assert_eq!(style.background_opacity, 0.75);
    }

    #[test]
    fn new_overlay_style_defaults_to_current_secondary_display() {
        assert_eq!(OverlayStyleSettings::default().background_opacity, 0.6);
        assert_eq!(
            OverlayStyleSettings::default().background_mode,
            OverlayBackgroundMode::Solid
        );
        assert_eq!(
            OverlayStyleSettings::default().secondary_display,
            SecondaryDisplayMode::TranslationRomanization
        );
        assert_eq!(OverlayStyleSettings::default().secondary_font_scale, 1.0);
        assert!(!OverlayStyleSettings::default().auto_center_with_translation_or_romanization);
    }

    #[test]
    fn legacy_fill_migrates_and_manual_bounds_are_restored() {
        let style: OverlayStyleSettings = serde_json::from_str(
            r##"{"karaokeStyle":"fill","horizontalMaxWidth":640,"verticalMaxHeight":480}"##,
        )
        .unwrap();
        let style = style.normalized();
        assert_eq!(style.karaoke_style, KaraokeStyle::Sweep);
        assert_eq!(style.horizontal_max_width, Some(640.0));
        assert_eq!(style.vertical_max_height, Some(480.0));
        let serialized = serde_json::to_string(&style).unwrap();
        assert!(serialized.contains(r#""karaokeStyle":"sweep""#));
        assert!(serialized.contains(r#""horizontalMaxWidth":640.0"#));
        assert!(serialized.contains(r#""verticalMaxHeight":480.0"#));
    }

    #[test]
    fn reset_bounds_response_clears_only_manual_axes() {
        let mut style = OverlayStyleSettings {
            font_size: 46,
            horizontal_max_width: Some(920.0),
            vertical_max_height: Some(700.0),
            ..OverlayStyleSettings::default()
        };
        clear_manual_overlay_bounds(&mut style);

        assert_eq!(style.font_size, 46);
        assert_eq!(style.horizontal_max_width, None);
        assert_eq!(style.vertical_max_height, None);
        let response = serde_json::to_value(style).unwrap();
        assert_eq!(response["horizontalMaxWidth"], serde_json::Value::Null);
        assert_eq!(response["verticalMaxHeight"], serde_json::Value::Null);
    }

    #[test]
    fn reset_bounds_restores_horizontal_width_and_preserves_height() {
        assert_eq!(
            reset_overlay_dimensions(OverlayOrientation::Horizontal, 920.0, 184.0),
            (760.0, 184.0)
        );
        assert_eq!(
            reset_overlay_dimensions(OverlayOrientation::Horizontal, 920.0, 40.0),
            (760.0, 76.0)
        );
    }

    #[test]
    fn reset_bounds_restores_vertical_height_and_preserves_width() {
        assert_eq!(
            reset_overlay_dimensions(OverlayOrientation::Vertical, 260.0, 780.0),
            (260.0, 620.0)
        );
        assert_eq!(
            reset_overlay_dimensions(OverlayOrientation::Vertical, 120.0, 780.0),
            (190.0, 620.0)
        );
    }

    #[test]
    fn old_romanization_only_style_migrates_to_romanization_mode() {
        let style: OverlayStyleSettings =
            serde_json::from_str(r##"{"translationEnabled":false,"romanizationEnabled":true}"##)
                .unwrap();
        assert_eq!(
            style.normalized().secondary_display,
            SecondaryDisplayMode::Romanization
        );
    }

    fn search_result(score: f64, translation: bool, romanization: bool) -> LyricsSearchResult {
        LyricsSearchResult {
            id: format!("{score}"),
            provider_id: "test".into(),
            title: "歌曲".into(),
            artist: "歌手".into(),
            album: None,
            duration_ms: None,
            source: "测试".into(),
            synced: true,
            has_translation: translation,
            has_word_timing: false,
            has_romanization: romanization,
            score,
            lyrics: "[00:01.00]歌词".into(),
        }
    }

    #[test]
    fn translation_preference_wins_inside_quality_window() {
        let mut results = vec![
            search_result(0.91, false, false),
            search_result(0.88, true, false),
        ];
        prefer_candidate_capabilities(&mut results, SecondaryDisplayMode::Translation);
        assert!(results[0].has_translation);
    }

    #[test]
    fn translation_preference_does_not_cross_quality_window() {
        let mut results = vec![
            search_result(0.91, false, false),
            search_result(0.86, true, false),
        ];
        prefer_candidate_capabilities(&mut results, SecondaryDisplayMode::Translation);
        assert!(!results[0].has_translation);
    }

    #[test]
    fn combined_mode_prefers_results_with_both_tracks() {
        let mut results = vec![
            search_result(0.92, false, false),
            search_result(0.91, true, false),
            search_result(0.90, true, true),
        ];
        prefer_candidate_capabilities(&mut results, SecondaryDisplayMode::TranslationRomanization);
        assert!(results[0].has_translation && results[0].has_romanization);
    }

    fn word_timed_result(score: f64, word_timing: bool, translation: bool) -> LyricsSearchResult {
        let mut result = search_result(score, translation, false);
        result.has_word_timing = word_timing;
        result
    }

    #[test]
    fn word_timing_wins_inside_quality_window() {
        let mut results = vec![
            word_timed_result(0.91, false, false),
            word_timed_result(0.88, true, false),
        ];
        prefer_candidate_capabilities(&mut results, SecondaryDisplayMode::Next);
        assert!(results[0].has_word_timing);
    }

    #[test]
    fn word_timing_does_not_cross_quality_window() {
        let mut results = vec![
            word_timed_result(0.91, false, false),
            word_timed_result(0.86, true, false),
        ];
        prefer_candidate_capabilities(&mut results, SecondaryDisplayMode::Next);
        assert!(!results[0].has_word_timing);
    }

    #[test]
    fn translation_precedes_word_timing_for_translation_display() {
        let mut results = vec![
            word_timed_result(0.92, false, true),
            word_timed_result(0.91, true, false),
        ];
        prefer_candidate_capabilities(&mut results, SecondaryDisplayMode::Translation);
        assert!(results[0].has_translation);
    }

    #[test]
    fn auxiliary_preference_breaks_word_timing_ties() {
        let mut results = vec![
            word_timed_result(0.92, true, false),
            word_timed_result(0.91, true, true),
        ];
        prefer_candidate_capabilities(&mut results, SecondaryDisplayMode::Translation);
        assert!(results[0].has_translation);
    }

    #[test]
    fn overlay_style_normalizes_unsafe_ranges_and_empty_colors() {
        let style = OverlayStyleSettings {
            font_size: 200,
            active_color: String::new(),
            inactive_color: String::new(),
            opacity: -1.0,
            background_opacity: 2.0,
            background_blur: 100.0,
            secondary_font_scale: 0.1,
            solid_color: String::new(),
            horizontal_max_width: Some(80.0),
            vertical_max_height: Some(120.0),
            ..OverlayStyleSettings::default()
        }
        .normalized();
        assert_eq!(style.font_size, 72);
        assert_eq!(style.opacity, 0.2);
        assert_eq!(style.background_opacity, 1.0);
        assert_eq!(style.background_blur, 40.0);
        assert_eq!(style.secondary_font_scale, 0.35);
        assert_eq!(style.active_color, "#a3e635");
        assert_eq!(style.solid_color, "#171821");
        assert_eq!(style.horizontal_max_width, Some(320.0));
        assert_eq!(style.vertical_max_height, Some(280.0));
    }

    #[test]
    fn overlay_layout_and_orientation_serialize_independently() {
        for (layout, orientation, expected_layout, expected_orientation) in [
            (
                OverlayLayout::Single,
                OverlayOrientation::Horizontal,
                "single",
                "horizontal",
            ),
            (
                OverlayLayout::Double,
                OverlayOrientation::Horizontal,
                "double",
                "horizontal",
            ),
            (
                OverlayLayout::Single,
                OverlayOrientation::Vertical,
                "single",
                "vertical",
            ),
            (
                OverlayLayout::Double,
                OverlayOrientation::Vertical,
                "double",
                "vertical",
            ),
        ] {
            let style = OverlayStyleSettings {
                layout,
                orientation,
                ..OverlayStyleSettings::default()
            };
            let value = serde_json::to_value(style).unwrap();
            assert_eq!(value["layout"], expected_layout);
            assert_eq!(value["orientation"], expected_orientation);
        }
    }

    #[test]
    fn adaptive_bounds_keep_the_user_position_stable() {
        let (position, size) = fit_overlay_bounds(
            tauri::PhysicalPosition::new(500, 300),
            600.0,
            300.0,
            1.0,
            tauri::PhysicalPosition::new(0, 0),
            tauri::PhysicalSize::new(1920, 1080),
        );
        assert_eq!(size, tauri::PhysicalSize::new(600, 300));
        assert_eq!(position, tauri::PhysicalPosition::new(500, 300));
    }

    #[test]
    fn adaptive_bounds_allow_monitor_edges_and_respect_minimums() {
        let (large_position, large_size) = fit_overlay_bounds(
            tauri::PhysicalPosition::new(300, 200),
            2_000.0,
            2_000.0,
            1.0,
            tauri::PhysicalPosition::new(0, 0),
            tauri::PhysicalSize::new(1_000, 800),
        );
        assert_eq!(large_size, tauri::PhysicalSize::new(1_000, 800));
        assert_eq!(large_position, tauri::PhysicalPosition::new(0, 0));

        let (_, small_size) = fit_overlay_bounds(
            tauri::PhysicalPosition::new(300, 200),
            10.0,
            10.0,
            1.0,
            tauri::PhysicalPosition::new(0, 0),
            tauri::PhysicalSize::new(1_000, 800),
        );
        assert_eq!(small_size, tauri::PhysicalSize::new(190, 76));
    }

    #[test]
    fn manual_edge_resize_keeps_the_opposite_edge_anchored() {
        let monitor_position = tauri::PhysicalPosition::new(0, 0);
        let monitor_size = tauri::PhysicalSize::new(2880, 1800);
        let horizontal_position = tauri::PhysicalPosition::new(400, 500);
        let horizontal_size = tauri::PhysicalSize::new(800, 600);
        let (left_position, left_size) = resize_overlay_edge_bounds(
            horizontal_position,
            horizontal_size,
            OverlayResizeEdge::Left,
            500.0,
            0.0,
            2.0,
            monitor_position,
            monitor_size,
        );
        assert_eq!(left_position.x as i64 + left_size.width as i64, 1200);
        assert_eq!(left_size.width, 1000);
        let (right_position, right_size) = resize_overlay_edge_bounds(
            horizontal_position,
            horizontal_size,
            OverlayResizeEdge::Right,
            500.0,
            0.0,
            2.0,
            monitor_position,
            monitor_size,
        );
        assert_eq!(right_position.x, horizontal_position.x);
        assert_eq!(right_size.width, 1000);

        let vertical_position = tauri::PhysicalPosition::new(400, 500);
        let vertical_size = tauri::PhysicalSize::new(800, 1200);
        let (top_position, top_size) = resize_overlay_edge_bounds(
            vertical_position,
            vertical_size,
            OverlayResizeEdge::Top,
            400.0,
            0.0,
            2.0,
            monitor_position,
            monitor_size,
        );
        assert_eq!(top_position.y as i64 + top_size.height as i64, 1700);
        assert_eq!(top_size.height, 800);
        let (bottom_position, bottom_size) = resize_overlay_edge_bounds(
            vertical_position,
            vertical_size,
            OverlayResizeEdge::Bottom,
            400.0,
            0.0,
            2.0,
            monitor_position,
            monitor_size,
        );
        assert_eq!(bottom_position.y, vertical_position.y);
        assert_eq!(bottom_size.height, 800);
    }

    #[test]
    fn manual_edge_resize_respects_minimums_and_monitor_edges() {
        let position = tauri::PhysicalPosition::new(400, 300);
        let size = tauri::PhysicalSize::new(800, 700);
        let monitor_position = tauri::PhysicalPosition::new(0, 0);
        let monitor_size = tauri::PhysicalSize::new(2880, 1800);
        let (_, minimum) = resize_overlay_edge_bounds(
            position,
            size,
            OverlayResizeEdge::Right,
            10.0,
            0.0,
            2.0,
            monitor_position,
            monitor_size,
        );
        assert_eq!(minimum.width, 640);
        let (_, maximum) = resize_overlay_edge_bounds(
            position,
            size,
            OverlayResizeEdge::Right,
            10_000.0,
            0.0,
            2.0,
            monitor_position,
            monitor_size,
        );
        assert_eq!(position.x as i64 + maximum.width as i64, 2880);
    }

    #[test]
    fn manual_edge_resize_respects_toolbar_minimums() {
        let position = tauri::PhysicalPosition::new(400, 300);
        let size = tauri::PhysicalSize::new(800, 900);
        let monitor_position = tauri::PhysicalPosition::new(0, 0);
        let monitor_size = tauri::PhysicalSize::new(2880, 1800);
        let (_, horizontal) = resize_overlay_edge_bounds(
            position,
            size,
            OverlayResizeEdge::Right,
            10.0,
            380.0,
            2.0,
            monitor_position,
            monitor_size,
        );
        assert_eq!(horizontal.width, 760);

        let (_, vertical) = resize_overlay_edge_bounds(
            position,
            size,
            OverlayResizeEdge::Bottom,
            10.0,
            360.0,
            2.0,
            monitor_position,
            monitor_size,
        );
        assert_eq!(vertical.height, 720);
    }

    #[test]
    fn content_fit_cannot_override_the_fixed_layout_axis() {
        let horizontal = OverlayStyleSettings {
            layout: OverlayLayout::Double,
            orientation: OverlayOrientation::Horizontal,
            horizontal_max_width: Some(540.0),
            ..OverlayStyleSettings::default()
        };
        assert_eq!(
            fixed_axis_content_size(&horizontal, 1200.0, 180.0, 320.0, 280.0, false),
            (540.0, 180.0)
        );
        assert_eq!(
            fixed_axis_content_size(&horizontal, 1200.0, 180.0, 320.0, 280.0, true),
            (320.0, 180.0)
        );

        let vertical = OverlayStyleSettings {
            layout: OverlayLayout::Double,
            orientation: OverlayOrientation::Vertical,
            vertical_max_height: Some(430.0),
            ..OverlayStyleSettings::default()
        };
        assert_eq!(
            fixed_axis_content_size(&vertical, 220.0, 900.0, 320.0, 280.0, false),
            (220.0, 430.0)
        );
        assert_eq!(
            fixed_axis_content_size(&vertical, 220.0, 900.0, 320.0, 280.0, true),
            (220.0, 280.0)
        );
    }

    #[test]
    fn legacy_edge_alignments_migrate_to_distributed() {
        let left: OverlayAlignment = serde_json::from_str(r#""left""#).unwrap();
        let right: OverlayAlignment = serde_json::from_str(r#""right""#).unwrap();
        assert_eq!(left, OverlayAlignment::Distributed);
        assert_eq!(right, OverlayAlignment::Distributed);
        assert_eq!(
            serde_json::to_string(&OverlayAlignment::Distributed).unwrap(),
            r#""distributed""#
        );
    }

    #[test]
    fn resolves_a_macos_application_bundle() {
        let root =
            std::env::temp_dir().join(format!("lyrics-plus-app-resolver-{}", std::process::id()));
        let application = root.join("Example.app");
        std::fs::create_dir_all(application.join("Contents")).unwrap();
        std::fs::write(
            application.join("Contents/Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>org.example.Player</string><key>CFBundleName</key><string>Example Player</string></dict></plist>"#,
        )
        .unwrap();
        let resolved = resolve_registered_application(&application).unwrap();
        assert_eq!(resolved.bundle_id, "org.example.Player");
        assert_eq!(resolved.name, "Example Player");
        assert_eq!(
            resolve_system_media_applications(vec![application.clone(), application.clone()])
                .unwrap()
                .len(),
            1
        );
        assert!(resolve_registered_application(&root).is_err());
        let missing_plist = root.join("Missing.app");
        std::fs::create_dir_all(&missing_plist).unwrap();
        assert!(resolve_registered_application(&missing_plist).is_err());
        let missing_bundle_id = root.join("NoId.app/Contents");
        std::fs::create_dir_all(&missing_bundle_id).unwrap();
        std::fs::write(
            missing_bundle_id.join("Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleName</key><string>No ID</string></dict></plist>"#,
        )
        .unwrap();
        assert!(resolve_registered_application(&root.join("NoId.app")).is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn application_icons_are_small_pngs_and_skip_missing_apps() {
        use base64::Engine as _;

        let icon = application_icon_at_path("/System/Applications/Music.app")
            .expect("Music.app should have a readable native icon");
        let icons = collect_application_icons(vec!["invalid.lyrics-plus.icon-test".into()]);
        let png = base64::engine::general_purpose::STANDARD
            .decode(icon.split_once(',').unwrap().1)
            .unwrap();

        assert!(icon.starts_with("data:image/png;base64,iVBORw0KGgo"));
        assert_eq!(&png[16..24], &[0, 0, 0, 64, 0, 0, 0, 64]);
        assert!(icons.is_empty());
    }
}
