fn should_show_overlay(visible: bool, hide_when_not_playing: bool, is_playing: bool) -> bool {
    visible && (!hide_when_not_playing || is_playing)
}

pub(crate) fn reconcile_overlay_visibility(app: &tauri::AppHandle) -> Result<bool, String> {
    let state = app.state::<AppState>();
    let configured = state.config.snapshot();
    let is_playing = state
        .last_snapshot
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .is_playing;
    let should_show = should_show_overlay(
        configured.overlay.visible,
        configured.overlay.hide_when_not_playing,
        is_playing,
    );
    let window = app
        .get_webview_window("lyrics-overlay")
        .ok_or_else(|| "歌词浮窗不存在".to_string())?;
    let is_visible = window.is_visible().unwrap_or(false);
    if should_show {
        if !is_visible {
            restore_overlay_position(app, &window);
        }
        // 显示前同步统一的歌词窗口 Space 行为，避免窗口重新显示时使用旧状态。
        crate::apply_joining_other_apps_fullscreen(&window)
            .map_err(|error| error.to_string())?;
        crate::apply_lyrics_window_space_behavior(
            &window,
            configured.app.lyrics_windows_show_on_all_spaces,
        )
            .map_err(|error| error.to_string())?;
        if !is_visible {
            window.show().map_err(|error| error.to_string())?;
        }
    } else {
        if is_visible {
            window.hide().map_err(|error| error.to_string())?;
        }
        crate::apply_lyrics_window_space_behavior(
            &window,
            configured.app.lyrics_windows_show_on_all_spaces,
        )
        .map_err(|error| error.to_string())?;
    }
    sync_unlock_handle(app);
    Ok(should_show)
}

fn start_player_monitor(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let selection = app
                .try_state::<AppState>()
                .map(|state| *state.selection.read().unwrap_or_else(|e| e.into_inner()))
                .unwrap_or(PlayerSelection::Auto);
            let previous_auto_player = app
                .try_state::<AppState>()
                .map(|state| {
                    *state
                        .auto_player
                        .read()
                        .unwrap_or_else(|error| error.into_inner())
                })
                .unwrap_or(None);
            let system_media = app
                .try_state::<AppState>()
                .map(|state| state.system_media.clone())
                .unwrap_or_else(|| Arc::new(SystemMediaService::default()));
            let (system_media_filter_mode, system_media_applications) = app
                .try_state::<AppState>()
                .map(|state| {
                    let config = state.config.snapshot();
                    (
                        config.app.system_media_filter_mode,
                        config.app.system_media_applications,
                    )
                })
                .unwrap_or_default();

            let (snapshot, next_auto_player) = tauri::async_runtime::spawn_blocking(move || {
                query_selected_player(
                    selection,
                    previous_auto_player,
                    &system_media,
                    system_media_filter_mode,
                    &system_media_applications,
                )
            })
            .await
            .unwrap_or_else(|error| {
                (
                    player::PlaybackSnapshot::unavailable(
                        selection.preferred_kind(),
                        format!("播放器读取任务失败：{error}"),
                    ),
                    previous_auto_player,
                )
            });

            if let Some(state) = app.try_state::<AppState>() {
                *state
                    .last_snapshot
                    .write()
                    .unwrap_or_else(|e| e.into_inner()) = snapshot.clone();
                *state.auto_player.write().unwrap_or_else(|e| e.into_inner()) = next_auto_player;
                state.spectrum.sync_snapshot(&app, &snapshot);
            }
            let _ = app.emit("playback://snapshot", &snapshot);
            commands::sync_lyrics_runtime(&app, &snapshot);
            if let Err(error) = reconcile_overlay_visibility(&app) {
                log::warn!("Failed to reconcile overlay visibility with playback state: {error}");
            }
            if let Some(window) = app.get_webview_window("lyrics-overlay") {
                if window.is_visible().unwrap_or(false) {
                    reconcile_overlay_placement(&app, &window);
                }
            }
            let any_window_visible = app
                .try_state::<AppState>()
                .is_some_and(|state| state.config.snapshot().lyrics.displays.status_bar.enabled)
                || [
                    "main",
                    "lyrics-overlay",
                    "lyrics-list",
                    "lyrics-notch",
                ]
                .iter()
                .any(|label| {
                    app.get_webview_window(label)
                        .and_then(|window| window.is_visible().ok())
                        .unwrap_or(false)
                });
            tokio::time::sleep(Duration::from_millis(if any_window_visible {
                750
            } else {
                2_000
            }))
            .await;
        }
    });
}

pub(crate) fn monitor_id(monitor: &tauri::Monitor) -> String {
    monitor.name().cloned().unwrap_or_else(|| {
        let position = monitor.position();
        let size = monitor.size();
        format!(
            "{}x{}-{}x{}",
            position.x, position.y, size.width, size.height
        )
    })
}
