use std::cell::RefCell;
use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::ptr::NonNull;
use std::rc::Rc;

use block2::RcBlock;
use objc2::rc::autoreleasepool;
use objc2_app_kit::{
    NSRunningApplication, NSWorkspace, NSWorkspaceApplicationKey,
    NSWorkspaceDidLaunchApplicationNotification, NSWorkspaceDidTerminateApplicationNotification,
};
use objc2_foundation::{NSNotification, NSOperationQueue, NSRunLoop, NSString};

const APP_BUNDLE_ID: &str = "com.xiaoafei.lyrics-plus";
const TARGET_FILE: &str =
    "Library/Application Support/com.xiaoafei.lyrics-plus/player-follower-target";

#[derive(Debug, PartialEq, Eq)]
enum LaunchAction {
    Ignore,
    Open,
    Defer,
}

fn main() {
    let target_path = target_path();
    let Some(app_bundle) = app_bundle_path() else {
        return;
    };

    let pending_target = Rc::new(RefCell::new(None::<String>));
    let event_target_path = target_path.clone();
    let event_app_bundle = app_bundle.clone();
    let launch_pending_target = Rc::clone(&pending_target);
    let launched = RcBlock::new(move |notification: NonNull<NSNotification>| {
        let Some(bundle_id) = notification_bundle_id(notification) else {
            return;
        };
        let target = configured_target(&event_target_path);
        match launch_action(
            target.as_deref(),
            &bundle_id,
            application_is_running(APP_BUNDLE_ID),
        ) {
            LaunchAction::Open => {
                launch_pending_target.borrow_mut().take();
                open_app(&event_app_bundle);
            }
            LaunchAction::Defer => {
                launch_pending_target.borrow_mut().replace(bundle_id);
            }
            LaunchAction::Ignore => {}
        }
    });

    let termination_target_path = target_path.clone();
    let termination_app_bundle = app_bundle.clone();
    let termination_pending_target = Rc::clone(&pending_target);
    let terminated = RcBlock::new(move |notification: NonNull<NSNotification>| {
        if notification_bundle_id(notification).as_deref() != Some(APP_BUNDLE_ID) {
            return;
        }
        let Some(pending) = termination_pending_target.borrow_mut().take() else {
            return;
        };
        let target = configured_target(&termination_target_path);
        if should_open_pending(
            Some(&pending),
            target.as_deref(),
            application_is_running(&pending),
            application_is_running(APP_BUNDLE_ID),
        ) {
            open_app(&termination_app_bundle);
        }
    });

    let center = NSWorkspace::sharedWorkspace().notificationCenter();
    let queue = NSOperationQueue::mainQueue();
    let _launch_observer = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidLaunchApplicationNotification),
            None,
            Some(&queue),
            &launched,
        )
    };
    let _termination_observer = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidTerminateApplicationNotification),
            None,
            Some(&queue),
            &terminated,
        )
    };

    if let Some(target) = configured_target(&target_path) {
        if application_is_running(&target) && !application_is_running(APP_BUNDLE_ID) {
            open_app(&app_bundle);
        }
    }
    NSRunLoop::mainRunLoop().run();
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

fn configured_target(path: &PathBuf) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn launch_action(
    target: Option<&str>,
    launched_bundle_id: &str,
    app_running: bool,
) -> LaunchAction {
    if target != Some(launched_bundle_id) {
        LaunchAction::Ignore
    } else if app_running {
        LaunchAction::Defer
    } else {
        LaunchAction::Open
    }
}

fn should_open_pending(
    pending: Option<&str>,
    target: Option<&str>,
    target_running: bool,
    app_running: bool,
) -> bool {
    pending.is_some() && pending == target && target_running && !app_running
}

fn notification_bundle_id(notification: NonNull<NSNotification>) -> Option<String> {
    autoreleasepool(|_| {
        let notification = unsafe { notification.as_ref() };
        let user_info = notification.userInfo()?;
        let application = user_info
            .objectForKey(unsafe { NSWorkspaceApplicationKey })?
            .downcast::<NSRunningApplication>()
            .ok()?;
        application
            .bundleIdentifier()
            .map(|value| value.to_string())
    })
}

fn open_app(app_bundle: &PathBuf) {
    match Command::new("/usr/bin/open")
        .args(["-g"])
        .arg(app_bundle)
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => eprintln!("failed to open Lyrics Plus: {status}"),
        Err(error) => eprintln!("failed to open Lyrics Plus: {error}"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_state_handles_fast_player_restart_without_reopening_after_manual_quit() {
        assert_eq!(
            launch_action(Some("org.example.Player"), "org.example.Player", false),
            LaunchAction::Open
        );
        assert_eq!(
            launch_action(Some("org.example.Player"), "org.example.Player", true),
            LaunchAction::Defer
        );
        assert_eq!(
            launch_action(Some("org.example.Player"), "org.example.Other", false),
            LaunchAction::Ignore
        );
        assert!(should_open_pending(
            Some("org.example.Player"),
            Some("org.example.Player"),
            true,
            false,
        ));
        assert!(!should_open_pending(
            None,
            Some("org.example.Player"),
            true,
            false,
        ));
        assert!(!should_open_pending(
            Some("org.example.Player"),
            Some("org.example.Other"),
            true,
            false,
        ));
    }
}
