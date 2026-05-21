# Phase 11b — Copy Mode Design

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add vim-style keyboard copy mode to SYNAPSE_ — enter with Ctrl+Shift+Space, move cursor with hjkl/w/b/e, select with v/V, yank with y.

**Architecture:** `CopyModeState` lives in `Pane` (pane-local state). `AppState::in_copy_mode: bool` gates keyboard routing in `keyboard.rs`. Selection rendering reuses the existing `term.selection` mechanism (zero new render code for highlight). Copy mode cursor rendered as amber block in `push_cursor_rect`. New function `handle_copy_mode_key()` in `keyboard.rs` handles all copy mode input.

**Tech Stack:** Rust, alacritty_terminal 0.24 (`TermSelection`, `SelectionType`, `Point`, `Side`), arboard (existing clipboard).

**Out of scope:** `/` search (planned for a later phase with status bar/command line).

---

## 1. Data Types (pane.rs)

New types added to `synapse_ui::pane`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum CopySelMode {
    None,
    Char,  // entered with v
    Line,  // entered with V
}

#[derive(Debug, Clone)]
pub struct CopyModeState {
    /// Current copy mode cursor position (absolute grid coords, same system as Term grid).
    pub cursor: alacritty_terminal::index::Point,
    /// Selection anchor — set when user presses v or V. None = no active selection.
    pub anchor: Option<alacritty_terminal::index::Point>,
    pub sel_mode: CopySelMode,
}
```

New field on `Pane`:

```rust
pub copy_mode: Option<CopyModeState>,
```

Initialized to `None` in `Pane::new`.

## 2. AppState Change (state.rs)

New field:

```rust
pub in_copy_mode: bool,
```

Initialized to `false`.

## 3. Keyboard Routing (keyboard.rs)

At the top of the key handler, before any PTY write:

```rust
if state.in_copy_mode {
    handle_copy_mode_key(key, modifiers, pane, state, clipboard);
    return;
}
```

`Ctrl+Shift+Space` is the toggle keybind. It is handled **before** the copy mode gate — pressing it while in copy mode exits; pressing it while out enters.

New function signature:

```rust
fn handle_copy_mode_key(
    key: &Key,
    modifiers: ModifiersState,
    pane: &mut Pane,
    state: &mut AppState,
    clipboard: &mut Option<arboard::Clipboard>,
)
```

### Enter copy mode

```rust
fn enter_copy_mode(pane: &mut Pane, state: &mut AppState) {
    // Cursor starts at PTY cursor position.
    let cursor = pane.term.lock().map(|t| t.grid().cursor.point).unwrap_or_default();
    pane.copy_mode = Some(CopyModeState {
        cursor,
        anchor: None,
        sel_mode: CopySelMode::None,
    });
    state.in_copy_mode = true;
}
```

### Exit copy mode

```rust
fn exit_copy_mode(pane: &mut Pane, state: &mut AppState) {
    if let Ok(mut term) = pane.term.lock() {
        term.selection = None;
    }
    pane.copy_mode = None;
    state.in_copy_mode = false;
    pane.dirty.store(true, std::sync::atomic::Ordering::Release);
}
```

### Keybind mapping inside copy mode

| Key | Action |
|-----|--------|
| `Escape` / `q` | Exit copy mode |
| `h` | Move cursor left |
| `j` | Move cursor down |
| `k` | Move cursor up |
| `l` | Move cursor right |
| `w` | Jump to start of next word |
| `b` | Jump to start of current/previous word |
| `e` | Jump to end of current/next word |
| `v` | Enter charwise selection (anchor = cursor) |
| `V` | Enter linewise selection (anchor = cursor) |
| `y` | Yank selection (or current line if no selection) |

All other keys: no-op (not forwarded to PTY).

## 4. Motion Logic (keyboard.rs)

### h/j/k/l

```rust
fn move_cursor(
    cms: &mut CopyModeState,
    pane: &Pane,
    delta_col: i32,
    delta_row: i32,
) {
    // Read grid dimensions under lock.
    let (cols, rows, history) = {
        let term = pane.term.lock().unwrap();
        (
            term.columns() as i32,
            term.screen_lines() as i32,
            term.grid().history_size() as i32,
        )
    };
    let new_col = (cms.cursor.column.0 as i32 + delta_col).clamp(0, cols - 1);
    let new_row = (cms.cursor.line.0 + delta_row).clamp(-history, rows - 1);
    cms.cursor = alacritty_terminal::index::Point::new(
        alacritty_terminal::index::Line(new_row),
        alacritty_terminal::index::Column(new_col as usize),
    );
}
```

After moving, if cursor goes outside the visible area, scroll viewport to follow:

```rust
fn scroll_to_follow_cursor(pane: &Pane, cms: &CopyModeState) {
    let (display_offset, screen_lines, history) = {
        let term = pane.term.lock().unwrap();
        (
            term.grid().display_offset() as i32,
            term.screen_lines() as i32,
            term.grid().history_size() as i32,
        )
    };
    // Visible rows: from -(display_offset + screen_lines - 1) to -display_offset
    let top = -(display_offset + screen_lines - 1);
    let bottom = -display_offset;
    let row = cms.cursor.line.0;
    if row < top {
        pane.scroll_viewport(alacritty_terminal::grid::Scroll::Delta(top - row));
    } else if row > bottom {
        pane.scroll_viewport(alacritty_terminal::grid::Scroll::Delta(bottom - row));
    }
}
```

### Word motion helpers

Word character: `c.is_alphanumeric() || c == '_'` (vim `iskeyword` style).

```rust
fn cell_char(pane: &Pane, point: alacritty_terminal::index::Point) -> char {
    pane.term.lock()
        .map(|t| t.grid()[point].c)
        .unwrap_or(' ')
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}
```

Cursor wrapping between lines: end of column → col 0 of next row.

**`w`** — start of next word:
1. Skip current word chars
2. Skip whitespace
3. Land on first word char

**`b`** — start of current/previous word:
1. If on word char: skip back to start of current word
2. If on whitespace: skip back over whitespace, then to start of word

**`e`** — end of current/next word:
1. If not on word char: skip whitespace
2. Skip word chars, stop at last

## 5. Selection (keyboard.rs)

### Enter charwise selection (`v`)

```rust
let cms = pane.copy_mode.as_mut().unwrap();
cms.anchor = Some(cms.cursor);
cms.sel_mode = CopySelMode::Char;
// Sync term.selection
if let Ok(mut term) = pane.term.lock() {
    term.selection = Some(alacritty_terminal::selection::Selection::new(
        alacritty_terminal::selection::SelectionType::Simple,
        cms.cursor,
        alacritty_terminal::index::Side::Left,
    ));
}
```

### Enter linewise selection (`V`)

Same but `SelectionType::Lines`.

### Extend selection on motion

After every cursor move, if `sel_mode != None`:

```rust
if let Ok(mut term) = pane.term.lock() {
    if let Some(ref mut sel) = term.selection {
        sel.update(cms.cursor, alacritty_terminal::index::Side::Right);
    }
}
```

### Yank (`y`)

```rust
// If no selection active, select current line.
if cms.sel_mode == CopySelMode::None {
    if let Ok(mut term) = pane.term.lock() {
        term.selection = Some(alacritty_terminal::selection::Selection::new(
            alacritty_terminal::selection::SelectionType::Lines,
            cms.cursor,
            alacritty_terminal::index::Side::Left,
        ));
    }
}
// Yank.
let text = pane.term.lock()
    .ok()
    .and_then(|t| t.selection_to_string());
if let Some(text) = text {
    if let Some(ref mut cb) = clipboard {
        let _ = cb.set_text(text);
    }
}
exit_copy_mode(pane, state);
```

## 6. Rendering (render.rs)

### Copy mode cursor (amber block)

In `push_cursor_rect`, when `state.in_copy_mode` and pane is active:

```rust
if state.in_copy_mode {
    if let Some(ref cms) = pane.copy_mode {
        // Convert grid point to screen coords.
        let col = cms.cursor.column.0 as f32;
        let row = /* convert cms.cursor.line to screen row accounting for display_offset */;
        let x = margin + col * cell_w;
        let y = tab_bar_h + row * cell_h;
        rects.push(UiRect {
            x, y,
            w: cell_w, h: cell_h,
            color: [1.0, 0.75, 0.0, 0.8],  // #FFBF00 amber, 80% opacity
        });
    }
    return;  // skip normal PTY cursor
}
```

Row conversion: follow the existing render.rs pattern for mapping `Point.line` + `display_offset` to a screen row. Implementer should grep render.rs for how `SelectionRange`/grid points are converted to screen rows and use the same formula.
If `screen_row` is outside `[0, screen_lines)`, cursor is off-screen — skip draw.

### Selection highlight

No changes. The existing `term.selection` → `SelectionRange` → per-cell highlight in `render.rs` handles this already.

## 7. Action + Keybind (keybinds.rs)

New action:

```rust
pub enum Action {
    // ... existing ...
    ToggleCopyMode,
}
```

String: `"toggle_copy_mode"`.

Default binding: `Ctrl+Shift+Space` → `Action::ToggleCopyMode`.

In `keyboard.rs`, `ToggleCopyMode` is processed **before** the copy mode gate:

```rust
if action == Action::ToggleCopyMode {
    if state.in_copy_mode {
        exit_copy_mode(pane, state);
    } else {
        enter_copy_mode(pane, state);
    }
    return;
}
```

## 8. Task Summary

| Task | Files | Description |
|------|-------|-------------|
| T1 | `pane.rs`, `state.rs` | `CopySelMode`, `CopyModeState`, `Pane::copy_mode`, `AppState::in_copy_mode` |
| T2 | `keybinds.rs`, `keyboard.rs` | `ToggleCopyMode` action, default binding, enter/exit logic, routing gate |
| T3 | `keyboard.rs` | h/j/k/l motion + scroll-to-follow |
| T4 | `keyboard.rs` | w/b/e word motions |
| T5 | `keyboard.rs` | v/V selection + y yank (including yy-style line yank) |
| T6 | `render.rs` | Amber cursor rect in copy mode + skip normal cursor |

## 9. Testing

- T1: `test_copy_mode_state_initial_none`, `test_copy_sel_mode_eq`
- T2: `test_enter_copy_mode_sets_cursor`, `test_exit_copy_mode_clears`, `test_toggle_copy_mode_enters`, `test_toggle_copy_mode_exits`
- T3: `test_hjkl_basic_move`, `test_hjkl_clamp_at_bounds`, `test_scroll_to_follow_cursor_below`, `test_scroll_to_follow_cursor_above`
- T4: `test_word_motion_w_skips_to_next`, `test_word_motion_b_goes_to_start`, `test_word_motion_e_lands_on_end`, `test_word_motion_wrap_row`
- T5: `test_v_sets_char_sel_mode`, `test_V_sets_line_sel_mode`, `test_y_yanks_selection`, `test_y_no_selection_copies_line`
- T6: `test_cursor_rect_color_in_copy_mode`, `test_cursor_rect_off_screen_skipped`

## 10. Performance

Copy mode cursor: one extra `UiRect` per frame (negligible). Word motions: linear scan of grid cells, bounded by `cols * scrollback_rows` — only triggered on keypress, not in render loop. No allocations in hot path.
