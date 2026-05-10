# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```sh
cargo build -p Luna-app                      # build binary
cargo run -p Luna-app                        # run
cargo build --release                        # release build (thin LTO)
cargo test --workspace                       # all tests (~67)
cargo test -p Luna-terminal                  # single crate tests
cargo test -p Luna-terminal -- --nocapture   # with stdout
cargo fmt --all -- --check                   # format check
cargo clippy --workspace --all-targets -- -D warnings  # lint (warnings = errors)
```

Linux requires system deps for building: `libx11-dev libxkbcommon-dev libwayland-dev libxrandr-dev libxi-dev`

## Architecture

Cargo workspace with 5 crates under `crates/`. Crate dirs use PascalCase (`Luna-app`) but lib names use snake_case (`luna_renderer`).

```
Luna-app        → binary, winit event loop, owns App struct
Luna-terminal   → PTY, VT parser, grid/buffer
Luna-renderer   → wgpu GPU rendering, texture atlas, text shaping
Luna-ui         → layout, pane tree, tab bar, theme
Luna-config     → TOML config, keybinds
```

### Data flow

```
PTY (portable-pty) → reader thread → mpsc channel
  → VteProcessor (vte 0.13) → Grid (CharCell matrix)
  → render.rs reads Grid → CellInstance vec → draw_frame()
  → CellRenderer (cell.wgsl) + UIRenderer (ui.wgsl) → wgpu surface
```

### Ownership model

- `App` owns `Vec<Pane>` (flat list), `TabBar`, `Layout`, `Renderer`, `AppState`
- Each `Tab` owns a `PaneTree` (binary split tree) and `active_pane: PaneId`
- `Pane` holds `PaneSession` (PTY + mpsc rx), `Rc<RefCell<Grid>>`, `VteProcessor`
- `PaneTree` is `Leaf(PaneId) | Split { direction, ratio, first, second }` — split at 50% by default, draggable

### Rendering

Single render pass per frame: `draw_frame(cells, ui_rects)` — one `get_current_texture`/`present` call.

**Instanced rendering**: one GPU instance per visible cell, 64 bytes each (pos:2, size:2, uv:4, fg:4, bg:4). Single draw call.

**Atlas**: glyphs cached in `TextureAtlas` (texture + UV map). `cosmic-text` + swash for rasterization. `SwashContent::Mask` = 1-byte alpha, convert to RGBA manually.

**Bind groups**: group 0 = atlas texture + sampler, group 1 = screen uniform (vec2).

### Non-obvious API constraints

**winit 0.30**: `EventLoop::new()?` then `event_loop.create_window(WindowAttributes::default())`. `EventLoop::run()` is deprecated but still used. `RedrawRequested` + `AboutToWait` drive the render loop.

**wgpu 22**: `entry_point` in vertex/fragment state is `&str` (not `Option<&str>`). `BindGroupLayout` does not implement `Clone` — pass by reference. `create_surface` requires `Arc<Window>`. Use `pollster::block_on` for adapter/device init.

**cosmic-text 0.12**: `CacheKey::new(font_id, glyph_id, font_size, (x, y), flags)` returns `(CacheKey, i32, i32)`. `SwashCache::get_image_uncached(font_system, cache_key)` → `Option<SwashImage>`.

## Color palette

```
#ff3d94  cursor, selection, active prompt
#b5307e  active tab, active pane border
#6a2a98  inactive tab, separators, tab bar bg
#3f1c6d  hover, inactive panes
#210b4b  main background (wgpu clear color)
```

## Config

Auto-created at first launch:
- Linux: `~/.config/Luna/config.toml`
- macOS: `~/Library/Application Support/Luna/config.toml`
- Windows: `%APPDATA%\Luna\config.toml`

Hot-reload: `Ctrl+,`

## Performance targets (non-negotiable)

- Input→render latency: <5ms
- FPS: 60 stable, ≥30 under heavy output
- Startup: <200ms
- RAM idle: <50MB

## Platform scope

**Active targets:** macOS (Metal) + Linux (X11/Wayland).
**Windows:** deferred — not tested, not a priority right now.

When writing platform-specific code, implement macOS + Linux paths fully. Add a `#[cfg(target_os = "windows")]` stub only if needed to keep compilation clean, but don't invest in Windows behavior.

## No network, no database

Desktop app. Everything in-memory. No REST routes, no DB.
