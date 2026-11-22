/// Causal three-point median filter used as a lightweight de-clicker.
///
/// This is intentionally simple: it removes isolated sample spikes (keyboard
/// clicks) while leaving sustained voiced sounds such as "ん〜" mostly intact.
/// It does not delay the signal.
pub struct Declicker {
    history: [f32; 2],
    filled: usize,
    pos: usize,
}

impl Default for Declicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Declicker {
    pub fn new() -> Self {
        Self {
            history: [0.0; 2],
            filled: 0,
            pos: 0,
        }
    }

    pub fn reset(&mut self) {
        self.history = [0.0; 2];
        self.filled = 0;
        self.pos = 0;
    }

    /// Process one sample. The first two samples pass through while the
    /// history fills; after that a three-point median is used.
    #[inline]
    pub fn process(&mut self, sample: f32) -> f32 {
        let sample = if sample.is_finite() { sample } else { 0.0 };

        if self.filled < 2 {
            self.history[self.pos] = sample;
            self.pos = (self.pos + 1) % 2;
            self.filled += 1;
            return sample;
        }

        let mut window = [self.history[0], self.history[1], sample];
        window.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = window[1];

        // Keep the raw history. Feeding the median back would make the filter
        // latch onto one value for monotonic signals and attenuate the whole
        // waveform.
        self.history[self.pos] = sample;
        self.pos = (self.pos + 1) % 2;
        median
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_isolated_impulse() {
        let mut declicker = Declicker::new();
        let mut input = Vec::new();
        for i in 0..256 {
            let phase = i as f32 * 1_000.0 * std::f32::consts::TAU / 48_000.0;
            input.push(phase.sin() * 0.2);
        }
        input[100] = 1.5;

        let output: Vec<f32> = input.iter().map(|&x| declicker.process(x)).collect();

        assert!(
            output[100].abs() < 0.25,
            "impulse remained: {}",
            output[100]
        );
        assert!(output[99].abs() < 0.25);
        assert!(output[101].abs() < 0.25);
    }

    #[test]
    fn preserves_sine() {
        let mut declicker = Declicker::new();
        let mut input = Vec::new();
        for i in 0..480 {
            let phase = i as f32 * 440.0 * std::f32::consts::TAU / 48_000.0;
            input.push(phase.sin() * 0.2);
        }
        let output: Vec<f32> = input.iter().map(|&x| declicker.process(x)).collect();

        let input_rms = rms(&input);
        let output_rms = rms(&output);
        assert!(
            (input_rms - output_rms).abs() < 0.02,
            "{input_rms} vs {output_rms}"
        );
    }

    #[test]
    fn reset_clears_history() {
        let mut declicker = Declicker::new();
        declicker.process(1.0);
        declicker.process(1.0);
        declicker.process(1.0);
        declicker.reset();
        assert_eq!(declicker.process(0.25), 0.25);
        assert_eq!(declicker.process(0.5), 0.5);
    }

    fn rms(values: &[f32]) -> f32 {
        if values.is_empty() {
            return 0.0;
        }
        (values.iter().map(|x| x * x).sum::<f32>() / values.len() as f32).sqrt()
    }
}
