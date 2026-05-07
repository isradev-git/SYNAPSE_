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
}
