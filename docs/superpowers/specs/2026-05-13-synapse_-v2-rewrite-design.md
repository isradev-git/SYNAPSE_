# SYNAPSE_ v0.2 — Rewrite Design Spec

**Date:** 2026-05-13  
**Status:** Completed  
**Goal:** Rewrite on a proven foundation to fix glyph rendering and deliver a fast, complete, local-first terminal emulator.

---

## 1. Problem Statement

v1 had two blocking issues:

1. **Glyph rendering broken** — cosmic-text + swash atlas pipeline produces incorrect output; root cause in SwashContent handling and atlas packing.
2. **Dev velocity too low** — maintaining custom VT parser, grid, PTY handling in custom terminal crate is expensive; bugs are hard to isolate.

**Solution:** Replace custom terminal with `alacritty_terminal` (battle-tested, Apache 2.0) and replace cosmic-text with `fontdue` (pure Rust, simple, reliable). Keep wgpu GPU pipeline and existing UI concepts.

---

## 2. Architecture

### Crate Structure

```
SYNAPSE_-app (binary)
  ├── alacritty_terminal   ← external crate
  ├── SYNAPSE_-renderer    ← wgpu 22 + fontdue
  ├── SYNAPSE_-ui          ← pane tree, tab bar, layout
  ├── SYNAPSE_-suggest     ← local history autosuggestions
  └── SYNAPSE_-config      ← TOML config, themes, keybinds
```

**SYNAPSE_-terminal crate is eliminated.**

### Dependency Rationale

| Component | v1 | v2 | Reason |
|-----------|----|----|--------|
| Terminal logic | custom (eliminated) | `alacritty_terminal` | Years of VT compat, stable grid/scrollback |
| Font rasterize | cosmic-text + swash | `fontdue` | Pure Rust, simple API, no pipeline bugs |
| GPU pipeline | wgpu 22 | wgpu 22 | Keep — correct choice |
| Autosuggestions | none | `SYNAPSE_-suggest` | New feature |
| Window | winit 0.30 | winit 0.30 | Keep |

---

## 3. Data Flow

```
winit EventLoop (Poll mode)
  │
  ├── KeyboardInput
  │     ├── SYNAPSE_-suggest.on_key(key)
  │     │     ├── Tab / →    → accept suggestion → inject to PTY
  │     │     ├── Shift+→    → accept one word
  │     │     ├── Esc / ↑↓   → discard suggestion
  │     │     └── any char   → query trie → update ghost text
  │     └── other keys → PTY write()
  │
  ├── MouseInput / CursorMoved → selection, scroll, divider drag
  │
  └── RedrawRequested
        ├── PTY rx.try_recv() → alacritty_terminal.process(bytes) → Grid dirty
        ├── [if dirty] build_frame_data()
        │     ├── Grid.visible_cells() → Vec<CellInstance>
        │     ├── cursor, selection, search highlights
        │     ├── ghost text overlay (SYNAPSE_-suggest)
        │     └── UI rects: tab bar, borders, dividers
        └── Renderer.draw_frame()
              ├── fontdue.rasterize(char, size) → glyph pixels
              ├── atlas.get_or_insert(key) → UV coords
              ├── queue.write_texture() if new glyph
              └── 3 draw calls:
                    1. BG layer    — colored cell backgrounds (UIRenderer)
                    2. Cell layer  — atlas glyphs (CellRenderer)
                    3. UI layer    — cursor, borders, tab bar, ghost text (UIRenderer)
```

### Dirty Tracking

Frame data rebuilt only when:
- PTY bytes received (grid dirty flag set)
- User keypress changed suggestion state
- Blink tick (cursor blink)
- Resize / tab switch / config reload

---

## 4. SYNAPSE_-suggest: Autosuggestions

### History Loading (startup)

Read all available shell history files:
- `~/.zsh_history`
- `~/.bash_history`
- `~/.local/share/fish/fish_history`

Parse into Vec<String>, deduplicate, build prefix trie in memory. Skip corrupted lines silently.

### Runtime Behavior

```
User types "git p"
  → trie prefix match → "git push origin main"
  → ghost text rendered at cursor position, grey color

Tab or →     → accept full suggestion → send string to PTY
Shift+→      → accept next word only
Esc / ↑ / ↓ → discard ghost text
Any other char → re-query trie with new prefix
```

### Ghost Text Rendering

- Same line as cursor, after cursor position
- Color: `fg` with 40% alpha (grey ghost)
- Rendered in UI layer (draw call 3), above cell layer
- Not sent to PTY until accepted

### Data Structure

Prefix trie: O(m) lookup where m = prefix length. Built once at startup, immutable during session. Memory: ~5–20MB for typical shell history.

---

## 5. UI & Features

### Tabs

```
Ctrl+T        → new tab
Ctrl+W        → close current tab
Ctrl+[1-9]   → switch to tab N
```

Tab bar: top of window, same synapse_ dark theme.

### Pane Splits

Binary tree structure: `Leaf(PaneId) | Split { dir, ratio, first, second }`.

```
Ctrl+Shift+D  → split vertical (right)
Ctrl+Shift+E  → split horizontal (below)
Ctrl+Shift+W  → close pane
Alt+arrows    → focus adjacent pane
Alt+drag      → resize divider (ratio 0.0–1.0)
```

Default split ratio: 0.5. Each pane = independent `alacritty_terminal` instance with own PTY.

### Search

`Ctrl+F` — overlay search bar, highlight all matches, `Enter`/`Shift+Enter` navigate, `Esc` close. Carry over from v1.

### Themes

4 built-in (synapse_, dracula, catppuccin-mocha, tokyo-night). Custom themes via TOML. Hot-reload: `Ctrl+,`.

### Cursor Styles

Block (default), beam, underline. Configurable in `config.toml`. Optional blink.

### Image Protocol (Phase 2)

Kitty image protocol — inline images in terminal. Deferred to Phase 2 after foundation stable.

---

## 6. Renderer Design

### Atlas

- Texture: 2048×2048 RGBA
- Per-glyph key: `(char, font_size_px)` → `UvRect`
- LRU eviction when >90% full
- Row-based bin packing (same as v1, proven)
- wgpu row alignment: 256-byte boundary on upload

### fontdue Integration

```rust
// Startup
let font = fontdue::Font::from_bytes(JETBRAINS_MONO_BYTES, fontdue::FontSettings::default())?;

// Per glyph (cached after first render)
let (metrics, bitmap) = font.rasterize(ch, font_size_px);
// bitmap: Vec<u8> grayscale → convert to RGBA for atlas upload
```

Font embedded as bytes in binary (`include_bytes!`). No system font dependency at runtime. Fallback: if char not in JetBrains Mono, rasterize from embedded Noto Sans fallback.

### GPU Instances

`CellInstance` (64 bytes):
```rust
struct CellInstance {
    pos:  [f32; 2],  // screen position
    size: [f32; 2],  // cell dimensions
    uv:   [f32; 4],  // atlas UV rect
    fg:   [f32; 4],  // foreground RGBA
    bg:   [f32; 4],  // background RGBA
}
```

Single draw call per layer. Instanced rendering via wgpu `draw_instanced`.

---

## 7. Error Handling

### Startup — Fail Fast

| Error | Behavior |
|-------|----------|
| No GPU adapter | `panic!("No GPU adapter found. SYNAPSE_ requires Vulkan/Metal/DX12.")` |
| PTY spawn fails | `panic!("Failed to spawn shell: {err}")` |
| Font bytes corrupt | `panic!("Embedded font invalid")` |

### Runtime — Degrade Gracefully

| Error | Behavior |
|-------|----------|
| Atlas full | Evict LRU glyphs, continue |
| PTY rx error | Log, close affected pane, keep app running |
| History parse error | Skip corrupted line, continue loading |
| Config TOML invalid | Use defaults, show warning in tab bar |
| Glyph rasterize fails | Render blank cell, log once |

---

## 8. Testing

### Unit Tests

**SYNAPSE_-renderer:**
- Atlas: UV rects do not overlap after N insertions
- fontdue rasterize: non-empty bitmap for ASCII chars
- draw_frame: no panic with 0 cells, 1 cell, max cells

**SYNAPSE_-suggest:**
- Trie prefix "git p" → "git push origin main" (seeded history)
- Empty history → no suggestion, no panic
- Corrupted history line → skipped, rest loads
- Tab accept → correct string produced
- Shift+→ → single word accepted

**SYNAPSE_-ui:**
- PaneTree: split → 2 panes at ratio 0.5
- Close pane → tree collapses, parent becomes sibling
- `get_layout()` → rects non-overlapping, fill full area

### Integration

`cargo test --workspace` — target: ~80 tests.

`alacritty_terminal` has its own VT conformance tests — we do not duplicate.

### Performance (manual verification)

| Target | Threshold |
|--------|-----------|
| Input→render latency | <5ms |
| FPS stable | 60fps |
| FPS under heavy output | ≥30fps |
| Startup | <200ms |
| RAM idle | <50MB |

---

## 9. Implementation Phases

| Phase | Scope | Done when |
|-------|-------|-----------|
| **1 — Foundation** | Swap SYNAPSE_-terminal → alacritty_terminal. Replace cosmic-text → fontdue. Renderer renders ASCII correctly. | Text displays without glyph artifacts |
| **2 — UI rework** | Tabs, splits, config hot-reload, themes — clean implementation on new foundation | Tabs + splits working, all keybinds functional |
| **3 — Autosuggestions** | SYNAPSE_-suggest crate, trie, ghost text overlay, Tab/→ accept | Ghost text shows from real shell history |
| **4 — Polish + Image** | Kitty image protocol, ligatures (opt-in), scrollback perf, theme polish | Image protocol works, 60fps stable |

---

## 10. Out of Scope

- AI features (no cloud, no local model)
- Windows support (deferred)
- Multiplexer (no tmux replacement — splits are handled natively)
- Sixel graphics (Phase 4+ if ever)
- Accessibility / screen reader support
