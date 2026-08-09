use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri_plugin_global_shortcut::Shortcut;

use crate::commands::{
    KaraokeStyle, LongTextMode, OverlayAlignment, OverlayBackground, OverlayBackgroundMode,
    OverlayLayout, OverlayOrientation, OverlayStyleSettings, SecondaryDisplayMode,
};
use crate::language::{detect_config_comment_language, ConfigComment, UiLanguage};
use crate::lyrics::provider::{
    complete_settings, validate_settings, ProviderOrderMode, ProviderSettings,
};
use crate::player::PlayerSelection;
use crate::storage::Storage;

pub const CONFIG_SCHEMA_VERSION: u16 = 14;

fn canonical_config_jsonc(value: &AppConfig, language: UiLanguage) -> Result<String, String> {
    let json =
        serde_json::to_string_pretty(value).map_err(|error| format!("序列化配置失败：{error}"))?;
    let mut output = String::with_capacity(json.len() + 1_200);
    for line in json.lines() {
        let comment = match line {
            line if line.starts_with("  \"schemaVersion\":") => {
                Some(("  ", ConfigComment::SchemaVersion))
            }
            line if line.starts_with("    \"uiFontScale\":") => {
                Some(("    ", ConfigComment::UiFontScale))
            }
            line if line.starts_with("    \"language\":") => {
                Some(("    ", ConfigComment::Language))
            }
            line if line.starts_with("    \"playerSelection\":") => {
                Some(("    ", ConfigComment::PlayerSelection))
            }
            line if line.starts_with("    \"hideDockIcon\":") => {
                Some(("    ", ConfigComment::HideDockIcon))
            }
            line if line.starts_with("    \"shortcuts\":") => {
                Some(("    ", ConfigComment::Shortcuts))
            }
            line if line.starts_with("      \"autoApplyThreshold\":") => {
                Some(("      ", ConfigComment::AutoApplyThreshold))
            }
            line if line.starts_with("      \"mode\":") => {
                Some(("      ", ConfigComment::ProviderMode))
            }
            line if line.starts_with("      \"providers\":") => {
                Some(("      ", ConfigComment::Providers))
            }
            line if line.starts_with("    \"visible\":") => {
                Some(("    ", ConfigComment::OverlayState))
            }
            line if line.starts_with("    \"hideWhenNotPlaying\":") => {
                Some(("    ", ConfigComment::HideWhenNotPlaying))
            }
            line if line.starts_with("      \"fontSize\":") => {
                Some(("      ", ConfigComment::FontSize))
            }
            line if line.starts_with("      \"opacity\":") => {
                Some(("      ", ConfigComment::Opacity))
            }
            line if line.starts_with("      \"backgroundOpacity\":") => {
                Some(("      ", ConfigComment::BackgroundOpacity))
            }
            line if line.starts_with("      \"backgroundBlur\":") => {
                Some(("      ", ConfigComment::BackgroundBlur))
            }
            line if line.starts_with("      \"backgroundMode\":") => {
                Some(("      ", ConfigComment::BackgroundMode))
            }
            line if line.starts_with("      \"background\":") => {
                Some(("      ", ConfigComment::Background))
            }
            line if line.starts_with("      \"layout\":") => {
                Some(("      ", ConfigComment::Layout))
            }
            line if line.starts_with("      \"alignment\":") => {
                Some(("      ", ConfigComment::Alignment))
            }
            line if line.starts_with("      \"longText\":") => {
                Some(("      ", ConfigComment::LongText))
            }
            line if line.starts_with("      \"secondaryDisplay\":") => {
                Some(("      ", ConfigComment::SecondaryDisplay))
            }
            line if line.starts_with("      \"autoCenterWithTranslationOrRomanization\":") => {
                Some(("      ", ConfigComment::AutoCenter))
            }
            line if line.starts_with("      \"karaokeStyle\":") => {
                Some(("      ", ConfigComment::KaraokeStyle))
            }
            line if line.starts_with("      \"secondaryFontScale\":") => {
                Some(("      ", ConfigComment::SecondaryFontScale))
            }
            _ => None,
        };
        if let Some((indent, comment)) = comment {
            output.push_str(indent);
            output.push_str("// ");
            output.push_str(language.config_comment(comment));
            output.push('\n');
        }
        output.push_str(line);
        output.push('\n');
    }
    Ok(output)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppConfig {
    pub schema_version: u16,
    pub app: AppPreferences,
    pub lyrics: LyricsPreferences,
    pub overlay: OverlayPreferences,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            app: AppPreferences::default(),
            lyrics: LyricsPreferences::default(),
            overlay: OverlayPreferences::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppPreferences {
    pub ui_font_scale: u16,
    pub language: LanguagePreference,
    pub player_selection: PlayerSelection,
    pub hide_dock_icon: bool,
    pub shortcuts: GlobalShortcutSettings,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            ui_font_scale: 100,
            language: LanguagePreference::default(),
            player_selection: PlayerSelection::Auto,
            hide_dock_icon: false,
            shortcuts: GlobalShortcutSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct LanguagePreference(String);

impl LanguagePreference {
    pub fn uses_native_chinese(&self) -> bool {
        self.0 == "zh-CN"
    }

    pub fn is_valid(&self) -> bool {
        is_valid_language_preference(&self.0)
    }
}

impl Default for LanguagePreference {
    fn default() -> Self {
        Self("system".into())
    }
}

impl From<&str> for LanguagePreference {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct GlobalShortcutSettings {
    pub toggle_overlay: String,
    pub unlock_overlay: String,
    pub reset_overlay: String,
}

impl Default for GlobalShortcutSettings {
    fn default() -> Self {
        Self {
            toggle_overlay: "CommandOrControl+Shift+KeyL".into(),
            unlock_overlay: "CommandOrControl+Shift+KeyU".into(),
            reset_overlay: "CommandOrControl+Shift+Digit0".into(),
        }
    }
}

impl GlobalShortcutSettings {
    pub fn parsed(&self) -> Result<[Shortcut; 3], String> {
        let entries = [
            ("显示 / 隐藏桌面歌词", self.toggle_overlay.as_str()),
            ("锁定 / 解锁桌面歌词", self.unlock_overlay.as_str()),
            ("复位并显示桌面歌词", self.reset_overlay.as_str()),
        ];
        let mut parsed = Vec::with_capacity(entries.len());
        for (label, value) in entries {
            let shortcut = value
                .parse::<Shortcut>()
                .map_err(|error| format!("{label}快捷键无效：{error}"))?;
            if shortcut.mods.is_empty() {
                return Err(format!("{label}快捷键必须包含至少一个修饰键"));
            }
            if parsed
                .iter()
                .any(|existing: &Shortcut| existing.id() == shortcut.id())
            {
                return Err("三个全局快捷键不能重复".into());
            }
            parsed.push(shortcut);
        }
        parsed
            .try_into()
            .map_err(|_| "全局快捷键配置不完整".to_string())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LyricsPreferences {
    pub providers: ProviderSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OverlayPreferences {
    pub visible: bool,
    pub locked: bool,
    pub hide_when_not_playing: bool,
    pub appearance: OverlayAppearance,
}

impl Default for OverlayPreferences {
    fn default() -> Self {
        Self {
            visible: true,
            locked: false,
            hide_when_not_playing: false,
            appearance: OverlayAppearance::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OverlayAppearance {
    pub font_size: u16,
    pub active_color: String,
    pub inactive_color: String,
    pub opacity: f64,
    pub background_opacity: f64,
    pub background_blur: f64,
    pub background_mode: OverlayBackgroundMode,
    pub background: OverlayBackground,
    pub solid_color: String,
    pub layout: OverlayLayout,
    pub orientation: OverlayOrientation,
    pub alignment: OverlayAlignment,
    pub long_text: LongTextMode,
    pub secondary_display: SecondaryDisplayMode,
    pub auto_center_with_translation_or_romanization: bool,
    pub karaoke_style: KaraokeStyle,
    pub secondary_font_scale: f64,
    pub translation_font_scale: f64,
    pub romanization_font_scale: f64,
    pub translation_color: String,
    pub romanization_color: String,
}

impl Default for OverlayAppearance {
    fn default() -> Self {
        Self::from(&OverlayStyleSettings::default())
    }
}

impl From<&OverlayStyleSettings> for OverlayAppearance {
    fn from(style: &OverlayStyleSettings) -> Self {
        Self {
            font_size: style.font_size,
            active_color: style.active_color.clone(),
            inactive_color: style.inactive_color.clone(),
            opacity: style.opacity,
            background_opacity: style.background_opacity,
            background_blur: style.background_blur,
            background_mode: style.background_mode,
            background: style.background,
            solid_color: style.solid_color.clone(),
            layout: style.layout,
            orientation: style.orientation,
            alignment: style.alignment,
            long_text: style.long_text,
            secondary_display: style.secondary_display,
            auto_center_with_translation_or_romanization: style
                .auto_center_with_translation_or_romanization,
            karaoke_style: style.karaoke_style,
            secondary_font_scale: style.secondary_font_scale,
            translation_font_scale: style.translation_font_scale,
            romanization_font_scale: style.romanization_font_scale,
            translation_color: style.translation_color.clone(),
            romanization_color: style.romanization_color.clone(),
        }
    }
}

impl OverlayAppearance {
    pub fn into_style(self) -> OverlayStyleSettings {
        OverlayStyleSettings {
            font_size: self.font_size,
            active_color: self.active_color,
            inactive_color: self.inactive_color,
            opacity: self.opacity,
            background_opacity: self.background_opacity,
            background_blur: self.background_blur,
            background_mode: self.background_mode,
            background: self.background,
            solid_color: self.solid_color,
            layout: self.layout,
            orientation: self.orientation,
            alignment: self.alignment,
            long_text: self.long_text,
            secondary_display: self.secondary_display,
            auto_center_with_translation_or_romanization: self
                .auto_center_with_translation_or_romanization,
            translation_enabled: false,
            romanization_enabled: false,
            karaoke_style: self.karaoke_style,
            secondary_font_scale: self.secondary_font_scale,
            translation_font_scale: self.translation_font_scale,
            romanization_font_scale: self.romanization_font_scale,
            translation_color: self.translation_color,
            romanization_color: self.romanization_color,
            horizontal_max_width: None,
            vertical_max_height: None,
        }
        .normalized()
    }
}

impl AppConfig {
    pub fn normalized(mut self) -> Result<Self, String> {
        if self.schema_version > CONFIG_SCHEMA_VERSION {
            return Err(format!(
                "配置文件版本 {} 高于当前支持的版本 {}",
                self.schema_version, CONFIG_SCHEMA_VERSION
            ));
        }
        self.schema_version = CONFIG_SCHEMA_VERSION;
        self.app.ui_font_scale = normalize_ui_font_scale(self.app.ui_font_scale);
        self.app.shortcuts.parsed()?;
        let normalized_style = self.overlay.appearance.clone().into_style();
        for (name, color) in color_fields(&normalized_style) {
            if !is_supported_color(color) {
                return Err(format!("{name}不是有效的颜色值"));
            }
        }
        validate_settings(&self.lyrics.providers)?;
        complete_settings(&mut self.lyrics.providers);
        self.overlay.appearance = OverlayAppearance::from(&normalized_style);
        Ok(self)
    }
}

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

pub fn normalize_ui_font_scale(value: u16) -> u16 {
    let clamped = value.clamp(80, 150);
    (((clamped as u32 + 5) / 10) * 10).clamp(80, 150) as u16
}

pub fn migrate_v1_font_scale(value: u16) -> u16 {
    normalize_ui_font_scale(((value as f64 / 1.2) / 10.0).round() as u16 * 10)
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
    validate_known_fields(&user, raw)?;
    validate_field_types_and_options(&user, raw)?;

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
    let migrated_layout = migrate_legacy_overlay_layout(&mut user, version, raw)?;
    #[cfg(test)]
    let migrated = version < CONFIG_SCHEMA_VERSION
        || user
            .get("app")
            .and_then(Value::as_object)
            .is_some_and(|app| app.contains_key("autostart"))
        || migrated_layout;
    #[cfg(not(test))]
    let _ = migrated_layout;
    if version < 2 {
        if let Some(scale) = user
            .get_mut("app")
            .and_then(Value::as_object_mut)
            .and_then(|app| app.get_mut("uiFontScale"))
        {
            let old = scale
                .as_u64()
                .ok_or_else(|| error_at_key(raw, "uiFontScale", "uiFontScale 必须是数字"))?
                as u16;
            *scale = Value::from(migrate_v1_font_scale(old));
        }
    }
    if let Some(app) = user.get_mut("app").and_then(Value::as_object_mut) {
        app.remove("autostart");
    }
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
        check_keys(
            app,
            raw,
            &[
                "uiFontScale",
                "language",
                "playerSelection",
                "hideDockIcon",
                "autostart",
                "shortcuts",
            ],
        )?;
        if let Some(shortcuts) = app.get("shortcuts") {
            check_keys(
                shortcuts,
                raw,
                &["toggleOverlay", "unlockOverlay", "resetOverlay"],
            )?;
        }
    }
    if let Some(lyrics) = value.get("lyrics") {
        check_keys(lyrics, raw, &["providers"])?;
        if let Some(providers) = lyrics.get("providers") {
            check_keys(providers, raw, &["mode", "providers", "autoApplyThreshold"])?;
            if let Some(items) = providers.get("providers").and_then(Value::as_array) {
                for item in items {
                    check_keys(item, raw, &["id", "enabled"])?;
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
                    "fontSize",
                    "activeColor",
                    "inactiveColor",
                    "opacity",
                    "backgroundOpacity",
                    "backgroundBlur",
                    "backgroundMode",
                    "background",
                    "solidColor",
                    "layout",
                    "orientation",
                    "alignment",
                    "longText",
                    "secondaryDisplay",
                    "autoCenterWithTranslationOrRomanization",
                    "karaokeStyle",
                    "secondaryFontScale",
                    "translationFontScale",
                    "romanizationFontScale",
                    "translationColor",
                    "romanizationColor",
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
    for (pointer, key) in [
        ("/app/autostart", "autostart"),
        ("/app/hideDockIcon", "hideDockIcon"),
        ("/overlay/visible", "visible"),
        ("/overlay/locked", "locked"),
        ("/overlay/hideWhenNotPlaying", "hideWhenNotPlaying"),
        (
            "/overlay/appearance/autoCenterWithTranslationOrRomanization",
            "autoCenterWithTranslationOrRomanization",
        ),
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
        ("/app/uiFontScale", "uiFontScale"),
        ("/lyrics/providers/autoApplyThreshold", "autoApplyThreshold"),
        ("/overlay/appearance/fontSize", "fontSize"),
    ] {
        if value
            .pointer(pointer)
            .is_some_and(|candidate| candidate.as_u64().is_none())
        {
            return Err(error_at_key(raw, key, &format!("{key} 必须是整数")));
        }
    }
    validate_language_preference(value, raw)?;
    validate_string_option(
        value,
        raw,
        "/app/playerSelection",
        "playerSelection",
        &["auto", "apple_music", "spotify"],
    )?;
    for (pointer, key) in [
        ("/app/shortcuts/toggleOverlay", "toggleOverlay"),
        ("/app/shortcuts/unlockOverlay", "unlockOverlay"),
        ("/app/shortcuts/resetOverlay", "resetOverlay"),
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
    for (pointer, key, options) in [
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
            "/overlay/appearance/orientation",
            "orientation",
            &["horizontal", "vertical"],
        ),
        (
            "/overlay/appearance/alignment",
            "alignment",
            &["center", "distributed"],
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

    for key in [
        "activeColor",
        "inactiveColor",
        "solidColor",
        "translationColor",
        "romanizationColor",
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
            "uiFontScale",
            value.pointer("/app/uiFontScale"),
            80.0,
            150.0,
        ),
        (
            "fontSize",
            value.pointer("/overlay/appearance/fontSize"),
            16.0,
            72.0,
        ),
        (
            "autoApplyThreshold",
            value.pointer("/lyrics/providers/autoApplyThreshold"),
            0.0,
            100.0,
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

fn color_fields(style: &OverlayStyleSettings) -> [(&'static str, &str); 5] {
    [
        ("高亮颜色", &style.active_color),
        ("未唱颜色", &style.inactive_color),
        ("背景颜色", &style.solid_color),
        ("翻译颜色", &style.translation_color),
        ("音译颜色", &style.romanization_color),
    ]
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

impl ConfigStore {
    pub fn load(app_dir: &Path, storage: &Storage) -> Result<(Self, bool), String> {
        let path = app_dir.join("config.json");
        if path.exists() {
            let raw =
                fs::read_to_string(&path).map_err(|error| format!("读取配置文件失败：{error}"))?;
            return match parse_config_draft(&raw) {
                Ok(parsed) => {
                    let comment_language =
                        detect_config_comment_language(&raw).unwrap_or_else(|| {
                            configured_comment_language(&parsed.config.app.language)
                        });
                    let source_raw = canonical_config_jsonc(&parsed.config, comment_language)?;
                    if raw != source_raw {
                        atomic_write(&path, &source_raw)?;
                    }
                    Ok((
                        Self {
                            path,
                            state: RwLock::new(ConfigStoreState {
                                value: parsed.config,
                                revision: 1,
                                source_raw,
                                source_error: None,
                                comment_language,
                            }),
                        },
                        false,
                    ))
                }
                Err(error) => Ok((
                    Self {
                        path,
                        state: RwLock::new(ConfigStoreState {
                            value: AppConfig::default(),
                            revision: 1,
                            source_raw: raw,
                            source_error: Some(error),
                            comment_language: UiLanguage::ZhCn,
                        }),
                    },
                    false,
                )),
            };
        }

        let mut value = AppConfig::default();
        value.app.player_selection = PlayerSelection::from_stored(
            storage.get_preference("player.selection").unwrap_or(None),
        );
        value.lyrics.providers = storage
            .get_preference("lyrics.providers")
            .unwrap_or(None)
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();
        migrate_legacy_provider_order(&mut value.lyrics.providers);
        let bool_preference =
            |key: &str| storage.get_preference(key).unwrap_or(None).as_deref() == Some("true");
        value.overlay.visible = bool_preference("overlay.visible");
        value.overlay.locked =
            bool_preference("overlay.locked") || bool_preference("overlay.passthrough");
        let last_monitor = storage
            .get_preference("overlay.last_monitor")
            .unwrap_or(None);
        let legacy_style = last_monitor
            .as_ref()
            .and_then(|id| {
                storage
                    .get_preference(&format!("overlay.style.{id}"))
                    .ok()
                    .flatten()
            })
            .or_else(|| {
                storage
                    .get_preference("overlay.style.default")
                    .ok()
                    .flatten()
            })
            .and_then(|raw| serde_json::from_str::<OverlayStyleSettings>(&raw).ok())
            .map(OverlayStyleSettings::normalized);
        if let Some(style) = legacy_style {
            value.overlay.appearance = OverlayAppearance::from(&style);
        }
        let value = value.normalized()?;
        let comment_language = configured_comment_language(&value.app.language);
        let source_raw = canonical_config_jsonc(&value, comment_language)?;
        atomic_write(&path, &source_raw)?;
        Ok((
            Self {
                path,
                state: RwLock::new(ConfigStoreState {
                    value,
                    revision: 1,
                    source_raw,
                    source_error: None,
                    comment_language,
                }),
            },
            true,
        ))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn snapshot(&self) -> AppConfig {
        self.state
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .value
            .clone()
    }

    #[cfg(test)]
    fn revision(&self) -> u64 {
        self.state
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .revision
    }

    pub fn editor_data(&self) -> ConfigEditorData {
        let state = self.state.read().unwrap_or_else(|error| error.into_inner());
        let validation = if let Some(error) = &state.source_error {
            ConfigDraftValidation {
                valid: false,
                error: Some(error.clone()),
                normalized_json: None,
                effective_config: AppConfig::default(),
            }
        } else {
            validate_config_draft(&state.source_raw)
        };
        ConfigEditorData {
            default_jsonc: canonical_config_jsonc(&AppConfig::default(), state.comment_language)
                .expect("默认配置必须可以序列化"),
            user_json: state.source_raw.clone(),
            revision: state.revision,
            validation,
        }
    }

    pub fn replace(&self, value: AppConfig) -> Result<AppConfig, String> {
        self.replace_inner(value, None)
    }

    pub fn replace_at_revision(
        &self,
        value: AppConfig,
        expected_revision: u64,
    ) -> Result<AppConfig, String> {
        self.replace_inner(value, Some(expected_revision))
    }

    fn replace_inner(
        &self,
        value: AppConfig,
        expected_revision: Option<u64>,
    ) -> Result<AppConfig, String> {
        let value = value.normalized()?;
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if expected_revision.is_some_and(|expected| expected != state.revision) {
            return Err(
                "config.conflict: configuration changed at another location; reload before saving"
                    .into(),
            );
        }
        let raw = canonical_config_jsonc(&value, state.comment_language)?;
        atomic_write(&self.path, &raw)?;
        state.value = value.clone();
        state.source_raw = raw;
        state.source_error = None;
        state.revision = state.revision.saturating_add(1);
        Ok(value)
    }

    pub fn update(&self, edit: impl FnOnce(&mut AppConfig)) -> Result<AppConfig, String> {
        let mut next = self.snapshot();
        edit(&mut next);
        self.replace(next)
    }

    pub fn set_comment_language(&self, language: UiLanguage) -> Result<bool, String> {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if state.comment_language == language {
            return Ok(false);
        }
        let localized = if state.source_error.is_none() {
            Some(canonical_config_jsonc(&state.value, language)?)
        } else {
            None
        };
        if let Some(raw) = localized.as_ref() {
            atomic_write(&self.path, raw)?;
        }
        state.comment_language = language;
        if let Some(raw) = localized {
            state.source_raw = raw;
        }
        state.revision = state.revision.saturating_add(1);
        Ok(true)
    }

    pub fn export_json(&self) -> Result<String, String> {
        let state = self.state.read().unwrap_or_else(|error| error.into_inner());
        canonical_config_jsonc(&state.value, state.comment_language)
    }
}

fn atomic_write(path: &Path, raw: &str) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| "配置目录无效".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("创建配置目录失败：{error}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, raw).map_err(|error| format!("写入临时配置失败：{error}"))?;
    fs::rename(&temporary, path).map_err(|error| format!("替换配置文件失败：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("lyrics-plus-{name}-{stamp}"))
    }

    #[test]
    fn new_scale_rebases_old_scale() {
        assert_eq!(migrate_v1_font_scale(80), 80);
        assert_eq!(migrate_v1_font_scale(100), 80);
        assert_eq!(migrate_v1_font_scale(120), 100);
        assert_eq!(migrate_v1_font_scale(150), 130);
    }

    #[test]
    fn jsonc_supports_comments_trailing_commas_and_partial_values() {
        let parsed = parse_config_draft(
            r##"{
              // only override two fields
              "app": { "uiFontScale": 120, },
              /* keep everything else default */
              "overlay": { "appearance": { "activeColor": "#ff0000", }, },
            }"##,
        )
        .unwrap();
        assert_eq!(parsed.config.app.ui_font_scale, 120);
        assert_eq!(parsed.config.overlay.appearance.active_color, "#ff0000");
        assert_eq!(parsed.config.app.player_selection, PlayerSelection::Auto);
    }

    #[test]
    fn provided_provider_arrays_are_completed_with_disabled_entries() {
        let parsed = parse_config_draft(
            r#"{
              "lyrics": {
                "providers": {
                  "providers": [{ "id": "lrclib", "enabled": true }]
                }
              }
            }"#,
        )
        .unwrap();
        assert_eq!(parsed.config.lyrics.providers.providers.len(), 4);
        assert_eq!(parsed.config.lyrics.providers.providers[0].id, "lrclib");
        assert!(parsed.config.lyrics.providers.providers[0].enabled);
        assert!(parsed.config.lyrics.providers.providers[1..]
            .iter()
            .all(|provider| !provider.enabled));
        let default_lines = canonical_config_jsonc(&AppConfig::default(), UiLanguage::ZhCn)
            .unwrap()
            .lines()
            .count();
        assert_eq!(parsed.normalized_json.lines().count(), default_lines);
    }

    #[test]
    fn unknown_fields_are_rejected_with_location() {
        let validation = validate_config_draft("{\n  \"app\": { \"fontScael\": 120 }\n}");
        assert!(!validation.valid);
        let error = validation.error.unwrap();
        assert_eq!(error.line, 2);
        assert!(error.message.contains("fontScael"));
    }

    #[test]
    fn invalid_draft_falls_back_to_default() {
        let validation = validate_config_draft("{ nope }");
        assert!(!validation.valid);
        assert_eq!(validation.effective_config.app.ui_font_scale, 100);
    }

    #[test]
    fn default_template_matches_runtime_default() {
        let default_jsonc =
            canonical_config_jsonc(&AppConfig::default(), UiLanguage::ZhCn).unwrap();
        let parsed = parse_config_draft(&default_jsonc).unwrap();
        assert_eq!(parsed.config.app.ui_font_scale, 100);
        assert_eq!(parsed.config.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(parsed.normalized_json, default_jsonc);
        assert!(parsed.normalized_json.contains("// 配置结构版本"));
    }

    #[test]
    fn localized_templates_have_the_same_effective_config() {
        let zh = canonical_config_jsonc(&AppConfig::default(), UiLanguage::ZhCn).unwrap();
        let en = canonical_config_jsonc(&AppConfig::default(), UiLanguage::EnUs).unwrap();
        assert!(zh.contains("// 配置结构版本"));
        assert!(en.contains("// Configuration schema version"));
        assert_eq!(
            serde_json::to_value(parse_config_draft(&zh).unwrap().config).unwrap(),
            serde_json::to_value(parse_config_draft(&en).unwrap().config).unwrap(),
        );
    }

    #[test]
    fn schema_eight_adds_shortcuts_and_independent_overlay_controls() {
        let parsed = parse_config_draft(r#"{"schemaVersion":8}"#).unwrap();
        assert!(parsed.migrated);
        assert_eq!(parsed.config.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(
            parsed.config.app.shortcuts,
            GlobalShortcutSettings::default()
        );
        assert_eq!(parsed.config.overlay.appearance.secondary_font_scale, 0.8);
        assert_eq!(parsed.config.overlay.appearance.background_opacity, 0.6);
        assert_eq!(parsed.config.overlay.appearance.background_blur, 18.0);
    }

    #[test]
    fn schema_nine_adds_background_blur() {
        let parsed = parse_config_draft(r#"{"schemaVersion":9}"#).unwrap();
        assert!(parsed.migrated);
        assert_eq!(parsed.config.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(parsed.config.overlay.appearance.background_blur, 18.0);
    }

    #[test]
    fn schema_ten_transparent_background_becomes_transparent_mode() {
        let parsed = parse_config_draft(
            r#"{"schemaVersion":10,"overlay":{"appearance":{"background":"transparent","backgroundOpacity":0.75}}}"#,
        )
        .unwrap();
        assert!(parsed.migrated);
        assert_eq!(
            parsed.config.overlay.appearance.background,
            OverlayBackground::Solid
        );
        assert_eq!(
            parsed.config.overlay.appearance.background_mode,
            OverlayBackgroundMode::Transparent
        );
        assert_eq!(parsed.config.overlay.appearance.background_opacity, 0.75);
    }

    #[test]
    fn schema_ten_glass_and_solid_backgrounds_become_solid_mode() {
        for background in ["glass", "solid"] {
            let raw = format!(
                r#"{{"schemaVersion":10,"overlay":{{"appearance":{{"background":"{background}","backgroundOpacity":0.35,"backgroundBlur":26}}}}}}"#
            );
            let parsed = parse_config_draft(&raw).unwrap();
            assert!(parsed.migrated);
            assert_eq!(
                parsed.config.overlay.appearance.background_mode,
                OverlayBackgroundMode::Solid
            );
            assert_eq!(parsed.config.overlay.appearance.background_opacity, 0.35);
            assert_eq!(parsed.config.overlay.appearance.background_blur, 26.0);
        }
    }

    #[test]
    fn current_background_opacity_is_preserved() {
        let parsed = parse_config_draft(
            r#"{"schemaVersion":14,"overlay":{"appearance":{"backgroundOpacity":0.85}}}"#,
        )
        .unwrap();
        assert_eq!(parsed.config.overlay.appearance.background_opacity, 0.85);
    }

    #[test]
    fn schema_eleven_adds_disabled_hide_when_not_playing() {
        let parsed = parse_config_draft(r#"{"schemaVersion":11}"#).unwrap();
        assert!(parsed.migrated);
        assert_eq!(parsed.config.schema_version, CONFIG_SCHEMA_VERSION);
        assert!(!parsed.config.overlay.hide_when_not_playing);
    }

    #[test]
    fn schema_twelve_adds_system_language_preference() {
        let parsed = parse_config_draft(r#"{"schemaVersion":12}"#).unwrap();
        assert!(parsed.migrated);
        assert_eq!(parsed.config.app.language, LanguagePreference::default());
        assert!(parsed.normalized_json.contains("\"language\": \"system\""));
    }

    #[test]
    fn schema_thirteen_migrates_only_the_old_provider_default() {
        let old_default = parse_config_draft(
            r#"{
              "schemaVersion": 13,
              "lyrics": { "providers": {
                "mode": "smart",
                "autoApplyThreshold": 60,
                "providers": [
                  { "id": "lrclib", "enabled": true },
                  { "id": "kugou", "enabled": true },
                  { "id": "qqmusic", "enabled": true },
                  { "id": "netease", "enabled": true }
                ]
              } }
            }"#,
        )
        .unwrap();
        assert!(old_default.config.lyrics.providers.providers[0].enabled);
        assert!(old_default.config.lyrics.providers.providers[1..]
            .iter()
            .all(|provider| !provider.enabled));

        let customized = parse_config_draft(
            r#"{
              "schemaVersion": 13,
              "lyrics": { "providers": {
                "mode": "smart",
                "autoApplyThreshold": 61,
                "providers": [
                  { "id": "lrclib", "enabled": true },
                  { "id": "kugou", "enabled": true },
                  { "id": "qqmusic", "enabled": true },
                  { "id": "netease", "enabled": true }
                ]
              } }
            }"#,
        )
        .unwrap();
        assert!(customized
            .config
            .lyrics
            .providers
            .providers
            .iter()
            .all(|provider| provider.enabled));
    }

    #[test]
    fn language_preference_accepts_supported_and_future_bcp_47_values() {
        for (raw, expected) in [
            (r#"{"app":{"language":"system"}}"#, "system"),
            (r#"{"app":{"language":"zh-CN"}}"#, "zh-CN"),
            (r#"{"app":{"language":"zh-TW"}}"#, "zh-TW"),
            (r#"{"app":{"language":"en-US"}}"#, "en-US"),
            (r#"{"app":{"language":"fr-FR"}}"#, "fr-FR"),
        ] {
            let parsed = parse_config_draft(raw).unwrap();
            assert_eq!(parsed.config.app.language.0, expected);
        }
    }

    #[test]
    fn language_preference_rejects_invalid_language_tags() {
        for language in ["", "zh_TW", "-zh", "1", "zh--TW", "zh-繁體"] {
            let raw = format!(r#"{{"app":{{"language":"{language}"}}}}"#);
            let validation = validate_config_draft(&raw);
            assert!(!validation.valid, "{language} should be rejected");
            assert!(validation.error.unwrap().message.contains("language"));
        }
    }

    #[test]
    fn only_simplified_chinese_uses_chinese_native_copy() {
        assert_eq!(
            configured_comment_language(&LanguagePreference::from("zh-CN")),
            UiLanguage::ZhCn
        );
        for language in ["system", "zh-TW", "en-US", "ja-JP"] {
            assert_eq!(
                configured_comment_language(&LanguagePreference::from(language)),
                UiLanguage::EnUs
            );
        }
    }

    #[test]
    fn hide_when_not_playing_requires_boolean_value() {
        let validation = validate_config_draft(r#"{"overlay":{"hideWhenNotPlaying":"yes"}}"#);
        assert!(!validation.valid);
        assert!(validation
            .error
            .unwrap()
            .message
            .contains("hideWhenNotPlaying 必须是布尔值"));
    }

    #[test]
    fn shortcuts_require_modifiers_and_unique_combinations() {
        let missing_modifier =
            validate_config_draft(r#"{"app":{"shortcuts":{"toggleOverlay":"KeyL"}}}"#);
        assert!(!missing_modifier.valid);
        assert!(missing_modifier.error.unwrap().message.contains("修饰键"));

        let duplicate = validate_config_draft(
            r#"{"app":{"shortcuts":{"toggleOverlay":"Control+KeyL","unlockOverlay":"Control+KeyL"}}}"#,
        );
        assert!(!duplicate.valid);
        assert!(duplicate.error.unwrap().message.contains("不能重复"));
    }

    #[test]
    fn schema_five_migrates_compound_overlay_layouts() {
        for (legacy, layout, orientation) in [
            (
                "single",
                OverlayLayout::Single,
                OverlayOrientation::Horizontal,
            ),
            (
                "stacked",
                OverlayLayout::Double,
                OverlayOrientation::Horizontal,
            ),
            (
                "side_by_side",
                OverlayLayout::Double,
                OverlayOrientation::Horizontal,
            ),
            (
                "vertical_single",
                OverlayLayout::Single,
                OverlayOrientation::Vertical,
            ),
            (
                "vertical_double",
                OverlayLayout::Double,
                OverlayOrientation::Vertical,
            ),
        ] {
            let raw = format!(
                r#"{{"schemaVersion":5,"overlay":{{"appearance":{{"layout":"{legacy}"}}}}}}"#
            );
            let parsed = parse_config_draft(&raw).unwrap();
            assert!(parsed.migrated);
            assert_eq!(parsed.config.overlay.appearance.layout, layout);
            assert_eq!(parsed.config.overlay.appearance.orientation, orientation);
            assert!(parsed.normalized_json.contains("\"schemaVersion\": 14"));
        }
    }

    #[test]
    fn current_schema_rejects_compound_overlay_layouts() {
        for layout in [
            "stacked",
            "side_by_side",
            "vertical_single",
            "vertical_double",
        ] {
            let raw = format!(
                r#"{{"schemaVersion":14,"overlay":{{"appearance":{{"layout":"{layout}"}}}}}}"#
            );
            let validation = validate_config_draft(&raw);
            assert!(
                !validation.valid,
                "{layout} should be invalid in the current schema"
            );
            assert!(validation.error.unwrap().message.contains("orientation"));
        }
    }

    #[test]
    fn schema_four_migrates_legacy_provider_order_once() {
        let parsed = parse_config_draft(
            r#"{
              "schemaVersion": 4,
              "lyrics": { "providers": {
                "mode": "smart",
                "providers": [
                  { "id": "netease", "enabled": true },
                  { "id": "qqmusic", "enabled": true },
                  { "id": "kugou", "enabled": true },
                  { "id": "lrclib", "enabled": true }
                ]
              } }
            }"#,
        )
        .unwrap();

        assert!(parsed.migrated);
        assert_eq!(parsed.config.lyrics.providers, ProviderSettings::default());
    }

    #[test]
    fn current_schema_preserves_explicit_legacy_provider_order() {
        let parsed = parse_config_draft(
            r#"{
              "schemaVersion": 14,
              "lyrics": { "providers": {
                "mode": "smart",
                "providers": [
                  { "id": "netease", "enabled": true },
                  { "id": "qqmusic", "enabled": true },
                  { "id": "kugou", "enabled": true },
                  { "id": "lrclib", "enabled": true }
                ]
              } }
            }"#,
        )
        .unwrap();

        assert!(!parsed.migrated);
        assert_eq!(
            parsed
                .config
                .lyrics
                .providers
                .providers
                .iter()
                .map(|provider| provider.id.as_str())
                .collect::<Vec<_>>(),
            vec!["netease", "qqmusic", "kugou", "lrclib"]
        );
    }

    #[test]
    fn newer_schema_is_rejected() {
        let validation = validate_config_draft(r#"{"schemaVersion":99}"#);
        assert!(!validation.valid);
        assert!(validation.error.unwrap().message.contains("高于当前支持"));
    }

    #[test]
    fn oversized_schema_version_is_rejected_without_wrapping() {
        let validation = validate_config_draft(r#"{"schemaVersion":65538}"#);
        assert!(!validation.valid);
        assert!(validation.error.unwrap().message.contains("超出支持范围"));
    }

    #[test]
    fn invalid_types_enums_colors_and_ranges_report_the_field() {
        for (raw, field) in [
            (r#"{"app":{"autostart":"yes"}}"#, "autostart"),
            (r#"{"app":{"hideDockIcon":"yes"}}"#, "hideDockIcon"),
            (r#"{"app":{"playerSelection":"music"}}"#, "playerSelection"),
            (r#"{"app":{"uiFontScale":151}}"#, "uiFontScale"),
            (
                r#"{"lyrics":{"providers":{"autoApplyThreshold":101}}}"#,
                "autoApplyThreshold",
            ),
            (
                r#"{"lyrics":{"providers":{"autoApplyThreshold":60.5}}}"#,
                "autoApplyThreshold",
            ),
            (
                r#"{"overlay":{"appearance":{"layout":"triple"}}}"#,
                "layout",
            ),
            (
                r#"{"overlay":{"appearance":{"orientation":"diagonal"}}}"#,
                "orientation",
            ),
            (
                r#"{"overlay":{"appearance":{"activeColor":"not a color"}}}"#,
                "activeColor",
            ),
        ] {
            let validation = validate_config_draft(raw);
            assert!(!validation.valid, "{raw} should be invalid");
            let error = validation.error.unwrap();
            assert!(error.message.contains(field), "{}", error.message);
            assert_eq!(error.line, 1);
            assert!(error.column > 1);
        }
    }

    #[test]
    fn duplicate_and_unknown_providers_are_rejected() {
        for raw in [
            r#"{"lyrics":{"providers":{"providers":[{"id":"lrclib","enabled":true},{"id":"lrclib","enabled":true}]}}}"#,
            r#"{"lyrics":{"providers":{"providers":[{"id":"unknown","enabled":true}]}}}"#,
        ] {
            let validation = validate_config_draft(raw);
            assert!(!validation.valid);
            assert!(validation.error.unwrap().message.contains("歌词源"));
        }
    }

    #[test]
    fn appearance_does_not_serialize_geometry() {
        let raw = serde_json::to_string(&AppConfig::default()).unwrap();
        assert!(!raw.contains("horizontalMaxWidth"));
        assert!(!raw.contains("verticalMaxHeight"));
    }

    #[test]
    fn schema_six_defaults_auto_center_to_disabled() {
        let parsed = parse_config_draft(r#"{"schemaVersion":6}"#).unwrap();
        assert!(parsed.migrated);
        assert!(
            !parsed
                .config
                .overlay
                .appearance
                .auto_center_with_translation_or_romanization
        );
        assert!(parsed
            .normalized_json
            .contains("\"autoCenterWithTranslationOrRomanization\": false"));
    }

    #[test]
    fn legacy_config_defaults_auto_apply_threshold_to_sixty() {
        let parsed = parse_config_draft(r#"{"schemaVersion":7}"#).unwrap();
        assert!(parsed.migrated);
        assert_eq!(parsed.config.lyrics.providers.auto_apply_threshold, 60);
        assert!(parsed
            .normalized_json
            .contains("\"autoApplyThreshold\": 60"));
    }

    #[test]
    fn auto_apply_threshold_round_trips_at_boundaries() {
        for threshold in [0, 100] {
            let raw =
                format!(r#"{{"lyrics":{{"providers":{{"autoApplyThreshold":{threshold}}}}}}}"#);
            let parsed = parse_config_draft(&raw).unwrap();
            assert_eq!(
                parsed.config.lyrics.providers.auto_apply_threshold,
                threshold
            );
        }
    }

    #[test]
    fn auto_center_preference_round_trips() {
        let parsed = parse_config_draft(
            r#"{"overlay":{"appearance":{"autoCenterWithTranslationOrRomanization":true}}}"#,
        )
        .unwrap();
        assert!(
            parsed
                .config
                .overlay
                .appearance
                .auto_center_with_translation_or_romanization
        );
        assert!(parsed
            .normalized_json
            .contains("\"autoCenterWithTranslationOrRomanization\": true"));
    }

    #[test]
    fn version_two_autostart_is_accepted_and_removed() {
        let parsed =
            parse_config_draft(r#"{"schemaVersion":2,"app":{"uiFontScale":120,"autostart":true}}"#)
                .unwrap();
        assert!(parsed.migrated);
        assert_eq!(parsed.config.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(parsed.config.app.ui_font_scale, 120);
        assert!(!parsed.config.app.hide_dock_icon);
        assert!(!parsed.normalized_json.contains("autostart"));
    }

    #[test]
    fn dock_icon_preference_round_trips() {
        let parsed = parse_config_draft(r#"{"app":{"hideDockIcon":true}}"#).unwrap();
        assert!(parsed.config.app.hide_dock_icon);
        assert!(parsed.normalized_json.contains("\"hideDockIcon\": true"));
    }

    #[test]
    fn failed_atomic_replace_keeps_in_memory_value() {
        let root = test_root("config-rollback");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("config.json")).unwrap();
        let store = ConfigStore {
            path: root.join("config.json"),
            state: RwLock::new(ConfigStoreState {
                value: AppConfig::default(),
                revision: 1,
                source_raw: "{}".into(),
                source_error: None,
                comment_language: UiLanguage::ZhCn,
            }),
        };
        let result = store.update(|config| config.app.ui_font_scale = 140);
        assert!(result.is_err());
        assert_eq!(store.snapshot().app.ui_font_scale, 100);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_disk_config_uses_default_without_overwrite() {
        let root = test_root("invalid-disk-config");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("config.json"), "{ broken }").unwrap();
        let storage = Storage::open(root.clone(), root.join("library")).unwrap();
        let (store, migrated) = ConfigStore::load(&root, &storage).unwrap();
        assert!(!migrated);
        assert_eq!(store.snapshot().app.ui_font_scale, 100);
        assert_eq!(
            fs::read_to_string(root.join("config.json")).unwrap(),
            "{ broken }"
        );
        assert!(!store.editor_data().validation.valid);
        let revision = store.revision();
        assert!(store.set_comment_language(UiLanguage::EnUs).unwrap());
        assert_eq!(store.revision(), revision + 1);
        assert_eq!(
            fs::read_to_string(root.join("config.json")).unwrap(),
            "{ broken }"
        );
        assert!(store
            .editor_data()
            .default_jsonc
            .contains("// Configuration schema version"));
        drop(storage);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn valid_disk_config_is_rewritten_as_complete_commented_jsonc() {
        let root = test_root("canonical-disk-config");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("config.json"), r#"{"app":{"uiFontScale":120}}"#).unwrap();
        let storage = Storage::open(root.clone(), root.join("library")).unwrap();
        let (store, migrated) = ConfigStore::load(&root, &storage).unwrap();
        assert!(!migrated);
        assert_eq!(store.snapshot().app.ui_font_scale, 120);
        let persisted = fs::read_to_string(root.join("config.json")).unwrap();
        assert!(persisted.contains("// Interface text scale"));
        assert!(persisted.contains("\"uiFontScale\": 120"));
        assert_eq!(persisted, store.editor_data().user_json);
        drop(storage);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn updates_keep_canonical_comments() {
        let root = test_root("commented-update");
        fs::create_dir_all(&root).unwrap();
        let storage = Storage::open(root.clone(), root.join("library")).unwrap();
        let (store, _) = ConfigStore::load(&root, &storage).unwrap();
        let revision = store.revision();
        let before_language_change = serde_json::to_value(store.snapshot()).unwrap();
        assert!(store.set_comment_language(UiLanguage::ZhCn).unwrap());
        assert_eq!(store.revision(), revision + 1);
        assert_eq!(
            serde_json::to_value(store.snapshot()).unwrap(),
            before_language_change
        );
        assert!(store.set_comment_language(UiLanguage::EnUs).unwrap());
        assert_eq!(store.revision(), revision + 2);
        assert!(!store.set_comment_language(UiLanguage::EnUs).unwrap());
        assert_eq!(store.revision(), revision + 2);
        store
            .update(|config| config.app.ui_font_scale = 130)
            .unwrap();
        let persisted = fs::read_to_string(root.join("config.json")).unwrap();
        assert!(persisted.contains("// Interface text scale"));
        assert!(persisted.contains("\"uiFontScale\": 130"));
        assert!(store
            .export_json()
            .unwrap()
            .contains("// Interface text scale"));
        drop(storage);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn revision_conflict_does_not_overwrite_newer_config() {
        let root = test_root("revision-conflict");
        fs::create_dir_all(&root).unwrap();
        let storage = Storage::open(root.clone(), root.join("library")).unwrap();
        let (store, _) = ConfigStore::load(&root, &storage).unwrap();
        let revision = store.revision();
        store
            .update(|config| config.app.ui_font_scale = 120)
            .unwrap();
        let mut stale = store.snapshot();
        stale.app.ui_font_scale = 140;
        let error = store.replace_at_revision(stale, revision).unwrap_err();
        assert!(error.starts_with("config.conflict:"));
        assert_eq!(store.snapshot().app.ui_font_scale, 120);
        drop(storage);
        let _ = fs::remove_dir_all(root);
    }
}
