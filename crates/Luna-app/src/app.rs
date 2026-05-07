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

pub struct AppState {
    pub modifiers: ModifiersState,
    pub selection: Option<Selection>,
    pub selecting: bool,
    pub cursor_x: f64,
    pub cursor_y: f64,
    pub dragging_divider: Option<DividerDrag>,
    pub hover_divider: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            modifiers: ModifiersState::empty(),
            selection: None,
            selecting: false,
            cursor_x: 0.0,
            cursor_y: 0.0,
            dragging_divider: None,
            hover_divider: false,
        }
    }
}
