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
}
