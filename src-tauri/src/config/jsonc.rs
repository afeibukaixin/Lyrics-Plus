use serde::ser::{SerializeMap, SerializeSeq, Serializer};

fn canonical_config_jsonc(value: &AppConfig, language: UiLanguage) -> Result<String, String> {
    let value = serde_json::to_value(value).map_err(|error| format!("序列化配置失败：{error}"))?;
    canonical_jsonc(&value, language)
}

fn canonical_user_jsonc(value: &Value, language: UiLanguage) -> Result<String, String> {
    canonical_jsonc(value, language)
}

fn canonical_jsonc(value: &Value, language: UiLanguage) -> Result<String, String> {
    let json = serde_json::to_string_pretty(&CanonicalJsonValue {
        value,
        path: String::new(),
    })
    .map_err(|error| format!("序列化配置失败：{error}"))?;
    let mut output = String::with_capacity(json.len() + 1_200);
    let mut desktop_state_comments_remaining = 0_u8;
    for line in json.lines() {
        if line.starts_with("      \"desktop\":") {
            desktop_state_comments_remaining = 2;
        } else if desktop_state_comments_remaining > 0
            && line.starts_with("      \"")
            && !line.starts_with("        \"")
        {
            // Sparse user documents may omit desktop state fields. Stop the
            // state-specific comment scope before reaching another sibling.
            desktop_state_comments_remaining = 0;
        }
        let comment = match line {
            line if line.starts_with("  \"schemaVersion\":") => {
                Some(("  ", ConfigComment::SchemaVersion))
            }
            line if line.starts_with("    \"theme\":") => Some(("    ", ConfigComment::Theme)),
            line if line.starts_with("    \"uiFontFamily\":") => {
                Some(("    ", ConfigComment::UiFontFamily))
            }
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
            line if line.starts_with("    \"hideMenuBarIcon\":") => {
                Some(("    ", ConfigComment::HideMenuBarIcon))
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
            line if line.starts_with("      \"autoSearchDebounceMs\":") => {
                Some(("      ", ConfigComment::AutoSearchDebounce))
            }
            line if line.starts_with("      \"maxCandidatesPerProvider\":") => {
                Some(("      ", ConfigComment::MaxCandidatesPerProvider))
            }
            line if line.starts_with("      \"titleFilterKeywords\":") => {
                Some(("      ", ConfigComment::TitleFilterKeywords))
            }
            line if line.starts_with("      \"amllBaseUrl\":") => {
                Some(("      ", ConfigComment::AmllBaseUrl))
            }
            line if line.starts_with("      \"mode\":") => {
                Some(("      ", ConfigComment::ProviderMode))
            }
            line if line.starts_with("      \"providers\":") => {
                Some(("      ", ConfigComment::Providers))
            }
            line if line.starts_with("    \"chineseConversion\":") => {
                Some(("    ", ConfigComment::ChineseConversion))
            }
            line if line.starts_with("    \"repairSimplifiedJapanese\":") => {
                Some(("    ", ConfigComment::RepairSimplifiedJapanese))
            }
            line if line.starts_with("    \"displays\":") => {
                Some(("    ", ConfigComment::LyricsDisplays))
            }
            line if desktop_state_comments_remaining > 0
                && line.starts_with("        \"enabled\":") => {
                desktop_state_comments_remaining -= 1;
                Some(("        ", ConfigComment::OverlayState))
            }
            line if desktop_state_comments_remaining > 0
                && line.starts_with("        \"hideWhenNotPlaying\":") => {
                desktop_state_comments_remaining -= 1;
                Some(("        ", ConfigComment::HideWhenNotPlaying))
            }
            // display-mode appearance and presentation fields now live two levels below lyrics.displays
            line if line.starts_with("          \"fontSize\":") => {
                Some(("          ", ConfigComment::FontSize))
            }
            line if line.starts_with("          \"fontFamily\":") => {
                Some(("          ", ConfigComment::FontFamily))
            }
            line if line.starts_with("          \"fontFamilies\":") => {
                Some(("          ", ConfigComment::FontFamilies))
            }
            line if line.starts_with("          \"lineHeight\":") => {
                Some(("          ", ConfigComment::LineHeight))
            }
            line if line.starts_with("          \"opacity\":") => {
                Some(("          ", ConfigComment::Opacity))
            }
            line if line.starts_with("          \"backgroundOpacity\":") => {
                Some(("          ", ConfigComment::BackgroundOpacity))
            }
            line if line.starts_with("          \"backgroundBlur\":") => {
                Some(("          ", ConfigComment::BackgroundBlur))
            }
            line if line.starts_with("          \"backgroundRadius\":") => {
                Some(("          ", ConfigComment::BackgroundGeometry))
            }
            line if line.starts_with("          \"backgroundMode\":") => {
                Some(("          ", ConfigComment::BackgroundMode))
            }
            line if line.starts_with("          \"background\":") => {
                Some(("          ", ConfigComment::Background))
            }
            line if line.starts_with("          \"layout\":") => {
                Some(("          ", ConfigComment::Layout))
            }
            line if line.starts_with("          \"doubleLineMode\":") => {
                Some(("          ", ConfigComment::DoubleLineMode))
            }
            line if line.starts_with("          \"showTranslation\":") => {
                Some(("          ", ConfigComment::ShowTranslation))
            }
            line if line.starts_with("          \"showRomanization\":") => {
                Some(("          ", ConfigComment::ShowRomanization))
            }
            line if line.starts_with("        \"lineOrder\":") => {
                Some(("        ", ConfigComment::LineOrder))
            }
            line if line.starts_with("          \"primaryLinePosition\":") => {
                Some(("          ", ConfigComment::PrimaryLinePosition))
            }
            line if line.starts_with("          \"lineGap\":") => {
                Some(("          ", ConfigComment::LineGap))
            }
            line if line.starts_with("          \"secondaryLineGap\":") => {
                Some(("          ", ConfigComment::SecondaryLineGap))
            }
            line if line.starts_with("          \"longText\":") => {
                Some(("          ", ConfigComment::LongText))
            }
            line if line.starts_with("          \"autoCenterWithTranslationOrRomanization\":") => {
                Some(("          ", ConfigComment::AutoCenter))
            }
            line if line.starts_with("          \"karaokeStyle\":") => {
                Some(("          ", ConfigComment::KaraokeStyle))
            }
            line if line.starts_with("          \"secondaryFontScale\":") => {
                Some(("          ", ConfigComment::SecondaryFontScale))
            }
            line if line.starts_with("          \"activeOpacity\":") => {
                Some(("          ", ConfigComment::ActiveLyricsOpacity))
            }
            line if line.starts_with("          \"inactiveOpacity\":") => {
                Some(("          ", ConfigComment::InactiveLyricsOpacity))
            }
            line if line.starts_with("          \"textShadowOffsetX\":") => {
                Some(("          ", ConfigComment::TextShadow))
            }
            line if line.starts_with("          \"textStrokeWidth\":") => {
                Some(("          ", ConfigComment::TextStroke))
            }
            line if line.starts_with("    \"lyricsWindowsShowOnAllSpaces\":") => {
                Some(("    ", ConfigComment::LyricsWindowsSpaceBehavior))
            }
            line if line.starts_with("      \"fontSize\":") => {
                Some(("      ", ConfigComment::FontSize))
            }
            line if line.starts_with("      \"fontFamily\":") => {
                Some(("      ", ConfigComment::FontFamily))
            }
            line if line.starts_with("      \"fontFamilies\":") => {
                Some(("      ", ConfigComment::FontFamilies))
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
            line if line.starts_with("      \"doubleLineMode\":") => {
                Some(("      ", ConfigComment::DoubleLineMode))
            }
            line if line.starts_with("        \"doubleLineMode\":") => {
                Some(("        ", ConfigComment::DoubleLineMode))
            }
            line if line.starts_with("      \"alignment\":") => {
                Some(("      ", ConfigComment::Alignment))
            }
            line if line.starts_with("          \"alignment\":") => {
                Some(("          ", ConfigComment::StatusBarAlignment))
            }
            line if line.starts_with("          \"verticalOffset\":") => {
                Some(("          ", ConfigComment::StatusBarVerticalOffset))
            }
            line if line.starts_with("      \"primaryLinePosition\":") => {
                Some(("      ", ConfigComment::PrimaryLinePosition))
            }
            line if line.starts_with("      \"lineGap\":") => {
                Some(("      ", ConfigComment::LineGap))
            }
            line if line.starts_with("          \"borderRadius\":") => {
                Some(("          ", ConfigComment::NotchDefaultBorderRadius))
            }
            line if line.starts_with("          \"expandedBorderRadius\":") => {
                Some(("          ", ConfigComment::NotchExpandedBorderRadius))
            }
            line if line.starts_with("          \"topBorderRadius\":") => {
                Some(("          ", ConfigComment::NotchTopBorderRadius))
            }
            line if line.starts_with("        \"inlineLyricsOnNonNotch\":") => {
                Some(("        ", ConfigComment::NotchInlineLyricsOnNonNotch))
            }
            line if line.starts_with("      \"secondaryLineGap\":") => {
                Some(("      ", ConfigComment::SecondaryLineGap))
            }
            line if line.starts_with("      \"longText\":") => {
                Some(("      ", ConfigComment::LongText))
            }
            line if line.starts_with("      \"secondaryDisplay\":") => {
                Some(("      ", ConfigComment::SecondaryDisplay))
            }
            line if line.starts_with("          \"supportingPriority\":") => {
                Some(("          ", ConfigComment::SupportingPriority))
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
            line if line.starts_with("      \"activeOpacity\":") => {
                Some(("      ", ConfigComment::ActiveLyricsOpacity))
            }
            line if line.starts_with("      \"inactiveOpacity\":") => {
                Some(("      ", ConfigComment::InactiveLyricsOpacity))
            }
            line if line.starts_with("      \"textShadowOffsetX\":") => {
                Some(("      ", ConfigComment::TextShadow))
            }
            line if line.starts_with("      \"textStrokeWidth\":") => {
                Some(("      ", ConfigComment::TextStroke))
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

/// `serde_json::Value` 默认使用按键名排序的 BTreeMap；配置文件则沿用
/// AppConfig 的声明顺序，便于在默认配置和稀疏用户配置之间对照阅读。
struct CanonicalJsonValue<'a> {
    value: &'a Value,
    path: String,
}

impl serde::Serialize for CanonicalJsonValue<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.value {
            Value::Null => serializer.serialize_none(),
            Value::Bool(value) => serializer.serialize_bool(*value),
            Value::Number(value) => value.serialize(serializer),
            Value::String(value) => serializer.serialize_str(value),
            Value::Array(values) => {
                let mut sequence = serializer.serialize_seq(Some(values.len()))?;
                for value in values {
                    sequence.serialize_element(&CanonicalJsonValue {
                        value,
                        path: self.path.clone(),
                    })?;
                }
                sequence.end()
            }
            Value::Object(values) => {
                let keys = ordered_keys(&self.path, values);
                let mut object = serializer.serialize_map(Some(keys.len()))?;
                for key in keys {
                    let value = values
                        .get(key)
                        .expect("ordered configuration key must exist");
                    let path = if self.path.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", self.path, key)
                    };
                    object.serialize_entry(
                        key,
                        &CanonicalJsonValue { value, path },
                    )?;
                }
                object.end()
            }
        }
    }
}

fn ordered_keys<'a>(
    path: &str,
    values: &'a serde_json::Map<String, Value>,
) -> Vec<&'a String> {
    let preferred = match path {
        "" => &[
            "schemaVersion",
            "app",
            "lyrics",
        ][..],
        "app" => &[
            "theme",
            "uiFontFamily",
            "language",
            "playerSelection",
            "systemMediaFilterMode",
            "systemMediaApplications",
            "playerFollowerApplication",
            "hideDockIcon",
            "hideMenuBarIcon",
            "silentStartup",
            "autoCheckUpdates",
            "lyricsWindowsShowOnAllSpaces",
            "shortcuts",
        ][..],
        "app.shortcuts" => &[
            "toggleOverlay",
            "unlockOverlay",
            "resetOverlay",
            "toggleStatusBarLyrics",
            "toggleListLyrics",
            "toggleNotchLyrics",
            "switchLyrics",
        ][..],
        "app.systemMediaApplications" | "app.playerFollowerApplication" =>
            &["name", "bundleId"][..],
        "lyrics" => &[
            "chineseConversion",
            "repairSimplifiedJapanese",
            "providers",
            "displays",
            "baseAppearance",
            "styleInheritance",
        ][..],
        "lyrics.providers" => &[
            "mode",
            "providers",
            "autoApplyThreshold",
            "autoSearchDebounceMs",
            "maxCandidatesPerProvider",
            "preferCapabilities",
            "capabilityPreferenceTolerance",
            "matchWeights",
            "normalizeChinese",
            "titleFilterKeywords",
            "amllBaseUrl",
        ][..],
        "lyrics.providers.providers" => &["id", "enabled"][..],
        "lyrics.providers.matchWeights" => &["title", "artist", "album", "duration", "version"][..],
        "lyrics.baseAppearance" => &[
            "fontFamily",
            "fontFamilies",
            "activeColor",
            "inactiveColor",
            "translationColor",
            "romanizationColor",
            "supportingColor",
            "backgroundColor",
        ][..],
        "lyrics.styleInheritance" => &["desktop", "statusBar", "listWindow", "notch"][..],
        "lyrics.styleInheritance.desktop"
        | "lyrics.styleInheritance.statusBar"
        | "lyrics.styleInheritance.listWindow"
        | "lyrics.styleInheritance.notch" => &["inheritFontFamily", "inheritColors"][..],
        "lyrics.displays" => &["desktop", "statusBar", "listWindow", "notch"][..],
        "lyrics.displays.desktop" => &[
            "enabled",
            "locked",
            "hideWhenNotPlaying",
            "presentation",
            "appearance",
        ][..],
        "lyrics.displays.desktop.presentation" => &[
            "layout",
            "doubleLineMode",
            "showTranslation",
            "showRomanization",
            "supportingPriority",
            "orientation",
            "alignment",
            "primaryLinePosition",
            "longText",
            "autoCenterWithTranslationOrRomanization",
        ][..],
        "lyrics.displays.statusBar" => &[
            "enabled",
            "hideWhenNotPlaying",
            "presentation",
            "appearance",
        ][..],
        "lyrics.displays.statusBar.presentation" => &[
            "layout",
            "doubleLineMode",
            "showTranslation",
            "showRomanization",
            "supportingPriority",
            "alignment",
        ][..],
        "lyrics.displays.statusBar.appearance" => &[
            "fontFamily",
            "fontFamilies",
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
            "width",
        ][..],
        "lyrics.displays.listWindow" => &[
            "enabled",
            "alwaysOnTop",
            "locked",
            "showTranslation",
            "showRomanization",
            "lineOrder",
            "appearance",
        ][..],
        "lyrics.displays.listWindow.appearance" => &[
            "fontFamily",
            "fontFamilies",
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
            "textShadowOffsetX",
            "textShadowOffsetY",
            "textShadowBlur",
            "textShadowColor",
            "textStrokeWidth",
            "textStrokeColor",
            "alignment",
        ][..],
        "lyrics.displays.notch" => &[
            "enabled",
            "hideWhenNotPlaying",
            "monitorId",
            "showLyrics",
            "leftSlot",
            "rightSlot",
            "presentation",
            "inlineLyricsOnNonNotch",
            "appearance",
        ][..],
        "lyrics.displays.notch.presentation" => &[
            "layout",
            "doubleLineMode",
            "showTranslation",
            "showRomanization",
            "supportingPriority",
        ][..],
        "lyrics.displays.notch.appearance" => &[
            "fontFamily",
            "fontFamilies",
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
            "expandedBorderRadius",
            "topBorderRadius",
            "maxWidth",
            "expandedMaxWidth",
        ][..],
        "lyrics.displays.desktop.appearance" => &[
            "fontFamily",
            "fontFamilies",
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
            "safetyInsetX",
            "safetyInsetY",
            "backgroundMode",
            "background",
            "solidColor",
            "lineGap",
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
        ][..],
        _ => &[][..],
    };
    let mut keys = values.keys().collect::<Vec<_>>();
    keys.sort_by_key(|key| {
        (
            preferred
                .iter()
                .position(|candidate| *candidate == key.as_str())
                .unwrap_or(usize::MAX),
            key.as_str(),
        )
    });
    keys
}
