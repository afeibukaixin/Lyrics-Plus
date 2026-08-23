impl Storage {
    pub fn load(&self, track_key: &str) -> Result<Option<LyricsDocument>, String> {
        let Some(association) = self.association(track_key)? else {
            return Ok(None);
        };
        let raw = fs::read_to_string(&association.path)
            .map_err(|error| format!("读取歌词文件失败：{error}"))?;
        let mut document =
            parse_lrc_with_options(&raw, &association.source, association.manual_selected)?;
        document.metadata.title = Some(association.title);
        document.metadata.artist = Some(association.artist);
        document.metadata.original_format = association.original_format;
        document.offset_ms = association.offset_ms;
        Ok(Some(document))
    }

    fn association(&self, track_key: &str) -> Result<Option<Association>, String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .query_row(
                "SELECT title, artist, source, content_path, offset_ms, original_format, manual_selected
                 FROM lyric_associations WHERE track_key=?1",
                params![track_key],
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

    fn file_is_app_owned(&self, path: &Path) -> Option<bool> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .query_row(
                "SELECT app_owned FROM lyric_files WHERE content_path=?1",
                params![path.to_string_lossy()],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .ok()
            .flatten()
            .map(|value| value != 0)
    }

    pub fn set_offset(&self, track_key: &str, offset_ms: i64) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let changed = connection
            .execute(
                "UPDATE lyric_associations SET offset_ms=?2, updated_at=unixepoch() WHERE track_key=?1",
                params![track_key, offset_ms],
            )
            .map_err(|error| format!("保存歌词偏移失败：{error}"))?;
        if changed == 0 {
            Err("当前歌曲尚未关联歌词".into())
        } else {
            Ok(())
        }
    }

    pub fn remove(&self, track_key: &str) -> Result<(), String> {
        let association = self.association(track_key)?;
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .execute(
                "DELETE FROM lyric_associations WHERE track_key=?1",
                params![track_key],
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
                        "SELECT app_owned FROM lyric_files WHERE content_path=?1",
                        params![path.to_string_lossy()],
                        |row| row.get::<_, i64>(0),
                    )
                    .optional()
                    .ok()
                    .flatten()
            })
            .unwrap_or(1)
            != 0;
        if let Some(path) = path.filter(|_| references == 0 && app_owned) {
            connection
                .execute(
                    "DELETE FROM lyric_files WHERE content_path=?1",
                    params![path.to_string_lossy()],
                )
                .map_err(|error| format!("删除歌词索引失败：{error}"))?;
            drop(connection);
            if path.exists() {
                fs::remove_file(path).map_err(|error| format!("删除歌词文件失败：{error}"))?;
            }
        }
        Ok(())
    }
}
