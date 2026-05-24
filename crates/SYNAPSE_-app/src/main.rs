mod app;
mod cli;
mod history;
mod image_protocol;
mod input;
mod keyboard;
mod mouse;
mod overlay;
mod palette;
mod pane_ops;
mod quake;
mod record;
mod render;
mod search;
mod session;
mod setup;
mod sixel;
mod state;
mod workspace;

use clap::Parser;

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
    let cli = cli::Cli::parse();
    let config = synapse_config::config::Config::load();
    setup::maybe_install_integration(&cli, &config);
    let (app, event_loop) = app::App::new(cli)?;
    app.run(event_loop)
}
