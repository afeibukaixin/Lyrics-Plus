#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn test_dirs(name: &str) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "lyrics-plus-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        (root.join("app"), root.join("Music").join("Lyrics Plus"))
    }

    #[test]
    fn preferences_and_lyrics_survive_reopen() {
        let (app_dir, library_dir) = test_dirs("storage");
        {
            let storage =
                Storage::open(app_dir.clone(), library_dir.clone()).expect("open storage");
            storage
                .set_preference("player.selection", "spotify")
                .expect("save preference");
            storage
                .set_preference(
                    crate::LEGAL_NOTICE_PREFERENCE,
                    &crate::LEGAL_NOTICE_VERSION.to_string(),
                )
                .expect("save legal notice version");
            storage
                .save(SaveRequest {
                    track_key: "spotify:test",
                    title: "测试歌曲",
                    artist: "测试歌手",
                    source: "test",
                    raw: "[00:01.00]第一行\n[00:02.00]第二行",
                    provider_id: Some("test"),
                    provider_item_id: Some("version-1"),
                    kind: SaveKind::ManualSelection,
                })
                .expect("save lyrics");
            storage
                .set_offset("spotify:test", 300)
                .expect("save offset");
        }

        let reopened = Storage::open(app_dir.clone(), library_dir.clone()).expect("reopen storage");
        assert_eq!(
            reopened
                .get_preference("player.selection")
                .expect("load preference")
                .as_deref(),
            Some("spotify")
        );
        assert!(crate::legal_notice_accepted(&reopened).expect("load legal notice version"));
        reopened
            .set_preference(crate::LEGAL_NOTICE_PREFERENCE, "0")
            .expect("save stale legal notice version");
        assert!(!crate::legal_notice_accepted(&reopened).expect("reject stale notice version"));
        let lyrics = reopened
            .load("spotify:test")
            .expect("load lyrics")
            .expect("lyrics exist");
        assert_eq!(lyrics.tracks.original.lines.len(), 2);
        assert_eq!(lyrics.offset_ms, 300);
        assert!(lyrics.metadata.manual_selected);
        assert!(library_dir.join("测试歌手 - 测试歌曲.lrc").exists());
        let selected_version: (String, String) = reopened
            .connection
            .lock()
            .expect("lock database")
            .query_row(
                "SELECT provider_id, provider_item_id FROM lyric_associations WHERE track_key='spotify:test'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("load provider version");
        assert_eq!(selected_version, ("test".into(), "version-1".into()));
        let _ = fs::remove_dir_all(app_dir.parent().unwrap());
    }

    #[test]
    fn migrates_legacy_provider_names_across_stored_metadata() {
        let (app_dir, library_dir) = test_dirs("provider-names");
        let legacy_names = [
            ("netease", "网易云音乐", NETEASE_DISPLAY_NAME),
            ("qqmusic-spaced", "QQ 音乐", QQMUSIC_DISPLAY_NAME),
            ("qqmusic", "QQ音乐", QQMUSIC_DISPLAY_NAME),
            ("kugou", "酷狗音乐", KUGOU_DISPLAY_NAME),
        ];

        {
            let storage = Storage::open(app_dir.clone(), library_dir.clone()).unwrap();
            for (track_key, legacy_name, _) in legacy_names {
                storage
                    .save(SaveRequest {
                        track_key,
                        title: track_key,
                        artist: "Artist",
                        source: legacy_name,
                        raw: "[00:01]Legacy source",
                        provider_id: Some(track_key),
                        provider_item_id: Some("1"),
                        kind: SaveKind::ManualSelection,
                    })
                    .unwrap();
            }
        }

        let reopened = Storage::open(app_dir.clone(), library_dir.clone()).unwrap();
        for (track_key, _, expected_name) in legacy_names {
            let document = reopened.load(track_key).unwrap().unwrap();
            assert_eq!(document.metadata.source, expected_name);
        }

        let connection = reopened.connection.lock().unwrap();
        let legacy_history_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM lyric_history
                 WHERE source IN ('网易云音乐', 'QQ 音乐', 'QQ音乐', '酷狗音乐')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(legacy_history_count, 0);
        drop(connection);

        let _ = fs::remove_dir_all(app_dir.parent().unwrap());
    }

    #[test]
    fn automatic_search_does_not_overwrite_manual_lyrics() {
        let (app_dir, library_dir) = test_dirs("priority");
        let storage = Storage::open(app_dir.clone(), library_dir).expect("open storage");
        storage
            .save(SaveRequest {
                track_key: "track",
                title: "Song",
                artist: "Artist",
                source: "本地导入",
                raw: "[00:01]Manual",
                provider_id: None,
                provider_item_id: None,
                kind: SaveKind::Import,
            })
            .unwrap();
        let result = storage
            .save(SaveRequest {
                track_key: "track",
                title: "Song",
                artist: "Artist",
                source: "LRCLIB",
                raw: "[00:01]Network",
                provider_id: Some("lrclib"),
                provider_item_id: Some("1"),
                kind: SaveKind::Automatic,
            })
            .unwrap();
        assert_eq!(result.tracks.original.lines[0].text, "Manual");
        let _ = fs::remove_dir_all(app_dir.parent().unwrap());
    }

    #[test]
    fn resetting_overlay_preferences_clears_overlay_only() {
        let (app_dir, library_dir) = test_dirs("reset-preferences");
        let storage = Storage::open(app_dir.clone(), library_dir).expect("open storage");
        storage
            .set_preference("overlay.style.display-a", "custom")
            .unwrap();
        storage
            .set_preference("overlay.position.display-a", "bounds")
            .unwrap();
        storage
            .set_preference("overlay.last_monitor", "display-a")
            .unwrap();
        storage.set_preference("overlay.visible", "true").unwrap();
        storage.set_preference("overlay.locked", "true").unwrap();
        storage
            .set_preference("overlay.passthrough", "true")
            .unwrap();
        storage
            .set_preference("player.selection", "spotify")
            .unwrap();
        storage
            .save(SaveRequest {
                track_key: "track",
                title: "Song",
                artist: "Artist",
                source: "本地导入",
                raw: "[00:01]Manual",
                provider_id: None,
                provider_item_id: None,
                kind: SaveKind::Import,
            })
            .unwrap();

        storage
            .remove_preferences_with_prefix("overlay.style.")
            .unwrap();
        storage
            .remove_preferences_with_prefix("overlay.position.")
            .unwrap();
        storage.remove_preference("overlay.last_monitor").unwrap();
        storage.remove_preference("overlay.visible").unwrap();
        storage.remove_preference("overlay.locked").unwrap();
        storage.remove_preference("overlay.passthrough").unwrap();

        assert_eq!(
            storage.get_preference("overlay.style.display-a").unwrap(),
            None
        );
        for key in [
            "overlay.position.display-a",
            "overlay.last_monitor",
            "overlay.visible",
            "overlay.locked",
            "overlay.passthrough",
        ] {
            assert_eq!(storage.get_preference(key).unwrap(), None);
        }
        assert_eq!(
            storage
                .get_preference("player.selection")
                .unwrap()
                .as_deref(),
            Some("spotify")
        );
        assert_eq!(
            storage
                .load("track")
                .unwrap()
                .unwrap()
                .tracks
                .original
                .lines[0]
                .text,
            "Manual"
        );
        let _ = fs::remove_dir_all(app_dir.parent().unwrap());
    }

    #[test]
    fn migrates_hashed_cache_to_readable_library_file() {
        let (app_dir, library_dir) = test_dirs("migration");
        fs::create_dir_all(app_dir.join("lyrics")).unwrap();
        let legacy = app_dir.join("lyrics/0123456789abcdef.lrc");
        fs::write(&legacy, "[00:01]Legacy").unwrap();
        let connection = Connection::open(app_dir.join("lyrics-plus.sqlite3")).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE lyric_associations (
               track_key TEXT PRIMARY KEY, title TEXT NOT NULL, artist TEXT NOT NULL,
               source TEXT NOT NULL, content_path TEXT NOT NULL,
               offset_ms INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL DEFAULT 0
             );",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO lyric_associations VALUES (?1, ?2, ?3, ?4, ?5, 240, 0)",
                params![
                    "track",
                    "Legacy Song",
                    "Legacy Artist",
                    "LRCLIB",
                    legacy.to_string_lossy()
                ],
            )
            .unwrap();
        drop(connection);

        let storage = Storage::open(app_dir.clone(), library_dir.clone()).unwrap();
        let lyrics = storage.load("track").unwrap().unwrap();
        assert_eq!(lyrics.offset_ms, 240);
        assert!(library_dir.join("Legacy Artist - Legacy Song.lrc").exists());
        assert!(!legacy.exists());
        let _ = fs::remove_dir_all(app_dir.parent().unwrap());
    }
}
