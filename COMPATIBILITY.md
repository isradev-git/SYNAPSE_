# Luna — Compatibilidad por Sistema Operativo

## Linux

| Distro       | Estado    | Notas                                              |
|-------------|-----------|-----------------------------------------------------|
| Ubuntu 22.04| ✅        | Target principal. X11 y Wayland (winit).           |
| Fedora 40   | ✅        | Requiere dependencias de desarrollo.                |
| Arch Linux  | ✅        | Funciona con X11/Wayland estándar.                  |
| Debian 12   | ✅        | Igual que Ubuntu.                                   |

### Dependencias en Linux

```sh
# Ubuntu/Debian
sudo apt install libx11-dev libxkbcommon-dev libwayland-dev libxrandr-dev libxi-dev

# Fedora
sudo dnf install libX11-devel libxkbcommon-devel wayland-devel

# Arch
sudo pacman -S libx11 libxkbcommon wayland
```

---

## macOS

| Versión     | Estado    | Notas                                              |
|-------------|-----------|-----------------------------------------------------|
| macOS 14+   | ✅        | Metal backend via wgpu. HiDPI nativo.              |
| macOS 12-13 | ✅        | Requiere al menos macOS 12.                         |

### Notas
- Binario universal: x86_64 + arm64
- HiDPI (Retina) soportado nativamente por winit/wgpu
- Para desarrollo: `xcode-select --install`

---

## Windows

| Versión     | Estado    | Notas                                              |
|-------------|-----------|-----------------------------------------------------|
| Windows 11  | ✅        | DirectX 12 backend via wgpu.                        |
| Windows 10  | ✅        | DirectX 12 o Vulkan si está disponible.             |

### Notas
- Shell por defecto: `cmd.exe` (se detecta automáticamente)
- PowerShell también soportado
- Codificación: UTF-8 por defecto, CP1252 como fallback

---

## Conformidad VT/xterm

| Categoría              | Estado | Notas                              |
|------------------------|--------|-------------------------------------|
| C0 Controls            | ✅     | LF, CR, BS, HT, FF, BEL            |
| CSI Cursor Movement    | ✅     | CUU/CUD/CUF/CUB, CUP, CHA, VPA, CNL, CPL |
| CSI Erase              | ✅     | ED (0/1/2/3), EL (0/1/2), ECH      |
| CSI Insert/Delete      | ✅     | ICH, DCH, IL, DL (scroll-region aware) |
| SGR Colors             | ✅     | 8-color, bright, 256-color, true color |
| SGR Attributes         | ✅     | Bold, Italic, Underline, Blink, Inverse, Invisible |
| Save/Restore Cursor    | ✅     | ESC 7/8, CSI s/u                   |
| RIS (Reset)            | ✅     | ESC c (resets region + modes)      |
| OSC Title              | ✅     | OSC 0, OSC 2                       |
| OSC CWD                | ✅     | OSC 7                              |
| Scroll Regions         | ✅     | DECSTBM (CSI r), SU (CSI S), SD (CSI T) |
| Line Drawing Chars     | ✅     | DEC Special Graphics (ESC(0/B, SO/SI) |
| Mouse Reporting        | ✅     | X10, button-motion, any-motion, SGR |
| Bracketed Paste        | ✅     | `\e[200~` ... `\e[201~`            |
| Focus Events           | ✅     | `?1004h/l`                         |
| Application Cursor     | ✅     | DECCKM (`?1h/l`), arrows send `\eOA`–`D` |
| Double Height/Width    | ⬜     | Pendiente                          |
| Kitty Keyboard Protocol| ⬜     | Pendiente (R-022)                  |

---

## Shells detectados por OS

| OS      | Shell por defecto       | Fallback              |
|---------|------------------------|-----------------------|
| Linux   | `$SHELL`               | `/bin/bash`           |
| macOS   | `$SHELL`               | `/bin/zsh`            |
| Windows | `cmd.exe`              | `powershell.exe`      |
