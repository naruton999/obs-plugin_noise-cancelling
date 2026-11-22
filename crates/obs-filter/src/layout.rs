//! Channel helpers kept free of OBS FFI so they can be unit-tested.

use df_core::stereo::{mid, side};

pub(crate) fn split_mid_side(
    left: &[f32],
    right: &[f32],
    mid_out: &mut [f32],
    side_out: &mut [f32],
) {
    let frames = left
        .len()
        .min(right.len())
        .min(mid_out.len())
        .min(side_out.len());
    for i in 0..frames {
        mid_out[i] = mid(left[i], right[i]);
        side_out[i] = side(left[i], right[i]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use df_core::stereo::decode;

    #[test]
    fn mid_side_round_trip() {
        let left = [0.5, -0.25, 0.0, 0.75];
        let right = [-0.5, 0.25, 0.0, 0.75];
        let mut mid_buf = [0.0; 4];
        let mut side_buf = [0.0; 4];
        split_mid_side(&left, &right, &mut mid_buf, &mut side_buf);

        for i in 0..4 {
            let (left_out, right_out) = decode(mid_buf[i], side_buf[i]);
            assert!((left[i] - left_out).abs() < 1e-6);
            assert!((right[i] - right_out).abs() < 1e-6);
        }
    }
}
