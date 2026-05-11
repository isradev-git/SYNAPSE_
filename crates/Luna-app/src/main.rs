mod app;
mod input;
mod keyboard;
mod mouse;
mod pane_ops;
mod render;
mod search;
mod state;

fn main() {
    if let Err(e) = try_main() {
        let msg = format!("{}", e);
        eprintln!("Luna error: {}", msg);
        #[cfg(target_os = "windows")]
        {
            // On Windows, double-clicking the .exe opens a console that closes instantly.
            // Pause so the user can read the error before the window disappears.
            eprintln!("\nPress Enter to exit...");
            let mut buf = String::new();
            let _ = std::io::stdin().read_line(&mut buf);
        }
        std::process::exit(1);
    }
}

fn try_main() -> Result<(), Box<dyn std::error::Error>> {
    let (app, event_loop) = app::App::new()?;
    app.run(event_loop)
}
