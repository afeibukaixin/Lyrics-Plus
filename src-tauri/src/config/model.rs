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
    pub hide_menu_bar_icon: bool,
    pub silent_startup: bool,
    pub auto_check_updates: bool,
    pub lyrics_windows_show_on_all_spaces: bool,
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
            hide_menu_bar_icon: false,
            silent_startup: false,
            auto_check_updates: true,
            lyrics_windows_show_on_all_spaces: false,
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
    pub toggle_status_bar_lyrics: String,
    pub toggle_list_lyrics: String,
    pub toggle_notch_lyrics: String,
}

impl Default for GlobalShortcutSettings {
    fn default() -> Self {
        Self {
            toggle_overlay: "CommandOrControl+Shift+KeyL".into(),
            unlock_overlay: "CommandOrControl+Shift+KeyU".into(),
            reset_overlay: "CommandOrControl+Shift+Digit0".into(),
            toggle_status_bar_lyrics: String::new(),
            toggle_list_lyrics: String::new(),
            toggle_notch_lyrics: String::new(),
        }
    }
}

impl GlobalShortcutSettings {
    pub fn parsed(&self) -> Result<([Shortcut; 3], [Option<Shortcut>; 3]), String> {
        let mut parsed = Vec::<Shortcut>::with_capacity(6);
        let mut parse = |label: &str, value: &str, optional: bool| {
            let value = value.trim();
            if optional && value.is_empty() {
                return Ok(None);
            }
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
                return Err("全局快捷键不能重复".into());
            }
            parsed.push(shortcut);
            Ok(Some(shortcut))
        };
        let required = [
            parse("显示 / 隐藏桌面歌词", &self.toggle_overlay, false)?
                .ok_or_else(|| "全局快捷键配置不完整".to_string())?,
            parse("锁定 / 解锁桌面歌词", &self.unlock_overlay, false)?
                .ok_or_else(|| "全局快捷键配置不完整".to_string())?,
            parse("复位并显示桌面歌词", &self.reset_overlay, false)?
                .ok_or_else(|| "全局快捷键配置不完整".to_string())?,
        ];
        let optional = [
            parse(
                "显示 / 隐藏菜单栏歌词",
                &self.toggle_status_bar_lyrics,
                true,
            )?,
            parse("显示 / 隐藏歌词窗口", &self.toggle_list_lyrics, true)?,
            parse("显示 / 隐藏灵动岛歌词", &self.toggle_notch_lyrics, true)?,
        ];
        Ok((required, optional))
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CompactKaraokeStyle {
    #[default]
    Sweep,
    Highlight,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StatusBarAlignment {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotchSlotContent {
    Empty,
    Title,
    Artist,
    Artwork,
    Spectrum,
}

impl Default for NotchSlotContent {
    fn default() -> Self {
        Self::Artwork
    }
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
    pub hide_when_not_playing: bool,
    pub appearance: StatusBarLyricsAppearance,
}

impl Default for StatusBarLyricsPreferences {
    fn default() -> Self {
        Self {
            enabled: false,
            hide_when_not_playing: false,
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
    pub karaoke_style: CompactKaraokeStyle,
    pub alignment: StatusBarAlignment,
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
            karaoke_style: CompactKaraokeStyle::Sweep,
            alignment: StatusBarAlignment::Left,
            width: 220,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ListLyricsPreferences {
    pub enabled: bool,
    pub always_on_top: bool,
    pub show_translation: bool,
    pub show_romanization: bool,
    pub appearance: ListLyricsAppearance,
}

impl Default for ListLyricsPreferences {
    fn default() -> Self {
        Self {
            enabled: false,
            always_on_top: false,
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
    pub background_opacity: f64,
    pub background_mode: String,
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
            background_opacity: 1.0,
            background_mode: "solid".into(),
            alignment: "center".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NotchLyricsPreferences {
    pub enabled: bool,
    pub hide_when_not_playing: bool,
    pub monitor_id: Option<String>,
    pub show_lyrics: bool,
    pub left_slot: NotchSlotContent,
    pub right_slot: NotchSlotContent,
    pub layout: OverlayLayout,
    pub double_line_mode: DoubleLineMode,
    pub show_translation: bool,
    pub show_romanization: bool,
    pub appearance: NotchLyricsAppearance,
}

impl Default for NotchLyricsPreferences {
    fn default() -> Self {
        Self {
            enabled: false,
            hide_when_not_playing: false,
            monitor_id: None,
            show_lyrics: false,
            left_slot: NotchSlotContent::Artwork,
            right_slot: NotchSlotContent::Spectrum,
            layout: OverlayLayout::Single,
            double_line_mode: DoubleLineMode::Rolling,
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
    pub secondary_font_weight: u16,
    pub active_color: String,
    pub inactive_color: String,
    pub translation_color: String,
    pub romanization_color: String,
    pub karaoke_style: CompactKaraokeStyle,
    pub line_gap: f64,
    pub border_radius: f64,
    pub max_width: u16,
    pub expanded_max_width: u16,
}

impl Default for NotchLyricsAppearance {
    fn default() -> Self {
        Self {
            font_family: OverlayAppearance::default().font_family,
            font_size: 18,
            font_weight: 700,
            secondary_font_weight: 500,
            active_color: "#a3e635".into(),
            inactive_color: "#ecfccb".into(),
            translation_color: "#d9f99d".into(),
            romanization_color: "#bef264".into(),
            karaoke_style: CompactKaraokeStyle::Sweep,
            line_gap: 8.0,
            border_radius: 22.0,
            max_width: 320,
            expanded_max_width: 440,
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
    pub double_line_mode: DoubleLineMode,
    pub orientation: OverlayOrientation,
    pub alignment: OverlayAlignment,
    pub primary_line_position: crate::overlay_model::PrimaryLinePosition,
    pub line_gap: f64,
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
    pub text_stroke_width: f64,
    pub text_stroke_color: String,
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
            double_line_mode: style.double_line_mode,
            orientation: style.orientation,
            alignment: style.alignment,
            primary_line_position: style.primary_line_position,
            line_gap: style.line_gap,
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
            text_stroke_width: style.text_stroke_width,
            text_stroke_color: style.text_stroke_color.clone(),
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
            double_line_mode: self.double_line_mode,
            orientation: self.orientation,
            alignment: self.alignment,
            primary_line_position: self.primary_line_position,
            line_gap: self.line_gap,
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
            text_stroke_width: self.text_stroke_width,
            text_stroke_color: self.text_stroke_color,
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
        list_appearance.background_opacity =
            list_appearance.background_opacity.clamp(0.0, 1.0);
        if !matches!(
            list_appearance.background_mode.as_str(),
            "solid" | "transparent"
        ) {
            list_appearance.background_mode = "solid".into();
        }
        if !matches!(
            list_appearance.alignment.as_str(),
            "left" | "center" | "right"
        ) {
            list_appearance.alignment = "center".into();
        }
        let notch_appearance = &mut self.lyrics.displays.notch.appearance;
        notch_appearance.font_size = notch_appearance.font_size.clamp(12, 32);
        notch_appearance.font_weight = normalize_display_font_weight(notch_appearance.font_weight);
        notch_appearance.secondary_font_weight =
            normalize_display_font_weight(notch_appearance.secondary_font_weight);
        notch_appearance.line_gap = notch_appearance.line_gap.clamp(0.0, 32.0);
        notch_appearance.border_radius = notch_appearance.border_radius.clamp(0.0, 40.0);
        notch_appearance.max_width = notch_appearance.max_width.clamp(320, 640);
        notch_appearance.expanded_max_width = notch_appearance
            .expanded_max_width
            .clamp(440, 640)
            .max(notch_appearance.max_width);
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
