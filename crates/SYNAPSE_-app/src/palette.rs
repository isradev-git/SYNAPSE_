use crate::state::AppState;
use synapse_config::keybinds::Action;
use synapse_ui::tab_bar::TabBar;
use winit::keyboard::Key;

pub struct PaletteState {
    pub active: bool,
    pub query: String,
    pub results: Vec<PaletteItem>,
    pub selected: usize,
    pub pending_action: Option<Action>,
    pub pending_theme_reload: bool,
    #[allow(dead_code)]
    plugins: Vec<synapse_config::PluginCommand>,
}

#[derive(Debug, Clone)]
pub enum PaletteItem {
    Action {
        label: String,
        keybind: Option<String>,
        action: Action,
    },
    Tab {
        label: String,
        index: usize,
    },
    Theme {
        name: String,
    },
}

impl PaletteState {
    pub fn new() -> Self {
        Self {
            active: false,
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            pending_action: None,
            pending_theme_reload: false,
            plugins: Vec::new(),
        }
    }

    pub fn set_plugins(&mut self, plugins: Vec<synapse_config::PluginCommand>) {
        self.plugins = plugins;
    }

    pub fn toggle(&mut self, tab_bar: &TabBar) {
        self.active = !self.active;
        if self.active {
            self.query.clear();
            self.selected = 0;
            self.results = build_palette_items(tab_bar, &self.plugins);
        }
    }

    pub fn take_pending_action(&mut self) -> Option<Action> {
        self.pending_action.take()
    }

    pub fn take_pending_theme_reload(&mut self) -> bool {
        let v = self.pending_theme_reload;
        self.pending_theme_reload = false;
        v
    }
}

fn fuzzy_score(query: &str, candidate: &str) -> Option<i32> {
    let q = query.to_lowercase();
    let c = candidate.to_lowercase();
    if q.is_empty() {
        return Some(0);
    }

    let q_chars: Vec<char> = q.chars().collect();
    let c_chars: Vec<char> = c.chars().collect();

    let mut qi = 0;
    let mut first_match_idx = None;
    for (ci, &cc) in c_chars.iter().enumerate() {
        if qi < q_chars.len() && cc == q_chars[qi] {
            if first_match_idx.is_none() {
                first_match_idx = Some(ci);
            }
            qi += 1;
        }
    }

    if qi == q_chars.len() {
        Some(-(first_match_idx.unwrap_or(0) as i32))
    } else {
        None
    }
}

macro_rules! action_item {
    ($label:expr, $kb:expr, $action:expr) => {
        PaletteItem::Action {
            label: $label.into(),
            keybind: $kb.map(|s: &str| s.to_string()),
            action: $action,
        }
    };
}

macro_rules! theme_item {
    ($name:expr) => {
        PaletteItem::Theme { name: $name.into() }
    };
}

fn build_palette_items(
    tab_bar: &TabBar,
    plugins: &[synapse_config::PluginCommand],
) -> Vec<PaletteItem> {
    let mut items = vec![
        action_item!("New Tab", Some("Ctrl+T"), Action::NewTab),
        action_item!("Close Tab", Some("Ctrl+W"), Action::CloseTab),
        action_item!("Next Tab", Some("Ctrl+Tab"), Action::NextTab),
        action_item!("Previous Tab", Some("Ctrl+Shift+Tab"), Action::PrevTab),
        action_item!(
            "Split Vertical",
            Some("Ctrl+Shift+D"),
            Action::SplitVertical
        ),
        action_item!(
            "Split Horizontal",
            Some("Ctrl+Shift+H"),
            Action::SplitHorizontal
        ),
        action_item!("Auto Split", Some("Ctrl+Enter"), Action::AutoSplit),
        action_item!("Close Pane", Some("Ctrl+Shift+W"), Action::ClosePane),
        action_item!("Navigate Up", Some("Ctrl+Shift+Up"), Action::NavigateUp),
        action_item!(
            "Navigate Down",
            Some("Ctrl+Shift+Down"),
            Action::NavigateDown
        ),
        action_item!(
            "Navigate Left",
            Some("Ctrl+Shift+Left"),
            Action::NavigateLeft
        ),
        action_item!(
            "Navigate Right",
            Some("Ctrl+Shift+Right"),
            Action::NavigateRight
        ),
        action_item!("Search", Some("Ctrl+Shift+F"), Action::Search),
        action_item!("History Search", Some("Ctrl+R"), Action::HistorySearch),
        action_item!("Clear Screen", Some("Ctrl+L"), Action::ClearScreen),
        action_item!("Copy", Some("Ctrl+Shift+C"), Action::Copy),
        action_item!("Paste", Some("Ctrl+Shift+V"), Action::Paste),
        action_item!("Font Increase", Some("Ctrl+="), Action::FontIncrease),
        action_item!("Font Decrease", Some("Ctrl+-"), Action::FontDecrease),
        action_item!("Font Reset", Some("Ctrl+0"), Action::FontReset),
        action_item!("Toggle Fullscreen", Some("F11"), Action::Fullscreen),
        action_item!(
            "Toggle Effects",
            Some("Ctrl+Shift+E"),
            Action::EffectsToggle
        ),
        action_item!(
            "Toggle Status Bar",
            Some("Ctrl+Shift+S"),
            Action::ToggleStatusBar
        ),
        action_item!(
            "Toggle Copy Mode",
            Some("Ctrl+Shift+Space"),
            Action::ToggleCopyMode
        ),
        action_item!("Reload Config", Some("Ctrl+,"), Action::ReloadConfig),
        action_item!(
            "Resize Pane Left",
            Some("Ctrl+Shift+Alt+Left"),
            Action::ResizePaneLeft
        ),
        action_item!(
            "Resize Pane Right",
            Some("Ctrl+Shift+Alt+Right"),
            Action::ResizePaneRight
        ),
        action_item!(
            "Resize Pane Up",
            Some("Ctrl+Shift+Alt+Up"),
            Action::ResizePaneUp
        ),
        action_item!(
            "Resize Pane Down",
            Some("Ctrl+Shift+Alt+Down"),
            Action::ResizePaneDown
        ),
        action_item!(
            "Jump to Previous Mark",
            Some("Ctrl+Up"),
            Action::JumpPrevMark
        ),
        action_item!("Jump to Next Mark", Some("Ctrl+Down"), Action::JumpNextMark),
        action_item!("New Workspace", Some("Ctrl+Shift+N"), Action::WorkspaceNew),
        action_item!(
            "Switch Workspace",
            Some("Ctrl+Alt+Tab"),
            Action::WorkspaceSwitch
        ),
        action_item!(
            "Delete Workspace",
            Some("Ctrl+Shift+Alt+D"),
            Action::WorkspaceDelete
        ),
        action_item!("Toggle Profiler", Some("F12"), Action::ToggleProfiler),
        action_item!("Show Keybinds", Some("F1"), Action::ToggleKeybinds),
        action_item!("Settings", Some("F2"), Action::ToggleSettings),
    ];

    for (i, plugin) in plugins.iter().enumerate() {
        let keybind_str = plugin
            .keybind
            .as_ref()
            .map(|s| {
                synapse_config::combo_to_string(&synapse_config::parse_keybind_string(s).unwrap_or(
                    synapse_config::KeyCombo {
                        key: s.clone(),
                        ctrl: false,
                        shift: false,
                        alt: false,
                    },
                ))
            })
            .or_else(|| Some("<none>".to_string()));
        items.push(PaletteItem::Action {
            label: format!("Plugin: {}", plugin.name),
            keybind: keybind_str,
            action: Action::PluginExecute(i),
        });
    }

    for (i, tab) in tab_bar.tabs.iter().enumerate() {
        items.push(PaletteItem::Tab {
            label: format!("Switch to: {}", tab.title),
            index: i,
        });
    }

    items.push(theme_item!("synapse_"));
    items.push(theme_item!("dracula"));
    items.push(theme_item!("catppuccin-mocha"));
    items.push(theme_item!("tokyo-night"));

    items
}

pub fn do_fuzzy_filter(query: &str, items: &[PaletteItem]) -> Vec<PaletteItem> {
    let mut scored: Vec<(i32, PaletteItem)> = items
        .iter()
        .filter_map(|item| {
            let label = match item {
                PaletteItem::Action { label, .. } => label,
                PaletteItem::Tab { label, .. } => label,
                PaletteItem::Theme { name } => name,
            };
            fuzzy_score(query, label).map(|s| (s, item.clone()))
        })
        .collect();
    scored.sort_by_key(|(s, _)| *s);
    scored.into_iter().map(|(_, item)| item).collect()
}

pub fn handle_palette_input(
    key: &winit::keyboard::Key,
    event: &winit::event::KeyEvent,
    state: &mut AppState,
    tab_bar: &mut TabBar,
) {
    use winit::keyboard::NamedKey;

    match key {
        Key::Named(NamedKey::Escape) => {
            state.palette.active = false;
        }
        Key::Named(NamedKey::Enter) => {
            if !state.palette.results.is_empty() {
                let idx = state.palette.selected.min(state.palette.results.len() - 1);
                let item = state.palette.results[idx].clone();
                execute_palette_item(item, state, tab_bar);
                state.palette.active = false;
            }
        }
        Key::Named(NamedKey::ArrowUp) => {
            if state.palette.selected > 0 {
                state.palette.selected -= 1;
            }
        }
        Key::Named(NamedKey::ArrowDown) => {
            if !state.palette.results.is_empty()
                && state.palette.selected + 1 < state.palette.results.len()
            {
                state.palette.selected += 1;
            }
        }
        Key::Named(NamedKey::Backspace) => {
            state.palette.query.pop();
            let items = build_palette_items(tab_bar, &state.palette.plugins);
            state.palette.results = do_fuzzy_filter(&state.palette.query, &items);
            if state.palette.selected >= state.palette.results.len() {
                state.palette.selected = state.palette.results.len().saturating_sub(1);
            }
        }
        Key::Named(NamedKey::Delete) => {
            if !state.palette.query.is_empty() {
                state.palette.query.remove(0);
            }
            let items = build_palette_items(tab_bar, &state.palette.plugins);
            state.palette.results = do_fuzzy_filter(&state.palette.query, &items);
            if state.palette.selected >= state.palette.results.len() {
                state.palette.selected = state.palette.results.len().saturating_sub(1);
            }
        }
        _ => {
            if let Some(text) = &event.text {
                if !text.is_empty() {
                    for c in text.chars() {
                        if !c.is_control() {
                            state.palette.query.push(c);
                        }
                    }
                    let items = build_palette_items(tab_bar, &state.palette.plugins);
                    state.palette.results = do_fuzzy_filter(&state.palette.query, &items);
                    state.palette.selected = 0;
                }
            }
        }
    }
}

pub fn execute_palette_item(item: PaletteItem, state: &mut AppState, tab_bar: &mut TabBar) {
    match item {
        PaletteItem::Action { action, .. } => {
            state.palette.pending_action = Some(action);
        }
        PaletteItem::Tab { index, .. } => {
            if index < tab_bar.tabs.len() {
                tab_bar.active = index;
            }
        }
        PaletteItem::Theme { name } => {
            let new_theme =
                synapse_config::themes::Theme::load(&name, synapse_config::Config::config_dir());
            state.theme = new_theme;
            state.palette.pending_theme_reload = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use synapse_ui::pane::PaneId;
    use synapse_ui::tab_bar::{Tab, TabBar, TabId};

    fn empty_tab_bar() -> TabBar {
        let tab = Tab::new(TabId(1), PaneId(1));
        let mut tb = TabBar::new(tab);
        tb.tabs.clear();
        tb
    }

    #[test]
    fn test_palette_state_new() {
        let state = PaletteState::new();
        assert!(!state.active);
        assert!(state.query.is_empty());
        assert!(state.results.is_empty());
        assert_eq!(state.selected, 0);
        assert!(state.pending_action.is_none());
        assert!(!state.pending_theme_reload);
    }

    #[test]
    fn test_palette_toggle_opens() {
        let mut state = PaletteState::new();
        let tb = empty_tab_bar();
        state.toggle(&tb);
        assert!(state.active);
        assert!(state.query.is_empty());
        assert!(!state.results.is_empty());
    }

    #[test]
    fn test_palette_toggle_closes() {
        let mut state = PaletteState::new();
        let tb = empty_tab_bar();
        state.active = true;
        state.query = "test".into();
        state.toggle(&tb);
        assert!(!state.active);
    }

    #[test]
    fn test_take_pending_action() {
        let mut state = PaletteState::new();
        assert!(state.take_pending_action().is_none());
        state.pending_action = Some(Action::NewTab);
        assert_eq!(state.take_pending_action(), Some(Action::NewTab));
        assert!(state.pending_action.is_none());
    }

    #[test]
    fn test_take_pending_theme_reload() {
        let mut state = PaletteState::new();
        assert!(!state.take_pending_theme_reload());
        state.pending_theme_reload = true;
        assert!(state.take_pending_theme_reload());
        assert!(!state.pending_theme_reload);
    }

    #[test]
    fn test_fuzzy_score_exact_match() {
        assert!(fuzzy_score("new", "New Tab").is_some());
        assert!(fuzzy_score("close", "Close Tab").is_some());
        assert!(fuzzy_score("split", "Split Vertical").is_some());
    }

    #[test]
    fn test_fuzzy_score_case_insensitive() {
        assert!(fuzzy_score("NEW", "New Tab").is_some());
        assert!(fuzzy_score("new tab", "New Tab").is_some());
        assert!(fuzzy_score("CLOSE", "Close Tab").is_some());
    }

    #[test]
    fn test_fuzzy_score_subsequence() {
        assert!(fuzzy_score("nw", "New").is_some());
        assert!(fuzzy_score("nt", "New Tab").is_some());
        assert!(fuzzy_score("sv", "Split Vertical").is_some());
    }

    #[test]
    fn test_fuzzy_score_no_match() {
        assert!(fuzzy_score("xyz", "New Tab").is_none());
        assert!(fuzzy_score("zzz", "Close Tab").is_none());
    }

    #[test]
    fn test_fuzzy_score_empty_query() {
        assert_eq!(fuzzy_score("", "anything"), Some(0));
    }

    #[test]
    fn test_do_fuzzy_filter_returns_all_on_empty_query() {
        let items = build_palette_items(&empty_tab_bar(), &[]);
        let results = do_fuzzy_filter("", &items);
        assert_eq!(results.len(), items.len());
    }

    #[test]
    fn test_do_fuzzy_filter_specific_action() {
        let items = build_palette_items(&empty_tab_bar(), &[]);
        let results = do_fuzzy_filter("split vertical", &items);
        assert!(!results.is_empty());
        assert!(results.iter().any(|item| matches!(
            item,
            PaletteItem::Action { label, .. } if label == "Split Vertical"
        )));
    }

    #[test]
    fn test_do_fuzzy_filter_no_results() {
        let items = build_palette_items(&empty_tab_bar(), &[]);
        let results = do_fuzzy_filter("zzz nonexistent", &items);
        assert!(results.is_empty());
    }

    #[test]
    fn test_do_fuzzy_filter_theme_results() {
        let items = build_palette_items(&empty_tab_bar(), &[]);
        let results = do_fuzzy_filter("dracula", &items);
        assert!(!results.is_empty());
        assert!(results.iter().any(|item| matches!(
            item,
            PaletteItem::Theme { name } if name == "dracula"
        )));
    }

    #[test]
    fn test_palette_item_clone() {
        let item = PaletteItem::Action {
            label: "Test".into(),
            keybind: Some("Ctrl+T".into()),
            action: Action::NewTab,
        };
        let cloned = item.clone();
        match cloned {
            PaletteItem::Action {
                label,
                keybind,
                action,
            } => {
                assert_eq!(label, "Test");
                assert_eq!(keybind, Some("Ctrl+T".to_string()));
                assert_eq!(action, Action::NewTab);
            }
            _ => panic!("wrong variant"),
        }
    }
}
