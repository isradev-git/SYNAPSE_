use std::collections::HashSet;

use winit::{
    event::KeyEvent,
    keyboard::{Key, NamedKey},
};

use synapse_ui::pane::{EventProxy, Pane};
use synapse_ui::tab_bar::TabBar;

use crate::{
    pane_ops::active_pane_mut,
    state::{AppState, SearchMatch},
};

/// Phase 1 stub: scrollback search will be reimplemented on top of
/// alacritty_terminal's grid in Phase 2.
pub fn find_matches(
    _term: &alacritty_terminal::term::Term<EventProxy>,
    _term_str: &str,
) -> Vec<SearchMatch> {
    Vec::new()
}

pub fn build_match_set(matches: &[SearchMatch], term_len: usize) -> HashSet<(usize, usize)> {
    let mut set = HashSet::new();
    for m in matches {
        for i in 0..term_len {
            set.insert((m.col + i, m.row));
        }
    }
    set
}

pub fn update_search_matches(state: &mut AppState, tab_bar: &TabBar, panes: &[Pane]) {
    let pane = panes
        .iter()
        .find(|p| p.id == tab_bar.active_tab().active_pane)
        .unwrap();
    let term = pane.term.lock().unwrap();
    state.search.matches = find_matches(&term, &state.search.term);
    state.search.current_match = 0;
    if state.search.matches.is_empty() || state.search.current_match >= state.search.matches.len() {
        state.search.current_match = 0;
    }
}

/// Phase 1 stub: scrollback scrolling not yet wired through alacritty_terminal.
pub fn scroll_to_current_match(_state: &AppState, _tab_bar: &TabBar, _panes: &[Pane]) {}

pub fn handle_search_input(
    key: &Key,
    event: &KeyEvent,
    state: &mut AppState,
    tab_bar: &TabBar,
    panes: &[Pane],
) {
    match key {
        Key::Named(NamedKey::Escape) => {
            state.search.toggle();
        }
        Key::Named(NamedKey::Enter) => {
            if state.modifiers.shift_key() {
                state.search.prev_match();
            } else {
                state.search.next_match();
            }
            scroll_to_current_match(state, tab_bar, panes);
        }
        Key::Named(NamedKey::Backspace) => {
            state.search.backspace();
            update_search_matches(state, tab_bar, panes);
        }
        Key::Named(NamedKey::Delete) => {
            state.search.delete_forward();
            update_search_matches(state, tab_bar, panes);
        }
        Key::Named(NamedKey::ArrowLeft) => {
            state.search.move_left();
        }
        Key::Named(NamedKey::ArrowRight) => {
            state.search.move_right();
        }
        Key::Named(NamedKey::Home) => {
            state.search.move_home();
        }
        Key::Named(NamedKey::End) => {
            state.search.move_end();
        }
        _ => {
            if let Some(text) = &event.text {
                if !text.is_empty() {
                    for c in text.chars() {
                        if !c.is_control() {
                            state.search.insert_char(c);
                        }
                    }
                    update_search_matches(state, tab_bar, panes);
                }
            }
        }
    }
}

pub fn handle_history_search_input(
    key: &Key,
    event: &KeyEvent,
    state: &mut AppState,
    tab_bar: &TabBar,
    panes: &mut [Pane],
) {
    let ctrl = state.modifiers.control_key();

    match key {
        Key::Named(NamedKey::Escape) => {
            state.history_search.deactivate();
        }
        Key::Named(NamedKey::Enter) => {
            if let Some(text) = state.history_search.current_text().map(|s| s.to_string()) {
                let pane = active_pane_mut(panes, tab_bar);
                pane.write_to_pty(text.as_bytes());
            }
            state.history_search.deactivate();
        }
        Key::Named(NamedKey::Backspace) => {
            state.history_search.backspace();
            state.history_search.update_filter();
        }
        _ => {
            if ctrl {
                if let Key::Character(c) = key {
                    if c.as_str() == "r" || c.as_str() == "R" {
                        state.history_search.next_match();
                        return;
                    }
                }
            }

            if let Some(text) = &event.text {
                if !text.is_empty() {
                    for c in text.chars() {
                        if !c.is_control() {
                            state.history_search.insert_char(c);
                        }
                    }
                    state.history_search.update_filter();
                }
            }
        }
    }
}
