mod app;
mod image_protocol;
mod input;
mod keyboard;
mod mouse;
mod pane_ops;
mod render;
mod search;
mod state;

fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .try_init()
        .ok();

    std::panic::set_hook(Box::new(|info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        tracing::error!("PANIC: {} at {}", payload, location);
        eprintln!("SYNAPSE_ crashed:\n{}\n\nLocation: {}", payload, location);
    }));

    if let Err(e) = try_main() {
        let msg = format!("SYNAPSE_ failed to start:\n\n{}", e);
        tracing::error!("STARTUP ERROR: {}", e);
        eprintln!("{}", msg);
        std::process::exit(1);
    }
}

fn try_main() -> Result<(), Box<dyn std::error::Error>> {
    let (app, event_loop) = app::App::new()?;
    app.run(event_loop)
}
