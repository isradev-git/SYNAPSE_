# Fase 6 — Split de Paneles

## Arquitectura general

```
main.rs (event loop)
    │
    ├── TabBar ────────── Vec<Tab>
    │                        ├── pane_tree: PaneTree ──── Split { direction, ratio, first, second }
    │                        │
    │                        └── active_pane: PaneId ─── que Leaf está activo
    │
    ├── Vec<Pane> ──────── Pane ── grid + PTY por cada panel
    │
    └── redraw:
         pane_tree.get_layout(pane_rect) → Vec<(PaneId, PaneRect)>
         → renderizar cada pane en su rect
         → borders (1px activo/inactivo) + dividers (2px)
         → tab_bar (Fase 5)
```

Un tab puede tener múltiples panes via `PaneTree::Split`. Cada pane tiene su propio PTY, grid y CWD. Los rectángulos de layout se calculan recursivamente desde el `pane_area`.

---

## T-029 · PaneTree binario

**Archivo:** `crates/Luna-ui/src/splitter.rs`

### Estructuras

```rust
pub struct PaneRect { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }

pub enum SplitDirection { Horizontal, Vertical }

pub enum PaneTree {
    Leaf(PaneId),
    Split { direction: SplitDirection, ratio: f32, first: Box<PaneTree>, second: Box<PaneTree> },
}
```

### Métodos principales

| Método | Comportamiento |
|--------|---------------|
| `leaf(id)` | Constructor Leaf |
| `all_panes()` | Recursivo: colecciona todos los PaneId |
| `find_active(active)` | Si existe en el árbol lo retorna; si no, el primero |
| `split(id, new_id, dir) -> Result` | Reemplaza Leaf(id) con Split(Leaf(id), Leaf(new_id)) ratio=0.5 |
| `close(id) -> Option<PaneId>` | Remueve Leaf(id), reemplaza el Split padre con el hermano. No permite cerrar último Leaf |
| `get_layout(rect) -> Vec<(PaneId, PaneRect)>` | Layout recursivo: cada Leaf recibe su sub-rectángulo |
| `get_dividers(rect) -> Vec<DividerInfo>` | Rectángulos hitbox (6px) para detección de hover + drag |
| `set_ratio(pane_id, ratio)` | Busca el Split que contiene pane_id y actualiza su ratio (clamped 0.1–0.9) |

### `split()` — implementación con swap

```rust
// Toma ownership mediante swap para evitar problemas de borrow checker
pub fn split(&mut self, pane_id, new_id, direction) -> Result<(PaneId, PaneId), ()> {
    let mut temp = PaneTree::Leaf(PaneId(u64::MAX));
    std::mem::swap(self, &mut temp);
    let (new_tree, result) = temp.split_into(pane_id, new_id, direction);
    *self = new_tree;
    result
}
```

### `close()` — misma técnica de swap

```rust
pub fn close(&mut self, pane_id) -> Option<PaneId> {
    if self.all_panes().len() <= 1 { return None; }
    let mut temp = PaneTree::Leaf(PaneId(u64::MAX));
    std::mem::swap(self, &mut temp);
    let (new_tree, removed) = temp.remove(pane_id);
    *self = new_tree;
    removed
}
```

El método `remove()` recursivo maneja 3 casos:
- **Direct child**: si first o second es Leaf(pane_id), reemplaza self con el hermano
- **Recurse first**: busca en first; si lo encuentra, reconstruye el Split con new_first
- **Recurse second**: igual con second

### DividerInfo

```rust
pub struct DividerInfo {
    pub hitbox: PaneRect,       // 6px (centrado en la línea de 2px)
    pub direction: SplitDirection,
    pub pane_id: PaneId,        // un pane de la rama "first" del split
    pub parent_rect: PaneRect,  // el área completa del split (para calcular ratio en drag)
}
```

Hitbox de 6px: `[pos - 2, pos + 4]` centrado en la línea de 2px del divisor.

### Tests unitarios (6)

```rust
test_split_leaf                    // split single leaf → Split con 2 panes
test_split_wrong_id                // split con ID que no existe → Err
test_close_pane                    // split + close → collapse a single leaf
test_close_last_pane_fails         // close único leaf → None
test_four_pane_layout_no_overlap   // 4 panes, get_layout, verificar no overlap
test_split_recursive_in_deep_tree  // split en sub-árbol, verificar layout
```

---

## T-030 · Rendering multi-pane

**Archivo:** `crates/Luna-app/src/main.rs` — `RedrawRequested`

### Flujo de rendering

```
RedrawRequested:
  1. Procesar PTY de todos los panes (iterar panes[] entero)
  2. Sincronizar titles
  3. pane_tree.get_layout(pane_rect) → layouts: Vec<(PaneId, PaneRect)>
  4. pane_tree.get_dividers(pane_rect) → dividers: Vec<DividerInfo>
  5. Para cada (pane_id, rect) en layouts:
       a. content_rect = rect - margins
       b. pane_cols = floor(content_w / cell_w)
       c. pane_rows = floor(content_h / cell_h)
       d. Iterar visible_cells(), skip si col>=pane_cols o vrow>=pane_rows
       e. Render cursor (solo si pane_id == active_pane_id)
       f. UIRect para borde: 1px, color PANEL_ACTIVE_BORDER o PANEL_INACTIVE_BORDER
  6. Para cada divider: UIRect de 2px al centro del hitbox, color PANEL_DIVIDER
  7. build_tab_bar_ui_rects() + build_tab_bar_text() (Fase 5)
  8. renderer.draw_frame(cell_data, ui_rects)
```

### Theme añadido

```rust
pub const PANEL_ACTIVE_BORDER: [f32; 4] = [0.71, 0.19, 0.49, 1.0];   // #b5307e
pub const PANEL_INACTIVE_BORDER: [f32; 4] = [0.25, 0.11, 0.43, 1.0]; // #3f1c6d
pub const PANEL_DIVIDER: [f32; 4] = [0.42, 0.16, 0.60, 1.0];        // #6a2a98
```

### Resize multi-pane

`Resized` ahora recalcula cols/rows de **todos** los panes del tab activo usando `get_layout()`:

```rust
for (pane_id, rect) in layouts {
    let new_cols = ((rect.w - margin*2) / cell_w).max(1) as usize;
    let new_rows = ((rect.h - margin*2) / cell_h).max(1) as usize;
    if new_cols != pane.cols || new_rows != pane.rows {
        pane.grid.borrow_mut().resize(new_cols, new_rows);
        pane.pty_session.pty.resize(new_cols as u16, new_rows as u16);
    }
}
```

---

## T-031 · Atajos de teclado

**Archivo:** `crates/Luna-app/src/main.rs`

### SplitAction enum + handler

```rust
enum SplitAction<'a> {
    NewPane(SplitDirection),
    ClosePane,
    Navigate(&'a str),
}

fn handle_split_keyboard<'a>(key: &'a Key) -> Option<SplitAction<'a>> {
    match key {
        Key::Character("d" | "D") => Some(NewPane(Vertical)),
        Key::Character("e" | "E") => Some(NewPane(Horizontal)),
        Key::Character("w" | "W") => Some(ClosePane),
        Key::Named(ArrowUp)    => Some(Navigate("up")),
        Key::Named(ArrowDown)  => Some(Navigate("down")),
        Key::Named(ArrowLeft)  => Some(Navigate("left")),
        Key::Named(ArrowRight) => Some(Navigate("right")),
        _ => None,
    }
}
```

Se invoca solo cuando `Ctrl+Shift` están presionados, ANTES del handler de tabs (Ctrl sin Shift):

```rust
if ctrl && shift {
    if let Some(action) = handle_split_keyboard(logical_key) {
        match action {
            NewPane(dir)  => tree.split(active_id, new_id, dir).ok(), push pane
            ClosePane     => tree.close(active_id).map(remove + kill)
            Navigate(dir) => adjacent_pane(&layouts, active_id, dir).map(set_active)
        }
    }
}
```

### Navegación entre paneles

```rust
fn adjacent_pane(layouts: &[(PaneId, PaneRect)], from: PaneId, dir: &str) -> Option<PaneId> {
    // right: min_by(r.x) where r.x >= from_rect.x + from_rect.w && overlap_y
    // left:  max_by(r.x) where r.x + r.w <= from_rect.x && overlap_y
    // down:  min_by(r.y) where r.y >= from_rect.y + from_rect.h && overlap_x
    // up:    max_by(r.y) where r.y + r.h <= from_rect.y && overlap_x
}
```

---

## T-032 · Drag de divisores

**Archivo:** `crates/Luna-app/src/app.rs` + `main.rs`

### AppState extendido

```rust
pub struct AppState {
    // ... existing: modifiers, selection, selecting, cursor_x, cursor_y
    pub dragging_divider: Option<DividerDrag>,
    pub hover_divider: bool,
}

pub struct DividerDrag {
    pub pane_id: PaneId,
    pub direction: SplitDirection,
    pub parent_rect: PaneRect,   // bounds del split → para calcular ratio
}
```

### Flujo de interacción

```
CursorMoved:
  1. Si dragging_divider:
       ratio = (cursor - parent_rect.start) / parent_rect.size
       tree.set_ratio(pane_id, clamped_ratio)
  2. Si no dragging y zona terminal (y > TAB_BAR_HEIGHT):
       revisar hitbox de cada divider
       si hover: set_cursor_icon(EwResize | NsResize), hover_divider = true
       si no:    set_cursor_icon(Text), hover_divider = false

MouseInput(Pressed, Left):
  1. Si y < TAB_BAR_HEIGHT → handle_tab_click (Fase 5)
  2. Si hover_divider → iniciar drag: dragging_divider = Some(info)
  3. Si no → selection normal

MouseInput(Released, Left):
  1. dragging_divider = None
  2. selecting = false
```

### Ratio clamp

`set_ratio()` clamp a `0.1..0.9` (10% mínimo por panel). La `parent_rect` viene directamente de `DividerInfo.parent_rect` que se computa en `get_dividers()`.

---

## T-033 · PTY independiente por panel

**Archivo:** `crates/Luna-terminal/src/shell.rs` + `pty.rs` + `main.rs`

### ShellConfig extendido

```rust
pub struct ShellConfig {
    pub program: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: Option<String>,   // <-- NUEVO
}
```

`CommandBuilder::cwd(&Path)` se llama si `shell.cwd.is_some()`:

```rust
// En PtyHandle::spawn()
if let Some(ref cwd) = shell.cwd {
    cmd.cwd(std::path::Path::new(cwd));
}
```

### Kill explícito

```rust
impl PtyHandle {
    pub fn kill(&mut self) -> Result<(), String> {
        self.child.kill().map_err(|e| format!("PTY kill error: {}", e))
    }
}
```

Se llama antes de remover un pane del vec:

```rust
// Cerrar pane via Ctrl+Shift+W
if let Some(pane) = panes.iter_mut().find(|p| p.id == removed) {
    let _ = pane.pty_session.pty.kill();
}
panes.retain(|p| p.id != removed);

// Cerrar tab (kill ALL panes de la tab)
for pane in panes.iter_mut() {
    if closed_panes.contains(&pane.id) {
        let _ = pane.pty_session.pty.kill();
    }
}
panes.retain(|p| !closed_panes.contains(&p.id));
```

Debe llamarse antes de `retain` porque `retain` pasa `&T` (no `&mut T`), y `kill` requiere `&mut self`.

### CWD inheritance

Al crear pane via split (`Ctrl+Shift+D/E`):

```rust
let cwd = pane.cwd();
let cwd_opt = if cwd.is_empty() { None } else { Some(cwd) };
panes.push(create_pane_with_cwd(new_pane_id, pane.cols, pane.rows, cwd_opt));
```

`create_pane_with_cwd()` inyecta el path en `ShellConfig.cwd` antes de spawn, lo que hace que `CommandBuilder::cwd()` se ejecute y el nuevo shell herede el directorio del panel activo.

---

## Resumen de cambios en archivos

| Archivo | Estado | Cambio |
|---------|--------|--------|
| `crates/Luna-ui/src/splitter.rs` | **REESCRITO** | `PaneRect`, `DividerInfo`; `split()`, `close()`, `get_layout()`, `get_dividers()`, `set_ratio()` |
| `crates/Luna-ui/src/theme.rs` | Modificado | `PANEL_ACTIVE_BORDER`, `PANEL_INACTIVE_BORDER`, `PANEL_DIVIDER` |
| `crates/Luna-ui/src/lib.rs` | Modificado | Re-export de `PaneRect`, `DividerInfo`, `SplitDirection` |
| `crates/Luna-ui/src/pane.rs` | Modificado | `PaneId` ahora deriva `Ord` + `PartialOrd` |
| `crates/Luna-terminal/src/shell.rs` | Modificado | `ShellConfig.cwd: Option<String>` |
| `crates/Luna-terminal/src/pty.rs` | Modificado | `PtyHandle::kill()`, `cmd.cwd()` si config.cwd presente |
| `crates/Luna-app/src/main.rs` | Refactorizado | Multi-pane rendering, split keyboard, drag de divisores, kill + CWD inheritance |
| `crates/Luna-app/src/app.rs` | Modificado | `DividerDrag` struct, `dragging_divider`, `hover_divider` en AppState |

## Tests

```sh
cargo test --workspace
# 29 tests: 21 Luna-terminal + 2 Luna-renderer + 6 Luna-ui (splitter)
```

Los 6 tests de splitter cubren: split leaf, split con ID erróneo, close y colapso, close del último leaf (no-op), layout de 4 paneles sin solapamiento, split recursivo en árbol profundo.
