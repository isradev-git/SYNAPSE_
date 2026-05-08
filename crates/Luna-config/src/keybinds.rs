use serde::{Deserialize, Serialize};
use winit::keyboard::{Key, ModifiersState, NamedKey};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyCombo {
    pub key: String,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    Search,
    HistorySearch,
    ClearScreen,
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    TabSwitch1,
    TabSwitch2,
    TabSwitch3,
    TabSwitch4,
    TabSwitch5,
    TabSwitch6,
    TabSwitch7,
    TabSwitch8,
    TabSwitch9,
    SplitVertical,
    SplitHorizontal,
    ClosePane,
    NavigateUp,
    NavigateDown,
    NavigateLeft,
    NavigateRight,
    FontIncrease,
    FontDecrease,
    FontReset,
    Fullscreen,
    Copy,
    Paste,
    ReloadConfig,
}

impl Action {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "search" => Some(Action::Search),
            "history_search" => Some(Action::HistorySearch),
            "clear_screen" => Some(Action::ClearScreen),
            "new_tab" => Some(Action::NewTab),
            "close_tab" => Some(Action::CloseTab),
            "next_tab" => Some(Action::NextTab),
            "prev_tab" => Some(Action::PrevTab),
            "tab_1" => Some(Action::TabSwitch1),
            "tab_2" => Some(Action::TabSwitch2),
            "tab_3" => Some(Action::TabSwitch3),
            "tab_4" => Some(Action::TabSwitch4),
            "tab_5" => Some(Action::TabSwitch5),
            "tab_6" => Some(Action::TabSwitch6),
            "tab_7" => Some(Action::TabSwitch7),
            "tab_8" => Some(Action::TabSwitch8),
            "tab_9" => Some(Action::TabSwitch9),
            "split_vertical" => Some(Action::SplitVertical),
            "split_horizontal" => Some(Action::SplitHorizontal),
            "close_pane" => Some(Action::ClosePane),
            "navigate_up" => Some(Action::NavigateUp),
            "navigate_down" => Some(Action::NavigateDown),
            "navigate_left" => Some(Action::NavigateLeft),
            "navigate_right" => Some(Action::NavigateRight),
            "font_increase" => Some(Action::FontIncrease),
            "font_decrease" => Some(Action::FontDecrease),
            "font_reset" => Some(Action::FontReset),
            "fullscreen" => Some(Action::Fullscreen),
            "copy" => Some(Action::Copy),
            "paste" => Some(Action::Paste),
            "reload_config" => Some(Action::ReloadConfig),
            _ => None,
        }
    }
}

fn key_matches(key: &Key, combo_key: &str) -> bool {
    match key {
        Key::Character(c) => c.as_str().eq_ignore_ascii_case(combo_key),
        Key::Named(named) => match_key_name(named, combo_key),
        _ => false,
    }
}

fn match_key_name(named: &NamedKey, key_str: &str) -> bool {
    let s = key_str.to_lowercase();
    match named {
        NamedKey::Enter => s == "enter" || s == "return",
        NamedKey::Tab => s == "tab",
        NamedKey::Space => s == "space",
        NamedKey::ArrowDown => s == "down" || s == "arrowdown",
        NamedKey::ArrowLeft => s == "left" || s == "arrowleft",
        NamedKey::ArrowRight => s == "right" || s == "arrowright",
        NamedKey::ArrowUp => s == "up" || s == "arrowup",
        NamedKey::End => s == "end",
        NamedKey::Home => s == "home",
        NamedKey::PageDown => s == "pagedown",
        NamedKey::PageUp => s == "pageup",
        NamedKey::Backspace => s == "backspace",
        NamedKey::Delete => s == "delete",
        NamedKey::Insert => s == "insert",
        NamedKey::Escape => s == "escape",
        NamedKey::F1 => s == "f1",
        NamedKey::F2 => s == "f2",
        NamedKey::F3 => s == "f3",
        NamedKey::F4 => s == "f4",
        NamedKey::F5 => s == "f5",
        NamedKey::F6 => s == "f6",
        NamedKey::F7 => s == "f7",
        NamedKey::F8 => s == "f8",
        NamedKey::F9 => s == "f9",
        NamedKey::F10 => s == "f10",
        NamedKey::F11 => s == "f11",
        NamedKey::F12 => s == "f12",
        _ => false,
    }
}

pub struct Keybinds {
    entries: Vec<KeyBindEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBindEntry {
    pub key: String,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
    pub action: String,
}

impl Keybinds {
    pub fn new() -> Self {
        let entries = default_entries();
        Self { entries }
    }

    pub fn from_config(overrides: &[KeyBindEntry]) -> Self {
        let mut entries = default_entries();
        for ov in overrides {
            if let Some(combo) = entry_to_combo(ov) {
                entries.retain(|e| entry_to_combo(e) != Some(combo.clone()));
                entries.push(ov.clone());
            }
        }
        Self { entries }
    }

    pub fn lookup(&self, key: &Key, modifiers: ModifiersState) -> Option<Action> {
        let ctrl = modifiers.control_key();
        let shift = modifiers.shift_key();
        let alt = modifiers.alt_key();

        for entry in &self.entries {
            if entry.ctrl == ctrl && entry.shift == shift && entry.alt == alt {
                if key_matches(key, &entry.key) {
                    return Action::from_str(&entry.action);
                }
            }
        }
        None
    }

    pub fn entries(&self) -> &[KeyBindEntry] {
        &self.entries
    }
}

fn entry_to_combo(entry: &KeyBindEntry) -> Option<KeyCombo> {
    Some(KeyCombo {
        key: entry.key.clone(),
        ctrl: entry.ctrl,
        shift: entry.shift,
        alt: entry.alt,
    })
}

fn default_entries() -> Vec<KeyBindEntry> {
    vec![
        KeyBindEntry {
            key: "f".into(),
            ctrl: true,
            shift: true,
            alt: false,
            action: "search".into(),
        },
        KeyBindEntry {
            key: "r".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "history_search".into(),
        },
        KeyBindEntry {
            key: "l".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "clear_screen".into(),
        },
        KeyBindEntry {
            key: "t".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "new_tab".into(),
        },
        KeyBindEntry {
            key: "w".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "close_tab".into(),
        },
        KeyBindEntry {
            key: "Tab".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "next_tab".into(),
        },
        KeyBindEntry {
            key: "Tab".into(),
            ctrl: true,
            shift: true,
            alt: false,
            action: "prev_tab".into(),
        },
        KeyBindEntry {
            key: "1".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "tab_1".into(),
        },
        KeyBindEntry {
            key: "2".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "tab_2".into(),
        },
        KeyBindEntry {
            key: "3".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "tab_3".into(),
        },
        KeyBindEntry {
            key: "4".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "tab_4".into(),
        },
        KeyBindEntry {
            key: "5".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "tab_5".into(),
        },
        KeyBindEntry {
            key: "6".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "tab_6".into(),
        },
        KeyBindEntry {
            key: "7".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "tab_7".into(),
        },
        KeyBindEntry {
            key: "8".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "tab_8".into(),
        },
        KeyBindEntry {
            key: "9".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "tab_9".into(),
        },
        KeyBindEntry {
            key: "d".into(),
            ctrl: true,
            shift: true,
            alt: false,
            action: "split_vertical".into(),
        },
        KeyBindEntry {
            key: "e".into(),
            ctrl: true,
            shift: true,
            alt: false,
            action: "split_horizontal".into(),
        },
        KeyBindEntry {
            key: "w".into(),
            ctrl: true,
            shift: true,
            alt: false,
            action: "close_pane".into(),
        },
        KeyBindEntry {
            key: "Up".into(),
            ctrl: true,
            shift: true,
            alt: false,
            action: "navigate_up".into(),
        },
        KeyBindEntry {
            key: "Down".into(),
            ctrl: true,
            shift: true,
            alt: false,
            action: "navigate_down".into(),
        },
        KeyBindEntry {
            key: "Left".into(),
            ctrl: true,
            shift: true,
            alt: false,
            action: "navigate_left".into(),
        },
        KeyBindEntry {
            key: "Right".into(),
            ctrl: true,
            shift: true,
            alt: false,
            action: "navigate_right".into(),
        },
        KeyBindEntry {
            key: "=".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "font_increase".into(),
        },
        KeyBindEntry {
            key: "-".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "font_decrease".into(),
        },
        KeyBindEntry {
            key: "0".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "font_reset".into(),
        },
        KeyBindEntry {
            key: "F11".into(),
            ctrl: false,
            shift: false,
            alt: false,
            action: "fullscreen".into(),
        },
        KeyBindEntry {
            key: "c".into(),
            ctrl: true,
            shift: true,
            alt: false,
            action: "copy".into(),
        },
        KeyBindEntry {
            key: "v".into(),
            ctrl: true,
            shift: true,
            alt: false,
            action: "paste".into(),
        },
        KeyBindEntry {
            key: ",".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "reload_config".into(),
        },
    ]
}
