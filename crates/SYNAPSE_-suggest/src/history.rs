use std::collections::HashSet;
use std::path::PathBuf;

/// Load all commands with recency weighting.
/// Each occurrence of a command adds to its frequency, with more recent
/// occurrences contributing slightly more weight. The vector contains
/// duplicates: a command appearing 10 times in history appears 10 times
/// in the output. Recent entries appear later in the vector.
pub fn load_all() -> Vec<String> {
    let mut all_lines: Vec<String> = Vec::new();

    if let Some(home) = home_dir() {
        let zsh = home.join(".zsh_history");
        if zsh.exists() {
            all_lines.extend(load_zsh(&zsh));
        }
        let bash = home.join(".bash_history");
        if bash.exists() {
            all_lines.extend(load_bash(&bash));
        }
        let fish = home.join(".local/share/fish/fish_history");
        if fish.exists() {
            all_lines.extend(load_fish(&fish));
        }
    }

    // Filter short/empty, trim. Keep duplicates (frequency == count).
    all_lines
        .into_iter()
        .map(|l| l.trim().to_string())
        .filter(|l| l.len() >= 2)
        .collect()
}

pub fn load_path_exes() -> HashSet<String> {
    let mut exes = HashSet::new();
    let path = std::env::var("PATH").unwrap_or_default();
    let mut seen = HashSet::new();
    for dir_str in path.split(':') {
        if dir_str.is_empty() {
            continue;
        }
        let dir = std::path::Path::new(dir_str);
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        if let Some(name) = entry.file_name().to_str() {
                            if !seen.insert(name.to_string()) {
                                continue;
                            }
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                if metadata.permissions().mode() & 0o111 != 0 {
                                    exes.insert(name.to_string());
                                }
                            }
                            #[cfg(not(unix))]
                            {
                                let _full_path = dir.join(name);
                                exes.insert(name.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    exes
}

pub fn suggest_state_path() -> Option<PathBuf> {
    home_dir().map(|h| {
        let base = if cfg!(target_os = "linux") {
            std::env::var("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| h.join(".config"))
        } else if cfg!(target_os = "macos") {
            h.join("Library").join("Application Support")
        } else {
            h.join(".config")
        };
        base.join("SYNAPSE_").join("suggest.state")
    })
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn load_zsh(path: &PathBuf) -> Vec<String> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&bytes);
    let mut result = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Extended format variants:
        //   `: 1234567890:0;command`  (with space after colon)
        //   `:1234567890:0;command`   (no space after colon)
        //   `:1234567890:0;`          (empty command)
        let cmd = if let Some(rest) = trimmed.strip_prefix(':') {
            let rest = rest.trim_start(); // skip optional space
            if let Some(semi) = rest.find(';') {
                rest[semi + 1..].trim()
            } else {
                trimmed
            }
        } else {
            trimmed
        };
        if !cmd.is_empty() {
            result.push(cmd.to_string());
        }
    }
    result
}

fn load_bash(path: &PathBuf) -> Vec<String> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&bytes);
    text.lines()
        .filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())
        .map(|l| l.trim().to_string())
        .collect()
}

fn load_fish(path: &PathBuf) -> Vec<String> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&bytes);
    text.lines()
        .filter_map(|l| l.trim().strip_prefix("- cmd: "))
        .filter(|cmd| !cmd.trim().is_empty())
        .map(|cmd| cmd.trim().to_string())
        .collect()
}
