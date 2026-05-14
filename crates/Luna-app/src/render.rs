use std::collections::HashSet;

use alacritty_terminal::vte::ansi::Color as TermColor;
use luna_config::Theme;
use luna_renderer::{renderer::Renderer, ui::UIRect};
use luna_ui::{layout::Layout, pane::Pane, tab_bar::TabBar, theme, PaneId, SCROLL_BTN_W};

use crate::{app::CellData, pane_ops::find_pane, search::build_match_set, state::AppState};

const TAB_FONT_SIZE: f32 = 12.0;

fn xterm256_to_rgba(idx: u8) -> [f32; 4] {
    let rgb: [u8; 3] = match idx {
        0 => [0, 0, 0],
        1 => [128, 0, 0],
        2 => [0, 128, 0],
        3 => [128, 128, 0],
        4 => [0, 0, 128],
        5 => [128, 0, 128],
        6 => [0, 128, 128],
        7 => [192, 192, 192],
        8 => [128, 128, 128],
        9 => [255, 0, 0],
        10 => [0, 255, 0],
        11 => [255, 255, 0],
        12 => [0, 0, 255],
        13 => [255, 0, 255],
        14 => [0, 255, 255],
        15 => [255, 255, 255],
        16..=231 => {
            let n = idx - 16;
            let b = (n % 6) * 51;
            let g = ((n / 6) % 6) * 51;
            let r = (n / 36) * 51;
            [r, g, b]
        }
        232..=255 => {
            let v = (idx - 232) * 10 + 8;
            [v, v, v]
        }
    };
    [
        rgb[0] as f32 / 255.0,
        rgb[1] as f32 / 255.0,
        rgb[2] as f32 / 255.0,
        1.0,
    ]
}

fn named_color_to_rgba(
    nc: alacritty_terminal::vte::ansi::NamedColor,
    fg: [f32; 4],
    bg: [f32; 4],
) -> [f32; 4] {
    use alacritty_terminal::vte::ansi::NamedColor::*;
    // Standard xterm 16-color ANSI palette.
    match nc {
        Black => [0.000, 0.000, 0.000, 1.0],
        Red => [0.800, 0.000, 0.000, 1.0],
        Green => [0.306, 0.604, 0.024, 1.0],
        Yellow => [0.769, 0.627, 0.000, 1.0],
        Blue => [0.204, 0.396, 0.643, 1.0],
        Magenta => [0.459, 0.314, 0.482, 1.0],
        Cyan => [0.024, 0.596, 0.604, 1.0],
        White => [0.827, 0.843, 0.812, 1.0],
        BrightBlack => [0.333, 0.341, 0.325, 1.0],
        BrightRed => [0.937, 0.161, 0.161, 1.0],
        BrightGreen => [0.541, 0.886, 0.204, 1.0],
        BrightYellow => [0.988, 0.914, 0.310, 1.0],
        BrightBlue => [0.447, 0.624, 0.812, 1.0],
        BrightMagenta => [0.678, 0.498, 0.659, 1.0],
        BrightCyan => [0.204, 0.886, 0.886, 1.0],
        BrightWhite => [0.933, 0.933, 0.925, 1.0],
        // Semantic aliases — use the caller-supplied fg/bg.
        Foreground | BrightForeground | DimForeground => fg,
        Background => bg,
        // Dim variants: darken the normal color by ~50%.
        DimBlack => [0.000, 0.000, 0.000, 1.0],
        DimRed => [0.400, 0.000, 0.000, 1.0],
        DimGreen => [0.153, 0.302, 0.012, 1.0],
        DimYellow => [0.385, 0.314, 0.000, 1.0],
        DimBlue => [0.102, 0.198, 0.322, 1.0],
        DimMagenta => [0.230, 0.157, 0.241, 1.0],
        DimCyan => [0.012, 0.298, 0.302, 1.0],
        DimWhite => [0.414, 0.422, 0.406, 1.0],
        // Cursor / other terminal-managed colors: fall back to fg.
        Cursor => fg,
    }
}

fn term_color_to_rgba(color: TermColor, fallback: [f32; 4]) -> [f32; 4] {
    match color {
        TermColor::Spec(rgb) => [
            rgb.r as f32 / 255.0,
            rgb.g as f32 / 255.0,
            rgb.b as f32 / 255.0,
            1.0,
        ],
        TermColor::Named(nc) => named_color_to_rgba(nc, fallback, [0.067, 0.075, 0.102, 1.0]),
        TermColor::Indexed(idx) => xterm256_to_rgba(idx),
    }
}

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
    cached_bg_rects: &mut Vec<UIRect>,
    cached_blink: &mut bool,
    cached_font_size: &mut f32,
    cached_active_tab: &mut usize,
    effective_font_size: f32,
) -> Vec<PaneId> {
    let font_size = effective_font_size;

    // PTY parsing happens on per-pane reader threads. We just check the
    // dirty flag here to know whether to rebuild the frame.
    let pty_received = panes.iter_mut().any(|pane| pane.is_dirty());

    // Phase 1: pane-exit detection through alacritty_terminal is not wired up.
    let exited_panes: Vec<PaneId> = Vec::new();

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

    let font_changed = (*cached_font_size - font_size).abs() > 0.01;
    let blink_changed = *cached_blink != cursor_blink_on;
    let tab_changed = tab_bar.active != *cached_active_tab;
    let ui_active = state.selecting || state.search.active || state.history_search.active;
    let first_frame = cached_cell_data.is_empty();

    let needs_rebuild = pty_received
        || font_changed
        || blink_changed
        || tab_changed
        || ui_active
        || first_frame;

    if needs_rebuild {
        cached_cell_data.clear();
        cached_ui_rects.clear();
        cached_bg_rects.clear();

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

        let _match_set: HashSet<(usize, usize)> =
            if state.search.active && !state.search.term.is_empty() {
                build_match_set(&state.search.matches, state.search.term.len())
            } else {
                HashSet::new()
            };

        for &(pane_id, rect) in &layouts {
            let pane = match find_pane(panes, pane_id) {
                Some(p) => p,
                None => continue,
            };

            let content_x = rect.x + margin;
            let content_y = rect.y + margin;
            let content_w = (rect.w - margin * 2.0).max(0.0);
            let content_h = (rect.h - margin * 2.0).max(0.0);

            let pane_cols = ((content_w) / cell_w).max(1.0) as usize;
            let pane_rows = ((content_h) / cell_h).max(1.0) as usize;

            let is_active = pane_id == active_pane_id;

            // Snapshot grid contents and cursor under the lock, then release it.
            let (cells, cursor_col, cursor_row): (
                Vec<(usize, i32, char, TermColor, TermColor)>,
                usize,
                i32,
            ) = {
                let term = match pane.term.lock() {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let grid = term.grid();
                let cursor_point = grid.cursor.point;
                let cursor_col = cursor_point.column.0;
                let cursor_row = cursor_point.line.0;

                let mut buf = Vec::with_capacity(pane_cols * pane_rows);
                for indexed in grid.display_iter() {
                    let col = indexed.point.column.0;
                    let row_val = indexed.point.line.0;
                    if row_val < 0 || row_val as usize >= pane_rows {
                        continue;
                    }
                    if col >= pane_cols {
                        continue;
                    }
                    buf.push((col, row_val, indexed.c, indexed.fg, indexed.bg));
                }
                (buf, cursor_col, cursor_row)
            };

            for (col, row_val, ch, fg_c, bg_c) in cells {
                let x = content_x + col as f32 * cell_w;
                let y = content_y + row_val as f32 * cell_h;

                let fg = term_color_to_rgba(fg_c, state.theme.fg);
                let bg = term_color_to_rgba(bg_c, state.theme.bg);

                let is_cursor =
                    is_active && col == cursor_col && row_val == cursor_row && cursor_blink_on;

                let (final_fg, final_bg) = if is_cursor {
                    ([0.067, 0.075, 0.102, 1.0], state.theme.cursor)
                } else {
                    (fg, bg)
                };

                let bg_is_default = !is_cursor && matches!(bg_c, TermColor::Named(_));

                if !bg_is_default {
                    cached_bg_rects.push(UIRect {
                        pos: [x, y],
                        size: [cell_w, cell_h],
                        color: final_bg,
                    });
                }

                if ch != ' ' {
                    cached_cell_data.push((ch, x, y, font_size, final_fg, final_bg));
                }
            }

            // Cursor styles (beam / underline). The block style is drawn inline above.
            if is_active
                && cursor_blink_on
                && cursor_row >= 0
                && (cursor_row as usize) < pane_rows
                && cursor_col < pane_cols
            {
                let cx = content_x + cursor_col as f32 * cell_w;
                let cy = content_y + cursor_row as f32 * cell_h;
                let cursor_color = state.theme.cursor;
                match state.config.cursor_style {
                    luna_config::CursorStyle::Block => {
                        // Block cursor is rendered via the per-cell pass above.
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

    renderer.draw_frame(cached_cell_data, cached_ui_rects, cached_bg_rects);

    exited_panes
}

use crate::app::App;
use crate::pane_ops::create_pane;

impl App {
    pub(crate) fn render(&mut self) {
        self.frame_count += 1;
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.fps_last_print);
        if elapsed >= std::time::Duration::from_secs(1) {
            let fps = self.frame_count as f64 / elapsed.as_secs_f64();
            tracing::info!(target: "luna::bench", "FPS: {:.1}", fps);
            self.frame_count = 0;
            self.fps_last_print = now;
        }

        if self.state.config.cursor_blink {
            let blink_ms = self.state.config.cursor_blink_ms;
            if self.last_blink.elapsed() >= std::time::Duration::from_millis(blink_ms) {
                self.cursor_blink_on = !self.cursor_blink_on;
                self.last_blink = std::time::Instant::now();
            }
        } else {
            self.cursor_blink_on = true;
        }

        let effective_fs = self.state.font_size * self.scale_factor;
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
            &mut self.cached_bg_rects,
            &mut self.cached_blink,
            &mut self.cached_font_size,
            &mut self.cached_active_tab,
            effective_fs,
        );

        for pane_id in exited {
            self.handle_pane_exit(pane_id);
            self.cached_cell_data.clear();
            self.cached_ui_rects.clear();
        }
    }

    fn handle_pane_exit(&mut self, pane_id: luna_ui::PaneId) {
        // Phase 1: exited_panes is always empty so this path is unused; the
        // implementation is kept so we can wire alacritty exit events later
        // without redoing the tab/pane rebalancing logic.
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
                    match create_pane(new_pane_id, new_cols, new_rows) {
                        Ok(pane) => self.panes.push(pane),
                        Err(e) => tracing::warn!("Failed to spawn replacement PTY: {}", e),
                    }
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
