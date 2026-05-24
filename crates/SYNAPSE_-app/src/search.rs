use std::collections::HashSet;

use winit::{
    event::KeyEvent,
    keyboard::{Key, NamedKey},
};

use alacritty_terminal::grid::Dimensions;
use synapse_ui::pane::{EventProxy, Pane};
use synapse_ui::tab_bar::TabBar;

use crate::{
    pane_ops::active_pane_mut,
    state::{AppState, SearchMatch},
};

/// Search all rows (history + visible viewport) for `query` (case-insensitive).
/// When `regex_mode` is true, `query` is compiled as a regex pattern.
/// Returns `(matches, invalid_regex)` — `true` means the regex failed to compile.
pub fn find_matches(
    term: &alacritty_terminal::term::Term<EventProxy>,
    query: &str,
    regex_mode: bool,
) -> (Vec<SearchMatch>, bool) {
    if query.is_empty() {
        return (Vec::new(), false);
    }

    let grid = term.grid();
    let history_size = grid.history_size();
    let screen_lines = grid.screen_lines();
    let columns = grid.columns();

    if regex_mode {
        let re = match regex::Regex::new(query) {
            Ok(r) => r,
            Err(_) => return (Vec::new(), true),
        };

        let mut matches = Vec::new();
        let top = -(history_size as i32);
        let bottom = screen_lines as i32;

        for line_idx in top..bottom {
            let line = alacritty_terminal::index::Line(line_idx);
            let row = &grid[line];
            let chars: String = (0..columns)
                .map(|c| {
                    let ch = row[alacritty_terminal::index::Column(c)].c;
                    if ch == '\0' {
                        ' '
                    } else {
                        ch
                    }
                })
                .collect();

            for m in re.find_iter(&chars) {
                matches.push(SearchMatch {
                    col: m.start(),
                    row: line_idx,
                });
            }
        }
        return (matches, false);
    }

    let query_lower: Vec<char> = query.to_lowercase().chars().collect();
    let q_len = query_lower.len();

    let mut matches = Vec::new();

    let top = -(history_size as i32);
    let bottom = screen_lines as i32;

    for line_idx in top..bottom {
        let line = alacritty_terminal::index::Line(line_idx);
        let row = &grid[line];

        let chars: Vec<char> = (0..columns)
            .map(|c| {
                let ch = row[alacritty_terminal::index::Column(c)].c;
                if ch == '\0' {
                    ' '
                } else {
                    ch
                }
            })
            .collect();

        let chars_lower: Vec<char> = chars
            .iter()
            .map(|c| c.to_lowercase().next().unwrap_or(*c))
            .collect();

        let search_len = chars_lower.len();
        if search_len < q_len {
            continue;
        }
        for col in 0..=(search_len - q_len) {
            if chars_lower[col..col + q_len] == query_lower[..] {
                matches.push(SearchMatch { col, row: line_idx });
            }
        }
    }

    (matches, false)
}

/// Returns a `HashSet<(col, row)>` covering every cell occupied by any match.
/// Used by the renderer to highlight matched cells.
pub fn build_match_set(matches: &[SearchMatch], term_len: usize) -> HashSet<(usize, i32)> {
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
    let (matches, invalid) = find_matches(&term, &state.search.term, state.search.regex_mode);
    state.search.matches = matches;
    state.search.invalid_regex = invalid;
    state.search.current_match = 0;
    if state.search.matches.is_empty() || state.search.current_match >= state.search.matches.len() {
        state.search.current_match = 0;
    }
}

/// Scroll the active pane so the current search match is visible in the viewport.
pub fn scroll_to_current_match(state: &AppState, tab_bar: &TabBar, panes: &[Pane]) {
    let Some(m) = state.search.matches.get(state.search.current_match) else {
        return;
    };
    let Some(pane) = panes
        .iter()
        .find(|p| p.id == tab_bar.active_tab().active_pane)
    else {
        return;
    };
    let Ok(mut term) = pane.term.lock() else {
        return;
    };

    let screen_lines = term.screen_lines() as i32;
    let history_size = term.grid().history_size();

    // Determine the display_offset needed to centre the match row in the viewport.
    // display_offset=0 shows lines 0..screen_lines-1; display_offset=N shifts up by N.
    let target_offset = if m.row >= 0 {
        // Row is in the current viewport — scroll back to bottom.
        0usize
    } else {
        // Row is in history. Offset to put the match at ~30% from the top.
        let centre = (screen_lines / 3).max(1);
        let offset = (-m.row - 1 + centre).max(0) as usize;
        offset.min(history_size)
    };

    let current_offset = term.grid().display_offset();
    use alacritty_terminal::grid::Scroll;
    let delta = target_offset as i32 - current_offset as i32;
    if delta != 0 {
        term.scroll_display(Scroll::Delta(delta));
        pane.dirty.store(true, std::sync::atomic::Ordering::Release);
    }
}

pub fn handle_search_input(
    key: &Key,
    event: &KeyEvent,
    state: &mut AppState,
    tab_bar: &TabBar,
    panes: &[Pane],
) {
    if state.modifiers.alt_key() {
        if let Key::Character(c) = key {
            if c == "r" || c == "R" {
                state.search.toggle_regex();
                update_search_matches(state, tab_bar, panes);
                return;
            }
        }
    }

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
