use clap::Parser;

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
}
