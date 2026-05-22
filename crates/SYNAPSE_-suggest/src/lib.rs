mod builtins;
mod history;
mod trie;

use std::collections::HashSet;

pub use trie::Suggester;

pub fn load_suggester() -> Suggester {
    let path_exes = history::load_path_exes();
    let history = history::load_all();
    let mut s = Suggester::new(history, path_exes);

    for &(cmd, count) in builtins::BUILTINS_TIERED {
        s.seed(cmd, count);
    }

    // Load persisted learned commands on top (suggest.state)
    if let Some(state_path) = history::suggest_state_path() {
        if let Ok(data) = std::fs::read_to_string(&state_path) {
            let entries: Vec<(String, u32)> = data
                .lines()
                .filter_map(|line| {
                    let mut parts = line.splitn(2, '|');
                    let count: u32 = parts.next()?.parse().ok()?;
                    let cmd = parts.next()?.trim().to_string();
                    if cmd.is_empty() {
                        None
                    } else {
                        Some((cmd, count))
                    }
                })
                .collect();
            s.load_counts(&entries);
        }
    }

    s
}

/// Persist the current command counts to disk.
/// Returns Ok on success, Err if directory creation or write fails.
pub fn save_suggester(suggester: &Suggester) -> std::io::Result<()> {
    if let Some(state_path) = history::suggest_state_path() {
        if let Some(parent) = state_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let snapshot = suggester.snapshot_counts();
        let mut data = String::with_capacity(snapshot.len() * 64);
        for (cmd, count) in snapshot {
            use std::fmt::Write;
            let _ = writeln!(data, "{}|{}", count, cmd);
        }
        std::fs::write(&state_path, data)?;
    }
    Ok(())
}

/// Scan PATH for available executables. Useful for checking if a
/// command's base name exists on the system.
pub fn load_path_exes() -> HashSet<String> {
    history::load_path_exes()
}
