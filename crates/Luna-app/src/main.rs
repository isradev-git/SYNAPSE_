mod app;
mod input;
mod keyboard;
mod mouse;
mod pane_ops;
mod render;
mod search;
mod state;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (app, event_loop) = app::App::new()?;
    app.run(event_loop)
}
