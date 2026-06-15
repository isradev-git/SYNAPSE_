use std::collections::HashMap;
use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, PhysicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::{Window, WindowAttributes, WindowId, WindowLevel},
};

use alacritty_terminal::term::TermMode;
use portable_pty::PtySize;
use synapse_renderer::renderer::Renderer;
use synapse_renderer::ui::UIRect;
use synapse_renderer::underline::UnderlineInstance;
use synapse_ui::{
    layout::Layout,
    pane::{Pane, PaneId},
    tab_bar::{Tab, TabBar, TabId},
};

use crate::{
    cli::Cli,
    image_protocol::ImageStore,
    overlay::OverlayEvent,
    pane_ops::{create_pane_full, write_to_panes, TermSize},
    quake::QuakeMode,
    session,
    state::AppState,
    workspace::{Workspace, WorkspaceManager},
};

pub type CellData = Vec<(char, f32, f32, f32, [f32; 4], [f32; 4])>;

/// Cross-thread events sent to the winit event loop (e.g. from the IPC thread).
#[derive(Debug)]
pub enum AppEvent {
    /// Open a new OS window, optionally running a command in a given directory.
    NewWindow {
        command: Option<String>,
        cwd: Option<String>,
    },
}

fn is_wayland_backend() -> bool {
    if let Ok(backend) = std::env::var("WINIT_UNIX_BACKEND") {
        return backend.to_lowercase() == "wayland";
    }
    std::env::var("WAYLAND_DISPLAY").is_ok()
}

pub(crate) fn adjusted_bg(bg: [f32; 4], opacity: f32) -> [f32; 4] {
    let mut c = bg;
    c[3] *= opacity;
    c
}

pub struct AppCore {
    pub window: Arc<Window>,
    pub renderer: Renderer,
    pub layout: Layout,
    pub workspaces: WorkspaceManager,
    pub clipboard: Option<arboard::Clipboard>,
    pub state: AppState,
    pub cell_w: f32,
    pub cell_h: f32,
    pub margin: f32,
    pub cached_cell_data: CellData,
    pub cached_ui_rects: Vec<UIRect>,
    pub cached_bg_rects: Vec<UIRect>,
    pub cached_underline_instances: Vec<UnderlineInstance>,
    pub cached_blink: f32,
    pub cached_font_size: f32,
    pub cached_active_tab: usize,
    pub cached_cursor_rects_start: usize,
    pub cached_cursor_pixel: Option<(f32, f32)>,
    pub frame_count: u64,
    pub fps_last_print: std::time::Instant,
    pub start_time: std::time::Instant,
    pub scale_factor: f32,
    pub image_store: ImageStore,
    pub splash_start: Option<std::time::Instant>,
    pub cached_url_spans: Vec<crate::state::UrlSpan>,
    pub last_session_save: std::time::Instant,
    pub session_save_interval: u64,
    pub quake: Option<QuakeMode>,
    pub overlay_rx: Option<std::sync::mpsc::Receiver<OverlayEvent>>,
}

pub struct App {
    /// One AppCore per OS window, keyed by WindowId.
    pub cores: HashMap<WindowId, AppCore>,
    cli: Cli,
    proxy: EventLoopProxy<AppEvent>,
    /// IPC receiver owned at App level so it outlives individual windows.
    ipc_rx: Option<std::sync::mpsc::Receiver<crate::ipc::IpcRequest>>,
    /// Tracks which window currently has OS focus (for IPC routing).
    focused_window: Option<WindowId>,
}

impl App {
    pub fn new(cli: Cli) -> Result<(Self, EventLoop<AppEvent>), Box<dyn std::error::Error>> {
        let event_loop = EventLoop::<AppEvent>::with_user_event().build()?;
        let proxy = event_loop.create_proxy();

        // Start the IPC server once per process.
        let ipc_rx = {
            let (tx, rx) = std::sync::mpsc::channel();
            match crate::ipc::start_ipc_server(tx) {
                Ok(()) => Some(rx),
                Err(e) => {
                    tracing::warn!("IPC server failed to start: {e}");
                    None
                }
            }
        };

        Ok((
            App {
                cores: HashMap::new(),
                cli,
                proxy,
                ipc_rx,
                focused_window: None,
            },
            event_loop,
        ))
    }

    pub fn run(
        mut self,
        event_loop: EventLoop<AppEvent>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        event_loop.run_app(&mut self)?;
        Ok(())
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.cores.is_empty() {
            let cli = std::mem::take(&mut self.cli);
            let core = AppCore::initialize(event_loop, cli);
            let id = core.window.id();
            self.cores.insert(id, core);
        }
        event_loop.set_control_flow(ControlFlow::Poll);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                if let Some(core) = self.cores.get_mut(&window_id) {
                    core.save_current_session();
                    core.stop_recording_if_active();
                    if let Some(path) =
                        session::session_cache_dir().map(|d| d.join("glyph_atlas.bin"))
                    {
                        core.renderer.save_warm_cache(&path);
                    }
                }
                self.cores.remove(&window_id);
                if self.focused_window == Some(window_id) {
                    self.focused_window = None;
                }
                if self.cores.is_empty() {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(core) = self.cores.get_mut(&window_id) {
                    core.handle_resize(size);
                }
            }
            WindowEvent::ModifiersChanged(m) => {
                if let Some(core) = self.cores.get_mut(&window_id) {
                    core.state.modifiers = m.state();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(core) = self.cores.get_mut(&window_id) {
                    core.handle_scroll(delta);
                }
            }
            WindowEvent::MouseInput {
                state: button_state,
                button,
                ..
            } => {
                if let Some(core) = self.cores.get_mut(&window_id) {
                    core.handle_mouse_button(button_state, button);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(core) = self.cores.get_mut(&window_id) {
                    core.handle_cursor_moved(position);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(core) = self.cores.get_mut(&window_id) {
                    core.render();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if self.handle_quake_key(window_id, &event) {
                    return;
                }
                if let Some(core) = self.cores.get_mut(&window_id) {
                    core.handle_keyboard(event);
                }
            }
            WindowEvent::Focused(focused) => {
                if focused {
                    self.focused_window = Some(window_id);
                } else if self.focused_window == Some(window_id) {
                    self.focused_window = None;
                }
                if let Some(core) = self.cores.get_mut(&window_id) {
                    core.handle_focus(focused);
                    if !focused {
                        core.quake_focus_lost();
                    }
                }
            }
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                mut inner_size_writer,
            } => {
                if let Some(core) = self.cores.get_mut(&window_id) {
                    core.scale_factor = scale_factor as f32;
                    let size = core.window.inner_size();
                    if let Err(e) = inner_size_writer.request_inner_size(size) {
                        tracing::warn!("ScaleFactorChanged size request failed: {:?}", e);
                    }
                    core.handle_scale_factor_change();
                }
            }
            WindowEvent::DroppedFile(path) => {
                if let Some(core) = self.cores.get_mut(&window_id) {
                    match std::fs::canonicalize(&path) {
                        Ok(abs) => {
                            let mut s = abs.to_string_lossy().into_owned();
                            s.push(' ');
                            write_to_panes(
                                core.workspaces.active_panes(),
                                core.workspaces.active_tab_bar(),
                                core.state.broadcasting,
                                s.as_bytes(),
                            );
                        }
                        Err(e) => {
                            tracing::warn!("DroppedFile canonicalize failed for {:?}: {}", path, e);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.poll_ipc();
        for core in self.cores.values_mut() {
            core.maybe_autosave_session();
            core.quake_animate();
            core.window.request_redraw();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::NewWindow { command, cwd } => {
                let new_cli = Cli {
                    command,
                    working_directory: cwd,
                    ..Default::default()
                };
                let core = AppCore::initialize(event_loop, new_cli);
                let id = core.window.id();
                self.cores.insert(id, core);
                tracing::info!("Opened new window {id:?} via IPC");
            }
        }
    }
}

impl App {
    fn handle_quake_key(&mut self, window_id: WindowId, event: &winit::event::KeyEvent) -> bool {
        let core = match self.cores.get_mut(&window_id) {
            Some(c) => c,
            None => return false,
        };
        let quake = match core.quake.as_mut() {
            Some(q) => q,
            None => return false,
        };
        if event.state.is_pressed()
            && core.state.modifiers.control_key()
            && !core.state.modifiers.alt_key()
            && !core.state.modifiers.shift_key()
            && is_space_key(event)
        {
            quake.toggle();
            return true;
        }
        false
    }

    /// Returns the focused window's core, or the first available one.
    fn active_core_mut(&mut self) -> Option<&mut AppCore> {
        if let Some(id) = self.focused_window {
            if self.cores.contains_key(&id) {
                return self.cores.get_mut(&id);
            }
        }
        self.cores.values_mut().next()
    }

    fn poll_ipc(&mut self) {
        let mut requests = Vec::new();
        if let Some(rx) = &self.ipc_rx {
            while let Ok(req) = rx.try_recv() {
                requests.push(req);
            }
        }
        for req in requests {
            let resp = self.handle_ipc_request(req.command);
            let _ = req.response_tx.send(resp);
        }
    }

    fn handle_ipc_request(&mut self, cmd: crate::ipc::IpcCommandKind) -> crate::ipc::IpcResponse {
        use crate::ipc::{IpcCommandKind, IpcResponse};

        match cmd {
            IpcCommandKind::NewWindow { command, cwd } => {
                let _ = self.proxy.send_event(AppEvent::NewWindow { command, cwd });
                IpcResponse::ok()
            }
            other => {
                if let Some(core) = self.active_core_mut() {
                    core.handle_ipc_command(other)
                } else {
                    IpcResponse::err("no window available")
                }
            }
        }
    }
}

impl AppCore {
    fn initialize(event_loop: &ActiveEventLoop, cli: Cli) -> Self {
        let config = synapse_config::Config::load();
        let keybinds = synapse_config::Keybinds::new();
        let logical_font_size = config.font_size;

        let quake_active = cli.quake || config.quake.enabled;

        let is_wayland = is_wayland_backend();

        let (window, quake) = if quake_active {
            let monitor = event_loop
                .available_monitors()
                .next()
                .expect("No monitor available");
            let screen_size: winit::dpi::LogicalSize<f64> =
                monitor.size().to_logical(monitor.scale_factor());
            let wh = screen_size.height * config.quake.height_percent as f64;
            let ww = screen_size.width;

            let mut attrs = WindowAttributes::default()
                .with_title("SYNAPSE_")
                .with_window_icon(crate::icon::window_icon())
                .with_inner_size(winit::dpi::LogicalSize::new(ww, wh))
                .with_resizable(false)
                .with_decorations(false)
                .with_window_level(WindowLevel::AlwaysOnTop);
            if config.window_opacity < 1.0 || config.window_blur {
                attrs = attrs.with_transparent(true);
            }
            let window = Arc::new(
                event_loop
                    .create_window(attrs)
                    .expect("Failed to create quake window"),
            );
            window.set_outer_position(LogicalPosition::new(0.0, 0.0));

            let quake = QuakeMode::new(&config.quake, wh, screen_size.height, screen_size.width);
            (window, Some(quake))
        } else {
            let mut attrs = WindowAttributes::default()
                .with_title("SYNAPSE_")
                .with_window_icon(crate::icon::window_icon())
                .with_inner_size(winit::dpi::LogicalSize::new(
                    config.window_width as f64,
                    config.window_height as f64,
                ))
                .with_resizable(true);
            if is_wayland {
                attrs = attrs.with_decorations(false);
            }
            if config.window_opacity < 1.0 || config.window_blur {
                attrs = attrs.with_transparent(true);
            }
            let window = Arc::new(
                event_loop
                    .create_window(attrs)
                    .expect("Failed to create window"),
            );

            (window, None)
        };
        let mut renderer =
            Renderer::new(window.clone(), &config.font_family).expect("Renderer init failed");

        if let Some(path) = session::session_cache_dir().map(|d| d.join("glyph_atlas.bin")) {
            renderer.load_warm_cache(&path);
        }

        let mut layout = Layout::new();
        layout.wayland_decorated = is_wayland;
        let size = renderer.size();
        layout.update(size.width as f32, size.height as f32);

        let scale = window.scale_factor() as f32;
        layout.tab_bar_height = synapse_ui::TAB_BAR_HEIGHT * scale;
        // Sidebar width: use config value or default, scaled by DPI
        let sidebar_logical = if config.sidebar_width > 0.0 {
            config.sidebar_width
        } else {
            synapse_ui::SIDEBAR_DEFAULT_WIDTH
        };
        layout.sidebar_width = sidebar_logical * scale;
        layout.status_bar_visible = config.status_bar;
        let effective_initial_font_size = logical_font_size * scale;
        let (cell_w, cell_h) = renderer.cell_metrics(effective_initial_font_size);
        let margin = layout.pane_margin();

        let pane_area = layout.pane_area();
        let cols = ((pane_area.2 - margin * 2.0) / cell_w).max(1.0) as usize;
        let rows = ((pane_area.3 - margin * 2.0) / cell_h).max(1.0) as usize;

        let first_tab_id = TabId(0);
        let first_pane_id = PaneId(0);

        let cwd = cli.working_directory.map(|d| {
            std::fs::canonicalize(&d)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|e| {
                    tracing::warn!("Failed to canonicalize -d '{}': {e}", d);
                    d
                })
        });

        let (shell_override, shell_args_vec) = if let Some(ref cmd) = cli.command {
            let shell = crate::shell::default_shell();
            let wrapped_cmd = if cli.hold {
                format!(
                    "{}; echo; echo '\\x1b[2m[Process exited - press Enter to close]\\x1b[0m'; read",
                    cmd
                )
            } else {
                cmd.clone()
            };
            (Some(shell), vec!["-c".to_string(), wrapped_cmd])
        } else {
            (None, Vec::new())
        };

        let (restored_tb, restored_panes) = if let Some(ref name) = cli.restore_session {
            let path = if name.is_empty() {
                session::default_session_path()
            } else {
                session::named_session_path(name)
            };
            try_restore_session(&path, cols, rows, config.scrollback_lines)
        } else if config.restore_session {
            try_restore_session(
                &session::default_session_path(),
                cols,
                rows,
                config.scrollback_lines,
            )
        } else {
            (None, None)
        };

        let mut workspaces = match (restored_tb, restored_panes) {
            (Some(tab_bars), Some(ws_panes_vec)) => {
                let first_ws = tab_bars
                    .keys()
                    .next()
                    .cloned()
                    .unwrap_or_else(|| "default".into());
                let mut panes_by_ws: HashMap<String, Vec<Pane>> = HashMap::new();
                for (ws_name, panes) in ws_panes_vec {
                    panes_by_ws.insert(ws_name, panes);
                }
                let mut mgr = WorkspaceManager::new(
                    "default",
                    TabBar::new(Tab::new(TabId(0), PaneId(0))),
                    vec![],
                );
                mgr.workspaces.clear();
                for (ws_name, tab_bar) in tab_bars {
                    let tab_bar = tab_bar.from_saved();
                    let panes = panes_by_ws.remove(&ws_name).unwrap_or_default();
                    mgr.workspaces.insert(
                        ws_name.clone(),
                        Workspace {
                            name: ws_name.clone(),
                            tab_bar,
                            panes,
                            pane_cell_caches: HashMap::new(),
                        },
                    );
                }
                if mgr.workspaces.contains_key(&first_ws) {
                    mgr.active = first_ws;
                } else {
                    mgr.active = mgr
                        .workspaces
                        .keys()
                        .next()
                        .cloned()
                        .unwrap_or_else(|| "default".into());
                }
                mgr
            }
            _ => {
                let first_pane = create_pane_full(
                    first_pane_id,
                    cols,
                    rows,
                    cwd,
                    shell_override.as_deref(),
                    &shell_args_vec,
                    config.scrollback_lines,
                )
                .expect("Pane creation failed");
                let first_tab = Tab::new(first_tab_id, first_pane_id);
                let tab_bar = TabBar::new(first_tab);
                WorkspaceManager::new("default", tab_bar, vec![first_pane])
            }
        };

        if let Some(ref new_tab_cmd) = cli.new_tab {
            let shell = crate::shell::default_shell();
            let ws = workspaces.active_ws_mut();
            let (_tab_id, pane_id) = ws.tab_bar.new_tab();
            match create_pane_full(
                pane_id,
                cols,
                rows,
                None,
                Some(&shell),
                &["-c".to_string(), new_tab_cmd.clone()],
                config.scrollback_lines,
            ) {
                Ok(pane) => {
                    ws.panes.push(pane);
                    ws.tab_bar.active = 0;
                }
                Err(e) => {
                    tracing::error!("Failed to create --new-tab pane: {e}");
                }
            }
        }

        let clipboard = arboard::Clipboard::new().ok();
        let session_save_interval = config.session_save_interval_secs;

        // Initialize recording shared state (must happen before pane creation so PTY threads have it)
        let _ = crate::record::RECORDING.set(crate::record::RecordingShared::new());

        let mut state = AppState::new(config, keybinds, logical_font_size);
        state.active_workspace = workspaces.active.clone();

        let mut bound_keys: Vec<String> = Vec::new();
        for (i, plugin) in state.config.plugins.iter().enumerate() {
            if let Some(ref kb_str) = plugin.keybind {
                if let Some(combo) = synapse_config::parse_keybind_string(kb_str) {
                    let combo_str = synapse_config::combo_to_string(&combo);
                    if bound_keys.contains(&combo_str) {
                        tracing::warn!(
                            "Duplicate plugin keybind: '{}' for plugin '{}'",
                            kb_str,
                            plugin.name
                        );
                    } else {
                        bound_keys.push(combo_str);
                    }
                    state.keybinds.push(synapse_config::KeyBindEntry {
                        key: combo.key,
                        ctrl: combo.ctrl,
                        shift: combo.shift,
                        alt: combo.alt,
                        action: format!("plugin_{}", i),
                    });
                } else {
                    tracing::warn!(
                        "Invalid plugin keybind '{}' for plugin '{}'",
                        kb_str,
                        plugin.name
                    );
                }
            }
        }
        state.palette.set_plugins(state.config.plugins.clone());
        state
            .palette
            .set_ssh_profiles(state.config.ssh_profiles.clone());
        state
            .palette
            .set_tab_profiles(state.config.tab_profiles.clone());

        // When blur is on but opacity is fully opaque, default to 0.85 so vibrancy shows through.
        let effective_opacity = if state.config.window_blur && state.config.window_opacity >= 1.0 {
            0.85_f32
        } else {
            state.config.window_opacity
        };
        renderer.set_clear_color(adjusted_bg(state.theme.bg, effective_opacity));

        #[cfg(target_os = "macos")]
        if state.config.window_blur {
            crate::platform_macos::apply_window_blur(&window);
        }
        #[cfg(target_os = "linux")]
        if state.config.window_blur {
            crate::platform_linux::apply_window_blur(&window);
        }
        renderer.set_effects_config(state.config.effects.clone());
        renderer.set_effects_enabled(state.config.effects.enabled);

        if let Some(ref bg_path) = state.config.background_image {
            if let Some((w, h, rgba)) = crate::image_protocol::load_background_image(
                bg_path,
                state.config.background_opacity,
            ) {
                renderer.upload_image(crate::image_protocol::BG_IMAGE_ID, &rgba, w, h);
            } else {
                tracing::warn!("Failed to load background image: {}", bg_path);
            }
        }

        let mut image_store = ImageStore::new();
        if let Some(ref bg_path) = state.config.background_image {
            if let Some((w, h, rgba)) = crate::image_protocol::load_background_image(
                bg_path,
                state.config.background_opacity,
            ) {
                image_store.store_bg_image(crate::image_protocol::BG_IMAGE_ID, w, h, rgba);
            }
        }

        let reduce_motion = state.config.reduce_motion;

        AppCore {
            window,
            renderer,
            layout,
            workspaces,
            clipboard,
            state,
            cell_w,
            cell_h,
            margin,
            cached_cell_data: Vec::new(),
            cached_ui_rects: Vec::new(),
            cached_bg_rects: Vec::new(),
            cached_underline_instances: Vec::new(),
            cached_blink: 1.0,
            cached_font_size: effective_initial_font_size,
            cached_active_tab: 0,
            cached_cursor_rects_start: 0,
            cached_cursor_pixel: None,
            frame_count: 0,
            fps_last_print: std::time::Instant::now(),
            start_time: std::time::Instant::now(),
            scale_factor: scale,
            image_store,
            splash_start: if reduce_motion {
                None
            } else {
                Some(std::time::Instant::now())
            },
            cached_url_spans: Vec::new(),
            last_session_save: std::time::Instant::now(),
            session_save_interval,
            quake,
            overlay_rx: None,
        }
    }

    pub(crate) fn handle_resize(&mut self, size: PhysicalSize<u32>) {
        self.scale_factor = self.window.scale_factor() as f32;
        self.renderer.resize(size);
        self.layout.update(size.width as f32, size.height as f32);

        let pane_area = self.layout.pane_area();
        let pane_rect = synapse_ui::PaneRect {
            x: pane_area.0,
            y: pane_area.1,
            w: pane_area.2,
            h: pane_area.3,
        };
        let (margin, cell_w, cell_h) = (self.margin, self.cell_w, self.cell_h);
        let scrollback_lines = self.state.config.scrollback_lines;

        // Resize panes across ALL tabs — background tabs keep correct PTY dimensions.
        let all_layouts: Vec<(PaneId, synapse_ui::PaneRect)> = self
            .workspaces
            .active_tab_bar()
            .tabs
            .iter()
            .flat_map(|tab| tab.pane_tree.get_layout(pane_rect))
            .collect();

        for (pane_id, rect) in &all_layouts {
            let new_cols = ((rect.w - margin * 2.0) / cell_w).max(1.0) as usize;
            let new_rows = ((rect.h - margin * 2.0) / cell_h).max(1.0) as usize;
            if let Some(pane) = self
                .workspaces
                .active_panes_mut()
                .iter_mut()
                .find(|p| p.id == *pane_id)
            {
                if new_cols != pane.cols || new_rows != pane.rows {
                    pane.cols = new_cols;
                    pane.rows = new_rows;
                    if let Ok(mut term) = pane.term.lock() {
                        term.resize(TermSize {
                            cols: new_cols,
                            rows: new_rows,
                            scrollback_lines,
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
        self.cached_cell_data.clear();
        self.cached_ui_rects.clear();
        self.cached_bg_rects.clear();
        self.cached_underline_instances.clear();
        self.workspaces.active_cell_caches_mut().clear();
    }

    fn handle_focus(&mut self, focused: bool) {
        self.state.window_focused = focused;
        let active_id = self.workspaces.active_tab_bar().active_tab().active_pane;
        if let Some(pane) = self
            .workspaces
            .active_panes()
            .iter()
            .find(|p| p.id == active_id)
        {
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

    pub fn save_current_session(&self) {
        if let Some(ref path) = session::default_session_path() {
            if let Err(e) = session::save_session(&self.workspaces, path) {
                tracing::warn!("Failed to save session: {e}");
            }
        }
        if self.state.config.persistent_history {
            self.state.command_history.save();
        }
    }

    pub fn maybe_autosave_session(&mut self) {
        if self.session_save_interval == 0 {
            return;
        }
        let elapsed = self.last_session_save.elapsed().as_secs();
        if elapsed >= self.session_save_interval {
            self.save_current_session();
            self.last_session_save = std::time::Instant::now();
        }
    }

    pub fn quake_focus_lost(&mut self) {
        if let Some(ref mut quake) = self.quake {
            if quake.hide_on_focus_lost {
                quake.hide();
            }
        }
    }

    pub fn quake_animate(&mut self) {
        if let Some(ref mut quake) = self.quake {
            quake.animate(self.state.config.reduce_motion);
            quake.apply_position(&self.window);
        }
    }

    /// Handle window-specific IPC commands (List, Send, NewTab, Kill).
    /// NewWindow is handled at the App level before reaching here.
    pub fn handle_ipc_command(
        &mut self,
        cmd: crate::ipc::IpcCommandKind,
    ) -> crate::ipc::IpcResponse {
        use crate::ipc::{IpcCommandKind, IpcResponse, PaneInfo};

        match cmd {
            IpcCommandKind::List => {
                let ws = self.workspaces.active_ws();
                let active_tab = ws.tab_bar.active_tab();
                let active_pane_id = active_tab.active_pane;
                let panes: Vec<PaneInfo> = ws
                    .panes
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let tab_index = ws
                            .tab_bar
                            .tabs
                            .iter()
                            .position(|t| t.pane_tree.all_panes().contains(&p.id))
                            .unwrap_or(i);
                        PaneInfo {
                            id: p.id.0 as u32,
                            title: p.title(),
                            cwd: p.cwd(),
                            active: p.id == active_pane_id,
                            tab_index,
                        }
                    })
                    .collect();
                IpcResponse::Panes { ok: true, panes }
            }

            IpcCommandKind::Send { text } => {
                let ws = self.workspaces.active_ws();
                let active_pane_id = ws.tab_bar.active_tab().active_pane;
                if let Some(pane) = ws.panes.iter().find(|p| p.id == active_pane_id) {
                    match pane.pty_writer.lock() {
                        Ok(mut w) => {
                            use std::io::Write as _;
                            if w.write_all(text.as_bytes()).is_err() {
                                return IpcResponse::err("write to PTY failed");
                            }
                        }
                        Err(_) => return IpcResponse::err("PTY writer lock poisoned"),
                    }
                    IpcResponse::ok()
                } else {
                    IpcResponse::err("no active pane")
                }
            }

            IpcCommandKind::NewTab { command, cwd } => {
                let cols = {
                    let ws = self.workspaces.active_ws();
                    let tab = ws.tab_bar.active_tab();
                    ws.panes
                        .iter()
                        .find(|p| p.id == tab.active_pane)
                        .map_or(80, |p| p.cols)
                };
                let rows = {
                    let ws = self.workspaces.active_ws();
                    let tab = ws.tab_bar.active_tab();
                    ws.panes
                        .iter()
                        .find(|p| p.id == tab.active_pane)
                        .map_or(24, |p| p.rows)
                };
                let scrollback = self.state.config.scrollback_lines;
                let shell = crate::shell::default_shell();
                let (shell_path, shell_args): (String, Vec<String>) = match command {
                    Some(cmd) => (shell.clone(), vec!["-c".to_string(), cmd]),
                    None => (shell, vec![]),
                };
                let ws = self.workspaces.active_ws_mut();
                let (_tab_id, pane_id) = ws.tab_bar.new_tab();
                match create_pane_full(
                    pane_id,
                    cols,
                    rows,
                    cwd,
                    Some(&shell_path),
                    &shell_args,
                    scrollback,
                ) {
                    Ok(pane) => {
                        ws.panes.push(pane);
                        self.window.request_redraw();
                        IpcResponse::ok()
                    }
                    Err(e) => IpcResponse::err(format!("pane creation: {e}")),
                }
            }

            IpcCommandKind::Kill { pane_id } => {
                let (tab_bar, panes, _caches) = self.workspaces.active_split_mut();
                let target_id = match pane_id {
                    Some(id) => synapse_ui::pane::PaneId(id as u64),
                    None => tab_bar.active_tab().active_pane,
                };
                if let Some(removed) = tab_bar.active_tab_mut().pane_tree.close(target_id) {
                    panes.retain(|p| p.id != removed);
                    self.window.request_redraw();
                    IpcResponse::ok()
                } else {
                    IpcResponse::err(format!("pane {} not found", target_id.0))
                }
            }

            // Handled at App level via EventLoopProxy; should not reach here.
            IpcCommandKind::NewWindow { .. } => IpcResponse::err("internal routing error"),
        }
    }

    pub fn stop_recording_if_active(&mut self) {
        if !self.state.recording {
            return;
        }
        if let Some(shared) = crate::record::RECORDING.get() {
            if let Some((duration, events)) = shared.stop() {
                self.state.recording = false;
                let shell = crate::shell::default_shell();
                let path = match &self.state.config.recording_path {
                    Some(p) => std::path::PathBuf::from(p),
                    None => {
                        let ts = crate::record::format_timestamp();
                        crate::record::default_recording_dir().join(format!("synapse_{}.cast", ts))
                    }
                };
                let (cols, rows) = {
                    let ws = self.workspaces.active_ws();
                    let tab = ws.tab_bar.active_tab();
                    let pane_id = tab.active_pane;
                    ws.panes
                        .iter()
                        .find(|p| p.id == pane_id)
                        .map_or((80, 24), |p| (p.cols, p.rows))
                };
                match crate::record::write_cast_file(
                    &path,
                    cols as u16,
                    rows as u16,
                    duration,
                    &shell,
                    "xterm-256color",
                    &events,
                ) {
                    Ok(()) => {
                        tracing::info!("Recording saved to {}", path.display());
                        crate::render::send_desktop_notification(
                            "SYNAPSE_",
                            &format!("Recording saved to {}", path.display()),
                        );
                    }
                    Err(e) => {
                        tracing::error!("Failed to save recording: {e}");
                    }
                }
            }
        }
    }
}

fn is_space_key(event: &winit::event::KeyEvent) -> bool {
    use winit::keyboard::{Key, NamedKey};
    matches!(event.logical_key, Key::Named(NamedKey::Space))
}

#[allow(clippy::type_complexity)]
fn try_restore_session(
    path: &Option<std::path::PathBuf>,
    cols: usize,
    rows: usize,
    scrollback_lines: usize,
) -> (
    Option<HashMap<String, TabBar>>,
    Option<Vec<(String, Vec<Pane>)>>,
) {
    let path = match path {
        Some(p) if session::session_exists(p) => p,
        _ => return (None, None),
    };

    match session::load_session(path) {
        Ok(tab_bars) => {
            let mut ws_panes: Vec<(String, Vec<Pane>)> = Vec::new();
            for (ws_name, tab_bar) in &tab_bars {
                let mut panes = Vec::new();
                for tab in &tab_bar.tabs {
                    for pane_id in tab.pane_tree.all_panes() {
                        match create_pane_full(
                            pane_id,
                            cols,
                            rows,
                            if tab.cwd.is_empty() {
                                None
                            } else {
                                Some(tab.cwd.clone())
                            },
                            None,
                            &[],
                            scrollback_lines,
                        ) {
                            Ok(pane) => panes.push(pane),
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to restore pane {} in ws '{}': {e}",
                                    pane_id.0,
                                    ws_name
                                );
                            }
                        }
                    }
                }
                ws_panes.push((ws_name.clone(), panes));
            }
            (Some(tab_bars), Some(ws_panes))
        }
        Err(e) => {
            tracing::warn!("Session restore failed ({}): {e}", path.display());
            (None, None)
        }
    }
}
