use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use synapse_ui::tab_bar::TabBar;

use crate::workspace::WorkspaceManager;

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    pub version: String,
    pub saved_at: String,
    pub workspaces: HashMap<String, TabBar>,
}

fn now_iso() -> String {
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    let days_since_epoch = secs / 86400;
    let (y, m, d) = civil_from_days(days_since_epoch as i64);
    let sod = secs % 86400;
    let h = sod / 3600;
    let min = (sod % 3600) / 60;
    let s = sod % 60;
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, h, min, s)
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

pub fn session_cache_dir() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".cache")))
            .ok()
            .map(|d| d.join("SYNAPSE_"))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var("HOME")
            .map(|h| {
                PathBuf::from(h)
                    .join("Library")
                    .join("Caches")
                    .join("SYNAPSE_")
            })
            .ok()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

pub fn default_session_path() -> Option<PathBuf> {
    session_cache_dir().map(|d| d.join("session.json"))
}

pub fn named_session_path(name: &str) -> Option<PathBuf> {
    session_cache_dir().map(|d| d.join(format!("session_{}.json", name)))
}

pub fn save_session(workspaces: &WorkspaceManager, path: &std::path::Path) -> Result<(), String> {
    let tab_bars = workspaces.tab_bar_for_save();
    let session = Session {
        version: env!("CARGO_PKG_VERSION").to_string(),
        saved_at: now_iso(),
        workspaces: tab_bars,
    };
    let json = serde_json::to_string_pretty(&session).map_err(|e| format!("serialize: {e}"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    std::fs::write(path, json).map_err(|e| format!("write: {e}"))
}

pub fn load_session(path: &std::path::Path) -> Result<HashMap<String, TabBar>, String> {
    let json = std::fs::read_to_string(path).map_err(|e| format!("read: {e}"))?;
    let session: Session = serde_json::from_str(&json).map_err(|e| format!("deserialize: {e}"))?;
    Ok(session.workspaces)
}

pub fn session_exists(path: &std::path::Path) -> bool {
    path.exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use synapse_ui::pane::PaneId;
    use synapse_ui::splitter::{PaneTree, SplitDirection};
    use synapse_ui::tab_bar::{Tab, TabId};

    #[test]
    fn test_session_round_trip() {
        let tab = Tab::new(TabId(0), PaneId(0));
        let mut tab_bar = TabBar::new(tab);
        tab_bar.set_title(TabId(0), "shell".into());
        tab_bar.tabs[0].cwd = "/home/user".into();

        let json = serde_json::to_string_pretty(&tab_bar).unwrap();
        let restored: TabBar = serde_json::from_str(&json).unwrap();
        let restored = restored.from_saved();

        assert_eq!(restored.tabs.len(), 1);
        assert_eq!(restored.tabs[0].title, "shell");
        assert_eq!(restored.tabs[0].cwd, "/home/user");
        assert_eq!(restored.active, 0);
    }

    #[test]
    fn test_tab_bar_json_round_trip_with_ids() {
        let tab = Tab::new(TabId(3), PaneId(7));
        let mut tab_bar = TabBar::new(tab);
        tab_bar.tabs[0].title = "zsh".into();
        tab_bar.tabs[0].cwd = "/tmp".into();

        let json = serde_json::to_string_pretty(&tab_bar).unwrap();
        let restored: TabBar = serde_json::from_str(&json).unwrap();
        let restored = restored.from_saved();

        assert_eq!(restored.tabs.len(), 1);
        assert_eq!(restored.tabs[0].id, TabId(3));
        assert_eq!(restored.tabs[0].active_pane, PaneId(7));
        assert_eq!(restored.tabs[0].title, "zsh");
        assert_eq!(restored.tabs[0].cwd, "/tmp");
    }

    #[test]
    fn test_session_multi_tab_round_trip() {
        let tab0 = Tab::new(TabId(0), PaneId(0));
        let mut tab_bar = TabBar::new(tab0);
        tab_bar.tabs[0].title = "tab0".into();
        tab_bar.tabs[0].cwd = "/a".into();

        tab_bar.new_tab();
        tab_bar.tabs[1].title = "tab1".into();
        tab_bar.tabs[1].cwd = "/b".into();

        tab_bar.new_tab();
        tab_bar.tabs[2].title = "tab2".into();
        tab_bar.tabs[2].cwd = "/c".into();

        let json = serde_json::to_string_pretty(&tab_bar).unwrap();
        let restored: TabBar = serde_json::from_str(&json).unwrap();
        let restored = restored.from_saved();

        assert_eq!(restored.tabs.len(), 3);
        assert_eq!(restored.tabs[0].title, "tab0");
        assert_eq!(restored.tabs[1].title, "tab1");
        assert_eq!(restored.tabs[2].title, "tab2");
        assert_eq!(restored.tabs[0].cwd, "/a");
        assert_eq!(restored.tabs[1].cwd, "/b");
        assert_eq!(restored.tabs[2].cwd, "/c");
    }

    #[test]
    fn test_session_with_split_panes_round_trip() {
        let tab0 = Tab::new(TabId(0), PaneId(0));
        let mut tab_bar = TabBar::new(tab0);
        tab_bar.tabs[0].title = "split_test".into();
        tab_bar.tabs[0].cwd = "/src".into();

        let tree = PaneTree::Split {
            direction: SplitDirection::Vertical,
            ratio: 0.5,
            first: Box::new(PaneTree::Leaf(PaneId(0))),
            second: Box::new(PaneTree::Leaf(PaneId(1))),
        };
        tab_bar.tabs[0].pane_tree = tree;
        tab_bar.tabs[0].active_pane = PaneId(1);

        let json = serde_json::to_string_pretty(&tab_bar).unwrap();
        let restored: TabBar = serde_json::from_str(&json).unwrap();
        let restored = restored.from_saved();

        assert_eq!(restored.tabs.len(), 1);
        let restored_panes = restored.tabs[0].pane_tree.all_panes();
        assert_eq!(restored_panes.len(), 2);
        assert_eq!(restored.tabs[0].active_pane, PaneId(1));
    }

    #[test]
    fn test_session_active_tab_saved() {
        let tab0 = Tab::new(TabId(0), PaneId(0));
        let mut tab_bar = TabBar::new(tab0);
        tab_bar.new_tab();
        tab_bar.new_tab();
        tab_bar.activate(2);

        let json = serde_json::to_string_pretty(&tab_bar).unwrap();
        let restored: TabBar = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.active, 2);
    }

    #[test]
    fn test_session_path_functions() {
        let default = default_session_path();
        let named = named_session_path("mysession");
        if let Some(def) = default {
            assert!(def.ends_with("session.json"));
        }
        if let Some(n) = named {
            assert!(n.to_string_lossy().contains("session_mysession.json"));
        }
    }
}
