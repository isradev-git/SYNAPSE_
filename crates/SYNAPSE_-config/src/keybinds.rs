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
    EffectsToggle,
    JumpPrevMark,
    JumpNextMark,
    ToggleCopyMode,
    AutoSplit,
    ResizePaneLeft,
    ResizePaneRight,
    ResizePaneUp,
    ResizePaneDown,
    ToggleStatusBar,
    PaletteOpen,
}

impl Action {
    #[allow(clippy::should_implement_trait)]
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
            "effects_toggle" => Some(Action::EffectsToggle),
            "jump_prev_mark" => Some(Action::JumpPrevMark),
            "jump_next_mark" => Some(Action::JumpNextMark),
            "toggle_copy_mode" => Some(Action::ToggleCopyMode),
            "auto_split" => Some(Action::AutoSplit),
            "resize_pane_left" => Some(Action::ResizePaneLeft),
            "resize_pane_right" => Some(Action::ResizePaneRight),
            "resize_pane_up" => Some(Action::ResizePaneUp),
            "resize_pane_down" => Some(Action::ResizePaneDown),
            "toggle_status_bar" => Some(Action::ToggleStatusBar),
            "palette_open" => Some(Action::PaletteOpen),
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
            if entry.ctrl == ctrl
                && entry.shift == shift
                && entry.alt == alt
                && key_matches(key, &entry.key)
            {
                return Action::from_str(&entry.action);
            }
        }
        None
    }

    pub fn entries(&self) -> &[KeyBindEntry] {
        &self.entries
    }

    pub fn bindings(&self) -> impl Iterator<Item = (KeyCombo, Action)> + '_ {
        self.entries.iter().filter_map(|e| {
            let combo = entry_to_combo(e)?;
            let action = Action::from_str(&e.action)?;
            Some((combo, action))
        })
    }
}

impl Default for Keybinds {
    fn default() -> Self {
        Self::new()
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
            key: "h".into(),
            ctrl: true,
            shift: true,
            alt: false,
            action: "split_horizontal".into(),
        },
        KeyBindEntry {
            key: "e".into(),
            ctrl: true,
            shift: true,
            alt: false,
            action: "effects_toggle".into(),
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
        KeyBindEntry {
            key: "Up".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "jump_prev_mark".into(),
        },
        KeyBindEntry {
            key: "Down".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "jump_next_mark".into(),
        },
        KeyBindEntry {
            key: "Space".into(),
            ctrl: true,
            shift: true,
            alt: false,
            action: "toggle_copy_mode".into(),
        },
        KeyBindEntry {
            key: "Enter".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "auto_split".into(),
        },
        KeyBindEntry {
            key: "Left".into(),
            ctrl: true,
            shift: true,
            alt: true,
            action: "resize_pane_left".into(),
        },
        KeyBindEntry {
            key: "Right".into(),
            ctrl: true,
            shift: true,
            alt: true,
            action: "resize_pane_right".into(),
        },
        KeyBindEntry {
            key: "Up".into(),
            ctrl: true,
            shift: true,
            alt: true,
            action: "resize_pane_up".into(),
        },
        KeyBindEntry {
            key: "Down".into(),
            ctrl: true,
            shift: true,
            alt: true,
            action: "resize_pane_down".into(),
        },
        KeyBindEntry {
            key: "s".into(),
            ctrl: true,
            shift: true,
            alt: false,
            action: "toggle_status_bar".into(),
        },
        KeyBindEntry {
            key: "p".into(),
            ctrl: true,
            shift: true,
            alt: false,
            action: "palette_open".into(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::{ModifiersState, NamedKey};

    fn mods(ctrl: bool, shift: bool, alt: bool) -> ModifiersState {
        let mut m = ModifiersState::empty();
        if ctrl {
            m |= ModifiersState::CONTROL;
        }
        if shift {
            m |= ModifiersState::SHIFT;
        }
        if alt {
            m |= ModifiersState::ALT;
        }
        m
    }

    #[test]
    fn test_action_from_str_all_actions() {
        let actions = [
            "search",
            "history_search",
            "clear_screen",
            "new_tab",
            "close_tab",
            "next_tab",
            "prev_tab",
            "tab_1",
            "tab_2",
            "tab_3",
            "tab_4",
            "tab_5",
            "tab_6",
            "tab_7",
            "tab_8",
            "tab_9",
            "split_vertical",
            "split_horizontal",
            "close_pane",
            "navigate_up",
            "navigate_down",
            "navigate_left",
            "navigate_right",
            "font_increase",
            "font_decrease",
            "font_reset",
            "fullscreen",
            "copy",
            "paste",
            "reload_config",
            "effects_toggle",
            "jump_prev_mark",
            "jump_next_mark",
            "toggle_copy_mode",
            "auto_split",
            "resize_pane_left",
            "resize_pane_right",
            "resize_pane_up",
            "resize_pane_down",
            "toggle_status_bar",
            "palette_open",
        ];
        for &action_str in &actions {
            assert!(
                Action::from_str(action_str).is_some(),
                "Action::from_str({}) failed",
                action_str
            );
        }
    }

    #[test]
    fn test_action_from_str_invalid() {
        assert!(Action::from_str("nonexistent").is_none());
        assert!(Action::from_str("").is_none());
        assert!(Action::from_str("TAB_1").is_none());
    }

    #[test]
    fn test_default_entries_count() {
        let kb = Keybinds::new();
        assert!(
            kb.entries().len() >= 28,
            "Expected at least 28 default bindings"
        );
    }

    #[test]
    fn test_lookup_new_tab() {
        let kb = Keybinds::new();
        let action = kb.lookup(&Key::Character("t".into()), mods(true, false, false));
        assert_eq!(action, Some(Action::NewTab));
    }

    #[test]
    fn test_lookup_close_tab() {
        let kb = Keybinds::new();
        let action = kb.lookup(&Key::Character("w".into()), mods(true, false, false));
        assert_eq!(action, Some(Action::CloseTab));
    }

    #[test]
    fn test_lookup_shift_variant() {
        let kb = Keybinds::new();
        let action = kb.lookup(&Key::Character("w".into()), mods(true, true, false));
        assert_eq!(action, Some(Action::ClosePane));
    }

    #[test]
    fn test_lookup_no_match() {
        let kb = Keybinds::new();
        let action = kb.lookup(&Key::Character("z".into()), mods(false, false, false));
        assert!(action.is_none());
    }

    #[test]
    fn test_lookup_named_key_tab() {
        let kb = Keybinds::new();
        let action = kb.lookup(&Key::Named(NamedKey::Tab), mods(true, false, false));
        assert_eq!(action, Some(Action::NextTab));
    }

    #[test]
    fn test_lookup_named_key_f11() {
        let kb = Keybinds::new();
        let action = kb.lookup(&Key::Named(NamedKey::F11), mods(false, false, false));
        assert_eq!(action, Some(Action::Fullscreen));
    }

    #[test]
    fn test_lookup_case_insensitive() {
        let kb = Keybinds::new();
        let action = kb.lookup(&Key::Character("T".into()), mods(true, false, false));
        assert_eq!(action, Some(Action::NewTab));
    }

    #[test]
    fn test_entry_to_combo_round_trip() {
        let entry = KeyBindEntry {
            key: "f".into(),
            ctrl: true,
            shift: true,
            alt: false,
            action: "search".into(),
        };
        let combo = entry_to_combo(&entry).unwrap();
        assert_eq!(combo.key, "f");
        assert!(combo.ctrl);
        assert!(combo.shift);
        assert!(!combo.alt);
    }

    #[test]
    fn test_from_config_overrides() {
        let overrides = vec![KeyBindEntry {
            key: "t".into(),
            ctrl: true,
            shift: true,
            alt: false,
            action: "clear_screen".into(),
        }];
        let kb = Keybinds::from_config(&overrides);
        let action = kb.lookup(&Key::Character("t".into()), mods(true, true, false));
        assert_eq!(action, Some(Action::ClearScreen));
        let action = kb.lookup(&Key::Character("t".into()), mods(true, false, false));
        assert_eq!(action, Some(Action::NewTab));
    }

    #[test]
    fn test_lookup_arrow_navigation() {
        let kb = Keybinds::new();
        assert_eq!(
            kb.lookup(&Key::Named(NamedKey::ArrowUp), mods(true, true, false)),
            Some(Action::NavigateUp)
        );
        assert_eq!(
            kb.lookup(&Key::Named(NamedKey::ArrowDown), mods(true, true, false)),
            Some(Action::NavigateDown)
        );
        assert_eq!(
            kb.lookup(&Key::Named(NamedKey::ArrowLeft), mods(true, true, false)),
            Some(Action::NavigateLeft)
        );
        assert_eq!(
            kb.lookup(&Key::Named(NamedKey::ArrowRight), mods(true, true, false)),
            Some(Action::NavigateRight)
        );
    }

    #[test]
    fn test_toggle_copy_mode_action_from_str() {
        assert_eq!(
            Action::from_str("toggle_copy_mode"),
            Some(Action::ToggleCopyMode)
        );
    }

    #[test]
    fn test_toggle_copy_mode_default_binding() {
        let kb = Keybinds::new();
        let action = kb.lookup(&Key::Named(NamedKey::Space), mods(true, true, false));
        assert_eq!(action, Some(Action::ToggleCopyMode));
    }

    #[test]
    fn test_effects_toggle_action() {
        assert_eq!(
            Action::from_str("effects_toggle"),
            Some(Action::EffectsToggle)
        );
        assert_eq!(Action::from_str("unknown_xyz"), None);
    }

    #[test]
    fn test_effects_toggle_default_binding() {
        let kb = Keybinds::default();
        let has_effects = kb.bindings().any(|(_, a)| a == Action::EffectsToggle);
        assert!(has_effects, "EffectsToggle must have a default binding");
    }

    #[test]
    fn test_auto_split_action_from_str() {
        assert_eq!(Action::from_str("auto_split"), Some(Action::AutoSplit));
    }

    #[test]
    fn test_auto_split_default_binding() {
        let kb = Keybinds::new();
        let action = kb.lookup(&Key::Named(NamedKey::Enter), mods(true, false, false));
        assert_eq!(action, Some(Action::AutoSplit));
    }

    #[test]
    fn test_resize_pane_actions_from_str() {
        assert_eq!(
            Action::from_str("resize_pane_left"),
            Some(Action::ResizePaneLeft)
        );
        assert_eq!(
            Action::from_str("resize_pane_right"),
            Some(Action::ResizePaneRight)
        );
        assert_eq!(
            Action::from_str("resize_pane_up"),
            Some(Action::ResizePaneUp)
        );
        assert_eq!(
            Action::from_str("resize_pane_down"),
            Some(Action::ResizePaneDown)
        );
    }

    #[test]
    fn test_resize_pane_default_bindings() {
        let kb = Keybinds::new();
        assert_eq!(
            kb.lookup(&Key::Named(NamedKey::ArrowLeft), mods(true, true, true)),
            Some(Action::ResizePaneLeft)
        );
        assert_eq!(
            kb.lookup(&Key::Named(NamedKey::ArrowRight), mods(true, true, true)),
            Some(Action::ResizePaneRight)
        );
        assert_eq!(
            kb.lookup(&Key::Named(NamedKey::ArrowUp), mods(true, true, true)),
            Some(Action::ResizePaneUp)
        );
        assert_eq!(
            kb.lookup(&Key::Named(NamedKey::ArrowDown), mods(true, true, true)),
            Some(Action::ResizePaneDown)
        );
    }

    #[test]
    fn test_jump_prev_mark_action_from_str() {
        assert_eq!(
            Action::from_str("jump_prev_mark"),
            Some(Action::JumpPrevMark)
        );
    }

    #[test]
    fn test_jump_next_mark_action_from_str() {
        assert_eq!(
            Action::from_str("jump_next_mark"),
            Some(Action::JumpNextMark)
        );
    }

    #[test]
    fn test_toggle_status_bar_action_from_str() {
        assert_eq!(
            Action::from_str("toggle_status_bar"),
            Some(Action::ToggleStatusBar)
        );
    }

    #[test]
    fn test_toggle_status_bar_default_binding() {
        let kb = Keybinds::new();
        let action = kb.lookup(&Key::Character("s".into()), mods(true, true, false));
        assert_eq!(action, Some(Action::ToggleStatusBar));
    }
}
