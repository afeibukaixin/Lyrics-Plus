use std::collections::VecDeque;
use std::sync::Arc;

use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};

use super::input::{DEFAULT_SAMPLE_RATE, FFT_SIZE};
use super::model::VISUAL_BAR_COUNT;

const INTERNAL_BAND_COUNT: usize = 16;
const MIN_FREQUENCY: f32 = 40.0;
const MAX_FREQUENCY: f32 = 10_000.0;
const NOISE_FLOOR_DB: f32 = -72.0;
const CEILING_DB: f32 = -12.0;
const MIN_DYNAMIC_CEILING_DB: f32 = -36.0;
const CEILING_HEADROOM_DB: f32 = 6.0;
const NORMALIZATION_CURVE: f32 = 0.78;
const SILENCE_THRESHOLD_DB: f32 = NOISE_FLOOR_DB + 1.0;
// 上升快速跟随；动态响度参考缓慢回落，视觉柱则使用独立的更快回缩。
const ATTACK_SMOOTHING: f32 = 0.65;
const RELEASE_SMOOTHING: f32 = 0.18;
// 动态响度参考保持原有回落速度，视觉柱使用更快的下降响应。
const VISUAL_RELEASE_SMOOTHING: f32 = 0.28;
// 瞬态增强只放大 FFT 中实际出现的正向变化，不生成固定轮廓。
const SPECTRAL_FLUX_BOOST: f32 = 0.18;
const KICK_TRANSIENT_BOOST: f32 = 0.30;
const MAX_TRANSIENT_BOOST: f32 = 0.24;
// 衰减到该阈值后直接归零，保证暂停时对外最终是严格的全零。
const SILENCE_EPSILON: f32 = 0.001;

const VISUAL_BAND_RANGES: [(f32, f32); VISUAL_BAR_COUNT] = [
    (40.0, 140.0),
    (100.0, 300.0),
    (250.0, 800.0),
    (600.0, 1_800.0),
    (1_500.0, 4_000.0),
    (3_500.0, 10_000.0),
];

// 高频通常有更少的平均能量；这是频段灵敏度校准，不是柱形造型或对称约束。
const VISUAL_BAND_CALIBRATION_DB: [f32; VISUAL_BAR_COUNT] = [0.0, 0.5, 1.0, 1.5, 2.5, 3.0];

pub(super) struct AudioVisualizerProcessor {
    fft: Arc<dyn Fft<f32>>,
    window: [f32; FFT_SIZE],
    scratch: Vec<Complex32>,
    previous_visual_levels: [f32; VISUAL_BAR_COUNT],
    previous_normalized_levels: [f32; VISUAL_BAR_COUNT],
    adaptive_ceiling_db: f32,
}

impl AudioVisualizerProcessor {
    pub(super) fn new() -> Self {
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
            previous_visual_levels: [0.0; VISUAL_BAR_COUNT],
            previous_normalized_levels: [0.0; VISUAL_BAR_COUNT],
            adaptive_ceiling_db: CEILING_DB,
        }
    }

    pub(super) fn analyze(
        &mut self,
        input: &mut VecDeque<f32>,
        sample_rate: f32,
    ) -> [f32; VISUAL_BAR_COUNT] {
        let sample_rate = if sample_rate.is_finite() && sample_rate > 0.0 {
            sample_rate
        } else {
            DEFAULT_SAMPLE_RATE as f32
        };
        if input.len() < FFT_SIZE {
            return self.process_visual_levels([0.0; VISUAL_BAR_COUNT]);
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

        let internal_edges = self.internal_band_edges(sample_rate);
        let internal_powers = self.internal_band_powers(&internal_edges, sample_rate);
        let visual_powers = self.merge_visual_powers(&internal_edges, &internal_powers);
        let visual_decibels = std::array::from_fn(|index| {
            10.0 * visual_powers[index].max(1e-12).log10() + VISUAL_BAND_CALIBRATION_DB[index]
        });
        self.process_visual_decibels(visual_decibels)
    }

    fn internal_band_edges(&self, sample_rate: f32) -> [f32; INTERNAL_BAND_COUNT + 1] {
        let nyquist = (sample_rate * 0.5).max(MIN_FREQUENCY + 1.0);
        let maximum_frequency = MAX_FREQUENCY.min(nyquist).max(MIN_FREQUENCY + 1.0);
        let ratio = maximum_frequency / MIN_FREQUENCY;
        std::array::from_fn(|index| {
            MIN_FREQUENCY * ratio.powf(index as f32 / INTERNAL_BAND_COUNT as f32)
        })
    }

    fn internal_band_powers(
        &self,
        edges: &[f32; INTERNAL_BAND_COUNT + 1],
        sample_rate: f32,
    ) -> [f32; INTERNAL_BAND_COUNT] {
        let fft_scale = (FFT_SIZE as f32 * 0.5).powi(2);
        std::array::from_fn(|band| {
            let start = ((edges[band] / sample_rate) * FFT_SIZE as f32)
                .floor()
                .max(1.0) as usize;
            let end = ((edges[band + 1] / sample_rate) * FFT_SIZE as f32)
                .ceil()
                .max((start + 1) as f32) as usize;
            let start = start.min(FFT_SIZE / 2);
            let end = end.min(FFT_SIZE / 2 + 1).max(start + 1);
            let values = &self.scratch[start..end];
            let total_power: f32 = values.iter().map(|value| value.norm_sqr()).sum();
            let power = total_power / values.len() as f32 / fft_scale;
            if power.is_finite() {
                power.max(0.0)
            } else {
                0.0
            }
        })
    }

    fn merge_visual_powers(
        &self,
        internal_edges: &[f32; INTERNAL_BAND_COUNT + 1],
        internal_powers: &[f32; INTERNAL_BAND_COUNT],
    ) -> [f32; VISUAL_BAR_COUNT] {
        std::array::from_fn(|visual_bar| {
            let (range_low, range_high) = VISUAL_BAND_RANGES[visual_bar];
            let low = range_low.max(internal_edges[0]);
            let high = range_high.min(internal_edges[INTERNAL_BAND_COUNT]);
            if high <= low {
                return 0.0;
            }

            let mut weighted_power = 0.0;
            let mut total_weight = 0.0;
            for band in 0..INTERNAL_BAND_COUNT {
                let overlap_low = low.max(internal_edges[band]);
                let overlap_high = high.min(internal_edges[band + 1]);
                if overlap_high <= overlap_low {
                    continue;
                }
                // 在 log-frequency 轴上按实际交叠比例加权，避免硬切造成频段跳变。
                let band_width = (internal_edges[band + 1] / internal_edges[band]).ln();
                let overlap = (overlap_high / overlap_low).ln();
                let weight = if band_width > 0.0 {
                    overlap / band_width
                } else {
                    0.0
                };
                weighted_power += internal_powers[band] * weight;
                total_weight += weight;
            }
            if total_weight > 0.0 {
                let power = weighted_power / total_weight;
                if power.is_finite() {
                    power.max(0.0)
                } else {
                    0.0
                }
            } else {
                0.0
            }
        })
    }

    fn process_visual_decibels(
        &mut self,
        decibels: [f32; VISUAL_BAR_COUNT],
    ) -> [f32; VISUAL_BAR_COUNT] {
        let peak_db = decibels
            .iter()
            .copied()
            .filter(|value| value.is_finite())
            .fold(NOISE_FLOOR_DB, f32::max);
        if !peak_db.is_finite() || peak_db <= SILENCE_THRESHOLD_DB {
            return self.process_visual_levels([0.0; VISUAL_BAR_COUNT]);
        }

        let target_ceiling =
            (peak_db + CEILING_HEADROOM_DB).clamp(MIN_DYNAMIC_CEILING_DB, CEILING_DB);
        let ceiling_smoothing = if target_ceiling > self.adaptive_ceiling_db {
            ATTACK_SMOOTHING
        } else {
            RELEASE_SMOOTHING
        };
        self.adaptive_ceiling_db += (target_ceiling - self.adaptive_ceiling_db) * ceiling_smoothing;
        let dynamic_range = (self.adaptive_ceiling_db - NOISE_FLOOR_DB).max(1.0);
        let normalized = std::array::from_fn(|index| {
            let value = ((decibels[index] - NOISE_FLOOR_DB) / dynamic_range).clamp(0.0, 1.0);
            if value.is_finite() {
                value
            } else {
                0.0
            }
        });
        let kick_flux = (normalized[0] - self.previous_normalized_levels[0]).max(0.0);
        let levels = std::array::from_fn(|index| {
            let flux = (normalized[index] - self.previous_normalized_levels[index]).max(0.0);
            let local_boost = (flux * SPECTRAL_FLUX_BOOST).min(MAX_TRANSIENT_BOOST);
            let kick_boost = if index == 0 {
                (kick_flux * KICK_TRANSIENT_BOOST).min(MAX_TRANSIENT_BOOST)
            } else {
                0.0
            };
            (normalized[index].powf(NORMALIZATION_CURVE) + local_boost + kick_boost).min(1.0)
        });
        self.previous_normalized_levels = normalized;
        self.process_visual_levels(levels)
    }

    fn process_visual_levels(&mut self, next: [f32; VISUAL_BAR_COUNT]) -> [f32; VISUAL_BAR_COUNT] {
        let should_reset_flux = next
            .iter()
            .all(|value| !value.is_finite() || *value <= SILENCE_EPSILON);
        for (previous, value) in self
            .previous_visual_levels
            .iter_mut()
            .zip(next.iter().copied())
        {
            let value = if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                0.0
            };
            let smoothing = if value > *previous {
                ATTACK_SMOOTHING
            } else {
                VISUAL_RELEASE_SMOOTHING
            };
            *previous += (value - *previous) * smoothing;
            if (*previous).abs() < SILENCE_EPSILON {
                *previous = 0.0;
            }
        }
        if should_reset_flux {
            self.previous_normalized_levels = [0.0; VISUAL_BAR_COUNT];
        }
        self.previous_visual_levels
    }
}
