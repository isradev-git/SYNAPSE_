# Fase 4 — Input de Usuario

## T-021 · Captura de input de teclado (input.rs)

**Archivo:** `crates/Luna-app/src/input.rs` (nuevo en Fase 4)

```rust
pub enum InputAction {
    Write(Vec<u8>),     // bytes para enviar al PTY
    ScrollUp(usize),    // scroll local hacia arriba
    ScrollDown(usize),  // scroll local hacia abajo
    ScrollToTop,        // scroll al inicio del scrollback
    ScrollToBottom,     // scroll al final del scrollback
    Copy,               // Ctrl+Shift+C
    Paste,              // Ctrl+Shift+V
    Ignore,             // tecla no mapeada
}
```

- `from_key(event, modifiers)`: punto de entrada único para todo input de teclado
- `event.text` → handle de texto imprimible + Ctrl+letter:
  - Ctrl+Shift+C → Copy (bytes `[3]` + control + shift)
  - Ctrl+Shift+V → Paste (bytes `[22]` + control + shift)
  - Todo lo demás → `Write(bytes)`
- `from_named_key(key, modifiers)`: teclas sin text (especiales):
  - `Key::Character(c)`: Ctrl+letra → byte de control (Ctrl+A=1, Ctrl+Z=26, Ctrl+[=27, etc.)
    - Sin Ctrl → `Write(c.as_bytes())`
  - `Named::Enter` → `\r`, `Backspace` → `\x7f`, `Escape` → `\x1b`
  - `Tab` → `\t`, `Shift+Tab` → `\x1b[Z`
  - Flechas: CSI `\x1b[A`..`\x1b[D` con modificadores Shift (`1;2`) y Ctrl (`1;5`)
  - Home/End: CSI `H`/`F` con modificadores
  - Delete: CSI `3~` con modificadores (`3;2~`, `3;5~`)
  - Insert: CSI `2~`
  - PgUp/PgDn: CSI `5~`/`6~` (normal); Shift → scroll local 24 líneas; Ctrl+Shift → top/bottom
  - F1-F12: secuencias xterm (`\x1bOP`..`\x1b[24~`)

## T-022 · Scroll con teclado y ratón

### Mouse wheel

```rust
WindowEvent::MouseWheel { delta, .. } => {
    // LineDelta → abs(y) líneas; PixelDelta → abs(y) / cell_h
    // y > 0 → scroll_down (hacia arriba), y < 0 → scroll_up
}
```

### Scroll con teclado (en input.rs)

| Tecla | Modo | Acción |
|-------|------|--------|
| Shift+PgUp | scroll | `ScrollUp(24)` — sube 24 líneas |
| Shift+PgDn | scroll | `ScrollDown(24)` — baja 24 líneas |
| Ctrl+Shift+PgUp | scroll | `ScrollToTop` — inicio del scrollback |
| Ctrl+Shift+PgDn | scroll | `ScrollToBottom` — fondo |

### Grid scroll methods

**Archivo:** `crates/Luna-terminal/src/grid.rs` (aumentados en Fase 4)

```rust
pub fn scroll_up(&mut self, lines: usize)     // incrementa scroll_offset (hacia atrás)
pub fn scroll_down(&mut self, lines: usize)   // decrementa scroll_offset (hacia adelante)
pub fn scroll_to_bottom(&mut self)            // scroll_offset = 0
pub fn scroll_to_top(&mut self)               // scroll_offset = scrollback.len()
pub fn is_at_bottom(&self) -> bool            // scroll_offset == 0
```

### Auto-scroll en escritura

En `main.rs:174`, al recibir `InputAction::Write`:

```rust
if bytes != b"\x1b[5~" && bytes != b"\x1b[6~" {
    grid.borrow_mut().scroll_to_bottom();
}
```

Auto-scroll al fondo cuando el usuario escribe texto, excepto para PgUp/PgDn (que son secuencias de navegación dentro del programa, no del scrollback).

### shift_up interno

`grid.new_line()` llama a `shift_up(1)` cuando el cursor excede rows-1. `shift_up`:

1. Push de la línea superior a scrollback
2. **Mantiene scroll_offset**: si el usuario está viendo historial (scroll_offset > 0), incrementa scroll_offset para mantener la posición viewport estable
3. Shift de todas las filas hacia arriba
4. Empty en la última fila

Esto asegura que las líneas nuevas no empujen el viewport lejos del usuario cuando está scrolleado.

## T-023 · Selección de texto con ratón

**Archivo:** `crates/Luna-app/src/app.rs` (nuevo en Fase 4)

```rust
pub struct Selection {
    pub start: (usize, usize),  // (col, viewport_row)
    pub end: (usize, usize),
}

impl Selection {
    pub fn new(col, row) -> Self
    pub fn update_end(&mut self, col, row)
    pub fn normalized(&self) -> ((usize, usize), (usize, usize))  // (min, max) ordenado
    pub fn contains(&self, col, row) -> bool
}
```

### Flujo de selección

```
MouseInput(Pressed, Left) → state.selecting = true
CursorMoved (si selecting) → {
    col = (x / sf - margin) / cell_w
    vrow = (y / sf - margin) / cell_h
    selection.update_end(col, vrow) o Selection::new(col, vrow)
}
MouseInput(Released, Left) → state.selecting = false
```

### Rendering de selección

En el loop de celdas (`main.rs:144`):

```rust
let bg = if state.selection.as_ref().map_or(false, |s| s.contains(col, vrow)) {
    [1.0, 0.239, 0.58, 0.4]  // #ff3d94 con 40% alpha
} else {
    cell.bg.bg_rgba()
};
```

El color de selección se aplica como background de la celda, overrideando el bg original. El shader se encarga del blending alfa.

### extract_selection

**Archivo:** `crates/Luna-app/src/main.rs:228`

```rust
fn extract_selection(grid: &Grid, sel: &Selection, cols: usize) -> String
```

1. Normaliza start/end (fila menor primero)
2. Itera `(vrow in start_row..=end_row, col in line_start..=line_end)`
3. Usa `grid.get_visible(col, vrow)` → resuelve scrollback o grid
4. Concatena caracteres, trim trailing spaces por línea
5. Añade `\n` entre líneas, trim final

No implementado (pendiente para mejora): doble click (selección de palabra), triple click (selección de línea).

## T-024 · Copiar y pegar (clipboard con arboard 3)

**Dependencia:** `arboard = { workspace = true }` en `Luna-app/Cargo.toml`

### Copy (Ctrl+Shift+C)

```rust
InputAction::Copy => {
    let grid_ref = grid.borrow();
    if let Some(ref sel) = state.selection {
        let text = extract_selection(&grid_ref, sel, cols);
        if let Some(ref mut clip) = clipboard {
            let _ = clip.set_text(text);
        }
    }
}
```

- `clipboard` es `Option<arboard::Clipboard>` (puede fallar en algunos entornos)
- `set_text(text)` envía a clipboard del sistema

### Paste (Ctrl+Shift+V)

```rust
InputAction::Paste => {
    if let Some(ref mut clip) = clipboard {
        if let Ok(text) = clip.get_text() {
            // Bracketed paste
            let _ = pty_session.pty.write(b"\x1b[200~");
            let _ = pty_session.pty.write(text.as_bytes());
            let _ = pty_session.pty.write(b"\x1b[201~");
        }
    }
}
```

- Bracketed paste mode envuelve el texto pegado entre `\e[200~` y `\e[201~`
- El shell (bash/zsh) automáticamente trata el contenido como un solo bloque, evitando que caracteres especiales (tabs, newlines) sean interpretados como comandos

### Diferencia Ctrl+C vs Copy

| Combo | bytes | Modifiers | Acción |
|-------|-------|-----------|--------|
| Ctrl+C | `[3]` | control | Write → PTY (SIGINT) |
| Ctrl+Shift+C | `[3]` | control + shift | Copy → clipboard |

Misma lógica para Ctrl+V (Write `[22]`) vs Ctrl+Shift+V (Paste).

### Estado del clipboard

```rust
let mut clipboard = arboard::Clipboard::new().ok();
```

- `Option` porque `Clipboard::new()` puede fallar (entornos headless, Wayland sin protocolo)
- En `Copy` y `Paste`: `if let Some(ref mut clip) = clipboard`

## Refactor de main.rs

**Archivo:** `crates/Luna-app/src/main.rs` — reescrito significativamente en Fase 4

### AppState

```rust
pub struct AppState {
    pub modifiers: ModifiersState,  // trackeado via ModifiersChanged
    pub selection: Option<Selection>,
    pub selecting: bool,
}
```

### Event loop completo

| Evento | Manejo |
|--------|--------|
| `CloseRequested` | `elwt.exit()` |
| `Resized` | resize renderer + grid + PTY |
| `ModifiersChanged` | actualizar `state.modifiers` |
| `MouseWheel` | scroll grid (up/down según delta) |
| `MouseInput(Left, Pressed)` | `state.selecting = true` |
| `MouseInput(Left, Released)` | `state.selecting = false` |
| `CursorMoved` | si selecting: crear/actualizar Selection |
| `KeyboardInput` | `InputAction::from_key()` → Write/Scroll/Copy/Paste |
| `RedrawRequested` | drenar PTY reader + build cell_data + render |
| `AboutToWait` | `window.request_redraw()` |

### Detalles API winit 0.30

- `ModifiersChanged(mods)` donde `mods: Modifiers` con `.state()` → `ModifiersState`
- `KeyEvent` no tiene campo `modifiers`; se trackean separadamente vía el evento
- `KeyboardInput { event, .. }` donde `event: KeyEvent` con `.logical_key`, `.text`, `.state`, `.repeat`
- `Key<SmolStr>.as_ref()` → `Key<&str>` para pasar a `from_named_key`

### CellMetrics de celda

```rust
cell_metrics('W', 14.0) → (width + 1, height + 4)
```
- Ancho: `glyph.placement.width + 1px`
- Alto: `glyph.placement.height + 4px`

Margen de 4px alrededor del contenido.

### Render loop

```
RedrawRequested:
  1. while let Ok(data) = pty_session.rx.try_recv() → processor.process(&data)
  2. grid_ref = grid.borrow()
  3. for (col, vrow, cell) in grid_ref.visible_cells():
       - skip si posición = cursor (no scrolleado)
       - skip si ' ' + bg default
       - bg = selection color si seleccionado
       - push a cell_data
  4. cursor overlay (si no scrolleado)
  5. renderer.draw_cells(&cell_data)
```

## Resumen de cambios en archivos

| Archivo | Estado | Cambio |
|---------|--------|--------|
| `crates/Luna-app/src/input.rs` | **NUEVO** | `InputAction` enum + `from_key()` + `from_named_key()` |
| `crates/Luna-app/src/app.rs` | **NUEVO** | `AppState` + `Selection` con normalized/contains |
| `crates/Luna-app/src/main.rs` | Modificado | Event loop completo: scroll, selección, clipboard, AppState, extract_selection |
| `crates/Luna-app/Cargo.toml` | Modificado | +`arboard` |
| `crates/Luna-terminal/src/grid.rs` | Modificado | +scroll_up/down/to_top/to_bottom/is_at_bottom/get_visible; shift_up renombrado |

### Tests: 23 tests

```sh
cargo test
# Luna-terminal: 21 tests (shell:1, pty:2, parser:8, grid:6, buffer:3, scroll:1)
# Luna-renderer: 2 tests (text:1, atlas:1)
```
