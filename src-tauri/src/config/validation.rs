pub fn validate_config_draft(raw: &str) -> ConfigDraftValidation {
    match parse_config_draft(raw) {
        Ok(parsed) => ConfigDraftValidation {
            valid: true,
            error: None,
            normalized_json: Some(parsed.normalized_json),
            effective_config: parsed.config,
        },
        Err(error) => ConfigDraftValidation {
            valid: false,
            error: Some(error),
            normalized_json: None,
            effective_config: AppConfig::default(),
        },
    }
}

fn parse_config_draft(raw: &str) -> Result<ParsedDraft, ConfigDraftError> {
    let sanitized = sanitize_jsonc(raw)?;
    let mut user = serde_json::from_str::<Value>(&sanitized).map_err(|error| ConfigDraftError {
        message: format!("JSONC 语法错误：{}", error),
        line: error.line(),
        column: error.column(),
    })?;
    if !user.is_object() {
        return Err(ConfigDraftError {
            message: "配置根节点必须是对象".into(),
            line: 1,
            column: 1,
        });
    }
    let version = match user.get("schemaVersion") {
        None => CONFIG_SCHEMA_VERSION,
        Some(Value::Number(value)) => {
            let version = value
                .as_u64()
                .ok_or_else(|| error_at_key(raw, "schemaVersion", "schemaVersion 必须是正整数"))?;
            u16::try_from(version)
                .map_err(|_| error_at_key(raw, "schemaVersion", "schemaVersion 超出支持范围"))?
        }
        Some(_) => {
            return Err(error_at_key(
                raw,
                "schemaVersion",
                "schemaVersion 必须是数字",
            ));
        }
    };
    if version > CONFIG_SCHEMA_VERSION {
        return Err(error_at_key(
            raw,
            "schemaVersion",
            &format!("配置文件版本 {version} 高于当前支持的版本 {CONFIG_SCHEMA_VERSION}"),
        ));
    }
    user.as_object_mut()
        .expect("checked object")
        .remove("artwork");
    if version < CONFIG_SCHEMA_VERSION {
        if let Some(app) = user.get_mut("app").and_then(Value::as_object_mut) {
            app.retain(|key, _| APP_CONFIG_KEYS.contains(&key.as_str()));
        }
    }
    if version < 24 {
        let app = user
            .as_object_mut()
            .expect("checked object")
            .entry("app")
            .or_insert_with(|| Value::Object(Default::default()))
            .as_object_mut()
            .ok_or_else(|| error_at_key(raw, "app", "app 必须是对象"))?;
        let mode = if app
            .get("systemMediaApplications")
            .and_then(Value::as_array)
            .is_some_and(|applications| !applications.is_empty())
        {
            "allowlist"
        } else {
            "blocklist"
        };
        app.insert("systemMediaFilterMode".into(), Value::from(mode));
    }
    migrate_v32_display_appearances(&mut user, version);
    migrate_v34_lyrics_base_appearance(&mut user, version);
    migrate_v37_notch_width(&mut user, version);
    migrate_v38_notch_line_count(&mut user, version);
    migrate_v39_notch_supporting_tracks(&mut user, version);
    migrate_v40_notch_colors(&mut user, version);
    migrate_v41_fixed_notch_background(&mut user, version);
    migrate_v42_list_preferences(&mut user, version);
    migrate_v48_notch_mode(&mut user, version);
    migrate_v49_notch_width(&mut user, version);
    migrate_v50_notch_layout(&mut user, version);
    migrate_v54_notch_double_line_settings(&mut user, version);
    migrate_v57_chinese_conversion(&mut user, version);
    migrate_v59_switch_lyrics_shortcut(&mut user);
    migrate_v62_status_bar_secondary_font_weight(&mut user, version);
    remove_retired_fullscreen_space_preferences(&mut user);
    validate_known_fields(&user, raw)?;
    validate_field_types_and_options(&user, raw)?;
    migrate_status_bar_status_item_fields(&mut user);

    let migrated_layout = migrate_legacy_overlay_layout(&mut user, version, raw)?;
    #[cfg(test)]
    let migrated = version < CONFIG_SCHEMA_VERSION || migrated_layout;
    #[cfg(not(test))]
    let _ = migrated_layout;
    user.as_object_mut()
        .expect("checked object")
        .insert("schemaVersion".into(), Value::from(CONFIG_SCHEMA_VERSION));

    validate_numeric_ranges(&user, raw)?;
    let mut merged = serde_json::to_value(AppConfig::default()).map_err(internal_draft_error)?;
    merge_json(&mut merged, user);
    let mut config =
        serde_json::from_value::<AppConfig>(merged).map_err(|error| ConfigDraftError {
            message: format!("配置字段类型或选项无效：{error}"),
            line: 1,
            column: 1,
        })?;
    if version < 5 {
        migrate_legacy_provider_order(&mut config.lyrics.providers);
    }
    if version < 14 {
        migrate_v13_provider_defaults(&mut config.lyrics.providers);
    }
    if version < 45 {
        migrate_v45_provider_sources(&mut config.lyrics.providers);
    }
    if version < 58 {
        migrate_v58_enable_all_provider_sources(&mut config.lyrics.providers);
    }
    let config = config.normalized().map_err(|message| {
        let key = if message.contains("歌词源") {
            "providers"
        } else if message.contains("快捷键") {
            "shortcuts"
        } else {
            "appearance"
        };
        error_at_key(raw, key, &message)
    })?;
    let normalized_json =
        canonical_config_jsonc(&config, UiLanguage::ZhCn).map_err(internal_draft_error)?;
    Ok(ParsedDraft {
        config,
        normalized_json,
        #[cfg(test)]
        migrated,
    })
}

fn sanitize_jsonc(raw: &str) -> Result<String, ConfigDraftError> {
    #[derive(Clone, Copy)]
    enum State {
        Normal,
        String,
        LineComment,
        BlockComment { line: usize, column: usize },
    }
    let characters = raw.chars().collect::<Vec<_>>();
    let mut output = characters.clone();
    let mut state = State::Normal;
    let mut escaped = false;
    let mut line = 1;
    let mut column = 1;
    let mut index = 0;
    while index < characters.len() {
        let current = characters[index];
        let next = characters.get(index + 1).copied();
        match state {
            State::Normal if current == '"' => state = State::String,
            State::Normal if current == '/' && next == Some('/') => {
                output[index] = ' ';
                output[index + 1] = ' ';
                state = State::LineComment;
                index += 1;
                column += 1;
            }
            State::Normal if current == '/' && next == Some('*') => {
                output[index] = ' ';
                output[index + 1] = ' ';
                state = State::BlockComment { line, column };
                index += 1;
                column += 1;
            }
            State::String if escaped => escaped = false,
            State::String if current == '\\' => escaped = true,
            State::String if current == '"' => state = State::Normal,
            State::LineComment if current == '\n' => state = State::Normal,
            State::LineComment => output[index] = ' ',
            State::BlockComment { .. } if current == '*' && next == Some('/') => {
                output[index] = ' ';
                output[index + 1] = ' ';
                state = State::Normal;
                index += 1;
                column += 1;
            }
            State::BlockComment { .. } if current != '\n' => output[index] = ' ',
            _ => {}
        }
        if current == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
        index += 1;
    }
    if let State::BlockComment { line, column } = state {
        return Err(ConfigDraftError {
            message: "块注释没有结束".into(),
            line,
            column,
        });
    }

    let mut in_string = false;
    let mut escaped = false;
    for index in 0..output.len() {
        let current = output[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if current == '\\' {
                escaped = true;
            } else if current == '"' {
                in_string = false;
            }
            continue;
        }
        if current == '"' {
            in_string = true;
            continue;
        }
        if current != ',' {
            continue;
        }
        let mut lookahead = index + 1;
        while lookahead < output.len() && output[lookahead].is_whitespace() {
            lookahead += 1;
        }
        if matches!(output.get(lookahead), Some('}') | Some(']')) {
            output[index] = ' ';
        }
    }
    Ok(output.into_iter().collect())
}

fn validate_known_fields(value: &Value, raw: &str) -> Result<(), ConfigDraftError> {
    check_keys(value, raw, &["schemaVersion", "app", "lyrics", "overlay"])?;
    if let Some(app) = value.get("app") {
        check_keys(app, raw, APP_CONFIG_KEYS)?;
        if let Some(shortcuts) = app.get("shortcuts") {
            check_keys(
                shortcuts,
                raw,
                &[
                    "toggleOverlay",
                    "unlockOverlay",
                    "resetOverlay",
                    "toggleStatusBarLyrics",
                    "toggleListLyrics",
                    "toggleNotchLyrics",
                    "switchLyrics",
                ],
            )?;
        }
        if let Some(applications) = app.get("systemMediaApplications").and_then(Value::as_array) {
            for application in applications {
                check_keys(application, raw, &["name", "bundleId"])?;
            }
        }
    }
    if let Some(lyrics) = value.get("lyrics") {
        check_keys(
            lyrics,
            raw,
            &[
                "chineseConversion",
                "providers",
                "displays",
                "baseAppearance",
                "styleInheritance",
            ],
        )?;
        if let Some(base) = lyrics.get("baseAppearance") {
            check_keys(
                base,
                raw,
                &[
                    "fontFamily",
                    "activeColor",
                    "inactiveColor",
                    "translationColor",
                    "romanizationColor",
                    "supportingColor",
                    "backgroundColor",
                ],
            )?;
        }
        if let Some(inheritance) = lyrics.get("styleInheritance") {
            check_keys(
                inheritance,
                raw,
                &["desktop", "statusBar", "listWindow", "notch"],
            )?;
            for mode in ["desktop", "statusBar", "listWindow", "notch"] {
                if let Some(value) = inheritance.get(mode) {
                    check_keys(value, raw, &["inheritFontFamily", "inheritColors"])?;
                }
            }
        }
        if let Some(providers) = lyrics.get("providers") {
            check_keys(
                providers,
                raw,
                &[
                    "mode",
                    "providers",
                    "autoApplyThreshold",
                    "autoSearchDebounceMs",
                    "preferCapabilities",
                    "capabilityPreferenceTolerance",
                    "matchWeights",
                    "normalizeChinese",
                    "titleFilterKeywords",
                    "amllBaseUrl",
                ],
            )?;
            if let Some(match_weights) = providers.get("matchWeights") {
                check_keys(
                    match_weights,
                    raw,
                    &["title", "artist", "album", "duration"],
                )?;
            }
            if let Some(items) = providers.get("providers").and_then(Value::as_array) {
                for item in items {
                    check_keys(item, raw, &["id", "enabled"])?;
                }
            }
        }
        if let Some(displays) = lyrics.get("displays") {
            check_keys(displays, raw, &["statusBar", "listWindow", "notch"])?;
            if let Some(status_bar) = displays.get("statusBar") {
                check_keys(
                    status_bar,
                    raw,
                    &[
                        "enabled",
                        "hideWhenNotPlaying",
                        "doubleLine",
                        "showTranslation",
                        "showRomanization",
                        // Accepted only so older configurations can be migrated.
                        "showTrayIcon",
                        "locked",
                        "maxCharacters",
                        "appearance",
                    ],
                )?;
                if let Some(appearance) = status_bar.get("appearance") {
                    check_keys(
                        appearance,
                        raw,
                        &[
                            "fontFamily",
                            "fontSize",
                            "verticalOffset",
                            "fontWeight",
                            "secondaryFontWeight",
                            "textColor",
                            "inactiveColor",
                            "highlightColor",
                            "translationColor",
                            "romanizationColor",
                            "karaokeStyle",
                            "alignment",
                            "width",
                            // Legacy floating-window fields remain valid input.
                            "backgroundColor",
                            "backgroundOpacity",
                            "backgroundBlur",
                            "borderRadius",
                            "paddingX",
                            "paddingY",
                            "maxWidth",
                        ],
                    )?;
                }
            }
            if let Some(list_window) = displays.get("listWindow") {
                check_keys(
                    list_window,
                    raw,
                    &[
                        "enabled",
                        "alwaysOnTop",
                        "locked",
                        "showTranslation",
                        "showRomanization",
                        "appearance",
                    ],
                )?;
                if let Some(appearance) = list_window.get("appearance") {
                    check_keys(
                        appearance,
                        raw,
                        &[
                            "fontFamily",
                            "fontSize",
                            "fontWeight",
                            "secondaryFontScale",
                            "lineHeight",
                            "lineGap",
                            "secondaryLineGap",
                            "activeColor",
                            "inactiveColor",
                            "activeOpacity",
                            "inactiveOpacity",
                            "translationColor",
                            "romanizationColor",
                            "activeBackgroundColor",
                            "backgroundColor",
                            "backgroundOpacity",
                            "backgroundMode",
                            "alignment",
                        ],
                    )?;
                }
            }
            if let Some(notch) = displays.get("notch") {
                check_keys(
                    notch,
                    raw,
                    &[
                        "enabled",
                        "hideWhenNotPlaying",
                        "monitorId",
                        "showLyrics",
                        "leftSlot",
                        "rightSlot",
                        "layout",
                        "doubleLineMode",
                        "showTranslation",
                        "showRomanization",
                        "inlineLyricsOnNonNotch",
                        "appearance",
                    ],
                )?;
                if let Some(appearance) = notch.get("appearance") {
                    check_keys(
                        appearance,
                        raw,
                        &[
                            "fontFamily",
                            "fontSize",
                            "fontWeight",
                            "secondaryFontWeight",
                            "activeColor",
                            "inactiveColor",
                            "translationColor",
                            "romanizationColor",
                            "karaokeStyle",
                            "lineGap",
                            "borderRadius",
                            "topBorderRadius",
                            "maxWidth",
                            "expandedMaxWidth",
                        ],
                    )?;
                }
            }
        }
    }
    if let Some(overlay) = value.get("overlay") {
        check_keys(
            overlay,
            raw,
            &["visible", "locked", "hideWhenNotPlaying", "appearance"],
        )?;
        if let Some(appearance) = overlay.get("appearance") {
            check_keys(
                appearance,
                raw,
                &[
                    "fontFamily",
                    "fontSize",
                    "fontWeight",
                    "secondaryFontWeight",
                    "lineHeight",
                    "activeColor",
                    "inactiveColor",
                    "opacity",
                    "backgroundOpacity",
                    "backgroundBlur",
                    "backgroundRadius",
                    "backgroundPaddingX",
                    "backgroundPaddingY",
                    "backgroundMode",
                    "background",
                    "solidColor",
                    "layout",
                    "doubleLineMode",
                    "orientation",
                    "alignment",
                    "primaryLinePosition",
                    "lineGap",
                    "longText",
                    "secondaryDisplay",
                    "autoCenterWithTranslationOrRomanization",
                    "karaokeStyle",
                    "secondaryFontScale",
                    "translationFontScale",
                    "romanizationFontScale",
                    "translationColor",
                    "romanizationColor",
                    "textShadowOffsetX",
                    "textShadowOffsetY",
                    "textShadowBlur",
                    "textShadowColor",
                    "textStrokeWidth",
                    "textStrokeColor",
                ],
            )?;
        }
    }
    Ok(())
}

fn validate_field_types_and_options(value: &Value, raw: &str) -> Result<(), ConfigDraftError> {
    for (pointer, key) in [
        ("/app", "app"),
        ("/app/shortcuts", "shortcuts"),
        ("/lyrics", "lyrics"),
        ("/lyrics/providers", "providers"),
        ("/lyrics/displays", "displays"),
        ("/lyrics/displays/statusBar", "statusBar"),
        ("/lyrics/displays/statusBar/appearance", "appearance"),
        ("/lyrics/displays/listWindow", "listWindow"),
        ("/lyrics/displays/listWindow/appearance", "appearance"),
        ("/lyrics/displays/notch", "notch"),
        ("/lyrics/displays/notch/appearance", "appearance"),
        ("/overlay", "overlay"),
        ("/overlay/appearance", "appearance"),
    ] {
        if value
            .pointer(pointer)
            .is_some_and(|candidate| !candidate.is_object())
        {
            return Err(error_at_key(raw, key, &format!("{key} 必须是对象")));
        }
    }
    if let Some(applications) = value.pointer("/app/systemMediaApplications") {
        let Some(applications) = applications.as_array() else {
            return Err(error_at_key(
                raw,
                "systemMediaApplications",
                "systemMediaApplications 必须是数组",
            ));
        };
        for application in applications {
            let Some(application) = application.as_object() else {
                return Err(error_at_key(
                    raw,
                    "systemMediaApplications",
                    "系统播放应用必须是对象",
                ));
            };
            for key in ["name", "bundleId"] {
                if !application.get(key).is_some_and(Value::is_string) {
                    return Err(error_at_key(raw, key, &format!("{key} 必须是字符串")));
                }
            }
        }
    }
    if let Some(application) = value.pointer("/app/playerFollowerApplication") {
        if !application.is_null() {
            let Some(application) = application.as_object() else {
                return Err(error_at_key(
                    raw,
                    "playerFollowerApplication",
                    "playerFollowerApplication 必须是对象或 null",
                ));
            };
            for key in ["name", "bundleId"] {
                if !application.get(key).is_some_and(Value::is_string) {
                    return Err(error_at_key(raw, key, &format!("{key} 必须是字符串")));
                }
            }
        }
    }
    for (pointer, key) in [
        ("/app/hideDockIcon", "hideDockIcon"),
        ("/app/hideMenuBarIcon", "hideMenuBarIcon"),
        ("/app/silentStartup", "silentStartup"),
        ("/app/autoCheckUpdates", "autoCheckUpdates"),
        (
            "/app/lyricsWindowsShowOnAllSpaces",
            "lyricsWindowsShowOnAllSpaces",
        ),
        ("/overlay/visible", "visible"),
        ("/overlay/locked", "locked"),
        ("/overlay/hideWhenNotPlaying", "hideWhenNotPlaying"),
        ("/lyrics/displays/statusBar/enabled", "enabled"),
        (
            "/lyrics/displays/statusBar/hideWhenNotPlaying",
            "hideWhenNotPlaying",
        ),
        ("/lyrics/displays/statusBar/doubleLine", "doubleLine"),
        (
            "/lyrics/displays/statusBar/showTranslation",
            "showTranslation",
        ),
        (
            "/lyrics/displays/statusBar/showRomanization",
            "showRomanization",
        ),
        ("/lyrics/displays/statusBar/showTrayIcon", "showTrayIcon"),
        ("/lyrics/displays/statusBar/locked", "locked"),
        ("/lyrics/displays/listWindow/enabled", "enabled"),
        ("/lyrics/displays/listWindow/alwaysOnTop", "alwaysOnTop"),
        ("/lyrics/displays/listWindow/locked", "locked"),
        (
            "/lyrics/displays/listWindow/showTranslation",
            "showTranslation",
        ),
        (
            "/lyrics/displays/listWindow/showRomanization",
            "showRomanization",
        ),
        ("/lyrics/displays/notch/enabled", "enabled"),
        (
            "/lyrics/displays/notch/hideWhenNotPlaying",
            "hideWhenNotPlaying",
        ),
        ("/lyrics/displays/notch/showLyrics", "showLyrics"),
        ("/lyrics/displays/notch/showTranslation", "showTranslation"),
        (
            "/lyrics/displays/notch/showRomanization",
            "showRomanization",
        ),
        (
            "/lyrics/displays/notch/inlineLyricsOnNonNotch",
            "inlineLyricsOnNonNotch",
        ),
        (
            "/lyrics/styleInheritance/desktop/inheritFontFamily",
            "inheritFontFamily",
        ),
        (
            "/lyrics/styleInheritance/desktop/inheritColors",
            "inheritColors",
        ),
        (
            "/lyrics/styleInheritance/statusBar/inheritFontFamily",
            "inheritFontFamily",
        ),
        (
            "/lyrics/styleInheritance/statusBar/inheritColors",
            "inheritColors",
        ),
        (
            "/lyrics/styleInheritance/listWindow/inheritFontFamily",
            "inheritFontFamily",
        ),
        (
            "/lyrics/styleInheritance/listWindow/inheritColors",
            "inheritColors",
        ),
        (
            "/lyrics/styleInheritance/notch/inheritFontFamily",
            "inheritFontFamily",
        ),
        (
            "/lyrics/styleInheritance/notch/inheritColors",
            "inheritColors",
        ),
        (
            "/overlay/appearance/autoCenterWithTranslationOrRomanization",
            "autoCenterWithTranslationOrRomanization",
        ),
        ("/lyrics/providers/preferCapabilities", "preferCapabilities"),
        ("/lyrics/providers/normalizeChinese", "normalizeChinese"),
    ] {
        if value
            .pointer(pointer)
            .is_some_and(|candidate| !candidate.is_boolean())
        {
            return Err(error_at_key(raw, key, &format!("{key} 必须是布尔值")));
        }
    }
    for (pointer, key) in [
        ("/schemaVersion", "schemaVersion"),
        ("/lyrics/providers/autoApplyThreshold", "autoApplyThreshold"),
        (
            "/lyrics/providers/autoSearchDebounceMs",
            "autoSearchDebounceMs",
        ),
        ("/lyrics/displays/statusBar/appearance/fontSize", "fontSize"),
        (
            "/lyrics/displays/statusBar/appearance/fontWeight",
            "fontWeight",
        ),
        (
            "/lyrics/displays/statusBar/appearance/secondaryFontWeight",
            "secondaryFontWeight",
        ),
        ("/lyrics/displays/statusBar/appearance/width", "width"),
        ("/lyrics/displays/statusBar/appearance/maxWidth", "maxWidth"),
        (
            "/lyrics/displays/listWindow/appearance/fontSize",
            "fontSize",
        ),
        (
            "/lyrics/displays/listWindow/appearance/fontWeight",
            "fontWeight",
        ),
        ("/lyrics/displays/notch/appearance/fontSize", "fontSize"),
        ("/lyrics/displays/notch/appearance/fontWeight", "fontWeight"),
        (
            "/lyrics/displays/notch/appearance/secondaryFontWeight",
            "secondaryFontWeight",
        ),
        ("/lyrics/displays/notch/appearance/maxWidth", "maxWidth"),
        (
            "/lyrics/displays/notch/appearance/expandedMaxWidth",
            "expandedMaxWidth",
        ),
        ("/overlay/appearance/fontSize", "fontSize"),
        ("/overlay/appearance/fontWeight", "fontWeight"),
        (
            "/overlay/appearance/secondaryFontWeight",
            "secondaryFontWeight",
        ),
        (
            "/lyrics/providers/capabilityPreferenceTolerance",
            "capabilityPreferenceTolerance",
        ),
    ] {
        if value
            .pointer(pointer)
            .is_some_and(|candidate| candidate.as_u64().is_none())
        {
            return Err(error_at_key(raw, key, &format!("{key} 必须是整数")));
        }
    }
    validate_language_preference(value, raw)?;
    if let Some(candidate) = value.pointer("/lyrics/displays/notch/monitorId") {
        if !candidate.is_null() && !candidate.is_string() {
            return Err(error_at_key(
                raw,
                "monitorId",
                "monitorId 必须是字符串或 null",
            ));
        }
    }
    validate_string_option(
        value,
        raw,
        "/app/theme",
        "theme",
        &["system", "light", "dark"],
    )?;
    validate_string_option(
        value,
        raw,
        "/app/playerSelection",
        "playerSelection",
        &["auto", "apple_music", "spotify", "system"],
    )?;
    validate_string_option(
        value,
        raw,
        "/app/systemMediaFilterMode",
        "systemMediaFilterMode",
        &["allowlist", "blocklist"],
    )?;
    for (pointer, key) in [
        ("/app/shortcuts/toggleOverlay", "toggleOverlay"),
        ("/app/shortcuts/unlockOverlay", "unlockOverlay"),
        ("/app/shortcuts/resetOverlay", "resetOverlay"),
        (
            "/app/shortcuts/toggleStatusBarLyrics",
            "toggleStatusBarLyrics",
        ),
        ("/app/shortcuts/toggleListLyrics", "toggleListLyrics"),
        ("/app/shortcuts/toggleNotchLyrics", "toggleNotchLyrics"),
        ("/app/shortcuts/switchLyrics", "switchLyrics"),
    ] {
        if value
            .pointer(pointer)
            .is_some_and(|candidate| !candidate.is_string())
        {
            return Err(error_at_key(raw, key, &format!("{key} 必须是字符串")));
        }
    }
    validate_string_option(
        value,
        raw,
        "/lyrics/providers/mode",
        "mode",
        &["strict", "smart"],
    )?;
    validate_string_option(
        value,
        raw,
        "/lyrics/chineseConversion",
        "chineseConversion",
        &["original", "simplified", "traditional"],
    )?;
    for (pointer, key, options) in [
        (
            "/lyrics/displays/listWindow/appearance/backgroundMode",
            "backgroundMode",
            &["solid", "transparent"] as &[&str],
        ),
        (
            "/lyrics/displays/statusBar/appearance/karaokeStyle",
            "karaokeStyle",
            &["sweep", "highlight"] as &[&str],
        ),
        (
            "/lyrics/displays/statusBar/appearance/alignment",
            "alignment",
            &["left", "center", "right"] as &[&str],
        ),
        (
            "/lyrics/displays/notch/appearance/karaokeStyle",
            "karaokeStyle",
            &["sweep", "highlight"] as &[&str],
        ),
        (
            "/lyrics/displays/notch/leftSlot",
            "leftSlot",
            &["empty", "title", "artist", "artwork", "spectrum"] as &[&str],
        ),
        (
            "/lyrics/displays/notch/rightSlot",
            "rightSlot",
            &["empty", "title", "artist", "artwork", "spectrum"] as &[&str],
        ),
        (
            "/lyrics/displays/notch/layout",
            "layout",
            &["single", "double"] as &[&str],
        ),
        (
            "/lyrics/displays/notch/doubleLineMode",
            "doubleLineMode",
            &["rolling", "alternating"] as &[&str],
        ),
        (
            "/overlay/appearance/backgroundMode",
            "backgroundMode",
            &["solid", "transparent"] as &[&str],
        ),
        (
            "/overlay/appearance/background",
            "background",
            &["glass", "transparent", "solid"] as &[&str],
        ),
        (
            "/overlay/appearance/layout",
            "layout",
            &[
                "single",
                "double",
                "stacked",
                "side_by_side",
                "vertical_single",
                "vertical_double",
            ],
        ),
        (
            "/overlay/appearance/doubleLineMode",
            "doubleLineMode",
            &["rolling", "alternating"],
        ),
        (
            "/overlay/appearance/orientation",
            "orientation",
            &["horizontal", "vertical"],
        ),
        (
            "/overlay/appearance/alignment",
            "alignment",
            &["start", "center", "end", "distributed"],
        ),
        (
            "/overlay/appearance/primaryLinePosition",
            "primaryLinePosition",
            &["first", "second"],
        ),
        (
            "/overlay/appearance/longText",
            "longText",
            &["shrink", "wrap", "marquee"],
        ),
        (
            "/overlay/appearance/secondaryDisplay",
            "secondaryDisplay",
            &[
                "next",
                "translation",
                "romanization",
                "translation_romanization",
            ],
        ),
        (
            "/overlay/appearance/karaokeStyle",
            "karaokeStyle",
            &["sweep", "bounce", "highlight"],
        ),
    ] {
        validate_string_option(value, raw, pointer, key, options)?;
    }

    if let Some(providers) = value.pointer("/lyrics/providers/providers") {
        let items = providers
            .as_array()
            .ok_or_else(|| error_at_key(raw, "providers", "providers 必须是数组"))?;
        for item in items {
            if !item.is_object() {
                return Err(error_at_key(raw, "providers", "每个歌词源必须是对象"));
            }
            if item
                .get("id")
                .is_some_and(|candidate| !candidate.is_string())
            {
                return Err(error_at_key(raw, "id", "歌词源 id 必须是字符串"));
            }
            if item.get("id").is_none() {
                return Err(error_at_key(raw, "providers", "每个歌词源都必须包含 id"));
            }
            if item
                .get("enabled")
                .is_some_and(|candidate| !candidate.is_boolean())
            {
                return Err(error_at_key(raw, "enabled", "enabled 必须是布尔值"));
            }
            if item.get("enabled").is_none() {
                return Err(error_at_key(
                    raw,
                    "providers",
                    "每个歌词源都必须包含 enabled",
                ));
            }
        }
    }
    if let Some(candidate) = value.pointer("/overlay/appearance/fontFamily") {
        let font_family = candidate
            .as_str()
            .ok_or_else(|| error_at_key(raw, "fontFamily", "fontFamily 必须是字符串"))?;
        if font_family.trim().is_empty() {
            return Err(error_at_key(raw, "fontFamily", "fontFamily 不能为空"));
        }
    }
    if let Some(candidate) = value.pointer("/lyrics/providers/amllBaseUrl") {
        let base_url = candidate
            .as_str()
            .ok_or_else(|| error_at_key(raw, "amllBaseUrl", "amllBaseUrl 必须是字符串"))?;
        if base_url.trim().is_empty() {
            return Err(error_at_key(raw, "amllBaseUrl", "amllBaseUrl 不能为空"));
        }
    }
    for key in [
        "activeColor",
        "inactiveColor",
        "solidColor",
        "translationColor",
        "romanizationColor",
        "textShadowColor",
        "textStrokeColor",
    ] {
        let pointer = format!("/overlay/appearance/{key}");
        if let Some(candidate) = value.pointer(&pointer) {
            let color = candidate
                .as_str()
                .ok_or_else(|| error_at_key(raw, key, &format!("{key} 必须是颜色字符串")))?;
            if !is_supported_color(color) {
                return Err(error_at_key(raw, key, &format!("{key} 不是有效颜色")));
            }
        }
    }
    Ok(())
}

fn validate_string_option(
    value: &Value,
    raw: &str,
    pointer: &str,
    key: &str,
    options: &[&str],
) -> Result<(), ConfigDraftError> {
    let Some(candidate) = value.pointer(pointer) else {
        return Ok(());
    };
    let candidate = candidate
        .as_str()
        .ok_or_else(|| error_at_key(raw, key, &format!("{key} 必须是字符串")))?;
    if !options.contains(&candidate) {
        return Err(error_at_key(
            raw,
            key,
            &format!("{key} 可选值：{}", options.join("、")),
        ));
    }
    Ok(())
}

fn validate_language_preference(value: &Value, raw: &str) -> Result<(), ConfigDraftError> {
    let Some(candidate) = value.pointer("/app/language") else {
        return Ok(());
    };
    let candidate = candidate
        .as_str()
        .ok_or_else(|| error_at_key(raw, "language", "language 必须是字符串"))?;
    if is_valid_language_preference(candidate) {
        return Ok(());
    }
    Err(error_at_key(
        raw,
        "language",
        "language 必须是 system 或有效的 BCP 47 语言标签",
    ))
}

fn is_valid_language_preference(candidate: &str) -> bool {
    if candidate == "system" {
        return true;
    }
    let mut subtags = candidate.split('-');
    let primary = subtags.next().unwrap_or_default();
    let primary_valid = (2..=8).contains(&primary.len())
        && primary
            .chars()
            .all(|character| character.is_ascii_alphabetic());
    let remaining_valid = subtags.all(|subtag| {
        (1..=8).contains(&subtag.len())
            && subtag
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
    });
    candidate.len() <= 64 && primary_valid && remaining_valid
}

fn check_keys(value: &Value, raw: &str, allowed: &[&str]) -> Result<(), ConfigDraftError> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(error_at_key(raw, key, &format!("未知配置字段：{key}")));
        }
    }
    Ok(())
}

fn validate_numeric_ranges(value: &Value, raw: &str) -> Result<(), ConfigDraftError> {
    let checks = [
        (
            "fontSize",
            value.pointer("/overlay/appearance/fontSize"),
            16.0,
            72.0,
        ),
        (
            "fontWeight",
            value.pointer("/overlay/appearance/fontWeight"),
            400.0,
            800.0,
        ),
        (
            "secondaryFontWeight",
            value.pointer("/overlay/appearance/secondaryFontWeight"),
            400.0,
            800.0,
        ),
        (
            "lineHeight",
            value.pointer("/overlay/appearance/lineHeight"),
            0.8,
            2.0,
        ),
        (
            "autoApplyThreshold",
            value.pointer("/lyrics/providers/autoApplyThreshold"),
            0.0,
            100.0,
        ),
        (
            "autoSearchDebounceMs",
            value.pointer("/lyrics/providers/autoSearchDebounceMs"),
            0.0,
            5_000.0,
        ),
        (
            "capabilityPreferenceTolerance",
            value.pointer("/lyrics/providers/capabilityPreferenceTolerance"),
            0.0,
            20.0,
        ),
        (
            "title",
            value.pointer("/lyrics/providers/matchWeights/title"),
            0.0,
            100.0,
        ),
        (
            "artist",
            value.pointer("/lyrics/providers/matchWeights/artist"),
            0.0,
            100.0,
        ),
        (
            "album",
            value.pointer("/lyrics/providers/matchWeights/album"),
            0.0,
            100.0,
        ),
        (
            "duration",
            value.pointer("/lyrics/providers/matchWeights/duration"),
            0.0,
            100.0,
        ),
        (
            "fontSize",
            value.pointer("/lyrics/displays/statusBar/appearance/fontSize"),
            10.0,
            32.0,
        ),
        (
            "verticalOffset",
            value.pointer("/lyrics/displays/statusBar/appearance/verticalOffset"),
            -6.0,
            6.0,
        ),
        (
            "width",
            value.pointer("/lyrics/displays/statusBar/appearance/width"),
            120.0,
            360.0,
        ),
        (
            "maxWidth",
            value.pointer("/lyrics/displays/statusBar/appearance/maxWidth"),
            120.0,
            720.0,
        ),
        (
            "fontSize",
            value.pointer("/lyrics/displays/listWindow/appearance/fontSize"),
            12.0,
            56.0,
        ),
        (
            "backgroundOpacity",
            value.pointer("/lyrics/displays/listWindow/appearance/backgroundOpacity"),
            0.0,
            1.0,
        ),
        (
            "secondaryLineGap",
            value.pointer("/lyrics/displays/listWindow/appearance/secondaryLineGap"),
            0.0,
            32.0,
        ),
        (
            "activeOpacity",
            value.pointer("/lyrics/displays/listWindow/appearance/activeOpacity"),
            0.0,
            1.0,
        ),
        (
            "inactiveOpacity",
            value.pointer("/lyrics/displays/listWindow/appearance/inactiveOpacity"),
            0.0,
            1.0,
        ),
        (
            "fontSize",
            value.pointer("/lyrics/displays/notch/appearance/fontSize"),
            12.0,
            32.0,
        ),
        (
            "fontWeight",
            value.pointer("/lyrics/displays/notch/appearance/fontWeight"),
            400.0,
            800.0,
        ),
        (
            "secondaryFontWeight",
            value.pointer("/lyrics/displays/notch/appearance/secondaryFontWeight"),
            400.0,
            800.0,
        ),
        (
            "lineGap",
            value.pointer("/lyrics/displays/notch/appearance/lineGap"),
            0.0,
            32.0,
        ),
        (
            "maxWidth",
            value.pointer("/lyrics/displays/notch/appearance/maxWidth"),
            320.0,
            640.0,
        ),
        (
            "expandedMaxWidth",
            value.pointer("/lyrics/displays/notch/appearance/expandedMaxWidth"),
            440.0,
            640.0,
        ),
        (
            "topBorderRadius",
            value.pointer("/lyrics/displays/notch/appearance/topBorderRadius"),
            0.0,
            40.0,
        ),
        (
            "opacity",
            value.pointer("/overlay/appearance/opacity"),
            0.2,
            1.0,
        ),
        (
            "backgroundOpacity",
            value.pointer("/overlay/appearance/backgroundOpacity"),
            0.0,
            1.0,
        ),
        (
            "backgroundBlur",
            value.pointer("/overlay/appearance/backgroundBlur"),
            0.0,
            40.0,
        ),
        (
            "backgroundRadius",
            value.pointer("/overlay/appearance/backgroundRadius"),
            0.0,
            64.0,
        ),
        (
            "backgroundPaddingX",
            value.pointer("/overlay/appearance/backgroundPaddingX"),
            0.0,
            64.0,
        ),
        (
            "backgroundPaddingY",
            value.pointer("/overlay/appearance/backgroundPaddingY"),
            0.0,
            64.0,
        ),
        (
            "lineGap",
            value.pointer("/overlay/appearance/lineGap"),
            0.0,
            32.0,
        ),
        (
            "textShadowOffsetX",
            value.pointer("/overlay/appearance/textShadowOffsetX"),
            -20.0,
            20.0,
        ),
        (
            "textShadowOffsetY",
            value.pointer("/overlay/appearance/textShadowOffsetY"),
            -20.0,
            20.0,
        ),
        (
            "textShadowBlur",
            value.pointer("/overlay/appearance/textShadowBlur"),
            0.0,
            40.0,
        ),
        (
            "textStrokeWidth",
            value.pointer("/overlay/appearance/textStrokeWidth"),
            0.0,
            8.0,
        ),
        (
            "secondaryFontScale",
            value.pointer("/overlay/appearance/secondaryFontScale"),
            0.35,
            1.0,
        ),
        (
            "translationFontScale",
            value.pointer("/overlay/appearance/translationFontScale"),
            0.35,
            1.0,
        ),
        (
            "romanizationFontScale",
            value.pointer("/overlay/appearance/romanizationFontScale"),
            0.35,
            1.0,
        ),
    ];
    for (key, candidate, minimum, maximum) in checks {
        if let Some(candidate) = candidate {
            let number = candidate
                .as_f64()
                .ok_or_else(|| error_at_key(raw, key, &format!("{key} 必须是数字")))?;
            if !number.is_finite() || number < minimum || number > maximum {
                return Err(error_at_key(
                    raw,
                    key,
                    &format!("{key} 必须在 {minimum}–{maximum} 之间"),
                ));
            }
        }
    }
    if let Some(candidate) = value.pointer("/lyrics/providers/autoSearchDebounceMs") {
        if let Some(milliseconds) = candidate.as_u64() {
            if milliseconds % 100 != 0 {
                return Err(error_at_key(
                    raw,
                    "autoSearchDebounceMs",
                    "autoSearchDebounceMs 必须是 100 毫秒的整数倍",
                ));
            }
        }
    }
    Ok(())
}

fn merge_json(base: &mut Value, override_value: Value) {
    match (base, override_value) {
        (Value::Object(base), Value::Object(override_object)) => {
            for (key, value) in override_object {
                if let Some(existing) = base.get_mut(&key) {
                    merge_json(existing, value);
                } else {
                    base.insert(key, value);
                }
            }
        }
        (base, value) => *base = value,
    }
}

fn error_at_key(raw: &str, key: &str, message: &str) -> ConfigDraftError {
    let needle = format!("\"{key}\"");
    let offset = raw.find(&needle).unwrap_or(0);
    let prefix = &raw[..offset];
    ConfigDraftError {
        message: message.into(),
        line: prefix
            .chars()
            .filter(|character| *character == '\n')
            .count()
            + 1,
        column: prefix
            .rsplit('\n')
            .next()
            .map(|line| line.chars().count() + 1)
            .unwrap_or(1),
    }
}

fn internal_draft_error(error: impl std::fmt::Display) -> ConfigDraftError {
    ConfigDraftError {
        message: format!("处理配置失败：{error}"),
        line: 1,
        column: 1,
    }
}

fn color_fields(style: &OverlayStyleSettings) -> [(&'static str, &str); 7] {
    [
        ("高亮颜色", &style.active_color),
        ("未唱颜色", &style.inactive_color),
        ("背景颜色", &style.solid_color),
        ("翻译颜色", &style.translation_color),
        ("音译颜色", &style.romanization_color),
        ("文字阴影颜色", &style.text_shadow_color),
        ("文字描边颜色", &style.text_stroke_color),
    ]
}

fn normalize_display_font_weight(value: u16) -> u16 {
    [400_u16, 500, 600, 700, 800]
        .into_iter()
        .min_by_key(|candidate| (*candidate).abs_diff(value))
        .unwrap_or(600)
}

fn is_supported_color(value: &str) -> bool {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#') {
        return matches!(hex.len(), 3 | 4 | 6 | 8)
            && hex.chars().all(|character| character.is_ascii_hexdigit());
    }
    if value.eq_ignore_ascii_case("transparent") || value.eq_ignore_ascii_case("currentcolor") {
        return true;
    }
    let lower = value.to_ascii_lowercase();
    let functions = [
        "rgb(", "rgba(", "hsl(", "hsla(", "hwb(", "lab(", "lch(", "oklab(", "oklch(", "color(",
    ];
    functions.iter().any(|prefix| lower.starts_with(prefix))
        && lower.ends_with(')')
        && lower.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || character.is_ascii_whitespace()
                || matches!(character, '(' | ')' | ',' | '.' | '%' | '/' | '+' | '-')
        })
}
