use std::cell::RefCell;
use std::rc::Rc;

use crate::grid::{CellFlags, CharCell, Color, Grid};
use crate::kitty::KittyKeyboard;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum MouseReportMode {
    #[default]
    None,
    X10,
    ButtonMotion,
    AnyMotion,
}

#[derive(Debug, Clone, Default)]
pub struct TerminalModes {
    pub bracketed_paste: bool,
    pub mouse_report: MouseReportMode,
    pub mouse_sgr: bool,
    pub focus_events: bool,
    pub application_cursor: bool,
    pub kitty: KittyKeyboard,
}

pub struct VteProcessor {
    grid: Rc<RefCell<Grid>>,
    fg: Color,
    bg: Color,
    flags: CellFlags,
    title: Rc<RefCell<String>>,
    cwd: Rc<RefCell<String>>,
    modes: Rc<RefCell<TerminalModes>>,
    use_dec_graphics: bool,
    pending_kitty_responses: Vec<Vec<u8>>,
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
            modes: Rc::new(RefCell::new(TerminalModes::default())),
            use_dec_graphics: false,
            pending_kitty_responses: Vec::new(),
        }
    }

    pub fn new_with_title(
        grid: Rc<RefCell<Grid>>,
        title: Rc<RefCell<String>>,
        cwd: Rc<RefCell<String>>,
        modes: Rc<RefCell<TerminalModes>>,
    ) -> Self {
        Self {
            grid,
            fg: Color::Default,
            bg: Color::Default,
            flags: CellFlags::empty(),
            title,
            cwd,
            modes,
            use_dec_graphics: false,
            pending_kitty_responses: Vec::new(),
        }
    }

    pub fn title_rc(&self) -> Rc<RefCell<String>> {
        self.title.clone()
    }

    pub fn cwd_rc(&self) -> Rc<RefCell<String>> {
        self.cwd.clone()
    }

    pub fn process(&mut self, bytes: &[u8]) {
        let filtered = self.preprocess_kitty(bytes);
        let mut parser = vte::Parser::new();
        for &byte in &filtered {
            parser.advance(self, byte);
        }
    }

    pub fn drain_kitty_responses(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.pending_kitty_responses)
    }

    /// Scan raw bytes for Kitty keyboard protocol CSI sequences.
    /// Handle them directly and remove them from the byte stream.
    /// Supported:
    ///   CSI ? u          → query flags → respond with CSI ? flags u
    ///   CSI = flags ; mode u → set flags with mode
    ///   CSI > flags u   → push flags
    ///   CSI < n u       → pop n entries
    fn preprocess_kitty(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut result = Vec::with_capacity(bytes.len());
        let mut i = 0;

        while i < bytes.len() {
            if i + 3 < bytes.len()
                && bytes[i] == 0x1b
                && bytes[i + 1] == b'['
            {
                let marker = bytes[i + 2];
                if matches!(marker, b'?' | b'=' | b'>' | b'<') {
                    let start = i;
                    i += 3; // skip ESC [ marker

                    // Collect parameter bytes (0x30-0x3F) and intermediates (0x20-0x2F)
                    let param_start = i;
                    while i < bytes.len()
                        && (bytes[i] >= 0x20 && bytes[i] <= 0x3F)
                    {
                        i += 1;
                    }
                    let param_bytes = &bytes[param_start..i];

                    if i < bytes.len()
                        && bytes[i] >= 0x40
                        && bytes[i] <= 0x7E
                    {
                        let final_byte = bytes[i];
                        i += 1;

                        if final_byte == b'u' {
                            let handled = match marker {
                                b'?' => self.handle_kitty_query(),
                                b'=' => self.handle_kitty_set(param_bytes),
                                b'>' => self.handle_kitty_push(param_bytes),
                                b'<' => self.handle_kitty_pop(param_bytes),
                                _ => false,
                            };
                            if handled {
                                continue; // consumed the sequence
                            }
                        }
                    }
                    // Not a kitty sequence, pass through unchanged
                    result.extend_from_slice(&bytes[start..i]);
                    continue;
                }
            }
            result.push(bytes[i]);
            i += 1;
        }
        result
    }

    fn handle_kitty_query(&mut self) -> bool {
        let flags = self.modes.borrow().kitty.flags;
        let response = format!("\x1b[?{}u", flags);
        self.pending_kitty_responses
            .push(response.into_bytes());
        true
    }

    fn handle_kitty_set(&mut self, param_bytes: &[u8]) -> bool {
        let (flags, mode) = parse_kitty_params(param_bytes);
        let mode = if mode == 0 { 1 } else { mode };
        self.modes.borrow_mut().kitty.set_flags(flags, mode);
        true
    }

    fn handle_kitty_push(&mut self, param_bytes: &[u8]) -> bool {
        let (flags, _) = parse_kitty_params(param_bytes);
        self.modes.borrow_mut().kitty.push(flags);
        true
    }

    fn handle_kitty_pop(&mut self, param_bytes: &[u8]) -> bool {
        let (n, _) = parse_kitty_params(param_bytes);
        let n = if n == 0 { 1 } else { n as usize };
        self.modes.borrow_mut().kitty.pop(n);
        true
    }
}

impl vte::Perform for VteProcessor {
    fn print(&mut self, c: char) {
        let c = if self.use_dec_graphics && c.is_ascii() {
            dec_graphics(c as u8)
        } else {
            c
        };
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
        match byte {
            b'\x0e' => {
                self.use_dec_graphics = true;
                return;
            }
            b'\x0f' => {
                self.use_dec_graphics = false;
                return;
            }
            _ => {}
        }
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

    fn hook(&mut self, _params: &vte::Params, _intermediates: &[u8], _ignore: bool, _action: char) {
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
            "0" | "2" if !trimmed.is_empty() => {
                *self.title.borrow_mut() = trimmed.to_string();
            }
            "0" | "2" => {}
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
        intermediates: &[u8],
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
            'G' => {
                let col = (get_param(params, 0).max(1) as usize).saturating_sub(1);
                let mut grid = self.grid.borrow_mut();
                let max_col = grid.cols().saturating_sub(1);
                let row = grid.cursor_row();
                grid.set_cursor(col.min(max_col), row);
            }
            'd' => {
                let row = (get_param(params, 0).max(1) as usize).saturating_sub(1);
                let mut grid = self.grid.borrow_mut();
                let max_row = grid.rows().saturating_sub(1);
                let col = grid.cursor_col();
                grid.set_cursor(col, row.min(max_row));
            }
            'E' => {
                let mut grid = self.grid.borrow_mut();
                let max_row = grid.rows().saturating_sub(1);
                let new_row = (grid.cursor_row() + n).min(max_row);
                grid.set_cursor(0, new_row);
            }
            'F' => {
                let mut grid = self.grid.borrow_mut();
                let new_row = grid.cursor_row().saturating_sub(n);
                grid.set_cursor(0, new_row);
            }
            '@' => {
                self.grid.borrow_mut().insert_chars(n);
            }
            'P' => {
                self.grid.borrow_mut().delete_chars(n);
            }
            'L' => {
                self.grid.borrow_mut().insert_lines(n);
            }
            'M' => {
                self.grid.borrow_mut().delete_lines(n);
            }
            'S' => {
                self.grid.borrow_mut().shift_up_region(n);
            }
            'T' => {
                self.grid.borrow_mut().shift_down_region(n);
            }
            'X' => {
                self.grid.borrow_mut().erase_chars(n);
            }
            'r' if intermediates.is_empty() => {
                let p0 = get_param(params, 0);
                let p1 = get_param(params, 1);
                let mut grid = self.grid.borrow_mut();
                let rows = grid.rows();
                let top = if p0 <= 0 {
                    0
                } else {
                    (p0 as usize - 1).min(rows - 1)
                };
                let bottom = if p1 <= 0 {
                    rows - 1
                } else {
                    (p1 as usize - 1).min(rows - 1)
                };
                grid.set_scroll_region(top, bottom);
            }
            'm' => {
                self.handle_sgr(params);
            }
            'h' | 'l' => {
                let enable = action == 'h';
                if intermediates.contains(&b'?') {
                    let mode_num = get_param(params, 0);
                    let mut modes = self.modes.borrow_mut();
                    match mode_num {
                        1 => modes.application_cursor = enable,
                        1000 => {
                            modes.mouse_report = if enable {
                                MouseReportMode::X10
                            } else {
                                MouseReportMode::None
                            }
                        }
                        1002 => {
                            modes.mouse_report = if enable {
                                MouseReportMode::ButtonMotion
                            } else {
                                MouseReportMode::None
                            }
                        }
                        1003 => {
                            modes.mouse_report = if enable {
                                MouseReportMode::AnyMotion
                            } else {
                                MouseReportMode::None
                            }
                        }
                        1004 => modes.focus_events = enable,
                        1006 => modes.mouse_sgr = enable,
                        2004 => modes.bracketed_paste = enable,
                        _ => {}
                    }
                }
            }
            's' => {
                self.grid.borrow_mut().save_cursor();
            }
            'u' => {
                self.grid.borrow_mut().restore_cursor();
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        match (intermediates.first().copied(), byte) {
            (None, b'7') => {
                self.grid.borrow_mut().save_cursor();
            }
            (None, b'8') => {
                self.grid.borrow_mut().restore_cursor();
            }
            (None, b'c') => {
                self.reset_state();
                self.use_dec_graphics = false;
                let mut g = self.grid.borrow_mut();
                let last_row = g.rows() - 1;
                g.clear_region(0, last_row);
                g.set_cursor(0, 0);
                g.set_scroll_region(0, last_row);
                drop(g);
                *self.modes.borrow_mut() = TerminalModes::default();
            }
            (Some(b'('), b'0') => {
                self.use_dec_graphics = true;
            }
            (Some(b'('), b'B') => {
                self.use_dec_graphics = false;
            }
            _ => {}
        }
    }
}

fn dec_graphics(b: u8) -> char {
    match b {
        b'j' => '┘',
        b'k' => '┐',
        b'l' => '┌',
        b'm' => '└',
        b'n' => '┼',
        b'q' => '─',
        b't' => '├',
        b'u' => '┤',
        b'v' => '┴',
        b'w' => '┬',
        b'x' => '│',
        b'`' => '◆',
        b'a' => '▒',
        b'f' => '°',
        b'g' => '±',
        b'o' => '⎺',
        b'p' => '⎻',
        b'r' => '⎼',
        b's' => '⎽',
        b'y' => '≤',
        b'z' => '≥',
        b'~' => '·',
        _ => b as char,
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

/// Parse Kitty CSI parameter bytes "flags ; mode" or multiple flag values.
/// All numeric values are OR'd together to form the flags.
/// If exactly 2 values are present and the second is 1-3, it's treated as mode.
fn parse_kitty_params(param_bytes: &[u8]) -> (u8, u8) {
    let s = String::from_utf8_lossy(param_bytes);
    let parts: Vec<u16> = s.split(';')
        .filter_map(|p| p.trim().parse::<u16>().ok())
        .collect();

    if parts.is_empty() {
        return (0, 1);
    }

    // If exactly 2 values and the second looks like a mode (1-3), use it
    if parts.len() == 2 && parts[1] >= 1 && parts[1] <= 3 {
        return (parts[0].min(255) as u8, parts[1] as u8);
    }

    // OR all values together for flags, mode defaults to 1
    let flags: u16 = parts.iter().fold(0u16, |acc, &v| acc | v);
    (flags.min(255) as u8, 1)
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
                            2 if i + 4 < flat.len() => {
                                self.fg = Color::Rgb(
                                    flat[i + 2] as u8,
                                    flat[i + 3] as u8,
                                    flat[i + 4] as u8,
                                );
                                i += 4;
                            }
                            5 if i + 2 < flat.len() => {
                                self.fg = Color::Indexed(flat[i + 2] as u8);
                                i += 2;
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
                            2 if i + 4 < flat.len() => {
                                self.bg = Color::Rgb(
                                    flat[i + 2] as u8,
                                    flat[i + 3] as u8,
                                    flat[i + 4] as u8,
                                );
                                i += 4;
                            }
                            5 if i + 2 < flat.len() => {
                                self.bg = Color::Indexed(flat[i + 2] as u8);
                                i += 2;
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
        assert!(proc
            .flags
            .contains(CellFlags::UNDERLINE | CellFlags::ITALIC | CellFlags::BOLD));
        proc.process(b"\x1b[24;23;22m");
        assert!(!proc
            .flags
            .contains(CellFlags::UNDERLINE | CellFlags::ITALIC | CellFlags::BOLD));
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
        let modes = Rc::new(RefCell::new(TerminalModes::default()));
        let mut proc = VteProcessor::new_with_title(grid, title.clone(), cwd, modes);
        proc.process(b"\x1b]0;MyTitle\x07");
        assert_eq!(&*title.borrow(), "MyTitle");
    }

    #[test]
    fn test_osc_set_title_osc2() {
        let grid = make_grid(80, 24);
        let title = Rc::new(RefCell::new(String::new()));
        let cwd = Rc::new(RefCell::new(String::new()));
        let modes = Rc::new(RefCell::new(TerminalModes::default()));
        let mut proc = VteProcessor::new_with_title(grid, title.clone(), cwd, modes);
        proc.process(b"\x1b]2;Another Title\x07");
        assert_eq!(&*title.borrow(), "Another Title");
    }

    #[test]
    fn test_osc_set_cwd() {
        let grid = make_grid(80, 24);
        let title = Rc::new(RefCell::new(String::new()));
        let cwd = Rc::new(RefCell::new(String::new()));
        let modes = Rc::new(RefCell::new(TerminalModes::default()));
        let mut proc = VteProcessor::new_with_title(grid, title, cwd.clone(), modes);
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

    // ── R-016 vttest conformance tests ──────────────────────────────────

    #[test]
    fn test_cha_cursor_horizontal_absolute() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"HELLO\x1b[3G");
        let g = grid.borrow();
        assert_eq!(g.cursor_col(), 2);
    }

    #[test]
    fn test_vpa_vertical_position_absolute() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[5d");
        let g = grid.borrow();
        assert_eq!(g.cursor_row(), 4);
    }

    #[test]
    fn test_cnl_cursor_next_line() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[5;5H\x1b[2E");
        let g = grid.borrow();
        assert_eq!(g.cursor_col(), 0);
        assert_eq!(g.cursor_row(), 6);
    }

    #[test]
    fn test_cpl_cursor_prev_line() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[5;5H\x1b[2F");
        let g = grid.borrow();
        assert_eq!(g.cursor_col(), 0);
        assert_eq!(g.cursor_row(), 2);
    }

    #[test]
    fn test_ich_insert_chars() {
        let grid = make_grid(10, 5);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"ABCDE\x1b[1;3H\x1b[2@");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).c, 'A');
        assert_eq!(g.get(1, 0).c, 'B');
        assert_eq!(g.get(2, 0).c, ' ');
        assert_eq!(g.get(3, 0).c, ' ');
        assert_eq!(g.get(4, 0).c, 'C');
    }

    #[test]
    fn test_dch_delete_chars() {
        let grid = make_grid(10, 5);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"ABCDE\x1b[1;2H\x1b[2P");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).c, 'A');
        assert_eq!(g.get(1, 0).c, 'D');
        assert_eq!(g.get(2, 0).c, 'E');
        assert_eq!(g.get(3, 0).c, ' ');
    }

    #[test]
    fn test_ech_erase_chars() {
        let grid = make_grid(10, 5);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"ABCDE\x1b[1;2H\x1b[3X");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).c, 'A');
        assert_eq!(g.get(1, 0).c, ' ');
        assert_eq!(g.get(2, 0).c, ' ');
        assert_eq!(g.get(3, 0).c, ' ');
        assert_eq!(g.get(4, 0).c, 'E');
    }

    #[test]
    fn test_decstbm_scroll_region() {
        let grid = make_grid(10, 5);
        let mut proc = VteProcessor::new(grid.clone());
        // Set scroll region rows 2-4 (1-based), fill some content
        proc.process(b"\x1b[1;1H"); // cursor home
        proc.process(b"AAAAA");
        proc.process(b"\x1b[2;1H");
        proc.process(b"BBBBB");
        proc.process(b"\x1b[3;1H");
        proc.process(b"CCCCC");
        proc.process(b"\x1b[4;1H");
        proc.process(b"DDDDD");
        proc.process(b"\x1b[5;1H");
        proc.process(b"EEEEE");

        // Set scroll region to rows 3-5 (0-indexed 2-4)
        proc.process(b"\x1b[3;5r");
        // Cursor should be at home after DECSTBM
        {
            let g = grid.borrow();
            assert_eq!(g.cursor_row(), 0);
        }
        // Scroll up within region: row 3 → row 2, row 4 → row 3, row 2 (new) → blank
        proc.process(b"\x1b[5;1H"); // go to row 5 (bottom of region)
        proc.process(b"\n"); // LF at bottom of region triggers scroll

        let g = grid.borrow();
        // Row 0 (A) and row 1 (B) should be unaffected
        assert_eq!(g.get(0, 0).c, 'A');
        assert_eq!(g.get(0, 1).c, 'B');
        // Within region: D scrolled to row 2, E scrolled to row 3, row 4 blank
        assert_eq!(g.get(0, 2).c, 'D');
        assert_eq!(g.get(0, 3).c, 'E');
        assert_eq!(g.get(0, 4).c, ' ');
    }

    #[test]
    fn test_il_insert_lines() {
        let grid = make_grid(10, 5);
        let mut proc = VteProcessor::new(grid.clone());
        for (i, row_str) in [b"AAAAA" as &[u8], b"BBBBB", b"CCCCC"].iter().enumerate() {
            proc.process(format!("\x1b[{};1H", i + 1).as_bytes());
            proc.process(row_str);
        }
        // Insert 1 line at row 2 (1-based)
        proc.process(b"\x1b[2;1H\x1b[1L");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).c, 'A');
        assert_eq!(g.get(0, 1).c, ' '); // newly inserted blank
        assert_eq!(g.get(0, 2).c, 'B');
        assert_eq!(g.get(0, 3).c, 'C');
    }

    #[test]
    fn test_dl_delete_lines() {
        let grid = make_grid(10, 5);
        let mut proc = VteProcessor::new(grid.clone());
        for (i, row_str) in [b"AAAAA" as &[u8], b"BBBBB", b"CCCCC", b"DDDDD"]
            .iter()
            .enumerate()
        {
            proc.process(format!("\x1b[{};1H", i + 1).as_bytes());
            proc.process(row_str);
        }
        // Delete 1 line at row 2 (1-based)
        proc.process(b"\x1b[2;1H\x1b[1M");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).c, 'A');
        assert_eq!(g.get(0, 1).c, 'C'); // B deleted, C shifted up
        assert_eq!(g.get(0, 2).c, 'D');
        assert_eq!(g.get(0, 3).c, ' '); // blank
    }

    #[test]
    fn test_dec_graphics_line_drawing() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        // ESC(0 switches to DEC Special Graphics
        proc.process(b"\x1b(0jklmnqx\x1b(B");
        let g = grid.borrow();
        assert_eq!(g.get(0, 0).c, '┘');
        assert_eq!(g.get(1, 0).c, '┐');
        assert_eq!(g.get(2, 0).c, '┌');
        assert_eq!(g.get(3, 0).c, '└');
        assert_eq!(g.get(4, 0).c, '┼');
        assert_eq!(g.get(5, 0).c, '─');
        assert_eq!(g.get(6, 0).c, '│');
    }

    #[test]
    fn test_decckm_application_cursor() {
        let grid = make_grid(80, 24);
        let title = Rc::new(RefCell::new(String::new()));
        let cwd = Rc::new(RefCell::new(String::new()));
        let modes = Rc::new(RefCell::new(TerminalModes::default()));
        let mut proc = VteProcessor::new_with_title(grid, title, cwd, modes.clone());
        assert!(!modes.borrow().application_cursor);
        proc.process(b"\x1b[?1h");
        assert!(modes.borrow().application_cursor);
        proc.process(b"\x1b[?1l");
        assert!(!modes.borrow().application_cursor);
    }

    // ── Kitty keyboard protocol tests ────────────────────────────────────

    #[test]
    fn test_kitty_query_flags_response() {
        let grid = make_grid(80, 24);
        let title = Rc::new(RefCell::new(String::new()));
        let cwd = Rc::new(RefCell::new(String::new()));
        let modes = Rc::new(RefCell::new(TerminalModes::default()));
        let mut proc = VteProcessor::new_with_title(grid, title, cwd, modes.clone());
        proc.process(b"\x1b[?u");
        let responses = proc.drain_kitty_responses();
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0], b"\x1b[?0u");
    }

    #[test]
    fn test_kitty_set_disambiguate() {
        let grid = make_grid(80, 24);
        let title = Rc::new(RefCell::new(String::new()));
        let cwd = Rc::new(RefCell::new(String::new()));
        let modes = Rc::new(RefCell::new(TerminalModes::default()));
        let mut proc = VteProcessor::new_with_title(grid, title, cwd, modes.clone());
        proc.process(b"\x1b[=1u");
        assert_eq!(modes.borrow().kitty.flags, 1);
        assert!(modes.borrow().kitty.is_disambiguate());
    }

    #[test]
    fn test_kitty_set_multiple_flags() {
        let grid = make_grid(80, 24);
        let title = Rc::new(RefCell::new(String::new()));
        let cwd = Rc::new(RefCell::new(String::new()));
        let modes = Rc::new(RefCell::new(TerminalModes::default()));
        let mut proc = VteProcessor::new_with_title(grid, title, cwd, modes.clone());
        proc.process(b"\x1b[=1;2;4u");
        assert_eq!(modes.borrow().kitty.flags, 7);
    }

    #[test]
    fn test_kitty_set_with_mode() {
        let grid = make_grid(80, 24);
        let title = Rc::new(RefCell::new(String::new()));
        let cwd = Rc::new(RefCell::new(String::new()));
        let modes = Rc::new(RefCell::new(TerminalModes::default()));
        let mut proc = VteProcessor::new_with_title(grid, title, cwd, modes.clone());
        // Set flag 1 with mode 1 (replace)
        proc.process(b"\x1b[=1;1u");
        assert_eq!(modes.borrow().kitty.flags, 1);
        // Add flag 2 with mode 2 (OR)
        proc.process(b"\x1b[=2;2u");
        assert_eq!(modes.borrow().kitty.flags, 3);
    }

    #[test]
    fn test_kitty_push_pop() {
        let grid = make_grid(80, 24);
        let title = Rc::new(RefCell::new(String::new()));
        let cwd = Rc::new(RefCell::new(String::new()));
        let modes = Rc::new(RefCell::new(TerminalModes::default()));
        let mut proc = VteProcessor::new_with_title(grid, title, cwd, modes.clone());
        proc.process(b"\x1b[>1u");
        assert_eq!(modes.borrow().kitty.flags, 1);
        proc.process(b"\x1b[>3u");
        assert_eq!(modes.borrow().kitty.flags, 3);
        proc.process(b"\x1b[<1u");
        assert_eq!(modes.borrow().kitty.flags, 1);
    }

    #[test]
    fn test_kitty_normal_csi_u_still_works() {
        let grid = make_grid(80, 24);
        let mut proc = VteProcessor::new(grid.clone());
        proc.process(b"\x1b[5;10H\x1b[s");
        proc.process(b"\x1b[1;1H\x1b[u");
        let g = grid.borrow();
        assert_eq!(g.cursor_col(), 9);
        assert_eq!(g.cursor_row(), 4);
    }
}
