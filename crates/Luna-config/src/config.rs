use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_window_width")]
    pub window_width: u32,
    #[serde(default = "default_window_height")]
    pub window_height: u32,
    #[serde(default = "default_scrollback_lines")]
    pub scrollback_lines: usize,
}

fn default_font_size() -> f32 {
    14.0
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

impl Default for Config {
    fn default() -> Self {
        Self {
            font_size: default_font_size(),
            window_width: default_window_width(),
            window_height: default_window_height(),
            scrollback_lines: default_scrollback_lines(),
        }
    }
}

impl Config {
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
            .or_else(|_| {
                std::env::var("HOME").map(|h| PathBuf::from(h).join(".config"))
            })
            .ok()
            .map(|d| d.join("Luna"))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var("HOME")
            .map(|h| {
                PathBuf::from(h)
                    .join("Library")
                    .join("Application Support")
                    .join("Luna")
            })
            .ok()
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA")
            .map(|d| PathBuf::from(d).join("Luna"))
            .ok()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        None
    }
}
