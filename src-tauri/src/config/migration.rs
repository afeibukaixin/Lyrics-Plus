#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDraftError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDraftValidation {
    pub valid: bool,
    pub error: Option<ConfigDraftError>,
    pub normalized_json: Option<String>,
    pub effective_config: AppConfig,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigEditorData {
    pub default_jsonc: String,
    pub user_json: String,
    pub revision: u64,
    pub validation: ConfigDraftValidation,
}

struct ParsedDraft {
    config: AppConfig,
    normalized_json: String,
    #[cfg(test)]
    migrated: bool,
}

struct ConfigStoreState {
    value: AppConfig,
    revision: u64,
    source_raw: String,
    source_error: Option<ConfigDraftError>,
    comment_language: UiLanguage,
}

pub struct ConfigStore {
    path: PathBuf,
    state: RwLock<ConfigStoreState>,
}

fn configured_comment_language(preference: &LanguagePreference) -> UiLanguage {
    if preference.uses_native_chinese() {
        UiLanguage::ZhCn
    } else {
        UiLanguage::EnUs
    }
}

fn migrate_legacy_provider_order(settings: &mut ProviderSettings) {
    let is_legacy_default = settings.mode == ProviderOrderMode::Smart
        && settings.providers.len() == 4
        && settings.providers.iter().all(|provider| provider.enabled)
        && settings
            .providers
            .iter()
            .map(|provider| provider.id.as_str())
            .eq(["netease", "qqmusic", "kugou", "lrclib"]);
    if is_legacy_default {
        *settings = ProviderSettings::default();
    }
}

fn migrate_v13_provider_defaults(settings: &mut ProviderSettings) {
    let is_old_default = settings.mode == ProviderOrderMode::Smart
        && settings.auto_apply_threshold == 60
        && settings.providers.len() == 4
        && settings.providers.iter().all(|provider| provider.enabled)
        && settings
            .providers
            .iter()
            .map(|provider| provider.id.as_str())
            .eq(["lrclib", "kugou", "qqmusic", "netease"]);
    if is_old_default {
        *settings = ProviderSettings::default();
    }
}

fn migrate_v45_provider_sources(settings: &mut ProviderSettings) {
    for id in ["kuwo", "amll_ttml", "migu", "musixmatch"] {
        if !settings.providers.iter().any(|provider| provider.id == id) {
            settings
                .providers
                .push(crate::lyrics::provider::ProviderPreference {
                    id: id.into(),
                    enabled: true,
                });
        }
    }
}

fn migrate_v58_enable_all_provider_sources(settings: &mut ProviderSettings) {
    let is_old_default = settings.providers.len() == 8
        && settings.providers.iter().all(|provider| match provider.id.as_str() {
            "lrclib" | "kugou" | "qqmusic" | "netease" | "kuwo" | "migu" => {
                provider.enabled
            }
            "amll_ttml" | "musixmatch" => !provider.enabled,
            _ => false,
        });
    if is_old_default {
        for provider in &mut settings.providers {
            provider.enabled = true;
        }
    }
}

fn migrate_v59_switch_lyrics_shortcut(user: &mut Value) {
    let Some(shortcuts) = user
        .get_mut("app")
        .and_then(Value::as_object_mut)
        .and_then(|app| app.get_mut("shortcuts"))
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    if shortcuts.contains_key("switchLyrics") {
        return;
    }

    // 通过解析后的 ID 判断快捷键别名，避免升级时覆盖用户已有绑定。
    let default_id = DEFAULT_SWITCH_LYRICS_SHORTCUT
        .parse::<tauri_plugin_global_shortcut::Shortcut>()
        .ok()
        .map(|shortcut| shortcut.id());
    let conflicts = default_id.is_some_and(|default_id| {
        shortcuts.values().filter_map(Value::as_str).any(|value| {
            value
                .trim()
                .parse::<tauri_plugin_global_shortcut::Shortcut>()
                .ok()
                .is_some_and(|shortcut| shortcut.id() == default_id)
        })
    });
    shortcuts.insert(
        "switchLyrics".into(),
        Value::from(if conflicts {
            ""
        } else {
            DEFAULT_SWITCH_LYRICS_SHORTCUT
        }),
    );
}

fn migrate_legacy_overlay_layout(
    user: &mut Value,
    version: u16,
    raw: &str,
) -> Result<bool, ConfigDraftError> {
    let Some(appearance) = user
        .get_mut("overlay")
        .and_then(Value::as_object_mut)
        .and_then(|overlay| overlay.get_mut("appearance"))
        .and_then(Value::as_object_mut)
    else {
        return Ok(false);
    };
    let Some(layout) = appearance
        .get("layout")
        .and_then(Value::as_str)
        .map(str::to_owned)
    else {
        return Ok(false);
    };

    let legacy = match layout.as_str() {
        "single" if version < 6 => Some(("single", "horizontal")),
        "stacked" | "side_by_side" => Some(("double", "horizontal")),
        "vertical_single" => Some(("single", "vertical")),
        "vertical_double" => Some(("double", "vertical")),
        _ => None,
    };
    let Some((next_layout, orientation)) = legacy else {
        return Ok(false);
    };
    if version >= 6 {
        return Err(error_at_key(
            raw,
            "layout",
            "layout 只支持 single 或 double；请通过 orientation 设置横竖方向",
        ));
    }
    if appearance.contains_key("orientation") {
        return Err(error_at_key(
            raw,
            "orientation",
            "旧版复合 layout 不能与 orientation 同时设置",
        ));
    }
    appearance.insert("layout".into(), Value::from(next_layout));
    appearance.insert("orientation".into(), Value::from(orientation));
    Ok(true)
}

fn migrate_v32_display_appearances(user: &mut Value, version: u16) {
    if version >= 33 {
        return;
    }
    let Some(displays) = user
        .get_mut("lyrics")
        .and_then(|value| value.get_mut("displays"))
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    if let Some(notch) = displays.get_mut("notch").and_then(Value::as_object_mut) {
        let mut appearance = serde_json::Map::new();
        if let Some(value) = notch.remove("fontSize") {
            appearance.insert("fontSize".into(), value);
        }
        if let Some(value) = notch.remove("backgroundOpacity") {
            appearance.insert("backgroundOpacity".into(), value);
        }
        if !appearance.is_empty() {
            notch.insert("appearance".into(), Value::Object(appearance));
        }
    }
}

fn migrate_v34_lyrics_base_appearance(user: &mut Value, version: u16) {
    if version >= 34 {
        return;
    }
    let Some(appearance) = user
        .pointer_mut("/lyrics/displays/listWindow/appearance")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    if let Some(secondary) = appearance.remove("secondaryColor") {
        appearance.insert("translationColor".into(), secondary.clone());
        appearance.insert("romanizationColor".into(), secondary);
    }
}

fn migrate_v37_notch_width(user: &mut Value, version: u16) {
    if version >= 37 {
        return;
    }
    let Some(max_width) = user.pointer_mut("/lyrics/displays/notch/appearance/maxWidth") else {
        return;
    };
    if max_width.as_u64() == Some(404) {
        *max_width = Value::from(640);
    }
}

fn migrate_v38_notch_line_count(user: &mut Value, version: u16) {
    if version >= 38 {
        return;
    }
    let Some(notch) = user
        .pointer_mut("/lyrics/displays/notch")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    notch.remove("expandedOnHover");
    notch.insert("showTwoLines".into(), Value::from(false));
}

fn migrate_v39_notch_supporting_tracks(user: &mut Value, version: u16) {
    if version >= 39 {
        return;
    }
    let Some(notch) = user
        .pointer_mut("/lyrics/displays/notch")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    if !notch.contains_key("showTranslation") {
        notch.insert("showTranslation".into(), Value::from(false));
    }
    if !notch.contains_key("showRomanization") {
        notch.insert("showRomanization".into(), Value::from(false));
    }
}

fn migrate_v40_notch_colors(user: &mut Value, version: u16) {
    if version >= 40 {
        return;
    }
    let Some(appearance) = user
        .pointer_mut("/lyrics/displays/notch/appearance")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    if let Some(secondary) = appearance.remove("secondaryColor") {
        for key in ["inactiveColor", "translationColor", "romanizationColor"] {
            appearance
                .entry(key.to_string())
                .or_insert_with(|| secondary.clone());
        }
    }
}

fn migrate_v41_fixed_notch_background(user: &mut Value, version: u16) {
    if version >= 41 {
        return;
    }
    let Some(appearance) = user
        .pointer_mut("/lyrics/displays/notch/appearance")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    for key in ["backgroundColor", "backgroundOpacity", "backgroundBlur"] {
        appearance.remove(key);
    }
}

fn migrate_v42_list_preferences(user: &mut Value, version: u16) {
    if version >= 42 {
        return;
    }
    let Some(list_window) = user
        .pointer_mut("/lyrics/displays/listWindow")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    list_window
        .entry("alwaysOnTop")
        .or_insert_with(|| Value::from(false));
    let Some(appearance) = list_window
        .get_mut("appearance")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    appearance
        .entry("backgroundOpacity")
        .or_insert_with(|| Value::from(1.0));
    appearance
        .entry("backgroundMode")
        .or_insert_with(|| Value::from("solid"));
}

fn migrate_v48_notch_mode(user: &mut Value, version: u16) {
    if version >= 48 {
        return;
    }
    let Some(notch) = user
        .pointer_mut("/lyrics/displays/notch")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    notch
        .entry("showLyrics")
        .or_insert_with(|| Value::from(true));
    notch
        .entry("leftSlot")
        .or_insert_with(|| Value::from("artwork"));
    notch
        .entry("rightSlot")
        .or_insert_with(|| Value::from("spectrum"));
}

fn migrate_v49_notch_width(user: &mut Value, version: u16) {
    if version >= 49 {
        return;
    }
    let Some(appearance) = user
        .pointer_mut("/lyrics/displays/notch/appearance")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    let max_width = appearance
        .get("maxWidth")
        .and_then(Value::as_u64)
        .unwrap_or(320);
    let expanded_max_width = max_width.max(440).min(640);
    appearance
        .entry("expandedMaxWidth")
        .or_insert_with(|| Value::from(expanded_max_width));
}

fn migrate_v50_notch_layout(user: &mut Value, version: u16) {
    if version >= 50 {
        return;
    }
    let Some(notch) = user
        .pointer_mut("/lyrics/displays/notch")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    let layout = notch
        .remove("showTwoLines")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    notch.insert(
        "layout".into(),
        Value::from(if layout { "double" } else { "single" }),
    );
}

fn migrate_v54_notch_double_line_settings(user: &mut Value, version: u16) {
    if version >= 54 {
        return;
    }
    let Some(notch) = user
        .pointer_mut("/lyrics/displays/notch")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    notch
        .entry("doubleLineMode")
        .or_insert_with(|| Value::from("rolling"));
    if let Some(appearance) = notch.get_mut("appearance").and_then(Value::as_object_mut) {
        appearance
            .entry("secondaryFontWeight")
            .or_insert_with(|| Value::from(500));
    }
}

fn migrate_v57_chinese_conversion(user: &mut Value, version: u16) {
    if version >= 57 {
        return;
    }
    if let Some(lyrics) = user.pointer_mut("/lyrics").and_then(Value::as_object_mut) {
        lyrics
            .entry("chineseConversion")
            .or_insert_with(|| Value::from("original"));
    }
}

fn remove_retired_fullscreen_space_preferences(user: &mut Value) {
    if let Some(overlay) = user.pointer_mut("/overlay").and_then(Value::as_object_mut) {
        overlay.remove("joinOtherAppsFullscreen");
    }
    if let Some(notch) = user
        .pointer_mut("/lyrics/displays/notch")
        .and_then(Value::as_object_mut)
    {
        notch.remove("joinOtherAppsFullscreen");
    }
}

fn migrate_status_bar_status_item_fields(user: &mut Value) {
    let Some(status_bar) = user
        .pointer_mut("/lyrics/displays/statusBar")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    status_bar.remove("locked");
    status_bar.remove("maxCharacters");
    status_bar.remove("showTrayIcon");

    let Some(appearance) = status_bar
        .get_mut("appearance")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    if !appearance.contains_key("width") {
        if let Some(width) = appearance.remove("maxWidth") {
            appearance.insert("width".into(), width);
        }
    } else {
        appearance.remove("maxWidth");
    }
    for key in [
        "backgroundColor",
        "backgroundOpacity",
        "backgroundBlur",
        "borderRadius",
        "paddingX",
        "paddingY",
    ] {
        appearance.remove(key);
    }
}
