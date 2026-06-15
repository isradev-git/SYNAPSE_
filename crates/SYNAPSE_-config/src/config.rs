use crate::effects::EffectsConfig;
use serde::de;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn deserialize_font_families<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct FontFamilyVisitor;
    impl<'de> de::Visitor<'de> for FontFamilyVisitor {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or list of strings")
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<Vec<String>, E> {
            Ok(vec![value.to_string()])
        }

        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<String>, A::Error> {
            let mut vec = Vec::new();
            while let Some(elem) = seq.next_element::<String>()? {
                vec.push(elem);
            }
            Ok(vec)
        }
    }
    deserializer.deserialize_any(FontFamilyVisitor)
}

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

fn default_quake_height() -> f32 {
    0.4
}
fn default_quake_anim_ms() -> u64 {
    200
}
fn default_quake_hotkey() -> String {
    "Ctrl+Space".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuakeConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_quake_height")]
    pub height_percent: f32,
    #[serde(default = "default_quake_anim_ms")]
    pub animation_ms: u64,
    #[serde(default = "default_true")]
    pub hide_on_focus_lost: bool,
    #[serde(default = "default_quake_hotkey")]
    pub hotkey: String,
}

impl Default for QuakeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            height_percent: default_quake_height(),
            animation_ms: default_quake_anim_ms(),
            hide_on_focus_lost: true,
            hotkey: default_quake_hotkey(),
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_history_max_entries() -> usize {
    10000
}
fn default_plugin_split() -> String {
    "tab".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCommand {
    pub name: String,
    #[serde(default)]
    pub keybind: Option<String>,
    pub command: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default = "default_plugin_split")]
    pub split: String,
    #[serde(default)]
    pub replace_selection: bool,
}

/// Tab profile — spawns a new tab with preset shell, cwd and environment variables.
///
/// Example TOML:
/// ```toml
/// [[tab_profile]]
/// name = "work"
/// cwd = "~/work/myproject"
/// shell = "/bin/zsh"        # optional, falls back to $SHELL
/// shell_args = ["--login"]  # optional
///
/// [tab_profile.env]
/// API_URL = "https://api.example.com"
/// NODE_ENV = "development"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TabProfile {
    pub name: String,
    #[serde(default)]
    pub shell: Option<String>,
    #[serde(default)]
    pub shell_args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

/// SSH connection profile for command-palette quick-connect.
///
/// Example TOML:
/// ```toml
/// [[ssh_profile]]
/// name = "prod"
/// host = "user@example.com"
/// port = 22
/// identity_file = "~/.ssh/id_rsa"
/// forward_agent = false
/// extra_args = ["-o", "StrictHostKeyChecking=no"]
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SshProfile {
    pub name: String,
    pub host: String,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub identity_file: Option<String>,
    #[serde(default)]
    pub forward_agent: bool,
    #[serde(default)]
    pub extra_args: Vec<String>,
}

impl SshProfile {
    /// Build the argv for spawning this SSH session.
    pub fn to_argv(&self) -> Vec<String> {
        let mut args = vec!["ssh".to_string()];
        if let Some(port) = self.port {
            args.push("-p".to_string());
            args.push(port.to_string());
        }
        if let Some(ref key) = self.identity_file {
            args.push("-i".to_string());
            // Expand leading `~`
            let expanded = if let Some(rest) = key.strip_prefix('~') {
                let home = std::env::var("HOME").unwrap_or_default();
                format!("{}{}", home, rest)
            } else {
                key.clone()
            };
            args.push(expanded);
        }
        if self.forward_agent {
            args.push("-A".to_string());
        }
        args.extend(self.extra_args.clone());
        args.push(self.host.clone());
        args
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CursorStyle {
    Block,
    Beam,
    Underline,
    NeonUnderbar,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundMode {
    Cover,
    Contain,
    Stretch,
    Tile,
}

impl BackgroundMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackgroundMode::Cover => "cover",
            BackgroundMode::Contain => "contain",
            BackgroundMode::Stretch => "stretch",
            BackgroundMode::Tile => "tile",
        }
    }
}

fn default_background_opacity() -> f32 {
    1.0
}
fn default_background_mode() -> BackgroundMode {
    BackgroundMode::Cover
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(
        default = "default_font_families",
        deserialize_with = "deserialize_font_families"
    )]
    pub font_family: Vec<String>,
    #[serde(default = "default_true")]
    pub font_ligatures: bool,
    #[serde(default)]
    pub font_features: Vec<String>,
    #[serde(default = "default_font_weight")]
    pub font_weight: u16,
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
    #[serde(default = "default_true")]
    pub clickable_paths: bool,
    #[serde(default = "default_true")]
    pub sixel_enabled: bool,
    #[serde(default = "default_true")]
    pub iterm2_images: bool,
    #[serde(default)]
    pub restore_session: bool,
    #[serde(default)]
    pub session_save_interval_secs: u64,
    #[serde(default)]
    pub quake: QuakeConfig,
    #[serde(default = "default_true")]
    pub freeze_background_tabs: bool,
    #[serde(default = "default_true")]
    pub persistent_history: bool,
    #[serde(default = "default_history_max_entries")]
    pub history_max_entries: usize,
    #[serde(default)]
    pub plugins: Vec<PluginCommand>,
    #[serde(default)]
    pub recording_path: Option<String>,
    #[serde(default)]
    pub reduce_motion: bool,
    #[serde(default)]
    pub background_image: Option<String>,
    #[serde(default = "default_background_opacity")]
    pub background_opacity: f32,
    #[serde(default = "default_background_mode")]
    pub background_mode: BackgroundMode,
    #[serde(default = "default_true")]
    pub scrollbar: bool,
    #[serde(default)]
    pub pane_badge: bool,
    #[serde(default = "default_pane_badge_format")]
    pub pane_badge_format: String,
    #[serde(default = "default_window_opacity")]
    pub window_opacity: f32,
    #[serde(default)]
    pub window_blur: bool,
    #[serde(default)]
    pub shell_integration: bool,
    #[serde(default)]
    pub ssh_profiles: Vec<SshProfile>,
    #[serde(default)]
    pub tab_profiles: Vec<TabProfile>,
    #[serde(default)]
    pub check_updates_on_startup: bool,
    /// Reduce memory usage for constrained systems (Pi, VMs, old hardware).
    /// Caps effective scrollback to 5 000 lines, uses a 1024×1024 glyph atlas,
    /// and disables the cross-session glyph warm cache.
    #[serde(default)]
    pub low_memory_mode: bool,
    /// Maximum RAM budget for cached protocol images (Kitty / Sixel / iTerm2).
    /// Oldest images are evicted when the budget is exceeded. Default: 64 MB.
    #[serde(default = "default_max_image_cache_mb")]
    pub max_image_cache_mb: usize,
}

fn default_window_opacity() -> f32 {
    1.0
}

fn default_pane_badge_format() -> String {
    "{cwd}".to_string()
}

fn default_font_size() -> f32 {
    14.0
}
fn default_font_weight() -> u16 {
    400
}
fn default_font_families() -> Vec<String> {
    vec!["JetBrainsMono NF".to_string(), "JetBrains Mono".to_string()]
}
fn default_window_width() -> u32 {
    1280
}
fn default_window_height() -> u32 {
    800
}
fn default_scrollback_lines() -> usize {
    10_000
}
fn default_max_image_cache_mb() -> usize {
    64
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
            font_family: default_font_families(),
            font_ligatures: true,
            font_features: Vec::new(),
            font_weight: default_font_weight(),
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
            clickable_paths: true,
            sixel_enabled: true,
            iterm2_images: true,
            restore_session: false,
            session_save_interval_secs: 0,
            quake: QuakeConfig::default(),
            freeze_background_tabs: true,
            persistent_history: true,
            history_max_entries: default_history_max_entries(),
            plugins: Vec::new(),
            recording_path: None,
            reduce_motion: false,
            background_image: None,
            background_opacity: default_background_opacity(),
            background_mode: default_background_mode(),
            scrollbar: true,
            pane_badge: false,
            pane_badge_format: default_pane_badge_format(),
            window_opacity: 1.0,
            window_blur: false,
            shell_integration: false,
            ssh_profiles: Vec::new(),
            tab_profiles: Vec::new(),
            check_updates_on_startup: false,
            low_memory_mode: false,
            max_image_cache_mb: default_max_image_cache_mb(),
        }
    }
}

impl Config {
    /// Returns platform-specific defaults. Called on first launch (no config file yet).
    ///
    /// Detection order:
    /// - Linux ARM: check `/sys/firmware/devicetree/base/model` for "Raspberry Pi"
    /// - macOS: compile-time known
    /// - Everything else: generic defaults
    pub fn platform_defaults() -> Self {
        let mut cfg = Self::default();

        #[cfg(target_os = "macos")]
        {
            cfg.window_blur = true;
            cfg.max_image_cache_mb = 128;
        }

        if is_raspberry_pi() {
            // Tuned for VideoCore VI/VII (GLES 3.1) + SD card I/O + limited RAM.
            cfg.scrollback_lines = 3_000;
            cfg.low_memory_mode = true;
            cfg.max_image_cache_mb = 32;
            // HarfBuzz shaping is CPU-heavy on Cortex-A72/A76 — skip for plain ASCII.
            cfg.font_ligatures = false;
            // Timer-driven blink causes a redraw every 500 ms even at idle.
            cfg.cursor_blink = false;
            // Sixel and iTerm2 image decode is CPU-bound; GPU upload is also slow on Pi.
            cfg.sixel_enabled = false;
            cfg.iterm2_images = false;
            // git process spawns on SD card stall the render thread for 50–200 ms.
            cfg.status_bar_show_git = false;
            cfg.status_bar_show_k8s = false;
            cfg.pane_badge = false;
            cfg.window_opacity = 1.0;
            cfg.history_max_entries = 2_000;
        }

        cfg
    }

    /// Effective scrollback line count, capped at 5 000 when `low_memory_mode` is set.
    pub fn effective_scrollback(&self) -> usize {
        if self.low_memory_mode {
            self.scrollback_lines.min(5_000)
        } else {
            self.scrollback_lines
        }
    }

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
            let config = Config::platform_defaults();
            let _ = config.save_to(&path);
            config
        } else {
            Config::platform_defaults()
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

/// Returns true when running on a Raspberry Pi (any model that exposes the DTB model file).
/// Always false on non-Linux targets.
fn is_raspberry_pi() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/sys/firmware/devicetree/base/model")
            .map(|s| s.contains("Raspberry Pi"))
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quake_config_default() {
        let cfg = QuakeConfig::default();
        assert!(!cfg.enabled);
        assert!(cfg.hide_on_focus_lost);
    }

    #[test]
    fn test_freeze_background_tabs_default_true() {
        let cfg = Config::default();
        assert!(cfg.freeze_background_tabs);
    }

    #[test]
    fn test_freeze_background_tabs_toml() {
        let toml_str = r#"freeze_background_tabs = false"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(!config.freeze_background_tabs);
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
        assert_eq!(
            cfg.font_family,
            vec!["JetBrainsMono NF".to_string(), "JetBrains Mono".to_string()]
        );
        assert!(cfg.font_ligatures);
        assert_eq!(cfg.window_width, 1280);
        assert_eq!(cfg.window_height, 800);
        assert_eq!(cfg.scrollback_lines, 10_000);
        assert_eq!(cfg.shell_program, "");
        assert!(cfg.shell_args.is_empty());
        assert_eq!(cfg.cursor_style, CursorStyle::Block);
        assert!(cfg.cursor_blink);
        assert_eq!(cfg.cursor_blink_ms, 500);
        assert!(!cfg.restore_session);
        assert_eq!(cfg.session_save_interval_secs, 0);
        assert!(cfg.freeze_background_tabs);
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
        assert_eq!(
            config.font_family,
            vec!["JetBrainsMono NF".to_string(), "JetBrains Mono".to_string()]
        );
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
        assert!(cfg.clickable_paths);
        assert!(cfg.sixel_enabled);
        assert!(cfg.iterm2_images);
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

    #[test]
    fn test_font_family_single_string() {
        let toml_str = r#"font_family = "JetBrains Mono""#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.font_family, vec!["JetBrains Mono".to_string()]);
    }

    #[test]
    fn test_font_family_array_of_strings() {
        let toml_str = r#"font_family = ["Fira Code", "Noto Color Emoji"]"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.font_family,
            vec!["Fira Code".to_string(), "Noto Color Emoji".to_string()]
        );
    }

    #[test]
    fn test_font_family_default_is_jetbrains_mono_nf() {
        let config = Config::default();
        assert_eq!(
            config.font_family,
            vec!["JetBrainsMono NF".to_string(), "JetBrains Mono".to_string()]
        );
    }

    #[test]
    fn test_font_ligatures_default_true_in_toml() {
        // serde(default) on bool gives false; we use default_true to keep ligatures on
        let cfg: Config = toml::from_str("").unwrap();
        assert!(
            cfg.font_ligatures,
            "ligatures should default to true when key absent from TOML"
        );
    }

    #[test]
    fn test_reduce_motion_default_false() {
        let cfg = Config::default();
        assert!(!cfg.reduce_motion);
    }

    #[test]
    fn test_reduce_motion_toml() {
        let toml_str = "reduce_motion = true\n";
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert!(cfg.reduce_motion);
    }

    #[test]
    fn test_background_image_defaults() {
        let cfg = Config::default();
        assert!(cfg.background_image.is_none());
        assert_eq!(cfg.background_opacity, 1.0);
        assert_eq!(cfg.background_mode, BackgroundMode::Cover);
    }

    #[test]
    fn test_background_image_toml() {
        let toml_str = r#"
background_image = "/home/user/wallpaper.png"
background_opacity = 0.5
background_mode = "contain"
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(
            cfg.background_image.as_deref(),
            Some("/home/user/wallpaper.png")
        );
        assert_eq!(cfg.background_opacity, 0.5);
        assert_eq!(cfg.background_mode, BackgroundMode::Contain);
    }

    #[test]
    fn test_background_mode_serde_all_variants() {
        for (s, expected) in &[
            ("cover", BackgroundMode::Cover),
            ("contain", BackgroundMode::Contain),
            ("stretch", BackgroundMode::Stretch),
            ("tile", BackgroundMode::Tile),
        ] {
            let toml_str = format!("background_mode = \"{}\"", s);
            let cfg: Config = toml::from_str(&toml_str).unwrap();
            assert_eq!(cfg.background_mode, *expected, "mode: {}", s);
        }
    }

    #[test]
    fn test_platform_defaults_not_pi_on_host() {
        // On any non-Pi host, platform_defaults should not apply Pi overrides.
        // We can't mock is_raspberry_pi() here, but we verify the function runs
        // and returns a valid Config.
        let cfg = Config::platform_defaults();
        // Basic sanity: these fields must always have valid values
        assert!(cfg.scrollback_lines > 0);
        assert!(cfg.max_image_cache_mb > 0);
        assert!(cfg.history_max_entries > 0);
    }

    #[test]
    fn test_effective_scrollback_low_memory() {
        let cfg = Config {
            scrollback_lines: 20_000,
            low_memory_mode: false,
            ..Default::default()
        };
        assert_eq!(cfg.effective_scrollback(), 20_000);

        let cfg_lm = Config {
            scrollback_lines: 20_000,
            low_memory_mode: true,
            ..Default::default()
        };
        assert_eq!(cfg_lm.effective_scrollback(), 5_000);

        let cfg_small = Config {
            scrollback_lines: 3_000,
            low_memory_mode: true,
            ..Default::default()
        };
        assert_eq!(cfg_small.effective_scrollback(), 3_000); // already under cap
    }

    #[test]
    fn test_is_raspberry_pi_non_pi_host() {
        // On a dev machine (Mac or x86 Linux) this must return false.
        // The DTB model file doesn't exist or doesn't contain "Raspberry Pi".
        #[cfg(not(target_os = "linux"))]
        assert!(!is_raspberry_pi());
        // On Linux x86 the file won't contain "Raspberry Pi"
        #[cfg(target_os = "linux")]
        {
            let result = is_raspberry_pi();
            // CI always runs on x86 — must be false there.
            // We can't assert false unconditionally since a Pi builder would break the test.
            let _ = result; // just ensure it compiles and doesn't panic
        }
    }
}
