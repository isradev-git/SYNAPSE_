# Fase 5 — Tabs (Pestañas)

## Arquitectura general

```
main.rs (event loop)
    │
    ├── TabBar ────────── Vec<Tab>
    │                        ├── id: TabId
    │                        ├── title: String
    │                        ├── pane_tree: PaneTree
    │                        └── active_pane: PaneId
    │
    ├── Vec<Pane> ──────── Pane
    │                        ├── id: PaneId
    │                        ├── pty_session: PtySession
    │                        ├── grid: Rc<RefCell<Grid>>
    │                        ├── processor: VteProcessor
    │                        ├── title: Rc<RefCell<String>>
    │                        └── cwd: Rc<RefCell<String>>
    │
    └── redraw: build_tab_bar_ui_rects() + build_tab_bar_text()
          → UIRect (colored rects) + cell_data (glyphs)
          → Renderer::draw_frame(cells, rects)
```

Cada tab tiene un `PaneTree` (árbol binario de splits; Fase 5 solo usa `Leaf`).
Cada tab refiere a su pane activo via `active_pane: PaneId`.
Los panes viven en un `Vec<Pane>` independiente, compartidos por referencia de ID.

---

## T-025 · Data structures (Luna-ui)

### PaneId / Pane

**Archivo:** `crates/Luna-ui/src/pane.rs`

```rust
pub struct PaneId(pub u64);

pub struct Pane {
    pub id: PaneId,
    pub pty_session: PtySession,
    pub grid: Rc<RefCell<Grid>>,
    pub processor: VteProcessor,
    pub cols: usize,
    pub rows: usize,
    title: Rc<RefCell<String>>,
    cwd: Rc<RefCell<String>>,
}
```

- `Pane::new()` crea el `VteProcessor` con `new_with_title()`, compartiendo `Rc<RefCell<String>>` para title y cwd entre el processor (OSC handler) y el pane (lectura desde main.rs).
- `title()` / `cwd()`: getters que clonan el String interno.

### PaneTree (Splitter)

**Archivo:** `crates/Luna-ui/src/splitter.rs`

```rust
pub enum SplitDirection { Horizontal, Vertical }

pub enum PaneTree {
    Leaf(PaneId),
    Split {
        direction: SplitDirection,
        ratio: f32,
        first: Box<PaneTree>,
        second: Box<PaneTree>,
    },
}
```

- `leaf(id)`: constructor para Leaf
- `all_panes()`: recorrido recursivo, colecciona todos los PaneId del árbol
- `find_active(active)`: si `active` existe en el árbol lo retorna; si no, retorna el primer Leaf (fallback)

### Tab / TabBar

**Archivo:** `crates/Luna-ui/src/tab_bar.rs`

```rust
pub struct TabId(pub u64);

pub struct Tab {
    pub id: TabId,
    pub title: String,
    pub pane_tree: PaneTree,
    pub active_pane: PaneId,
}

pub struct TabBar {
    pub tabs: Vec<Tab>,
    pub active: usize,
    next_tab_id: u64,
    next_pane_id: u64,
}
```

| Método | Comportamiento |
|--------|---------------|
| `new(initial_tab)` | Crea TabBar con 1 tab |
| `active_tab()` / `active_tab_mut()` | Ref al tab activo (`&self.tabs[self.active]`) |
| `new_tab()` | Crea TabId + PaneId autoincremental, push a tabs, activa la nueva |
| `close_tab(index)` | Remove del vec. Si `active >= len`, ajusta. **No permite cerrar la última tab** (retorna None si ≤ 1) |
| `activate(index)` | Clamped a `0..tabs.len()` |
| `next_tab()` | `(active + 1) % len` |
| `prev_tab()` | `(active - 1).max(0)` con wrap |
| `set_title(tab_id, title)` | Busca por ID y actualiza título |

### Layout

**Archivo:** `crates/Luna-ui/src/layout.rs`

```rust
pub struct Layout {
    pub window_width: f32,
    pub window_height: f32,
    pub tab_bar_height: f32,
}
```

| Método | Descripción |
|--------|-------------|
| `new()` | Default 1280×800 |
| `update(w, h)` | Llamado en `Resized` |
| `pane_area()` | `(x, y, w, h)` — y = `tab_bar_height`, h = `window_height - tab_bar_height` |
| `pane_margin()` | `4.0` |
| `tab_width(count)` | `(window_width - 56) / count`, clamped `80..200` |
| `tab_x(index, count)` | `index * tab_width(count)` — posición X de la tab N |

### Theme

**Archivo:** `crates/Luna-ui/src/theme.rs`

```rust
pub const TAB_BAR_HEIGHT: f32 = 32.0;

pub const TAB_ACTIVE_BG: [f32; 4] = [0.71, 0.19, 0.49, 1.0];     // #b5307e
pub const TAB_INACTIVE_BG: [f32; 4] = [0.42, 0.16, 0.60, 1.0];   // #6a2a98
pub const TAB_BAR_BG: [f32; 4] = [0.42, 0.16, 0.60, 1.0];        // #6a2a98
pub const TAB_HOVER_BG: [f32; 4] = [1.0, 0.24, 0.58, 0.13];      // #ff3d9422
pub const TAB_TEXT: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
pub const TAB_TEXT_INACTIVE: [f32; 4] = [0.8, 0.8, 0.8, 1.0];
pub const TAB_BUTTON_TEXT: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
pub const TAB_SEPARATOR: [f32; 4] = [0.25, 0.11, 0.43, 1.0];     // #3f1c6d
pub const BG_COLOR: [f32; 4] = [0.13, 0.04, 0.29, 1.0];          // #210b4b
```

---

## T-026 · UI rendering pipeline (Luna-renderer)

### Shader ui.wgsl

**Archivo:** `crates/Luna-renderer/src/shaders/ui.wgsl`

Shader de rectángulos coloreados (sin textura). Cada instancia = un UIRect.

```
struct UIRect { pos: vec2<f32>, size: vec2<f32>, color: vec4<f32> }
struct ScreenUniform { screen_size: vec2<f32> }

vs_main(vertex_index, rect: UIRect):
    corner = corners[vertex_index]  // 4 vértices TriangleStrip
    pixel_pos = rect.pos + corner * rect.size
    position = (pixel_pos / screen_size) * 2 - 1  (Y flip)

fs_main: return rect.color
```

Mismo `ScreenUniform` que cell.wgsl. El vertex shader convierte coordenadas de píxel a NDC flip-Y.

### UIRenderer

**Archivo:** `crates/Luna-renderer/src/ui.rs`

```rust
pub struct UIRect {
    pub pos: [f32; 2],    // top-left en píxeles
    pub size: [f32; 2],   // ancho × alto
    pub color: [f32; 4],  // RGBA (blend mode ALPHA_BLENDING)
}
```

| Componente | Detalle |
|------------|---------|
| Pipeline | `TriangleStrip`, `UIRect` como instancia (VertexStepMode::Instance) |
| Vertex atributos | pos (Float32x2), size (Float32x2), color (Float32x4) |
| Blend | `ALPHA_BLENDING` — permite transparencia |
| Instance buffer | 256 rects de tamaño fijo |
| Screen uniform | Buffer de 16 bytes (vec2 + padding) |

### Integración en Renderer

**Archivo:** `crates/Luna-renderer/src/renderer.rs`

**Nuevo método `draw_frame()`:**

```rust
pub fn draw_frame(
    &mut self,
    cells: &[(char, f32, f32, f32, [f32; 4], [f32; 4])],
    ui_rects: &[UIRect],
)
```

- Rasteriza todos los glyphs (con cache por `(char, font_size)`)
- Construye `Vec<CellInstance>` y `Vec<UIRect>`
- Llama a `render_instances(instances, ui_rects)`

**`render_instances()` modificado:**
1. Update screen size uniform para `cell_renderer` y `ui_renderer`
2. Begin render pass con clear color `#210b4b`
3. `cell_renderer.draw()` con cell instances
4. `ui_renderer.draw()` con UI rects (solo si no vacío)
5. Submit + present

Esto garantiza que cells y UI se dibujen en **un solo render pass**, evitando get_current_texture() múltiple.

---

## T-027 · Interacción con tabs (main.rs)

### Creación de panes

```rust
fn create_pane(id: PaneId, cols: usize, rows: usize) -> Pane {
    let grid = Rc::new(RefCell::new(Grid::new(cols, rows)));
    let shell = detect_shell();
    let pty = PtyHandle::spawn(cols as u16, rows as u16, &shell).expect("...");
    let session = PtyHandle::start_reader(pty);
    Pane::new(id, session, grid, cols, rows)
}
```

### Navegación por teclado

**Archivo:** `crates/Luna-app/src/main.rs` — `handle_tab_keyboard()`

| Atajo | Acción |
|-------|--------|
| `Ctrl+T` | `TabAction::NewTab` → `tab_bar.new_tab()`, push nuevo `Pane` |
| `Ctrl+W` | `TabAction::CloseTab` → `tab_bar.close_tab()`, `panes.retain()` |
| `Ctrl+Tab` | `tab_bar.next_tab()` |
| `Ctrl+Shift+Tab` | `tab_bar.prev_tab()` |
| `Ctrl+1..9` | `tab_bar.activate(n)` (0-indexed; Ctrl+9 → index 8) |

`new_tab()` crea pane con dimensiones del `pane_area` actual.

`close_tab()` remueve el pane activo del `Vec<Pane>`. No permite cerrar la última tab restante.

### Navegación por click

```rust
MouseInput(Pressed, Left) → {
    let x = state.cursor_x;   // storeado en CursorMoved ÷ scale_factor
    let y = state.cursor_y;

    if y < TAB_BAR_HEIGHT as f64 {
        handle_tab_click(tab_bar, panes, x, layout.window_width);
    } else {
        state.selecting = true;  // selección en terminal
    }
}
```

**`handle_tab_click()`:**

```rust
fn handle_tab_click(tab_bar, panes, x, window_width) {
    let tab_w = (window_width - 56.0) / count;  // clamped 80..200

    // Click en botón "+" (últimos 32px)
    if x >= tab_w * count && x < tab_w * count + 32.0 {
        tab_bar.new_tab();  // → nueva tab con pane
        return;
    }

    // Click en tab N
    let clicked = (x / tab_w) as usize;
    if clicked < count { tab_bar.activate(clicked); }
}
```

### Construcción de UI rects

```rust
fn build_tab_bar_ui_rects(layout, tab_bar) -> Vec<UIRect> {
    // 1. Fondo de la barra (window_width × TAB_BAR_HEIGHT)
    // 2. Por cada tab: UIRect coloreado según active/inactive
    // 3. Separadores verticales entre tabs (1px, color TAB_SEPARATOR)
    // 4. Botón "+" (últimos 32px, color TAB_BAR_BG)
}
```

### Construcción de texto de tabs

```rust
fn build_tab_bar_text(layout, tab_bar) -> Vec<(char, x, y, font_size, fg, bg)> {
    // Por cada tab: título con truncado (max_chars = (tab_w - 24) / char_w)
    //   Si title vacío → "Tab N"
    //   Si muy largo → truncar con "…"
    // + símbolo "+" al final
}
```

El texto de la tab bar se renderiza como celdas de terminal (glyphs) en el mismo `draw_frame`, usando `cell_data`.

### Flujo de redraw

```
RedrawRequested:
  1. Por cada pane: drenar PTY reader → processor.process()
  2. Sincronizar titles: tab.title = pane.title() (si cambió)
  3. Obtener grid activo + cursor + scroll
  4. Construir cell_data (visible_cells + cursor overlay)
  5. build_tab_bar_ui_rects() → ui_rects
  6. build_tab_bar_text() → cell_data (append)
  7. renderer.draw_frame(cell_data, ui_rects)
```

### Manejo de resize

```
Resized(size):
  1. renderer.resize(size)
  2. layout.update(window_width, window_height)
  3. Calcular nuevas cols/rows del pane_area
  4. grid.resize() + pty.resize()
```

---

## T-028 · OSC 0/2 title tracking (Luna-terminal)

**Archivo:** `crates/Luna-terminal/src/parser.rs`

### VteProcessor extendido

```rust
pub struct VteProcessor {
    grid: Rc<RefCell<Grid>>,
    fg: Color,
    bg: Color,
    flags: CellFlags,
    title: Rc<RefCell<String>>,
    cwd: Rc<RefCell<String>>,
}
```

### Constructor

```rust
pub fn new_with_title(grid, title, cwd) -> Self { ... }
```

- `title` y `cwd` son `Rc<RefCell<String>>` inyectados desde `Pane::new()`
- `title_rc()` / `cwd_rc()`: expone los Rc para acceso externo

### osc_dispatch

```rust
fn osc_dispatch(&mut self, params, _bell_terminated) {
    match code_str {
        "0" | "2" => *self.title.borrow_mut() = trimmed,
        "7" => {  // file://host/path → path
            if let Some(path) = param_str.strip_prefix("file://") {
                if let Some(slash_pos) = path.find('/') {
                    *self.cwd.borrow_mut() = path[slash_pos..].to_string();
                }
            }
        }
        _ => {}
    }
}
```

| Código OSC | Propósito | Ejemplo |
|-----------|-----------|---------|
| `OSC 0;title` | Set icon name + title | `echo -ne "\e]0;my title\a"` |
| `OSC 2;title` | Set window title | `echo -ne "\e]2;my title\a"` |
| `OSC 7;file://host/path` | Set CWD | `echo -ne "\e]7;file://host/home/user\a"` |

### Sincronización en main.rs

```rust
RedrawRequested → {
    for tab in tab_bar.tabs.iter_mut() {
        if let Some(p) = panes.iter().find(|p| p.id == tab.active_pane) {
            let t = p.title();
            if !t.is_empty() && t != tab.title {
                tab.title = t;
            }
        }
    }
}
```

Título del tab se actualiza en cada frame si cambió. Si el shell no envía OSC, se muestra "Tab N".

---

## Resumen de cambios en archivos

| Archivo | Estado | Cambio |
|---------|--------|--------|
| `crates/Luna-ui/src/pane.rs` | **REESCRITO** | `PaneId`, `Pane` con PTY + grid + processor + title/cwd via Rc |
| `crates/Luna-ui/src/splitter.rs` | **REESCRITO** | `SplitDirection`, `PaneTree` enum (Leaf/Split) con `all_panes()`, `find_active()` |
| `crates/Luna-ui/src/tab_bar.rs` | **REESCRITO** | `TabId`, `Tab`, `TabBar` con new_tab, close_tab, activate, next/prev, set_title |
| `crates/Luna-ui/src/layout.rs` | **REESCRITO** | `Layout` con `pane_area()`, `tab_width()`, `tab_x()`, `pane_margin()` |
| `crates/Luna-ui/src/theme.rs` | **REESCRITO** | Constantes de paleta Luna + TAB_BAR_HEIGHT |
| `crates/Luna-ui/src/lib.rs` | Modificado | Re-export de `Pane`, `PaneId`, `PaneTree`, `Tab`, `TabBar`, `TabId`, `TAB_BAR_HEIGHT` |
| `crates/Luna-ui/Cargo.toml` | Modificado | Dependencias de Luna-terminal, Luna-renderer, winit |
| `crates/Luna-renderer/src/shaders/ui.wgsl` | **NUEVO** | Shader UI (vertex + fragment) para rectángulos coloreados |
| `crates/Luna-renderer/src/ui.rs` | **NUEVO** | `UIRect`, `UIRenderer` — pipeline completo con instance buffer |
| `crates/Luna-renderer/src/renderer.rs` | Modificado | +`UIRenderer`, +`draw_frame()`, render_instances ahora dibuja cells + UI rects en 1 pass |
| `crates/Luna-terminal/src/parser.rs` | Modificado | +`new_with_title()`, +`title`/`cwd` Rc, +`osc_dispatch` para OSC 0/2/7 |
| `crates/Luna-app/src/main.rs` | Refactorizado | TabBar + Vec<Pane> en event loop, interacción teclado/click, build_tab_bar_ui_rects/text |
| `crates/Luna-app/src/app.rs` | Modificado | +`cursor_x`/`cursor_y` en `AppState` |

## Fixes de bugs de mouse (sesión actual)

### Bug 1: Uso incorrecto de `window.inner_position()` para detectar clicks en tab bar

**Problema:** `MouseInput` calculaba posición usando `window.inner_position()` (origen de la ventana en coordenadas de pantalla) en lugar de la posición del cursor relativa a la ventana. Esto devolvía coordenadas incorrectas (cerca de 0,0) y hacía que `y < TAB_BAR_HEIGHT` nunca se cumpliera.

**Fix:** `CursorMoved` almacena `cursor_x = position.x / sf` y `cursor_y = position.y / sf` en `AppState` **siempre** (no solo cuando selecting). `MouseInput` lee `state.cursor_x` / `state.cursor_y`.

### Bug 2: Doble división por `scale_factor` en `handle_tab_click`

**Problema:** `handle_tab_click` recibía `x` en píxeles lógicos (ya dividido por sf en el caller) y volvía a dividir por sf dentro de la función.

**Fix:** `handle_tab_click` ahora recibe coordenadas ya en espacio lógico.

### Bug 3: Layout no actualizado en `Resized`

**Problema:** `Resized` llamaba `renderer.resize()` pero no `layout.update()`, por lo que los cálculos de `tab_width()` y `pane_area()` siempre usaban el tamaño inicial.

**Fix:** `Resized` ahora llama `layout.update(window_width, window_height)`.

### Tests: 23 tests
```sh
cargo test
# Luna-terminal: 21 tests
# Luna-renderer: 2 tests
```
