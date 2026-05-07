# Fase 1 — Ventana Base con winit + wgpu

## T-006 · Ventana winit 0.30

**Archivo:** `crates/Luna-app/src/main.rs`

- `EventLoop::new()` para crear el event loop
- `WindowAttributes` con título "Luna", tamaño 1280×800, resizable
- `create_window()` sobre el event loop
- `ControlFlow::Wait` para bajar CPU idle
- Maneja `WindowEvent::CloseRequested` → `elwt.exit()`
- Maneja `WindowEvent::Resized` → delega al renderer

**Detalle winit 0.30:** `create_window` y `run` están deprecados en favor de `ActiveEventLoop::create_window` y `run_app`. Se usan los métodos deprecated para simplificar; migrar a `ApplicationHandler` en refactor posterior.

## T-007 · Surface wgpu + clear

**Archivo:** `crates/Luna-renderer/src/renderer.rs`

- `wgpu::Instance` con `Backends::all()`
- `Surface` creada desde `Arc<Window>` via `instance.create_surface()`
- `request_adapter` con `PowerPreference::HighPerformance` y `compatible_surface`
- `Device` + `Queue` + `SurfaceConfiguration`
  - Present mode: `AutoVsync`
  - Formato: sRGB preferido
- Clear color: `#210b4b` (el casi-negro de la paleta Luna)
- Métodos públicos: `new(window)`, `resize()`, `render()`, `size()`
- `pollster::block_on` para las llamadas async de wgpu

## T-008 · Carga de JetBrains Mono (cosmic-text 0.12)

**Archivo:** `crates/Luna-renderer/src/text.rs`

- Fuentes embebidas via `include_bytes!`:
  - `JetBrainsMono-Regular.ttf`
  - `JetBrainsMono-Bold.ttf`
  - `JetBrainsMono-Italic.ttf`
- `FontSystem::new()` + `db_mut().load_font_data()` para cargar
- `SwashCache::new()` para rasterización
- API `rasterize_glyph(c, font_size) -> Option<(SwashImage, CacheKey)>`:
  1. Crea `Buffer` con la letra a renderizar
  2. Shapea con `Shaping::Advanced`
  3. Construye `CacheKey` desde los `LayoutGlyph` resultantes
  4. Llama `SwashCache::get_image_uncached()`
- Test: rasteriza 'A' a 14px, verifica que `Some`

**CacheKey:** contiene `font_id`, `glyph_id`, `font_size_bits`, `x_bin`, `y_bin`, `flags`. Se construye con `CacheKey::new(font_id, glyph_id, font_size, (x, y), flags)` que devuelve `(CacheKey, i32, i32)`.

## T-009 · Texture atlas de glifos

**Archivo:** `crates/Luna-renderer/src/atlas.rs`

- Textura wgpu: 2048×2048, `Rgba8Unorm`, uso `COPY_DST | TEXTURE_BINDING`
- Sampler: `Nearest` (pixelado, ideal para terminal)
- `BindGroupLayout` con texture + sampler en binding 0 y 1
- **Shelf-packing allocator:**
  - `x_offset`, `y_offset`, `row_height` para tracking de posición
  - Cuando un glyph no cabe en la fila actual: salta a siguiente fila
  - Cuando se acaba el atlas: retorna `None`
- `HashMap<CacheKey, UvRect>`: cachea posiciones de glyphs ya insertados
- `UvRect { u0, v0, u1, v1 }`: coordenadas UV en espacio [0, 1]
- `upload_glyph(queue, rect, bitmap, w, h)`: usa `queue.write_texture()` para subir pixels a GPU
- `SwashContent::Mask` → se convierte a RGBA (1 channel → 4 channel con alpha)
- Test: 100 glyphs de tamaños variados, verifica que no se solapan

## T-010 · Render instanciado de texto

**Archivos:**
- `crates/Luna-renderer/src/cell.rs` — pipeline + instancias
- `crates/Luna-renderer/src/shaders/cell.wgsl` — shader VS + FS
- `crates/Luna-renderer/src/renderer.rs` — integración + `draw_text`

### CellInstance (64 bytes por instancia)

```rust
#[repr(C)]
struct CellInstance {
    cell_pos: [f32; 2],    // pixel position
    cell_size: [f32; 2],   // width, height
    uv_rect: [f32; 4],     // u0, v0, u1, v1
    fg_color: [f32; 4],    // RGBA foreground
    bg_color: [f32; 4],    // RGBA background
}
```

### Shader cell.wgsl

- **Vertex:** 4 vértices (quad corners) por instancia en `TriangleStrip`
  - Posición pixel → NDC: `[x/w * 2 - 1, 1 - y/h * 2]` (flip Y)
  - UV interpolated desde UV rect de la instancia
- **Fragment:** muestrea atlas con UV, `mix(bg, fg, alpha)` para blending

### ScreenUniform

Uniform buffer con `screen_size: vec2<f32>` para convertir coordenadas pixel a NDC.

### Pipeline

- `atlas_bind_group_layout` en group 0 (atlas texture + sampler)
- `screen_bind_group_layout` en group 1 (screen size uniform)
- Vertex buffers: instance data step mode, attributes: cell_pos, cell_size, uv_rect, fg, bg
- `AlphaBlending` en el fragment stage
- `TriangleStrip` topology, back-face culling

### draw_text

1. Itera caracteres del string
2. Para cada char: `rasterize_glyph()` → obtiene `SwashImage` + `CacheKey`
3. Si bitmap tamaño > 0: `atlas.get_or_insert(key, w, h)` → UV rect
4. Convierte máscara 1-canal a RGBA (channel alpha)
5. `atlas.upload_glyph()` → sube pixels a GPU
6. Construye `CellInstance` con posición calculada + uv + colores
7. `render_instances()` → clear a `#210b4b` + draw call instanciado

### Dependencias añadidas en Fase 1

- `pollster 0.3` — bloqueo de futures async (wgpu init)
- `bytemuck 1` — Pod/Zeroable para instance data + uniforms

### Tests

```sh
cargo test -p Luna-renderer
# 2 tests pass:
# - text::tests::test_rasterize_a  (ok)
# - atlas::tests::test_allocate_no_overlap  (ok)
```
