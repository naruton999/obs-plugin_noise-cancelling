use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

use df_core::{DenoiseParams, ModelKind};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Settings {
    pub denoise: DenoiseParams,
    pub model: ModelKind,
    pub low_latency: bool,
    pub advanced: bool,
    pub sample_rate: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            denoise: DenoiseParams::default(),
            model: ModelKind::LowLatency,
            low_latency: false,
            advanced: false,
            sample_rate: 48_000,
        }
    }
}

impl Settings {
    pub(crate) fn sanitized(mut self) -> Self {
        self.denoise = self.denoise.sanitized();
        self
    }
}

/// Lock-free parameter store shared between the OBS UI thread, the audio
/// thread and the worker.
pub(crate) struct AtomicParams {
    enabled: AtomicBool,
    bypass: AtomicBool,
    attenuation_limit_db: AtomicU32,
    post_filter_enabled: AtomicBool,
    post_filter_beta: AtomicU32,
    mix: AtomicU32,
    min_db_thresh: AtomicU32,
    max_db_erb_thresh: AtomicU32,
    max_db_df_thresh: AtomicU32,
    model: AtomicU8,
    low_latency: AtomicBool,
    advanced: AtomicBool,
    sample_rate: AtomicU32,
}

impl AtomicParams {
    pub(crate) fn new(settings: Settings) -> Self {
        let store = Self {
            enabled: AtomicBool::new(true),
            bypass: AtomicBool::new(false),
            attenuation_limit_db: AtomicU32::new(100.0f32.to_bits()),
            post_filter_enabled: AtomicBool::new(false),
            post_filter_beta: AtomicU32::new(0.0f32.to_bits()),
            mix: AtomicU32::new(1.0f32.to_bits()),
            min_db_thresh: AtomicU32::new((-10.0f32).to_bits()),
            max_db_erb_thresh: AtomicU32::new(30.0f32.to_bits()),
            max_db_df_thresh: AtomicU32::new(20.0f32.to_bits()),
            model: AtomicU8::new(model_to_u8(settings.model)),
            low_latency: AtomicBool::new(false),
            advanced: AtomicBool::new(false),
            sample_rate: AtomicU32::new(settings.sample_rate),
        };
        store.store(settings);
        store
    }

    pub(crate) fn store(&self, settings: Settings) {
        let settings = settings.sanitized();
        self.enabled
            .store(settings.denoise.enabled, Ordering::Relaxed);
        self.bypass
            .store(settings.denoise.bypass, Ordering::Relaxed);
        self.attenuation_limit_db.store(
            settings.denoise.attenuation_limit_db.to_bits(),
            Ordering::Relaxed,
        );
        self.post_filter_enabled
            .store(settings.denoise.post_filter_enabled, Ordering::Relaxed);
        self.post_filter_beta.store(
            settings.denoise.post_filter_beta.to_bits(),
            Ordering::Relaxed,
        );
        self.mix
            .store(settings.denoise.mix.to_bits(), Ordering::Relaxed);
        self.min_db_thresh
            .store(settings.denoise.min_db_thresh.to_bits(), Ordering::Relaxed);
        self.max_db_erb_thresh.store(
            settings.denoise.max_db_erb_thresh.to_bits(),
            Ordering::Relaxed,
        );
        self.max_db_df_thresh.store(
            settings.denoise.max_db_df_thresh.to_bits(),
            Ordering::Relaxed,
        );
        self.model
            .store(model_to_u8(settings.model), Ordering::Relaxed);
        self.low_latency
            .store(settings.low_latency, Ordering::Relaxed);
        self.advanced.store(settings.advanced, Ordering::Relaxed);
        self.sample_rate
            .store(settings.sample_rate, Ordering::Relaxed);
    }

    pub(crate) fn load(&self) -> Settings {
        Settings {
            denoise: DenoiseParams {
                enabled: self.enabled.load(Ordering::Relaxed),
                attenuation_limit_db: f32::from_bits(
                    self.attenuation_limit_db.load(Ordering::Relaxed),
                ),
                post_filter_enabled: self.post_filter_enabled.load(Ordering::Relaxed),
                post_filter_beta: f32::from_bits(self.post_filter_beta.load(Ordering::Relaxed)),
                mix: f32::from_bits(self.mix.load(Ordering::Relaxed)),
                min_db_thresh: f32::from_bits(self.min_db_thresh.load(Ordering::Relaxed)),
                max_db_erb_thresh: f32::from_bits(self.max_db_erb_thresh.load(Ordering::Relaxed)),
                max_db_df_thresh: f32::from_bits(self.max_db_df_thresh.load(Ordering::Relaxed)),
                bypass: self.bypass.load(Ordering::Relaxed),
            },
            model: model_from_u8(self.model.load(Ordering::Relaxed)),
            low_latency: self.low_latency.load(Ordering::Relaxed),
            advanced: self.advanced.load(Ordering::Relaxed),
            sample_rate: self.sample_rate.load(Ordering::Relaxed),
        }
        .sanitized()
    }

    pub(crate) fn load_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

fn model_to_u8(model: ModelKind) -> u8 {
    match model {
        ModelKind::LowLatency => 0,
        ModelKind::Standard => 1,
    }
}

fn model_from_u8(value: u8) -> ModelKind {
    match value {
        1 => ModelKind::Standard,
        _ => ModelKind::LowLatency,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn non_default() -> Settings {
        Settings {
            denoise: DenoiseParams {
                enabled: true,
                attenuation_limit_db: 24.0,
                post_filter_enabled: true,
                post_filter_beta: 0.01,
                mix: 0.75,
                min_db_thresh: -12.0,
                max_db_erb_thresh: 28.0,
                max_db_df_thresh: 18.0,
                bypass: true,
            },
            model: ModelKind::Standard,
            low_latency: true,
            advanced: true,
            sample_rate: 44_100,
        }
    }

    #[test]
    fn round_trips_settings() {
        let settings = non_default();
        let store = AtomicParams::new(settings);
        assert_eq!(store.load(), settings.sanitized());
    }

    #[test]
    fn store_replaces_all_fields() {
        let store = AtomicParams::new(Settings::default());
        store.store(non_default());
        assert_eq!(store.load(), non_default().sanitized());
    }

    #[test]
    fn clamps_invalid_values() {
        let mut settings = Settings::default();
        settings.denoise.attenuation_limit_db = 300.0;
        settings.denoise.mix = 2.0;
        let store = AtomicParams::new(settings);
        let loaded = store.load();
        assert_eq!(loaded.denoise.attenuation_limit_db, 100.0);
        assert_eq!(loaded.denoise.mix, 1.0);
    }
}
