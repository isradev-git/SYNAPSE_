use std::cell::RefCell;
use std::rc::Rc;

use crate::grid::{CellFlags, CharCell, Color, Grid};

pub struct VteProcessor {
    grid: Rc<RefCell<Grid>>,
    fg: Color,
    bg: Color,
    flags: CellFlags,
    title: Rc<RefCell<String>>,
    cwd: Rc<RefCell<String>>,
}

impl VteProcessor {
    pub fn new(grid: Rc<RefCell<Grid>>) -> Self {
        Self {
            grid,
            fg: Color::Default,
            bg: Color::Default,
            flags: CellFlags::empty(),
            title: Rc::new(RefCell::new(String::new())),
            cwd: Rc::new(RefCell::new(String::new())),
        }
    }

    pub fn new_with_title(grid: Rc<RefCell<Grid>>, title: Rc<RefCell<String>>, cwd: Rc<RefCell<String>>) -> Self {
        Self {
            grid,
            fg: Color::Default,
            bg: Color::Default,
            flags: CellFlags::empty(),
            title,
            cwd,
        }
    }

    pub fn title_rc(&self) -> Rc<RefCell<String>> {
        self.title.clone()
    }

    pub fn cwd_rc(&self) -> Rc<RefCell<String>> {
        self.cwd.clone()
    }

    pub fn process(&mut self, bytes: &[u8]) {
        let mut parser = vte::Parser::new();
        for &byte in bytes {
            parser.advance(self, byte);
        }
    }
}

impl vte::Perform for VteProcessor {
    fn print(&mut self, c: char) {
        let mut grid = self.grid.borrow_mut();
        let col = grid.cursor_col();
        let row = grid.cursor_row();

        let mut cell = CharCell {
            c,
            fg: self.fg,
            bg: self.bg,
            flags: self.flags,
            dirty: true,
        };

        if self.flags.contains(CellFlags::INVERSE) {
            std::mem::swap(&mut cell.fg, &mut cell.bg);
        }

        grid.set(col, row, cell);
        grid.advance_cursor();
    }

    fn execute(&mut self, byte: u8) {
        let mut grid = self.grid.borrow_mut();
        match byte {
            b'\n' => grid.new_line(),
            b'\r' => grid.carriage_return(),
            b'\x08' => {
                let col = grid.cursor_col();
                let row = grid.cursor_row();
                if col > 0 {
                    grid.set_cursor(col - 1, row);
                }
            }
            b'\t' => {
                let col = grid.cursor_col();
                let row = grid.cursor_row();
                let next_tab = ((col / 8) + 1) * 8;
                let max_col = grid.cols().saturating_sub(1);
                grid.set_cursor(next_tab.min(max_col), row);
            }
            b'\x0c' => {
                let last_row = grid.rows() - 1;
                grid.clear_region(0, last_row);
                grid.set_cursor(0, 0);
            }
            _ => {}
        }
    }

    fn hook(
        &mut self,
        _params: &vte::Params,
        _intermediates: &[u8],
        _ignore: bool,
        _action: char,
    ) {
    }

    fn put(&mut self, _byte: u8) {}

    fn unhook(&mut self) {}

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.len() < 2 {
            return;
        }
        let code_str = String::from_utf8_lossy(params[0]);
        let param_str = String::from_utf8_lossy(params[1]);
        let trimmed = param_str.trim();

        match code_str.as_ref() {
            "0" | "2" => {
                if !trimmed.is_empty() {
                    *self.title.borrow_mut() = trimmed.to_string();
                }
            }
            "7" => {
                if let Some(path) = param_str.strip_prefix("file://") {
                    if let Some(slash_pos) = path.find('/') {
                        let cwd = &path[slash_pos..];
                        if !cwd.is_empty() {
                            *self.cwd.borrow_mut() = cwd.to_string();
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        let n = get_param(params, 0).max(1) as usize;

        match action {
            'A' => {
                let mut grid = self.grid.borrow_mut();
                let col = grid.cursor_col();
                let new_row = grid.cursor_row().saturating_sub(n);
                grid.set_cursor(col, new_row);
            }
            'B' => {
                let mut grid = self.grid.borrow_mut();
                let col = grid.cursor_col();
                let max_row = grid.rows().saturating_sub(1);
                let new_row = (grid.cursor_row() + n).min(max_row);
                grid.set_cursor(col, new_row);
            }
            'C' => {
                let mut grid = self.grid.borrow_mut();
                let max_col = grid.cols().saturating_sub(1);
                let row = grid.cursor_row();
                let new_col = (grid.cursor_col() + n).min(max_col);
                grid.set_cursor(new_col, row);
            }
            'D' => {
                let mut grid = self.grid.borrow_mut();
                let row = grid.cursor_row();
                let new_col = grid.cursor_col().saturating_sub(n);
                grid.set_cursor(new_col, row);
            }
            'H' | 'f' => {
                let p0 = get_param(params, 0).max(1) as usize;
                let p1 = get_param(params, 1).max(1) as usize;
                let mut grid = self.grid.borrow_mut();
                let max_row = grid.rows().saturating_sub(1);
                let max_col = grid.cols().saturating_sub(1);
                let row = (p0.saturating_sub(1)).min(max_row);
                let col = (p1.saturating_sub(1)).min(max_col);
                grid.set_cursor(col, row);
            }
            'J' => {
                let mode = get_param(params, 0);
                let mut grid = self.grid.borrow_mut();
                match mode {
                    0 => {
                        let row = grid.cursor_row();
                        let col = grid.cursor_col();
                        let max_row = grid.rows() - 1;
                        grid.clear_line(col);
                        if row + 1 < grid.rows() {
                            grid.clear_region(row + 1, max_row);
                        }
                    }
                    1 => {
                        let row = grid.cursor_row();
                        let col = grid.cursor_col();
                        if row > 0 {
                            grid.clear_region(0, row.saturating_sub(1));
                        }
                        grid.clear_line_from_start(col);
                    }
                    2 | 3 => {
                        let last_row = grid.rows() - 1;
                        grid.clear_region(0, last_row);
                    }
                    _ => {}
                }
            }
            'K' => {
                let mode = get_param(params, 0);
                let mut grid = self.grid.borrow_mut();
                let col = grid.cursor_col();
                match mode {
                    0 => grid.clear_line(col),
                    1 => grid.clear_line_from_start(col),
                    2 => grid.clear_line(0),
                    _ => {}
                }
            }
            'm' => {
                self.handle_sgr(params);
            }
            'h' | 'l' => {}
            's' => {
                let mut grid = self.grid.borrow_mut();
                grid.save_cursor();
            }
            'u' => {
                let mut grid = self.grid.borrow_mut();
                grid.restore_cursor();
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        let mut grid = self.grid.borrow_mut();
        match byte {
            b'7' => grid.save_cursor(),
            b'8' => grid.restore_cursor(),
            b'c' => {
                drop(grid);
                self.reset_state();
                let mut g = self.grid.borrow_mut();
                let last_row = g.rows() - 1;
                g.clear_region(0, last_row);
                g.set_cursor(0, 0);
            }
            _ => {}
        }
    }
}

fn get_param(params: &vte::Params, idx: usize) -> i64 {
    let mut iter = params.iter();
    for _ in 0..idx {
        iter.next();
    }
    iter.next()
        .and_then(|g| g.first().copied())
        .map(|v| v as i64)
        .unwrap_or(0)
}

impl VteProcessor {
    pub fn fg(&self) -> Color {
        self.fg
    }

    pub fn bg(&self) -> Color {
        self.bg
    }

    pub fn flags(&self) -> CellFlags {
        self.flags
    }

    fn handle_sgr(&mut self, params: &vte::Params) {
        let flat: Vec<i64> = params.iter().flatten().copied().map(|v| v as i64).collect();

        if flat.is_empty() {
            self.reset_state();
            return;
        }

        let mut i = 0;
        while i < flat.len() {
            let p = flat[i];
            match p {
                0 => self.reset_state(),
                1 => {
                    self.flags.insert(CellFlags::BOLD);
                }
                3 => {
                    self.flags.insert(CellFlags::ITALIC);
                }
                4 => {
                    self.flags.insert(CellFlags::UNDERLINE);
                }
                5 | 6 => {
                    self.flags.insert(CellFlags::BLINK);
                }
                7 => {
                    self.flags.insert(CellFlags::INVERSE);
                }
                8 => {
                    self.flags.insert(CellFlags::INVISIBLE);
                }
                22 => {
                    self.flags.remove(CellFlags::BOLD);
                }
                23 => {
                    self.flags.remove(CellFlags::ITALIC);
                }
                24 => {
                    self.flags.remove(CellFlags::UNDERLINE);
                }
                25 => {
                    self.flags.remove(CellFlags::BLINK);
                }
                27 => {
                    self.flags.remove(CellFlags::INVERSE);
                }
                28 => {
                    self.flags.remove(CellFlags::INVISIBLE);
                }
                30..=37 => {
                    self.fg = ansi_3bit_color((p - 30) as u8);
                }
                38 => {
                    if i + 2 < flat.len() {
                        match flat[i + 1] {
                            2 => {
                                if i + 4 < flat.len() {
                                    self.fg = Color::Rgb(
                                        flat[i + 2] as u8,
                                        flat[i + 3] as u8,
                                        flat[i + 4] as u8,
                                    );
                                    i += 4;
                                }
                            }
                            5 => {
                                if i + 2 < flat.len() {
                                    self.fg = Color::Indexed(flat[i + 2] as u8);
                                    i += 2;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                39 => {
                    self.fg = Color::Default;
                }
                40..=47 => {
                    self.bg = ansi_3bit_color((p - 40) as u8);
                }
                48 => {
                    if i + 2 < flat.len() {
                        match flat[i + 1] {
                            2 => {
                                if i + 4 < flat.len() {
                                    self.bg = Color::Rgb(
                                        flat[i + 2] as u8,
                                        flat[i + 3] as u8,
                                        flat[i + 4] as u8,
                                    );
                                    i += 4;
                                }
                            }
                            5 => {
                                if i + 2 < flat.len() {
                                    self.bg = Color::Indexed(flat[i + 2] as u8);
                                    i += 2;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                49 => {
                    self.bg = Color::Default;
                }
                90..=97 => {
                    self.fg = ansi_3bit_bright((p - 90) as u8);
                }
                100..=107 => {
                    self.bg = ansi_3bit_bright((p - 100) as u8);
                }
                _ => {
                    i += 1;
                    continue;
                }
            }
            i += 1;
        }
    }

    pub fn reset_state(&mut self) {
        self.fg = Color::Default;
        self.bg = Color::Default;
        self.flags = CellFlags::empty();
    }
}

fn ansi_3bit_color(idx: u8) -> Color {
    match idx {
        0 => Color::Indexed(0),
        1 => Color::Indexed(1),
        2 => Color::Indexed(2),
        3 => Color::Indexed(3),
        4 => Color::Indexed(4),
        5 => Color::Indexed(5),
        6 => Color::Indexed(6),
        7 => Color::Indexed(7),
        _ => Color::Default,
    }
}

fn ansi_3bit_bright(idx: u8) -> Color {
    match idx {
        0 => Color::Indexed(8),
        1 => Color::Indexed(9),
        2 => Color::Indexed(10),
        3 => Color::Indexed(11),
        4 => Color::Indexed(12),
        5 => Color::Indexed(13),
        6 => Color::Indexed(14),
        7 => Color::Indexed(15),
        _ => Color::Default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_grid(cols: usize, rows: usize) -> Rc<RefCell<Grid>> {
        Rc::new(RefCell::new(Grid::new(cols, rows)))
    }

    #[test]
    fn test_print_char() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"A");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).c, 'A');
        assert_eq!(g.cursor_col(), 1);
    }

    #[test]
    fn test_newline() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"A\nB");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).c, 'A');
        assert_eq!(g.get(0, 1).c, 'B');
        assert_eq!(g.cursor_row(), 1);
    }

    #[test]
    fn test_cursor_movement() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[10;5H");
        let g = grid.borrow();
        assert_eq!(g.cursor_col(), 4);
        assert_eq!(g.cursor_row(), 9);
    }

    #[test]
    fn test_sgr_bold_green() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[1;32mX");
        let g = grid.borrow();
        let cell = g.get(0, 0);
        assert_eq!(cell.c, 'X');
        assert!(cell.flags.contains(CellFlags::BOLD));
        assert_eq!(cell.fg, Color::Indexed(2));
    }

    #[test]
    fn test_clear_screen() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"HELLO");
        // \x1b[2J clears display but does NOT move cursor (VT100 spec)
        // Cursor stays where it was (after "HELLO" = col 5, row 0)
        proc.process(b"\x1b[2J");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).c, ' ');
        assert_eq!(g.get(1, 0).c, ' ');
        assert_eq!(g.get(4, 0).c, ' ');
    }

    #[test]
    fn test_esc_save_restore_cursor() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[5;10H");
        proc.process(b"\x1b7");
        proc.process(b"\x1b[1;1H");
        proc.process(b"\x1b8");
        let g = grid.borrow();
        assert_eq!(g.cursor_col(), 9);
        assert_eq!(g.cursor_row(), 4);
    }

    #[test]
    fn test_get_param() {
        // Build params manually isn't possible without crate access,
        // so we test through the CSI dispatch.
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[1m");
        assert!(proc.flags.contains(CellFlags::BOLD));
    }

    #[test]
    fn test_sgr_true_color() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[38;2;255;100;0mX");
        let g = grid.borrow();
        let cell = g.get(0, 0);
        assert_eq!(cell.c, 'X');
        assert_eq!(cell.fg, Color::Rgb(255, 100, 0));
    }

    // ── C0 control character tests ──────────────────────────────────────

    #[test]
    fn test_c0_cr() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"ABCDE\rX");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).c, 'X');
        assert_eq!(g.cursor_row(), 0);
    }

    #[test]
    fn test_c0_bs() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"AB\x08X");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).c, 'A');
        assert_eq!(g.get(1, 0).c, 'X');
    }

    #[test]
    fn test_c0_tab() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"A\t");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).c, 'A');
        assert_eq!(g.cursor_col(), 8);
    }

    #[test]
    fn test_c0_tab_multiple() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"A\t\t");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).c, 'A');
        assert_eq!(g.cursor_col(), 16);
    }

    #[test]
    fn test_c0_ff_form_feed() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"HELLO\x0cX");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).c, 'X');
        assert_eq!(g.cursor_row(), 0);
        assert_eq!(g.cursor_col(), 1);
    }

    // ── CSI cursor movement tests ───────────────────────────────────────

    #[test]
    fn test_cuu_cursor_up() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[10;1H");
        proc.process(b"\x1b[3A");
        let g = grid.borrow();
        assert_eq!(g.cursor_row(), 6);
    }

    #[test]
    fn test_cud_cursor_down() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[3;1H");
        proc.process(b"\x1b[5B");
        let g = grid.borrow();
        assert_eq!(g.cursor_row(), 7);
    }

    #[test]
    fn test_cuf_cursor_forward() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[5C");
        let g = grid.borrow();
        assert_eq!(g.cursor_col(), 5);
    }

    #[test]
    fn test_cub_cursor_back() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"ABCDE\x1b[2D");
        let g = grid.borrow();
        assert_eq!(g.cursor_col(), 3);
    }

    #[test]
    fn test_cup_no_args() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"HELLO\x1b[HX");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).c, 'X');
        assert_eq!(g.cursor_row(), 0);
    }

    // ── CSI erase tests ─────────────────────────────────────────────────

    #[test]
    fn test_ed_0_cursor_to_end() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        // Fill first 3 rows explicitly
        for _ in 0..(80 * 3) {
            proc.process(b"X");
        }
        // Move cursor to row 1, col 5
        proc.process(b"\x1b[2;6H");
        proc.process(b"\x1b[0J");
        let g = grid.borrow();
        // Row 0 should be fully untouched (cursor is at row 1)
        assert_eq!(g.get(0, 0).c, 'X');
        // Row 1: col 0-4 should be 'X', col 5 onwards should be cleared
        assert_eq!(g.get(0, 1).c, 'X');
        assert_eq!(g.get(4, 1).c, 'X');
        assert_eq!(g.get(5, 1).c, ' ');
        // Row 2+ should be cleared
        assert_eq!(g.get(0, 2).c, ' ');
    }

    #[test]
    fn test_ed_1_start_to_cursor() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        // Fill first 3 rows so total of 240 X positions
        for _ in 0..(80 * 3) {
            proc.process(b"X");
        }
        proc.process(b"\x1b[2;6H");
        proc.process(b"\x1b[1J");
        let g = grid.borrow();
        // Row 0 cleared by clear_region(0, 0)
        assert_eq!(g.get(0, 0).c, ' ');
        // Row 1: cols 0-5 cleared, col 6 should still be X
        assert_eq!(g.get(0, 1).c, ' ');
        assert_eq!(g.get(5, 1).c, ' ');
        assert_eq!(g.get(6, 1).c, 'X');
        // Row 2: untouched, should be all X
        assert_eq!(g.get(0, 2).c, 'X');
    }

    #[test]
    fn test_ed_2_entire_display() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        for _ in 0..(80 * 5) {
            proc.process(b"X");
        }
        proc.process(b"\x1b[2J");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).c, ' ');
        assert_eq!(g.get(10, 10).c, ' ');
    }

    #[test]
    fn test_el_0_cursor_to_end() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"ABCDEFGH");
        // Move cursor back 4 positions to col 4 (after H which is col 7)
        proc.process(b"\x1b[4D");
        proc.process(b"\x1b[0K");
        let g = grid.borrow();
        // cols 0-3 should remain, col 4+ cleared
        assert_eq!(g.get(0, 0).c, 'A');
        assert_eq!(g.get(3, 0).c, 'D');
        assert_eq!(g.get(4, 0).c, ' ');
    }

    #[test]
    fn test_el_1_start_to_cursor() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"ABCDEFGH");
        // Move cursor back 3 positions to col 5 (after H which is col 7)
        proc.process(b"\x1b[3D");
        proc.process(b"\x1b[1K");
        let g = grid.borrow();
        // cols 0-5 should be cleared, cols 6+ remain
        assert_eq!(g.get(0, 0).c, ' ');
        assert_eq!(g.get(5, 0).c, ' ');
        assert_eq!(g.get(6, 0).c, 'G');
        assert_eq!(g.get(7, 0).c, 'H');
    }

    #[test]
    fn test_el_2_entire_line() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"ABCDEFGH");
        proc.process(b"\x1b[2K");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).c, ' ');
    }

    // ── SGR attribute tests ─────────────────────────────────────────────

    #[test]
    fn test_sgr_reset() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[1;34;45m");
        assert!(proc.flags.contains(CellFlags::BOLD));
        proc.process(b"\x1b[0mX");
        let g = grid.borrow();
        let cell = g.get(0, 0);
        assert!(!cell.flags.contains(CellFlags::BOLD));
        assert_eq!(cell.fg, Color::Default);
        assert_eq!(cell.bg, Color::Default);
    }

    #[test]
    fn test_sgr_bright_colors() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[91mX");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).fg, Color::Indexed(9));
    }

    #[test]
    fn test_sgr_bright_bg() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[102mX");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).bg, Color::Indexed(10));
    }

    #[test]
    fn test_sgr_underline() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[4mX");
        let g = grid.borrow();
        assert!(g.get(0, 0).flags.contains(CellFlags::UNDERLINE));
    }

    #[test]
    fn test_sgr_italic() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[3mX");
        let g = grid.borrow();
        assert!(g.get(0, 0).flags.contains(CellFlags::ITALIC));
    }

    #[test]
    fn test_sgr_blink() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[5mX");
        let g = grid.borrow();
        assert!(g.get(0, 0).flags.contains(CellFlags::BLINK));
    }

    #[test]
    fn test_sgr_inverse() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[7mX");
        let g = grid.borrow();
        assert!(g.get(0, 0).flags.contains(CellFlags::INVERSE));
    }

    #[test]
    fn test_sgr_256_color() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[38;5;196mX");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).fg, Color::Indexed(196));
    }

    #[test]
    fn test_sgr_256_bg() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[48;5;22mX");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).bg, Color::Indexed(22));
    }

    #[test]
    fn test_sgr_attribute_off() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[4;3;1m");
        assert!(proc.flags.contains(CellFlags::UNDERLINE | CellFlags::ITALIC | CellFlags::BOLD));
        proc.process(b"\x1b[24;23;22m");
        assert!(!proc.flags.contains(CellFlags::UNDERLINE | CellFlags::ITALIC | CellFlags::BOLD));
    }

    // ── RIS (reset to initial state) ────────────────────────────────────

    #[test]
    fn test_ris_reset() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"HELLO\x1b[2;2H\x1b[1;32m");
        proc.process(b"\x1bcX");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).c, 'X');
        let cell = g.get(0, 0);
        assert_eq!(cell.fg, Color::Default);
        assert_eq!(cell.bg, Color::Default);
        assert!(cell.flags.is_empty());
    }

    // ── CSI save/restore cursor (s/u) ───────────────────────────────────

    #[test]
    fn test_csi_save_restore() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[5;10H");
        proc.process(b"\x1b[s");
        proc.process(b"\x1b[1;1H");
        proc.process(b"\x1b[u");
        let g = grid.borrow();
        assert_eq!(g.cursor_col(), 9);
        assert_eq!(g.cursor_row(), 4);
    }

    // ── Auto-wrap tests ──────────────────────────────────────────────────

    #[test]
    fn test_auto_wrap() {
        let grid = make_grid(5, 3);
        let mut proc = VteProcessor::new(grid.clone());
        // Write 5 chars to fill row 0 exactly
        proc.process(b"ABCDE");
        {
            let g = grid.borrow();
            // Cursor wrapped to row 1 col 0 after 5th char
            assert_eq!(g.get(0, 0).c, 'A');
            assert_eq!(g.get(4, 0).c, 'E');
        }
        // Next char goes to row 1 col 0
        proc.process(b"F");
        let g = grid.borrow();
        assert_eq!(g.get(0, 1).c, 'F');
    }

    #[test]
    fn test_line_feed_scroll() {
        let grid = make_grid(80, 3);
        let mut proc = VteProcessor::new(grid.clone());
        // Write 'A' then newline, 'B' then newline, 'C' then newline, 'D'
        proc.process(b"A\nB\nC\nD");
        let g = grid.borrow();
        // After scrolling, row 0 = 'B', row 1 = 'C', row 2 = 'D', row 0 shifted out
        assert_eq!(g.get(0, 0).c, 'B');
        assert_eq!(g.get(0, 1).c, 'C');
        assert_eq!(g.get(0, 2).c, 'D');
    }

    // ── OSC title tests ──────────────────────────────────────────────────

    #[test]
    fn test_osc_set_title() {
        let grid = make_grid(80, 24);
        let title = Rc::new(RefCell::new(String::new()));
        let cwd = Rc::new(RefCell::new(String::new()));
        let mut proc = VteProcessor::new_with_title(grid, title.clone(), cwd);
        proc.process(b"\x1b]0;MyTitle\x07");
        assert_eq!(&*title.borrow(), "MyTitle");
    }

    #[test]
    fn test_osc_set_title_osc2() {
        let grid = make_grid(80, 24);
        let title = Rc::new(RefCell::new(String::new()));
        let cwd = Rc::new(RefCell::new(String::new()));
        let mut proc = VteProcessor::new_with_title(grid, title.clone(), cwd);
        proc.process(b"\x1b]2;Another Title\x07");
        assert_eq!(&*title.borrow(), "Another Title");
    }

    #[test]
    fn test_osc_set_cwd() {
        let grid = make_grid(80, 24);
        let title = Rc::new(RefCell::new(String::new()));
        let cwd = Rc::new(RefCell::new(String::new()));
        let mut proc = VteProcessor::new_with_title(grid, title, cwd.clone());
        proc.process(b"\x1b]7;file://hostname/home/user\x07");
        assert_eq!(&*cwd.borrow(), "/home/user");
    }

    // ── Edge cases ──────────────────────────────────────────────────────

    #[test]
    fn test_empty_csi() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[mX");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).c, 'X');
        assert_eq!(g.get(0, 0).fg, Color::Default);
    }

    #[test]
    fn test_multi_byte_utf8() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process("ñ".as_bytes());
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).c, 'ñ');
    }

    #[test]
    fn test_cursor_up_clamped() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[100A");
        let g = grid.borrow();
        assert_eq!(g.cursor_row(), 0);
    }

    #[test]
    fn test_cursor_down_clamped() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[100B");
        let g = grid.borrow();
        assert_eq!(g.cursor_row(), 23);
    }

    #[test]
    fn test_cursor_fwd_clamped() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[200C");
        let g = grid.borrow();
        assert_eq!(g.cursor_col(), 79);
    }
}
