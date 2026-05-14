use std::sync::Arc;

use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowAttributes},
};

use alacritty_terminal::term::TermMode;
use luna_renderer::renderer::Renderer;
use luna_renderer::ui::UIRect;
use luna_ui::{
    layout::Layout,
    pane::{Pane, PaneId},
    tab_bar::{Tab, TabBar, TabId},
};
use portable_pty::PtySize;

use crate::{pane_ops::{create_pane, TermSize}, state::AppState};

pub type CellData = Vec<(char, f32, f32, f32, [f32; 4], [f32; 4])>;

pub struct App {
    pub window: Arc<Window>,
    pub renderer: Renderer,
    pub layout: Layout,
    pub tab_bar: TabBar,
    pub panes: Vec<Pane>,
    pub clipboard: Option<arboard::Clipboard>,
    pub state: AppState,
    pub cell_w: f32,
    pub cell_h: f32,
    pub margin: f32,
    pub cursor_blink_on: bool,
    pub last_blink: std::time::Instant,
    pub cached_cell_data: CellData,
    pub cached_ui_rects: Vec<UIRect>,
    pub cached_bg_rects: Vec<UIRect>,
    pub cached_blink: bool,
    pub cached_font_size: f32,
    pub cached_active_tab: usize,
    pub frame_count: u64,
    pub fps_last_print: std::time::Instant,
    pub scale_factor: f32,
}

impl App {
    pub fn new() -> Result<(Self, EventLoop<()>), Box<dyn std::error::Error>> {
        let config = luna_config::Config::load();
        let keybinds = luna_config::Keybinds::new();
        let logical_font_size = config.font_size;

        let event_loop = EventLoop::new()?;

        #[allow(deprecated)]
        let window = Arc::new(
            event_loop.create_window(
                WindowAttributes::default()
                    .with_title("Luna")
                    .with_inner_size(winit::dpi::LogicalSize::new(
                        config.window_width as f64,
                        config.window_height as f64,
                    ))
                    .with_resizable(true),
            )?,
        );
        let mut renderer = Renderer::new(window.clone())?;

        let mut layout = Layout::new();
        let size = renderer.size();
        layout.update(size.width as f32, size.height as f32);

        let scale = window.scale_factor() as f32;
        let effective_initial_font_size = logical_font_size * scale;
        let (cell_w, cell_h) = renderer.cell_metrics(effective_initial_font_size);
        let margin = layout.pane_margin();

        let pane_area = layout.pane_area();
        let cols = ((pane_area.2 - margin * 2.0) / cell_w).max(1.0) as usize;
        let rows = ((pane_area.3 - margin * 2.0) / cell_h).max(1.0) as usize;

        let first_tab_id = TabId(0);
        let first_pane_id = PaneId(0);
        let first_pane = create_pane(first_pane_id, cols, rows)?;
        let first_tab = Tab::new(first_tab_id, first_pane_id);
        let tab_bar = TabBar::new(first_tab);

        let panes = vec![first_pane];

        let clipboard = arboard::Clipboard::new().ok();
        let state = AppState::new(config, keybinds, logical_font_size);
        renderer.set_clear_color(state.theme.bg);

        Ok((
            App {
                window,
                renderer,
                layout,
                tab_bar,
                panes,
                clipboard,
                state,
                cell_w,
                cell_h,
                margin,
                cursor_blink_on: true,
                last_blink: std::time::Instant::now(),
                cached_cell_data: Vec::new(),
                cached_ui_rects: Vec::new(),
                cached_bg_rects: Vec::new(),
                cached_blink: true,
                cached_font_size: effective_initial_font_size,
                cached_active_tab: 0,
                frame_count: 0,
                fps_last_print: std::time::Instant::now(),
                scale_factor: scale,
            },
            event_loop,
        ))
    }

    pub fn run(mut self, event_loop: EventLoop<()>) -> Result<(), Box<dyn std::error::Error>> {
        event_loop.set_control_flow(ControlFlow::Poll);
        #[allow(deprecated)]
        event_loop.run(move |event, elwt| match event {
            Event::WindowEvent { event, .. } => self.handle_window_event(event, elwt),
            Event::AboutToWait => self.window.request_redraw(),
            _ => {}
        })?;
        Ok(())
    }

    fn handle_window_event(&mut self, event: WindowEvent, elwt: &ActiveEventLoop) {
        match event {
            WindowEvent::CloseRequested => elwt.exit(),
            WindowEvent::Resized(size) => self.handle_resize(size),
            WindowEvent::ModifiersChanged(m) => self.state.modifiers = m.state(),
            WindowEvent::MouseWheel { delta, .. } => self.handle_scroll(delta),
            WindowEvent::MouseInput {
                state: button_state,
                button,
                ..
            } => self.handle_mouse_button(button_state, button),
            WindowEvent::CursorMoved { position, .. } => self.handle_cursor_moved(position),
            WindowEvent::RedrawRequested => self.render(),
            WindowEvent::KeyboardInput { event, .. } => self.handle_keyboard(event),
            WindowEvent::Focused(focused) => self.handle_focus(focused),
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                mut inner_size_writer,
            } => {
                self.scale_factor = scale_factor as f32;
                let size = self.window.inner_size();
                if let Err(e) = inner_size_writer.request_inner_size(size) {
                    tracing::warn!("ScaleFactorChanged size request failed: {:?}", e);
                }
                self.handle_scale_factor_change();
            }
            _ => {}
        }
    }

    fn handle_focus(&mut self, focused: bool) {
        let active_id = self.tab_bar.active_tab().active_pane;
        if let Some(pane) = self.panes.iter().find(|p| p.id == active_id) {
            let send_focus = pane
                .term
                .lock()
                .map(|t| t.mode().contains(TermMode::FOCUS_IN_OUT))
                .unwrap_or(false);
            if send_focus {
                let seq: &[u8] = if focused { b"\x1b[I" } else { b"\x1b[O" };
                pane.write_to_pty(seq);
            }
        }
    }

    fn handle_resize(&mut self, size: PhysicalSize<u32>) {
        self.scale_factor = self.window.scale_factor() as f32;
        self.renderer.resize(size);
        self.layout.update(size.width as f32, size.height as f32);

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
                if new_cols != pane.cols || new_rows != pane.rows {
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
    }
}
