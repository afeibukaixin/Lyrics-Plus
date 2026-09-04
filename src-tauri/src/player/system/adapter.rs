use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, RwLock,
};
use std::time::{Duration, Instant};

use media_remote::{Controller, NowPlayingPerl, Subscription};
use serde_json::Value;

use super::super::{run_with_timeout, PlaybackAction};
use super::metadata::{milliseconds, timed_info, TimedInfo};

pub(super) struct AdapterClient {
    pub(super) player: NowPlayingPerl,
    pub(super) latest: Arc<RwLock<Option<TimedInfo>>>,
    pub(super) resync_requested: Arc<AtomicBool>,
    pub(super) script_path: PathBuf,
    pub(super) framework_path: PathBuf,
}

pub(super) fn initialize() -> Result<AdapterClient, String> {
    let existing = adapter_directories();
    let player = catch_unwind(AssertUnwindSafe(NowPlayingPerl::new))
        .map_err(|_| "无法启动系统媒体适配器".to_string())?;
    let latest = Arc::new(RwLock::new(None));
    let latest_for_listener = latest.clone();
    let resync_requested = Arc::new(AtomicBool::new(true));
    let resync_for_listener = resync_requested.clone();
    player.subscribe(move |info| {
        let next = info.as_ref().cloned().and_then(timed_info);
        *latest_for_listener
            .write()
            .unwrap_or_else(|error| error.into_inner()) = next;
        resync_for_listener.store(true, Ordering::SeqCst);
    });
    // 适配器的 get 脚本仍用于刷新精确进度；固定版本并定位它刚创建的临时目录，避免自行维护资源副本。
    let directory = adapter_directories()
        .into_iter()
        .find(|path| !existing.contains(path))
        .ok_or_else(|| "无法定位系统媒体适配器资源".to_string())?;
    let script_path = directory.join("mediaremote-adapter.pl");
    let framework_path = directory.join("MediaRemoteAdapter.framework");
    if !script_path.is_file() || !framework_path.is_dir() {
        return Err("系统媒体适配器资源不完整".into());
    }
    let output = run_adapter(
        &script_path,
        &framework_path,
        ["get", "--no-artwork", "--now"],
    )?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if detail.is_empty() {
            "系统媒体适配器自检失败".into()
        } else {
            detail
        });
    }
    Ok(AdapterClient {
        player,
        latest,
        resync_requested,
        script_path,
        framework_path,
    })
}

pub(super) fn control(client: &AdapterClient, action: PlaybackAction) -> Result<(), String> {
    let accepted = match action {
        PlaybackAction::Play => client.player.play(),
        PlaybackAction::Pause => client.player.pause(),
        PlaybackAction::TogglePlayPause => client.player.toggle(),
        PlaybackAction::Previous => client.player.previous(),
        PlaybackAction::Next => client.player.next(),
    };
    if accepted {
        Ok(())
    } else {
        Err("系统媒体播放器未接受控制命令".into())
    }
}

pub(super) fn seek(client: &AdapterClient, position_ms: u64) -> Result<(), String> {
    let position_micros = position_ms.saturating_mul(1_000);
    let position = position_micros.to_string();
    let output = run_adapter(
        &client.script_path,
        &client.framework_path,
        ["seek", position.as_str()],
    )?;
    if output.status.success() {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if detail.is_empty() {
            "系统媒体播放器未接受跳转命令".into()
        } else {
            detail
        })
    }
}

pub(super) fn refresh_elapsed(client: &AdapterClient) {
    if client
        .latest
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .is_none()
        || !client.resync_requested.swap(false, Ordering::SeqCst)
    {
        return;
    }
    if let Ok(output) = run_adapter(
        &client.script_path,
        &client.framework_path,
        ["get", "--no-artwork", "--now"],
    ) {
        if output.status.success() {
            sync_elapsed_from_adapter(&client.latest, &output.stdout);
        }
    }
}

pub(super) fn sync_elapsed_from_adapter(latest: &RwLock<Option<TimedInfo>>, output: &[u8]) -> bool {
    let Ok(payload) = serde_json::from_slice::<Value>(output) else {
        return false;
    };
    let Some(position_ms) = milliseconds(payload.get("elapsedTimeNow").and_then(Value::as_f64))
    else {
        return false;
    };
    let mut latest = latest.write().unwrap_or_else(|error| error.into_inner());
    let Some(timed) = latest.as_mut() else {
        return false;
    };
    let same_track = payload.get("title").and_then(Value::as_str) == timed.info.title.as_deref()
        && payload.get("bundleIdentifier").and_then(Value::as_str)
            == timed.info.bundle_id.as_deref();
    if same_track {
        timed.info.elapsed_time = Some(position_ms as f64 / 1000.0);
        timed.received_at = Instant::now();
    }
    same_track
}

fn run_adapter(
    script_path: &std::path::Path,
    framework_path: &std::path::Path,
    arguments: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
) -> Result<std::process::Output, String> {
    let mut command = Command::new("/usr/bin/perl");
    command.arg(script_path).arg(framework_path).args(arguments);
    run_with_timeout(command, Duration::from_secs(3))
}

fn adapter_directories() -> Vec<PathBuf> {
    std::fs::read_dir(std::env::temp_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            name.to_string_lossy()
                .starts_with("mediaremote-adapter")
                .then(|| entry.path())
        })
        .collect()
}
