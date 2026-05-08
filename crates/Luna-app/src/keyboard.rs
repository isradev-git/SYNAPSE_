use std::sync::Arc;

use winit::{event::KeyEvent, window::Window};

use luna_config::Action;
use luna_ui::{layout::Layout, pane::Pane, splitter::SplitDirection, tab_bar::TabBar};

use crate::{
    input::InputAction,
    pane_ops::{
        active_pane_mut, adjacent_pane,
        create_pane, create_pane_with_cwd, find_pane,
    },
    search::{handle_history_search_input, handle_search_input, update_search_matches},
    state::AppState,
};

pub enum PostKeyAction {
    None,
    FontChange(f32),
}

fn extract_selection(grid: &luna_terminal::grid::Grid, sel: &crate::state::Selection, cols: usize) -> String {
    let (start, end) = sel.normalized();
    let mut result = String::new();

    for vrow in start.1..=end.1 {
        let line_start = if vrow == start.1 { start.0 } else { 0 };
        let line_end = if vrow == end.1 { end.0.min(cols - 1) } else { cols - 1 };

        for col in line_start..=line_end {
            let cell = match grid.get_visible(col, vrow) {
                Some(c) => c,
                None => continue,
            };

            if cell.c == '\0' || cell.flags.contains(luna_terminal::grid::CellFlags::INVISIBLE) {
                result.push(' ');
            } else {
                result.push(cell.c);
            }
        }

        if vrow < end.1 {
            while result.ends_with(' ') {
                result.pop();
            }
            result.push('\n');
        }
    }

    while result.ends_with(' ') || result.ends_with('\n') {
        result.pop();
    }

    result
}

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
    if event.state == winit::event::ElementState::Pressed && !event.repeat {
        let logical_key = &event.logical_key;

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

        // Keybind lookup
        let action_opt = state.keybinds.lookup(&logical_key, state.modifiers);
        let mut keybind_handled = true;
        match action_opt {
            Some(Action::Search) => {
                state.search.toggle();
                if state.search.active {
                    update_search_matches(state, tab_bar, panes);
                }
            }
            Some(Action::HistorySearch) => {
                state.history_search.activate();
                if let Some(pane) = find_pane(&panes, tab_bar.active_tab().active_pane) {
                    let grid = pane.grid.borrow();
                    let lines = grid.all_lines();
                    state.history_search.build_history(&lines);
                    state.history_search.update_filter();
                }
            }
            Some(Action::ClearScreen) => {
                let pane = active_pane_mut(panes, tab_bar);
                let mut grid = pane.grid.borrow_mut();
                let last_row = grid.rows() - 1;
                grid.clear_region(0, last_row);
                grid.set_cursor(0, 0);
                let _ = pane.pty_session.pty.write(b"\x0c");
            }
            Some(Action::NewTab) => {
                let pane_area = layout.pane_area();
                let new_cols = ((pane_area.2 - margin * 2.0) / cell_w).max(1.0) as usize;
                let new_rows = ((pane_area.3 - margin * 2.0) / cell_h).max(1.0) as usize;
                let (_, pane_id) = tab_bar.new_tab();
                panes.push(create_pane(pane_id, new_cols, new_rows));
            }
            Some(Action::CloseTab) => {
                if let Some(closed) = tab_bar.close_tab(tab_bar.active) {
                    let closed_panes = closed.pane_tree.all_panes();
                    for pane in panes.iter_mut() {
                        if closed_panes.contains(&pane.id) {
                            let _ = pane.pty_session.pty.kill();
                        }
                    }
                    panes.retain(|p| !closed_panes.contains(&p.id));
                }
            }
            Some(Action::NextTab) => {
                tab_bar.next_tab();
            }
            Some(Action::PrevTab) => {
                tab_bar.prev_tab();
            }
            Some(Action::TabSwitch1) => tab_bar.activate(0),
            Some(Action::TabSwitch2) => tab_bar.activate(1),
            Some(Action::TabSwitch3) => tab_bar.activate(2),
            Some(Action::TabSwitch4) => tab_bar.activate(3),
            Some(Action::TabSwitch5) => tab_bar.activate(4),
            Some(Action::TabSwitch6) => tab_bar.activate(5),
            Some(Action::TabSwitch7) => tab_bar.activate(6),
            Some(Action::TabSwitch8) => tab_bar.activate(7),
            Some(Action::TabSwitch9) => tab_bar.activate(8),
            Some(Action::SplitVertical) => {
                let active_id = tab_bar.active_tab().active_pane;
                let new_pane_id = tab_bar.next_pane_id();
                if tab_bar.active_tab_mut().pane_tree.split(active_id, new_pane_id, SplitDirection::Vertical).is_ok() {
                    if let Some(pane) = find_pane(&panes, active_id) {
                        let cwd = pane.cwd();
                        let cwd_opt = if cwd.is_empty() { None } else { Some(cwd) };
                        panes.push(create_pane_with_cwd(new_pane_id, pane.cols, pane.rows, cwd_opt));
                    }
                }
            }
            Some(Action::SplitHorizontal) => {
                let active_id = tab_bar.active_tab().active_pane;
                let new_pane_id = tab_bar.next_pane_id();
                if tab_bar.active_tab_mut().pane_tree.split(active_id, new_pane_id, SplitDirection::Horizontal).is_ok() {
                    if let Some(pane) = find_pane(&panes, active_id) {
                        let cwd = pane.cwd();
                        let cwd_opt = if cwd.is_empty() { None } else { Some(cwd) };
                        panes.push(create_pane_with_cwd(new_pane_id, pane.cols, pane.rows, cwd_opt));
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
                        if let Some(pane) = panes.iter_mut().find(|p| p.id == removed) {
                            let _ = pane.pty_session.pty.kill();
                        }
                        panes.retain(|p| p.id != removed);
                        let remaining = tab_bar.active_tab().pane_tree.all_panes();
                        if !remaining.is_empty() {
                            tab_bar.active_tab_mut().active_pane = remaining[0];
                        }
                    }
                }
            }
            Some(Action::NavigateUp) | Some(Action::NavigateDown)
            | Some(Action::NavigateLeft) | Some(Action::NavigateRight) => {
                let dir = match action_opt {
                    Some(Action::NavigateUp) => "up",
                    Some(Action::NavigateDown) => "down",
                    Some(Action::NavigateLeft) => "left",
                    Some(Action::NavigateRight) => "right",
                    _ => unreachable!(),
                };
                let pane_area = layout.pane_area();
                let pane_rect = luna_ui::PaneRect {
                    x: pane_area.0,
                    y: pane_area.1,
                    w: pane_area.2,
                    h: pane_area.3,
                };
                let layouts = tab_bar.active_tab().pane_tree.get_layout(pane_rect);
                let active_id = tab_bar.active_tab().active_pane;
                if let Some(next) = adjacent_pane(&layouts, active_id, dir) {
                    tab_bar.active_tab_mut().active_pane = next;
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
                let pane = active_pane_mut(panes, tab_bar);
                let grid_ref = pane.grid.borrow();
                if let Some(ref sel) = state.selection {
                    let text = extract_selection(&grid_ref, sel, pane.cols);
                    if let Some(ref mut clip) = clipboard {
                        let _ = clip.set_text(text);
                    }
                }
            }
            Some(Action::Paste) => {
                if let Some(ref mut clip) = clipboard {
                    if let Ok(text) = clip.get_text() {
                        let pane = active_pane_mut(panes, tab_bar);
                        let _ = pane.pty_session.pty.write(b"\x1b[200~");
                        let _ = pane.pty_session.pty.write(text.as_bytes());
                        let _ = pane.pty_session.pty.write(b"\x1b[201~");
                    }
                }
            }
            Some(Action::ReloadConfig) => {
                state.config.reload();
                return PostKeyAction::FontChange(state.config.font_size);
            }
            None => {
                keybind_handled = false;
            }
        }
        if keybind_handled {
            return PostKeyAction::None;
        }

        let action = InputAction::from_key(event, state.modifiers);
        match action {
            InputAction::Write(bytes) => {
                if bytes != b"\x1b[5~" && bytes != b"\x1b[6~" {
                    active_pane_mut(panes, tab_bar)
                        .grid.borrow_mut().scroll_to_bottom();
                }
                let pane = active_pane_mut(panes, tab_bar);
                if let Err(e) = pane.pty_session.pty.write(&bytes) {
                    eprintln!("PTY write error: {}", e);
                }
            }
            InputAction::ScrollUp(lines) => {
                active_pane_mut(panes, tab_bar)
                    .grid.borrow_mut().scroll_up(lines);
            }
            InputAction::ScrollDown(lines) => {
                active_pane_mut(panes, tab_bar)
                    .grid.borrow_mut().scroll_down(lines);
            }
            InputAction::ScrollToTop => {
                active_pane_mut(panes, tab_bar)
                    .grid.borrow_mut().scroll_to_top();
            }
            InputAction::ScrollToBottom => {
                active_pane_mut(panes, tab_bar)
                    .grid.borrow_mut().scroll_to_bottom();
            }
            InputAction::Copy => {
                let pane = active_pane_mut(panes, tab_bar);
                let grid_ref = pane.grid.borrow();
                if let Some(ref sel) = state.selection {
                    let text = extract_selection(&grid_ref, sel, pane.cols);
                    if let Some(ref mut clip) = clipboard {
                        let _ = clip.set_text(text);
                    }
                }
            }
            InputAction::Paste => {
                if let Some(ref mut clip) = clipboard {
                    if let Ok(text) = clip.get_text() {
                        let pane = active_pane_mut(panes, tab_bar);
                        let _ = pane.pty_session.pty.write(b"\x1b[200~");
                        let _ = pane.pty_session.pty.write(text.as_bytes());
                        let _ = pane.pty_session.pty.write(b"\x1b[201~");
                    }
                }
            }
            InputAction::Ignore => {}
        }
    }
    PostKeyAction::None
}

use crate::app::App;

impl App {
    pub(crate) fn handle_keyboard(&mut self, event: winit::event::KeyEvent) {
        let action = handle_keyboard(
            &event,
            &mut self.state,
            &mut self.tab_bar,
            &mut self.panes,
            &self.layout,
            self.margin,
            self.cell_w,
            self.cell_h,
            &mut self.clipboard,
            &self.window,
        );
        if let PostKeyAction::FontChange(size) = action {
            self.change_font_size(size);
        }
    }
}
