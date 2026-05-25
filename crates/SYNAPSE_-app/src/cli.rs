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

    #[arg(long)]
    pub quake: bool,

    #[arg(long)]
    pub setup: bool,

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
}
