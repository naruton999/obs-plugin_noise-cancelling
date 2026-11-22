use std::ffi::{CStr, CString, c_void};
use std::ptr;
use std::sync::atomic::Ordering;

use df_core::{DenoiseParams, ModelKind};

use crate::ffi::{
    self, LOG_INFO, OBS_COMBO_FORMAT_STRING, OBS_COMBO_TYPE_LIST, OBS_TEXT_INFO, ObsData,
    ObsProperties, ObsProperty,
};
use crate::filter::{
    self, FilterData, STATUS_FAILED, STATUS_LOADING, STATUS_READY, STATUS_UNSUPPORTED,
};
use crate::guard;
use crate::log;
use crate::models::ModelPaths;
use crate::params::Settings;
use crate::preset::{PresetEntry, load_presets};

pub(crate) unsafe extern "C" fn get_defaults(settings: *mut ObsData) {
    guard((), || {
        if settings.is_null() {
            return;
        }

        let defaults = DenoiseParams::default();
        unsafe {
            ffi::obs_data_set_default_bool(settings, c"enabled".as_ptr(), defaults.enabled);
            ffi::obs_data_set_default_bool(settings, c"bypass".as_ptr(), defaults.bypass);
            ffi::obs_data_set_default_double(
                settings,
                c"attenuation_limit_db".as_ptr(),
                defaults.attenuation_limit_db as f64,
            );
            ffi::obs_data_set_default_bool(
                settings,
                c"post_filter_enabled".as_ptr(),
                defaults.post_filter_enabled,
            );
            ffi::obs_data_set_default_double(
                settings,
                c"post_filter_beta".as_ptr(),
                defaults.post_filter_beta as f64,
            );
            ffi::obs_data_set_default_double(
                settings,
                c"mix".as_ptr(),
                defaults.mix as f64 * 100.0,
            );
            ffi::obs_data_set_default_bool(settings, c"declick".as_ptr(), false);
            ffi::obs_data_set_default_double(settings, c"voice_protect".as_ptr(), 0.0);
            ffi::obs_data_set_default_double(
                settings,
                c"min_db_thresh".as_ptr(),
                defaults.min_db_thresh as f64,
            );
            ffi::obs_data_set_default_double(
                settings,
                c"max_db_erb_thresh".as_ptr(),
                defaults.max_db_erb_thresh as f64,
            );
            ffi::obs_data_set_default_double(
                settings,
                c"max_db_df_thresh".as_ptr(),
                defaults.max_db_df_thresh as f64,
            );
            ffi::obs_data_set_default_string(settings, c"model".as_ptr(), c"ll".as_ptr());
            ffi::obs_data_set_default_bool(settings, c"low_latency".as_ptr(), false);
            ffi::obs_data_set_default_bool(settings, c"advanced".as_ptr(), false);
            ffi::obs_data_set_default_string(settings, c"preset".as_ptr(), c"standard".as_ptr());
        }
    });
}

pub(crate) unsafe extern "C" fn update(data: *mut c_void, settings: *mut ObsData) {
    guard((), || {
        if data.is_null() || settings.is_null() {
            return;
        }

        let filter = unsafe { &*data.cast::<FilterData>() };
        let sample_rate = filter::audio_sample_rate();
        let settings = unsafe { read_settings(settings, sample_rate) };
        filter.shared.params.store(settings);
    });
}

pub(crate) unsafe extern "C" fn get_properties(data: *mut c_void) -> *mut ObsProperties {
    guard(ptr::null_mut(), || {
        let (settings, status, latency, underruns, overruns, model_paths) = if data.is_null() {
            (
                Settings::default(),
                STATUS_LOADING,
                0,
                0,
                0,
                ModelPaths::default(),
            )
        } else {
            let filter = unsafe { &*data.cast::<FilterData>() };
            (
                filter.shared.params.load(),
                filter.shared.status.load(Ordering::Acquire),
                filter.shared.latency.load(Ordering::Acquire),
                filter.shared.underruns.load(Ordering::Relaxed),
                filter.shared.overruns.load(Ordering::Relaxed),
                filter.model_paths.clone(),
            )
        };

        let presets = load_presets();
        let props = unsafe { ffi::obs_properties_create() };
        if props.is_null() {
            return props;
        }

        unsafe {
            build_properties(
                props,
                &settings,
                status,
                latency,
                underruns,
                overruns,
                &model_paths,
                &presets,
            );
        }
        props
    })
}

#[allow(clippy::too_many_arguments)]
unsafe fn build_properties(
    props: *mut ObsProperties,
    settings: &Settings,
    status: u32,
    latency: u64,
    underruns: u64,
    overruns: u64,
    model_paths: &ModelPaths,
    presets: &[PresetEntry],
) {
    unsafe {
        let preset_list = ffi::obs_properties_add_list(
            props,
            c"preset".as_ptr(),
            c"プリセット".as_ptr(),
            OBS_COMBO_TYPE_LIST,
            OBS_COMBO_FORMAT_STRING,
        );
        if !preset_list.is_null() {
            for entry in presets {
                if let Ok(name) = CString::new(entry.file.as_str()) {
                    ffi::obs_property_list_add_string(preset_list, name.as_ptr(), name.as_ptr());
                }
            }
        }
        ffi::obs_properties_add_button(
            props,
            c"preset_apply".as_ptr(),
            c"プリセットを適用".as_ptr(),
            Some(apply_preset),
        );
        if presets.is_empty() {
            add_info(props, "preset_error", "プリセットを読み込めませんでした");
        }

        let model_list = ffi::obs_properties_add_list(
            props,
            c"model".as_ptr(),
            c"モデル".as_ptr(),
            OBS_COMBO_TYPE_LIST,
            OBS_COMBO_FORMAT_STRING,
        );
        if !model_list.is_null() {
            for kind in [ModelKind::LowLatency, ModelKind::Standard] {
                let missing = !df_core::model_embedded(kind)
                    && model_paths
                        .path(kind)
                        .map(|path| !path.is_file())
                        .unwrap_or(true);
                let label = if missing {
                    format!("{}（未検出）", kind.label())
                } else {
                    kind.label().to_string()
                };
                if let (Ok(label), Ok(value)) = (CString::new(label), CString::new(kind.id())) {
                    ffi::obs_property_list_add_string(model_list, label.as_ptr(), value.as_ptr());
                }
            }
        }

        ffi::obs_properties_add_bool(
            props,
            c"low_latency".as_ptr(),
            c"低遅延モード（1 hop、約 -10 ms）".as_ptr(),
        );
        ffi::obs_properties_add_bool(
            props,
            c"enabled".as_ptr(),
            c"ノイズキャンセリングを有効にする".as_ptr(),
        );

        let attenuation = ffi::obs_properties_add_float_slider(
            props,
            c"attenuation_limit_db".as_ptr(),
            c"低減量".as_ptr(),
            0.0,
            100.0,
            1.0,
        );
        if !attenuation.is_null() {
            ffi::obs_property_float_set_suffix(attenuation, c" dB".as_ptr());
        }

        let mix = ffi::obs_properties_add_float_slider(
            props,
            c"mix".as_ptr(),
            c"ミックス".as_ptr(),
            0.0,
            100.0,
            1.0,
        );
        if !mix.is_null() {
            ffi::obs_property_float_set_suffix(mix, c" %".as_ptr());
        }

        ffi::obs_properties_add_bool(
            props,
            c"declick".as_ptr(),
            c"クリック音除去（試作）".as_ptr(),
        );
        let voice_protect = ffi::obs_properties_add_float_slider(
            props,
            c"voice_protect".as_ptr(),
            c"声の保護（試作）".as_ptr(),
            0.0,
            50.0,
            1.0,
        );
        if !voice_protect.is_null() {
            ffi::obs_property_float_set_suffix(voice_protect, c" %".as_ptr());
        }

        let post_filter = ffi::obs_properties_add_bool(
            props,
            c"post_filter_enabled".as_ptr(),
            c"ポストフィルタ".as_ptr(),
        );
        let beta = ffi::obs_properties_add_float_slider(
            props,
            c"post_filter_beta".as_ptr(),
            c"ポストフィルタ β".as_ptr(),
            0.0,
            0.05,
            0.001,
        );
        if !beta.is_null() {
            ffi::obs_property_set_enabled(beta, settings.denoise.post_filter_enabled);
        }
        if !post_filter.is_null() {
            ffi::obs_property_set_modified_callback(post_filter, Some(post_filter_changed));
        }

        let advanced = ffi::obs_properties_add_bool(
            props,
            c"advanced".as_ptr(),
            c"上級者向けしきい値".as_ptr(),
        );
        add_threshold(props, c"min_db_thresh", "Min", settings.advanced);
        add_threshold(props, c"max_db_erb_thresh", "ERB", settings.advanced);
        add_threshold(props, c"max_db_df_thresh", "DF", settings.advanced);
        if !advanced.is_null() {
            ffi::obs_property_set_modified_callback(advanced, Some(advanced_changed));
        }

        ffi::obs_properties_add_bool(props, c"bypass".as_ptr(), c"遅延維持バイパス".as_ptr());

        add_info(props, "status", status_text(status, settings));
        add_info(props, "rate", &rate_text(settings.sample_rate));
        add_info(
            props,
            "latency",
            &latency_text(latency, settings.sample_rate),
        );
        add_info(props, "version", concat!("v", env!("CARGO_PKG_VERSION")));

        let missing = [ModelKind::LowLatency, ModelKind::Standard]
            .iter()
            .any(|kind| {
                !df_core::model_embedded(*kind)
                    && model_paths
                        .path(*kind)
                        .map(|path| !path.is_file())
                        .unwrap_or(true)
            });
        if missing {
            add_info(
                props,
                "model_warning",
                "モデルファイルが見つかりません。models/ に 2 つの tar.gz を配置してください。",
            );
        }
        if underruns != 0 || overruns != 0 {
            add_info(
                props,
                "runtime_counters",
                &format!("アンダーラン: {underruns} / オーバーラン: {overruns}"),
            );
        }
    }
}

unsafe fn add_threshold(
    props: *mut ObsProperties,
    name: &std::ffi::CStr,
    label: &str,
    enabled: bool,
) {
    let Ok(label) = CString::new(label) else {
        return;
    };
    let property = unsafe {
        ffi::obs_properties_add_float_slider(props, name.as_ptr(), label.as_ptr(), -15.0, 35.0, 0.1)
    };
    if !property.is_null() {
        unsafe {
            ffi::obs_property_float_set_suffix(property, c" dB".as_ptr());
            ffi::obs_property_set_enabled(property, enabled);
        }
    }
}

fn add_info(props: *mut ObsProperties, name: &str, text: &str) {
    let (Ok(name), Ok(text)) = (CString::new(name), CString::new(text)) else {
        return;
    };
    unsafe {
        ffi::obs_properties_add_text(props, name.as_ptr(), text.as_ptr(), OBS_TEXT_INFO);
    }
}

unsafe extern "C" fn post_filter_changed(
    props: *mut ObsProperties,
    _property: *mut ObsProperty,
    settings: *mut ObsData,
) -> bool {
    guard(false, || {
        if props.is_null() || settings.is_null() {
            return false;
        }
        let enabled = unsafe { ffi::obs_data_get_bool(settings, c"post_filter_enabled".as_ptr()) };
        let beta = unsafe { ffi::obs_properties_get(props, c"post_filter_beta".as_ptr()) };
        if !beta.is_null() {
            unsafe { ffi::obs_property_set_enabled(beta, enabled) };
        }
        true
    })
}

unsafe extern "C" fn advanced_changed(
    props: *mut ObsProperties,
    _property: *mut ObsProperty,
    settings: *mut ObsData,
) -> bool {
    guard(false, || {
        if props.is_null() || settings.is_null() {
            return false;
        }
        let enabled = unsafe { ffi::obs_data_get_bool(settings, c"advanced".as_ptr()) };
        for name in ["min_db_thresh", "max_db_erb_thresh", "max_db_df_thresh"] {
            let Ok(name) = CString::new(name) else {
                continue;
            };
            let property = unsafe { ffi::obs_properties_get(props, name.as_ptr()) };
            if !property.is_null() {
                unsafe { ffi::obs_property_set_enabled(property, enabled) };
            }
        }
        true
    })
}

unsafe extern "C" fn apply_preset(
    _props: *mut ObsProperties,
    _property: *mut ObsProperty,
    data: *mut c_void,
) -> bool {
    guard(false, || {
        if data.is_null() {
            return false;
        }

        let filter = unsafe { &*data.cast::<FilterData>() };
        if filter.source.is_null() {
            return false;
        }

        let presets = load_presets();
        if presets.is_empty() {
            return false;
        }

        let settings = unsafe { ffi::obs_source_get_settings(filter.source) };
        if settings.is_null() {
            return false;
        }

        let selected = unsafe { get_string(settings, c"preset") };
        let chosen = presets
            .iter()
            .find(|entry| entry.file == selected)
            .or_else(|| presets.iter().find(|entry| entry.file == "standard"))
            .unwrap_or(&presets[0]);

        unsafe {
            let params = chosen.preset.params;
            ffi::obs_data_set_bool(settings, c"enabled".as_ptr(), params.enabled);
            ffi::obs_data_set_bool(settings, c"bypass".as_ptr(), chosen.preset.bypass);
            ffi::obs_data_set_double(
                settings,
                c"attenuation_limit_db".as_ptr(),
                params.attenuation_limit_db as f64,
            );
            ffi::obs_data_set_bool(
                settings,
                c"post_filter_enabled".as_ptr(),
                params.post_filter_enabled,
            );
            ffi::obs_data_set_double(
                settings,
                c"post_filter_beta".as_ptr(),
                params.post_filter_beta as f64,
            );
            ffi::obs_data_set_double(settings, c"mix".as_ptr(), params.mix as f64 * 100.0);
            ffi::obs_data_set_double(
                settings,
                c"min_db_thresh".as_ptr(),
                params.min_db_thresh as f64,
            );
            ffi::obs_data_set_double(
                settings,
                c"max_db_erb_thresh".as_ptr(),
                params.max_db_erb_thresh as f64,
            );
            ffi::obs_data_set_double(
                settings,
                c"max_db_df_thresh".as_ptr(),
                params.max_db_df_thresh as f64,
            );
            ffi::obs_data_set_bool(settings, c"declick".as_ptr(), params.declick);
            ffi::obs_data_set_double(
                settings,
                c"voice_protect".as_ptr(),
                params.voice_protect as f64 * 100.0,
            );

            if let Ok(file) = CString::new(chosen.file.as_str()) {
                ffi::obs_data_set_string(settings, c"preset".as_ptr(), file.as_ptr());
            }

            ffi::obs_source_update(filter.source, settings);
            ffi::obs_data_release(settings);
        }

        log(LOG_INFO, &format!("applied preset: {}", chosen.file));
        true
    })
}

pub(crate) unsafe fn read_settings(data: *mut ObsData, sample_rate: u32) -> Settings {
    let denoise = DenoiseParams {
        enabled: unsafe { ffi::obs_data_get_bool(data, c"enabled".as_ptr()) },
        attenuation_limit_db: unsafe {
            ffi::obs_data_get_double(data, c"attenuation_limit_db".as_ptr())
        } as f32,
        post_filter_enabled: unsafe {
            ffi::obs_data_get_bool(data, c"post_filter_enabled".as_ptr())
        },
        post_filter_beta: unsafe { ffi::obs_data_get_double(data, c"post_filter_beta".as_ptr()) }
            as f32,
        mix: unsafe { ffi::obs_data_get_double(data, c"mix".as_ptr()) } as f32 / 100.0,
        min_db_thresh: unsafe { ffi::obs_data_get_double(data, c"min_db_thresh".as_ptr()) } as f32,
        max_db_erb_thresh: unsafe { ffi::obs_data_get_double(data, c"max_db_erb_thresh".as_ptr()) }
            as f32,
        max_db_df_thresh: unsafe { ffi::obs_data_get_double(data, c"max_db_df_thresh".as_ptr()) }
            as f32,
        bypass: unsafe { ffi::obs_data_get_bool(data, c"bypass".as_ptr()) },
        declick: unsafe { ffi::obs_data_get_bool(data, c"declick".as_ptr()) },
        voice_protect: unsafe { ffi::obs_data_get_double(data, c"voice_protect".as_ptr()) } as f32
            / 100.0,
    };

    let model = ModelKind::from_id(&unsafe { get_string(data, c"model") }).unwrap_or_default();

    Settings {
        denoise,
        model,
        low_latency: unsafe { ffi::obs_data_get_bool(data, c"low_latency".as_ptr()) },
        advanced: unsafe { ffi::obs_data_get_bool(data, c"advanced".as_ptr()) },
        sample_rate,
    }
    .sanitized()
}

unsafe fn get_string(data: *mut ObsData, name: &std::ffi::CStr) -> String {
    let value = unsafe { ffi::obs_data_get_string(data, name.as_ptr()) };
    if value.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(value) }
            .to_string_lossy()
            .into_owned()
    }
}

fn status_text(status: u32, settings: &Settings) -> &'static str {
    if !settings.denoise.enabled {
        return "無効（素通し）";
    }
    match status {
        STATUS_READY if settings.denoise.bypass => "バイパス中（遅延維持）",
        STATUS_READY => "動作中",
        STATUS_LOADING => "モデル読込中 / 切替中",
        STATUS_FAILED => "モデル読込失敗（素通し）",
        STATUS_UNSUPPORTED => "非対応レート（48 kHz / 44.1 kHz を使用してください）",
        _ => "不明",
    }
}

fn rate_text(rate: u32) -> String {
    match rate {
        48_000 => "レート: 48 kHz".to_string(),
        44_100 => "レート: 44.1 kHz（48 kHz へリサンプル）".to_string(),
        other => format!("レート: {other} Hz（非対応）"),
    }
}

fn latency_text(latency: u64, sample_rate: u32) -> String {
    if latency == 0 || sample_rate == 0 {
        "概算遅延: 計測中".to_string()
    } else {
        format!(
            "概算遅延: 約 {:.1} ms（+ OBS 約 21 ms）",
            latency as f64 / sample_rate as f64 * 1000.0
        )
    }
}
