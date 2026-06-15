# Configuring SYNAPSE_

Configuration is a single TOML file, auto-created on first launch and hot-reloaded
with `Ctrl+,`.

| OS    | Path |
|-------|------|
| Linux | `~/.config/SYNAPSE_/config.toml` |
| macOS | `~/Library/Application Support/SYNAPSE_/config.toml` |

All keys are optional; omitted keys fall back to the defaults below.

---

## Appearance

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `font_size` | float | `14.0` | Cell font size in points. |
| `font_family` | [string] | `["JetBrains Mono"]` | Fallback chain; first family with a glyph wins. Bundled JetBrains Mono is always the final fallback. |
| `font_ligatures` | bool | `true` | Enable `calt`/`liga`/`clig` shaping (e.g. `->` → `→`). |
| `theme` | string | `"synapse_"` | One of `synapse_`, `dracula`, `catppuccin-mocha`, `tokyo-night`, `high-contrast`. |
| `cursor_style` | enum | `Block` | `Block`, `Bar`, `Underline`, `HollowBlock`, `NeonUnderbar`. |
| `cursor_blink` | bool | `true` | Blink the cursor. |
| `cursor_blink_ms` | int | `500` | Blink half-period in milliseconds. |

## Window

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `window_width` | int | `1280` | Initial width in logical pixels. |
| `window_height` | int | `800` | Initial height in logical pixels. |
| `window_opacity` | float | `1.0` | Background opacity `0.0`–`1.0`. |
| `window_blur` | bool | `false` | Native blur-behind: NSVisualEffectView (macOS), `_KDE_NET_WM_BLUR` / KWin (Linux). |

## Shell & history

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `shell_program` | string | `""` | Override shell (empty = `$SHELL` / system default). |
| `shell_args` | [string] | `[]` | Extra arguments passed to the shell. |
| `scrollback_lines` | int | `10000` | Lines of scrollback per pane. |
| `shell_integration` | bool | `false` | Auto-install OSC 133/7 hooks (also via `--setup`). |
| `persistent_history` | bool | `true` | Cross-session command history for autosuggestions. |
| `history_max_entries` | int | `10000` | Cap on stored history entries. |
| `restore_session` | bool | `false` | Restore tabs/panes on launch. |
| `session_save_interval_secs` | int | `0` | Periodic session autosave (`0` = on exit only). |

## UI chrome

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `sidebar_width` | float | `180.0` | Vertical tab sidebar width in pixels. |
| `show_pane_labels` | bool | `true` | Overlay pane labels. |
| `show_resize_indicator` | bool | `true` | Show ratios while dragging splits. |
| `sidebar_show_process_dot` | bool | `true` | Foreground-process activity dot. |
| `status_bar` | bool | `true` | Bottom status bar. |
| `status_bar_show_git` | bool | `true` | Git branch in status bar. |
| `status_bar_show_time` | bool | `true` | Clock in status bar. |
| `scrollbar` | bool | `true` | Per-pane scrollbar. |
| `pane_badge` | bool | `false` | Per-pane badge overlay. |
| `pane_badge_format` | string | `"{index}"` | Badge template. |
| `clickable_paths` | bool | `true` | Detect & click file paths / URLs. |

## Graphics & images

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `sixel_enabled` | bool | `true` | Sixel image decoding. |
| `background_image` | string? | `none` | Path to a background image. |
| `background_opacity` | float | `1.0` | Background image opacity. |
| `background_mode` | enum | `Cover` | `Cover`, `Contain`, `Tile`, `Center`. |

## Performance

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `freeze_background_tabs` | bool | `true` | Pause rendering of inactive tabs. |
| `reduce_motion` | bool | `false` | Disable animations (accessibility). |
| `check_updates_on_startup` | bool | `false` | Background GitHub update check (needs `self-update` feature). |
| `low_memory_mode` | bool | `false` | Constrained-RAM mode: caps scrollback at 5 000 lines, uses 1024×1024 glyph atlas (saves ~12 MB VRAM), disables cross-session glyph warm cache. Ideal for Pi 4 / VMs with ≤2 GB RAM. |
| `max_image_cache_mb` | int | `64` | RAM budget (MB) for protocol images (Kitty / Sixel / iTerm2). Oldest images are evicted when the budget is exceeded. |

---

## Effects (`[effects]`)

Cyberpunk GPU post-processing. The master toggle is `effects.enabled`; SYNAPSE_
**auto-disables effects on downlevel/GLES adapters** (e.g. Raspberry Pi) regardless
of this setting.

```toml
[effects]
enabled = true
hex_grid = false
pane_pulse = true
cursor_trail = 4          # 0 disables the cursor trail

[effects.scanlines]
intensity = 0.5
freq = 3.0

[effects.bloom]
threshold = 0.6
sigma = 2.0
tint = "#FF003C"

[effects.chroma]
strength = 0.0            # chromatic aberration

[effects.matrix_bg]
enabled = false
color = "#00FF66"
density = 1.0
```

---

## Bell (`[bell]`)

```toml
[bell]
visual = true             # flash on BEL
notify_unfocused = true   # desktop notification when window is unfocused
```

## Quake mode (`[quake]`)

Drop-down terminal (`synapse_ --quake`).

```toml
[quake]
enabled = false
height_percent = 0.4
animation_ms = 200
hide_on_focus_lost = true
hotkey = "F12"
```

---

## Profiles & integrations

### Tab profiles — named sessions (appear in the command palette as `Profile:`)
```toml
[[tab_profile]]
name = "server"
shell = "/bin/bash"
shell_args = ["-l"]
cwd = "/srv/app"
env = { RAILS_ENV = "production" }
```

### SSH profiles
```toml
[[ssh_profile]]
name = "edge"
host = "edge.example.com"
port = 22
identity_file = "~/.ssh/id_ed25519"
forward_agent = true
extra_args = ["-C"]
```

### Plugin commands — bind shell commands to keys / palette
```toml
[[plugins]]
name = "Open lazygit"
keybind = "ctrl+g"
command = "lazygit"
split = "vertical"        # none | horizontal | vertical
replace_selection = false
```

See [KEYBINDS.md](KEYBINDS.md) for the full keybinding reference.
