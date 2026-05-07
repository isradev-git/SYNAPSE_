# Fase 0 — Entorno y Scaffolding

**Rust:** 1.95.0 (2021 edition)

## T-001 · Rust toolchain

- `rustup` instalado
- `rustc 1.95.0 + cargo 1.95.0`
- Herramientas dev: `cargo-watch`, `cargo-deny`, `cargo-dist`
- Targets cross-platform añadidos: `x86_64-pc-windows-msvc`, `x86_64-apple-darwin`, `aarch64-apple-darwin`

## T-002 · Workspace Cargo

```
Cargo.toml (workspace root)
crates/
├── Luna-app/           → binario "luna"     (entry point)
├── Luna-terminal/      → lib "luna_terminal" (PTY + VT parser + grid)
├── Luna-renderer/      → lib "luna_renderer" (wgpu + cosmic-text)
├── Luna-ui/            → lib "luna_ui"       (tabs, paneles, theme)
└── Luna-config/        → lib "luna_config"   (TOML, keybinds)
```

`cargo build` compila los 5 crates sin errores.

## T-003 · Dependencias compartidas

En `[workspace.dependencies]` del `Cargo.toml` raíz:

| Crate | Versión |
|---|---|
| `winit` | 0.30 |
| `wgpu` | 22 |
| `cosmic-text` | 0.12 |
| `portable-pty` | 0.8 |
| `vte` | 0.13 |
| `tokio` | 1 (features = ["full"]) |
| `serde` | 1 (features = ["derive"]) |
| `toml` | 0.8 |
| `arboard` | 3 |
| `clap` | 4 (features = ["derive"]) |
| `bitflags` | 2 |
| `tracing` | 0.1 |
| `tracing-subscriber` | 0.3 |
| `pollster` | 0.3 |
| `bytemuck` | 1 (features = ["derive"]) |

Cada crate hereda via `{ workspace = true }`. `cargo check` pasa.

## T-004 · Repositorio git

- `git init` en raíz
- `.gitignore`: `/target`, `/dist`, `*.log`, `*.tmp`
- `README.md`: descripción + stack + build instructions
- Commit inicial: `chore: initial project scaffold (workspace + 5 crates + deps)`

## T-005 · Release profile + linker

```toml
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
```

Linker `mold` configurado para Linux en `.cargo/config.toml`. `cargo build --release` verificado.

## Estructura de archivos creada

```
.cargo/config.toml
.gitignore
AGENTS.md
Cargo.lock
Cargo.toml                    ← workspace root
README.md
proyecto.md
tasks.md
assets/fonts/                 ← JetBrainsMono (descargados en T-008)
build/
crates/
  Luna-app/
    Cargo.toml
    src/main.rs
  Luna-terminal/
    Cargo.toml
    src/lib.rs, buffer.rs, grid.rs, parser.rs, pty.rs, shell.rs
  Luna-renderer/
    Cargo.toml
    src/lib.rs, atlas.rs, cell.rs, renderer.rs, text.rs
    src/shaders/cell.wgsl, cursor.wgsl
  Luna-ui/
    Cargo.toml
    src/lib.rs, layout.rs, pane.rs, splitter.rs, tab_bar.rs, theme.rs
  Luna-config/
    Cargo.toml
    src/lib.rs, config.rs, keybinds.rs
dist/
documentacion/desarrollo/
```
