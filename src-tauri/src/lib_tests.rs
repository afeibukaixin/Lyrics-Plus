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
