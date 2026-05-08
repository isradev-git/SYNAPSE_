mod state;
mod input;
mod pane_ops;
mod search;
mod render;
mod mouse;
mod keyboard;
use render::render_frame;
use pane_ops::create_pane;

use std::sync::Arc;

use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowAttributes,
};

use state::AppState;
use luna_ui::{
    layout::Layout,
    pane::{Pane, PaneId},
    tab_bar::{Tab, TabBar, TabId},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = luna_config::Config::load();
    let keybinds = luna_config::Keybinds::new();

    let event_loop = EventLoop::new()?;
    let window_attrs = WindowAttributes::default()
        .with_title("Luna")
        .with_inner_size(winit::dpi::LogicalSize::new(
            config.window_width as f64,
            config.window_height as f64,
        ))
        .with_resizable(true);
    let window = Arc::new(event_loop.create_window(window_attrs)?);
    let mut renderer = luna_renderer::renderer::Renderer::new(window.clone());

    let mut layout = Layout::new();
    let size = renderer.size();
    layout.update(size.width as f32, size.height as f32);

    let initial_font_size = config.font_size;
    let (mut cell_w, mut cell_h) = renderer.cell_metrics(initial_font_size);

    let pane_area = layout.pane_area();
    let margin = layout.pane_margin();
    let cols = ((pane_area.2 - margin * 2.0) / cell_w).max(1.0) as usize;
    let rows = ((pane_area.3 - margin * 2.0) / cell_h).max(1.0) as usize;

    let first_tab_id = TabId(0);
    let first_pane_id = PaneId(0);
    let first_pane = create_pane(first_pane_id, cols, rows);
    let first_tab = Tab::new(first_tab_id, first_pane_id);
    let mut tab_bar = TabBar::new(first_tab);

    let mut panes: Vec<Pane> = Vec::new();
    panes.push(first_pane);

    let mut clipboard = arboard::Clipboard::new().ok();
    let mut state = AppState::new(config, keybinds, initial_font_size);

    event_loop.set_control_flow(ControlFlow::Poll);

    #[allow(deprecated)]
    event_loop.run(move |event, elwt| match event {
        Event::WindowEvent { event, .. } => match event {
            WindowEvent::CloseRequested => elwt.exit(),

            WindowEvent::Resized(size) => {
                renderer.resize(size);
                let window_width = size.width as f32;
                let window_height = size.height as f32;
                layout.update(window_width, window_height);

                let pane_area = layout.pane_area();
                let pane_rect = luna_ui::PaneRect {
                    x: pane_area.0,
                    y: pane_area.1,
                    w: pane_area.2,
                    h: pane_area.3,
                };
                let layouts = tab_bar.active_tab().pane_tree.get_layout(pane_rect);

                for (pane_id, rect) in &layouts {
                    let new_cols = ((rect.w - margin * 2.0) / cell_w).max(1.0) as usize;
                    let new_rows = ((rect.h - margin * 2.0) / cell_h).max(1.0) as usize;
                    if let Some(pane) = panes.iter_mut().find(|p| p.id == *pane_id) {
                        if new_cols != pane.cols || new_rows != pane.rows {
                            pane.cols = new_cols;
                            pane.rows = new_rows;
                            pane.grid.borrow_mut().resize(new_cols, new_rows);
                            let _ = pane.pty_session.pty.resize(new_cols as u16, new_rows as u16);
                        }
                    }
                }
            }

            WindowEvent::ModifiersChanged(mods) => {
                state.modifiers = mods.state();
            }

            WindowEvent::MouseWheel { delta, .. } => {
                mouse::handle_scroll(delta, &mut panes, &tab_bar, cell_h);
            }

            WindowEvent::MouseInput { state: button_state, button, .. } => {
                mouse::handle_mouse_button(button_state, button, &mut state, &mut tab_bar, &mut panes, &layout);
            }

            WindowEvent::CursorMoved { position, .. } => {
                mouse::handle_cursor_moved(
                    position,
                    window.scale_factor(),
                    &mut state,
                    &mut tab_bar,
                    &layout,
                    &window,
                    cell_w,
                    cell_h,
                    margin,
                );
            }

            WindowEvent::RedrawRequested => {
                render_frame(&mut renderer, &layout, &mut tab_bar, &mut panes, &state, cell_w, cell_h);
            }

            WindowEvent::KeyboardInput { event, .. } => {
                keyboard::handle_keyboard(
                    &event,
                    &mut state,
                    &mut tab_bar,
                    &mut panes,
                    &layout,
                    &mut renderer,
                    margin,
                    &mut cell_w,
                    &mut cell_h,
                    &mut clipboard,
                    &window,
                );
            }

            _ => {}
        },
        Event::AboutToWait => {
            window.request_redraw();
        }
        _ => {}
    })?;

    Ok(())
}
