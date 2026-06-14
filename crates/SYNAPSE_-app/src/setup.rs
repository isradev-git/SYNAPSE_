use std::fs;
use std::io::{self, Write as IoWrite};
use std::path::PathBuf;
use std::process::Command;

use crate::cli::Cli;
use synapse_config::config::Config;

// ── ZDOTDIR shim ─────────────────────────────────────────────────────

/// Auto-invoked at startup (not just on `--setup`).
///
/// Writes a ZDOTDIR shim directory so that zsh sessions inside SYNAPSE_
/// automatically load the user's real `.zshrc` + `synapse-env.zsh` without
/// requiring any manual user action.
pub fn ensure_zdotdir_shim() {
    let cfg_dir = match Config::config_dir() {
        Some(d) => d,
        None => return,
    };

    let shell_dir = cfg_dir.join("shell");
    let env_script = shell_dir.join("synapse-env.zsh");
    let zdotdir = cfg_dir.join("zdotdir");

    // Sync embedded assets to disk (write_if_changed avoids inode bumps).
    let _ = fs::create_dir_all(&shell_dir);
    write_if_changed(
        env_script.clone(),
        include_str!("../../../assets/shell/synapse-env.zsh"),
    );
    write_if_changed(
        cfg_dir.join("fastfetch.jsonc"),
        include_str!("../../../assets/configs/fastfetch.jsonc"),
    );

    if let Err(e) = fs::create_dir_all(&zdotdir) {
        tracing::warn!("zdotdir: failed to create {:?}: {e}", zdotdir);
        return;
    }

    let env_path = env_script.to_string_lossy();

    // .zshenv — forward to user's real .zshenv (PATH, etc.)
    write_if_changed(
        zdotdir.join(".zshenv"),
        "# SYNAPSE_ zshenv shim — do not edit\n\
         [[ -f \"$HOME/.zshenv\" ]] && source \"$HOME/.zshenv\"\n",
    );

    // .zprofile — forward to user's real .zprofile (login shells)
    write_if_changed(
        zdotdir.join(".zprofile"),
        "# SYNAPSE_ zprofile shim — do not edit\n\
         [[ -f \"$HOME/.zprofile\" ]] && source \"$HOME/.zprofile\"\n",
    );

    // .zshrc — load user's .zshrc then layer SYNAPSE_ env
    let zshrc_content = format!(
        "# SYNAPSE_ zshrc shim — do not edit\n\
         unset ZDOTDIR\n\
         [[ -f \"$HOME/.zshrc\" ]] && source \"$HOME/.zshrc\"\n\
         [[ -f \"{env_path}\" ]] && source \"{env_path}\"\n"
    );
    write_if_changed(zdotdir.join(".zshrc"), &zshrc_content);
}

fn write_if_changed(path: PathBuf, content: &str) {
    let existing = fs::read_to_string(&path).unwrap_or_default();
    if existing != content {
        if let Err(e) = fs::write(&path, content) {
            tracing::warn!("zdotdir: failed to write {:?}: {e}", path);
        }
    }
}

// ── Shell integration scripts ─────────────────────────────────────────

const SCRIPTS: &[(&str, &str)] = &[
    (
        "synapse-integration.zsh",
        include_str!("../../../assets/shell/synapse-integration.zsh"),
    ),
    (
        "synapse-integration.bash",
        include_str!("../../../assets/shell/synapse-integration.bash"),
    ),
    (
        "synapse-integration.fish",
        include_str!("../../../assets/shell/synapse-integration.fish"),
    ),
    (
        "synapse-env.zsh",
        include_str!("../../../assets/shell/synapse-env.zsh"),
    ),
];

const ZSH_SOURCE: &str = "\
\n# SYNAPSE_ shell integration\
\n[ -f ~/.config/SYNAPSE_/shell/synapse-integration.zsh ] && source ~/.config/SYNAPSE_/shell/synapse-integration.zsh\
\n[ -f ~/.config/SYNAPSE_/shell/synapse-env.zsh ] && source ~/.config/SYNAPSE_/shell/synapse-env.zsh\n";

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
        install_recommended_tools();
        println!("\nSYNAPSE_ shell integration installed.");
        println!("Restart your terminal or run:");
        println!("  source ~/.config/SYNAPSE_/shell/synapse-env.zsh");
        std::process::exit(0);
    }
}

// ── Recommended tool installer ────────────────────────────────────────

const R: &str = "\x1b[38;2;255;0;60m";
const M: &str = "\x1b[38;2;208;216;228m";
const D: &str = "\x1b[38;2;60;69;96m";
const G: &str = "\x1b[38;2;77;230;107m";
const B: &str = "\x1b[1m";
const RST: &str = "\x1b[0m";

struct Tool {
    name: &'static str,
    description: &'static str,
    /// Shell command whose presence means "already installed"
    binary: &'static str,
    /// Extra file paths to check (for zsh plugins that have no binary)
    check_paths: &'static [&'static str],
    brew: &'static str,
    apt: &'static str,
    pacman: &'static str,
    dnf: &'static str,
}

const TOOLS: &[Tool] = &[
    Tool {
        name: "fastfetch",
        description: "system info display — the neofetch successor",
        binary: "fastfetch",
        check_paths: &[],
        brew: "fastfetch",
        apt: "fastfetch",
        pacman: "fastfetch",
        dnf: "fastfetch",
    },
    Tool {
        name: "zsh-autosuggestions",
        description: "fish-like command suggestions as you type",
        binary: "",
        check_paths: &[
            "/opt/homebrew/share/zsh-autosuggestions/zsh-autosuggestions.zsh",
            "/usr/local/share/zsh-autosuggestions/zsh-autosuggestions.zsh",
            "/usr/share/zsh-autosuggestions/zsh-autosuggestions.zsh",
        ],
        brew: "zsh-autosuggestions",
        apt: "zsh-autosuggestions",
        pacman: "zsh-autosuggestions",
        dnf: "zsh-autosuggestions",
    },
    Tool {
        name: "zsh-syntax-highlighting",
        description: "real-time syntax coloring in the prompt",
        binary: "",
        check_paths: &[
            "/opt/homebrew/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh",
            "/usr/local/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh",
            "/usr/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh",
        ],
        brew: "zsh-syntax-highlighting",
        apt: "zsh-syntax-highlighting",
        pacman: "zsh-syntax-highlighting",
        dnf: "zsh-syntax-highlighting",
    },
    Tool {
        name: "fzf",
        description: "fuzzy finder — supercharged Ctrl+R history + file search",
        binary: "fzf",
        check_paths: &[],
        brew: "fzf",
        apt: "fzf",
        pacman: "fzf",
        dnf: "fzf",
    },
    Tool {
        name: "eza",
        description: "modern ls — colors, icons, git integration",
        binary: "eza",
        check_paths: &[],
        brew: "eza",
        apt: "eza",
        pacman: "eza",
        dnf: "eza",
    },
    Tool {
        name: "bat",
        description: "cat with syntax highlighting and line numbers",
        binary: "bat",
        check_paths: &[],
        brew: "bat",
        apt: "bat",
        pacman: "bat",
        dnf: "bat",
    },
];

#[derive(PartialEq)]
enum Pm {
    Brew,
    Apt,
    Pacman,
    Dnf,
    None,
}

impl Pm {
    fn detect() -> Self {
        if bin_exists("brew") {
            Pm::Brew
        } else if bin_exists("apt") {
            Pm::Apt
        } else if bin_exists("pacman") {
            Pm::Pacman
        } else if bin_exists("dnf") {
            Pm::Dnf
        } else {
            Pm::None
        }
    }

    fn label(&self) -> &str {
        match self {
            Pm::Brew => "Homebrew (brew)",
            Pm::Apt => "apt",
            Pm::Pacman => "pacman",
            Pm::Dnf => "dnf",
            Pm::None => "none",
        }
    }

    fn pkg_for<'a>(&self, tool: &'a Tool) -> &'a str {
        match self {
            Pm::Brew => tool.brew,
            Pm::Apt => tool.apt,
            Pm::Pacman => tool.pacman,
            Pm::Dnf => tool.dnf,
            Pm::None => "",
        }
    }
}

fn bin_exists(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn tool_installed(tool: &Tool) -> bool {
    if !tool.binary.is_empty() && bin_exists(tool.binary) {
        return true;
    }
    tool.check_paths.iter().any(|p| std::path::Path::new(p).exists())
}

fn run_install(pm: &Pm, pkgs: &[&str]) {
    if pkgs.is_empty() {
        return;
    }
    let pkg_list = pkgs.join(" ");
    println!("\n{D}  ▸ Installing: {M}{pkg_list}{RST}\n");

    let status = match pm {
        Pm::Brew => Command::new("brew")
            .arg("install")
            .args(pkgs)
            .status(),
        Pm::Apt => Command::new("sudo")
            .args(["apt", "install", "-y"])
            .args(pkgs)
            .status(),
        Pm::Pacman => Command::new("sudo")
            .args(["pacman", "-S", "--noconfirm"])
            .args(pkgs)
            .status(),
        Pm::Dnf => Command::new("sudo")
            .args(["dnf", "install", "-y"])
            .args(pkgs)
            .status(),
        Pm::None => return,
    };

    match status {
        Ok(s) if s.success() => println!("\n{G}  ✓ Installed successfully.{RST}"),
        Ok(s) => println!("\n{R}  ✗ Install exited with code {}.{RST}", s.code().unwrap_or(-1)),
        Err(e) => println!("\n{R}  ✗ Failed to run installer: {e}{RST}"),
    }
}

fn prompt_yn(question: &str) -> bool {
    print!("  {D}{question}{RST} [{R}Y{RST}/n] ");
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).ok();
    buf.trim().to_lowercase() != "n"
}

pub fn install_recommended_tools() {
    let sep = format!("{D}  {}{RST}", "─".repeat(54));
    println!("\n{R}{B}  ╔══ SYNAPSE_ — Recommended Tools ══╗{RST}");
    println!("{D}  Enhance your terminal experience with these tools.{RST}\n");

    let pm = Pm::detect();

    // Partition: already installed vs missing
    let (installed, missing): (Vec<_>, Vec<_>) = TOOLS.iter().partition(|t| tool_installed(t));

    if !installed.is_empty() {
        println!("{G}  Already installed:{RST}");
        for t in &installed {
            println!("    {G}✓{RST} {M}{}{RST}", t.name);
        }
        println!();
    }

    if missing.is_empty() {
        println!("{G}  All recommended tools are already installed.{RST}\n");
        println!("{sep}");
        return;
    }

    println!("{M}{B}  Missing tools:{RST}");
    for t in &missing {
        println!("    {R}·{RST}  {M}{:<28}{RST}{D}{}{RST}", t.name, t.description);
    }
    println!();

    if pm == Pm::None {
        println!("{D}  No supported package manager found (brew / apt / pacman / dnf).");
        println!("  Install the tools above manually to enable these features.{RST}\n");
        println!("{sep}");
        return;
    }

    println!("  Package manager: {M}{}{RST}\n", pm.label());

    if !prompt_yn("Install all missing tools?") {
        // Per-tool prompts
        let mut to_install: Vec<&str> = Vec::new();
        for t in &missing {
            if prompt_yn(&format!("Install {}?", t.name)) {
                to_install.push(pm.pkg_for(t));
            }
        }
        run_install(&pm, &to_install);
    } else {
        let pkgs: Vec<&str> = missing.iter().map(|t| pm.pkg_for(t)).collect();
        run_install(&pm, &pkgs);
    }

    println!("\n{sep}\n");
}
