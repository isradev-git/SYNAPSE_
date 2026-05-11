use crate::pane::PaneId;
use crate::splitter::PaneTree;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TabId(pub u64);

pub struct Tab {
    pub id: TabId,
    pub title: String,
    pub cwd: String,
    pub pane_tree: PaneTree,
    pub active_pane: PaneId,
}

impl Tab {
    pub fn new(id: TabId, pane_id: PaneId) -> Self {
        Self {
            id,
            title: String::new(),
            cwd: String::new(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane::PaneId;

    fn make_tab(id: u64, pane_id: u64) -> Tab {
        Tab::new(TabId(id), PaneId(pane_id))
    }

    #[test]
    fn test_tab_new() {
        let tab = make_tab(0, 0);
        assert_eq!(tab.id, TabId(0));
        assert_eq!(tab.title, "");
        assert_eq!(tab.cwd, "");
    }

    #[test]
    fn test_tab_bar_new() {
        let initial = make_tab(0, 0);
        let bar = TabBar::new(initial);
        assert_eq!(bar.tabs.len(), 1);
        assert_eq!(bar.active, 0);
    }

    #[test]
    fn test_new_tab() {
        let initial = make_tab(0, 0);
        let mut bar = TabBar::new(initial);
        let (tab_id, pane_id) = bar.new_tab();
        assert_eq!(tab_id, TabId(1));
        assert_eq!(pane_id, PaneId(1));
        assert_eq!(bar.tabs.len(), 2);
        assert_eq!(bar.active, 1);
    }

    #[test]
    fn test_close_tab_cannot_close_last() {
        let initial = make_tab(0, 0);
        let mut bar = TabBar::new(initial);
        assert!(bar.close_tab(0).is_none());
        assert_eq!(bar.tabs.len(), 1);
    }

    #[test]
    fn test_close_tab_active_last() {
        let initial = make_tab(0, 0);
        let mut bar = TabBar::new(initial);
        bar.new_tab();
        bar.new_tab();
        assert_eq!(bar.tabs.len(), 3);
        bar.active = 2;
        let removed = bar.close_tab(2);
        assert!(removed.is_some());
        assert_eq!(bar.tabs.len(), 2);
        assert_eq!(bar.active, 1); // shifted to last valid index
    }

    #[test]
    fn test_close_tab_non_last() {
        let initial = make_tab(0, 0);
        let mut bar = TabBar::new(initial);
        bar.new_tab();
        bar.new_tab();
        bar.active = 0;
        let removed = bar.close_tab(0);
        assert!(removed.is_some());
        assert_eq!(bar.tabs.len(), 2);
        assert_eq!(bar.active, 0);
    }

    #[test]
    fn test_activate_clamped() {
        let initial = make_tab(0, 0);
        let mut bar = TabBar::new(initial);
        bar.activate(99);
        assert_eq!(bar.active, 0); // stays at 0
        bar.activate(0);
        assert_eq!(bar.active, 0);
    }

    #[test]
    fn test_next_tab_circular() {
        let initial = make_tab(0, 0);
        let mut bar = TabBar::new(initial);
        bar.new_tab();
        bar.new_tab();
        bar.active = 0;
        assert_eq!(bar.next_tab(), 1);
        assert_eq!(bar.next_tab(), 2);
        assert_eq!(bar.next_tab(), 0); // wraps around
    }

    #[test]
    fn test_prev_tab_circular() {
        let initial = make_tab(0, 0);
        let mut bar = TabBar::new(initial);
        bar.new_tab();
        bar.new_tab();
        bar.active = 0;
        assert_eq!(bar.prev_tab(), 2); // wraps around to last
        assert_eq!(bar.prev_tab(), 1);
        assert_eq!(bar.prev_tab(), 0);
    }

    #[test]
    fn test_set_title_by_id() {
        let initial = make_tab(0, 0);
        let mut bar = TabBar::new(initial);
        bar.set_title(TabId(0), "Shell".into());
        assert_eq!(bar.tabs[0].title, "Shell");
    }

    #[test]
    fn test_set_title_nonexistent() {
        let initial = make_tab(0, 0);
        let mut bar = TabBar::new(initial);
        bar.set_title(TabId(99), "Ghost".into());
        assert_eq!(bar.tabs[0].title, ""); // unchanged
    }

    #[test]
    fn test_active_tab() {
        let initial = make_tab(0, 0);
        let mut bar = TabBar::new(initial);
        bar.new_tab();
        bar.active = 1;
        assert_eq!(bar.active_tab().id, TabId(1));
    }

    #[test]
    fn test_active_tab_mut() {
        let initial = make_tab(0, 0);
        let mut bar = TabBar::new(initial);
        bar.active_tab_mut().title = "Modified".into();
        assert_eq!(bar.tabs[0].title, "Modified");
    }

    #[test]
    fn test_id_incrementation() {
        let initial = make_tab(5, 10);
        let mut bar = TabBar::new(initial);
        assert_eq!(bar.next_tab_id(), TabId(6));
        assert_eq!(bar.next_tab_id(), TabId(7));
        assert_eq!(bar.next_pane_id(), PaneId(11));
        assert_eq!(bar.next_pane_id(), PaneId(12));
    }
}
