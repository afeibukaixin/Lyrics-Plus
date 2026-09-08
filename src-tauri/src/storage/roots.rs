pub(crate) const MANAGED_ROOT_ID: &str = "managed_downloads";
pub(crate) const LEGACY_ROOT_ID: &str = "legacy_history";
pub(crate) const CACHE_ROOT_ID: &str = "cache";

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryRootView {
    pub root_id: String,
    pub root_kind: String,
    pub display_name: String,
    pub path: String,
    pub read_only: bool,
    pub enabled: bool,
    pub file_count: u64,
    pub unavailable_count: u64,
    pub last_scan_at: Option<i64>,
    pub last_error: Option<String>,
}

impl Storage {
    pub(crate) fn initialize_library_roots(
        &self,
        app_dir: &Path,
        legacy_dir: &Path,
    ) -> Result<(), String> {
        let cache_dir = app_dir.join("cache");
        fs::create_dir_all(&cache_dir).map_err(|error| format!("创建歌词缓存目录失败：{error}"))?;
        let roots = [
            (
                MANAGED_ROOT_ID,
                "managed",
                "应用下载目录",
                self.library_directory(),
                false,
            ),
            (
                LEGACY_ROOT_ID,
                "legacy",
                "历史目录",
                legacy_dir.to_path_buf(),
                true,
            ),
            (CACHE_ROOT_ID, "cache", "内部缓存", cache_dir, false),
        ];
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        for (root_id, root_kind, display_name, path, read_only) in roots {
            connection
                .execute(
                    "INSERT INTO library_roots
                       (root_id, root_kind, display_name, path, read_only, enabled, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, 1, unixepoch())
                     ON CONFLICT(root_id) DO UPDATE SET
                       root_kind=excluded.root_kind,
                       display_name=excluded.display_name,
                       path=excluded.path,
                       read_only=excluded.read_only,
                       enabled=1,
                       updated_at=unixepoch()",
                    params![
                        root_id,
                        root_kind,
                        display_name,
                        path.to_string_lossy(),
                        read_only,
                    ],
                )
                .map_err(|error| format!("登记歌词根目录失败：{error}"))?;
        }
        Ok(())
    }

    pub fn list_library_roots(&self) -> Result<Vec<LibraryRootView>, String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut statement = connection
            .prepare(
                "SELECT root_id, root_kind, display_name, path, read_only, enabled,
                        file_count, unavailable_count, last_scan_at, last_error
                 FROM library_roots
                 ORDER BY CASE root_kind
                    WHEN 'managed' THEN 0 WHEN 'local' THEN 1 WHEN 'legacy' THEN 2 ELSE 3 END,
                    display_name, root_id",
            )
            .map_err(|error| format!("读取歌词根目录失败：{error}"))?;
        let roots = statement
            .query_map([], |row| {
                Ok(LibraryRootView {
                    root_id: row.get(0)?,
                    root_kind: row.get(1)?,
                    display_name: row.get(2)?,
                    path: row.get(3)?,
                    read_only: row.get::<_, i64>(4)? != 0,
                    enabled: row.get::<_, i64>(5)? != 0,
                    file_count: row.get::<_, i64>(6)?.max(0) as u64,
                    unavailable_count: row.get::<_, i64>(7)?.max(0) as u64,
                    last_scan_at: row.get(8)?,
                    last_error: row.get(9)?,
                })
            })
            .map_err(|error| format!("解析歌词根目录失败：{error}"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| format!("解析歌词根目录失败：{error}"))?;
        Ok(roots)
    }

    pub fn add_library_root(
        &self,
        path: &str,
        display_name: Option<&str>,
    ) -> Result<LibraryRootView, String> {
        let path = PathBuf::from(path.trim())
            .canonicalize()
            .map_err(|error| format!("无法访问歌词文件夹：{error}"))?;
        if !path.is_dir() {
            return Err("所选路径不是文件夹".into());
        }
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut statement = connection
            .prepare("SELECT root_id, path FROM library_roots WHERE enabled=1")
            .map_err(|error| format!("读取歌词根目录失败：{error}"))?;
        let existing = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    PathBuf::from(row.get::<_, String>(1)?),
                ))
            })
            .map_err(|error| format!("解析歌词根目录失败：{error}"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| format!("解析歌词根目录失败：{error}"))?;
        if let Some((root_id, _)) = existing.iter().find(|(_, existing_path)| {
            path.starts_with(existing_path) || existing_path.starts_with(&path)
        }) {
            return Err(format!("歌词目录与已有根目录重叠：{root_id}"));
        }
        drop(statement);
        let root_id = format!("local_{}", content_hash(&path.to_string_lossy()));
        let display_name = display_name
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("本地歌词");
        if connection
            .query_row(
                "SELECT 1 FROM library_roots WHERE root_id=?1 AND root_kind='local'",
                params![root_id],
                |_row| Ok(()),
            )
            .optional()
            .map_err(|error| format!("读取已有本地歌词根目录失败：{error}"))?
            .is_some()
        {
            connection
                .execute(
                    "UPDATE library_roots SET display_name=?2, path=?3, enabled=1,
                            last_error=NULL, updated_at=unixepoch()
                     WHERE root_id=?1 AND root_kind='local'",
                    params![root_id, display_name, path.to_string_lossy()],
                )
                .map_err(|error| format!("恢复本地歌词根目录失败：{error}"))?;
            drop(connection);
            return self
                .list_library_roots()?
                .into_iter()
                .find(|root| root.root_id == root_id)
                .ok_or_else(|| "恢复本地歌词根目录后无法读取".into());
        }
        connection
            .execute(
                "INSERT INTO library_roots
                   (root_id, root_kind, display_name, path, read_only, enabled, updated_at)
                 VALUES (?1, 'local', ?2, ?3, 1, 1, unixepoch())",
                params![root_id, display_name, path.to_string_lossy()],
            )
            .map_err(|error| format!("保存本地歌词根目录失败：{error}"))?;
        drop(connection);
        self.list_library_roots()?
            .into_iter()
            .find(|root| root.root_id == root_id)
            .ok_or_else(|| "保存本地歌词根目录后无法读取".into())
    }

    pub fn remove_library_root(&self, root_id: &str) -> Result<(), String> {
        if matches!(root_id, MANAGED_ROOT_ID | LEGACY_ROOT_ID | CACHE_ROOT_ID) {
            return Err("应用固定歌词根目录不能移除".into());
        }
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let path = connection
            .query_row(
                "SELECT path FROM library_roots WHERE root_id=?1 AND root_kind='local'",
                params![root_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("读取本地歌词根目录失败：{error}"))?
            .ok_or_else(|| "本地歌词根目录不存在".to_string())?;
        self.scanner.cancel_if_matches(Path::new(&path));
        let transaction = connection
            .transaction()
            .map_err(|error| format!("开始移除歌词根目录失败：{error}"))?;
        invalidate_library_root(&transaction, root_id, &path)?;
        transaction
            .execute(
                "DELETE FROM library_roots WHERE root_id=?1 AND root_kind='local'",
                params![root_id],
            )
            .map_err(|error| format!("删除本地歌词根目录登记失败：{error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("提交移除歌词根目录失败：{error}"))
    }

    pub fn set_library_root_enabled(
        &self,
        root_id: &str,
        enabled: bool,
    ) -> Result<LibraryRootView, String> {
        if matches!(root_id, MANAGED_ROOT_ID | LEGACY_ROOT_ID | CACHE_ROOT_ID) {
            return Err("应用固定歌词根目录不能切换启用状态".into());
        }
        if !enabled {
            let mut connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let path = connection
                .query_row(
                    "SELECT path FROM library_roots WHERE root_id=?1 AND root_kind='local'",
                    params![root_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|error| format!("读取本地歌词根目录失败：{error}"))?
                .ok_or_else(|| "本地歌词根目录不存在".to_string())?;
            self.scanner.cancel_if_matches(Path::new(&path));
            let transaction = connection
                .transaction()
                .map_err(|error| format!("开始停用歌词根目录失败：{error}"))?;
            invalidate_library_root(&transaction, root_id, &path)?;
            transaction
                .execute(
                    "UPDATE library_roots SET enabled=0, updated_at=unixepoch() WHERE root_id=?1",
                    params![root_id],
                )
                .map_err(|error| format!("停用本地歌词根目录失败：{error}"))?;
            transaction
                .commit()
                .map_err(|error| format!("提交停用歌词根目录失败：{error}"))?;
        } else {
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let path = connection
                .query_row(
                    "SELECT path FROM library_roots WHERE root_id=?1 AND root_kind='local'",
                    params![root_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|error| format!("读取本地歌词根目录失败：{error}"))?
                .ok_or_else(|| "本地歌词根目录不存在".to_string())?;
            if !Path::new(&path).is_dir() {
                return Err("本地歌词根目录不可用".into());
            }
            connection
                .execute(
                    "UPDATE library_roots SET enabled=1, last_error=NULL, updated_at=unixepoch()
                     WHERE root_id=?1 AND root_kind='local'",
                    params![root_id],
                )
                .map_err(|error| format!("启用本地歌词根目录失败：{error}"))?;
        }
        self.list_library_roots()?
            .into_iter()
            .find(|root| root.root_id == root_id)
            .ok_or_else(|| "更新本地歌词根目录后无法读取".into())
    }
}

fn invalidate_library_root(
    transaction: &rusqlite::Transaction<'_>,
    root_id: &str,
    path: &str,
) -> Result<(), String> {
    let separator = std::path::MAIN_SEPARATOR.to_string();
    let prefix = if path.ends_with(&separator) {
        path.to_owned()
    } else {
        format!("{path}{}", std::path::MAIN_SEPARATOR)
    };
    transaction
        .execute(
            "UPDATE lyric_files SET available=0, updated_at=unixepoch()
             WHERE content_path=?1 OR substr(content_path, 1, length(?2))=?2",
            params![path, prefix],
        )
        .map_err(|error| format!("标记本地歌词不可用失败：{error}"))?;
    transaction
        .execute(
            "UPDATE lyric_asset_sources SET available=0, updated_at=unixepoch()
             WHERE root_dir_id=?1",
            params![root_id],
        )
        .map_err(|error| format!("标记歌词来源不可用失败：{error}"))?;
    transaction
        .execute(
            "UPDATE lyric_assets SET available=0, updated_at=unixepoch()
             WHERE root_dir_id=?1 AND NOT EXISTS (
               SELECT 1 FROM lyric_asset_sources
               WHERE lyric_asset_sources.asset_id=lyric_assets.asset_id
                 AND lyric_asset_sources.available=1
             )",
            params![root_id],
        )
        .map_err(|error| format!("标记歌词资源不可用失败：{error}"))?;
    Ok(())
}
