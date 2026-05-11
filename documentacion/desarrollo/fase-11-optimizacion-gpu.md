# Fase 11 — Optimización GPU y Frame Cache

> Implementada: 2026-05-11
> Archivos modificados: 6
> Tests: 138/138 pasan, clippy clean

---

## Resumen

Tres optimizaciones que eliminan trabajo CPU/GPU redundante en frames idle:

1. **Dirty-frame tracking** — Grid sabe si cambió desde el último frame
2. **Buffers dinámicos** — Sin límite fijo, crecen automáticamente
3. **Caché de frame idle** — En idle se reusa el último frame sin reconstruir nada

---

## 1. Dirty-frame tracking (`grid.rs`)

### Cambio

Añadido campo `dirty_frame: bool` a `Grid`. Se inicializa a `true` (primer frame siempre requiere rebuild).

Se marca `dirty_frame = true` en todos los métodos que alteran el estado visible:

| Método | Efecto |
|--------|--------|
| `set()` | Celda modificada |
| `set_cursor()` | Cursor se mueve |
| `advance_cursor()` | Cursor avanza |
| `new_line()` | Nueva línea (scroll potencial) |
| `carriage_return()` | Cursor vuelve a col 0 |
| `restore_cursor()` | Cursor restaurado |
| `shift_up_region()` / `shift_down_region()` | Scroll de región |
| `scroll_up()` / `scroll_down()` / `scroll_to_bottom()` / `scroll_to_top()` / `set_scroll_offset()` | Viewport scroll |
| `set_scroll_region()` | Región de scroll cambiada |
| `insert_chars()` / `delete_chars()` / `insert_lines()` / `delete_lines()` / `erase_chars()` | Edición de línea |
| `clear_region()` / `clear_line_from_start()` / `clear_line()` | Borrado |
| `resize()` | Redimensionado |

### API pública

```rust
impl Grid {
    pub fn has_frame_dirty(&self) -> bool;
    pub fn clear_frame_dirty(&mut self);
}
```

**No confundir con `CharCell.dirty`**: cada celda tiene un flag `dirty` individual que existe pero **no está conectado al pipeline de render**. El render itera todo el viewport siempre. `dirty_frame` es un flag agregado a nivel de Grid.

---

## 2. Buffers de instancias dinámicos (`cell.rs`, `ui.rs`)

### Antes

```rust
// Tamaño fijo
let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
    size: 8192 * std::mem::size_of::<CellInstance>() as u64,
    ...
});
// Overflow: silent fail
if size > self.instance_buffer.size() { return; }
```

### Después

```rust
if needed > self.instance_buffer.size() {
    let new_size = (needed * 2).next_power_of_two().max(256);
    self.instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
        size: new_size,
        ...
    });
}
```

El buffer crece al doble del tamaño necesario, redondeado a la potencia de dos más cercana (`next_power_of_two`), mínimo 256 bytes.

### Arc\<Device\>

`wgpu::Device` no implementa `Clone`. Para compartirlo entre `Renderer`, `CellRenderer` y `UIRenderer` se usa `Arc<wgpu::Device>`:

```rust
let device = Arc::new(device);
// ...
let cell_renderer = CellRenderer::new(Arc::clone(&device), ...);
let ui_renderer = UIRenderer::new(Arc::clone(&device), ...);
```

### Firma cambiada

`CellRenderer::draw()` y `UIRenderer::draw()` pasaron de `&self` a `&mut self` (necesario para reemplazar el buffer).

---

## 3. Caché de frame idle (`app.rs`, `render.rs`)

### Condiciones que invalidan el caché

En `render_frame()`, tras procesar PTY y actualizar títulos, se evalúa:

```rust
let needs_rebuild = pty_received
    || any_grid_dirty
    || font_changed
    || blink_changed
    || tab_changed
    || ui_active
    || first_frame;
```

| Condición | Se cumple cuando |
|-----------|------------------|
| `pty_received` | Se procesaron bytes del PTY (cambia el grid) |
| `any_grid_dirty` | Algún Grid tiene `dirty_frame == true` |
| `font_changed` | `cached_font_size != state.font_size` (margen 0.01) |
| `blink_changed` | `cached_blink != cursor_blink_on` (cursor aparece/desaparece) |
| `tab_changed` | `cached_active_tab != tab_bar.active` (otro pane activo) |
| `ui_active` | `state.selecting`, `state.search.active`, o `state.history_search.active` |
| `first_frame` | `cached_cell_data.is_empty()` (primera vez o tras invalidación) |

### Si `needs_rebuild == false`

Se salta todo el bloque de reconstrucción de `cell_data` y `ui_rects`. Se llama a `draw_frame()` directamente con los datos cacheados. No se itera el grid, no se rasterizan glifos, no se suben instancias a GPU.

### Datos cacheados en `App`

```rust
pub struct App {
    // ...
    pub cached_cell_data: CellData,          // Vec<(char, f32, f32, f32, [f32;4], [f32;4])>
    pub cached_ui_rects: Vec<UIRect>,
    pub cached_blink: bool,
    pub cached_font_size: f32,
    pub cached_active_tab: usize,
}
```

### Invalidación adicional

Cuando `handle_pane_exit()` elimina panes, el caché se invalida explícitamente:

```rust
for pane_id in exited {
    self.handle_pane_exit(pane_id);
    self.cached_cell_data.clear();
    self.cached_ui_rects.clear();
}
```

---

## 4. Limpieza de código muerto (`renderer.rs`)

Eliminado `Renderer::render()` — método que creaba un render pass sin draw calls ni clear:

```rust
pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
    // ... creaba encoder, render pass vacío, submit y present
    // Sin ningún draw call — era código muerto
}
```

Nunca era llamado. El rendering real usa `Renderer::draw_frame()` → `render_instances()`.

---

## Archivos modificados

| Archivo | Cambio |
|---------|--------|
| `crates/Luna-terminal/src/grid.rs` | `dirty_frame` flag en struct + todos los métodos + `has_frame_dirty()`/`clear_frame_dirty()` |
| `crates/Luna-renderer/src/renderer.rs` | `Device` → `Arc<Device>`, eliminar `render()`, pasar `Arc` a cell/ui renderers |
| `crates/Luna-renderer/src/cell.rs` | `Arc<Device>`, `draw()` a `&mut self`, resize dinámico |
| `crates/Luna-renderer/src/ui.rs` | `Arc<Device>`, `draw()` a `&mut self`, resize dinámico |
| `crates/Luna-app/src/app.rs` | Campos de caché, `CellData` type alias |
| `crates/Luna-app/src/render.rs` | Lógica de caché en `render_frame()`, invalidación en `handle_pane_exit()` |
| `tasks.md` | Nueva Fase 11 con T-051 a T-054 |
| `AGENTS.md` | Sección de Frame Cache + `Arc<Device>` + `Grid.dirty_frame` |

---

## Verificación

```sh
cargo test --workspace        # 138 tests, todos pasan
cargo clippy --workspace --all-targets -- -D warnings  # 0 warnings
cargo build -p Luna-app       # compila sin errores
cargo build --release         # release build con thin LTO
```
