use std::fs;
use std::path::PathBuf;

use crate::cli::Cli;
use synapse_config::config::Config;

const SCRIPTS: &[(&str, &str)] = &[
    ("synapse-integration.zsh", include_str!("../../../assets/shell/synapse-integration.zsh")),
    ("synapse-integration.bash", include_str!("../../../assets/shell/synapse-integration.bash")),
    ("synapse-integration.fish", include_str!("../../../assets/shell/synapse-integration.fish")),
];

const ZSH_SOURCE: &str = "\n# SYNAPSE_ shell integration\n[ -f ~/.config/SYNAPSE_/shell/synapse-integration.zsh ] && source ~/.config/SYNAPSE_/shell/synapse-integration.zsh\n";

const BASH_SOURCE: &str = "\n# SYNAPSE_ shell integration\n[ -f ~/.config/SYNAPSE_/shell/synapse-integration.bash ] && source ~/.config/SYNAPSE_/shell/synapse-integration.bash\n";

const FISH_SOURCE: &str = "\n# SYNAPSE_ shell integration\ntest -f ~/.config/SYNAPSE_/shell/synapse-integration.fish && source ~/.config/SYNAPSE_/shell/synapse-integration.fish\n";

fn shell_dir() -> Option<PathBuf> {
    Config::config_dir().map(|d| d.join("shell"))
}

fn rc_paths() -> Vec<(PathBuf, &'static str)> {
    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/home"));
    vec![
        (home.join(".zshrc"), ZSH_SOURCE),
        (home.join(".bashrc"), BASH_SOURCE),
        (home.join(".config/fish/config.fish"), FISH_SOURCE),
    ]
}

pub fn install_shell_integration() {
    let dir = match shell_dir() {
        Some(d) => d,
        None => {
            tracing::warn!("Cannot determine config dir for shell integration");
            return;
        }
    };

    if let Err(e) = fs::create_dir_all(&dir) {
        tracing::warn!("Failed to create shell dir {:?}: {}", dir, e);
        return;
    }

    for (name, content) in SCRIPTS {
        let path = dir.join(name);
        match fs::write(&path, *content) {
            Ok(_) => tracing::info!("Installed: {:?}", path),
            Err(e) => tracing::warn!("Failed to write {:?}: {}", path, e),
        }
    }
}

pub fn update_rc_files() {
    for (rc_path, snippet) in rc_paths() {
        if !rc_path.exists() {
            continue;
        }
        match fs::read_to_string(&rc_path) {
            Ok(content) => {
                if content.contains("SYNAPSE_ shell integration") {
                    continue;
                }
                let new_content = content + snippet;
                match fs::write(&rc_path, &new_content) {
                    Ok(_) => tracing::info!("Updated: {:?}", rc_path),
                    Err(e) => tracing::warn!("Failed to update {:?}: {}", rc_path, e),
                }
            }
            Err(e) => tracing::warn!("Failed to read {:?}: {}", rc_path, e),
        }
    }
}

pub fn maybe_install_integration(cli: &Cli, config: &Config) {
    if !cli.setup && !config.shell_integration {
        return;
    }
    install_shell_integration();
    if cli.setup {
        update_rc_files();
        println!("SYNAPSE_ shell integration installed.");
        println!("Close and re-open your terminal, or run:");
        println!("  source ~/.config/SYNAPSE_/shell/synapse-integration.zsh  # for zsh");
        println!("  source ~/.config/SYNAPSE_/shell/synapse-integration.bash # for bash");
        println!("  source ~/.config/SYNAPSE_/shell/synapse-integration.fish # for fish");
        std::process::exit(0);
    }
}
