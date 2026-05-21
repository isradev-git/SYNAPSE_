# Phase 11b — Copy Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add vim-style copy mode to SYNAPSE_ — enter with Ctrl+Shift+Space, move with hjkl/w/b/e, select with v/V, yank with y.

**Architecture:** `CopyModeState` lives in `Pane` (pane-local). `AppState::in_copy_mode: bool` gates keyboard routing. Selection reuses `term.selection` (existing render highlight). Copy mode cursor is an amber `UIRect` added in the render loop before the blink-cursor marker, so it survives blink-only redraws. `pane.dirty` is set on every cursor move to trigger rebuild.

**Tech Stack:** Rust, alacritty_terminal 0.24 (`Selection`, `SelectionType`, `Point`, `Side`), arboard (existing).

---

## File Map

| File | Change |
|------|--------|
| `crates/SYNAPSE_-ui/src/pane.rs` | Add `CopySelMode`, `CopyModeState`, `Pane::copy_mode` field |
| `crates/SYNAPSE_-app/src/state.rs` | Add `AppState::in_copy_mode` field |
| `crates/SYNAPSE_-config/src/keybinds.rs` | Add `ToggleCopyMode` action + default binding |
| `crates/SYNAPSE_-app/src/keyboard.rs` | Routing gate, enter/exit, `handle_copy_mode_key`, motion helpers |
| `crates/SYNAPSE_-app/src/render.rs` | Amber cursor UIRect in pane loop + early-return in `push_cursor_rect` |

---

## Task 1: Data Types

**Files:**
- Modify: `crates/SYNAPSE_-ui/src/pane.rs`
- Modify: `crates/SYNAPSE_-app/src/state.rs`

- [ ] **Step 1: Write failing tests in pane.rs**

Add to the `#[cfg(test)] mod tests` block at the bottom of `crates/SYNAPSE_-ui/src/pane.rs`:

```rust
#[test]
fn test_copy_sel_mode_eq() {
    assert_eq!(CopySelMode::None, CopySelMode::None);
    assert_ne!(CopySelMode::Char, CopySelMode::Line);
    assert_ne!(CopySelMode::None, CopySelMode::Char);
}

#[test]
fn test_copy_mode_state_fields() {
    let cms = CopyModeState {
        cursor: alacritty_terminal::index::Point::default(),
        anchor: None,
        sel_mode: CopySelMode::None,
    };
    assert!(cms.anchor.is_none());
    assert_eq!(cms.sel_mode, CopySelMode::None);
}
```

- [ ] **Step 2: Run to confirm fail**

```bash
cargo test -p SYNAPSE_-ui -- test_copy_sel_mode_eq test_copy_mode_state_fields 2>&1 | tail -10
```

Expected: `error[E0422]: cannot find struct/enum CopySelMode`

- [ ] **Step 3: Add CopySelMode and CopyModeState to pane.rs**

Add after the `KkpCommand` enum (around line 65), before `pub struct Pane`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum CopySelMode {
    None,
    Char,
    Line,
}

#[derive(Debug, Clone)]
pub struct CopyModeState {
    pub cursor: alacritty_terminal::index::Point,
    pub anchor: Option<alacritty_terminal::index::Point>,
    pub sel_mode: CopySelMode,
}
```

- [ ] **Step 4: Add copy_mode field to Pane struct**

In the `pub struct Pane` block, add after `semantic_marks`:

```rust
pub copy_mode: Option<CopyModeState>,
```

In `Pane::new`, add to the `Self { ... }` initializer after `semantic_marks: Vec::new(),`:

```rust
copy_mode: None,
```

- [ ] **Step 5: Write failing test in state.rs**

Add to the `#[cfg(test)] mod tests` block of `crates/SYNAPSE_-app/src/state.rs`:

```rust
#[test]
fn test_in_copy_mode_initial_false() {
    let state = make_state();
    assert!(!state.in_copy_mode);
}
```

- [ ] **Step 6: Run to confirm fail**

```bash
cargo test -p SYNAPSE_-app -- test_in_copy_mode_initial_false 2>&1 | tail -10
```

Expected: `error[E0609]: no field 'in_copy_mode'`

- [ ] **Step 7: Add in_copy_mode to AppState**

In `pub struct AppState` (around line 205 of state.rs), add after `term_cursor_style`:

```rust
pub in_copy_mode: bool,
```

In `AppState::new`, add after `term_cursor_style: None,`:

```rust
in_copy_mode: false,
```

- [ ] **Step 8: Run all new tests**

```bash
cargo test -p SYNAPSE_-ui -- test_copy_sel_mode_eq test_copy_mode_state_fields
cargo test -p SYNAPSE_-app -- test_in_copy_mode_initial_false
```

Expected: all PASS.

- [ ] **Step 9: Verify workspace compiles**

```bash
cargo build --workspace 2>&1 | tail -5
```

Expected: `Finished`

- [ ] **Step 10: Commit**

```bash
git add crates/SYNAPSE_-ui/src/pane.rs crates/SYNAPSE_-app/src/state.rs
git commit -m "feat(copy-mode): add CopySelMode, CopyModeState types and AppState::in_copy_mode"
```

---

## Task 2: ToggleCopyMode Action + Enter/Exit Logic

**Files:**
- Modify: `crates/SYNAPSE_-config/src/keybinds.rs`
- Modify: `crates/SYNAPSE_-app/src/keyboard.rs`

- [ ] **Step 1: Write failing tests in keybinds.rs**

Add to the `#[cfg(test)] mod tests` block of `crates/SYNAPSE_-config/src/keybinds.rs`:

```rust
#[test]
fn test_toggle_copy_mode_action_from_str() {
    assert_eq!(Action::from_str("toggle_copy_mode"), Some(Action::ToggleCopyMode));
}

#[test]
fn test_toggle_copy_mode_default_binding() {
    let kb = Keybinds::new();
    let action = kb.lookup(&Key::Named(NamedKey::Space), mods(true, true, false));
    assert_eq!(action, Some(Action::ToggleCopyMode));
}
```

- [ ] **Step 2: Run to confirm fail**

```bash
cargo test -p SYNAPSE_-config -- test_toggle_copy_mode 2>&1 | tail -10
```

Expected: errors about missing `ToggleCopyMode`

- [ ] **Step 3: Add ToggleCopyMode to keybinds.rs**

In the `pub enum Action` block, add after `JumpNextMark,`:

```rust
ToggleCopyMode,
```

In `Action::from_str`, add before the closing `_ => None`:

```rust
"toggle_copy_mode" => Some(Action::ToggleCopyMode),
```

In `default_entries()`, add at the end of the vec (before the closing bracket):

```rust
KeyBindEntry {
    key: "Space".into(),
    ctrl: true,
    shift: true,
    alt: false,
    action: "toggle_copy_mode".into(),
},
```

- [ ] **Step 4: Run keybinds tests**

```bash
cargo test -p SYNAPSE_-config -- test_toggle_copy_mode 2>&1 | tail -5
```

Expected: 2 tests PASS.

- [ ] **Step 5: Add new imports to keyboard.rs**

At the top of `crates/SYNAPSE_-app/src/keyboard.rs`, add after the existing `use alacritty_terminal` lines:

```rust
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use std::sync::atomic::Ordering;
use synapse_ui::pane::{CopyModeState, CopySelMode};
```

- [ ] **Step 6: Add enter_copy_mode and exit_copy_mode to keyboard.rs**

Add before `pub fn handle_keyboard`:

```rust
fn enter_copy_mode(pane: &mut Pane, state: &mut AppState) {
    use alacritty_terminal::grid::Dimensions;
    let cursor = pane.term.lock()
        .map(|t| t.grid().cursor.point)
        .unwrap_or_default();
    pane.copy_mode = Some(CopyModeState {
        cursor,
        anchor: None,
        sel_mode: CopySelMode::None,
    });
    state.in_copy_mode = true;
    pane.dirty.store(true, Ordering::Release);
}

fn exit_copy_mode(pane: &mut Pane, state: &mut AppState) {
    if let Ok(mut term) = pane.term.lock() {
        term.selection = None;
    }
    pane.copy_mode = None;
    state.in_copy_mode = false;
    pane.dirty.store(true, Ordering::Release);
}
```

- [ ] **Step 7: Add handle_copy_mode_key stub to keyboard.rs**

Add after `exit_copy_mode`:

```rust
fn handle_copy_mode_key(
    key: &winit::keyboard::Key,
    _modifiers: ModifiersState,
    pane: &mut Pane,
    state: &mut AppState,
    clipboard: &mut Option<arboard::Clipboard>,
) {
    use winit::keyboard::NamedKey;
    let key_char: Option<&str> = match key {
        winit::keyboard::Key::Character(c) => Some(c.as_str()),
        _ => None,
    };
    let is_escape = matches!(key, winit::keyboard::Key::Named(NamedKey::Escape));

    if is_escape || key_char == Some("q") || key_char == Some("Q") {
        exit_copy_mode(pane, state);
        return;
    }

    // Remaining keys handled in Tasks 3–5.
    let _ = clipboard; // suppress unused warning until T5
}
```

- [ ] **Step 8: Add routing gate to handle_keyboard**

In `pub fn handle_keyboard`, inside `if event.state == winit::event::ElementState::Pressed {`, right after `let is_repeat = event.repeat;` and before the existing `if !is_repeat {` block, insert:

```rust
// ToggleCopyMode: first press only, processed before the copy mode gate.
if !is_repeat {
    if let Some(Action::ToggleCopyMode) = state.keybinds.lookup(logical_key, state.modifiers) {
        let pane = active_pane_mut(panes, tab_bar);
        if state.in_copy_mode {
            exit_copy_mode(pane, state);
        } else {
            enter_copy_mode(pane, state);
        }
        return PostKeyAction::None;
    }
}

// Copy mode gate: consume all keypresses (including repeats) when active.
if state.in_copy_mode {
    let pane = active_pane_mut(panes, tab_bar);
    handle_copy_mode_key(logical_key, state.modifiers, pane, state, clipboard);
    return PostKeyAction::None;
}
```

- [ ] **Step 9: Compile**

```bash
cargo build -p SYNAPSE_-app 2>&1 | tail -10
```

Expected: `Finished` (no errors)

- [ ] **Step 10: Run all tests**

```bash
cargo test --workspace 2>&1 | tail -15
```

Expected: all pass.

- [ ] **Step 11: Commit**

```bash
git add crates/SYNAPSE_-config/src/keybinds.rs crates/SYNAPSE_-app/src/keyboard.rs
git commit -m "feat(copy-mode): ToggleCopyMode action, enter/exit logic, keyboard routing gate"
```

---

## Task 3: h/j/k/l Motion + Scroll-to-Follow

**Files:**
- Modify: `crates/SYNAPSE_-app/src/keyboard.rs`

**Background on coordinates:**  
`Point.line.0` = 0 at top of visible screen, increases downward, negative = scrollback history.  
`display_offset` = 0 when at bottom; positive = scrolled into history.  
`viewport_row = raw_row + display_offset`. Visible range: `[0, screen_lines)`.  
`Scroll::Delta(positive)` scrolls up into history; `Delta(negative)` scrolls toward bottom.

- [ ] **Step 1: Write failing tests in keyboard.rs**

Add to `#[cfg(test)] mod tests` in `crates/SYNAPSE_-app/src/keyboard.rs`:

```rust
#[test]
fn test_compute_moved_cursor_basic() {
    use alacritty_terminal::index::{Column, Line, Point};
    let start = Point::new(Line(5), Column(10));
    let moved = compute_moved_cursor(start, 1, 0, 80, 24, 100);
    assert_eq!(moved.column.0, 11);
    assert_eq!(moved.line.0, 5);

    let moved_down = compute_moved_cursor(start, 0, 1, 80, 24, 100);
    assert_eq!(moved_down.line.0, 6);
    assert_eq!(moved_down.column.0, 10);
}

#[test]
fn test_compute_moved_cursor_clamp() {
    use alacritty_terminal::index::{Column, Line, Point};
    // Right edge
    let right = Point::new(Line(0), Column(79));
    assert_eq!(compute_moved_cursor(right, 1, 0, 80, 24, 100).column.0, 79);
    // Left edge
    let left = Point::new(Line(0), Column(0));
    assert_eq!(compute_moved_cursor(left, -1, 0, 80, 24, 100).column.0, 0);
    // Bottom edge
    let bottom = Point::new(Line(23), Column(0));
    assert_eq!(compute_moved_cursor(bottom, 0, 1, 80, 24, 100).line.0, 23);
    // History top
    let hist_top = Point::new(Line(-100), Column(0));
    assert_eq!(compute_moved_cursor(hist_top, 0, -1, 80, 24, 100).line.0, -100);
}

#[test]
fn test_compute_scroll_delta_above() {
    // cursor at viewport_row = -3 → scroll up 3
    assert_eq!(compute_scroll_delta(-3, 24), 3);
}

#[test]
fn test_compute_scroll_delta_below() {
    // cursor at viewport_row = 25, screen_lines = 24 → scroll down 2
    assert_eq!(compute_scroll_delta(25, 24), -2);
}

#[test]
fn test_compute_scroll_delta_visible() {
    assert_eq!(compute_scroll_delta(12, 24), 0);
}
```

- [ ] **Step 2: Run to confirm fail**

```bash
cargo test -p SYNAPSE_-app -- test_compute_moved_cursor test_compute_scroll_delta 2>&1 | tail -10
```

Expected: errors — `compute_moved_cursor` and `compute_scroll_delta` not found.

- [ ] **Step 3: Add pure helper functions to keyboard.rs**

Add before `handle_copy_mode_key`:

```rust
fn compute_moved_cursor(
    cursor: Point,
    delta_col: i32,
    delta_row: i32,
    cols: i32,
    rows: i32,
    history: i32,
) -> Point {
    let new_col = (cursor.column.0 as i32 + delta_col).clamp(0, cols - 1);
    let new_row = (cursor.line.0 + delta_row).clamp(-history, rows - 1);
    Point::new(Line(new_row), Column(new_col as usize))
}

fn compute_scroll_delta(viewport_row: i32, screen_lines: i32) -> i32 {
    if viewport_row < 0 {
        -viewport_row
    } else if viewport_row >= screen_lines {
        screen_lines - 1 - viewport_row
    } else {
        0
    }
}

fn move_cursor(pane: &mut Pane, delta_col: i32, delta_row: i32) {
    use alacritty_terminal::grid::Dimensions;
    let (cols, rows, history) = {
        match pane.term.lock() {
            Ok(t) => (t.columns() as i32, t.screen_lines() as i32, t.grid().history_size() as i32),
            Err(_) => return,
        }
    };
    if let Some(ref mut cms) = pane.copy_mode {
        cms.cursor = compute_moved_cursor(cms.cursor, delta_col, delta_row, cols, rows, history);
    }
    pane.dirty.store(true, Ordering::Release);
}

fn scroll_to_follow_cursor(pane: &mut Pane) {
    use alacritty_terminal::grid::Dimensions;
    let raw_row = match pane.copy_mode.as_ref() {
        Some(cms) => cms.cursor.line.0,
        None => return,
    };
    let (display_offset, screen_lines) = {
        match pane.term.lock() {
            Ok(t) => (t.grid().display_offset() as i32, t.screen_lines() as i32),
            Err(_) => return,
        }
    };
    let viewport_row = raw_row + display_offset;
    let delta = compute_scroll_delta(viewport_row, screen_lines);
    if delta != 0 {
        pane.scroll_viewport(alacritty_terminal::grid::Scroll::Delta(delta));
    }
}
```

- [ ] **Step 4: Wire h/j/k/l into handle_copy_mode_key**

In `handle_copy_mode_key`, after the escape/q block, add:

```rust
match key_char {
    Some("h") => {
        move_cursor(pane, -1, 0);
        scroll_to_follow_cursor(pane);
    }
    Some("j") => {
        move_cursor(pane, 0, 1);
        scroll_to_follow_cursor(pane);
    }
    Some("k") => {
        move_cursor(pane, 0, -1);
        scroll_to_follow_cursor(pane);
    }
    Some("l") => {
        move_cursor(pane, 1, 0);
        scroll_to_follow_cursor(pane);
    }
    _ => {}
}
```

Replace the `let _ = clipboard;` placeholder with this match (keep `let _ = clipboard;` if clipboard isn't used yet in this task — it will be removed in T5).

- [ ] **Step 5: Run tests**

```bash
cargo test -p SYNAPSE_-app -- test_compute_moved_cursor test_compute_scroll_delta 2>&1 | tail -10
```

Expected: 5 tests PASS.

- [ ] **Step 6: Build check**

```bash
cargo build -p SYNAPSE_-app 2>&1 | tail -5
```

- [ ] **Step 7: Commit**

```bash
git add crates/SYNAPSE_-app/src/keyboard.rs
git commit -m "feat(copy-mode): hjkl motion + scroll-to-follow cursor"
```

---

## Task 4: w/b/e Word Motions

**Files:**
- Modify: `crates/SYNAPSE_-app/src/keyboard.rs`

**Rules:**  
Word char = `c.is_alphanumeric() || c == '_'`.  
Line wrapping: col reaches end → col=0, row+1. Row reaches bottom → stop.

- [ ] **Step 1: Write failing tests**

Add to `#[cfg(test)] mod tests`:

```rust
#[test]
fn test_is_word_char() {
    assert!(is_word_char('a'));
    assert!(is_word_char('Z'));
    assert!(is_word_char('5'));
    assert!(is_word_char('_'));
    assert!(!is_word_char(' '));
    assert!(!is_word_char('-'));
    assert!(!is_word_char('.'));
    assert!(!is_word_char('\0'));
}
```

- [ ] **Step 2: Run to confirm fail**

```bash
cargo test -p SYNAPSE_-app -- test_is_word_char 2>&1 | tail -5
```

Expected: `error[E0425]: cannot find function 'is_word_char'`

- [ ] **Step 3: Add word motion helpers to keyboard.rs**

Add before `handle_copy_mode_key`:

```rust
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn cell_char_at(
    term: &alacritty_terminal::term::Term<synapse_ui::pane::EventProxy>,
    row: i32,
    col: usize,
) -> char {
    term.grid()[Point::new(Line(row), Column(col))].c
}

fn word_motion_w(pane: &mut Pane) {
    use alacritty_terminal::grid::Dimensions;
    let target = {
        let term = match pane.term.lock() {
            Ok(t) => t,
            Err(_) => return,
        };
        let start = match pane.copy_mode.as_ref() {
            Some(cms) => cms.cursor,
            None => return,
        };
        let cols = term.columns();
        let max_row = term.screen_lines() as i32 - 1;
        let mut row = start.line.0;
        let mut col = start.column.0;

        // Skip current word chars
        while row <= max_row && is_word_char(cell_char_at(&term, row, col)) {
            col += 1;
            if col >= cols { col = 0; row += 1; }
        }
        // Skip whitespace
        while row <= max_row && !is_word_char(cell_char_at(&term, row, col)) {
            col += 1;
            if col >= cols { col = 0; row += 1; }
        }
        if row > max_row { row = max_row; col = cols.saturating_sub(1); }
        Point::new(Line(row), Column(col))
    };
    if let Some(ref mut cms) = pane.copy_mode {
        cms.cursor = target;
    }
    pane.dirty.store(true, Ordering::Release);
}

fn word_motion_b(pane: &mut Pane) {
    use alacritty_terminal::grid::Dimensions;
    let target = {
        let term = match pane.term.lock() {
            Ok(t) => t,
            Err(_) => return,
        };
        let start = match pane.copy_mode.as_ref() {
            Some(cms) => cms.cursor,
            None => return,
        };
        let cols = term.columns();
        let min_row = -(term.grid().history_size() as i32);
        let mut row = start.line.0;
        let mut col = start.column.0;

        // Step back one position to start movement
        if col == 0 { col = cols.saturating_sub(1); row -= 1; } else { col -= 1; }
        if row < min_row { row = min_row; col = 0; }

        // Skip whitespace backwards
        while row >= min_row && !is_word_char(cell_char_at(&term, row, col)) {
            if col == 0 { if row <= min_row { break; } col = cols.saturating_sub(1); row -= 1; }
            else { col -= 1; }
        }
        // Find start of word
        while row >= min_row && is_word_char(cell_char_at(&term, row, col)) {
            if col == 0 { break; }
            col -= 1;
        }
        // If we stepped back past the start of word, advance one
        if !is_word_char(cell_char_at(&term, row, col)) && col + 1 < cols {
            col += 1;
        }
        Point::new(Line(row), Column(col))
    };
    if let Some(ref mut cms) = pane.copy_mode {
        cms.cursor = target;
    }
    pane.dirty.store(true, Ordering::Release);
}

fn word_motion_e(pane: &mut Pane) {
    use alacritty_terminal::grid::Dimensions;
    let target = {
        let term = match pane.term.lock() {
            Ok(t) => t,
            Err(_) => return,
        };
        let start = match pane.copy_mode.as_ref() {
            Some(cms) => cms.cursor,
            None => return,
        };
        let cols = term.columns();
        let max_row = term.screen_lines() as i32 - 1;
        let mut row = start.line.0;
        let mut col = start.column.0;

        // Advance one position to start
        col += 1;
        if col >= cols { col = 0; row += 1; }
        if row > max_row { row = max_row; col = cols.saturating_sub(1); }

        // Skip whitespace
        while row <= max_row && !is_word_char(cell_char_at(&term, row, col)) {
            col += 1;
            if col >= cols { col = 0; row += 1; }
        }
        // Advance to end of word
        while row <= max_row {
            let next_col = col + 1;
            let next_row = if next_col >= cols { row + 1 } else { row };
            let nc = if next_col >= cols { 0 } else { next_col };
            if next_row > max_row || !is_word_char(cell_char_at(&term, next_row, nc)) {
                break;
            }
            col = nc;
            row = next_row;
        }
        if row > max_row { row = max_row; col = cols.saturating_sub(1); }
        Point::new(Line(row), Column(col))
    };
    if let Some(ref mut cms) = pane.copy_mode {
        cms.cursor = target;
    }
    pane.dirty.store(true, Ordering::Release);
}
```

- [ ] **Step 4: Wire w/b/e into handle_copy_mode_key**

In the `match key_char { ... }` block, add inside the existing match before `_ => {}`:

```rust
Some("w") => {
    word_motion_w(pane);
    scroll_to_follow_cursor(pane);
}
Some("b") => {
    word_motion_b(pane);
    scroll_to_follow_cursor(pane);
}
Some("e") => {
    word_motion_e(pane);
    scroll_to_follow_cursor(pane);
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test -p SYNAPSE_-app -- test_is_word_char 2>&1 | tail -5
```

Expected: PASS.

- [ ] **Step 6: Build check**

```bash
cargo build -p SYNAPSE_-app 2>&1 | tail -5
```

- [ ] **Step 7: Commit**

```bash
git add crates/SYNAPSE_-app/src/keyboard.rs
git commit -m "feat(copy-mode): w/b/e word motions"
```

---

## Task 5: v/V Selection + y Yank

**Files:**
- Modify: `crates/SYNAPSE_-app/src/keyboard.rs`

- [ ] **Step 1: Write failing tests**

Add to `#[cfg(test)] mod tests`:

```rust
#[test]
fn test_copy_sel_mode_char_not_none() {
    assert_ne!(CopySelMode::Char, CopySelMode::None);
}

#[test]
fn test_copy_sel_mode_line_not_none() {
    assert_ne!(CopySelMode::Line, CopySelMode::None);
}
```

These compile-check the enum variants are distinct. Run:

```bash
cargo test -p SYNAPSE_-app -- test_copy_sel_mode 2>&1 | tail -5
```

Both should PASS immediately (they test type-level things). If not, something is wrong with T1 imports.

- [ ] **Step 2: Add update_selection_after_move helper**

Add before `handle_copy_mode_key`:

```rust
fn update_selection_after_move(pane: &mut Pane) {
    let (sel_mode, cursor) = match pane.copy_mode.as_ref() {
        Some(cms) => (cms.sel_mode.clone(), cms.cursor),
        None => return,
    };
    if sel_mode == CopySelMode::None {
        return;
    }
    if let Ok(mut term) = pane.term.lock() {
        if let Some(ref mut sel) = term.selection {
            sel.update(cursor, Side::Right);
        }
    }
}
```

- [ ] **Step 3: Wire selection update into motion handlers**

In `handle_copy_mode_key`, update the `match key_char` block so every motion calls `update_selection_after_move` after scroll:

```rust
match key_char {
    Some("h") => {
        move_cursor(pane, -1, 0);
        scroll_to_follow_cursor(pane);
        update_selection_after_move(pane);
    }
    Some("j") => {
        move_cursor(pane, 0, 1);
        scroll_to_follow_cursor(pane);
        update_selection_after_move(pane);
    }
    Some("k") => {
        move_cursor(pane, 0, -1);
        scroll_to_follow_cursor(pane);
        update_selection_after_move(pane);
    }
    Some("l") => {
        move_cursor(pane, 1, 0);
        scroll_to_follow_cursor(pane);
        update_selection_after_move(pane);
    }
    Some("w") => {
        word_motion_w(pane);
        scroll_to_follow_cursor(pane);
        update_selection_after_move(pane);
    }
    Some("b") => {
        word_motion_b(pane);
        scroll_to_follow_cursor(pane);
        update_selection_after_move(pane);
    }
    Some("e") => {
        word_motion_e(pane);
        scroll_to_follow_cursor(pane);
        update_selection_after_move(pane);
    }
    Some("v") => {
        let cursor = match pane.copy_mode.as_ref() {
            Some(cms) => cms.cursor,
            None => return,
        };
        if let Some(ref mut cms) = pane.copy_mode {
            cms.anchor = Some(cursor);
            cms.sel_mode = CopySelMode::Char;
        }
        if let Ok(mut term) = pane.term.lock() {
            term.selection = Some(Selection::new(SelectionType::Simple, cursor, Side::Left));
        }
        pane.dirty.store(true, Ordering::Release);
    }
    Some("V") => {
        let cursor = match pane.copy_mode.as_ref() {
            Some(cms) => cms.cursor,
            None => return,
        };
        if let Some(ref mut cms) = pane.copy_mode {
            cms.anchor = Some(cursor);
            cms.sel_mode = CopySelMode::Line;
        }
        if let Ok(mut term) = pane.term.lock() {
            term.selection = Some(Selection::new(SelectionType::Lines, cursor, Side::Left));
        }
        pane.dirty.store(true, Ordering::Release);
    }
    Some("y") => {
        // If no active selection, select current line (yy-style).
        let sel_mode = pane.copy_mode.as_ref().map(|cms| cms.sel_mode.clone()).unwrap_or(CopySelMode::None);
        if sel_mode == CopySelMode::None {
            let cursor = match pane.copy_mode.as_ref() {
                Some(cms) => cms.cursor,
                None => { exit_copy_mode(pane, state); return; }
            };
            if let Ok(mut term) = pane.term.lock() {
                term.selection = Some(Selection::new(SelectionType::Lines, cursor, Side::Left));
            }
        }
        let text = pane.term.lock().ok().and_then(|t| t.selection_to_string());
        if let Some(text) = text {
            if let Some(ref mut cb) = clipboard {
                let _ = cb.set_text(text);
            }
        }
        exit_copy_mode(pane, state);
    }
    _ => {}
}
```

Remove the `let _ = clipboard;` placeholder from the stub (it's now used in the `y` handler).

- [ ] **Step 4: Build and test**

```bash
cargo build -p SYNAPSE_-app 2>&1 | tail -10
cargo test --workspace 2>&1 | tail -15
```

Expected: compiles, all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/SYNAPSE_-app/src/keyboard.rs
git commit -m "feat(copy-mode): v/V charwise/linewise selection, y yank with line fallback"
```

---

## Task 6: Amber Cursor Rendering

**Files:**
- Modify: `crates/SYNAPSE_-app/src/render.rs`

**Design:**  
Amber cursor rect is added in the pane loop — before `cached_cursor_rects_start` is set — so it survives blink-only redraws (the truncation+re-push of blink only touches rects after the marker).  
`push_cursor_rect` returns early when `in_copy_mode` to suppress the normal PTY cursor.

- [ ] **Step 1: Write failing tests in render.rs**

Add to the `#[cfg(test)] mod tests` block at the bottom of `crates/SYNAPSE_-app/src/render.rs`:

```rust
#[test]
fn test_push_cursor_rect_skips_in_copy_mode() {
    use synapse_config::{Config, Keybinds};
    use crate::state::AppState;
    use synapse_renderer::ui::UIRect;
    let mut state = AppState::new(Config::default(), Keybinds::default(), 14.0);
    state.in_copy_mode = true;
    let mut rects: Vec<UIRect> = Vec::new();
    push_cursor_rect(&mut rects, Some((0.0, 0.0)), true, 8.0, 16.0, &mut state);
    assert!(rects.is_empty(), "copy mode must suppress normal cursor rect");
}

#[test]
fn test_push_cursor_rect_emits_rect_normally() {
    use synapse_config::{Config, Keybinds};
    use crate::state::AppState;
    use synapse_renderer::ui::UIRect;
    let mut state = AppState::new(Config::default(), Keybinds::default(), 14.0);
    state.in_copy_mode = false;
    let mut rects: Vec<UIRect> = Vec::new();
    push_cursor_rect(&mut rects, Some((10.0, 20.0)), true, 8.0, 16.0, &mut state);
    assert!(!rects.is_empty(), "normal mode must emit cursor rect");
}
```

- [ ] **Step 2: Run to confirm fail**

```bash
cargo test -p SYNAPSE_-app -- test_push_cursor_rect 2>&1 | tail -10
```

Expected: `test_push_cursor_rect_skips_in_copy_mode` FAILS (no early return yet).

- [ ] **Step 3: Add in_copy_mode early return to push_cursor_rect**

In `fn push_cursor_rect` (around line 423 of render.rs), add at the very beginning of the function body, before the `if !cursor_blink_on { return; }` line:

```rust
if state.in_copy_mode {
    return;
}
```

- [ ] **Step 4: Run tests to confirm pass**

```bash
cargo test -p SYNAPSE_-app -- test_push_cursor_rect 2>&1 | tail -10
```

Expected: both tests PASS.

- [ ] **Step 5: Add amber cursor rect in the pane render loop**

In `render_frame`, find the block that computes `cursor_pixel_for_frame` (around lines 736–746):

```rust
let cursor_viewport_row = cursor_row + display_offset as i32;
if is_active
    && cursor_viewport_row >= 0
    && (cursor_viewport_row as usize) < pane_rows
    && cursor_col < pane_cols
{
    let cx = content_x + cursor_col as f32 * cell_w;
    let cy = content_y + cursor_viewport_row as f32 * cell_h;
    cursor_pixel_for_frame = Some((cx, cy));
}
```

Immediately **after** that block (still inside the `for &(pane_id, rect) in &layouts {` loop), add:

```rust
// Copy mode cursor: amber block, added before cached_cursor_rects_start so it
// survives blink-only redraws (not truncated by the blink update path).
if is_active && state.in_copy_mode {
    if let Some(ref cms) = pane.copy_mode {
        let copy_col = cms.cursor.column.0;
        let copy_viewport_row = cms.cursor.line.0 + display_offset as i32;
        if copy_viewport_row >= 0 && (copy_viewport_row as usize) < pane_rows && copy_col < pane_cols {
            let cx = content_x + copy_col as f32 * cell_w;
            let cy = content_y + copy_viewport_row as f32 * cell_h;
            cached_ui_rects.push(UIRect {
                pos: [cx, cy],
                size: [cell_w, cell_h],
                color: [1.0, 0.75, 0.0, 0.8],
            });
        }
    }
}
```

Note: `display_offset` is already available at this point (captured in the tuple on the same `let (...) = { let term = ...; ... }` block earlier in the pane loop).

- [ ] **Step 6: Build**

```bash
cargo build -p SYNAPSE_-app 2>&1 | tail -10
```

Expected: `Finished`

- [ ] **Step 7: Run all tests**

```bash
cargo test --workspace 2>&1 | tail -15
```

Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add crates/SYNAPSE_-app/src/render.rs
git commit -m "feat(copy-mode): amber cursor rect in render loop, suppress PTY cursor in copy mode"
```

---

## Final Verification

- [ ] **Build release**

```bash
cargo build --release 2>&1 | tail -5
```

- [ ] **Clippy**

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -20
```

Fix any warnings before proceeding.

- [ ] **All tests**

```bash
cargo test --workspace 2>&1 | grep -E "FAILED|error|ok\."
```

Expected: no FAILED, no error.

- [ ] **Manual smoke test**

Start the app: `cargo run -p SYNAPSE_-app`

1. Press `Ctrl+Shift+Space` → amber block cursor appears at PTY cursor position, PTY cursor disappears.
2. Press `hjkl` → amber cursor moves; holding a key repeats movement.
3. Press `k` until scrollback → viewport scrolls to follow.
4. Press `w`/`b`/`e` → cursor jumps to word boundaries.
5. Press `v` → start charwise selection; move → selection highlight extends.
6. Press `V` → start linewise selection; move → whole lines highlight.
7. Press `y` → selection copied to clipboard, copy mode exits, PTY cursor returns.
8. Press `Ctrl+Shift+Space`, then `y` without selection → current line copied, copy mode exits.
9. Press `Ctrl+Shift+Space`, then `Escape` or `q` → copy mode exits, no selection.
10. Press `Ctrl+Shift+Space` again → re-enters copy mode correctly.
