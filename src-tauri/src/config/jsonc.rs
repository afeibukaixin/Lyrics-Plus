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
            line if line.starts_with("    \"displays\":") => {
                Some(("    ", ConfigComment::LyricsDisplays))
            }
            line if line.starts_with("    \"visible\":") => {
                Some(("    ", ConfigComment::OverlayState))
            }
            line if line.starts_with("    \"hideWhenNotPlaying\":") => {
                Some(("    ", ConfigComment::HideWhenNotPlaying))
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
            line if line.starts_with("      \"primaryLinePosition\":") => {
                Some(("      ", ConfigComment::PrimaryLinePosition))
            }
            line if line.starts_with("      \"lineGap\":") => {
                Some(("      ", ConfigComment::LineGap))
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
