//! OBS audio filter boundary for obs-plugin_noise-cancelling.

#[cfg(all(not(any(target_os = "macos", target_os = "windows")), not(test)))]
compile_error!("obs-filter supports macOS and Windows only");
