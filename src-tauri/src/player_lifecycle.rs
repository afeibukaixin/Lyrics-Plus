use serde::Serialize;

use crate::config::AppPreferences;

const LAUNCH_AGENT_LABEL: &str = "com.xiaoafei.lyrics-plus.player-follower";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerFollowerServiceState {
    #[cfg(target_os = "macos")]
    Development,
    #[cfg(not(target_os = "macos"))]
    Unsupported,
    #[cfg(target_os = "macos")]
    NotRegistered,
    #[cfg(target_os = "macos")]
    Enabled,
    #[cfg(target_os = "macos")]
    RequiresApproval,
    #[cfg(target_os = "macos")]
    NotFound,
}

pub(crate) fn followed_player_bundle_id(preferences: &AppPreferences) -> Option<&str> {
    preferences
        .player_follower_application
        .as_ref()
        .map(|application| application.bundle_id.as_str())
}

fn runtime_supports_follower(is_dev: bool, is_macos: bool) -> bool {
    !is_dev && is_macos
}

#[cfg(target_os = "macos")]
mod macos {
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use std::sync::mpsc;
    use std::time::Duration;

    use block2::RcBlock;
    use objc2::rc::autoreleasepool;
    use objc2_app_kit::NSRunningApplication;
    use objc2_foundation::{NSError, NSString};
    use objc2_service_management::{SMAppService, SMAppServiceStatus};
    use tauri::Manager;

    use super::{
        followed_player_bundle_id, runtime_supports_follower, PlayerFollowerServiceState,
        LAUNCH_AGENT_LABEL,
    };
    use crate::commands::AppState;
    use crate::config::AppPreferences;

    const HELPER_BUNDLE_ID: &str = "com.xiaoafei.lyrics-plus.player-follower";
    const TARGET_FILE_NAME: &str = "player-follower-target";
    const VERSION_FILE_NAME: &str = "player-follower-service-version";

    pub(crate) fn sync_service(
        app: &tauri::AppHandle,
        preferences: &AppPreferences,
    ) -> Result<(), String> {
        if !runtime_supports_follower(tauri::is_dev(), true) {
            return Ok(());
        }
        cleanup_legacy_launch_agent(app)?;

        let app_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| format!("无法定位应用数据目录：{error}"))?;
        let target_path = app_dir.join(TARGET_FILE_NAME);
        let version_path = app_dir.join(VERSION_FILE_NAME);
        let service = service();

        let Some(target) = followed_player_bundle_id(preferences) else {
            remove_file_if_exists(&target_path)?;
            remove_file_if_exists(&version_path)?;
            return unregister_if_needed(&service);
        };

        fs::create_dir_all(&app_dir)
            .map_err(|error| format!("创建播放器跟随配置目录失败：{error}"))?;
        atomic_write(&target_path, target)?;

        let status = unsafe { service.status() };
        if status == SMAppServiceStatus::NotFound {
            return Err("播放器跟随 Helper 未正确打包".into());
        }
        if status == SMAppServiceStatus::NotRegistered {
            register(&service)?;
            atomic_write(&version_path, env!("CARGO_PKG_VERSION"))?;
            return Ok(());
        }
        if status == SMAppServiceStatus::Enabled
            && fs::read_to_string(&version_path)
                .ok()
                .is_none_or(|version| version.trim() != env!("CARGO_PKG_VERSION"))
        {
            reregister(&service)?;
        }
        atomic_write(&version_path, env!("CARGO_PKG_VERSION"))
    }

    pub(crate) fn service_state() -> PlayerFollowerServiceState {
        if tauri::is_dev() {
            return PlayerFollowerServiceState::Development;
        }
        let status = unsafe { service().status() };
        if status == SMAppServiceStatus::NotRegistered {
            PlayerFollowerServiceState::NotRegistered
        } else if status == SMAppServiceStatus::Enabled {
            PlayerFollowerServiceState::Enabled
        } else if status == SMAppServiceStatus::RequiresApproval {
            PlayerFollowerServiceState::RequiresApproval
        } else {
            PlayerFollowerServiceState::NotFound
        }
    }

    pub(crate) fn open_system_settings() -> Result<(), String> {
        if tauri::is_dev() {
            return Err("开发模式下播放器跟随不可用".into());
        }
        unsafe { SMAppService::openSystemSettingsLoginItems() };
        Ok(())
    }

    pub(crate) fn start_exit_monitor(app: tauri::AppHandle) {
        if tauri::is_dev() {
            return;
        }
        tauri::async_runtime::spawn(async move {
            let mut tracked = None;
            loop {
                let target = app.try_state::<AppState>().and_then(|state| {
                    let config = state.config.snapshot();
                    followed_player_bundle_id(&config.app).map(str::to_owned)
                });
                let running = target.as_deref().is_some_and(application_is_running);
                if should_exit(&mut tracked, target.as_deref(), running) {
                    app.exit(0);
                    break;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
    }

    fn service() -> objc2::rc::Retained<SMAppService> {
        unsafe {
            SMAppService::loginItemServiceWithIdentifier(&NSString::from_str(HELPER_BUNDLE_ID))
        }
    }

    fn register(service: &SMAppService) -> Result<(), String> {
        unsafe { service.registerAndReturnError() }
            .map_err(|error| format!("注册播放器跟随服务失败：{error}"))
    }

    fn unregister_if_needed(service: &SMAppService) -> Result<(), String> {
        let status = unsafe { service.status() };
        if status == SMAppServiceStatus::NotRegistered || status == SMAppServiceStatus::NotFound {
            return Ok(());
        }
        unsafe { service.unregisterAndReturnError() }
            .map_err(|error| format!("停用播放器跟随服务失败：{error}"))
    }

    fn reregister(service: &SMAppService) -> Result<(), String> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let completion = RcBlock::new(move |error: *mut NSError| {
            let _ = sender.send(error.is_null());
        });
        unsafe { service.unregisterWithCompletionHandler(&completion) };
        match receiver.recv_timeout(Duration::from_secs(3)) {
            Ok(true) => register(service),
            Ok(false) => Err("更新播放器跟随服务失败".into()),
            Err(_) => Err("等待播放器跟随服务停止超时".into()),
        }
    }

    fn should_exit(
        tracked: &mut Option<(String, bool)>,
        target: Option<&str>,
        running: bool,
    ) -> bool {
        let Some(target) = target else {
            *tracked = None;
            return false;
        };
        match tracked {
            Some((previous, was_running)) if previous == target => {
                let exit = *was_running && !running;
                *was_running = running;
                exit
            }
            _ => {
                *tracked = Some((target.to_owned(), running));
                false
            }
        }
    }

    fn application_is_running(bundle_id: &str) -> bool {
        autoreleasepool(|_| {
            NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(
                bundle_id,
            ))
            .count()
                > 0
        })
    }

    fn atomic_write(path: &Path, value: &str) -> Result<(), String> {
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, value).map_err(|error| format!("写入播放器跟随配置失败：{error}"))?;
        fs::rename(&temporary, path).map_err(|error| format!("保存播放器跟随配置失败：{error}"))
    }

    fn remove_file_if_exists(path: &Path) -> Result<(), String> {
        if path.exists() {
            fs::remove_file(path).map_err(|error| format!("移除播放器跟随配置失败：{error}"))?;
        }
        Ok(())
    }

    fn cleanup_legacy_launch_agent(app: &tauri::AppHandle) -> Result<(), String> {
        let path = app
            .path()
            .home_dir()
            .map_err(|error| format!("无法定位用户目录：{error}"))?
            .join("Library/LaunchAgents")
            .join(format!("{LAUNCH_AGENT_LABEL}.plist"));
        if !path.exists() {
            return Ok(());
        }
        let domain = launchd_domain()?;
        let _ = Command::new("/bin/launchctl")
            .args(["bootout", &domain])
            .arg(&path)
            .output();
        fs::remove_file(path).map_err(|error| format!("移除旧播放器跟随服务失败：{error}"))
    }

    fn launchd_domain() -> Result<String, String> {
        let output = Command::new("/usr/bin/id")
            .arg("-u")
            .output()
            .map_err(|error| format!("读取用户 ID 失败：{error}"))?;
        let uid = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if output.status.success() && !uid.is_empty() {
            Ok(format!("gui/{uid}"))
        } else {
            Err("读取用户 ID 失败".into())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn exits_only_when_the_same_target_stops() {
            let mut tracked = None;
            assert!(!should_exit(
                &mut tracked,
                Some("org.example.Player"),
                false
            ));
            assert!(!should_exit(&mut tracked, Some("org.example.Player"), true));
            assert!(should_exit(&mut tracked, Some("org.example.Player"), false));

            assert!(!should_exit(&mut tracked, Some("org.example.Other"), false));
            assert!(!should_exit(&mut tracked, None, false));
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) use macos::{open_system_settings, service_state, start_exit_monitor, sync_service};

#[cfg(not(target_os = "macos"))]
pub(crate) fn sync_service(
    _app: &tauri::AppHandle,
    _preferences: &AppPreferences,
) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn service_state() -> PlayerFollowerServiceState {
    PlayerFollowerServiceState::Unsupported
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn open_system_settings() -> Result<(), String> {
    Err("当前系统不支持播放器跟随".into())
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn start_exit_monitor(_app: tauri::AppHandle) {}

#[cfg(test)]
mod tests {
    use crate::config::RegisteredApplication;

    use super::*;

    #[test]
    fn follows_only_an_explicit_player() {
        let mut preferences = AppPreferences::default();
        assert_eq!(followed_player_bundle_id(&preferences), None);

        preferences.player_follower_application = Some(RegisteredApplication {
            name: "Player".into(),
            bundle_id: "org.example.Player".into(),
        });
        assert_eq!(
            followed_player_bundle_id(&preferences),
            Some("org.example.Player")
        );
    }

    #[test]
    fn development_mode_disables_follower() {
        assert!(!runtime_supports_follower(true, true));
        assert!(!runtime_supports_follower(false, false));
        assert!(runtime_supports_follower(false, true));
    }
}
