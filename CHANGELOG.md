# Changelog

All notable changes to SYNAPSE_ are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## Pending / Roadmap

### App Icons
- **Status:** not started
- **Needs:** source PNG 1024×1024 (cyberpunk design, `#0A0C14` bg, `#FF003C` accent)
- **macOS:** convert to `.icns` via `iconutil`, embed in `.app` bundle `Info.plist`
- **Linux:** 256×256 PNG at `assets/icon.png`, loaded at runtime via `window.set_window_icon()`
- **Windows:** `.ico` via ImageMagick (deferred — Windows not an active target)
- **Code:** `build.rs` + `app.rs` `set_window_icon()` call — no code written yet

### Screenshots in README
- **Status:** not started
- **Needs:** run `cargo run -p SYNAPSE_-app`, capture screenshots showing:
  - Main terminal with cyberpunk theme
  - Split panes
  - Search bar with regex active
  - Animated GIF playback
  - Command palette
- Add images to `assets/screenshots/` and reference in `README.md`

### Variable Fonts / OpenType Features (optional)
- **Status:** not started
- Ligature support, stylistic sets, font weight variation via OpenType feature tags
- Requires `rustybuzz` or `harfbuzz-sys` for full OT shaping pipeline

---

## [Unreleased]

### Added
- **Animated GIF/APNG playback** via iTerm2 OSC 1337 protocol. Frames decoded with `image::codecs::gif::GifDecoder`, per-frame GPU texture re-upload, per-image `AnimState` tracking delays. Static images unaffected.
- **Search regex toggle** — `Ctrl+/` (or `Alt+R`) switches between literal and regex search inside the search bar. Right-side `[re]` indicator: dim = off, accent = active, red = invalid pattern.
- **Multi-window support** — launching a second `synapse_` process delegates to the existing instance via IPC. `--new-window` flag forces a fresh window. Single IPC server per user session.

### Changed
- Search bar UI: regex indicator moved to right side (always visible), no longer shifts the query text area when toggled.

### Fixed
- **Atlas LRU eviction** — compact now clears all glyph entries and resets the cursor instead of performing broken in-place relocation.

### Performance
- **Damage tracking** — GPU atlas rebuild skipped when PTY fires but the terminal grid reports no changed cells. Reduces CPU/GPU load during idle or light-output sessions.

---

## [0.2.0] — 2025

### Added
- **Copy mode** — vim-style `hjkl` navigation, `w`/`b`/`e` word motions, `v`/`V` charwise/linewise selection, `y` yank. Amber cursor rect in render; PTY cursor suppressed while active.
- **Background image per pane** — cover / contain / stretch / tile modes via config.
- **Wayland CSD** — custom title bar + window transparency on Wayland compositors.
- **Color emoji** — `ttf-parser` CBDT/CBLC bitmap extraction + shader `is_emoji` path.
- **High contrast theme** + `reduce_motion` accessibility flag.
- **Per-pane scrollbar** — click-to-jump and drag support.
- **Pane badge watermark** — CWD, title, or `user@host` overlay per pane.
- **Shell integration scripts** — zsh / bash / fish auto-install via `--setup` CLI flag.
- **Word occurrence highlight** — selected word highlighted across visible viewport.
- **OSC 52 clipboard** — bidirectional read + write via `arboard`.
- **DECSCUSR cursor shape** — reads `term.cursor_style()` per frame; respects app overrides.
- **OSC 133 mark navigation** — `JumpPrevMark` / `JumpNextMark` actions.
- **Bracketed paste** — newline sanitized to prevent multi-line injection.
- **`BellConfig`** — configurable bell behavior in TOML.
- **DECRQM mode queries** — responds to `CSI ? Ps $ p`.
- **`--check-update` flag** — one-shot update check; background check on startup (opt-out via config).
- **Kitty graphics protocol** — spec compliance audit; correct chunked transmission, placement IDs, delete operations.
- **Inline pane rename bar** — `F2` to rename active pane in-place.

### Changed
- Version bump to 0.2.0.
- README rewritten with full feature list and comparison vs Kitty / WezTerm / Alacritty / Warp / iTerm2.

### Fixed
- Tabs, fonts, cursor regressions from earlier refactor.
- Copy mode: guard `rows`/`cols == 0` in `compute_moved_cursor` and word motions.
- Copy mode: single-lock `y` arm, remove redundant clones, `Copy` derive on `CopySelMode`.

---

## [0.1.0] — Initial

- GPU-accelerated rendering via wgpu (Metal / Vulkan / OpenGL ES 3.1).
- Instanced cell rendering — one draw call, 64 bytes/cell.
- `fontdue` glyph rasterization, 2048×2048 texture atlas with LRU eviction.
- `alacritty_terminal` VTE processor + `portable-pty` PTY backend.
- Tabs, pane splits (horizontal / vertical, draggable ratio).
- Quake-style dropdown mode (`--quake`).
- Command palette.
- Session save / restore.
- 4 built-in themes: `synapse_`, `dracula`, `catppuccin-mocha`, `tokyo-night`.
- Hot-reload config (`Ctrl+,`).
- Raspberry Pi 4/5 support (OpenGL ES 3.1 via Mesa V3D).
