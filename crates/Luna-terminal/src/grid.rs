use bitflags::bitflags;

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct CellFlags: u8 {
        const BOLD      = 0b00000001;
        const ITALIC    = 0b00000010;
        const UNDERLINE = 0b00000100;
        const BLINK     = 0b00001000;
        const INVERSE   = 0b00010000;
        const INVISIBLE = 0b00100000;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Color {
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

impl Color {
    pub fn fg_rgba(&self) -> [f32; 4] {
        match self {
            Color::Default => [1.0, 1.0, 1.0, 1.0],
            Color::Indexed(idx) => ansi_256_to_rgba(*idx),
            Color::Rgb(r, g, b) => [
                *r as f32 / 255.0,
                *g as f32 / 255.0,
                *b as f32 / 255.0,
                1.0,
            ],
        }
    }

    pub fn bg_rgba(&self) -> [f32; 4] {
        match self {
            Color::Default => [0.0, 0.0, 0.0, 0.0],
            Color::Indexed(idx) => ansi_256_to_rgba(*idx),
            Color::Rgb(r, g, b) => [
                *r as f32 / 255.0,
                *g as f32 / 255.0,
                *b as f32 / 255.0,
                1.0,
            ],
        }
    }

    pub fn to_rgba_f32(&self) -> [f32; 4] {
        self.fg_rgba()
    }
}

fn ansi_256_to_rgba(idx: u8) -> [f32; 4] {
    match idx {
        0..=15 => {
            const NAMES: [[f32; 3]; 16] = [
                [0.0,      0.0,      0.0     ], // Black
                [0.8,      0.0,      0.0     ], // Red
                [0.0,      0.8,      0.0     ], // Green
                [0.8,      0.8,      0.0     ], // Yellow
                [0.0,      0.0,      0.8     ], // Blue
                [0.8,      0.0,      0.8     ], // Magenta
                [0.0,      0.8,      0.8     ], // Cyan
                [0.749,    0.749,    0.749   ], // White
                [0.501,    0.501,    0.501   ], // Bright Black
                [1.0,      0.333,    0.333   ], // Bright Red
                [0.333,    1.0,      0.333   ], // Bright Green
                [1.0,      1.0,      0.333   ], // Bright Yellow
                [0.333,    0.333,    1.0     ], // Bright Blue
                [1.0,      0.333,    1.0     ], // Bright Magenta
                [0.333,    1.0,      1.0     ], // Bright Cyan
                [1.0,      1.0,      1.0     ], // Bright White
            ];
            let [r, g, b] = NAMES[idx as usize];
            [r, g, b, 1.0]
        }
        16..=231 => {
            let offset = idx - 16;
            let r = if offset < 36 {
                0.0
            } else if offset < 72 {
                0.333
            } else if offset < 108 {
                0.502
            } else if offset < 144 {
                0.667
            } else if offset < 180 {
                0.835
            } else {
                1.0
            };
            let g = match (offset / 6) % 6 {
                0 => 0.0, 1 => 0.333, 2 => 0.502,
                3 => 0.667, 4 => 0.835, _ => 1.0,
            };
            let b = match offset % 6 {
                0 => 0.0, 1 => 0.333, 2 => 0.502,
                3 => 0.667, 4 => 0.835, _ => 1.0,
            };
            [r, g, b, 1.0]
        }
        232..=255 => {
            let v = 0.031 + (idx - 232) as f32 * (1.0 - 0.031) / 23.0;
            [v, v, v, 1.0]
        }
    }
}

#[derive(Clone, Debug)]
pub struct CharCell {
    pub c: char,
    pub fg: Color,
    pub bg: Color,
    pub flags: CellFlags,
    pub dirty: bool,
}

impl Default for CharCell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: Color::Default,
            bg: Color::Default,
            flags: CellFlags::empty(),
            dirty: true,
        }
    }
}

impl CharCell {
    pub fn reset_attrs(&mut self) {
        self.fg = Color::Default;
        self.bg = Color::Default;
        self.flags = CellFlags::empty();
        self.dirty = true;
    }
}

pub struct Grid {
    cells: Vec<CharCell>,
    cols: usize,
    rows: usize,
    cursor_col: usize,
    cursor_row: usize,
    scrollback: crate::buffer::ScrollbackBuffer,
    scroll_offset: usize,
    saved_cursor_col: usize,
    saved_cursor_row: usize,
}

impl Grid {
    pub fn new(cols: usize, rows: usize) -> Self {
        let capacity = cols * rows;
        let mut cells = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            cells.push(CharCell::default());
        }
        Self {
            cells,
            cols,
            rows,
            cursor_col: 0,
            cursor_row: 0,
            scrollback: crate::buffer::ScrollbackBuffer::new(100_000),
            scroll_offset: 0,
            saved_cursor_col: 0,
            saved_cursor_row: 0,
        }
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cursor_col(&self) -> usize {
        self.cursor_col
    }

    pub fn cursor_row(&self) -> usize {
        self.cursor_row
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub fn scroll_up(&mut self, lines: usize) {
        let total = self.scrollback.len();
        self.scroll_offset = (self.scroll_offset + lines).min(total);
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = self.scrollback.len();
    }

    pub fn is_at_bottom(&self) -> bool {
        self.scroll_offset == 0
    }

    fn index(&self, col: usize, row: usize) -> usize {
        row * self.cols + col
    }

    pub fn set(&mut self, col: usize, row: usize, cell: CharCell) {
        if col < self.cols && row < self.rows {
            let idx = self.index(col, row);
            self.cells[idx] = cell;
        }
    }

    pub fn get(&self, col: usize, row: usize) -> &CharCell {
        if col < self.cols && row < self.rows {
            &self.cells[self.index(col, row)]
        } else {
            // This should not happen, but we need a reference. Use a const.
            // Real implementation would panic or return Option.
            &EMPTY_CELL
        }
    }

    pub fn get_mut(&mut self, col: usize, row: usize) -> Option<&mut CharCell> {
        if col < self.cols && row < self.rows {
            let idx = self.index(col, row);
            Some(&mut self.cells[idx])
        } else {
            None
        }
    }

    pub fn set_cursor(&mut self, col: usize, row: usize) {
        self.cursor_col = col.min(self.cols.saturating_sub(1));
        self.cursor_row = row.min(self.rows.saturating_sub(1));
    }

    pub fn save_cursor(&mut self) {
        self.saved_cursor_col = self.cursor_col;
        self.saved_cursor_row = self.cursor_row;
    }

    pub fn restore_cursor(&mut self) {
        self.cursor_col = self.saved_cursor_col;
        self.cursor_row = self.saved_cursor_row;
    }

    pub fn advance_cursor(&mut self) {
        self.cursor_col += 1;
        if self.cursor_col >= self.cols {
            self.cursor_col = 0;
            self.cursor_row += 1;
            if self.cursor_row >= self.rows {
                self.shift_up(1);
                self.cursor_row = self.rows - 1;
            }
        }
    }

    pub fn new_line(&mut self) {
        self.cursor_col = 0;
        self.cursor_row += 1;
        if self.cursor_row >= self.rows {
            self.scroll_up(1);
            self.cursor_row = self.rows - 1;
        }
    }

    pub fn carriage_return(&mut self) {
        self.cursor_col = 0;
    }

    fn shift_up(&mut self, n: usize) {
        for _ in 0..n {
            let line: Vec<CharCell> = self.cells[..self.cols].to_vec();
            self.scrollback.push(line);

            if self.scroll_offset > 0 {
                self.scroll_offset = (self.scroll_offset + 1).min(self.scrollback.len());
            }

            let row_size = self.cols;
            let total = self.rows * row_size;
            for i in 0..total - row_size {
                self.cells[i] = self.cells[i + row_size].clone();
            }

            let last_row_start = (self.rows - 1) * row_size;
            for i in last_row_start..self.rows * row_size {
                self.cells[i] = CharCell::default();
            }
        }
    }

    pub fn clear_region(&mut self, first_row: usize, last_row_inclusive: usize) {
        let first = first_row.min(last_row_inclusive);
        let last = first_row.max(last_row_inclusive).min(self.rows - 1);
        for row in first..=last {
            let start = self.index(0, row);
            let end = start + self.cols;
            for i in start..end {
                self.cells[i] = CharCell::default();
            }
        }
    }

    pub fn clear_line_from_start(&mut self, col_end: usize) {
        let row_start = self.index(0, self.cursor_row);
        let row_end = self.index((col_end + 1).min(self.cols), self.cursor_row);
        for i in row_start..row_end {
            self.cells[i] = CharCell::default();
        }
    }

    pub fn clear_line(&mut self, col_start: usize) {
        let row_start = self.index(col_start.min(self.cols), self.cursor_row);
        let row_end = self.index(self.cols, self.cursor_row);
        for i in row_start..row_end {
            self.cells[i] = CharCell::default();
        }
    }

    pub fn resize(&mut self, new_cols: usize, new_rows: usize) {
        if new_cols == self.cols && new_rows == self.rows {
            return;
        }

        let mut new_cells = Vec::with_capacity(new_cols * new_rows);
        for row in 0..new_rows {
            for col in 0..new_cols {
                if row < self.rows && col < self.cols {
                    let old_idx = self.index(col, row);
                    new_cells.push(self.cells[old_idx].clone());
                } else {
                    new_cells.push(CharCell::default());
                }
            }
        }

        self.cells = new_cells;
        self.cols = new_cols;
        self.rows = new_rows;

        if self.cursor_col >= new_cols {
            self.cursor_col = new_cols.saturating_sub(1);
        }
        if self.cursor_row >= new_rows {
            self.cursor_row = new_rows.saturating_sub(1);
        }
    }

    pub fn dirty_cells(&self) -> impl Iterator<Item = (usize, usize, &CharCell)> {
        self.cells
            .iter()
            .enumerate()
            .filter(|(_, cell)| cell.dirty)
            .map(|(i, cell)| (i % self.cols, i / self.cols, cell))
    }

    pub fn clear_dirty(&mut self) {
        for cell in &mut self.cells {
            cell.dirty = false;
        }
    }

    /// Get a cell by viewport coordinates (col, visible_row), resolving into scrollback or grid.
    pub fn get_visible(&self, col: usize, vrow: usize) -> Option<&CharCell> {
        let sb_lines = {
            let start = self.scroll_offset;
            let end = (start + self.rows).min(self.scrollback.len());
            end - start
        };

        if vrow < sb_lines {
            let line = self.scrollback.get_line(self.scroll_offset + vrow);
            if col < line.len() {
                Some(&line[col])
            } else {
                None
            }
        } else {
            let grid_row = vrow - sb_lines;
            if grid_row < self.rows && col < self.cols {
                Some(&self.cells[self.index(col, grid_row)])
            } else {
                None
            }
        }
    }

    pub fn visible_cells(&self) -> impl Iterator<Item = (usize, usize, &CharCell)> {
        let start = self.scroll_offset;
        let end = (start + self.rows).min(self.scrollback.len());
        let scrollback_lines = end - start;

        // Scrollback lines first
        let sb_iter = (0..scrollback_lines).flat_map(move |i| {
            let line = self.scrollback.get_line(start + i);
            line.iter().enumerate().map(move |(col, cell)| (col, i, cell))
        });

        // Then visible grid
        let grid_start = scrollback_lines;
        let grid_iter = (grid_start..self.rows).flat_map(move |row| {
            let grid_row = row - grid_start;
            let row_start = self.index(0, grid_row);
            self.cells[row_start..row_start + self.cols]
                .iter()
                .enumerate()
                .map(move |(col, cell)| (col, row, cell))
        });

        sb_iter.chain(grid_iter).collect::<Vec<_>>().into_iter()
    }
}

static EMPTY_CELL: CharCell = CharCell {
    c: ' ',
    fg: Color::Default,
    bg: Color::Default,
    flags: CellFlags::empty(),
    dirty: false,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_new() {
        let grid = Grid::new(80, 24);
        assert_eq!(grid.cols(), 80);
        assert_eq!(grid.rows(), 24);
        assert_eq!(grid.cursor_col(), 0);
        assert_eq!(grid.cursor_row(), 0);
    }

    #[test]
    fn test_set_and_get() {
        let mut grid = Grid::new(80, 24);
        let cell = CharCell {
            c: 'A',
            fg: Color::Rgb(255, 0, 0),
            bg: Color::Default,
            flags: CellFlags::BOLD,
            dirty: true,
        };
        grid.set(10, 5, cell);
        let retrieved = grid.get(10, 5);
        assert_eq!(retrieved.c, 'A');
        assert_eq!(retrieved.fg, Color::Rgb(255, 0, 0));
        assert!(retrieved.flags.contains(CellFlags::BOLD));
    }

    #[test]
    fn test_cursor_advance() {
        let mut grid = Grid::new(80, 24);
        grid.set_cursor(0, 0);
        grid.advance_cursor();
        assert_eq!(grid.cursor_col(), 1);
        assert_eq!(grid.cursor_row(), 0);
    }

    #[test]
    fn test_cursor_wrap() {
        let mut grid = Grid::new(3, 3);
        grid.set_cursor(0, 0);
        for _ in 0..5 {
            grid.advance_cursor();
        }
        assert_eq!(grid.cursor_col(), 2);
        assert_eq!(grid.cursor_row(), 1);
    }

    #[test]
    fn test_scroll_up() {
        let mut grid = Grid::new(80, 3);
        grid.set(0, 0, CharCell { c: 'A', ..CharCell::default() });
        grid.set(0, 1, CharCell { c: 'B', ..CharCell::default() });
        grid.set_cursor(0, 2);
        for _ in 0..80 {
            grid.advance_cursor();
        }
        assert_eq!(grid.get(0, 0).c, 'B');
        assert_eq!(grid.scrollback.len(), 1);
    }

    #[test]
    fn test_ansi_256_colors() {
        assert_eq!(ansi_256_to_rgba(0), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(ansi_256_to_rgba(1), [0.8, 0.0, 0.0, 1.0]);
        assert_eq!(ansi_256_to_rgba(15), [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_resize() {
        let mut grid = Grid::new(80, 24);
        grid.set(0, 0, CharCell { c: 'X', ..CharCell::default() });
        grid.resize(40, 12);
        assert_eq!(grid.cols(), 40);
        assert_eq!(grid.rows(), 12);
        assert_eq!(grid.get(0, 0).c, 'X');
    }
}
