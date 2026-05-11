use std::collections::HashSet;

use luna_config::Theme;
use luna_renderer::{renderer::Renderer, ui::UIRect};
use luna_ui::{layout::Layout, pane::Pane, tab_bar::TabBar, theme, PaneId, SCROLL_BTN_W};

use crate::{app::CellData, pane_ops::find_pane, search::build_match_set, state::AppState};

const TAB_FONT_SIZE: f32 = 12.0;

pub fn build_tab_bar_ui_rects(
    layout: &Layout,
    tab_bar: &TabBar,
    hover_tab: Option<usize>,
    scroll_offset: usize,
    theme: &Theme,
) -> Vec<UIRect> {
    let mut rects = Vec::new();

    rects.push(UIRect {
        pos: [0.0, 0.0],
        size: [layout.window_width, layout.tab_bar_height],
        color: theme.tab_bar_bg,
    });

    let tab_count = tab_bar.tabs.len();
    let (start, end, show_left, show_right) = layout.tab_visible_range(tab_count, scroll_offset);
    let vis_count = end - start;
    let tab_w = layout.scrolled_tab_width(vis_count, show_left, show_right);
    let x_start = if show_left { SCROLL_BTN_W } else { 0.0 };

    // < scroll button
    if show_left {
        rects.push(UIRect {
            pos: [0.0, 0.0],
            size: [SCROLL_BTN_W, layout.tab_bar_height],
            color: theme.tab_bar_bg,
        });
    }

    for (vis_i, i) in (start..end).enumerate() {
        let x = x_start + vis_i as f32 * tab_w;
        let color = if i == tab_bar.active {
            theme.tab_active_bg
        } else {
            theme.tab_inactive_bg
        };
        rects.push(UIRect {
            pos: [x, 0.0],
            size: [tab_w, layout.tab_bar_height],
            color,
        });

        if hover_tab == Some(i) && i != tab_bar.active {
            rects.push(UIRect {
                pos: [x, 0.0],
                size: [tab_w, layout.tab_bar_height],
                color: theme.tab_hover_bg,
            });
        }

        if vis_i > 0 {
            rects.push(UIRect {
                pos: [x, 4.0],
                size: [1.0, layout.tab_bar_height - 8.0],
                color: theme.tab_separator,
            });
        }
    }

    // + button
    let plus_x = x_start + vis_count as f32 * tab_w;
    rects.push(UIRect {
        pos: [plus_x, 0.0],
        size: [32.0, layout.tab_bar_height],
        color: theme.tab_bar_bg,
    });

    // > scroll button
    if show_right {
        rects.push(UIRect {
            pos: [plus_x + 32.0, 0.0],
            size: [SCROLL_BTN_W, layout.tab_bar_height],
            color: theme.tab_bar_bg,
        });
    }

    rects
}

#[allow(clippy::type_complexity)]
pub fn build_tab_bar_text(
    layout: &Layout,
    tab_bar: &TabBar,
    _scale_factor: f64,
    scroll_offset: usize,
    theme: &Theme,
) -> Vec<(char, f32, f32, f32, [f32; 4], [f32; 4])> {
    let mut result = Vec::new();
    let tab_count = tab_bar.tabs.len();
    let (start, end, show_left, show_right) = layout.tab_visible_range(tab_count, scroll_offset);
    let vis_count = end - start;
    let tab_w = layout.scrolled_tab_width(vis_count, show_left, show_right);
    let x_start = if show_left { SCROLL_BTN_W } else { 0.0 };
    let char_w = TAB_FONT_SIZE * 0.6;
    let text_y = 8.0;

    // < button text
    if show_left {
        result.push((
            '<',
            4.0,
            text_y,
            TAB_FONT_SIZE,
            theme.tab_button_text,
            theme.tab_bar_bg,
        ));
    }

    for (vis_i, i) in (start..end).enumerate() {
        let tab = &tab_bar.tabs[i];
        let x = x_start + vis_i as f32 * tab_w;
        let fg = if i == tab_bar.active {
            theme.tab_text
        } else {
            theme.tab_text_inactive
        };
        let bg = if i == tab_bar.active {
            theme.tab_active_bg
        } else {
            theme.tab_inactive_bg
        };

        let raw_title: String = if !tab.title.is_empty() {
            tab.title.clone()
        } else if !tab.cwd.is_empty() {
            std::path::Path::new(&tab.cwd)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&tab.cwd)
                .to_string()
        } else {
            format!("Tab {}", i + 1)
        };
        let max_chars = ((tab_w - 24.0) / char_w).max(1.0) as usize;
        let title: String = if raw_title.chars().count() > max_chars {
            raw_title
                .chars()
                .take(max_chars.saturating_sub(1))
                .collect::<String>()
                + "…"
        } else {
            raw_title
        };

        let text_x = x + 8.0;
        for (j, c) in title.chars().enumerate() {
            result.push((c, text_x + j as f32 * char_w, text_y, TAB_FONT_SIZE, fg, bg));
        }

        let close_x = x + tab_w - 14.0;
        let close_fg = if i == tab_bar.active {
            [1.0, 1.0, 1.0, 0.7_f32]
        } else {
            [0.8, 0.8, 0.8, 0.5_f32]
        };
        result.push(('×', close_x, text_y, TAB_FONT_SIZE, close_fg, bg));
    }

    // + button
    let plus_x = x_start + vis_count as f32 * tab_w;
    result.push((
        '+',
        plus_x + 8.0,
        text_y,
        TAB_FONT_SIZE,
        theme.tab_button_text,
        theme.tab_bar_bg,
    ));

    // > button text
    if show_right {
        result.push((
            '>',
            plus_x + 32.0 + 4.0,
            text_y,
            TAB_FONT_SIZE,
            theme.tab_button_text,
            theme.tab_bar_bg,
        ));
    }

    result
}

#[allow(clippy::too_many_arguments, clippy::ptr_arg, clippy::type_complexity)]
pub fn render_frame(
    renderer: &mut Renderer,
    layout: &Layout,
    tab_bar: &mut TabBar,
    panes: &mut Vec<Pane>,
    state: &AppState,
    cell_w: f32,
    cell_h: f32,
    cursor_blink_on: bool,
    cached_cell_data: &mut CellData,
    cached_ui_rects: &mut Vec<UIRect>,
    cached_blink: &mut bool,
    cached_font_size: &mut f32,
    cached_active_tab: &mut usize,
) -> Vec<PaneId> {
    renderer.set_font_ligatures(state.config.font_ligatures);
    let font_size = state.font_size;
    let mut exited_panes: Vec<PaneId> = Vec::new();
    let mut pty_received = false;

    for pane in panes.iter_mut() {
        loop {
            match pane.pty_session.rx.try_recv() {
                Ok(Some(data)) => {
                    pane.processor.process(&data);
                    pty_received = true;
                    // Write any kitty protocol responses back to PTY
                    let responses = pane.processor.drain_kitty_responses();
                    for resp in responses {
                        let _ = pane.pty_session.pty.write(&resp);
                    }
                }
                Ok(None) => {
                    let responses = pane.processor.drain_kitty_responses();
                    for resp in responses {
                        let _ = pane.pty_session.pty.write(&resp);
                    }
                    exited_panes.push(pane.id);
                    break;
                }
                Err(_) => {
                    let responses = pane.processor.drain_kitty_responses();
                    for resp in responses {
                        let _ = pane.pty_session.pty.write(&resp);
                    }
                    break;
                }
            }
        }
    }

    for tab in tab_bar.tabs.iter_mut() {
        if let Some(p) = panes.iter().find(|p| p.id == tab.active_pane) {
            let t = p.title();
            if !t.is_empty() && t != tab.title {
                tab.title = t;
            }
            let c = p.cwd();
            if !c.is_empty() && c != tab.cwd {
                tab.cwd = c;
            }
        }
    }

    let any_grid_dirty = panes.iter().any(|p| p.grid.borrow().has_frame_dirty());
    let font_changed = (*cached_font_size - font_size).abs() > 0.01;
    let blink_changed = *cached_blink != cursor_blink_on;
    let tab_changed = tab_bar.active != *cached_active_tab;
    let ui_active = state.selecting || state.search.active || state.history_search.active;
    let first_frame = cached_cell_data.is_empty();

    let needs_rebuild = pty_received
        || any_grid_dirty
        || font_changed
        || blink_changed
        || tab_changed
        || ui_active
        || first_frame;

    if needs_rebuild {
        cached_cell_data.clear();
        cached_ui_rects.clear();

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

        let match_set: HashSet<(usize, usize)> =
            if state.search.active && !state.search.term.is_empty() {
                build_match_set(&state.search.matches, state.search.term.len())
            } else {
                HashSet::new()
            };

        for &(pane_id, rect) in &layouts {
            let pane = find_pane(panes, pane_id);
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
            let sb_visible =
                ((scroll_offset + pane_rows).min(scrollback_len)).saturating_sub(scroll_offset);
            let scrolled = scroll_offset > 0;
            let is_active = pane_id == active_pane_id;

            for (col, vrow, cell) in grid_ref.visible_cells_bounded(pane_rows, pane_cols) {
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
                    && state
                        .selection
                        .as_ref()
                        .is_some_and(|s| s.contains(col, vrow));
                let match_is_current = state.search.active
                    && !state.search.term.is_empty()
                    && !state.search.matches.is_empty()
                    && {
                        let cm = &state.search.matches[state.search.current_match];
                        cm.row == global_row
                            && col >= cm.col
                            && col < cm.col + state.search.term.len()
                    };
                let match_is_in = match_set.contains(&(col, global_row));
                let bg = if selection_bg {
                    state.theme.selection
                } else if match_is_current {
                    state.theme.search_current
                } else if match_is_in {
                    state.theme.search_highlight
                } else {
                    cell.bg.bg_rgba()
                };

                cached_cell_data.push((cell.c, x, y, font_size, cell.fg.fg_rgba(), bg));
            }

            if is_active
                && !scrolled
                && cursor_col < pane_cols
                && cursor_row < pane_rows
                && cursor_blink_on
            {
                let cx = content_x + cursor_col as f32 * cell_w;
                let cy = content_y + cursor_row as f32 * cell_h;
                let cursor_color = state.theme.cursor;
                match state.config.cursor_style {
                    luna_config::CursorStyle::Block => {
                        let cell = grid_ref.get(cursor_col, cursor_row);
                        let cursor_fg = [0.13, 0.04, 0.29, 1.0];
                        cached_cell_data.push((cell.c, cx, cy, font_size, cursor_fg, cursor_color));
                    }
                    luna_config::CursorStyle::Beam => {
                        cached_ui_rects.push(UIRect {
                            pos: [cx, cy],
                            size: [1.5, cell_h],
                            color: cursor_color,
                        });
                    }
                    luna_config::CursorStyle::Underline => {
                        cached_ui_rects.push(UIRect {
                            pos: [cx, cy + cell_h - 2.0],
                            size: [cell_w, 2.0],
                            color: cursor_color,
                        });
                    }
                }
            }
            drop(grid_ref);

            let border_color = if is_active {
                state.theme.panel_active_border
            } else {
                state.theme.panel_inactive_border
            };
            cached_ui_rects.push(UIRect {
                pos: [rect.x, rect.y],
                size: [rect.w, 1.0],
                color: border_color,
            });
            cached_ui_rects.push(UIRect {
                pos: [rect.x, rect.y + rect.h - 1.0],
                size: [rect.w, 1.0],
                color: border_color,
            });
            cached_ui_rects.push(UIRect {
                pos: [rect.x, rect.y],
                size: [1.0, rect.h],
                color: border_color,
            });
            cached_ui_rects.push(UIRect {
                pos: [rect.x + rect.w - 1.0, rect.y],
                size: [1.0, rect.h],
                color: border_color,
            });
        }

        for info in &dividers {
            let d = info.hitbox;
            let (dx, dy, dw, dh) = if d.w > d.h {
                (d.x, d.y + 2.0, d.w, 2.0)
            } else {
                (d.x + 2.0, d.y, 2.0, d.h)
            };
            cached_ui_rects.push(UIRect {
                pos: [dx, dy],
                size: [dw, dh],
                color: state.theme.panel_divider,
            });
        }

        // Search bar overlay
        if state.search.active {
            let pane_area = layout.pane_area();
            let bar_x = pane_area.0;
            let bar_y = pane_area.1;
            let bar_w = pane_area.2;
            let bar_h = theme::SEARCH_BAR_HEIGHT;

            cached_ui_rects.push(UIRect {
                pos: [bar_x, bar_y],
                size: [bar_w, bar_h],
                color: state.theme.search_bar_bg,
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
                cached_cell_data.push((
                    c,
                    text_x + j as f32 * char_w,
                    text_y,
                    search_fs,
                    state.theme.search_text_dim,
                    transparent,
                ));
            }
            for (j, &c) in term_chars.iter().enumerate() {
                cached_cell_data.push((
                    c,
                    text_x + (prefix_chars.len() + j) as f32 * char_w,
                    text_y,
                    search_fs,
                    state.theme.search_text,
                    transparent,
                ));
            }
            let cursor_col = prefix_chars.len() + state.search.cursor_pos;
            cached_cell_data.push((
                '|',
                text_x + cursor_col as f32 * char_w,
                text_y,
                search_fs,
                state.theme.search_text,
                transparent,
            ));

            let counter = if state.search.term.is_empty() {
                String::new()
            } else if state.search.matches.is_empty() {
                "0 matches".to_string()
            } else {
                format!(
                    "{}/{}",
                    state.search.current_match + 1,
                    state.search.matches.len()
                )
            };
            if !counter.is_empty() {
                let counter_x = bar_x + bar_w - counter.len() as f32 * char_w - 8.0;
                for (j, c) in counter.chars().enumerate() {
                    cached_cell_data.push((
                        c,
                        counter_x + j as f32 * char_w,
                        text_y,
                        search_fs,
                        state.theme.search_text_dim,
                        transparent,
                    ));
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

            cached_ui_rects.push(UIRect {
                pos: [bar_x, bar_y],
                size: [bar_w, bar_h],
                color: state.theme.search_bar_bg,
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
                cached_cell_data.push((
                    c,
                    text_x + j as f32 * char_w,
                    text_y,
                    search_fs,
                    state.theme.search_text,
                    transparent,
                ));
            }

            if !state.history_search.matches.is_empty() {
                let counter = format!(
                    "{}/{}",
                    state.history_search.current_match + 1,
                    state.history_search.matches.len()
                );
                let cx = bar_x + bar_w - counter.len() as f32 * char_w - 8.0;
                for (j, c) in counter.chars().enumerate() {
                    cached_cell_data.push((
                        c,
                        cx + j as f32 * char_w,
                        text_y,
                        search_fs,
                        state.theme.search_text_dim,
                        transparent,
                    ));
                }
            }
        }

        let tab_ui = build_tab_bar_ui_rects(
            layout,
            tab_bar,
            state.hover_tab,
            state.tab_scroll_offset,
            &state.theme,
        );
        cached_ui_rects.extend(tab_ui);

        for tab_cell in
            build_tab_bar_text(layout, tab_bar, 1.0, state.tab_scroll_offset, &state.theme)
        {
            cached_cell_data.push(tab_cell);
        }

        *cached_font_size = font_size;
        *cached_blink = cursor_blink_on;
        *cached_active_tab = tab_bar.active;
    }

    for pane in panes.iter_mut() {
        pane.grid.borrow_mut().clear_frame_dirty();
        pane.grid.borrow_mut().clear_dirty();
    }

    renderer.draw_frame(cached_cell_data, cached_ui_rects);

    exited_panes
}

use crate::app::App;
use crate::pane_ops::create_pane;

impl App {
    pub(crate) fn render(&mut self) {
        if self.state.config.cursor_blink {
            let blink_ms = self.state.config.cursor_blink_ms;
            if self.last_blink.elapsed() >= std::time::Duration::from_millis(blink_ms) {
                self.cursor_blink_on = !self.cursor_blink_on;
                self.last_blink = std::time::Instant::now();
            }
        } else {
            self.cursor_blink_on = true;
        }

        let exited = render_frame(
            &mut self.renderer,
            &self.layout,
            &mut self.tab_bar,
            &mut self.panes,
            &self.state,
            self.cell_w,
            self.cell_h,
            self.cursor_blink_on,
            &mut self.cached_cell_data,
            &mut self.cached_ui_rects,
            &mut self.cached_blink,
            &mut self.cached_font_size,
            &mut self.cached_active_tab,
        );

        for pane_id in exited {
            self.handle_pane_exit(pane_id);
            self.cached_cell_data.clear();
            self.cached_ui_rects.clear();
        }
    }

    fn handle_pane_exit(&mut self, pane_id: luna_ui::PaneId) {
        if let Some(pane) = self.panes.iter_mut().find(|p| p.id == pane_id) {
            let _ = pane.pty_session.pty.kill();
        }

        let tab_idx = self
            .tab_bar
            .tabs
            .iter()
            .position(|t| t.pane_tree.all_panes().contains(&pane_id));

        if let Some(idx) = tab_idx {
            let pane_count = self.tab_bar.tabs[idx].pane_tree.all_panes().len();

            if pane_count == 1 {
                if self.tab_bar.tabs.len() == 1 {
                    let pane_area = self.layout.pane_area();
                    let new_cols =
                        ((pane_area.2 - self.margin * 2.0) / self.cell_w).max(1.0) as usize;
                    let new_rows =
                        ((pane_area.3 - self.margin * 2.0) / self.cell_h).max(1.0) as usize;
                    let new_pane_id = self.tab_bar.next_pane_id();
                    self.tab_bar.tabs[0].pane_tree = luna_ui::PaneTree::leaf(new_pane_id);
                    self.tab_bar.tabs[0].active_pane = new_pane_id;
                    self.panes
                        .push(create_pane(new_pane_id, new_cols, new_rows));
                } else {
                    self.tab_bar.close_tab(idx);
                }
            } else {
                self.tab_bar.tabs[idx].pane_tree.close(pane_id);
                if self.tab_bar.tabs[idx].active_pane == pane_id {
                    let first = self.tab_bar.tabs[idx].pane_tree.all_panes()[0];
                    self.tab_bar.tabs[idx].active_pane = first;
                }
            }
        }

        self.panes.retain(|p| p.id != pane_id);
    }
}
