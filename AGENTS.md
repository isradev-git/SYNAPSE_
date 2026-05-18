# AGENTS.md — SYNAPSE_

> Terminal emulator GPU-accelerated en Rust. Platforms: **Linux + macOS** (no Windows native). Windows via WSL2 only. 13 fases completadas.

## Fuentes de verdad

- `proyecto.md` — arquitectura, stack, paleta de colores, estructuras de datos
- `tasks.md` — 54 tareas atómicas en 11 fases (0-11), orden estricto
- `ROADMAP.md` — 25 items R-001 a R-025, estado actual de cada uno
- `docs/desarrollo/` — docs de fases completadas
- `docs/desarrollo/desarrollo_principal.md` — tracking de fases actual

## Datos clave que un agente erraría sin ayuda

- **Nombre**: "SYNAPSE_" (no "Synapse", no "synapse")
- **Lenguaje**: Rust stable, edition 2021
- **Workspace**: 5 crates bajo `crates/` — `SYNAPSE_-app` (bin), `SYNAPSE_-renderer`, `SYNAPSE_-ui`, `SYNAPSE_-config`, `SYNAPSE_-suggest`
- **Entry point**: `crates/SYNAPSE_-app/src/main.rs` — winit event loop
- **Renderer**: `crates/SYNAPSE_-renderer/src/renderer.rs` — wgpu instance + surface + device + queue
- **Tab/Pane system**: `TabBar { tabs: Vec<Tab>, active: usize }`, cada `Tab` tiene `PaneTree` + `active_pane: PaneId`. Los `Pane` viven en `Vec<Pane>` separado, referenciados por ID.
- **Single render pass**: `draw_frame(cells, ui_rects)` rasteriza glyphs y dibuja cells + UI rects en un solo `get_current_texture`/`present`.
- **Mouse position**: `AppState.cursor_x/y` se actualiza en cada `CursorMoved` (÷ scale_factor). `MouseInput` lee estas coordenadas para detectar clicks en tab bar.
- **Código existente**: Fase 0–4 implementadas. Hay source real (`main.rs`, `renderer.rs`, `text.rs`, `atlas.rs`, `cell.rs`, `cell.wgsl`, `ui.rs`, `ui.wgsl`, `pane.rs`, `splitter.rs`, `tab_bar.rs`, `layout.rs`, `theme.rs`)
- **Nombres de crate con mayúscula** (ej. `SYNAPSE_-app`) como consta en `proyecto.md`. El `[lib] name` usa snake_case (ej. `synapse_renderer` para el crate `SYNAPSE_-renderer`).

## Stack y versiones exactas

Versiones fijadas en `[workspace.dependencies]` del `Cargo.toml` raíz:

`winit` 0.30, `wgpu` 22, `fontdue` 0.9, `alacritty_terminal` 0.24, `portable-pty` 0.8, `rustybuzz` 0.14, `tokio` 1 (full), `serde` 1 (derive), `toml` 0.8, `arboard` 3, `clap` 4 (derive), `tracing` 0.1, `tracing-subscriber` 0.3, `pollster` 0.3, `bytemuck` 1 (derive), `base64` 0.22, `png` 0.17.

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
- `Backends::DX11` **no existe** en wgpu 22 (fue eliminado). Solo DX12.
- Vertex shader: no usar structs como `@location(0)`, usar parámetros planos (`@location(0) pos: vec2<f32>`, etc.). Struct-based vertex input causa `type does not match the varying` en naga.
- Dynamic array indexing en WGSL (`arr[vertex_index]`) rechazado por naga en backends estrictos. Usar `if/else`. Ver `corner_for_index()` en `cell.wgsl`.

## APIs fontdue 0.9 (no obvias)

- `Font::rasterize(ch, px)` → `(Metrics, Vec<u8>)` — grayscale bitmap, 1 byte por pixel
- `Font::rasterize_indexed(glyph_id, px)` — rasterizar por glyph ID para ligaduras
- `Metrics { xmin, ymin, width, height, advance_width, advance_height }`
- Convertir grayscale → RGBA antes de subir al atlas

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

## Frame cache (Fase 11 — GPU optimization)

`render_frame()` en `crates/SYNAPSE_-app/src/render.rs` decide si reconstruir `cell_data`/`ui_rects` o reusar el caché:

**Invalidan el caché**:
- `pty_received`: se procesaron datos del PTY
- `any_grid_dirty`: algún `Grid` tiene `dirty_frame == true`
- `font_changed`: `cached_font_size != state.font_size`
- `blink_changed`: `cached_blink != cursor_blink_on`
- `tab_changed`: `cached_active_tab != tab_bar.active`
- `ui_active`: `state.selecting || state.search.active || state.history_search.active`
- `first_frame`: `cached_cell_data.is_empty()`

Si ninguna condición se cumple: el caché de instancias GPU se re-usa, no se sube nada a la GPU ni se itera el grid.

**Buffers dinámicos**: `CellRenderer` y `UIRenderer` crean buffers GPU del tamaño justo ×2 (`next_power_of_two`). Antes: overflow = silent fail. Ahora: resize automático.

**Grid.dirty_frame**: cada método mutante (`set()`, `advance_cursor()`, `scroll_up()`, `new_line()`, `resize()`, etc.) marca `dirty_frame = true`. `clear_frame_dirty()` se llama tras renderizar. No confundir con el `dirty` por-celda (CharCell.dirty) que no está conectado al render.

**Arc\<Device\>**: `Renderer`, `CellRenderer` y `UIRenderer` comparten el device via `Arc<wgpu::Device>` (wgpu 22 no implementa Clone, el Arc permite la compartición).

## Convenciones de arquitectura

- **Dirty tracking por frame**: `Grid.dirty_frame` marca si algo cambió desde el último frame. El render salta la reconstrucción de instancias si no hay cambios.
- **CharCell.dirty**: existe pero no está conectado al render. Cada celda tiene el flag pero el render itera todo el viewport.
- **Instanced rendering**: un solo draw call por frame, una instancia GPU por celda visible.
- **PaneTree**: árbol binario de splits (`Leaf | Split { direction, ratio, first, second }`).
- **Scrollback**: buffer circular, default 100.000 líneas, con `scroll_offset` para viewport scrolling.
- **Config**: TOML en `~/.config/SYNAPSE_/config.toml` (Linux) o `~/Library/Application Support/SYNAPSE_/config.toml` (macOS).
- **Shell detection**: `$SHELL` → `/bin/zsh` (macOS) → `/bin/bash` (Linux).

## Paleta de colores (identidad visual)

Tema profesional oscuro con acento azul acero. Los colores se definen en `Theme::synapse_()` (`crates/SYNAPSE_-config/src/themes.rs`).
Los 4 temas built-in (`synapse_`, `dracula`, `catppuccin-mocha`, `tokyo-night`) son intercambiables via `theme = "..."` en `config.toml`.

```
#11131a  → fondo principal (clear color en render)
#d2d5db  → texto del buffer
#7098cc  → cursor, borde de panel activo (azul acero)
#181b24  → tab bar bg
#222739  → tab activa, separadores, bordes inactivos
#e5e8ee  → texto UI activo
#737a8c  → texto UI inactivo
```

## Comandos útiles

```sh
cargo build -p SYNAPSE_-app                  # build binario
cargo build -p SYNAPSE_-renderer             # build crate renderer
cargo test -p SYNAPSE_-renderer              # tests de renderer
cargo test -p SYNAPSE_-renderer -- --nocapture # test con stdout visible
cargo build --release                        # release con mold linker
cargo watch -x test                          # hot-reload tests
WINIT_UNIX_BACKEND=x11 ./target/debug/synapse_ # forzar X11 (WSLg workaround)
```

## Cross-compilation (solo referencia, no soportado)

```sh
cargo install cargo-xwin
cargo xwin build --release -p SYNAPSE_-app --target x86_64-pc-windows-msvc
```

## Benchmarking

```sh
cargo build --release
RUST_LOG=synapse_::bench=info ./target/release/synapse_   # FPS logging
./build/bench.sh release                          # benchmark script
```

## Referencia rápida Kitty Keyboard (R-022)

- `crates/SYNAPSE_-app/src/input.rs` — `from_key_kitty()`, CSI u encoding, keycodes
- `crates/SYNAPSE_-app/src/image_protocol.rs` — `scan_kkp()`, `KkpScan` enum
- Pre-scan en PTY reader (`image_protocol.rs`) intercepta `\e[?u` (query), `\e[= u` (set), `\e[> u` (push), `\e[< u` (pop) antes del parser vte
- Flags: `KITTY_DISAMBIGUATE=1`, `KITTY_REPORT_EVENTS=2`, `KITTY_REPORT_ALL=8`
- Sin esto: neovim no distingue Ctrl+[ de Escape, Ctrl+I de Tab, etc.

## Métricas objetivo (no negociables)

- Latencia input→render: <5ms
- FPS: 60 estables, ≥30 con output masivo
- Arranque: <200ms
- RAM idle: <50MB

## Sin base de datos, sin HTTP

App de escritorio. Sin rutas REST, sin DB. Todo in-memory.
