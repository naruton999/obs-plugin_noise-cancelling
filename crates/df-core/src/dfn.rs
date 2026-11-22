use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use df::tract::{DfParams, DfTract, RuntimeParams};
use ndarray::Array2;

use crate::backend::Backend;
use crate::{DenoiseParams, ModelKind};

/// DeepFilterNet always runs at 48 kHz.
pub const DFN_SAMPLE_RATE: u32 = 48_000;

/// DeepFilterNet3 backend using an external or embedded `.tar.gz` model.
pub struct DfnBackend {
    df: DfTract,
    noisy: Array2<f32>,
    enhanced: Array2<f32>,
    latency: usize,
    params: DenoiseParams,
}

impl DfnBackend {
    /// Load the selected model from the compile-time embedded blob when the
    /// corresponding `embed-*` feature is enabled, otherwise from `path`.
    pub fn from_model(kind: ModelKind, path: Option<&Path>) -> Result<Self> {
        #[cfg(feature = "embed-ll")]
        if kind == ModelKind::LowLatency {
            return Self::from_bytes(df_models::LOW_LATENCY)
                .context("parse embedded low-latency model");
        }

        #[cfg(feature = "embed-standard")]
        if kind == ModelKind::Standard {
            return Self::from_bytes(df_models::STANDARD).context("parse embedded standard model");
        }

        let path = path.ok_or_else(|| anyhow::anyhow!("model file not found for {}", kind.id()))?;
        Self::from_file(path)
    }

    pub fn from_file(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let params = DfParams::new(path.clone())
            .with_context(|| format!("open DeepFilterNet model: {}", path.display()))?;
        Self::from_params(params)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let params = DfParams::from_bytes(bytes).context("parse DeepFilterNet model bytes")?;
        Self::from_params(params)
    }

    fn from_params(params: DfParams) -> Result<Self> {
        let runtime = RuntimeParams::default_with_ch(1);
        let df = DfTract::new(params, &runtime).context("initialize DeepFilterNet")?;
        let hop = df.hop_size;
        let latency = (df.lookahead + 1) * hop;
        Ok(Self {
            noisy: Array2::zeros((1, hop)),
            enhanced: Array2::zeros((1, hop)),
            df,
            latency,
            params: DenoiseParams::default(),
        })
    }

    pub fn hop_size(&self) -> usize {
        self.df.hop_size
    }

    pub fn latency_samples(&self) -> usize {
        self.latency
    }
}

impl Backend for DfnBackend {
    fn sample_rate(&self) -> u32 {
        self.df.sr as u32
    }

    fn hop_size(&self) -> usize {
        self.df.hop_size
    }

    fn latency_samples(&self) -> usize {
        self.latency
    }

    fn set_params(&mut self, params: &DenoiseParams) {
        self.params = *params;
        self.df.set_atten_lim(params.attenuation_limit_db);
        let beta = if params.post_filter_enabled {
            params.post_filter_beta
        } else {
            0.0
        };
        self.df.set_pf_beta(beta.clamp(0.0, 0.05));
        self.df.min_db_thresh = params.min_db_thresh;
        self.df.max_db_erb_thresh = params.max_db_erb_thresh;
        self.df.max_db_df_thresh = params.max_db_df_thresh;
    }

    fn process_hop(&mut self, input: &[f32], output: &mut [f32]) -> Result<f32> {
        anyhow::ensure!(input.len() == self.df.hop_size, "invalid input hop size");
        anyhow::ensure!(output.len() == self.df.hop_size, "invalid output hop size");

        for (dst, &src) in self.noisy.row_mut(0).iter_mut().zip(input.iter()) {
            *dst = src;
        }

        let lsnr = self
            .df
            .process(self.noisy.view(), self.enhanced.view_mut())
            .context("DeepFilterNet process failed")?;

        for (dst, &src) in output.iter_mut().zip(self.enhanced.row(0).iter()) {
            *dst = src;
        }

        Ok(lsnr)
    }

    fn reset(&mut self) -> Result<()> {
        self.df.init().context("DeepFilterNet reset failed")
    }
}

impl DfnBackend {
    pub fn current_params(&self) -> &DenoiseParams {
        &self.params
    }
}
