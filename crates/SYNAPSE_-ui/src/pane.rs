use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
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

/// Kitty keyboard protocol command sent from the PTY reader thread to the main thread.
#[derive(Debug, Clone)]
pub enum KkpCommand {
    /// App queried our capability level — we already responded; this just notifies main thread.
    Query,
    /// App pushed flags: activate KKP with these flags.
    Push(u8),
    /// App popped flags: restore previous or disable.
    Pop,
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
    /// Kitty keyboard protocol flags (0 = disabled). Written from reader thread.
    pub kitty_flags: Arc<AtomicU8>,
    /// Whether kitty keyboard protocol is active for this pane.
    pub kitty_active: Arc<AtomicBool>,
    /// Channel for KKP commands detected in the PTY output stream.
    pub kkp_rx: mpsc::Receiver<KkpCommand>,
    /// Stack of previous kitty_flags values for push/pop support.
    pub kitty_flags_stack: Vec<u8>,
    /// Channel for raw APC inner strings (Kitty image protocol payloads).
    pub apc_rx: mpsc::Receiver<String>,
    /// Channel for OSC 7 CWD updates from the PTY reader thread.
    osc7_rx: mpsc::Receiver<String>,
    title: String,
    cwd: String,
}

impl Pane {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: PaneId,
        term: Arc<Mutex<Term<EventProxy>>>,
        pty_writer: Arc<Mutex<Box<dyn Write + Send>>>,
        pty_master: Box<dyn MasterPty + Send>,
        event_rx: mpsc::Receiver<Event>,
        dirty: Arc<AtomicBool>,
        cols: usize,
        rows: usize,
        kitty_flags: Arc<AtomicU8>,
        kitty_active: Arc<AtomicBool>,
        kkp_rx: mpsc::Receiver<KkpCommand>,
        apc_rx: mpsc::Receiver<String>,
        osc7_rx: mpsc::Receiver<String>,
    ) -> Self {
        Self {
            id,
            term,
            pty_writer,
            pty_master: Arc::new(Mutex::new(pty_master)),
            event_rx,
            dirty,
            cols,
            rows,
            kitty_flags,
            kitty_active,
            kkp_rx,
            kitty_flags_stack: Vec::new(),
            apc_rx,
            osc7_rx,
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

    pub fn kitty_active(&self) -> bool {
        self.kitty_active.load(Ordering::Acquire)
    }

    pub fn kitty_flags(&self) -> u8 {
        self.kitty_flags.load(Ordering::Acquire)
    }

    /// Drain all pending events from both channels. Updates title on Event::Title,
    /// handles KKP push/pop commands. Returns true if Event::Exit was received.
    pub fn poll_events(&mut self) -> bool {
        let mut exited = false;
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                Event::Exit | Event::ChildExit(_) => exited = true,
                Event::Title(title) => self.title = title,
                _ => {}
            }
        }
        while let Ok(path) = self.osc7_rx.try_recv() {
            self.cwd = path;
        }
        while let Ok(cmd) = self.kkp_rx.try_recv() {
            match cmd {
                KkpCommand::Push(flags) => {
                    let prev = self.kitty_flags.load(Ordering::Acquire);
                    self.kitty_flags_stack.push(prev);
                    self.kitty_flags.store(flags, Ordering::Release);
                    self.kitty_active.store(flags > 0, Ordering::Release);
                }
                KkpCommand::Pop => {
                    let prev = self.kitty_flags_stack.pop().unwrap_or(0);
                    self.kitty_flags.store(prev, Ordering::Release);
                    self.kitty_active.store(prev > 0, Ordering::Release);
                }
                KkpCommand::Query => {}
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
