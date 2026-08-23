#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silent_startup_hides_only_after_accepting_the_notice() {
        assert!(should_show_main_window(false, true));
        assert!(should_show_main_window(true, false));
        assert!(!should_show_main_window(true, true));
    }

    #[test]
    fn overlay_initial_size_restores_the_saved_fixed_axis() {
        let horizontal = OverlayStyleSettings {
            horizontal_max_width: Some(540.0),
            ..OverlayStyleSettings::default()
        };
        assert_eq!(initial_overlay_dimensions(&horizontal), (540.0, 156.0));

        let vertical = OverlayStyleSettings {
            orientation: OverlayOrientation::Vertical,
            vertical_max_height: Some(480.0),
            ..OverlayStyleSettings::default()
        };
        assert_eq!(initial_overlay_dimensions(&vertical), (190.0, 480.0));
    }

    #[test]
    fn overlay_initial_size_uses_orientation_defaults_without_saved_geometry() {
        assert_eq!(
            initial_overlay_dimensions(&OverlayStyleSettings::default()),
            (760.0, 156.0)
        );
        let vertical = OverlayStyleSettings {
            orientation: OverlayOrientation::Vertical,
            ..OverlayStyleSettings::default()
        };
        assert_eq!(initial_overlay_dimensions(&vertical), (190.0, 620.0));
    }

    #[test]
    fn edge_snap_only_applies_inside_threshold() {
        assert_eq!(snap_coordinate(8, 0, 100), 0);
        assert_eq!(snap_coordinate(91, 0, 100), 100);
        assert_eq!(snap_coordinate(50, 0, 100), 50);
    }

    #[test]
    fn toolbar_placement_stays_until_opposite_edge() {
        use OverlayOrientation::{Horizontal, Vertical};
        use ToolbarPlacement::{Bottom, Left, Right, Top};

        let point = tauri::PhysicalPosition::new;
        let moved = |orientation, placement, x, y| {
            toolbar_placement_after_move(
                orientation,
                placement,
                point(x, y),
                tauri::PhysicalSize::new(300, 100),
                1.0,
                point(100, 200),
                tauri::PhysicalSize::new(1_200, 800),
            )
        };

        assert_eq!(moved(Horizontal, Top, 500, 205), (Bottom, point(500, 251)),);
        assert_eq!(
            moved(Horizontal, Bottom, 500, 500),
            (Bottom, point(500, 500)),
        );
        assert_eq!(moved(Horizontal, Bottom, 500, 890), (Top, point(500, 844)),);
        assert_eq!(moved(Vertical, Right, 995, 500), (Left, point(947, 500)),);
        assert_eq!(moved(Vertical, Left, 500, 500), (Left, point(500, 500)),);
        assert_eq!(moved(Vertical, Left, 105, 500), (Right, point(153, 500)),);
    }

    #[test]
    fn overlay_hover_is_frozen_while_primary_button_is_pressed() {
        assert!(stable_overlay_hover(Some(true), false, true));
        assert!(!stable_overlay_hover(Some(false), true, true));
        assert!(stable_overlay_hover(None, true, true));
        assert!(!stable_overlay_hover(Some(true), false, false));
    }

    #[test]
    fn old_position_records_remain_compatible() {
        let bounds: StoredBounds =
            serde_json::from_str(r#"{"x":12,"y":34,"width":760,"height":156}"#).unwrap();
        assert_eq!((bounds.x, bounds.y), (12, 34));
        assert_eq!(bounds.relative_x, None);
        assert_eq!(bounds.work_width, None);
        assert_eq!(bounds.toolbar_placement, None);
    }

    #[test]
    fn old_position_records_are_clamped_to_the_current_work_area() {
        let bounds: StoredBounds = serde_json::from_str(r#"{"x":900,"y":700}"#).unwrap();
        assert_eq!(
            restored_overlay_position(
                &bounds,
                tauri::PhysicalPosition::new(0, 0),
                tauri::PhysicalSize::new(800, 600),
                tauri::PhysicalSize::new(200, 100),
                2.0,
            ),
            tauri::PhysicalPosition::new(600, 500),
        );
    }

    #[test]
    fn relative_position_adapts_to_resolution_and_monitor_origin_changes() {
        let bounds: StoredBounds = serde_json::from_str(
            r#"{"x":400,"y":200,"workX":0,"workY":0,"workWidth":1000,"workHeight":800,"scaleFactor":2.0,"relativeX":0.5,"relativeY":0.25}"#,
        )
        .unwrap();
        assert_eq!(
            restored_overlay_position(
                &bounds,
                tauri::PhysicalPosition::new(1920, 0),
                tauri::PhysicalSize::new(2000, 1200),
                tauri::PhysicalSize::new(200, 100),
                1.0,
            ),
            tauri::PhysicalPosition::new(2820, 275),
        );
    }

    #[test]
    fn unchanged_work_area_preserves_exact_saved_position() {
        let bounds: StoredBounds = serde_json::from_str(
            r#"{"x":321,"y":234,"workX":0,"workY":24,"workWidth":1440,"workHeight":876,"scaleFactor":2.0,"relativeX":0.1,"relativeY":0.9}"#,
        )
        .unwrap();
        assert_eq!(
            restored_overlay_position(
                &bounds,
                tauri::PhysicalPosition::new(0, 24),
                tauri::PhysicalSize::new(1440, 876),
                tauri::PhysicalSize::new(760, 156),
                2.0,
            ),
            tauri::PhysicalPosition::new(321, 234),
        );
    }

    #[test]
    fn main_window_is_centered_inside_negative_origin_work_area() {
        assert_eq!(
            centered_position(
                tauri::PhysicalPosition::new(-1920, 24),
                tauri::PhysicalSize::new(1920, 1056),
                tauri::PhysicalSize::new(980, 720),
            ),
            tauri::PhysicalPosition::new(-1450, 192),
        );
    }

    #[test]
    fn topology_changes_preserve_preferred_monitor_and_clear_programmatic_move() {
        let topology = |width| {
            vec![MonitorTopologyEntry {
                id: "external".into(),
                x: 0,
                y: 0,
                width,
                height: 1080,
                work_x: 0,
                work_y: 24,
                work_width: width,
                work_height: 1056,
                scale_factor_bits: 1.0_f64.to_bits(),
            }]
        };
        let mut placement = OverlayPlacementState {
            preferred_monitor: Some("external".into()),
            expected_programmatic_position: Some(tauri::PhysicalPosition::new(10, 20)),
            ..OverlayPlacementState::default()
        };
        assert!(!placement.update_topology(topology(1920)));
        assert!(placement.consume_programmatic_move(tauri::PhysicalPosition::new(10, 20)));
        placement.expected_programmatic_position = Some(tauri::PhysicalPosition::new(30, 40));
        placement.programmatic_move_started_at = Some(Instant::now());
        assert!(placement.update_topology(topology(2560)));
        assert_eq!(placement.preferred_monitor.as_deref(), Some("external"));
        assert_eq!(placement.expected_programmatic_position, None);
        assert_eq!(placement.programmatic_move_started_at, None);
    }

    #[test]
    fn programmatic_move_suppression_expires() {
        let now = Instant::now();
        let mut placement = OverlayPlacementState {
            expected_programmatic_position: Some(tauri::PhysicalPosition::new(10, 20)),
            programmatic_move_started_at: Some(
                now - PROGRAMMATIC_MOVE_SUPPRESSION - Duration::from_millis(1),
            ),
            ..OverlayPlacementState::default()
        };
        assert!(!placement.suppress_persistence(now));
        assert_eq!(placement.expected_programmatic_position, None);
    }

    #[test]
    fn horizontal_unlock_handle_is_centered_at_the_top() {
        let overlay_position = tauri::PhysicalPosition::new(100, 200);
        let overlay_size = tauri::PhysicalSize::new(760, 156);
        let handle_size = tauri::PhysicalSize::new(28, 28);
        assert_eq!(
            unlock_handle_position(
                ToolbarPlacement::Top,
                overlay_position,
                overlay_size,
                handle_size,
                46,
                6,
            ),
            tauri::PhysicalPosition::new(466, 212),
        );
        assert_eq!(
            unlock_handle_position(
                ToolbarPlacement::Bottom,
                overlay_position,
                overlay_size,
                handle_size,
                46,
                6,
            ),
            tauri::PhysicalPosition::new(466, 316),
        );
    }

    #[test]
    fn vertical_unlock_handle_is_centered_at_the_right() {
        let overlay_position = tauri::PhysicalPosition::new(100, 200);
        let overlay_size = tauri::PhysicalSize::new(190, 620);
        let handle_size = tauri::PhysicalSize::new(28, 28);
        assert_eq!(
            unlock_handle_position(
                ToolbarPlacement::Right,
                overlay_position,
                overlay_size,
                handle_size,
                48,
                6,
            ),
            tauri::PhysicalPosition::new(248, 496),
        );
        assert_eq!(
            unlock_handle_position(
                ToolbarPlacement::Left,
                overlay_position,
                overlay_size,
                handle_size,
                48,
                6,
            ),
            tauri::PhysicalPosition::new(114, 496),
        );
    }

    #[test]
    fn toolbar_flip_compensates_position_and_uses_hysteresis() {
        let work_position = tauri::PhysicalPosition::new(0, 25);
        let work_size = tauri::PhysicalSize::new(1920, 1055);
        let horizontal_size = tauri::PhysicalSize::new(760, 156);
        let (placement, position) = toolbar_placement_after_move(
            OverlayOrientation::Horizontal,
            ToolbarPlacement::Top,
            tauri::PhysicalPosition::new(300, 25),
            horizontal_size,
            1.0,
            work_position,
            work_size,
        );
        assert_eq!(placement, ToolbarPlacement::Bottom);
        assert_eq!(position, tauri::PhysicalPosition::new(300, 71));
        assert_eq!(
            toolbar_placement_after_move(
                OverlayOrientation::Horizontal,
                placement,
                position,
                horizontal_size,
                1.0,
                work_position,
                work_size,
            ),
            (placement, position),
        );
        assert_eq!(
            toolbar_placement_after_move(
                OverlayOrientation::Horizontal,
                placement,
                tauri::PhysicalPosition::new(300, 84),
                horizontal_size,
                1.0,
                work_position,
                work_size,
            ),
            (placement, tauri::PhysicalPosition::new(300, 84)),
        );

        let vertical_size = tauri::PhysicalSize::new(380, 1240);
        let (placement, position) = toolbar_placement_after_move(
            OverlayOrientation::Vertical,
            ToolbarPlacement::Right,
            tauri::PhysicalPosition::new(1540, 100),
            vertical_size,
            2.0,
            tauri::PhysicalPosition::new(0, 0),
            tauri::PhysicalSize::new(1920, 2160),
        );
        assert_eq!(placement, ToolbarPlacement::Left);
        assert_eq!(position, tauri::PhysicalPosition::new(1444, 100));
        assert_eq!(
            toolbar_placement_after_move(
                OverlayOrientation::Vertical,
                placement,
                tauri::PhysicalPosition::new(1431, 100),
                vertical_size,
                2.0,
                tauri::PhysicalPosition::new(0, 0),
                tauri::PhysicalSize::new(1920, 2160),
            ),
            (placement, tauri::PhysicalPosition::new(1431, 100)),
        );
    }

    #[test]
    fn point_in_window_bounds_uses_exclusive_right_and_bottom_edges() {
        let position = tauri::PhysicalPosition::new(100, 200);
        let size = tauri::PhysicalSize::new(28, 28);
        assert!(point_in_window_bounds(
            tauri::PhysicalPosition::new(100.0, 200.0),
            position,
            size,
        ));
        assert!(point_in_window_bounds(
            tauri::PhysicalPosition::new(127.9, 227.9),
            position,
            size,
        ));
        assert!(!point_in_window_bounds(
            tauri::PhysicalPosition::new(128.0, 228.0),
            position,
            size,
        ));
    }

    #[test]
    fn overlay_hover_requires_visible_unlocked_overlay() {
        let cursor = tauri::PhysicalPosition::new(110.0, 210.0);
        let position = tauri::PhysicalPosition::new(100, 200);
        let size = tauri::PhysicalSize::new(28, 28);
        let mut settings = OverlaySettings::default();

        assert!(should_hover_overlay(&settings, cursor, position, size));

        settings.visible = false;
        assert!(!should_hover_overlay(&settings, cursor, position, size));

        settings.visible = true;
        settings.locked = true;
        assert!(!should_hover_overlay(&settings, cursor, position, size));
    }

    #[test]
    fn overlay_visibility_respects_preference_and_playback_state() {
        assert!(!should_show_overlay(false, false, false));
        assert!(!should_show_overlay(false, false, true));
        assert!(!should_show_overlay(false, true, false));
        assert!(!should_show_overlay(false, true, true));
        assert!(should_show_overlay(true, false, false));
        assert!(should_show_overlay(true, false, true));
        assert!(!should_show_overlay(true, true, false));
        assert!(should_show_overlay(true, true, true));
    }

    #[test]
    fn overlay_hover_uses_window_bounds() {
        let settings = OverlaySettings::default();
        let position = tauri::PhysicalPosition::new(100, 200);
        let size = tauri::PhysicalSize::new(28, 28);

        assert!(should_hover_overlay(
            &settings,
            tauri::PhysicalPosition::new(127.9, 227.9),
            position,
            size,
        ));
        assert!(!should_hover_overlay(
            &settings,
            tauri::PhysicalPosition::new(128.0, 228.0),
            position,
            size,
        ));
    }
}
