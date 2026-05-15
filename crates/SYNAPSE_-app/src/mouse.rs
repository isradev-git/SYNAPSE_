use std::sync::Arc;

use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton, MouseScrollDelta},
    window::{CursorIcon, Window},
};

use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection as TermSelection, SelectionType};
use alacritty_terminal::term::TermMode;
use synapse_ui::{
    layout::Layout, pane::Pane, tab_bar::TabBar, PaneRect, SplitDirection, SCROLL_BTN_W,
    TAB_BAR_HEIGHT,
};

use crate::{
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

#[allow(clippy::too_many_arguments)]
pub fn handle_scroll(
    delta: MouseScrollDelta,
    panes: &mut [Pane],
    tab_bar: &TabBar,
    state: &AppState,
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

    let active_id = tab_bar.active_tab().active_pane;
    let (mouse_active, sgr) = active_pane_mouse_modes(panes, active_id);

    if mouse_active && state.cursor_y >= TAB_BAR_HEIGHT as f64 {
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
) {
    let active_id = tab_bar.active_tab().active_pane;
    let shift_held = state.modifiers.shift_key();

    // Mouse reporting: intercept clicks for apps like vim/htop when Shift not held
    {
        let (mouse_active, sgr) = active_pane_mouse_modes(panes, active_id);

        if mouse_active && !shift_held && state.cursor_y >= TAB_BAR_HEIGHT as f64 {
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

                // Multi-click tracking
                let now = std::time::Instant::now();
                if now.duration_since(state.last_click_time) < std::time::Duration::from_millis(400)
                {
                    state.click_count = state.click_count.saturating_add(1).min(3);
                } else {
                    state.click_count = 1;
                }
                state.last_click_time = now;
                let click = state.click_count;

                if y < TAB_BAR_HEIGHT as f64 {
                    handle_tab_click(tab_bar, panes, x, layout, &mut state.tab_scroll_offset);
                } else if state.hover_divider && click == 1 {
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
                } else {
                    let sel_type = match click {
                        3.. => SelectionType::Lines,
                        2 => SelectionType::Semantic,
                        _ => SelectionType::Simple,
                    };
                    if let Some((col, row)) = cursor_to_pane_cell(
                        x, y, tab_bar, layout, cell_w, cell_h, margin,
                    ) {
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
    let sf = scale_factor;
    state.cursor_x = position.x / sf;
    state.cursor_y = position.y / sf;

    // Tab hover detection
    if state.cursor_y < TAB_BAR_HEIGHT as f64 {
        let tab_count = tab_bar.tabs.len();
        let (start, end, show_left, show_right) =
            layout.tab_visible_range(tab_count, state.tab_scroll_offset);
        let vis_count = end - start;
        let x_start = if show_left { SCROLL_BTN_W as f64 } else { 0.0 };
        if vis_count > 0 {
            let tab_w = layout.scrolled_tab_width(vis_count, show_left, show_right) as f64;
            let rel = state.cursor_x - x_start;
            if rel >= 0.0 {
                let vis_idx = (rel / tab_w).floor() as usize;
                let actual = start + vis_idx;
                state.hover_tab = if actual < end { Some(actual) } else { None };
            } else {
                state.hover_tab = None;
            }
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
        if let Some((col, row)) =
            cursor_to_pane_cell(x, y, tab_bar, layout, cell_w, cell_h, margin)
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

use crate::app::App;

impl App {
    pub(crate) fn handle_scroll(&mut self, delta: winit::event::MouseScrollDelta) {
        handle_scroll(
            delta,
            &mut self.panes,
            &self.tab_bar,
            &self.state,
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
        handle_mouse_button(
            button_state,
            button,
            &mut self.state,
            &mut self.tab_bar,
            &mut self.panes,
            &self.layout,
            self.cell_w,
            self.cell_h,
            self.margin,
        );
        // Phase 1: auto-copy on multi-click is deferred until selection
        // extraction is reimplemented against the alacritty grid.
    }

    pub(crate) fn handle_cursor_moved(&mut self, position: winit::dpi::PhysicalPosition<f64>) {
        handle_cursor_moved(
            position,
            self.window.scale_factor(),
            &mut self.state,
            &mut self.tab_bar,
            &self.panes,
            &self.layout,
            &self.window,
            self.cell_w,
            self.cell_h,
            self.margin,
        );
    }
}
