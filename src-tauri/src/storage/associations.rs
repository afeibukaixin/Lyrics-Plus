pub(crate) enum LyricsLoadResult {
    Missing,
    Ready(LyricsDocument),
    Invalid(String),
}

impl Storage {
    pub(crate) fn load_with_status(&self, track_key: &str) -> Result<LyricsLoadResult, String> {
        if let Some(document) = self.load_v2_lyrics(track_key)? {
            return Ok(LyricsLoadResult::Ready(document));
        }
        // “暂不绑定”创建的独立歌曲没有 V2 歌词绑定；此时不能泄漏旧表中
        // 按平台曲目标识保存的历史歌词，否则拆分后界面仍会显示原歌词。
        if self.has_managed_recording(track_key)? {
            return Ok(LyricsLoadResult::Missing);
        }
        let Some(association) = self.association(track_key)? else {
            return Ok(LyricsLoadResult::Missing);
        };
        let raw = match read_lyric_text(&association.path) {
            Ok(raw) => raw,
            Err(error) => return Ok(LyricsLoadResult::Invalid(error)),
        };
        let mut document =
            match parse_lrc_with_options(&raw, &association.source, association.manual_selected) {
                Ok(document) => document,
                Err(error) => return Ok(LyricsLoadResult::Invalid(error)),
            };
        document.metadata.title = Some(association.title);
        document.metadata.artist = Some(association.artist);
        document.metadata.original_format = association.original_format;
        document.offset_ms = association.offset_ms;
        Ok(LyricsLoadResult::Ready(document))
    }

    pub fn load(&self, track_key: &str) -> Result<Option<LyricsDocument>, String> {
        match self.load_with_status(track_key)? {
            LyricsLoadResult::Missing => Ok(None),
            LyricsLoadResult::Ready(document) => Ok(Some(document)),
            LyricsLoadResult::Invalid(error) => Err(error),
        }
    }

    fn has_managed_recording(&self, track_key: &str) -> Result<bool, String> {
        let observed_track_key = track_key.trim();
        if observed_track_key.is_empty() {
            return Ok(false);
        }
        let canonical_track_key = self.canonical_track_key(observed_track_key)?;
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .query_row(
                "SELECT split_from_recording_id IS NOT NULL OR EXISTS (
                   SELECT 1 FROM platform_lyric_overrides AS offset
                   WHERE offset.recording_id=track_observations.recording_id)
                 FROM track_observations
                 WHERE track_key=?1 OR track_key=?2
                 ORDER BY CASE WHEN track_key=?1 THEN 0 ELSE 1 END,
                          observed_at DESC, observation_id DESC LIMIT 1",
                params![observed_track_key, canonical_track_key],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map(|value| value.unwrap_or(0) != 0)
            .map_err(|error| format!("读取独立歌曲状态失败：{error}"))
    }

    fn association(&self, track_key: &str) -> Result<Option<Association>, String> {
        let canonical_track_key = self.canonical_track_key(track_key)?;
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .query_row(
                "SELECT title, artist, source, content_path, offset_ms, original_format,
                        manual_selected
                 FROM lyric_associations WHERE track_key=?1",
                params![canonical_track_key],
                |row| {
                    Ok(Association {
                        title: row.get(0)?,
                        artist: row.get(1)?,
                        source: row.get(2)?,
                        path: PathBuf::from(row.get::<_, String>(3)?),
                        offset_ms: row.get(4)?,
                        original_format: row.get(5)?,
                        manual_selected: row.get::<_, i64>(6)? != 0,
                    })
                },
            )
            .optional()
            .map_err(|error| format!("读取歌词关联失败：{error}"))
    }

    pub fn set_offset(&self, track_key: &str, offset_ms: i64) -> Result<(), String> {
        let canonical_track_key = self.canonical_track_key(track_key)?;
        let changed = {
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            connection
                .execute(
                    "UPDATE lyric_associations SET offset_ms=?2, updated_at=unixepoch() WHERE track_key=?1",
                    params![canonical_track_key, offset_ms],
                )
                .map_err(|error| format!("保存歌词偏移失败：{error}"))?
        };
        if changed == 0 {
            Err("当前歌曲尚未关联歌词".into())
        } else {
            if let Err(error) = self.set_v2_lyrics_offset(track_key, offset_ms) {
                log::warn!("同步新歌词偏移失败，保留旧结构：{error}");
            }
            Ok(())
        }
    }

    pub fn remove(&self, track_key: &str) -> Result<(), String> {
        let canonical_track_key = self.canonical_track_key(track_key)?;
        {
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            connection
                .execute(
                    "DELETE FROM lyric_associations WHERE track_key=?1",
                    params![canonical_track_key],
                )
                .map_err(|error| format!("解除歌词关联失败：{error}"))?;
        }
        if let Err(error) = self.unbind_v2_lyrics(track_key) {
            log::warn!("同步新歌词解绑失败，保留旧结构：{error}");
        }
        // 解绑只取消生效关系；资源索引和磁盘文件留给未来独立的删除能力处理。
        Ok(())
    }
}
