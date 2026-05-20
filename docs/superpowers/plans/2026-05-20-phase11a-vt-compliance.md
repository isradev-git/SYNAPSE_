# Phase 11a — VT OSC Compliance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire OSC 52 clipboard, OSC 133 semantic marks + jump navigation, OSC 9/777 desktop notifications, bell visual flash, DECSCUSR cursor shape override, DECRQM forwarding, and bracketed paste newline sanitization into the SYNAPSE_ terminal.

**Architecture:** Two integration paths: (1) alacritty_terminal already emits `Event::Bell`, `Event::ClipboardStore`, `Event::ClipboardLoad`, and `Event::PtyWrite` into `Pane::event_rx` — extend `poll_events` to handle them; (2) For OSC 133 and OSC 9/777 (not handled by alacritty), add raw byte scanners in the PTY reader thread following the existing `extract_osc7_paths` / `osc7_rx` pattern in `pane_ops.rs`. All per-pane drains (bell, clipboard, notifications) run inside `AppCore::render()` where `self.clipboard` is accessible.

**Tech Stack:** Rust, alacritty_terminal 0.24, arboard (already in `AppCore::clipboard`), notify-rust 4 (new dep, Linux dbus + macOS NSUserNotification). All files in `crates/SYNAPSE_-app` or `crates/SYNAPSE_-ui` or `crates/SYNAPSE_-config`.

---

## File Map

| File | Change |
|------|--------|
| `crates/SYNAPSE_-ui/src/pane.rs` | +4 fields, 2 new types, poll_events extension, osc133/osc9 drain |
| `crates/SYNAPSE_-app/src/pane_ops.rs` | +2 byte scanners, +2 mpsc channels, wire into reader + Pane::new |
| `crates/SYNAPSE_-app/src/state.rs` | +`bell_flash_end`, +`window_focused` |
| `crates/SYNAPSE_-app/src/render.rs` | bell drain, clipboard drain, notification drain in render(), border red on flash, DECSCUSR in push_cursor_rect |
| `crates/SYNAPSE_-config/src/config.rs` | +`BellConfig` struct, +`bell` field on Config |
| `crates/SYNAPSE_-config/src/keybinds.rs` | +`JumpPrevMark`, `JumpNextMark` actions + default bindings |
| `crates/SYNAPSE_-app/src/keyboard.rs` | +`JumpPrevMark`/`JumpNextMark` handling, bracketed paste sanitize |
| `crates/SYNAPSE_-app/src/app.rs` | update `handle_focus` to set `window_focused` |
| `crates/SYNAPSE_-app/Cargo.toml` | +`notify-rust = "4"` |

---

## Task 1: Pane types + poll_events extension (Bell, Clipboard, PtyWrite)

**Files:**
- Modify: `crates/SYNAPSE_-ui/src/pane.rs`

Context: `Pane::poll_events()` at line ~131 currently handles only `Event::Exit`, `Event::ChildExit`, `Event::Title`. `Event::Bell`, `Event::ClipboardStore`, `Event::ClipboardLoad`, `Event::PtyWrite` are silently dropped. `alacritty_terminal::event::Event` is already imported.

- [ ] **Step 1: Write failing tests**

Add to the `#[cfg(test)]` module at the bottom of `pane.rs` (create it if needed):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_op_write_variant() {
        // ClipboardOp::Write holds kind and string
        let op = ClipboardOp::Write(
            alacritty_terminal::event::ClipboardType::Clipboard,
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
}
```

- [ ] **Step 2: Run to confirm FAIL**

```bash
~/.cargo/bin/cargo test -p synapse-ui -- tests 2>&1 | tail -20
```

Expected: compile error — `ClipboardOp`, `MarkKind`, `SemanticMark` not found.

- [ ] **Step 3: Add new types and Pane fields**

Add to `pane.rs` after the existing imports, before `pub struct Pane`:

```rust
/// A clipboard operation queued from OSC 52 (alacritty event).
pub enum ClipboardOp {
    Write(alacritty_terminal::event::ClipboardType, String),
    /// fmt: alacritty-provided formatter that encodes the OSC 52 response bytes.
    Read(
        alacritty_terminal::event::ClipboardType,
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
```

Add to `Pane` struct (after `osc7_rx` field):

```rust
    pub pending_bell: bool,
    pub clipboard_pending: std::collections::VecDeque<ClipboardOp>,
    pub notifications: std::collections::VecDeque<String>,
    pub semantic_marks: Vec<SemanticMark>,
```

Initialize in `Pane::new` body (before `Self { ... }`):

```rust
        pending_bell: false,
        clipboard_pending: std::collections::VecDeque::new(),
        notifications: std::collections::VecDeque::new(),
        semantic_marks: Vec::new(),
```

- [ ] **Step 4: Extend poll_events**

In `poll_events`, inside the `while let Ok(event) = self.event_rx.try_recv()` loop, add new arms before the `_ => {}` wildcard:

```rust
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
```

- [ ] **Step 5: Run tests to confirm PASS**

```bash
~/.cargo/bin/cargo test -p synapse-ui -- tests 2>&1 | tail -20
```

Expected: `test result: ok. 3 passed`

- [ ] **Step 6: Run full workspace tests + clippy**

```bash
~/.cargo/bin/cargo test --workspace 2>&1 | tail -10
~/.cargo/bin/cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -20
```

Expected: all tests pass, no clippy warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/SYNAPSE_-ui/src/pane.rs
git commit -m "feat(ui): add ClipboardOp/MarkKind/SemanticMark types + extend poll_events for Bell/Clipboard/PtyWrite"
```

---

## Task 2: OSC 133 byte scanner + channel

**Files:**
- Modify: `crates/SYNAPSE_-app/src/pane_ops.rs`
- Modify: `crates/SYNAPSE_-ui/src/pane.rs`

Context: OSC 133 format is `ESC ] 133 ; <kind> BEL` or `ESC ] 133 ; <kind> ESC \`. Kinds: `A`=PromptStart, `B`=CommandStart, `C`/`D`=CommandEnd. Follow the exact pattern of `extract_osc7_paths` (lines 17-58 in pane_ops.rs) and `osc7_rx` channel wiring.

- [ ] **Step 1: Write failing tests**

Add to `pane_ops.rs` test module at bottom:

```rust
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
        assert_eq!(marks[0], MarkKind::PromptStart);
    }

    #[test]
    fn test_extract_osc133_command_start() {
        let bytes = b"\x1b]133;B\x07";
        let marks = extract_osc133_marks(bytes);
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0], MarkKind::CommandStart);
    }

    #[test]
    fn test_extract_osc133_command_end_c() {
        let bytes = b"\x1b]133;C\x07";
        let marks = extract_osc133_marks(bytes);
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0], MarkKind::CommandEnd);
    }

    #[test]
    fn test_extract_osc133_command_end_d_with_code() {
        // D;1 = CommandEnd with exit code 1
        let bytes = b"\x1b]133;D;1\x07";
        let marks = extract_osc133_marks(bytes);
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0], MarkKind::CommandEnd);
    }

    #[test]
    fn test_extract_osc133_st_terminator() {
        // ESC \ (ST) terminator instead of BEL
        let bytes = b"\x1b]133;A\x1b\\";
        let marks = extract_osc133_marks(bytes);
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0], MarkKind::PromptStart);
    }

    #[test]
    fn test_extract_osc133_multiple() {
        let bytes = b"\x1b]133;A\x07some output\x1b]133;B\x07";
        let marks = extract_osc133_marks(bytes);
        assert_eq!(marks.len(), 2);
        assert_eq!(marks[0], MarkKind::PromptStart);
        assert_eq!(marks[1], MarkKind::CommandStart);
    }

    #[test]
    fn test_extract_osc133_unknown_kind_ignored() {
        let bytes = b"\x1b]133;Z\x07";
        let marks = extract_osc133_marks(bytes);
        assert!(marks.is_empty());
    }
}
```

- [ ] **Step 2: Run to confirm FAIL**

```bash
~/.cargo/bin/cargo test -p synapse-app -- tests 2>&1 | tail -20
```

Expected: compile error — `extract_osc133_marks` not found.

- [ ] **Step 3: Implement extract_osc133_marks in pane_ops.rs**

Add after `extract_osc7_paths` function (around line 58):

```rust
/// Scan `bytes` for OSC 133 sequences and return the mark kinds found.
/// OSC 133 format: `ESC ] 133 ; <kind>[;<data>] BEL|ST`
/// Kinds: A=PromptStart, B=CommandStart, C/D=CommandEnd.
fn extract_osc133_marks(bytes: &[u8]) -> Vec<synapse_ui::pane::MarkKind> {
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
                // kind is bytes[start..start+1], rest (;data) is optional
                if term_pos > start {
                    let kind_byte = bytes[start];
                    let mark = match kind_byte {
                        b'A' => Some(MarkKind::PromptStart),
                        b'B' => Some(MarkKind::CommandStart),
                        b'C' | b'D' => Some(MarkKind::CommandEnd),
                        _ => None,
                    };
                    if let Some(k) = mark {
                        results.push(k);
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
```

- [ ] **Step 4: Add osc133_rx field to Pane and update Pane::new**

In `crates/SYNAPSE_-ui/src/pane.rs`, add field to `Pane` struct (after `osc7_rx`):

```rust
    pub osc133_rx: mpsc::Receiver<synapse_ui::pane::SemanticMark>,
```

Wait — `Pane` is IN `pane.rs` so use `mpsc::Receiver<SemanticMark>` (no module prefix):

```rust
    pub osc133_rx: mpsc::Receiver<SemanticMark>,
```

Update `Pane::new` signature to add:
```rust
        osc133_rx: mpsc::Receiver<SemanticMark>,
```

Update `Pane::new` body to initialize:
```rust
            osc133_rx,
```

- [ ] **Step 5: Wire osc133 channel in create_pane_full (pane_ops.rs)**

In `create_pane_full`, after the `osc7_rx` channel line (around line 164):

```rust
    let (osc133_tx, osc133_rx) = mpsc::sync_channel::<synapse_ui::pane::SemanticMark>(64);
```

In the reader thread body, after `extract_osc7_paths` call (after line 215):

```rust
                        for kind in extract_osc133_marks(&staging) {
                            let _ = osc133_tx.try_send(synapse_ui::pane::SemanticMark {
                                kind,
                                history_snapshot: 0, // corrected in poll_events under term lock
                            });
                        }
```

Update `Pane::new(...)` call at the bottom of `create_pane_full` to add `osc133_rx`:

```rust
    Ok(Pane::new(
        id,
        term,
        pty_writer_main,
        pty_master,
        event_rx,
        dirty,
        cols,
        rows,
        kitty_flags,
        kitty_active,
        kkp_rx,
        apc_rx,
        osc7_rx,
        osc133_rx,  // NEW
    ))
```

- [ ] **Step 6: Drain osc133_rx in poll_events**

In `pane.rs`, after the `osc7_rx` drain block in `poll_events`, add:

```rust
        use alacritty_terminal::grid::Dimensions;
        while let Ok(mut mark) = self.osc133_rx.try_recv() {
            if let Ok(term) = self.term.lock() {
                mark.history_snapshot = term.grid().history_size();
            }
            self.semantic_marks.push(mark);
            if self.semantic_marks.len() > 500 {
                self.semantic_marks.remove(0);
            }
        }
```

Note: `alacritty_terminal::grid::Dimensions` is required for `.history_size()` on Grid — add it to the `use` statement at top of file if not already there.

- [ ] **Step 7: Run tests + clippy**

```bash
~/.cargo/bin/cargo test --workspace 2>&1 | tail -15
~/.cargo/bin/cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -20
```

Expected: all pass. `test_extract_osc133_*` tests all green.

- [ ] **Step 8: Commit**

```bash
git add crates/SYNAPSE_-app/src/pane_ops.rs crates/SYNAPSE_-ui/src/pane.rs
git commit -m "feat(app): OSC 133 semantic marks — byte scanner + channel + poll_events drain"
```

---

## Task 3: OSC 9/777 byte scanner + channel

**Files:**
- Modify: `crates/SYNAPSE_-app/src/pane_ops.rs`
- Modify: `crates/SYNAPSE_-ui/src/pane.rs`

Context: OSC 9 format: `ESC ] 9 ; <message> BEL|ST` → notification body, title = "SYNAPSE_". OSC 777 format: `ESC ] 777 ; notify ; <title> ; <body> BEL|ST` → title + body separated by `\x00` in the string. Stored in `pane.notifications` as `String`. Caller splits on `\x00`.

- [ ] **Step 1: Write failing tests**

Add to `pane_ops.rs` test module:

```rust
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
        // Encoded as "My Title\x00The body text"
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
        assert!(notifs.is_empty(), "empty message should not produce notification");
    }
```

- [ ] **Step 2: Run to confirm FAIL**

```bash
~/.cargo/bin/cargo test -p synapse-app -- test_extract_osc9 2>&1 | tail -10
```

Expected: compile error — `extract_osc9_notifications` not found.

- [ ] **Step 3: Implement extract_osc9_notifications in pane_ops.rs**

Add after `extract_osc133_marks`:

```rust
/// Scan `bytes` for OSC 9 and OSC 777 notification sequences.
///
/// OSC 9 ; <message> BEL|ST  → returns "<message>" (title is "SYNAPSE_" on display).
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
                        // Strip leading "notify;" prefix if present
                        let payload = payload.strip_prefix("notify;").unwrap_or(payload);
                        // Split on first ';' to get title + body
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
```

- [ ] **Step 4: Add osc9_rx field to Pane + update Pane::new**

In `crates/SYNAPSE_-ui/src/pane.rs`, add to Pane struct:

```rust
    pub osc9_rx: mpsc::Receiver<String>,
```

Update `Pane::new` signature to add at end:
```rust
        osc9_rx: mpsc::Receiver<String>,
```

Update `Pane::new` body:
```rust
            osc9_rx,
```

- [ ] **Step 5: Wire osc9 channel in create_pane_full**

In `create_pane_full` (pane_ops.rs), after osc133 channel line:

```rust
    let (osc9_tx, osc9_rx) = mpsc::sync_channel::<String>(32);
```

In reader thread, after osc133 scanner block:

```rust
                        for notif in extract_osc9_notifications(&staging) {
                            let _ = osc9_tx.try_send(notif);
                        }
```

Update `Pane::new(...)` call to add `osc9_rx`:

```rust
    Ok(Pane::new(
        id, term, pty_writer_main, pty_master, event_rx, dirty,
        cols, rows, kitty_flags, kitty_active, kkp_rx, apc_rx,
        osc7_rx, osc133_rx, osc9_rx,  // osc9_rx is new
    ))
```

- [ ] **Step 6: Drain osc9_rx in poll_events**

In `pane.rs`, after the osc133 drain block:

```rust
        while let Ok(notif) = self.osc9_rx.try_recv() {
            self.notifications.push_back(notif);
        }
```

- [ ] **Step 7: Run tests + clippy**

```bash
~/.cargo/bin/cargo test --workspace 2>&1 | tail -15
~/.cargo/bin/cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -20
```

Expected: all pass. 5 osc9 tests green.

- [ ] **Step 8: Commit**

```bash
git add crates/SYNAPSE_-app/src/pane_ops.rs crates/SYNAPSE_-ui/src/pane.rs
git commit -m "feat(app): OSC 9/777 notification scanner + channel + poll_events drain"
```

---

## Task 4: Bell flash (state + render + app)

**Files:**
- Modify: `crates/SYNAPSE_-app/src/state.rs`
- Modify: `crates/SYNAPSE_-app/src/render.rs`
- Modify: `crates/SYNAPSE_-app/src/app.rs`

Context: `AppState` is in `state.rs`. `AppCore::handle_focus` is at line 289 in `app.rs`. The active pane border color is computed at ~line 794 in `render.rs`. Bell drain goes in `AppCore::render()` at ~line 1278 in `render.rs` (alongside the APC drain loop). `BellConfig` (Task 8) gating is added in Task 8 — for now hard-code `visual = true` check so bell always flashes; Task 8 replaces the hard-code with config field.

- [ ] **Step 1: Write failing test**

Add to `state.rs` test module:

```rust
    #[test]
    fn test_bell_flash_end_initial_none() {
        let state = make_state();
        assert!(state.bell_flash_end.is_none());
    }

    #[test]
    fn test_bell_flash_active() {
        let mut state = make_state();
        state.bell_flash_end = Some(std::time::Instant::now() + std::time::Duration::from_millis(200));
        assert!(state.bell_flash_end.map(|t| t > std::time::Instant::now()).unwrap_or(false));
    }
```

- [ ] **Step 2: Run to confirm FAIL**

```bash
~/.cargo/bin/cargo test -p synapse-app -- test_bell_flash 2>&1 | tail -10
```

Expected: compile error — `bell_flash_end` field not found.

- [ ] **Step 3: Add fields to AppState**

In `state.rs`, add two fields to `AppState` struct after `cursor_trail`:

```rust
    pub bell_flash_end: Option<std::time::Instant>,
    pub window_focused: bool,
```

In `AppState::new`, add to the `Self { ... }` block:

```rust
            bell_flash_end: None,
            window_focused: true,
```

- [ ] **Step 4: Update handle_focus in app.rs**

Find `fn handle_focus` at line ~289 in `app.rs`. It currently exists (called from `WindowEvent::Focused`). Add `window_focused` update:

```rust
    fn handle_focus(&mut self, focused: bool) {
        self.state.window_focused = focused;
        // existing code (if any) stays
    }
```

- [ ] **Step 5: Add bell drain in AppCore::render (render.rs)**

In `render.rs`, in `AppCore::render()`, after the APC drain loop (after line ~1288, before `self.frame_count += 1`), add:

```rust
        // Drain bell signals from all panes.
        for pane in self.panes.iter_mut() {
            if pane.pending_bell {
                pane.pending_bell = false;
                self.state.bell_flash_end = Some(
                    std::time::Instant::now() + std::time::Duration::from_millis(200),
                );
                // Desktop notification if unfocused — added in Task 5.
            }
        }
```

- [ ] **Step 6: Add red border when bell flashing**

In `render.rs`, at ~line 794, replace:

```rust
                let border_color = if is_active {
                    if state.effects_enabled && state.config.effects.pane_pulse {
                        let pulse = (time_secs * std::f32::consts::PI).sin() * 0.5 + 0.5;
                        let alpha = 0.6 + pulse * 0.4;
                        let c = state.theme.panel_active_border;
                        [c[0], c[1], c[2], alpha]
                    } else {
                        state.theme.panel_active_border
                    }
                } else {
                    state.theme.panel_inactive_border
                };
```

with:

```rust
                let border_color = if is_active {
                    let bell_active = state
                        .bell_flash_end
                        .map(|t| t > std::time::Instant::now())
                        .unwrap_or(false);
                    if bell_active {
                        [1.0_f32, 0.0, 0.047, 1.0] // #FF000C cyberpunk red
                    } else if state.effects_enabled && state.config.effects.pane_pulse {
                        let pulse = (time_secs * std::f32::consts::PI).sin() * 0.5 + 0.5;
                        let alpha = 0.6 + pulse * 0.4;
                        let c = state.theme.panel_active_border;
                        [c[0], c[1], c[2], alpha]
                    } else {
                        state.theme.panel_active_border
                    }
                } else {
                    state.theme.panel_inactive_border
                };
```

- [ ] **Step 7: Run tests + clippy**

```bash
~/.cargo/bin/cargo test --workspace 2>&1 | tail -15
~/.cargo/bin/cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -20
```

Expected: all pass, no warnings.

- [ ] **Step 8: Commit**

```bash
git add crates/SYNAPSE_-app/src/state.rs crates/SYNAPSE_-app/src/render.rs crates/SYNAPSE_-app/src/app.rs
git commit -m "feat(app): bell visual flash — red pane border 200ms on bell signal"
```

---

## Task 5: Desktop notifications (OSC 9 drain + notify-rust)

**Files:**
- Modify: `crates/SYNAPSE_-app/Cargo.toml`
- Modify: `crates/SYNAPSE_-app/src/render.rs`

Context: `pane.notifications` is a `VecDeque<String>`. Each entry is either `"body_only"` (OSC 9) or `"title\x00body"` (OSC 777). Drain happens in `AppCore::render()`. Only send notification if `!self.state.window_focused`.

- [ ] **Step 1: Add notify-rust dependency**

In `crates/SYNAPSE_-app/Cargo.toml`, add to `[dependencies]`:

```toml
notify-rust = "4"
```

- [ ] **Step 2: Verify it compiles**

```bash
~/.cargo/bin/cargo build -p synapse-app 2>&1 | tail -10
```

Expected: compiles (notify-rust downloads and builds).

If build fails with `dbus` linker error on Linux, install: `sudo apt install libdbus-1-dev pkg-config` and retry.

- [ ] **Step 3: Add send_notification helper and drain in render()**

In `render.rs`, add this free function near the top of the file (after the imports):

```rust
fn send_desktop_notification(title: &str, body: &str) {
    let _ = notify_rust::Notification::new()
        .summary(title)
        .body(body)
        .timeout(notify_rust::Timeout::Milliseconds(4000))
        .show();
}
```

Add `use notify_rust;` import at top of render.rs (or use fully qualified path).

In `AppCore::render()`, after the bell drain loop (Task 4), add the notification drain:

```rust
        // Drain desktop notifications from all panes.
        for pane in self.panes.iter_mut() {
            while let Some(raw) = pane.notifications.pop_front() {
                if !self.state.window_focused {
                    let (title, body) = if let Some(sep) = raw.find('\x00') {
                        (&raw[..sep], &raw[sep + 1..])
                    } else {
                        ("SYNAPSE_", raw.as_str())
                    };
                    send_desktop_notification(title, body);
                }
            }
        }
```

- [ ] **Step 4: Run tests + clippy**

```bash
~/.cargo/bin/cargo test --workspace 2>&1 | tail -15
~/.cargo/bin/cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -20
```

Expected: all pass. No clippy warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/SYNAPSE_-app/Cargo.toml crates/SYNAPSE_-app/src/render.rs
git commit -m "feat(app): OSC 9/777 desktop notifications via notify-rust (unfocused window only)"
```

---

## Task 6: OSC 52 clipboard (bidirectional)

**Files:**
- Modify: `crates/SYNAPSE_-app/src/render.rs`

Context: `AppCore` has `clipboard: Option<arboard::Clipboard>`. `pane.clipboard_pending` is `VecDeque<ClipboardOp>`. `ClipboardOp::Write` → set clipboard. `ClipboardOp::Read(_, fmt)` → read clipboard, call `fmt(&text)` to get the OSC 52 response, write response to PTY via `pane.pty_writer`. Drain in `AppCore::render()`.

- [ ] **Step 1: Write test**

Add to a test file in `crates/SYNAPSE_-ui/src/pane.rs` (can be added to existing test module):

```rust
    #[test]
    fn test_clipboard_op_read_has_formatter() {
        let fmt: std::sync::Arc<dyn Fn(&str) -> String + Send + Sync> =
            std::sync::Arc::new(|s: &str| format!("response:{}", s));
        let op = ClipboardOp::Read(
            alacritty_terminal::event::ClipboardType::Clipboard,
            fmt.clone(),
        );
        // Verify the formatter can be called
        if let ClipboardOp::Read(_, f) = op {
            assert_eq!(f("clipboard_text"), "response:clipboard_text");
        }
    }
```

- [ ] **Step 2: Run to confirm PASS** (this test should already pass once pane.rs types exist from Task 1)

```bash
~/.cargo/bin/cargo test -p synapse-ui -- test_clipboard_op_read_has_formatter 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 3: Add clipboard drain in AppCore::render()**

In `render.rs`, in `AppCore::render()`, after the notification drain (Task 5), add:

```rust
        // Drain OSC 52 clipboard operations.
        for pane in self.panes.iter_mut() {
            while let Some(op) = pane.clipboard_pending.pop_front() {
                match op {
                    synapse_ui::pane::ClipboardOp::Write(_, data) => {
                        if let Some(cb) = self.clipboard.as_mut() {
                            let _ = cb.set_text(data);
                        }
                    }
                    synapse_ui::pane::ClipboardOp::Read(_, fmt) => {
                        let text = self
                            .clipboard
                            .as_mut()
                            .and_then(|cb| cb.get_text().ok())
                            .unwrap_or_default();
                        let response = fmt(&text);
                        if let Ok(mut w) = pane.pty_writer.lock() {
                            let _ = std::io::Write::write_all(&mut *w, response.as_bytes());
                        }
                    }
                }
            }
        }
```

- [ ] **Step 4: Run tests + clippy**

```bash
~/.cargo/bin/cargo test --workspace 2>&1 | tail -10
~/.cargo/bin/cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -20
```

Expected: all pass, no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/SYNAPSE_-app/src/render.rs
git commit -m "feat(app): OSC 52 bidirectional clipboard (write + read+respond) via arboard"
```

---

## Task 7: DECSCUSR cursor shape override

**Files:**
- Modify: `crates/SYNAPSE_-app/src/render.rs`

Context: `push_cursor_rect` is at line ~422 in `render.rs`. It currently matches on `state.config.cursor_style` (the TOML config). alacritty_terminal processes `CSI Ps SP q` (DECSCUSR) internally and stores the result. Access via `term.cursor_style()` which returns `alacritty_terminal::vte::ansi::CursorStyle { shape: CursorShape, blinking: bool }`. When no DECSCUSR has been sent, `term.cursor_style()` returns the default (block, non-blinking or whatever alacritty's default is). The TOML config should still serve as the fallback when the terminal hasn't set a style.

`push_cursor_rect` receives `cursor_pixel: Option<(f32, f32)>` and `state: &mut AppState`, but not the term directly. The term is accessed via `pane.term` — but `push_cursor_rect` doesn't have access to the pane. The cursor shape override needs to be computed BEFORE calling `push_cursor_rect` and passed in, or read from AppState.

**Approach:** Store the app-set cursor shape in `AppState` as `AppState::term_cursor_style: Option<(CursorShape, bool)>` (shape, blinking). Update it in `render_frame` when reading from the active pane's term. Then `push_cursor_rect` reads it from `state`.

- [ ] **Step 1: Write failing test**

Add to `state.rs` test module:

```rust
    #[test]
    fn test_term_cursor_style_initial_none() {
        let state = make_state();
        assert!(state.term_cursor_style.is_none());
    }
```

- [ ] **Step 2: Run to confirm FAIL**

```bash
~/.cargo/bin/cargo test -p synapse-app -- test_term_cursor_style 2>&1 | tail -10
```

Expected: compile error — `term_cursor_style` not found on AppState.

- [ ] **Step 3: Add term_cursor_style to AppState**

In `state.rs`, add to `AppState` struct:

```rust
    /// Cursor shape/blink set by the app via DECSCUSR. Overrides TOML config when Some.
    pub term_cursor_style: Option<(alacritty_terminal::vte::ansi::CursorShape, bool)>,
```

In `AppState::new`, initialize:

```rust
            term_cursor_style: None,
```

Add import at top of `state.rs`:
```rust
use alacritty_terminal::vte::ansi::CursorShape;
```

- [ ] **Step 4: Read term.cursor_style() in render_frame**

In `render_frame` in `render.rs`, after the `poll_events` drain (around line 496), find where the active pane is accessed. Add code to capture cursor style into `state.term_cursor_style`:

```rust
    // Capture active pane's cursor style for DECSCUSR override.
    {
        let active_id = tab_bar.active_tab().active_pane;
        if let Some(pane) = panes.iter().find(|p| p.id == active_id) {
            if let Ok(term) = pane.term.lock() {
                let cs = term.cursor_style();
                // cursor_style() returns CursorStyle { shape, blinking }.
                // Only override when the app explicitly set a non-default shape.
                state.term_cursor_style = Some((cs.shape, cs.blinking));
            }
        }
    }
```

Note: if `term.cursor_style()` does not exist in alacritty_terminal 0.24, check `term.grid().cursor.shape` or consult `cargo doc -p alacritty_terminal --open`. Adapt accordingly.

- [ ] **Step 5: Use term_cursor_style in push_cursor_rect**

In `push_cursor_rect` (~line 452), replace:

```rust
    let color = state.theme.cursor;
    match state.config.cursor_style {
        synapse_config::CursorStyle::Block => {
            ui_rects.push(UIRect { pos: [cx, cy], size: [cell_w, cell_h], color });
        }
        synapse_config::CursorStyle::Beam => {
            ui_rects.push(UIRect { pos: [cx, cy], size: [1.5, cell_h], color });
        }
        synapse_config::CursorStyle::Underline => {
            ui_rects.push(UIRect { pos: [cx, cy + cell_h - 2.0], size: [cell_w, 2.0], color });
        }
    }
```

with:

```rust
    let color = state.theme.cursor;
    use alacritty_terminal::vte::ansi::CursorShape;
    let shape = state.term_cursor_style.map(|(s, _)| s);
    match shape {
        Some(CursorShape::Beam) => {
            ui_rects.push(UIRect { pos: [cx, cy], size: [1.5, cell_h], color });
        }
        Some(CursorShape::Underline) => {
            ui_rects.push(UIRect { pos: [cx, cy + cell_h - 2.0], size: [cell_w, 2.0], color });
        }
        Some(CursorShape::Hidden) => {
            // Hidden cursor: render nothing.
        }
        // Block or None (fallback to TOML config)
        _ => match state.config.cursor_style {
            synapse_config::CursorStyle::Block => {
                ui_rects.push(UIRect { pos: [cx, cy], size: [cell_w, cell_h], color });
            }
            synapse_config::CursorStyle::Beam => {
                ui_rects.push(UIRect { pos: [cx, cy], size: [1.5, cell_h], color });
            }
            synapse_config::CursorStyle::Underline => {
                ui_rects.push(UIRect { pos: [cx, cy + cell_h - 2.0], size: [cell_w, 2.0], color });
            }
        },
    }
```

- [ ] **Step 6: Run tests + clippy**

```bash
~/.cargo/bin/cargo test --workspace 2>&1 | tail -10
~/.cargo/bin/cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -20
```

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add crates/SYNAPSE_-app/src/state.rs crates/SYNAPSE_-app/src/render.rs
git commit -m "feat(app): DECSCUSR cursor shape override — read term.cursor_style() per frame"
```

---

## Task 8: BellConfig + JumpPrevMark/JumpNextMark + bracketed paste fix

**Files:**
- Modify: `crates/SYNAPSE_-config/src/config.rs`
- Modify: `crates/SYNAPSE_-config/src/keybinds.rs`
- Modify: `crates/SYNAPSE_-app/src/keyboard.rs`
- Modify: `crates/SYNAPSE_-app/src/render.rs` (gate bell on BellConfig)

Context: `Action` enum is in `keybinds.rs`. `handle_keyboard` is in `keyboard.rs` at line ~55. The active pane is accessed via `active_pane_mut(panes, tab_bar)` (from `pane_ops.rs`). `Scroll` is `alacritty_terminal::grid::Scroll`. `Dimensions` trait needed for `history_size()`.

Bracketed paste: in `keyboard.rs`, `Action::Paste` handling sends clipboard text. When `TermMode::BRACKETED_PASTE` is active, it wraps with `\x1b[200~` / `\x1b[201~`. Add newline sanitization inside the brackets.

- [ ] **Step 1: Write failing tests**

Add to `crates/SYNAPSE_-config/src/keybinds.rs` test module:

```rust
    #[test]
    fn test_jump_prev_mark_action_from_str() {
        assert_eq!(Action::from_str("jump_prev_mark"), Some(Action::JumpPrevMark));
    }

    #[test]
    fn test_jump_next_mark_action_from_str() {
        assert_eq!(Action::from_str("jump_next_mark"), Some(Action::JumpNextMark));
    }

    #[test]
    fn test_jump_marks_default_bindings() {
        let kb = Keybinds::default();
        let has_prev = kb.bindings().any(|(_, a)| a == Action::JumpPrevMark);
        let has_next = kb.bindings().any(|(_, a)| a == Action::JumpNextMark);
        assert!(has_prev, "JumpPrevMark must have a default binding");
        assert!(has_next, "JumpNextMark must have a default binding");
    }
```

Add to `crates/SYNAPSE_-config/src/config.rs` test module:

```rust
    #[test]
    fn test_bell_config_default() {
        let cfg = BellConfig::default();
        assert!(cfg.visual);
        assert!(cfg.notify_unfocused);
    }

    #[test]
    fn test_config_has_bell_field() {
        let cfg = Config::default();
        assert!(cfg.bell.visual);
    }
```

Add to `crates/SYNAPSE_-app/src/keyboard.rs` test module (create if not exists):

```rust
    #[test]
    fn test_bracketed_paste_newline_sanitize() {
        let text = "line1\r\nline2\nline3";
        let sanitized = sanitize_paste(text);
        assert_eq!(sanitized, "line1\rline2\rline3");
    }
```

- [ ] **Step 2: Run to confirm FAIL**

```bash
~/.cargo/bin/cargo test -p synapse-config -- test_jump test_bell 2>&1 | tail -15
~/.cargo/bin/cargo test -p synapse-app -- test_bracketed 2>&1 | tail -10
```

Expected: compile errors for missing Action variants, BellConfig, sanitize_paste.

- [ ] **Step 3: Add BellConfig to config.rs**

In `crates/SYNAPSE_-config/src/config.rs`, after `EffectsConfig` import, add:

```rust
/// Configuration for the terminal bell (BEL character / \a).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BellConfig {
    /// Flash the active pane border red for 200ms on bell. Default true.
    #[serde(default = "default_true")]
    pub visual: bool,
    /// Send a desktop notification when the bell fires and the window is not focused. Default true.
    #[serde(default = "default_true")]
    pub notify_unfocused: bool,
}

fn default_true() -> bool { true }

impl Default for BellConfig {
    fn default() -> Self {
        Self { visual: true, notify_unfocused: true }
    }
}
```

Add to `Config` struct:

```rust
    #[serde(default)]
    pub bell: BellConfig,
```

Add to `Config::default()`:

```rust
            bell: BellConfig::default(),
```

Add `pub use config::BellConfig;` to `crates/SYNAPSE_-config/src/lib.rs`.

- [ ] **Step 4: Add Action variants and default bindings to keybinds.rs**

In `Action` enum, add after `EffectsToggle`:

```rust
    JumpPrevMark,
    JumpNextMark,
```

In `Action::from_str`, add:

```rust
            "jump_prev_mark" => Some(Action::JumpPrevMark),
            "jump_next_mark" => Some(Action::JumpNextMark),
```

In `default_entries()`, add at end (before closing bracket):

```rust
        KeyBindEntry {
            key: "Up".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "jump_prev_mark".into(),
        },
        KeyBindEntry {
            key: "Down".into(),
            ctrl: true,
            shift: false,
            alt: false,
            action: "jump_next_mark".into(),
        },
```

Note: Ctrl+Up (no Shift) is distinct from Ctrl+Shift+Up (NavigateUp). Adding this binding will intercept Ctrl+Up from PTY programs. This is the intended behavior — users can override in TOML.

In the exhaustive `test_action_from_str_all_actions` test, add `"jump_prev_mark"` and `"jump_next_mark"` to the actions array.

- [ ] **Step 5: Add sanitize_paste helper and handle actions in keyboard.rs**

In `keyboard.rs`, add a helper function:

```rust
pub(crate) fn sanitize_paste(text: &str) -> String {
    text.replace("\r\n", "\r").replace('\n', "\r")
}
```

Apply it in the `Action::Paste` branch. Find where paste sends bracketed text (keyboard.rs ~line 363-368 and 528-533). Wrap the pasted text before sending:

```rust
// Before: writes `text` inside brackets
// After: writes sanitized text
let sanitized = sanitize_paste(&text);
// use `sanitized` instead of `text` inside the brackets
```

Find the two paste send sites (one for bracketed, one for plain) and apply sanitization to both. The bracketed paste path should apply sanitization before writing. The non-bracketed path can also sanitize `\r\n` → `\r` (harmless for non-bracketed).

- [ ] **Step 6: Add JumpPrevMark / JumpNextMark handling in handle_keyboard (keyboard.rs)**

Add `PostKeyAction::JumpMark` is NOT needed — the jump can be done inline since `handle_keyboard` already has mutable access to `panes`. Add inside the `Some(action)` match:

```rust
            Some(Action::JumpPrevMark) => {
                use alacritty_terminal::grid::{Dimensions, Scroll};
                let active_id = tab_bar.active_tab().active_pane;
                if let Some(pane) = panes.iter_mut().find(|p| p.id == active_id) {
                    if let Ok(term) = pane.term.lock() {
                        let cur = term.grid().display_offset();
                        let cur_hist = term.grid().history_size();
                        drop(term);
                        // Each mark's effective_offset = lines that scrolled into history since mark fired
                        let target = pane
                            .semantic_marks
                            .iter()
                            .filter(|m| {
                                matches!(
                                    m.kind,
                                    synapse_ui::pane::MarkKind::PromptStart
                                        | synapse_ui::pane::MarkKind::CommandStart
                                )
                            })
                            .filter_map(|m| {
                                let eff = cur_hist.saturating_sub(m.history_snapshot);
                                if eff > cur { Some(eff) } else { None }
                            })
                            .min(); // closest mark above current (smallest offset > cur)
                        if let Some(t) = target {
                            let delta = t as i32 - cur as i32;
                            pane.scroll_viewport(Scroll::Delta(delta));
                        }
                    }
                }
                return PostKeyAction::None;
            }
            Some(Action::JumpNextMark) => {
                use alacritty_terminal::grid::{Dimensions, Scroll};
                let active_id = tab_bar.active_tab().active_pane;
                if let Some(pane) = panes.iter_mut().find(|p| p.id == active_id) {
                    if let Ok(term) = pane.term.lock() {
                        let cur = term.grid().display_offset();
                        let cur_hist = term.grid().history_size();
                        drop(term);
                        let target = pane
                            .semantic_marks
                            .iter()
                            .filter(|m| {
                                matches!(
                                    m.kind,
                                    synapse_ui::pane::MarkKind::PromptStart
                                        | synapse_ui::pane::MarkKind::CommandStart
                                )
                            })
                            .filter_map(|m| {
                                let eff = cur_hist.saturating_sub(m.history_snapshot);
                                if eff < cur { Some(eff) } else { None }
                            })
                            .max(); // closest mark below (largest offset < cur)
                        if let Some(t) = target {
                            let delta = t as i32 - cur as i32;
                            pane.scroll_viewport(Scroll::Delta(delta));
                        }
                    }
                }
                return PostKeyAction::None;
            }
```

- [ ] **Step 7: Gate bell visual/notify on BellConfig**

In `render.rs`, update the bell drain from Task 4 to check `self.state.config.bell.visual`:

```rust
        for pane in self.panes.iter_mut() {
            if pane.pending_bell {
                pane.pending_bell = false;
                if self.state.config.bell.visual {
                    self.state.bell_flash_end = Some(
                        std::time::Instant::now() + std::time::Duration::from_millis(200),
                    );
                }
                if !self.state.window_focused && self.state.config.bell.notify_unfocused {
                    send_desktop_notification("SYNAPSE_", "Bell");
                }
            }
        }
```

- [ ] **Step 8: Run all tests + clippy**

```bash
~/.cargo/bin/cargo test --workspace 2>&1 | tail -20
~/.cargo/bin/cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -20
```

Expected: all tests pass. No clippy warnings.

- [ ] **Step 9: Commit**

```bash
git add crates/SYNAPSE_-config/src/config.rs crates/SYNAPSE_-config/src/keybinds.rs crates/SYNAPSE_-config/src/lib.rs crates/SYNAPSE_-app/src/keyboard.rs crates/SYNAPSE_-app/src/render.rs
git commit -m "feat(app): JumpPrevMark/JumpNextMark OSC 133 navigation + BellConfig TOML + bracketed paste newline sanitize"
```

---

## Final verification

- [ ] **Run full workspace tests**

```bash
~/.cargo/bin/cargo test --workspace 2>&1 | tail -20
```

Expected: all tests pass (baseline was 132).

- [ ] **Run clippy**

```bash
~/.cargo/bin/cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -20
```

Expected: no warnings.

- [ ] **Release build**

```bash
~/.cargo/bin/cargo build --release -p synapse-app 2>&1 | tail -5
```

Expected: `Finished release profile`.

- [ ] **Smoke test OSC 133** (requires zsh/bash configured with OSC 133)

Add to `~/.zshrc` or `~/.bashrc`:
```bash
# OSC 133 prompt marks
precmd() { printf '\033]133;A\007'; }
preexec() { printf '\033]133;B\007'; }
```

Reload shell, run several commands, then press Ctrl+Up/Down to jump between prompt marks.

- [ ] **Smoke test bell**

```bash
printf '\a'
```

Expected: active pane border flashes red for ~200ms.

- [ ] **Smoke test OSC 52** (SSH clipboard)

```bash
printf '\033]52;c;%s\007' "$(echo -n 'test clipboard' | base64)"
```

Expected: "test clipboard" appears in system clipboard.
