/// Mid/side helpers and a non-allocating delay line.
///
/// The delay line is used by the OBS audio thread. `set_delay` never allocates;
/// the backing buffer is sized once with `new`.
pub struct DelayLine {
    buffer: Vec<f32>,
    delay: usize,
    pos: usize,
}

impl DelayLine {
    pub fn new(max_delay: usize) -> Self {
        Self {
            buffer: vec![0.0; max_delay.max(1)],
            delay: 0,
            pos: 0,
        }
    }

    /// Set the delay in samples. Values above the preallocated maximum are
    /// clamped. The used part of the buffer is cleared.
    pub fn set_delay(&mut self, delay: usize) {
        self.delay = delay.min(self.buffer.len());
        self.pos = 0;
        self.buffer[..self.delay].fill(0.0);
    }

    pub fn delay(&self) -> usize {
        self.delay
    }

    pub fn reset(&mut self) {
        self.buffer[..self.delay].fill(0.0);
        self.pos = 0;
    }

    /// Push one sample and return the sample from `delay` samples ago.
    #[inline]
    pub fn push_pop(&mut self, sample: f32) -> f32 {
        if self.delay == 0 {
            return sample;
        }

        let out = self.buffer[self.pos];
        self.buffer[self.pos] = sample;
        self.pos += 1;
        if self.pos >= self.delay {
            self.pos = 0;
        }
        out
    }
}

#[inline]
pub fn mid(left: f32, right: f32) -> f32 {
    0.5 * (left + right)
}

#[inline]
pub fn side(left: f32, right: f32) -> f32 {
    0.5 * (left - right)
}

#[inline]
pub fn decode(mid: f32, side: f32) -> (f32, f32) {
    (mid + side, mid - side)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mid_side_round_trip() {
        let (l, r) = (0.25, -0.5);
        let (m, s) = (mid(l, r), side(l, r));
        let (l2, r2) = decode(m, s);
        assert!((l - l2).abs() < 1e-6);
        assert!((r - r2).abs() < 1e-6);
    }

    #[test]
    fn identical_channels_have_zero_side() {
        assert_eq!(side(0.3, 0.3), 0.0);
    }

    #[test]
    fn left_only_is_preserved() {
        let (m, s) = (mid(0.8, 0.0), side(0.8, 0.0));
        let (l, r) = decode(m, s);
        assert!((l - 0.8).abs() < 1e-6);
        assert!(r.abs() < 1e-6);
    }

    #[test]
    fn delay_line_returns_delayed_samples() {
        let mut line = DelayLine::new(8);
        line.set_delay(3);
        let mut out = Vec::new();
        for x in 1..=6 {
            out.push(line.push_pop(x as f32));
        }
        assert_eq!(out, vec![0.0, 0.0, 0.0, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn zero_delay_is_passthrough() {
        let mut line = DelayLine::new(8);
        line.set_delay(0);
        assert_eq!(line.push_pop(0.5), 0.5);
    }
}
