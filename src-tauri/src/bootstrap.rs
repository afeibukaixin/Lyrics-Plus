#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = configure_web_content_process_handler(tauri::Builder::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Debug)
                .max_file_size(1_000_000)
                .rotation_strategy(RotationStrategy::KeepSome(3))
                .target(Target::new(TargetKind::Webview))
                .build(),
        )
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .skip_initial_state("main")
                .skip_initial_state("lyrics-overlay")
                .skip_initial_state("quick-lyrics")
                .skip_initial_state("lyrics-notch")
                .build(),
        )
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_macos_fps::init())
        .setup(|app| {
            let storage = storage::Storage::new(app.handle())?;
            let notice_accepted = legal_notice_accepted(&storage).unwrap_or(false);
            let app_dir = app.path().app_data_dir()?;
            let (config, migrated) = ConfigStore::load(&app_dir, &storage)
                .map_err(|error| std::io::Error::other(error))?;
            let config = Arc::new(config);
            let configured = config.snapshot();
            let provider_settings = configured.lyrics.providers.clone();
            let selection = configured.app.player_selection;
            let locked = configured.overlay.locked;
            let overlay_settings = OverlaySettings {
                visible: configured.overlay.visible,
                locked,
            };
            let last_overlay_monitor = storage
                .get_preference("overlay.last_monitor")
                .unwrap_or(None);
            let geometry = overlay_geometry(&storage, last_overlay_monitor.as_deref());
            if migrated {
                let geometry_key = last_overlay_monitor
                    .as_ref()
                    .map(|id| format!("overlay.geometry.{id}"))
                    .unwrap_or_else(|| "overlay.geometry.default".into());
                let raw = serde_json::to_string(&geometry)?;
                storage.set_preference(&geometry_key, &raw)?;
                for key in [
                    "player.selection",
                    "lyrics.providers",
                    "overlay.visible",
                    "overlay.locked",
                    "overlay.passthrough",
                ] {
                    storage.remove_preference(key)?;
                }
                storage.remove_preferences_with_prefix("overlay.style.")?;
            }
            let mut overlay_style = configured.overlay.appearance.clone().into_style();
            overlay_style.horizontal_max_width = geometry.horizontal_max_width;
            overlay_style.vertical_max_height = geometry.vertical_max_height;
            let initial_toolbar_placement =
                ToolbarPlacement::for_orientation(overlay_style.orientation);
            app.manage(AppState {
                runtime_started: Mutex::new(false),
                selection: Arc::new(RwLock::new(selection)),
                auto_player: Arc::new(RwLock::new(None)),
                overlay_settings: Arc::new(RwLock::new(overlay_settings.clone())),
                overlay_style: Arc::new(RwLock::new(overlay_style)),
                overlay_monitor: Arc::new(RwLock::new(last_overlay_monitor.clone())),
                overlay_placement: Arc::new(Mutex::new(OverlayPlacementState {
                    preferred_monitor: last_overlay_monitor,
                    toolbar_placement: initial_toolbar_placement,
                    ..OverlayPlacementState::default()
                })),
                last_snapshot: Arc::new(RwLock::new(player::PlaybackSnapshot::empty())),
                spectrum: Arc::new(player::PlaybackSpectrumService::default()),
                pointer_monitor_wake: Arc::new(tokio::sync::Notify::new()),
                status_bar_wake: Arc::new(tokio::sync::Notify::new()),
                lyrics_runtime: Arc::new(RwLock::new(lyrics::LyricsRuntimeSnapshot::default())),
                lyrics_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
                lyrics_search_session: Arc::new(Mutex::new(lyrics::LyricsSearchSession::default())),
                notch_layout_metrics: Arc::new(RwLock::new(NotchLayoutMetrics::default())),
                notch_visibility: Arc::new(Mutex::new(NotchVisibilityState::default())),
                webview_surface_lifecycle: Arc::new(Mutex::new(Default::default())),
                storage: Arc::new(storage),
                config,
                providers: Arc::new(
                    lyrics::provider::ProviderRegistry::new_with_app_dir(
                        provider_settings,
                        &app_dir,
                    )
                    .map_err(std::io::Error::other)?,
                ),
                system_media: Arc::new(SystemMediaService::default()),
                http: reqwest::Client::builder()
                    .user_agent(concat!(
                        "Lyrics Plus/",
                        env!("CARGO_PKG_VERSION"),
                        " (https://github.com/afeibukaixin/Lyrics-Plus)"
                    ))
                    .timeout(Duration::from_secs(8))
                    .build()
                    .map_err(|error| error.to_string())?,
            });

            if let Err(error) = player_lifecycle::sync_service(app.handle(), &configured.app) {
                log::warn!("Failed to configure player follower: {error}");
            }

            if notice_accepted {
                activate_runtime(app.handle()).map_err(std::io::Error::other)?;
            }
            if should_show_main_window(notice_accepted, configured.app.silent_startup) {
                show_main_window_centered(app.handle()).map_err(std::io::Error::other)?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                if let Some(state) = window.app_handle().try_state::<AppState>() {
                    state.spectrum.unsubscribe(&window.app_handle(), window.label());
                }
                if is_managed_surface_label(window.label()) {
                    handle_surface_destroyed(window.app_handle(), window.label());
                }
                if window.label() == "lyrics-overlay" {
                    set_overlay_drag_active(window.app_handle(), false);
                }
            }
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    let runtime_started = window
                        .app_handle()
                        .try_state::<AppState>()
                        .map(|state| {
                            *state
                                .runtime_started
                                .lock()
                                .unwrap_or_else(|error| error.into_inner())
                        })
                        .unwrap_or(false);
                    if !runtime_started {
                        log::info!(
                            "Application exit requested: reason=main_window_closed_before_runtime_started"
                        );
                        window.app_handle().exit(0);
                        return;
                    }
                    api.prevent_close();
                    if let Err(error) = hide_surface(window.app_handle(), "main") {
                        log::warn!("关闭主窗口时隐藏 WebView 失败：{error}");
                    }
                    schedule_surface_destroy(window.app_handle(), "main");
                    #[cfg(target_os = "macos")]
                    if window
                        .app_handle()
                        .state::<AppState>()
                        .config
                        .snapshot()
                        .app
                        .hide_dock_icon
                    {
                        // 隐藏主窗口后再次施加 Accessory 策略，避免 macOS 恢复 Dock 图标。
                        if let Err(error) = window
                            .app_handle()
                            .set_activation_policy(tauri::ActivationPolicy::Accessory)
                        {
                            log::warn!("关闭主窗口后隐藏 Dock 图标失败：{error}");
                        }
                    }
                }
            }
            if window.label() == "quick-lyrics" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    if let Err(error) = hide_surface(window.app_handle(), "quick-lyrics") {
                        log::warn!("关闭快速歌词窗口时隐藏 WebView 失败：{error}");
                    }
                    schedule_surface_destroy(window.app_handle(), "quick-lyrics");
                }
            }
            if window.label() == "lyrics-list" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    if let Some(state) = window.app_handle().try_state::<AppState>() {
                        if let Ok(config) = state.config.update(|config| {
                            config.lyrics.displays.list_window.enabled = false;
                        }) {
                            let _ = window.app_handle().emit("config://changed", &config);
                        }
                    }
                    if let Err(error) = hide_surface(window.app_handle(), "lyrics-list") {
                        log::warn!("关闭列表歌词窗口时隐藏 WebView 失败：{error}");
                    }
                    if let Err(error) = hide_surface(
                        window.app_handle(),
                        "lyrics-list-unlock-handle",
                    ) {
                        log::warn!("关闭列表歌词解锁按钮失败：{error}");
                    }
                    schedule_surface_destroy(window.app_handle(), "lyrics-list");
                    schedule_surface_destroy(window.app_handle(), "lyrics-list-unlock-handle");
                    sync_list_unlock_handle(window.app_handle());
                    sync_lyrics_surfaces(window.app_handle());
                }
            }
            if window.label() == "lyrics-overlay" {
                if let tauri::WindowEvent::Moved(position) = event {
                    if let Some(overlay) = window.app_handle().get_webview_window("lyrics-overlay")
                    {
                        // 原生拖动期间 macOS 独占窗口位置；松手后由 start_overlay_drag 统一收尾。
                        if overlay_drag_active(window.app_handle()) {
                            update_overlay_toolbar_placement_during_drag(
                                window.app_handle(),
                                &overlay,
                                *position,
                            );
                            return;
                        }
                        if ignore_overlay_move(window.app_handle(), &overlay, *position) {
                            return;
                        }
                        settle_overlay_position_at(window.app_handle(), &overlay, *position);
                    }
                }
                if matches!(event, tauri::WindowEvent::Resized(_)) {
                    if let Some(overlay) = window.app_handle().get_webview_window("lyrics-overlay")
                    {
                        if let Some(state) = window.app_handle().try_state::<AppState>() {
                            let style = state
                                .overlay_style
                                .read()
                                .unwrap_or_else(|error| error.into_inner())
                                .clone();
                            sync_overlay_vibrancy(&overlay, &style);
                        }
                        if !suppress_overlay_persistence(window.app_handle(), &overlay) {
                            persist_overlay_state(window.app_handle(), &overlay);
                        }
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_legal_notice_status,
            commands::accept_legal_notice,
            commands::quit_application,
            commands::get_playback_snapshot,
            commands::control_playback,
            commands::seek_playback,
            commands::get_playback_artwork,
            commands::start_playback_spectrum,
            commands::stop_playback_spectrum,
            commands::get_playback_spectrum_state,
            commands::get_player_selection,
            commands::set_player_selection,
            commands::search_lyrics,
            commands::get_completed_lyrics_search,
            commands::get_provider_settings,
            commands::get_provider_credentials,
            commands::set_provider_settings,
            commands::set_musixmatch_token,
            commands::clear_musixmatch_token,
            commands::test_provider,
            commands::get_cached_lyrics,
            commands::get_lyrics_runtime_snapshot,
            commands::get_notch_layout_metrics,
            commands::get_lyrics_monitors,
            commands::save_lyrics,
            commands::import_lyrics,
            commands::set_lyrics_offset,
            commands::remove_lyrics_association,
            commands::get_library_scan_status,
            commands::rescan_lyrics_library,
            commands::set_lyrics_directory,
            commands::open_lyrics_directory,
            commands::set_overlay_visible,
            commands::get_overlay_settings,
            commands::set_overlay_locked,
            commands::get_overlay_style,
            commands::get_overlay_toolbar_placement,
            commands::set_overlay_style,
            commands::start_overlay_drag,
            commands::nudge_overlay,
            commands::reset_overlay_bounds,
            commands::resize_overlay_edge,
            commands::fit_overlay_content,
            commands::fit_notch_lyrics_content,
            commands::show_main_window,
            commands::show_lyrics_style_settings,
            commands::show_quick_lyrics_window,
            commands::get_app_config,
            commands::set_theme,
            commands::resolve_system_media_applications,
            commands::set_system_media_filter_mode,
            commands::set_system_media_applications,
            commands::resolve_player_follower_application,
            commands::set_player_follower_application,
            commands::get_player_follower_service_status,
            commands::open_player_follower_system_settings,
            commands::open_automation_system_settings,
            commands::get_application_icons,
            commands::resolve_application_by_bundle_id,
            commands::set_language,
            commands::set_native_language,
            commands::get_global_shortcut_status,
            commands::set_global_shortcuts,
            commands::set_dock_icon_hidden,
            commands::set_menu_bar_icon_hidden,
            commands::set_silent_startup,
            commands::set_auto_check_updates,
            commands::set_overlay_hide_when_not_playing,
            commands::set_lyrics_windows_show_on_all_spaces,
            commands::set_status_bar_lyrics_enabled,
            commands::set_list_lyrics_visible,
            commands::set_list_lyrics_options,
            commands::set_list_lyrics_locked,
            commands::set_lyrics_chinese_conversion,
            commands::set_lyrics_japanese_repair_enabled,
            commands::set_notch_lyrics_visible,
            commands::set_lyrics_display_preferences,
            commands::set_lyrics_base_appearance,
            commands::set_lyrics_style_inheritance,
            commands::reset_lyrics_base_appearance,
            commands::reset_lyrics_style_mode,
            commands::reset_lyrics_display_position,
            commands::reset_list_lyrics_window_size,
            commands::export_app_config,
            commands::reveal_config_directory,
            commands::get_config_editor_data,
            commands::validate_app_config_draft,
            commands::save_app_config_draft,
            commands::reset_settings_section,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Lyrics Plus");
    app.run(|app, event| {
        if let tauri::RunEvent::ExitRequested { code: Some(_), .. } = &event {
            if let Some(state) = app.try_state::<AppState>() {
                state
                    .webview_surface_lifecycle
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .shutdown_requested = true;
            }
        }
        if let tauri::RunEvent::ExitRequested {
            code: None, api, ..
        } = &event
        {
            let runtime_started = app.try_state::<AppState>().is_some_and(|state| {
                *state
                    .runtime_started
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
            });
            if runtime_started {
                api.prevent_exit();
                let app_handle = app.clone();
                if let Err(error) = app.run_on_main_thread(move || {
                    if let Err(error) = setup_tray(&app_handle) {
                        log::warn!("无窗口保活时恢复菜单栏失败：{error}");
                        return;
                    }
                    if let Err(error) = sync_app_menu_bar_icon_visibility(&app_handle) {
                        log::warn!("无窗口保活时恢复菜单栏图标失败：{error}");
                    }
                    #[cfg(target_os = "macos")]
                    macos_status_item::sync(&app_handle);
                }) {
                    log::warn!("无窗口保活时调度菜单栏恢复失败：{error}");
                }
            }
        }
        #[cfg(target_os = "macos")]
        if matches!(event, tauri::RunEvent::Reopen { .. }) {
            if let Err(error) = sync_app_menu_bar_icon_visibility(app) {
                log::warn!("重新打开应用时恢复菜单栏图标失败：{error}");
            }
            let _ = show_main_window_centered(app);
        }
    });
}
