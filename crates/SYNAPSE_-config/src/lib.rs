pub mod config;
pub mod effects;
pub mod keybinds;
pub mod themes;

pub use config::{Config, CursorStyle};
pub use effects::EffectsConfig;
pub use keybinds::{Action, KeyBindEntry, Keybinds};
pub use themes::Theme;
