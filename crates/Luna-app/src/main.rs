mod state;
mod input;

use std::collections::HashSet;
use std::sync::Arc;

use winit::{
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::WindowAttributes,
};

use state::AppState;
use input::InputAction;
use luna_config::Action;
use luna_renderer::ui::UIRect;
use luna_ui::{
    layout::Layout,
    pane::{Pane, PaneId},
    splitter::{PaneRect, SplitDirection},
    tab_bar::{Tab, TabBar, TabId},
    theme, TAB_BAR_HEIGHT,
};

const TAB_FONT_SIZE: f32 = 12.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = luna_config::Config::load();
    let keybinds = luna_config::Keybinds::new();

    let event_loop = EventLoop::new()?;
    let window_attrs = WindowAttributes::default()
        .with_title("Luna")
        .with_inner_size(winit::dpi::LogicalSize::new(
            config.window_width as f64,
            config.window_height as f64,
        ))
        .with_resizable(true);
    let window = Arc::new(event_loop.create_window(window_attrs)?);
    let mut renderer = luna_renderer::renderer::Renderer::new(window.clone());

    let mut layout = Layout::new();
    let size = renderer.size();
    layout.update(size.width as f32, size.height as f32);

    let initial_font_size = config.font_size;
    let (mut cell_w, mut cell_h) = renderer.cell_metrics(initial_font_size);

    let pane_area = layout.pane_area();
    let margin = layout.pane_margin();
    let cols = ((pane_area.2 - margin * 2.0) / cell_w).max(1.0) as usize;
    let rows = ((pane_area.3 - margin * 2.0) / cell_h).max(1.0) as usize;

    let first_tab_id = TabId(0);
    let first_pane_id = PaneId(0);
    let first_pane = create_pane(first_pane_id, cols, rows);
    let first_tab = Tab::new(first_tab_id, first_pane_id);
    let mut tab_bar = TabBar::new(first_tab);

    let mut panes: Vec<Pane> = Vec::new();
    panes.push(first_pane);

    let mut clipboard = arboard::Clipboard::new().ok();
    let mut state = AppState::new(config, keybinds, initial_font_size);

    event_loop.set_control_flow(ControlFlow::Poll);

    #[allow(deprecated)]
    event_loop.run(move |event, elwt| match event {
        Event::WindowEvent { event, .. } => match event {
            WindowEvent::CloseRequested => elwt.exit(),

            WindowEvent::Resized(size) => {
                renderer.resize(size);
                let window_width = size.width as f32;
                let window_height = size.height as f32;
                layout.update(window_width, window_height);

                let pane_area = layout.pane_area();
                let pane_rect = luna_ui::PaneRect {
                    x: pane_area.0,
                    y: pane_area.1,
                    w: pane_area.2,
                    h: pane_area.3,
                };
                let layouts = tab_bar.active_tab().pane_tree.get_layout(pane_rect);

                for (pane_id, rect) in &layouts {
                    let new_cols = ((rect.w - margin * 2.0) / cell_w).max(1.0) as usize;
                    let new_rows = ((rect.h - margin * 2.0) / cell_h).max(1.0) as usize;
                    if let Some(pane) = panes.iter_mut().find(|p| p.id == *pane_id) {
                        if new_cols != pane.cols || new_rows != pane.rows {
                            pane.cols = new_cols;
                            pane.rows = new_rows;
                            pane.grid.borrow_mut().resize(new_cols, new_rows);
                            let _ = pane.pty_session.pty.resize(new_cols as u16, new_rows as u16);
                        }
                    }
                }
            }

            WindowEvent::ModifiersChanged(mods) => {
                state.modifiers = mods.state();
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let pane = active_pane_mut(&mut panes, &tab_bar);
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

            WindowEvent::MouseInput { state: button_state, button, .. } => {
                if button == MouseButton::Left {
                    match button_state {
                        ElementState::Pressed => {
                            let x = state.cursor_x;
                            let y = state.cursor_y;
                            if y < TAB_BAR_HEIGHT as f64 {
                                handle_tab_click(
                                    &mut tab_bar,
                                    &mut panes,
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
                                    state.dragging_divider = Some(state::DividerDrag {
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

            WindowEvent::CursorMoved { position, .. } => {
                let sf = window.scale_factor();
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
                                window.set_cursor_icon(winit::window::CursorIcon::NsResize);
                            }
                            SplitDirection::Vertical => {
                                window.set_cursor_icon(winit::window::CursorIcon::EwResize);
                            }
                        }
                        state.hover_divider = true;
                    } else {
                        if state.hover_divider {
                            window.set_cursor_icon(winit::window::CursorIcon::Text);
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
                        state.selection = Some(state::Selection::new(col, viewport_row));
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                let font_size = state.font_size;
                for pane in panes.iter_mut() {
                    while let Ok(data) = pane.pty_session.rx.try_recv() {
                        pane.processor.process(&data);
                    }
                }

                for tab in tab_bar.tabs.iter_mut() {
                    if let Some(p) = panes.iter().find(|p| p.id == tab.active_pane) {
                        let t = p.title();
                        if !t.is_empty() && t != tab.title {
                            tab.title = t;
                        }
                    }
                }

                let active_pane_id = tab_bar.active_tab().active_pane;
                let pane_tree = &tab_bar.active_tab().pane_tree;

                let pane_area = layout.pane_area();
                let margin = layout.pane_margin();
                let pane_rect = luna_ui::PaneRect {
                    x: pane_area.0,
                    y: pane_area.1,
                    w: pane_area.2,
                    h: pane_area.3,
                };

                let layouts = pane_tree.get_layout(pane_rect);
                let dividers = pane_tree.get_dividers(pane_rect);

                let mut cell_data: Vec<(char, f32, f32, f32, [f32; 4], [f32; 4])> = Vec::new();
                let mut ui_rects: Vec<UIRect> = Vec::new();

                let match_set: HashSet<(usize, usize)> = if state.search.active && !state.search.term.is_empty() {
                    build_match_set(&state.search.matches, state.search.term.len())
                } else {
                    HashSet::new()
                };

                for &(pane_id, rect) in &layouts {
                    let pane = find_pane(&panes, pane_id);
                    if pane.is_none() {
                        continue;
                    }
                    let pane = pane.unwrap();
                    let grid_ref = pane.grid.borrow();

                    let content_x = rect.x + margin;
                    let content_y = rect.y + margin;
                    let content_w = (rect.w - margin * 2.0).max(0.0);
                    let content_h = (rect.h - margin * 2.0).max(0.0);

                    let pane_cols = ((content_w) / cell_w).max(1.0) as usize;
                    let pane_rows = ((content_h) / cell_h).max(1.0) as usize;

                    let cursor_col = grid_ref.cursor_col();
                    let cursor_row = grid_ref.cursor_row();
                    let scrollback_len = grid_ref.scrollback_len();
                    let scroll_offset = grid_ref.scroll_offset();
                    let sb_visible = ((scroll_offset + pane_rows).min(scrollback_len)).saturating_sub(scroll_offset);
                    let scrolled = scroll_offset > 0;
                    let is_active = pane_id == active_pane_id;

                    for (col, vrow, cell) in grid_ref.visible_cells() {
                        if col >= pane_cols || vrow >= pane_rows {
                            continue;
                        }
                        if !scrolled && col == cursor_col && vrow == cursor_row {
                            continue;
                        }
                        if cell.c == ' ' && cell.bg == luna_terminal::grid::Color::Default {
                            continue;
                        }

                        let x = content_x + col as f32 * cell_w;
                        let y = content_y + vrow as f32 * cell_h;

                        let global_row = if vrow < sb_visible {
                            scroll_offset + vrow
                        } else {
                            scrollback_len + vrow - sb_visible
                        };
                        let selection_bg = is_active
                            && state.selection.as_ref().map_or(false, |s| s.contains(col, vrow));
                        let match_is_current = state.search.active && !state.search.term.is_empty()
                            && !state.search.matches.is_empty()
                            && {
                                let cm = &state.search.matches[state.search.current_match];
                                cm.row == global_row && col >= cm.col && col < cm.col + state.search.term.len()
                            };
                        let match_is_in = match_set.contains(&(col, global_row));
                        let bg = if selection_bg {
                            [1.0, 0.239, 0.58, 0.4]
                        } else if match_is_current {
                            theme::SEARCH_CURRENT
                        } else if match_is_in {
                            theme::SEARCH_HIGHLIGHT
                        } else {
                            cell.bg.bg_rgba()
                        };

                        cell_data.push((cell.c, x, y, font_size, cell.fg.fg_rgba(), bg));
                    }

                    if !scrolled && cursor_col < pane_cols && cursor_row < pane_rows {
                        let cell = grid_ref.get(cursor_col, cursor_row);
                        let cx = content_x + cursor_col as f32 * cell_w;
                        let cy = content_y + cursor_row as f32 * cell_h;
                        let cursor_fg = [1.0, 0.239, 0.58, 1.0];
                        let cursor_bg = [1.0, 1.0, 1.0, 0.15];
                        cell_data.push((cell.c, cx, cy, font_size, cursor_fg, cursor_bg));
                    }
                    drop(grid_ref);

                    // Pane borders (1px)
                    let border_color = if is_active {
                        theme::PANEL_ACTIVE_BORDER
                    } else {
                        theme::PANEL_INACTIVE_BORDER
                    };
                    ui_rects.push(UIRect {
                        pos: [rect.x, rect.y],
                        size: [rect.w, 1.0],
                        color: border_color,
                    });
                    ui_rects.push(UIRect {
                        pos: [rect.x, rect.y + rect.h - 1.0],
                        size: [rect.w, 1.0],
                        color: border_color,
                    });
                    ui_rects.push(UIRect {
                        pos: [rect.x, rect.y],
                        size: [1.0, rect.h],
                        color: border_color,
                    });
                    ui_rects.push(UIRect {
                        pos: [rect.x + rect.w - 1.0, rect.y],
                        size: [1.0, rect.h],
                        color: border_color,
                    });
                }

                // Splitter dividers (2px line at center of 6px hitbox)
                for info in &dividers {
                    let d = info.hitbox;
                    let (dx, dy, dw, dh) = if d.w > d.h {
                        (d.x, d.y + 2.0, d.w, 2.0)
                    } else {
                        (d.x + 2.0, d.y, 2.0, d.h)
                    };
                    ui_rects.push(UIRect {
                        pos: [dx, dy],
                        size: [dw, dh],
                        color: theme::PANEL_DIVIDER,
                    });
                }

                // Search bar overlay
                if state.search.active {
                    let pane_area = layout.pane_area();
                    let bar_x = pane_area.0;
                    let bar_y = pane_area.1;
                    let bar_w = pane_area.2;
                    let bar_h = theme::SEARCH_BAR_HEIGHT;

                    ui_rects.push(UIRect {
                        pos: [bar_x, bar_y],
                        size: [bar_w, bar_h],
                        color: theme::SEARCH_BAR_BG,
                    });

                    let search_fs = 12.0;
                    let char_w = search_fs * 0.6;
                    let text_y = bar_y + 7.0;
                    let text_x = bar_x + 8.0;
                    let prefix = "Search: ";
                    let prefix_chars: Vec<char> = prefix.chars().collect();
                    let term_chars: Vec<char> = state.search.term.chars().collect();
                    let transparent = [0.0, 0.0, 0.0, 0.0];

                    for (j, &c) in prefix_chars.iter().enumerate() {
                        cell_data.push((c, text_x + j as f32 * char_w, text_y, search_fs, theme::SEARCH_TEXT_DIM, transparent));
                    }
                    for (j, &c) in term_chars.iter().enumerate() {
                        cell_data.push((c, text_x + (prefix_chars.len() + j) as f32 * char_w, text_y, search_fs, theme::SEARCH_TEXT, transparent));
                    }
                    let cursor_col = prefix_chars.len() + state.search.cursor_pos;
                    cell_data.push(('|', text_x + cursor_col as f32 * char_w, text_y, search_fs, theme::SEARCH_TEXT, transparent));

                    let counter = if state.search.term.is_empty() {
                        String::new()
                    } else if state.search.matches.is_empty() {
                        "0 matches".to_string()
                    } else {
                        format!("{}/{}", state.search.current_match + 1, state.search.matches.len())
                    };
                    if !counter.is_empty() {
                        let counter_x = bar_x + bar_w - counter.len() as f32 * char_w - 8.0;
                        for (j, c) in counter.chars().enumerate() {
                            cell_data.push((c, counter_x + j as f32 * char_w, text_y, search_fs, theme::SEARCH_TEXT_DIM, transparent));
                        }
                    }
                }

                // History search bar overlay
                if state.history_search.active {
                    let pane_area = layout.pane_area();
                    let bar_x = pane_area.0;
                    let bar_y = pane_area.1 + pane_area.3 - theme::SEARCH_BAR_HEIGHT;
                    let bar_w = pane_area.2;
                    let bar_h = theme::SEARCH_BAR_HEIGHT;

                    ui_rects.push(UIRect {
                        pos: [bar_x, bar_y],
                        size: [bar_w, bar_h],
                        color: theme::SEARCH_BAR_BG,
                    });

                    let search_fs = 12.0;
                    let char_w = search_fs * 0.6;
                    let text_y = bar_y + 7.0;
                    let text_x = bar_x + 8.0;
                    let transparent = [0.0, 0.0, 0.0, 0.0];
                    let max_chars = ((bar_w - 24.0) / char_w).max(20.0) as usize;

                    let mut line = format!("(reverse-i-search)`{}': ", state.history_search.term);
                    if let Some(mt) = state.history_search.current_text() {
                        if mt.len() > max_chars.saturating_sub(line.len()) {
                            line.push_str(&mt[..max_chars.saturating_sub(line.len()).max(1)]);
                        } else {
                            line.push_str(mt);
                        }
                    }

                    for (j, c) in line.chars().enumerate() {
                        if j >= max_chars {
                            break;
                        }
                        cell_data.push((c, text_x + j as f32 * char_w, text_y, search_fs, theme::SEARCH_TEXT, transparent));
                    }

                    // Match counter
                    if !state.history_search.matches.is_empty() {
                        let counter = format!("{}/{}", state.history_search.current_match + 1, state.history_search.matches.len());
                        let cx = bar_x + bar_w - counter.len() as f32 * char_w - 8.0;
                        for (j, c) in counter.chars().enumerate() {
                            cell_data.push((c, cx + j as f32 * char_w, text_y, search_fs, theme::SEARCH_TEXT_DIM, transparent));
                        }
                    }
                }

                let tab_ui = build_tab_bar_ui_rects(&layout, &tab_bar);
                ui_rects.extend(tab_ui);

                for tab_cell in build_tab_bar_text(&layout, &tab_bar, window.scale_factor()) {
                    cell_data.push(tab_cell);
                }

                renderer.draw_frame(&cell_data, &ui_rects);
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed && !event.repeat {
                    let logical_key = &event.logical_key;

                    // Search input handling (when active)
                    if state.search.active {
                        handle_search_input(logical_key, &event, &mut state, &tab_bar, &panes);
                        return;
                    }

                    // History search input handling (when active)
                    if state.history_search.active {
                        handle_history_search_input(logical_key, &event, &mut state, &tab_bar, &mut panes);
                        return;
                    }

                    // Keybind lookup
                    let action_opt = state.keybinds.lookup(&logical_key, state.modifiers);
                    let mut keybind_handled = true;
                    match action_opt {
                        Some(Action::Search) => {
                            state.search.toggle();
                            if state.search.active {
                                update_search_matches(&mut state, &tab_bar, &panes);
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
                            let pane = active_pane_mut(&mut panes, &tab_bar);
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
                            let new_size = (state.font_size + 1.0).min(32.0);
                            change_font_size(&mut state, &mut renderer, &mut panes, &tab_bar, &layout, margin, &mut cell_w, &mut cell_h, new_size);
                        }
                        Some(Action::FontDecrease) => {
                            let new_size = (state.font_size - 1.0).max(6.0);
                            change_font_size(&mut state, &mut renderer, &mut panes, &tab_bar, &layout, margin, &mut cell_w, &mut cell_h, new_size);
                        }
                        Some(Action::FontReset) => {
                            let default_size = state.config.font_size;
                            change_font_size(&mut state, &mut renderer, &mut panes, &tab_bar, &layout, margin, &mut cell_w, &mut cell_h, default_size);
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
                            let pane = active_pane_mut(&mut panes, &tab_bar);
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
                                    let pane = active_pane_mut(&mut panes, &tab_bar);
                                    let _ = pane.pty_session.pty.write(b"\x1b[200~");
                                    let _ = pane.pty_session.pty.write(text.as_bytes());
                                    let _ = pane.pty_session.pty.write(b"\x1b[201~");
                                }
                            }
                        }
                        Some(Action::ReloadConfig) => {
                            state.config.reload();
                            let default_size = state.config.font_size;
                            change_font_size(&mut state, &mut renderer, &mut panes, &tab_bar, &layout, margin, &mut cell_w, &mut cell_h, default_size);
                        }
                        None => {
                            keybind_handled = false;
                        }
                    }
                    if keybind_handled {
                        return;
                    }

                    let action = InputAction::from_key(&event, state.modifiers);
                    match action {
                        InputAction::Write(bytes) => {
                            if bytes != b"\x1b[5~" && bytes != b"\x1b[6~" {
                                active_pane_mut(&mut panes, &tab_bar)
                                    .grid.borrow_mut().scroll_to_bottom();
                            }
                            let pane = active_pane_mut(&mut panes, &tab_bar);
                            if let Err(e) = pane.pty_session.pty.write(&bytes) {
                                eprintln!("PTY write error: {}", e);
                            }
                        }
                        InputAction::ScrollUp(lines) => {
                            active_pane_mut(&mut panes, &tab_bar)
                                .grid.borrow_mut().scroll_up(lines);
                        }
                        InputAction::ScrollDown(lines) => {
                            active_pane_mut(&mut panes, &tab_bar)
                                .grid.borrow_mut().scroll_down(lines);
                        }
                        InputAction::ScrollToTop => {
                            active_pane_mut(&mut panes, &tab_bar)
                                .grid.borrow_mut().scroll_to_top();
                        }
                        InputAction::ScrollToBottom => {
                            active_pane_mut(&mut panes, &tab_bar)
                                .grid.borrow_mut().scroll_to_bottom();
                        }
                        InputAction::Copy => {
                            let pane = active_pane_mut(&mut panes, &tab_bar);
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
                                    let pane = active_pane_mut(&mut panes, &tab_bar);
                                    let _ = pane.pty_session.pty.write(b"\x1b[200~");
                                    let _ = pane.pty_session.pty.write(text.as_bytes());
                                    let _ = pane.pty_session.pty.write(b"\x1b[201~");
                                }
                            }
                        }
                        InputAction::Ignore => {}
                    }
                }
            }

            _ => {}
        },
        Event::AboutToWait => {
            window.request_redraw();
        }
        _ => {}
    })?;

    Ok(())
}

fn change_font_size(
    state: &mut AppState,
    renderer: &mut luna_renderer::renderer::Renderer,
    panes: &mut [Pane],
    tab_bar: &TabBar,
    layout: &Layout,
    margin: f32,
    cell_w: &mut f32,
    cell_h: &mut f32,
    new_size: f32,
) {
    state.font_size = new_size;
    state.config.font_size = new_size;
    let _ = state.config.save();

    (*cell_w, *cell_h) = renderer.cell_metrics(new_size);

    let pane_area = layout.pane_area();
    let pane_rect = luna_ui::PaneRect {
        x: pane_area.0,
        y: pane_area.1,
        w: pane_area.2,
        h: pane_area.3,
    };
    let layouts = tab_bar.active_tab().pane_tree.get_layout(pane_rect);

    for (pane_id, rect) in &layouts {
        let new_cols = ((rect.w - margin * 2.0) / *cell_w).max(1.0) as usize;
        let new_rows = ((rect.h - margin * 2.0) / *cell_h).max(1.0) as usize;
        if let Some(pane) = panes.iter_mut().find(|p| p.id == *pane_id) {
            pane.cols = new_cols;
            pane.rows = new_rows;
            pane.grid.borrow_mut().resize(new_cols, new_rows);
            let _ = pane.pty_session.pty.resize(new_cols as u16, new_rows as u16);
        }
    }
}

fn create_pane(id: PaneId, cols: usize, rows: usize) -> Pane {
    create_pane_with_cwd(id, cols, rows, None)
}

fn create_pane_with_cwd(id: PaneId, cols: usize, rows: usize, cwd: Option<String>) -> Pane {
    use luna_terminal::grid::Grid;
    use std::cell::RefCell;
    use std::rc::Rc;

    let grid = Rc::new(RefCell::new(Grid::new(cols, rows)));
    let mut shell_config = luna_terminal::shell::detect_shell();
    if let Some(cwd) = cwd {
        shell_config.cwd = Some(cwd);
    }
    let pty_handle =
        luna_terminal::pty::PtyHandle::spawn(cols as u16, rows as u16, &shell_config)
            .expect("Failed to spawn PTY");
    let pty_session = luna_terminal::pty::PtyHandle::start_reader(pty_handle);
    Pane::new(id, pty_session, grid, cols, rows)
}

fn find_pane(panes: &[Pane], id: PaneId) -> Option<&Pane> {
    panes.iter().find(|p| p.id == id)
}

fn active_pane_mut<'a>(panes: &'a mut [Pane], tab_bar: &TabBar) -> &'a mut Pane {
    let active_id = tab_bar.active_tab().active_pane;
    panes
        .iter_mut()
        .find(|p| p.id == active_id)
        .expect("Active pane not found")
}

fn adjacent_pane(layouts: &[(PaneId, PaneRect)], from: PaneId, direction: &str) -> Option<PaneId> {
    let from_rect = layouts.iter().find(|(id, _)| *id == from)?.1;

    match direction {
        "right" => layouts
            .iter()
            .filter(|(id, r)| *id != from
                && r.x >= from_rect.x + from_rect.w - 1.0
                && r.y < from_rect.y + from_rect.h
                && r.y + r.h > from_rect.y)
            .min_by(|(_, a), (_, b)| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| *id),
        "left" => layouts
            .iter()
            .filter(|(id, r)| *id != from
                && r.x + r.w <= from_rect.x + 1.0
                && r.y < from_rect.y + from_rect.h
                && r.y + r.h > from_rect.y)
            .max_by(|(_, a), (_, b)| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| *id),
        "down" => layouts
            .iter()
            .filter(|(id, r)| *id != from
                && r.y >= from_rect.y + from_rect.h - 1.0
                && r.x < from_rect.x + from_rect.w
                && r.x + r.w > from_rect.x)
            .min_by(|(_, a), (_, b)| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| *id),
        "up" => layouts
            .iter()
            .filter(|(id, r)| *id != from
                && r.y + r.h <= from_rect.y + 1.0
                && r.x < from_rect.x + from_rect.w
                && r.x + r.w > from_rect.x)
            .max_by(|(_, a), (_, b)| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| *id),
        _ => None,
    }
}

fn handle_tab_click(tab_bar: &mut TabBar, panes: &mut Vec<Pane>, x: f64, window_width: f64) {
    let tab_count = tab_bar.tabs.len();
    let tab_w = (window_width - 56.0) / tab_count as f64;
    let tab_w = tab_w.min(200.0).max(80.0);

    // + button area (last 32px)
    if x >= tab_w * tab_count as f64 && x < tab_w * tab_count as f64 + 32.0 {
        // Approximate pane size for new tab
        let margin = 4.0;
        let cell_w = 8.4;
        let cell_h = 18.0;
        let pane_height = 800.0 - TAB_BAR_HEIGHT as f64 - margin * 2.0;
        let new_cols = ((window_width - margin * 2.0) / cell_w).max(1.0) as usize;
        let new_rows = ((pane_height) / cell_h).max(1.0) as usize;
        let (_, pane_id) = tab_bar.new_tab();
        panes.push(create_pane(pane_id, new_cols, new_rows));
        return;
    }

    // Tab click
    let clicked = (x / tab_w) as usize;
    if clicked < tab_count {
        tab_bar.activate(clicked);
    }
}

fn build_tab_bar_ui_rects(layout: &Layout, tab_bar: &TabBar) -> Vec<UIRect> {
    let mut rects = Vec::new();

    // Tab bar background
    rects.push(UIRect {
        pos: [0.0, 0.0],
        size: [layout.window_width, layout.tab_bar_height],
        color: theme::TAB_BAR_BG,
    });

    let tab_count = tab_bar.tabs.len();
    let tab_w = layout.tab_width(tab_count);

    for (i, _tab) in tab_bar.tabs.iter().enumerate() {
        let x = layout.tab_x(i, tab_count);
        let color = if i == tab_bar.active {
            theme::TAB_ACTIVE_BG
        } else {
            theme::TAB_INACTIVE_BG
        };

        rects.push(UIRect {
            pos: [x, 0.0],
            size: [tab_w, layout.tab_bar_height],
            color,
        });

        // Separator between tabs
        if i > 0 {
            rects.push(UIRect {
                pos: [x, 4.0],
                size: [1.0, layout.tab_bar_height - 8.0],
                color: theme::TAB_SEPARATOR,
            });
        }
    }

    // + button
    let plus_x = layout.tab_x(tab_count, tab_count);
    rects.push(UIRect {
        pos: [plus_x, 0.0],
        size: [32.0, layout.tab_bar_height],
        color: theme::TAB_BAR_BG,
    });

    rects
}

fn build_tab_bar_text(
    layout: &Layout,
    tab_bar: &TabBar,
    _scale_factor: f64,
) -> Vec<(char, f32, f32, f32, [f32; 4], [f32; 4])> {
    let mut result = Vec::new();
    let tab_count = tab_bar.tabs.len();
    let tab_w = layout.tab_width(tab_count);
    let char_w = TAB_FONT_SIZE * 0.6;
    let text_y = 8.0; // vertical offset within tab bar

    for (i, tab) in tab_bar.tabs.iter().enumerate() {
        let x = layout.tab_x(i, tab_count);
        let fg = if i == tab_bar.active {
            theme::TAB_TEXT
        } else {
            theme::TAB_TEXT_INACTIVE
        };
        let bg = if i == tab_bar.active {
            theme::TAB_ACTIVE_BG
        } else {
            theme::TAB_INACTIVE_BG
        };

        let title: String = if tab.title.is_empty() {
            format!("Tab {}", i + 1)
        } else {
            let max_chars = ((tab_w - 24.0) / char_w) as usize;
            if tab.title.chars().count() > max_chars.max(1) {
                tab.title.chars().take(max_chars.saturating_sub(1)).collect::<String>() + "…"
            } else {
                tab.title.clone()
            }
        };

        let text_x = x + 8.0;
        for (j, c) in title.chars().enumerate() {
            result.push((
                c,
                text_x + j as f32 * char_w,
                text_y,
                TAB_FONT_SIZE,
                fg,
                bg,
            ));
        }
    }

    // + text
    let plus_x = layout.tab_x(tab_count, tab_count);
    result.push((
        '+',
        plus_x + 8.0,
        text_y,
        TAB_FONT_SIZE,
        theme::TAB_BUTTON_TEXT,
        theme::TAB_BAR_BG,
    ));

    result
}

fn find_hovered_divider<'a>(
    dividers: &'a [luna_ui::DividerInfo],
    x: f64,
    y: f64,
) -> Option<&'a luna_ui::DividerInfo> {
    dividers.iter().find(|info| {
        let h = info.hitbox;
        x >= h.x as f64 && x <= (h.x + h.w) as f64 && y >= h.y as f64 && y <= (h.y + h.h) as f64
    })
}

fn extract_selection(grid: &luna_terminal::grid::Grid, sel: &state::Selection, cols: usize) -> String {
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

fn find_matches(grid: &luna_terminal::grid::Grid, term: &str) -> Vec<state::SearchMatch> {
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
            matches.push(state::SearchMatch {
                col: start + pos,
                row,
            });
            start += pos + 1;
        }
    }
    matches
}

fn build_match_set(matches: &[state::SearchMatch], term_len: usize) -> HashSet<(usize, usize)> {
    let mut set = HashSet::new();
    for m in matches {
        for i in 0..term_len {
            set.insert((m.col + i, m.row));
        }
    }
    set
}

fn update_search_matches(state: &mut AppState, tab_bar: &TabBar, panes: &[Pane]) {
    let pane = panes.iter().find(|p| p.id == tab_bar.active_tab().active_pane).unwrap();
    let grid = pane.grid.borrow();
    state.search.matches = find_matches(&grid, &state.search.term);
    if state.search.matches.is_empty() {
        state.search.current_match = 0;
    } else if state.search.current_match >= state.search.matches.len() {
        state.search.current_match = 0;
    }
}

fn scroll_to_current_match(state: &AppState, tab_bar: &TabBar, panes: &[Pane]) {
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

fn handle_search_input(
    key: &Key,
    event: &winit::event::KeyEvent,
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
            // Handle regular text input
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

fn handle_history_search_input(
    key: &Key,
    event: &winit::event::KeyEvent,
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
                // Ctrl+R cycles to next older match
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
