use serde_json::Value;

use super::{
    AppConfig, ConfigDraftError, ConfigDraftValidation, OverlayStyleSettings, ParsedDraft,
};

mod draft;
mod fields;
mod ranges;
mod structure;

pub(super) use fields::{is_supported_color, is_valid_language_preference};

pub(crate) fn validate_config_draft(raw: &str) -> ConfigDraftValidation {
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

pub(crate) fn parse_config_draft(raw: &str) -> Result<ParsedDraft, ConfigDraftError> {
    draft::parse_config_draft(raw)
}

pub(super) fn user_overrides_from_config(config: &AppConfig) -> Value {
    let mut value = serde_json::to_value(config).expect("应用配置必须可以序列化");
    let defaults = serde_json::to_value(AppConfig::default()).expect("默认配置必须可以序列化");
    prune_default_values(&mut value, &defaults, true);
    strip_inherited_overrides(&mut value, config);
    value
}

pub(super) fn prune_default_overrides(value: &mut Value) {
    let defaults = serde_json::to_value(AppConfig::default()).expect("默认配置必须可以序列化");
    prune_default_values(value, &defaults, true);
    remove_empty_objects(value, true);
}

pub(super) fn config_from_user_overrides(user: &Value) -> Result<AppConfig, String> {
    let mut merged = serde_json::to_value(AppConfig::default())
        .map_err(|error| format!("序列化默认配置失败：{error}"))?;
    merge_json(&mut merged, user.clone());
    serde_json::from_value::<AppConfig>(merged)
        .map_err(|error| format!("配置字段类型或选项无效：{error}"))?
        .normalized()
}

pub(super) fn sync_user_overrides(user: &mut Value, before: &AppConfig, after: &AppConfig) {
    let previous_user = user.clone();
    let before_config = before.clone();
    let after_config = after.clone();
    let before = serde_json::to_value(before).expect("应用配置必须可以序列化");
    let after = serde_json::to_value(after).expect("应用配置必须可以序列化");
    if !user.is_object() {
        *user = Value::Object(Default::default());
    }
    sync_override_value(user, &before, &after);
    restore_inherited_overrides(user, &previous_user, &before_config, &after_config, after);
    user.as_object_mut()
        .expect("用户配置根节点必须是对象")
        .insert(
            "schemaVersion".into(),
            Value::from(crate::config::CONFIG_SCHEMA_VERSION),
        );
}

pub(super) fn reset_user_overrides(user: &mut Value, pointers: &[&str]) {
    for pointer in pointers {
        remove_override_pointer(user, pointer);
    }
    remove_empty_objects(user, true);
    if !user.is_object() {
        *user = Value::Object(Default::default());
    }
    user.as_object_mut()
        .expect("用户配置根节点必须是对象")
        .insert(
            "schemaVersion".into(),
            Value::from(crate::config::CONFIG_SCHEMA_VERSION),
        );
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

fn prune_default_values(value: &mut Value, default: &Value, root: bool) {
    let (Value::Object(value), Value::Object(default)) = (value, default) else {
        return;
    };
    let keys = value.keys().cloned().collect::<Vec<_>>();
    for key in keys {
        if root && key == "schemaVersion" {
            continue;
        }
        let remove = match (value.get_mut(&key), default.get(&key)) {
            (Some(candidate), Some(default_value)) if candidate == default_value => true,
            (Some(candidate), Some(default_value)) => {
                prune_default_values(candidate, default_value, false);
                matches!(candidate, Value::Object(object) if object.is_empty())
            }
            _ => false,
        };
        if remove {
            value.remove(&key);
        }
    }
}

fn sync_override_value(overrides: &mut Value, before: &Value, after: &Value) {
    if before == after {
        return;
    }
    let (Value::Object(before), Value::Object(after)) = (before, after) else {
        *overrides = after.clone();
        return;
    };
    let Some(overrides_map) = overrides.as_object_mut() else {
        *overrides = Value::Object(after.clone());
        return;
    };
    for (key, after_value) in after {
        if key == "schemaVersion" {
            continue;
        }
        let Some(before_value) = before.get(key) else {
            overrides_map.insert(key.clone(), after_value.clone());
            continue;
        };
        if before_value == after_value {
            continue;
        }
        if before_value.is_object() && after_value.is_object() {
            let child = overrides_map
                .entry(key.clone())
                .or_insert_with(|| Value::Object(Default::default()));
            sync_override_value(child, before_value, after_value);
        } else {
            overrides_map.insert(key.clone(), after_value.clone());
        }
    }
}

fn remove_override_pointer(value: &mut Value, pointer: &str) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let mut segments = pointer
        .strip_prefix('/')
        .unwrap_or(pointer)
        .split('/')
        .map(|segment| segment.replace("~1", "/").replace("~0", "~"))
        .collect::<Vec<_>>();
    let Some(last) = segments.pop() else {
        return;
    };
    let mut current = object;
    for segment in segments {
        let Some(next) = current.get_mut(&segment).and_then(Value::as_object_mut) else {
            return;
        };
        current = next;
    }
    current.remove(&last);
}

fn set_override_pointer(value: &mut Value, pointer: &str, replacement: Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let mut segments = pointer
        .strip_prefix('/')
        .unwrap_or(pointer)
        .split('/')
        .map(|segment| segment.replace("~1", "/").replace("~0", "~"))
        .collect::<Vec<_>>();
    let Some(last) = segments.pop() else {
        return;
    };
    let mut current = object;
    for segment in segments {
        let entry = current
            .entry(segment)
            .or_insert_with(|| Value::Object(Default::default()));
        if !entry.is_object() {
            *entry = Value::Object(Default::default());
        }
        current = entry.as_object_mut().expect("用户配置中间节点必须是对象");
    }
    current.insert(last, replacement);
}

fn strip_inherited_overrides(value: &mut Value, config: &AppConfig) {
    let pointers = inherited_override_pointers(config);
    reset_user_overrides_without_version(value, &pointers);
}

fn restore_inherited_overrides(
    user: &mut Value,
    previous_user: &Value,
    before: &AppConfig,
    after: &AppConfig,
    after_value: Value,
) {
    let before_pointers = inherited_override_pointers(before);
    let after_pointers = inherited_override_pointers(after);
    for pointer in &after_pointers {
        if let Some(previous) = previous_user.pointer(pointer).cloned() {
            set_override_pointer(user, pointer, previous);
        } else {
            remove_override_pointer(user, pointer);
        }
    }
    // When inheritance is turned off, materialize the currently displayed
    // value once so disabling inheritance does not unexpectedly fall back to
    // a different mode default. Existing explicit mode overrides are kept.
    for pointer in before_pointers {
        if after_pointers.contains(&pointer) || previous_user.pointer(pointer).is_some() {
            continue;
        }
        if let Some(current) = after_value.pointer(pointer).cloned() {
            set_override_pointer(user, pointer, current);
        }
    }
    remove_empty_objects(user, true);
}

fn inherited_override_pointers(config: &AppConfig) -> Vec<&'static str> {
    let inheritance = &config.lyrics.style_inheritance;
    let mut pointers = Vec::new();
    if inheritance.desktop.inherit_font_family {
        pointers.push("/lyrics/displays/desktop/appearance/fontFamily");
    }
    if inheritance.desktop.inherit_colors {
        pointers.extend([
            "/lyrics/displays/desktop/appearance/activeColor",
            "/lyrics/displays/desktop/appearance/inactiveColor",
            "/lyrics/displays/desktop/appearance/translationColor",
            "/lyrics/displays/desktop/appearance/romanizationColor",
            "/lyrics/displays/desktop/appearance/solidColor",
        ]);
    }
    if inheritance.status_bar.inherit_font_family {
        pointers.push("/lyrics/displays/statusBar/appearance/fontFamily");
    }
    if inheritance.status_bar.inherit_colors {
        pointers.extend([
            "/lyrics/displays/statusBar/appearance/textColor",
            "/lyrics/displays/statusBar/appearance/inactiveColor",
            "/lyrics/displays/statusBar/appearance/highlightColor",
            "/lyrics/displays/statusBar/appearance/translationColor",
            "/lyrics/displays/statusBar/appearance/romanizationColor",
        ]);
    }
    if inheritance.list_window.inherit_font_family {
        pointers.push("/lyrics/displays/listWindow/appearance/fontFamily");
    }
    if inheritance.list_window.inherit_colors {
        pointers.extend([
            "/lyrics/displays/listWindow/appearance/activeColor",
            "/lyrics/displays/listWindow/appearance/inactiveColor",
            "/lyrics/displays/listWindow/appearance/translationColor",
            "/lyrics/displays/listWindow/appearance/romanizationColor",
            "/lyrics/displays/listWindow/appearance/backgroundColor",
        ]);
    }
    if inheritance.notch.inherit_font_family {
        pointers.push("/lyrics/displays/notch/appearance/fontFamily");
    }
    if inheritance.notch.inherit_colors {
        pointers.extend([
            "/lyrics/displays/notch/appearance/activeColor",
            "/lyrics/displays/notch/appearance/inactiveColor",
            "/lyrics/displays/notch/appearance/translationColor",
            "/lyrics/displays/notch/appearance/romanizationColor",
        ]);
    }
    pointers
}

fn reset_user_overrides_without_version(value: &mut Value, pointers: &[&str]) {
    for pointer in pointers {
        remove_override_pointer(value, pointer);
    }
    remove_empty_objects(value, true);
}

fn remove_empty_objects(value: &mut Value, root: bool) -> bool {
    let Value::Object(object) = value else {
        return false;
    };
    let keys = object.keys().cloned().collect::<Vec<_>>();
    for key in keys {
        if root && key == "schemaVersion" {
            continue;
        }
        let remove = object
            .get_mut(&key)
            .is_some_and(|candidate| remove_empty_objects(candidate, false));
        if remove {
            object.remove(&key);
        }
    }
    !root && object.is_empty()
}

pub(super) fn error_at_key(raw: &str, key: &str, message: &str) -> ConfigDraftError {
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

pub(super) fn internal_draft_error(error: impl std::fmt::Display) -> ConfigDraftError {
    ConfigDraftError {
        message: format!("处理配置失败：{error}"),
        line: 1,
        column: 1,
    }
}

pub(super) fn color_fields(style: &OverlayStyleSettings) -> [(&'static str, &str); 7] {
    [
        ("高亮颜色", &style.active_color),
        ("未唱颜色", &style.inactive_color),
        ("背景颜色", &style.solid_color),
        ("翻译颜色", &style.translation_color),
        ("音译颜色", &style.romanization_color),
        ("文字阴影颜色", &style.text_shadow_color),
        ("文字描边颜色", &style.text_stroke_color),
    ]
}

pub(super) fn normalize_display_font_weight(value: u16) -> u16 {
    [400_u16, 500, 600, 700, 800]
        .into_iter()
        .min_by_key(|candidate| (*candidate).abs_diff(value))
        .unwrap_or(600)
}
