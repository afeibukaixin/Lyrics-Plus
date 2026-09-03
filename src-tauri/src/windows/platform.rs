use tauri::Manager;

#[cfg(target_os = "macos")]
fn apply_joining_other_apps_fullscreen_on_main(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};

    if MainThreadMarker::new().is_none() {
        return Err(std::io::Error::other(
            "macOS window collection behavior must be updated on the main thread",
        )
        .into());
    }
    let ns_window = window.ns_window()?;
    let ns_window = unsafe { &*ns_window.cast::<NSWindow>() };
    let original_behavior = ns_window.collectionBehavior();
    let mut behavior = original_behavior;
    behavior.remove(
        NSWindowCollectionBehavior::CanJoinAllApplications
            | NSWindowCollectionBehavior::Primary
            | NSWindowCollectionBehavior::Auxiliary
            | NSWindowCollectionBehavior::FullScreenPrimary
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::FullScreenNone,
    );
    // CanJoinAllApplications 允许悬浮窗加入其他应用，而 FullScreenAuxiliary 明确允许
    // 它进入其他应用占用的全屏 Space；二者结合后才能跨显示器拖入全屏应用。
    behavior.insert(
        NSWindowCollectionBehavior::CanJoinAllApplications
            | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
    if behavior != original_behavior {
        ns_window.setCollectionBehavior(behavior);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn run_window_collection_behavior_update(
    window: &tauri::WebviewWindow,
    operation: impl FnOnce(&tauri::WebviewWindow) -> tauri::Result<()> + Send + 'static,
) -> tauri::Result<()> {
    use objc2::MainThreadMarker;

    if MainThreadMarker::new().is_some() {
        return operation(window);
    }

    // Playback monitoring can reconcile visibility off the main thread. AppKit
    // collection behavior must still be changed on the main thread.
    let target = window.clone();
    let (result_sender, result_receiver) = std::sync::mpsc::sync_channel(1);
    window.run_on_main_thread(move || {
        let result = operation(&target).map_err(|error| error.to_string());
        let _ = result_sender.send(result);
    })?;
    match result_receiver.recv() {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(std::io::Error::other(error).into()),
        Err(error) => Err(std::io::Error::other(format!(
            "macOS window behavior update was interrupted: {error}"
        ))
        .into()),
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn apply_joining_other_apps_fullscreen(
    window: &tauri::WebviewWindow,
) -> tauri::Result<()> {
    run_window_collection_behavior_update(window, apply_joining_other_apps_fullscreen_on_main)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn apply_joining_other_apps_fullscreen(
    _window: &tauri::WebviewWindow,
) -> tauri::Result<()> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn apply_lyrics_window_space_behavior_on_main(
    window: &tauri::WebviewWindow,
    enabled: bool,
) -> tauri::Result<()> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};

    if MainThreadMarker::new().is_none() {
        return Err(std::io::Error::other(
            "macOS window collection behavior must be updated on the main thread",
        )
        .into());
    }
    let ns_window = window.ns_window()?;
    let ns_window = unsafe { &*ns_window.cast::<NSWindow>() };
    let original_behavior = ns_window.collectionBehavior();
    let mut behavior = original_behavior;
    // Managed 与 Transient 同时决定窗口参与 Spaces 和 Mission Control 的方式；
    // 先清理互斥的 Space 行为，再按统一设置选择最终模式。
    behavior.remove(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::MoveToActiveSpace
            | NSWindowCollectionBehavior::Managed
            | NSWindowCollectionBehavior::Transient
            | NSWindowCollectionBehavior::Stationary,
    );
    if enabled {
        behavior.insert(
            NSWindowCollectionBehavior::Managed | NSWindowCollectionBehavior::CanJoinAllSpaces,
        );
    } else {
        behavior.insert(NSWindowCollectionBehavior::Transient);
    }
    if behavior != original_behavior {
        ns_window.setCollectionBehavior(behavior);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn apply_lyrics_window_space_behavior(
    window: &tauri::WebviewWindow,
    enabled: bool,
) -> tauri::Result<()> {
    run_window_collection_behavior_update(window, move |target| {
        apply_lyrics_window_space_behavior_on_main(target, enabled)
    })
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn apply_lyrics_window_space_behavior(
    window: &tauri::WebviewWindow,
    enabled: bool,
) -> tauri::Result<()> {
    window.set_visible_on_all_workspaces(enabled)
}

pub(crate) fn apply_lyrics_windows_space_behavior(
    app: &tauri::AppHandle,
    enabled: bool,
) -> tauri::Result<()> {
    for label in ["lyrics-overlay", "lyrics-list", "lyrics-notch"] {
        if let Some(window) = app.get_webview_window(label) {
            apply_lyrics_window_space_behavior(&window, enabled)?;
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(super) fn enable_notch_window_behavior(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use objc2_app_kit::{NSStatusWindowLevel, NSWindow};

    apply_joining_other_apps_fullscreen(window)?;
    let ns_window = window.ns_window()?;
    let ns_window = unsafe { &*ns_window.cast::<NSWindow>() };
    ns_window.setLevel(NSStatusWindowLevel);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(super) fn enable_notch_window_behavior(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    apply_joining_other_apps_fullscreen(window)
}

#[cfg(target_os = "macos")]
fn refresh_macos_mouse_tracking(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use objc2_app_kit::{NSView, NSWindow};

    fn update_tracking_areas(view: &NSView) {
        view.updateTrackingAreas();
        for child in view.subviews().iter() {
            update_tracking_areas(&child);
        }
    }

    let ns_window = window.ns_window()?;
    let ns_window = unsafe { &*ns_window.cast::<NSWindow>() };
    ns_window.setAcceptsMouseMovedEvents(true);
    ns_window.resetCursorRects();

    let ns_view = window.ns_view()?;
    let ns_view = unsafe { &*ns_view.cast::<NSView>() };
    update_tracking_areas(ns_view);
    Ok(())
}

pub(crate) fn refresh_overlay_mouse_tracking(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    {
        let target = window.clone();
        if let Err(error) = window.run_on_main_thread(move || {
            if let Err(error) = refresh_macos_mouse_tracking(&target) {
                log::warn!("Failed to refresh overlay mouse tracking: {error}");
            }
        }) {
            log::warn!("Failed to schedule the overlay mouse tracking refresh: {error}");
        }
    }

    #[cfg(not(target_os = "macos"))]
    let _ = window;
}
