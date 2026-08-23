use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OverlayBackground {
    #[default]
    Glass,
    Transparent,
    Solid,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OverlayBackgroundMode {
    #[default]
    Solid,
    Transparent,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OverlayLayout {
    #[default]
    Single,
    Double,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OverlayOrientation {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OverlayAlignment {
    #[default]
    Center,
    #[serde(alias = "left", alias = "right")]
    Distributed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LongTextMode {
    #[default]
    Shrink,
    Wrap,
    Marquee,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum KaraokeStyle {
    #[default]
    Sweep,
    // 兼容已持久化的旧选项；归一化后会保存为 Sweep。
    Fill,
    Bounce,
    Highlight,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SecondaryDisplayMode {
    #[default]
    Legacy,
    Next,
    Translation,
    Romanization,
    TranslationRomanization,
}

fn legacy_secondary_display() -> SecondaryDisplayMode {
    SecondaryDisplayMode::Legacy
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OverlayStyleSettings {
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
    #[serde(default = "legacy_secondary_display")]
    pub secondary_display: SecondaryDisplayMode,
    pub auto_center_with_translation_or_romanization: bool,
    #[serde(skip_serializing)]
    pub translation_enabled: bool,
    #[serde(skip_serializing)]
    pub romanization_enabled: bool,
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
    pub horizontal_max_width: Option<f64>,
    pub vertical_max_height: Option<f64>,
}

impl Default for OverlayStyleSettings {
    fn default() -> Self {
        Self {
            font_family: "Inter, \"SF Pro Text\", \"SF Pro Display\", -apple-system, BlinkMacSystemFont, \"Segoe UI\", \"PingFang SC\", \"Hiragino Sans GB\", \"Microsoft YaHei\", \"Noto Sans CJK SC\", \"Noto Sans SC\", Arial, sans-serif".into(),
            font_size: 36,
            font_weight: 800,
            secondary_font_weight: 500,
            line_height: 1.2,
            active_color: "#a3e635".into(),
            inactive_color: "#ecfccb".into(),
            opacity: 1.0,
            background_opacity: 0.6,
            background_blur: 18.0,
            background_radius: 18.0,
            background_padding_x: 26.0,
            background_padding_y: 22.0,
            background_mode: OverlayBackgroundMode::Solid,
            background: OverlayBackground::Glass,
            solid_color: "#171821".into(),
            layout: OverlayLayout::Single,
            orientation: OverlayOrientation::Horizontal,
            alignment: OverlayAlignment::Center,
            long_text: LongTextMode::Marquee,
            secondary_display: SecondaryDisplayMode::TranslationRomanization,
            auto_center_with_translation_or_romanization: false,
            translation_enabled: true,
            romanization_enabled: true,
            karaoke_style: KaraokeStyle::Sweep,
            secondary_font_scale: 1.0,
            translation_font_scale: 0.8,
            romanization_font_scale: 0.8,
            translation_color: "#d9f99d".into(),
            romanization_color: "#bef264".into(),
            text_shadow_offset_x: 0.0,
            text_shadow_offset_y: 1.0,
            text_shadow_blur: 4.0,
            text_shadow_color: "rgba(0, 0, 0, 0.55)".into(),
            horizontal_max_width: None,
            vertical_max_height: None,
        }
    }
}

impl OverlayStyleSettings {
    pub(crate) fn normalized(mut self) -> Self {
        if self.secondary_display == SecondaryDisplayMode::Legacy {
            self.secondary_display = if self.translation_enabled {
                SecondaryDisplayMode::Translation
            } else if self.romanization_enabled {
                SecondaryDisplayMode::Romanization
            } else {
                SecondaryDisplayMode::Next
            };
        }
        self.font_size = self.font_size.clamp(16, 72);
        self.font_weight = nearest_overlay_font_weight(self.font_weight);
        self.secondary_font_weight = nearest_overlay_font_weight(self.secondary_font_weight);
        self.line_height = self.line_height.clamp(0.8, 2.0);
        self.opacity = self.opacity.clamp(0.2, 1.0);
        self.background_opacity = self.background_opacity.clamp(0.0, 1.0);
        self.background_blur = self.background_blur.clamp(0.0, 40.0);
        self.background_radius = self.background_radius.clamp(0.0, 64.0);
        self.background_padding_x = self.background_padding_x.clamp(0.0, 64.0);
        self.background_padding_y = self.background_padding_y.clamp(0.0, 64.0);
        self.text_shadow_offset_x = self.text_shadow_offset_x.clamp(-20.0, 20.0);
        self.text_shadow_offset_y = self.text_shadow_offset_y.clamp(-20.0, 20.0);
        self.text_shadow_blur = self.text_shadow_blur.clamp(0.0, 40.0);
        if self.background == OverlayBackground::Transparent {
            self.background = OverlayBackground::Solid;
            self.background_mode = OverlayBackgroundMode::Transparent;
        }
        self.secondary_font_scale = self.secondary_font_scale.clamp(0.35, 1.0);
        self.translation_font_scale = self.translation_font_scale.clamp(0.35, 1.0);
        self.romanization_font_scale = self.romanization_font_scale.clamp(0.35, 1.0);
        self.horizontal_max_width = self
            .horizontal_max_width
            .filter(|value| value.is_finite())
            .map(|value| value.clamp(320.0, 10_000.0));
        self.vertical_max_height = self
            .vertical_max_height
            .filter(|value| value.is_finite())
            .map(|value| value.clamp(280.0, 10_000.0));
        if self.karaoke_style == KaraokeStyle::Fill {
            self.karaoke_style = KaraokeStyle::Sweep;
        }
        if self.active_color.trim().is_empty() {
            self.active_color = "#a3e635".into();
        }
        if self.font_family.trim().is_empty() {
            self.font_family = Self::default().font_family;
        } else {
            self.font_family = self.font_family.trim().to_string();
        }
        if self.text_shadow_color.trim().is_empty() {
            self.text_shadow_color = "rgba(0, 0, 0, 0.55)".into();
        }
        if self.inactive_color.trim().is_empty() {
            self.inactive_color = "#ecfccb".into();
        }
        if self.solid_color.trim().is_empty() {
            self.solid_color = "#171821".into();
        }
        if self.translation_color.trim().is_empty() {
            self.translation_color = "#d9f99d".into();
        }
        if self.romanization_color.trim().is_empty() {
            self.romanization_color = "#bef264".into();
        }
        self
    }
}

fn nearest_overlay_font_weight(value: u16) -> u16 {
    [400_u16, 500, 600, 700, 800]
        .into_iter()
        .min_by_key(|weight| weight.abs_diff(value))
        .unwrap_or(800)
}
