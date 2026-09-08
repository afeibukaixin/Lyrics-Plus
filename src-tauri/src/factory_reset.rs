use std::fs;
use std::path::Path;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tauri_plugin_window_state::AppHandleExt;

use crate::storage::Storage;

pub(crate) const CONFIRMATION: &str = "RESET LYRICS PLUS";

const PENDING_MARKER: &str = ".factory-reset.pending";
const MARKER_CONTENT: &str = "lyrics-plus-factory-reset-v1";

/// 删除本次初始化前必须清理的外部文件，并安排下一次启动清空应用数据。
pub(crate) fn prepare(app: &AppHandle, storage: &Storage, confirmation: &str) -> Result<(), String> {
    if confirmation != CONFIRMATION {
        return Err("确认短语不正确，请输入 RESET LYRICS PLUS".into());
    }

    let deleted_files = storage.remove_application_downloads()?;
    clear_webview_data(app)?;
    remove_window_state(app)?;
    write_pending_marker(app)?;
    log::info!(
        "Application factory reset scheduled: deleted_managed_lyrics={deleted_files}"
    );

    // 使用 Tauri 原生重启，避免窗口状态插件在退出事件中把旧状态重新写回磁盘。
    app.restart()
}

/// 在插件和应用状态初始化前消费一次性标记，恢复到首次启动所需的磁盘状态。
pub(crate) fn consume_pending(app: &AppHandle) -> Result<(), String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法定位应用数据目录：{error}"))?;
    let marker = app_data_dir.join(PENDING_MARKER);
    if !marker.exists() {
        return Ok(());
    }

    let marker_content = fs::read_to_string(&marker)
        .map_err(|error| format!("读取初始化标记失败：{error}"))?;
    if marker_content.trim() != MARKER_CONTENT {
        return Err("初始化标记无效，请删除后重启应用".into());
    }

    let app_config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("无法定位应用配置目录：{error}"))?;
    let app_cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("无法定位应用缓存目录：{error}"))?;

    clear_directory_except(&app_data_dir, &marker)?;
    if app_config_dir != app_data_dir {
        remove_directory_if_exists(&app_config_dir)?;
    }
    if app_cache_dir != app_data_dir
        && app_cache_dir != app_config_dir
    {
        remove_directory_if_exists(&app_cache_dir)?;
    }
    remove_file_if_exists(&marker)
        .map_err(|error| format!("删除初始化标记失败：{error}"))?;
    Ok(())
}

fn clear_webview_data(app: &AppHandle) -> Result<(), String> {
    for window in app.webview_windows().into_values() {
        window
            .clear_all_browsing_data()
            .map_err(|error| format!("清除界面本地数据失败：{error}"))?;
    }
    Ok(())
}

fn remove_window_state(app: &AppHandle) -> Result<(), String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("无法定位窗口状态目录：{error}"))?;
    let state_path = config_dir.join(app.filename());
    remove_file_if_exists(&state_path)
        .map_err(|error| format!("清除窗口状态失败：{error}"))
}

fn write_pending_marker(app: &AppHandle) -> Result<(), String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法定位应用数据目录：{error}"))?;
    fs::create_dir_all(&app_data_dir)
        .map_err(|error| format!("创建初始化标记目录失败：{error}"))?;
    let marker = app_data_dir.join(PENDING_MARKER);
    let temporary = app_data_dir.join(format!("{PENDING_MARKER}.tmp"));
    fs::write(&temporary, MARKER_CONTENT)
        .map_err(|error| format!("写入初始化标记失败：{error}"))?;
    if let Err(error) = fs::rename(&temporary, &marker) {
        let _ = fs::remove_file(&temporary);
        return Err(format!("保存初始化标记失败：{error}"));
    }
    Ok(())
}

fn remove_directory_if_exists(path: &Path) -> Result<(), String> {
    let mut last_error = None;
    for attempt in 0..5 {
        match fs::remove_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                if attempt < 4 {
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }
    }
    Err(format!(
        "删除目录 {} 失败：{}",
        path.display(),
        last_error.expect("目录删除重试应记录最后一次错误")
    ))
}

fn clear_directory_except(directory: &Path, preserved: &Path) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("读取目录 {} 失败：{error}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("读取目录项失败：{error}"))?;
        let path = entry.path();
        if path == preserved {
            continue;
        }
        let file_type = entry
            .file_type()
            .map_err(|error| format!("读取目录项 {} 失败：{error}", path.display()))?;
        if file_type.is_dir() {
            remove_directory_if_exists(&path)?;
        } else {
            remove_file_if_exists(&path)
                .map_err(|error| format!("删除文件 {} 失败：{error}", path.display()))?;
        }
    }
    Ok(())
}

fn remove_file_if_exists(path: &Path) -> Result<(), std::io::Error> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
