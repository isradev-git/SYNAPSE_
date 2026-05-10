use std::cell::RefCell;
use std::rc::Rc;

use luna_terminal::grid::Grid;
use luna_terminal::parser::{TerminalModes, VteProcessor};
use luna_terminal::pty::PtySession;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PaneId(pub u64);

pub struct Pane {
    pub id: PaneId,
    pub pty_session: PtySession,
    pub grid: Rc<RefCell<Grid>>,
    pub processor: VteProcessor,
    pub cols: usize,
    pub rows: usize,
    pub modes: Rc<RefCell<TerminalModes>>,
    title: Rc<RefCell<String>>,
    cwd: Rc<RefCell<String>>,
}

impl Pane {
    pub fn new(
        id: PaneId,
        pty_session: PtySession,
        grid: Rc<RefCell<Grid>>,
        cols: usize,
        rows: usize,
    ) -> Self {
        let title = Rc::new(RefCell::new(String::new()));
        let cwd = Rc::new(RefCell::new(String::new()));
        let modes = Rc::new(RefCell::new(TerminalModes {
            bracketed_paste: true,
            ..Default::default()
        }));
        let processor =
            VteProcessor::new_with_title(grid.clone(), title.clone(), cwd.clone(), modes.clone());
        Self {
            id,
            pty_session,
            grid,
            processor,
            cols,
            rows,
            modes,
            title,
            cwd,
        }
    }

    pub fn title(&self) -> String {
        self.title.borrow().clone()
    }

    pub fn cwd(&self) -> String {
        self.cwd.borrow().clone()
    }
}
