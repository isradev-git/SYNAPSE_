use std::collections::HashSet;
use std::path::PathBuf;

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

    // Deduplicate preserving most recent occurrence (reverse, dedup, re-reverse).
    all_lines.reverse();
    let mut seen = HashSet::new();
    let mut result: Vec<String> = all_lines
        .into_iter()
        .filter(|line| {
            let trimmed = line.trim().to_string();
            !trimmed.is_empty() && trimmed.len() >= 2 && seen.insert(trimmed)
        })
        .collect();
    result.reverse();
    result
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
        // Extended format: `: timestamp:elapsed;command`
        let cmd = if let Some(rest) = trimmed.strip_prefix(": ") {
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
