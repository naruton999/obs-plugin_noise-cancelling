use std::path::PathBuf;

use anyhow::{Context, Result};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(
        args.next()
            .unwrap_or_else(|| "target/test_noisy.wav".into()),
    );
    let seconds: f32 = args
        .next()
        .unwrap_or_else(|| "3.0".to_string())
        .parse()
        .context("duration must be a number")?;
    let sample_rate: u32 = args
        .next()
        .unwrap_or_else(|| "48000".to_string())
        .parse()
        .context("sample rate must be an integer")?;
    anyhow::ensure!(
        sample_rate == 44_100 || sample_rate == 48_000,
        "sample rate must be 44100 or 48000"
    );
    let frames = (sample_rate as f32 * seconds) as u32;
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&output, spec)
        .with_context(|| format!("create {}", output.display()))?;

    let mut noise_state = 0x1234_5678u32;
    for i in 0..frames {
        let t = i as f32 / sample_rate as f32;
        let voice = (t * 220.0 * std::f32::consts::TAU).sin() * 0.18
            + (t * 440.0 * std::f32::consts::TAU).sin() * 0.10
            + (t * 1_300.0 * std::f32::consts::TAU).sin() * 0.04;
        noise_state = noise_state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        let noise = ((noise_state >> 8) as f32 / 16_777_215.0 - 0.5) * 0.08;
        let sample = (voice + noise).clamp(-1.0, 1.0);
        writer.write_sample::<i16>((sample * 32_767.0).round() as i16)?;
    }
    writer.finalize()?;
    println!(
        "created {} ({:.1}s, {} Hz)",
        output.display(),
        seconds,
        sample_rate
    );
    Ok(())
}
