use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;

use tauri::{AppHandle, Emitter};

use super::super::{PlaybackErrorCode, PlaybackSnapshot};
use super::audio_tap::{
    NativeTap, AUDIO_DEVICE_PERMISSIONS_ERROR, LYRICS_PLUS_SPECTRUM_UNSUPPORTED,
};
use super::input::SpectrumInput;
use super::model::{
    spectrum_target_bundle_id, PlaybackSpectrumFrame, PlaybackSpectrumState, PlaybackSpectrumStatus,
};
use super::worker::{run_spectrum_worker, RuntimeState, WorkerHandle};
use super::{PLAYBACK_SPECTRUM_FRAME_EVENT, PLAYBACK_SPECTRUM_STATE_EVENT};

pub struct PlaybackSpectrumService {
    subscribers: Arc<Mutex<std::collections::HashSet<String>>>,
    runtime: Mutex<RuntimeState>,
    operation: Mutex<()>,
}

impl Default for PlaybackSpectrumService {
    fn default() -> Self {
        Self {
            subscribers: Arc::new(Mutex::new(std::collections::HashSet::new())),
            runtime: Mutex::new(RuntimeState::default()),
            operation: Mutex::new(()),
        }
    }
}

impl PlaybackSpectrumService {
    pub fn subscribe(
        &self,
        app: &AppHandle,
        window_label: &str,
        snapshot: &PlaybackSnapshot,
    ) -> PlaybackSpectrumState {
        if let Ok(mut subscribers) = self.subscribers.lock() {
            subscribers.insert(window_label.to_string());
        }
        self.sync_snapshot(app, snapshot);
        let state = self.state();
        self.emit_state_to(app, window_label, &state);
        state
    }

    pub fn unsubscribe(&self, app: &AppHandle, window_label: &str) {
        let _operation = self.operation.lock().ok();
        let became_empty = self
            .subscribers
            .lock()
            .map(|mut subscribers| {
                subscribers.remove(window_label);
                subscribers.is_empty()
            })
            .unwrap_or(false);
        if !became_empty {
            return;
        }
        self.stop_capture();
        if let Ok(mut runtime) = self.runtime.lock() {
            runtime.state = PlaybackSpectrumState::default();
            runtime.target_bundle_id = None;
        }
        let _ = app;
    }

    pub fn state(&self) -> PlaybackSpectrumState {
        self.runtime
            .lock()
            .map(|runtime| runtime.state.clone())
            .unwrap_or_else(|_| PlaybackSpectrumState {
                status: PlaybackSpectrumStatus::Unavailable,
                source_app_bundle_id: None,
                error: Some("频谱服务状态不可用".into()),
            })
    }

    pub fn sync_snapshot(&self, app: &AppHandle, snapshot: &PlaybackSnapshot) {
        let Ok(_operation) = self.operation.lock() else {
            return;
        };
        let has_subscribers = self
            .subscribers
            .lock()
            .map(|subscribers| !subscribers.is_empty())
            .unwrap_or(false);
        if !has_subscribers {
            return;
        }

        if snapshot.error_code == Some(PlaybackErrorCode::SourceNotAllowed) {
            let preserve_current_target = self
                .runtime
                .lock()
                .map(|runtime| {
                    runtime.target_bundle_id.as_deref().is_some_and(|target| {
                        snapshot.source_app_bundle_id.as_deref() != Some(target)
                    })
                })
                .unwrap_or(false);
            if preserve_current_target {
                // 被过滤的系统播放器不应打断当前有效播放器的频谱捕获。
                return;
            }
        }

        let target = spectrum_target_bundle_id(snapshot).map(str::to_owned);
        if target.is_none() {
            self.stop_capture();
            self.set_state(
                app,
                PlaybackSpectrumState {
                    status: PlaybackSpectrumStatus::Waiting,
                    source_app_bundle_id: None,
                    error: Some("当前没有可捕获的播放器".into()),
                },
            );
            self.emit_silent_frame(app, None);
            return;
        }
        let target = target.expect("target checked above");

        let target_is_blocked = self
            .runtime
            .lock()
            .map(|runtime| {
                runtime.target_bundle_id.as_deref() == Some(target.as_str())
                    && matches!(
                        runtime.state.status,
                        PlaybackSpectrumStatus::PermissionDenied
                            | PlaybackSpectrumStatus::Unsupported
                    )
            })
            .unwrap_or(false);
        let current_target_is_usable = self
            .runtime
            .lock()
            .map(|runtime| {
                runtime.target_bundle_id.as_deref() == Some(target.as_str())
                    && runtime.tap.as_ref().is_some_and(|tap| {
                        #[cfg(target_os = "macos")]
                        {
                            tap.matches_bundle(&target)
                        }
                        #[cfg(not(target_os = "macos"))]
                        {
                            let _ = tap;
                            false
                        }
                    })
            })
            .unwrap_or(false);
        if current_target_is_usable || target_is_blocked {
            return;
        }

        self.stop_capture();
        self.set_state(
            app,
            PlaybackSpectrumState {
                status: PlaybackSpectrumStatus::Starting,
                source_app_bundle_id: Some(target.clone()),
                error: None,
            },
        );

        let input = Arc::new(SpectrumInput::default());
        #[cfg(target_os = "macos")]
        let tap = match NativeTap::start(&target, input.clone()) {
            Ok(tap) => tap,
            Err(status) => {
                self.set_error_state(app, target, status);
                self.emit_silent_frame(app, None);
                return;
            }
        };
        #[cfg(not(target_os = "macos"))]
        {
            let _ = input;
            self.set_state(
                app,
                PlaybackSpectrumState {
                    status: PlaybackSpectrumStatus::Unsupported,
                    source_app_bundle_id: Some(target),
                    error: Some("频谱捕获仅支持 macOS 14.2 及更高版本".into()),
                },
            );
            self.emit_silent_frame(app, None);
            return;
        }

        #[cfg(target_os = "macos")]
        {
            let stop = Arc::new(AtomicBool::new(false));
            let worker_input = input.clone();
            let worker_stop = stop.clone();
            let app_for_worker = app.clone();
            let subscribers = self.subscribers.clone();
            let source_app_bundle_id = target.clone();
            let thread = thread::spawn(move || {
                run_spectrum_worker(
                    app_for_worker,
                    subscribers,
                    worker_input,
                    worker_stop,
                    source_app_bundle_id,
                )
            });
            if let Ok(mut runtime) = self.runtime.lock() {
                runtime.target_bundle_id = Some(target.clone());
                runtime.tap = Some(tap);
                runtime.worker = Some(WorkerHandle {
                    stop,
                    input,
                    thread: Some(thread),
                });
            } else {
                drop(tap);
                let _ = thread.join();
                return;
            }
            self.set_state(
                app,
                PlaybackSpectrumState {
                    status: PlaybackSpectrumStatus::Running,
                    source_app_bundle_id: Some(target),
                    error: None,
                },
            );
        }
    }

    fn set_state(&self, app: &AppHandle, state: PlaybackSpectrumState) {
        if let Ok(mut runtime) = self.runtime.lock() {
            runtime.state = state.clone();
        }
        self.emit_state(app, &state);
    }

    fn set_error_state(&self, app: &AppHandle, target: String, status: i32) {
        let (state, error) = match status {
            LYRICS_PLUS_SPECTRUM_UNSUPPORTED => (
                PlaybackSpectrumStatus::Unsupported,
                "频谱捕获需要 macOS 14.2 或更高版本".into(),
            ),
            AUDIO_DEVICE_PERMISSIONS_ERROR => (
                PlaybackSpectrumStatus::PermissionDenied,
                "没有系统音频录制权限，请到“系统设置 → 隐私与安全性 → 屏幕与系统音频录制”允许 Lyrics Plus".into(),
            ),
            _ => (
                PlaybackSpectrumStatus::Unavailable,
                format!("无法捕获当前播放器的音频（错误码 {status}）"),
            ),
        };
        if let Ok(mut runtime) = self.runtime.lock() {
            runtime.target_bundle_id = Some(target.clone());
        }
        self.set_state(
            app,
            PlaybackSpectrumState {
                status: state,
                source_app_bundle_id: Some(target),
                error: Some(error),
            },
        );
    }

    fn stop_capture(&self) {
        let (worker, tap) = self
            .runtime
            .lock()
            .map(|mut runtime| {
                runtime.target_bundle_id = None;
                (runtime.worker.take(), runtime.tap.take())
            })
            .unwrap_or((None, None));
        if let Some(worker) = worker {
            worker.stop();
        }
        drop(tap);
    }

    fn emit_state(&self, app: &AppHandle, state: &PlaybackSpectrumState) {
        let labels = self.subscriber_labels();
        for label in labels {
            self.emit_state_to(app, &label, state);
        }
    }

    fn emit_state_to(&self, app: &AppHandle, label: &str, state: &PlaybackSpectrumState) {
        let _ = app.emit_to(label, PLAYBACK_SPECTRUM_STATE_EVENT, state.clone());
    }

    fn emit_silent_frame(&self, app: &AppHandle, source_app_bundle_id: Option<String>) {
        let frame = PlaybackSpectrumFrame::silent(source_app_bundle_id);
        for label in self.subscriber_labels() {
            let _ = app.emit_to(label, PLAYBACK_SPECTRUM_FRAME_EVENT, frame.clone());
        }
    }

    fn subscriber_labels(&self) -> Vec<String> {
        self.subscribers
            .lock()
            .map(|subscribers| subscribers.iter().cloned().collect())
            .unwrap_or_default()
    }
}
