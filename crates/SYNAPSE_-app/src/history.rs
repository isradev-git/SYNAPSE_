use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEntry {
    pub cmd: String,
    pub cwd: String,
    pub timestamp: u64,
    pub exit_code: Option<i32>,
}

#[derive(Debug)]
pub struct CommandHistory {
    entries: VecDeque<CommandEntry>,
    max_entries: usize,
    path: Option<PathBuf>,
}

impl CommandHistory {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries,
            path: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_path(path: Option<PathBuf>, max_entries: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries,
            path,
        }
    }

    pub fn set_path(&mut self, path: PathBuf) {
        self.path = Some(path);
    }

    pub fn add(&mut self, cmd: String, cwd: String, exit_code: Option<i32>) {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Deduplicate: if command already exists, remove it first (MRU reorder)
        self.entries.retain(|e| e.cmd != cmd);

        // Cap before inserting
        while self.entries.len() >= self.max_entries {
            self.entries.pop_front();
        }

        self.entries.push_back(CommandEntry {
            cmd,
            cwd,
            timestamp,
            exit_code,
        });
    }

    pub fn commands(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().rev().map(|e| e.cmd.as_str())
    }

    pub fn entries(&self) -> &VecDeque<CommandEntry> {
        &self.entries
    }

    pub fn load(&mut self) {
        let path = match &self.path {
            Some(p) => p.clone(),
            None => match command_history_path() {
                Some(p) => p,
                None => return,
            },
        };
        if !path.exists() {
            return;
        }
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(loaded) = serde_json::from_str::<Vec<CommandEntry>>(&data) {
                self.entries.clear();
                for entry in loaded {
                    if self.entries.len() >= self.max_entries {
                        break;
                    }
                    // Deduplicate on load too
                    self.entries.retain(|e| e.cmd != entry.cmd);
                    self.entries.push_back(entry);
                }
                self.path = Some(path);
            }
        }
    }

    pub fn save(&self) {
        let path = match &self.path {
            Some(p) => p.clone(),
            None => match command_history_path() {
                Some(p) => p,
                None => return,
            },
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let entries: Vec<&CommandEntry> = self.entries.iter().collect();
        if let Ok(json) = serde_json::to_string(&entries) {
            let _ = std::fs::write(&path, json);
        }
    }
}

pub fn command_history_path() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".cache")))
            .ok()
            .map(|d| d.join("SYNAPSE_").join("history.json"))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var("HOME")
            .map(|h| {
                PathBuf::from(h)
                    .join("Library")
                    .join("Caches")
                    .join("SYNAPSE_")
                    .join("history.json")
            })
            .ok()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}
