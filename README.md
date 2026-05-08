# Luna

> Terminal emulator GPU-accelerated · Rust · wgpu · Multiplataforma

Luna es un emulador de terminal moderno con rendering por GPU (Vulkan/Metal/DirectX 12), soporte completo VT100/xterm-256color, paneles divididos, tabs, y configuración en TOML.

## Estado

Fase 10 (Calidad y Documentación) — MVP funcional con tabs, splits, búsqueda y configuración.

## Stack

| Componente    | Tecnología                        |
|---------------|-----------------------------------|
| Lenguaje      | Rust (stable, edition 2021)       |
| Windowing     | winit 0.30                        |
| GPU Rendering | wgpu 22 (Vulkan/Metal/DX12)       |
| Text Shaping  | cosmic-text 0.12 + JetBrains Mono |
| PTY           | portable-pty 0.8                  |
| VT Parser     | vte 0.13                          |
| Async I/O     | tokio 1                           |
| Config        | serde + TOML                      |

## Características

- **Rendimiento GPU**: 60fps estables, latencia input→render <5ms
- **Splits y tabs**: Árbol binario de paneles, tabs con títulos dinámicos
- **VT100/xterm**: C0, CSI, SGR (256-color + true color), OSC, ESC
- **Scrollback**: 100.000 líneas configurables
- **Búsqueda**: Ctrl+Shift+F (buffer) + Ctrl+R (historial)
- **Fuente dinámica**: Ctrl+=/-/0 en runtime
- **Config TOML**: ~/.config/Luna/config.toml con keybinds personalizables
- **Multiplataforma**: Linux (X11/Wayland), macOS (Metal), Windows (DX12)

## Instalación rápida

```sh
# Desde source
git clone https://github.com/Luna/Luna.git
cd Luna
cargo build --release -p Luna-app
./target/release/luna

# Linux (precompilado)
curl -fsSL https://github.com/Luna/Luna/releases/latest/download/Luna-app-installer.sh | sh

# macOS (Homebrew)
brew install Luna/Luna/luna
```

## Documentación

| Documento                                           | Contenido                                |
|-----------------------------------------------------|------------------------------------------|
| [CONFIGURATION.md](CONFIGURATION.md)                | Opciones TOML y keybinds personalizados  |
| [KEYBINDS.md](KEYBINDS.md)                          | Tabla completa de atajos de teclado      |
| [COMPATIBILITY.md](COMPATIBILITY.md)                | Compatibilidad por OS y conformidad VT   |
| [BENCHMARKS.md](BENCHMARKS.md)                      | Métricas de rendimiento                  |
| [CHANGELOG.md](CHANGELOG.md)                        | Historial de cambios                     |
| [CONTRIBUTING.md](CONTRIBUTING.md)                  | Guía para contribuir                     |
| [documentacion/desarrollo/](documentacion/desarrollo/)| Docs técnicos por fase                 |

## Comandos de desarrollo

```sh
cargo build -p Luna-app          # Build
cargo run -p Luna-app            # Ejecutar
cargo test --workspace           # Tests (67 unit tests)
cargo fmt --all -- --check       # Formato
cargo clippy --workspace         # Lint
```

## Licencia

MIT — ver [LICENSE](LICENSE).
