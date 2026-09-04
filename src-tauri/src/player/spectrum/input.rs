use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

pub(super) const FFT_SIZE: usize = 2048;
pub(super) const MAX_INPUT_SAMPLES: usize = FFT_SIZE * 8;
pub(super) const DEFAULT_SAMPLE_RATE: f64 = 48_000.0;

pub(super) struct SpectrumInput {
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
    pub(super) fn push(&self, samples: &[f32], sample_rate: f64) {
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

    pub(super) fn wait_for_data(&self, timeout: Duration) {
        let Ok(buffer) = self.samples.lock() else {
            return;
        };
        if buffer.is_empty() && !self.closed.load(Ordering::Acquire) {
            let _ = self.wake.wait_timeout(buffer, timeout);
        }
    }

    pub(super) fn drain_into(&self, destination: &mut VecDeque<f32>) {
        let Ok(mut buffer) = self.samples.lock() else {
            return;
        };
        while let Some(sample) = buffer.pop_front() {
            destination.push_back(sample);
        }
    }

    pub(super) fn sample_rate(&self) -> f32 {
        let value = f64::from_bits(self.sample_rate_bits.load(Ordering::Acquire));
        if value.is_finite() && value > 0.0 {
            value as f32
        } else {
            DEFAULT_SAMPLE_RATE as f32
        }
    }

    pub(super) fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.wake.notify_all();
    }
}
