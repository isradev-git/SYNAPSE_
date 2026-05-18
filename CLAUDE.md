# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```sh
cargo build -p SYNAPSE_-app                  # build binary
cargo run -p SYNAPSE_-app                    # run
cargo build --release                        # release build (thin LTO)
cargo test --workspace                       # all tests (~80)
cargo test -p SYNAPSE_-config                # single crate tests
cargo test -p SYNAPSE_-config -- --nocapture # with stdout
cargo fmt --all -- --check                   # format check
cargo clippy --workspace --all-targets -- -D warnings  # lint (warnings = errors)
```

Linux requires system deps for building: `libx11-dev libxkbcommon-dev libwayland-dev libxrandr-dev libxi-dev`

## Architecture

Cargo workspace with 4 crates under `crates/`. Crate dirs use `SYNAPSE_-*` prefix, lib names use snake_case (`synapse_renderer`).

```
SYNAPSE_-app        → binary, winit event loop, owns App struct
SYNAPSE_-renderer   → wgpu GPU rendering, texture atlas, text shaping
SYNAPSE_-ui         → layout, pane tree, tab bar, theme
SYNAPSE_-config     → TOML config, keybinds
```

### Data flow

```
PTY (portable-pty) → reader thread → mpsc channel
  → alacritty_terminal VTE processor → Term grid
  → render.rs reads grid → CellInstance vec → draw_frame()
  → CellRenderer (cell.wgsl) + UIRenderer (ui.wgsl) → wgpu surface
```

### Ownership model

- `App` owns `Vec<Pane>` (flat list), `TabBar`, `Layout`, `Renderer`, `AppState`
- Each `Tab` owns a `PaneTree` (binary split tree) and `active_pane: PaneId`
- `Pane` holds `Term` (alacritty_terminal), PTY master/writer, event_rx, dirty AtomicBool
- `PaneTree` is `Leaf(PaneId) | Split { direction, ratio, first, second }` — split at 50% by default, draggable

### Rendering

Single render pass per frame: `draw_frame(cells, ui_rects)` — one `get_current_texture`/`present` call.

**Instanced rendering**: one GPU instance per visible cell, 64 bytes each (pos:2, size:2, uv:4, fg:4, bg:4). Single draw call.

**Atlas**: glyphs cached in `TextureAtlas` (texture + UV map). `fontdue` for rasterization. 2048×2048 RGBA, reset at 90% full.

**Bind groups**: group 0 = atlas texture + sampler, group 1 = screen uniform (vec2).

### Non-obvious API constraints

**winit 0.30**: `EventLoop::new()?` then `event_loop.create_window(WindowAttributes::default())`. `EventLoop::run()` is deprecated but still used. `RedrawRequested` + `AboutToWait` drive the render loop.

**wgpu 22**: `entry_point` in vertex/fragment state is `&str` (not `Option<&str>`). `BindGroupLayout` does not implement `Clone` — pass by reference. `create_surface` requires `Arc<Window>`. Use `pollster::block_on` for adapter/device init.

**alacritty_terminal 0.24**: `grid.display_offset()` = 0 at bottom, positive = scrolled into history. `grid.history_size()` requires `Dimensions` trait in scope. `term.selection_to_string()` extracts selected text.

## Color palette

Cyberpunk dark theme with steel blue accent. Defined in `Theme::synapse_()` (`crates/SYNAPSE_-config/src/themes.rs`).
4 built-in themes: `synapse_`, `dracula`, `catppuccin-mocha`, `tokyo-night`.

```
#11131a  main background (wgpu clear color)
#d2d5db  buffer text
#7098cc  cursor, active pane border (steel blue)
#181b24  tab bar bg
#222739  active tab, separators, inactive borders
#e5e8ee  active UI text
#737a8c  inactive UI text
```

## Config

Auto-created at first launch:
- Linux: `~/.config/SYNAPSE_/config.toml`
- macOS: `~/Library/Application Support/SYNAPSE_/config.toml`
- Windows: `%APPDATA%\SYNAPSE_\config.toml`

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
