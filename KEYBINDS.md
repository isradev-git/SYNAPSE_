# SYNAPSE_ Keybindings Reference

> All default keybindings. Customizable via `~/.config/SYNAPSE_/config.toml`.

## Tabs

| Combo | Action |
|-------|--------|
| `Ctrl+T` | New tab |
| `Ctrl+W` | Close tab |
| `Ctrl+Tab` | Next tab |
| `Ctrl+Shift+Tab` | Previous tab |
| `Ctrl+1` | Switch to tab 1 |
| `Ctrl+2` | Switch to tab 2 |
| `Ctrl+3` | Switch to tab 3 |
| `Ctrl+4` | Switch to tab 4 |
| `Ctrl+5` | Switch to tab 5 |
| `Ctrl+6` | Switch to tab 6 |
| `Ctrl+7` | Switch to tab 7 |
| `Ctrl+8` | Switch to tab 8 |
| `Ctrl+9` | Switch to tab 9 |

## Panes (Splits)

| Combo | Action |
|-------|--------|
| `Ctrl+Shift+D` | Split vertical |
| `Ctrl+Shift+H` | Split horizontal |
| `Ctrl+Enter` | Auto split (direction based on pane shape) |
| `Ctrl+Shift+W` | Close active pane |

## Pane Navigation

| Combo | Action |
|-------|--------|
| `Ctrl+Shift+Up` | Navigate to pane above |
| `Ctrl+Shift+Down` | Navigate to pane below |
| `Ctrl+Shift+Left` | Navigate to pane left |
| `Ctrl+Shift+Right` | Navigate to pane right |

## Pane Resize

| Combo | Action |
|-------|--------|
| `Ctrl+Shift+Alt+Left` | Resize pane left (−5%) |
| `Ctrl+Shift+Alt+Right` | Resize pane right (+5%) |
| `Ctrl+Shift+Alt+Up` | Resize pane up (−5%) |
| `Ctrl+Shift+Alt+Down` | Resize pane down (+5%) |

## Font

| Combo | Action |
|-------|--------|
| `Ctrl+=` | Increase font size (+1px) |
| `Ctrl+-` | Decrease font size (−1px) |
| `Ctrl+0` | Reset font size to config default |

## Search & Navigation

| Combo | Action |
|-------|--------|
| `Ctrl+Shift+F` | Search in terminal buffer |
| `Ctrl+R` | Reverse history search |
| `Ctrl+Up` | Jump to previous prompt mark |
| `Ctrl+Down` | Jump to next prompt mark |
| `Ctrl+L` | Clear screen (send ^L to PTY) |

## Clipboard

| Combo | Action |
|-------|--------|
| `Ctrl+Shift+C` | Copy selection to clipboard |
| `Ctrl+Shift+V` | Paste from clipboard |

## Toggles

| Combo | Action |
|-------|--------|
| `Ctrl+Shift+E` | Toggle visual effects (CRT, bloom, scanlines) |
| `Ctrl+Shift+S` | Toggle status bar |
| `Ctrl+Shift+Space` | Toggle copy mode (vim-like navigation) |
| `F11` | Toggle fullscreen |

## Command Palette

| Combo | Action |
|-------|--------|
| `Ctrl+Shift+P` | Open command palette |

## Config

| Combo | Action |
|-------|--------|
| `Ctrl+,` | Reload config + open in editor |

---

## Palette Navigation (when palette is open)

| Key | Action |
|-----|--------|
| `Esc` | Close palette |
| `Enter` | Execute selected item |
| `↑` / `↓` | Move selection up/down |
| `Backspace` | Delete last character from query |
| `Any character` | Type in fuzzy search query |

## Copy Mode (when active)

| Key | Action |
|-----|--------|
| `h` / `j` / `k` / `l` | Move cursor |
| `v` | Toggle visual selection |
| `y` | Yank (copy) selection |
| `Esc` / `Ctrl+C` | Exit copy mode |

## Search Bar (when active)

| Key | Action |
|-----|--------|
| `Esc` | Close search |
| `Enter` | Next match |
| `Shift+Enter` | Previous match |
| `←` / `→` | Move cursor in search term |
| `Backspace` | Delete character |

## History Search (when active)

| Key | Action |
|-----|--------|
| `Esc` | Cancel |
| `Enter` | Execute matched command |
| `Ctrl+R` | Next older match |
| `Backspace` | Edit search term |

---

*Generated from `default_entries()` in `crates/SYNAPSE_-config/src/keybinds.rs`*
