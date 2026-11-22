//! Optional embedded DeepFilterNet model blobs.
//!
//! This crate exists so the large model files are compiled separately from
//! `df-core`. When the model files do not change, Cargo can reuse the compiled
//! `df-models` rlib and only rebuild the filter code.

#[cfg(feature = "embed-ll")]
pub static LOW_LATENCY: &[u8] = include_bytes!("../../../models/DeepFilterNet3_ll_onnx.tar.gz");

#[cfg(feature = "embed-standard")]
pub static STANDARD: &[u8] = include_bytes!("../../../models/DeepFilterNet3_onnx.tar.gz");
