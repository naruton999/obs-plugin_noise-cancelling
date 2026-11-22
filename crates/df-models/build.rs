use std::env;
use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    if env::var_os("CARGO_FEATURE_EMBED_LL").is_some() {
        check_model(
            &root.join("models/DeepFilterNet3_ll_onnx.tar.gz"),
            "embed-ll",
        );
    }
    if env::var_os("CARGO_FEATURE_EMBED_STANDARD").is_some() {
        check_model(
            &root.join("models/DeepFilterNet3_onnx.tar.gz"),
            "embed-standard",
        );
    }
}

fn check_model(path: &Path, feature: &str) {
    if !path.is_file() {
        panic!(
            "{feature} requires the model file at {}\n\
             Run scripts/fetch-models.sh (or scripts/fetch-models.ps1) before building.",
            path.display()
        );
    }
    println!("cargo:rerun-if-changed={}", path.display());
}
