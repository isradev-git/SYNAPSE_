# Contributing to SYNAPSE_

## Plataformas soportadas

**Activo:** macOS (Metal) + Linux (X11/Wayland).
**Windows:** no soportado actualmente — contribuciones aceptadas pero sin garantía de mantenimiento.

## Configuración del entorno

```sh
# Clonar
git clone https://github.com/isradev-git/synapse_.git
cd synapse_

# Instalar dependencias del sistema
# Ubuntu/Debian:
sudo apt install libx11-dev libxkbcommon-dev libwayland-dev libxrandr-dev libxi-dev
# Fedora:
sudo dnf install libX11-devel libxkbcommon-devel wayland-devel
# macOS:
xcode-select --install

# Build
cargo build -p SYNAPSE_-app

# Ejecutar
cargo run -p SYNAPSE_-app

# Tests
cargo test --workspace

# Lint
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

## Estructura del proyecto

```
SYNAPSE_/
├── crates/
│   ├── SYNAPSE_-app/         # Binario principal, event loop
│   ├── SYNAPSE_-renderer/    # wgpu pipeline, atlas, text, cells, UI
│   ├── SYNAPSE_-ui/          # Tab bar, splitter, layout, theme
│   ├── SYNAPSE_-config/      # Config TOML, keybinds
│   └── SYNAPSE_-suggest/     # Autosuggestions, shell history
└── assets/fonts/             # JetBrains Mono
```

## Convenciones de código

- Rust edition 2021, stable
- `cargo fmt` en cada commit
- Nombres de crate: `SYNAPSE_-app` (Cargo.toml), `synapse_app` (en imports Rust)
- Tests unitarios en módulo `#[cfg(test)] mod tests` al final de cada archivo
- Documentación de fases en `docs/desarrollo/`
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
- Logs: `RUST_LOG=debug synapse_ 2> synapse_.log`
