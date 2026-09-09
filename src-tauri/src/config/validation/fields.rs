use serde_json::Value;

use super::error_at_key;
use crate::config::ConfigDraftError;

pub(super) fn validate_field_types_and_options(
    value: &Value,
    raw: &str,
) -> Result<(), ConfigDraftError> {
    for (pointer, key) in [
        ("/app", "app"),
        ("/app/shortcuts", "shortcuts"),
        ("/lyrics", "lyrics"),
        ("/lyrics/providers", "providers"),
        ("/lyrics/displays", "displays"),
        ("/lyrics/displays/desktop", "desktop"),
        ("/lyrics/displays/desktop/presentation", "presentation"),
        ("/lyrics/displays/desktop/appearance", "appearance"),
        ("/lyrics/displays/statusBar", "statusBar"),
        ("/lyrics/displays/statusBar/presentation", "presentation"),
        ("/lyrics/displays/statusBar/appearance", "appearance"),
        ("/lyrics/displays/listWindow", "listWindow"),
        ("/lyrics/displays/listWindow/appearance", "appearance"),
        ("/lyrics/displays/notch", "notch"),
        ("/lyrics/displays/notch/presentation", "presentation"),
        ("/lyrics/displays/notch/appearance", "appearance"),
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
        ("/lyrics/displays/desktop/enabled", "enabled"),
        ("/lyrics/displays/desktop/locked", "locked"),
        (
            "/lyrics/displays/desktop/hideWhenNotPlaying",
            "hideWhenNotPlaying",
        ),
        (
            "/lyrics/displays/desktop/presentation/showTranslation",
            "showTranslation",
        ),
        (
            "/lyrics/displays/desktop/presentation/showRomanization",
            "showRomanization",
        ),
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
        (
            "/lyrics/displays/statusBar/presentation/showTranslation",
            "showTranslation",
        ),
        (
            "/lyrics/displays/statusBar/presentation/showRomanization",
            "showRomanization",
        ),
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
        (
            "/lyrics/displays/notch/presentation/showTranslation",
            "showTranslation",
        ),
        (
            "/lyrics/displays/notch/presentation/showRomanization",
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
            "/lyrics/displays/desktop/presentation/autoCenterWithTranslationOrRomanization",
            "autoCenterWithTranslationOrRomanization",
        ),
        ("/lyrics/providers/preferCapabilities", "preferCapabilities"),
        ("/lyrics/providers/normalizeChinese", "normalizeChinese"),
        (
            "/lyrics/repairSimplifiedJapanese",
            "repairSimplifiedJapanese",
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
        (
            "/lyrics/providers/autoSearchDebounceMs",
            "autoSearchDebounceMs",
        ),
        (
            "/lyrics/providers/maxCandidatesPerProvider",
            "maxCandidatesPerProvider",
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
        ("/lyrics/displays/desktop/appearance/fontSize", "fontSize"),
        (
            "/lyrics/displays/desktop/appearance/fontWeight",
            "fontWeight",
        ),
        (
            "/lyrics/displays/desktop/appearance/secondaryFontWeight",
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
    validate_list_line_order(value, raw)?;
    validate_string_option(
        value,
        raw,
        "/app/theme",
        "theme",
        &["system", "light", "dark"],
    )?;
    if let Some(candidate) = value.pointer("/app/uiFontFamily") {
        if !candidate.is_null() {
            let ui_font_family = candidate.as_str().ok_or_else(|| {
                error_at_key(raw, "uiFontFamily", "uiFontFamily 必须是字符串或 null")
            })?;
            if ui_font_family.trim().is_empty() {
                return Err(error_at_key(raw, "uiFontFamily", "uiFontFamily 不能为空"));
            }
        }
    }
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
            "/lyrics/displays/statusBar/presentation/layout",
            "layout",
            &["single", "double"] as &[&str],
        ),
        (
            "/lyrics/displays/statusBar/presentation/doubleLineMode",
            "doubleLineMode",
            &["rolling", "alternating"] as &[&str],
        ),
        (
            "/lyrics/displays/statusBar/presentation/supportingPriority",
            "supportingPriority",
            &["translation", "romanization"] as &[&str],
        ),
        (
            "/lyrics/displays/statusBar/presentation/alignment",
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
            "/lyrics/displays/notch/presentation/layout",
            "layout",
            &["single", "double"] as &[&str],
        ),
        (
            "/lyrics/displays/notch/presentation/doubleLineMode",
            "doubleLineMode",
            &["rolling", "alternating"] as &[&str],
        ),
        (
            "/lyrics/displays/notch/presentation/supportingPriority",
            "supportingPriority",
            &["translation", "romanization"] as &[&str],
        ),
        (
            "/lyrics/displays/desktop/appearance/backgroundMode",
            "backgroundMode",
            &["solid", "transparent"] as &[&str],
        ),
        (
            "/lyrics/displays/desktop/appearance/background",
            "background",
            &["glass", "transparent", "solid"] as &[&str],
        ),
        (
            "/lyrics/displays/desktop/presentation/layout",
            "layout",
            &["single", "double"],
        ),
        (
            "/lyrics/displays/desktop/presentation/doubleLineMode",
            "doubleLineMode",
            &["rolling", "alternating"],
        ),
        (
            "/lyrics/displays/desktop/presentation/orientation",
            "orientation",
            &["horizontal", "vertical"],
        ),
        (
            "/lyrics/displays/desktop/presentation/alignment",
            "alignment",
            &["start", "center", "end", "distributed"],
        ),
        (
            "/lyrics/displays/desktop/presentation/primaryLinePosition",
            "primaryLinePosition",
            &["first", "second"],
        ),
        (
            "/lyrics/displays/desktop/presentation/longText",
            "longText",
            &["shrink", "wrap", "marquee"],
        ),
        (
            "/lyrics/displays/desktop/presentation/supportingPriority",
            "supportingPriority",
            &["translation", "romanization"],
        ),
        (
            "/lyrics/displays/desktop/appearance/karaokeStyle",
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
    if let Some(candidate) = value.pointer("/lyrics/displays/desktop/appearance/fontFamily") {
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
    for appearance_path in [
        "/lyrics/displays/desktop/appearance",
        "/lyrics/displays/listWindow/appearance",
    ] {
        for key in [
            "activeColor",
            "inactiveColor",
            "solidColor",
            "translationColor",
            "romanizationColor",
            "textShadowColor",
            "textStrokeColor",
        ] {
            let pointer = format!("{appearance_path}/{key}");
            if let Some(candidate) = value.pointer(&pointer) {
                let color = candidate
                    .as_str()
                    .ok_or_else(|| error_at_key(raw, key, &format!("{key} 必须是颜色字符串")))?;
                if !is_supported_color(color) {
                    return Err(error_at_key(raw, key, &format!("{key} 不是有效颜色")));
                }
            }
        }
    }
    Ok(())
}

fn validate_list_line_order(value: &Value, raw: &str) -> Result<(), ConfigDraftError> {
    let Some(candidate) = value.pointer("/lyrics/displays/listWindow/lineOrder") else {
        return Ok(());
    };
    let items = candidate
        .as_array()
        .ok_or_else(|| error_at_key(raw, "lineOrder", "lineOrder 必须是数组"))?;
    if items.len() != 3 {
        return Err(error_at_key(
            raw,
            "lineOrder",
            "lineOrder 必须恰好包含 original、translation、romanization 各一次",
        ));
    }
    let mut seen = [false; 3];
    for item in items {
        let kind = item
            .as_str()
            .ok_or_else(|| error_at_key(raw, "lineOrder", "lineOrder 中的项目必须是字符串"))?;
        let index = match kind {
            "original" => 0,
            "translation" => 1,
            "romanization" => 2,
            _ => {
                return Err(error_at_key(
                    raw,
                    "lineOrder",
                    "lineOrder 仅支持 original、translation、romanization",
                ));
            }
        };
        if seen[index] {
            return Err(error_at_key(raw, "lineOrder", "lineOrder 不能包含重复项目"));
        }
        seen[index] = true;
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

pub(in crate::config) fn is_valid_language_preference(candidate: &str) -> bool {
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

pub(in crate::config) fn is_supported_color(value: &str) -> bool {
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
