use std::ffi::{CStr, CString};
use std::path::PathBuf;

use df_core::ModelKind;

use crate::ffi;
use crate::module;

#[derive(Clone, Debug, Default)]
pub(crate) struct ModelPaths {
    pub low_latency: Option<PathBuf>,
    pub standard: Option<PathBuf>,
}

impl ModelPaths {
    pub(crate) fn path(&self, kind: ModelKind) -> Option<&PathBuf> {
        match kind {
            ModelKind::LowLatency => self.low_latency.as_ref(),
            ModelKind::Standard => self.standard.as_ref(),
        }
    }
}

pub(crate) fn resolve_all() -> ModelPaths {
    ModelPaths {
        low_latency: resolve_model_path(ModelKind::LowLatency),
        standard: resolve_model_path(ModelKind::Standard),
    }
}

pub(crate) fn resolve_model_path(kind: ModelKind) -> Option<PathBuf> {
    let module = module::obs_current_module();
    let relative = format!("models/{}", kind.file_name());
    let Ok(relative_c) = CString::new(relative) else {
        return None;
    };

    let found = unsafe { ffi::obs_find_module_file(module, relative_c.as_ptr()) };
    if !found.is_null() {
        let path = unsafe { CStr::from_ptr(found) }
            .to_string_lossy()
            .into_owned();
        unsafe { ffi::bfree(found.cast()) };
        return Some(PathBuf::from(path));
    }

    let data_path = unsafe { ffi::obs_get_module_data_path(module) };
    if data_path.is_null() {
        return None;
    }
    let data_path = unsafe { CStr::from_ptr(data_path) }
        .to_string_lossy()
        .into_owned();
    Some(
        PathBuf::from(data_path)
            .join("models")
            .join(kind.file_name()),
    )
}
