use std::time::Instant;

use luna_config::{Config, Keybinds};
use luna_ui::pane::PaneId;
use luna_ui::splitter::{PaneRect, SplitDirection};
use winit::keyboard::ModifiersState;

#[derive(Debug, Clone)]
pub struct Selection {
    pub start: (usize, usize),
    pub end: (usize, usize),
}

impl Selection {
    pub fn new(col: usize, row: usize) -> Self {
        Self {
            start: (col, row),
            end: (col, row),
        }
    }

    pub fn update_end(&mut self, col: usize, row: usize) {
        self.end = (col, row);
    }

    pub fn normalized(&self) -> ((usize, usize), (usize, usize)) {
        let (s_col, s_row) = self.start;
        let (e_col, e_row) = self.end;

        if s_row < e_row || (s_row == e_row && s_col <= e_col) {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }

    pub fn contains(&self, col: usize, row: usize) -> bool {
        let (min, max) = self.normalized();
        if row < min.1 || row > max.1 {
            return false;
        }
        if row == min.1 && row == max.1 {
            return col >= min.0 && col <= max.0;
        }
        if row == min.1 {
            return col >= min.0;
        }
        if row == max.1 {
            return col <= max.0;
        }
        true
    }
}

pub struct DividerDrag {
    pub pane_id: PaneId,
    pub direction: SplitDirection,
    pub parent_rect: PaneRect,
}

#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub col: usize,
    pub row: usize,
}

pub struct SearchState {
    pub active: bool,
    pub term: String,
    pub matches: Vec<SearchMatch>,
    pub current_match: usize,
    pub cursor_pos: usize,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            active: false,
            term: String::new(),
            matches: Vec::new(),
            current_match: 0,
            cursor_pos: 0,
        }
    }

    pub fn toggle(&mut self) {
        self.active = !self.active;
        if !self.active {
            self.term.clear();
            self.matches.clear();
            self.current_match = 0;
            self.cursor_pos = 0;
        }
    }

    pub fn next_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match = (self.current_match + 1) % self.matches.len();
        }
    }

    pub fn prev_match(&mut self) {
        if !self.matches.is_empty() {
            if self.current_match == 0 {
                self.current_match = self.matches.len() - 1;
            } else {
                self.current_match -= 1;
            }
        }
    }

    pub fn insert_char(&mut self, c: char) {
        let pos = self.cursor_pos.min(self.term.len());
        self.term.insert(pos, c);
        self.cursor_pos = pos + 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.term.remove(self.cursor_pos);
        }
    }

    pub fn delete_forward(&mut self) {
        if self.cursor_pos < self.term.len() {
            self.term.remove(self.cursor_pos);
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor_pos < self.term.len() {
            self.cursor_pos += 1;
        }
    }

    pub fn move_home(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor_pos = self.term.len();
    }
}

pub struct AppState {
    pub config: Config,
    pub keybinds: Keybinds,
    pub modifiers: ModifiersState,
    pub selection: Option<Selection>,
    pub selecting: bool,
    pub cursor_x: f64,
    pub cursor_y: f64,
    pub dragging_divider: Option<DividerDrag>,
    pub hover_divider: bool,
    pub hover_tab: Option<usize>,
    pub last_click_time: Instant,
    pub click_count: u8,
    pub search: SearchState,
    pub history_search: HistorySearchState,
    pub font_size: f32,
    pub fullscreen: bool,
    pub tab_scroll_offset: usize,
}

impl AppState {
    pub fn new(config: Config, keybinds: Keybinds, font_size: f32) -> Self {
        Self {
            config,
            keybinds,
            modifiers: ModifiersState::empty(),
            selection: None,
            selecting: false,
            cursor_x: 0.0,
            cursor_y: 0.0,
            dragging_divider: None,
            hover_divider: false,
            hover_tab: None,
            last_click_time: Instant::now(),
            click_count: 0,
            search: SearchState::new(),
            history_search: HistorySearchState::new(),
            font_size,
            fullscreen: false,
            tab_scroll_offset: 0,
        }
    }
}

pub struct HistorySearchState {
    pub active: bool,
    pub term: String,
    pub history: Vec<String>,
    pub matches: Vec<usize>,
    pub current_match: usize,
}

impl HistorySearchState {
    pub fn new() -> Self {
        Self {
            active: false,
            term: String::new(),
            history: Vec::new(),
            matches: Vec::new(),
            current_match: 0,
        }
    }

    pub fn activate(&mut self) {
        self.active = true;
        self.term.clear();
        self.matches.clear();
        self.current_match = 0;
    }

    pub fn deactivate(&mut self) {
        self.active = false;
        self.term.clear();
        self.history.clear();
        self.matches.clear();
        self.current_match = 0;
    }

    pub fn next_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match = (self.current_match + 1) % self.matches.len();
        }
    }

    pub fn current_text(&self) -> Option<&str> {
        if self.matches.is_empty() {
            None
        } else {
            self.history.get(self.matches[self.current_match]).map(|s| s.as_str())
        }
    }

    pub fn build_history(&mut self, lines: &[Vec<char>]) {
        let mut seen = std::collections::HashSet::new();
        self.history.clear();
        for line in lines.iter().rev() {
            let text: String = line.iter().collect();
            let trimmed = text.trim();
            if !trimmed.is_empty() && trimmed.len() > 1 {
                let owned = trimmed.to_string();
                if seen.insert(owned.clone()) {
                    self.history.push(owned);
                }
            }
        }
    }

    pub fn update_filter(&mut self) {
        let term_lower = self.term.to_lowercase();
        self.matches.clear();
        self.current_match = 0;
        for (i, line) in self.history.iter().enumerate() {
            if line.to_lowercase().contains(&term_lower) {
                self.matches.push(i);
            }
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.term.push(c);
    }

    pub fn backspace(&mut self) {
        self.term.pop();
    }
}
