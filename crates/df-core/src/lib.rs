//! OBS-independent DeepFilterNet noise suppression core.
//!
//! The engine consumes native-rate mono blocks and appends processed samples to
//! an output buffer. The OBS binding owns the worker thread, ring buffers and
//! stereo reconstruction.

pub mod backend;
mod declick;
pub mod dfn;
pub mod preset;
mod resample;
pub mod stereo;

use std::collections::VecDeque;

use anyhow::{Result, ensure};

use backend::Backend;

pub use declick::Declicker;
pub use dfn::DfnBackend;
pub use preset::{Preset, parse_preset};
pub use resample::ResamplingDenoiser;
pub use stereo::{DelayLine, decode, mid, side};

pub const DFN_SAMPLE_RATE: u32 = 48_000;
pub const SUPPORTED_SAMPLE_RATES: [u32; 2] = [44_100, 48_000];
pub const DFN_HOP_SIZE: usize = 480;
pub const DEFAULT_WORKER_HOPS: usize = 2;
pub const LOW_LATENCY_WORKER_HOPS: usize = 1;
pub const MAX_BLOCK_FRAMES: usize = 4096;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModelKind {
    #[default]
    LowLatency,
    Standard,
}

impl ModelKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::LowLatency => "ll",
            Self::Standard => "standard",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value {
            "ll" | "low-latency" | "low_latency" | "lowlatency" => Some(Self::LowLatency),
            "standard" | "std" => Some(Self::Standard),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::LowLatency => "LL（低遅延）",
            Self::Standard => "標準（高品質）",
        }
    }

    pub fn file_name(self) -> &'static str {
        match self {
            Self::LowLatency => "DeepFilterNet3_ll_onnx.tar.gz",
            Self::Standard => "DeepFilterNet3_onnx.tar.gz",
        }
    }
}

/// Whether the selected model is compiled into the binary.
pub fn model_embedded(kind: ModelKind) -> bool {
    match kind {
        ModelKind::LowLatency => cfg!(feature = "embed-ll"),
        ModelKind::Standard => cfg!(feature = "embed-standard"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DenoiseParams {
    pub enabled: bool,
    /// 0..100 dB. Values below 0.01 are effectively a bypass.
    pub attenuation_limit_db: f32,
    pub post_filter_enabled: bool,
    /// 0..0.05.
    pub post_filter_beta: f32,
    /// 0..1.
    pub mix: f32,
    pub min_db_thresh: f32,
    pub max_db_erb_thresh: f32,
    pub max_db_df_thresh: f32,
    pub bypass: bool,
    /// Lightweight transient suppression (keyboard clicks).
    pub declick: bool,
    /// Additional dry blend when the backend detects voice (0..0.5).
    pub voice_protect: f32,
}

impl Default for DenoiseParams {
    fn default() -> Self {
        Self {
            enabled: true,
            attenuation_limit_db: 100.0,
            post_filter_enabled: false,
            post_filter_beta: 0.0,
            mix: 1.0,
            min_db_thresh: -10.0,
            max_db_erb_thresh: 30.0,
            max_db_df_thresh: 20.0,
            bypass: false,
            declick: false,
            voice_protect: 0.0,
        }
    }
}

impl DenoiseParams {
    pub fn sanitized(mut self) -> Self {
        self.attenuation_limit_db = clamp_finite(self.attenuation_limit_db, 0.0, 100.0, 100.0);
        self.post_filter_beta = clamp_finite(self.post_filter_beta, 0.0, 0.05, 0.0);
        self.mix = clamp_finite(self.mix, 0.0, 1.0, 1.0);
        self.min_db_thresh = clamp_finite(self.min_db_thresh, -15.0, 35.0, -10.0);
        self.max_db_erb_thresh = clamp_finite(self.max_db_erb_thresh, -15.0, 35.0, 30.0);
        self.max_db_df_thresh = clamp_finite(self.max_db_df_thresh, -15.0, 35.0, 20.0);
        self.voice_protect = clamp_finite(self.voice_protect, 0.0, 0.5, 0.0);
        if !self.post_filter_enabled {
            self.post_filter_beta = 0.0;
        }
        self
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EngineConfig {
    pub sample_rate: u32,
}

#[derive(Clone, Copy)]
struct Smoother {
    current: f32,
    target: f32,
    step: f32,
    remaining: u32,
}

impl Smoother {
    fn new(value: f32) -> Self {
        Self {
            current: value,
            target: value,
            step: 0.0,
            remaining: 0,
        }
    }

    fn reset(&mut self, value: f32) {
        self.current = value;
        self.target = value;
        self.step = 0.0;
        self.remaining = 0;
    }

    fn set_target(&mut self, target: f32, samples: usize) {
        // Do not restart an in-progress ramp when the target is unchanged.
        // This matters for the per-hop voice-protect target.
        if (target - self.target).abs() < 1e-6 && self.remaining > 0 {
            return;
        }
        self.target = target;
        if samples == 0 {
            self.current = target;
            self.step = 0.0;
            self.remaining = 0;
        } else {
            self.step = (target - self.current) / samples as f32;
            self.remaining = samples as u32;
        }
    }

    #[inline]
    fn next(&mut self) -> f32 {
        if self.remaining > 0 {
            self.current += self.step;
            self.remaining -= 1;
            if self.remaining == 0 {
                self.current = self.target;
            }
        }
        self.current
    }

    fn advance(&mut self, samples: usize) -> f32 {
        for _ in 0..samples {
            self.next();
        }
        self.current
    }
}

/// Streaming 48 kHz denoiser.
///
/// `process` appends processed samples to `output`; the number of output samples
/// can differ from the input because the backend buffers full hops. The caller
/// (worker or offline renderer) owns the output buffer.
pub struct Denoiser {
    backend: Box<dyn Backend>,
    hop: usize,
    latency: usize,
    params: DenoiseParams,
    active_params: DenoiseParams,
    mix: Smoother,
    atten: Smoother,
    protect: Smoother,
    declicker: Declicker,
    in_fifo: VecDeque<f32>,
    dry_fifo: VecDeque<f32>,
    noisy: Vec<f32>,
    enhanced: Vec<f32>,
    errors: u64,
    was_enabled: bool,
}

impl Denoiser {
    pub fn new(backend: Box<dyn Backend>, max_block: usize) -> Result<Self> {
        ensure!(
            backend.sample_rate() == DFN_SAMPLE_RATE,
            "Denoiser requires a 48 kHz backend"
        );
        let hop = backend.hop_size();
        ensure!(hop > 0, "backend hop size must be positive");
        let latency = backend.latency_samples();
        let cap = max_block.max(1) + 4 * hop + latency + 8;

        let mut dry_fifo = VecDeque::with_capacity(cap);
        dry_fifo.extend(std::iter::repeat_n(0.0, latency));

        let params = DenoiseParams::default();
        let mut backend = backend;
        backend.set_params(&params);

        Ok(Self {
            backend,
            hop,
            latency,
            params,
            active_params: params,
            mix: Smoother::new(1.0),
            atten: Smoother::new(params.attenuation_limit_db),
            protect: Smoother::new(0.0),
            declicker: Declicker::new(),
            in_fifo: VecDeque::with_capacity(cap),
            dry_fifo,
            noisy: vec![0.0; hop],
            enhanced: vec![0.0; hop],
            errors: 0,
            was_enabled: true,
        })
    }

    pub fn hop_size(&self) -> usize {
        self.hop
    }

    pub fn latency_samples(&self) -> usize {
        self.latency
    }

    pub fn errors(&self) -> u64 {
        self.errors
    }

    pub fn set_params(&mut self, params: DenoiseParams) {
        let params = params.sanitized();
        if params == self.params {
            return;
        }
        if params.declick != self.params.declick {
            self.declicker.reset();
        }
        let mix_target = if params.bypass { 0.0 } else { params.mix };
        self.mix.set_target(mix_target, self.hop * 2);
        self.atten
            .set_target(params.attenuation_limit_db, self.hop * 2);
        self.params = params;
        self.active_params = params;
        self.active_params.attenuation_limit_db = self.atten.current;
        if !params.enabled {
            self.was_enabled = false;
        }
    }

    pub fn reset(&mut self) -> Result<()> {
        self.clear_fifos();
        self.declicker.reset();
        self.backend.reset()?;
        self.reset_smoothers();
        Ok(())
    }

    /// Process a mono block. `output` is cleared first and receives the
    /// processed samples. The output length is usually the input length but
    /// can be zero for very small blocks or one hop larger when a backlog is
    /// flushed.
    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) {
        output.clear();
        if input.is_empty() {
            return;
        }

        if !self.params.enabled {
            if self.was_enabled {
                self.reset_internal();
                self.was_enabled = false;
            }
            output.extend(input.iter().copied().map(sanitize));
            return;
        }

        if !self.was_enabled {
            self.reset_internal();
            self.was_enabled = true;
        }

        for &sample in input {
            let raw = sanitize(sample);
            let sample = if self.params.declick {
                self.declicker.process(raw)
            } else {
                raw
            };
            self.in_fifo.push_back(sample);
            self.dry_fifo.push_back(sample);
        }

        while self.in_fifo.len() >= self.hop {
            for i in 0..self.hop {
                self.noisy[i] = self.in_fifo.pop_front().unwrap_or(0.0);
            }

            self.active_params.attenuation_limit_db = self.atten.advance(self.hop);
            self.backend.set_params(&self.active_params);

            let (enhanced, lsnr) = match self.backend.process_hop(&self.noisy, &mut self.enhanced) {
                Ok(lsnr) => (true, lsnr),
                Err(_) => {
                    self.errors = self.errors.saturating_add(1);
                    (false, -15.0)
                }
            };

            // DeepFilterNet's local SNR is high for voiced sounds. When the
            // user has voice protect enabled, blend some dry signal back so
            // sustained sounds such as "ん〜" are not over-suppressed.
            let protect_target = if self.params.voice_protect > 0.0 && lsnr > 0.0 {
                self.params.voice_protect
            } else {
                0.0
            };
            let protect_ramp = if protect_target > self.protect.target {
                (self.hop / 2).max(1) // ~5 ms attack
            } else {
                (self.hop * 5).max(1) // ~50 ms release
            };
            self.protect.set_target(protect_target, protect_ramp);

            for i in 0..self.hop {
                let wet = if enhanced {
                    self.enhanced[i]
                } else {
                    self.noisy[i]
                };
                let dry = self.dry_fifo.pop_front().unwrap_or(0.0);
                let mix = self.mix.next();
                let protect = self.protect.next();
                let wet_gain = mix * (1.0 - protect);
                let dry_gain = 1.0 - wet_gain;
                output.push(sanitize(wet_gain * wet + dry_gain * dry));
            }
        }
    }

    fn mix_target(&self) -> f32 {
        if self.params.bypass {
            0.0
        } else {
            self.params.mix
        }
    }

    fn reset_smoothers(&mut self) {
        self.mix.reset(self.mix_target());
        self.atten.reset(self.params.attenuation_limit_db);
        self.protect.reset(0.0);
        self.active_params = self.params;
        self.active_params.attenuation_limit_db = self.atten.current;
    }

    fn clear_fifos(&mut self) {
        self.in_fifo.clear();
        self.dry_fifo.clear();
        self.dry_fifo.extend(std::iter::repeat_n(0.0, self.latency));
    }

    fn reset_internal(&mut self) {
        self.clear_fifos();
        self.declicker.reset();
        if self.backend.reset().is_err() {
            self.errors = self.errors.saturating_add(1);
        }
        self.reset_smoothers();
    }
}

/// Native-rate engine that selects the 48 kHz path or the 44.1 kHz resampling
/// path.
pub enum Engine {
    Native(Box<Denoiser>),
    Resampled(Box<ResamplingDenoiser>),
}

impl Engine {
    pub fn new(backend: Box<dyn Backend>, sample_rate: u32, max_block: usize) -> Result<Self> {
        match sample_rate {
            DFN_SAMPLE_RATE => Ok(Self::Native(Box::new(Denoiser::new(backend, max_block)?))),
            44_100 => Ok(Self::Resampled(Box::new(ResamplingDenoiser::new(
                backend,
                sample_rate,
                max_block,
            )?))),
            other => anyhow::bail!("unsupported sample rate: {other}"),
        }
    }

    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) {
        match self {
            Self::Native(denoiser) => denoiser.process(input, output),
            Self::Resampled(denoiser) => denoiser.process(input, output),
        }
    }

    pub fn set_params(&mut self, params: DenoiseParams) {
        match self {
            Self::Native(denoiser) => denoiser.set_params(params),
            Self::Resampled(denoiser) => denoiser.set_params(params),
        }
    }

    pub fn reset(&mut self) -> Result<()> {
        match self {
            Self::Native(denoiser) => denoiser.reset(),
            Self::Resampled(denoiser) => denoiser.reset(),
        }
    }

    pub fn latency_samples(&self) -> usize {
        match self {
            Self::Native(denoiser) => denoiser.latency_samples(),
            Self::Resampled(denoiser) => denoiser.latency_samples(),
        }
    }

    pub fn errors(&self) -> u64 {
        match self {
            Self::Native(denoiser) => denoiser.errors(),
            Self::Resampled(denoiser) => denoiser.errors(),
        }
    }
}

#[inline]
fn sanitize(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

fn clamp_finite(value: f32, min: f32, max: f32, default: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockBackend {
        hop: usize,
        latency_hops: usize,
        queue: VecDeque<Vec<f32>>,
        params: DenoiseParams,
        suppress: bool,
        lsnr: f32,
    }

    impl MockBackend {
        fn new() -> Self {
            Self {
                hop: 8,
                latency_hops: 2,
                queue: VecDeque::new(),
                params: DenoiseParams::default(),
                suppress: false,
                lsnr: 0.0,
            }
        }
    }

    impl Backend for MockBackend {
        fn sample_rate(&self) -> u32 {
            DFN_SAMPLE_RATE
        }

        fn hop_size(&self) -> usize {
            self.hop
        }

        fn latency_samples(&self) -> usize {
            self.hop * self.latency_hops
        }

        fn set_params(&mut self, params: &DenoiseParams) {
            self.params = *params;
        }

        fn process_hop(&mut self, input: &[f32], output: &mut [f32]) -> Result<f32> {
            assert_eq!(input.len(), self.hop);
            assert_eq!(output.len(), self.hop);
            if self.suppress {
                output.fill(0.0);
                return Ok(self.lsnr);
            }
            self.queue.push_back(input.to_vec());
            if self.queue.len() > self.latency_hops {
                let delayed = self.queue.pop_front().unwrap();
                output.copy_from_slice(&delayed);
            } else {
                output.fill(0.0);
            }
            Ok(self.lsnr)
        }

        fn reset(&mut self) -> Result<()> {
            self.queue.clear();
            Ok(())
        }
    }

    fn process_block(denoiser: &mut Denoiser, input: &[f32]) -> Vec<f32> {
        let mut output = Vec::with_capacity(input.len() + 64);
        denoiser.process(input, &mut output);
        output
    }

    #[test]
    fn model_kind_ids() {
        assert_eq!(ModelKind::from_id("ll"), Some(ModelKind::LowLatency));
        assert_eq!(ModelKind::from_id("standard"), Some(ModelKind::Standard));
        assert_eq!(ModelKind::LowLatency.id(), "ll");
        assert_eq!(ModelKind::Standard.id(), "standard");
        assert_eq!(ModelKind::from_id("unknown"), None);
    }

    #[test]
    fn embedded_flags_follow_features() {
        assert_eq!(
            model_embedded(ModelKind::LowLatency),
            cfg!(feature = "embed-ll")
        );
        assert_eq!(
            model_embedded(ModelKind::Standard),
            cfg!(feature = "embed-standard")
        );
    }

    #[test]
    fn requires_supported_rate() {
        let backend = MockBackend::new();
        assert!(Engine::new(Box::new(backend), 96_000, 1024).is_err());

        let backend = MockBackend::new();
        assert!(Engine::new(Box::new(backend), 48_000, 1024).is_ok());

        let backend = MockBackend::new();
        assert!(Engine::new(Box::new(backend), 44_100, 1024).is_ok());
    }

    #[test]
    fn latency_is_stable() {
        let backend = MockBackend::new();
        let latency = backend.latency_samples();
        let mut denoiser = Denoiser::new(Box::new(backend), 1024).unwrap();

        let mut input = vec![0.0f32; 256];
        input[0] = 1.0;
        let output = process_block(&mut denoiser, &input);

        let peak = output
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .map(|(i, _)| i)
            .unwrap();
        assert_eq!(peak, latency);
    }

    #[test]
    fn enabled_is_immediate_passthrough() {
        let mut denoiser = Denoiser::new(Box::new(MockBackend::new()), 1024).unwrap();
        denoiser.set_params(DenoiseParams {
            enabled: false,
            ..DenoiseParams::default()
        });

        let input: Vec<f32> = (0..64).map(|i| i as f32 * 0.01).collect();
        let output = process_block(&mut denoiser, &input);
        assert_eq!(output, input);
    }

    #[test]
    fn voice_protect_blends_dry() {
        let mut backend = MockBackend::new();
        backend.suppress = true;
        backend.lsnr = 10.0;
        let mut denoiser = Denoiser::new(Box::new(backend), 1024).unwrap();
        denoiser.set_params(DenoiseParams {
            voice_protect: 0.5,
            ..DenoiseParams::default()
        });

        let input: Vec<f32> = (0..512)
            .map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / 48_000.0).sin() * 0.5)
            .collect();
        let output = process_block(&mut denoiser, &input);
        let tail = &output[64..];
        let rms = (tail.iter().map(|x| x * x).sum::<f32>() / tail.len() as f32).sqrt();
        assert!(rms > 0.05, "dry blend was not audible: rms={rms}");
    }

    #[test]
    fn bypass_is_delayed_dry() {
        let backend = MockBackend::new();
        let latency = backend.latency_samples();
        let mut denoiser = Denoiser::new(Box::new(backend), 1024).unwrap();
        denoiser.set_params(DenoiseParams {
            bypass: true,
            ..DenoiseParams::default()
        });

        let warmup = vec![0.0f32; latency * 2];
        let _ = process_block(&mut denoiser, &warmup);

        let mut input = vec![0.0f32; 128];
        input[0] = 1.0;
        let output = process_block(&mut denoiser, &input);
        let peak = output
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .map(|(i, _)| i)
            .unwrap();
        assert_eq!(peak, latency);
    }

    #[test]
    fn mix_zero_is_delayed_dry() {
        let backend = MockBackend::new();
        let latency = backend.latency_samples();
        let mut denoiser = Denoiser::new(Box::new(backend), 1024).unwrap();
        denoiser.set_params(DenoiseParams {
            mix: 0.0,
            ..DenoiseParams::default()
        });

        let warmup = vec![0.0f32; latency * 2];
        let _ = process_block(&mut denoiser, &warmup);

        let mut input = vec![0.0f32; 128];
        input[0] = 1.0;
        let output = process_block(&mut denoiser, &input);
        let peak = output
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .map(|(i, _)| i)
            .unwrap();
        assert_eq!(peak, latency);
    }

    #[test]
    fn block_size_independent() {
        let input: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.03).sin() * 0.5).collect();

        let mut a = Denoiser::new(Box::new(MockBackend::new()), 2048).unwrap();
        let mut out_a = Vec::new();
        for chunk in input.chunks(8) {
            out_a.extend(process_block(&mut a, chunk));
        }

        let mut b = Denoiser::new(Box::new(MockBackend::new()), 2048).unwrap();
        let mut out_b = Vec::new();
        for chunk in input.chunks(37) {
            out_b.extend(process_block(&mut b, chunk));
        }

        assert_eq!(out_a.len(), out_b.len());
        for (x, y) in out_a.iter().zip(out_b.iter()) {
            assert!((x - y).abs() < 1e-6, "{x} vs {y}");
        }
    }

    #[test]
    fn nan_input_is_sanitized() {
        let mut denoiser = Denoiser::new(Box::new(MockBackend::new()), 1024).unwrap();
        let input = [f32::NAN, f32::INFINITY, 0.5, f32::NEG_INFINITY];
        let output = process_block(&mut denoiser, &input);
        assert!(output.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn reset_clears_state() {
        let mut denoiser = Denoiser::new(Box::new(MockBackend::new()), 1024).unwrap();
        let input = vec![1.0f32; 64];
        let _ = process_block(&mut denoiser, &input);
        denoiser.reset().unwrap();

        let silence = vec![0.0f32; 256];
        let output = process_block(&mut denoiser, &silence);
        assert!(output.iter().all(|v| v.abs() < 1e-9));
    }

    #[test]
    fn resample_44k_roundtrip() {
        let backend = MockBackend::new();
        let mut engine = Engine::new(Box::new(backend), 44_100, 2048).unwrap();
        engine.set_params(DenoiseParams::default());

        let mut input = Vec::with_capacity(44_100);
        for i in 0..44_100 {
            input.push((i as f32 * 440.0 * std::f32::consts::TAU / 44_100.0).sin() * 0.25);
        }

        let mut output = Vec::new();
        for chunk in input.chunks(1024) {
            let mut block_out = Vec::new();
            engine.process(chunk, &mut block_out);
            output.extend(block_out);
        }

        assert!(!output.is_empty());
        assert!(output.iter().all(|v| v.is_finite()));
        assert!(output.iter().any(|v| v.abs() > 0.01));
        assert!(engine.latency_samples() > 0);
    }
}
