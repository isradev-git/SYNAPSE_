use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use synapse_config::config::Config as SynapseConfig;

use alacritty_terminal::event::Event;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config as TermConfig, Term, TermMode};
use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};
use synapse_ui::pane::{EventProxy, KkpCommand, Pane, PaneId};

use crate::image_protocol;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use synapse_ui::{
    layout::Layout, splitter::PaneRect, tab_bar::TabBar, SIDEBAR_TAB_HEIGHT,
    TAB_BAR_HEIGHT as TAB_BAR_HEIGHT_PX,
};

const CLOSE_BTN_W: f32 = 16.0;

/// Map DEC private mode number to DECRPM `Pm` response code:
/// 0 = not recognized, 1 = set, 2 = reset.
fn decrqm_pm(mode: u16, flags: &TermMode) -> u8 {
    let flag = match mode {
        1 => TermMode::APP_CURSOR,
        7 => TermMode::LINE_WRAP,
        20 => TermMode::LINE_FEED_NEW_LINE,
        25 => TermMode::SHOW_CURSOR,
        1000 => TermMode::MOUSE_REPORT_CLICK,
        1002 => TermMode::MOUSE_DRAG,
        1003 => TermMode::MOUSE_MOTION,
        1004 => TermMode::FOCUS_IN_OUT,
        1005 => TermMode::UTF8_MOUSE,
        1006 => TermMode::SGR_MOUSE,
        1049 => TermMode::ALT_SCREEN,
        2004 => TermMode::BRACKETED_PASTE,
        _ => return 0,
    };
    if flags.contains(flag) {
        1
    } else {
        2
    }
}

/// Scan `bytes` for OSC 7 sequences (`ESC ] 7 ; <uri> BEL/ST`) and return
/// the decoded filesystem paths. Strips the `file://hostname` prefix so only
/// the absolute path remains.
fn extract_osc7_paths(bytes: &[u8]) -> Vec<String> {
    let mut results = Vec::new();
    let mut i = 0;
    while i + 3 < bytes.len() {
        if bytes[i] == 0x1b && bytes[i + 1] == b']' && bytes[i + 2] == b'7' && bytes[i + 3] == b';'
        {
            let start = i + 4;
            let mut j = start;
            let mut end = None;
            while j < bytes.len() {
                if bytes[j] == 0x07 {
                    end = Some((j, j + 1));
                    break;
                }
                if j + 1 < bytes.len() && bytes[j] == 0x1b && bytes[j + 1] == b'\\' {
                    end = Some((j, j + 2));
                    break;
                }
                j += 1;
            }
            if let Some((term_start, next_i)) = end {
                if let Ok(uri) = std::str::from_utf8(&bytes[start..term_start]) {
                    let path = if let Some(rest) = uri.strip_prefix("file://") {
                        // rest = "hostname/absolute/path" — skip hostname component
                        rest.find('/').map(|s| &rest[s..]).unwrap_or(rest)
                    } else {
                        uri
                    };
                    if !path.is_empty() {
                        results.push(path.to_string());
                    }
                }
                i = next_i;
                continue;
            }
        }
        i += 1;
    }
    results
}

/// Scan `bytes` for OSC 133 sequences and return the mark kinds found.
/// OSC 133 format: `ESC ] 133 ; <kind>[;<data>] BEL|ST`
/// Kinds: A=PromptStart, B=CommandStart, C/D=CommandEnd.
/// For D, the data field is the exit code.
/// Returns (MarkKind, Optional exit_code).
fn extract_osc133_marks(bytes: &[u8]) -> Vec<(synapse_ui::pane::MarkKind, Option<i32>)> {
    use synapse_ui::pane::MarkKind;
    let mut results = Vec::new();
    let mut i = 0;
    while i + 6 < bytes.len() {
        // Match: ESC ] 1 3 3 ;
        if bytes[i] == 0x1b
            && bytes[i + 1] == b']'
            && bytes[i + 2] == b'1'
            && bytes[i + 3] == b'3'
            && bytes[i + 4] == b'3'
            && bytes[i + 5] == b';'
        {
            let start = i + 6;
            let mut j = start;
            let mut end = None;
            while j < bytes.len() {
                if bytes[j] == 0x07 {
                    end = Some((j, j + 1));
                    break;
                }
                if j + 1 < bytes.len() && bytes[j] == 0x1b && bytes[j + 1] == b'\\' {
                    end = Some((j, j + 2));
                    break;
                }
                j += 1;
            }
            if let Some((term_pos, next_i)) = end {
                if term_pos > start {
                    let kind_byte = bytes[start];
                    let (kind, exit_code) = match kind_byte {
                        b'A' => (MarkKind::PromptStart, None),
                        b'B' => (MarkKind::CommandStart, None),
                        b'C' => (MarkKind::CommandEnd, None),
                        b'D' => {
                            let code = if term_pos > start + 2 && bytes[start + 1] == b';' {
                                let code_start = start + 2;
                                let code_slice = &bytes[code_start..term_pos];
                                std::str::from_utf8(code_slice)
                                    .ok()
                                    .and_then(|s| s.parse::<i32>().ok())
                            } else {
                                None
                            };
                            (MarkKind::CommandEnd, code)
                        }
                        _ => {
                            i = next_i;
                            continue;
                        }
                    };
                    results.push((kind, exit_code));
                }
                i = next_i;
                continue;
            }
        }
        i += 1;
    }
    results
}

/// Scan `bytes` for OSC 9 and OSC 777 notification sequences.
///
/// OSC 9 ; <message> BEL|ST  → returns "<message>".
/// OSC 777 ; notify ; <title> ; <body> BEL|ST → returns "<title>\x00<body>".
/// Empty messages are skipped.
fn extract_osc9_notifications(bytes: &[u8]) -> Vec<String> {
    let mut results = Vec::new();
    let mut i = 0;

    while i + 3 < bytes.len() {
        if bytes[i] != 0x1b || bytes[i + 1] != b']' {
            i += 1;
            continue;
        }

        let payload_start;
        let is_777;

        // Check for OSC 777: ESC ] 7 7 7 ;
        if i + 5 < bytes.len()
            && bytes[i + 2] == b'7'
            && bytes[i + 3] == b'7'
            && bytes[i + 4] == b'7'
            && bytes[i + 5] == b';'
        {
            payload_start = i + 6;
            is_777 = true;
        // Check for OSC 9: ESC ] 9 ;
        } else if i + 3 < bytes.len() && bytes[i + 2] == b'9' && bytes[i + 3] == b';' {
            payload_start = i + 4;
            is_777 = false;
        } else {
            i += 1;
            continue;
        }

        // Find BEL or ST terminator
        let mut j = payload_start;
        let mut end = None;
        while j < bytes.len() {
            if bytes[j] == 0x07 {
                end = Some((j, j + 1));
                break;
            }
            if j + 1 < bytes.len() && bytes[j] == 0x1b && bytes[j + 1] == b'\\' {
                end = Some((j, j + 2));
                break;
            }
            j += 1;
        }

        if let Some((term_pos, next_i)) = end {
            if term_pos > payload_start {
                if let Ok(payload) = std::str::from_utf8(&bytes[payload_start..term_pos]) {
                    let encoded = if is_777 {
                        // OSC 777: "notify ; <title> ; <body>"
                        let payload = payload.strip_prefix("notify;").unwrap_or(payload);
                        if let Some(sep) = payload.find(';') {
                            let title = &payload[..sep];
                            let body = &payload[sep + 1..];
                            if !title.is_empty() || !body.is_empty() {
                                format!("{}\x00{}", title, body)
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        }
                    } else {
                        payload.to_string()
                    };
                    if !encoded.is_empty() {
                        results.push(encoded);
                    }
                }
            }
            i = next_i;
            continue;
        }
        i += 1;
    }
    results
}

/// Minimal `Dimensions` impl used to construct and resize a `Term`.
pub(crate) struct TermSize {
    pub(crate) cols: usize,
    pub(crate) rows: usize,
    pub(crate) scrollback_lines: usize,
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize {
        self.rows + self.scrollback_lines
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

pub fn create_pane(
    id: PaneId,
    cols: usize,
    rows: usize,
    scrollback_lines: usize,
) -> Result<Pane, String> {
    create_pane_with_cwd(id, cols, rows, None, scrollback_lines)
}

pub fn create_pane_with_cwd(
    id: PaneId,
    cols: usize,
    rows: usize,
    cwd: Option<String>,
    scrollback_lines: usize,
) -> Result<Pane, String> {
    create_pane_full(id, cols, rows, cwd, None, &[], scrollback_lines)
}

pub fn create_pane_full(
    id: PaneId,
    cols: usize,
    rows: usize,
    cwd: Option<String>,
    shell_override: Option<&str>,
    shell_args: &[String],
    scrollback_lines: usize,
) -> Result<Pane, String> {
    create_pane_full_env(
        id,
        cols,
        rows,
        cwd,
        shell_override,
        shell_args,
        &[],
        scrollback_lines,
    )
}

/// Like `create_pane_full` but also injects `extra_env` key-value pairs into
/// the child process environment before spawn. Used by tab profiles (P-006).
#[allow(clippy::too_many_arguments)]
pub fn create_pane_full_env(
    id: PaneId,
    cols: usize,
    rows: usize,
    cwd: Option<String>,
    shell_override: Option<&str>,
    shell_args: &[String],
    extra_env: &[(String, String)],
    scrollback_lines: usize,
) -> Result<Pane, String> {
    // 1. Open a PTY pair sized to the current pane geometry.
    let pty_system = native_pty_system();
    let pty_pair = pty_system
        .openpty(PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("openpty failed: {e}"))?;

    // 2. Build the shell command, applying overrides and cwd.
    let shell = match shell_override {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => crate::shell::default_shell(),
    };
    let mut cmd = CommandBuilder::new(&shell);
    cmd.env("SYNAPSE_INSIDE", "true");
    cmd.env("SYNAPSE_VERSION", env!("CARGO_PKG_VERSION"));
    cmd.env("TERM_PROGRAM", "SYNAPSE_");
    cmd.env("COLORTERM", "truecolor");

    // ZDOTDIR injection for zsh: redirect to our shim directory so that
    // synapse-env.zsh (prompt, plugins, completions) loads automatically
    // without the user having to run --setup.
    let shell_basename = std::path::Path::new(&shell)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if shell_basename == "zsh" {
        if let Some(zdotdir) = SynapseConfig::config_dir()
            .map(|d| d.join("zdotdir"))
            .filter(|p| p.join(".zshrc").exists())
        {
            cmd.env("ZDOTDIR", zdotdir);
        }
    }

    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    if shell_override.is_some() {
        for arg in shell_args {
            cmd.arg(arg);
        }
    }
    if let Some(cwd) = cwd {
        cmd.cwd(cwd);
    }

    // 3. Spawn the child on the slave side. The child handle is dropped — the
    //    OS reaps the process when the PTY is closed.
    let _child = pty_pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("spawn_command failed: {e}"))?;

    // 4. Pull a reader + writer out of the master, but keep the master itself
    //    so we can resize the PTY later.
    let pty_reader = pty_pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("try_clone_reader failed: {e}"))?;
    let pty_writer = pty_pair
        .master
        .take_writer()
        .map_err(|e| format!("take_writer failed: {e}"))?;
    let pty_master = pty_pair.master;
    // Shared PTY writer: the reader thread writes terminal query responses
    // (DSR/DA via EventProxy, KKP, DECRQM) through it synchronously, and it also
    // backs keyboard input on the main thread.
    let pty_writer_shared = Arc::new(Mutex::new(pty_writer));

    // 5. Event channel for alacritty_terminal -> SYNAPSE_ events.
    let (event_tx, event_rx) = mpsc::channel::<Event>();
    let exit_tx = event_tx.clone();
    let proxy = EventProxy::new(event_tx, Arc::clone(&pty_writer_shared));

    // 6. Construct the terminal.
    let size = TermSize {
        cols,
        rows,
        scrollback_lines,
    };
    let term = Term::new(TermConfig::default(), &size, proxy);
    let term = Arc::new(Mutex::new(term));

    let dirty = Arc::new(AtomicBool::new(false));
    let frozen = Arc::new(AtomicBool::new(false));

    // KKP: shared flags between reader thread and main thread.
    let kitty_flags = Arc::new(AtomicU8::new(0));
    let kitty_active = Arc::new(AtomicBool::new(false));
    let (kkp_tx, kkp_rx) = mpsc::sync_channel::<KkpCommand>(64);
    let (apc_tx, apc_rx) = mpsc::sync_channel::<String>(64);
    let (osc7_tx, osc7_rx) = mpsc::sync_channel::<String>(16);
    let (osc133_tx, osc133_rx) = mpsc::sync_channel::<synapse_ui::pane::SemanticMark>(64);
    let (osc9_tx, osc9_rx) = mpsc::sync_channel::<String>(32);
    let (sixel_tx, sixel_rx) = mpsc::sync_channel::<Vec<u8>>(64);
    let (iterm2_tx, iterm2_rx) = mpsc::sync_channel::<String>(64);

    // 7. Spin up the reader thread: PTY bytes -> VTE parser -> Term handler.
    let term_reader = Arc::clone(&term);
    let dirty_reader = Arc::clone(&dirty);
    let frozen_reader = Arc::clone(&frozen);
    // Clones needed inside the reader thread.
    let pty_writer_kkp = Arc::clone(&pty_writer_shared);
    let pty_writer_main = Arc::clone(&pty_writer_shared);
    std::thread::Builder::new()
        .name(format!("synapse-pty-{}", id.0))
        .spawn(move || {
            let mut reader = pty_reader;
            let mut buf = [0u8; 1024];
            let mut processor: Processor<StdSyncHandler> = Processor::new();
            // Staging queue: accumulate bytes when the term lock is contended,
            // process them on the next iteration that acquires the lock.
            let mut staging: Vec<u8> = Vec::new();
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => {
                        let _ = exit_tx.send(Event::Exit);
                        break;
                    }
                    Ok(n) => {
                        staging.extend_from_slice(&buf[..n]);

                        // When frozen, pause processing but keep reading into
                        // staging so we can detect bell (\x07) for auto-unfreeze.
                        let is_frozen = frozen_reader.load(Ordering::Acquire);
                        if is_frozen && staging.contains(&0x07) {
                            frozen_reader.store(false, Ordering::Release);
                        }
                        if frozen_reader.load(Ordering::Acquire) {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            continue;
                        }

                        // Pre-scan: detect KKP queries/push and APC image sequences
                        // before passing bytes to alacritty's VTE processor.
                        for scan_cmd in image_protocol::scan_kkp(&staging) {
                            match scan_cmd {
                                image_protocol::KkpScan::Query => {
                                    if let Ok(mut w) = pty_writer_kkp.lock() {
                                        let _ = std::io::Write::write_all(&mut *w, b"\x1b[?1u");
                                    }
                                    let _ = kkp_tx.try_send(KkpCommand::Query);
                                }
                                image_protocol::KkpScan::Push(flags) => {
                                    let _ = kkp_tx.try_send(KkpCommand::Push(flags));
                                }
                                image_protocol::KkpScan::Pop => {
                                    let _ = kkp_tx.try_send(KkpCommand::Pop);
                                }
                            }
                        }
                        // DECRQM: respond to CSI ? Ps $ p mode queries.
                        for mode_num in image_protocol::scan_decrqm(&staging) {
                            let pm = if let Ok(term) = term_reader.try_lock() {
                                decrqm_pm(mode_num, term.mode())
                            } else {
                                0
                            };
                            if let Ok(mut w) = pty_writer_kkp.lock() {
                                let response = format!("\x1b[?{};{}$y", mode_num, pm);
                                let _ = std::io::Write::write_all(&mut *w, response.as_bytes());
                            }
                        }
                        for apc_seq in image_protocol::extract_apc_sequences(&staging) {
                            // Respond to a=q (query) commands immediately in the reader
                            // thread so the app learns we support the Kitty graphics protocol
                            // before any image data is sent.
                            if let Some(cmd) = image_protocol::parse_apc(&apc_seq) {
                                if cmd.action == image_protocol::KittyAction::Query {
                                    if let Some(resp) = image_protocol::kitty_response(&cmd, None) {
                                        if let Ok(mut w) = pty_writer_kkp.lock() {
                                            let _ = std::io::Write::write_all(&mut *w, &resp);
                                        }
                                    }
                                }
                            }
                            let _ = apc_tx.try_send(apc_seq);
                        }
                        for path in extract_osc7_paths(&staging) {
                            let _ = osc7_tx.try_send(path);
                        }
                        for (kind, exit_code) in extract_osc133_marks(&staging) {
                            let _ = osc133_tx.try_send(synapse_ui::pane::SemanticMark {
                                kind,
                                history_snapshot: 0, // corrected in poll_events under term lock
                                exit_code,
                                command_text: None,
                            });
                        }
                        for notif in extract_osc9_notifications(&staging) {
                            let _ = osc9_tx.try_send(notif);
                        }

                        // Extract and strip Sixel DCS sequences before VTE processes them.
                        let sixel_seqs = image_protocol::process_sixel_sequences(&mut staging);
                        for seq in sixel_seqs {
                            let _ = sixel_tx.try_send(seq);
                        }

                        // Extract and strip iTerm2 OSC 1337 inline images.
                        for img in image_protocol::process_iterm2_sequences(&mut staging) {
                            let encoded = format!(
                                "{},{},{}:{}",
                                img.display_width,
                                img.display_height,
                                img.preserve_aspect_ratio as u8,
                                img.data
                            );
                            let _ = iterm2_tx.try_send(encoded);
                        }

                        match term_reader.lock() {
                            Ok(mut term) => {
                                // Push to recording if active (before VTE processing)
                                if let Some(rec) = crate::record::RECORDING.get() {
                                    if rec.is_recording() {
                                        rec.push_event(&staging);
                                    }
                                }
                                for &byte in &staging {
                                    processor.advance(&mut *term, byte);
                                }
                                staging.clear();
                                dirty_reader.store(true, Ordering::Release);
                            }
                            Err(e) => {
                                tracing::warn!("PTY reader: term lock poisoned: {}", e);
                                break;
                            }
                        }
                    }
                }
            }
        })
        .map_err(|e| format!("failed to spawn pty reader thread: {e}"))?;

    Ok(Pane::new(
        id,
        term,
        pty_writer_main,
        pty_master,
        event_rx,
        dirty,
        frozen,
        cols,
        rows,
        kitty_flags,
        kitty_active,
        kkp_rx,
        apc_rx,
        osc7_rx,
        osc133_rx,
        osc9_rx,
        sixel_rx,
        iterm2_rx,
    ))
}

pub fn find_pane(panes: &[Pane], id: PaneId) -> Option<&Pane> {
    panes.iter().find(|p| p.id == id)
}

pub fn apply_tab_freeze(panes: &[Pane], tab_bar: &TabBar, freeze_bg: bool) {
    if tab_bar.tabs.is_empty() {
        return;
    }
    let active_pane_ids: std::collections::HashSet<PaneId> = tab_bar
        .active_tab()
        .pane_tree
        .all_panes()
        .into_iter()
        .collect();

    for pane in panes {
        let should_freeze = freeze_bg && !active_pane_ids.contains(&pane.id);
        pane.frozen.store(should_freeze, Ordering::Release);
    }
}

pub fn active_pane_mut<'a>(panes: &'a mut [Pane], tab_bar: &TabBar) -> &'a mut Pane {
    let active_id = tab_bar.active_tab().active_pane;
    panes
        .iter_mut()
        .find(|p| p.id == active_id)
        .expect("Active pane not found")
}

pub fn write_to_panes(panes: &[Pane], tab_bar: &TabBar, broadcasting: bool, data: &[u8]) {
    if broadcasting {
        let pane_ids = tab_bar.active_tab().pane_tree.all_panes();
        for pane in panes.iter().filter(|p| pane_ids.contains(&p.id)) {
            pane.write_to_pty(data);
        }
    } else {
        let active_id = tab_bar.active_tab().active_pane;
        if let Some(pane) = find_pane(panes, active_id) {
            pane.write_to_pty(data);
        }
    }
}

pub fn adjacent_pane(
    layouts: &[(PaneId, PaneRect)],
    from: PaneId,
    direction: &str,
) -> Option<PaneId> {
    let from_rect = layouts.iter().find(|(id, _)| *id == from)?.1;

    match direction {
        "right" => layouts
            .iter()
            .filter(|(id, r)| {
                *id != from
                    && r.x >= from_rect.x + from_rect.w - 1.0
                    && r.y < from_rect.y + from_rect.h
                    && r.y + r.h > from_rect.y
            })
            .min_by(|(_, a), (_, b)| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| *id),
        "left" => layouts
            .iter()
            .filter(|(id, r)| {
                *id != from
                    && r.x + r.w <= from_rect.x + 1.0
                    && r.y < from_rect.y + from_rect.h
                    && r.y + r.h > from_rect.y
            })
            .max_by(|(_, a), (_, b)| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| *id),
        "down" => layouts
            .iter()
            .filter(|(id, r)| {
                *id != from
                    && r.y >= from_rect.y + from_rect.h - 1.0
                    && r.x < from_rect.x + from_rect.w
                    && r.x + r.w > from_rect.x
            })
            .min_by(|(_, a), (_, b)| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| *id),
        "up" => layouts
            .iter()
            .filter(|(id, r)| {
                *id != from
                    && r.y + r.h <= from_rect.y + 1.0
                    && r.x < from_rect.x + from_rect.w
                    && r.x + r.w > from_rect.x
            })
            .max_by(|(_, a), (_, b)| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| *id),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_tab_click(
    tab_bar: &mut TabBar,
    panes: &mut Vec<Pane>,
    x: f64,
    y: f64,
    layout: &Layout,
    scroll_offset: &mut usize,
    cell_w: f32,
    cell_h: f32,
    scrollback_lines: usize,
    shell: &str,
    shell_args: &[String],
) {
    let sw = layout.sidebar_width as f64;
    let header_h = synapse_ui::SIDEBAR_HEADER_HEIGHT as f64;
    let tab_h = SIDEBAR_TAB_HEIGHT as f64;
    let bottom_btn_h = tab_h;
    let scroll_btn_h = synapse_ui::SIDEBAR_SCROLL_BTN_H as f64;

    let tab_count = tab_bar.tabs.len();
    let (start, end, show_up, show_down) = layout.tab_visible_range(tab_count, *scroll_offset);
    let vis_count = end.saturating_sub(start);

    // "+" button at bottom
    if y >= (layout.window_height as f64 - bottom_btn_h) {
        let pane_area = layout.pane_area();
        let margin = layout.pane_margin() as f64;
        let new_cols = ((pane_area.2 as f64 - margin * 2.0) / cell_w as f64).max(1.0) as usize;
        let new_rows = ((pane_area.3 as f64 - margin * 2.0) / cell_h as f64).max(1.0) as usize;
        let (_, pane_id) = tab_bar.new_tab();
        match create_pane_full(
            pane_id,
            new_cols,
            new_rows,
            None,
            Some(shell),
            shell_args,
            scrollback_lines,
        ) {
            Ok(pane) => panes.push(pane),
            Err(e) => {
                tracing::warn!("Failed to spawn PTY for new tab: {}", e);
                tab_bar.close_tab(tab_bar.active);
                return;
            }
        }
        let new_count = tab_bar.tabs.len();
        let (new_start, new_end, _, _) = layout.tab_visible_range(new_count, *scroll_offset);
        if tab_bar.active >= new_end {
            let vis = new_end.saturating_sub(new_start).max(1);
            *scroll_offset = tab_bar.active.saturating_sub(vis.saturating_sub(1));
        }
        return;
    }

    // Scroll up button
    if show_up && y >= header_h && y < header_h + scroll_btn_h {
        *scroll_offset = scroll_offset.saturating_sub(1);
        return;
    }

    // Scroll down button
    if show_down {
        let down_y = layout.tab_y(vis_count, show_up) as f64;
        if y >= down_y && y < down_y + scroll_btn_h {
            *scroll_offset = (*scroll_offset + 1).min(tab_count.saturating_sub(1));
            return;
        }
    }

    // Header area — ignore clicks on the logo
    if y < header_h {
        return;
    }

    // Tab area: determine which visible tab was clicked
    let scroll_top = if show_up { scroll_btn_h } else { 0.0 };
    let rel_y = y - header_h - scroll_top;
    if rel_y < 0.0 {
        return;
    }
    let vis_idx = (rel_y / tab_h).floor() as usize;
    let actual_idx = start + vis_idx;
    if actual_idx >= end {
        return;
    }

    // × close button zone (rightmost CLOSE_BTN_W px of sidebar)
    let close_start = sw - CLOSE_BTN_W as f64;
    if x >= close_start && tab_count > 1 {
        if let Some(closed) = tab_bar.close_tab(actual_idx) {
            let closed_panes = closed.pane_tree.all_panes();
            panes.retain(|p| !closed_panes.contains(&p.id));
        }
        let new_count = tab_bar.tabs.len();
        if *scroll_offset >= new_count && new_count > 0 {
            *scroll_offset = new_count - 1;
        }
        return;
    }

    tab_bar.activate(actual_idx);
}

pub fn find_hovered_divider(
    dividers: &[synapse_ui::DividerInfo],
    x: f64,
    y: f64,
) -> Option<&synapse_ui::DividerInfo> {
    dividers.iter().find(|info| {
        let h = info.hitbox;
        x >= h.x as f64 && x <= (h.x + h.w) as f64 && y >= h.y as f64 && y <= (h.y + h.h) as f64
    })
}

use crate::app::AppCore;

impl AppCore {
    pub(crate) fn change_font_size(&mut self, new_size: f32) {
        self.state.font_size = new_size;
        self.state.config.font_size = new_size;
        let _ = self.state.config.save();

        let effective = new_size * self.scale_factor;
        (self.cell_w, self.cell_h) = self.renderer.cell_metrics(effective);

        let pane_area = self.layout.pane_area();
        let pane_rect = synapse_ui::PaneRect {
            x: pane_area.0,
            y: pane_area.1,
            w: pane_area.2,
            h: pane_area.3,
        };
        let (margin, cell_w, cell_h) = (self.margin, self.cell_w, self.cell_h);
        let scrollback_lines = self.state.config.effective_scrollback();

        // Resize panes across ALL tabs — background tabs keep correct PTY dimensions.
        let all_layouts: Vec<(synapse_ui::PaneId, synapse_ui::PaneRect)> = self
            .workspaces
            .active_tab_bar()
            .tabs
            .iter()
            .flat_map(|tab| tab.pane_tree.get_layout(pane_rect))
            .collect();

        for (pane_id, rect) in &all_layouts {
            let new_cols = ((rect.w - margin * 2.0) / cell_w).max(1.0) as usize;
            let new_rows = ((rect.h - margin * 2.0) / cell_h).max(1.0) as usize;
            if let Some(pane) = self
                .workspaces
                .active_panes_mut()
                .iter_mut()
                .find(|p| p.id == *pane_id)
            {
                pane.cols = new_cols;
                pane.rows = new_rows;
                if let Ok(mut term) = pane.term.lock() {
                    term.resize(TermSize {
                        cols: new_cols,
                        rows: new_rows,
                        scrollback_lines,
                    });
                }
                if let Ok(master) = pane.pty_master.lock() {
                    let _ = master.resize(PtySize {
                        rows: new_rows as u16,
                        cols: new_cols as u16,
                        pixel_width: 0,
                        pixel_height: 0,
                    });
                }
            }
        }
    }

    pub(crate) fn handle_scale_factor_change(&mut self) {
        self.layout.tab_bar_height = TAB_BAR_HEIGHT_PX * self.scale_factor;
        let sidebar_logical = if self.state.config.sidebar_width > 0.0 {
            self.state.config.sidebar_width
        } else {
            synapse_ui::SIDEBAR_DEFAULT_WIDTH
        };
        self.layout.sidebar_width = sidebar_logical * self.scale_factor;
        self.layout.status_bar_visible = self.state.status_bar_visible;
        let current_size = self.state.font_size;
        self.change_font_size(current_size);
        self.cached_cell_data.clear();
        self.cached_ui_rects.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use synapse_ui::pane::MarkKind;

    #[test]
    fn test_extract_osc133_prompt_start() {
        // ESC ] 133 ; A BEL
        let bytes = b"\x1b]133;A\x07";
        let marks = extract_osc133_marks(bytes);
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0], (MarkKind::PromptStart, None));
    }

    #[test]
    fn test_extract_osc133_command_start() {
        let bytes = b"\x1b]133;B\x07";
        let marks = extract_osc133_marks(bytes);
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0], (MarkKind::CommandStart, None));
    }

    #[test]
    fn test_extract_osc133_command_end_c() {
        let bytes = b"\x1b]133;C\x07";
        let marks = extract_osc133_marks(bytes);
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0], (MarkKind::CommandEnd, None));
    }

    #[test]
    fn test_extract_osc133_command_end_d_with_code() {
        // D;1 = CommandEnd with exit code 1
        let bytes = b"\x1b]133;D;1\x07";
        let marks = extract_osc133_marks(bytes);
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0], (MarkKind::CommandEnd, Some(1)));
    }

    #[test]
    fn test_extract_osc133_st_terminator() {
        // ESC \ (ST) terminator instead of BEL
        let bytes = b"\x1b]133;A\x1b\\";
        let marks = extract_osc133_marks(bytes);
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0], (MarkKind::PromptStart, None));
    }

    #[test]
    fn test_extract_osc133_multiple() {
        let bytes = b"\x1b]133;A\x07some output\x1b]133;B\x07";
        let marks = extract_osc133_marks(bytes);
        assert_eq!(marks.len(), 2);
        assert_eq!(marks[0], (MarkKind::PromptStart, None));
        assert_eq!(marks[1], (MarkKind::CommandStart, None));
    }

    #[test]
    fn test_extract_osc133_unknown_kind_ignored() {
        let bytes = b"\x1b]133;Z\x07";
        let marks = extract_osc133_marks(bytes);
        assert!(marks.is_empty());
    }

    #[test]
    fn test_extract_osc9_simple() {
        let bytes = b"\x1b]9;Build complete\x07";
        let notifs = extract_osc9_notifications(bytes);
        assert_eq!(notifs.len(), 1);
        assert_eq!(notifs[0], "Build complete");
    }

    #[test]
    fn test_extract_osc9_st_terminator() {
        let bytes = b"\x1b]9;Hello\x1b\\";
        let notifs = extract_osc9_notifications(bytes);
        assert_eq!(notifs.len(), 1);
        assert_eq!(notifs[0], "Hello");
    }

    #[test]
    fn test_extract_osc777_title_body() {
        let bytes = b"\x1b]777;notify;My Title;The body text\x07";
        let notifs = extract_osc9_notifications(bytes);
        assert_eq!(notifs.len(), 1);
        let parts: Vec<&str> = notifs[0].splitn(2, '\x00').collect();
        assert_eq!(parts[0], "My Title");
        assert_eq!(parts[1], "The body text");
    }

    #[test]
    fn test_extract_osc9_multiple() {
        let bytes = b"\x1b]9;First\x07\x1b]9;Second\x07";
        let notifs = extract_osc9_notifications(bytes);
        assert_eq!(notifs.len(), 2);
    }

    #[test]
    fn test_extract_osc9_empty_ignored() {
        let bytes = b"\x1b]9;\x07";
        let notifs = extract_osc9_notifications(bytes);
        assert!(
            notifs.is_empty(),
            "empty message should not produce notification"
        );
    }

    // ─── decrqm_pm tests ─────────────────────────────────────────────────────

    #[test]
    fn decrqm_pm_unknown_mode_returns_0() {
        let flags = TermMode::empty();
        assert_eq!(decrqm_pm(999, &flags), 0);
    }

    #[test]
    fn decrqm_pm_show_cursor_set() {
        let flags = TermMode::SHOW_CURSOR;
        assert_eq!(decrqm_pm(25, &flags), 1);
    }

    #[test]
    fn decrqm_pm_show_cursor_reset() {
        let flags = TermMode::empty();
        assert_eq!(decrqm_pm(25, &flags), 2);
    }

    #[test]
    fn decrqm_pm_bracketed_paste_set() {
        let flags = TermMode::BRACKETED_PASTE;
        assert_eq!(decrqm_pm(2004, &flags), 1);
    }

    #[test]
    fn decrqm_pm_alt_screen_reset() {
        let flags = TermMode::SHOW_CURSOR | TermMode::LINE_WRAP;
        assert_eq!(decrqm_pm(1049, &flags), 2);
    }
}
