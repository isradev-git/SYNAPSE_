use std::collections::HashSet;

use winit::{
    event::KeyEvent,
    keyboard::{Key, NamedKey},
};

use luna_ui::{pane::Pane, tab_bar::TabBar};

use crate::{
    pane_ops::active_pane_mut,
    state::{AppState, SearchMatch},
};

pub fn find_matches(grid: &luna_terminal::grid::Grid, term: &str) -> Vec<SearchMatch> {
    let mut matches = Vec::new();
    if term.is_empty() {
        return matches;
    }
    let term_lower = term.to_lowercase();
    let lines = grid.all_lines();
    for (row, line) in lines.iter().enumerate() {
        let line_str: String = line.iter().collect();
        let line_lower = line_str.to_lowercase();
        let mut start = 0;
        while let Some(pos) = line_lower[start..].find(&term_lower) {
            matches.push(SearchMatch {
                col: start + pos,
                row,
            });
            start += pos + 1;
        }
    }
    matches
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
    let pane = panes.iter().find(|p| p.id == tab_bar.active_tab().active_pane).unwrap();
    let grid = pane.grid.borrow();
    state.search.matches = find_matches(&grid, &state.search.term);
    if state.search.matches.is_empty() {
        state.search.current_match = 0;
    } else if state.search.current_match >= state.search.matches.len() {
        state.search.current_match = 0;
    }
}

pub fn scroll_to_current_match(state: &AppState, tab_bar: &TabBar, panes: &[Pane]) {
    if state.search.matches.is_empty() {
        return;
    }
    let current = &state.search.matches[state.search.current_match];
    let pane = panes.iter().find(|p| p.id == tab_bar.active_tab().active_pane).unwrap();
    let mut grid = pane.grid.borrow_mut();
    let sb_len = grid.scrollback_len();
    let grid_rows = grid.rows();

    if current.row < sb_len {
        let target = if current.row >= grid_rows / 2 {
            current.row - grid_rows / 2
        } else {
            0
        };
        grid.set_scroll_offset(target);
    } else {
        grid.scroll_to_bottom();
    }
}

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
                let _ = pane.pty_session.pty.write(text.as_bytes());
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
