pub mod config;
pub mod effects;
pub mod keybinds;
pub mod themes;

pub use config::{BackgroundMode, BellConfig, Config, CursorStyle, PluginCommand, SshProfile};
pub use effects::EffectsConfig;
pub use keybinds::{
    combo_to_string, parse_keybind_string, Action, KeyBindEntry, KeyCombo, Keybinds,
};
pub use themes::Theme;
