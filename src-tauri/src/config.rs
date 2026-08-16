use std::collections::HashSet;
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
use crate::lyrics::provider::{normalize_settings, ProviderOrderMode, ProviderSettings};
use crate::player::PlayerSelection;
use crate::storage::Storage;

pub const CONFIG_SCHEMA_VERSION: u16 = 41;
const APP_CONFIG_KEYS: &[&str] = &[
    "theme",
    "language",
    "playerSelection",
    "systemMediaFilterMode",
    "systemMediaApplications",
    "playerFollowerApplication",
    "hideDockIcon",
    "silentStartup",
    "autoCheckUpdates",
    "shortcuts",
];

fn canonical_config_jsonc(value: &AppConfig, language: UiLanguage) -> Result<String, String> {
    let json =
        serde_json::to_string_pretty(value).map_err(|error| format!("序列化配置失败：{error}"))?;
    let mut output = String::with_capacity(json.len() + 1_200);
    for line in json.lines() {
        let comment = match line {
            line if line.starts_with("  \"schemaVersion\":") => {
                Some(("  ", ConfigComment::SchemaVersion))
            }
            line if line.starts_with("    \"theme\":") => Some(("    ", ConfigComment::Theme)),
            line if line.starts_with("    \"language\":") => {
                Some(("    ", ConfigComment::Language))
            }
            line if line.starts_with("    \"playerSelection\":") => {
                Some(("    ", ConfigComment::PlayerSelection))
            }
            line if line.starts_with("    \"systemMediaFilterMode\":") => {
                Some(("    ", ConfigComment::SystemMediaFilterMode))
            }
            line if line.starts_with("    \"systemMediaApplications\":") => {
                Some(("    ", ConfigComment::SystemMediaApplications))
            }
            line if line.starts_with("    \"playerFollowerApplication\":") => {
                Some(("    ", ConfigComment::PlayerFollowerApplication))
            }
            line if line.starts_with("    \"hideDockIcon\":") => {
                Some(("    ", ConfigComment::HideDockIcon))
            }
            line if line.starts_with("    \"silentStartup\":") => {
                Some(("    ", ConfigComment::SilentStartup))
            }
            line if line.starts_with("    \"autoCheckUpdates\":") => {
                Some(("    ", ConfigComment::AutoCheckUpdates))
            }
            line if line.starts_with("    \"shortcuts\":") => {
                Some(("    ", ConfigComment::Shortcuts))
            }
            line if line.starts_with("      \"autoApplyThreshold\":") => {
                Some(("      ", ConfigComment::AutoApplyThreshold))
            }
            line if line.starts_with("      \"titleFilterKeywords\":") => {
                Some(("      ", ConfigComment::TitleFilterKeywords))
            }
            line if line.starts_with("      \"mode\":") => {
                Some(("      ", ConfigComment::ProviderMode))
            }
            line if line.starts_with("      \"providers\":") => {
                Some(("      ", ConfigComment::Providers))
            }
            line if line.starts_with("    \"displays\":") => {
                Some(("    ", ConfigComment::LyricsDisplays))
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
            line if line.starts_with("      \"fontFamily\":") => {
                Some(("      ", ConfigComment::FontFamily))
            }
            line if line.starts_with("      \"lineHeight\":") => {
                Some(("      ", ConfigComment::LineHeight))
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
            line if line.starts_with("      \"backgroundRadius\":") => {
                Some(("      ", ConfigComment::BackgroundGeometry))
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
            line if line.starts_with("      \"textShadowOffsetX\":") => {
                Some(("      ", ConfigComment::TextShadow))
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
    pub theme: ThemePreference,
    pub language: LanguagePreference,
    pub player_selection: PlayerSelection,
    pub system_media_filter_mode: SystemMediaFilterMode,
    pub system_media_applications: Vec<RegisteredApplication>,
    pub player_follower_application: Option<RegisteredApplication>,
    pub hide_dock_icon: bool,
    pub silent_startup: bool,
    pub auto_check_updates: bool,
    pub shortcuts: GlobalShortcutSettings,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            theme: ThemePreference::Dark,
            language: LanguagePreference::default(),
            player_selection: PlayerSelection::Auto,
            system_media_filter_mode: SystemMediaFilterMode::Allowlist,
            system_media_applications: Vec::new(),
            player_follower_application: None,
            hide_dock_icon: false,
            silent_startup: false,
            auto_check_updates: true,
            shortcuts: GlobalShortcutSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    System,
    Light,
    #[default]
    Dark,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SystemMediaFilterMode {
    #[default]
    Allowlist,
    Blocklist,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredApplication {
    pub name: String,
    pub bundle_id: String,
}

pub fn is_dedicated_player_bundle_id(bundle_id: &str) -> bool {
    matches!(bundle_id, "com.apple.Music" | "com.spotify.client")
}

pub fn normalize_system_media_applications(
    applications: Vec<RegisteredApplication>,
) -> Result<Vec<RegisteredApplication>, String> {
    let mut bundle_ids = HashSet::new();
    let mut normalized = Vec::new();
    for application in applications {
        let application = normalize_registered_application(application)?;
        if is_dedicated_player_bundle_id(&application.bundle_id) {
            return Err("Apple Music 和 Spotify 使用专用通道，不能添加到系统播放应用".into());
        }
        if bundle_ids.insert(application.bundle_id.clone()) {
            normalized.push(application);
        }
    }
    Ok(normalized)
}

pub fn normalize_player_follower_application(
    application: Option<RegisteredApplication>,
) -> Result<Option<RegisteredApplication>, String> {
    application
        .map(normalize_registered_application)
        .transpose()
}

pub(crate) fn normalize_registered_application(
    application: RegisteredApplication,
) -> Result<RegisteredApplication, String> {
    let bundle_id = application.bundle_id.trim();
    if bundle_id.is_empty() {
        return Err("应用的 Bundle ID 不能为空".into());
    }
    if bundle_id.len() > 255
        || bundle_id.starts_with('.')
        || bundle_id.ends_with('.')
        || bundle_id
            .chars()
            .any(|value| !(value.is_ascii_alphanumeric() || matches!(value, '.' | '-')))
    {
        return Err(format!("无效的 Bundle ID：{bundle_id}"));
    }
    let name = application.name.trim();
    Ok(RegisteredApplication {
        name: if name.is_empty() { bundle_id } else { name }.to_owned(),
        bundle_id: bundle_id.to_owned(),
    })
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LyricsPreferences {
    pub providers: ProviderSettings,
    pub displays: LyricsDisplayPreferences,
    pub base_appearance: LyricsBaseAppearance,
    pub style_inheritance: LyricsStyleInheritance,
}

impl Default for LyricsPreferences {
    fn default() -> Self {
        Self {
            providers: ProviderSettings::default(),
            displays: LyricsDisplayPreferences::default(),
            base_appearance: LyricsBaseAppearance::default(),
            style_inheritance: LyricsStyleInheritance::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LyricsBaseAppearance {
    pub font_family: String,
    pub active_color: String,
    pub inactive_color: String,
    pub translation_color: String,
    pub romanization_color: String,
    pub supporting_color: String,
    pub background_color: String,
}

impl Default for LyricsBaseAppearance {
    fn default() -> Self {
        let overlay = OverlayStyleSettings::default();
        Self {
            font_family: overlay.font_family,
            active_color: "#a3e635".into(),
            inactive_color: "#ecfccb".into(),
            translation_color: "#d9f99d".into(),
            romanization_color: "#bef264".into(),
            supporting_color: "#94a3b8".into(),
            background_color: "#171821".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LyricsModeStyleInheritance {
    pub inherit_font_family: bool,
    pub inherit_colors: bool,
}

impl Default for LyricsModeStyleInheritance {
    fn default() -> Self {
        Self {
            inherit_font_family: true,
            inherit_colors: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LyricsStyleInheritance {
    pub desktop: LyricsModeStyleInheritance,
    pub status_bar: LyricsModeStyleInheritance,
    pub list_window: LyricsModeStyleInheritance,
    pub notch: LyricsModeStyleInheritance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LyricsDisplayPreferences {
    pub status_bar: StatusBarLyricsPreferences,
    pub list_window: ListLyricsPreferences,
    pub notch: NotchLyricsPreferences,
}

impl Default for LyricsDisplayPreferences {
    fn default() -> Self {
        Self {
            status_bar: StatusBarLyricsPreferences::default(),
            list_window: ListLyricsPreferences::default(),
            notch: NotchLyricsPreferences::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StatusBarLyricsPreferences {
    pub enabled: bool,
    pub appearance: StatusBarLyricsAppearance,
}

impl Default for StatusBarLyricsPreferences {
    fn default() -> Self {
        Self {
            enabled: false,
            appearance: StatusBarLyricsAppearance::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StatusBarLyricsAppearance {
    pub font_family: String,
    pub font_size: u16,
    pub font_weight: u16,
    pub text_color: String,
    pub inactive_color: String,
    pub highlight_color: String,
    #[serde(alias = "maxWidth")]
    pub width: u16,
}

impl Default for StatusBarLyricsAppearance {
    fn default() -> Self {
        Self {
            font_family: OverlayAppearance::default().font_family,
            font_size: 14,
            font_weight: 600,
            text_color: "#a3e635".into(),
            inactive_color: "#ecfccb".into(),
            highlight_color: "#a3e635".into(),
            width: 220,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ListLyricsPreferences {
    pub enabled: bool,
    pub show_translation: bool,
    pub show_romanization: bool,
    pub appearance: ListLyricsAppearance,
}

impl Default for ListLyricsPreferences {
    fn default() -> Self {
        Self {
            enabled: false,
            show_translation: true,
            show_romanization: false,
            appearance: ListLyricsAppearance::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ListLyricsAppearance {
    pub font_family: String,
    pub font_size: u16,
    pub font_weight: u16,
    pub secondary_font_scale: f64,
    pub line_height: f64,
    pub line_gap: f64,
    pub active_color: String,
    pub inactive_color: String,
    pub translation_color: String,
    pub romanization_color: String,
    pub active_background_color: String,
    pub background_color: String,
    pub alignment: String,
}

impl Default for ListLyricsAppearance {
    fn default() -> Self {
        Self {
            font_family: OverlayAppearance::default().font_family,
            font_size: 24,
            font_weight: 600,
            secondary_font_scale: 0.58,
            line_height: 1.45,
            line_gap: 8.0,
            active_color: "#a3e635".into(),
            inactive_color: "#ecfccb".into(),
            translation_color: "#d9f99d".into(),
            romanization_color: "#bef264".into(),
            active_background_color: "rgba(148, 163, 184, 0.14)".into(),
            background_color: "#171821".into(),
            alignment: "left".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NotchLyricsPreferences {
    pub enabled: bool,
    pub monitor_id: Option<String>,
    pub show_two_lines: bool,
    pub show_translation: bool,
    pub show_romanization: bool,
    pub appearance: NotchLyricsAppearance,
}

impl Default for NotchLyricsPreferences {
    fn default() -> Self {
        Self {
            enabled: false,
            monitor_id: None,
            show_two_lines: false,
            show_translation: false,
            show_romanization: false,
            appearance: NotchLyricsAppearance::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NotchLyricsAppearance {
    pub font_family: String,
    pub font_size: u16,
    pub font_weight: u16,
    pub active_color: String,
    pub inactive_color: String,
    pub translation_color: String,
    pub romanization_color: String,
    pub border_radius: f64,
    pub max_width: u16,
}

impl Default for NotchLyricsAppearance {
    fn default() -> Self {
        Self {
            font_family: OverlayAppearance::default().font_family,
            font_size: 18,
            font_weight: 700,
            active_color: "#a3e635".into(),
            inactive_color: "#ecfccb".into(),
            translation_color: "#d9f99d".into(),
            romanization_color: "#bef264".into(),
            border_radius: 22.0,
            max_width: 520,
        }
    }
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
    pub font_family: String,
    pub font_size: u16,
    pub font_weight: u16,
    pub secondary_font_weight: u16,
    pub line_height: f64,
    pub active_color: String,
    pub inactive_color: String,
    pub opacity: f64,
    pub background_opacity: f64,
    pub background_blur: f64,
    pub background_radius: f64,
    pub background_padding_x: f64,
    pub background_padding_y: f64,
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
    pub text_shadow_offset_x: f64,
    pub text_shadow_offset_y: f64,
    pub text_shadow_blur: f64,
    pub text_shadow_color: String,
}

impl Default for OverlayAppearance {
    fn default() -> Self {
        Self::from(&OverlayStyleSettings::default())
    }
}

impl From<&OverlayStyleSettings> for OverlayAppearance {
    fn from(style: &OverlayStyleSettings) -> Self {
        Self {
            font_family: style.font_family.clone(),
            font_size: style.font_size,
            font_weight: style.font_weight,
            secondary_font_weight: style.secondary_font_weight,
            line_height: style.line_height,
            active_color: style.active_color.clone(),
            inactive_color: style.inactive_color.clone(),
            opacity: style.opacity,
            background_opacity: style.background_opacity,
            background_blur: style.background_blur,
            background_radius: style.background_radius,
            background_padding_x: style.background_padding_x,
            background_padding_y: style.background_padding_y,
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
            text_shadow_offset_x: style.text_shadow_offset_x,
            text_shadow_offset_y: style.text_shadow_offset_y,
            text_shadow_blur: style.text_shadow_blur,
            text_shadow_color: style.text_shadow_color.clone(),
        }
    }
}

impl OverlayAppearance {
    pub fn into_style(self) -> OverlayStyleSettings {
        OverlayStyleSettings {
            font_family: self.font_family,
            font_size: self.font_size,
            font_weight: self.font_weight,
            secondary_font_weight: self.secondary_font_weight,
            line_height: self.line_height,
            active_color: self.active_color,
            inactive_color: self.inactive_color,
            opacity: self.opacity,
            background_opacity: self.background_opacity,
            background_blur: self.background_blur,
            background_radius: self.background_radius,
            background_padding_x: self.background_padding_x,
            background_padding_y: self.background_padding_y,
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
            text_shadow_offset_x: self.text_shadow_offset_x,
            text_shadow_offset_y: self.text_shadow_offset_y,
            text_shadow_blur: self.text_shadow_blur,
            text_shadow_color: self.text_shadow_color,
            horizontal_max_width: None,
            vertical_max_height: None,
        }
        .normalized()
    }
}

impl AppConfig {
    pub fn apply_lyrics_style_inheritance(&mut self) {
        let base = self.lyrics.base_appearance.clone();
        let inheritance = self.lyrics.style_inheritance.clone();

        if inheritance.desktop.inherit_font_family {
            self.overlay.appearance.font_family = base.font_family.clone();
        }
        if inheritance.desktop.inherit_colors {
            self.overlay.appearance.active_color = base.active_color.clone();
            self.overlay.appearance.inactive_color = base.inactive_color.clone();
            self.overlay.appearance.translation_color = base.translation_color.clone();
            self.overlay.appearance.romanization_color = base.romanization_color.clone();
            self.overlay.appearance.solid_color = base.background_color.clone();
        }

        let status = &mut self.lyrics.displays.status_bar.appearance;
        if inheritance.status_bar.inherit_font_family {
            status.font_family = base.font_family.clone();
        }
        if inheritance.status_bar.inherit_colors {
            status.text_color = base.active_color.clone();
            status.inactive_color = base.inactive_color.clone();
            status.highlight_color = base.active_color.clone();
        }

        let list = &mut self.lyrics.displays.list_window.appearance;
        if inheritance.list_window.inherit_font_family {
            list.font_family = base.font_family.clone();
        }
        if inheritance.list_window.inherit_colors {
            list.active_color = base.active_color.clone();
            list.inactive_color = base.inactive_color.clone();
            list.translation_color = base.translation_color.clone();
            list.romanization_color = base.romanization_color.clone();
            list.background_color = base.background_color.clone();
        }

        let notch = &mut self.lyrics.displays.notch.appearance;
        if inheritance.notch.inherit_font_family {
            notch.font_family = base.font_family;
        }
        if inheritance.notch.inherit_colors {
            notch.active_color = base.active_color;
            notch.inactive_color = base.inactive_color;
            notch.translation_color = base.translation_color;
            notch.romanization_color = base.romanization_color;
        }
    }

    pub fn normalized(mut self) -> Result<Self, String> {
        if self.schema_version > CONFIG_SCHEMA_VERSION {
            return Err(format!(
                "配置文件版本 {} 高于当前支持的版本 {}",
                self.schema_version, CONFIG_SCHEMA_VERSION
            ));
        }
        self.schema_version = CONFIG_SCHEMA_VERSION;
        self.lyrics.base_appearance.font_family =
            self.lyrics.base_appearance.font_family.trim().to_owned();
        if self.lyrics.base_appearance.font_family.is_empty() {
            self.lyrics.base_appearance.font_family = LyricsBaseAppearance::default().font_family;
        }
        for (name, color) in [
            (
                "基础主歌词颜色",
                self.lyrics.base_appearance.active_color.as_str(),
            ),
            (
                "基础普通歌词颜色",
                self.lyrics.base_appearance.inactive_color.as_str(),
            ),
            (
                "基础翻译颜色",
                self.lyrics.base_appearance.translation_color.as_str(),
            ),
            (
                "基础音译颜色",
                self.lyrics.base_appearance.romanization_color.as_str(),
            ),
            (
                "基础辅助内容颜色",
                self.lyrics.base_appearance.supporting_color.as_str(),
            ),
            (
                "基础背景颜色",
                self.lyrics.base_appearance.background_color.as_str(),
            ),
        ] {
            if !is_supported_color(color) {
                return Err(format!("{name}不是有效的颜色值"));
            }
        }
        self.apply_lyrics_style_inheritance();
        self.app.system_media_applications =
            normalize_system_media_applications(self.app.system_media_applications)?;
        self.app.player_follower_application =
            normalize_player_follower_application(self.app.player_follower_application)?;
        self.app.shortcuts.parsed()?;
        self.lyrics.displays.notch.monitor_id = self
            .lyrics
            .displays
            .notch
            .monitor_id
            .take()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let status_appearance = &mut self.lyrics.displays.status_bar.appearance;
        status_appearance.font_size = status_appearance.font_size.clamp(10, 18);
        status_appearance.font_weight =
            normalize_display_font_weight(status_appearance.font_weight);
        status_appearance.width = status_appearance.width.clamp(120, 360);
        let list_appearance = &mut self.lyrics.displays.list_window.appearance;
        list_appearance.font_size = list_appearance.font_size.clamp(12, 56);
        list_appearance.font_weight = normalize_display_font_weight(list_appearance.font_weight);
        list_appearance.secondary_font_scale =
            list_appearance.secondary_font_scale.clamp(0.35, 1.0);
        list_appearance.line_height = list_appearance.line_height.clamp(0.8, 2.0);
        list_appearance.line_gap = list_appearance.line_gap.clamp(0.0, 32.0);
        if !matches!(
            list_appearance.alignment.as_str(),
            "left" | "center" | "right"
        ) {
            list_appearance.alignment = "left".into();
        }
        let notch_appearance = &mut self.lyrics.displays.notch.appearance;
        notch_appearance.font_size = notch_appearance.font_size.clamp(12, 32);
        notch_appearance.font_weight = normalize_display_font_weight(notch_appearance.font_weight);
        notch_appearance.border_radius = notch_appearance.border_radius.clamp(0.0, 40.0);
        notch_appearance.max_width = notch_appearance.max_width.clamp(400, 640);
        for (name, color) in [
            ("状态栏文字颜色", status_appearance.text_color.as_str()),
            ("状态栏未唱颜色", status_appearance.inactive_color.as_str()),
            ("状态栏高亮颜色", status_appearance.highlight_color.as_str()),
            ("列表当前歌词颜色", list_appearance.active_color.as_str()),
            ("列表普通歌词颜色", list_appearance.inactive_color.as_str()),
            ("列表翻译颜色", list_appearance.translation_color.as_str()),
            ("列表音译颜色", list_appearance.romanization_color.as_str()),
            (
                "列表当前行背景",
                list_appearance.active_background_color.as_str(),
            ),
            ("列表窗口背景", list_appearance.background_color.as_str()),
            ("灵动岛歌词颜色", notch_appearance.active_color.as_str()),
            ("灵动岛未激活颜色", notch_appearance.inactive_color.as_str()),
            ("灵动岛翻译颜色", notch_appearance.translation_color.as_str()),
            ("灵动岛音译颜色", notch_appearance.romanization_color.as_str()),
        ] {
            if !is_supported_color(color) {
                return Err(format!("{name}不是有效的颜色值"));
            }
        }
        let normalized_style = self.overlay.appearance.clone().into_style();
        for (name, color) in color_fields(&normalized_style) {
            if !is_supported_color(color) {
                return Err(format!("{name}不是有效的颜色值"));
            }
        }
        normalize_settings(&mut self.lyrics.providers)?;
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
                &["toggleOverlay", "unlockOverlay", "resetOverlay"],
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
                    "titleFilterKeywords",
                ],
            )?;
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
                            "fontWeight",
                            "textColor",
                            "inactiveColor",
                            "highlightColor",
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
                            "activeColor",
                            "inactiveColor",
                            "translationColor",
                            "romanizationColor",
                            "activeBackgroundColor",
                            "backgroundColor",
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
                        "monitorId",
                        "showTwoLines",
                        "showTranslation",
                        "showRomanization",
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
                            "activeColor",
                            "inactiveColor",
                            "translationColor",
                            "romanizationColor",
                            "borderRadius",
                            "maxWidth",
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
                    "textShadowOffsetX",
                    "textShadowOffsetY",
                    "textShadowBlur",
                    "textShadowColor",
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
        ("/app/silentStartup", "silentStartup"),
        ("/app/autoCheckUpdates", "autoCheckUpdates"),
        ("/overlay/visible", "visible"),
        ("/overlay/locked", "locked"),
        ("/overlay/hideWhenNotPlaying", "hideWhenNotPlaying"),
        ("/lyrics/displays/statusBar/enabled", "enabled"),
        (
            "/lyrics/displays/statusBar/showTrayIcon",
            "showTrayIcon",
        ),
        ("/lyrics/displays/statusBar/locked", "locked"),
        ("/lyrics/displays/listWindow/enabled", "enabled"),
        (
            "/lyrics/displays/listWindow/showTranslation",
            "showTranslation",
        ),
        (
            "/lyrics/displays/listWindow/showRomanization",
            "showRomanization",
        ),
        ("/lyrics/displays/notch/enabled", "enabled"),
        ("/lyrics/displays/notch/showTwoLines", "showTwoLines"),
        (
            "/lyrics/displays/notch/showTranslation",
            "showTranslation",
        ),
        (
            "/lyrics/displays/notch/showRomanization",
            "showRomanization",
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
        ("/lyrics/displays/statusBar/appearance/fontSize", "fontSize"),
        (
            "/lyrics/displays/statusBar/appearance/fontWeight",
            "fontWeight",
        ),
        ("/lyrics/displays/statusBar/appearance/width", "width"),
        (
            "/lyrics/displays/statusBar/appearance/maxWidth",
            "maxWidth",
        ),
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
        ("/overlay/appearance/fontSize", "fontSize"),
        ("/overlay/appearance/fontWeight", "fontWeight"),
        (
            "/overlay/appearance/secondaryFontWeight",
            "secondaryFontWeight",
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
    if let Some(candidate) = value.pointer("/overlay/appearance/fontFamily") {
        let font_family = candidate
            .as_str()
            .ok_or_else(|| error_at_key(raw, "fontFamily", "fontFamily 必须是字符串"))?;
        if font_family.trim().is_empty() {
            return Err(error_at_key(raw, "fontFamily", "fontFamily 不能为空"));
        }
    }
    for key in [
        "activeColor",
        "inactiveColor",
        "solidColor",
        "translationColor",
        "romanizationColor",
        "textShadowColor",
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
            "fontSize",
            value.pointer("/lyrics/displays/statusBar/appearance/fontSize"),
            10.0,
            32.0,
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
            "fontSize",
            value.pointer("/lyrics/displays/notch/appearance/fontSize"),
            12.0,
            32.0,
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

fn color_fields(style: &OverlayStyleSettings) -> [(&'static str, &str); 6] {
    [
        ("高亮颜色", &style.active_color),
        ("未唱颜色", &style.inactive_color),
        ("背景颜色", &style.solid_color),
        ("翻译颜色", &style.translation_color),
        ("音译颜色", &style.romanization_color),
        ("文字阴影颜色", &style.text_shadow_color),
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
    fn jsonc_supports_comments_trailing_commas_and_partial_values() {
        let parsed = parse_config_draft(
            r##"{
              // only override two fields
              "app": { "theme": "light", },
              /* keep everything else default */
              "overlay": { "appearance": { "activeColor": "#ff0000", }, },
            }"##,
        )
        .unwrap();
        assert_eq!(parsed.config.app.theme, ThemePreference::Light);
        assert_eq!(parsed.config.overlay.appearance.active_color, "#ff0000");
        assert_eq!(parsed.config.app.player_selection, PlayerSelection::Auto);
    }

    #[test]
    fn accepts_system_player_selection() {
        let parsed = parse_config_draft(r#"{"app":{"playerSelection":"system"}}"#).unwrap();
        assert_eq!(parsed.config.app.player_selection, PlayerSelection::System);
        let serialized = serde_json::to_value(parsed.config).unwrap();
        assert_eq!(
            serialized
                .pointer("/app/playerSelection")
                .and_then(Value::as_str),
            Some("system")
        );
    }

    #[test]
    fn system_media_filter_mode_defaults_validates_and_round_trips() {
        let default = parse_config_draft(r#"{"app":{}}"#).unwrap();
        assert_eq!(
            default.config.app.system_media_filter_mode,
            SystemMediaFilterMode::Allowlist
        );

        let blocklist = parse_config_draft(
            r#"{"app":{"systemMediaFilterMode":"blocklist","systemMediaApplications":[{"name":"Browser","bundleId":"org.example.Browser"}]}}"#,
        )
        .unwrap();
        assert_eq!(
            blocklist.config.app.system_media_filter_mode,
            SystemMediaFilterMode::Blocklist
        );
        assert_eq!(
            serde_json::to_value(blocklist.config)
                .unwrap()
                .pointer("/app/systemMediaFilterMode")
                .and_then(Value::as_str),
            Some("blocklist")
        );
        assert!(parse_config_draft(r#"{"app":{"systemMediaFilterMode":"unsupported"}}"#).is_err());
    }

    #[test]
    fn schema_twenty_three_preserves_system_media_filter_behavior() {
        let empty =
            parse_config_draft(r#"{"schemaVersion":23,"app":{"systemMediaApplications":[]}}"#)
                .unwrap();
        assert_eq!(
            empty.config.app.system_media_filter_mode,
            SystemMediaFilterMode::Blocklist
        );

        let listed = parse_config_draft(
            r#"{"schemaVersion":23,"app":{"systemMediaApplications":[{"name":"Player","bundleId":"org.example.Player"}]}}"#,
        )
        .unwrap();
        assert_eq!(
            listed.config.app.system_media_filter_mode,
            SystemMediaFilterMode::Allowlist
        );
        assert_eq!(listed.config.app.system_media_applications.len(), 1);
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
        assert_eq!(validation.effective_config.app.theme, ThemePreference::Dark);
    }

    #[test]
    fn default_template_matches_runtime_default() {
        let default_jsonc =
            canonical_config_jsonc(&AppConfig::default(), UiLanguage::ZhCn).unwrap();
        let parsed = parse_config_draft(&default_jsonc).unwrap();
        assert!(!parsed.config.app.silent_startup);
        assert_eq!(parsed.config.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(parsed.normalized_json, default_jsonc);
        assert!(parsed.normalized_json.contains("// 配置结构版本"));
        assert!(parsed.normalized_json.contains("// 仅在本地匹配评分前"));
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
        assert_eq!(parsed.config.overlay.appearance.secondary_font_scale, 1.0);
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
        assert!(old_default
            .config
            .lyrics
            .providers
            .providers
            .iter()
            .all(|provider| provider.enabled));

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
    fn schema_fifteen_adds_default_title_keywords_but_preserves_an_empty_list() {
        let migrated = parse_config_draft(r#"{"schemaVersion":15}"#).unwrap();
        assert!(migrated.migrated);
        assert_eq!(
            migrated.config.lyrics.providers.title_filter_keywords,
            ProviderSettings::default().title_filter_keywords
        );

        let disabled = parse_config_draft(
            r#"{"schemaVersion":15,"lyrics":{"providers":{"titleFilterKeywords":[]}}}"#,
        )
        .unwrap();
        assert!(disabled
            .config
            .lyrics
            .providers
            .title_filter_keywords
            .is_empty());
    }

    #[test]
    fn previous_schema_discards_removed_app_fields() {
        let parsed = parse_config_draft(
            r#"{"schemaVersion":20,"app":{"removedField":true,"systemMediaApplications":[{"name":"Player","bundleId":"org.example.Player"}]}}"#,
        )
        .unwrap();
        assert!(parsed.migrated);
        assert_eq!(
            parsed.config.app.system_media_applications,
            [RegisteredApplication {
                name: "Player".into(),
                bundle_id: "org.example.Player".into(),
            }]
        );
        assert!(!parsed.normalized_json.contains("removedField"));
    }

    #[test]
    fn system_media_apps_reject_dedicated_players() {
        let applications = vec![
            RegisteredApplication {
                name: "Spotify".into(),
                bundle_id: "com.spotify.client".into(),
            },
            RegisteredApplication {
                name: "Player".into(),
                bundle_id: "org.example.Player".into(),
            },
        ];
        assert_eq!(
            normalize_system_media_applications(vec![applications[1].clone()]).unwrap(),
            [RegisteredApplication {
                name: "Player".into(),
                bundle_id: "org.example.Player".into(),
            }]
        );
        assert!(normalize_system_media_applications(vec![applications[0].clone()]).is_err());
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
            assert!(parsed
                .normalized_json
                .contains(&format!("\"schemaVersion\": {CONFIG_SCHEMA_VERSION}")));
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
                r#"{{"schemaVersion":{CONFIG_SCHEMA_VERSION},"overlay":{{"appearance":{{"layout":"{layout}"}}}}}}"#
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
              "schemaVersion": 27,
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
    fn schema_sixteen_migrates_to_current_schema() {
        let parsed = parse_config_draft(r#"{"schemaVersion":16}"#).unwrap();
        assert!(parsed.migrated);
        assert_eq!(parsed.config.schema_version, CONFIG_SCHEMA_VERSION);
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
            (r#"{"app":{"hideDockIcon":"yes"}}"#, "hideDockIcon"),
            (r#"{"app":{"playerSelection":"music"}}"#, "playerSelection"),
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
    fn dock_icon_preference_round_trips() {
        let parsed = parse_config_draft(r#"{"app":{"hideDockIcon":true}}"#).unwrap();
        assert!(parsed.config.app.hide_dock_icon);
        assert!(parsed.normalized_json.contains("\"hideDockIcon\": true"));
    }

    #[test]
    fn silent_startup_migrates_round_trips_and_validates() {
        let migrated = parse_config_draft(r#"{"schemaVersion":21}"#).unwrap();
        assert!(migrated.migrated);
        assert!(!migrated.config.app.silent_startup);

        let enabled = parse_config_draft(r#"{"app":{"silentStartup":true}}"#).unwrap();
        assert!(enabled.config.app.silent_startup);
        assert!(enabled.normalized_json.contains("\"silentStartup\": true"));

        let invalid = validate_config_draft(r#"{"app":{"silentStartup":"yes"}}"#);
        assert!(!invalid.valid);
        assert!(invalid
            .error
            .unwrap()
            .message
            .contains("silentStartup 必须是布尔值"));
    }

    #[test]
    fn update_preference_migrates_round_trips_and_validates() {
        let migrated = parse_config_draft(r#"{"schemaVersion":14,"app":{}}"#).unwrap();
        assert!(migrated.migrated);
        assert!(migrated.config.app.auto_check_updates);

        let disabled = parse_config_draft(r#"{"app":{"autoCheckUpdates":false}}"#).unwrap();
        assert!(!disabled.config.app.auto_check_updates);
        assert!(disabled
            .normalized_json
            .contains("\"autoCheckUpdates\": false"));

        let invalid = match parse_config_draft(r#"{"app":{"autoCheckUpdates":"yes"}}"#) {
            Ok(_) => panic!("string update preference should be rejected"),
            Err(error) => error,
        };
        assert!(invalid.message.contains("autoCheckUpdates 必须是布尔值"));
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
        let result = store.update(|config| config.app.theme = ThemePreference::Light);
        assert!(result.is_err());
        assert_eq!(store.snapshot().app.theme, ThemePreference::Dark);
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
        assert_eq!(store.snapshot().app.theme, ThemePreference::Dark);
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
        fs::write(root.join("config.json"), r#"{"app":{"theme":"light"}}"#).unwrap();
        let storage = Storage::open(root.clone(), root.join("library")).unwrap();
        let (store, migrated) = ConfigStore::load(&root, &storage).unwrap();
        assert!(!migrated);
        assert_eq!(store.snapshot().app.theme, ThemePreference::Light);
        let persisted = fs::read_to_string(root.join("config.json")).unwrap();
        assert!(persisted.contains("// Application theme"));
        assert!(persisted.contains("\"theme\": \"light\""));
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
            .update(|config| config.app.theme = ThemePreference::Light)
            .unwrap();
        let persisted = fs::read_to_string(root.join("config.json")).unwrap();
        assert!(persisted.contains("// Application theme"));
        assert!(persisted.contains("\"theme\": \"light\""));
        assert!(store
            .export_json()
            .unwrap()
            .contains("// Application theme"));
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
            .update(|config| config.app.theme = ThemePreference::Light)
            .unwrap();
        let mut stale = store.snapshot();
        stale.app.theme = ThemePreference::Dark;
        let error = store.replace_at_revision(stale, revision).unwrap_err();
        assert!(error.starts_with("config.conflict:"));
        assert_eq!(store.snapshot().app.theme, ThemePreference::Light);
        drop(storage);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn system_media_applications_validate_deduplicate_and_round_trip() {
        let parsed = parse_config_draft(
            r#"{"app":{"systemMediaApplications":[{"name":"Player","bundleId":"org.example.Player"},{"name":"Duplicate","bundleId":"org.example.Player"}]}}"#,
        )
        .unwrap();
        assert_eq!(parsed.config.app.system_media_applications.len(), 1);
        assert_eq!(
            parsed.config.app.system_media_applications[0].bundle_id,
            "org.example.Player"
        );
        assert!(parse_config_draft(
            r#"{"app":{"systemMediaApplications":[{"name":"Broken","bundleId":""}]}}"#
        )
        .is_err());
        assert!(parse_config_draft(
            r#"{"app":{"systemMediaApplications":[{"name":"Broken","bundleId":"not valid"}]}}"#
        )
        .is_err());
        for bundle_id in ["com.apple.Music", "com.spotify.client"] {
            let raw = format!(
                r#"{{"app":{{"systemMediaApplications":[{{"name":"Dedicated","bundleId":"{bundle_id}"}}]}}}}"#
            );
            assert!(parse_config_draft(&raw).is_err());
        }
        assert!(parse_config_draft(r#"{"app":{"systemMediaApplications":{}}}"#).is_err());
        let multiple = parse_config_draft(
            r#"{"app":{"systemMediaApplications":[{"name":"First","bundleId":"org.example.First"},{"name":"Second","bundleId":"org.example.Second"}]}}"#,
        )
        .unwrap();
        assert_eq!(multiple.config.app.system_media_applications.len(), 2);

        let migrated = parse_config_draft(
            r#"{"schemaVersion":22,"app":{"systemMediaApplications":[{"name":"First","bundleId":"org.example.First"},{"name":"Second","bundleId":"org.example.Second"}]}}"#,
        )
        .unwrap();
        assert_eq!(migrated.config.app.system_media_applications.len(), 2);
        assert_eq!(migrated.config.app.player_follower_application, None);
    }

    #[test]
    fn player_follower_is_a_separate_optional_application() {
        for bundle_id in [
            "com.apple.Music",
            "com.spotify.client",
            "org.example.Player",
        ] {
            let raw = format!(
                r#"{{"app":{{"playerFollowerApplication":{{"name":"Player","bundleId":"{bundle_id}"}}}}}}"#
            );
            let parsed = parse_config_draft(&raw).unwrap();
            assert_eq!(
                parsed
                    .config
                    .app
                    .player_follower_application
                    .as_ref()
                    .map(|application| application.bundle_id.as_str()),
                Some(bundle_id)
            );
        }
        assert!(parse_config_draft(
            r#"{"app":{"playerFollowerApplication":{"name":"Broken","bundleId":""}}}"#
        )
        .is_err());
        assert!(parse_config_draft(r#"{"app":{"playerFollowerApplication":[]}}"#).is_err());
    }
}
