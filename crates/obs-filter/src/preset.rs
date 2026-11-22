use std::ffi::{CStr, CString};
use std::path::PathBuf;

use df_core::preset::{Preset, parse_preset};

use crate::ffi;
use crate::ffi::{LOG_INFO, LOG_WARNING};
use crate::log;
use crate::module;

pub(crate) struct PresetEntry {
    pub file: String,
    pub preset: Preset,
}

const BUILTIN_PRESETS: [(&str, &str); 5] = [
    ("gentle", include_str!("../../../presets/gentle.toml")),
    ("hybrid", include_str!("../../../presets/hybrid.toml")),
    ("speech", include_str!("../../../presets/speech.toml")),
    ("standard", include_str!("../../../presets/standard.toml")),
    ("strong", include_str!("../../../presets/strong.toml")),
];

pub(crate) fn load_presets() -> Vec<PresetEntry> {
    let mut entries = Vec::new();
    let mut skipped = 0usize;
    let mut source = "(built-in)".to_string();

    if let Some(dir) = presets_dir() {
        source = dir.display().to_string();
        match std::fs::read_dir(&dir) {
            Ok(read_dir) => {
                for entry in read_dir.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                        continue;
                    }
                    let Some(file) = path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .map(str::to_owned)
                    else {
                        continue;
                    };
                    let Ok(text) = std::fs::read_to_string(&path) else {
                        skipped += 1;
                        continue;
                    };
                    let Ok(preset) = parse_preset(&text) else {
                        skipped += 1;
                        continue;
                    };
                    entries.push(PresetEntry { file, preset });
                }
            }
            Err(err) => {
                log(
                    LOG_WARNING,
                    &format!("preset directory not readable: {} ({err})", dir.display()),
                );
            }
        }
    }

    for (file, text) in BUILTIN_PRESETS {
        if entries.iter().any(|entry| entry.file == file) {
            continue;
        }
        if let Ok(preset) = parse_preset(text) {
            entries.push(PresetEntry {
                file: file.to_string(),
                preset,
            });
        }
    }

    entries.sort_by(|a, b| a.file.cmp(&b.file));
    log(
        LOG_INFO,
        &format!(
            "loaded {} preset(s) from {source} (skipped {skipped})",
            entries.len()
        ),
    );
    entries
}

fn presets_dir() -> Option<PathBuf> {
    let module = module::obs_current_module();
    let probe = CString::new("presets/standard.toml").ok()?;
    let found = unsafe { ffi::obs_find_module_file(module, probe.as_ptr()) };
    if !found.is_null() {
        let path = unsafe { CStr::from_ptr(found) }
            .to_string_lossy()
            .into_owned();
        unsafe { ffi::bfree(found.cast()) };
        return std::path::Path::new(&path)
            .parent()
            .map(std::path::Path::to_path_buf);
    }

    let data_path = unsafe { ffi::obs_get_module_data_path(module) };
    if data_path.is_null() {
        return None;
    }
    let data_path = unsafe { CStr::from_ptr(data_path) }
        .to_string_lossy()
        .into_owned();
    Some(PathBuf::from(data_path).join("presets"))
}
