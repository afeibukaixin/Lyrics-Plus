use std::collections::VecDeque;
#[cfg(target_os = "macos")]
use std::ffi::{c_char, c_void, CString};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::{now_ms, PlaybackSnapshot, PlayerKind};

pub const PLAYBACK_SPECTRUM_FRAME_EVENT: &str = "playback://spectrum-frame";
pub const PLAYBACK_SPECTRUM_STATE_EVENT: &str = "playback://spectrum-state";

// 频谱对外固定为 16 段，按低频到高频的对数区间排列。
const BAND_COUNT: usize = 16;
const FFT_SIZE: usize = 2048;
const MAX_INPUT_SAMPLES: usize = FFT_SIZE * 8;
// 约 30 FPS，给动画留出足够的平滑空间，同时避免事件过于密集。
const FRAME_INTERVAL: Duration = Duration::from_millis(33);
const DEFAULT_SAMPLE_RATE: f64 = 48_000.0;
const MIN_FREQUENCY: f32 = 60.0;
const MAX_FREQUENCY: f32 = 16_000.0;
const NOISE_FLOOR_DB: f32 = -72.0;
const CEILING_DB: f32 = -12.0;
// 上升快速跟随、下降缓慢回落，便于做视觉上的音频跳动。
const ATTACK_SMOOTHING: f32 = 0.65;
const RELEASE_SMOOTHING: f32 = 0.18;
// 衰减到该阈值后直接归零，保证暂停时对外最终是严格的全零。
const SILENCE_EPSILON: f32 = 0.001;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackSpectrumStatus {
    Idle,
    Waiting,
    Starting,
    Running,
    PermissionDenied,
    Unsupported,
    Unavailable,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSpectrumState {
    pub status: PlaybackSpectrumStatus,
    pub source_app_bundle_id: Option<String>,
    pub error: Option<String>,
}

impl Default for PlaybackSpectrumState {
    fn default() -> Self {
        Self {
            status: PlaybackSpectrumStatus::Idle,
            source_app_bundle_id: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSpectrumFrame {
    pub bands: [f32; BAND_COUNT],
    pub source_app_bundle_id: Option<String>,
    pub observed_at_ms: u64,
}

impl PlaybackSpectrumFrame {
    fn silent(source_app_bundle_id: Option<String>) -> Self {
        Self {
            bands: [0.0; BAND_COUNT],
            source_app_bundle_id,
            observed_at_ms: now_ms(),
        }
    }
}

struct SpectrumInput {
    samples: Mutex<VecDeque<f32>>,
    sample_rate_bits: AtomicU64,
    wake: Condvar,
    closed: AtomicBool,
}

impl Default for SpectrumInput {
    fn default() -> Self {
        Self {
            // 预先分配固定容量，避免在 Core Audio 回调线程扩容。
            samples: Mutex::new(VecDeque::with_capacity(MAX_INPUT_SAMPLES)),
            sample_rate_bits: AtomicU64::new(DEFAULT_SAMPLE_RATE.to_bits()),
            wake: Condvar::new(),
            closed: AtomicBool::new(false),
        }
    }
}

impl SpectrumInput {
    fn push(&self, samples: &[f32], sample_rate: f64) {
        if self.closed.load(Ordering::Acquire) || samples.is_empty() {
            return;
        }
        self.sample_rate_bits
            .store(sample_rate.to_bits(), Ordering::Release);
        let Ok(mut buffer) = self.samples.try_lock() else {
            // 音频回调不能阻塞；下一次 IO 周期会继续提供样本。
            return;
        };
        for &raw_sample in samples {
            if buffer.len() >= MAX_INPUT_SAMPLES {
                buffer.pop_front();
            }
            buffer.push_back(if raw_sample.is_finite() {
                raw_sample
            } else {
                0.0
            });
        }
        drop(buffer);
        self.wake.notify_one();
    }

    fn wait_for_data(&self, timeout: Duration) {
        let Ok(buffer) = self.samples.lock() else {
            return;
        };
        if buffer.is_empty() && !self.closed.load(Ordering::Acquire) {
            let _ = self.wake.wait_timeout(buffer, timeout);
        }
    }

    fn drain_into(&self, destination: &mut VecDeque<f32>) {
        let Ok(mut buffer) = self.samples.lock() else {
            return;
        };
        while let Some(sample) = buffer.pop_front() {
            destination.push_back(sample);
        }
    }

    fn sample_rate(&self) -> f32 {
        let value = f64::from_bits(self.sample_rate_bits.load(Ordering::Acquire));
        if value.is_finite() && value > 0.0 {
            value as f32
        } else {
            DEFAULT_SAMPLE_RATE as f32
        }
    }

    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.wake.notify_all();
    }
}

struct SpectrumAnalyzer {
    fft: Arc<dyn Fft<f32>>,
    window: [f32; FFT_SIZE],
    scratch: Vec<Complex32>,
    previous_bands: [f32; BAND_COUNT],
}

impl SpectrumAnalyzer {
    fn new() -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let window = std::array::from_fn(|index| {
            let phase = std::f32::consts::PI * 2.0 * index as f32 / FFT_SIZE as f32;
            0.5 - 0.5 * phase.cos()
        });
        Self {
            fft,
            window,
            scratch: vec![Complex32::new(0.0, 0.0); FFT_SIZE],
            previous_bands: [0.0; BAND_COUNT],
        }
    }

    fn analyze(&mut self, input: &mut VecDeque<f32>, sample_rate: f32) -> [f32; BAND_COUNT] {
        if input.len() < FFT_SIZE {
            return self.smooth([0.0; BAND_COUNT]);
        }

        let skip = input.len().saturating_sub(FFT_SIZE);
        for _ in 0..skip {
            input.pop_front();
        }
        for (index, value) in input.iter().take(FFT_SIZE).enumerate() {
            self.scratch[index] = Complex32::new(*value * self.window[index], 0.0);
        }
        while input.len() > FFT_SIZE / 2 {
            input.pop_front();
        }
        self.fft.process(&mut self.scratch);

        let nyquist = (sample_rate * 0.5).max(MIN_FREQUENCY);
        let maximum_frequency = MAX_FREQUENCY.min(nyquist).max(MIN_FREQUENCY + 1.0);
        let ratio = maximum_frequency / MIN_FREQUENCY;
        let mut bands = [0.0; BAND_COUNT];
        for band in 0..BAND_COUNT {
            let low = MIN_FREQUENCY * ratio.powf(band as f32 / BAND_COUNT as f32);
            let high = MIN_FREQUENCY * ratio.powf((band + 1) as f32 / BAND_COUNT as f32);
            let start = ((low / sample_rate) * FFT_SIZE as f32).floor() as usize;
            let end = ((high / sample_rate) * FFT_SIZE as f32).ceil() as usize;
            let start = start.max(1).min(FFT_SIZE / 2);
            let end = end.max(start + 1).min(FFT_SIZE / 2 + 1);
            let mut total = 0.0;
            let mut count = 0usize;
            for value in &self.scratch[start..end] {
                total += value.norm();
                count += 1;
            }
            let magnitude = if count == 0 {
                0.0
            } else {
                total / count as f32 / (FFT_SIZE as f32 * 0.5)
            };
            let decibels = 20.0 * magnitude.max(1e-6).log10();
            bands[band] =
                ((decibels - NOISE_FLOOR_DB) / (CEILING_DB - NOISE_FLOOR_DB)).clamp(0.0, 1.0);
        }
        self.smooth(bands)
    }

    fn smooth(&mut self, next: [f32; BAND_COUNT]) -> [f32; BAND_COUNT] {
        for (previous, value) in self.previous_bands.iter_mut().zip(next) {
            let smoothing = if value > *previous {
                ATTACK_SMOOTHING
            } else {
                RELEASE_SMOOTHING
            };
            *previous += (value - *previous) * smoothing;
            if (*previous).abs() < SILENCE_EPSILON {
                *previous = 0.0;
            }
        }
        self.previous_bands
    }
}

#[cfg(target_os = "macos")]
type NativeTapContext = *mut c_void;

#[cfg(target_os = "macos")]
extern "C" {
    fn lyrics_plus_audio_tap_start(
        bundle_id: *const c_char,
        callback: unsafe extern "C" fn(*const f32, u32, f64, *mut c_void),
        context: *mut c_void,
        out_tap: *mut *mut c_void,
    ) -> i32;
    fn lyrics_plus_audio_tap_stop(tap: *mut c_void);
    fn lyrics_plus_audio_tap_matches_bundle(tap: *mut c_void, bundle_id: *const c_char) -> i32;
}

#[cfg(target_os = "macos")]
struct NativeTap {
    opaque: *mut c_void,
    context: NativeTapContext,
    _input: Arc<SpectrumInput>,
}

#[cfg(target_os = "macos")]
unsafe impl Send for NativeTap {}

#[cfg(target_os = "macos")]
impl NativeTap {
    fn start(bundle_id: &str, input: Arc<SpectrumInput>) -> Result<Self, i32> {
        let bundle_id = CString::new(bundle_id).map_err(|_| -10001)?;
        let context = Arc::into_raw(input.clone()) as *mut c_void;
        let mut opaque = std::ptr::null_mut();
        let status = unsafe {
            lyrics_plus_audio_tap_start(bundle_id.as_ptr(), audio_callback, context, &mut opaque)
        };
        if status != 0 || opaque.is_null() {
            unsafe {
                drop(Arc::from_raw(context as *const SpectrumInput));
            }
            return Err(status);
        }
        Ok(Self {
            opaque,
            context,
            _input: input,
        })
    }

    fn matches_bundle(&self, bundle_id: &str) -> bool {
        let Ok(bundle_id) = CString::new(bundle_id) else {
            return false;
        };
        unsafe { lyrics_plus_audio_tap_matches_bundle(self.opaque, bundle_id.as_ptr()) != 0 }
    }
}

#[cfg(target_os = "macos")]
impl Drop for NativeTap {
    fn drop(&mut self) {
        unsafe {
            lyrics_plus_audio_tap_stop(self.opaque);
            drop(Arc::from_raw(self.context as *const SpectrumInput));
        }
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn audio_callback(
    samples: *const f32,
    sample_count: u32,
    sample_rate: f64,
    context: *mut c_void,
) {
    if samples.is_null() || context.is_null() || sample_count == 0 {
        return;
    }
    let input = unsafe { &*(context as *const SpectrumInput) };
    let samples = unsafe { std::slice::from_raw_parts(samples, sample_count as usize) };
    input.push(samples, sample_rate);
}

#[cfg(not(target_os = "macos"))]
struct NativeTap;

struct WorkerHandle {
    stop: Arc<AtomicBool>,
    input: Arc<SpectrumInput>,
    thread: Option<JoinHandle<()>>,
}

impl WorkerHandle {
    fn stop(mut self) {
        self.stop.store(true, Ordering::Release);
        self.input.close();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct RuntimeState {
    state: PlaybackSpectrumState,
    target_bundle_id: Option<String>,
    tap: Option<NativeTap>,
    worker: Option<WorkerHandle>,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            state: PlaybackSpectrumState::default(),
            target_bundle_id: None,
            tap: None,
            worker: None,
        }
    }
}

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

fn run_spectrum_worker(
    app: AppHandle,
    subscribers: Arc<Mutex<std::collections::HashSet<String>>>,
    input: Arc<SpectrumInput>,
    stop: Arc<AtomicBool>,
    source_app_bundle_id: String,
) {
    let mut analyzer = SpectrumAnalyzer::new();
    let mut pending = VecDeque::with_capacity(FFT_SIZE * 2);
    let mut last_frame_at = Instant::now() - FRAME_INTERVAL;
    while !stop.load(Ordering::Acquire) {
        let wait = FRAME_INTERVAL.saturating_sub(last_frame_at.elapsed());
        input.wait_for_data(wait);
        if stop.load(Ordering::Acquire) {
            break;
        }
        if let Some(remaining) = FRAME_INTERVAL.checked_sub(last_frame_at.elapsed()) {
            thread::sleep(remaining);
        }
        input.drain_into(&mut pending);
        while pending.len() > FFT_SIZE * 4 {
            pending.pop_front();
        }
        let bands = analyzer.analyze(&mut pending, input.sample_rate());
        let frame = PlaybackSpectrumFrame {
            bands,
            source_app_bundle_id: Some(source_app_bundle_id.clone()),
            observed_at_ms: now_ms(),
        };
        let labels = subscribers
            .lock()
            .map(|subscribers| subscribers.iter().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        for label in labels {
            let _ = app.emit_to(label, PLAYBACK_SPECTRUM_FRAME_EVENT, frame.clone());
        }
        last_frame_at = Instant::now();
    }
}

fn spectrum_target_bundle_id(snapshot: &PlaybackSnapshot) -> Option<&str> {
    // 播放中允许元数据尚未到达；暂停时需有曲目，才能保持订阅并等待恢复。
    let has_title = snapshot
        .title
        .as_deref()
        .is_some_and(|title| !title.trim().is_empty());
    if !snapshot.is_running || (!snapshot.is_playing && !has_title) {
        return None;
    }
    match snapshot.player {
        Some(PlayerKind::AppleMusic) => Some("com.apple.Music"),
        Some(PlayerKind::Spotify) => Some("com.spotify.client"),
        Some(PlayerKind::System) => snapshot.source_app_bundle_id.as_deref(),
        None => None,
    }
}

const LYRICS_PLUS_SPECTRUM_UNSUPPORTED: i32 = -10000;
const AUDIO_DEVICE_PERMISSIONS_ERROR: i32 = i32::from_be_bytes(*b"!hog");
