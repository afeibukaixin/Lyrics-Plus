use serde_json::Value;

use super::error_at_key;
use crate::config::{ConfigDraftError, APP_CONFIG_KEYS};

pub(super) fn validate_known_fields(value: &Value, raw: &str) -> Result<(), ConfigDraftError> {
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
                "repairSimplifiedJapanese",
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
                    "autoApplyDurationGuardEnabled",
                    "autoApplyDurationToleranceSeconds",
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
