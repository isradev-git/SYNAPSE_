mod app;
mod cli;
mod history;
mod icon;
mod image_protocol;
mod input;
mod ipc;
mod keyboard;
mod mouse;
mod overlay;
mod palette;
mod pane_ops;
#[cfg(target_os = "linux")]
mod platform_linux;
#[cfg(target_os = "macos")]
mod platform_macos;
mod quake;
mod record;
mod render;
mod search;
mod session;
mod setup;
mod shell;
mod sixel;
mod state;
mod update;
mod workspace;

use clap::Parser;

fn main() {
    // Quiet by default — wgpu/naga log a line *per frame* at INFO, which floods
    // the console and can itself cost frames. Keep our own crates at info; honour
    // RUST_LOG when set for debugging.
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new("warn,synapse_=info,synapse_renderer=info")
    });
    tracing_subscriber::fmt()
        .with_env_filter(filter)
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
        write_crash_log(&payload, &location);
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
    setup::ensure_zdotdir_shim();

    if cli.check_update {
        return update::run_check();
    }

    if let Some(path) = &cli.export_icon {
        icon::export_png(path, cli.icon_size)?;
        println!("Wrote {}×{} icon to {path}", cli.icon_size, cli.icon_size);
        return Ok(());
    }

    if let Some(cli::IpcSubcmd::Ipc { command }) = &cli.subcommand {
        return run_ipc_client(command);
    }

    // If another SYNAPSE_ instance is already running, delegate new-window/new-tab to it.
    // Plain `synapse_` launch also opens a new window in the existing instance.
    if try_delegate_to_existing(&cli) {
        return Ok(());
    }

    if config.check_updates_on_startup {
        update::spawn_background_check();
    }

    let (app, event_loop) = app::App::new(cli)?;
    app.run(event_loop)
}

/// Returns true if an existing instance handled the request (this process should exit).
fn try_delegate_to_existing(cli: &cli::Cli) -> bool {
    use ipc::IpcCommandKind;

    // Only delegate on new-window / plain launch (no -e command that implies a fresh window).
    // --quake, --restore, --setup always start their own instance.
    if cli.quake || cli.setup || cli.restore_session.is_some() {
        return false;
    }

    let cmd = IpcCommandKind::NewWindow {
        command: cli.command.clone(),
        cwd: cli.working_directory.clone(),
    };

    match ipc::client_send(&cmd) {
        Ok(_) => {
            tracing::info!("Delegated new window to existing SYNAPSE_ instance.");
            true
        }
        Err(_) => false, // No existing instance — start normally.
    }
}

fn write_crash_log(payload: &str, location: &str) {
    use std::fmt::Write as _;
    use std::time::{SystemTime, UNIX_EPOCH};

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let backtrace = std::backtrace::Backtrace::force_capture();

    let mut report = String::new();
    let _ = writeln!(report, "SYNAPSE_ crash report — {ts}");
    let _ = writeln!(report, "panic: {payload}");
    let _ = writeln!(report, "location: {location}");
    let _ = writeln!(report, "version: {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(report, "\n{backtrace}");

    let cache_dir = session::session_cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp/SYNAPSE_"));
    let _ = std::fs::create_dir_all(&cache_dir);
    let path = cache_dir.join(format!("crash-{ts}.log"));
    if std::fs::write(&path, &report).is_ok() {
        eprintln!("Crash log: {}", path.display());
    }
}

fn run_ipc_client(command: &cli::IpcCommand) -> Result<(), Box<dyn std::error::Error>> {
    use ipc::{IpcCommandKind, IpcResponse};

    let kind = match command {
        cli::IpcCommand::List => IpcCommandKind::List,
        cli::IpcCommand::Send { text } => IpcCommandKind::Send { text: text.clone() },
        cli::IpcCommand::NewTab { cmd, cwd } => IpcCommandKind::NewTab {
            command: cmd.clone(),
            cwd: cwd.clone(),
        },
        cli::IpcCommand::Kill { pane_id } => IpcCommandKind::Kill { pane_id: *pane_id },
        cli::IpcCommand::NewWindow { cmd, cwd } => IpcCommandKind::NewWindow {
            command: cmd.clone(),
            cwd: cwd.clone(),
        },
    };

    let response = ipc::client_send(&kind).map_err(|e| e.as_str().to_owned())?;

    match &response {
        IpcResponse::Panes { panes, .. } => {
            if panes.is_empty() {
                println!("No panes");
            } else {
                println!("{:<6} {:<8} {:<40} TITLE", "ID", "TAB", "CWD");
                println!("{}", "-".repeat(64));
                for p in panes {
                    let active_marker = if p.active { "*" } else { " " };
                    println!(
                        "{:<6} {:<8} {:<40} {}{}",
                        p.id, p.tab_index, p.cwd, active_marker, p.title
                    );
                }
            }
        }
        IpcResponse::Ok { .. } => println!("ok"),
        IpcResponse::Text { output, .. } => print!("{output}"),
        IpcResponse::Err { error, .. } => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }

    Ok(())
}
