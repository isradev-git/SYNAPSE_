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
        per_tab.clamp(80.0, 200.0)
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
        (available / vis_count.max(1) as f32).clamp(80.0, 200.0)
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout_1280x800() -> Layout {
        Layout {
            window_width: 1280.0,
            window_height: 800.0,
            tab_bar_height: 32.0,
        }
    }

    #[test]
    fn test_pane_area() {
        let layout = layout_1280x800();
        let (x, y, w, h) = layout.pane_area();
        assert_eq!(x, 0.0);
        assert_eq!(y, 32.0);
        assert_eq!(w, 1280.0);
        assert_eq!(h, 768.0);
    }

    #[test]
    fn test_pane_area_zero_height() {
        let mut layout = layout_1280x800();
        layout.window_height = 10.0;
        let (_x, _y, _w, h) = layout.pane_area();
        assert_eq!(h, 0.0); // clamped to 0
    }

    #[test]
    fn test_pane_margin() {
        let layout = layout_1280x800();
        assert_eq!(layout.pane_margin(), 4.0);
    }

    #[test]
    fn test_tab_width_single_tab() {
        let layout = layout_1280x800();
        let width = layout.tab_width(1);
        assert!(width <= 200.0);
        assert!(width >= 80.0);
    }

    #[test]
    fn test_tab_width_many_tabs() {
        let layout = layout_1280x800();
        let width = layout.tab_width(20);
        // With 20 tabs and 1280px - 56px = 1224px, each gets 61px,
        // clamped to minimum 80px
        assert_eq!(width, 80.0); // clamped to min
    }

    #[test]
    fn test_tab_width_few_tabs() {
        let layout = layout_1280x800();
        let width = layout.tab_width(3);
        assert!(width <= 200.0);
        assert!(width >= 80.0);
    }

    #[test]
    fn test_tab_width_clamped_max() {
        let layout = Layout {
            window_width: 3000.0,
            window_height: 800.0,
            tab_bar_height: 32.0,
        };
        let width = layout.tab_width(1);
        assert_eq!(width, 200.0); // clamped to max
    }

    #[test]
    fn test_tab_x() {
        let layout = layout_1280x800();
        let tab_count = 5;
        let w = layout.tab_width(tab_count);
        assert_eq!(layout.tab_x(0, tab_count), 0.0);
        assert_eq!(layout.tab_x(1, tab_count), w);
        assert_eq!(layout.tab_x(2, tab_count), w * 2.0);
    }

    #[test]
    fn test_tab_visible_range_all_fit() {
        let layout = layout_1280x800();
        // 3 tabs fit easily
        let (start, end, show_left, show_right) = layout.tab_visible_range(3, 0);
        assert_eq!(start, 0);
        assert_eq!(end, 3);
        assert!(!show_left);
        assert!(!show_right);
    }

    #[test]
    fn test_tab_visible_range_many_tabs() {
        let layout = layout_1280x800();
        // Many tabs require scrolling
        let (start, end, show_left, show_right) = layout.tab_visible_range(50, 0);
        assert_eq!(start, 0);
        // end should be less than 50 since they don't fit in 1280px
        assert!(end < 50);
        assert!(!show_left);
        assert!(show_right); // more tabs to the right
    }

    #[test]
    fn test_tab_visible_range_scrolled() {
        let layout = layout_1280x800();
        let (start, end, show_left, show_right) = layout.tab_visible_range(50, 10);
        assert_eq!(start, 10);
        assert!(end > start);
        assert!(show_left);
        assert!(show_right);
    }

    #[test]
    fn test_tab_visible_range_near_end() {
        let layout = layout_1280x800();
        let (_start, _end, show_left, show_right) = layout.tab_visible_range(50, 48);
        assert!(show_left);
        assert!(!show_right); // no more tabs to the right
    }

    #[test]
    fn test_scrolled_tab_width() {
        let layout = layout_1280x800();
        let w = layout.scrolled_tab_width(5, true, true);
        assert!(w >= 80.0);
        assert!(w <= 200.0);
    }
}
