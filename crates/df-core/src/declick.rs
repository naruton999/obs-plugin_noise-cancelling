/// Causal conditional seven-point median de-clicker.
///
/// A click is detected when the current sample deviates from the local median
/// by more than a multiple of the slow signal envelope. Only then is the median
/// substituted and held for a few samples. This removes multi-sample keyboard
/// clicks while leaving sustained high-frequency voice content mostly intact.
///
/// The filter adds three samples of latency (about 0.06 ms at 48 kHz).
pub struct Declicker {
    history: [f32; HISTORY],
    filled: usize,
    pos: usize,
    envelope: f32,
    hold: u32,
    hold_value: f32,
}

const WINDOW: usize = 7;
const HISTORY: usize = WINDOW - 1;
const HOLD_SAMPLES: u32 = 3;

/// Slow envelope followers. The attack is deliberately slow so a click does
/// not raise the detection threshold before the click is removed.
const ENVELOPE_ATTACK: f32 = 0.001;
const ENVELOPE_RELEASE: f32 = 0.0005;

/// A sample is treated as a click when it is this many times the envelope
/// above the local median, with an absolute floor for quiet signals.
const CLICK_RATIO: f32 = 3.0;
const CLICK_FLOOR: f32 = 0.02;

impl Default for Declicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Declicker {
    pub fn new() -> Self {
        Self {
            history: [0.0; HISTORY],
            filled: 0,
            pos: 0,
            envelope: 0.0,
            hold: 0,
            hold_value: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.history = [0.0; HISTORY];
        self.filled = 0;
        self.pos = 0;
        self.envelope = 0.0;
        self.hold = 0;
        self.hold_value = 0.0;
    }

    /// Process one sample. The first six samples pass through while the
    /// history fills; after that a conditional seven-point median is used.
    #[inline]
    pub fn process(&mut self, sample: f32) -> f32 {
        let sample = if sample.is_finite() { sample } else { 0.0 };

        let level = sample.abs();
        if level > self.envelope {
            self.envelope += ENVELOPE_ATTACK * (level - self.envelope);
        } else {
            self.envelope += ENVELOPE_RELEASE * (level - self.envelope);
        }

        if self.filled < HISTORY {
            self.history[self.pos] = sample;
            self.pos = (self.pos + 1) % HISTORY;
            self.filled += 1;
            return sample;
        }

        let mut window = [0.0f32; WINDOW];
        window[..HISTORY].copy_from_slice(&self.history);
        window[HISTORY] = sample;
        window.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = window[WINDOW / 2];

        let threshold = (self.envelope * CLICK_RATIO).max(CLICK_FLOOR);
        if (sample - median).abs() > threshold {
            // Keep the last clean median while a multi-sample click is still
            // passing through the window. Updating the hold value from a
            // contaminated window would let part of the click through.
            if self.hold == 0 {
                self.hold_value = median;
            }
            self.hold = HOLD_SAMPLES;
        }

        let output = if self.hold > 0 {
            self.hold -= 1;
            self.hold_value
        } else {
            sample
        };

        // Keep the raw history. Feeding the median back would make the filter
        // latch onto one value for monotonic signals and attenuate the whole
        // waveform.
        self.history[self.pos] = sample;
        self.pos = (self.pos + 1) % HISTORY;
        output
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
    fn removes_three_sample_click() {
        let mut declicker = Declicker::new();
        let mut input = Vec::new();
        for i in 0..256 {
            let phase = i as f32 * 440.0 * std::f32::consts::TAU / 48_000.0;
            input.push(phase.sin() * 0.2);
        }
        input[100] = 1.5;
        input[101] = 1.2;
        input[102] = 0.8;

        let output: Vec<f32> = input.iter().map(|&x| declicker.process(x)).collect();
        let peak = output[95..108].iter().map(|x| x.abs()).fold(0.0, f32::max);

        assert!(peak < 0.3, "three-sample click remained: peak={peak}");
    }

    #[test]
    fn removes_one_millisecond_burst() {
        let mut declicker = Declicker::new();
        let mut input = Vec::new();
        for i in 0..2_400 {
            let phase = i as f32 * 440.0 * std::f32::consts::TAU / 48_000.0;
            input.push(phase.sin() * 0.2);
        }

        let mut seed = 12_345u32;
        for k in 0..48 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = ((seed >> 8) as f32 / 16_777_216.0) * 2.0 - 1.0;
            input[1_200 + k] = noise * 1.5 * (-(k as f32) / 12.0).exp();
        }

        let output: Vec<f32> = input.iter().map(|&x| declicker.process(x)).collect();
        let input_rms = rms(&input[1_180..1_280]);
        let output_rms = rms(&output[1_180..1_280]);

        assert!(
            output_rms < input_rms * 0.8,
            "burst was not reduced enough: {input_rms} vs {output_rms}"
        );
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
    fn preserves_high_frequency_sine() {
        let mut declicker = Declicker::new();
        let mut input = Vec::new();
        for i in 0..4_800 {
            let phase = i as f32 * 8_000.0 * std::f32::consts::TAU / 48_000.0;
            input.push(phase.sin() * 0.2);
        }
        let output: Vec<f32> = input.iter().map(|&x| declicker.process(x)).collect();

        let input_rms = rms(&input[100..]);
        let output_rms = rms(&output[100..]);
        assert!(
            (input_rms - output_rms).abs() < 0.02,
            "8 kHz sine was altered: {input_rms} vs {output_rms}"
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
