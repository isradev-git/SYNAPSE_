use std::sync::Arc;

use winit::{event::KeyEvent, window::Window};

use alacritty_terminal::term::TermMode;
use luna_config::Action;
use luna_ui::{layout::Layout, pane::Pane, splitter::SplitDirection, tab_bar::TabBar};

use crate::{
    input::InputAction,
    pane_ops::{active_pane_mut, adjacent_pane, create_pane_full, find_pane},
    search::{handle_history_search_input, handle_search_input, update_search_matches},
    state::AppState,
};

pub enum PostKeyAction {
    None,
    FontChange(f32),
    ThemeChange,
}

fn ensure_tab_visible(
    active_idx: usize,
    tab_count: usize,
    layout: &luna_ui::layout::Layout,
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

/// Phase 1 stub: selection extraction will be reimplemented against the
/// alacritty_terminal grid in Phase 2.
#[allow(dead_code)]
pub(crate) fn extract_selection(
    _pane: &Pane,
    _sel: &crate::state::Selection,
    _cols: usize,
) -> String {
    String::new()
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
    // Kitty keyboard protocol is Phase 2 — these stubs keep the legacy path active.
    let kitty_flags: u8 = 0;
    let kitty_active = false;

    // Ignore key releases entirely for the legacy path.
    if event.state == winit::event::ElementState::Released {
        return PostKeyAction::None;
    }

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
        let action_opt = state.keybinds.lookup(logical_key, state.modifiers);
        let mut keybind_handled = true;
        match action_opt {
            Some(Action::Search) => {
                state.search.toggle();
                if state.search.active {
                    update_search_matches(state, tab_bar, panes);
                }
            }
            Some(Action::HistorySearch) => {
                // Phase 2: rebuild history from alacritty grid scrollback.
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
                match create_pane_full(pane_id, new_cols, new_rows, None, Some(shell), args) {
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
                tab_bar.next_tab();
                let n = tab_bar.tabs.len();
                ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
            }
            Some(Action::PrevTab) => {
                tab_bar.prev_tab();
                let n = tab_bar.tabs.len();
                ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
            }
            Some(Action::TabSwitch1) => {
                tab_bar.activate(0);
                let n = tab_bar.tabs.len();
                ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
            }
            Some(Action::TabSwitch2) => {
                tab_bar.activate(1);
                let n = tab_bar.tabs.len();
                ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
            }
            Some(Action::TabSwitch3) => {
                tab_bar.activate(2);
                let n = tab_bar.tabs.len();
                ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
            }
            Some(Action::TabSwitch4) => {
                tab_bar.activate(3);
                let n = tab_bar.tabs.len();
                ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
            }
            Some(Action::TabSwitch5) => {
                tab_bar.activate(4);
                let n = tab_bar.tabs.len();
                ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
            }
            Some(Action::TabSwitch6) => {
                tab_bar.activate(5);
                let n = tab_bar.tabs.len();
                ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
            }
            Some(Action::TabSwitch7) => {
                tab_bar.activate(6);
                let n = tab_bar.tabs.len();
                ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
            }
            Some(Action::TabSwitch8) => {
                tab_bar.activate(7);
                let n = tab_bar.tabs.len();
                ensure_tab_visible(tab_bar.active, n, layout, &mut state.tab_scroll_offset);
            }
            Some(Action::TabSwitch9) => {
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
                        ) {
                            Ok(new_pane) => panes.push(new_pane),
                            Err(e) => {
                                tracing::warn!("Failed to spawn PTY for horizontal split: {}", e)
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
                // Phase 1: copy needs grid-aware selection extraction (Phase 2).
            }
            Some(Action::Paste) => {
                if let Some(ref mut clip) = clipboard {
                    if let Ok(text) = clip.get_text() {
                        let pane = active_pane_mut(panes, tab_bar);
                        let bracketed = bracketed_paste_active(pane);
                        if bracketed {
                            pane.write_to_pty(b"\x1b[200~");
                        }
                        pane.write_to_pty(text.as_bytes());
                        if bracketed {
                            pane.write_to_pty(b"\x1b[201~");
                        }
                    }
                }
            }
            Some(Action::ReloadConfig) => {
                state.config.reload();
                state.theme = luna_config::Theme::load(
                    &state.config.theme,
                    luna_config::Config::config_dir(),
                );
                if let Some(config_path) = luna_config::Config::config_path() {
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
            None => {
                keybind_handled = false;
            }
        }
        if keybind_handled {
            return PostKeyAction::None;
        }

        let _ = (kitty_flags, kitty_active);

        let app_cursor = find_pane(panes, tab_bar.active_tab().active_pane)
            .map(app_cursor_active)
            .unwrap_or(false);
        let action = InputAction::from_key(event, state.modifiers, app_cursor);
        match action {
            InputAction::Write(bytes) => {
                let pane = active_pane_mut(panes, tab_bar);
                pane.write_to_pty(&bytes);
            }
            InputAction::ScrollUp(_)
            | InputAction::ScrollDown(_)
            | InputAction::ScrollToTop
            | InputAction::ScrollToBottom => {
                // Phase 2: scrollback scrolling.
            }
            InputAction::Copy => {
                // Phase 2: selection extraction.
            }
            InputAction::Paste => {
                if let Some(ref mut clip) = clipboard {
                    if let Ok(text) = clip.get_text() {
                        let pane = active_pane_mut(panes, tab_bar);
                        let bracketed = bracketed_paste_active(pane);
                        if bracketed {
                            pane.write_to_pty(b"\x1b[200~");
                        }
                        pane.write_to_pty(text.as_bytes());
                        if bracketed {
                            pane.write_to_pty(b"\x1b[201~");
                        }
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
        match action {
            PostKeyAction::FontChange(size) => self.change_font_size(size),
            PostKeyAction::ThemeChange => {
                self.renderer.set_clear_color(self.state.theme.bg);
                self.change_font_size(self.state.config.font_size);
            }
            PostKeyAction::None => {}
        }
    }
}
