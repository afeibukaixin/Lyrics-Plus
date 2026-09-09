use std::collections::HashSet;

use rusqlite::{params, OptionalExtension, TransactionBehavior};

use super::super::Storage;
use super::models::{ClearCandidateLyricsResult, LibraryLyricBatchFailure};

impl Storage {
    pub fn bind_library_lyric(
        &self,
        recording_id: i64,
        asset_id: i64,
        replace_default: bool,
    ) -> Result<(), String> {
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let transaction = connection
            .transaction()
            .map_err(|error| format!("开始绑定歌词失败：{error}"))?;
        let available = transaction
            .query_row(
                "SELECT available FROM lyric_assets WHERE asset_id=?1",
                params![asset_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| format!("读取歌词资源失败：{error}"))?
            .ok_or_else(|| "歌词资源不存在".to_string())?
            != 0;
        if !available {
            return Err("歌词资源当前不可用".into());
        }
        let recording_exists = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM recordings WHERE recording_id=?1)",
                params![recording_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|error| format!("读取歌曲失败：{error}"))?;
        if !recording_exists {
            return Err("歌曲不存在".into());
        }
        let has_default = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM recording_lyric_bindings WHERE recording_id=?1 AND is_default=1)",
            params![recording_id], |row| row.get::<_, bool>(0),
        ).map_err(|error| format!("读取歌曲默认歌词失败：{error}"))?;
        let is_default = replace_default || !has_default;
        if is_default {
            transaction.execute(
                "UPDATE recording_lyric_bindings SET is_default=0, updated_at=unixepoch() WHERE recording_id=?1",
                params![recording_id],
            ).map_err(|error| format!("整理歌曲默认歌词失败：{error}"))?;
        }
        transaction.execute(
            "INSERT INTO recording_lyric_bindings
               (recording_id, asset_id, selection_source, confidence, evidence_json, is_default, offset_ms)
             VALUES (?1, ?2, 'user', 100, '{}', ?3, 0)
             ON CONFLICT(recording_id, asset_id) DO UPDATE SET
               selection_source='user', confidence=100, is_default=?3, updated_at=unixepoch()",
            params![recording_id, asset_id, is_default],
        ).map_err(|error| format!("绑定歌词失败：{error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("提交歌词绑定失败：{error}"))
    }

    pub fn unbind_library_lyric(&self, recording_id: i64, asset_id: i64) -> Result<(), String> {
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let transaction = connection
            .transaction()
            .map_err(|error| format!("开始解除歌词绑定失败：{error}"))?;
        transaction
            .execute(
                "DELETE FROM platform_lyric_overrides WHERE recording_id=?1 AND asset_id=?2",
                params![recording_id, asset_id],
            )
            .map_err(|error| format!("解除平台歌词覆盖失败：{error}"))?;
        transaction
            .execute(
                "DELETE FROM recording_lyric_bindings WHERE recording_id=?1 AND asset_id=?2",
                params![recording_id, asset_id],
            )
            .map_err(|error| format!("解除歌曲歌词绑定失败：{error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("提交歌词解绑失败：{error}"))
    }

    pub fn clear_library_lyric_candidates(
        &self,
        asset_ids: &[i64],
    ) -> Result<ClearCandidateLyricsResult, String> {
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("开始批量解除候选歌词失败：{error}"))?;
        let mut processed_assets = 0_u64;
        let mut removed_bindings = 0_u64;
        let mut failures = Vec::new();
        let mut seen = HashSet::new();

        for asset_id in asset_ids.iter().copied().filter(|asset_id| *asset_id > 0) {
            if !seen.insert(asset_id) {
                continue;
            }
            let (binding_count, default_count) = transaction
                .query_row(
                    "SELECT COUNT(*), COALESCE(SUM(is_default), 0)
                     FROM recording_lyric_bindings WHERE asset_id=?1",
                    params![asset_id],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
                )
                .map_err(|error| format!("读取候选歌词绑定失败：{error}"))?;
            let override_count = transaction
                .query_row(
                    "SELECT COUNT(*) FROM platform_lyric_overrides WHERE asset_id=?1",
                    params![asset_id],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(|error| format!("读取候选歌词平台使用状态失败：{error}"))?;
            if binding_count <= 0 || default_count > 0 || override_count > 0 {
                failures.push(LibraryLyricBatchFailure {
                    asset_id,
                    error: "歌词已不再是仅候选状态，未解除绑定".into(),
                });
                continue;
            }
            let removed = transaction
                .execute(
                    "DELETE FROM recording_lyric_bindings
                     WHERE asset_id=?1 AND is_default=0",
                    params![asset_id],
                )
                .map_err(|error| format!("解除候选歌词绑定失败：{error}"))?;
            processed_assets += 1;
            removed_bindings += removed as u64;
        }

        transaction
            .commit()
            .map_err(|error| format!("提交批量候选歌词解绑失败：{error}"))?;
        Ok(ClearCandidateLyricsResult {
            processed_assets,
            removed_bindings,
            failures,
        })
    }
}
