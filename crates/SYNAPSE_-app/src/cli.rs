use clap::{Parser, Subcommand};

#[derive(Parser, Default)]
#[command(
    name = "synapse_",
    version,
    about = "A GPU-accelerated terminal emulator"
)]
pub struct Cli {
    #[arg(short = 'e')]
    pub command: Option<String>,

    #[arg(short = 'd', long)]
    pub working_directory: Option<String>,

    #[arg(long)]
    pub new_tab: Option<String>,

    #[arg(long)]
    pub hold: bool,

    #[arg(long = "restore")]
    pub restore_session: Option<String>,

    /// Open a new window. Delegates to a running instance via IPC if one exists.
    #[arg(long)]
    pub new_window: bool,

    #[arg(long)]
    pub quake: bool,

    #[arg(long)]
    pub setup: bool,

    /// Check for a newer SYNAPSE_ release on GitHub and exit
    #[arg(long = "check-update")]
    pub check_update: bool,

    /// Render the app icon to a PNG at the given path and exit (for packaging).
    #[arg(long = "export-icon", value_name = "PATH")]
    pub export_icon: Option<String>,

    /// Icon size in pixels for --export-icon (default 256).
    #[arg(long = "icon-size", default_value_t = 256)]
    pub icon_size: u32,

    #[command(subcommand)]
    pub subcommand: Option<IpcSubcmd>,
}

/// IPC subcommands for controlling a running SYNAPSE_ instance.
#[derive(Subcommand)]
pub enum IpcSubcmd {
    /// Control a running SYNAPSE_ instance via IPC
    Ipc {
        #[command(subcommand)]
        command: IpcCommand,
    },
}

#[derive(Subcommand)]
pub enum IpcCommand {
    /// List all panes in the running instance
    List,

    /// Send text (stdin) to the active pane
    Send {
        /// Text to send; use $'\n' for newline in shells
        text: String,
    },

    /// Open a new tab (optionally running a command)
    #[command(name = "new-tab")]
    NewTab {
        /// Command to run in the new tab
        #[arg(long)]
        cmd: Option<String>,

        /// Working directory for the new tab
        #[arg(long)]
        cwd: Option<String>,
    },

    /// Kill a pane by ID (defaults to the active pane)
    Kill {
        /// Pane ID to kill (see `list` for IDs)
        pane_id: Option<u32>,
    },

    /// Open a new window in the running instance
    #[command(name = "new-window")]
    NewWindow {
        /// Command to run in the new window
        #[arg(long)]
        cmd: Option<String>,

        /// Working directory for the new window
        #[arg(long)]
        cwd: Option<String>,
    },
}
