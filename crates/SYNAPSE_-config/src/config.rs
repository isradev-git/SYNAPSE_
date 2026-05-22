use crate::effects::EffectsConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BellConfig {
    #[serde(default = "default_true")]
    pub visual: bool,
    #[serde(default = "default_true")]
    pub notify_unfocused: bool,
}

impl Default for BellConfig {
    fn default() -> Self {
        Self {
            visual: true,
            notify_unfocused: true,
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CursorStyle {
    Block,
    Beam,
    Underline,
    NeonUnderbar,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default)]
    pub font_ligatures: bool,
    #[serde(default = "default_window_width")]
    pub window_width: u32,
    #[serde(default = "default_window_height")]
    pub window_height: u32,
    #[serde(default = "default_scrollback_lines")]
    pub scrollback_lines: usize,
    #[serde(default)]
    pub shell_program: String,
    #[serde(default)]
    pub shell_args: Vec<String>,
    #[serde(default = "default_cursor_style")]
    pub cursor_style: CursorStyle,
    #[serde(default = "default_cursor_blink")]
    pub cursor_blink: bool,
    #[serde(default = "default_cursor_blink_ms")]
    pub cursor_blink_ms: u64,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub effects: EffectsConfig,
    #[serde(default)]
    pub bell: BellConfig,
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: f32,
    #[serde(default = "default_true")]
    pub show_pane_labels: bool,
    #[serde(default = "default_true")]
    pub show_resize_indicator: bool,
    #[serde(default = "default_true")]
    pub sidebar_show_process_dot: bool,
    #[serde(default = "default_true")]
    pub status_bar: bool,
    #[serde(default = "default_true")]
    pub status_bar_show_git: bool,
    #[serde(default = "default_true")]
    pub status_bar_show_k8s: bool,
    #[serde(default = "default_true")]
    pub status_bar_show_time: bool,
}

fn default_font_size() -> f32 {
    14.0
}
fn default_font_family() -> String {
    "monospace".to_string()
}
fn default_window_width() -> u32 {
    1280
}
fn default_window_height() -> u32 {
    800
}
fn default_scrollback_lines() -> usize {
    100_000
}
fn default_cursor_style() -> CursorStyle {
    CursorStyle::Block
}
fn default_cursor_blink() -> bool {
    true
}
fn default_cursor_blink_ms() -> u64 {
    500
}
fn default_theme() -> String {
    "synapse_".to_string()
}
fn default_sidebar_width() -> f32 {
    180.0
}

impl Default for Config {
    fn default() -> Self {
        Self {
            font_size: default_font_size(),
            font_family: default_font_family(),
            font_ligatures: true,
            window_width: default_window_width(),
            window_height: default_window_height(),
            scrollback_lines: default_scrollback_lines(),
            shell_program: String::new(),
            shell_args: Vec::new(),
            cursor_style: default_cursor_style(),
            cursor_blink: default_cursor_blink(),
            cursor_blink_ms: default_cursor_blink_ms(),
            theme: default_theme(),
            effects: EffectsConfig::default(),
            bell: BellConfig::default(),
            sidebar_width: default_sidebar_width(),
            show_pane_labels: true,
            show_resize_indicator: true,
            sidebar_show_process_dot: true,
            status_bar: true,
            status_bar_show_git: true,
            status_bar_show_k8s: true,
            status_bar_show_time: true,
        }
    }
}

impl Config {
    pub fn config_dir() -> Option<PathBuf> {
        config_dir()
    }

    pub fn config_path() -> Option<PathBuf> {
        config_dir().map(|d| d.join("config.toml"))
    }

    pub fn load() -> Self {
        if let Some(path) = Self::config_path() {
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(config) = toml::from_str(&content) {
                        return config;
                    }
                }
            }
            let config = Config::default();
            let _ = config.save_to(&path);
            config
        } else {
            Config::default()
        }
    }

    pub fn reload(&mut self) {
        if let Some(path) = Self::config_path() {
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(config) = toml::from_str(&content) {
                        *self = config;
                    }
                }
            }
        }
    }

    pub fn save(&self) -> Result<(), String> {
        if let Some(path) = Self::config_path() {
            self.save_to(&path)
        } else {
            Err("No config directory found".into())
        }
    }

    fn save_to(&self, path: &PathBuf) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}", e))?;
        }
        let content = toml::to_string_pretty(self).map_err(|e| format!("{}", e))?;
        std::fs::write(path, content).map_err(|e| format!("{}", e))
    }
}

fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
            .ok()
            .map(|d| d.join("SYNAPSE_"))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var("HOME")
            .map(|h| {
                PathBuf::from(h)
                    .join("Library")
                    .join("Application Support")
                    .join("SYNAPSE_")
            })
            .ok()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bell_config_default() {
        let cfg = BellConfig::default();
        assert!(cfg.visual);
        assert!(cfg.notify_unfocused);
    }

    #[test]
    fn test_cursor_style_serde() {
        let config = Config {
            cursor_style: CursorStyle::Block,
            ..Config::default()
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.cursor_style, CursorStyle::Block);

        let config = Config {
            cursor_style: CursorStyle::Beam,
            ..Config::default()
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.cursor_style, CursorStyle::Beam);

        let config = Config {
            cursor_style: CursorStyle::Underline,
            ..Config::default()
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.cursor_style, CursorStyle::Underline);

        let config = Config {
            cursor_style: CursorStyle::NeonUnderbar,
            ..Config::default()
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.cursor_style, CursorStyle::NeonUnderbar);
    }

    #[test]
    fn test_neon_underbar_toml_parse() {
        let toml_str = r#"cursor_style = "neon_underbar""#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.cursor_style, CursorStyle::NeonUnderbar);
    }

    #[test]
    fn test_default_values() {
        let cfg = Config::default();
        assert_eq!(cfg.font_size, 14.0);
        assert_eq!(cfg.font_family, "monospace");
        assert!(cfg.font_ligatures);
        assert_eq!(cfg.window_width, 1280);
        assert_eq!(cfg.window_height, 800);
        assert_eq!(cfg.scrollback_lines, 100_000);
        assert_eq!(cfg.shell_program, "");
        assert!(cfg.shell_args.is_empty());
        assert_eq!(cfg.cursor_style, CursorStyle::Block);
        assert!(cfg.cursor_blink);
        assert_eq!(cfg.cursor_blink_ms, 500);
    }

    #[test]
    fn test_config_toml_round_trip() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.font_size, config.font_size);
        assert_eq!(parsed.font_family, config.font_family);
        assert_eq!(parsed.scrollback_lines, config.scrollback_lines);
        assert_eq!(parsed.cursor_style, config.cursor_style);
        assert!(parsed.cursor_blink);
    }

    #[test]
    fn test_config_partial_override() {
        let toml_str = r#"
font_size = 18.0
cursor_style = "beam"
scrollback_lines = 50000
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.font_size, 18.0);
        assert_eq!(config.cursor_style, CursorStyle::Beam);
        assert_eq!(config.scrollback_lines, 50000);
        // Other fields use defaults
        assert_eq!(config.font_family, "monospace");
        assert_eq!(config.window_width, 1280);
        assert!(config.cursor_blink);
    }

    #[test]
    fn test_config_save_and_load_temp() {
        let dir = std::env::temp_dir().join(format!("synapse_config_test_{}", std::process::id()));
        let path = dir.join("config.toml");

        let config = Config {
            font_size: 22.0,
            ..Config::default()
        };
        config.save_to(&path).unwrap();
        assert!(path.exists());

        let content = std::fs::read_to_string(&path).unwrap();
        let loaded: Config = toml::from_str(&content).unwrap();
        assert_eq!(loaded.font_size, 22.0);
        assert_eq!(loaded.cursor_style, CursorStyle::Block);

        // Clean up
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_config_path_exists() {
        let path = Config::config_path();
        if let Some(p) = path {
            // Path should point to the SYNAPSE_ config directory
            let dir = p.parent().unwrap();
            assert!(dir.ends_with("SYNAPSE_"));
        }
    }

    #[test]
    fn test_shell_config_custom() {
        let toml_str = r#"
shell_program = "/usr/bin/fish"
shell_args = ["-l"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.shell_program, "/usr/bin/fish");
        assert_eq!(config.shell_args, vec!["-l"]);
    }

    #[test]
    fn test_config_ui_flags_default() {
        let cfg = Config::default();
        assert!(cfg.show_pane_labels);
        assert!(cfg.show_resize_indicator);
        assert!(cfg.sidebar_show_process_dot);
    }

    #[test]
    fn test_config_status_bar_defaults() {
        let cfg = Config::default();
        assert!(cfg.status_bar);
        assert!(cfg.status_bar_show_git);
        assert!(cfg.status_bar_show_k8s);
        assert!(cfg.status_bar_show_time);
    }

    #[test]
    fn test_window_config_coverage() {
        let config = Config::default();
        assert_eq!(config.window_width, 1280);
        assert_eq!(config.window_height, 800);
        assert_eq!(config.sidebar_width, 180.0);

        let toml_str = r#"
window_width = 1920
window_height = 1080
sidebar_width = 200
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.window_width, 1920);
        assert_eq!(config.window_height, 1080);
        assert_eq!(config.sidebar_width, 200.0);
    }
}
