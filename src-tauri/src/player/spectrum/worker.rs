use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use tauri::{AppHandle, Emitter};

use super::super::now_ms;
use super::audio_tap::NativeTap;
use super::dsp::AudioVisualizerProcessor;
use super::input::{SpectrumInput, FFT_SIZE};
use super::model::{PlaybackSpectrumFrame, PlaybackSpectrumState};
use super::{FRAME_INTERVAL, PLAYBACK_SPECTRUM_FRAME_EVENT};

pub(super) struct WorkerHandle {
    pub(super) stop: Arc<AtomicBool>,
    pub(super) input: Arc<SpectrumInput>,
    pub(super) thread: Option<JoinHandle<()>>,
}

impl WorkerHandle {
    pub(super) fn stop(mut self) {
        self.stop.store(true, Ordering::Release);
        self.input.close();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(super) struct RuntimeState {
    pub(super) state: PlaybackSpectrumState,
    pub(super) target_bundle_id: Option<String>,
    pub(super) tap: Option<NativeTap>,
    pub(super) worker: Option<WorkerHandle>,
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

pub(super) fn run_spectrum_worker(
    app: AppHandle,
    subscribers: Arc<Mutex<std::collections::HashSet<String>>>,
    input: Arc<SpectrumInput>,
    stop: Arc<AtomicBool>,
    source_app_bundle_id: String,
) {
    let mut analyzer = AudioVisualizerProcessor::new();
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
