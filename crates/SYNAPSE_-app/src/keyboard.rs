use std::sync::Arc;

use winit::{event::KeyEvent, keyboard::ModifiersState, window::Window};

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::Side;
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::TermMode;
use std::sync::atomic::Ordering;
use synapse_config::Action;
use synapse_ui::pane::{CopyModeState, CopySelMode};
use synapse_ui::{
    auto_split_direction, layout::Layout, pane::Pane, splitter::PaneTree, splitter::SplitDirection,
    tab_bar::TabBar,
};

use crate::{
    input::InputAction,
    pane_ops::{active_pane_mut, adjacent_pane, create_pane_full, find_pane, write_to_panes},
    search::{handle_history_search_input, handle_search_input, update_search_matches},
    state::AppState,
};

pub enum PostKeyAction {
    None,
    FontChange(f32),
    ThemeChange,
    EffectsToggle,
    ToggleStatusBar,
    WorkspaceAction(synapse_config::keybinds::Action),
}

fn ensure_tab_visible(
    active_idx: usize,
    tab_count: usize,
    layout: &synapse_ui::layout::Layout,
    offset: &mut usize,
) {
    let (start, end, _, _) = layout.tab_visible_range(tab_count, *offset);
    if active_idx < start {
        *offset = active_idx;
    } else if active_idx >= end && end > start {
        let vis = end - start;
        *offset = active_idx.saturating_sub(vis.saturating_sub(1));
    }
}

pub(crate) fn sanitize_paste(text: &str) -> String {
    text.replace("\r\n", "\r").replace('\n', "\r")
}

fn bracketed_paste_active(pane: &Pane) -> bool {
    pane.term
        .lock()
        .map(|t| t.mode().contains(TermMode::BRACKETED_PASTE))
        .unwrap_or(false)
}

fn app_cursor_active(pane: &Pane) -> bool {
    pane.term
        .lock()
        .map(|t| t.mode().contains(TermMode::APP_CURSOR))
        .unwrap_or(false)
}

fn enter_copy_mode(pane: &mut Pane, state: &mut AppState) {
    let cursor = pane
        .term
        .lock()
        .map(|t| t.grid().cursor.point)
        .unwrap_or_default();
    pane.copy_mode = Some(CopyModeState {
        cursor,
        anchor: None,
        sel_mode: CopySelMode::None,
    });
    state.in_copy_mode = true;
    pane.dirty.store(true, Ordering::Release);
}

fn exit_copy_mode(pane: &mut Pane, state: &mut AppState) {
    if let Ok(mut term) = pane.term.lock() {
        term.selection = None;
    }
    pane.copy_mode = None;
    state.in_copy_mode = false;
    pane.dirty.store(true, Ordering::Release);
}

pub(crate) fn toggle_zoom(state: &mut AppState, tab_bar: &mut TabBar) {
    if let Some(_zoomed) = state.zoomed_pane.take() {
        if let Some(tree) = state.zoom_saved_tree.take() {
            tab_bar.active_tab_mut().pane_tree = tree;
        }
    } else {
        let active_id = tab_bar.active_tab().active_pane;
        let saved = tab_bar.active_tab().pane_tree.clone();
        state.zoom_saved_tree = Some(saved);
        state.zoomed_pane = Some(active_id);
        tab_bar.active_tab_mut().pane_tree = PaneTree::leaf(active_id);
    }
}

pub(crate) fn clear_zoom(state: &mut AppState, tab_bar: &mut TabBar) {
    if state.zoomed_pane.is_some() {
        toggle_zoom(state, tab_bar);
    }
}

fn compute_moved_cursor(
    cursor: alacritty_terminal::index::Point,
    delta_col: i32,
    delta_row: i32,
    cols: i32,
    rows: i32,
    history: i32,
) -> alacritty_terminal::index::Point {
    use alacritty_terminal::index::{Column, Line, Point};
    if cols <= 0 || rows <= 0 {
        return cursor;
    }
    let new_col = (cursor.column.0 as i32 + delta_col).clamp(0, cols - 1);
    let new_row = (cursor.line.0 + delta_row).clamp(-history, rows - 1);
    Point::new(Line(new_row), Column(new_col as usize))
}

fn compute_scroll_delta(viewport_row: i32, screen_lines: i32) -> i32 {
    if viewport_row < 0 {
        -viewport_row
    } else if viewport_row >= screen_lines {
        screen_lines - 1 - viewport_row
    } else {
        0
    }
}

fn move_cursor(pane: &mut Pane, delta_col: i32, delta_row: i32) {
    let (cols, rows, history) = {
        match pane.term.lock() {
            Ok(t) => (
                t.columns() as i32,
                t.screen_lines() as i32,
                t.grid().history_size() as i32,
            ),
            Err(_) => return,
        }
    };
    if let Some(ref mut cms) = pane.copy_mode {
        cms.cursor = compute_moved_cursor(cms.cursor, delta_col, delta_row, cols, rows, history);
        pane.dirty.store(true, Ordering::Release);
    }
}

fn scroll_to_follow_cursor(pane: &mut Pane) {
    let raw_row = match pane.copy_mode.as_ref() {
        Some(cms) => cms.cursor.line.0,
        None => return,
    };
    let (display_offset, screen_lines) = {
        match pane.term.lock() {
            Ok(t) => (t.grid().display_offset() as i32, t.screen_lines() as i32),
            Err(_) => return,
        }
    };
    let viewport_row = raw_row + display_offset;
    let delta = compute_scroll_delta(viewport_row, screen_lines);
    if delta != 0 {
        pane.scroll_viewport(alacritty_terminal::grid::Scroll::Delta(delta));
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn cell_char_at(
    term: &alacritty_terminal::term::Term<synapse_ui::pane::EventProxy>,
    row: i32,
    col: usize,
) -> char {
    let grid = term.grid();
    let cell = &grid[alacritty_terminal::index::Line(row)][alacritty_terminal::index::Column(col)];
    cell.c
}

fn word_motion_w(pane: &mut Pane) {
    let target = {
        let term = match pane.term.lock() {
            Ok(t) => t,
            Err(_) => return,
        };
        let start = match pane.copy_mode.as_ref() {
            Some(cms) => cms.cursor,
            None => return,
        };
        let cols = term.columns();
        if cols == 0 {
            return;
        }
        let max_row = term.screen_lines() as i32 - 1;
        let mut row = start.line.0;
        let mut col = start.column.0;

        // Skip current word chars
        while row <= max_row && is_word_char(cell_char_at(&term, row, col)) {
            col += 1;
            if col >= cols {
                col = 0;
                row += 1;
            }
        }
        // Skip whitespace
        while row <= max_row && !is_word_char(cell_char_at(&term, row, col)) {
            col += 1;
            if col >= cols {
                col = 0;
                row += 1;
            }
        }
        if row > max_row {
            row = max_row;
            col = cols.saturating_sub(1);
        }
        alacritty_terminal::index::Point::new(
            alacritty_terminal::index::Line(row),
            alacritty_terminal::index::Column(col),
        )
    };
    if let Some(ref mut cms) = pane.copy_mode {
        if cms.cursor != target {
            cms.cursor = target;
            pane.dirty.store(true, Ordering::Release);
        }
    }
}

fn word_motion_b(pane: &mut Pane) {
    let target = {
        let term = match pane.term.lock() {
            Ok(t) => t,
            Err(_) => return,
        };
        let start = match pane.copy_mode.as_ref() {
            Some(cms) => cms.cursor,
            None => return,
        };
        let cols = term.columns();
        let min_row = -(term.grid().history_size() as i32);
        let mut row = start.line.0;
        let mut col = start.column.0;

        // Step back one position to start movement
        if col == 0 {
            col = cols.saturating_sub(1);
            row -= 1;
        } else {
            col -= 1;
        }
        if row < min_row {
            row = min_row;
            col = 0;
        }

        // Skip whitespace backwards
        while row >= min_row && !is_word_char(cell_char_at(&term, row, col)) {
            if col == 0 {
                if row <= min_row {
                    break;
                }
                col = cols.saturating_sub(1);
                row -= 1;
            } else {
                col -= 1;
            }
        }
        // Find start of word
        while row >= min_row && is_word_char(cell_char_at(&term, row, col)) {
            if col == 0 {
                break;
            }
            col -= 1;
        }
        // If we stepped back past the start of word, advance one
        if !is_word_char(cell_char_at(&term, row, col)) && col + 1 < cols {
            col += 1;
        }
        alacritty_terminal::index::Point::new(
            alacritty_terminal::index::Line(row),
            alacritty_terminal::index::Column(col),
        )
    };
    if let Some(ref mut cms) = pane.copy_mode {
        if cms.cursor != target {
            cms.cursor = target;
            pane.dirty.store(true, Ordering::Release);
        }
    }
}

fn word_motion_e(pane: &mut Pane) {
    let target = {
        let term = match pane.term.lock() {
            Ok(t) => t,
            Err(_) => return,
        };
        let start = match pane.copy_mode.as_ref() {
            Some(cms) => cms.cursor,
            None => return,
        };
        let cols = term.columns();
        if cols == 0 {
            return;
        }
        let max_row = term.screen_lines() as i32 - 1;
        let mut row = start.line.0;
        let mut col = start.column.0;

        // Advance one position to start
        col += 1;
        if col >= cols {
            col = 0;
            row += 1;
        }
        if row > max_row {
            row = max_row;
            col = cols.saturating_sub(1);
        }

        // Skip whitespace
        while row <= max_row && !is_word_char(cell_char_at(&term, row, col)) {
            col += 1;
            if col >= cols {
                col = 0;
                row += 1;
            }
        }
        // Advance to end of word
        while row <= max_row {
            let next_col = col + 1;
            let next_row = if next_col >= cols { row + 1 } else { row };
            let nc = if next_col >= cols { 0 } else { next_col };
            if next_row > max_row || !is_word_char(cell_char_at(&term, next_row, nc)) {
                break;
            }
            col = nc;
            row = next_row;
        }
        if row > max_row {
            row = max_row;
            col = cols.saturating_sub(1);
        }
        alacritty_terminal::index::Point::new(
            alacritty_terminal::index::Line(row),
            alacritty_terminal::index::Column(col),
        )
    };
    if let Some(ref mut cms) = pane.copy_mode {
        if cms.cursor != target {
            cms.cursor = target;
            pane.dirty.store(true, Ordering::Release);
        }
    }
}

fn update_selection_after_move(pane: &mut Pane) {
    let (sel_mode, cursor) = match pane.copy_mode.as_ref() {
        Some(cms) => (cms.sel_mode, cms.cursor),
        None => return,
    };
    if sel_mode == CopySelMode::None {
        return;
    }
    if let Ok(mut term) = pane.term.lock() {
        if let Some(ref mut sel) = term.selection {
            sel.update(cursor, Side::Right);
        }
    }
}

fn handle_copy_mode_key(
    key: &winit::keyboard::Key,
    _modifiers: ModifiersState,
    pane: &mut Pane,
    state: &mut AppState,
    clipboard: &mut Option<arboard::Clipboard>,
) {
    use winit::keyboard::NamedKey;
    let key_char: Option<&str> = match key {
        winit::keyboard::Key::Character(c) => Some(c.as_str()),
        _ => None,
    };
    let is_escape = matches!(key, winit::keyboard::Key::Named(NamedKey::Escape));

    if is_escape || key_char == Some("q") || key_char == Some("Q") {
        exit_copy_mode(pane, state);
        return;
    }

    match key_char {
        Some("h") => {
            move_cursor(pane, -1, 0);
            scroll_to_follow_cursor(pane);
            update_selection_after_move(pane);
        }
        Some("j") => {
            move_cursor(pane, 0, 1);
            scroll_to_follow_cursor(pane);
            update_selection_after_move(pane);
        }
        Some("k") => {
            move_cursor(pane, 0, -1);
            scroll_to_follow_cursor(pane);
            update_selection_after_move(pane);
        }
        Some("l") => {
            move_cursor(pane, 1, 0);
            scroll_to_follow_cursor(pane);
            update_selection_after_move(pane);
        }
        Some("w") => {
            word_motion_w(pane);
            scroll_to_follow_cursor(pane);
            update_selection_after_move(pane);
        }
        Some("b") => {
            word_motion_b(pane);
            scroll_to_follow_cursor(pane);
            update_selection_after_move(pane);
        }
        Some("e") => {
            word_motion_e(pane);
            scroll_to_follow_cursor(pane);
            update_selection_after_move(pane);
        }
        Some("v") => {
            let cursor = match pane.copy_mode.as_ref() {
                Some(cms) => cms.cursor,
                None => return,
            };
            if let Some(ref mut cms) = pane.copy_mode {
                cms.anchor = Some(cursor);
                cms.sel_mode = CopySelMode::Char;
            }
            if let Ok(mut term) = pane.term.lock() {
                term.selection = Some(Selection::new(SelectionType::Simple, cursor, Side::Left));
            }
            pane.dirty.store(true, Ordering::Release);
        }
        Some("V") => {
            let cursor = match pane.copy_mode.as_ref() {
                Some(cms) => cms.cursor,
                None => return,
            };
            if let Some(ref mut cms) = pane.copy_mode {
                cms.anchor = Some(cursor);
                cms.sel_mode = CopySelMode::Line;
            }
            if let Ok(mut term) = pane.term.lock() {
                term.selection = Some(Selection::new(SelectionType::Lines, cursor, Side::Left));
            }
            pane.dirty.store(true, Ordering::Release);
        }
        Some("y") => {
            let sel_mode = pane
                .copy_mode
                .as_ref()
                .map(|cms| cms.sel_mode)
                .unwrap_or(CopySelMode::None);
            let cursor = match pane.copy_mode.as_ref() {
                Some(cms) => cms.cursor,
                None => {
                    exit_copy_mode(pane, state);
                    return;
                }
            };
            let text = pane.term.lock().ok().and_then(|mut t| {
                if sel_mode == CopySelMode::None {
                    t.selection = Some(Selection::new(SelectionType::Lines, cursor, Side::Left));
                }
                t.selection_to_string()
            });
            if let Some(text) = text {
                if let Some(ref mut cb) = *clipboard {
                    let _ = cb.set_text(text);
                }
            }
            exit_copy_mode(pane, state);
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_keyboard(
    event: &KeyEvent,
    state: &mut AppState,
    tab_bar: &mut TabBar,
    panes: &mut Vec<Pane>,
    layout: &Layout,
    margin: f32,
    cell_w: f32,
    cell_h: f32,
    clipboard: &mut Option<arboard::Clipboard>,
    window: &Arc<Window>,
) -> PostKeyAction {
    // Read KKP state from the active pane (set by the PTY reader thread).
    let active_id = tab_bar.active_tab().active_pane;
    let kitty_flags = find_pane(panes, active_id)
        .map(|p| p.kitty_flags())
        .unwrap_or(0);
    let kitty_active = find_pane(panes, active_id)
        .map(|p| p.kitty_active())
        .unwrap_or(false);

    let is_release = event.state == winit::event::ElementState::Released;
    // KKP can request release events (flags bit 1). Legacy path ignores them.
    let kkp_wants_release = kitty_active && (kitty_flags & 2 != 0);
    if is_release && !kkp_wants_release {
        return PostKeyAction::None;
    }
    // Forward release events directly to KKP encoder without keybind processing.
    if is_release && kkp_wants_release {
        let pane = active_pane_mut(panes, tab_bar);
        let bytes = InputAction::from_key_kitty(event, state.modifiers, kitty_flags, true);
        if let InputAction::Write(b) = bytes {
            pane.write_to_pty(&b);
        }
        return PostKeyAction::None;
    }

    if event.state == winit::event::ElementState::Pressed {
        let logical_key = &event.logical_key;
        let is_repeat = event.repeat;

        // ToggleCopyMode: first press only, processed before the copy mode gate.
        if !is_repeat {
            if let Some(Action::ToggleCopyMode) =
                state.keybinds.lookup(logical_key, state.modifiers)
            {
                let pane = active_pane_mut(panes, tab_bar);
                if state.in_copy_mode {
                    exit_copy_mode(pane, state);
                } else {
                    enter_copy_mode(pane, state);
                }
                return PostKeyAction::None;
            }
        }

        // Copy mode gate: consume all keypresses (including repeats) when active.
        if state.in_copy_mode {
            let pane = active_pane_mut(panes, tab_bar);
            handle_copy_mode_key(logical_key, state.modifiers, pane, state, clipboard);
            return PostKeyAction::None;
        }

        // Only process keybinds and search UI on first press — repeats go
        // straight to input encoding below so holding Backspace works.
        if !is_repeat {
            // Search input handling (when active)
            if state.search.active {
                handle_search_input(logical_key, event, state, tab_bar, panes);
                return PostKeyAction::None;
            }

            // History search input handling (when active)
            if state.history_search.active {
                handle_history_search_input(logical_key, event, state, tab_bar, panes);
                return PostKeyAction::None;
            }

            // Overlay input handling (when active)
            if state.overlay.active {
                return handle_overlay_input(logical_key, state);
            }

            // Command palette input handling (when active)
            if state.palette.active {
                crate::palette::handle_palette_input(logical_key, event, state, tab_bar);
                // If palette just closed via Enter with a pending action or theme reload:
                if !state.palette.active {
                    if let Some(action) = state.palette.take_pending_action() {
                        match action {
                            Action::WorkspaceNew
                            | Action::WorkspaceSwitch
                            | Action::WorkspaceDelete
                            | Action::ToggleProfiler
                            | Action::ToggleRecording
                            | Action::ToggleKeybinds
                            | Action::ToggleSettings
                            | Action::PluginExecute(_) => {
                                return PostKeyAction::WorkspaceAction(action);
                            }
                            _ => {
                                let result = dispatch_action(
                                    action, state, tab_bar, panes, layout, margin, cell_w, cell_h,
                                    clipboard, window,
                                );
                                return result;
                            }
                        }
                    }
                    if state.palette.take_pending_theme_reload() {
                        return PostKeyAction::ThemeChange;
                    }
                }
                return PostKeyAction::None;
            }

            // Keybind lookup
            let action_opt = state.keybinds.lookup(logical_key, state.modifiers);
            let mut keybind_handled = true;
            match action_opt {
                Some(Action::Search) => {
                    state.search.toggle();
                    if state.search.active {
                        update_search_matches(state, tab_bar, panes);
                    }
                }
                Some(Action::PaletteOpen) => {
                    state.palette.toggle(tab_bar);
                }
                Some(Action::HistorySearch) => {
                    if state.history_search.active {
                        state.history_search.next_match();
                    } else {
                        let active_id = tab_bar.active_tab().active_pane;
                        if let Some(pane) = find_pane(panes, active_id) {
                            if let Ok(term) = pane.term.lock() {
                                let grid = term.grid();
                                let history_size = grid.history_size();
                                let screen_lines = grid.screen_lines();
                                let mut lines_buf: Vec<Vec<char>> = Vec::new();
                                for line_idx in (-(history_size as i32))..(screen_lines as i32) {
                                    let row = &grid[alacritty_terminal::index::Line(line_idx)];
                                    let line: Vec<char> =
                                        row.into_iter().map(|cell| cell.c).collect();
                                    lines_buf.push(line);
                                }
                                state.history_search.activate();
                                state.history_search.build_history(&lines_buf);
                                // Inject persistent history entries
                                if state.config.persistent_history {
                                    state
                                        .history_search
                                        .add_persistent_commands(state.command_history.commands());
                                }
                            }
                        }
                    }
                }
                Some(Action::ClearScreen) => {
                    let pane = active_pane_mut(panes, tab_bar);
                    pane.write_to_pty(b"\x0c");
                }
                Some(Action::NewTab) => {
                    let pane_area = layout.pane_area();
                    let new_cols = ((pane_area.2 - margin * 2.0) / cell_w).max(1.0) as usize;
                    let new_rows = ((pane_area.3 - margin * 2.0) / cell_h).max(1.0) as usize;
                    let (_, pane_id) = tab_bar.new_tab();
                    let shell = state.config.shell_program.as_str();
                    let args = &state.config.shell_args;
                    match create_pane_full(
                        pane_id,
                        new_cols,
                        new_rows,
                        None,
                        Some(shell),
                        args,
                        state.config.scrollback_lines,
                    ) {
                        Ok(pane) => panes.push(pane),
                        Err(e) => {
                            tracing::warn!("Failed to spawn PTY for new tab: {}", e);
                            tab_bar.close_tab(tab_bar.active);
                            return PostKeyAction::None;
                        }
                    }
                    let n = tab_bar.tabs.len();
                    ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
                }
                Some(Action::CloseTab) => {
                    clear_zoom(state, tab_bar);
                    if let Some(closed) = tab_bar.close_tab(tab_bar.active) {
                        let closed_panes = closed.pane_tree.all_panes();
                        // Dropping the panes drops their PTY writers/masters and ends the children.
                        panes.retain(|p| !closed_panes.contains(&p.id));
                    }
                    let n = tab_bar.tabs.len();
                    if state.tab_scroll_offset >= n && n > 0 {
                        state.tab_scroll_offset = n - 1;
                    }
                    ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
                }
                Some(Action::NextTab) => {
                    clear_zoom(state, tab_bar);
                    tab_bar.next_tab();
                    let n = tab_bar.tabs.len();
                    ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
                }
                Some(Action::PrevTab) => {
                    clear_zoom(state, tab_bar);
                    tab_bar.prev_tab();
                    let n = tab_bar.tabs.len();
                    ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
                }
                Some(Action::TabSwitch1) => {
                    clear_zoom(state, tab_bar);
                    tab_bar.activate(0);
                    let n = tab_bar.tabs.len();
                    ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
                }
                Some(Action::TabSwitch2) => {
                    clear_zoom(state, tab_bar);
                    tab_bar.activate(1);
                    let n = tab_bar.tabs.len();
                    ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
                }
                Some(Action::TabSwitch3) => {
                    clear_zoom(state, tab_bar);
                    tab_bar.activate(2);
                    let n = tab_bar.tabs.len();
                    ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
                }
                Some(Action::TabSwitch4) => {
                    clear_zoom(state, tab_bar);
                    tab_bar.activate(3);
                    let n = tab_bar.tabs.len();
                    ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
                }
                Some(Action::TabSwitch5) => {
                    clear_zoom(state, tab_bar);
                    tab_bar.activate(4);
                    let n = tab_bar.tabs.len();
                    ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
                }
                Some(Action::TabSwitch6) => {
                    clear_zoom(state, tab_bar);
                    tab_bar.activate(5);
                    let n = tab_bar.tabs.len();
                    ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
                }
                Some(Action::TabSwitch7) => {
                    clear_zoom(state, tab_bar);
                    tab_bar.activate(6);
                    let n = tab_bar.tabs.len();
                    ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
                }
                Some(Action::TabSwitch8) => {
                    clear_zoom(state, tab_bar);
                    tab_bar.activate(7);
                    let n = tab_bar.tabs.len();
                    ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
                }
                Some(Action::TabSwitch9) => {
                    clear_zoom(state, tab_bar);
                    tab_bar.activate(8);
                    let n = tab_bar.tabs.len();
                    ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
                }
                Some(Action::SplitVertical) => {
                    let active_id = tab_bar.active_tab().active_pane;
                    let new_pane_id = tab_bar.next_pane_id();
                    if tab_bar
                        .active_tab_mut()
                        .pane_tree
                        .split(active_id, new_pane_id, SplitDirection::Vertical)
                        .is_ok()
                    {
                        if let Some(pane) = find_pane(panes, active_id) {
                            let cwd = pane.cwd();
                            let cwd_opt = if cwd.is_empty() { None } else { Some(cwd) };
                            let shell = state.config.shell_program.as_str();
                            let args = &state.config.shell_args;
                            match create_pane_full(
                                new_pane_id,
                                pane.cols,
                                pane.rows,
                                cwd_opt,
                                Some(shell),
                                args,
                                state.config.scrollback_lines,
                            ) {
                                Ok(new_pane) => panes.push(new_pane),
                                Err(e) => {
                                    tracing::warn!("Failed to spawn PTY for vertical split: {}", e)
                                }
                            }
                        }
                    }
                }
                Some(Action::SplitHorizontal) => {
                    let active_id = tab_bar.active_tab().active_pane;
                    let new_pane_id = tab_bar.next_pane_id();
                    if tab_bar
                        .active_tab_mut()
                        .pane_tree
                        .split(active_id, new_pane_id, SplitDirection::Horizontal)
                        .is_ok()
                    {
                        if let Some(pane) = find_pane(panes, active_id) {
                            let cwd = pane.cwd();
                            let cwd_opt = if cwd.is_empty() { None } else { Some(cwd) };
                            let shell = state.config.shell_program.as_str();
                            let args = &state.config.shell_args;
                            match create_pane_full(
                                new_pane_id,
                                pane.cols,
                                pane.rows,
                                cwd_opt,
                                Some(shell),
                                args,
                                state.config.scrollback_lines,
                            ) {
                                Ok(new_pane) => panes.push(new_pane),
                                Err(e) => {
                                    tracing::warn!(
                                        "Failed to spawn PTY for horizontal split: {}",
                                        e
                                    )
                                }
                            }
                        }
                    }
                }
                Some(Action::ClosePane) => {
                    let pane_count = tab_bar.active_tab().pane_tree.all_panes().len();
                    if pane_count <= 1 {
                        keybind_handled = true;
                    } else {
                        let active_id = tab_bar.active_tab().active_pane;
                        if let Some(removed) = tab_bar.active_tab_mut().pane_tree.close(active_id) {
                            // Dropping the Pane closes its PTY writer/master.
                            panes.retain(|p| p.id != removed);
                            let remaining = tab_bar.active_tab().pane_tree.all_panes();
                            if !remaining.is_empty() {
                                tab_bar.active_tab_mut().active_pane = remaining[0];
                            }
                        }
                    }
                }
                Some(Action::NavigateUp)
                | Some(Action::NavigateDown)
                | Some(Action::NavigateLeft)
                | Some(Action::NavigateRight) => {
                    let dir = match action_opt {
                        Some(Action::NavigateUp) => "up",
                        Some(Action::NavigateDown) => "down",
                        Some(Action::NavigateLeft) => "left",
                        Some(Action::NavigateRight) => "right",
                        _ => unreachable!(),
                    };
                    let pane_area = layout.pane_area();
                    let pane_rect = synapse_ui::PaneRect {
                        x: pane_area.0,
                        y: pane_area.1,
                        w: pane_area.2,
                        h: pane_area.3,
                    };
                    let layouts = tab_bar.active_tab().pane_tree.get_layout(pane_rect);
                    let active_id = tab_bar.active_tab().active_pane;
                    if let Some(next) = adjacent_pane(&layouts, active_id, dir) {
                        tab_bar.active_tab_mut().active_pane = next;
                        state.pane_label_until =
                            Some(std::time::Instant::now() + std::time::Duration::from_millis(600));
                        state.pane_label_id = next.0 as u32;
                    }
                }
                Some(Action::FontIncrease) => {
                    return PostKeyAction::FontChange((state.font_size + 1.0).min(32.0));
                }
                Some(Action::FontDecrease) => {
                    return PostKeyAction::FontChange((state.font_size - 1.0).max(6.0));
                }
                Some(Action::FontReset) => {
                    return PostKeyAction::FontChange(state.config.font_size);
                }
                Some(Action::Fullscreen) => {
                    state.fullscreen = !state.fullscreen;
                    if state.fullscreen {
                        window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                    } else {
                        window.set_fullscreen(None);
                    }
                }
                Some(Action::Copy) => {
                    let active_id = tab_bar.active_tab().active_pane;
                    if let Some(pane) = find_pane(panes, active_id) {
                        if let Ok(term) = pane.term.lock() {
                            if let Some(text) = term.selection_to_string() {
                                if let Some(ref mut clip) = clipboard {
                                    let _ = clip.set_text(text);
                                }
                            }
                        }
                    }
                }
                Some(Action::Paste) => {
                    if let Some(ref mut clip) = clipboard {
                        if let Ok(text) = clip.get_text() {
                            let bracketed = {
                                let pane = active_pane_mut(panes, tab_bar);
                                bracketed_paste_active(pane)
                            };
                            if bracketed {
                                write_to_panes(panes, tab_bar, state.broadcasting, b"\x1b[200~");
                                write_to_panes(
                                    panes,
                                    tab_bar,
                                    state.broadcasting,
                                    sanitize_paste(&text).as_bytes(),
                                );
                                write_to_panes(panes, tab_bar, state.broadcasting, b"\x1b[201~");
                            } else {
                                write_to_panes(panes, tab_bar, state.broadcasting, text.as_bytes());
                            }
                        }
                    }
                }
                Some(Action::JumpPrevMark) => {
                    let active_id = tab_bar.active_tab().active_pane;
                    if let Some(pane) = panes.iter_mut().find(|p| p.id == active_id) {
                        let (cur, cur_hist) = {
                            match pane.term.lock() {
                                Ok(term) => {
                                    use alacritty_terminal::grid::Dimensions;
                                    (term.grid().display_offset(), term.grid().history_size())
                                }
                                Err(_) => return PostKeyAction::None,
                            }
                        };
                        let target = pane
                            .semantic_marks
                            .iter()
                            .filter(|m| {
                                matches!(
                                    m.kind,
                                    synapse_ui::pane::MarkKind::PromptStart
                                        | synapse_ui::pane::MarkKind::CommandStart
                                )
                            })
                            .filter_map(|m| {
                                let eff = cur_hist.saturating_sub(m.history_snapshot);
                                if eff > cur {
                                    Some(eff)
                                } else {
                                    None
                                }
                            })
                            .min();
                        if let Some(t) = target {
                            let delta = t as i32 - cur as i32;
                            pane.scroll_viewport(alacritty_terminal::grid::Scroll::Delta(delta));
                        }
                    }
                    return PostKeyAction::None;
                }
                Some(Action::JumpNextMark) => {
                    let active_id = tab_bar.active_tab().active_pane;
                    if let Some(pane) = panes.iter_mut().find(|p| p.id == active_id) {
                        let (cur, cur_hist) = {
                            match pane.term.lock() {
                                Ok(term) => {
                                    use alacritty_terminal::grid::Dimensions;
                                    (term.grid().display_offset(), term.grid().history_size())
                                }
                                Err(_) => return PostKeyAction::None,
                            }
                        };
                        let target = pane
                            .semantic_marks
                            .iter()
                            .filter(|m| {
                                matches!(
                                    m.kind,
                                    synapse_ui::pane::MarkKind::PromptStart
                                        | synapse_ui::pane::MarkKind::CommandStart
                                )
                            })
                            .filter_map(|m| {
                                let eff = cur_hist.saturating_sub(m.history_snapshot);
                                if eff < cur {
                                    Some(eff)
                                } else {
                                    None
                                }
                            })
                            .max();
                        if let Some(t) = target {
                            let delta = t as i32 - cur as i32;
                            pane.scroll_viewport(alacritty_terminal::grid::Scroll::Delta(delta));
                        }
                    }
                    return PostKeyAction::None;
                }
                Some(Action::EffectsToggle) => {
                    state.effects_enabled = !state.effects_enabled;
                    return PostKeyAction::EffectsToggle;
                }
                Some(Action::ToggleCopyMode) => {
                    unreachable!("ToggleCopyMode is handled by the routing gate before this match");
                }
                Some(Action::AutoSplit) => {
                    let active_id = tab_bar.active_tab().active_pane;
                    let new_pane_id = tab_bar.next_pane_id();
                    let pane_area = layout.pane_area();
                    let pane_rect = synapse_ui::PaneRect {
                        x: pane_area.0,
                        y: pane_area.1,
                        w: pane_area.2,
                        h: pane_area.3,
                    };
                    let layouts = tab_bar.active_tab().pane_tree.get_layout(pane_rect);
                    let split_dir = layouts
                        .iter()
                        .find(|(id, _)| *id == active_id)
                        .map(|(_, rect)| auto_split_direction(rect))
                        .unwrap_or(SplitDirection::Vertical);
                    if tab_bar
                        .active_tab_mut()
                        .pane_tree
                        .split(active_id, new_pane_id, split_dir)
                        .is_ok()
                    {
                        if let Some(pane) = find_pane(panes, active_id) {
                            let cwd = pane.cwd();
                            let cwd_opt = if cwd.is_empty() { None } else { Some(cwd) };
                            let shell = state.config.shell_program.as_str();
                            let args = &state.config.shell_args;
                            match create_pane_full(
                                new_pane_id,
                                pane.cols,
                                pane.rows,
                                cwd_opt,
                                Some(shell),
                                args,
                                state.config.scrollback_lines,
                            ) {
                                Ok(new_pane) => panes.push(new_pane),
                                Err(e) => {
                                    tracing::warn!("Failed to spawn PTY for auto split: {}", e)
                                }
                            }
                        }
                    }
                }
                Some(Action::ResizePaneLeft) => {
                    let active_id = tab_bar.active_tab().active_pane;
                    tab_bar.active_tab_mut().pane_tree.adjust_ratio(
                        active_id,
                        SplitDirection::Vertical,
                        -0.05,
                    );
                }
                Some(Action::ResizePaneRight) => {
                    let active_id = tab_bar.active_tab().active_pane;
                    tab_bar.active_tab_mut().pane_tree.adjust_ratio(
                        active_id,
                        SplitDirection::Vertical,
                        0.05,
                    );
                }
                Some(Action::ResizePaneUp) => {
                    let active_id = tab_bar.active_tab().active_pane;
                    tab_bar.active_tab_mut().pane_tree.adjust_ratio(
                        active_id,
                        SplitDirection::Horizontal,
                        -0.05,
                    );
                }
                Some(Action::ResizePaneDown) => {
                    let active_id = tab_bar.active_tab().active_pane;
                    tab_bar.active_tab_mut().pane_tree.adjust_ratio(
                        active_id,
                        SplitDirection::Horizontal,
                        0.05,
                    );
                }
                Some(Action::ToggleStatusBar) => {
                    state.status_bar_visible = !state.status_bar_visible;
                    return PostKeyAction::ToggleStatusBar;
                }
                Some(Action::Zoom) => {
                    toggle_zoom(state, tab_bar);
                }
                Some(Action::ToggleBroadcast) => {
                    state.broadcasting = !state.broadcasting;
                }
                Some(Action::ReloadConfig) => {
                    state.config.reload();
                    state.theme = synapse_config::Theme::load(
                        &state.config.theme,
                        synapse_config::Config::config_dir(),
                    );
                    if let Some(config_path) = synapse_config::Config::config_path() {
                        let editor = std::env::var("EDITOR")
                            .or_else(|_| std::env::var("VISUAL"))
                            .unwrap_or_else(|_| {
                                #[cfg(target_os = "macos")]
                                {
                                    "open".to_string()
                                }
                                #[cfg(not(target_os = "macos"))]
                                {
                                    "xdg-open".to_string()
                                }
                            });
                        let cmd = format!("{} {}\r", editor, config_path.display());
                        let pane = active_pane_mut(panes, tab_bar);
                        pane.write_to_pty(cmd.as_bytes());
                    }
                    return PostKeyAction::ThemeChange;
                }
                Some(Action::WorkspaceNew)
                | Some(Action::WorkspaceSwitch)
                | Some(Action::WorkspaceRename)
                | Some(Action::WorkspaceDelete)
                | Some(Action::ToggleProfiler)
                | Some(Action::ToggleRecording)
                | Some(Action::ToggleKeybinds)
                | Some(Action::ToggleSettings)
                | Some(Action::PluginExecute(_)) => {
                    return PostKeyAction::WorkspaceAction(action_opt.unwrap());
                }
                None => {
                    keybind_handled = false;
                }
            }
            if keybind_handled {
                return PostKeyAction::None;
            }
        } // end !is_repeat: keybinds handled, input encoding runs for both press+repeat

        let action = if kitty_active {
            InputAction::from_key_kitty(event, state.modifiers, kitty_flags, is_release)
        } else {
            let app_cursor = find_pane(panes, tab_bar.active_tab().active_pane)
                .map(app_cursor_active)
                .unwrap_or(false);
            InputAction::from_key(event, state.modifiers, app_cursor)
        };
        match action {
            InputAction::Write(bytes) => {
                // Ctrl+C (byte 3) with active selection → copy to clipboard instead of ^C
                if bytes.as_slice() == [3] {
                    let active_id = tab_bar.active_tab().active_pane;
                    if let Some(pane) = find_pane(panes, active_id) {
                        if let Ok(term) = pane.term.lock() {
                            if let Some(text) = term.selection_to_string() {
                                if let Some(ref mut clip) = clipboard {
                                    let _ = clip.set_text(text);
                                }
                                state.suggest.clear();
                                return PostKeyAction::None;
                            }
                        }
                    }
                }

                // Ghost text: Tab or Right → accept full suggestion (first press only).
                if !is_repeat && state.suggest.has_ghost() {
                    let bslice = bytes.as_slice();
                    if bslice == b"\t" || bslice == b"\x1b[C" || bslice == b"\x1bOC" {
                        let suffix_opt = state.suggest.ghost_suffix_owned();
                        if let Some(suffix) = suffix_opt {
                            let pane = active_pane_mut(panes, tab_bar);
                            pane.write_to_pty(suffix.as_bytes());
                        }
                        state.suggest.clear();
                        return PostKeyAction::None;
                    }
                    // Shift+Right → accept next word.
                    if bslice == b"\x1b[1;2C" {
                        let word_opt = state.suggest.next_word_owned();
                        if let Some(word) = word_opt {
                            let pane = active_pane_mut(panes, tab_bar);
                            pane.write_to_pty(word.as_bytes());
                            state.suggest.prefix.push_str(&word);
                            let new_ghost = state
                                .suggester
                                .query(&state.suggest.prefix)
                                .map(|s| s.to_string());
                            state.suggest.ghost = new_ghost;
                        }
                        return PostKeyAction::None;
                    }
                }

                write_to_panes(panes, tab_bar, state.broadcasting, &bytes);

                // Learn executed command and update suggestions (first press only —
                // repeats would corrupt the prefix trie with duplicate characters).
                if !is_repeat {
                    if bytes.as_slice() == [0x0d] || bytes.as_slice() == [0x0a] {
                        let cmd = state.suggest.prefix.trim().to_string();
                        if !cmd.is_empty() {
                            state.suggester.insert(&cmd);
                            let _ = synapse_suggest::save_suggester(&state.suggester);
                        }
                    }

                    // Update suggestion prefix and re-query trie.
                    let needs_query = state.suggest.update(&bytes);
                    if needs_query {
                        let new_ghost = state
                            .suggester
                            .query(&state.suggest.prefix)
                            .map(|s| s.to_string());
                        state.suggest.ghost = new_ghost;
                    }
                }
            }
            InputAction::ScrollUp(n) => {
                if !is_repeat {
                    let pane = active_pane_mut(panes, tab_bar);
                    pane.scroll_viewport(alacritty_terminal::grid::Scroll::Delta(n as i32));
                }
            }
            InputAction::ScrollDown(n) => {
                if !is_repeat {
                    let pane = active_pane_mut(panes, tab_bar);
                    pane.scroll_viewport(alacritty_terminal::grid::Scroll::Delta(-(n as i32)));
                }
            }
            InputAction::ScrollToTop => {
                if !is_repeat {
                    let pane = active_pane_mut(panes, tab_bar);
                    pane.scroll_viewport(alacritty_terminal::grid::Scroll::Top);
                }
            }
            InputAction::ScrollToBottom => {
                if !is_repeat {
                    let pane = active_pane_mut(panes, tab_bar);
                    pane.scroll_viewport(alacritty_terminal::grid::Scroll::Bottom);
                }
            }
            InputAction::Copy => {
                let active_id = tab_bar.active_tab().active_pane;
                if let Some(pane) = find_pane(panes, active_id) {
                    if let Ok(term) = pane.term.lock() {
                        if let Some(text) = term.selection_to_string() {
                            if let Some(ref mut clip) = clipboard {
                                let _ = clip.set_text(text);
                            }
                        }
                    }
                }
            }
            InputAction::Paste => {
                if !is_repeat {
                    if let Some(ref mut clip) = clipboard {
                        if let Ok(text) = clip.get_text() {
                            let bracketed = {
                                let pane = active_pane_mut(panes, tab_bar);
                                bracketed_paste_active(pane)
                            };
                            if bracketed {
                                write_to_panes(panes, tab_bar, state.broadcasting, b"\x1b[200~");
                                write_to_panes(
                                    panes,
                                    tab_bar,
                                    state.broadcasting,
                                    sanitize_paste(&text).as_bytes(),
                                );
                                write_to_panes(panes, tab_bar, state.broadcasting, b"\x1b[201~");
                            } else {
                                write_to_panes(panes, tab_bar, state.broadcasting, text.as_bytes());
                            }
                        }
                    }
                }
            }
            InputAction::Ignore => {}
        }
    }
    PostKeyAction::None
}

fn handle_overlay_input(logical_key: &winit::keyboard::Key, state: &mut AppState) -> PostKeyAction {
    use winit::keyboard::NamedKey;
    match logical_key {
        winit::keyboard::Key::Named(NamedKey::Escape) => {
            state.overlay.close();
        }
        winit::keyboard::Key::Named(NamedKey::ArrowUp) => {
            state.overlay.scroll_down();
        }
        winit::keyboard::Key::Named(NamedKey::ArrowDown) => {
            let vis = 15usize;
            state.overlay.scroll_up(vis);
        }
        _ => {}
    }
    PostKeyAction::None
}

#[allow(clippy::too_many_arguments)]
fn dispatch_action(
    action: Action,
    state: &mut AppState,
    tab_bar: &mut TabBar,
    panes: &mut Vec<Pane>,
    layout: &Layout,
    margin: f32,
    cell_w: f32,
    cell_h: f32,
    clipboard: &mut Option<arboard::Clipboard>,
    window: &Arc<Window>,
) -> PostKeyAction {
    match action {
        Action::NewTab => {
            let pane_area = layout.pane_area();
            let new_cols = ((pane_area.2 - margin * 2.0) / cell_w).max(1.0) as usize;
            let new_rows = ((pane_area.3 - margin * 2.0) / cell_h).max(1.0) as usize;
            let (_, pane_id) = tab_bar.new_tab();
            let shell = state.config.shell_program.as_str();
            let args = &state.config.shell_args;
            match create_pane_full(
                pane_id,
                new_cols,
                new_rows,
                None,
                Some(shell),
                args,
                state.config.scrollback_lines,
            ) {
                Ok(pane) => panes.push(pane),
                Err(e) => {
                    tracing::warn!("Failed to spawn PTY for new tab: {}", e);
                    tab_bar.close_tab(tab_bar.active);
                }
            }
            let n = tab_bar.tabs.len();
            ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
        }
        Action::CloseTab => {
            clear_zoom(state, tab_bar);
            if let Some(closed) = tab_bar.close_tab(tab_bar.active) {
                let closed_panes = closed.pane_tree.all_panes();
                panes.retain(|p| !closed_panes.contains(&p.id));
            }
            let n = tab_bar.tabs.len();
            if state.tab_scroll_offset >= n && n > 0 {
                state.tab_scroll_offset = n - 1;
            }
            ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
        }
        Action::NextTab => {
            clear_zoom(state, tab_bar);
            tab_bar.next_tab();
            let n = tab_bar.tabs.len();
            ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
        }
        Action::PrevTab => {
            clear_zoom(state, tab_bar);
            tab_bar.prev_tab();
            let n = tab_bar.tabs.len();
            ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
        }
        Action::SplitVertical => {
            let active_id = tab_bar.active_tab().active_pane;
            let new_pane_id = tab_bar.next_pane_id();
            if tab_bar
                .active_tab_mut()
                .pane_tree
                .split(active_id, new_pane_id, SplitDirection::Vertical)
                .is_ok()
            {
                if let Some(pane) = find_pane(panes, active_id) {
                    let cwd = pane.cwd();
                    let cwd_opt = if cwd.is_empty() { None } else { Some(cwd) };
                    let shell = state.config.shell_program.as_str();
                    let args = &state.config.shell_args;
                    match create_pane_full(
                        new_pane_id,
                        pane.cols,
                        pane.rows,
                        cwd_opt,
                        Some(shell),
                        args,
                        state.config.scrollback_lines,
                    ) {
                        Ok(new_pane) => panes.push(new_pane),
                        Err(e) => {
                            tracing::warn!("Failed to spawn PTY for vertical split: {}", e)
                        }
                    }
                }
            }
        }
        Action::SplitHorizontal => {
            let active_id = tab_bar.active_tab().active_pane;
            let new_pane_id = tab_bar.next_pane_id();
            if tab_bar
                .active_tab_mut()
                .pane_tree
                .split(active_id, new_pane_id, SplitDirection::Horizontal)
                .is_ok()
            {
                if let Some(pane) = find_pane(panes, active_id) {
                    let cwd = pane.cwd();
                    let cwd_opt = if cwd.is_empty() { None } else { Some(cwd) };
                    let shell = state.config.shell_program.as_str();
                    let args = &state.config.shell_args;
                    match create_pane_full(
                        new_pane_id,
                        pane.cols,
                        pane.rows,
                        cwd_opt,
                        Some(shell),
                        args,
                        state.config.scrollback_lines,
                    ) {
                        Ok(new_pane) => panes.push(new_pane),
                        Err(e) => {
                            tracing::warn!("Failed to spawn PTY for horizontal split: {}", e)
                        }
                    }
                }
            }
        }
        Action::AutoSplit => {
            let active_id = tab_bar.active_tab().active_pane;
            let new_pane_id = tab_bar.next_pane_id();
            let pane_area = layout.pane_area();
            let pane_rect = synapse_ui::PaneRect {
                x: pane_area.0,
                y: pane_area.1,
                w: pane_area.2,
                h: pane_area.3,
            };
            let layouts = tab_bar.active_tab().pane_tree.get_layout(pane_rect);
            if let Some(rect) = layouts
                .iter()
                .find(|(id, _)| *id == active_id)
                .map(|(_, r)| *r)
            {
                let dir = auto_split_direction(&rect);
                if tab_bar
                    .active_tab_mut()
                    .pane_tree
                    .split(active_id, new_pane_id, dir)
                    .is_ok()
                {
                    if let Some(pane) = find_pane(panes, active_id) {
                        let cwd = pane.cwd();
                        let cwd_opt = if cwd.is_empty() { None } else { Some(cwd) };
                        let shell = state.config.shell_program.as_str();
                        let args = &state.config.shell_args;
                        match create_pane_full(
                            new_pane_id,
                            pane.cols,
                            pane.rows,
                            cwd_opt,
                            Some(shell),
                            args,
                            state.config.scrollback_lines,
                        ) {
                            Ok(new_pane) => panes.push(new_pane),
                            Err(e) => tracing::warn!("Failed to spawn PTY for auto split: {}", e),
                        }
                    }
                }
            }
        }
        Action::ClosePane => {
            let pane_count = tab_bar.active_tab().pane_tree.all_panes().len();
            if pane_count > 1 {
                let active_id = tab_bar.active_tab().active_pane;
                if let Some(removed) = tab_bar.active_tab_mut().pane_tree.close(active_id) {
                    panes.retain(|p| p.id != removed);
                    let remaining = tab_bar.active_tab().pane_tree.all_panes();
                    if !remaining.is_empty() {
                        tab_bar.active_tab_mut().active_pane = remaining[0];
                    }
                }
            }
        }
        Action::Zoom => {
            toggle_zoom(state, tab_bar);
        }
        Action::ToggleBroadcast => {
            state.broadcasting = !state.broadcasting;
        }
        Action::NavigateUp
        | Action::NavigateDown
        | Action::NavigateLeft
        | Action::NavigateRight => {
            let dir = match action {
                Action::NavigateUp => "up",
                Action::NavigateDown => "down",
                Action::NavigateLeft => "left",
                Action::NavigateRight => "right",
                _ => unreachable!(),
            };
            let pane_area = layout.pane_area();
            let pane_rect = synapse_ui::PaneRect {
                x: pane_area.0,
                y: pane_area.1,
                w: pane_area.2,
                h: pane_area.3,
            };
            let layouts = tab_bar.active_tab().pane_tree.get_layout(pane_rect);
            let active_id = tab_bar.active_tab().active_pane;
            if let Some(next) = adjacent_pane(&layouts, active_id, dir) {
                tab_bar.active_tab_mut().active_pane = next;
                state.pane_label_until =
                    Some(std::time::Instant::now() + std::time::Duration::from_millis(600));
                state.pane_label_id = next.0 as u32;
            }
        }
        Action::Search => {
            state.search.toggle();
        }
        Action::HistorySearch => {
            if state.history_search.active {
                state.history_search.next_match();
            } else {
                let active_id = tab_bar.active_tab().active_pane;
                if let Some(pane) = find_pane(panes, active_id) {
                    if let Ok(term) = pane.term.lock() {
                        let grid = term.grid();
                        let history_size = grid.history_size();
                        let screen_lines = grid.screen_lines();
                        let mut lines_buf: Vec<Vec<char>> = Vec::new();
                        for line_idx in (-(history_size as i32))..(screen_lines as i32) {
                            let row = &grid[alacritty_terminal::index::Line(line_idx)];
                            let line: Vec<char> = row.into_iter().map(|cell| cell.c).collect();
                            lines_buf.push(line);
                        }
                        state.history_search.activate();
                        state.history_search.build_history(&lines_buf);
                        if state.config.persistent_history {
                            state
                                .history_search
                                .add_persistent_commands(state.command_history.commands());
                        }
                    }
                }
            }
        }
        Action::ClearScreen => {
            let pane = active_pane_mut(panes, tab_bar);
            pane.write_to_pty(b"\x0c");
        }
        Action::Copy => {
            let active_id = tab_bar.active_tab().active_pane;
            if let Some(pane) = find_pane(panes, active_id) {
                if let Ok(term) = pane.term.lock() {
                    if let Some(text) = term.selection_to_string() {
                        if let Some(ref mut clip) = clipboard {
                            let _ = clip.set_text(text);
                        }
                    }
                }
            }
        }
        Action::Paste => {
            if let Some(ref mut clip) = clipboard {
                if let Ok(text) = clip.get_text() {
                    let pane = active_pane_mut(panes, tab_bar);
                    let bracketed = bracketed_paste_active(pane);
                    if bracketed {
                        pane.write_to_pty(b"\x1b[200~");
                        pane.write_to_pty(sanitize_paste(&text).as_bytes());
                        pane.write_to_pty(b"\x1b[201~");
                    } else {
                        pane.write_to_pty(text.as_bytes());
                    }
                }
            }
        }
        Action::FontIncrease => {
            return PostKeyAction::FontChange((state.font_size + 1.0).min(32.0));
        }
        Action::FontDecrease => {
            return PostKeyAction::FontChange((state.font_size - 1.0).max(6.0));
        }
        Action::FontReset => {
            return PostKeyAction::FontChange(state.config.font_size);
        }
        Action::Fullscreen => {
            state.fullscreen = !state.fullscreen;
            if state.fullscreen {
                window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
            } else {
                window.set_fullscreen(None);
            }
        }
        Action::EffectsToggle => {
            state.effects_enabled = !state.effects_enabled;
            return PostKeyAction::EffectsToggle;
        }
        Action::ToggleStatusBar => {
            state.status_bar_visible = !state.status_bar_visible;
            return PostKeyAction::ToggleStatusBar;
        }
        Action::ToggleCopyMode => {
            let pane = active_pane_mut(panes, tab_bar);
            if state.in_copy_mode {
                exit_copy_mode(pane, state);
            } else {
                enter_copy_mode(pane, state);
            }
        }
        Action::ResizePaneLeft => {
            let active_id = tab_bar.active_tab().active_pane;
            tab_bar.active_tab_mut().pane_tree.adjust_ratio(
                active_id,
                SplitDirection::Vertical,
                -0.05,
            );
        }
        Action::ResizePaneRight => {
            let active_id = tab_bar.active_tab().active_pane;
            tab_bar.active_tab_mut().pane_tree.adjust_ratio(
                active_id,
                SplitDirection::Vertical,
                0.05,
            );
        }
        Action::ResizePaneUp => {
            let active_id = tab_bar.active_tab().active_pane;
            tab_bar.active_tab_mut().pane_tree.adjust_ratio(
                active_id,
                SplitDirection::Horizontal,
                -0.05,
            );
        }
        Action::ResizePaneDown => {
            let active_id = tab_bar.active_tab().active_pane;
            tab_bar.active_tab_mut().pane_tree.adjust_ratio(
                active_id,
                SplitDirection::Horizontal,
                0.05,
            );
        }
        Action::JumpPrevMark => {
            let active_id = tab_bar.active_tab().active_pane;
            if let Some(pane) = panes.iter_mut().find(|p| p.id == active_id) {
                let (cur, cur_hist) = {
                    match pane.term.lock() {
                        Ok(term) => {
                            use alacritty_terminal::grid::Dimensions;
                            (term.grid().display_offset(), term.grid().history_size())
                        }
                        Err(_) => return PostKeyAction::None,
                    }
                };
                let target = pane
                    .semantic_marks
                    .iter()
                    .filter(|m| {
                        matches!(
                            m.kind,
                            synapse_ui::pane::MarkKind::PromptStart
                                | synapse_ui::pane::MarkKind::CommandStart
                        )
                    })
                    .filter_map(|m| {
                        let eff = cur_hist.saturating_sub(m.history_snapshot);
                        if eff > cur {
                            Some(eff)
                        } else {
                            None
                        }
                    })
                    .min();
                if let Some(t) = target {
                    let delta = t as i32 - cur as i32;
                    pane.scroll_viewport(alacritty_terminal::grid::Scroll::Delta(delta));
                }
            }
        }
        Action::JumpNextMark => {
            let active_id = tab_bar.active_tab().active_pane;
            if let Some(pane) = panes.iter_mut().find(|p| p.id == active_id) {
                let (cur, cur_hist) = {
                    match pane.term.lock() {
                        Ok(term) => {
                            use alacritty_terminal::grid::Dimensions;
                            (term.grid().display_offset(), term.grid().history_size())
                        }
                        Err(_) => return PostKeyAction::None,
                    }
                };
                let target = pane
                    .semantic_marks
                    .iter()
                    .filter(|m| {
                        matches!(
                            m.kind,
                            synapse_ui::pane::MarkKind::PromptStart
                                | synapse_ui::pane::MarkKind::CommandStart
                        )
                    })
                    .filter_map(|m| {
                        let eff = cur_hist.saturating_sub(m.history_snapshot);
                        if eff < cur {
                            Some(eff)
                        } else {
                            None
                        }
                    })
                    .max();
                if let Some(t) = target {
                    let delta = t as i32 - cur as i32;
                    pane.scroll_viewport(alacritty_terminal::grid::Scroll::Delta(delta));
                }
            }
        }
        Action::ReloadConfig => {
            let new_config = synapse_config::Config::load();
            state.config = new_config;
            return PostKeyAction::ThemeChange;
        }
        Action::PaletteOpen => {
            state.palette.toggle(tab_bar);
        }
        _ => {}
    }
    PostKeyAction::None
}

use crate::app::AppCore;

impl AppCore {
    pub(crate) fn handle_keyboard(&mut self, event: winit::event::KeyEvent) {
        use winit::keyboard::NamedKey;

        // When keybinds overlay is open, intercept keys for scroll/close.
        if self.state.keybinds_open
            && event.state == winit::event::ElementState::Pressed
        {
            match &event.logical_key {
                winit::keyboard::Key::Named(NamedKey::Escape)
                | winit::keyboard::Key::Named(NamedKey::F1) => {
                    self.state.keybinds_open = false;
                    self.state.keybinds_scroll = 0;
                    return;
                }
                winit::keyboard::Key::Named(NamedKey::ArrowUp) => {
                    self.state.keybinds_scroll =
                        self.state.keybinds_scroll.saturating_sub(1);
                    return;
                }
                winit::keyboard::Key::Named(NamedKey::ArrowDown) => {
                    self.state.keybinds_scroll += 1;
                    return;
                }
                winit::keyboard::Key::Named(NamedKey::PageUp) => {
                    self.state.keybinds_scroll =
                        self.state.keybinds_scroll.saturating_sub(10);
                    return;
                }
                winit::keyboard::Key::Named(NamedKey::PageDown) => {
                    self.state.keybinds_scroll += 10;
                    return;
                }
                _ => return,
            }
        }

        // Settings overlay intercept — must come before workspace action dispatch.
        if self.state.settings_open && event.state == winit::event::ElementState::Pressed {
            use winit::keyboard::NamedKey;
            const ITEM_COUNT: usize = 10;
            match &event.logical_key {
                winit::keyboard::Key::Named(NamedKey::Escape)
                | winit::keyboard::Key::Named(NamedKey::F2) => {
                    if let Some(orig) = self.state.settings_original_config.take() {
                        self.state.config = orig;
                        self.state.theme = synapse_config::Theme::load(
                            &self.state.config.theme,
                            synapse_config::Config::config_dir(),
                        );
                        self.state.effects_enabled = self.state.config.effects.enabled;
                    }
                    self.state.settings_open = false;
                    return;
                }
                winit::keyboard::Key::Named(NamedKey::ArrowUp) => {
                    self.state.settings_item = self.state.settings_item.saturating_sub(1);
                    return;
                }
                winit::keyboard::Key::Named(NamedKey::ArrowDown) => {
                    if self.state.settings_item + 1 < ITEM_COUNT {
                        self.state.settings_item += 1;
                    }
                    return;
                }
                winit::keyboard::Key::Named(NamedKey::ArrowLeft) => {
                    self.settings_change(-1);
                    return;
                }
                winit::keyboard::Key::Named(NamedKey::ArrowRight) => {
                    self.settings_change(1);
                    return;
                }
                winit::keyboard::Key::Character(c)
                    if c.as_str().eq_ignore_ascii_case("s") =>
                {
                    let new_size = self.state.config.font_size;
                    let _ = self.state.config.save();
                    self.state.settings_original_config = None;
                    self.state.settings_open = false;
                    self.state.theme = synapse_config::Theme::load(
                        &self.state.config.theme,
                        synapse_config::Config::config_dir(),
                    );
                    self.renderer.set_clear_color(crate::app::adjusted_bg(
                        self.state.theme.bg,
                        self.state.config.window_opacity,
                    ));
                    self.renderer
                        .set_effects_config(self.state.config.effects.clone());
                    self.state.effects_enabled = self.state.config.effects.enabled;
                    self.state.status_bar_visible = self.state.config.status_bar;
                    self.layout.status_bar_visible = self.state.status_bar_visible;
                    let win_size = self.window.inner_size();
                    self.handle_resize(win_size);
                    self.change_font_size(new_size);
                    return;
                }
                _ => return,
            }
        }

        // Handle workspace/profiler actions before main keyboard handler
        // (these need access to AppCore.workspaces).
        if event.state == winit::event::ElementState::Pressed && !event.repeat {
            let logical_key = &event.logical_key;
            if let Some(ws_action) = self
                .state
                .keybinds
                .lookup(logical_key, self.state.modifiers)
            {
                match ws_action {
                    synapse_config::keybinds::Action::WorkspaceNew
                    | synapse_config::keybinds::Action::WorkspaceSwitch
                    | synapse_config::keybinds::Action::WorkspaceDelete
                    | synapse_config::keybinds::Action::ToggleProfiler
                    | synapse_config::keybinds::Action::ToggleKeybinds
                    | synapse_config::keybinds::Action::ToggleSettings
                    | synapse_config::keybinds::Action::PluginExecute(_) => {
                        self.handle_workspace_action(ws_action);
                        return;
                    }
                    synapse_config::keybinds::Action::WorkspaceRename => {
                        if self.state.palette.active {
                            self.state.palette.active = false;
                        } else {
                            self.state.palette.toggle(self.workspaces.active_tab_bar());
                        }
                        return;
                    }
                    _ => {}
                }
            }
        }

        let (tab_bar, panes, _) = self.workspaces.active_split_mut();
        let action = handle_keyboard(
            &event,
            &mut self.state,
            tab_bar,
            panes,
            &self.layout,
            self.margin,
            self.cell_w,
            self.cell_h,
            &mut self.clipboard,
            &self.window,
        );
        match action {
            PostKeyAction::FontChange(size) => self.change_font_size(size),
            PostKeyAction::ThemeChange => {
                self.renderer.set_clear_color(crate::app::adjusted_bg(
                    self.state.theme.bg,
                    self.state.config.window_opacity,
                ));
                self.renderer
                    .set_effects_config(self.state.config.effects.clone());
                self.change_font_size(self.state.config.font_size);
            }
            PostKeyAction::EffectsToggle => {
                self.renderer
                    .set_effects_enabled(self.state.effects_enabled);
            }
            PostKeyAction::ToggleStatusBar => {
                self.layout.status_bar_visible = self.state.status_bar_visible;
                let size = self.window.inner_size();
                self.handle_resize(size);
            }
            PostKeyAction::None => {}
            PostKeyAction::WorkspaceAction(wa) => {
                if let Action::PluginExecute(n) = wa {
                    self.execute_plugin(n);
                } else {
                    self.handle_workspace_action(wa);
                }
            }
        }
        crate::pane_ops::apply_tab_freeze(
            self.workspaces.active_panes(),
            self.workspaces.active_tab_bar(),
            self.state.config.freeze_background_tabs,
        );
    }
}

impl AppCore {
    fn handle_workspace_action(&mut self, action: synapse_config::keybinds::Action) {
        match action {
            synapse_config::keybinds::Action::WorkspaceNew => {
                let name = format!("workspace-{}", self.workspaces.workspaces.len() + 1);
                let pane_area = self.layout.pane_area();
                let new_cols = ((pane_area.2 - self.margin * 2.0) / self.cell_w).max(1.0) as usize;
                let new_rows = ((pane_area.3 - self.margin * 2.0) / self.cell_h).max(1.0) as usize;
                if let Err(e) = self.workspaces.create(
                    &name,
                    new_cols,
                    new_rows,
                    &self.state.config.shell_program,
                    &self.state.config.shell_args,
                    self.state.config.scrollback_lines,
                ) {
                    tracing::warn!("Failed to create workspace: {e}");
                } else {
                    self.cached_cell_data.clear();
                    self.cached_ui_rects.clear();
                    self.cached_bg_rects.clear();
                    self.cached_underline_instances.clear();
                    self.workspaces.active_cell_caches_mut().clear();
                }
            }
            synapse_config::keybinds::Action::WorkspaceSwitch => {
                let names_clone: Vec<String> = self
                    .workspaces
                    .workspace_names()
                    .iter()
                    .map(|n| n.to_string())
                    .collect();
                if names_clone.len() > 1 {
                    let pos = names_clone
                        .iter()
                        .position(|n| *n == self.workspaces.active)
                        .unwrap_or(0);
                    let next = &names_clone[(pos + 1) % names_clone.len()];
                    self.workspaces.switch(next);
                    self.state.active_workspace = next.clone();
                    self.cached_cell_data.clear();
                    self.cached_ui_rects.clear();
                    self.cached_bg_rects.clear();
                    self.cached_underline_instances.clear();
                    self.workspaces.active_cell_caches_mut().clear();
                    self.cached_active_tab = 0;
                    self.state.tab_scroll_offset = 0;
                }
            }
            synapse_config::keybinds::Action::WorkspaceDelete => {
                if self.workspaces.workspaces.len() <= 1 {
                    return;
                }
                let name = self.workspaces.active.clone();
                self.workspaces.delete(&name);
                self.state.active_workspace = self.workspaces.active.clone();
                self.cached_cell_data.clear();
                self.cached_ui_rects.clear();
                self.cached_bg_rects.clear();
                self.cached_underline_instances.clear();
                self.workspaces.active_cell_caches_mut().clear();
                self.cached_active_tab = 0;
                self.state.tab_scroll_offset = 0;
            }
            synapse_config::keybinds::Action::ToggleProfiler => {
                self.state.profiler_active = !self.state.profiler_active;
            }
            synapse_config::keybinds::Action::ToggleKeybinds => {
                self.state.keybinds_open = !self.state.keybinds_open;
                self.state.keybinds_scroll = 0;
            }
            synapse_config::keybinds::Action::ToggleSettings => {
                if self.state.settings_open {
                    if let Some(orig) = self.state.settings_original_config.take() {
                        self.state.config = orig;
                        self.state.theme = synapse_config::Theme::load(
                            &self.state.config.theme,
                            synapse_config::Config::config_dir(),
                        );
                        self.state.effects_enabled = self.state.config.effects.enabled;
                    }
                    self.state.settings_open = false;
                } else {
                    self.state.settings_original_config = Some(self.state.config.clone());
                    self.state.settings_item = 0;
                    self.state.settings_open = true;
                }
            }
            synapse_config::keybinds::Action::ToggleRecording => {
                if let Some(shared) = crate::record::RECORDING.get() {
                    if shared.is_recording() {
                        self.stop_recording_if_active();
                    } else {
                        shared.start();
                        self.state.recording = true;
                    }
                }
            }
            synapse_config::keybinds::Action::PluginExecute(n) => {
                self.execute_plugin(n);
            }
            _ => {}
        }
    }

    fn settings_change(&mut self, dir: i32) {
        use synapse_config::config::CursorStyle;
        const THEMES: &[&str] = &["synapse_", "dracula", "catppuccin-mocha", "tokyo-night"];
        match self.state.settings_item {
            0 => {
                let new = (self.state.config.font_size + dir as f32).clamp(6.0, 32.0);
                self.state.config.font_size = new;
            }
            1 => self.state.config.font_ligatures = !self.state.config.font_ligatures,
            2 => {
                let cur = THEMES
                    .iter()
                    .position(|&t| t == self.state.config.theme.as_str())
                    .unwrap_or(0);
                let next = (cur as i32 + dir).rem_euclid(THEMES.len() as i32) as usize;
                self.state.config.theme = THEMES[next].to_string();
                self.state.theme = synapse_config::Theme::load(
                    &self.state.config.theme,
                    synapse_config::Config::config_dir(),
                );
                self.renderer.set_clear_color(crate::app::adjusted_bg(
                    self.state.theme.bg,
                    self.state.config.window_opacity,
                ));
            }
            3 => {
                self.state.config.cursor_style = match &self.state.config.cursor_style {
                    CursorStyle::Block => {
                        if dir > 0 { CursorStyle::Beam } else { CursorStyle::NeonUnderbar }
                    }
                    CursorStyle::Beam => {
                        if dir > 0 { CursorStyle::Underline } else { CursorStyle::Block }
                    }
                    CursorStyle::Underline => {
                        if dir > 0 { CursorStyle::NeonUnderbar } else { CursorStyle::Beam }
                    }
                    CursorStyle::NeonUnderbar => {
                        if dir > 0 { CursorStyle::Block } else { CursorStyle::Underline }
                    }
                };
            }
            4 => self.state.config.cursor_blink = !self.state.config.cursor_blink,
            5 => self.state.config.status_bar = !self.state.config.status_bar,
            6 => self.state.config.scrollbar = !self.state.config.scrollbar,
            7 => self.state.config.show_pane_labels = !self.state.config.show_pane_labels,
            8 => self.state.config.pane_badge = !self.state.config.pane_badge,
            9 => {
                self.state.config.effects.enabled = !self.state.config.effects.enabled;
                self.state.effects_enabled = self.state.config.effects.enabled;
                self.renderer
                    .set_effects_enabled(self.state.effects_enabled);
            }
            _ => {}
        }
    }

    fn execute_plugin(&mut self, index: usize) {
        let plugin = match self.state.config.plugins.get(index) {
            Some(p) => p.clone(),
            None => return,
        };

        let (pane_cwd, selected_text, clipboard_text) = {
            let ws = self.workspaces.active_ws();
            let active_id = ws.tab_bar.active_tab().active_pane;
            let cwd = ws
                .panes
                .iter()
                .find(|p| p.id == active_id)
                .map(|p| p.cwd())
                .unwrap_or_default();

            let sel = ws
                .panes
                .iter()
                .find(|p| p.id == active_id)
                .and_then(|pane| pane.term.lock().ok())
                .and_then(|term| term.selection_to_string())
                .unwrap_or_default();

            let clip = self
                .clipboard
                .as_mut()
                .and_then(|cb| cb.get_text().ok())
                .unwrap_or_default();

            (cwd, sel, clip)
        };

        let resolved_cwd =
            crate::overlay::resolve_plugin_cwd(&plugin, &pane_cwd, &selected_text, &clipboard_text);
        let resolved_cmd = crate::overlay::resolve_plugin_command(
            &plugin,
            &pane_cwd,
            &selected_text,
            &clipboard_text,
        );

        match plugin.split.as_str() {
            "horizontal" | "vertical" | "tab" => {
                self.execute_plugin_split(&plugin, resolved_cmd, resolved_cwd, &plugin.split);
            }
            "overlay" => {
                let title = plugin.name.clone();
                self.state.overlay.active = true;
                self.state.overlay.title = title.clone();
                self.state.overlay.lines.clear();
                self.state.overlay.scroll = 0;
                self.state.overlay.status = crate::overlay::OverlayStatus::Running;

                let (tx, rx) = std::sync::mpsc::channel();
                crate::overlay::spawn_overlay_command(tx, title, resolved_cmd);
                self.overlay_rx = Some(rx);
            }
            _ => {
                self.execute_plugin_split(&plugin, resolved_cmd, resolved_cwd, &plugin.split);
            }
        }

        if plugin.replace_selection {
            match crate::overlay::run_replace_selection(
                &plugin,
                &pane_cwd,
                &selected_text,
                &clipboard_text,
            ) {
                Ok(output) => {
                    let ws = self.workspaces.active_ws_mut();
                    let active_id = ws.tab_bar.active_tab().active_pane;
                    if let Some(pane) = ws.panes.iter().find(|p| p.id == active_id) {
                        if let Ok(mut w) = pane.pty_writer.lock() {
                            let _ = std::io::Write::write_all(&mut *w, output.as_bytes());
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("replace_selection command failed: {}", e);
                }
            }
        }
    }

    fn execute_plugin_split(
        &mut self,
        _plugin: &synapse_config::PluginCommand,
        command: String,
        cwd: Option<String>,
        split: &str,
    ) {
        let ws = self.workspaces.active_ws_mut();
        let tab_bar = &mut ws.tab_bar;
        let panes = &mut ws.panes;

        let active_id = tab_bar.active_tab().active_pane;
        let pane_geometry = panes.iter().find(|p| p.id == active_id);

        let (cols, rows) = match pane_geometry {
            Some(p) => (p.cols, p.rows),
            None => {
                let pane_area = self.layout.pane_area();
                let cols = ((pane_area.2 - self.margin * 2.0) / self.cell_w).max(1.0) as usize;
                let rows = ((pane_area.3 - self.margin * 2.0) / self.cell_h).max(1.0) as usize;
                (cols, rows)
            }
        };

        let shell_args: Vec<String> = vec!["-c".to_string(), command];

        if split == "tab" {
            let (_, new_pane_id) = tab_bar.new_tab();
            match crate::pane_ops::create_pane_full(
                new_pane_id,
                cols,
                rows,
                cwd,
                Some("/bin/sh"),
                &shell_args,
                self.state.config.scrollback_lines,
            ) {
                Ok(pane) => panes.push(pane),
                Err(e) => tracing::warn!("Plugin tab spawn failed: {}", e),
            }
        } else {
            let direction = match split {
                "horizontal" => synapse_ui::SplitDirection::Horizontal,
                _ => synapse_ui::SplitDirection::Vertical,
            };

            let new_pane_id = tab_bar.next_pane_id();
            if tab_bar
                .active_tab_mut()
                .pane_tree
                .split(active_id, new_pane_id, direction)
                .is_ok()
            {
                match crate::pane_ops::create_pane_full(
                    new_pane_id,
                    cols,
                    rows,
                    cwd,
                    Some("/bin/sh"),
                    &shell_args,
                    self.state.config.scrollback_lines,
                ) {
                    Ok(pane) => panes.push(pane),
                    Err(e) => tracing::warn!("Plugin split spawn failed: {}", e),
                }
            }
        }

        self.cached_cell_data.clear();
        self.cached_ui_rects.clear();
        self.cached_bg_rects.clear();
        self.cached_underline_instances.clear();
        self.workspaces.active_cell_caches_mut().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bracketed_paste_newline_sanitize() {
        let text = "line1\r\nline2\nline3";
        let sanitized = sanitize_paste(text);
        assert_eq!(sanitized, "line1\rline2\rline3");
    }

    #[test]
    fn test_compute_moved_cursor_basic() {
        use alacritty_terminal::index::{Column, Line, Point};
        let start = Point::new(Line(5), Column(10));
        let moved = compute_moved_cursor(start, 1, 0, 80, 24, 100);
        assert_eq!(moved.column.0, 11);
        assert_eq!(moved.line.0, 5);

        let moved_down = compute_moved_cursor(start, 0, 1, 80, 24, 100);
        assert_eq!(moved_down.line.0, 6);
        assert_eq!(moved_down.column.0, 10);
    }

    #[test]
    fn test_compute_moved_cursor_clamp() {
        use alacritty_terminal::index::{Column, Line, Point};
        // Right edge
        let right = Point::new(Line(0), Column(79));
        assert_eq!(compute_moved_cursor(right, 1, 0, 80, 24, 100).column.0, 79);
        // Left edge
        let left = Point::new(Line(0), Column(0));
        assert_eq!(compute_moved_cursor(left, -1, 0, 80, 24, 100).column.0, 0);
        // Bottom edge
        let bottom = Point::new(Line(23), Column(0));
        assert_eq!(compute_moved_cursor(bottom, 0, 1, 80, 24, 100).line.0, 23);
        // History top
        let hist_top = Point::new(Line(-100), Column(0));
        assert_eq!(
            compute_moved_cursor(hist_top, 0, -1, 80, 24, 100).line.0,
            -100
        );
    }

    #[test]
    fn test_compute_scroll_delta_above() {
        // cursor at viewport_row = -3 → scroll up 3
        assert_eq!(compute_scroll_delta(-3, 24), 3);
    }

    #[test]
    fn test_compute_scroll_delta_below() {
        // cursor at viewport_row = 25, screen_lines = 24 → scroll down 2
        assert_eq!(compute_scroll_delta(25, 24), -2);
    }

    #[test]
    fn test_compute_scroll_delta_visible() {
        assert_eq!(compute_scroll_delta(12, 24), 0);
    }

    #[test]
    fn test_copy_sel_mode_char_not_none() {
        assert_ne!(CopySelMode::Char, CopySelMode::None);
    }

    #[test]
    fn test_copy_sel_mode_line_not_none() {
        assert_ne!(CopySelMode::Line, CopySelMode::None);
    }

    #[test]
    fn test_is_word_char() {
        assert!(is_word_char('a'));
        assert!(is_word_char('Z'));
        assert!(is_word_char('5'));
        assert!(is_word_char('_'));
        assert!(!is_word_char(' '));
        assert!(!is_word_char('-'));
        assert!(!is_word_char('.'));
        assert!(!is_word_char('\0'));
    }
}
