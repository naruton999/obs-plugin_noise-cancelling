use crate::DenoiseParams;

/// A hop-based 48 kHz denoising backend.
///
/// The backend receives exactly one hop of mono audio and returns one hop of
/// enhanced audio. Its internal latency must be reported in samples so the
/// streaming wrapper can align the dry signal.
pub trait Backend {
    fn sample_rate(&self) -> u32;
    fn hop_size(&self) -> usize;
    fn latency_samples(&self) -> usize;
    fn set_params(&mut self, params: &DenoiseParams);
    fn process_hop(&mut self, input: &[f32], output: &mut [f32]) -> anyhow::Result<()>;
    fn reset(&mut self) -> anyhow::Result<()>;
}
