# Fase 3 — Rendering de Terminal Completo

## T-017 · Conectar Grid → Renderer

**Archivo:** `crates/Luna-app/src/main.rs` (event loop)

### Flujo de render por frame

```
Event::AboutToWait → window.request_redraw()
WindowEvent::RedrawRequested → {
    1. PTY reader: pty_session.rx.try_recv() → processor.process(data)
    2. Grid → cell_data: iterate visible_cells(), build (char, x, y, font_size, fg, bg)
    3. Cursor overlay: skip cursor position in loop, draw after
    4. renderer.draw_cells(&cell_data)
}
```

### visible_cells()

**Archivo:** `crates/Luna-terminal/src/grid.rs:401`

```rust
pub fn visible_cells(&self) -> impl Iterator<Item = (usize, usize, &CharCell)>
```

- Calcula scrollback visible: desde `scroll_offset` hasta `min(scroll_offset + rows, scrollback.len())`
- Primero itera scrollback lines (como viewport rows 0..sb_lines)
- Luego grid actual (desde viewport row sb_lines..rows)
- Retorna `(col, viewport_row, &CharCell)`

### draw_cells()

**Archivo:** `crates/Luna-renderer/src/renderer.rs:173`

```rust
pub fn draw_cells(&mut self, cells: &[(char, f32, f32, f32, [f32;4], [f32;4])])
```

- Deduplicación de glifos: `HashMap<(char, u32), (SwashImage, CacheKey)>`
  - `u32` = `font_size.to_bits()` para evitar rasterizar el mismo char varias veces
- Por cada celda:
  1. Si `c == ' '`, skip (solo background, manejado por clear color / bg en shader)
  2. `rasterize_glyph(c, font_size)` → `(SwashImage, CacheKey)`
  3. `atlas.get_or_insert(key, w, h)` → `UvRect` (cachea en GPU)
  4. Convierte `SwashContent::Mask` → RGBA (1-byte alpha → 4-byte RGBA)
  5. `atlas.upload_glyph(queue, uv, &pixels, w, h)` → sube a textura GPU
  6. Construye `CellInstance` con posición, UV, fg/bg color
  7. `render_instances(&instances)`: clear + draw call instanciado

### Posición de celdas

```rust
let x = margin + col as f32 * cell_w;
let y = margin + vrow as f32 * cell_h;
```

- `cell_metrics(font_size)`: rasteriza 'W', retorna `(width + 1, height + 4)`
- Margen: 4px alrededor del contenido

### CellInstance (64 bytes)

```rust
#[repr(C)]
struct CellInstance {
    cell_pos: [f32; 2],    // pixel position (x, y)
    cell_size: [f32; 2],   // glyph bitmap width, height
    uv_rect: [f32; 4],     // u0, v0, u1, v1 en atlas
    fg_color: [f32; 4],    // RGBA foreground
    bg_color: [f32; 4],    // RGBA background
}
```

- `step_mode: Instance` — un draw call por frame, todas las celdas como instancias
- `TriangleStrip` con 4 vértices por instancia

## T-018 · Shader de cursor animado (pendiente)

**Archivo:** `crates/Luna-renderer/src/shaders/cursor.wgsl` (creado, sin usar)

El shader `cursor.wgsl` existe pero no se usa en el pipeline actual. El cursor se renderiza como una celda más con colores especiales:

```rust
// Cursor en main.rs:154
let cursor_fg = [1.0, 0.239, 0.58, 1.0];   // #ff3d94
let cursor_bg = [1.0, 1.0, 1.0, 0.15];     // semitransparente
```

El cursor se dibuja como overlay: en el loop de celdas se salta la posición del cursor, luego se inserta una instancia extra con fg rosa neón y bg blanco semitransparente.

### Optimizaciones existentes

- **Dirty tracking**: `Grid::dirty_cells()` itera solo celdas con `dirty = true`. Sin embargo, en el rendering actual se itera TODO el grid visible (sin filtrar por dirty) porque se reconstruyen todas las instancias cada frame. La mejora pendiente es usar dirty tracking para re-subir solo celdas modificadas.
- **Glyph cache en CPU**: `HashMap<(char, u32), (SwashImage, CacheKey)>` evita rasterizar el mismo glyph múltiples veces en un mismo frame.

## T-019 · Soporte de colores ANSI completo

Implementado en el parser (`VteProcessor::handle_sgr`):

- **3-bit**: 30-37 (fg), 40-47 (bg) → `Color::Indexed(0-7)`
- **Bright**: 90-97 (fg), 100-107 (bg) → `Color::Indexed(8-15)`
- **xterm-256**: `38;5;N` / `48;5;N` → `Color::Indexed(N)` donde N=0..255
- **True color 24-bit**: `38;2;R;G;B` / `48;2;R;G;B` → `Color::Rgb(r, g, b)`
- **Default**: 39 (fg default), 49 (bg default) → `Color::Default`
- **Atributos**: bold(1), italic(3), underline(4), blink(5/6), inverse(7), invisible(8), con sus respectivos remove (22-28)

### ansi_256_to_rgba()

Mapeo completo de 256 colores ANSI:
- 0-15: tabla fija de 16 colores estándar (con nombres: Black, Red, Green, Yellow, Blue, Magenta, Cyan, White + bright)
- 16-231: cubo 6×6×6 (216 colores)
- 232-255: escala de grises (24 tonos)

### Color::Default renderizado

- `fg_rgba()` → `[1.0, 1.0, 1.0, 1.0]` (blanco)
- `bg_rgba()` → `[0.0, 0.0, 0.0, 0.0]` (transparente — el shader mezcla sobre el clear color `#210b4b`)

## T-020 · Resize de ventana → resize de PTY y grid

```rust
WindowEvent::Resized(size) => {
    renderer.resize(size);  // reconfigura surface wgpu
    let new_cols = ((size.width - margin*2) / cell_w).max(1);
    let new_rows = ((size.height - margin*2) / cell_h).max(1);
    if new_cols != cols || new_rows != rows {
        cols = new_cols;
        rows = new_rows;
        grid.borrow_mut().resize(cols, rows);
        pty_session.pty.resize(cols as u16, rows as u16);
    }
}
```

- `Grid::resize()`: preserva contenido existente, fill con default para nuevas celdas, reclamp cursor
- `PtyHandle::resize()`: llama `master.resize(PtySize { rows, cols, 0, 0 })`
- `Renderer::resize()`: reconfigura `surface.configure()` con nuevo width/height
- Celdas vacías (espacio con bg default) no se envían a GPU (skip en el loop de render)

## Shader cell.wgsl

**Archivo:** `crates/Luna-renderer/src/shaders/cell.wgsl`

```wgsl
// Vertex: 4 vértices en TriangleStrip, pixel→NDC con flip Y
out.position = vec4(
    (pixel_pos.x / screen.screen_size.x) * 2.0 - 1.0,
    1.0 - (pixel_pos.y / screen.screen_size.y) * 2.0,
    0.0, 1.0);

// Fragment: sample atlas → mix(bg, fg, alpha)
let alpha = atlas_color.a;
return mix(in.bg_color, in.fg_color, alpha);
```

### Bind groups

- Group 0: atlas texture (binding 0) + sampler nearest (binding 1)
- Group 1: screen uniform (vec2 screen_size) para conversión pixel→NDC

### Render pipeline

- `AlphaBlending` en fragment
- `TriangleStrip`, back-face culling
- Instance buffer pre-asignado para 8192 instancias

### Tests: 2 tests

```sh
cargo test -p Luna-renderer
# text::tests::test_rasterize_a
# atlas::tests::test_allocate_no_overlap
```
