# Contributing to Luna

## Configuración del entorno

```sh
# Clonar
git clone https://github.com/Luna/Luna.git
cd Luna

# Instalar dependencias del sistema
# Ubuntu/Debian:
sudo apt install libx11-dev libxkbcommon-dev libwayland-dev libxrandr-dev libxi-dev
# Fedora:
sudo dnf install libX11-devel libxkbcommon-devel wayland-devel
# macOS:
xcode-select --install

# Build
cargo build -p Luna-app

# Ejecutar
cargo run -p Luna-app

# Tests
cargo test --workspace

# Lint
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

## Estructura del proyecto

```
Luna/
├── crates/
│   ├── Luna-app/         # Binario principal, event loop
│   ├── Luna-terminal/    # PTY, VT parser, grid, scrollback
│   ├── Luna-renderer/    # wgpu pipeline, atlas, text, cells, UI
│   ├── Luna-ui/          # Tab bar, splitter, layout, theme
│   └── Luna-config/      # Config TOML, keybinds
└── assets/fonts/         # JetBrains Mono
```

## Convenciones de código

- Rust edition 2021, stable
- `cargo fmt` en cada commit
- Nombres de crate: `Luna-app` (Cargo.toml), `Luna_app` (en imports Rust)
- Tests unitarios en módulo `#[cfg(test)] mod tests` al final de cada archivo
- Documentación de fases en `documentacion/desarrollo/`
- Sin dependencias externas innecesarias (sin HTTP, sin DB)

## Flujo de trabajo

1. Crear rama desde `main`
2. Implementar cambios con tests
3. `cargo fmt --all && cargo clippy --workspace -- -D warnings`
4. `cargo test --workspace`
5. PR a `main`

## Reportar bugs

Abrir un issue en GitHub con:
- OS y versión
- Pasos para reproducir
- Output esperado vs observado
- Logs: `RUST_LOG=debug luna 2> luna.log`
