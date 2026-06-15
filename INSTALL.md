# Installing SYNAPSE_

SYNAPSE_ is a GPU-accelerated terminal emulator built in Rust. It runs natively on
**macOS** (Metal) and **Linux** (X11 / Wayland, Vulkan or GLES 3.1), including
**Raspberry Pi 4/5** (Mesa V3D).

> Windows is **not supported**.

---

## 1. Prerequisites

### Toolchain
- [Rust](https://rustup.rs) 1.77 or newer (`rustup` recommended).

### Linux build dependencies
```sh
# Debian / Ubuntu
sudo apt install libx11-dev libxkbcommon-dev libwayland-dev libxrandr-dev libxi-dev

# Fedora
sudo dnf install libX11-devel libxkbcommon-devel wayland-devel libXrandr-devel libXi-devel
```

### Raspberry Pi (4 / 5 only)
Ensure the V3D driver is active (`raspi-config` → Advanced → GL Driver) and the same
X11/Wayland dev packages as Linux are installed. Pi 3 and older are **not supported**
(VideoCore IV is GLES 2.0; wgpu needs GLES 3.1).

### macOS
No extra system packages — the Xcode command-line tools (`xcode-select --install`)
provide everything needed.

---

## 2. Build from source

```sh
git clone https://github.com/isradev-git/synapse_
cd synapse_

cargo build --release            # optimized binary
cargo run   -p SYNAPSE_-app      # build + run
```

The binary is produced at `target/release/synapse_`.

---

## 3. Shell integration (optional)

Enables OSC 133 prompt marks, OSC 7 working-directory tracking and command history:

```sh
synapse_ --setup           # installs the zsh/bash/fish hooks for your shell
```

---

## 4. App icon for packaging

The window icon is generated procedurally at runtime — no asset needed. To produce
PNGs for `.app` / `.desktop` packaging:

```sh
synapse_ --export-icon icon.png --icon-size 512
```

- **Linux** (`.desktop`): install `icon.png` to `~/.local/share/icons/`.
- **macOS** (`.icns`): `iconutil -c icns icon.iconset` from a generated icon set.

---

## 5. Configuration

A default config is written on first launch. Hot-reload with `Ctrl+,`.

| OS    | Path |
|-------|------|
| Linux | `~/.config/SYNAPSE_/config.toml` |
| macOS | `~/Library/Application Support/SYNAPSE_/config.toml` |

See [CONFIGURATION.md](CONFIGURATION.md) for every option and
[COMPATIBILITY.md](COMPATIBILITY.md) for terminal feature support.
