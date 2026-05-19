use std::collections::HashSet;

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::vte::ansi::Color as TermColor;
use synapse_config::Theme;
use synapse_renderer::{image::ImageInstance, renderer::Renderer, ui::UIRect};
use synapse_ui::{layout::Layout, pane::Pane, tab_bar::TabBar, theme, PaneId, SCROLL_BTN_W};

use crate::app::CellData;
use crate::image_protocol::ImageStore;
use crate::pane_ops::find_pane;
use crate::search::build_match_set;
use crate::state::{AppState, UrlSpan};

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

/// Devuelve c con la luminosidad reducida al 50% (colores "dim" ANSI).
fn dim_color(c: [f32; 4]) -> [f32; 4] {
    [c[0] * 0.5, c[1] * 0.5, c[2] * 0.5, c[3]]
}

/// Convierte un NamedColor ANSI a RGBA usando la paleta de 16 colores del tema.
/// `fallback` se usa para Foreground/Background/Cursor (debe ser theme.fg o theme.bg
/// según el contexto donde se invoca).
fn named_color_to_rgba(
    nc: alacritty_terminal::vte::ansi::NamedColor,
    ansi: &[[f32; 4]; 16],
    fallback: [f32; 4],
) -> [f32; 4] {
    use alacritty_terminal::vte::ansi::NamedColor::*;
    match nc {
        // Colores normales 0-7 → paleta ANSI del tema
        Black   => ansi[0],
        Red     => ansi[1],
        Green   => ansi[2],
        Yellow  => ansi[3],
        Blue    => ansi[4],
        Magenta => ansi[5],
        Cyan    => ansi[6],
        White   => ansi[7],
        // Colores brillantes 8-15
        BrightBlack   => ansi[8],
        BrightRed     => ansi[9],
        BrightGreen   => ansi[10],
        BrightYellow  => ansi[11],
        BrightBlue    => ansi[12],
        BrightMagenta => ansi[13],
        BrightCyan    => ansi[14],
        BrightWhite   => ansi[15],
        // Semánticos → usan el fallback (fg o bg según contexto)
        Foreground | BrightForeground | DimForeground => fallback,
        Background => fallback,
        // Dim: versión oscurecida de los colores normales del tema
        DimBlack   => dim_color(ansi[0]),
        DimRed     => dim_color(ansi[1]),
        DimGreen   => dim_color(ansi[2]),
        DimYellow  => dim_color(ansi[3]),
        DimBlue    => dim_color(ansi[4]),
        DimMagenta => dim_color(ansi[5]),
        DimCyan    => dim_color(ansi[6]),
        DimWhite   => dim_color(ansi[7]),
        // Cursor → fallback
        Cursor => fallback,
    }
}

fn term_color_to_rgba(color: TermColor, fallback: [f32; 4], ansi: &[[f32; 4]; 16]) -> [f32; 4] {
    match color {
        TermColor::Spec(rgb) => [
            rgb.r as f32 / 255.0,
            rgb.g as f32 / 255.0,
            rgb.b as f32 / 255.0,
            1.0,
        ],
        // NamedColor usa la paleta del tema
        TermColor::Named(nc) => named_color_to_rgba(nc, ansi, fallback),
        // Indexed 0-15 también usa la paleta del tema; 16-255 usa xterm estándar
        TermColor::Indexed(idx) => {
            if (idx as usize) < 16 {
                ansi[idx as usize]
            } else {
                xterm256_to_rgba(idx)
            }
        }
    }
}

fn has_prefix_at(chars: &[char], pos: usize, prefix: &str) -> bool {
    let pchars: Vec<char> = prefix.chars().collect();
    pos + pchars.len() <= chars.len()
        && chars[pos..pos + pchars.len()].iter().zip(&pchars).all(|(a, b)| a == b)
}

/// Scan visible terminal rows for bare `http://` / `https://` URLs.
/// Returns `(col_start, raw_row, col_end_exclusive, url_string)`.
fn detect_auto_urls(
    cells: &[(usize, i32, char, TermColor, TermColor)],
    display_offset: usize,
    pane_rows: usize,
) -> Vec<(usize, i32, usize, String)> {
    let mut rows: std::collections::BTreeMap<i32, Vec<(usize, char)>> = Default::default();
    for &(col, raw_row, ch, _, _) in cells {
        let vrow = raw_row + display_offset as i32;
        if vrow >= 0 && (vrow as usize) < pane_rows {
            rows.entry(raw_row).or_default().push((col, ch));
        }
    }

    let mut spans = Vec::new();
    for (raw_row, mut row_cells) in rows {
        row_cells.sort_by_key(|&(col, _)| col);
        let chars: Vec<char> = row_cells.iter().map(|&(_, c)| c).collect();
        let cols: Vec<usize> = row_cells.iter().map(|&(col, _)| col).collect();
        let n = chars.len();

        let mut i = 0;
        while i < n {
            let pfx_len = if has_prefix_at(&chars, i, "https://") {
                8
            } else if has_prefix_at(&chars, i, "http://") {
                7
            } else {
                i += 1;
                continue;
            };

            let mut end = i + pfx_len;
            while end < n && !chars[end].is_whitespace() {
                end += 1;
            }
            if end > i + pfx_len {
                let url: String = chars[i..end].iter().collect();
                let col_start = cols[i];
                let col_end = cols[end - 1] + 1;
                spans.push((col_start, raw_row, col_end, url));
            }
            i = end.max(i + 1);
        }
    }
    spans
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

        // Separator between tabs: skip the one adjacent to the active tab,
        // because the active tab's darker background creates a visual
        // "notch" / triangular cut that looks like a bowtie. The contrast
        // between tab_active_bg and tab_inactive_bg already separates them
        // visually without needing a line.
        if vis_i > 0 && i != tab_bar.active && (i - 1) != tab_bar.active {
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
    scale_factor: f64,
    scroll_offset: usize,
    theme: &Theme,
) -> Vec<(char, f32, f32, f32, [f32; 4], [f32; 4])> {
    let mut result = Vec::new();
    let tab_count = tab_bar.tabs.len();
    let (start, end, show_left, show_right) = layout.tab_visible_range(tab_count, scroll_offset);
    let vis_count = end - start;
    let tab_w = layout.scrolled_tab_width(vis_count, show_left, show_right);
    let x_start = if show_left { SCROLL_BTN_W } else { 0.0 };
    let tab_font_size = TAB_FONT_SIZE * scale_factor as f32;
    let char_w = tab_font_size * 0.6;
    let text_y = 8.0 * scale_factor as f32;

    // < button text
    if show_left {
        result.push((
            '<',
            4.0,
            text_y,
            tab_font_size,
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
            result.push((c, text_x + j as f32 * char_w, text_y, tab_font_size, fg, bg));
        }

        let close_x = x + tab_w - 14.0;
        let close_fg = if i == tab_bar.active {
            [1.0, 1.0, 1.0, 0.7_f32]
        } else {
            [0.8, 0.8, 0.8, 0.5_f32]
        };
        result.push(('×', close_x, text_y, tab_font_size, close_fg, bg));
    }

    // + button
    let plus_x = x_start + vis_count as f32 * tab_w;
    result.push((
        '+',
        plus_x + 8.0,
        text_y,
        tab_font_size,
        theme.tab_button_text,
        theme.tab_bar_bg,
    ));

    // > button text
    if show_right {
        result.push((
            '>',
            plus_x + 32.0 + 4.0,
            text_y,
            tab_font_size,
            theme.tab_button_text,
            theme.tab_bar_bg,
        ));
    }

    result
}

#[allow(clippy::too_many_arguments, clippy::ptr_arg, clippy::type_complexity)]
fn push_cursor_rect(
    ui_rects: &mut Vec<UIRect>,
    cursor_pixel: Option<(f32, f32)>,
    cursor_blink_on: bool,
    cell_w: f32,
    cell_h: f32,
    state: &AppState,
) {
    if !cursor_blink_on {
        return;
    }
    let (cx, cy) = match cursor_pixel {
        Some(p) => p,
        None => return,
    };
    let color = state.theme.cursor;
    match state.config.cursor_style {
        synapse_config::CursorStyle::Block => {
            ui_rects.push(UIRect { pos: [cx, cy], size: [cell_w, cell_h], color });
        }
        synapse_config::CursorStyle::Beam => {
            ui_rects.push(UIRect { pos: [cx, cy], size: [1.5, cell_h], color });
        }
        synapse_config::CursorStyle::Underline => {
            ui_rects.push(UIRect { pos: [cx, cy + cell_h - 2.0], size: [cell_w, 2.0], color });
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::ptr_arg, clippy::type_complexity)]
pub fn render_frame(
    renderer: &mut Renderer,
    layout: &Layout,
    tab_bar: &mut TabBar,
    panes: &mut Vec<Pane>,
    image_store: &ImageStore,
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
    cached_cursor_rects_start: &mut usize,
    cached_cursor_pixel: &mut Option<(f32, f32)>,
    cached_url_spans: &mut Vec<UrlSpan>,
    effective_font_size: f32,
    scale_factor: f32,
) -> Vec<PaneId> {
    let font_size = effective_font_size;

    // Drain event channels: pick up exit signals and title updates, then
    // check the dirty flag for PTY output.
    let exited_panes: Vec<PaneId> = panes
        .iter_mut()
        .filter_map(|p| if p.poll_events() { Some(p.id) } else { None })
        .collect();

    // 4.3: drain dirty for all panes but only trigger rebuild for active-tab panes.
    let active_ids: HashSet<PaneId> = tab_bar.active_tab().pane_tree.all_panes().into_iter().collect();
    let pty_received = panes.iter_mut().fold(false, |acc, pane| {
        let dirty = pane.is_dirty();
        acc || (dirty && active_ids.contains(&pane.id))
    });

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
    let ui_active = state.selecting || state.search.active || state.history_search.active || state.suggest.ghost.is_some();
    let first_frame = cached_cell_data.is_empty();

    // Cell data only changes on real terminal events — not on cursor blink.
    let needs_cell_rebuild = pty_received || font_changed || tab_changed || first_frame || ui_active;
    // UI rects (cursor shape) also change on blink.
    let needs_ui_rebuild = needs_cell_rebuild || blink_changed;

    if needs_cell_rebuild {
        cached_cell_data.clear();
        cached_ui_rects.clear();
        cached_bg_rects.clear();
        cached_url_spans.clear();

        // Cursor pixel position computed during pane iteration, used at end of rebuild.
        let mut cursor_pixel_for_frame: Option<(f32, f32)> = None;

        let active_pane_id = tab_bar.active_tab().active_pane;
        let pane_tree = &tab_bar.active_tab().pane_tree;

        let pane_area = layout.pane_area();
        let margin = layout.pane_margin();
        let pane_rect = synapse_ui::PaneRect {
            x: pane_area.0,
            y: pane_area.1,
            w: pane_area.2,
            h: pane_area.3,
        };

        let layouts = pane_tree.get_layout(pane_rect);
        let dividers = pane_tree.get_dividers(pane_rect);

        let match_set: HashSet<(usize, i32)> =
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

            // Snapshot grid contents, cursor, selection range, and OSC 8 hyperlinks under the lock.
            let (cells, osc8_cells, cursor_col, cursor_row, sel_range, display_offset, history_size): (
                Vec<(usize, i32, char, TermColor, TermColor)>,
                Vec<(usize, i32, String)>,
                usize,
                i32,
                Option<alacritty_terminal::selection::SelectionRange>,
                usize,
                usize,
            ) = {
                let term = match pane.term.lock() {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let grid = term.grid();
                let cursor_point = grid.cursor.point;
                let cursor_col = cursor_point.column.0;
                let cursor_row = cursor_point.line.0;

                let sel_range = term.selection.as_ref().and_then(|s| s.to_range(&*term));
                let display_offset = grid.display_offset();
                let history_size = grid.history_size();

                let mut buf = Vec::with_capacity(pane_cols * pane_rows);
                let mut hyperlinks: Vec<(usize, i32, String)> = Vec::new();
                for indexed in grid.display_iter() {
                    let col = indexed.point.column.0;
                    let raw_row = indexed.point.line.0;
                    // Shift by display_offset to get the viewport row (0 = top of display).
                    let viewport_row = raw_row + display_offset as i32;
                    if viewport_row < 0 || viewport_row as usize >= pane_rows {
                        continue;
                    }
                    if col >= pane_cols {
                        continue;
                    }
                    if let Some(hl) = indexed.hyperlink() {
                        hyperlinks.push((col, raw_row, hl.uri().to_string()));
                    }
                    buf.push((col, raw_row, indexed.c, indexed.fg, indexed.bg));
                }
                (buf, hyperlinks, cursor_col, cursor_row, sel_range, display_offset, history_size)
            };

            // Pre-compute URL spans before consuming `cells` in the render loop.
            let mut url_span_list: Vec<(usize, i32, usize, String)> = Vec::new();
            {
                if !osc8_cells.is_empty() {
                    let mut sorted = osc8_cells.clone();
                    sorted.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
                    let mut k = 0;
                    while k < sorted.len() {
                        let col_start = sorted[k].0;
                        let row = sorted[k].1;
                        let url = sorted[k].2.clone();
                        let mut col_end = col_start + 1;
                        let mut j = k + 1;
                        while j < sorted.len()
                            && sorted[j].1 == row
                            && sorted[j].2 == url
                            && sorted[j].0 == col_end
                        {
                            col_end += 1;
                            j += 1;
                        }
                        url_span_list.push((col_start, row, col_end, url));
                        k = j;
                    }
                }
                url_span_list.extend(detect_auto_urls(&cells, display_offset, pane_rows));
            }

            for (col, raw_row, ch, fg_c, bg_c) in cells {
                let viewport_row = raw_row + display_offset as i32;
                let x = content_x + col as f32 * cell_w;
                let y = content_y + viewport_row as f32 * cell_h;

                let fg = term_color_to_rgba(fg_c, state.theme.fg, &state.theme.ansi_colors);
                let bg = term_color_to_rgba(bg_c, state.theme.bg, &state.theme.ansi_colors);

                let in_selection = is_active
                    && sel_range.as_ref().map(|r| {
                        r.contains(alacritty_terminal::index::Point::new(
                            alacritty_terminal::index::Line(raw_row),
                            alacritty_terminal::index::Column(col),
                        ))
                    }).unwrap_or(false);

                let in_match = is_active && match_set.contains(&(col, raw_row));

                // Block cursor is now a UIRect overlay — cell uses its natural colors.
                let (final_fg, final_bg) = if in_selection {
                    (fg, state.theme.selection)
                } else if in_match {
                    (state.theme.search_current, state.theme.search_highlight)
                } else {
                    (fg, bg)
                };

                let bg_is_default =
                    !in_selection && !in_match && matches!(bg_c, TermColor::Named(_));

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

            // Track cursor pixel position for unified rendering after all panes.
            // All cursor styles (block/beam/underline) are now UIRect overlays.
            let cursor_viewport_row = cursor_row + display_offset as i32;
            if is_active
                && cursor_viewport_row >= 0
                && (cursor_viewport_row as usize) < pane_rows
                && cursor_col < pane_cols
            {
                let cx = content_x + cursor_col as f32 * cell_w;
                let cy = content_y + cursor_viewport_row as f32 * cell_h;
                cursor_pixel_for_frame = Some((cx, cy));
            }

            // Ghost text overlay — fish-shell style suggestion after cursor.
            if is_active && cursor_row >= 0 && (cursor_row as usize) < pane_rows {
                if let Some(suffix) = state.suggest.ghost_suffix() {
                    let ghost_fg = state.theme.ghost_text;
                    let transparent = [0.0, 0.0, 0.0, 0.0];
                    let cy = content_y + cursor_row as f32 * cell_h;
                    for (j, c) in suffix.chars().enumerate() {
                        let col = cursor_col + j;
                        if col >= pane_cols {
                            break;
                        }
                        let cx = content_x + col as f32 * cell_w;
                        cached_cell_data.push((c, cx, cy, font_size, ghost_fg, transparent));
                    }
                }
            }

            // Emit underline UIRects + populate cached_url_spans for mouse hover.
            for (col_start, raw_row, col_end, url) in url_span_list {
                let viewport_row = raw_row + display_offset as i32;
                if viewport_row < 0 || viewport_row as usize >= pane_rows {
                    continue;
                }
                let span_x = content_x + col_start as f32 * cell_w;
                let span_y = content_y + viewport_row as f32 * cell_h;
                let span_w = (col_end - col_start) as f32 * cell_w;
                cached_ui_rects.push(UIRect {
                    pos: [span_x, span_y + cell_h - 1.5],
                    size: [span_w, 1.5],
                    color: state.theme.hyperlink,
                });
                cached_url_spans.push(UrlSpan {
                    url,
                    x: span_x,
                    y: span_y,
                    w: span_w,
                    h: cell_h,
                });
            }

            // Scrollback position indicator: slim thumb on the right edge
            // when the viewport is scrolled above the bottom of history.
            if display_offset > 0 && history_size > 0 {
                let total_rows = (pane_rows + history_size) as f32;
                let thumb_h = (content_h * pane_rows as f32 / total_rows).max(8.0);
                let scroll_frac = (display_offset as f32 / history_size as f32).min(1.0);
                let travel = content_h - thumb_h;
                let thumb_y = content_y + travel * (1.0 - scroll_frac);
                cached_ui_rects.push(UIRect {
                    pos: [rect.x + rect.w - 4.0, thumb_y],
                    size: [3.0, thumb_h],
                    color: [0.44, 0.56, 0.78, 0.55],
                });
            }

            // Only draw pane borders when there's more than one pane in the
            // active tab. With a single pane the borders just duplicate the
            // window edge and the top border draws a cyan line right below
            // the tab bar — which looks like a stray UI element.
            if layouts.len() > 1 {
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
        cached_bg_rects.extend(tab_ui);

        for tab_cell in
            build_tab_bar_text(layout, tab_bar, scale_factor as f64, state.tab_scroll_offset, &state.theme)
        {
            cached_cell_data.push(tab_cell);
        }

        // Cursor rects go last so blink-only updates can truncate+re-push without
        // touching the stable rects (borders, dividers, search bars, tab bar).
        *cached_cursor_pixel = cursor_pixel_for_frame;
        *cached_cursor_rects_start = cached_ui_rects.len();
        push_cursor_rect(cached_ui_rects, cursor_pixel_for_frame, cursor_blink_on, cell_w, cell_h, state);

        *cached_font_size = font_size;
        *cached_blink = cursor_blink_on;
        *cached_active_tab = tab_bar.active;
    } else if blink_changed {
        // Blink-only: truncate cursor rects and re-push with new blink state.
        // Cell instances and bg rects are untouched — skips atlas lookups + GPU upload.
        cached_ui_rects.truncate(*cached_cursor_rects_start);
        push_cursor_rect(cached_ui_rects, *cached_cursor_pixel, cursor_blink_on, cell_w, cell_h, state);
        *cached_blink = cursor_blink_on;
    }

    // Build image draw list from placements in the active tab's panes.
    let mut image_draws: Vec<ImageInstance> = Vec::new();
    let mut image_draw_ids: Vec<u32> = Vec::new();
    let mut image_clips: Vec<[u32; 4]> = Vec::new();
    {
        let pane_tree = &tab_bar.active_tab().pane_tree;
        let pane_area = layout.pane_area();
        let margin = layout.pane_margin();
        let pane_rect = synapse_ui::PaneRect {
            x: pane_area.0,
            y: pane_area.1,
            w: pane_area.2,
            h: pane_area.3,
        };
        let layouts = pane_tree.get_layout(pane_rect);

        for placement in &image_store.placements {
            // Only render placements for panes in the active tab.
            let placement_pane_id = match placement.pane_id {
                Some(id) => id,
                None => continue,
            };
            // Check if this pane belongs to the active tab's pane tree.
            if !pane_tree.all_panes().contains(&placement_pane_id) {
                continue;
            }

            let pane = match find_pane(panes, placement_pane_id) {
                Some(p) => p,
                None => continue,
            };

            let layout_rect = match layouts.iter().find(|(pid, _)| *pid == placement_pane_id) {
                Some((_, r)) => *r,
                None => continue,
            };

            let content_x = layout_rect.x + margin;
            let content_y = layout_rect.y + margin;

            // Snapshot display offset for the placement's pane.
            let display_offset = {
                let term = match pane.term.lock() {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let grid = term.grid();
                grid.display_offset()
            };

            if let Some(image) = image_store.images.get(&placement.image_id) {
                // Upload to GPU if not yet cached.
                if !renderer.has_image(placement.image_id) {
                    renderer.upload_image(
                        placement.image_id,
                        &image.rgba,
                        image.width,
                        image.height,
                    );
                }

                let col = placement.col as f32;
                let row = (placement.row as isize - display_offset as isize).max(0) as f32;

                let img_w = if placement.columns > 0 {
                    placement.columns as f32 * cell_w
                } else {
                    image.width as f32
                };
                let img_h = if placement.rows > 0 {
                    placement.rows as f32 * cell_h
                } else {
                    image.height as f32
                };

                let px = content_x + col * cell_w;
                let py = content_y + row * cell_h;

                // Compute content area and clip rect for scissor.
                let cw = (layout_rect.w - margin * 2.0).max(1.0);
                let ch = (layout_rect.h - margin * 2.0).max(1.0);
                let clip_x = content_x.max(0.0) as u32;
                let clip_y = content_y.max(0.0) as u32;
                let clip_w = cw as u32;
                let clip_h = ch as u32;

                image_draws.push(ImageInstance {
                    pos: [px, py],
                    size: [img_w, img_h],
                });
                image_draw_ids.push(placement.image_id);
                image_clips.push([clip_x, clip_y, clip_w, clip_h]);
            }
        }
    }

    renderer.draw_frame_with_options(
        cached_cell_data,
        cached_ui_rects,
        cached_bg_rects,
        &image_draws,
        &image_draw_ids,
        &image_clips,
        state.config.font_ligatures,
        needs_cell_rebuild,
        needs_ui_rebuild,
    );

    exited_panes
}

/// Duración del splash en segundos.
const SPLASH_DURATION_SECS: f32 = 2.5;

/// Renderiza la pantalla de arranque cyberpunk.
/// `progress` va de 0.0 (inicio) a 1.0 (fin).
pub fn render_splash_screen(
    renderer: &mut Renderer,
    layout: &Layout,
    theme: &Theme,
    progress: f32,
) {
    let w = layout.window_width;
    let h = layout.window_height;

    let mut bg_rects: Vec<UIRect> = Vec::new();
    let mut ui_rects: Vec<UIRect> = Vec::new();
    let mut cells: CellData = Vec::new();

    let transparent = [0.0_f32, 0.0, 0.0, 0.0];
    // Color atenuado para subtítulo y decoraciones
    let dim_fg = [theme.fg[0], theme.fg[1], theme.fg[2], 0.45];

    // ── Fondo completo ──────────────────────────────────────────────────────
    bg_rects.push(UIRect { pos: [0.0, 0.0], size: [w, h], color: theme.bg });

    // ── Título: "S Y N A P S E  _" con letra espaciada ──────────────────────
    let title_fs: f32 = 30.0;
    let title_char_w = title_fs * 0.6;
    // Espacio entre letras para efecto cyberpunk
    let title = "S Y N A P S E  _";
    let title_w = title.chars().count() as f32 * title_char_w;
    let title_x = (w - title_w) * 0.5;
    // Centrado vertical ligeramente por encima del centro
    let title_y = h * 0.38 - title_fs;

    for (j, c) in title.chars().enumerate() {
        cells.push((
            c,
            title_x + j as f32 * title_char_w,
            title_y,
            title_fs,
            theme.fg,
            transparent,
        ));
    }

    // ── Línea decorativa bajo el título ─────────────────────────────────────
    let line_y = title_y + title_fs * 1.5;
    let line_w = (title_w * 1.15).min(w * 0.75);
    let line_x = (w - line_w) * 0.5;
    ui_rects.push(UIRect {
        pos: [line_x, line_y],
        size: [line_w, 1.0],
        color: dim_fg,
    });

    // ── Subtítulo ────────────────────────────────────────────────────────────
    let sub_fs: f32 = 11.0;
    let sub_char_w = sub_fs * 0.6;
    let subtitle = "NEURAL INTERFACE // v0.2.0";
    let sub_w = subtitle.chars().count() as f32 * sub_char_w;
    let sub_x = (w - sub_w) * 0.5;
    let sub_y = line_y + 10.0;
    for (j, c) in subtitle.chars().enumerate() {
        cells.push((
            c,
            sub_x + j as f32 * sub_char_w,
            sub_y,
            sub_fs,
            dim_fg,
            transparent,
        ));
    }

    // ── Barra de progreso ────────────────────────────────────────────────────
    let bar_y = sub_y + 45.0;
    let bar_w: f32 = (w * 0.5).clamp(200.0, 500.0);
    let bar_x = (w - bar_w) * 0.5;
    let bar_h: f32 = 2.0;

    // Track (fondo de la barra)
    ui_rects.push(UIRect {
        pos: [bar_x, bar_y],
        size: [bar_w, bar_h],
        color: dim_fg,
    });
    // Fill (progreso)
    if progress > 0.0 {
        ui_rects.push(UIRect {
            pos: [bar_x, bar_y],
            size: [bar_w * progress, bar_h],
            color: theme.cursor,
        });
    }

    // ── Texto de estado animado ───────────────────────────────────────────────
    let status_fs: f32 = 11.0;
    let status_char_w = status_fs * 0.6;
    let status = match (progress * 4.0) as u32 {
        0 => "> INITIALIZING KERNEL...",
        1 => "> MOUNTING NEURAL INTERFACE...",
        2 => "> SYNCING UPLINK PROTOCOLS...",
        3 => "> ESTABLISHING SECURE CHANNEL...",
        _ => "> SYSTEM READY",
    };
    let status_w = status.chars().count() as f32 * status_char_w;
    let status_x = (w - status_w) * 0.5;
    let status_y = bar_y + 14.0;
    // Al llegar al final, usar el color del cursor (más brillante) como confirmación
    let status_color = if progress >= 0.95 { theme.cursor } else { dim_fg };
    for (j, c) in status.chars().enumerate() {
        cells.push((
            c,
            status_x + j as f32 * status_char_w,
            status_y,
            status_fs,
            status_color,
            transparent,
        ));
    }

    renderer.draw_frame_with_options(&cells, &ui_rects, &bg_rects, &[], &[], &[], false, true, true);
}

use crate::app::AppCore;
use crate::image_protocol::{parse_apc, KittyAction};
use crate::pane_ops::create_pane;

impl AppCore {
    pub(crate) fn render(&mut self) {
        // ── Splash screen ──────────────────────────────────────────────────
        if let Some(start) = self.splash_start {
            let progress = start.elapsed().as_secs_f32() / SPLASH_DURATION_SECS;
            if progress < 1.0 {
                render_splash_screen(
                    &mut self.renderer,
                    &self.layout,
                    &self.state.theme,
                    progress,
                );
                // Pedir el siguiente frame para animar la barra de progreso
                self.window.request_redraw();
                return;
            }
            // Splash terminado
            self.splash_start = None;
        }

        // Drain APC image sequences from all panes into the image store.
        for pane in self.panes.iter_mut() {
            while let Ok(raw) = pane.apc_rx.try_recv() {
                if let Some(cmd) = parse_apc(&raw) {
                    if matches!(cmd.action, KittyAction::Delete) && cmd.image_id != 0 {
                        self.renderer.remove_image(cmd.image_id);
                    }
                    self.image_store.process(cmd, Some(pane.id));
                }
            }
        }

        self.frame_count += 1;
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.fps_last_print);
        if elapsed >= std::time::Duration::from_secs(1) {
            let fps = self.frame_count as f64 / elapsed.as_secs_f64();
            tracing::info!(target: "synapse_::bench", "FPS: {:.1}", fps);
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
            &self.image_store,
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
            &mut self.cached_cursor_rects_start,
            &mut self.cached_cursor_pixel,
            &mut self.cached_url_spans,
            effective_fs,
            self.scale_factor,
        );

        for pane_id in exited {
            self.handle_pane_exit(pane_id);
            self.cached_cell_data.clear();
            self.cached_ui_rects.clear();
            self.cached_cursor_rects_start = 0;
            self.cached_cursor_pixel = None;
        }
    }

    fn handle_pane_exit(&mut self, pane_id: synapse_ui::PaneId) {
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
                    self.tab_bar.tabs[0].pane_tree = synapse_ui::PaneTree::leaf(new_pane_id);
                    self.tab_bar.tabs[0].active_pane = new_pane_id;
                    match create_pane(new_pane_id, new_cols, new_rows, self.state.config.scrollback_lines) {
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
