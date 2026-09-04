use std::path::PathBuf;

use rusqlite::{params, Connection};

use super::super::{ensure_column, read_lyric_text};
use super::metadata::{collision_filename_parts, lrc_tag};

pub(in crate::storage) fn initialize_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute(
        "CREATE INDEX IF NOT EXISTS lyric_files_content_hash ON lyric_files(content_hash)",
        [],
    )?;
    connection.execute(
        "CREATE INDEX IF NOT EXISTS lyric_associations_content_path
         ON lyric_associations(content_path)",
        [],
    )?;
    ensure_column(connection, "lyric_files", "folder_id", "INTEGER")?;
    ensure_column(
        connection,
        "lyric_files",
        "managed",
        "INTEGER NOT NULL DEFAULT 1",
    )?;
    ensure_column(
        connection,
        "lyric_files",
        "file_size",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(connection, "lyric_files", "modified_at_ms", "INTEGER")?;
    ensure_column(connection, "lyric_files", "duration_ms", "INTEGER")?;
    ensure_column(
        connection,
        "lyric_files",
        "file_fingerprint",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        connection,
        "lyric_files",
        "has_translation",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        connection,
        "lyric_files",
        "has_word_timing",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        connection,
        "lyric_files",
        "has_romanization",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    let added_app_owned = ensure_column(
        connection,
        "lyric_files",
        "app_owned",
        "INTEGER NOT NULL DEFAULT 1",
    )?;
    if added_app_owned {
        connection.execute(
            "UPDATE lyric_files SET app_owned=0 WHERE source IN ('本地文件', '本地导入', '手动导入')",
            [],
        )?;
    } else {
        connection.execute(
            "UPDATE lyric_files SET app_owned=0
             WHERE source IN ('本地文件', '本地导入', '手动导入') AND app_owned!=0",
            [],
        )?;
    }
    Ok(())
}

pub(in crate::storage) fn normalize_collision_metadata(
    connection: &Connection,
) -> rusqlite::Result<()> {
    let rows = {
        let mut statement = connection.prepare(
            "SELECT content_path, title, artist
             FROM lyric_files
             WHERE app_owned=1 AND source NOT IN ('本地文件', '本地导入', '手动导入')",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    PathBuf::from(row.get::<_, String>(0)?),
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };
    for (path, title, artist) in rows {
        let Some((base_path, filename_artist, filename_title)) = collision_filename_parts(&path)
        else {
            continue;
        };
        if !base_path.is_file()
            || lrc_tag(&read_lyric_text(&path).unwrap_or_default(), "ti").is_some()
        {
            continue;
        }
        if title != filename_title || artist != filename_artist {
            continue;
        }
        connection.execute(
            "UPDATE lyric_files SET title=?2, artist=?3, updated_at=unixepoch()
             WHERE content_path=?1 AND app_owned=1",
            params![path.to_string_lossy(), filename_title, filename_artist],
        )?;
        connection.execute(
            "UPDATE lyric_associations SET title=?2, artist=?3
             WHERE content_path=?1",
            params![path.to_string_lossy(), filename_title, filename_artist],
        )?;
    }
    Ok(())
}
