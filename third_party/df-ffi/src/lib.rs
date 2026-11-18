use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr::null_mut;

use df::tract::{DfParams, DfTract, RuntimeParams};
use ndarray::{ArrayView2, ArrayViewMut2};

pub struct DfHandle {
    tract: DfTract,
}

/// Create a DeepFilterNet state using the embedded DeepFilterNet3 model.
///
/// Returns null on failure. The caller must free the handle with
/// `dfx_destroy`.
#[no_mangle]
pub extern "C" fn dfx_create(atten_lim_db: f32, post_filter_beta: f32) -> *mut DfHandle {
    let result = catch_unwind(|| {
        let beta = if post_filter_beta.is_finite() {
            post_filter_beta.max(0.0)
        } else {
            0.0
        };

        let params = DfParams::default();
        let runtime = RuntimeParams::default_with_ch(1)
            .with_atten_lim(atten_lim_db)
            .with_post_filter(beta);

        let tract = DfTract::new(params, &runtime).ok()?;
        Some(Box::into_raw(Box::new(DfHandle { tract })))
    });

    match result {
        Ok(Some(handle)) => handle,
        _ => null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn dfx_destroy(handle: *mut DfHandle) {
    if handle.is_null() {
        return;
    }

    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        drop(Box::from_raw(handle));
    }));
}

#[no_mangle]
pub extern "C" fn dfx_hop_size(handle: *const DfHandle) -> usize {
    if handle.is_null() {
        return 0;
    }

    unsafe { (*handle).tract.hop_size }
}

#[no_mangle]
pub extern "C" fn dfx_sample_rate(handle: *const DfHandle) -> usize {
    if handle.is_null() {
        return 0;
    }

    unsafe { (*handle).tract.sr }
}

/// Process exactly `dfx_hop_size()` samples.
///
/// Returns 0 on success, a negative value on error.
#[no_mangle]
pub extern "C" fn dfx_process(handle: *mut DfHandle, input: *const f32, output: *mut f32) -> i32 {
    if handle.is_null() || input.is_null() || output.is_null() {
        return -1;
    }

    let result = catch_unwind(AssertUnwindSafe(|| unsafe {
        let handle = &mut *handle;
        let hop = handle.tract.hop_size;

        let input = std::slice::from_raw_parts(input, hop);
        let output = std::slice::from_raw_parts_mut(output, hop);

        let input = ArrayView2::from_shape((1, hop), input).map_err(|_| -2)?;
        let output = ArrayViewMut2::from_shape((1, hop), output).map_err(|_| -2)?;

        handle.tract.process(input, output).map_err(|_| -3)?;
        Ok::<i32, i32>(0)
    }));

    match result {
        Ok(Ok(code)) => code,
        Ok(Err(code)) => code,
        Err(_) => -4,
    }
}

#[no_mangle]
pub extern "C" fn dfx_set_atten_lim(handle: *mut DfHandle, db: f32) {
    if handle.is_null() || !db.is_finite() {
        return;
    }

    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        (*handle).tract.set_atten_lim(db);
    }));
}

#[no_mangle]
pub extern "C" fn dfx_set_post_filter_beta(handle: *mut DfHandle, beta: f32) {
    if handle.is_null() || !beta.is_finite() {
        return;
    }

    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        (*handle).tract.set_pf_beta(beta.max(0.0));
    }));
}
