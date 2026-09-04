use serde_json::Value;

use super::{error_at_key, internal_draft_error, merge_json};
use crate::config::{self, AppConfig, ConfigDraftError, ParsedDraft, CONFIG_SCHEMA_VERSION};
use crate::language::UiLanguage;

pub(super) fn parse_config_draft(raw: &str) -> Result<ParsedDraft, ConfigDraftError> {
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
            app.retain(|key, _| config::APP_CONFIG_KEYS.contains(&key.as_str()));
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
    config::migrate_v32_display_appearances(&mut user, version);
    config::migrate_v34_lyrics_base_appearance(&mut user, version);
    config::migrate_v37_notch_width(&mut user, version);
    config::migrate_v38_notch_line_count(&mut user, version);
    config::migrate_v39_notch_supporting_tracks(&mut user, version);
    config::migrate_v40_notch_colors(&mut user, version);
    config::migrate_v41_fixed_notch_background(&mut user, version);
    config::migrate_v42_list_preferences(&mut user, version);
    config::migrate_v48_notch_mode(&mut user, version);
    config::migrate_v49_notch_width(&mut user, version);
    config::migrate_v50_notch_layout(&mut user, version);
    config::migrate_v54_notch_double_line_settings(&mut user, version);
    config::migrate_v57_chinese_conversion(&mut user, version);
    config::migrate_v59_switch_lyrics_shortcut(&mut user);
    config::migrate_v62_status_bar_secondary_font_weight(&mut user, version);
    config::migrate_v63_simplified_japanese_repair(&mut user, version);
    config::remove_retired_fullscreen_space_preferences(&mut user);
    super::structure::validate_known_fields(&user, raw)?;
    super::fields::validate_field_types_and_options(&user, raw)?;
    config::migrate_status_bar_status_item_fields(&mut user);

    let migrated_layout = config::migrate_legacy_overlay_layout(&mut user, version, raw)?;
    #[cfg(test)]
    let migrated = version < CONFIG_SCHEMA_VERSION || migrated_layout;
    #[cfg(not(test))]
    let _ = migrated_layout;
    user.as_object_mut()
        .expect("checked object")
        .insert("schemaVersion".into(), Value::from(CONFIG_SCHEMA_VERSION));

    super::ranges::validate_numeric_ranges(&user, raw)?;
    let mut merged = serde_json::to_value(AppConfig::default()).map_err(internal_draft_error)?;
    merge_json(&mut merged, user);
    let mut config =
        serde_json::from_value::<AppConfig>(merged).map_err(|error| ConfigDraftError {
            message: format!("配置字段类型或选项无效：{error}"),
            line: 1,
            column: 1,
        })?;
    if version < 5 {
        config::migrate_legacy_provider_order(&mut config.lyrics.providers);
    }
    if version < 14 {
        config::migrate_v13_provider_defaults(&mut config.lyrics.providers);
    }
    if version < 45 {
        config::migrate_v45_provider_sources(&mut config.lyrics.providers);
    }
    if version < 58 {
        config::migrate_v58_enable_all_provider_sources(&mut config.lyrics.providers);
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
        config::canonical_config_jsonc(&config, UiLanguage::ZhCn).map_err(internal_draft_error)?;
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
