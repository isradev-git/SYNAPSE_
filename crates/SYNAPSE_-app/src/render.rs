use std::collections::{HashMap, HashSet};

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{LineDamageBounds, TermDamage};
use alacritty_terminal::vte::ansi::Color as TermColor;
use alacritty_terminal::vte::ansi::CursorShape;
use base64::Engine;
use synapse_config::Theme;
use synapse_renderer::{
    image::ImageInstance, renderer::Renderer, ui::UIRect, underline::UnderlineInstance,
};
use synapse_ui::{layout::Layout, pane::Pane, tab_bar::TabBar, theme, PaneId};

use crate::app::CellData;
use crate::image_protocol::ImageStore;
use crate::pane_ops::find_pane;
use crate::search::build_match_set;
use crate::state::{AppState, UrlSpan};

const TAB_FONT_SIZE: f32 = 12.0;

#[derive(Clone, Copy)]
struct CachedGridCell {
    ch: char,
    fg: TermColor,
    bg: TermColor,
    flags: Flags,
}

impl Default for CachedGridCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: TermColor::Named(alacritty_terminal::vte::ansi::NamedColor::Foreground),
            bg: TermColor::Named(alacritty_terminal::vte::ansi::NamedColor::Background),
            flags: Flags::empty(),
        }
    }
}

/// Per-pane cache of grid cells in row-major order.
/// Enables incremental updates via alacritty's damage tracking.
/// Stores raw TermColor values so the existing processing pipeline
/// (selection/match/URL detection) works unchanged.
#[derive(Default)]
pub(crate) struct PaneCellCache {
    cols: usize,
    rows: usize,
    cells: Vec<CachedGridCell>,
}

impl PaneCellCache {
    fn needs_resize(&self, cols: usize, rows: usize) -> bool {
        self.cols != cols || self.rows != rows
    }

    fn ensure_size(&mut self, cols: usize, rows: usize) {
        if self.cols != cols || self.rows != rows {
            self.cols = cols;
            self.rows = rows;
            self.cells.resize(cols * rows, CachedGridCell::default());
        }
    }

    fn row(&self, row: usize) -> &[CachedGridCell] {
        let start = row * self.cols;
        &self.cells[start..start + self.cols]
    }

    fn rebuild_full(
        &mut self,
        grid: &alacritty_terminal::grid::Grid<alacritty_terminal::term::cell::Cell>,
        pane_cols: usize,
        pane_rows: usize,
        display_offset: usize,
    ) {
        self.ensure_size(pane_cols, pane_rows);
        for indexed in grid.display_iter() {
            let col = indexed.point.column.0;
            let raw_row = indexed.point.line.0;
            let viewport_row = raw_row + display_offset as i32;
            if viewport_row < 0 || viewport_row as usize >= pane_rows {
                continue;
            }
            if col >= pane_cols {
                continue;
            }
            let idx = (viewport_row as usize) * pane_cols + col;
            self.cells[idx] = CachedGridCell {
                ch: indexed.c,
                fg: indexed.fg,
                bg: indexed.bg,
                flags: indexed.flags,
            };
        }
    }

    fn update_damaged_from_term(
        &mut self,
        term: &mut alacritty_terminal::term::Term<impl alacritty_terminal::event::EventListener>,
        pane_cols: usize,
        pane_rows: usize,
        display_offset: usize,
    ) -> bool {
        self.ensure_size(pane_cols, pane_rows);
        match term.damage() {
            TermDamage::Full => {
                term.reset_damage();
                let grid = term.grid();
                self.rebuild_full(grid, pane_cols, pane_rows, display_offset);
                true
            }
            TermDamage::Partial(iter) => {
                let damaged: Vec<LineDamageBounds> = iter.collect();
                term.reset_damage();
                if damaged.is_empty() {
                    return false;
                }
                let grid = term.grid();
                let mut dirty_count = 0u32;
                for bounds in &damaged {
                    let viewport_row = bounds.line;
                    if viewport_row >= pane_rows {
                        continue;
                    }
                    let raw_grid_line = viewport_row as i32 - display_offset as i32;
                    let row = &grid[Line(raw_grid_line)];
                    let row_start = viewport_row * pane_cols;
                    let left = bounds.left;
                    let right = (bounds.right + 1).min(pane_cols);
                    for col in left..right {
                        let cell = &row[Column(col)];
                        self.cells[row_start + col] = CachedGridCell {
                            ch: cell.c,
                            fg: cell.fg,
                            bg: cell.bg,
                            flags: cell.flags,
                        };
                    }
                    dirty_count += 1;
                }
                dirty_count > 0
            }
        }
    }
}

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
        Black => ansi[0],
        Red => ansi[1],
        Green => ansi[2],
        Yellow => ansi[3],
        Blue => ansi[4],
        Magenta => ansi[5],
        Cyan => ansi[6],
        White => ansi[7],
        // Colores brillantes 8-15
        BrightBlack => ansi[8],
        BrightRed => ansi[9],
        BrightGreen => ansi[10],
        BrightYellow => ansi[11],
        BrightBlue => ansi[12],
        BrightMagenta => ansi[13],
        BrightCyan => ansi[14],
        BrightWhite => ansi[15],
        // Semánticos → usan el fallback (fg o bg según contexto)
        Foreground | BrightForeground | DimForeground => fallback,
        Background => fallback,
        // Dim: versión oscurecida de los colores normales del tema
        DimBlack => dim_color(ansi[0]),
        DimRed => dim_color(ansi[1]),
        DimGreen => dim_color(ansi[2]),
        DimYellow => dim_color(ansi[3]),
        DimBlue => dim_color(ansi[4]),
        DimMagenta => dim_color(ansi[5]),
        DimCyan => dim_color(ansi[6]),
        DimWhite => dim_color(ansi[7]),
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

pub(crate) fn build_underline_spans(
    ul_cells: &[(usize, i32, u32, [f32; 4])],
    content_x: f32,
    content_y: f32,
    cell_w: f32,
    cell_h: f32,
) -> Vec<UnderlineInstance> {
    let mut result = Vec::new();
    if ul_cells.is_empty() {
        return result;
    }

    let make = |col: usize, row: i32, w: usize, style: u32, color: [f32; 4]| {
        let (y_off, h) = match style {
            1 => (cell_h - 3.5, 3.0_f32),
            2 => (cell_h - 4.0, 4.0_f32),
            _ => (cell_h - 2.0, 1.5_f32),
        };
        UnderlineInstance {
            pos: [
                content_x + col as f32 * cell_w,
                content_y + row as f32 * cell_h + y_off,
            ],
            size: [w as f32 * cell_w, h],
            color,
            style,
            _pad: [0; 3],
        }
    };

    let (mut s_col, mut s_row, mut s_style, mut s_color) =
        (ul_cells[0].0, ul_cells[0].1, ul_cells[0].2, ul_cells[0].3);
    let mut span_w = 1usize;

    for &(col, row, style, color) in &ul_cells[1..] {
        let extends = row == s_row && col == s_col + span_w && style == s_style && color == s_color;
        if extends {
            span_w += 1;
        } else {
            result.push(make(s_col, s_row, span_w, s_style, s_color));
            (s_col, s_row, s_style, s_color) = (col, row, style, color);
            span_w = 1;
        }
    }
    result.push(make(s_col, s_row, span_w, s_style, s_color));
    result
}

fn has_prefix_at(chars: &[char], pos: usize, prefix: &str) -> bool {
    let pchars: Vec<char> = prefix.chars().collect();
    pos + pchars.len() <= chars.len()
        && chars[pos..pos + pchars.len()]
            .iter()
            .zip(&pchars)
            .all(|(a, b)| a == b)
}

fn is_path_start(chars: &[char], pos: usize) -> bool {
    if chars.len() <= pos {
        return false;
    }
    let c = chars[pos];
    if c == '/' || c == '~' {
        return true;
    }
    if c == '.' {
        return has_prefix_at(chars, pos, "./") || has_prefix_at(chars, pos, "../");
    }
    false
}

/// Scan visible terminal rows for clickable file paths.
/// Detects: `./main.c:42:10`, `/home/user/project/src/main.rs:10`, `~/docs/readme.md`
fn detect_paths(
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
            if !is_path_start(&chars, i) {
                i += 1;
                continue;
            }

            let mut end = i + 1;
            while end < n
                && !chars[end].is_whitespace()
                && chars[end] != '\''
                && chars[end] != '"'
                && chars[end] != ')'
                && chars[end] != ']'
                && chars[end] != '}'
            {
                end += 1;
            }

            let raw: String = chars[i..end].iter().collect();
            let has_ext =
                raw.rfind('.').map(|dot| dot > i + 1).unwrap_or(false) || raw.contains('/');

            if has_ext && end > i + 2 {
                let col_start = cols[i];
                let col_end = cols[end - 1] + 1;
                spans.push((col_start, raw_row, col_end, raw));
            }
            i = end;
        }
    }
    spans
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
    effects_enabled: bool,
) -> Vec<UIRect> {
    let mut rects = Vec::new();
    let sw = layout.sidebar_width;
    let header_h = theme::SIDEBAR_HEADER_HEIGHT;
    let tab_h = theme::SIDEBAR_TAB_HEIGHT;
    let bottom_btn_h = tab_h;

    // Sidebar background (full height)
    rects.push(UIRect {
        pos: [0.0, 0.0],
        size: [sw, layout.window_height],
        color: theme.tab_bar_bg,
    });

    // Header area (logo)
    rects.push(UIRect {
        pos: [0.0, 0.0],
        size: [sw, header_h],
        color: theme.tab_bar_bg,
    });

    // Header separator line
    rects.push(UIRect {
        pos: [0.0, header_h],
        size: [sw, 1.0],
        color: theme.tab_separator,
    });

    let tab_count = tab_bar.tabs.len();
    let (start, end, show_up, show_down) = layout.tab_visible_range(tab_count, scroll_offset);

    // Scroll up button
    if show_up {
        rects.push(UIRect {
            pos: [0.0, header_h],
            size: [sw, theme::SIDEBAR_SCROLL_BTN_H],
            color: theme.tab_bar_bg,
        });
    }

    // Tab rows
    for (vis_i, i) in (start..end).enumerate() {
        let y = layout.tab_y(vis_i, show_up);

        let color = if i == tab_bar.active {
            theme.tab_active_bg
        } else {
            theme.tab_inactive_bg
        };
        rects.push(UIRect {
            pos: [0.0, y],
            size: [sw, tab_h],
            color,
        });

        // Active tab left border accent (3px neon) with optional glow layers behind it
        if i == tab_bar.active {
            if effects_enabled {
                rects.push(UIRect {
                    pos: [0.0, y],
                    size: [10.0, tab_h],
                    color: [1.0, 0.0, 0.24, 0.1],
                });
                rects.push(UIRect {
                    pos: [0.0, y],
                    size: [6.0, tab_h],
                    color: [1.0, 0.0, 0.24, 0.3],
                });
            }
            rects.push(UIRect {
                pos: [0.0, y],
                size: [3.0, tab_h],
                color: theme.panel_active_border,
            });
        }

        // Hover overlay (not for active tab)
        if hover_tab == Some(i) && i != tab_bar.active {
            rects.push(UIRect {
                pos: [0.0, y],
                size: [sw, tab_h],
                color: theme.tab_hover_bg,
            });
        }
    }

    // Scroll down button
    if show_down {
        let top_y = layout.tab_y(end.saturating_sub(start), show_up);
        rects.push(UIRect {
            pos: [0.0, top_y],
            size: [sw, theme::SIDEBAR_SCROLL_BTN_H],
            color: theme.tab_bar_bg,
        });
    }

    // Bottom separator + New tab button
    let bottom_y = layout.window_height - bottom_btn_h;
    rects.push(UIRect {
        pos: [0.0, bottom_y - 1.0],
        size: [sw, 1.0],
        color: theme.tab_separator,
    });
    rects.push(UIRect {
        pos: [0.0, bottom_y],
        size: [sw, bottom_btn_h],
        color: theme.tab_bar_bg,
    });

    rects
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn build_tab_bar_text(
    layout: &Layout,
    tab_bar: &TabBar,
    scale_factor: f64,
    scroll_offset: usize,
    theme: &Theme,
    sidebar_show_process_dot: bool,
    cell_w: f32,
    cell_h: f32,
) -> Vec<(char, f32, f32, f32, [f32; 4], [f32; 4])> {
    let mut result = Vec::new();
    let sw = layout.sidebar_width;
    let header_h = theme::SIDEBAR_HEADER_HEIGHT;
    let tab_h = theme::SIDEBAR_TAB_HEIGHT;
    let tab_font_size = TAB_FONT_SIZE * scale_factor as f32;
    let char_aspect = if cell_h > 0.0 { cell_w / cell_h } else { 0.6 };
    let char_w = tab_font_size * char_aspect;
    let text_y_offset = (tab_h - tab_font_size) * 0.5;
    let tab_count = tab_bar.tabs.len();
    let (start, end, show_up, show_down) = layout.tab_visible_range(tab_count, scroll_offset);

    // Logo/title text in header
    let logo_text = "SYNAPSE_";
    let logo_fs = 14.0 * scale_factor as f32;
    let logo_cw = logo_fs * char_aspect;
    let logo_x = 8.0;
    // Center logo vertically within the unscaled header rect
    let logo_line_h = logo_fs * 1.2;
    let logo_y = ((header_h - logo_line_h) * 0.5).max(2.0);
    let transparent = [0.0f32, 0.0, 0.0, 0.0];
    for (j, c) in logo_text.chars().enumerate() {
        result.push((
            c,
            logo_x + j as f32 * logo_cw,
            logo_y,
            logo_fs,
            theme.tab_text,
            transparent,
        ));
    }

    // Scroll up arrow
    if show_up {
        let btn_y = header_h + 4.0;
        result.push((
            '▲',
            8.0,
            btn_y,
            tab_font_size,
            theme.tab_button_text,
            transparent,
        ));
    }

    // Tab rows
    for (vis_i, i) in (start..end).enumerate() {
        let tab = &tab_bar.tabs[i];
        let y = layout.tab_y(vis_i, show_up) + text_y_offset;

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

        let text_x = 10.0;
        let dot_indent = if sidebar_show_process_dot {
            char_w * 1.5
        } else {
            0.0
        };
        let title_text_x = text_x + dot_indent;

        // Truncate to fit sidebar accounting for dot indent
        let max_chars = ((sw - 32.0 - dot_indent) / char_w).max(1.0) as usize;
        let title: String = if raw_title.chars().count() > max_chars {
            raw_title
                .chars()
                .take(max_chars.saturating_sub(1))
                .collect::<String>()
                + "…"
        } else {
            raw_title
        };

        // Process dot: green = alive, red = dead
        if sidebar_show_process_dot {
            let dot_color = if tab.alive {
                [0.0_f32, 0.8, 0.267, 1.0]
            } else {
                [1.0_f32, 0.0, 0.235, 1.0]
            };
            result.push(('●', text_x, y, tab_font_size, dot_color, bg));
        }

        for (j, c) in title.chars().enumerate() {
            result.push((
                c,
                title_text_x + j as f32 * char_w,
                y,
                tab_font_size,
                fg,
                bg,
            ));
        }

        // Close button (×)
        let close_x = sw - 16.0;
        let close_fg = if i == tab_bar.active {
            [1.0, 1.0, 1.0, 0.7_f32]
        } else {
            [0.8, 0.8, 0.8, 0.5_f32]
        };
        result.push(('×', close_x, y, tab_font_size, close_fg, transparent));
    }

    // Scroll down arrow
    if show_down {
        let btn_y = layout.tab_y(end.saturating_sub(start), show_up) + 4.0;
        result.push((
            '▼',
            8.0,
            btn_y,
            tab_font_size,
            theme.tab_button_text,
            transparent,
        ));
    }

    // Bottom "+" button
    let plus_y = layout.window_height - tab_h + text_y_offset;
    result.push((
        '+',
        8.0,
        plus_y,
        tab_font_size,
        theme.tab_button_text,
        transparent,
    ));

    result
}

fn build_title_bar(layout: &Layout, theme: &Theme) -> Vec<UIRect> {
    if !layout.wayland_decorated {
        return Vec::new();
    }
    let h = layout.title_bar_height();
    vec![
        UIRect {
            pos: [0.0, 0.0],
            size: [layout.window_width, h],
            color: theme.tab_bar_bg,
        },
        // Thin separator line below title bar
        UIRect {
            pos: [0.0, h - 1.0],
            size: [layout.window_width, 1.0],
            color: theme.tab_separator,
        },
    ]
}

#[allow(clippy::type_complexity)]
fn build_title_bar_text(
    layout: &Layout,
    font_size: f32,
    theme: &Theme,
    char_aspect: f32,
) -> Vec<(char, f32, f32, f32, [f32; 4], [f32; 4])> {
    if !layout.wayland_decorated {
        return Vec::new();
    }
    let y = (layout.title_bar_height() - font_size) / 2.0;
    let x = layout.sidebar_width + 12.0;
    let mut cells = Vec::new();
    let transparent = [0.0, 0.0, 0.0, 0.0];
    for (i, c) in "SYNAPSE_".chars().enumerate() {
        cells.push((
            c,
            x + i as f32 * font_size * char_aspect,
            y,
            font_size,
            theme.tab_text_inactive,
            transparent,
        ));
    }
    cells
}

#[allow(clippy::type_complexity)]
pub fn build_status_bar(
    layout: &Layout,
    state: &AppState,
    active_cwd: &str,
    workspace_name: &str,
    char_aspect: f32,
) -> (Vec<UIRect>, Vec<(char, f32, f32, f32, [f32; 4], [f32; 4])>) {
    let mut rects = Vec::new();
    let mut cells = Vec::new();

    if !state.status_bar_visible {
        return (rects, cells);
    }

    let bar_h = theme::STATUS_BAR_HEIGHT;
    let bar_y = layout.window_height - bar_h;
    let bar_w = layout.window_width - layout.sidebar_width;
    let bar_x = layout.sidebar_width;

    rects.push(UIRect {
        pos: [bar_x, bar_y],
        size: [bar_w, bar_h],
        color: state.theme.tab_bar_bg,
    });
    rects.push(UIRect {
        pos: [bar_x, bar_y],
        size: [bar_w, 1.0],
        color: state.theme.tab_separator,
    });

    let fs = 13.0;
    let char_w = fs * char_aspect;
    let text_y = bar_y + (bar_h - fs) * 0.5;
    let transparent = [0.0f32, 0.0, 0.0, 0.0];
    let fg = state.theme.tab_text_inactive;

    // Left: ws name + CWD + git branch + k8s context
    let mut left = String::new();
    // Workspace name
    if !workspace_name.is_empty() {
        left.push('[');
        left.push_str(workspace_name);
        left.push_str("] ");
    }
    let home = std::env::var("HOME").unwrap_or_default();
    if !active_cwd.is_empty() {
        let cwd = if !home.is_empty() && active_cwd.starts_with(&home) {
            format!("~{}", &active_cwd[home.len()..])
        } else {
            active_cwd.to_string()
        };
        let max_cwd = 30usize;
        if cwd.chars().count() > max_cwd {
            let chars: Vec<char> = cwd.chars().collect();
            let start = chars.len().saturating_sub(max_cwd.saturating_sub(1));
            left.push_str(&format!(
                "\u{2026}{}",
                chars[start..].iter().collect::<String>()
            ));
        } else {
            left.push_str(&cwd);
        }
    }
    if state.config.status_bar_show_git {
        if let Some(ref branch) = state.git_branch {
            if !left.is_empty() {
                left.push_str("  ");
            }
            left.push(' ');
            left.push_str(branch);
        }
    }
    if state.config.status_bar_show_k8s {
        if let Some(ref ctx) = state.k8s_context {
            if !left.is_empty() {
                left.push_str("  ");
            }
            left.push_str(ctx);
        }
    }
    let left_max_x = bar_x + bar_w * 0.55;
    let mut lx = bar_x + 8.0;
    for c in left.chars() {
        if lx + char_w > left_max_x {
            break;
        }
        cells.push((c, lx, text_y, fs, fg, transparent));
        lx += char_w;
    }

    // Center: user@host
    if !state.user_host.is_empty() {
        let cw = state.user_host.chars().count() as f32 * char_w;
        let cx = bar_x + (bar_w - cw) * 0.5;
        for (j, c) in state.user_host.chars().enumerate() {
            cells.push((c, cx + j as f32 * char_w, text_y, fs, fg, transparent));
        }
    }

    // Broadcast indicator
    if state.broadcasting {
        let label = "[BROADCAST]";
        let bc = [1.0f32, 0.0, 0.24, 1.0];
        let blw = label.chars().count() as f32 * char_w;
        let blx = bar_x + bar_w - blw - 8.0;
        for (j, c) in label.chars().enumerate() {
            cells.push((c, blx + j as f32 * char_w, text_y, fs, bc, transparent));
        }
    }

    // Recording indicator
    if state.recording {
        let label = "[REC]";
        let rc = [1.0f32, 0.33, 0.0, 1.0];
        let blw = "[BROADCAST]".chars().count() as f32 * char_w;
        let broadcast_offset = if state.broadcasting { blw + 12.0 } else { 0.0 };
        let rlw = label.chars().count() as f32 * char_w;
        let rlx = bar_x + bar_w - rlw - broadcast_offset - 8.0;
        for (j, c) in label.chars().enumerate() {
            cells.push((c, rlx + j as f32 * char_w, text_y, fs, rc, transparent));
        }
    }

    // Right: HH:MM:SS (UTC)
    if state.config.status_bar_show_time && state.cached_time_sec > 0 {
        let s = state.cached_time_sec % 60;
        let m = (state.cached_time_sec / 60) % 60;
        let h = (state.cached_time_sec / 3600) % 24;
        let time_str = format!("{:02}:{:02}:{:02}", h, m, s);
        let tw = time_str.chars().count() as f32 * char_w;
        let mut tx_offset = 8.0f32;
        if state.broadcasting {
            tx_offset += "[BROADCAST]".chars().count() as f32 * char_w + 12.0;
        }
        if state.recording {
            tx_offset += "[REC]".chars().count() as f32 * char_w + 12.0;
        }
        let tx = bar_x + bar_w - tw - tx_offset;
        for (j, c) in time_str.chars().enumerate() {
            cells.push((c, tx + j as f32 * char_w, text_y, fs, fg, transparent));
        }
    }

    (rects, cells)
}

#[allow(clippy::too_many_arguments, clippy::ptr_arg, clippy::type_complexity)]
fn push_cursor_rect(
    ui_rects: &mut Vec<UIRect>,
    cursor_pixel: Option<(f32, f32)>,
    cursor_alpha: f32,
    cell_w: f32,
    cell_h: f32,
    state: &mut AppState,
) {
    if state.in_copy_mode {
        return;
    }
    if cursor_alpha < 0.01 {
        return;
    }
    let (cx, cy) = match cursor_pixel {
        Some(p) => p,
        None => return,
    };

    // Push current position and render fading trail rects behind cursor
    let max_trail = state.config.effects.cursor_trail as usize;
    state.push_cursor_trail(cx, cy, max_trail);
    if max_trail > 0 && state.effects_enabled && !state.config.reduce_motion {
        let trail_len = state.cursor_trail.len();
        for (i, &(tx, ty)) in state.cursor_trail.iter().enumerate().skip(1) {
            let alpha = 1.0 - (i as f32 / trail_len as f32);
            let c = state.theme.cursor;
            let faded = [c[0], c[1], c[2], c[3] * alpha * 0.6 * cursor_alpha];
            ui_rects.push(UIRect {
                pos: [tx, ty],
                size: [cell_w, cell_h],
                color: faded,
            });
        }
    }

    let base = state.theme.cursor;
    let color = [base[0], base[1], base[2], base[3] * cursor_alpha];
    let term_shape = state.term_cursor_style.map(|(s, _)| s);
    match term_shape {
        Some(CursorShape::Beam) => {
            ui_rects.push(UIRect {
                pos: [cx, cy],
                size: [1.5, cell_h],
                color,
            });
        }
        Some(CursorShape::Underline) => {
            ui_rects.push(UIRect {
                pos: [cx, cy + cell_h - 2.0],
                size: [cell_w, 2.0],
                color,
            });
        }
        Some(CursorShape::Hidden) => {
            // Hidden cursor: render nothing
        }
        // Block, HollowBlock, or None → fallback to TOML config
        _ => match state.config.cursor_style {
            synapse_config::CursorStyle::Block => {
                ui_rects.push(UIRect {
                    pos: [cx, cy],
                    size: [cell_w, cell_h],
                    color,
                });
            }
            synapse_config::CursorStyle::Beam => {
                ui_rects.push(UIRect {
                    pos: [cx, cy],
                    size: [1.5, cell_h],
                    color,
                });
            }
            synapse_config::CursorStyle::Underline => {
                ui_rects.push(UIRect {
                    pos: [cx, cy + cell_h - 2.0],
                    size: [cell_w, 2.0],
                    color,
                });
            }
            synapse_config::CursorStyle::NeonUnderbar => {
                let bar_y = cy + cell_h - 2.0;
                ui_rects.push(UIRect {
                    pos: [cx, bar_y],
                    size: [cell_w, 2.0],
                    color,
                });
                ui_rects.push(UIRect {
                    pos: [cx, bar_y],
                    size: [cell_w, 5.0],
                    color: [color[0], color[1], color[2], color[3] * 0.3],
                });
                ui_rects.push(UIRect {
                    pos: [cx, bar_y],
                    size: [cell_w, 8.0],
                    color: [color[0], color[1], color[2], color[3] * 0.08],
                });
            }
        },
    }
}

/// Compute the quad position(s) and size for a background image based on fit mode.
fn compute_bg_quads(
    content_x: f32,
    content_y: f32,
    cw: f32,
    ch: f32,
    img_w: f32,
    img_h: f32,
    mode: &synapse_config::BackgroundMode,
) -> Vec<(f32, f32, f32, f32)> {
    use synapse_config::BackgroundMode;
    match mode {
        BackgroundMode::Cover => {
            let iar = img_w / img_h;
            let par = cw / ch;
            if iar > par {
                let w = ch * iar;
                vec![(content_x - (w - cw) / 2.0, content_y, w, ch)]
            } else {
                let h = cw / iar;
                vec![(content_x, content_y - (h - ch) / 2.0, cw, h)]
            }
        }
        BackgroundMode::Contain => {
            let iar = img_w / img_h;
            let par = cw / ch;
            if iar > par {
                let h = cw / iar;
                vec![(content_x, content_y + (ch - h) / 2.0, cw, h)]
            } else {
                let w = ch * iar;
                vec![(content_x + (cw - w) / 2.0, content_y, w, ch)]
            }
        }
        BackgroundMode::Stretch => {
            vec![(content_x, content_y, cw, ch)]
        }
        BackgroundMode::Tile => {
            let mut quads = Vec::new();
            let mut y_base = content_y;
            while y_base < content_y + ch {
                let h = (content_y + ch - y_base).min(img_h);
                let mut x_base = content_x;
                while x_base < content_x + cw {
                    let w = (content_x + cw - x_base).min(img_w);
                    quads.push((x_base, y_base, w, h));
                    x_base += img_w;
                }
                y_base += img_h;
            }
            quads
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
    state: &mut AppState,
    cell_w: f32,
    cell_h: f32,
    cursor_alpha: f32,
    cached_cell_data: &mut CellData,
    cached_ui_rects: &mut Vec<UIRect>,
    cached_bg_rects: &mut Vec<UIRect>,
    cached_underline_instances: &mut Vec<UnderlineInstance>,
    cached_blink: &mut f32,
    cached_font_size: &mut f32,
    cached_active_tab: &mut usize,
    cached_cursor_rects_start: &mut usize,
    cached_cursor_pixel: &mut Option<(f32, f32)>,
    cached_url_spans: &mut Vec<UrlSpan>,
    pane_cell_caches: &mut HashMap<PaneId, PaneCellCache>,
    effective_font_size: f32,
    scale_factor: f32,
    time_secs: f32,
) -> Vec<PaneId> {
    let font_size = effective_font_size;
    let char_aspect = if cell_h > 0.0 { cell_w / cell_h } else { 0.6 };

    // Drain event channels: pick up exit signals and title updates, then
    // check the dirty flag for PTY output.
    let exited_panes: Vec<PaneId> = panes
        .iter_mut()
        .filter_map(|p| if p.poll_events() { Some(p.id) } else { None })
        .collect();

    // Drain pending commands extracted via OSC 133 into persistent history
    if state.config.persistent_history {
        let history_save_interval = 100_usize; // save every N commands
        let mut commands_since_save = 0_usize;
        for pane in panes.iter_mut() {
            while let Some((cmd, exit_code)) = pane.pending_commands.pop_front() {
                let cwd = pane.cwd();
                state.command_history.add(cmd, cwd, exit_code);
                commands_since_save += 1;

                // Update suggester with new command
                if let Some(entry) = state.command_history.entries().back() {
                    state.suggester.insert(&entry.cmd);
                }
            }
        }
        if commands_since_save > 0 && commands_since_save >= history_save_interval {
            state.command_history.save();
        }
    }

    // 4.3: drain dirty for all panes but only trigger rebuild for active-tab panes.
    let active_ids: HashSet<PaneId> = tab_bar
        .active_tab()
        .pane_tree
        .all_panes()
        .into_iter()
        .collect();
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
    let blink_changed = (*cached_blink * 64.0) as u8 != (cursor_alpha * 64.0) as u8;
    let tab_changed = tab_bar.active != *cached_active_tab;
    let ui_active = state.selecting
        || state.search.active
        || state.history_search.active
        || state.ws_rename.active
        || state.suggest.ghost.is_some()
        || state.palette.active
        || state.overlay.active;
    let first_frame = cached_cell_data.is_empty();
    let label_active = state.config.show_pane_labels
        && state
            .pane_label_until
            .map(|t| t > std::time::Instant::now())
            .unwrap_or(false);
    let resize_active = state.config.show_resize_indicator && state.dragging_divider.is_some();

    // Poll completed git branch async read
    let git_branch_updated = {
        let result = if let Some(ref rx) = state.git_branch_rx {
            rx.try_recv().ok()
        } else {
            None
        };
        if let Some(b) = result {
            state.git_branch = b;
            state.git_branch_rx = None;
            true
        } else {
            false
        }
    };

    // Spawn git branch reader on tab switch or first frame
    if (first_frame || tab_changed) && state.git_branch_rx.is_none() {
        let cwd = tab_bar.active_tab().cwd.clone();
        if !cwd.is_empty() {
            state.git_branch_rx = Some(crate::state::spawn_git_branch_reader(&cwd));
        }
    }

    // Status bar clock: dirty when UTC second ticks
    let now_sec = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let status_time_dirty = state.status_bar_visible
        && state.config.status_bar_show_time
        && now_sec != state.cached_time_sec;
    if status_time_dirty {
        state.cached_time_sec = now_sec;
    }

    // Changes triggered by non-PTY sources always force a GPU re-upload.
    // PTY-only frames may turn out to have zero terminal damage — tracked below.
    let non_pty_trigger = font_changed
        || tab_changed
        || first_frame
        || ui_active
        || label_active
        || resize_active
        || git_branch_updated
        || status_time_dirty
        || state.profiler_active
        || state.keybinds_open
        || state.settings_open;

    // Cell data only changes on real terminal events — not on cursor blink.
    let needs_cell_rebuild = pty_received || non_pty_trigger;
    // UI rects (cursor shape) also change on blink.
    let needs_ui_rebuild = needs_cell_rebuild || blink_changed;

    // Tracks whether any pane had genuine terminal damage this frame.
    // When pty_received fires but every pane's damage list is empty
    // (e.g. only invisible escape sequences were processed), we can skip
    // the GPU atlas rebuild and instance upload — cells_dirty = false.
    let mut any_pane_had_damage = non_pty_trigger;

    if needs_cell_rebuild {
        cached_cell_data.clear();
        cached_ui_rects.clear();
        cached_bg_rects.clear();
        cached_url_spans.clear();
        cached_underline_instances.clear();

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

        let match_set: HashSet<(usize, i32)> = {
            let mut set = if state.search.active && !state.search.term.is_empty() {
                build_match_set(&state.search.matches, state.search.term.len())
            } else {
                HashSet::new()
            };
            // Merge word occurrence highlights
            for &pos in &state.word_highlight_positions {
                set.insert(pos);
            }
            set
        };

        // Reused across pane iterations to avoid per-frame allocation.
        let mut ul_buf: Vec<(usize, i32, u32, Option<TermColor>)> = Vec::new();

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
            let (
                cells,
                osc8_cells,
                cursor_col,
                cursor_row,
                sel_range,
                display_offset,
                history_size,
            ): (
                Vec<(usize, i32, char, TermColor, TermColor)>,
                Vec<(usize, i32, String)>,
                usize,
                i32,
                Option<alacritty_terminal::selection::SelectionRange>,
                usize,
                usize,
            ) = {
                let mut term = match pane.term.lock() {
                    Ok(t) => t,
                    Err(_) => continue,
                };

                let cursor_col;
                let cursor_row;
                let display_offset;
                let history_size;
                let sel_range;
                {
                    let grid = term.grid();
                    cursor_col = grid.cursor.point.column.0;
                    cursor_row = grid.cursor.point.line.0;
                    display_offset = grid.display_offset();
                    history_size = grid.history_size();
                    sel_range = term.selection.as_ref().and_then(|s| s.to_range(&*term));

                    // Update word occurrence highlights for the active pane
                    if is_active {
                        if let Some(ref _range) = sel_range {
                            if let Some(text) = term.selection_to_string() {
                                let text = text.trim().to_string();
                                if !text.is_empty()
                                    && !text.contains(char::is_whitespace)
                                    && text.len() <= 80
                                {
                                    if state.word_highlight_word.as_deref() != Some(&text) {
                                        state.word_highlight_word = Some(text.clone());
                                        state.word_highlight_positions.clear();
                                        let grid = term.grid();
                                        let word_chars: Vec<char> = text.chars().collect();
                                        let word_len = word_chars.len();
                                        let cols = grid.columns();
                                        let start = -(display_offset as i32);
                                        let end = start + pane_rows as i32;
                                        for raw_row in start..end {
                                            for col in 0..cols.saturating_sub(word_len.max(1) - 1) {
                                                let mut matches_ok = true;
                                                for (i, &wc) in word_chars.iter().enumerate() {
                                                    let cell =
                                                        &grid[Line(raw_row)][Column(col + i)];
                                                    if cell.c != wc {
                                                        matches_ok = false;
                                                        break;
                                                    }
                                                }
                                                if matches_ok {
                                                    for i in 0..word_len {
                                                        state
                                                            .word_highlight_positions
                                                            .insert((col + i, raw_row));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    state.word_highlight_word = None;
                                    state.word_highlight_positions.clear();
                                }
                            }
                        } else {
                            state.word_highlight_word = None;
                            state.word_highlight_positions.clear();
                        }
                    }
                }

                let use_cache = !first_frame
                    && !font_changed
                    && !tab_changed
                    && !ui_active
                    && !pane_cell_caches
                        .get(&pane_id)
                        .map(|c| c.needs_resize(pane_cols, pane_rows))
                        .unwrap_or(true);

                let mut buf = Vec::with_capacity(pane_cols * pane_rows);
                let mut hyperlinks: Vec<(usize, i32, String)> = Vec::new();

                if use_cache {
                    let cache = pane_cell_caches.entry(pane_id).or_default();
                    let pane_had_damage = cache.update_damaged_from_term(
                        &mut term,
                        pane_cols,
                        pane_rows,
                        display_offset,
                    );
                    if pane_had_damage {
                        any_pane_had_damage = true;
                    }

                    for vp_row in 0..pane_rows {
                        let raw_row = vp_row as i32 - display_offset as i32;
                        let row_cells = cache.row(vp_row);
                        for (col, c) in row_cells.iter().enumerate().take(pane_cols) {
                            buf.push((col, raw_row, c.ch, c.fg, c.bg));
                            if c.flags.intersects(Flags::ALL_UNDERLINES) {
                                let style = if c.flags.contains(Flags::UNDERCURL) {
                                    2u32
                                } else if c.flags.contains(Flags::DOUBLE_UNDERLINE) {
                                    1
                                } else if c.flags.contains(Flags::DOTTED_UNDERLINE) {
                                    3
                                } else if c.flags.contains(Flags::DASHED_UNDERLINE) {
                                    4
                                } else {
                                    0
                                };
                                ul_buf.push((col, vp_row as i32, style, None));
                            }
                        }
                    }
                } else {
                    let grid = term.grid();
                    for indexed in grid.display_iter() {
                        let col = indexed.point.column.0;
                        let raw_row = indexed.point.line.0;
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
                        let flags = indexed.flags;
                        if flags.intersects(Flags::ALL_UNDERLINES) {
                            let style = if flags.contains(Flags::UNDERCURL) {
                                2u32
                            } else if flags.contains(Flags::DOUBLE_UNDERLINE) {
                                1
                            } else if flags.contains(Flags::DOTTED_UNDERLINE) {
                                3
                            } else if flags.contains(Flags::DASHED_UNDERLINE) {
                                4
                            } else {
                                0
                            };
                            ul_buf.push((col, viewport_row, style, indexed.underline_color()));
                        }
                    }
                    // Rebuild cache from full grid iteration.
                    let cache = pane_cell_caches.entry(pane_id).or_default();
                    cache.rebuild_full(grid, pane_cols, pane_rows, display_offset);
                    term.reset_damage();
                    any_pane_had_damage = true;
                }

                (
                    buf,
                    hyperlinks,
                    cursor_col,
                    cursor_row,
                    sel_range,
                    display_offset,
                    history_size,
                )
            };

            if !ul_buf.is_empty() {
                let ul_cells: Vec<(usize, i32, u32, [f32; 4])> = ul_buf
                    .drain(..)
                    .map(|(col, viewport_row, style, ul_color)| {
                        let rgba = ul_color
                            .map(|c| {
                                term_color_to_rgba(c, state.theme.fg, &state.theme.ansi_colors)
                            })
                            .unwrap_or(state.theme.fg);
                        (col, viewport_row, style, rgba)
                    })
                    .collect();
                let spans = build_underline_spans(&ul_cells, content_x, content_y, cell_w, cell_h);
                cached_underline_instances.extend(spans);
            }

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
                if state.config.clickable_paths {
                    url_span_list.extend(detect_paths(&cells, display_offset, pane_rows));
                }
            }

            // Pane badge watermark (semi-transparent large text behind cells)
            if state.config.pane_badge {
                let title = pane.title();
                let cwd = pane.cwd();
                let badge_text = state
                    .config
                    .pane_badge_format
                    .replace("{cwd}", &cwd)
                    .replace("{title}", &title)
                    .replace("{user}@{host}", &state.user_host);
                if !badge_text.is_empty() && !badge_text.ends_with("{}") {
                    let badge_scale = (content_h * 0.5).clamp(20.0, 60.0);
                    let char_w = badge_scale * char_aspect;
                    let total_w = badge_text.len() as f32 * char_w;
                    let start_x = content_x + (content_w - total_w) / 2.0;
                    let start_y = content_y + (content_h - badge_scale) / 2.0;
                    let badge_fg = [
                        state.theme.fg[0],
                        state.theme.fg[1],
                        state.theme.fg[2],
                        0.05,
                    ];
                    let transparent = [0.0, 0.0, 0.0, 0.0];
                    for (i, ch) in badge_text.chars().enumerate() {
                        let cx = start_x + i as f32 * char_w;
                        cached_cell_data.push((
                            ch,
                            cx,
                            start_y,
                            badge_scale,
                            badge_fg,
                            transparent,
                        ));
                    }
                }
            }

            for (col, raw_row, ch, fg_c, bg_c) in cells {
                let viewport_row = raw_row + display_offset as i32;
                let x = content_x + col as f32 * cell_w;
                let y = content_y + viewport_row as f32 * cell_h;

                let fg = term_color_to_rgba(fg_c, state.theme.fg, &state.theme.ansi_colors);
                let bg = term_color_to_rgba(bg_c, state.theme.bg, &state.theme.ansi_colors);

                let in_selection = is_active
                    && sel_range
                        .as_ref()
                        .map(|r| {
                            r.contains(alacritty_terminal::index::Point::new(
                                alacritty_terminal::index::Line(raw_row),
                                alacritty_terminal::index::Column(col),
                            ))
                        })
                        .unwrap_or(false);

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
            if cursor_viewport_row >= 0
                && (cursor_viewport_row as usize) < pane_rows
                && cursor_col < pane_cols
            {
                let cx = content_x + cursor_col as f32 * cell_w;
                let cy = content_y + cursor_viewport_row as f32 * cell_h;
                if is_active {
                    cursor_pixel_for_frame = Some((cx, cy));
                } else if layouts.len() > 1 {
                    // Hollow block cursor on inactive panes: render border outline.
                    let c = state.theme.cursor;
                    let hollow = [c[0], c[1], c[2], c[3] * 0.35];
                    cached_ui_rects.push(UIRect {
                        pos: [cx, cy],
                        size: [cell_w, 1.0],
                        color: hollow,
                    });
                    cached_ui_rects.push(UIRect {
                        pos: [cx, cy + cell_h - 1.0],
                        size: [cell_w, 1.0],
                        color: hollow,
                    });
                    cached_ui_rects.push(UIRect {
                        pos: [cx, cy],
                        size: [1.0, cell_h],
                        color: hollow,
                    });
                    cached_ui_rects.push(UIRect {
                        pos: [cx + cell_w - 1.0, cy],
                        size: [1.0, cell_h],
                        color: hollow,
                    });
                }
            }

            // Copy mode cursor: amber block, rendered before cached_cursor_rects_start
            // so it survives blink-only redraws.
            if is_active && state.in_copy_mode {
                if let Some(ref cms) = pane.copy_mode {
                    let copy_col = cms.cursor.column.0;
                    let copy_viewport_row = cms.cursor.line.0 + display_offset as i32;
                    if copy_viewport_row >= 0
                        && (copy_viewport_row as usize) < pane_rows
                        && copy_col < pane_cols
                    {
                        let cx = content_x + copy_col as f32 * cell_w;
                        let cy = content_y + copy_viewport_row as f32 * cell_h;
                        cached_ui_rects.push(UIRect {
                            pos: [cx, cy],
                            size: [cell_w, cell_h],
                            color: [1.0, 0.75, 0.0, 0.8],
                        });
                    }
                }
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

            // Scrollbar: track + thumb on the right edge when scrollable content exists
            let has_history = history_size > 0 || display_offset > 0;
            if state.config.scrollbar && has_history {
                let bar_w = 6.0;
                let bar_x = rect.x + rect.w - bar_w - 1.0;
                let total = (pane_rows + history_size).max(1) as f32;
                let thumb_h = (content_h * pane_rows as f32 / total).max(12.0);
                let scroll_frac = (display_offset as f32 / history_size.max(1) as f32).min(1.0);
                let travel = content_h - thumb_h;
                let thumb_y = content_y + travel * (1.0 - scroll_frac);

                // Track
                cached_ui_rects.push(UIRect {
                    pos: [bar_x, content_y],
                    size: [bar_w, content_h],
                    color: state.theme.scrollbar_track,
                });
                // Thumb
                cached_ui_rects.push(UIRect {
                    pos: [bar_x, thumb_y],
                    size: [bar_w, thumb_h],
                    color: state.theme.scrollbar_thumb,
                });
            }

            // Subtle dim overlay on inactive panes (Hyprland-style focus visual).
            if !is_active && layouts.len() > 1 {
                cached_ui_rects.push(UIRect {
                    pos: [rect.x, rect.y],
                    size: [rect.w, rect.h],
                    color: [0.0, 0.0, 0.0, 0.12],
                });
            }

            // Only draw pane borders when there's more than one pane in the
            // active tab. With a single pane the borders just duplicate the
            // window edge and the top border draws a cyan line right below
            // the tab bar — which looks like a stray UI element.
            if layouts.len() > 1 {
                let border_color = if is_active {
                    let bell_active = state
                        .bell_flash_end
                        .map(|t| t > std::time::Instant::now())
                        .unwrap_or(false);
                    if bell_active {
                        [1.0_f32, 0.0, 0.047, 1.0] // #FF000C cyberpunk red
                    } else if state.effects_enabled
                        && state.config.effects.pane_pulse
                        && !state.config.reduce_motion
                    {
                        let pulse = (time_secs * std::f32::consts::PI).sin() * 0.5 + 0.5;
                        let alpha = 0.6 + pulse * 0.4;
                        let c = state.theme.panel_active_border;
                        [c[0], c[1], c[2], alpha]
                    } else {
                        state.theme.panel_active_border
                    }
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

        // Pane label overlay (Hyprland-style, 600ms after focus change)
        if label_active {
            let labeled_pane = PaneId(state.pane_label_id as u64);
            if let Some(pos) = layouts.iter().position(|(pid, _)| *pid == labeled_pane) {
                let rect = layouts[pos].1;
                let lx = rect.x + margin + 4.0;
                let ly = rect.y + margin + 4.0;
                let label_fs = 14.0;
                let label_char_w = label_fs * char_aspect;
                let label = format!("P{}", pos + 1);
                let transparent = [0.0_f32, 0.0, 0.0, 0.0];
                for (j, c) in label.chars().enumerate() {
                    cached_cell_data.push((
                        c,
                        lx + j as f32 * label_char_w,
                        ly,
                        label_fs,
                        state.theme.fg,
                        transparent,
                    ));
                }
            }
        }

        // Resize indicator: cols×rows centered in active pane while dragging divider
        if resize_active {
            let active_id = tab_bar.active_tab().active_pane;
            if let Some(pos) = layouts.iter().position(|(pid, _)| *pid == active_id) {
                let rect = layouts[pos].1;
                let content_w = (rect.w - margin * 2.0).max(0.0);
                let content_h = (rect.h - margin * 2.0).max(0.0);
                let cols = (content_w / cell_w).max(1.0) as usize;
                let rows = (content_h / cell_h).max(1.0) as usize;
                let indicator = format!("{}\u{00D7}{}", cols, rows);
                let ind_fs = 12.0;
                let ind_char_w = ind_fs * char_aspect;
                let ind_w = indicator.chars().count() as f32 * ind_char_w;
                let ind_x = rect.x + rect.w * 0.5 - ind_w * 0.5;
                let ind_y = rect.y + rect.h * 0.5 - ind_fs * 0.5;
                cached_ui_rects.push(UIRect {
                    pos: [ind_x - 6.0, ind_y - 4.0],
                    size: [ind_w + 12.0, ind_fs + 8.0],
                    color: [0.0, 0.0, 0.0, 0.7],
                });
                let transparent = [0.0_f32, 0.0, 0.0, 0.0];
                for (j, c) in indicator.chars().enumerate() {
                    cached_cell_data.push((
                        c,
                        ind_x + j as f32 * ind_char_w,
                        ind_y,
                        ind_fs,
                        state.theme.fg,
                        transparent,
                    ));
                }
            }
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
            let char_w = search_fs * char_aspect;
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
            let x_offset = prefix_chars.len();

            for (j, &c) in term_chars.iter().enumerate() {
                cached_cell_data.push((
                    c,
                    text_x + (x_offset + j) as f32 * char_w,
                    text_y,
                    search_fs,
                    state.theme.search_text,
                    transparent,
                ));
            }
            let cursor_col = x_offset + state.search.cursor_pos;
            cached_cell_data.push((
                '|',
                text_x + cursor_col as f32 * char_w,
                text_y,
                search_fs,
                state.theme.search_text,
                transparent,
            ));

            // Right side: regex indicator + match counter
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
            let (regex_tag, regex_color) = if state.search.regex_mode {
                if state.search.invalid_regex {
                    ("[!re] ", state.theme.search_highlight)
                } else {
                    ("[re] ", state.theme.search_current)
                }
            } else {
                ("[re] ", state.theme.search_text_dim)
            };
            let right_text = if counter.is_empty() {
                regex_tag.to_string()
            } else {
                format!("{}{}", regex_tag, counter)
            };
            let right_x = bar_x + bar_w - right_text.len() as f32 * char_w - 8.0;
            let regex_end = regex_tag.len();
            for (j, c) in right_text.chars().enumerate() {
                let color = if j < regex_end {
                    regex_color
                } else {
                    state.theme.search_text_dim
                };
                cached_cell_data.push((
                    c,
                    right_x + j as f32 * char_w,
                    text_y,
                    search_fs,
                    color,
                    transparent,
                ));
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
            let char_w = search_fs * char_aspect;
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

        // Workspace rename bar — covers the status bar while active
        if state.ws_rename.active {
            let bar_h = theme::STATUS_BAR_HEIGHT;
            let bar_y = layout.window_height - bar_h;
            let bar_w = layout.window_width - layout.sidebar_width;
            let bar_x = layout.sidebar_width;
            let transparent = [0.0f32, 0.0, 0.0, 0.0];

            cached_ui_rects.push(UIRect {
                pos: [bar_x, bar_y],
                size: [bar_w, bar_h],
                color: state.theme.search_bar_bg,
            });
            cached_ui_rects.push(UIRect {
                pos: [bar_x, bar_y],
                size: [bar_w, 1.5],
                color: state.theme.panel_active_border,
            });

            let fs = 13.0;
            let char_w = fs * char_aspect;
            let text_y = bar_y + (bar_h - fs) * 0.5;
            let text_x = bar_x + 8.0;
            let prefix = "Rename workspace: ";
            let accent = state.theme.panel_active_border;

            for (j, c) in prefix.chars().enumerate() {
                cached_cell_data.push((
                    c,
                    text_x + j as f32 * char_w,
                    text_y,
                    fs,
                    accent,
                    transparent,
                ));
            }
            let prefix_len = prefix.chars().count();
            for (j, c) in state.ws_rename.term.chars().enumerate() {
                cached_cell_data.push((
                    c,
                    text_x + (prefix_len + j) as f32 * char_w,
                    text_y,
                    fs,
                    state.theme.search_text,
                    transparent,
                ));
            }
            let cursor_col = prefix_len + state.ws_rename.cursor_pos;
            cached_cell_data.push((
                '|',
                text_x + cursor_col as f32 * char_w,
                text_y,
                fs,
                accent,
                transparent,
            ));
        }

        // Command palette overlay
        if state.palette.active {
            let p_fs = 13.0;
            let p_char_w = p_fs * char_aspect;
            let p_line_h = p_fs * 1.5;
            let pane_area = layout.pane_area();
            let overlay_w = 600.0_f32.min(pane_area.2 - 40.0);
            let max_results = 8;
            let overlay_h = 40.0 + max_results as f32 * p_line_h + 16.0;
            let ox = pane_area.0 + (pane_area.2 - overlay_w) / 2.0;
            let oy = pane_area.1 + (pane_area.3 * 0.2 - 40.0).max(16.0);
            let transparent = [0.0, 0.0, 0.0, 0.0];

            // Overlay background with border
            cached_ui_rects.push(UIRect {
                pos: [ox, oy],
                size: [overlay_w, overlay_h],
                color: state.theme.search_bar_bg,
            });
            // Border
            let border_color = state.theme.panel_active_border;
            cached_ui_rects.push(UIRect {
                pos: [ox, oy],
                size: [overlay_w, 1.0],
                color: border_color,
            });
            cached_ui_rects.push(UIRect {
                pos: [ox, oy + overlay_h - 1.0],
                size: [overlay_w, 1.0],
                color: border_color,
            });
            cached_ui_rects.push(UIRect {
                pos: [ox, oy],
                size: [1.0, overlay_h],
                color: border_color,
            });
            cached_ui_rects.push(UIRect {
                pos: [ox + overlay_w - 1.0, oy],
                size: [1.0, overlay_h],
                color: border_color,
            });

            // Search icon + query input
            let input_x = ox + 12.0;
            let input_y = oy + 12.0;
            cached_cell_data.push((
                '\u{1F50D}', // 🔍
                input_x,
                input_y,
                p_fs,
                state.theme.search_text,
                transparent,
            ));
            let text_x = input_x + p_char_w * 2.0;
            let term_chars: Vec<char> = state.palette.query.chars().collect();
            for (j, &c) in term_chars.iter().enumerate() {
                cached_cell_data.push((
                    c,
                    text_x + j as f32 * p_char_w,
                    input_y,
                    p_fs,
                    state.theme.search_text,
                    transparent,
                ));
            }
            // Cursor in query input
            cached_cell_data.push((
                '|',
                text_x + term_chars.len() as f32 * p_char_w,
                input_y,
                p_fs,
                state.theme.search_text,
                transparent,
            ));

            // Divider line between input and results
            let div_y = oy + 32.0;
            cached_ui_rects.push(UIRect {
                pos: [ox + 8.0, div_y],
                size: [overlay_w - 16.0, 1.0],
                color: state.theme.tab_separator,
            });

            // Results list
            let result_start_y = div_y + 8.0;
            let visible = state.palette.results.iter().take(max_results).enumerate();

            for (i, item) in visible {
                let row_y = result_start_y + i as f32 * p_line_h;
                let is_selected = i == state.palette.selected;
                let (label, keybind, _is_action) = match item {
                    crate::palette::PaletteItem::Action { label, keybind, .. } => {
                        (label.clone(), keybind.clone(), true)
                    }
                    crate::palette::PaletteItem::Tab { label, .. } => (label.clone(), None, false),
                    crate::palette::PaletteItem::Theme { name } => {
                        (format!("Theme: {}", name), None, false)
                    }
                    crate::palette::PaletteItem::Ssh { label, .. } => (label.clone(), None, false),
                    crate::palette::PaletteItem::TabProfile { label, .. } => {
                        (label.clone(), None, false)
                    }
                };

                // Highlight selected row
                if is_selected {
                    cached_ui_rects.push(UIRect {
                        pos: [ox + 4.0, row_y - 2.0],
                        size: [overlay_w - 8.0, p_line_h],
                        color: state.theme.search_highlight,
                    });
                }

                let text_fg = if is_selected {
                    state.theme.search_text
                } else {
                    state.theme.search_text_dim
                };

                let row_x = ox + 12.0;
                let prefix = "  ";
                for (j, c) in prefix.chars().enumerate() {
                    cached_cell_data.push((
                        c,
                        row_x + j as f32 * p_char_w,
                        row_y,
                        p_fs,
                        text_fg,
                        transparent,
                    ));
                }
                for (j, c) in label.chars().enumerate() {
                    cached_cell_data.push((
                        c,
                        row_x + prefix.len() as f32 * p_char_w + j as f32 * p_char_w,
                        row_y,
                        p_fs,
                        text_fg,
                        transparent,
                    ));
                }

                // Keybind hint, right-aligned
                if let Some(kb) = keybind {
                    let kb_x = ox + overlay_w - kb.len() as f32 * p_char_w - 12.0;
                    for (j, c) in kb.chars().enumerate() {
                        cached_cell_data.push((
                            c,
                            kb_x + j as f32 * p_char_w,
                            row_y,
                            p_fs,
                            state.theme.search_text_dim,
                            transparent,
                        ));
                    }
                }
            }

            // "no results" message
            if state.palette.results.is_empty() && !state.palette.query.is_empty() {
                let msg = "No matching results";
                let msg_x = ox + 12.0;
                for (j, c) in msg.chars().enumerate() {
                    cached_cell_data.push((
                        c,
                        msg_x + j as f32 * p_char_w,
                        result_start_y,
                        p_fs,
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
            state.effects_enabled,
        );
        cached_bg_rects.extend(tab_ui);

        // Wayland CSD title bar
        for rect in build_title_bar(layout, &state.theme) {
            cached_bg_rects.push(rect);
        }
        for cell in build_title_bar_text(layout, font_size, &state.theme, char_aspect) {
            cached_cell_data.push(cell);
        }

        for tab_cell in build_tab_bar_text(
            layout,
            tab_bar,
            scale_factor as f64,
            state.tab_scroll_offset,
            &state.theme,
            state.config.sidebar_show_process_dot,
            cell_w,
            cell_h,
        ) {
            cached_cell_data.push(tab_cell);
        }

        // Status bar
        let (sb_rects, sb_cells) = build_status_bar(
            layout,
            state,
            &tab_bar.active_tab().cwd.clone(),
            &state.active_workspace,
            char_aspect,
        );
        cached_bg_rects.extend(sb_rects);
        for cell in sb_cells {
            cached_cell_data.push(cell);
        }

        // Capture the active pane's DECSCUSR cursor style before rendering.
        {
            let active_id = tab_bar.active_tab().active_pane;
            if let Some(pane) = panes.iter().find(|p| p.id == active_id) {
                if let Ok(term) = pane.term.lock() {
                    let cs = term.cursor_style();
                    state.term_cursor_style = Some((cs.shape, cs.blinking));
                }
            }
        }

        // Cursor rects go last so blink-only updates can truncate+re-push without
        // touching the stable rects (borders, dividers, search bars, tab bar).
        *cached_cursor_pixel = cursor_pixel_for_frame;
        *cached_cursor_rects_start = cached_ui_rects.len();
        push_cursor_rect(
            cached_ui_rects,
            cursor_pixel_for_frame,
            cursor_alpha,
            cell_w,
            cell_h,
            state,
        );

        *cached_font_size = font_size;
        *cached_blink = cursor_alpha;
        *cached_active_tab = tab_bar.active;
    } else if blink_changed {
        // Pulse-only: truncate cursor rects and re-push with updated alpha.
        // Cell instances and bg rects are untouched — skips atlas lookups + GPU upload.
        cached_ui_rects.truncate(*cached_cursor_rects_start);
        push_cursor_rect(
            cached_ui_rects,
            *cached_cursor_pixel,
            cursor_alpha,
            cell_w,
            cell_h,
            state,
        );
        *cached_blink = cursor_alpha;
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

        // ── Draw background image per pane ─────────────────────────────────
        let bg_id = crate::image_protocol::BG_IMAGE_ID;
        if let Some(bg_image) = image_store.images.get(&bg_id) {
            if !renderer.has_image(bg_id) {
                renderer.upload_image(bg_id, &bg_image.rgba, bg_image.width, bg_image.height);
            }
            let img_w = bg_image.width as f32;
            let img_h = bg_image.height as f32;
            for (_pid, layout_rect) in &layouts {
                let content_x = layout_rect.x + margin;
                let content_y = layout_rect.y + margin;
                let cw = (layout_rect.w - margin * 2.0).max(1.0);
                let ch = (layout_rect.h - margin * 2.0).max(1.0);

                let clip = [content_x as u32, content_y as u32, cw as u32, ch as u32];
                for (x, y, w, h) in compute_bg_quads(
                    content_x,
                    content_y,
                    cw,
                    ch,
                    img_w,
                    img_h,
                    &state.config.background_mode,
                ) {
                    image_draws.push(ImageInstance {
                        pos: [x, y],
                        size: [w, h],
                    });
                    image_draw_ids.push(bg_id);
                    image_clips.push(clip);
                }
            }
        }

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

    render_overlay(
        cached_cell_data,
        cached_bg_rects,
        state,
        layout,
        cell_w,
        cell_h,
    );

    if state.profiler_active {
        render_profiler_cells(
            cached_cell_data,
            cached_bg_rects,
            state,
            layout,
            cell_w,
            cell_h,
        );
    }
    if state.keybinds_open {
        render_keybinds_cells(
            cached_cell_data,
            cached_bg_rects,
            state,
            layout,
            cell_w,
            cell_h,
        );
    }
    if state.settings_open {
        render_settings_cells(
            cached_cell_data,
            cached_bg_rects,
            state,
            layout,
            cell_w,
            cell_h,
        );
    }

    // cells_dirty = false when pty_received fired but every active pane's
    // damage list was empty (invisible escape sequences, mode queries, etc.).
    // In that case cached_cell_data is identical to the previous frame, so we
    // can reuse the GPU instance buffer without re-running atlas lookups.
    let cells_dirty = needs_cell_rebuild && any_pane_had_damage;

    renderer.draw_frame_with_options(
        cached_cell_data,
        cached_ui_rects,
        cached_bg_rects,
        cached_underline_instances,
        &image_draws,
        &image_draw_ids,
        &image_clips,
        state.config.font_ligatures,
        cells_dirty,
        needs_ui_rebuild,
    );

    exited_panes
}

fn render_overlay(
    cell_data: &mut CellData,
    bg_rects: &mut Vec<UIRect>,
    state: &AppState,
    layout: &Layout,
    cell_w: f32,
    cell_h: f32,
) {
    if !state.overlay.active {
        return;
    }

    let pane_area = layout.pane_area();

    let ov_w = pane_area.2 * 0.75;
    let ov_h = pane_area.3 * 0.75;
    let ov_x = pane_area.0 + (pane_area.2 - ov_w) / 2.0;
    let ov_y = pane_area.1 + (pane_area.3 - ov_h) / 2.0;

    let bg = [0.06, 0.07, 0.10, 0.92];
    let title_bg = [0.12, 0.14, 0.20, 1.0];
    let fg = state.theme.tab_text;
    let dim_fg = [fg[0], fg[1], fg[2], 0.6];

    bg_rects.push(UIRect {
        pos: [ov_x, ov_y],
        size: [ov_w, ov_h],
        color: bg,
    });

    let title_h = cell_h + 8.0;
    bg_rects.push(UIRect {
        pos: [ov_x, ov_y],
        size: [ov_w, title_h],
        color: title_bg,
    });

    let title_max_chars = ((ov_w - 20.0) / cell_w).max(1.0) as usize;
    let title_display = if state.overlay.title.len() > title_max_chars {
        &state.overlay.title[..title_max_chars.min(state.overlay.title.len())]
    } else {
        &state.overlay.title
    };
    for (i, c) in title_display.chars().enumerate() {
        cell_data.push((
            c,
            ov_x + 8.0 + i as f32 * cell_w,
            ov_y + 4.0,
            cell_h,
            fg,
            title_bg,
        ));
    }

    let close_label = "ESC to close";
    let close_x = ov_x + ov_w - close_label.len() as f32 * cell_w - 8.0;
    for (i, c) in close_label.chars().enumerate() {
        cell_data.push((
            c,
            close_x + i as f32 * cell_w,
            ov_y + 4.0,
            cell_h,
            dim_fg,
            title_bg,
        ));
    }

    let content_y = ov_y + title_h + 4.0;
    let content_h = ov_h - title_h - 8.0;
    let visible_lines = (content_h / cell_h).max(1.0) as usize;
    let visible = state.overlay.visible_lines(visible_lines);

    if visible.is_empty() && state.overlay.status == crate::overlay::OverlayStatus::Running {
        let msg = "Running...";
        let msg_x = ov_x + (ov_w - msg.len() as f32 * cell_w) / 2.0;
        for (i, c) in msg.chars().enumerate() {
            cell_data.push((c, msg_x + i as f32 * cell_w, content_y, cell_h, dim_fg, bg));
        }
    }

    for (row, line) in visible.iter().enumerate() {
        let line_display = if line.len() > title_max_chars + 10 {
            &line[..title_max_chars + 10]
        } else {
            line
        };
        for (col, c) in line_display.chars().enumerate() {
            cell_data.push((
                c,
                ov_x + 8.0 + col as f32 * cell_w,
                content_y + row as f32 * cell_h,
                cell_h,
                fg,
                bg,
            ));
        }
    }
}

fn render_profiler_cells(
    cell_data: &mut CellData,
    bg_rects: &mut Vec<UIRect>,
    state: &AppState,
    layout: &Layout,
    cell_w: f32,
    cell_h: f32,
) {
    let pane_area = layout.pane_area();
    let bg = [0.0f32, 0.0, 0.0, 0.6];
    let fg = state.theme.tab_text;

    let lines = [
        format!("FPS: {:.1}", state.profiler.fps),
        format!("Frame: {:.1}ms", state.profiler.frame_time_ms),
        format!("PTY: {:.1} KB/s", state.profiler.pty_bytes_per_sec / 1024.0),
        format!("Cells: {}", state.profiler.cell_count),
        format!("Atlas: {:.0}%", state.profiler.atlas_used_percent),
    ];

    let max_w = lines.iter().map(|l| l.len()).max().unwrap_or(0) as f32 * cell_w;
    let bg_w = max_w + 12.0;
    let bg_h = lines.len() as f32 * cell_h + 8.0;
    let bg_x = pane_area.0 + pane_area.2 - bg_w - 8.0;
    let bg_y = pane_area.1 + 4.0;

    bg_rects.push(UIRect {
        pos: [bg_x, bg_y],
        size: [bg_w, bg_h],
        color: bg,
    });

    for (i, line) in lines.iter().enumerate() {
        let tx = bg_x + 6.0;
        let ty = bg_y + 4.0 + i as f32 * cell_h;
        for (j, c) in line.chars().enumerate() {
            cell_data.push((c, tx + j as f32 * cell_w, ty, cell_h, fg, bg));
        }
    }
}

fn render_keybinds_cells(
    cell_data: &mut CellData,
    bg_rects: &mut Vec<UIRect>,
    state: &AppState,
    layout: &Layout,
    cell_w: f32,
    cell_h: f32,
) {
    let pane_area = layout.pane_area();
    let screen_w = pane_area.2;
    let screen_h = pane_area.3;

    let theme = &state.theme;
    let title_fg = theme.cursor;
    let header_fg = [1.0f32, 0.647, 0.0, 1.0];
    let key_fg = [0.0f32, 0.898, 1.0, 1.0];
    let desc_fg = theme.tab_text;
    let dim_fg = [
        theme.tab_text_inactive[0],
        theme.tab_text_inactive[1],
        theme.tab_text_inactive[2],
        1.0,
    ];
    let overlay_bg = [0.03f32, 0.04, 0.08, 0.94];
    let header_bg = [0.06f32, 0.08, 0.16, 1.0];

    enum Row {
        Section(&'static str),
        Bind {
            key: &'static str,
            desc: &'static str,
        },
        Spacer,
    }

    let rows: &[Row] = &[
        Row::Section("─── GENERAL ───────────────────────────────────────"),
        Row::Bind {
            key: "F1",
            desc: "Show / hide this keybinds panel",
        },
        Row::Bind {
            key: "F2",
            desc: "Open settings overlay",
        },
        Row::Bind {
            key: "Ctrl+,",
            desc: "Reload config (hot-reload)",
        },
        Row::Bind {
            key: "F11",
            desc: "Fullscreen",
        },
        Row::Bind {
            key: "F12",
            desc: "Toggle performance profiler",
        },
        Row::Spacer,
        Row::Section("─── TABS ───────────────────────────────────────────"),
        Row::Bind {
            key: "Ctrl+T",
            desc: "New tab",
        },
        Row::Bind {
            key: "Ctrl+W",
            desc: "Close tab",
        },
        Row::Bind {
            key: "Ctrl+Tab",
            desc: "Next tab",
        },
        Row::Bind {
            key: "Ctrl+Shift+Tab",
            desc: "Previous tab",
        },
        Row::Bind {
            key: "Ctrl+1..9",
            desc: "Jump to tab N",
        },
        Row::Spacer,
        Row::Section("─── PANES ──────────────────────────────────────────"),
        Row::Bind {
            key: "Ctrl+Shift+D",
            desc: "Split pane vertically",
        },
        Row::Bind {
            key: "Ctrl+Shift+H",
            desc: "Split pane horizontally",
        },
        Row::Bind {
            key: "Ctrl+Enter",
            desc: "Auto split",
        },
        Row::Bind {
            key: "Ctrl+Shift+W",
            desc: "Close pane",
        },
        Row::Bind {
            key: "Ctrl+Shift+Arrows",
            desc: "Navigate between panes",
        },
        Row::Bind {
            key: "Ctrl+Shift+Alt+Arrows",
            desc: "Resize pane",
        },
        Row::Bind {
            key: "Ctrl+Shift+Z",
            desc: "Zoom / unzoom active pane",
        },
        Row::Spacer,
        Row::Section("─── SEARCH & COPY ──────────────────────────────────"),
        Row::Bind {
            key: "Ctrl+Shift+F",
            desc: "In-buffer search",
        },
        Row::Bind {
            key: "Ctrl+R",
            desc: "History search",
        },
        Row::Bind {
            key: "Ctrl+Shift+Space",
            desc: "Enter copy mode (vim-style)",
        },
        Row::Bind {
            key: "Ctrl+Shift+C",
            desc: "Copy selection",
        },
        Row::Bind {
            key: "Ctrl+Shift+V",
            desc: "Paste",
        },
        Row::Spacer,
        Row::Section("─── NAVIGATION ─────────────────────────────────────"),
        Row::Bind {
            key: "Ctrl+Up",
            desc: "Jump to previous prompt mark",
        },
        Row::Bind {
            key: "Ctrl+Down",
            desc: "Jump to next prompt mark",
        },
        Row::Spacer,
        Row::Section("─── FONT ───────────────────────────────────────────"),
        Row::Bind {
            key: "Ctrl+=",
            desc: "Increase font size",
        },
        Row::Bind {
            key: "Ctrl+-",
            desc: "Decrease font size",
        },
        Row::Bind {
            key: "Ctrl+0",
            desc: "Reset font size",
        },
        Row::Spacer,
        Row::Section("─── WORKSPACE ──────────────────────────────────────"),
        Row::Bind {
            key: "Ctrl+Shift+N",
            desc: "New workspace",
        },
        Row::Bind {
            key: "Ctrl+Alt+Tab",
            desc: "Switch workspace",
        },
        Row::Bind {
            key: "Ctrl+Shift+Alt+R",
            desc: "Rename workspace",
        },
        Row::Bind {
            key: "Ctrl+Shift+Alt+D",
            desc: "Delete workspace",
        },
        Row::Spacer,
        Row::Section("─── MISC ───────────────────────────────────────────"),
        Row::Bind {
            key: "Ctrl+L",
            desc: "Clear screen",
        },
        Row::Bind {
            key: "Ctrl+Shift+S",
            desc: "Toggle status bar",
        },
        Row::Bind {
            key: "Ctrl+Shift+P",
            desc: "Command palette",
        },
        Row::Bind {
            key: "Ctrl+Shift+B",
            desc: "Broadcast to all panes",
        },
        Row::Bind {
            key: "Ctrl+Shift+R",
            desc: "Start / stop asciinema recording",
        },
        Row::Bind {
            key: "Ctrl+Shift+E",
            desc: "Toggle GPU effects",
        },
        Row::Spacer,
        Row::Section("─── COPY MODE ──────────────────────────────────────"),
        Row::Bind {
            key: "h j k l",
            desc: "Move cursor",
        },
        Row::Bind {
            key: "v / V",
            desc: "Char / line selection",
        },
        Row::Bind {
            key: "y",
            desc: "Yank (copy) selection",
        },
        Row::Bind {
            key: "/ or ?",
            desc: "Search forward / backward",
        },
        Row::Bind {
            key: "Esc / q",
            desc: "Exit copy mode",
        },
    ];

    let key_col_w: usize = 26;
    let desc_col_w: usize = 48;
    let total_w_chars = key_col_w + 2 + desc_col_w;
    let box_w = total_w_chars as f32 * cell_w + 24.0;
    let title_h = cell_h + 8.0;
    let footer_h = cell_h + 4.0;
    let visible_rows = ((screen_h - 80.0 - title_h - footer_h) / cell_h).floor() as usize;
    let box_h = visible_rows as f32 * cell_h + title_h + footer_h + 16.0;
    let box_x = pane_area.0 + (screen_w - box_w) * 0.5;
    let box_y = pane_area.1 + (screen_h - box_h) * 0.5;

    // Full-screen dim
    bg_rects.push(UIRect {
        pos: [pane_area.0, pane_area.1],
        size: [screen_w, screen_h],
        color: [0.0, 0.0, 0.0, 0.72],
    });

    // Main box
    bg_rects.push(UIRect {
        pos: [box_x, box_y],
        size: [box_w, box_h],
        color: overlay_bg,
    });

    // Title bar
    bg_rects.push(UIRect {
        pos: [box_x, box_y],
        size: [box_w, title_h],
        color: header_bg,
    });
    let title = " SYNAPSE_ KEYBINDS  \u{b7}  \u{2191}\u{2193} scroll  \u{b7}  Esc / F1 close ";
    for (j, c) in title.chars().enumerate() {
        cell_data.push((
            c,
            box_x + 12.0 + j as f32 * cell_w,
            box_y + 4.0,
            cell_h,
            title_fg,
            header_bg,
        ));
    }

    // Footer note
    let ny = box_y + box_h - footer_h;
    bg_rects.push(UIRect {
        pos: [box_x, ny],
        size: [box_w, footer_h],
        color: header_bg,
    });
    let note = " macOS/Linux: Ctrl = ^   Windows: not yet supported ";
    for (j, c) in note.chars().enumerate() {
        cell_data.push((
            c,
            box_x + 12.0 + j as f32 * cell_w,
            ny + 2.0,
            cell_h,
            dim_fg,
            header_bg,
        ));
    }

    let content_y_start = box_y + title_h + 4.0;
    let max_scroll = rows.len().saturating_sub(visible_rows);
    let scroll = state.keybinds_scroll.min(max_scroll);

    let cx = box_x + 12.0;
    for (i, row) in rows.iter().enumerate().skip(scroll).take(visible_rows) {
        let ry = content_y_start + (i - scroll) as f32 * cell_h;
        match row {
            Row::Section(label) => {
                bg_rects.push(UIRect {
                    pos: [box_x + 4.0, ry],
                    size: [box_w - 8.0, cell_h],
                    color: [0.05, 0.07, 0.14, 1.0],
                });
                for (j, c) in label.chars().enumerate() {
                    cell_data.push((
                        c,
                        cx + j as f32 * cell_w,
                        ry,
                        cell_h,
                        header_fg,
                        [0.05, 0.07, 0.14, 1.0],
                    ));
                }
            }
            Row::Bind { key, desc } => {
                let padded_key = format!("{:<width$}", key, width = key_col_w);
                for (j, c) in padded_key.chars().enumerate() {
                    cell_data.push((c, cx + j as f32 * cell_w, ry, cell_h, key_fg, overlay_bg));
                }
                let sep_x = cx + key_col_w as f32 * cell_w;
                for (j, c) in "  ".chars().enumerate() {
                    cell_data.push((c, sep_x + j as f32 * cell_w, ry, cell_h, dim_fg, overlay_bg));
                }
                let dx = sep_x + 2.0 * cell_w;
                let truncated: String = desc.chars().take(desc_col_w).collect();
                for (j, c) in truncated.chars().enumerate() {
                    cell_data.push((c, dx + j as f32 * cell_w, ry, cell_h, desc_fg, overlay_bg));
                }
            }
            Row::Spacer => {}
        }
    }
}

fn render_settings_cells(
    cell_data: &mut CellData,
    bg_rects: &mut Vec<UIRect>,
    state: &AppState,
    layout: &Layout,
    cell_w: f32,
    cell_h: f32,
) {
    use synapse_config::config::CursorStyle;

    let pane_area = layout.pane_area();
    let screen_w = pane_area.2;
    let screen_h = pane_area.3;

    let theme = &state.theme;
    let overlay_bg = [0.03f32, 0.04, 0.08, 0.96];
    let header_bg = [0.06f32, 0.08, 0.16, 1.0];
    let title_fg = theme.cursor;
    let section_fg = [1.0f32, 0.647, 0.0, 1.0];
    let label_fg = theme.tab_text;
    let value_fg = [0.0f32, 0.898, 1.0, 1.0];
    let dim_fg = [
        theme.tab_text_inactive[0],
        theme.tab_text_inactive[1],
        theme.tab_text_inactive[2],
        1.0,
    ];
    let sel_bg = [0.06f32, 0.12, 0.24, 1.0];
    let sel_fg = [1.0f32, 1.0, 1.0, 1.0];

    struct Item {
        label: &'static str,
        value: String,
        section: Option<&'static str>,
    }

    let cfg = &state.config;
    let bool_str = |b: bool| -> String {
        if b {
            "ON".into()
        } else {
            "OFF".into()
        }
    };
    let cursor_str = |cs: &CursorStyle| -> String {
        match cs {
            CursorStyle::Block => "Block".into(),
            CursorStyle::Beam => "Beam".into(),
            CursorStyle::Underline => "Underline".into(),
            CursorStyle::NeonUnderbar => "NeonUnderbar".into(),
        }
    };

    let items: &[Item] = &[
        Item {
            label: "Font Size",
            value: format!("{:.0}", cfg.font_size),
            section: Some("FONT"),
        },
        Item {
            label: "Font Ligatures",
            value: bool_str(cfg.font_ligatures),
            section: None,
        },
        Item {
            label: "Theme",
            value: cfg.theme.clone(),
            section: Some("APPEARANCE"),
        },
        Item {
            label: "Cursor Style",
            value: cursor_str(&cfg.cursor_style),
            section: None,
        },
        Item {
            label: "Cursor Blink",
            value: bool_str(cfg.cursor_blink),
            section: None,
        },
        Item {
            label: "Status Bar",
            value: bool_str(cfg.status_bar),
            section: Some("UI"),
        },
        Item {
            label: "Scrollbar",
            value: bool_str(cfg.scrollbar),
            section: None,
        },
        Item {
            label: "Show Pane Labels",
            value: bool_str(cfg.show_pane_labels),
            section: None,
        },
        Item {
            label: "Pane Badge",
            value: bool_str(cfg.pane_badge),
            section: None,
        },
        Item {
            label: "Effects",
            value: bool_str(cfg.effects.enabled),
            section: Some("EFFECTS"),
        },
    ];

    let label_w: usize = 20;
    let value_w: usize = 14;
    let hint_w: usize = 8;
    let total_chars = label_w + 2 + value_w + 2 + hint_w;
    let box_w = total_chars as f32 * cell_w + 32.0;
    let title_h = cell_h + 8.0;
    let footer_h = cell_h + 6.0;
    let section_rows = items.iter().filter(|it| it.section.is_some()).count();
    let content_rows = items.len() + section_rows;
    let box_h = content_rows as f32 * cell_h + title_h + footer_h + 20.0;
    let box_x = pane_area.0 + (screen_w - box_w) * 0.5;
    let box_y = pane_area.1 + (screen_h - box_h) * 0.5;

    // Full-screen dim
    bg_rects.push(UIRect {
        pos: [pane_area.0, pane_area.1],
        size: [screen_w, screen_h],
        color: [0.0, 0.0, 0.0, 0.72],
    });

    // Main box
    bg_rects.push(UIRect {
        pos: [box_x, box_y],
        size: [box_w, box_h],
        color: overlay_bg,
    });

    // Title bar
    bg_rects.push(UIRect {
        pos: [box_x, box_y],
        size: [box_w, title_h],
        color: header_bg,
    });
    let title = " SETTINGS  \u{b7}  F2 close  \u{b7}  S save ";
    for (j, c) in title.chars().enumerate() {
        cell_data.push((
            c,
            box_x + 12.0 + j as f32 * cell_w,
            box_y + 4.0,
            cell_h,
            title_fg,
            header_bg,
        ));
    }

    // Footer
    let fy = box_y + box_h - footer_h;
    bg_rects.push(UIRect {
        pos: [box_x, fy],
        size: [box_w, footer_h],
        color: header_bg,
    });
    let footer =
        " \u{2191}\u{2193} navigate   \u{2190}\u{2192} change   S save & close   Esc cancel ";
    for (j, c) in footer.chars().enumerate() {
        cell_data.push((
            c,
            box_x + 8.0 + j as f32 * cell_w,
            fy + 3.0,
            cell_h,
            dim_fg,
            header_bg,
        ));
    }

    let cx = box_x + 12.0;
    let mut row_y = box_y + title_h + 6.0;

    for (item_idx, item) in items.iter().enumerate() {
        // Section header
        if let Some(sec) = item.section {
            let sec_label = format!("\u{2500}\u{2500}\u{2500} {} ", sec);
            bg_rects.push(UIRect {
                pos: [box_x + 4.0, row_y],
                size: [box_w - 8.0, cell_h],
                color: [0.04, 0.06, 0.12, 1.0],
            });
            for (j, c) in sec_label.chars().enumerate() {
                cell_data.push((
                    c,
                    cx + j as f32 * cell_w,
                    row_y,
                    cell_h,
                    section_fg,
                    [0.04, 0.06, 0.12, 1.0],
                ));
            }
            row_y += cell_h;
        }

        let is_selected = item_idx == state.settings_item;
        let row_bg = if is_selected { sel_bg } else { overlay_bg };
        let row_label_fg = if is_selected { sel_fg } else { label_fg };
        let row_value_fg = if is_selected {
            [0.0f32, 1.0, 0.8, 1.0]
        } else {
            value_fg
        };

        if is_selected {
            bg_rects.push(UIRect {
                pos: [box_x + 2.0, row_y],
                size: [box_w - 4.0, cell_h],
                color: sel_bg,
            });
        }

        // Label
        let padded_label = format!("{:<width$}", item.label, width = label_w);
        for (j, c) in padded_label.chars().enumerate() {
            cell_data.push((
                c,
                cx + j as f32 * cell_w,
                row_y,
                cell_h,
                row_label_fg,
                row_bg,
            ));
        }

        // Separator
        let sep_x = cx + label_w as f32 * cell_w;
        for (j, c) in "  ".chars().enumerate() {
            cell_data.push((c, sep_x + j as f32 * cell_w, row_y, cell_h, dim_fg, row_bg));
        }

        // Value
        let vx = sep_x + 2.0 * cell_w;
        let padded_val = format!("{:<width$}", item.value, width = value_w);
        for (j, c) in padded_val.chars().enumerate() {
            cell_data.push((
                c,
                vx + j as f32 * cell_w,
                row_y,
                cell_h,
                row_value_fg,
                row_bg,
            ));
        }

        // Hint arrows (only on selected row)
        if is_selected {
            let hx = vx + value_w as f32 * cell_w + cell_w;
            let hint = "\u{2190} \u{2192}";
            for (j, c) in hint.chars().enumerate() {
                cell_data.push((c, hx + j as f32 * cell_w, row_y, cell_h, dim_fg, row_bg));
            }
        }

        row_y += cell_h;
    }
}

const SPLASH_DURATION_SECS: f32 = 3.5;

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
    let accent = theme.cursor;
    let fg = theme.fg;
    let dim_fg = [fg[0], fg[1], fg[2], 0.55_f32];
    let mid_fg = [fg[0], fg[1], fg[2], 0.85_f32];
    let full_fg = [fg[0], fg[1], fg[2], 1.0_f32];
    let mid_acc = [accent[0], accent[1], accent[2], 0.75_f32];
    let full_acc = [accent[0], accent[1], accent[2], 1.0_f32];
    let dim_acc = [accent[0], accent[1], accent[2], 0.40_f32];

    // ── 1. Background + pulsing red glow ────────────────────────────────────
    bg_rects.push(UIRect {
        pos: [0.0, 0.0],
        size: [w, h],
        color: theme.bg,
    });
    let pulse = ((progress * std::f32::consts::TAU * 0.8).sin() * 0.5 + 0.5) * 0.02 + 0.03;
    bg_rects.push(UIRect {
        pos: [w * 0.08, h * 0.12],
        size: [w * 0.84, h * 0.76],
        color: [accent[0], accent[1], accent[2], pulse],
    });
    bg_rects.push(UIRect {
        pos: [0.0, 0.0],
        size: [w * 0.35, h * 0.35],
        color: [accent[0], accent[1], accent[2], 0.010],
    });

    // ── 2. Hex data rain — more rows, more visible ──────────────────────────
    let hex_digits = b"0123456789ABCDEF";
    let hex_fs: f32 = 12.0;
    let hex_cw = hex_fs * 0.60;
    let hex_scroll = progress * 6.0;
    for row in 0..8usize {
        let y_raw = row as f32 * 18.0 - (hex_scroll * 18.0).rem_euclid(8.0 * 18.0);
        let y_base = y_raw.rem_euclid(h * 0.42) + h * 0.02;
        let alpha = 0.12 + row as f32 * 0.028;
        let row_col = [fg[0], fg[1], fg[2], alpha];
        let addr_hi = ((row as u32 + 1)
            .wrapping_mul(0xDEAD)
            .wrapping_add((progress * 400.0) as u32))
            & 0xFFFF;
        let addr_lo = (row as u32).wrapping_mul(0x1337) & 0xFFFF;
        let addr = format!("0x{:04X}:{:04X}  ", addr_hi, addr_lo);
        let mut x = w * 0.025;
        for c in addr.chars() {
            cells.push((c, x, y_base, hex_fs, row_col, transparent));
            x += hex_cw;
        }
        for col in 0..52usize {
            let v = ((col as f32 * 7.31 + row as f32 * 19.13 + hex_scroll * 3.77) * 13.7).sin();
            let idx = ((v * 7.5 + 7.5) as usize).min(15);
            let c = hex_digits[idx] as char;
            cells.push((c, x, y_base, hex_fs, row_col, transparent));
            x += hex_cw;
            if col % 4 == 3 {
                x += hex_cw * 0.6;
            }
        }
    }

    // ── 3. CRT scan lines ───────────────────────────────────────────────────
    let scan1 = ((progress * 0.85) % 1.0) * h;
    let scan2 = ((progress * 0.55 + 0.4) % 1.0) * h;
    let scan3 = ((progress * 0.32 + 0.7) % 1.0) * h;
    ui_rects.push(UIRect {
        pos: [0.0, scan1],
        size: [w, 2.0],
        color: [accent[0], accent[1], accent[2], 0.14],
    });
    ui_rects.push(UIRect {
        pos: [0.0, scan2],
        size: [w, 1.5],
        color: [fg[0], fg[1], fg[2], 0.07],
    });
    ui_rects.push(UIRect {
        pos: [0.0, scan3],
        size: [w, 1.0],
        color: [accent[0], accent[1], accent[2], 0.06],
    });

    // ── 4. Vertical rail decorations ────────────────────────────────────────
    let rail_top = h * 0.10;
    let rail_bot = h * 0.90;
    let rail_h = rail_bot - rail_top;
    let lx = w * 0.08;
    let rx = w * 0.92;
    for seg in 0..14usize {
        let sy = rail_top + seg as f32 * rail_h / 14.0;
        let sl = rail_h / 14.0 * 0.68;
        ui_rects.push(UIRect {
            pos: [lx, sy],
            size: [1.5, sl],
            color: dim_fg,
        });
        ui_rects.push(UIRect {
            pos: [rx, sy],
            size: [1.5, sl],
            color: dim_fg,
        });
    }
    for ty in &[rail_top, rail_bot] {
        ui_rects.push(UIRect {
            pos: [lx - 6.0, *ty],
            size: [14.0, 1.5],
            color: mid_fg,
        });
        ui_rects.push(UIRect {
            pos: [rx - 6.0, *ty],
            size: [14.0, 1.5],
            color: mid_fg,
        });
        ui_rects.push(UIRect {
            pos: [lx - 2.0, *ty - 5.0],
            size: [1.5, 10.0],
            color: mid_acc,
        });
        ui_rects.push(UIRect {
            pos: [rx - 1.0, *ty - 5.0],
            size: [1.5, 10.0],
            color: mid_acc,
        });
    }

    // ── 5. Top header ───────────────────────────────────────────────────────
    let hdr_y = h * 0.14;
    let hdr_fs: f32 = 13.0;
    let hdr_cw = hdr_fs * 0.60;
    ui_rects.push(UIRect {
        pos: [lx + 8.0, hdr_y - 12.0],
        size: [rx - lx - 16.0, 1.5],
        color: dim_fg,
    });
    let hdr_str = "[ WINTERMUTE  ·  CORTEX I/O  ·  JACK IN ]";
    let hdr_w = hdr_str.chars().count() as f32 * hdr_cw;
    let hdr_x = (w - hdr_w) * 0.5;
    for (j, c) in hdr_str.chars().enumerate() {
        let col = if c == '[' || c == ']' || c == '·' {
            mid_acc
        } else {
            dim_fg
        };
        cells.push((
            c,
            hdr_x + j as f32 * hdr_cw,
            hdr_y,
            hdr_fs,
            col,
            transparent,
        ));
    }

    // ── 6. Main title — glitch reveal ───────────────────────────────────────
    let title_fs: f32 = 52.0;
    let title_cw = title_fs * 0.60;
    let title_chars: Vec<char> = "SYNAPSE_".chars().collect();
    let n = title_chars.len();
    let title_w = n as f32 * title_cw;
    let title_x = (w - title_w) * 0.5;
    let title_y = h * 0.33;

    // Layered glow behind title
    bg_rects.push(UIRect {
        pos: [title_x - 40.0, title_y - 16.0],
        size: [title_w + 80.0, title_fs + 32.0],
        color: [accent[0], accent[1], accent[2], 0.09],
    });
    bg_rects.push(UIRect {
        pos: [title_x - 12.0, title_y - 5.0],
        size: [title_w + 24.0, title_fs + 10.0],
        color: [accent[0], accent[1], accent[2], 0.05],
    });

    let glitch_pool = b"!@#$%^&*<>[]{}/\\|?~XZQW0987654321";
    let gp_len = glitch_pool.len();
    for (j, &real_ch) in title_chars.iter().enumerate() {
        let reveal_at = j as f32 / n as f32 * 0.50;
        let char_p = (progress - reveal_at) / 0.09;

        let (display_ch, col) = if char_p >= 1.0 {
            let flicker = (progress * 47.3 + j as f32 * 11.1).sin();
            if flicker > 0.96 && progress < 0.65 {
                let gi = (((j as f32 * 17.3 + progress * 210.0).sin() * 0.5 + 0.5) * gp_len as f32)
                    as usize;
                (glitch_pool[gi.min(gp_len - 1)] as char, mid_acc)
            } else {
                (real_ch, full_fg)
            }
        } else if char_p >= 0.0 {
            let gi =
                (((j as f32 * 13.7 + progress * 190.0).sin() * 0.5 + 0.5) * gp_len as f32) as usize;
            (glitch_pool[gi.min(gp_len - 1)] as char, full_acc)
        } else if real_ch == ' ' {
            (' ', transparent)
        } else {
            let gi =
                (((j as f32 * 9.1 + progress * 55.0).sin() * 0.5 + 0.5) * gp_len as f32) as usize;
            (glitch_pool[gi.min(gp_len - 1)] as char, dim_acc)
        };

        cells.push((
            display_ch,
            title_x + j as f32 * title_cw,
            title_y,
            title_fs,
            col,
            transparent,
        ));
    }

    // ── 7. Separator with chevron endcaps ───────────────────────────────────
    let sep_y = title_y + title_fs + 20.0;
    let sep_w = (title_w * 1.30).min(w * 0.74);
    let sep_x = (w - sep_w) * 0.5;
    ui_rects.push(UIRect {
        pos: [sep_x + 20.0, sep_y + 8.0],
        size: [sep_w - 40.0, 1.5],
        color: mid_fg,
    });
    let chev_fs: f32 = 15.0;
    cells.push(('▶', sep_x, sep_y, chev_fs, mid_acc, transparent));
    cells.push((
        '◀',
        sep_x + sep_w - chev_fs * 0.60,
        sep_y,
        chev_fs,
        mid_acc,
        transparent,
    ));

    // ── 8. Subtitle (fade in after title) ───────────────────────────────────
    let sub_fs: f32 = 15.0;
    let sub_cw = sub_fs * 0.60;
    let subtitle_chars: Vec<char> = "NEURAL INTERFACE  ──  v0.2.0".chars().collect();
    let sub_w = subtitle_chars.len() as f32 * sub_cw;
    let sub_x = (w - sub_w) * 0.5;
    let sub_y = sep_y + 24.0;
    let sub_alpha = ((progress - 0.38) / 0.14).clamp(0.0, 1.0);
    let sub_col = [fg[0], fg[1], fg[2], 0.88 * sub_alpha];
    for (j, &c) in subtitle_chars.iter().enumerate() {
        cells.push((
            c,
            sub_x + j as f32 * sub_cw,
            sub_y,
            sub_fs,
            sub_col,
            transparent,
        ));
    }

    // ── 9. Block progress bar ───────────────────────────────────────────────
    let bar_y = sub_y + 60.0;
    let bar_fs: f32 = 15.0;
    let bar_cw = bar_fs * 0.60;
    let bar_blocks: usize = 28;
    let filled = (progress * bar_blocks as f32) as usize;
    let bar_total_w = (bar_blocks + 2) as f32 * bar_cw;
    let bar_x = (w - bar_total_w) * 0.5 - 22.0;
    cells.push(('[', bar_x, bar_y, bar_fs, mid_fg, transparent));
    for i in 0..bar_blocks {
        let (c, col) = if i < filled {
            ('█', full_acc)
        } else {
            ('░', dim_fg)
        };
        cells.push((
            c,
            bar_x + (i + 1) as f32 * bar_cw,
            bar_y,
            bar_fs,
            col,
            transparent,
        ));
    }
    cells.push((
        ']',
        bar_x + (bar_blocks + 1) as f32 * bar_cw,
        bar_y,
        bar_fs,
        mid_fg,
        transparent,
    ));
    let pct_str = format!("  {:>3}%", (progress * 100.0) as u32);
    for (j, c) in pct_str.chars().enumerate() {
        cells.push((
            c,
            bar_x + (bar_blocks + 2) as f32 * bar_cw + j as f32 * bar_cw,
            bar_y,
            bar_fs,
            mid_fg,
            transparent,
        ));
    }

    // ── 10. Status line + blinking cursor ───────────────────────────────────
    let st_fs: f32 = 14.0;
    let st_cw = st_fs * 0.60;
    let st_msg = match (progress * 5.5) as u32 {
        0 => "CORTEX UPLINK INITIALIZING",
        1 => "SIMSTIM DECK CALIBRATING",
        2 => "NEURAL LACE HANDSHAKE",
        3 => "ICE BOUNDARY DISSOLVING",
        4 => "SECURE CHANNEL ESTABLISHED",
        _ => "SYSTEM READY  //  JACK IN",
    };
    let st_prefix = ">  ";
    let full_st = format!("{}{}", st_prefix, st_msg);
    let st_w = full_st.chars().count() as f32 * st_cw;
    let st_x = (w - st_w) * 0.5;
    let st_y = bar_y + 30.0;
    let st_col = if progress >= 0.95 { full_acc } else { mid_fg };
    for (j, c) in full_st.chars().enumerate() {
        let col = if j < st_prefix.len() { mid_acc } else { st_col };
        cells.push((c, st_x + j as f32 * st_cw, st_y, st_fs, col, transparent));
    }
    let cursor_on = (progress * std::f32::consts::TAU * 4.2).sin() > 0.0;
    if cursor_on {
        let cx = st_x + full_st.chars().count() as f32 * st_cw + st_cw * 0.4;
        cells.push(('▌', cx, st_y, st_fs, full_acc, transparent));
    }

    // ── 11. Module scan list ─────────────────────────────────────────────────
    let mod_fs: f32 = 13.0;
    let mod_cw = mod_fs * 0.60;
    let modules: &[(&str, f32)] = &[
        ("matrix.core", 0.18),
        ("ice.breaker", 0.38),
        ("cortex.link", 0.52),
        ("pty.backend", 0.64),
        ("uplink.proto", 0.78),
    ];
    let col_gap = 20.0 * mod_cw;
    let total_mw = modules.len() as f32 * col_gap;
    let mut mx = (w - total_mw) * 0.5;
    let my = st_y + 40.0;
    for &(name, appear_at) in modules {
        if progress >= appear_at {
            let settled = progress > appear_at + 0.14;
            let ok_col: [f32; 4] = if settled {
                [0.30, 0.90, 0.42, 0.92]
            } else {
                mid_acc
            };
            let label = if settled {
                format!("[OK] {}", name)
            } else {
                format!("[▪▪] {}", name)
            };
            for (j, c) in label.chars().enumerate() {
                let col = if j <= 3 { ok_col } else { dim_fg };
                cells.push((c, mx + j as f32 * mod_cw, my, mod_fs, col, transparent));
            }
        }
        mx += col_gap;
    }

    // ── 12. Bottom bar ──────────────────────────────────────────────────────
    let bot_y = h * 0.87;
    ui_rects.push(UIRect {
        pos: [lx + 8.0, bot_y],
        size: [rx - lx - 16.0, 1.5],
        color: dim_fg,
    });
    let corp_fs: f32 = 12.0;
    let corp_cw = corp_fs * 0.60;
    let corp_str = "SYNAPSE_  ·  CHIBA CITY CONSTRUCT  ·  2049";
    let corp_w = corp_str.chars().count() as f32 * corp_cw;
    let corp_x = (w - corp_w) * 0.5;
    for (j, c) in corp_str.chars().enumerate() {
        let col = if c == '·' {
            mid_acc
        } else {
            [fg[0], fg[1], fg[2], 0.50]
        };
        cells.push((
            c,
            corp_x + j as f32 * corp_cw,
            bot_y + 12.0,
            corp_fs,
            col,
            transparent,
        ));
    }

    renderer.draw_frame_with_options(
        &cells,
        &ui_rects,
        &bg_rects,
        &[],
        &[],
        &[],
        &[],
        false,
        true,
        true,
    );
}

pub fn send_desktop_notification(title: &str, body: &str) {
    let _ = notify_rust::Notification::new()
        .summary(title)
        .body(body)
        .timeout(notify_rust::Timeout::Milliseconds(4000))
        .show();
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
        for pane in self.workspaces.active_panes_mut().iter_mut() {
            // Snapshot cursor position once per pane; used as the image origin for
            // placements that rely on cursor-based positioning (the Kitty spec default).
            let (cursor_col, cursor_row) = if let Ok(term) = pane.term.try_lock() {
                let pt = term.grid().cursor.point;
                (pt.column.0, pt.line.0.max(0) as usize)
            } else {
                (0, 0)
            };
            while let Ok(raw) = pane.apc_rx.try_recv() {
                if let Some(cmd) = parse_apc(&raw) {
                    if matches!(cmd.action, KittyAction::Delete) && cmd.image_id != 0 {
                        self.renderer.remove_image(cmd.image_id);
                    }
                    self.image_store
                        .process(cmd, cursor_col, cursor_row, Some(pane.id));
                }
            }
        }

        // Drain Sixel sequences from all panes into the image store.
        if self.state.config.sixel_enabled {
            for pane in self.workspaces.active_panes_mut().iter_mut() {
                while let Ok(raw) = pane.sixel_rx.try_recv() {
                    if let Some(result) = crate::sixel::decode_sixel(&raw) {
                        let image_rows = (result.height as f32 / self.cell_h).ceil() as usize;
                        let (cursor_col, cursor_row) = if let Ok(mut term) = pane.term.lock() {
                            let point = term.grid().cursor.point;
                            let row = point.line.0.max(0) as usize;

                            if image_rows > 0
                                && row + image_rows > pane.rows
                                && pane.rows > 0
                                && term.grid().display_offset() == 0
                            {
                                let overflow = (row + image_rows) - pane.rows;
                                use alacritty_terminal::index::Line;
                                term.grid_mut()
                                    .scroll_up(&(Line(0)..Line(pane.rows as i32)), overflow);
                            }
                            (point.column.0, row)
                        } else {
                            (0, 0)
                        };
                        let next_id = (self.image_store.images.len() as u32).max(1) + 1;
                        self.image_store.accept_sixel(
                            next_id,
                            result.width,
                            result.height,
                            result.rgba,
                            cursor_col,
                            cursor_row,
                            Some(pane.id),
                        );
                    }
                }
            }
        }

        // Drain iTerm2 OSC 1337 inline image sequences from all panes.
        if self.state.config.iterm2_images {
            for pane in self.workspaces.active_panes_mut().iter_mut() {
                while let Ok(encoded) = pane.iterm2_rx.try_recv() {
                    let (meta, b64) = if let Some(pos) = encoded.find(':') {
                        (&encoded[..pos], &encoded[pos + 1..])
                    } else {
                        continue;
                    };
                    let mut meta_parts = meta.splitn(3, ',');
                    let _display_width: u32 =
                        meta_parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                    let _display_height: u32 =
                        meta_parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                    let _preserve_aspect: bool = meta_parts
                        .next()
                        .and_then(|s| s.parse::<u8>().ok())
                        .unwrap_or(0)
                        != 0;

                    let raw_bytes = match base64::engine::general_purpose::STANDARD.decode(b64) {
                        Ok(b) => b,
                        Err(_) => continue,
                    };

                    let (img_w, img_h, rgba, frames) =
                        match crate::image_protocol::decode_animated(&raw_bytes) {
                            Some(v) => v,
                            None => continue,
                        };

                    let image_rows = (img_h as f32 / self.cell_h).ceil() as usize;
                    let (cursor_col, cursor_row) = if let Ok(mut term) = pane.term.lock() {
                        let point = term.grid().cursor.point;
                        let row = point.line.0.max(0) as usize;

                        if image_rows > 0
                            && row + image_rows > pane.rows
                            && pane.rows > 0
                            && term.grid().display_offset() == 0
                        {
                            let overflow = (row + image_rows) - pane.rows;
                            use alacritty_terminal::index::Line;
                            term.grid_mut()
                                .scroll_up(&(Line(0)..Line(pane.rows as i32)), overflow);
                        }
                        (point.column.0, row)
                    } else {
                        (0, 0)
                    };
                    let next_id = (self.image_store.images.len() as u32).max(1) + 1;
                    self.image_store.accept_iterm2(
                        next_id,
                        img_w,
                        img_h,
                        &rgba,
                        frames,
                        cursor_col,
                        cursor_row,
                        Some(pane.id),
                    );
                }
            }
        }

        // Drain bell signals from all panes.
        for pane in self.workspaces.active_panes_mut().iter_mut() {
            if pane.pending_bell {
                pane.pending_bell = false;
                if self.state.config.bell.visual {
                    self.state.bell_flash_end =
                        Some(std::time::Instant::now() + std::time::Duration::from_millis(200));
                }
                if !self.state.window_focused && self.state.config.bell.notify_unfocused {
                    send_desktop_notification("SYNAPSE_", "Bell");
                }
            }
        }

        // Drain OSC 9/777 desktop notifications from all panes (only when unfocused).
        for pane in self.workspaces.active_panes_mut().iter_mut() {
            while let Some(raw) = pane.notifications.pop_front() {
                if !self.state.window_focused {
                    let (title, body) = if let Some(sep) = raw.find('\x00') {
                        (&raw[..sep], &raw[sep + 1..])
                    } else {
                        ("SYNAPSE_", raw.as_str())
                    };
                    send_desktop_notification(title, body);
                }
            }
        }

        // Drain OSC 52 clipboard operations.
        for pane in self.workspaces.active_panes_mut().iter_mut() {
            while let Some(op) = pane.clipboard_pending.pop_front() {
                match op {
                    synapse_ui::pane::ClipboardOp::Write(_, data) => {
                        if let Some(cb) = self.clipboard.as_mut() {
                            let _ = cb.set_text(data);
                        }
                    }
                    synapse_ui::pane::ClipboardOp::Read(_, fmt) => {
                        let text = self
                            .clipboard
                            .as_mut()
                            .and_then(|cb| cb.get_text().ok())
                            .unwrap_or_default();
                        let response = fmt(&text);
                        if let Ok(mut w) = pane.pty_writer.lock() {
                            let _ = std::io::Write::write_all(&mut *w, response.as_bytes());
                        }
                    }
                }
            }
        }

        self.frame_count += 1;
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.fps_last_print);
        if elapsed >= std::time::Duration::from_secs(1) {
            let fps = self.frame_count as f64 / elapsed.as_secs_f64();
            // Only emit the per-second FPS line when the profiler overlay is on;
            // it is otherwise console spam (and visible on-screen via F12).
            if self.state.profiler_active {
                tracing::info!(target: "synapse_::bench", "FPS: {:.1}", fps);
            }
            self.frame_count = 0;
            self.fps_last_print = now;
        }

        if self.state.profiler_active {
            let dt = now.duration_since(self.fps_last_print).as_secs_f32() * 1000.0;
            self.state.profiler.add_frame_time(dt);
        }

        let time_secs = self.start_time.elapsed().as_secs_f32();
        let cursor_alpha = if self.state.config.cursor_blink && !self.state.config.reduce_motion {
            let period = self.state.config.cursor_blink_ms as f32 / 500.0;
            // Smooth cosine pulse: starts fully bright, fades to 0, returns.
            (std::f32::consts::PI * time_secs / period).cos() * 0.5 + 0.5
        } else {
            1.0
        };

        self.poll_overlay_events();

        // Advance animated images (GIF / APNG). Re-upload GPU textures for any
        // frame that advanced. render_instances always runs so the new textures
        // take effect this frame without any additional dirty-flag plumbing.
        {
            let now_ms = self.start_time.elapsed().as_millis() as u64;
            let advanced = self.image_store.tick_animations(now_ms);
            for id in advanced {
                if let Some((rgba, w, h)) = self.image_store.current_frame_rgba(id) {
                    self.renderer.upload_image(id, rgba, w, h);
                }
            }
        }

        let effective_fs = self.state.font_size * self.scale_factor;
        let (tab_bar, panes, cell_caches) = self.workspaces.active_split_mut();
        let exited = render_frame(
            &mut self.renderer,
            &self.layout,
            tab_bar,
            panes,
            &self.image_store,
            &mut self.state,
            self.cell_w,
            self.cell_h,
            cursor_alpha,
            &mut self.cached_cell_data,
            &mut self.cached_ui_rects,
            &mut self.cached_bg_rects,
            &mut self.cached_underline_instances,
            &mut self.cached_blink,
            &mut self.cached_font_size,
            &mut self.cached_active_tab,
            &mut self.cached_cursor_rects_start,
            &mut self.cached_cursor_pixel,
            &mut self.cached_url_spans,
            cell_caches,
            effective_fs,
            self.scale_factor,
            time_secs,
        );

        for pane_id in exited {
            self.handle_pane_exit(pane_id);
            self.cached_cell_data.clear();
            self.cached_ui_rects.clear();
            self.cached_cursor_rects_start = 0;
            self.cached_cursor_pixel = None;
        }
    }

    fn poll_overlay_events(&mut self) {
        if let Some(ref rx) = self.overlay_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    crate::overlay::OverlayEvent::Output {
                        title: _,
                        lines,
                        success,
                    } => {
                        self.state.overlay.lines = lines;
                        self.state.overlay.scroll = 0;
                        self.state.overlay.status = if success {
                            crate::overlay::OverlayStatus::Completed
                        } else {
                            crate::overlay::OverlayStatus::Failed
                        };
                    }
                }
            }
        }
    }

    #[allow(dead_code)]
    fn render_profiler_overlay(&mut self) {
        let pane_area = self.layout.pane_area();
        let line_h = self.cell_h;
        let char_w = self.cell_w;

        let bg = [0.0f32, 0.0, 0.0, 0.6];
        let fg = self.state.theme.tab_text;

        let lines = [
            format!("FPS: {:.1}", self.state.profiler.fps),
            format!("Frame: {:.1}ms", self.state.profiler.frame_time_ms),
            format!(
                "PTY: {:.1} KB/s",
                self.state.profiler.pty_bytes_per_sec / 1024.0
            ),
            format!("Cells: {}", self.state.profiler.cell_count),
            format!("Atlas: {:.0}%", self.state.profiler.atlas_used_percent),
        ];

        let max_w = lines.iter().map(|l| l.len()).max().unwrap_or(0) as f32 * char_w;
        let bg_w = max_w + 12.0;
        let bg_h = lines.len() as f32 * line_h + 8.0;
        let bg_x = pane_area.0 + pane_area.2 - bg_w - 8.0;
        let bg_y = pane_area.1 + 4.0;

        self.cached_bg_rects.push(UIRect {
            pos: [bg_x, bg_y],
            size: [bg_w, bg_h],
            color: bg,
        });

        for (i, line) in lines.iter().enumerate() {
            let tx = bg_x + 6.0;
            let ty = bg_y + 4.0 + i as f32 * line_h;
            for (j, c) in line.chars().enumerate() {
                self.cached_cell_data
                    .push((c, tx + j as f32 * char_w, ty, self.cell_h, fg, bg));
            }
        }
    }

    #[allow(dead_code)]
    fn render_keybinds_overlay(&mut self) {
        let line_h = self.cell_h;
        let char_w = self.cell_w;
        let pane_area = self.layout.pane_area();
        let screen_w = pane_area.2;
        let screen_h = pane_area.3;

        let theme = &self.state.theme;
        let title_fg = theme.cursor;
        let header_fg = [1.0f32, 0.647, 0.0, 1.0]; // amber section headers
        let key_fg = [0.0f32, 0.898, 1.0, 1.0]; // bright cyan for keys
        let desc_fg = theme.tab_text;
        let dim_fg = [
            theme.tab_text_inactive[0],
            theme.tab_text_inactive[1],
            theme.tab_text_inactive[2],
            1.0,
        ];
        let overlay_bg = [0.03f32, 0.04, 0.08, 0.94];
        let header_bg = [0.06f32, 0.08, 0.16, 1.0];

        #[derive(Clone)]
        enum Row {
            Section(&'static str),
            Bind {
                key: &'static str,
                desc: &'static str,
            },
            Spacer,
        }

        let rows: &[Row] = &[
            Row::Section("─── GENERAL ───────────────────────────────────────"),
            Row::Bind {
                key: "F1",
                desc: "Show / hide this keybinds panel",
            },
            Row::Bind {
                key: "Ctrl+,",
                desc: "Reload config (hot-reload)",
            },
            Row::Bind {
                key: "F11",
                desc: "Fullscreen",
            },
            Row::Bind {
                key: "F12",
                desc: "Toggle performance profiler",
            },
            Row::Spacer,
            Row::Section("─── TABS ───────────────────────────────────────────"),
            Row::Bind {
                key: "Ctrl+T",
                desc: "New tab",
            },
            Row::Bind {
                key: "Ctrl+W",
                desc: "Close tab",
            },
            Row::Bind {
                key: "Ctrl+Tab",
                desc: "Next tab",
            },
            Row::Bind {
                key: "Ctrl+Shift+Tab",
                desc: "Previous tab",
            },
            Row::Bind {
                key: "Ctrl+1..9",
                desc: "Jump to tab N",
            },
            Row::Spacer,
            Row::Section("─── PANES ──────────────────────────────────────────"),
            Row::Bind {
                key: "Ctrl+Shift+D",
                desc: "Split pane vertically",
            },
            Row::Bind {
                key: "Ctrl+Shift+H",
                desc: "Split pane horizontally",
            },
            Row::Bind {
                key: "Ctrl+Enter",
                desc: "Auto split",
            },
            Row::Bind {
                key: "Ctrl+Shift+W",
                desc: "Close pane",
            },
            Row::Bind {
                key: "Ctrl+Shift+↑↓←→",
                desc: "Navigate between panes",
            },
            Row::Bind {
                key: "Ctrl+Shift+Alt+↑↓←→",
                desc: "Resize pane",
            },
            Row::Bind {
                key: "Ctrl+Shift+Z",
                desc: "Zoom / unzoom active pane",
            },
            Row::Spacer,
            Row::Section("─── SEARCH & COPY ──────────────────────────────────"),
            Row::Bind {
                key: "Ctrl+Shift+F",
                desc: "In-buffer search",
            },
            Row::Bind {
                key: "Ctrl+R",
                desc: "History search",
            },
            Row::Bind {
                key: "Ctrl+Shift+Space",
                desc: "Enter copy mode (vim-style)",
            },
            Row::Bind {
                key: "Ctrl+Shift+C",
                desc: "Copy selection",
            },
            Row::Bind {
                key: "Ctrl+Shift+V",
                desc: "Paste",
            },
            Row::Spacer,
            Row::Section("─── NAVIGATION ─────────────────────────────────────"),
            Row::Bind {
                key: "Ctrl+↑",
                desc: "Jump to previous prompt mark",
            },
            Row::Bind {
                key: "Ctrl+↓",
                desc: "Jump to next prompt mark",
            },
            Row::Spacer,
            Row::Section("─── FONT ───────────────────────────────────────────"),
            Row::Bind {
                key: "Ctrl+=",
                desc: "Increase font size",
            },
            Row::Bind {
                key: "Ctrl+-",
                desc: "Decrease font size",
            },
            Row::Bind {
                key: "Ctrl+0",
                desc: "Reset font size",
            },
            Row::Spacer,
            Row::Section("─── WORKSPACE ──────────────────────────────────────"),
            Row::Bind {
                key: "Ctrl+Shift+N",
                desc: "New workspace",
            },
            Row::Bind {
                key: "Ctrl+Alt+Tab",
                desc: "Switch workspace",
            },
            Row::Bind {
                key: "Ctrl+Shift+Alt+R",
                desc: "Rename workspace",
            },
            Row::Bind {
                key: "Ctrl+Shift+Alt+D",
                desc: "Delete workspace",
            },
            Row::Spacer,
            Row::Section("─── MISC ───────────────────────────────────────────"),
            Row::Bind {
                key: "Ctrl+L",
                desc: "Clear screen",
            },
            Row::Bind {
                key: "Ctrl+Shift+S",
                desc: "Toggle status bar",
            },
            Row::Bind {
                key: "Ctrl+Shift+P",
                desc: "Command palette",
            },
            Row::Bind {
                key: "Ctrl+Shift+B",
                desc: "Toggle broadcast to all panes",
            },
            Row::Bind {
                key: "Ctrl+Shift+R",
                desc: "Start / stop asciinema recording",
            },
            Row::Bind {
                key: "Ctrl+Shift+E",
                desc: "Toggle GPU effects (scanlines, bloom…)",
            },
            Row::Spacer,
            Row::Section("─── COPY MODE (when active) ────────────────────────"),
            Row::Bind {
                key: "h j k l",
                desc: "Move cursor",
            },
            Row::Bind {
                key: "v / V",
                desc: "Start char / line selection",
            },
            Row::Bind {
                key: "y",
                desc: "Yank (copy) selection",
            },
            Row::Bind {
                key: "/ or ?",
                desc: "Search forward / backward",
            },
            Row::Bind {
                key: "Esc / q",
                desc: "Exit copy mode",
            },
        ];

        let key_col_w = 26usize; // chars for key column
        let desc_col_w = 52usize; // chars for desc column
        let total_w_chars = key_col_w + 2 + desc_col_w;
        let box_w = total_w_chars as f32 * char_w + 24.0;
        let visible_rows = ((screen_h - 80.0) / line_h).floor() as usize;
        let box_h = visible_rows as f32 * line_h + 32.0;
        let box_x = pane_area.0 + (screen_w - box_w) * 0.5;
        let box_y = pane_area.1 + (screen_h - box_h) * 0.5;

        // Dim full screen behind overlay
        self.cached_bg_rects.push(UIRect {
            pos: [pane_area.0, pane_area.1],
            size: [screen_w, screen_h],
            color: [0.0, 0.0, 0.0, 0.72],
        });

        // Main box
        self.cached_bg_rects.push(UIRect {
            pos: [box_x, box_y],
            size: [box_w, box_h],
            color: overlay_bg,
        });

        // Title bar row
        let title = " SYNAPSE_ KEYBINDS  ·  ↑↓ scroll  ·  Esc or F1 close ";
        self.cached_bg_rects.push(UIRect {
            pos: [box_x, box_y],
            size: [box_w, line_h + 8.0],
            color: header_bg,
        });
        let tx = box_x + 12.0;
        let ty = box_y + 4.0;
        for (j, c) in title.chars().enumerate() {
            self.cached_cell_data.push((
                c,
                tx + j as f32 * char_w,
                ty,
                self.cell_h,
                title_fg,
                header_bg,
            ));
        }

        // Platform note
        let note = " macOS/Linux: Ctrl = ^ | Windows: not yet supported ";
        let ny = box_y + box_h - line_h - 4.0;
        self.cached_bg_rects.push(UIRect {
            pos: [box_x, ny],
            size: [box_w, line_h + 4.0],
            color: header_bg,
        });
        for (j, c) in note.chars().enumerate() {
            self.cached_cell_data.push((
                c,
                box_x + 12.0 + j as f32 * char_w,
                ny + 2.0,
                self.cell_h,
                dim_fg,
                header_bg,
            ));
        }

        let content_y_start = box_y + line_h + 12.0;
        let content_y_end = ny - 4.0;
        let visible_content_h = content_y_end - content_y_start;
        let max_visible = (visible_content_h / line_h).floor() as usize;

        // Clamp scroll
        let total_rows = rows.len();
        let max_scroll = total_rows.saturating_sub(max_visible);
        let scroll = self.state.keybinds_scroll.min(max_scroll);
        self.state.keybinds_scroll = scroll;

        let cx = box_x + 12.0;
        for (i, row) in rows.iter().enumerate().skip(scroll).take(max_visible) {
            let ry = content_y_start + (i - scroll) as f32 * line_h;
            match row {
                Row::Section(label) => {
                    self.cached_bg_rects.push(UIRect {
                        pos: [box_x + 4.0, ry],
                        size: [box_w - 8.0, line_h],
                        color: [0.05, 0.07, 0.14, 1.0],
                    });
                    for (j, c) in label.chars().enumerate() {
                        self.cached_cell_data.push((
                            c,
                            cx + j as f32 * char_w,
                            ry,
                            self.cell_h,
                            header_fg,
                            [0.05, 0.07, 0.14, 1.0],
                        ));
                    }
                }
                Row::Bind { key, desc } => {
                    let padded_key = format!("{:<width$}", key, width = key_col_w);
                    for (j, c) in padded_key.chars().enumerate() {
                        self.cached_cell_data.push((
                            c,
                            cx + j as f32 * char_w,
                            ry,
                            self.cell_h,
                            key_fg,
                            overlay_bg,
                        ));
                    }
                    let sep_x = cx + key_col_w as f32 * char_w;
                    for (j, c) in "  ".chars().enumerate() {
                        self.cached_cell_data.push((
                            c,
                            sep_x + j as f32 * char_w,
                            ry,
                            self.cell_h,
                            dim_fg,
                            overlay_bg,
                        ));
                    }
                    let dx = sep_x + 2.0 * char_w;
                    let truncated: String = desc.chars().take(desc_col_w).collect();
                    for (j, c) in truncated.chars().enumerate() {
                        self.cached_cell_data.push((
                            c,
                            dx + j as f32 * char_w,
                            ry,
                            self.cell_h,
                            desc_fg,
                            overlay_bg,
                        ));
                    }
                }
                Row::Spacer => {}
            }
        }
    }

    fn handle_pane_exit(&mut self, pane_id: synapse_ui::PaneId) {
        let ws = self.workspaces.active_ws_mut();
        let tab_bar = &mut ws.tab_bar;
        let panes = &mut ws.panes;
        let cell_caches = &mut ws.pane_cell_caches;

        let tab_idx = tab_bar
            .tabs
            .iter()
            .position(|t| t.pane_tree.all_panes().contains(&pane_id));

        if let Some(idx) = tab_idx {
            let pane_count = tab_bar.tabs[idx].pane_tree.all_panes().len();

            if pane_count == 1 {
                if tab_bar.tabs.len() == 1 {
                    let pane_area = self.layout.pane_area();
                    let new_cols =
                        ((pane_area.2 - self.margin * 2.0) / self.cell_w).max(1.0) as usize;
                    let new_rows =
                        ((pane_area.3 - self.margin * 2.0) / self.cell_h).max(1.0) as usize;
                    let new_pane_id = tab_bar.next_pane_id();
                    tab_bar.tabs[0].pane_tree = synapse_ui::PaneTree::leaf(new_pane_id);
                    tab_bar.tabs[0].active_pane = new_pane_id;
                    match create_pane(
                        new_pane_id,
                        new_cols,
                        new_rows,
                        self.state.config.scrollback_lines,
                    ) {
                        Ok(pane) => panes.push(pane),
                        Err(e) => tracing::warn!("Failed to spawn replacement PTY: {}", e),
                    }
                } else {
                    tab_bar.close_tab(idx);
                }
            } else {
                tab_bar.tabs[idx].pane_tree.close(pane_id);
                if tab_bar.tabs[idx].active_pane == pane_id {
                    let first = tab_bar.tabs[idx].pane_tree.all_panes()[0];
                    tab_bar.tabs[idx].active_pane = first;
                }
            }
        }

        panes.retain(|p| p.id != pane_id);
        cell_caches.remove(&pane_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_returns_empty() {
        assert!(build_underline_spans(&[], 0.0, 0.0, 8.0, 16.0).is_empty());
    }

    #[test]
    fn single_cell_one_instance() {
        let color = [1.0f32, 0.0, 0.0, 1.0];
        let result = build_underline_spans(&[(0, 0, 0, color)], 0.0, 0.0, 8.0, 16.0);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].style, 0);
        assert!((result[0].size[0] - 8.0).abs() < 0.001);
    }

    #[test]
    fn consecutive_same_style_merged() {
        let color = [1.0f32, 0.0, 0.0, 1.0];
        let cells = [(0, 0, 2, color), (1, 0, 2, color), (2, 0, 2, color)];
        let result = build_underline_spans(&cells, 0.0, 0.0, 8.0, 16.0);
        assert_eq!(result.len(), 1);
        assert!((result[0].size[0] - 24.0).abs() < 0.001);
    }

    #[test]
    fn different_style_breaks_span() {
        let color = [1.0f32, 0.0, 0.0, 1.0];
        let cells = [(0, 0, 0, color), (1, 0, 2, color)];
        let result = build_underline_spans(&cells, 0.0, 0.0, 8.0, 16.0);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].style, 0);
        assert_eq!(result[1].style, 2);
    }

    #[test]
    fn different_row_breaks_span() {
        let color = [1.0f32, 0.0, 0.0, 1.0];
        let cells = [(0, 0, 0, color), (1, 1, 0, color)];
        let result = build_underline_spans(&cells, 0.0, 0.0, 8.0, 16.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn undercurl_y_offset_and_height() {
        let cells = [(0, 0, 2, [1.0f32, 0.0, 0.0, 1.0])];
        let result = build_underline_spans(&cells, 0.0, 0.0, 8.0, 16.0);
        assert_eq!(result.len(), 1);
        // y = content_y + 0 * 16.0 + (16.0 - 4.0) = 12.0
        assert!((result[0].pos[1] - 12.0).abs() < 0.001);
        assert!((result[0].size[1] - 4.0).abs() < 0.001);
    }

    #[test]
    fn non_consecutive_column_breaks_span() {
        let color = [1.0f32, 0.0, 0.0, 1.0];
        let cells = [(0, 0, 0, color), (2, 0, 0, color)]; // col gap: 0 then 2
        let result = build_underline_spans(&cells, 0.0, 0.0, 8.0, 16.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_push_cursor_rect_skips_in_copy_mode() {
        use crate::state::AppState;
        use synapse_config::{Config, Keybinds};
        let mut state = AppState::new(Config::default(), Keybinds::default(), 14.0);
        state.in_copy_mode = true;
        let mut rects: Vec<UIRect> = Vec::new();
        push_cursor_rect(&mut rects, Some((0.0, 0.0)), 1.0, 8.0, 16.0, &mut state);
        assert!(
            rects.is_empty(),
            "copy mode must suppress normal cursor rect"
        );
    }

    #[test]
    fn test_push_cursor_rect_emits_rect_normally() {
        use crate::state::AppState;
        use synapse_config::{Config, Keybinds};
        let mut state = AppState::new(Config::default(), Keybinds::default(), 14.0);
        state.in_copy_mode = false;
        let mut rects: Vec<UIRect> = Vec::new();
        push_cursor_rect(&mut rects, Some((10.0, 20.0)), 1.0, 8.0, 16.0, &mut state);
        assert!(!rects.is_empty(), "normal mode must emit cursor rect");
        let last = rects.last().unwrap();
        assert_eq!(last.pos, [10.0, 20.0]);
        assert_eq!(last.size, [8.0, 16.0]);
    }

    #[test]
    fn test_push_cursor_rect_neon_underbar() {
        use crate::state::AppState;
        use synapse_config::{Config, CursorStyle, Keybinds};
        let config = Config {
            cursor_style: CursorStyle::NeonUnderbar,
            ..Config::default()
        };
        let mut state = AppState::new(config, Keybinds::default(), 14.0);
        state.in_copy_mode = false;
        state.term_cursor_style = None;
        let mut rects: Vec<UIRect> = Vec::new();
        push_cursor_rect(&mut rects, Some((10.0, 20.0)), 1.0, 8.0, 16.0, &mut state);
        assert_eq!(
            rects.len(),
            3,
            "NeonUnderbar emits 3 rects (bar + 2 glow layers)"
        );
        assert_eq!(rects[0].size, [8.0, 2.0]);
        assert_eq!(rects[1].size, [8.0, 5.0]);
        assert_eq!(rects[2].size, [8.0, 8.0]);
        assert!(rects[0].color[3] > rects[1].color[3]);
        assert!(rects[1].color[3] > rects[2].color[3]);
    }
}
