use std::collections::VecDeque;

use anyhow::{Result, ensure};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

use crate::backend::Backend;
use crate::{DFN_SAMPLE_RATE, DenoiseParams, Denoiser};

const UP_CHUNK: usize = 441; // 10 ms at 44.1 kHz
const DOWN_CHUNK: usize = 480; // 10 ms at 48 kHz

fn sinc_params() -> SincInterpolationParameters {
    SincInterpolationParameters {
        sinc_len: 64,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 128,
        window: WindowFunction::BlackmanHarris2,
    }
}

/// 44.1 kHz wrapper around the 48 kHz `Denoiser`.
///
/// Input is resampled to 48 kHz, denoised, then resampled back to 44.1 kHz.
/// The output buffer receives a variable number of native-rate samples.
pub struct ResamplingDenoiser {
    denoiser: Denoiser,
    up: SincFixedIn<f32>,
    down: SincFixedIn<f32>,
    up_in: Vec<f32>,
    up_out: Vec<f32>,
    down_in: Vec<f32>,
    down_out: Vec<f32>,
    native_in: VecDeque<f32>,
    to_48k: VecDeque<f32>,
    from_48k: VecDeque<f32>,
    hop_buf: Vec<f32>,
    denoised: Vec<f32>,
    latency: usize,
    errors: u64,
}

impl ResamplingDenoiser {
    pub fn new(backend: Box<dyn Backend>, sample_rate: u32, max_block: usize) -> Result<Self> {
        ensure!(
            sample_rate == 44_100,
            "ResamplingDenoiser requires a 44.1 kHz native rate"
        );
        ensure!(
            backend.sample_rate() == DFN_SAMPLE_RATE,
            "resampling backend must be 48 kHz"
        );

        let denoiser = Denoiser::new(backend, max_block)?;

        let up = SincFixedIn::<f32>::new(
            DFN_SAMPLE_RATE as f64 / 44_100.0,
            1.0,
            sinc_params(),
            UP_CHUNK,
            1,
        )?;
        let down = SincFixedIn::<f32>::new(
            44_100.0 / DFN_SAMPLE_RATE as f64,
            1.0,
            sinc_params(),
            DOWN_CHUNK,
            1,
        )?;

        let up_in = vec![0.0; up.input_frames_next()];
        let up_out = vec![0.0; up.output_frames_max()];
        let down_in = vec![0.0; down.input_frames_next()];
        let down_out = vec![0.0; down.output_frames_max()];

        let up_delay_native =
            (up.output_delay() as f64 * 44_100.0 / DFN_SAMPLE_RATE as f64).round() as usize;
        let dfn_delay_native = (denoiser.latency_samples() as f64 * 44_100.0
            / DFN_SAMPLE_RATE as f64)
            .round() as usize;
        let latency = up_delay_native + dfn_delay_native + down.output_delay();

        let cap = max_block.max(1) + 4 * DOWN_CHUNK + 64;
        let hop = denoiser.hop_size();

        Ok(Self {
            denoiser,
            up,
            down,
            up_in,
            up_out,
            down_in,
            down_out,
            native_in: VecDeque::with_capacity(cap),
            to_48k: VecDeque::with_capacity(cap),
            from_48k: VecDeque::with_capacity(cap),
            hop_buf: vec![0.0; hop],
            denoised: Vec::with_capacity(hop),
            latency,
            errors: 0,
        })
    }

    pub fn set_params(&mut self, params: DenoiseParams) {
        self.denoiser.set_params(params);
    }

    pub fn reset(&mut self) -> Result<()> {
        self.denoiser.reset()?;
        self.up.reset();
        self.down.reset();
        self.native_in.clear();
        self.to_48k.clear();
        self.from_48k.clear();
        self.denoised.clear();
        self.hop_buf.fill(0.0);
        Ok(())
    }

    pub fn latency_samples(&self) -> usize {
        self.latency
    }

    pub fn errors(&self) -> u64 {
        self.errors.saturating_add(self.denoiser.errors())
    }

    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) {
        output.clear();
        if input.is_empty() {
            return;
        }

        for &sample in input {
            self.native_in.push_back(sanitize(sample));
        }

        let up_chunk = self.up_in.len();
        while self.native_in.len() >= up_chunk {
            for slot in &mut self.up_in {
                *slot = self.native_in.pop_front().unwrap_or(0.0);
            }

            let result = {
                let Self {
                    up, up_in, up_out, ..
                } = self;
                up.process_into_buffer(&[&up_in[..]], &mut [&mut up_out[..]], None)
            };

            match result {
                Ok((_, produced)) => {
                    self.to_48k.extend(self.up_out[..produced].iter().copied());
                }
                Err(_) => {
                    self.errors = self.errors.saturating_add(1);
                    break;
                }
            }
        }

        let hop = self.hop_buf.len();
        while self.to_48k.len() >= hop {
            for slot in &mut self.hop_buf {
                *slot = self.to_48k.pop_front().unwrap_or(0.0);
            }

            {
                let Self {
                    denoiser,
                    hop_buf,
                    denoised,
                    from_48k,
                    ..
                } = self;
                denoiser.process(hop_buf, denoised);
                from_48k.extend(denoised.iter().copied());
            }
        }

        let down_chunk = self.down_in.len();
        while self.from_48k.len() >= down_chunk {
            for slot in &mut self.down_in {
                *slot = self.from_48k.pop_front().unwrap_or(0.0);
            }

            let result = {
                let Self {
                    down,
                    down_in,
                    down_out,
                    ..
                } = self;
                down.process_into_buffer(&[&down_in[..]], &mut [&mut down_out[..]], None)
            };

            match result {
                Ok((_, produced)) => {
                    output.extend(self.down_out[..produced].iter().copied().map(sanitize));
                }
                Err(_) => {
                    self.errors = self.errors.saturating_add(1);
                    break;
                }
            }
        }
    }
}

#[inline]
fn sanitize(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}
