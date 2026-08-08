use crate::commands::{
    OverlayBackground, OverlayBackgroundMode, OverlayOrientation, OverlayStyleSettings,
};

pub(crate) const HORIZONTAL_OVERLAY_SURFACE_INSET: f64 = 46.0;
pub(crate) const VERTICAL_OVERLAY_SURFACE_INSET: f64 = 48.0;
const OVERLAY_SURFACE_RADIUS: f64 = 18.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct SurfaceFrame {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn overlay_surface_frame(width: f64, height: f64, orientation: OverlayOrientation) -> SurfaceFrame {
    match orientation {
        OverlayOrientation::Horizontal => SurfaceFrame {
            x: 0.0,
            y: 0.0,
            width: width.max(0.0),
            height: (height - HORIZONTAL_OVERLAY_SURFACE_INSET).max(0.0),
        },
        OverlayOrientation::Vertical => SurfaceFrame {
            x: 0.0,
            y: 0.0,
            width: (width - VERTICAL_OVERLAY_SURFACE_INSET).max(0.0),
            height: height.max(0.0),
        },
    }
}

fn vibrancy_strength(background_blur: f64) -> f64 {
    (background_blur / 40.0).clamp(0.0, 1.0)
}

fn vibrancy_enabled(style: &OverlayStyleSettings) -> bool {
    style.background_mode == OverlayBackgroundMode::Solid
        && style.background == OverlayBackground::Glass
        && style.background_blur > 0.0
}

pub(crate) fn sync_overlay_vibrancy(window: &tauri::WebviewWindow, style: &OverlayStyleSettings) {
    #[cfg(target_os = "macos")]
    {
        let target = window.clone();
        let orientation = style.orientation;
        let enabled = vibrancy_enabled(style);
        let strength = vibrancy_strength(style.background_blur);
        if let Err(error) = window.run_on_main_thread(move || {
            if let Err(error) = sync_macos_vibrancy(&target, orientation, enabled, strength) {
                log::warn!("Failed to sync the native overlay vibrancy effect: {error}");
            }
        }) {
            log::warn!("Failed to schedule the native overlay vibrancy update: {error}");
        }
    }

    #[cfg(not(target_os = "macos"))]
    let _ = (window, style);
}

#[cfg(target_os = "macos")]
fn sync_macos_vibrancy(
    window: &tauri::WebviewWindow,
    orientation: OverlayOrientation,
    enabled: bool,
    strength: f64,
) -> Result<(), String> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSAutoresizingMaskOptions, NSUserInterfaceItemIdentification, NSView,
        NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState,
        NSVisualEffectView, NSWindowOrderingMode,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

    const EFFECT_VIEW_IDENTIFIER: &str = "lyrics-plus-overlay-vibrancy";

    let main_thread = MainThreadMarker::new().ok_or("原生磨砂效果必须在主线程更新")?;
    let raw_view = window.ns_view().map_err(|error| error.to_string())?;
    if raw_view.is_null() {
        return Err("歌词浮窗原生视图为空".into());
    }
    let root = unsafe { &*raw_view.cast::<NSView>() };
    let existing = root.subviews().iter().find(|view| {
        view.identifier()
            .is_some_and(|identifier| identifier.to_string() == EFFECT_VIEW_IDENTIFIER)
    });

    if !enabled {
        if let Some(view) = existing {
            view.removeFromSuperview();
        }
        return Ok(());
    }

    let bounds = root.bounds();
    let frame = overlay_surface_frame(bounds.size.width, bounds.size.height, orientation);
    let frame = NSRect::new(
        NSPoint::new(frame.x, frame.y),
        NSSize::new(frame.width, frame.height),
    );

    if let Some(view) = existing {
        view.setFrame(frame);
        view.setAlphaValue(strength);
        return Ok(());
    }

    let effect = NSVisualEffectView::initWithFrame(main_thread.alloc(), frame);
    effect.setMaterial(NSVisualEffectMaterial::UnderWindowBackground);
    effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    effect.setState(NSVisualEffectState::Active);
    effect.setAlphaValue(strength);
    effect.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    effect.setWantsLayer(true);
    if let Some(layer) = effect.layer() {
        layer.setCornerRadius(OVERLAY_SURFACE_RADIUS);
        layer.setMasksToBounds(true);
    }
    let identifier = NSString::from_str(EFFECT_VIEW_IDENTIFIER);
    effect.setIdentifier(Some(&identifier));
    root.addSubview_positioned_relativeTo(&effect, NSWindowOrderingMode::Below, None);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_surface_frame_reserves_top_controls() {
        assert_eq!(
            overlay_surface_frame(760.0, 156.0, OverlayOrientation::Horizontal),
            SurfaceFrame {
                x: 0.0,
                y: 0.0,
                width: 760.0,
                height: 110.0,
            }
        );
    }

    #[test]
    fn vertical_surface_frame_reserves_right_controls() {
        assert_eq!(
            overlay_surface_frame(190.0, 620.0, OverlayOrientation::Vertical),
            SurfaceFrame {
                x: 0.0,
                y: 0.0,
                width: 142.0,
                height: 620.0,
            }
        );
    }

    #[test]
    fn vibrancy_strength_maps_internal_range_linearly() {
        assert_eq!(vibrancy_strength(0.0), 0.0);
        assert_eq!(vibrancy_strength(20.0), 0.5);
        assert_eq!(vibrancy_strength(40.0), 1.0);
        assert_eq!(vibrancy_strength(80.0), 1.0);
    }

    #[test]
    fn transparent_mode_disables_vibrancy_without_changing_details() {
        let mut style = OverlayStyleSettings::default();
        style.background_opacity = 0.35;
        style.background_blur = 26.0;
        assert!(vibrancy_enabled(&style));

        style.background_mode = OverlayBackgroundMode::Transparent;
        assert!(!vibrancy_enabled(&style));
        assert_eq!(style.background, OverlayBackground::Glass);
        assert_eq!(style.background_opacity, 0.35);
        assert_eq!(style.background_blur, 26.0);

        style.background_mode = OverlayBackgroundMode::Solid;
        assert!(vibrancy_enabled(&style));
    }
}
