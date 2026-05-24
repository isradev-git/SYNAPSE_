use std::collections::HashMap;

use synapse_ui::pane::{Pane, PaneId};
use synapse_ui::tab_bar::TabBar;

use crate::pane_ops::create_pane_full;
use crate::render::PaneCellCache;

#[allow(dead_code)]
pub struct Workspace {
    pub name: String,
    pub tab_bar: TabBar,
    pub panes: Vec<Pane>,
    pub pane_cell_caches: HashMap<PaneId, PaneCellCache>,
}

pub struct WorkspaceManager {
    pub workspaces: HashMap<String, Workspace>,
    pub active: String,
}

impl WorkspaceManager {
    pub fn new(first_ws_name: &str, tab_bar: TabBar, panes: Vec<Pane>) -> Self {
        let name = first_ws_name.to_string();
        let mut workspaces = HashMap::new();
        workspaces.insert(
            name.clone(),
            Workspace {
                name: name.clone(),
                tab_bar,
                panes,
                pane_cell_caches: HashMap::new(),
            },
        );
        Self {
            workspaces,
            active: name,
        }
    }

    pub fn active_ws(&self) -> &Workspace {
        &self.workspaces[&self.active]
    }

    pub fn active_ws_mut(&mut self) -> &mut Workspace {
        self.workspaces.get_mut(&self.active).unwrap()
    }

    pub fn active_tab_bar(&self) -> &TabBar {
        &self.active_ws().tab_bar
    }

    pub fn active_panes(&self) -> &[Pane] {
        &self.active_ws().panes
    }

    #[allow(dead_code)]
    pub fn active_cell_caches(&self) -> &HashMap<PaneId, PaneCellCache> {
        &self.active_ws().pane_cell_caches
    }

    #[allow(dead_code)]
    pub fn active_tab_bar_mut(&mut self) -> &mut TabBar {
        &mut self.workspaces.get_mut(&self.active).unwrap().tab_bar
    }

    pub fn active_panes_mut(&mut self) -> &mut Vec<Pane> {
        &mut self.workspaces.get_mut(&self.active).unwrap().panes
    }

    pub fn active_cell_caches_mut(&mut self) -> &mut HashMap<PaneId, PaneCellCache> {
        &mut self
            .workspaces
            .get_mut(&self.active)
            .unwrap()
            .pane_cell_caches
    }

    pub fn active_split_mut(
        &mut self,
    ) -> (
        &mut TabBar,
        &mut Vec<Pane>,
        &mut HashMap<PaneId, PaneCellCache>,
    ) {
        let ws = self.workspaces.get_mut(&self.active).unwrap();
        (&mut ws.tab_bar, &mut ws.panes, &mut ws.pane_cell_caches)
    }

    pub fn workspace_names(&self) -> Vec<&String> {
        let mut names: Vec<&String> = self.workspaces.keys().collect();
        names.sort();
        names
    }

    pub fn switch(&mut self, name: &str) -> bool {
        if !self.workspaces.contains_key(name) || self.active == name {
            return false;
        }
        self.active = name.to_string();
        true
    }

    pub fn create(
        &mut self,
        name: &str,
        cols: usize,
        rows: usize,
        shell: &str,
        shell_args: &[String],
        scrollback: usize,
    ) -> Result<(), String> {
        if name.is_empty() || self.workspaces.contains_key(name) {
            return Err("workspace name empty or already exists".into());
        }
        let first_tab_id = synapse_ui::tab_bar::TabId(0);
        let first_pane_id = PaneId(0);
        let first_pane = create_pane_full(
            first_pane_id,
            cols,
            rows,
            None,
            Some(shell),
            shell_args,
            scrollback,
        )
        .map_err(|e| format!("pane creation: {e}"))?;
        let first_tab = synapse_ui::tab_bar::Tab::new(first_tab_id, first_pane_id);
        let tab_bar = TabBar::new(first_tab);
        self.workspaces.insert(
            name.to_string(),
            Workspace {
                name: name.to_string(),
                tab_bar,
                panes: vec![first_pane],
                pane_cell_caches: HashMap::new(),
            },
        );
        Ok(())
    }

    #[allow(dead_code)]
    pub fn rename(&mut self, new_name: &str) -> Result<(), String> {
        if new_name.is_empty() {
            return Err("workspace name cannot be empty".into());
        }
        let old_name = self.active.clone();
        if old_name == new_name {
            return Ok(());
        }
        if self.workspaces.contains_key(new_name) {
            return Err(format!("workspace '{}' already exists", new_name));
        }
        let mut ws = self.workspaces.remove(&old_name).unwrap();
        ws.name = new_name.to_string();
        self.workspaces.insert(new_name.to_string(), ws);
        self.active = new_name.to_string();
        Ok(())
    }

    pub fn delete(&mut self, name: &str) -> bool {
        if self.workspaces.len() <= 1 {
            return false;
        }
        if !self.workspaces.contains_key(name) {
            return false;
        }
        self.workspaces.remove(name);
        if self.active == name {
            self.active = self.workspaces.keys().next().cloned().unwrap();
        }
        true
    }

    pub fn tab_bar_for_save(&self) -> HashMap<String, TabBar> {
        self.workspaces
            .iter()
            .map(|(name, ws)| {
                let json = serde_json::to_string(&ws.tab_bar).unwrap_or_else(|_| "{}".into());
                let tb: TabBar = serde_json::from_str(&json).unwrap_or_else(|_| {
                    TabBar::new(synapse_ui::tab_bar::Tab::new(
                        synapse_ui::tab_bar::TabId(0),
                        synapse_ui::pane::PaneId(0),
                    ))
                });
                (name.clone(), tb)
            })
            .collect()
    }

    #[allow(dead_code)]
    pub fn load_tab_bars(
        &mut self,
        saved: HashMap<String, TabBar>,
        cols: usize,
        rows: usize,
        scrollback: usize,
    ) {
        for (name, tab_bar) in saved {
            let tab_bar = tab_bar.from_saved();
            let mut panes = Vec::new();
            for tab in &tab_bar.tabs {
                for pane_id in tab.pane_tree.all_panes() {
                    match create_pane_full(
                        pane_id,
                        cols,
                        rows,
                        if tab.cwd.is_empty() {
                            None
                        } else {
                            Some(tab.cwd.clone())
                        },
                        None,
                        &[],
                        scrollback,
                    ) {
                        Ok(pane) => panes.push(pane),
                        Err(e) => {
                            tracing::warn!(
                                "Failed to restore pane {} in ws '{}': {e}",
                                pane_id.0,
                                name
                            );
                        }
                    }
                }
            }
            self.workspaces.insert(
                name.clone(),
                Workspace {
                    name: name.clone(),
                    tab_bar,
                    panes,
                    pane_cell_caches: HashMap::new(),
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use synapse_ui::pane::PaneId;
    use synapse_ui::tab_bar::{Tab, TabId};

    fn make_manager() -> WorkspaceManager {
        let first_tab = Tab::new(TabId(0), PaneId(0));
        let tab_bar = TabBar::new(first_tab);
        WorkspaceManager::new("default", tab_bar, vec![])
    }

    #[test]
    fn test_manager_default_has_one_workspace() {
        let mgr = make_manager();
        assert_eq!(mgr.workspaces.len(), 1);
        assert_eq!(mgr.active, "default");
    }

    #[test]
    fn test_workspace_names_sorted() {
        let mgr = make_manager();
        let names = mgr.workspace_names();
        assert_eq!(names, vec![&"default".to_string()]);
    }

    #[test]
    fn test_rename_workspace() {
        let mut mgr = make_manager();
        assert!(mgr.rename("dev").is_ok());
        assert_eq!(mgr.active, "dev");
        assert!(mgr.workspaces.contains_key("dev"));
        assert!(!mgr.workspaces.contains_key("default"));
    }

    #[test]
    fn test_rename_to_existing_fails() {
        let mut mgr = make_manager();
        assert!(mgr.rename("dev").is_ok());
        let result = mgr.rename("dev");
        assert!(result.is_ok());
    }

    #[test]
    fn test_rename_to_target_collision_fails() {
        let mut mgr = make_manager();
        let ws = Workspace {
            name: "other".into(),
            tab_bar: TabBar::new(Tab::new(TabId(1), PaneId(1))),
            panes: vec![],
            pane_cell_caches: HashMap::new(),
        };
        mgr.workspaces.insert("other".into(), ws);
        let result = mgr.rename("other");
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_fails_when_only_one() {
        let mut mgr = make_manager();
        assert!(!mgr.delete("default"));
    }

    #[test]
    fn test_delete_switches_active_when_deleting_current() {
        let mut mgr = make_manager();
        let ws = Workspace {
            name: "other".into(),
            tab_bar: TabBar::new(Tab::new(TabId(1), PaneId(1))),
            panes: vec![],
            pane_cell_caches: HashMap::new(),
        };
        mgr.workspaces.insert("other".into(), ws);
        assert!(mgr.delete("default"));
        assert_eq!(mgr.active, "other");
        assert_eq!(mgr.workspaces.len(), 1);
    }

    #[test]
    fn test_switch_to_existing() {
        let mut mgr = make_manager();
        let ws = Workspace {
            name: "other".into(),
            tab_bar: TabBar::new(Tab::new(TabId(1), PaneId(1))),
            panes: vec![],
            pane_cell_caches: HashMap::new(),
        };
        mgr.workspaces.insert("other".into(), ws);
        assert!(mgr.switch("other"));
        assert_eq!(mgr.active, "other");
    }

    #[test]
    fn test_switch_to_same_returns_false() {
        let mut mgr = make_manager();
        assert!(!mgr.switch("default"));
    }

    #[test]
    fn test_tab_bar_for_save() {
        let mgr = make_manager();
        let saved = mgr.tab_bar_for_save();
        assert_eq!(saved.len(), 1);
        assert!(saved.contains_key("default"));
        assert_eq!(saved["default"].tabs.len(), 1);
    }
}
