# SYNAPSE_

> **The terminal that feels like Blade Runner.**
> GPU-accelerated · Cyberpunk aesthetics · Zero compromises on speed.

[![CI](https://github.com/isradev-git/synapse_/actions/workflows/ci.yml/badge.svg)](https://github.com/isradev-git/synapse_/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/isradev-git/synapse_)](https://github.com/isradev-git/synapse_/releases/latest)
[![Tests](https://img.shields.io/badge/tests-303%20passed-brightgreen)]()
[![License](https://img.shields.io/badge/license-MIT-blue)]()
[![Rust](https://img.shields.io/badge/rust-stable%202021-orange)]()

---

SYNAPSE_ is a GPU-rendered terminal emulator built in Rust. It renders every frame through a wgpu pipeline with real GPU post-processing shaders — CRT scanlines, neon bloom, chromatic aberration, matrix rain — all toggleable with a single key, all at 60fps.

It is fast enough for `cat 1GB-log.txt`. Visual enough to look like a hacking scene. Configurable enough to disappear and just be your terminal.

**v0.2.0 — 303 tests · 0 warnings · macOS + Linux**

---

## Why SYNAPSE_

Every other terminal is either fast or beautiful. None ships with a built-in GPU post-processing pipeline.

| | SYNAPSE_ | Kitty | WezTerm | Alacritty | Ghostty | Warp |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| **GPU postproc shaders** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **CRT / bloom / matrix rain** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Font ligatures** | ✅ | ✅ | ✅ | ❌ | ✅ | ❌ |
| **Color emoji (CBDT/CBLC)** | ✅ | ✅ | ✅ | ❌ | ✅ | ❌ |
| **Sixel images** | ✅ | ✅ | ✅ | ❌ | ✅ | ❌ |
| **Kitty graphics protocol** | ✅ | ✅ | ✅ | ❌ | ✅ | ❌ |
| **iTerm2 inline images** | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| **Split panes** | ✅ | ✅ | ✅ | ❌ | ✅ | ✅ |
| **Named workspaces** | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| **Broadcast input** | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Session save/restore** | ✅ | ❌ | ✅ | ❌ | ❌ | ✅ |
| **Recording (.cast)** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **OSC 133 semantic prompts** | ✅ | ✅ | ✅ | ❌ | ✅ | ✅ |
| **OSC 52 remote clipboard** | ✅ | ✅ | ✅ | ❌ | ✅ | ❌ |
| **Command palette** | ✅ | ✅ | ✅ | ❌ | ❌ | ✅ |
| **Quake dropdown mode** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Word occurrence highlight** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Plugin system (TOML)** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Background image per pane** | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Per-pane scrollbar** | ✅ | ❌ | ❌ | ❌ | ❌ | ✅ |
| **High contrast a11y theme** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **reduce_motion config** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Open source** | MIT | GPLv3 | MIT | Apache 2 | MIT | ❌ |
| **Telemetry** | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |

---

## Performance

Targets are hard numbers, not aspirational marketing.

| Metric | Target | Notes |
|--------|--------|-------|
| Input → render latency | **< 5ms** | Measured wgpu submit → present |
| Frame rate | **60fps stable** | Instanced draw, one submit/frame |
| Startup (cold) | **< 200ms** | Time to first interactive prompt |
| RAM (idle, 1 pane) | **< 50MB** | No resident JVM, no Electron |
| Scrollback | **100,000 lines** | Circular buffer, no disk paging |

---

## GPU Post-Processing Shaders

SYNAPSE_ has a second render pass that transforms every frame through configurable GPU effects. All effects are **off by default** — zero overhead when disabled.

Toggle everything with `Ctrl+Shift+E`. Configure in TOML:

```toml
[effects]
enabled = true

[effects.scanlines]
intensity = 0.3      # CRT scanline darkness (0.0–1.0)
freq = 2.0           # Lines per pixel-row

[effects.bloom]
threshold = 0.7      # Brightness cutoff for glow
sigma = 4.0          # Blur radius
tint = "#FF003C"     # Neon red glow (cyberpunk default)

[effects.chroma]
strength = 0.002     # RGB channel split at edges

[effects.matrix_bg]
enabled = false      # Katakana/ASCII rain behind cells
color = "#00FF55"    # Rain color
density = 0.3        # Character density

hex_grid = false     # Animated hexagonal mesh background
pane_pulse = false   # Neon border pulse (sin wave, 2s period)
cursor_trail = 0     # Frames of alpha-decaying cursor history
```

**Effects implemented:**
- **CRT scanlines** — horizontal scan lines + vignette
- **Bloom / neon glow** — 2-pass gaussian blur, threshold-masked, color-tinted
- **Chromatic aberration** — RGB channel split radially from center
- **Glitch / datamosh** — random horizontal shifts, triggered on pane focus or manually
- **Matrix rain** — katakana + ASCII characters falling behind the cell layer
- **Hex grid** — animated hexagonal mesh pulse
- **Pane border pulse** — active pane border oscillates opacity with sine wave
- **Cursor trail** — N ghost frames with alpha decay

---

## Features

### Core Terminal

- **Instanced GPU rendering** — one draw call per frame, 64-byte cell instances
- **Full VT100/xterm-256color** — C0, CSI, SGR true color (16M), OSC full set, DECSCUSR, mouse reporting
- **Copy mode** — vim-style navigation: `hjkl`, `v`/`V`, `y`, `/` search, `Ctrl+Shift+Space` to enter
- **Scrollback** — 100,000 lines, circular buffer
- **Frame cache** — GPU instance cache, dirty tracking per grid row via `alacritty_terminal::Term::damage()`
- **Atlas LRU eviction** — glyph atlas auto-evicts on overflow, dual-atlas for zero-pause resize

### Splits, Tabs, Workspaces

- **Split panes** — binary tree of splits (horizontal/vertical), drag-to-resize dividers
- **Pane zoom** — maximize any pane to full window (Ctrl+Shift+Z), layout preserved
- **Named workspaces** — Ctrl+Shift+N creates workspace, switch instantly between "dev", "ssh", "logs"
- **Pane badge** — semi-transparent CWD / title / `user@host` watermark per pane
- **Background image** — per-pane wallpaper with `cover`/`contain`/`stretch`/`tile` + opacity
- **Scrollbar** — per-pane thin track (6px), click-to-jump, drag

### Text & Typography

- **Font ligatures** — rustybuzz/HarfBuzz shaping, `calt/liga/clig` on by default
- **Font fallback chain** — `["JetBrains Mono", "Symbols Nerd Font", "Noto Color Emoji"]`, per-codepoint routing
- **Dynamic font size** — Ctrl+=/-/0 at runtime, no restart
- **Color emoji** — CBDT/CBLC bitmap emoji via ttf-parser, real RGBA in atlas with dedicated shader path
- **Bold/italic** — separate font style lookup for SGR 1/3

### Search & Selection

- **Regex search** — Ctrl+Shift+F, Alt+R toggles regex mode, match highlighting
- **History search** — Ctrl+R fuzzy-search across persistent cross-session history
- **Word occurrence highlight** — select a word, all visible occurrences highlight (VS Code-style)
- **Smart selection** — double-click word, triple-click line, Shift+click extend, Alt+click block
- **URL/path detection** — auto-detect URLs + `file:line` paths, Ctrl+click opens in `$EDITOR`/browser

### Images & Graphics

- **Sixel decoder** — native Rust, no external deps, full color register support
- **iTerm2 inline images** — OSC 1337 `File=inline=1` with base64 PNG/JPEG/GIF, auto-scroll
- **Kitty graphics protocol** — APC transmission, placement with z-order, delete commands

### Productivity

- **Broadcast input** — Ctrl+Shift+B sends keystrokes to all panes in the tab (SSH multi-server)
- **Command palette** — Ctrl+Shift+P: fuzzy search actions, tabs, themes, keybinds, plugins
- **Quake dropdown** — Ctrl+Space slides SYNAPSE_ down from screen top with ease-out animation, auto-hides on focus loss
- **Session save/restore** — tabs, panes, CWDs saved to `~/.cache/SYNAPSE_/session.json` on exit
- **Recording (.cast)** — Ctrl+Shift+R starts/stops asciinema-compatible session export
- **Persistent history** — cross-session MRU command history with deduplication, OSC 133-aware
- **Drag & drop** — drop files to paste their absolute path

### Shell Integration & Extensibility

- **Shell integration** — official zsh/bash/fish scripts with OSC 133 (prompt marks), OSC 7 (CWD), `--setup` CLI installer
- **Plugin system** — TOML-defined keybind → shell command with `$CURRENT_PANE_CWD`, `$SELECTED_TEXT`, `$CLIPBOARD`
- **Suggestion engine** — built-in frequency trie (`crates/SYNAPSE_-suggest`) for ghost-text autocomplete
- **OSC 9/777 notifications** — desktop notifications from PTY, forwarded via `notify-rust`
- **OSC 52** — remote clipboard read/write (SSH + Neovim workflow)
- **OSC 133** — semantic prompt/output marks + `Ctrl+Up/Down` to jump between prompts

### UI & Status

- **Status bar** — clock, git branch, K8s context, CWD, broadcast indicator, recording indicator (Ctrl+Shift+S toggle)
- **Window transparency** — `window_opacity` config, Wayland compositor alpha
- **Wayland CSD** — client-side decorations with custom title bar and drag support
- **Profiler overlay** — F12 shows FPS, frame time, PTY bytes/s, cell count, atlas utilization %

### Accessibility

- **High contrast theme** — built-in `high-contrast-dark`: black background, white text, yellow accent
- **`reduce_motion = true`** — disables all animations (splash, cursor blink, pane pulse, resize indicators)

### Performance Features

- **Background tab freeze** — pause PTY reads on non-visible tabs, save CPU/battery
- **Frame cache** — skip GPU upload when grid is unchanged

---

## Quick Start

```sh
# Build from source
cargo build --release -p SYNAPSE_-app
./target/release/synapse_

# Install shell integration (sets up OSC 133/7, adds to your shell rc)
./target/release/synapse_ --setup

# Launch with a command
./target/release/synapse_ -e "nvim ~/.config/SYNAPSE_/config.toml"

# Quake mode (fullscreen dropdown)
./target/release/synapse_ --quake

# Restore last session
./target/release/synapse_ --restore
```

### System dependencies (Linux)

```sh
# Ubuntu/Debian
sudo apt install libx11-dev libxkbcommon-dev libwayland-dev libxrandr-dev libxi-dev

# Fedora
sudo dnf install libX11-devel libxkbcommon-devel wayland-devel
```

---

## Configuration

Config auto-created at first launch:
- **Linux:** `~/.config/SYNAPSE_/config.toml`
- **macOS:** `~/Library/Application Support/SYNAPSE_/config.toml`

Hot-reload: `Ctrl+,`

```toml
# Typography
font_size = 14
font_family = ["JetBrains Mono", "Symbols Nerd Font", "Noto Color Emoji"]
font_ligatures = true

# Theme (synapse_ | dracula | catppuccin-mocha | tokyo-night | high-contrast-dark)
theme = "synapse_"
scrollback_lines = 100000

# Shell integration + history
shell_integration = true
persistent_history = true

# Pane UI
scrollbar = true
pane_badge = true
pane_badge_format = "{cwd}"   # {cwd} | {title} | {user@host}

# Background image (per-pane wallpaper)
background_image = "/path/to/wallpaper.png"
background_opacity = 0.3
background_mode = "cover"     # cover | contain | stretch | tile

# Window
window_opacity = 0.95

# Visual effects (GPU postproc — zero cost when disabled)
[effects]
enabled = false
pane_pulse = false
cursor_trail = 0

[effects.scanlines]
intensity = 0.3

[effects.bloom]
tint = "#FF003C"
```

---

## Keybinds

| Shortcut | Action |
|----------|--------|
| `Ctrl+T` | New tab |
| `Ctrl+W` | Close tab |
| `Ctrl+Shift+D` | Split vertical |
| `Ctrl+Shift+H` | Split horizontal |
| `Ctrl+Shift+Z` | Pane zoom (toggle maximize) |
| `Ctrl+Shift+W` | Close pane |
| `Ctrl+Shift+↑↓←→` | Navigate panes |
| `Ctrl+Shift+Alt+↑↓←→` | Resize pane (±5%) |
| `Ctrl+Shift+Space` | Copy mode (vim navigation) |
| `Ctrl+Shift+F` | Search buffer (Alt+R = regex) |
| `Ctrl+R` | History search |
| `Ctrl+Up / Ctrl+Down` | Jump to prev/next prompt mark |
| `Ctrl+Shift+P` | Command palette |
| `Ctrl+Shift+N` | New workspace |
| `Ctrl+Space` | Quake mode toggle |
| `Ctrl+Shift+B` | Broadcast input (all panes) |
| `Ctrl+Shift+R` | Start/stop `.cast` recording |
| `Ctrl+Shift+E` | Toggle GPU effects |
| `Ctrl+Shift+S` | Toggle status bar |
| `Ctrl+=/-/0` | Font size +/−/reset |
| `Ctrl+,` | Reload config + open in editor |
| `F11` | Fullscreen |
| `F12` | Profiler overlay |

See [KEYBINDS.md](KEYBINDS.md) for the complete reference.

---

## Architecture

Cargo workspace — 5 crates, ~21k LOC, zero unsafe except FFI boundaries.

```
SYNAPSE_-app        binary, winit event loop, render orchestration   ~6,200 LOC
  ├─ render.rs      main render loop, PTY→VT→GPU pipeline            ~3,100 LOC
  ├─ palette.rs     command palette state + fuzzy search
  ├─ quake.rs       dropdown animation (ease-out, configurable ms)
  ├─ session.rs     save/restore layout to JSON
  ├─ record.rs      asciinema .cast export
  └─ sixel.rs       sixel decoder (native Rust)

SYNAPSE_-renderer   wgpu pipelines, texture atlas, text shaping      ~2,000 LOC
  ├─ renderer.rs    surface, device, draw_frame, offscreen pass
  ├─ postproc.rs    PostProcRenderer — effects uniform, bloom pass
  ├─ text.rs        rustybuzz shaping + fontdue raster
  └─ atlas.rs       2048² RGBA glyph atlas with LRU eviction

SYNAPSE_-ui         layout, pane tree, tab bar, theme                ~1,300 LOC
SYNAPSE_-config     TOML config, keybinds, effects, themes           ~1,340 LOC
SYNAPSE_-suggest    frequency trie, builtins, ghost text              ~740 LOC
```

### Render pipeline

```
Frame N:
  Pass 0  →  offscreen texture (cells + UI + underlines + images + cursor)
  Pass 1a →  bloom: threshold + downsample 4× + gaussian H  (bloom_h.wgsl)
  Pass 1b →  bloom: gaussian V + upsample additive
  Pass 2  →  postproc.wgsl: scanlines + bloom composite + chroma + glitch + matrix → surface
```

### Data flow

```
PTY (portable-pty) → reader thread → mpsc channel
  → alacritty_terminal VTE processor → Term grid
  → render.rs reads damage() → CellInstance vec → draw_frame()
  → CellRenderer (cell.wgsl) + UIRenderer (ui.wgsl) → wgpu surface
```

---

## Development

```sh
cargo build -p SYNAPSE_-app              # debug build
cargo run -p SYNAPSE_-app                # run
cargo build --release -p SYNAPSE_-app    # release (thin LTO)
cargo test --workspace                   # all 353 tests
cargo fmt --all -- --check               # format check
cargo clippy --workspace --all-targets -- -D warnings  # lint (warnings = errors)
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for platform setup, code conventions, and PR guidelines.

---

## Documentation

| Doc | Contents |
|-----|----------|
| [INSTALL.md](INSTALL.md) | Build from source (macOS / Linux / Raspberry Pi), packaging |
| [CONFIGURATION.md](CONFIGURATION.md) | Every `config.toml` option with defaults |
| [COMPATIBILITY.md](COMPATIBILITY.md) | Platform matrix, escape sequences, graphics protocols |
| [KEYBINDS.md](KEYBINDS.md) | Complete keybinding reference |
| [docs/BENCHMARKS.md](docs/BENCHMARKS.md) | Performance targets + how to measure |
| [CHANGELOG.md](CHANGELOG.md) | Release history |

---

## Roadmap

v1.0 ships native **macOS · Linux · Raspberry Pi 4/5** with window blur, IPC daemon,
SSH/tab profiles, app icon, BiDi/RTL shaping and a complete Kitty graphics
implementation. See [CHANGELOG.md](CHANGELOG.md) for the full history and
[docs/PENDIENTES.md](docs/PENDIENTES.md) for remaining nice-to-haves
(Kitty shared-memory & unicode placeholders, visual-order BiDi selection).
Windows is not supported.

---

## License

MIT — see [LICENSE](LICENSE).

Built with [wgpu](https://wgpu.rs), [winit](https://github.com/rust-windowing/winit), [alacritty_terminal](https://github.com/alacritty/alacritty), [rustybuzz](https://github.com/harfbuzz/rustybuzz), [fontdue](https://github.com/mooman219/fontdue), [portable-pty](https://github.com/wezterm/wezterm/tree/main/pty).
