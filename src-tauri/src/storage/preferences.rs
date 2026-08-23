impl Storage {
    pub fn get_preference(&self, key: &str) -> Result<Option<String>, String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .query_row(
                "SELECT value FROM app_preferences WHERE key=?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("读取应用偏好失败：{error}"))
    }

    pub fn set_preference(&self, key: &str, value: &str) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .execute(
                "INSERT INTO app_preferences (key, value, updated_at)
                 VALUES (?1, ?2, unixepoch())
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=unixepoch()",
                params![key, value],
            )
            .map_err(|error| format!("保存应用偏好失败：{error}"))?;
        Ok(())
    }

    pub fn remove_preference(&self, key: &str) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .execute("DELETE FROM app_preferences WHERE key=?1", params![key])
            .map_err(|error| format!("重置应用偏好失败：{error}"))?;
        Ok(())
    }

    pub fn remove_preferences_with_prefix(&self, prefix: &str) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .execute(
                "DELETE FROM app_preferences WHERE substr(key, 1, length(?1))=?1",
                params![prefix],
            )
            .map_err(|error| format!("重置应用偏好失败：{error}"))?;
        Ok(())
    }
}
