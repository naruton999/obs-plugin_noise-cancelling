use crate::DenoiseParams;

#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    pub name: String,
    pub description: String,
    pub params: DenoiseParams,
    pub bypass: bool,
}

impl Default for Preset {
    fn default() -> Self {
        Self {
            name: "standard".to_string(),
            description: String::new(),
            params: DenoiseParams::default(),
            bypass: false,
        }
    }
}

pub fn parse_preset(text: &str) -> Result<Preset, String> {
    let mut preset = Preset::default();
    let mut version: Option<i32> = None;
    let mut section = "";
    let mut post_filter_beta = 0.0f32;

    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len() - 1];
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();

        if section.is_empty() {
            match key {
                "schema_version" => {
                    version = value
                        .parse::<i32>()
                        .map_err(|_| format!("invalid schema_version: {value}"))?
                        .into();
                }
                "name" => preset.name = unquote(value).to_string(),
                "description" => preset.description = unquote(value).to_string(),
                _ => {}
            }
            continue;
        }

        match section {
            "denoise" => match key {
                "enabled" => preset.params.enabled = parse_bool(value),
                "attenuation_limit_db" => preset.params.attenuation_limit_db = parse_f32(value)?,
                "post_filter_beta" => post_filter_beta = parse_f32(value)?,
                "mix" => preset.params.mix = parse_f32(value)?,
                "min_db_thresh" => preset.params.min_db_thresh = parse_f32(value)?,
                "max_db_erb_thresh" => preset.params.max_db_erb_thresh = parse_f32(value)?,
                "max_db_df_thresh" => preset.params.max_db_df_thresh = parse_f32(value)?,
                _ => {}
            },
            "ui" => {
                if key == "bypass" {
                    preset.bypass = parse_bool(value);
                }
            }
            _ => {}
        }
    }

    match version {
        Some(1) => {}
        Some(other) => return Err(format!("unsupported schema_version: {other}")),
        None => return Err("missing schema_version".to_string()),
    }

    preset.params.post_filter_beta = post_filter_beta;
    preset.params.post_filter_enabled = post_filter_beta > 0.0;
    preset.params = preset.params.sanitized();
    Ok(preset)
}

fn parse_bool(value: &str) -> bool {
    value.eq_ignore_ascii_case("true")
}

fn parse_f32(value: &str) -> Result<f32, String> {
    value
        .parse::<f32>()
        .map_err(|_| format!("invalid number: {value}"))
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_preset() {
        let preset = parse_preset(include_str!("../../../presets/standard.toml")).unwrap();
        assert_eq!(preset.name, "standard");
        assert_eq!(preset.params.attenuation_limit_db, 100.0);
        assert!(!preset.params.post_filter_enabled);
        assert_eq!(preset.params.mix, 1.0);
        assert!(preset.params.enabled);
        assert!(!preset.bypass);
    }

    #[test]
    fn parses_all_bundled_presets() {
        for text in [
            include_str!("../../../presets/standard.toml"),
            include_str!("../../../presets/gentle.toml"),
            include_str!("../../../presets/strong.toml"),
            include_str!("../../../presets/speech.toml"),
        ] {
            parse_preset(text).unwrap();
        }
    }

    #[test]
    fn strong_enables_post_filter() {
        let preset = parse_preset(include_str!("../../../presets/strong.toml")).unwrap();
        assert!(preset.params.post_filter_enabled);
        assert!((preset.params.post_filter_beta - 0.02).abs() < 1e-6);
    }

    #[test]
    fn ignores_unknown_fields() {
        let text = r#"
schema_version = 1
name = "test"
unknown = 1

[denoise]
attenuation_limit_db = 12.0
future_field = true

[other]
x = 1
"#;
        let preset = parse_preset(text).unwrap();
        assert_eq!(preset.name, "test");
        assert_eq!(preset.params.attenuation_limit_db, 12.0);
    }

    #[test]
    fn rejects_unsupported_schema() {
        let text = "schema_version = 2\n[denoise]\nmix = 1.0\n";
        assert!(parse_preset(text).is_err());
    }

    #[test]
    fn rejects_missing_schema() {
        assert!(parse_preset("[denoise]\nmix = 1.0\n").is_err());
    }

    #[test]
    fn clamps_out_of_range_values() {
        let text = r#"
schema_version = 1
[denoise]
enabled = true
attenuation_limit_db = 300.0
post_filter_beta = 0.5
mix = 2.0
min_db_thresh = -100.0
max_db_erb_thresh = 100.0
max_db_df_thresh = 100.0
"#;
        let preset = parse_preset(text).unwrap();
        assert_eq!(preset.params.attenuation_limit_db, 100.0);
        assert_eq!(preset.params.post_filter_beta, 0.05);
        assert_eq!(preset.params.mix, 1.0);
        assert_eq!(preset.params.min_db_thresh, -15.0);
        assert_eq!(preset.params.max_db_erb_thresh, 35.0);
        assert_eq!(preset.params.max_db_df_thresh, 35.0);
    }
}
