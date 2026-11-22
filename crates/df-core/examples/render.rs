use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use df_core::{
    DFN_HOP_SIZE, DFN_SAMPLE_RATE, DfnBackend, Engine, MAX_BLOCK_FRAMES, ModelKind, mid,
    parse_preset, side,
};
use hound::{SampleFormat, WavSpec};

fn main() -> Result<()> {
    let mut input = None;
    let mut output = None;
    let mut model = ModelKind::LowLatency;
    let mut preset_name = "standard".to_string();
    let mut model_path = None;
    let mut preset_path = None;
    let mut low_latency = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                usage();
                return Ok(());
            }
            "--model" => {
                let value = args.next().context("--model requires ll or standard")?;
                model = ModelKind::from_id(&value)
                    .with_context(|| format!("unknown model: {value}"))?;
            }
            "--preset" => {
                preset_name = args.next().context("--preset requires a name")?;
            }
            "--model-path" => {
                model_path = Some(PathBuf::from(
                    args.next().context("--model-path requires a path")?,
                ));
            }
            "--preset-path" => {
                preset_path = Some(PathBuf::from(
                    args.next().context("--preset-path requires a path")?,
                ));
            }
            "--low-latency" => low_latency = true,
            other if input.is_none() => input = Some(PathBuf::from(other)),
            other if output.is_none() => output = Some(PathBuf::from(other)),
            other => bail!("unexpected argument: {other}"),
        }
    }

    let input = input.context("missing input WAV path")?;
    let output = output.context("missing output WAV path")?;
    let repo = repo_root()?;
    let preset_path =
        preset_path.unwrap_or_else(|| repo.join("presets").join(format!("{preset_name}.toml")));
    let model_path = model_path.unwrap_or_else(|| repo.join("models").join(model.file_name()));

    let preset_text = std::fs::read_to_string(&preset_path)
        .with_context(|| format!("read preset: {}", preset_path.display()))?;
    let preset = parse_preset(&preset_text).map_err(anyhow::Error::msg)?;

    let (spec, channels) = read_wav(&input)?;
    if spec.channels == 0 || spec.channels > 2 {
        bail!("only mono and stereo WAV files are supported");
    }

    let mut engine = Engine::new(
        Box::new(DfnBackend::from_model(model, Some(model_path.as_path()))?),
        spec.sample_rate,
        MAX_BLOCK_FRAMES,
    )
    .context("create denoise engine")?;
    engine.set_params(preset.params);

    let worker_hops = if low_latency { 1 } else { 2 };
    let hop_native =
        (DFN_HOP_SIZE as f64 * spec.sample_rate as f64 / DFN_SAMPLE_RATE as f64).round() as usize;
    let dsp_latency = engine.latency_samples();

    let processed: Vec<Vec<f32>> = if spec.channels == 1 {
        vec![process_mono(&mut engine, &channels[0])]
    } else {
        let frames = channels[0].len();
        let mut mid_signal = Vec::with_capacity(frames);
        let mut side_signal = Vec::with_capacity(frames);
        for i in 0..frames {
            mid_signal.push(mid(channels[0][i], channels[1][i]));
            side_signal.push(side(channels[0][i], channels[1][i]));
        }

        let mid_out = process_mono(&mut engine, &mid_signal);
        let mut side_delayed = vec![0.0f32; frames];
        if frames > dsp_latency {
            side_delayed[dsp_latency..].copy_from_slice(&side_signal[..frames - dsp_latency]);
        }

        let mut left = Vec::with_capacity(frames);
        let mut right = Vec::with_capacity(frames);
        for i in 0..frames {
            left.push(mid_out[i] + side_delayed[i]);
            right.push(mid_out[i] - side_delayed[i]);
        }
        vec![left, right]
    };

    write_wav(&output, spec, &processed)?;

    println!("input: {}", input.display());
    println!("output: {}", output.display());
    println!("model: {} ({})", model.label(), model.file_name());
    println!("preset: {} ({})", preset.name, preset_path.display());
    println!("sample rate: {} Hz", spec.sample_rate);
    println!("channels: {}", spec.channels);
    println!(
        "input RMS / peak: {:.6} / {:.6}",
        rms(&channels),
        peak(&channels)
    );
    println!(
        "output RMS / peak: {:.6} / {:.6}",
        rms(&processed),
        peak(&processed)
    );
    println!(
        "DSP latency: {:.2} ms",
        dsp_latency as f64 / spec.sample_rate as f64 * 1000.0
    );
    println!(
        "plugin internal latency estimate: {:.2} ms (DSP + {} worker hop(s); OBS block adds about 21 ms)",
        (dsp_latency + worker_hops * hop_native) as f64 / spec.sample_rate as f64 * 1000.0,
        worker_hops
    );
    Ok(())
}

fn repo_root() -> Result<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .join("../..")
        .canonicalize()
        .context("resolve repository root")
}

fn rms(channels: &[Vec<f32>]) -> f32 {
    let mut sum = 0.0f64;
    let mut count = 0usize;
    for channel in channels {
        for &sample in channel {
            sum += (sample as f64) * (sample as f64);
        }
        count += channel.len();
    }
    if count == 0 {
        0.0
    } else {
        (sum / count as f64).sqrt() as f32
    }
}

fn peak(channels: &[Vec<f32>]) -> f32 {
    channels
        .iter()
        .flat_map(|channel| channel.iter())
        .map(|sample| sample.abs())
        .fold(0.0f32, f32::max)
}

fn process_mono(engine: &mut Engine, input: &[f32]) -> Vec<f32> {
    let latency = engine.latency_samples();
    let mut raw = Vec::with_capacity(input.len() + latency + MAX_BLOCK_FRAMES * 2);
    let mut temp = Vec::new();

    for chunk in input.chunks(MAX_BLOCK_FRAMES) {
        engine.process(chunk, &mut temp);
        raw.extend(temp.iter().copied());
    }

    let flush = latency + 4 * DFN_HOP_SIZE + MAX_BLOCK_FRAMES;
    let zeros = vec![0.0f32; flush];
    for chunk in zeros.chunks(MAX_BLOCK_FRAMES) {
        engine.process(chunk, &mut temp);
        raw.extend(temp.iter().copied());
    }

    let mut aligned = vec![0.0f32; input.len()];
    if raw.len() > latency {
        let count = (raw.len() - latency).min(input.len());
        aligned[..count].copy_from_slice(&raw[latency..latency + count]);
    }
    aligned
}

fn read_wav(path: &Path) -> Result<(WavSpec, Vec<Vec<f32>>)> {
    let mut reader =
        hound::WavReader::open(path).with_context(|| format!("open WAV: {}", path.display()))?;
    let spec = reader.spec();
    let channels = spec.channels as usize;

    let raw: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Float, 32) => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .context("read float WAV")?,
        (SampleFormat::Int, 16) => reader
            .samples::<i16>()
            .map(|sample| sample.map(|value| value as f32 / 32_768.0))
            .collect::<Result<Vec<_>, _>>()
            .context("read 16-bit WAV")?,
        (SampleFormat::Int, 24) => reader
            .samples::<i32>()
            .map(|sample| sample.map(|value| value as f32 / 8_388_608.0))
            .collect::<Result<Vec<_>, _>>()
            .context("read 24-bit WAV")?,
        (SampleFormat::Int, 32) => reader
            .samples::<i32>()
            .map(|sample| sample.map(|value| value as f32 / 2_147_483_648.0))
            .collect::<Result<Vec<_>, _>>()
            .context("read 32-bit WAV")?,
        _ => bail!(
            "unsupported WAV format: {:?}, {} bits",
            spec.sample_format,
            spec.bits_per_sample
        ),
    };

    let mut result = vec![Vec::new(); channels];
    for (index, sample) in raw.into_iter().enumerate() {
        result[index % channels].push(sample);
    }
    Ok((spec, result))
}

fn write_wav(path: &Path, spec: WavSpec, channels: &[Vec<f32>]) -> Result<()> {
    let frames = channels.first().map(Vec::len).unwrap_or(0);
    if channels.iter().any(|channel| channel.len() != frames) {
        bail!("channel lengths differ");
    }

    let mut writer = hound::WavWriter::create(path, spec)
        .with_context(|| format!("create WAV: {}", path.display()))?;

    match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Float, 32) => {
            for frame in 0..frames {
                for channel in channels {
                    writer.write_sample::<f32>(channel[frame])?;
                }
            }
        }
        (SampleFormat::Int, 16) => {
            for frame in 0..frames {
                for channel in channels {
                    writer.write_sample::<i16>(
                        (channel[frame].clamp(-1.0, 1.0) * 32_767.0).round() as i16,
                    )?;
                }
            }
        }
        (SampleFormat::Int, 24) => {
            for frame in 0..frames {
                for channel in channels {
                    writer.write_sample::<i32>(
                        (channel[frame].clamp(-1.0, 1.0) * 8_388_607.0).round() as i32,
                    )?;
                }
            }
        }
        (SampleFormat::Int, 32) => {
            for frame in 0..frames {
                for channel in channels {
                    writer.write_sample::<i32>(
                        (channel[frame].clamp(-1.0, 1.0) * 2_147_483_647.0).round() as i32,
                    )?;
                }
            }
        }
        _ => bail!(
            "unsupported output WAV format: {:?}, {} bits",
            spec.sample_format,
            spec.bits_per_sample
        ),
    }

    writer.finalize()?;
    Ok(())
}

fn usage() {
    eprintln!(
        "usage: render <input.wav> <output.wav> [--model ll|standard] [--preset standard]\n\
         \x20      [--model-path PATH] [--preset-path PATH] [--low-latency]"
    );
}
