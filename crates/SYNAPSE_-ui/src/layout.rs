use crate::theme;

pub const SCROLL_BTN_W: f32 = 20.0;

pub struct Layout {
    pub window_width: f32,
    pub window_height: f32,
    pub tab_bar_height: f32,
    pub sidebar_width: f32,
    pub status_bar_visible: bool,
    pub wayland_decorated: bool,
}

impl Layout {
    pub fn new() -> Self {
        Self {
            window_width: 1280.0,
            window_height: 800.0,
            tab_bar_height: theme::TAB_BAR_HEIGHT,
            sidebar_width: theme::SIDEBAR_DEFAULT_WIDTH,
            status_bar_visible: true,
            wayland_decorated: false,
        }
    }

    pub fn title_bar_height(&self) -> f32 {
        if self.wayland_decorated { 28.0 } else { 0.0 }
    }

    pub fn update(&mut self, width: f32, height: f32) {
        self.window_width = width;
        self.window_height = height;
    }

    pub fn pane_area(&self) -> (f32, f32, f32, f32) {
        let title_h = self.title_bar_height();
        let x = self.sidebar_width;
        let y = title_h;
        let w = (self.window_width - self.sidebar_width).max(0.0);
        let bar_h = if self.status_bar_visible {
            theme::STATUS_BAR_HEIGHT
        } else {
            0.0
        };
        let h = (self.window_height - title_h - bar_h).max(0.0);
        (x, y, w, h)
    }

    pub fn pane_margin(&self) -> f32 {
        4.0
    }

    pub fn sidebar_visible_height(&self) -> f32 {
        let header = theme::SIDEBAR_HEADER_HEIGHT;
        let bottom = theme::SIDEBAR_TAB_HEIGHT;
        let title_h = self.title_bar_height();
        (self.window_height - title_h - header - bottom).max(0.0)
    }

    /// Returns (first_visible_idx, end_exclusive, show_up_btn, show_down_btn).
    pub fn tab_visible_range(&self, tab_count: usize, offset: usize) -> (usize, usize, bool, bool) {
        if tab_count == 0 {
            return (0, 0, false, false);
        }
        let available = self.sidebar_visible_height();
        let tab_h = theme::SIDEBAR_TAB_HEIGHT;
        let max_vis = (available / tab_h).floor().max(1.0) as usize;
        if tab_count <= max_vis {
            return (0, tab_count, false, false);
        }
        let show_up = offset > 0;
        let up_h = if show_up {
            theme::SIDEBAR_SCROLL_BTN_H
        } else {
            0.0
        };
        let avail = available - up_h - theme::SIDEBAR_SCROLL_BTN_H;
        let vis = (avail / tab_h).floor().max(1.0) as usize;
        let start = offset.min(tab_count.saturating_sub(1));
        let end = (start + vis).min(tab_count);
        (start, end, show_up, end < tab_count)
    }

    pub fn tab_y(&self, vis_index: usize, show_up_btn: bool) -> f32 {
        let header = theme::SIDEBAR_HEADER_HEIGHT + self.title_bar_height();
        let up_btn_h = if show_up_btn {
            theme::SIDEBAR_SCROLL_BTN_H
        } else {
            0.0
        };
        header + up_btn_h + vis_index as f32 * theme::SIDEBAR_TAB_HEIGHT
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
            sidebar_width: 180.0,
            status_bar_visible: false,
            wayland_decorated: false,
        }
    }

    #[test]
    fn test_pane_area_with_sidebar() {
        let layout = layout_1280x800();
        let (x, y, w, h) = layout.pane_area();
        assert_eq!(x, 180.0);
        assert_eq!(y, 0.0);
        assert_eq!(w, 1100.0);
        assert_eq!(h, 800.0);
    }

    #[test]
    fn test_pane_area_zero_sidebar_width() {
        let mut layout = layout_1280x800();
        layout.sidebar_width = 0.0;
        let (x, _y, w, _h) = layout.pane_area();
        assert_eq!(x, 0.0);
        assert_eq!(w, 1280.0);
    }

    #[test]
    fn test_pane_area_window_smaller_than_sidebar() {
        let mut layout = layout_1280x800();
        layout.window_width = 100.0;
        layout.sidebar_width = 200.0;
        let (_x, _y, w, _h) = layout.pane_area();
        assert_eq!(w, 0.0);
    }

    #[test]
    fn test_pane_margin() {
        let layout = layout_1280x800();
        assert_eq!(layout.pane_margin(), 4.0);
    }

    #[test]
    fn test_sidebar_visible_height() {
        let layout = layout_1280x800();
        let vh = layout.sidebar_visible_height();
        // 800 - 48 (header) - 44 (bottom btn) = 708
        assert!((vh - 708.0).abs() < 0.01);
    }

    #[test]
    fn test_tab_visible_range_all_fit() {
        let layout = layout_1280x800();
        // 716 / 36 = 19 tabs fit without scroll
        let (start, end, up, down) = layout.tab_visible_range(10, 0);
        assert_eq!(start, 0);
        assert_eq!(end, 10);
        assert!(!up);
        assert!(!down);
    }

    #[test]
    fn test_tab_visible_range_overflow() {
        let layout = layout_1280x800();
        // 40 tabs > visible space
        let (start, end, up, down) = layout.tab_visible_range(40, 0);
        assert_eq!(start, 0);
        assert!(end < 40);
        assert!(!up);
        assert!(down);
    }

    #[test]
    fn test_tab_visible_range_scrolled() {
        let layout = layout_1280x800();
        let (start, end, up, down) = layout.tab_visible_range(40, 15);
        assert_eq!(start, 15);
        assert!(end > start);
        assert!(up);
        assert!(down);
    }

    #[test]
    fn test_tab_visible_range_near_end() {
        let layout = layout_1280x800();
        let (_start, _end, up, down) = layout.tab_visible_range(40, 39);
        assert!(up);
        assert!(!down);
    }

    #[test]
    fn test_tab_visible_range_zero_tabs() {
        let layout = layout_1280x800();
        let (start, end, up, down) = layout.tab_visible_range(0, 0);
        assert_eq!(start, 0);
        assert_eq!(end, 0);
        assert!(!up);
        assert!(!down);
    }

    #[test]
    fn test_tab_y_no_scroll_btn() {
        let layout = layout_1280x800();
        // header = 48, no up btn, tab 0
        let y = layout.tab_y(0, false);
        assert_eq!(y, 48.0);
        let y1 = layout.tab_y(1, false);
        assert_eq!(y1, 92.0); // 48 + 44
    }

    #[test]
    fn test_tab_y_with_scroll_btn() {
        let layout = layout_1280x800();
        let y0 = layout.tab_y(0, true);
        assert_eq!(y0, 68.0); // 48 + 20 (scroll btn)
        let y1 = layout.tab_y(1, true);
        assert_eq!(y1, 112.0);
    }

    #[test]
    fn test_update_sets_dimensions() {
        let mut layout = Layout::new();
        layout.update(1920.0, 1080.0);
        assert_eq!(layout.window_width, 1920.0);
        assert_eq!(layout.window_height, 1080.0);
    }

    #[test]
    fn test_tab_visible_range_one_tab() {
        let layout = layout_1280x800();
        let (start, end, up, down) = layout.tab_visible_range(1, 0);
        assert_eq!(start, 0);
        assert_eq!(end, 1);
        assert!(!up);
        assert!(!down);
    }

    #[test]
    fn test_pane_area_with_status_bar() {
        let layout = Layout {
            window_width: 1280.0,
            window_height: 800.0,
            tab_bar_height: 32.0,
            sidebar_width: 180.0,
            status_bar_visible: true,
            wayland_decorated: false,
        };
        let (x, y, w, h) = layout.pane_area();
        assert_eq!(x, 180.0);
        assert_eq!(y, 0.0);
        assert_eq!(w, 1100.0);
        assert!((h - (800.0 - theme::STATUS_BAR_HEIGHT)).abs() < 0.01);
    }

    #[test]
    fn test_pane_area_status_bar_hidden() {
        let layout = Layout {
            window_width: 1280.0,
            window_height: 800.0,
            tab_bar_height: 32.0,
            sidebar_width: 180.0,
            status_bar_visible: false,
            wayland_decorated: false,
        };
        let (_, _, _, h) = layout.pane_area();
        assert_eq!(h, 800.0);
    }
}
