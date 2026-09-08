use super::models::EffectiveLyricBinding;
use crate::storage::EXTERNAL_ROOT_ID;
use rusqlite::OptionalExtension;
use std::{fs, path::PathBuf};
use strsim::normalized_levenshtein;

pub(super) fn load_default_lyric_binding(
    connection: &rusqlite::Connection,
    recording_id: i64,
) -> Result<Option<EffectiveLyricBinding>, String> {
    connection
        .query_row(
            "SELECT binding.asset_id, asset.source_name, binding.selection_source,
                    binding.confidence, binding.evidence_json, binding.offset_ms
             FROM recording_lyric_bindings AS binding
             JOIN lyric_assets AS asset ON asset.asset_id=binding.asset_id
             WHERE binding.recording_id=?1 AND binding.is_default=1 AND asset.available=1
             LIMIT 1",
            rusqlite::params![recording_id],
            |row| {
                Ok(EffectiveLyricBinding {
                    asset_id: row.get(0)?,
                    selection_source: row.get(2)?,
                    confidence: row.get(3)?,
                    evidence_json: row.get(4)?,
                    offset_ms: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(|error| format!("读取歌曲默认歌词失败：{error}"))
}

pub(super) fn lyric_content_similarity(
    connection: &rusqlite::Connection,
    left_asset_id: i64,
    right_asset_id: i64,
) -> Option<(f64, bool)> {
    let left = load_lyric_asset_content(connection, left_asset_id)?;
    let right = load_lyric_asset_content(connection, right_asset_id)?;
    if !left.0.is_empty() && left.0 == right.0 {
        return Some((1.0, true));
    }
    let left = normalize_lyric_content(&left.1);
    let right = normalize_lyric_content(&right.1);
    if left.is_empty() || right.is_empty() {
        None
    } else {
        Some((normalized_levenshtein(&left, &right), false))
    }
}

pub(super) fn load_lyric_asset_content(
    connection: &rusqlite::Connection,
    asset_id: i64,
) -> Option<(String, String)> {
    let (fingerprint, root_id, relative_path, root_path) = connection
        .query_row(
            "SELECT asset.content_fingerprint, asset.root_dir_id, asset.relative_path,
                    root.path
             FROM lyric_assets AS asset
             LEFT JOIN library_roots AS root ON root.root_id=asset.root_dir_id
             WHERE asset.asset_id=?1 AND asset.available=1",
            rusqlite::params![asset_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()
        .ok()
        .flatten()?;
    let relative_path = relative_path?;
    let path = if root_id.as_deref() == Some(EXTERNAL_ROOT_ID) {
        PathBuf::from(relative_path)
    } else {
        let root_path = PathBuf::from(root_path?);
        let relative = std::path::Path::new(&relative_path);
        if relative.is_absolute() {
            return None;
        }
        let path = root_path.join(relative);
        if !path.starts_with(&root_path) {
            return None;
        }
        path
    };
    let raw = fs::read_to_string(path).ok()?;
    Some((fingerprint, raw))
}

pub(super) fn normalize_lyric_content(raw: &str) -> String {
    // 优先使用项目解析器提取正文，逐字时间轴、TTML 标签和格式元数据不参与比较。
    if let Ok(document) = crate::lyrics::parse_lrc_with_options(raw, "identity", false) {
        let text = document
            .tracks
            .original
            .lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("");
        if !text.is_empty() {
            return text
                .chars()
                .flat_map(char::to_lowercase)
                .filter(|character| character.is_alphanumeric())
                .collect();
        }
    }
    let mut normalized = String::new();
    for line in raw.lines() {
        let mut text = line.trim();
        loop {
            let Some(first) = text.chars().next() else {
                break;
            };
            let closing = match first {
                '[' => Some(']'),
                '<' => Some('>'),
                '{' => Some('}'),
                _ => None,
            };
            let Some(closing) = closing else {
                break;
            };
            let rest = &text[first.len_utf8()..];
            let Some(end) = rest.find(closing) else {
                break;
            };
            text = &rest[end + closing.len_utf8()..];
        }
        for character in text.chars().flat_map(|character| character.to_lowercase()) {
            if character.is_alphanumeric() {
                normalized.push(character);
            }
        }
    }
    normalized
}

pub(super) fn merge_lyric_bindings(
    transaction: &rusqlite::Transaction<'_>,
    target_recording_id: i64,
    source_recording_id: i64,
) -> Result<(), String> {
    let bindings = {
        let mut statement = transaction
            .prepare(
                "SELECT asset_id, selection_source, confidence, evidence_json, offset_ms
                 FROM recording_lyric_bindings WHERE recording_id=?1",
            )
            .map_err(|error| format!("读取候选歌词绑定失败：{error}"))?;
        let bindings = statement
            .query_map(rusqlite::params![source_recording_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })
            .map_err(|error| format!("读取候选歌词绑定失败：{error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("解析候选歌词绑定失败：{error}"))?;
        bindings
    };
    for (asset_id, selection_source, confidence, evidence_json, offset_ms) in bindings {
        transaction
            .execute(
                "INSERT INTO recording_lyric_bindings
                   (recording_id, asset_id, selection_source, confidence, evidence_json,
                    is_default, offset_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)
                 ON CONFLICT(recording_id, asset_id) DO NOTHING",
                rusqlite::params![
                    target_recording_id,
                    asset_id,
                    selection_source,
                    confidence,
                    evidence_json,
                    offset_ms
                ],
            )
            .map_err(|error| format!("迁移候选歌词绑定失败：{error}"))?;
    }
    Ok(())
}

pub(super) fn preserve_override_bindings(
    transaction: &rusqlite::Transaction<'_>,
    recording_id: i64,
) -> Result<(), String> {
    transaction
        .execute(
            "INSERT INTO recording_lyric_bindings
               (recording_id, asset_id, selection_source, confidence, evidence_json,
                is_default, offset_ms)
             SELECT override.recording_id, override.asset_id, 'user', 100, '{}', 0,
                    override.offset_ms
             FROM platform_lyric_overrides AS override
             WHERE override.recording_id=?1 AND override.asset_id IS NOT NULL
             ON CONFLICT(recording_id, asset_id) DO NOTHING",
            rusqlite::params![recording_id],
        )
        .map_err(|error| format!("保留平台歌词资源绑定失败：{error}"))?;
    Ok(())
}

/// 操作歌曲前保存旧平台资源，并将覆盖规范化为仅含时间偏移。
pub(crate) fn normalize_platform_lyrics(
    transaction: &rusqlite::Transaction<'_>,
    recording_id: i64,
) -> Result<(), String> {
    preserve_override_bindings(transaction, recording_id)?;
    // 把旧默认偏移显式保存到每个平台，避免更换共用歌词时丢失。
    transaction
        .execute(
            "INSERT INTO platform_lyric_overrides (recording_id, platform, asset_id, offset_ms)
         SELECT observation.recording_id, observation.platform, NULL,
                COALESCE((SELECT offset_ms FROM recording_lyric_bindings
                          WHERE recording_id=?1 AND is_default=1), 0)
         FROM track_observations AS observation WHERE observation.recording_id=?1
         ON CONFLICT(recording_id, platform) DO NOTHING",
            rusqlite::params![recording_id],
        )
        .map_err(|error| format!("保留各平台偏移失败：{error}"))?;
    transaction
        .execute(
            "UPDATE platform_lyric_overrides SET asset_id=NULL, updated_at=unixepoch()
         WHERE recording_id=?1",
            rusqlite::params![recording_id],
        )
        .map_err(|error| format!("规范化平台歌词覆盖失败：{error}"))?;
    Ok(())
}

pub(super) fn move_platform_overrides(
    transaction: &rusqlite::Transaction<'_>,
    source_recording_id: i64,
    target_recording_id: i64,
) -> Result<(), String> {
    let offsets = {
        let mut statement = transaction
            .prepare(
                "SELECT platform, asset_id, offset_ms
                 FROM platform_lyric_overrides WHERE recording_id=?1",
            )
            .map_err(|error| format!("读取平台歌词偏移失败：{error}"))?;
        let offsets = statement
            .query_map(rusqlite::params![source_recording_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(|error| format!("读取平台歌词偏移失败：{error}"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| format!("解析平台歌词偏移失败：{error}"))?;
        offsets
    };
    for (platform, asset_id, offset_ms) in offsets {
        transaction
            .execute(
                "INSERT INTO platform_lyric_overrides
                   (recording_id, platform, asset_id, offset_ms)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(recording_id, platform) DO NOTHING",
                rusqlite::params![target_recording_id, platform, asset_id, offset_ms],
            )
            .map_err(|error| format!("迁移平台歌词偏移失败：{error}"))?;
    }
    transaction
        .execute(
            "DELETE FROM platform_lyric_overrides WHERE recording_id=?1",
            rusqlite::params![source_recording_id],
        )
        .map_err(|error| format!("清理候选平台歌词偏移失败：{error}"))?;
    Ok(())
}

/// 调用方已规范化资源覆盖，这里只移动平台偏移。
pub(super) fn move_platform_override(
    transaction: &rusqlite::Transaction<'_>,
    source_recording_id: i64,
    target_recording_id: i64,
    platform: &str,
) -> Result<(), String> {
    let offset_ms = transaction
        .query_row(
            "SELECT offset_ms FROM platform_lyric_overrides WHERE recording_id=?1 AND platform=?2",
            rusqlite::params![source_recording_id, platform],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| format!("读取平台偏移失败：{error}"))?
        .unwrap_or_default();
    transaction
        .execute(
            "INSERT INTO platform_lyric_overrides (recording_id, platform, asset_id, offset_ms)
         VALUES (?1, ?2, NULL, ?3)",
            rusqlite::params![target_recording_id, platform, offset_ms],
        )
        .map_err(|error| format!("保留移出曲目偏移失败：{error}"))?;
    // 旧库中同平台重复观察共享一个偏移；原处还有观察时保留它。
    transaction
        .execute(
            "DELETE FROM platform_lyric_overrides WHERE recording_id=?1 AND platform=?2
         AND NOT EXISTS (SELECT 1 FROM track_observations WHERE recording_id=?1 AND platform=?2)",
            rusqlite::params![source_recording_id, platform],
        )
        .map_err(|error| format!("清理原平台偏移失败：{error}"))?;
    Ok(())
}

pub(super) fn set_shared_lyrics_default(
    transaction: &rusqlite::Transaction<'_>,
    recording_id: i64,
    binding: Option<&EffectiveLyricBinding>,
) -> Result<(), String> {
    transaction
        .execute(
            "UPDATE recording_lyric_bindings SET is_default=0, updated_at=unixepoch()
             WHERE recording_id=?1",
            rusqlite::params![recording_id],
        )
        .map_err(|error| format!("整理默认歌词绑定失败：{error}"))?;
    if let Some(binding) = binding {
        transaction
            .execute(
                "INSERT INTO recording_lyric_bindings
                   (recording_id, asset_id, selection_source, confidence, evidence_json,
                    is_default, offset_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)
                 ON CONFLICT(recording_id, asset_id) DO UPDATE SET
                   selection_source=excluded.selection_source,
                   confidence=excluded.confidence,
                   evidence_json=excluded.evidence_json,
                   is_default=1,
                   offset_ms=excluded.offset_ms,
                   updated_at=unixepoch()",
                rusqlite::params![
                    recording_id,
                    binding.asset_id,
                    binding.selection_source,
                    binding.confidence,
                    binding.evidence_json,
                    binding.offset_ms
                ],
            )
            .map_err(|error| format!("设置共享默认歌词失败：{error}"))?;
    }
    Ok(())
}

pub(super) fn load_current_lyric_binding(
    connection: &rusqlite::Connection,
    recording_id: i64,
    platform: &str,
) -> Result<(i64, String, i64, String, i64), String> {
    let mut binding = load_default_lyric_binding(connection, recording_id)?
        .ok_or_else(|| "当前没有可沿用的有效歌词".to_string())?;
    binding.offset_ms = connection
        .query_row(
            "SELECT offset_ms FROM platform_lyric_overrides WHERE recording_id=?1 AND platform=?2",
            rusqlite::params![recording_id, platform],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| format!("读取平台偏移失败：{error}"))?
        .unwrap_or(binding.offset_ms);
    Ok((
        binding.asset_id,
        binding.selection_source,
        binding.confidence,
        binding.evidence_json,
        binding.offset_ms,
    ))
}
