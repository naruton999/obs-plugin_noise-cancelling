//! OBS audio filter boundary for obs-plugin_noise-cancelling.

#![cfg_attr(test, allow(dead_code))]

#[cfg(all(
    not(any(target_os = "macos", target_os = "windows")),
    not(test),
    not(feature = "check-only")
))]
compile_error!("obs-filter supports macOS and Windows only");

mod ffi;
mod layout;
mod params;

#[cfg(any(not(test), feature = "check-only"))]
mod filter;
#[cfg(any(not(test), feature = "check-only"))]
mod models;
#[cfg(any(not(test), feature = "check-only"))]
mod module;
#[cfg(any(not(test), feature = "check-only"))]
mod preset;
#[cfg(any(not(test), feature = "check-only"))]
mod ui;

#[cfg(any(not(test), feature = "check-only"))]
pub(crate) fn guard<R>(default: R, f: impl FnOnce() -> R) -> R {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(value) => value,
        Err(_) => default,
    }
}

#[cfg(any(not(test), feature = "check-only"))]
pub(crate) fn log(level: i32, message: &str) {
    use std::ffi::CString;

    if let Ok(message) = CString::new(message) {
        unsafe {
            ffi::blog(level, c"%s".as_ptr(), message.as_ptr());
        }
    }
}
