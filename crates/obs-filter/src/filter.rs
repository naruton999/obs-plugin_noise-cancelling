use std::ffi::{c_char, c_void};
use std::panic::AssertUnwindSafe;
use std::ptr;
use std::slice;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::thread::{self, JoinHandle, Thread};
use std::time::Duration;

use df_core::stereo::DelayLine;
use df_core::{
    DFN_HOP_SIZE, DfnBackend, Engine, MAX_BLOCK_FRAMES, ModelKind, SUPPORTED_SAMPLE_RATES,
};

use crate::ffi::{
    self, LOG_INFO, LOG_WARNING, OBS_SOURCE_AUDIO, OBS_SOURCE_TYPE_FILTER, ObsAudioData,
    ObsAudioInfo, ObsData, ObsSource, ObsSourceInfo,
};
use crate::layout::split_mid_side;
use crate::models::{self, ModelPaths};
use crate::params::{AtomicParams, Settings};
use crate::{guard, log};

const RING_CAPACITY: usize = 16_384;
const MAX_DELAY: usize = 8_192;
const WORK_CHUNK: usize = 1_024;
const PARK_TIMEOUT_MS: u64 = 5;

pub(crate) const STATUS_LOADING: u32 = 0;
pub(crate) const STATUS_READY: u32 = 1;
pub(crate) const STATUS_FAILED: u32 = 2;
pub(crate) const STATUS_UNSUPPORTED: u32 = 3;

pub(crate) struct Shared {
    pub params: AtomicParams,
    pub status: AtomicU32,
    pub latency: AtomicU64,
    pub underruns: AtomicU64,
    pub overruns: AtomicU64,
    pub errors: AtomicU64,
    prime_request: AtomicU64,
    prime_done: AtomicU64,
    primed: AtomicBool,
}

struct AudioSide {
    in_tx: Option<rtrb::Producer<f32>>,
    out_rx: Option<rtrb::Consumer<f32>>,
    mid_dry: DelayLine,
    side_delay: DelayLine,
    mid_scratch: Vec<f32>,
    side_scratch: Vec<f32>,
    ready: bool,
    channels: usize,
}

struct WorkerHandle {
    join: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    thread: Thread,
}

impl WorkerHandle {
    fn stop(mut self) {
        self.stop.store(true, Ordering::Release);
        self.thread.unpark();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

pub(crate) struct FilterData {
    pub shared: Arc<Shared>,
    pub source: *mut ObsSource,
    pub model_paths: ModelPaths,
    audio: *mut AudioSide,
    worker: Option<WorkerHandle>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ActiveConfig {
    model: ModelKind,
    low_latency: bool,
    sample_rate: u32,
}

pub(crate) fn source_info() -> ObsSourceInfo {
    let mut info: ObsSourceInfo = unsafe { std::mem::zeroed() };
    info.id = c"obs_plugin_noise_cancelling_filter".as_ptr();
    info.type_ = OBS_SOURCE_TYPE_FILTER;
    info.output_flags = OBS_SOURCE_AUDIO;
    info.get_name = Some(get_name);
    info.create = Some(create);
    info.destroy = Some(destroy);
    info.get_defaults = Some(crate::ui::get_defaults);
    info.get_properties = Some(crate::ui::get_properties);
    info.update = Some(crate::ui::update);
    info.filter_audio = Some(filter_audio);
    info
}

unsafe extern "C" fn get_name(_type_data: *mut c_void) -> *const c_char {
    c"DeepFilterNet ノイズキャンセリング".as_ptr()
}

unsafe extern "C" fn create(settings: *mut ObsData, source: *mut ObsSource) -> *mut c_void {
    guard(ptr::null_mut(), || {
        let sample_rate = audio_sample_rate();
        // OBS calls get_defaults() and create() when loading a filter; it does
        // not call update() on load. Read the saved settings here so the DSP
        // starts with the values from the scene collection.
        let saved_settings = if settings.is_null() {
            Settings {
                sample_rate,
                ..Settings::default()
            }
        } else {
            unsafe { crate::ui::read_settings(settings, sample_rate) }
        };
        log(
            LOG_INFO,
            &format!(
                "create: model={} enabled={} mix={:.2} atten={:.1} low_latency={} bypass={}",
                saved_settings.model.id(),
                saved_settings.denoise.enabled,
                saved_settings.denoise.mix,
                saved_settings.denoise.attenuation_limit_db,
                saved_settings.low_latency,
                saved_settings.denoise.bypass
            ),
        );
        let shared = Arc::new(Shared {
            params: AtomicParams::new(saved_settings),
            status: AtomicU32::new(if is_supported_rate(sample_rate) {
                STATUS_LOADING
            } else {
                STATUS_UNSUPPORTED
            }),
            latency: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            overruns: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            prime_request: AtomicU64::new(0),
            prime_done: AtomicU64::new(0),
            primed: AtomicBool::new(false),
        });
        let model_paths = models::resolve_all();

        let (in_tx, in_rx) = rtrb::RingBuffer::new(RING_CAPACITY);
        let (out_tx, out_rx) = rtrb::RingBuffer::new(RING_CAPACITY);
        let audio = Box::new(AudioSide {
            in_tx: Some(in_tx),
            out_rx: Some(out_rx),
            mid_dry: DelayLine::new(MAX_DELAY),
            side_delay: DelayLine::new(MAX_DELAY),
            mid_scratch: vec![0.0; MAX_BLOCK_FRAMES],
            side_scratch: vec![0.0; MAX_BLOCK_FRAMES],
            ready: false,
            channels: 0,
        });
        let audio_ptr = Box::into_raw(audio);

        let worker = if is_supported_rate(sample_rate) {
            match start_worker(shared.clone(), model_paths.clone(), in_rx, out_tx) {
                Ok(worker) => Some(worker),
                Err(err) => {
                    shared.status.store(STATUS_FAILED, Ordering::Release);
                    log(LOG_WARNING, &format!("failed to start worker: {err}"));
                    None
                }
            }
        } else {
            log(
                LOG_WARNING,
                &format!("unsupported OBS sample rate: {sample_rate} Hz"),
            );
            None
        };

        Box::into_raw(Box::new(FilterData {
            shared,
            source,
            model_paths,
            audio: audio_ptr,
            worker,
        }))
        .cast::<c_void>()
    })
}

unsafe extern "C" fn destroy(data: *mut c_void) {
    guard((), || unsafe {
        if data.is_null() {
            return;
        }
        let mut filter = Box::from_raw(data.cast::<FilterData>());
        if let Some(worker) = filter.worker.take() {
            worker.stop();
        }
        if !filter.audio.is_null() {
            drop(Box::from_raw(filter.audio));
        }
    });
}

unsafe extern "C" fn filter_audio(
    data: *mut c_void,
    audio: *mut ObsAudioData,
) -> *mut ObsAudioData {
    guard(ptr::null_mut(), || {
        if data.is_null() || audio.is_null() {
            return audio;
        }

        let filter = unsafe { &*data.cast::<FilterData>() };
        let audio_ref = unsafe { &mut *audio };
        let frames = audio_ref.frames as usize;
        if frames == 0 {
            return audio;
        }

        let Some(side) = (!filter.audio.is_null()).then(|| unsafe { &mut *filter.audio }) else {
            return audio;
        };

        let status = filter.shared.status.load(Ordering::Acquire);
        let enabled = filter.shared.params.load_enabled();
        if !enabled {
            side.ready = false;
            drain_output(side);
            acknowledge_prime(&filter.shared);
            wake_worker(filter);
            return audio;
        }
        if status != STATUS_READY {
            side.ready = false;
            // The worker asks us to drain before it prefills the output ring.
            // Once `primed` is set, leave the ring alone and wait for READY.
            if !filter.shared.primed.load(Ordering::Acquire) {
                drain_output(side);
                acknowledge_prime(&filter.shared);
            }
            wake_worker(filter);
            return audio;
        }

        if !side.ready {
            let latency = filter.shared.latency.load(Ordering::Acquire) as usize;
            side.mid_dry.set_delay(latency);
            side.side_delay.set_delay(latency);
            side.channels = 0;
            side.ready = true;
        }

        let mut planes = [ptr::null_mut::<f32>(); 2];
        let mut channels = 0usize;
        for &plane in &audio_ref.data {
            if plane.is_null() {
                break;
            }
            if channels < planes.len() {
                planes[channels] = plane.cast::<f32>();
            }
            channels += 1;
        }

        if channels == 0 {
            return audio;
        }
        if channels > 2 {
            // v0.1 supports mono/stereo. Leave multi-channel audio untouched
            // and drop any queued stereo data so a later 2ch block starts clean.
            side.ready = false;
            drain_output(side);
            acknowledge_prime(&filter.shared);
            wake_worker(filter);
            return audio;
        }

        // OBS may duplicate a mono source into both planes using the same
        // pointer. Treat that as mono to avoid creating aliasing &mut slices.
        let linked = channels == 2 && planes[0] == planes[1];
        let layout_channels = if linked { 1 } else { channels };

        if side.channels != layout_channels {
            let latency = filter.shared.latency.load(Ordering::Acquire) as usize;
            side.mid_dry.set_delay(latency);
            side.side_delay.set_delay(latency);
            side.channels = layout_channels;
        }

        let mut underruns = 0u64;
        let mut overruns = 0u64;
        let mut offset = 0usize;
        while offset < frames {
            let len = (frames - offset).min(MAX_BLOCK_FRAMES);
            if layout_channels == 1 {
                let plane = planes[0];
                let buffer = unsafe { slice::from_raw_parts_mut(plane.add(offset), len) };
                process_mono(side, buffer, &mut underruns, &mut overruns);
            } else {
                let left = unsafe { slice::from_raw_parts_mut(planes[0].add(offset), len) };
                let right = unsafe { slice::from_raw_parts_mut(planes[1].add(offset), len) };
                process_stereo(side, left, right, &mut underruns, &mut overruns);
            }
            offset += len;
        }

        if underruns != 0 {
            filter
                .shared
                .underruns
                .fetch_add(underruns, Ordering::Relaxed);
        }
        if overruns != 0 {
            filter
                .shared
                .overruns
                .fetch_add(overruns, Ordering::Relaxed);
        }

        wake_worker(filter);
        audio
    })
}

fn wake_worker(filter: &FilterData) {
    if let Some(worker) = filter.worker.as_ref() {
        worker.thread.unpark();
    }
}

fn process_mono(side: &mut AudioSide, buffer: &mut [f32], underruns: &mut u64, overruns: &mut u64) {
    let len = buffer.len();
    side.mid_scratch[..len].copy_from_slice(buffer);
    push_input(&mut side.in_tx, &side.mid_scratch[..len], overruns);

    for (sample, &mid) in buffer.iter_mut().zip(side.mid_scratch[..len].iter()) {
        let dry = side.mid_dry.push_pop(mid);
        let wet = match side.out_rx.as_mut().and_then(|rx| rx.pop().ok()) {
            Some(value) => value,
            None => {
                *underruns += 1;
                dry
            }
        };
        *sample = finite(wet);
    }
}

fn process_stereo(
    side: &mut AudioSide,
    left: &mut [f32],
    right: &mut [f32],
    underruns: &mut u64,
    overruns: &mut u64,
) {
    let len = left.len().min(right.len());
    {
        let (mid, side_buf) = (&mut side.mid_scratch, &mut side.side_scratch);
        split_mid_side(
            &left[..len],
            &right[..len],
            &mut mid[..len],
            &mut side_buf[..len],
        );
    }
    push_input(&mut side.in_tx, &side.mid_scratch[..len], overruns);

    for i in 0..len {
        let mid = side.mid_scratch[i];
        let side_sample = side.side_scratch[i];
        let dry = side.mid_dry.push_pop(mid);
        let side_delayed = side.side_delay.push_pop(side_sample);
        let wet = match side.out_rx.as_mut().and_then(|rx| rx.pop().ok()) {
            Some(value) => value,
            None => {
                *underruns += 1;
                dry
            }
        };
        let (l, r) = join_mid_side_value(wet, side_delayed);
        left[i] = finite(l);
        right[i] = finite(r);
    }
}

#[inline]
fn join_mid_side_value(mid: f32, side: f32) -> (f32, f32) {
    (mid + side, mid - side)
}

fn push_input(tx: &mut Option<rtrb::Producer<f32>>, samples: &[f32], overruns: &mut u64) {
    let Some(tx) = tx.as_mut() else {
        return;
    };
    for &sample in samples {
        if tx.push(finite(sample)).is_err() {
            *overruns += 1;
            break;
        }
    }
}

fn drain_output(side: &mut AudioSide) {
    if let Some(rx) = side.out_rx.as_mut() {
        while rx.pop().is_ok() {}
    }
}

fn acknowledge_prime(shared: &Shared) {
    let request = shared.prime_request.load(Ordering::Acquire);
    if request != 0 && shared.prime_done.load(Ordering::Acquire) != request {
        shared.prime_done.store(request, Ordering::Release);
    }
}

#[inline]
fn finite(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

fn is_supported_rate(rate: u32) -> bool {
    SUPPORTED_SAMPLE_RATES.contains(&rate)
}

pub(crate) fn audio_sample_rate() -> u32 {
    let mut info = ObsAudioInfo::default();
    let ok = unsafe { ffi::obs_get_audio_info(&mut info) };
    if ok && info.samples_per_sec > 0 {
        info.samples_per_sec
    } else {
        48_000
    }
}

fn start_worker(
    shared: Arc<Shared>,
    model_paths: ModelPaths,
    in_rx: rtrb::Consumer<f32>,
    out_tx: rtrb::Producer<f32>,
) -> std::io::Result<WorkerHandle> {
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = stop.clone();
    let panic_shared = shared.clone();

    let join = thread::Builder::new()
        .name("noise-cancelling".to_string())
        .spawn(move || {
            let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
                worker_loop(shared, model_paths, in_rx, out_tx, thread_stop);
            }));
            if outcome.is_err() {
                panic_shared.status.store(STATUS_FAILED, Ordering::Release);
                log(LOG_WARNING, "worker thread panicked");
            }
        })?;

    let thread = join.thread().clone();
    Ok(WorkerHandle {
        join: Some(join),
        stop,
        thread,
    })
}

fn worker_loop(
    shared: Arc<Shared>,
    model_paths: ModelPaths,
    mut in_rx: rtrb::Consumer<f32>,
    mut out_tx: rtrb::Producer<f32>,
    stop: Arc<AtomicBool>,
) {
    let mut engine: Option<Engine> = None;
    let mut active_config: Option<ActiveConfig> = None;
    let mut worker_buffer = 0usize;
    let mut was_enabled = true;
    let mut input_buf = vec![0.0f32; WORK_CHUNK];
    let mut output_buf = Vec::with_capacity(WORK_CHUNK + DFN_HOP_SIZE * 4);

    loop {
        if stop.load(Ordering::Acquire) {
            return;
        }

        let settings = shared.params.load();
        if !is_supported_rate(settings.sample_rate) {
            shared.status.store(STATUS_UNSUPPORTED, Ordering::Release);
            drop(engine.take());
            active_config = None;
            drain_input(&mut in_rx);
            thread::park_timeout(Duration::from_millis(PARK_TIMEOUT_MS));
            continue;
        }

        let config = ActiveConfig {
            model: settings.model,
            low_latency: settings.low_latency,
            sample_rate: settings.sample_rate,
        };

        if active_config != Some(config) {
            shared.status.store(STATUS_LOADING, Ordering::Release);
            drop(engine.take());
            drain_input(&mut in_rx);

            match build_engine(&model_paths, settings) {
                Ok((new_engine, latency, buffer_samples)) => {
                    engine = Some(new_engine);
                    active_config = Some(config);
                    worker_buffer = buffer_samples;
                    shared.latency.store(latency as u64, Ordering::Release);
                    if !prime_output(&shared, &mut out_tx, &stop, worker_buffer) {
                        return;
                    }
                    was_enabled = settings.denoise.enabled;
                    log(
                        LOG_INFO,
                        &format!(
                            "ready: model={} rate={} low_latency={} latency={:.1} ms",
                            settings.model.id(),
                            settings.sample_rate,
                            settings.low_latency,
                            latency as f64 / settings.sample_rate as f64 * 1000.0
                        ),
                    );
                }
                Err(err) => {
                    active_config = Some(config);
                    shared.status.store(STATUS_FAILED, Ordering::Release);
                    log(
                        LOG_WARNING,
                        &format!(
                            "model load failed: model={} path={:?}: {err}",
                            settings.model.id(),
                            model_paths.path(settings.model)
                        ),
                    );
                    drain_input(&mut in_rx);
                    thread::park_timeout(Duration::from_millis(PARK_TIMEOUT_MS));
                    continue;
                }
            }
        }

        let Some(engine) = engine.as_mut() else {
            drain_input(&mut in_rx);
            thread::park_timeout(Duration::from_millis(PARK_TIMEOUT_MS));
            continue;
        };

        engine.set_params(settings.denoise);
        shared.errors.store(engine.errors(), Ordering::Relaxed);

        if !settings.denoise.enabled {
            if was_enabled {
                if engine.reset().is_err() {
                    log(LOG_WARNING, "engine reset failed");
                }
                was_enabled = false;
            }
            drain_input(&mut in_rx);
            thread::park_timeout(Duration::from_millis(PARK_TIMEOUT_MS));
            continue;
        }

        if !was_enabled {
            if engine.reset().is_err() {
                log(LOG_WARNING, "engine reset failed");
            }
            if !prime_output(&shared, &mut out_tx, &stop, worker_buffer) {
                return;
            }
            was_enabled = true;
        }

        let available = in_rx.slots();
        let count = available.min(input_buf.len());
        if count == 0 {
            thread::park_timeout(Duration::from_millis(PARK_TIMEOUT_MS));
            continue;
        }

        for slot in &mut input_buf[..count] {
            *slot = in_rx.pop().unwrap_or(0.0);
        }

        engine.process(&input_buf[..count], &mut output_buf);
        shared.errors.store(engine.errors(), Ordering::Relaxed);

        if output_buf.is_empty() {
            continue;
        }

        while out_tx.slots() < output_buf.len() {
            if stop.load(Ordering::Acquire) {
                return;
            }
            thread::park_timeout(Duration::from_millis(2));
        }

        for &sample in &output_buf {
            if out_tx.push(finite(sample)).is_err() {
                break;
            }
        }
    }
}

fn build_engine(
    model_paths: &ModelPaths,
    settings: Settings,
) -> anyhow::Result<(Engine, usize, usize)> {
    let path = model_paths.path(settings.model).map(|path| path.as_path());
    let backend = DfnBackend::from_model(settings.model, path)?;
    let mut engine = Engine::new(Box::new(backend), settings.sample_rate, MAX_BLOCK_FRAMES)?;
    engine.set_params(settings.denoise);

    let worker_hops = if settings.low_latency { 1 } else { 2 };
    let hop_native =
        (DFN_HOP_SIZE as f64 * settings.sample_rate as f64 / 48_000.0).round() as usize;
    let worker_buffer = worker_hops * hop_native;
    let latency = engine.latency_samples() + worker_buffer;
    Ok((engine, latency, worker_buffer))
}

fn prime_output(
    shared: &Shared,
    out_tx: &mut rtrb::Producer<f32>,
    stop: &AtomicBool,
    samples: usize,
) -> bool {
    shared.primed.store(false, Ordering::Release);
    let request = shared.prime_request.fetch_add(1, Ordering::AcqRel) + 1;
    shared.status.store(STATUS_LOADING, Ordering::Release);

    while shared.prime_done.load(Ordering::Acquire) != request {
        if stop.load(Ordering::Acquire) {
            return false;
        }
        thread::park_timeout(Duration::from_millis(PARK_TIMEOUT_MS));
    }

    // From here the audio thread must not drain the ring. `primed` is set
    // before the actual fill; the audio thread only reads after READY.
    shared.primed.store(true, Ordering::Release);
    prefill_output(out_tx, samples);
    shared.status.store(STATUS_READY, Ordering::Release);
    true
}

fn prefill_output(out_tx: &mut rtrb::Producer<f32>, samples: usize) {
    for _ in 0..samples {
        if out_tx.push(0.0).is_err() {
            break;
        }
    }
}

fn drain_input(in_rx: &mut rtrb::Consumer<f32>) {
    while in_rx.pop().is_ok() {}
}
