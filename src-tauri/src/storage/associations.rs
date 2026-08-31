pub(crate) enum LyricsLoadResult {
    Missing,
    Ready(LyricsDocument),
    Invalid(String),
}

impl Storage {
    pub(crate) fn load_with_status(&self, track_key: &str) -> Result<LyricsLoadResult, String> {
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

    pub(crate) fn automatic_provider_item_id(
        &self,
        track_key: &str,
        provider_id: &str,
    ) -> Result<Option<String>, String> {
        let Some(association) = self.association(track_key)? else {
            return Ok(None);
        };
        if association.manual_selected || association.provider_id.as_deref() != Some(provider_id) {
            return Ok(None);
        }
        Ok(association
            .provider_item_id
            .filter(|item_id| !item_id.trim().is_empty()))
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
                        manual_selected, provider_id, provider_item_id
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
                        provider_id: row.get(7)?,
                        provider_item_id: row.get(8)?,
                    })
                },
            )
            .optional()
            .map_err(|error| format!("读取歌词关联失败：{error}"))
    }

    fn file_is_app_owned(&self, path: &Path) -> Option<bool> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .query_row(
                "SELECT app_owned, source FROM lyric_files WHERE content_path=?1",
                params![path.to_string_lossy()],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .ok()
            .flatten()
            .map(|(app_owned, source)| app_owned != 0 && !is_user_owned_source(&source))
    }

    pub fn set_offset(&self, track_key: &str, offset_ms: i64) -> Result<(), String> {
        let canonical_track_key = self.canonical_track_key(track_key)?;
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let changed = connection
            .execute(
                "UPDATE lyric_associations SET offset_ms=?2, updated_at=unixepoch() WHERE track_key=?1",
                params![canonical_track_key, offset_ms],
            )
            .map_err(|error| format!("保存歌词偏移失败：{error}"))?;
        if changed == 0 {
            Err("当前歌曲尚未关联歌词".into())
        } else {
            Ok(())
        }
    }

    pub fn remove(&self, track_key: &str) -> Result<(), String> {
        let canonical_track_key = self.canonical_track_key(track_key)?;
        let association = self.association(&canonical_track_key)?;
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
        let path = association.map(|association| association.path);
        let references = path.as_ref().map_or(0, |path| {
            connection
                .query_row(
                    "SELECT COUNT(*) FROM lyric_associations WHERE content_path=?1",
                    params![path.to_string_lossy()],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap_or(1)
        });
        let app_owned = path
            .as_ref()
            .and_then(|path| {
                connection
                    .query_row(
                        "SELECT app_owned, source FROM lyric_files WHERE content_path=?1",
                        params![path.to_string_lossy()],
                        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()
                    .ok()
                    .flatten()
            })
            .is_some_and(|(app_owned, source)| app_owned != 0 && !is_user_owned_source(&source));
        drop(connection);
        if let Some(path) = path.filter(|_| references == 0 && app_owned) {
            self.cleanup_unreferenced_app_owned_files([path])?;
        }
        Ok(())
    }
}
