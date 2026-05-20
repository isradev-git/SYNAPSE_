use serde::{Deserialize, Serialize};

fn default_scanline_intensity() -> f32 { 0.3 }
fn default_scanline_freq() -> f32 { 2.0 }
fn default_bloom_threshold() -> f32 { 0.7 }
fn default_bloom_sigma() -> f32 { 4.0 }
fn default_bloom_tint() -> String { "#FF003C".to_string() }
fn default_chroma_strength() -> f32 { 0.002 }
fn default_matrix_color() -> String { "#00FF55".to_string() }
fn default_matrix_density() -> f32 { 0.3 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScanlinesConfig {
    #[serde(default = "default_scanline_intensity")]
    pub intensity: f32,
    #[serde(default = "default_scanline_freq")]
    pub freq: f32,
}

impl Default for ScanlinesConfig {
    fn default() -> Self {
        Self { intensity: default_scanline_intensity(), freq: default_scanline_freq() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BloomConfig {
    #[serde(default = "default_bloom_threshold")]
    pub threshold: f32,
    #[serde(default = "default_bloom_sigma")]
    pub sigma: f32,
    #[serde(default = "default_bloom_tint")]
    pub tint: String,
}

impl Default for BloomConfig {
    fn default() -> Self {
        Self {
            threshold: default_bloom_threshold(),
            sigma: default_bloom_sigma(),
            tint: default_bloom_tint(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChromaConfig {
    #[serde(default = "default_chroma_strength")]
    pub strength: f32,
}

impl Default for ChromaConfig {
    fn default() -> Self { Self { strength: default_chroma_strength() } }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatrixBgConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_matrix_color")]
    pub color: String,
    #[serde(default = "default_matrix_density")]
    pub density: f32,
}

impl Default for MatrixBgConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            color: default_matrix_color(),
            density: default_matrix_density(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct EffectsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub scanlines: ScanlinesConfig,
    #[serde(default)]
    pub bloom: BloomConfig,
    #[serde(default)]
    pub chroma: ChromaConfig,
    #[serde(default)]
    pub matrix_bg: MatrixBgConfig,
    #[serde(default)]
    pub hex_grid: bool,
    #[serde(default)]
    pub pane_pulse: bool,
    #[serde(default)]
    pub cursor_trail: u8,
}

/// Parse a hex color string "#RRGGBB" into [r, g, b, 1.0] floats.
/// Returns cyan [0,1,1,1] as fallback for invalid input.
pub fn parse_hex_color(s: &str) -> [f32; 4] {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        return [0.0, 1.0, 1.0, 1.0];
    }
    let r = u8::from_str_radix(&s[0..2], 16);
    let g = u8::from_str_radix(&s[2..4], 16);
    let b = u8::from_str_radix(&s[4..6], 16);
    match (r, g, b) {
        (Ok(r), Ok(g), Ok(b)) => [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0],
        _ => [0.0, 1.0, 1.0, 1.0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effects_config_defaults() {
        let cfg = EffectsConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.scanlines.intensity, 0.3);
        assert_eq!(cfg.scanlines.freq, 2.0);
        assert_eq!(cfg.bloom.threshold, 0.7);
        assert_eq!(cfg.bloom.sigma, 4.0);
        assert_eq!(cfg.bloom.tint, "#FF003C");
        assert_eq!(cfg.chroma.strength, 0.002);
        assert!(!cfg.matrix_bg.enabled);
        assert_eq!(cfg.matrix_bg.density, 0.3);
        assert!(!cfg.hex_grid);
        assert!(!cfg.pane_pulse);
        assert_eq!(cfg.cursor_trail, 0);
    }

    #[test]
    fn test_effects_config_toml_round_trip() {
        let cfg = EffectsConfig {
            enabled: true,
            scanlines: ScanlinesConfig { intensity: 0.5, freq: 3.0 },
            cursor_trail: 4,
            pane_pulse: true,
            ..EffectsConfig::default()
        };
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let parsed: EffectsConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed, cfg);
    }

    #[test]
    fn test_effects_partial_toml() {
        let toml_str = r#"
enabled = true
cursor_trail = 3
pane_pulse = true
"#;
        let cfg: EffectsConfig = toml::from_str(toml_str).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.cursor_trail, 3);
        assert!(cfg.pane_pulse);
        assert_eq!(cfg.scanlines.intensity, 0.3);
        assert_eq!(cfg.bloom.threshold, 0.7);
    }

    #[test]
    fn test_parse_hex_color() {
        let c = parse_hex_color("#FF003C");
        assert!((c[0] - 1.0).abs() < 0.01);
        assert!(c[1] < 0.01);
        assert!((c[2] - 0.235).abs() < 0.01);
        assert_eq!(c[3], 1.0);

        let c = parse_hex_color("#00FF55");
        assert!(c[0] < 0.01);
        assert!((c[1] - 1.0).abs() < 0.01);
        assert!((c[2] - 0.333).abs() < 0.01);

        let c = parse_hex_color("notacolor");
        assert_eq!(c, [0.0, 1.0, 1.0, 1.0]);

        // Partially malformed (invalid hex chars) → cyan fallback
        let c = parse_hex_color("#FFZZ00");
        assert_eq!(c, [0.0, 1.0, 1.0, 1.0]);
    }
}
