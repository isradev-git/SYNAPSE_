use luna_ui::{
    layout::Layout,
    pane::{Pane, PaneId},
    splitter::PaneRect,
    tab_bar::TabBar,
    TAB_BAR_HEIGHT,
};

const CLOSE_BTN_W: f32 = 16.0;

pub fn create_pane(id: PaneId, cols: usize, rows: usize) -> Pane {
    create_pane_with_cwd(id, cols, rows, None)
}

pub fn create_pane_with_cwd(id: PaneId, cols: usize, rows: usize, cwd: Option<String>) -> Pane {
    create_pane_full(id, cols, rows, cwd, None, &[])
}

pub fn create_pane_full(
    id: PaneId,
    cols: usize,
    rows: usize,
    cwd: Option<String>,
    shell_override: Option<&str>,
    shell_args: &[String],
) -> Pane {
    use luna_terminal::grid::Grid;
    use std::cell::RefCell;
    use std::rc::Rc;

    let grid = Rc::new(RefCell::new(Grid::new(cols, rows)));
    let mut shell_config = luna_terminal::shell::detect_shell();
    if let Some(cwd) = cwd {
        shell_config.cwd = Some(cwd);
    }
    if let Some(prog) = shell_override {
        if !prog.is_empty() {
            shell_config.program = prog.to_string();
            shell_config.args = shell_args.to_vec();
        }
    }
    let pty_handle =
        luna_terminal::pty::PtyHandle::spawn(cols as u16, rows as u16, &shell_config)
            .expect("Failed to spawn PTY");
    let pty_session = luna_terminal::pty::PtyHandle::start_reader(pty_handle);
    Pane::new(id, pty_session, grid, cols, rows)
}

pub fn find_pane(panes: &[Pane], id: PaneId) -> Option<&Pane> {
    panes.iter().find(|p| p.id == id)
}

pub fn active_pane_mut<'a>(panes: &'a mut [Pane], tab_bar: &TabBar) -> &'a mut Pane {
    let active_id = tab_bar.active_tab().active_pane;
    panes
        .iter_mut()
        .find(|p| p.id == active_id)
        .expect("Active pane not found")
}

pub fn adjacent_pane(layouts: &[(PaneId, PaneRect)], from: PaneId, direction: &str) -> Option<PaneId> {
    let from_rect = layouts.iter().find(|(id, _)| *id == from)?.1;

    match direction {
        "right" => layouts
            .iter()
            .filter(|(id, r)| *id != from
                && r.x >= from_rect.x + from_rect.w - 1.0
                && r.y < from_rect.y + from_rect.h
                && r.y + r.h > from_rect.y)
            .min_by(|(_, a), (_, b)| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| *id),
        "left" => layouts
            .iter()
            .filter(|(id, r)| *id != from
                && r.x + r.w <= from_rect.x + 1.0
                && r.y < from_rect.y + from_rect.h
                && r.y + r.h > from_rect.y)
            .max_by(|(_, a), (_, b)| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| *id),
        "down" => layouts
            .iter()
            .filter(|(id, r)| *id != from
                && r.y >= from_rect.y + from_rect.h - 1.0
                && r.x < from_rect.x + from_rect.w
                && r.x + r.w > from_rect.x)
            .min_by(|(_, a), (_, b)| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| *id),
        "up" => layouts
            .iter()
            .filter(|(id, r)| *id != from
                && r.y + r.h <= from_rect.y + 1.0
                && r.x < from_rect.x + from_rect.w
                && r.x + r.w > from_rect.x)
            .max_by(|(_, a), (_, b)| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| *id),
        _ => None,
    }
}

pub fn handle_tab_click(tab_bar: &mut TabBar, panes: &mut Vec<Pane>, x: f64, layout: &Layout) {
    let tab_count = tab_bar.tabs.len();
    let tab_w = layout.tab_width(tab_count) as f64;
    let close_w = CLOSE_BTN_W as f64;

    // + button area
    let plus_x = layout.tab_x(tab_count, tab_count) as f64;
    if x >= plus_x && x < plus_x + 32.0 {
        let margin = 4.0;
        let cell_w = 8.4;
        let cell_h = 18.0;
        let pane_height = layout.window_height as f64 - TAB_BAR_HEIGHT as f64 - margin * 2.0;
        let new_cols = ((layout.window_width as f64 - margin * 2.0) / cell_w).max(1.0) as usize;
        let new_rows = (pane_height / cell_h).max(1.0) as usize;
        let (_, pane_id) = tab_bar.new_tab();
        panes.push(create_pane(pane_id, new_cols, new_rows));
        return;
    }

    let clicked = (x / tab_w) as usize;
    if clicked >= tab_count {
        return;
    }

    // × close button zone (rightmost CLOSE_BTN_W px of each tab)
    let tab_start = layout.tab_x(clicked, tab_count) as f64;
    let close_start = tab_start + tab_w - close_w;
    if x >= close_start && tab_count > 1 {
        if let Some(closed) = tab_bar.close_tab(clicked) {
            let closed_panes = closed.pane_tree.all_panes();
            for pane in panes.iter_mut() {
                if closed_panes.contains(&pane.id) {
                    let _ = pane.pty_session.pty.kill();
                }
            }
            panes.retain(|p| !closed_panes.contains(&p.id));
        }
        return;
    }

    tab_bar.activate(clicked);
}

pub fn find_hovered_divider<'a>(
    dividers: &'a [luna_ui::DividerInfo],
    x: f64,
    y: f64,
) -> Option<&'a luna_ui::DividerInfo> {
    dividers.iter().find(|info| {
        let h = info.hitbox;
        x >= h.x as f64 && x <= (h.x + h.w) as f64 && y >= h.y as f64 && y <= (h.y + h.h) as f64
    })
}

use crate::app::App;

impl App {
    pub(crate) fn change_font_size(&mut self, new_size: f32) {
        self.state.font_size = new_size;
        self.state.config.font_size = new_size;
        let _ = self.state.config.save();

        (self.cell_w, self.cell_h) = self.renderer.cell_metrics(new_size);

        let pane_area = self.layout.pane_area();
        let pane_rect = luna_ui::PaneRect {
            x: pane_area.0,
            y: pane_area.1,
            w: pane_area.2,
            h: pane_area.3,
        };
        let layouts = self.tab_bar.active_tab().pane_tree.get_layout(pane_rect);
        let (margin, cell_w, cell_h) = (self.margin, self.cell_w, self.cell_h);

        for (pane_id, rect) in &layouts {
            let new_cols = ((rect.w - margin * 2.0) / cell_w).max(1.0) as usize;
            let new_rows = ((rect.h - margin * 2.0) / cell_h).max(1.0) as usize;
            if let Some(pane) = self.panes.iter_mut().find(|p| p.id == *pane_id) {
                pane.cols = new_cols;
                pane.rows = new_rows;
                pane.grid.borrow_mut().resize(new_cols, new_rows);
                let _ = pane.pty_session.pty.resize(new_cols as u16, new_rows as u16);
            }
        }
    }
}
