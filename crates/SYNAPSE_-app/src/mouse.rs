use std::sync::Arc;

use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton, MouseScrollDelta},
    window::{CursorIcon, Window},
};

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection as TermSelection, SelectionType};
use alacritty_terminal::term::TermMode;
use synapse_ui::{layout::Layout, pane::Pane, tab_bar::TabBar, PaneRect, SplitDirection};

use crate::{
    keyboard::clear_zoom,
    pane_ops::{find_hovered_divider, handle_tab_click},
    state::{AppState, DividerDrag},
};

fn cursor_to_pane_cell(
    cursor_x: f64,
    cursor_y: f64,
    tab_bar: &TabBar,
    layout: &Layout,
    cell_w: f32,
    cell_h: f32,
    margin: f32,
) -> Option<(usize, usize)> {
    let active_id = tab_bar.active_tab().active_pane;
    let pane_area = layout.pane_area();
    let pane_rect = PaneRect {
        x: pane_area.0,
        y: pane_area.1,
        w: pane_area.2,
        h: pane_area.3,
    };
    let layouts = tab_bar.active_tab().pane_tree.get_layout(pane_rect);
    let rect = layouts.iter().find(|(id, _)| *id == active_id)?.1;

    let content_x = (rect.x + margin) as f64;
    let content_y = (rect.y + margin) as f64;

    if cursor_x < content_x || cursor_y < content_y {
        return None;
    }

    let col = ((cursor_x - content_x) / cell_w as f64).floor() as usize + 1;
    let row = ((cursor_y - content_y) / cell_h as f64).floor() as usize + 1;
    Some((col, row))
}

fn encode_mouse_event(col: usize, row: usize, btn: u8, pressed: bool, sgr: bool) -> Vec<u8> {
    if sgr {
        let m = if pressed { b'M' } else { b'm' };
        let mut bytes = format!("\x1b[<{};{};{}", btn, col, row).into_bytes();
        bytes.push(m);
        bytes
    } else {
        let b = btn + 32;
        let x = ((col as u16 + 32).min(255)) as u8;
        let y = ((row as u16 + 32).min(255)) as u8;
        vec![0x1b, b'[', b'M', b, x, y]
    }
}

/// Return `(mouse_reporting_active, sgr_encoding)` for the active pane.
fn active_pane_mouse_modes(panes: &[Pane], active_id: synapse_ui::PaneId) -> (bool, bool) {
    if let Some(pane) = panes.iter().find(|p| p.id == active_id) {
        if let Ok(term) = pane.term.lock() {
            let mode = term.mode();
            let mouse_active = mode.intersects(TermMode::MOUSE_MODE);
            let sgr = mode.contains(TermMode::SGR_MOUSE);
            return (mouse_active, sgr);
        }
    }
    (false, false)
}

/// Returns true if the click hit a scrollbar area (consumes the event).
fn scrollbar_click(
    cursor_x: f64,
    cursor_y: f64,
    tab_bar: &TabBar,
    layout: &Layout,
    margin: f32,
    panes: &mut Vec<Pane>,
) -> bool {
    let pane_area = layout.pane_area();
    let pane_rect = PaneRect {
        x: pane_area.0,
        y: pane_area.1,
        w: pane_area.2,
        h: pane_area.3,
    };
    let layouts = tab_bar.active_tab().pane_tree.get_layout(pane_rect);
    let bar_w = 6.0f32;

    for (pid, rect) in &layouts {
        let bar_x = rect.x + rect.w - bar_w - 1.0;
        let content_y = rect.y + margin;
        let content_h = rect.h - margin * 2.0;

        if cursor_x >= bar_x as f64
            && cursor_x <= (bar_x + bar_w) as f64
            && cursor_y >= content_y as f64
            && cursor_y <= (content_y + content_h) as f64
        {
            if let Some(pane) = panes.iter().find(|p| p.id == *pid) {
                if let Ok(term) = pane.term.lock() {
                    let display_offset = term.grid().display_offset();
                    let history_size = term.grid().history_size();
                    if history_size > 0 {
                        let pane_rows = pane.rows;
                        let total = (pane_rows + history_size).max(1) as f32;
                        let thumb_h = (content_h * pane_rows as f32 / total).max(12.0);
                        let travel = content_h - thumb_h;
                        let frac =
                            ((cursor_y as f32 - content_y - thumb_h / 2.0) / travel).clamp(0.0, 1.0);
                        let target_offset = ((1.0 - frac) * history_size as f32) as usize;
                        let delta = target_offset as i32 - display_offset as i32;
                        if delta != 0 {
                            drop(term);
                            pane.scroll_viewport(alacritty_terminal::grid::Scroll::Delta(delta));
                        }
                    }
                }
            }
            return true;
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
pub fn handle_scroll(
    delta: MouseScrollDelta,
    panes: &mut [Pane],
    tab_bar: &TabBar,
    state: &mut AppState,
    layout: &Layout,
    cell_w: f32,
    cell_h: f32,
    margin: f32,
) {
    let lines = match delta {
        MouseScrollDelta::LineDelta(_, y) => (y.abs() as usize).max(1),
        MouseScrollDelta::PixelDelta(pos) => (pos.y.abs() / cell_h as f64).max(1.0) as usize,
    };
    let is_up = match delta {
        MouseScrollDelta::LineDelta(_, y) => y > 0.0,
        MouseScrollDelta::PixelDelta(pos) => pos.y > 0.0,
    };

    // If cursor is over sidebar, scroll the tab list instead of the pane
    if state.cursor_x < layout.sidebar_width as f64 {
        let lines = match delta {
            MouseScrollDelta::LineDelta(_, y) => (y.abs() as usize).max(1),
            MouseScrollDelta::PixelDelta(pos) => (pos.y.abs() / cell_h as f64).max(1.0) as usize,
        };
        let is_up = match delta {
            MouseScrollDelta::LineDelta(_, y) => y > 0.0,
            MouseScrollDelta::PixelDelta(pos) => pos.y > 0.0,
        };
        let offset = &mut state.tab_scroll_offset;
        let tab_count = tab_bar.tabs.len();
        if is_up {
            *offset = offset.saturating_sub(lines);
        } else {
            *offset = (*offset + lines).min(tab_count.saturating_sub(1));
        }
        return;
    }

    let active_id = tab_bar.active_tab().active_pane;
    let (mouse_active, sgr) = active_pane_mouse_modes(panes, active_id);

    if mouse_active && state.cursor_x >= layout.sidebar_width as f64 {
        if let Some((col, row)) = cursor_to_pane_cell(
            state.cursor_x,
            state.cursor_y,
            tab_bar,
            layout,
            cell_w,
            cell_h,
            margin,
        ) {
            let btn = if is_up { 64u8 } else { 65u8 };
            for _ in 0..lines {
                let bytes = encode_mouse_event(col, row, btn, true, sgr);
                if let Some(pane) = panes.iter().find(|p| p.id == active_id) {
                    pane.write_to_pty(&bytes);
                }
            }
        }
        return;
    }

    if let Some(pane) = panes.iter().find(|p| p.id == active_id) {
        let scroll = if is_up {
            alacritty_terminal::grid::Scroll::Delta(lines as i32)
        } else {
            alacritty_terminal::grid::Scroll::Delta(-(lines as i32))
        };
        pane.scroll_viewport(scroll);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_mouse_button(
    button_state: ElementState,
    button: MouseButton,
    state: &mut AppState,
    tab_bar: &mut TabBar,
    panes: &mut Vec<Pane>,
    layout: &Layout,
    cell_w: f32,
    cell_h: f32,
    margin: f32,
    scrollback_lines: usize,
) {
    let active_id = tab_bar.active_tab().active_pane;
    let shift_held = state.modifiers.shift_key();

    // Mouse reporting: intercept clicks for apps like vim/htop when Shift not held
    {
        let (mouse_active, sgr) = active_pane_mouse_modes(panes, active_id);

        if mouse_active && !shift_held && state.cursor_x >= layout.sidebar_width as f64 {
            if let Some((col, row)) = cursor_to_pane_cell(
                state.cursor_x,
                state.cursor_y,
                tab_bar,
                layout,
                cell_w,
                cell_h,
                margin,
            ) {
                let btn = match button {
                    MouseButton::Left => 0u8,
                    MouseButton::Middle => 1,
                    MouseButton::Right => 2,
                    _ => return,
                };
                let pressed = button_state == ElementState::Pressed;
                let bytes = encode_mouse_event(col, row, btn, pressed, sgr);
                if let Some(pane) = panes.iter().find(|p| p.id == active_id) {
                    pane.write_to_pty(&bytes);
                }
            }
            return;
        }
    }

    if button == MouseButton::Left {
        match button_state {
            ElementState::Pressed => {
                let x = state.cursor_x;
                let y = state.cursor_y;

                // Scrollbar click: intercept before other handlers when enabled.
                if x >= layout.sidebar_width as f64
                    && state.config.scrollbar
                    && scrollbar_click(x, y, tab_bar, layout, margin, panes)
                {
                    state.scrollbar_drag = Some(tab_bar.active_tab().active_pane);
                    return;
                }

                // Multi-click tracking (reset when modifier held)
                let alt_held = state.modifiers.alt_key();
                let now = std::time::Instant::now();
                if shift_held || alt_held {
                    state.click_count = 1;
                } else if now.duration_since(state.last_click_time)
                    < std::time::Duration::from_millis(400)
                {
                    state.click_count = state.click_count.saturating_add(1).min(3);
                } else {
                    state.click_count = 1;
                }
                state.last_click_time = now;
                let click = state.click_count;

                if x < layout.sidebar_width as f64 {
                    clear_zoom(state, tab_bar);
                    handle_tab_click(
                        tab_bar,
                        panes,
                        x,
                        y,
                        layout,
                        &mut state.tab_scroll_offset,
                        cell_w,
                        cell_h,
                        scrollback_lines,
                        state.config.shell_program.as_str(),
                        &state.config.shell_args,
                    );
                } else if state.hover_divider && click == 1 && !shift_held && !alt_held {
                    let pane_area = layout.pane_area();
                    let pane_rect = PaneRect {
                        x: pane_area.0,
                        y: pane_area.1,
                        w: pane_area.2,
                        h: pane_area.3,
                    };
                    let dividers = tab_bar.active_tab().pane_tree.get_dividers(pane_rect);
                    if let Some(info) = find_hovered_divider(&dividers, x, y) {
                        state.dragging_divider = Some(DividerDrag {
                            pane_id: info.pane_id,
                            direction: info.direction,
                            parent_rect: info.parent_rect,
                        });
                        state.selecting = false;
                    }
                } else if shift_held || alt_held {
                    // Shift+click: extend selection to click point
                    // Alt+click:   start/continue block selection
                    if let Some((col, row)) =
                        cursor_to_pane_cell(x, y, tab_bar, layout, cell_w, cell_h, margin)
                    {
                        if let Some(pane) = panes.iter().find(|p| p.id == active_id) {
                            if let Ok(mut term) = pane.term.lock() {
                                if alt_held {
                                    term.selection = Some(TermSelection::new(
                                        SelectionType::Block,
                                        Point::new(Line(row as i32), Column(col)),
                                        Side::Left,
                                    ));
                                } else if let Some(ref mut sel) = term.selection {
                                    sel.update(
                                        Point::new(Line(row as i32), Column(col)),
                                        Side::Right,
                                    );
                                } else {
                                    term.selection = Some(TermSelection::new(
                                        SelectionType::Simple,
                                        Point::new(Line(row as i32), Column(col)),
                                        Side::Left,
                                    ));
                                }
                            }
                            pane.dirty.store(true, std::sync::atomic::Ordering::Release);
                        }
                    }
                    state.selecting = true;
                } else {
                    let sel_type = match click {
                        3.. => SelectionType::Lines,
                        2 => SelectionType::Semantic,
                        _ => SelectionType::Simple,
                    };
                    if let Some((col, row)) =
                        cursor_to_pane_cell(x, y, tab_bar, layout, cell_w, cell_h, margin)
                    {
                        if let Some(pane) = panes.iter().find(|p| p.id == active_id) {
                            if let Ok(mut term) = pane.term.lock() {
                                term.selection = Some(TermSelection::new(
                                    sel_type,
                                    Point::new(Line(row as i32), Column(col)),
                                    Side::Left,
                                ));
                            }
                            pane.dirty.store(true, std::sync::atomic::Ordering::Release);
                        }
                    }
                    // Only drag-extend on single click; double/triple selects word/line immediately.
                    state.selecting = click == 1;
                }
            }
            ElementState::Released => {
                state.selecting = false;
                state.dragging_divider = None;
                state.scrollbar_drag = None;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_cursor_moved(
    position: PhysicalPosition<f64>,
    scale_factor: f64,
    state: &mut AppState,
    tab_bar: &mut TabBar,
    panes: &[Pane],
    layout: &Layout,
    window: &Arc<Window>,
    cell_w: f32,
    cell_h: f32,
    margin: f32,
) {
    // Guardamos cursor en píxeles FÍSICOS para que sea consistente con
    // el layout (que usa physical size). El scale_factor ya no se usa aquí.
    let _ = scale_factor;
    state.cursor_x = position.x;
    state.cursor_y = position.y;

    // Tab hover detection in vertical sidebar
    if state.cursor_x < layout.sidebar_width as f64 {
        let tab_count = tab_bar.tabs.len();
        let (start, end, show_up, _) = layout.tab_visible_range(tab_count, state.tab_scroll_offset);
        let header_h = synapse_ui::SIDEBAR_HEADER_HEIGHT as f64;
        let tab_h = synapse_ui::SIDEBAR_TAB_HEIGHT as f64;

        // Check if in tab area (below header, above bottom button)
        let scroll_top = header_h
            + if show_up {
                synapse_ui::SIDEBAR_SCROLL_BTN_H as f64
            } else {
                0.0
            };
        let rel_y = state.cursor_y - scroll_top;
        if rel_y >= 0.0 {
            let vis_idx = (rel_y / tab_h).floor() as usize;
            let actual = start + vis_idx;
            state.hover_tab = if actual < end { Some(actual) } else { None };
        } else {
            state.hover_tab = None;
        }
    } else {
        state.hover_tab = None;
    }

    if let Some(ref drag) = state.dragging_divider {
        let new_ratio = match drag.direction {
            SplitDirection::Horizontal => {
                (state.cursor_y as f32 - drag.parent_rect.y) / drag.parent_rect.h
            }
            SplitDirection::Vertical => {
                (state.cursor_x as f32 - drag.parent_rect.x) / drag.parent_rect.w
            }
        };
        tab_bar
            .active_tab_mut()
            .pane_tree
            .set_ratio(drag.pane_id, new_ratio);
        state.hover_divider = true;
    } else if let Some(scroll_pid) = state.scrollbar_drag {
        // Continue scrollbar drag: same click logic, but filtered to the dragged pane
        let pane_area = layout.pane_area();
        let pane_rect = PaneRect {
            x: pane_area.0,
            y: pane_area.1,
            w: pane_area.2,
            h: pane_area.3,
        };
        let layouts = tab_bar.active_tab().pane_tree.get_layout(pane_rect);
        if let Some((_pid, rect)) = layouts.iter().find(|(id, _)| *id == scroll_pid) {
            let content_y = rect.y + margin;
            let content_h = rect.h - margin * 2.0;
            if let Some(pane) = panes.iter().find(|p| p.id == scroll_pid) {
                if let Ok(term) = pane.term.lock() {
                    let display_offset = term.grid().display_offset();
                    let history_size = term.grid().history_size();
                    if history_size > 0 {
                        let pane_rows = pane.rows;
                        let total = (pane_rows + history_size).max(1) as f32;
                        let thumb_h = (content_h * pane_rows as f32 / total).max(12.0);
                        let travel = content_h - thumb_h;
                        let frac =
                            ((state.cursor_y as f32 - content_y - thumb_h / 2.0) / travel)
                                .clamp(0.0, 1.0);
                        let target_offset = ((1.0 - frac) * history_size as f32) as usize;
                        let delta = target_offset as i32 - display_offset as i32;
                        if delta != 0 {
                            drop(term);
                            pane.scroll_viewport(alacritty_terminal::grid::Scroll::Delta(delta));
                        }
                    }
                }
            }
        }
    } else if !state.selecting {
        let pane_area = layout.pane_area();
        let pane_rect = synapse_ui::PaneRect {
            x: pane_area.0,
            y: pane_area.1,
            w: pane_area.2,
            h: pane_area.3,
        };
        let dividers = tab_bar.active_tab().pane_tree.get_dividers(pane_rect);
        if let Some(info) = find_hovered_divider(&dividers, state.cursor_x, state.cursor_y) {
            match info.direction {
                SplitDirection::Horizontal => {
                    window.set_cursor(CursorIcon::NsResize);
                }
                SplitDirection::Vertical => {
                    window.set_cursor(CursorIcon::EwResize);
                }
            }
            state.hover_divider = true;
        } else {
            if state.hover_divider {
                window.set_cursor(CursorIcon::Text);
            }
            state.hover_divider = false;
        }
    }

    if state.selecting {
        let x = state.cursor_x;
        let y = state.cursor_y;
        if let Some((col, row)) = cursor_to_pane_cell(x, y, tab_bar, layout, cell_w, cell_h, margin)
        {
            let active_id = tab_bar.active_tab().active_pane;
            if let Some(pane) = panes.iter().find(|p| p.id == active_id) {
                if let Ok(mut term) = pane.term.lock() {
                    if let Some(sel) = term.selection.as_mut() {
                        sel.update(Point::new(Line(row as i32), Column(col)), Side::Right);
                    }
                }
                pane.dirty.store(true, std::sync::atomic::Ordering::Release);
            }
        }
    }
}

fn is_path_url(s: &str) -> bool {
    s.starts_with('/') || s.starts_with("./") || s.starts_with("../") || s.starts_with("~/")
}

fn open_url(url: &str) {
    if is_path_url(url) {
        open_path(url);
    } else {
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(url).spawn();
        #[cfg(target_os = "linux")]
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

fn open_path(path: &str) {
    let (file, line, col) = parse_path_spec(path);

    let editor = std::env::var("EDITOR").unwrap_or_default();

    if !line.is_empty() {
        let code_spec = if !col.is_empty() {
            format!("{file}:{line}:{col}")
        } else {
            format!("{file}:{line}")
        };
        let code_result = std::process::Command::new("code")
            .arg("--goto")
            .arg(&code_spec)
            .spawn();
        if code_result.is_ok() {
            return;
        }

        if !editor.is_empty() {
            let _ = std::process::Command::new(&editor)
                .arg(format!("+{line}"))
                .arg(&file)
                .spawn();
        } else {
            #[cfg(target_os = "macos")]
            let _ = std::process::Command::new("open").arg(&file).spawn();
            #[cfg(target_os = "linux")]
            let _ = std::process::Command::new("xdg-open").arg(&file).spawn();
        }
    } else {
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(&file).spawn();
        #[cfg(target_os = "linux")]
        let _ = std::process::Command::new("xdg-open").arg(&file).spawn();
    }
}

fn parse_path_spec(raw: &str) -> (String, String, String) {
    let re = regex::Regex::new(r"^(.+?):(\d+):(\d+)$").unwrap();
    if let Some(caps) = re.captures(raw) {
        return (
            caps[1].to_string(),
            caps[2].to_string(),
            caps[3].to_string(),
        );
    }
    let re2 = regex::Regex::new(r"^(.+?):(\d+)$").unwrap();
    if let Some(caps) = re2.captures(raw) {
        return (caps[1].to_string(), caps[2].to_string(), String::new());
    }
    (raw.to_string(), String::new(), String::new())
}

use crate::app::AppCore;

impl AppCore {
    pub(crate) fn handle_scroll(&mut self, delta: winit::event::MouseScrollDelta) {
        let ws = self.workspaces.active_ws_mut();
        handle_scroll(
            delta,
            &mut ws.panes,
            &ws.tab_bar,
            &mut self.state,
            &self.layout,
            self.cell_w,
            self.cell_h,
            self.margin,
        );
    }

    pub(crate) fn handle_mouse_button(
        &mut self,
        button_state: winit::event::ElementState,
        button: winit::event::MouseButton,
    ) {
        // Wayland CSD title bar: start window drag on left click.
        if button == winit::event::MouseButton::Left
            && button_state == winit::event::ElementState::Pressed
            && self.layout.wayland_decorated
            && self.state.cursor_x >= self.layout.sidebar_width as f64
            && self.state.cursor_y < self.layout.title_bar_height() as f64
        {
            let _ = self.window.drag_window();
            return;
        }

        // Ctrl+Click: open hovered URL, skip normal click handling.
        if button == winit::event::MouseButton::Left
            && button_state == winit::event::ElementState::Pressed
            && self.state.modifiers.control_key()
        {
            if let Some(ref url) = self.state.hovered_url.clone() {
                open_url(url);
                return;
            }
        }

        let was_selecting = self.state.selecting;
        let scrollback_lines = self.state.config.scrollback_lines;
        let (tab_bar, panes, _) = self.workspaces.active_split_mut();
        handle_mouse_button(
            button_state,
            button,
            &mut self.state,
            tab_bar,
            panes,
            &self.layout,
            self.cell_w,
            self.cell_h,
            self.margin,
            scrollback_lines,
        );
        crate::pane_ops::apply_tab_freeze(
            self.workspaces.active_panes(),
            self.workspaces.active_tab_bar(),
            self.state.config.freeze_background_tabs,
        );
        // Auto-copy: copy selection to clipboard on release after any selection
        // (multi-click word/line or click-drag).
        if button == winit::event::MouseButton::Left
            && button_state == winit::event::ElementState::Released
            && (self.state.click_count >= 2 || was_selecting)
        {
            let active_id = self.workspaces.active_tab_bar().active_tab().active_pane;
            if let Some(pane) = self
                .workspaces
                .active_panes()
                .iter()
                .find(|p| p.id == active_id)
            {
                if let Ok(term) = pane.term.lock() {
                    if let Some(text) = term.selection_to_string() {
                        if !text.is_empty() {
                            if let Some(cb) = self.clipboard.as_mut() {
                                let _ = cb.set_text(text);
                            }
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn handle_cursor_moved(&mut self, position: winit::dpi::PhysicalPosition<f64>) {
        let ws = self.workspaces.active_ws_mut();
        handle_cursor_moved(
            position,
            self.window.scale_factor(),
            &mut self.state,
            &mut ws.tab_bar,
            &ws.panes,
            &self.layout,
            &self.window,
            self.cell_w,
            self.cell_h,
            self.margin,
        );

        // Detect URL hover: update cursor icon and hovered_url.
        let cx = position.x as f32;
        let cy = position.y as f32;
        let hovered = self
            .cached_url_spans
            .iter()
            .find(|s| cx >= s.x && cx < s.x + s.w && cy >= s.y && cy < s.y + s.h)
            .map(|s| s.url.clone());
        let was_hovering = self.state.hovered_url.is_some();
        self.state.hovered_url = hovered.clone();
        if hovered.is_some() && !self.state.hover_divider {
            self.window.set_cursor(winit::window::CursorIcon::Pointer);
        } else if hovered.is_none() && was_hovering && !self.state.hover_divider {
            self.window.set_cursor(winit::window::CursorIcon::Text);
        }
    }
}

#[cfg(test)]
mod tests {
    use alacritty_terminal::{
        event::EventListener,
        index::{Column, Line, Point, Side},
        selection::{Selection, SelectionType},
        term::{test::TermSize, Term},
    };

    struct DummyListener;
    impl EventListener for DummyListener {
        fn send_event(&self, _e: alacritty_terminal::event::Event) {}
    }

    fn make_term(cols: usize, rows: usize) -> Term<DummyListener> {
        Term::<DummyListener>::new(
            alacritty_terminal::term::Config::default(),
            &TermSize::new(cols, rows),
            DummyListener,
        )
    }

    #[test]
    fn block_selection_creates_rectangular_range() {
        let term = make_term(80, 24);
        let start = Point::new(Line(2), Column(5));
        let end = Point::new(Line(5), Column(40));
        let mut sel = Selection::new(SelectionType::Block, start, Side::Left);
        sel.update(end, Side::Right);
        let range = sel.to_range(&term).unwrap();
        assert!(range.is_block);
        assert_eq!(range.start, Point::new(Line(2), Column(5)));
        // Block: end column is +1 right side
    }

    #[test]
    fn simple_selection_update_extends() {
        let term = make_term(80, 24);
        let start = Point::new(Line(1), Column(3));
        let mut sel = Selection::new(SelectionType::Simple, start, Side::Left);
        sel.update(Point::new(Line(1), Column(10)), Side::Right);
        let range = sel.to_range(&term).unwrap();
        assert!(!range.is_block);
        assert_eq!(range.start, Point::new(Line(1), Column(3)));
        assert!(range.end.column.0 >= 10);
    }

    #[test]
    fn shift_click_extends_existing_selection() {
        let term = make_term(80, 24);
        let start = Point::new(Line(0), Column(0));
        let mut sel = Selection::new(SelectionType::Simple, start, Side::Left);
        sel.update(Point::new(Line(2), Column(7)), Side::Right);
        let range = sel.to_range(&term).unwrap();
        assert_eq!(range.start, Point::new(Line(0), Column(0)));
        assert!(range.end.line >= Line(2));
        assert!(range.end.column.0 >= 7);
    }

    #[test]
    fn shift_click_starts_new_selection_if_none_exists() {
        let term = make_term(80, 24);
        let start = Point::new(Line(1), Column(4));
        let mut sel = Selection::new(SelectionType::Simple, start, Side::Left);
        sel.update(Point::new(Line(3), Column(12)), Side::Right);
        let range = sel.to_range(&term).unwrap();
        assert_eq!(range.start, Point::new(Line(1), Column(4)));
        assert!(range.end.line >= Line(3));
        assert!(range.end.column.0 >= 12);
    }

    #[test]
    fn alt_click_starts_block_selection() {
        let term = make_term(80, 24);
        let start = Point::new(Line(3), Column(10));
        let mut sel = Selection::new(SelectionType::Block, start, Side::Left);
        sel.update(Point::new(Line(6), Column(50)), Side::Right);
        let range = sel.to_range(&term).unwrap();
        assert!(range.is_block);
        assert_eq!(range.start, Point::new(Line(3), Column(10)));
    }

    #[test]
    fn single_click_starts_empty_selection() {
        let term = make_term(80, 24);
        let sel = Selection::new(
            SelectionType::Simple,
            Point::new(Line(5), Column(3)),
            Side::Left,
        );
        assert!(sel.is_empty());
        assert!(sel.to_range(&term).is_none());
    }

    #[test]
    fn simple_selection_becomes_visible_on_drag() {
        let term = make_term(80, 24);
        let start = Point::new(Line(5), Column(3));
        let mut sel = Selection::new(SelectionType::Simple, start, Side::Left);
        // Single cell is empty
        assert!(sel.is_empty());
        // Drag to next cell makes it visible
        sel.update(Point::new(Line(5), Column(4)), Side::Right);
        assert!(!sel.is_empty());
        let range = sel.to_range(&term).unwrap();
        assert!(!range.is_block);
    }

    #[test]
    fn double_click_semantic_selection() {
        let mut term = make_term(80, 24);
        for i in 0..20u8 {
            term.grid_mut()[Line(0)][Column(i as usize)].c = (b'a' + i) as char;
        }
        let sel = Selection::new(
            SelectionType::Semantic,
            Point::new(Line(0), Column(5)),
            Side::Left,
        );
        let range = sel.to_range(&term);
        assert!(range.is_some());
        let r = range.unwrap();
        assert!(!r.is_block);
        assert!(r.end.column.0 - r.start.column.0 > 1);
    }

    #[test]
    fn triple_click_lines_selection() {
        let term = make_term(80, 24);
        let mut sel = Selection::new(
            SelectionType::Lines,
            Point::new(Line(7), Column(0)),
            Side::Left,
        );
        sel.update(Point::new(Line(9), Column(0)), Side::Right);
        let range = sel.to_range(&term).unwrap();
        assert!(!range.is_block);
        assert_eq!(range.start.column, Column(0));
        assert!(range.end.line > range.start.line);
    }
}
