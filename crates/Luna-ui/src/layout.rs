pub const SCROLL_BTN_W: f32 = 20.0;

pub struct Layout {
    pub window_width: f32,
    pub window_height: f32,
    pub tab_bar_height: f32,
}

impl Layout {
    pub fn new() -> Self {
        Self {
            window_width: 1280.0,
            window_height: 800.0,
            tab_bar_height: crate::theme::TAB_BAR_HEIGHT,
        }
    }

    pub fn update(&mut self, width: f32, height: f32) {
        self.window_width = width;
        self.window_height = height;
    }

    pub fn pane_area(&self) -> (f32, f32, f32, f32) {
        let x = 0.0;
        let y = self.tab_bar_height;
        let w = self.window_width;
        let h = self.window_height - self.tab_bar_height;
        (x, y, w, h.max(0.0))
    }

    pub fn pane_margin(&self) -> f32 {
        4.0
    }

    pub fn tab_width(&self, tab_count: usize) -> f32 {
        if tab_count == 0 {
            return 32.0;
        }
        let available = self.window_width - 56.0; // + and separator space
        let per_tab = available / tab_count as f32;
        per_tab.min(200.0).max(80.0)
    }

    pub fn tab_x(&self, index: usize, tab_count: usize) -> f32 {
        index as f32 * self.tab_width(tab_count)
    }

    /// Returns (first_visible_idx, end_exclusive, show_left_btn, show_right_btn).
    pub fn tab_visible_range(&self, tab_count: usize, offset: usize) -> (usize, usize, bool, bool) {
        if tab_count == 0 {
            return (0, 0, false, false);
        }
        let available = self.window_width - 56.0;
        let max_vis = (available / 80.0).floor().max(1.0) as usize;
        if tab_count <= max_vis {
            return (0, tab_count, false, false);
        }
        let show_left = offset > 0;
        let left_w = if show_left { SCROLL_BTN_W } else { 0.0 };
        let avail = available - left_w - SCROLL_BTN_W;
        let vis = (avail / 80.0).floor().max(1.0) as usize;
        let start = offset.min(tab_count.saturating_sub(1));
        let end = (start + vis).min(tab_count);
        (start, end, show_left, end < tab_count)
    }

    /// Width of each tab when rendering only `vis_count` tabs with optional scroll buttons.
    pub fn scrolled_tab_width(&self, vis_count: usize, show_left: bool, show_right: bool) -> f32 {
        let left = if show_left { SCROLL_BTN_W } else { 0.0 };
        let right = if show_right { SCROLL_BTN_W } else { 0.0 };
        let available = self.window_width - 56.0 - left - right;
        (available / vis_count.max(1) as f32).min(200.0).max(80.0)
    }
}
