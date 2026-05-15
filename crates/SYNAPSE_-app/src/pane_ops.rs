use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use alacritty_terminal::event::Event;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config as TermConfig, Term};
use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};
use synapse_ui::pane::{EventProxy, Pane, PaneId};
use synapse_ui::{layout::Layout, splitter::PaneRect, tab_bar::TabBar, SCROLL_BTN_W, TAB_BAR_HEIGHT};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};

const CLOSE_BTN_W: f32 = 16.0;

/// Minimal `Dimensions` impl used to construct and resize a `Term`.
pub(crate) struct TermSize {
    pub(crate) cols: usize,
    pub(crate) rows: usize,
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize {
        // visible rows + scrollback
        self.rows + 10_000
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

pub fn create_pane(id: PaneId, cols: usize, rows: usize) -> Result<Pane, String> {
    create_pane_with_cwd(id, cols, rows, None)
}

pub fn create_pane_with_cwd(
    id: PaneId,
    cols: usize,
    rows: usize,
    cwd: Option<String>,
) -> Result<Pane, String> {
    create_pane_full(id, cols, rows, cwd, None, &[])
}

pub fn create_pane_full(
    id: PaneId,
    cols: usize,
    rows: usize,
    cwd: Option<String>,
    shell_override: Option<&str>,
    shell_args: &[String],
) -> Result<Pane, String> {
    // 1. Open a PTY pair sized to the current pane geometry.
    let pty_system = native_pty_system();
    let pty_pair = pty_system
        .openpty(PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("openpty failed: {e}"))?;

    // 2. Build the shell command, applying overrides and cwd.
    let shell = match shell_override {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string()),
    };
    let mut cmd = CommandBuilder::new(&shell);
    if shell_override.is_some() {
        for arg in shell_args {
            cmd.arg(arg);
        }
    }
    if let Some(cwd) = cwd {
        cmd.cwd(cwd);
    }

    // 3. Spawn the child on the slave side. The child handle is dropped — the
    //    OS reaps the process when the PTY is closed.
    let _child = pty_pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("spawn_command failed: {e}"))?;

    // 4. Pull a reader + writer out of the master, but keep the master itself
    //    so we can resize the PTY later.
    let pty_reader = pty_pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("try_clone_reader failed: {e}"))?;
    let pty_writer = pty_pair
        .master
        .take_writer()
        .map_err(|e| format!("take_writer failed: {e}"))?;
    let pty_master = pty_pair.master;

    // 5. Event channel for alacritty_terminal -> Luna events.
    let (event_tx, event_rx) = mpsc::sync_channel::<Event>(256);
    let proxy = EventProxy::new(event_tx);

    // 6. Construct the terminal.
    let size = TermSize { cols, rows };
    let term = Term::new(TermConfig::default(), &size, proxy);
    let term = Arc::new(Mutex::new(term));

    let dirty = Arc::new(AtomicBool::new(false));

    // 7. Spin up the reader thread: PTY bytes -> VTE parser -> Term handler.
    let term_reader = Arc::clone(&term);
    let dirty_reader = Arc::clone(&dirty);
    std::thread::Builder::new()
        .name(format!("synapse-pty-{}", id.0))
        .spawn(move || {
            let mut reader = pty_reader;
            let mut buf = [0u8; 1024];
            let mut processor: Processor<StdSyncHandler> = Processor::new();
            // Staging queue: accumulate bytes when the term lock is contended,
            // process them on the next iteration that acquires the lock.
            let mut staging: Vec<u8> = Vec::new();
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        staging.extend_from_slice(&buf[..n]);
                        match term_reader.try_lock() {
                            Ok(mut term) => {
                                for &byte in &staging {
                                    processor.advance(&mut *term, byte);
                                }
                                staging.clear();
                            }
                            Err(_) => {
                                // Render thread holds the lock; bytes stay in
                                // staging and are processed on the next read.
                            }
                        }
                        dirty_reader.store(true, Ordering::Release);
                    }
                }
            }
        })
        .map_err(|e| format!("failed to spawn pty reader thread: {e}"))?;

    Ok(Pane::new(
        id,
        term,
        Box::new(pty_writer),
        pty_master,
        event_rx,
        dirty,
        cols,
        rows,
    ))
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

pub fn adjacent_pane(
    layouts: &[(PaneId, PaneRect)],
    from: PaneId,
    direction: &str,
) -> Option<PaneId> {
    let from_rect = layouts.iter().find(|(id, _)| *id == from)?.1;

    match direction {
        "right" => layouts
            .iter()
            .filter(|(id, r)| {
                *id != from
                    && r.x >= from_rect.x + from_rect.w - 1.0
                    && r.y < from_rect.y + from_rect.h
                    && r.y + r.h > from_rect.y
            })
            .min_by(|(_, a), (_, b)| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| *id),
        "left" => layouts
            .iter()
            .filter(|(id, r)| {
                *id != from
                    && r.x + r.w <= from_rect.x + 1.0
                    && r.y < from_rect.y + from_rect.h
                    && r.y + r.h > from_rect.y
            })
            .max_by(|(_, a), (_, b)| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| *id),
        "down" => layouts
            .iter()
            .filter(|(id, r)| {
                *id != from
                    && r.y >= from_rect.y + from_rect.h - 1.0
                    && r.x < from_rect.x + from_rect.w
                    && r.x + r.w > from_rect.x
            })
            .min_by(|(_, a), (_, b)| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| *id),
        "up" => layouts
            .iter()
            .filter(|(id, r)| {
                *id != from
                    && r.y + r.h <= from_rect.y + 1.0
                    && r.x < from_rect.x + from_rect.w
                    && r.x + r.w > from_rect.x
            })
            .max_by(|(_, a), (_, b)| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| *id),
        _ => None,
    }
}

pub fn handle_tab_click(
    tab_bar: &mut TabBar,
    panes: &mut Vec<Pane>,
    x: f64,
    layout: &Layout,
    scroll_offset: &mut usize,
) {
    let tab_count = tab_bar.tabs.len();
    let (start, end, show_left, show_right) = layout.tab_visible_range(tab_count, *scroll_offset);
    let vis_count = end - start;
    let tab_w = layout.scrolled_tab_width(vis_count, show_left, show_right) as f64;
    let x_start = if show_left { SCROLL_BTN_W as f64 } else { 0.0 };

    // < scroll button
    if show_left && x < SCROLL_BTN_W as f64 {
        *scroll_offset = scroll_offset.saturating_sub(1);
        return;
    }

    // + button area
    let plus_x = x_start + vis_count as f64 * tab_w;
    if x >= plus_x && x < plus_x + 32.0 {
        let margin = 4.0;
        let cell_w = 8.4;
        let cell_h = 18.0;
        let pane_height = layout.window_height as f64 - TAB_BAR_HEIGHT as f64 - margin * 2.0;
        let new_cols = ((layout.window_width as f64 - margin * 2.0) / cell_w).max(1.0) as usize;
        let new_rows = (pane_height / cell_h).max(1.0) as usize;
        let (_, pane_id) = tab_bar.new_tab();
        match create_pane(pane_id, new_cols, new_rows) {
            Ok(pane) => panes.push(pane),
            Err(e) => {
                tracing::warn!("Failed to spawn PTY for new tab: {}", e);
                tab_bar.close_tab(tab_bar.active);
                return;
            }
        }
        // Scroll so the new active tab is visible
        let new_count = tab_bar.tabs.len();
        let (new_start, new_end, _, _) = layout.tab_visible_range(new_count, *scroll_offset);
        if tab_bar.active >= new_end {
            let vis = new_end.saturating_sub(new_start).max(1);
            *scroll_offset = tab_bar.active.saturating_sub(vis.saturating_sub(1));
        }
        return;
    }

    // > scroll button
    if show_right && x >= plus_x + 32.0 {
        *scroll_offset = (*scroll_offset + 1).min(tab_count.saturating_sub(1));
        return;
    }

    // Tab area
    let rel_x = x - x_start;
    if rel_x < 0.0 {
        return;
    }
    let vis_idx = (rel_x / tab_w).floor() as usize;
    let actual_idx = start + vis_idx;
    if actual_idx >= end {
        return;
    }

    // × close button zone (rightmost CLOSE_BTN_W px of each tab)
    let tab_start_x = x_start + vis_idx as f64 * tab_w;
    let close_start = tab_start_x + tab_w - CLOSE_BTN_W as f64;
    if x >= close_start && tab_count > 1 {
        if let Some(closed) = tab_bar.close_tab(actual_idx) {
            // Dropping the Pane drops pty_writer + pty_master, which closes the
            // PTY and lets the child process terminate on its next read/write.
            let closed_panes = closed.pane_tree.all_panes();
            panes.retain(|p| !closed_panes.contains(&p.id));
        }
        let new_count = tab_bar.tabs.len();
        if *scroll_offset >= new_count && new_count > 0 {
            *scroll_offset = new_count - 1;
        }
        return;
    }

    tab_bar.activate(actual_idx);
}

pub fn find_hovered_divider(
    dividers: &[synapse_ui::DividerInfo],
    x: f64,
    y: f64,
) -> Option<&synapse_ui::DividerInfo> {
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

        let effective = new_size * self.scale_factor;
        (self.cell_w, self.cell_h) = self.renderer.cell_metrics(effective);

        let pane_area = self.layout.pane_area();
        let pane_rect = synapse_ui::PaneRect {
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
                if let Ok(mut term) = pane.term.lock() {
                    term.resize(TermSize {
                        cols: new_cols,
                        rows: new_rows,
                    });
                }
                if let Ok(master) = pane.pty_master.lock() {
                    let _ = master.resize(PtySize {
                        rows: new_rows as u16,
                        cols: new_cols as u16,
                        pixel_width: 0,
                        pixel_height: 0,
                    });
                }
            }
        }
    }

    pub(crate) fn handle_scale_factor_change(&mut self) {
        let current_size = self.state.font_size;
        self.change_font_size(current_size);
        self.cached_cell_data.clear();
        self.cached_ui_rects.clear();
    }
}
