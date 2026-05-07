# AGENTS.md — Luna

> Terminal emulator multiplataforma GPU-accelerated en Rust. Fases 0-1 completadas, en Fase 2.

## Fuentes de verdad

- `proyecto.md` — arquitectura, stack, paleta de colores, estructuras de datos
- `tasks.md` — 50 tareas atómicas en 11 fases, orden estricto
- `documentacion/desarrollo/` — docs de fases completadas (fase-0-scaffolding.md, fase-1-ventana-y-wgpu.md)

## Datos clave que un agente erraría sin ayuda

- **Nombre**: "Luna" (no "LunaShell", no "lunashell")
- **Lenguaje**: Rust stable, edition 2021
- **Workspace**: 5 crates bajo `crates/` — `Luna-app` (bin), `Luna-terminal`, `Luna-renderer`, `Luna-ui`, `Luna-config`
- **Entry point**: `crates/Luna-app/src/main.rs` — winit event loop
- **Renderer**: `crates/Luna-renderer/src/renderer.rs` — wgpu instance + surface + device + queue
- **Código existente**: Fase 0–1 implementadas. Hay source real (`main.rs`, `renderer.rs`, `text.rs`, `atlas.rs`, `cell.rs`, `cell.wgsl`)
- **Nombres de crate con mayúscula** (ej. `Luna-app`) como consta en `proyecto.md`. El `[lib] name` usa snake_case (ej. `luna_renderer` para el crate `Luna-renderer`).

## Stack y versiones exactas

Versiones fijadas en `[workspace.dependencies]` del `Cargo.toml` raíz:

`winit` 0.30, `wgpu` 22, `cosmic-text` 0.12, `portable-pty` 0.8, `vte` 0.13, `tokio` 1 (full), `serde` 1 (derive), `toml` 0.8, `arboard` 3, `clap` 4 (derive), `bitflags` 2, `tracing` 0.1, `pollster` 0.3, `bytemuck` 1 (derive).

## APIs winit 0.30 (no obvias)

- `EventLoop::new()?` → luego `event_loop.create_window(WindowAttributes::default().with_title(...))?`
- `EventLoop::run()` está deprecado en favor de `run_app` (usamos el deprecated por simplicidad)
- `RedrawRequested` + `AboutToWait` para el loop de render
- Para refactor futuro: migrar a `ApplicationHandler` trait

## APIs wgpu 22 (no obvias)

- `entry_point` en vertex/fragment state es `&str` (no `Option<&str>`)
- `BindGroupLayout` no implementa `Clone` — pasar por referencia
- `create_surface(window)` requiere `Arc<Window>` (por `raw-window-handle`)
- `pollster::block_on` para init async (request_adapter, request_device)

## APIs cosmic-text 0.12 (no obvias)

- `CacheKey::new(font_id, glyph_id, font_size, (x, y), flags)` devuelve `(CacheKey, i32, i32)`
- `SwashCache::get_image_uncached(font_system, cache_key)` → `Option<SwashImage>`
- `SwashContent::Mask` = alpha channel 1-byte; convertir a RGBA manualmente
- `LayoutGlyph` tiene campos: `start`, `end`, `font_size`, `line_height_opt`, `font_id`, `glyph_id`, `x`, `y`, `w`, `h`, `x_offset`, `y_offset`

## Pipeline de instanced rendering

```
CellInstance { cell_pos, cell_size, uv_rect, fg_color, bg_color }
    → vertex buffer (step_mode: Instance)
    → cell.wgsl: 4 vértices en TriangleStrip por instancia
    → shader convierte pixel coords → NDC (flip Y)
    → fragment: textureSample(atlas, sampler, uv) → mix(bg, fg, alpha)
```

**CellInstance layout**: 16 floats = 64 bytes (pos:2, size:2, uv:4, fg:4, bg:4).

**Bind groups**: group 0 = atlas (texture + sampler), group 1 = screen uniform (vec2).

## Convenciones de arquitectura

- **Dirty tracking**: cada `CharCell` tiene flag dirty. Solo celdas modificadas se re-suben a GPU.
- **Instanced rendering**: un solo draw call por frame, una instancia GPU por celda visible.
- **PaneTree**: árbol binario de splits (`Leaf | Split { direction, ratio, first, second }`).
- **Scrollback**: buffer circular, default 100.000 líneas, con `scroll_offset` para viewport scrolling.
- **Config**: TOML en `~/.config/Luna/config.toml` (Linux/macOS) o `%APPDATA%\Luna\config.toml` (Windows).
- **Shell detection**: Windows → `cmd.exe`, macOS/Linux → `$SHELL` con fallbacks documentados.

## Paleta de colores (identidad visual)

```
#ff3d94  → cursor, selección, prompt activo
#b5307e  → tab activa, borde de panel activo
#6a2a98  → tab inactiva, separadores, tab bar bg
#3f1c6d  → hover, paneles inactivos
#210b4b  → fondo principal (clear color en render)
```

## Comandos útiles

```sh
cargo build -p Luna-app                     # build binario
cargo build -p Luna-renderer                # build crate renderer
cargo test -p Luna-renderer                 # tests de renderer
cargo test -p Luna-renderer -- --nocapture  # test con stdout visible
cargo build --release                       # release con mold linker
cargo watch -x test                         # hot-reload tests
```

## Métricas objetivo (no negociables)

- Latencia input→render: <5ms
- FPS: 60 estables, ≥30 con output masivo
- Arranque: <200ms
- RAM idle: <50MB

## Sin base de datos, sin HTTP

App de escritorio. Sin rutas REST, sin DB. Todo in-memory.
