use std::sync::Arc;

use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton, MouseScrollDelta},
    window::{CursorIcon, Window},
};

use luna_ui::{layout::Layout, pane::Pane, splitter::SplitDirection, tab_bar::TabBar, TAB_BAR_HEIGHT};

use crate::{
    pane_ops::{active_pane_mut, find_hovered_divider, handle_tab_click},
    state::{AppState, DividerDrag, Selection},
};

pub fn handle_scroll(
    delta: MouseScrollDelta,
    panes: &mut [Pane],
    tab_bar: &TabBar,
    cell_h: f32,
) {
    let pane = active_pane_mut(panes, tab_bar);
    let lines = match delta {
        MouseScrollDelta::LineDelta(_, y) => (y.abs() as usize).max(1),
        MouseScrollDelta::PixelDelta(pos) => (pos.y.abs() / cell_h as f64) as usize,
    };
    let is_up = match delta {
        MouseScrollDelta::LineDelta(_, y) => y > 0.0,
        MouseScrollDelta::PixelDelta(pos) => pos.y > 0.0,
    };
    let mut grid_mut = pane.grid.borrow_mut();
    if is_up {
        grid_mut.scroll_down(lines);
    } else {
        grid_mut.scroll_up(lines);
    }
}

pub fn handle_mouse_button(
    button_state: ElementState,
    button: MouseButton,
    state: &mut AppState,
    tab_bar: &mut TabBar,
    panes: &mut Vec<Pane>,
    layout: &Layout,
) {
    if button == MouseButton::Left {
        match button_state {
            ElementState::Pressed => {
                let x = state.cursor_x;
                let y = state.cursor_y;
                if y < TAB_BAR_HEIGHT as f64 {
                    handle_tab_click(
                        tab_bar,
                        panes,
                        x,
                        layout.window_width as f64,
                    );
                } else if state.hover_divider {
                    let pane_area = layout.pane_area();
                    let pane_rect = luna_ui::PaneRect {
                        x: pane_area.0,
                        y: pane_area.1,
                        w: pane_area.2,
                        h: pane_area.3,
                    };
                    let dividers =
                        tab_bar.active_tab().pane_tree.get_dividers(pane_rect);
                    if let Some(info) =
                        find_hovered_divider(&dividers, x, y)
                    {
                        state.dragging_divider = Some(DividerDrag {
                            pane_id: info.pane_id,
                            direction: info.direction,
                            parent_rect: info.parent_rect,
                        });
                        state.selecting = false;
                    }
                } else {
                    state.selecting = true;
                }
            }
            ElementState::Released => {
                state.selecting = false;
                state.dragging_divider = None;
            }
        }
    }
}

pub fn handle_cursor_moved(
    position: PhysicalPosition<f64>,
    scale_factor: f64,
    state: &mut AppState,
    tab_bar: &mut TabBar,
    layout: &Layout,
    window: &Arc<Window>,
    cell_w: f32,
    cell_h: f32,
    margin: f32,
) {
    let sf = scale_factor;
    state.cursor_x = position.x / sf;
    state.cursor_y = position.y / sf;

    if let Some(ref drag) = state.dragging_divider {
        let new_ratio = match drag.direction {
            SplitDirection::Horizontal => {
                ((state.cursor_y as f32 - drag.parent_rect.y) / drag.parent_rect.h)
                    as f32
            }
            SplitDirection::Vertical => {
                ((state.cursor_x as f32 - drag.parent_rect.x) / drag.parent_rect.w)
                    as f32
            }
        };
        tab_bar
            .active_tab_mut()
            .pane_tree
            .set_ratio(drag.pane_id, new_ratio);
        state.hover_divider = true;
    } else if !state.selecting {
        let pane_area = layout.pane_area();
        let pane_rect = luna_ui::PaneRect {
            x: pane_area.0,
            y: pane_area.1,
            w: pane_area.2,
            h: pane_area.3,
        };
        let dividers = tab_bar.active_tab().pane_tree.get_dividers(pane_rect);
        if let Some(info) =
            find_hovered_divider(&dividers, state.cursor_x, state.cursor_y)
        {
            match info.direction {
                SplitDirection::Horizontal => {
                    window.set_cursor_icon(CursorIcon::NsResize);
                }
                SplitDirection::Vertical => {
                    window.set_cursor_icon(CursorIcon::EwResize);
                }
            }
            state.hover_divider = true;
        } else {
            if state.hover_divider {
                window.set_cursor_icon(CursorIcon::Text);
            }
            state.hover_divider = false;
        }
    }

    if state.selecting {
        let x = state.cursor_x;
        let y = state.cursor_y;

        let pane_top = TAB_BAR_HEIGHT as f64;
        let col = ((x - margin as f64) / cell_w as f64).floor().max(0.0) as usize;
        let viewport_row = ((y - pane_top - margin as f64) / cell_h as f64)
            .floor()
            .max(0.0) as usize;

        if let Some(ref mut sel) = state.selection {
            sel.update_end(col, viewport_row);
        } else {
            state.selection = Some(Selection::new(col, viewport_row));
        }
    }
}

use crate::app::App;

impl App {
    pub(crate) fn handle_scroll(&mut self, delta: winit::event::MouseScrollDelta) {
        handle_scroll(delta, &mut self.panes, &self.tab_bar, self.cell_h);
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
        );
    }

    pub(crate) fn handle_cursor_moved(
        &mut self,
        position: winit::dpi::PhysicalPosition<f64>,
    ) {
        handle_cursor_moved(
            position,
            self.window.scale_factor(),
            &mut self.state,
            &mut self.tab_bar,
            &self.layout,
            &self.window,
            self.cell_w,
            self.cell_h,
            self.margin,
        );
    }
}
