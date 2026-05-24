# SYNAPSE_

> GPU-accelerated terminal emulator · Rust · wgpu · Cross-platform

[![CI](https://github.com/isradev-git/synapse_/actions/workflows/ci.yml/badge.svg)](https://github.com/isradev-git/synapse_/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/isradev-git/synapse_)](https://github.com/isradev-git/synapse_/releases/latest)
[![Tests](https://img.shields.io/badge/tests-303%20passed-brightgreen)]()
[![Rust](https://img.shields.io/badge/rust-stable%202021-orange)]()

SYNAPSE_ is a modern terminal emulator with GPU rendering (Vulkan/Metal/DirectX 12), full VT100/xterm-256color support, split panes, tabs, 30 built-in features, and everything configurable via TOML.

## Status

**v0.2.0** — Feature-complete. All 28 planned improvements implemented. 303 tests passing. 0 warnings.

## Stack

| Component    | Technology                        |
|--------------|-----------------------------------|
| Language     | Rust (stable, edition 2021)       |
| Windowing    | winit 0.30                        |
| GPU Rendering| wgpu 22 (Vulkan/Metal/DX12)       |
| Text Shaping | fontdue 0.9 + rustybuzz 0.14      |
| PTY          | portable-pty 0.8 + native_pty_system |
| VT Parser    | alacritty_terminal 0.24           |
| Async I/O    | tokio 1                           |
| Config       | serde + TOML                      |
| Image Decode | image 0.25                        |

## Features

### Core Terminal

- **GPU Rendering** — 60fps stable, <5ms input→render latency, instanced rendering (one draw call per frame)
- **Full VT100/xterm** — C0, CSI, SGR (16M true color), OSC, ESC, DECSCUSR cursor styles
- **Scrollback** — 100,000 lines configurable, circular buffer
- **Frame Cache** — GPU instance cache avoids rebuilding cells when nothing changes, dirty tracking per grid
- **Damage Tracking** — Real per-cell dirty tracking via `alacritty_terminal::Term::damage()`, >90% frame time reduction on massive output
- **Atlas LRU Eviction** — Glyph atlas auto-evicts least-recently-used entries, dual-atlas for zero-pause resize

### UI / Layout

- **Vertical Sidebar** — Tab bar moved to left sidebar (180px default), configurable width per TOML
- **Split Panes** — Binary tree of splits, horizontal/vertical, drag-to-resize dividers
- **Pane Zoom** — Maximize any pane to full window (Ctrl+Shift+Z), auto-exit on tab switch
- **Pane Badge** — Semi-transparent watermark with CWD, title, or `user@host` in pane background
- **Background Image** — Per-pane wallpaper with cover/contain/stretch/tile modes and configurable opacity
- **Status Bar** — Clock, git branch, K8s context, broadcast indicator, recording indicator, CWD
- **Window Transparency** — True window transparency (`with_transparent(true)`) on Wayland + compositors, configurable opacity
- **Wayland CSD** — Client-side decorations on Wayland with custom title bar + drag support

### Text & Typography

- **Font Ligatures** — rustybuzz + HarfBuzz, calt/liga/clig ON by default
- **Font Fallback Chain** — Multi-font chain (e.g. `["Dogica", "JetBrains Mono", "Noto Color Emoji"]`), per-codepoint glyph routing
- **Dynamic Font Size** — Ctrl+=/-/0 at runtime, no restart needed
- **Color Emoji** — CBDT/CBLC bitmap emoji via `ttf-parser`, rendered as actual RGBA in the atlas with separate shader path

### Search & Selection

- **Regex Search** — Ctrl+Shift+F with regex toggle (Alt+R), case-insensitive by default
- **History Search** — Ctrl+R fuzzy-search persistent command history
- **Word Occurrence Highlight** — Select a word to highlight all occurrences in the visible viewport (like VS Code)
- **Selection v2** — Double-click selects word, triple-click selects line, Shift+click extends, Alt+click block selection
- **URL/Path Detection** — Auto-detect URLs + file paths (e.g. `src/main.rs:42`), Ctrl+click to open (VS Code, $EDITOR, xdg-open)

### Images & Graphics

- **Sixel Decoder** — Native Rust decoder, no external libs, supports color registers and patterns
- **iTerm2 Inline Images** — OSC 1337 (`File=inline=1`) with base64 PNG/JPEG/GIF/BMP, auto-scroll on overflow
- **APC Kitty Images** — Full Kitty terminal graphics protocol with image placement, delete, and z-order

### Productivity

- **Broadcast Input** — Ctrl+Shift+B sends input to all panes in the active tab (SSH multi-server ops)
- **Command Palette** — Ctrl+Shift+P quick actions: tabs, themes, plugins, keybinds
- **Workspaces** — Named workspaces (Ctrl+Shift+N), switch rapidly between "dev", "ssh", "logs"
- **Drag & Drop** — Drop files from file manager to paste absolute paths
- **CLI Args** — `-e cmd`, `-d path`, `--new-tab`, `--hold`, `--restore`, `--quake`, `--setup`
- **Quake Mode** — Dropdown terminal overlay (Ctrl+Space), slide animation, auto-hide on focus loss

### Sessions & Persistence

- **Session Save/Restore** — Tabs, panes, and CWDs saved to `~/.cache/SYNAPSE_/session.json`, autosave on exit
- **Persistent History** — Cross-session command history with MRU ordering, deduplication, OSC 133 aware
- **Recording (.cast)** — Export sessions to asciinema-compatible `.cast` format (Ctrl+Shift+R)

### Extensibility

- **Plugin System** — TOML-defined plugins: keybind → shell command, `$CURRENT_PANE_CWD`, `$SELECTED_TEXT`, `$CLIPBOARD`, split modes
- **Shell Integration** — Official zsh/bash/fish scripts with OSC 133 (prompt markers), OSC 7 (CWD tracking), `--setup` auto-installer
- **Suggestion Engine** — Built-in `crates/SYNAPSE_-suggest` with frequency trie for autocomplete

### Accessibility

- **High Contrast Theme** — Built-in `high-contrast-dark` theme (black background, white text, yellow accent)
- **Reduce Motion** — `reduce_motion = true` disables all animations (splash, cursor blink, pane pulse, resize indicators)

### Performance

- **Background Tab Freeze** — Pause PTY reads on non-visible tabs, save CPU and battery
- **Profiler Overlay** — F12 toggles FPS, frame time, PTY bandwidth, cell count, atlas utilization
- **Scrollbar** — Per-pane thin scrollbar (6px track + 12px min thumb), click-to-jump + drag

## Competitive Comparison

| Feature | SYNAPSE_ | Kitty | WezTerm | Alacritty | Warp | iTerm2 |
|---------|----------|-------|---------|-----------|------|--------|
| **GPU Rendering** | wgpu (Vulkan/Metal/DX12) | OpenGL | wgpu | OpenGL/Metal | Metal | Metal |
| **Ligatures** | rustybuzz, ON by default | harfbuzz | harfbuzz | ❌ | ❌ | harfbuzz |
| **Color Emoji** | CBDT/CBLC native | ✅ | ✅ | ❌ | ❌ | ✅ |
| **Font Fallback** | Multi-chain per codepoint | ✅ | ✅ | ❌ | ❌ | ✅ |
| **Sixel** | Native Rust decoder | ✅ | ✅ | ❌ | ❌ | ✅ |
| **iTerm2 Images** | OSC 1337 inline | ❌ | ✅ | ❌ | ❌ | ✅ |
| **Kitty Graphics** | APC protocol | ✅ | ✅ | ❌ | ❌ | ❌ |
| **Split Panes** | Binary tree + zoom | ✅ | ✅ | ❌ | ✅ | ✅ |
| **Workspaces** | Named workspaces | ❌ | ✅ | ❌ | ❌ | ❌ |
| **Broadcast Input** | Ctrl+Shift+B | ✅ | ❌ | ❌ | ❌ | ✅ |
| **Session Save/Restore** | JSON autosave | ❌ | ✅ | ❌ | ✅ | ✅ |
| **Recording (.cast)** | Native asciinema export | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Persistent History** | MRU dedup, OSC 133 | ❌ | ❌ | ❌ | ✅ | ❌ |
| **Plugin System** | TOML keybind→shell | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Shell Integration** | zsh/bash/fish + OSC 7/133 | ✅ | ✅ | ❌ | ✅ | ✅ |
| **Command Palette** | Ctrl+Shift+P | ✅ | ✅ | ❌ | ✅ | ✅ |
| **Background Image** | Per-pane, 4 modes | ❌ | ✅ | ❌ | ❌ | ✅ |
| **Window Transparency** | Native wgpu alpha | ✅ | ✅ | ❌ | ❌ | ✅ |
| **Wayland CSD** | Custom title bar | ❌ | ✅ | ❌ | ❌ | N/A |
| **Quake Mode** | Dropdown overlay | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Word Highlight** | Selection occurrences | ❌ | ❌ | ❌ | ❌ | ❌ |
| **High Contrast Theme** | Built-in a11y theme | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Reduce Motion** | All animations off | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Background Tab Freeze** | PTY pause on hidden | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Profiler Overlay** | F12 debug HUD | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Scrollbar** | Per-pane thin + drag | ✅ | ❌ | ❌ | ❌ | ✅ |
| **Pane Badge/Watermark** | CWD/title watermark | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Open Source** | MIT | GPLv3 | MIT | Apache 2 | Proprietary | GPLv2 |
| **Telemetry** | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ |

## Quick Install

```sh
# Linux / macOS
curl -fsSL https://github.com/isradev-git/synapse_/releases/latest/download/SYNAPSE_-app-installer.sh | sh

# Windows (PowerShell)
irm https://github.com/isradev-git/synapse_/releases/latest/download/SYNAPSE_-app-installer.ps1 | iex
```

Also available as `.msi` (Windows) and tarballs on the [releases page](https://github.com/isradev-git/synapse_/releases/latest).

See [INSTALL.md](INSTALL.md) for platform-specific instructions, building from source, and initial configuration.

## Quick Start

```sh
# Build and run
cargo build --release -p SYNAPSE_-app
./target/release/synapse_

# Install shell integration (zsh/bash/fish)
./target/release/synapse_ --setup

# Run with custom command
./target/release/synapse_ -e "vim ~/.config/SYNAPSE_/config.toml"
```

## Configuration

`~/.config/SYNAPSE_/config.toml` (Linux) or `~/Library/Application Support/SYNAPSE_/config.toml` (macOS):

```toml
font_size = 14
font_family = ["Dogica", "JetBrains Mono", "Noto Color Emoji"]
theme = "synapse_"
scrollback_lines = 100000

shell_integration = true
persistent_history = true
scrollbar = true
pane_badge = true
pane_badge_format = "{cwd}"

background_image = "/home/user/wallpaper.png"
background_opacity = 0.3
background_mode = "cover"

window_opacity = 0.95
window_blur = true
```

See [CONFIGURATION.md](CONFIGURATION.md) for all options.

## Keybinds

| Shortcut | Action |
|----------|--------|
| Ctrl+Shift+Z | Pane zoom |
| Ctrl+Shift+B | Broadcast input |
| Ctrl+Shift+F | Search buffer (regex with Alt+R) |
| Ctrl+R | Search command history |
| Ctrl+Shift+P | Command palette |
| Ctrl+Shift+N | New workspace |
| Ctrl+Space | Quake mode toggle |
| Ctrl+Shift+R | Start/stop .cast recording |
| Ctrl+=/-/0 | Font size +/-/reset |
| F12 | Profiler overlay |

See [KEYBINDS.md](KEYBINDS.md) for the complete table.

## Development

```sh
cargo build -p SYNAPSE_-app              # Build
cargo run -p SYNAPSE_-app                # Run
cargo test --workspace                   # Tests (303)
cargo build --release -p SYNAPSE_-app    # Release build
cargo fmt --all -- --check               # Format
cargo clippy --workspace                 # Lint
```

## Documentation

| Document | Content |
|----------|---------|
| [CONFIGURATION.md](CONFIGURATION.md) | TOML options and custom keybinds |
| [KEYBINDS.md](KEYBINDS.md) | Complete keyboard shortcuts |
| [COMPATIBILITY.md](COMPATIBILITY.md) | OS compatibility and VT conformance |
| [BENCHMARKS.md](BENCHMARKS.md) | Performance metrics |
| [INSTALL.md](INSTALL.md) | Platform-specific install |
| [CHANGELOG.md](CHANGELOG.md) | Release history |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contributing guide |
| [docs/desarrollo/](docs/desarrollo/) | Technical docs per phase |

## License

MIT — see [LICENSE](LICENSE).
