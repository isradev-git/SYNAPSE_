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

/// A clipboard operation queued from OSC 52 (alacritty event).
pub enum ClipboardOp {
    Write(alacritty_terminal::term::ClipboardType, String),
    /// fmt: alacritty-provided formatter that encodes the OSC 52 response bytes.
    Read(
        alacritty_terminal::term::ClipboardType,
        std::sync::Arc<dyn Fn(&str) -> String + Send + Sync>,
    ),
}

#[derive(Debug, Clone, PartialEq)]
pub enum MarkKind {
    PromptStart,
    CommandStart,
    CommandEnd,
}

#[derive(Debug, Clone)]
pub struct SemanticMark {
    pub kind: MarkKind,
    /// `term.grid().history_size()` at capture time (requires Dimensions in scope).
    /// Used to compute how far into history the mark is: `current_history - history_snapshot`.
    pub history_snapshot: usize,
}

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
    /// Channel for OSC 133 semantic marks from the PTY reader thread.
    pub osc133_rx: mpsc::Receiver<SemanticMark>,
    /// Channel for OSC 9/777 notification strings from the PTY reader thread.
    pub osc9_rx: mpsc::Receiver<String>,
    title: String,
    cwd: String,
    pub pending_bell: bool,
    pub clipboard_pending: std::collections::VecDeque<ClipboardOp>,
    pub notifications: std::collections::VecDeque<String>,
    pub semantic_marks: Vec<SemanticMark>,
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
        osc133_rx: mpsc::Receiver<SemanticMark>,
        osc9_rx: mpsc::Receiver<String>,
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
            osc133_rx,
            osc9_rx,
            title: String::new(),
            cwd: String::new(),
            pending_bell: false,
            clipboard_pending: std::collections::VecDeque::new(),
            notifications: std::collections::VecDeque::new(),
            semantic_marks: Vec::new(),
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
                Event::Bell => {
                    self.pending_bell = true;
                }
                Event::ClipboardStore(kind, data) => {
                    self.clipboard_pending.push_back(ClipboardOp::Write(kind, data));
                }
                Event::ClipboardLoad(kind, fmt) => {
                    self.clipboard_pending.push_back(ClipboardOp::Read(kind, fmt));
                }
                Event::PtyWrite(s) => {
                    if let Ok(mut w) = self.pty_writer.lock() {
                        let _ = std::io::Write::write_all(&mut *w, s.as_bytes());
                    }
                }
                _ => {}
            }
        }
        while let Ok(path) = self.osc7_rx.try_recv() {
            self.cwd = path;
        }
        while let Ok(mut mark) = self.osc133_rx.try_recv() {
            if let Ok(term) = self.term.lock() {
                use alacritty_terminal::grid::Dimensions;
                mark.history_snapshot = term.grid().history_size();
            }
            self.semantic_marks.push(mark);
            if self.semantic_marks.len() > 500 {
                self.semantic_marks.remove(0);
            }
        }
        while let Ok(notif) = self.osc9_rx.try_recv() {
            self.notifications.push_back(notif);
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

    #[test]
    fn test_clipboard_op_write_variant() {
        let op = ClipboardOp::Write(
            alacritty_terminal::term::ClipboardType::Clipboard,
            "hello".to_string(),
        );
        assert!(matches!(op, ClipboardOp::Write(_, ref s) if s == "hello"));
    }

    #[test]
    fn test_mark_kind_variants() {
        assert_ne!(MarkKind::PromptStart, MarkKind::CommandStart);
        assert_ne!(MarkKind::CommandStart, MarkKind::CommandEnd);
    }

    #[test]
    fn test_semantic_mark_has_fields() {
        let m = SemanticMark { kind: MarkKind::PromptStart, history_snapshot: 42 };
        assert_eq!(m.history_snapshot, 42);
    }

    #[test]
    fn test_clipboard_op_read_has_formatter() {
        let fmt: std::sync::Arc<dyn Fn(&str) -> String + Send + Sync> =
            std::sync::Arc::new(|s: &str| format!("response:{}", s));
        let op = ClipboardOp::Read(
            alacritty_terminal::term::ClipboardType::Clipboard,
            fmt.clone(),
        );
        if let ClipboardOp::Read(_, f) = op {
            assert_eq!(f("clipboard_text"), "response:clipboard_text");
        }
    }
}
