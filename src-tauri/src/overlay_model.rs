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
pub enum DoubleLineMode {
    #[default]
    Rolling,
    Alternating,
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
    Start,
    End,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PrimaryLinePosition {
    #[default]
    First,
    Second,
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

/// A single user-defined fallback font family.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FontFamily {
    pub name: String,
    pub family: String,
}

pub(crate) const SYSTEM_FONT_FAMILY: &str = "-apple-system";
pub(crate) const SYSTEM_FONT_NAME: &str = "System Default";

pub(crate) fn default_font_families() -> String {
    "system-ui, sans-serif".into()
}

pub(crate) fn font_family_from_families(families: &[FontFamily]) -> String {
    let mut stack = Vec::with_capacity(families.len() + 2);
    let primary_is_system = families
        .first()
        .is_some_and(|item| is_system_font_family(&item.family));
    if primary_is_system {
        stack.push(SYSTEM_FONT_FAMILY.into());
    }
    let mut has_generic_fallback = false;
    let mut has_sans_serif = false;
    for (index, item) in families.iter().enumerate() {
        let family = item.family.trim();
        if family.is_empty() {
            continue;
        }
        if (index == 0 && primary_is_system) || is_legacy_system_alias(family) {
            continue;
        }
        if family.eq_ignore_ascii_case("system-ui") || family.eq_ignore_ascii_case("sans-serif") {
            has_generic_fallback = true;
            has_sans_serif |= family.eq_ignore_ascii_case("sans-serif");
            stack.push(family.to_ascii_lowercase());
        } else {
            stack.push(format!("\"{}\"", escape_css_string(family)));
        }
    }
    if !has_generic_fallback {
        stack.push("system-ui".into());
    }
    if !has_sans_serif {
        stack.push("sans-serif".into());
    }
    stack.join(", ")
}

/// Build the CSS stack used by lyric surfaces from the persisted primary and
/// comma-separated fallback settings.
pub(crate) fn font_family_stack(font_family: &str, font_families: &str) -> String {
    let parsed = font_families_from_css(font_family);
    let primary = parsed.first().cloned().unwrap_or_else(|| FontFamily {
        name: SYSTEM_FONT_NAME.into(),
        family: SYSTEM_FONT_FAMILY.into(),
    });
    let fallbacks = font_fallbacks_from_css(font_families);
    let mut entries = Vec::with_capacity(fallbacks.len() + 1);
    entries.push(primary);
    entries.extend(fallbacks);
    font_family_from_families(&entries)
}

/// Parse a legacy CSS font-family value into ordered user entries.
pub(crate) fn font_families_from_css(value: &str) -> Vec<FontFamily> {
    split_font_family_stack(value)
        .into_iter()
        .enumerate()
        .map(|(index, family)| {
            let item = if index == 0 && is_system_font_family(&family) {
                FontFamily {
                    name: SYSTEM_FONT_NAME.into(),
                    family: SYSTEM_FONT_FAMILY.into(),
                }
            } else {
                FontFamily {
                    name: family.clone(),
                    family,
                }
            };
            (index, item)
        })
        .filter(|(index, item)| *index == 0 || !is_legacy_system_alias(&item.family))
        .map(|(_, item)| item)
        .collect()
}

/// Parse fallback-only entries without treating the first system-ui token as a primary font.
fn font_fallbacks_from_css(value: &str) -> Vec<FontFamily> {
    split_font_family_stack(value)
        .into_iter()
        .filter(|family| !is_legacy_system_alias(family))
        .map(|family| FontFamily {
            name: family.clone(),
            family,
        })
        .collect()
}

fn split_font_family_stack(value: &str) -> Vec<String> {
    let mut families = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' && quote.is_some() {
            escaped = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if character == active_quote {
                quote = None;
            } else {
                current.push(character);
            }
        } else if matches!(character, '\'' | '"') {
            quote = Some(character);
        } else if character == ',' {
            let family = current.trim();
            if !family.is_empty() {
                families.push(family.to_owned());
            }
            current.clear();
        } else {
            current.push(character);
        }
    }
    if escaped {
        current.push('\\');
    }
    let family = current.trim();
    if !family.is_empty() {
        families.push(family.to_owned());
    }
    families
}

pub(crate) fn normalize_font_families(families: &mut String, font_family: &mut String) {
    let parsed = font_families_from_css(font_family);
    let mut primary = parsed.first().cloned().unwrap_or_else(|| FontFamily {
        name: SYSTEM_FONT_NAME.into(),
        family: SYSTEM_FONT_FAMILY.into(),
    });
    let items = if parsed.len() > 1 {
        parsed.iter().skip(1).cloned().collect()
    } else {
        font_fallbacks_from_css(families)
    };

    primary.name = primary.name.trim().to_owned();
    primary.family = primary.family.trim().to_owned();
    if is_system_font_family(&primary.family) || primary.family.is_empty() {
        primary = FontFamily {
            name: SYSTEM_FONT_NAME.into(),
            family: SYSTEM_FONT_FAMILY.into(),
        };
    } else if primary.name.is_empty() {
        primary.name = primary.family.clone();
    }

    let mut normalized = Vec::with_capacity(items.len());
    for mut item in items {
        item.name = item.name.trim().to_owned();
        item.family = item.family.trim().to_owned();
        if item.family.is_empty() || is_legacy_system_alias(&item.family) {
            continue;
        }
        if item.family.eq_ignore_ascii_case(&primary.family)
            || normalized
                .iter()
                .any(|existing: &FontFamily| existing.family.eq_ignore_ascii_case(&item.family))
        {
            continue;
        }
        if item.name.is_empty() {
            item.name = item.family.clone();
        }
        normalized.push(item);
    }
    *font_family = primary.family;
    *families = font_fallbacks_from_families(&normalized);
}

pub(crate) fn font_fallbacks_from_families(families: &[FontFamily]) -> String {
    families
        .iter()
        .map(|item| {
            let family = item.family.trim();
            if family.eq_ignore_ascii_case("system-ui") || family.eq_ignore_ascii_case("sans-serif")
            {
                family.to_ascii_lowercase()
            } else {
                format!("\"{}\"", escape_css_string(family))
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn is_system_font_family(family: &str) -> bool {
    matches!(
        family.trim().to_ascii_lowercase().as_str(),
        "-apple-system" | "system-ui" | "blinkmacsystemfont" | "sans-serif"
    )
}

fn is_legacy_system_alias(family: &str) -> bool {
    matches!(
        family.trim().to_ascii_lowercase().as_str(),
        "-apple-system" | "blinkmacsystemfont"
    )
}

fn escape_css_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\a ")
        .replace('\r', "\\d ")
        .replace('\u{000c}', "\\c ")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OverlayStyleSettings {
    pub font_family: String,
    pub font_families: String,
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
    pub primary_line_position: PrimaryLinePosition,
    pub line_gap: f64,
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
    pub text_stroke_width: f64,
    pub text_stroke_color: String,
    pub horizontal_max_width: Option<f64>,
    pub vertical_max_height: Option<f64>,
}

impl Default for OverlayStyleSettings {
    fn default() -> Self {
        Self {
            font_family: SYSTEM_FONT_FAMILY.into(),
            font_families: default_font_families(),
            font_size: 36,
            font_weight: 400,
            secondary_font_weight: 400,
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
            double_line_mode: DoubleLineMode::Rolling,
            orientation: OverlayOrientation::Horizontal,
            alignment: OverlayAlignment::Center,
            primary_line_position: PrimaryLinePosition::First,
            line_gap: 8.0,
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
            text_stroke_width: 0.0,
            text_stroke_color: "#000000".into(),
            horizontal_max_width: None,
            vertical_max_height: None,
        }
    }
}

impl OverlayStyleSettings {
    pub(crate) fn normalized(mut self) -> Self {
        normalize_font_families(&mut self.font_families, &mut self.font_family);
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
        self.line_gap = self.line_gap.clamp(0.0, 32.0);
        self.text_shadow_offset_x = self.text_shadow_offset_x.clamp(-20.0, 20.0);
        self.text_shadow_offset_y = self.text_shadow_offset_y.clamp(-20.0, 20.0);
        self.text_shadow_blur = self.text_shadow_blur.clamp(0.0, 40.0);
        self.text_stroke_width = self.text_stroke_width.clamp(0.0, 8.0);
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
        if self.text_stroke_color.trim().is_empty() {
            self.text_stroke_color = "#000000".into();
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
    [100_u16, 200, 300, 400, 500, 600, 700, 800, 900]
        .into_iter()
        .min_by_key(|weight| weight.abs_diff(value))
        .unwrap_or(800)
}
