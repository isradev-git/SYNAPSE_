use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::term::Term;
use portable_pty::MasterPty;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PaneId(pub u64);

#[derive(Clone)]
pub struct EventProxy {
    sender: mpsc::SyncSender<Event>,
}

impl EventProxy {
    pub fn new(sender: mpsc::SyncSender<Event>) -> Self {
        Self { sender }
    }
}

impl EventListener for EventProxy {
    fn send_event(&self, event: Event) {
        let _ = self.sender.try_send(event);
    }
}

pub struct Pane {
    pub id: PaneId,
    pub term: Arc<Mutex<Term<EventProxy>>>,
    pub pty_writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pub pty_master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    pub event_rx: mpsc::Receiver<Event>,
    pub dirty: Arc<AtomicBool>,
    pub cols: usize,
    pub rows: usize,
    title: String,
    cwd: String,
}

impl Pane {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: PaneId,
        term: Arc<Mutex<Term<EventProxy>>>,
        pty_writer: Box<dyn Write + Send>,
        pty_master: Box<dyn MasterPty + Send>,
        event_rx: mpsc::Receiver<Event>,
        dirty: Arc<AtomicBool>,
        cols: usize,
        rows: usize,
    ) -> Self {
        Self {
            id,
            term,
            pty_writer: Arc::new(Mutex::new(pty_writer)),
            pty_master: Arc::new(Mutex::new(pty_master)),
            event_rx,
            dirty,
            cols,
            rows,
            title: String::new(),
            cwd: String::new(),
        }
    }

    pub fn write_to_pty(&self, data: &[u8]) {
        if let Ok(mut w) = self.pty_writer.lock() {
            let _ = w.write_all(data);
        }
    }

    pub fn scroll_viewport(&self, scroll: Scroll) {
        if let Ok(mut term) = self.term.lock() {
            term.scroll_display(scroll);
        }
        self.dirty.store(true, Ordering::Release);
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::AcqRel)
    }

    /// Drain all pending events from the channel. Updates title on Event::Title.
    /// Returns true if Event::Exit was received (process died).
    pub fn poll_events(&mut self) -> bool {
        let mut exited = false;
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                Event::Exit | Event::ChildExit(_) => exited = true,
                Event::Title(title) => self.title = title,
                _ => {}
            }
        }
        exited
    }

    pub fn title(&self) -> String {
        self.title.clone()
    }

    pub fn cwd(&self) -> String {
        self.cwd.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_id_ordering() {
        let a = PaneId(1);
        let b = PaneId(2);
        assert!(a < b);
        assert_eq!(a, PaneId(1));
    }

    #[test]
    fn dirty_flag_clears_on_read() {
        use std::sync::atomic::Ordering;
        let dirty = Arc::new(AtomicBool::new(true));
        let was_dirty = dirty.swap(false, Ordering::AcqRel);
        assert!(was_dirty);
        let now_dirty = dirty.load(Ordering::Acquire);
        assert!(!now_dirty);
    }
}
