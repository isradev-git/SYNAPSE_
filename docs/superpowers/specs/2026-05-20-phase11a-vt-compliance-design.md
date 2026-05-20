# Phase 11a — VT OSC Compliance Design

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Wire OSC 52 clipboard, OSC 133 semantic marks, OSC 9/777 desktop notifications, bell visual flash, DECSCUSR cursor shape, DECRQM forwarding, and bracketed paste sanitization — all following the existing Pane event-channel pattern.

**Architecture:** Two integration paths. (1) alacritty_terminal already parses Bell, ClipboardStore, ClipboardLoad, PtyWrite into `Event::*` and queues them to `Pane::event_rx` — extend `poll_events` to handle them. (2) For OSC 133 and OSC 9/777 (not handled by alacritty), add raw byte scanners in the PTY reader thread following the existing `extract_osc7_paths` / `osc7_rx` pattern.

**Tech Stack:** Rust, alacritty_terminal 0.24, arboard (existing clipboard backend), notify-rust (new dep, Linux dbus + macOS NSUserNotification).

---

## 1. Data Types (pane.rs)

New types added to `synapse_ui::pane`:

```rust
/// A clipboard operation queued from OSC 52.
pub enum ClipboardOp {
    Write(alacritty_terminal::event::ClipboardType, String),
    /// fmt: alacritty-provided formatter that encodes the OSC 52 response.
    Read(alacritty_terminal::event::ClipboardType, std::sync::Arc<dyn Fn(&str) -> String + Send + Sync>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum MarkKind {
    PromptStart,   // OSC 133 ; A
    CommandStart,  // OSC 133 ; B
    CommandEnd,    // OSC 133 ; C or D
}

#[derive(Debug, Clone)]
pub struct SemanticMark {
    pub kind: MarkKind,
    /// Terminal grid row. Negative = scrollback. Set to 0 in byte scanner;
    /// corrected to actual cursor row when the mark is drained in poll_events.
    pub row: i32,
}
```

## 2. Pane Struct Changes (pane.rs)

New fields added to `Pane`:

```rust
pub pending_bell: bool,
pub clipboard_pending: std::collections::VecDeque<ClipboardOp>,
pub notifications: std::collections::VecDeque<String>,
pub semantic_marks: Vec<SemanticMark>,  // capped at 500, ordered by insertion
pub osc133_rx: mpsc::Receiver<SemanticMark>,
pub osc9_rx: mpsc::Receiver<String>,
```

All initialized in `Pane::new` with empty/default values. `osc133_rx` and `osc9_rx` are wired from `pane_ops.rs` (same as `osc7_rx`).

## 3. poll_events Extension (pane.rs)

Add to the `while let Ok(event) = self.event_rx.try_recv()` loop:

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

After draining `event_rx`, drain the two new channels:

```rust
while let Ok(mut mark) = self.osc133_rx.try_recv() {
    // Correct row using actual cursor position under term lock.
    if let Ok(term) = self.term.lock() {
        mark.row = term.grid().cursor.point.line.0;
    }
    self.semantic_marks.push(mark);
    if self.semantic_marks.len() > 500 {
        self.semantic_marks.remove(0);
    }
}
while let Ok(notif) = self.osc9_rx.try_recv() {
    self.notifications.push_back(notif);
}
```

`poll_events` signature stays `fn poll_events(&mut self) -> bool` — no breaking change.

## 4. Byte Scanners (pane_ops.rs)

### extract_osc133_marks

```rust
fn extract_osc133_marks(bytes: &[u8]) -> Vec<SemanticMark> {
    // Pattern: ESC ] 133 ; <A|B|C|D[;N]> BEL|ST
    // Returns marks with row=0 (corrected later in poll_events under term lock).
}
```

Supported mark types:
- `A` → `MarkKind::PromptStart`
- `B` → `MarkKind::CommandStart`
- `C` or `D` (with optional `;N` exit code) → `MarkKind::CommandEnd`

New channel in `create_pane_full`:

```rust
let (osc133_tx, osc133_rx) = mpsc::sync_channel::<SemanticMark>(64);
```

In reader thread, before `processor.advance`:

```rust
for mark in extract_osc133_marks(&staging) {
    let _ = osc133_tx.try_send(mark);
}
```

### extract_osc9_notifications

```rust
fn extract_osc9_notifications(bytes: &[u8]) -> Vec<String> {
    // OSC 9 ; <message> BEL|ST  → message
    // OSC 777 ; notify ; <title> ; <body> BEL|ST → "title\x00body" (null-separated)
    // Returns strings. Caller splits on '\0' to get title/body.
}
```

Encoding: single-string with `\x00` separator for OSC 777 (`"title\x00body"`); plain message for OSC 9 (`"title"` only, body = "").

New channel:

```rust
let (osc9_tx, osc9_rx) = mpsc::sync_channel::<String>(32);
```

## 5. Bell Flash (state.rs + render.rs + app.rs)

### state.rs

```rust
pub bell_flash_timer: f32,  // seconds remaining, decays each frame
pub window_focused: bool,   // updated via WindowEvent::Focused
```

Initialize: `bell_flash_timer: 0.0`, `window_focused: true`.

### app.rs — WindowEvent::Focused

```rust
WindowEvent::Focused(f) => self.core_mut().state.window_focused = f,
```

### app.rs — bell drain (each frame, after poll_events)

```rust
if pane.pending_bell {
    pane.pending_bell = false;
    if state.config.bell.visual {
        state.bell_flash_timer = 0.2;
    }
    if !state.window_focused && state.config.bell.notify_unfocused {
        // Fire desktop notification via notify-rust (see §6).
        send_notification("SYNAPSE_", "Bell");
    }
}
```

### render.rs — border color

In the active pane border rendering, replace `theme.panel_active_border` with:

```rust
let border_color = if state.bell_flash_timer > 0.0 {
    [1.0, 0.0, 0.047, 1.0]  // #FF000C red flash
} else {
    state.theme.panel_active_border
};
```

Bell timer decay: subtract `dt` each frame. `dt` computed from `start_time.elapsed()` (already on AppCore for postproc). Clamp to 0.

## 6. Desktop Notifications (app.rs)

Add to `SYNAPSE_-app/Cargo.toml`:

```toml
notify-rust = { version = "4", default-features = false, features = ["d"] }
# "d" = dbus (Linux). On macOS, use features = ["mac_notification_sys"].
```

Platform-conditional features:

```toml
[target.'cfg(target_os = "linux")'.dependencies]
notify-rust = { version = "4", default-features = false, features = ["d"] }

[target.'cfg(target_os = "macos")'.dependencies]
notify-rust = { version = "4", default-features = false, features = ["mac_notification_sys"] }
```

Helper in `app.rs`:

```rust
fn send_notification(title: &str, body: &str) {
    let _ = notify_rust::Notification::new()
        .summary(title)
        .body(body)
        .timeout(notify_rust::Timeout::Milliseconds(4000))
        .show();
}
```

OSC 9 drain each frame:

```rust
while let Some(raw) = pane.notifications.pop_front() {
    if !state.window_focused {
        let (title, body) = if let Some(i) = raw.find('\x00') {
            (&raw[..i], &raw[i+1..])
        } else {
            (raw.as_str(), "")
        };
        send_notification(title, body);
    }
}
```

If `window_focused`, notifications are silently dropped (user sees output in terminal).

## 7. OSC 52 Clipboard (app.rs)

Reuse the existing `arboard::Clipboard` instance in `AppCore` (already used for `Action::Copy`/`Paste`).

Drain `pane.clipboard_pending` each frame:

```rust
while let Some(op) = pane.clipboard_pending.pop_front() {
    match op {
        ClipboardOp::Write(_, data) => {
            let _ = self.clipboard.set_text(data);
        }
        ClipboardOp::Read(_, fmt) => {
            let text = self.clipboard.get_text().unwrap_or_default();
            let response = fmt(&text);
            if let Ok(mut w) = pane.pty_writer.lock() {
                let _ = std::io::Write::write_all(&mut *w, response.as_bytes());
            }
        }
    }
}
```

No new dependency — `arboard` already in `SYNAPSE_-app`.

## 8. DECSCUSR — Cursor Shape (render.rs)

alacritty_terminal handles `CSI Ps SP q` internally. `term.cursor_style()` returns `CursorStyle { shape: CursorShape, blinking: bool }`.

In `push_cursor_rect` (render.rs), after acquiring the term lock, read:

```rust
let cursor_style = term.cursor_style();
// Override TOML config shape with app-set shape.
let shape = match cursor_style.shape {
    CursorShape::Block     => CursorKind::Block,
    CursorShape::Underline => CursorKind::Underline,
    CursorShape::Beam      => CursorKind::Beam,
    _                      => state.config.cursor_shape(),  // fallback to TOML
};
```

Blink: `cursor_style.blinking` overrides `config.cursor_blink`.

DECSCUSR values handled by alacritty (0-6) — no additional parsing needed in SYNAPSE_.

## 9. DECRQM Forwarding

alacritty_terminal responds to `CSI ? Pm $ p` queries by emitting `Event::PtyWrite(response)`. The `poll_events` extension in §3 already handles this — writes the response bytes directly to the PTY. No additional work.

## 10. Bracketed Paste — Newline Sanitization (keyboard.rs)

Current behavior: `Action::Paste` sends raw clipboard text wrapped in `\x1b[200~` / `\x1b[201~` when `TermMode::BRACKETED_PASTE` is active.

Fix: sanitize newlines inside the bracketed region:

```rust
// Inside bracketed paste send:
let sanitized = text.replace("\r\n", "\r").replace('\n', "\r");
```

This prevents vim/neovim from treating each newline as a separate Enter keypress in insert mode.

## 11. Keybinds + Config

### New Actions (keybinds.rs)

```rust
pub enum Action {
    // ... existing ...
    JumpPrevMark,  // Ctrl+Up
    JumpNextMark,  // Ctrl+Down
}
```

Default bindings:

```rust
// In Keybinds::default():
KeyCombo { key: "Up",   mods: Ctrl } → Action::JumpPrevMark
KeyCombo { key: "Down", mods: Ctrl } → Action::JumpNextMark
```

String serialization: `"jump_prev_mark"`, `"jump_next_mark"`.

### Jump Logic (keyboard.rs)

```rust
Action::JumpPrevMark => {
    // Find highest mark.row below display_offset → scroll to it.
    let current_row = pane.term.lock().map(|t| t.grid().display_offset() as i32).unwrap_or(0);
    if let Some(mark) = pane.semantic_marks.iter()
        .filter(|m| matches!(m.kind, MarkKind::PromptStart | MarkKind::CommandStart))
        .filter(|m| m.row < current_row)
        .max_by_key(|m| m.row)
    {
        // scroll to mark.row
    }
}
```

`JumpNextMark`: same but `m.row > current_row`, use `min_by_key`.

### BellConfig (config.rs)

```rust
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct BellConfig {
    #[serde(default = "default_true")]
    pub visual: bool,
    #[serde(default = "default_true")]
    pub notify_unfocused: bool,
}

impl Default for BellConfig {
    fn default() -> Self { Self { visual: true, notify_unfocused: true } }
}
```

Add `#[serde(default)] pub bell: BellConfig` to `Config`. App checks `config.bell.visual` before setting flash timer, `config.bell.notify_unfocused` before sending desktop notification.

Example TOML:

```toml
[bell]
visual = true
notify_unfocused = true
```

## 12. Task Summary

| Task | Files | Description |
|------|-------|-------------|
| T1 | `pane.rs` | New types (ClipboardOp, MarkKind, SemanticMark), +4 Pane fields, poll_events extension (Bell, ClipboardStore/Load, PtyWrite, osc133/osc9 drain) |
| T2 | `pane_ops.rs` | `extract_osc133_marks()` byte scanner + `osc133_tx/rx` channel |
| T3 | `pane_ops.rs` | `extract_osc9_notifications()` byte scanner + `osc9_tx/rx` channel |
| T4 | `state.rs`, `render.rs`, `app.rs` | Bell flash: `bell_flash_timer`, `window_focused`, red border, WindowEvent::Focused |
| T5 | `app.rs`, `Cargo.toml` | notify-rust dep, `send_notification()`, OSC 9 drain |
| T6 | `app.rs` | OSC 52 drain (arboard write + read+respond) |
| T7 | `render.rs` | DECSCUSR: read `term.cursor_style()`, override shape+blink |
| T8 | `keybinds.rs`, `keyboard.rs`, `config.rs` | JumpPrevMark/JumpNextMark actions, jump logic, BellConfig TOML, bracketed paste newline sanitize |

## 13. Testing

Each task ships with unit tests:

- T1: `test_poll_events_bell`, `test_poll_events_clipboard_write`, `test_clipboard_op_read_variant`
- T2: `test_extract_osc133_prompt_start`, `test_extract_osc133_command_end_with_code`, `test_osc133_cap_at_500`
- T3: `test_extract_osc9_simple`, `test_extract_osc777_title_body`, `test_osc9_null_separator`
- T4: `test_bell_flash_timer_decay`, `test_bell_flash_sets_timer`
- T5: integration test — mock notify call (feature-gated)
- T6: `test_osc52_write_drains_to_clipboard`, `test_osc52_read_calls_formatter`
- T7: `test_cursor_shape_beam`, `test_cursor_shape_block`, `test_cursor_blink_override`
- T8: `test_jump_prev_mark_finds_nearest`, `test_jump_next_mark_wraps`, `test_bracketed_paste_sanitize_newlines`, `test_bell_config_default`

## 14. Performance

All operations are O(n) over small bounded collections (≤500 marks, ≤32 notifications). No allocations in the hot render path — channels drained before render, fields read by reference. notify-rust calls happen off the hot path (only when unfocused + notification arrives). Zero overhead when no OSC sequences are emitted.
