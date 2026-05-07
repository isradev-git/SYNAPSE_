use std::sync::Arc;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowAttributes,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let window_attrs = WindowAttributes::default()
        .with_title("Luna")
        .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0))
        .with_resizable(true);
    let window = Arc::new(event_loop.create_window(window_attrs)?);
    let mut renderer = luna_renderer::renderer::Renderer::new(window.clone());

    event_loop.set_control_flow(ControlFlow::Wait);

    #[allow(deprecated)]
    event_loop.run(move |event, elwt| match event {
        Event::WindowEvent { event, .. } => match event {
            WindowEvent::CloseRequested => elwt.exit(),
            WindowEvent::Resized(size) => renderer.resize(size),
            WindowEvent::RedrawRequested => {
                renderer.draw_text(
                    "Hello, Luna!",
                    100.0,
                    200.0,
                    48.0,
                    [1.0, 1.0, 1.0, 1.0],
                    [0.0, 0.0, 0.0, 0.0],
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
