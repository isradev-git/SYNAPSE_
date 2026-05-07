use crate::pane::PaneId;
use crate::splitter::PaneTree;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TabId(pub u64);

pub struct Tab {
    pub id: TabId,
    pub title: String,
    pub pane_tree: PaneTree,
    pub active_pane: PaneId,
}

impl Tab {
    pub fn new(id: TabId, pane_id: PaneId) -> Self {
        Self {
            id,
            title: String::from("Luna"),
            pane_tree: PaneTree::leaf(pane_id),
            active_pane: pane_id,
        }
    }
}

pub struct TabBar {
    pub tabs: Vec<Tab>,
    pub active: usize,
    next_tab_id: u64,
    next_pane_id: u64,
}

impl TabBar {
    pub fn new(initial_tab: Tab) -> Self {
        let next_tab_id = initial_tab.id.0 + 1;
        let next_pane_id = initial_tab.active_pane.0 + 1;
        Self {
            tabs: vec![initial_tab],
            active: 0,
            next_tab_id,
            next_pane_id,
        }
    }

    pub fn active_tab(&self) -> &Tab {
        &self.tabs[self.active]
    }

    pub fn active_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    pub fn next_tab_id(&mut self) -> TabId {
        let id = TabId(self.next_tab_id);
        self.next_tab_id += 1;
        id
    }

    pub fn next_pane_id(&mut self) -> PaneId {
        let id = PaneId(self.next_pane_id);
        self.next_pane_id += 1;
        id
    }

    pub fn new_tab(&mut self) -> (TabId, PaneId) {
        let pane_id = self.next_pane_id();
        let tab_id = self.next_tab_id();
        let tab = Tab::new(tab_id, pane_id);
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
        (tab_id, pane_id)
    }

    pub fn close_tab(&mut self, index: usize) -> Option<Tab> {
        if self.tabs.len() <= 1 {
            return None;
        }
        let removed = self.tabs.remove(index);
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        }
        Some(removed)
    }

    pub fn activate(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active = index;
        }
    }

    pub fn next_tab(&mut self) -> usize {
        self.active = (self.active + 1) % self.tabs.len();
        self.active
    }

    pub fn prev_tab(&mut self) -> usize {
        if self.active == 0 {
            self.active = self.tabs.len() - 1;
        } else {
            self.active -= 1;
        }
        self.active
    }

    pub fn set_title(&mut self, tab_id: TabId, title: String) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
            tab.title = title;
        }
    }
}
