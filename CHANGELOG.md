# Changelog

All notable changes to SYNAPSE_ are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## Pending / Roadmap

Optional, post-1.0 nice-to-haves:
- **Screenshots in README** — capture the cyberpunk theme, splits, search regex, GIF playback and command palette into `assets/screenshots/` (needs a display).
- **Variable fonts / OpenType features** — stylistic sets and weight axes via OT feature tags.
- **Kitty graphics** — `t=s` (shared memory) and `U=1` (unicode placeholders).
- **BiDi** — visual-order selection/copy (currently logical order).

---

## [1.0.0] — 2026-06-15

First stable release. Native on **macOS · Linux · Raspberry Pi 4/5**.

### Added
- **Procedural app icon** — neon `>_` prompt drawn at runtime (no asset dependency), set as the window icon. Export packaging PNGs with `--export-icon <path> [--icon-size N]`.
- **BiDi / RTL text** — `unicode-bidi` visual reordering (UAX #9) with Arabic/Hebrew joining via rustybuzz. Gated on RTL codepoints so left-to-right text keeps the fast path. Requires an RTL-capable font in `font_family`.
- **Documentation** — `INSTALL.md`, `CONFIGURATION.md`, `COMPATIBILITY.md`, `docs/BENCHMARKS.md`.
- From the 0.2.x sprint: animated GIF/APNG playback (OSC 1337), search regex toggle (`Ctrl+/`), multi-window via IPC, Linux window blur, Neuromancer boot screen, tab profiles, warm glyph atlas.

### Changed
- **Kitty graphics protocol — completed.** Chunked transmission (`m=1`) now preserves the image id and action across follow-up chunks, so `kitten icat` images display correctly; added zlib payloads (`o=z`) and file / temp-file media (`t=f` / `t=t`). Query responses were already handled in the reader thread.
- Search bar regex indicator moved to the right side (always visible).

### Fixed
- **Build fix** — `raw-window-handle` API drift: `XlibWindowHandle.window` is now a `u64` (the Linux `window_blur` path failed to compile).
- **Atlas LRU eviction** — `compact` now clears all glyph entries and resets the cursor instead of a broken in-place relocation.

### Performance
- **Terminal responses sent synchronously** — DSR/device-attribute replies are written from the reader thread immediately instead of being deferred to the main thread's per-frame event drain, removing up to a frame of latency for apps that block on the reply (vim, tmux).
- **Event channel is now unbounded** — terminal events (incl. response writes) are never silently dropped under bursts.
- **Quieter logging** — default log filter is `warn` (our crates `info`); the per-frame wgpu device-maintain spam and per-second FPS line no longer flood the console (FPS only logs with the profiler overlay).
- **Damage tracking** — GPU atlas rebuild skipped when the PTY fires but the terminal grid reports no changed cells. Reduces CPU/GPU load during idle or light-output sessions.

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
