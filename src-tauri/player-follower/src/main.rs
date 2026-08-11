use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

use objc2::rc::autoreleasepool;
use objc2_app_kit::NSRunningApplication;
use objc2_foundation::NSString;

const APP_BUNDLE_ID: &str = "com.xiaoafei.lyrics-plus";
const TARGET_FILE: &str =
    "Library/Application Support/com.xiaoafei.lyrics-plus/player-follower-target";

#[derive(Default)]
struct FollowerState {
    target: Option<String>,
    handled: bool,
}

impl FollowerState {
    fn update(&mut self, target: Option<&str>, target_running: bool, app_running: bool) -> bool {
        if self.target.as_deref() != target {
            self.target = target.map(str::to_owned);
            self.handled = target_running && app_running;
        }
        if target.is_none() || !target_running {
            self.handled = false;
            return false;
        }
        if app_running {
            self.handled = true;
            return false;
        }
        if self.handled {
            false
        } else {
            self.handled = true;
            true
        }
    }
}

fn main() {
    let target_path = target_path();
    let Some(app_bundle) = app_bundle_path() else {
        return;
    };
    let mut state = FollowerState::default();
    loop {
        let target = std::fs::read_to_string(&target_path)
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let target_running = target.as_deref().is_some_and(application_is_running);
        let app_running = application_is_running(APP_BUNDLE_ID);
        if state.update(target.as_deref(), target_running, app_running)
            && !Command::new("/usr/bin/open")
                .args(["-g"])
                .arg(&app_bundle)
                .status()
                .is_ok_and(|status| status.success())
        {
            state.handled = false;
        }
        thread::sleep(Duration::from_secs(1));
    }
}

fn target_path() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join(TARGET_FILE)
}

fn app_bundle_path() -> Option<PathBuf> {
    env::current_exe()
        .ok()?
        .ancestors()
        .filter_map(|path| {
            (path.extension().and_then(|value| value.to_str()) == Some("app"))
                .then(|| path.to_path_buf())
        })
        .last()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launches_once_per_target_run() {
        let mut state = FollowerState::default();
        assert!(!state.update(Some("org.example.Player"), false, false));
        assert!(state.update(Some("org.example.Player"), true, false));
        assert!(!state.update(Some("org.example.Player"), true, false));
        assert!(!state.update(Some("org.example.Player"), false, false));
        assert!(state.update(Some("org.example.Player"), true, false));
    }

    #[test]
    fn manual_app_quit_does_not_relaunch_until_player_restarts() {
        let mut state = FollowerState::default();
        assert!(!state.update(Some("org.example.Player"), true, true));
        assert!(!state.update(Some("org.example.Player"), true, false));
        assert!(!state.update(Some("org.example.Player"), false, false));
        assert!(state.update(Some("org.example.Player"), true, false));
    }

    #[test]
    fn switching_target_resets_tracking_without_launching_a_running_app() {
        let mut state = FollowerState::default();
        assert!(!state.update(Some("org.example.First"), true, true));
        assert!(!state.update(Some("org.example.Second"), true, true));
        assert!(!state.update(Some("org.example.Second"), true, false));
        assert!(!state.update(None, false, false));
    }
}
