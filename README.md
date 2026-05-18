# SYNAPSE_

> Terminal emulator GPU-accelerated · Rust · wgpu · Multiplataforma

[![CI](https://github.com/isradev-git/synapse_/actions/workflows/ci.yml/badge.svg)](https://github.com/isradev-git/synapse_/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/isradev-git/synapse_)](https://github.com/isradev-git/synapse_/releases/latest)

SYNAPSE_ es un emulador de terminal moderno con rendering por GPU (Vulkan/Metal/DirectX 12), soporte completo VT100/xterm-256color, paneles divididos, tabs, y configuración en TOML.

## Estado

Fase 10 (Calidad y Documentación) — MVP funcional con tabs, splits, búsqueda y configuración.

## Stack

| Componente    | Tecnología                        |
|---------------|-----------------------------------|
| Lenguaje      | Rust (stable, edition 2021)       |
| Windowing     | winit 0.30                        |
| GPU Rendering | wgpu 22 (Metal/Vulkan)            |
| Text Shaping  | fontdue 0.9 + rustybuzz 0.14      |
| PTY           | portable-pty 0.8                  |
| VT Parser     | alacritty_terminal 0.24           |
| Async I/O     | tokio 1                           |
| Config        | serde + TOML                      |

## Características

- **Rendimiento GPU**: 60fps estables, latencia input→render <5ms
- **Splits y tabs**: Árbol binario de paneles, tabs con títulos dinámicos
- **VT100/xterm**: C0, CSI, SGR (256-color + true color), OSC, ESC
- **Scrollback**: 100.000 líneas configurables
- **Búsqueda**: Ctrl+Shift+F (buffer) + Ctrl+R (historial)
- **Fuente dinámica**: Ctrl+=/-/0 en runtime
- **Config TOML**: ~/.config/SYNAPSE_/config.toml con keybinds personalizables
- **Plataformas**: macOS (Metal), Linux (X11/Wayland) y Windows (DirectX 12)

## Instalación rápida

```sh
# Linux / macOS
curl -fsSL https://github.com/isradev-git/synapse_/releases/latest/download/SYNAPSE_-app-installer.sh | sh

# Windows (PowerShell)
irm https://github.com/isradev-git/synapse_/releases/latest/download/SYNAPSE_-app-installer.ps1 | iex
```

También disponible como `.msi` (Windows) y tarballs en la [página de releases](https://github.com/isradev-git/synapse_/releases/latest).

Ver [INSTALL.md](INSTALL.md) para instrucciones detalladas por plataforma, compilar desde source, y configuración inicial.

## Documentación

| Documento                                           | Contenido                                |
|-----------------------------------------------------|------------------------------------------|
| [CONFIGURATION.md](CONFIGURATION.md)                | Opciones TOML y keybinds personalizados  |
| [KEYBINDS.md](KEYBINDS.md)                          | Tabla completa de atajos de teclado      |
| [COMPATIBILITY.md](COMPATIBILITY.md)                | Compatibilidad por OS y conformidad VT   |
| [BENCHMARKS.md](BENCHMARKS.md)                      | Métricas de rendimiento                  |
| [CHANGELOG.md](CHANGELOG.md)                        | Historial de cambios                     |
| [CONTRIBUTING.md](CONTRIBUTING.md)                  | Guía para contribuir                     |
| [docs/desarrollo/](docs/desarrollo/)                | Docs técnicos por fase                 |

## Comandos de desarrollo

```sh
cargo build -p SYNAPSE_-app          # Build
cargo run -p SYNAPSE_-app            # Ejecutar
cargo test --workspace               # Tests (103 unit tests)
cargo fmt --all -- --check           # Formato
cargo clippy --workspace             # Lint
```

## Licencia

MIT — ver [LICENSE](LICENSE).
