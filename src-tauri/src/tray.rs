fn initial_overlay_dimensions(style: &OverlayStyleSettings) -> (f64, f64) {
    match style.orientation {
        OverlayOrientation::Horizontal => (style.horizontal_max_width.unwrap_or(760.0), 156.0),
        OverlayOrientation::Vertical => (190.0, style.vertical_max_height.unwrap_or(620.0)),
    }
}

fn create_unlock_handle(app: &tauri::AppHandle) -> tauri::Result<()> {
    if app.get_webview_window("lyrics-unlock-handle").is_some() {
        return Ok(());
    }
    let builder = WebviewWindowBuilder::new(
        app,
        "lyrics-unlock-handle",
        WebviewUrl::App("index.html?view=unlock-handle".into()),
    )
    .title(UiLanguage::ZhCn.native_labels().unlock_title)
    .inner_size(28.0, 28.0)
    .transparent(true)
    .decorations(false)
    .shadow(false)
    .resizable(false)
    // 这是覆盖在歌词浮窗上的点击入口，不应成为 macOS 的键盘焦点窗口。
    // 否则歌词换行触发位置同步时，可能抢走当前应用的焦点。
    .focusable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false);

    #[cfg(target_os = "macos")]
    let builder = {
        let overlay = app
            .get_webview_window("lyrics-overlay")
            .ok_or(tauri::Error::WindowNotFound)?;
        builder.parent(&overlay)?
    };

    builder.build()?;

    Ok(())
}

fn create_list_unlock_handle(app: &tauri::AppHandle) -> tauri::Result<()> {
    if app
        .get_webview_window("lyrics-list-unlock-handle")
        .is_some()
    {
        return Ok(());
    }
    let builder = WebviewWindowBuilder::new(
        app,
        "lyrics-list-unlock-handle",
        WebviewUrl::App("index.html?view=lyrics-list-unlock-handle".into()),
    )
    .title(UiLanguage::ZhCn.native_labels().unlock_title)
    .inner_size(28.0, 28.0)
    .transparent(true)
    .decorations(false)
    .shadow(false)
    .resizable(false)
    .focusable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false);

    #[cfg(target_os = "macos")]
    let builder = {
        let list = app
            .get_webview_window("lyrics-list")
            .ok_or(tauri::Error::WindowNotFound)?;
        builder.parent(&list)?
    };

    builder.build()?;
    Ok(())
}

fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    if app.try_state::<TrayMenuState>().is_some() {
        return Ok(());
    }
    let configured = app.state::<AppState>().config.snapshot();
    let labels = configured.app.language.native_language().native_labels();
    let overlay_visible = app
        .state::<AppState>()
        .overlay_settings
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .visible;
    let display_config = configured.lyrics.displays;
    let shortcuts = configured.app.shortcuts;
    let accelerator = |value: &str| {
        (!value.trim().is_empty()).then(|| value.replace("CommandOrControl", "CmdOrCtrl"))
    };
    let toggle_accelerator = accelerator(&shortcuts.toggle_overlay);
    let status_bar_accelerator = accelerator(&shortcuts.toggle_status_bar_lyrics);
    let list_accelerator = accelerator(&shortcuts.toggle_list_lyrics);
    let notch_accelerator = accelerator(&shortcuts.toggle_notch_lyrics);
    let switch_accelerator = accelerator(&shortcuts.switch_lyrics);
    let toggle_overlay = CheckMenuItem::with_id(
        app,
        "toggle-overlay",
        labels.toggle_overlay,
        true,
        overlay_visible,
        toggle_accelerator.as_deref(),
    )?;
    let toggle_status_bar_lyrics = CheckMenuItem::with_id(
        app,
        "toggle-status-bar-lyrics",
        labels.toggle_status_bar_lyrics,
        true,
        display_config.status_bar.enabled,
        status_bar_accelerator.as_deref(),
    )?;
    let toggle_list_lyrics = CheckMenuItem::with_id(
        app,
        "toggle-list-lyrics",
        labels.toggle_list_lyrics,
        true,
        display_config.list_window.enabled,
        list_accelerator.as_deref(),
    )?;
    let toggle_notch_lyrics = CheckMenuItem::with_id(
        app,
        "toggle-notch-lyrics",
        labels.toggle_notch_lyrics,
        true,
        display_config.notch.enabled,
        notch_accelerator.as_deref(),
    )?;
    let switch_lyrics = MenuItem::with_id(
        app,
        "switch-lyrics",
        labels.switch_lyrics,
        true,
        switch_accelerator.as_deref(),
    )?;
    let settings = MenuItem::with_id(app, "settings", labels.settings, true, Some("CmdOrCtrl+,"))?;
    let quit = MenuItem::with_id(app, "quit", labels.quit, true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &toggle_overlay,
            &toggle_status_bar_lyrics,
            &toggle_list_lyrics,
            &toggle_notch_lyrics,
            &switch_lyrics,
            &settings,
            &quit,
        ],
    )?;

    #[cfg(target_os = "macos")]
    let tray_icon = {
        let rgba = image::load_from_memory(include_bytes!("../icons/tray-icon.png"))
            .expect("invalid embedded tray icon")
            .into_rgba8();
        let (width, height) = rgba.dimensions();
        tauri::image::Image::new_owned(rgba.into_raw(), width, height)
    };
    #[cfg(not(target_os = "macos"))]
    let tray_icon = app
        .default_window_icon()
        .cloned()
        .expect("missing application icon");

    let tray_builder = TrayIconBuilder::with_id("app-menu").icon(tray_icon);
    #[cfg(target_os = "macos")]
    let tray_builder = tray_builder.icon_as_template(true);

    let icon = tray_builder
        .tooltip("Lyrics Plus")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle-overlay" => {
                let visible = app
                    .state::<AppState>()
                    .overlay_settings
                    .read()
                    .unwrap_or_else(|error| error.into_inner())
                    .visible;
                let _ = commands::update_overlay_visible(app, !visible);
            }
            "toggle-status-bar-lyrics" => {
                let enabled = app
                    .state::<AppState>()
                    .config
                    .snapshot()
                    .lyrics
                    .displays
                    .status_bar
                    .enabled;
                let _ = commands::set_status_bar_lyrics_enabled(
                    app.clone(),
                    !enabled,
                    app.state::<AppState>(),
                );
            }
            "toggle-list-lyrics" => {
                let enabled = app
                    .state::<AppState>()
                    .config
                    .snapshot()
                    .lyrics
                    .displays
                    .list_window
                    .enabled;
                let _ = commands::set_list_lyrics_visible(
                    app.clone(),
                    !enabled,
                    app.state::<AppState>(),
                );
            }
            "toggle-notch-lyrics" => {
                let enabled = app
                    .state::<AppState>()
                    .config
                    .snapshot()
                    .lyrics
                    .displays
                    .notch
                    .enabled;
                let _ = commands::set_notch_lyrics_visible(
                    app.clone(),
                    !enabled,
                    app.state::<AppState>(),
                );
            }
            "switch-lyrics" => {
                if let Err(error) = toggle_quick_lyrics_window(app) {
                    log::warn!("Failed to toggle quick lyrics from the tray: {error}");
                }
            }
            "settings" => {
                if let Err(error) = show_main_window_centered(app) {
                    log::warn!("Failed to open settings from the tray: {error}");
                }
            }
            "quit" => {
                log::info!("Application exit requested: reason=tray_quit");
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    #[cfg(target_os = "macos")]
    let lyrics_icon = TrayIconBuilder::with_id("lyrics-status-item")
        .title("Lyrics Plus")
        .tooltip("Lyrics Plus")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .build(app)?;
    #[cfg(target_os = "macos")]
    lyrics_icon.with_inner_tray_icon(|inner| {
        if let Some(status_item) = inner.ns_status_item() {
            status_item.setVisible(false);
        }
    })?;

    app.manage(TrayMenuState {
        icon,
        #[cfg(target_os = "macos")]
        lyrics_icon,
        toggle_overlay: toggle_overlay.clone(),
        toggle_status_bar_lyrics: toggle_status_bar_lyrics.clone(),
        toggle_list_lyrics: toggle_list_lyrics.clone(),
        toggle_notch_lyrics: toggle_notch_lyrics.clone(),
        switch_lyrics: switch_lyrics.clone(),
        settings: settings.clone(),
        quit: quit.clone(),
    });
    #[cfg(target_os = "macos")]
    macos_status_item::configure_lyrics_icon_identity(app)?;
    sync_app_menu_bar_icon_visibility(app).map_err(std::io::Error::other)?;
    sync_lyrics_surfaces(app);
    #[cfg(target_os = "macos")]
    macos_status_item::start(app.clone());

    Ok(())
}
