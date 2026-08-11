use crate::config::AppPreferences;

pub(crate) fn followed_player_bundle_id(preferences: &AppPreferences) -> Option<&str> {
    preferences
        .player_follower_application
        .as_ref()
        .map(|application| application.bundle_id.as_str())
}

#[cfg(target_os = "macos")]
mod macos {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::Duration;

    use objc2::rc::autoreleasepool;
    use objc2_app_kit::NSRunningApplication;
    use objc2_foundation::NSString;
    use tauri::Manager;

    use super::followed_player_bundle_id;
    use crate::commands::AppState;
    use crate::config::AppPreferences;

    const LAUNCH_AGENT_LABEL: &str = "com.xiaoafei.lyrics-plus.player-follower";
    const FOLLOWER_SCRIPT: &str = r#"
ObjC.import('AppKit')
ObjC.import('Foundation')

function isRunning(bundleId) {
  return $.NSRunningApplication.runningApplicationsWithBundleIdentifier(bundleId).count > 0
}

function launch(path) {
  const task = $.NSTask.alloc.init
  task.launchPath = '/usr/bin/open'
  task.arguments = ['-g', path]
  task.launch()
}

function run(argv) {
  const targetBundleId = argv[0]
  const appPath = argv[1]
  const appBundleId = argv[2]
  let handled = isRunning(targetBundleId) && isRunning(appBundleId)
  while (true) {
    const targetRunning = isRunning(targetBundleId)
    const appRunning = isRunning(appBundleId)
    if (!targetRunning) handled = false
    else if (appRunning) handled = true
    else if (!handled) {
      launch(appPath)
      handled = true
    }
    delay(1)
  }
}
"#;

    pub(crate) fn sync_launch_agent(
        app: &tauri::AppHandle,
        preferences: &AppPreferences,
    ) -> Result<(), String> {
        let path = app
            .path()
            .home_dir()
            .map_err(|error| format!("无法定位用户目录：{error}"))?
            .join("Library/LaunchAgents")
            .join(format!("{LAUNCH_AGENT_LABEL}.plist"));
        let domain = launchd_domain()?;
        let Some(target) = followed_player_bundle_id(preferences) else {
            bootout(&domain, &path);
            if path.exists() {
                fs::remove_file(&path)
                    .map_err(|error| format!("移除播放器跟随配置失败：{error}"))?;
            }
            return Ok(());
        };
        let Some(app_bundle) = current_app_bundle()? else {
            log::debug!("Skipping player follower registration outside an application bundle");
            return Ok(());
        };

        let parent = path
            .parent()
            .ok_or_else(|| "播放器跟随配置目录无效".to_string())?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("创建播放器跟随配置目录失败：{error}"))?;
        let plist = launch_agent_plist(target, &app_bundle, &app.config().identifier);
        let temporary = path.with_extension("plist.tmp");
        fs::write(&temporary, plist).map_err(|error| format!("写入播放器跟随配置失败：{error}"))?;
        fs::rename(&temporary, &path)
            .map_err(|error| format!("保存播放器跟随配置失败：{error}"))?;

        bootout(&domain, &path);
        let output = Command::new("/bin/launchctl")
            .args(["bootstrap", &domain])
            .arg(&path)
            .output()
            .map_err(|error| format!("启动播放器跟随服务失败：{error}"))?;
        if output.status.success() {
            Ok(())
        } else {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(if detail.is_empty() {
                "启动播放器跟随服务失败".into()
            } else {
                format!("启动播放器跟随服务失败：{detail}")
            })
        }
    }

    pub(crate) fn start_exit_monitor(app: tauri::AppHandle) {
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

    fn current_app_bundle() -> Result<Option<PathBuf>, String> {
        let executable = std::env::current_exe()
            .map_err(|error| format!("无法定位 Lyrics Plus 可执行文件：{error}"))?;
        Ok(executable
            .ancestors()
            .find(|path| path.extension().and_then(|value| value.to_str()) == Some("app"))
            .map(Path::to_path_buf))
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

    fn bootout(domain: &str, path: &Path) {
        let _ = Command::new("/bin/launchctl")
            .args(["bootout", domain])
            .arg(path)
            .output();
    }

    fn launch_agent_plist(target: &str, app_bundle: &Path, app_bundle_id: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{}</string>
  <key>ProgramArguments</key>
  <array>
    <string>/usr/bin/osascript</string>
    <string>-l</string><string>JavaScript</string>
    <string>-e</string><string>{}</string>
    <string>--</string>
    <string>{}</string>
    <string>{}</string>
    <string>{}</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>ProcessType</key><string>Background</string>
</dict>
</plist>
"#,
            LAUNCH_AGENT_LABEL,
            xml_escape(FOLLOWER_SCRIPT),
            xml_escape(target),
            xml_escape(&app_bundle.to_string_lossy()),
            xml_escape(app_bundle_id),
        )
    }

    fn xml_escape(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    #[cfg(test)]
    mod tests {
        use std::io::Write;
        use std::process::Stdio;

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

        #[test]
        fn launch_agent_escapes_values() {
            let plist = launch_agent_plist(
                "org.example.A&B",
                Path::new("/Applications/A&B.app"),
                "org.example.Lyrics",
            );
            assert!(plist.contains("org.example.A&amp;B"));
            assert!(plist.contains("/Applications/A&amp;B.app"));
            assert!(plist.contains("KeepAlive"));

            let mut validator = Command::new("/usr/bin/plutil")
                .args(["-lint", "-"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .spawn()
                .unwrap();
            validator
                .stdin
                .take()
                .unwrap()
                .write_all(plist.as_bytes())
                .unwrap();
            assert!(validator.wait().unwrap().success());
        }

        #[test]
        fn follower_script_stays_running_while_waiting() {
            let mut follower = Command::new("/usr/bin/osascript")
                .args(["-l", "JavaScript", "-e", FOLLOWER_SCRIPT, "--"])
                .args([
                    "org.example.NotRunning",
                    "/Applications/NotInstalled.app",
                    "org.example.Lyrics",
                ])
                .spawn()
                .unwrap();
            std::thread::sleep(Duration::from_millis(200));
            assert!(follower.try_wait().unwrap().is_none());
            follower.kill().unwrap();
            follower.wait().unwrap();
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) use macos::{start_exit_monitor, sync_launch_agent};

#[cfg(not(target_os = "macos"))]
pub(crate) fn sync_launch_agent(
    _app: &tauri::AppHandle,
    _preferences: &AppPreferences,
) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn start_exit_monitor(_app: tauri::AppHandle) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RegisteredApplication;

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
}
