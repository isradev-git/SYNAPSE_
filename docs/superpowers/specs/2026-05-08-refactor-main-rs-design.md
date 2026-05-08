# Refactor: Descomposición de `main.rs` en `struct App` + módulos

**Fecha:** 2026-05-08  
**Crate afectado:** `crates/Luna-app`  
**Otros crates:** sin cambios  

---

## Objetivo

`crates/Luna-app/src/main.rs` tiene 1.218 líneas que mezclan: setup de winit, event loop, lógica de rendering, manejo de input, búsqueda, gestión de tabs/panes y helpers varios. El refactor lo descompone en módulos con responsabilidad única, introduciendo un `struct App` como punto de cohesión, sin cambiar ningún comportamiento observable.

---

## Arquitectura resultante

### `struct App`

```rust
pub struct App {
    window:    Arc<Window>,
    renderer:  Renderer,
    layout:    Layout,
    tab_bar:   TabBar,
    panes:     Vec<Pane>,
    clipboard: Option<arboard::Clipboard>,
    state:     AppState,
    cell_w:    f32,
    cell_h:    f32,
}
```

`App::new()` ejecuta todo el setup actual de `main()`. `App::run(event_loop)` contiene el closure de winit y delega cada `WindowEvent` a métodos `&mut self`.

### Estructura de archivos

```
crates/Luna-app/src/
  main.rs       ← ~25 líneas: crea App, llama run()
  app.rs        ← struct App, App::new(), App::run(), handle_window_event()
  state.rs      ← AppState, Selection, DividerDrag, SearchState, HistorySearchState
                   (contenido actual de app.rs, renombrado)
  input.rs      ← InputAction — sin cambios
  keyboard.rs   ← handle_keyboard(), dispatch_keybind(), dispatch_terminal_input()
  mouse.rs      ← handle_scroll(), handle_mouse_input(), handle_cursor_moved()
  search.rs     ← handle_search_input(), handle_history_search_input(),
                   find_matches(), update_search_matches(), scroll_to_current_match(),
                   build_match_set()
  render.rs     ← render(): celdas por pane, bordes, divisores,
                   tab bar (ui_rects + texto), overlays de búsqueda
  pane_ops.rs   ← create_pane(), create_pane_with_cwd(), find_pane(),
                   active_pane_mut(), adjacent_pane(), change_font_size(),
                   handle_tab_click()
```

Ningún archivo nuevo supera ~250 líneas.

---

## Distribución de responsabilidades

| Archivo | Responsabilidad | Líneas aprox. |
|---|---|---|
| `main.rs` | Entry point | ~25 |
| `app.rs` | Struct, init, event dispatch | ~100 |
| `state.rs` | Tipos de estado (sin lógica) | ~120 |
| `input.rs` | Mapeo tecla → InputAction | ~186 (sin cambio) |
| `keyboard.rs` | Keybinds + input al PTY | ~180 |
| `mouse.rs` | Scroll, click, cursor, drag, selección | ~130 |
| `search.rs` | Input de búsqueda + lógica de matches | ~180 |
| `render.rs` | Construcción completa del frame | ~230 |
| `pane_ops.rs` | Creación, lookup y gestión de panes | ~130 |

---

## Comunicación entre módulos

Cada archivo define su propio bloque `impl App`. Rust permite múltiples bloques `impl` en archivos separados dentro del mismo crate. Los métodos que solo se llaman desde `app.rs` o entre módulos hermanos usan visibilidad `pub(super)`. No se expone nada adicional fuera del crate.

```rust
// keyboard.rs
impl App {
    pub(super) fn handle_keyboard(&mut self, event: KeyEvent) { … }
    fn dispatch_keybind(&mut self, action: Action) { … }
    fn dispatch_terminal_input(&mut self, action: InputAction) { … }
}
```

---

## Invariantes preservadas

- **Sin cambios de comportamiento:** toda la lógica existente se mueve, no se reescribe.
- **API winit sin migrar:** se mantiene `event_loop.run()` con `#[allow(deprecated)]`. No se migra a `ApplicationHandler`.
- **PTY reader threads:** `pane.pty_session.rx` permanece igual; el loop de lectura sigue en threads separados.
- **Grid `Rc<RefCell<>>`:** no se cambia el modelo de ownership del grid.
- **`cell_w`/`cell_h`:** pasan de variables locales capturadas en closure a campos de `App`.
- **Clipboard:** de variable capturada en closure a `app.clipboard`.
- **Otros crates:** `Luna-terminal`, `Luna-ui`, `Luna-renderer`, `Luna-config` no se modifican.

---

## Lo que queda fuera de este refactor

- Cursor animado (feature pendiente, trabajo separado)
- Double/triple click (feature pendiente)
- Botón × en tabs (feature pendiente)
- Migración a `ApplicationHandler` de winit 0.30 (puede hacerse después)
- Actualización de dependencias (`wgpu`, `cosmic-text`)

---

## Criterio de éxito

El proyecto compila (`cargo build`) y el binario se comporta idénticamente al anterior: mismos keybinds, mismo rendering, mismas funciones de búsqueda, tabs y splits operativos.
