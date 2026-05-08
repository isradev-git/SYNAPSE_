# Luna-app Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Descomponer `crates/Luna-app/src/main.rs` (1.218 líneas) en módulos con responsabilidad única, introduciendo `struct App` como punto de cohesión, sin cambiar ningún comportamiento observable.

**Architecture:** Se extrae cada grupo de funciones a su propio módulo como funciones libres (`pub`), después se introduce `struct App` que posee todo el estado y expone métodos `&mut self` que delegan en esas funciones. `main.rs` queda en ~10 líneas.

**Tech Stack:** Rust 2021, winit 0.30 (API deprecada mantenida), `crates/Luna-app` únicamente — ningún otro crate se modifica.

---

## Mapa de archivos

| Archivo | Acción | Responsabilidad final |
|---|---|---|
| `src/main.rs` | Modificar | ~10 líneas: crea `App`, llama `run()` |
| `src/app.rs` | Reemplazar | `struct App`, `new()`, `run()`, `handle_window_event()`, `handle_resize()` |
| `src/state.rs` | Crear (= `app.rs` actual renombrado) | Tipos de estado: `AppState`, `Selection`, `DividerDrag`, `SearchState`, `HistorySearchState` |
| `src/input.rs` | Sin cambios | `InputAction` |
| `src/pane_ops.rs` | Crear | `create_pane`, `create_pane_with_cwd`, `find_pane`, `active_pane_mut`, `adjacent_pane` + `impl App` para `change_font_size`, `handle_tab_click_at` |
| `src/search.rs` | Crear | `find_matches`, `build_match_set`, `update_search_matches`, `scroll_to_current_match` + `impl App` para `handle_search_input`, `handle_history_search_input` |
| `src/render.rs` | Crear | `render_frame` + helpers de tab bar/overlays + `impl App { render() }` |
| `src/mouse.rs` | Crear | `impl App` para `handle_scroll`, `handle_mouse_button`, `handle_cursor_moved` |
| `src/keyboard.rs` | Crear | `impl App` para `handle_keyboard` |

---

## Task 1: Extraer tipos de estado → `state.rs`

**Files:**
- Create: `crates/Luna-app/src/state.rs`
- Modify: `crates/Luna-app/src/main.rs`
- Delete: `crates/Luna-app/src/app.rs` (su contenido pasa a `state.rs`)

- [ ] **Step 1: Crear `state.rs` copiando el contenido completo de `app.rs`**

```
cp crates/Luna-app/src/app.rs crates/Luna-app/src/state.rs
```

El archivo `state.rs` resultante ya tiene el contenido correcto (todos los tipos `AppState`, `Selection`, `DividerDrag`, `SearchState`, `HistorySearchState`). No se modifica nada del contenido.

- [ ] **Step 2: Eliminar `app.rs`**

```
rm crates/Luna-app/src/app.rs
```

- [ ] **Step 3: Actualizar `main.rs` — cambiar `mod app` por `mod state` y todas las referencias**

En `main.rs` línea 1, cambiar:
```rust
mod app;
```
por:
```rust
mod state;
```

Cambiar línea 14:
```rust
use app::AppState;
```
por:
```rust
use state::AppState;
```

Buscar y reemplazar todas las ocurrencias de `app::` por `state::` en `main.rs` (aparecen en: `app::DividerDrag`, `app::Selection`, `app::SearchMatch`).

- [ ] **Step 4: Verificar compilación**

```bash
cargo build -p Luna-app
```

Resultado esperado: compilación exitosa sin errores.

- [ ] **Step 5: Commit**

```bash
git add crates/Luna-app/src/state.rs crates/Luna-app/src/main.rs
git commit -m "refactor(Luna-app): rename app.rs → state.rs"
```

---

## Task 2: Extraer gestión de panes → `pane_ops.rs`

**Files:**
- Create: `crates/Luna-app/src/pane_ops.rs`
- Modify: `crates/Luna-app/src/main.rs`

Las siguientes funciones se mueven verbatim de `main.rs` a `pane_ops.rs` (todas se hacen `pub`):
- `change_font_size` (líneas 756–792)
- `create_pane` (líneas 794–796)
- `create_pane_with_cwd` (líneas 798–813)
- `find_pane` (líneas 815–817)
- `active_pane_mut` (líneas 819–825)
- `adjacent_pane` (líneas 827–865)
- `handle_tab_click` (líneas 867–891)

- [ ] **Step 1: Crear `pane_ops.rs` con los imports y las funciones**

```rust
// crates/Luna-app/src/pane_ops.rs
use std::sync::Arc;
use winit::window::Window;

use luna_renderer::renderer::Renderer;
use luna_terminal::{grid::Grid, pty::PtyHandle, shell};
use luna_ui::{
    layout::Layout,
    pane::{Pane, PaneId},
    splitter::{PaneRect, SplitDirection},
    tab_bar::TabBar,
};

use crate::state::AppState;

// Copiar aquí verbatim las funciones de main.rs líneas 756–891,
// añadiendo `pub` delante de cada `fn`:
//
//   pub fn change_font_size(...)  { ... }   ← líneas 756–792
//   pub fn create_pane(...)       { ... }   ← líneas 794–796
//   pub fn create_pane_with_cwd(...) { ... } ← líneas 798–813
//   pub fn find_pane(...)         { ... }   ← líneas 815–817
//   pub fn active_pane_mut(...)   { ... }   ← líneas 819–825
//   pub fn adjacent_pane(...)     { ... }   ← líneas 827–865
//   pub fn handle_tab_click(...)  { ... }   ← líneas 867–891
```

- [ ] **Step 2: Añadir el módulo en `main.rs` e importar con glob**

Al inicio de `main.rs`, después de `mod state;`:
```rust
mod pane_ops;
use pane_ops::{
    active_pane_mut, adjacent_pane, change_font_size,
    create_pane, create_pane_with_cwd, find_pane, handle_tab_click,
};
```

- [ ] **Step 3: Eliminar las funciones movidas de `main.rs`**

Eliminar de `main.rs` las líneas 756–891 completas (las 7 funciones que ya están en `pane_ops.rs`).

- [ ] **Step 4: Verificar compilación**

```bash
cargo build -p Luna-app
```

Resultado esperado: compilación exitosa. Los call sites en `main.rs` siguen funcionando porque importamos con glob.

- [ ] **Step 5: Commit**

```bash
git add crates/Luna-app/src/pane_ops.rs crates/Luna-app/src/main.rs
git commit -m "refactor(Luna-app): extract pane management to pane_ops.rs"
```

---

## Task 3: Extraer lógica de búsqueda → `search.rs`

**Files:**
- Create: `crates/Luna-app/src/search.rs`
- Modify: `crates/Luna-app/src/main.rs`

Funciones a mover verbatim desde `main.rs`:
- `find_matches` (líneas 1050–1070)
- `build_match_set` (líneas 1072–1080)
- `update_search_matches` (líneas 1082–1091)
- `scroll_to_current_match` (líneas 1093–1113)
- `handle_search_input` (líneas 1115–1168)
- `handle_history_search_input` (líneas 1170–1217)

*(Los números de línea pueden haber variado tras el Task 2 — buscar por nombre de función.)*

- [ ] **Step 1: Crear `search.rs`**

```rust
// crates/Luna-app/src/search.rs
use std::collections::HashSet;

use winit::{
    event::KeyEvent,
    keyboard::{Key, NamedKey},
};

use luna_ui::{pane::Pane, tab_bar::TabBar};

use crate::state::{AppState, SearchMatch};

// Copiar verbatim desde main.rs, añadiendo `pub` a cada función:
//
//   pub fn find_matches(grid: &luna_terminal::grid::Grid, term: &str) -> Vec<SearchMatch>
//   pub fn build_match_set(matches: &[SearchMatch], term_len: usize) -> HashSet<(usize, usize)>
//   pub fn update_search_matches(state: &mut AppState, tab_bar: &TabBar, panes: &[Pane])
//   pub fn scroll_to_current_match(state: &AppState, tab_bar: &TabBar, panes: &[Pane])
//   pub fn handle_search_input(key: &Key, event: &KeyEvent, state: &mut AppState, tab_bar: &TabBar, panes: &[Pane])
//   pub fn handle_history_search_input(key: &Key, event: &KeyEvent, state: &mut AppState, tab_bar: &TabBar, panes: &mut [Pane])
```

- [ ] **Step 2: Añadir módulo e imports en `main.rs`**

```rust
mod search;
use search::{
    build_match_set, find_matches, handle_history_search_input,
    handle_search_input, scroll_to_current_match, update_search_matches,
};
```

- [ ] **Step 3: Eliminar las 6 funciones de `main.rs`**

- [ ] **Step 4: Verificar compilación**

```bash
cargo build -p Luna-app
```

- [ ] **Step 5: Commit**

```bash
git add crates/Luna-app/src/search.rs crates/Luna-app/src/main.rs
git commit -m "refactor(Luna-app): extract search logic to search.rs"
```

---

## Task 4: Extraer lógica de render → `render.rs`

**Files:**
- Create: `crates/Luna-app/src/render.rs`
- Modify: `crates/Luna-app/src/main.rs`

El bloque `WindowEvent::RedrawRequested` (actualmente ~260 líneas inline en main.rs) más `build_tab_bar_ui_rects` y `build_tab_bar_text` se extraen a `render.rs`.

- [ ] **Step 1: Crear `render.rs` con los helpers de tab bar**

```rust
// crates/Luna-app/src/render.rs
use std::collections::HashSet;

use luna_renderer::{renderer::Renderer, ui::UIRect};
use luna_ui::{layout::Layout, pane::Pane, tab_bar::TabBar, theme};

use crate::state::AppState;

const TAB_FONT_SIZE: f32 = 12.0;

// Copiar verbatim desde main.rs:
//   pub fn build_tab_bar_ui_rects(layout: &Layout, tab_bar: &TabBar) -> Vec<UIRect>
//   pub fn build_tab_bar_text(layout: &Layout, tab_bar: &TabBar, scale_factor: f64)
//       -> Vec<(char, f32, f32, f32, [f32; 4], [f32; 4])>
```

- [ ] **Step 2: Añadir `render_frame` extrayendo el bloque RedrawRequested**

Después de las funciones copiadas, añadir en `render.rs`:

```rust
pub fn render_frame(
    renderer: &mut Renderer,
    layout: &Layout,
    tab_bar: &TabBar,
    panes: &mut [Pane],
    state: &AppState,
    cell_w: f32,
    cell_h: f32,
) {
    // Copiar verbatim el cuerpo de WindowEvent::RedrawRequested de main.rs.
    // Al final del bloque, las dos líneas que llaman a las helpers locales:
    //   build_tab_bar_ui_rects(layout, tab_bar)
    //   build_tab_bar_text(layout, tab_bar, ...)
    // siguen funcionando porque están en el mismo módulo.
    //
    // La línea final del bloque original:
    //   renderer.draw_frame(&cell_data, &ui_rects);
    // se convierte en la última línea de render_frame.
}
```

*Nota: la variable `font_size` que el bloque original lee de `state.font_size` pasa a `state.font_size` dentro de `render_frame`; ya está disponible via el parámetro `state`.*

- [ ] **Step 3: Añadir módulo e import en `main.rs`**

```rust
mod render;
use render::{build_tab_bar_text, build_tab_bar_ui_rects, render_frame};
```

- [ ] **Step 4: Reemplazar el bloque RedrawRequested en `main.rs`**

El brazo del match `WindowEvent::RedrawRequested` queda en:
```rust
WindowEvent::RedrawRequested => {
    render_frame(&mut renderer, &layout, &tab_bar, &mut panes, &state, cell_w, cell_h);
}
```

Eliminar también `build_tab_bar_ui_rects` y `build_tab_bar_text` del final de `main.rs` (ya están en `render.rs`).

- [ ] **Step 5: Verificar compilación**

```bash
cargo build -p Luna-app
```

- [ ] **Step 6: Eliminar la constante `TAB_FONT_SIZE` duplicada de `main.rs`** (ahora vive en `render.rs`)

- [ ] **Step 7: Verificar compilación de nuevo**

```bash
cargo build -p Luna-app
```

- [ ] **Step 8: Commit**

```bash
git add crates/Luna-app/src/render.rs crates/Luna-app/src/main.rs
git commit -m "refactor(Luna-app): extract render logic to render.rs"
```

---

## Task 5: Extraer manejadores de ratón → `mouse.rs`

**Files:**
- Create: `crates/Luna-app/src/mouse.rs`
- Modify: `crates/Luna-app/src/main.rs`

Los tres bloques de eventos de ratón (actualmente inline en el match del event loop) se convierten en funciones libres.

- [ ] **Step 1: Crear `mouse.rs`**

```rust
// crates/Luna-app/src/mouse.rs
use std::sync::Arc;

use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton, MouseScrollDelta},
    window::{CursorIcon, Window},
};

use luna_ui::{layout::Layout, pane::Pane, splitter::SplitDirection, tab_bar::TabBar, TAB_BAR_HEIGHT};

use crate::{
    pane_ops::{active_pane_mut, create_pane, find_hovered_divider},
    state::{AppState, DividerDrag, Selection},
};

pub fn handle_scroll(
    delta: MouseScrollDelta,
    panes: &mut [Pane],
    tab_bar: &TabBar,
    cell_h: f32,
) {
    let pane = active_pane_mut(panes, tab_bar);
    let lines = match delta {
        MouseScrollDelta::LineDelta(_, y) => (y.abs() as usize).max(1),
        MouseScrollDelta::PixelDelta(pos) => (pos.y.abs() / cell_h as f64) as usize,
    };
    let is_up = match delta {
        MouseScrollDelta::LineDelta(_, y) => y > 0.0,
        MouseScrollDelta::PixelDelta(pos) => pos.y > 0.0,
    };
    let mut grid_mut = pane.grid.borrow_mut();
    if is_up {
        grid_mut.scroll_down(lines);
    } else {
        grid_mut.scroll_up(lines);
    }
}

pub fn handle_mouse_button(
    button_state: ElementState,
    button: MouseButton,
    state: &mut AppState,
    tab_bar: &mut TabBar,
    panes: &mut Vec<Pane>,
    layout: &Layout,
) {
    // Copiar verbatim el cuerpo del brazo MouseInput del event loop de main.rs.
    // Sustituir `layout.window_width as f64` por ese valor para el handle_tab_click.
}

pub fn handle_cursor_moved(
    position: PhysicalPosition<f64>,
    scale_factor: f64,
    state: &mut AppState,
    tab_bar: &mut TabBar,
    layout: &Layout,
    window: &Arc<Window>,
    cell_w: f32,
    cell_h: f32,
    margin: f32,
) {
    // Copiar verbatim el cuerpo del brazo CursorMoved del event loop de main.rs.
}
```

*Nota: `find_hovered_divider` actualmente está al final de `main.rs` — moverla a `pane_ops.rs` en este mismo step (es una helper de lookup de UI, encaja allí).*

- [ ] **Step 2: Mover `find_hovered_divider` a `pane_ops.rs`**

```rust
// Añadir en pane_ops.rs:
pub fn find_hovered_divider<'a>(
    dividers: &'a [luna_ui::DividerInfo],
    x: f64,
    y: f64,
) -> Option<&'a luna_ui::DividerInfo> {
    dividers.iter().find(|info| {
        let h = info.hitbox;
        x >= h.x as f64 && x <= (h.x + h.w) as f64
            && y >= h.y as f64 && y <= (h.y + h.h) as f64
    })
}
```

Eliminar `find_hovered_divider` de `main.rs` y añadir `find_hovered_divider` al glob import de `pane_ops`.

- [ ] **Step 3: Añadir módulo en `main.rs`**

```rust
mod mouse;
```

- [ ] **Step 4: Reemplazar los tres bloques de eventos en `main.rs`**

```rust
WindowEvent::MouseWheel { delta, .. } => {
    mouse::handle_scroll(delta, &mut panes, &tab_bar, cell_h);
}

WindowEvent::MouseInput { state: button_state, button, .. } => {
    mouse::handle_mouse_button(button_state, button, &mut state, &mut tab_bar, &mut panes, &layout);
}

WindowEvent::CursorMoved { position, .. } => {
    mouse::handle_cursor_moved(
        position,
        window.scale_factor(),
        &mut state,
        &mut tab_bar,
        &layout,
        &window,
        cell_w,
        cell_h,
        margin,
    );
}
```

- [ ] **Step 5: Verificar compilación**

```bash
cargo build -p Luna-app
```

- [ ] **Step 6: Commit**

```bash
git add crates/Luna-app/src/mouse.rs crates/Luna-app/src/pane_ops.rs crates/Luna-app/src/main.rs
git commit -m "refactor(Luna-app): extract mouse handlers to mouse.rs"
```

---

## Task 6: Extraer manejador de teclado → `keyboard.rs`

**Files:**
- Create: `crates/Luna-app/src/keyboard.rs`
- Modify: `crates/Luna-app/src/main.rs`

El bloque completo de `WindowEvent::KeyboardInput` (actualmente ~240 líneas en main.rs) se convierte en una función pública.

- [ ] **Step 1: Crear `keyboard.rs`**

```rust
// crates/Luna-app/src/keyboard.rs
use std::sync::Arc;

use winit::{event::KeyEvent, window::Window};

use luna_config::Action;
use luna_renderer::renderer::Renderer;
use luna_ui::{layout::Layout, pane::Pane, splitter::SplitDirection, tab_bar::TabBar};

use crate::{
    input::InputAction,
    pane_ops::{active_pane_mut, change_font_size, create_pane_with_cwd, find_pane},
    search::{handle_history_search_input, handle_search_input, update_search_matches},
    state::AppState,
};

pub fn handle_keyboard(
    event: &KeyEvent,
    state: &mut AppState,
    tab_bar: &mut TabBar,
    panes: &mut Vec<Pane>,
    layout: &Layout,
    renderer: &mut Renderer,
    margin: f32,
    cell_w: &mut f32,
    cell_h: &mut f32,
    clipboard: &mut Option<arboard::Clipboard>,
    window: &Arc<Window>,
) {
    // Copiar verbatim el cuerpo del brazo WindowEvent::KeyboardInput de main.rs.
    // El `return;` dentro del bloque de búsqueda sigue siendo válido dentro
    // de una función libre — no cambia el comportamiento.
}
```

- [ ] **Step 2: Añadir módulo en `main.rs`**

```rust
mod keyboard;
```

- [ ] **Step 3: Reemplazar el brazo KeyboardInput en `main.rs`**

```rust
WindowEvent::KeyboardInput { event, .. } => {
    keyboard::handle_keyboard(
        &event,
        &mut state,
        &mut tab_bar,
        &mut panes,
        &layout,
        &mut renderer,
        margin,
        &mut cell_w,
        &mut cell_h,
        &mut clipboard,
        &window,
    );
}
```

- [ ] **Step 4: Verificar compilación**

```bash
cargo build -p Luna-app
```

En este punto, `main.rs` tiene únicamente: declaraciones de módulos, setup de inicialización y el event loop con brazos de una sola línea cada uno (~80 líneas totales).

- [ ] **Step 5: Commit**

```bash
git add crates/Luna-app/src/keyboard.rs crates/Luna-app/src/main.rs
git commit -m "refactor(Luna-app): extract keyboard handler to keyboard.rs"
```

---

## Task 7: Introducir `struct App` y finalizar `main.rs`

**Files:**
- Create: `crates/Luna-app/src/app.rs` (nuevo — reemplaza completamente el que había)
- Modify: `crates/Luna-app/src/main.rs`
- Modify: `crates/Luna-app/src/pane_ops.rs`
- Modify: `crates/Luna-app/src/render.rs`
- Modify: `crates/Luna-app/src/mouse.rs`
- Modify: `crates/Luna-app/src/keyboard.rs`
- Modify: `crates/Luna-app/src/search.rs`

- [ ] **Step 1: Crear el nuevo `app.rs` con `struct App`, `new()`, `run()` y `handle_window_event()`**

```rust
// crates/Luna-app/src/app.rs
use std::sync::Arc;

use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
    window::{Window, WindowAttributes},
};

use luna_renderer::renderer::Renderer;
use luna_ui::{layout::Layout, pane::Pane, tab_bar::TabBar};

use crate::{
    pane_ops::create_pane,
    state::AppState,
};

pub struct App {
    pub window:    Arc<Window>,
    pub renderer:  Renderer,
    pub layout:    Layout,
    pub tab_bar:   TabBar,
    pub panes:     Vec<Pane>,
    pub clipboard: Option<arboard::Clipboard>,
    pub state:     AppState,
    pub cell_w:    f32,
    pub cell_h:    f32,
    pub margin:    f32,
}

impl App {
    pub fn new() -> Result<(Self, EventLoop<()>), Box<dyn std::error::Error>> {
        let config = luna_config::Config::load();
        let keybinds = luna_config::Keybinds::new();

        let event_loop = EventLoop::new()?;
        let window_attrs = WindowAttributes::default()
            .with_title("Luna")
            .with_inner_size(winit::dpi::LogicalSize::new(
                config.window_width as f64,
                config.window_height as f64,
            ))
            .with_resizable(true);
        let window = Arc::new(event_loop.create_window(window_attrs)?);
        let mut renderer = Renderer::new(window.clone());

        let mut layout = Layout::new();
        let size = renderer.size();
        layout.update(size.width as f32, size.height as f32);

        let initial_font_size = config.font_size;
        let (cell_w, cell_h) = renderer.cell_metrics(initial_font_size);
        let margin = layout.pane_margin();

        let pane_area = layout.pane_area();
        let cols = ((pane_area.2 - margin * 2.0) / cell_w).max(1.0) as usize;
        let rows = ((pane_area.3 - margin * 2.0) / cell_h).max(1.0) as usize;

        let first_tab_id = luna_ui::tab_bar::TabId(0);
        let first_pane_id = luna_ui::pane::PaneId(0);
        let first_pane = create_pane(first_pane_id, cols, rows);
        let first_tab = luna_ui::tab_bar::Tab::new(first_tab_id, first_pane_id);
        let tab_bar = TabBar::new(first_tab);

        let mut panes = Vec::new();
        panes.push(first_pane);

        let clipboard = arboard::Clipboard::new().ok();
        let state = AppState::new(config, keybinds, initial_font_size);

        Ok((
            App { window, renderer, layout, tab_bar, panes, clipboard, state, cell_w, cell_h, margin },
            event_loop,
        ))
    }

    pub fn run(mut self, event_loop: EventLoop<()>) -> Result<(), Box<dyn std::error::Error>> {
        event_loop.set_control_flow(ControlFlow::Poll);
        #[allow(deprecated)]
        event_loop.run(move |event, elwt| match event {
            Event::WindowEvent { event, .. } => self.handle_window_event(event, elwt),
            Event::AboutToWait => self.window.request_redraw(),
            _ => {}
        })?;
        Ok(())
    }

    fn handle_window_event(
        &mut self,
        event: WindowEvent,
        elwt: &EventLoopWindowTarget<()>,
    ) {
        match event {
            WindowEvent::CloseRequested     => elwt.exit(),
            WindowEvent::Resized(size)      => self.handle_resize(size),
            WindowEvent::ModifiersChanged(m)=> self.state.modifiers = m.state(),
            WindowEvent::MouseWheel { delta, .. }          => self.handle_scroll(delta),
            WindowEvent::MouseInput { state, button, .. }  => self.handle_mouse_button(state, button),
            WindowEvent::CursorMoved { position, .. }      => self.handle_cursor_moved(position),
            WindowEvent::RedrawRequested                   => self.render(),
            WindowEvent::KeyboardInput { event, .. }       => self.handle_keyboard(event),
            _ => {}
        }
    }

    fn handle_resize(&mut self, size: PhysicalSize<u32>) {
        self.renderer.resize(size);
        self.layout.update(size.width as f32, size.height as f32);

        let pane_area = self.layout.pane_area();
        let pane_rect = luna_ui::PaneRect {
            x: pane_area.0, y: pane_area.1,
            w: pane_area.2, h: pane_area.3,
        };
        let layouts = self.tab_bar.active_tab().pane_tree.get_layout(pane_rect);
        let (margin, cell_w, cell_h) = (self.margin, self.cell_w, self.cell_h);

        for (pane_id, rect) in &layouts {
            let new_cols = ((rect.w - margin * 2.0) / cell_w).max(1.0) as usize;
            let new_rows = ((rect.h - margin * 2.0) / cell_h).max(1.0) as usize;
            if let Some(pane) = self.panes.iter_mut().find(|p| p.id == *pane_id) {
                if new_cols != pane.cols || new_rows != pane.rows {
                    pane.cols = new_cols;
                    pane.rows = new_rows;
                    pane.grid.borrow_mut().resize(new_cols, new_rows);
                    let _ = pane.pty_session.pty.resize(new_cols as u16, new_rows as u16);
                }
            }
        }
    }
}
```

- [ ] **Step 2: Añadir `impl App` wrappers en `render.rs`**

Al final de `render.rs`:
```rust
use crate::app::App;

impl App {
    pub(super) fn render(&mut self) {
        render_frame(
            &mut self.renderer,
            &self.layout,
            &self.tab_bar,
            &self.panes,
            &self.state,
            self.cell_w,
            self.cell_h,
        );
    }
}
```

- [ ] **Step 3: Añadir `impl App` wrappers en `mouse.rs`**

Al final de `mouse.rs`:
```rust
use crate::app::App;

impl App {
    pub(super) fn handle_scroll(&mut self, delta: winit::event::MouseScrollDelta) {
        handle_scroll(delta, &mut self.panes, &self.tab_bar, self.cell_h);
    }

    pub(super) fn handle_mouse_button(
        &mut self,
        button_state: winit::event::ElementState,
        button: winit::event::MouseButton,
    ) {
        handle_mouse_button(
            button_state,
            button,
            &mut self.state,
            &mut self.tab_bar,
            &mut self.panes,
            &self.layout,
        );
    }

    pub(super) fn handle_cursor_moved(
        &mut self,
        position: winit::dpi::PhysicalPosition<f64>,
    ) {
        handle_cursor_moved(
            position,
            self.window.scale_factor(),
            &mut self.state,
            &mut self.tab_bar,
            &self.layout,
            &self.window,
            self.cell_w,
            self.cell_h,
            self.margin,
        );
    }
}
```

- [ ] **Step 4: Añadir `impl App` wrapper provisional en `keyboard.rs`**

Al final de `keyboard.rs` (wrapper provisional — se actualizará en Step 6):
```rust
use crate::app::App;

impl App {
    pub(super) fn handle_keyboard(&mut self, event: winit::event::KeyEvent) {
        handle_keyboard(
            &event,
            &mut self.state,
            &mut self.tab_bar,
            &mut self.panes,
            &self.layout,
            &mut self.renderer,
            self.margin,
            &mut self.cell_w,
            &mut self.cell_h,
            &mut self.clipboard,
            &self.window,
        );
    }
}
```

*Este wrapper se actualizará en Step 6 para usar `PostKeyAction` y eliminar los parámetros `renderer/cell_w/cell_h`.*

- [ ] **Step 5: Actualizar `pane_ops.rs`: añadir `impl App` para `change_font_size`**

`change_font_size` como método usa `self` en lugar de los 9 parámetros. La función libre `handle_tab_click` permanece en `pane_ops.rs` y sigue siendo llamada internamente desde `handle_mouse_button`. Añadir al final de `pane_ops.rs`:

```rust
use crate::app::App;

impl App {
    pub(crate) fn change_font_size(&mut self, new_size: f32) {
        self.state.font_size = new_size;
        self.state.config.font_size = new_size;
        let _ = self.state.config.save();

        (self.cell_w, self.cell_h) = self.renderer.cell_metrics(new_size);

        let pane_area = self.layout.pane_area();
        let pane_rect = luna_ui::PaneRect {
            x: pane_area.0, y: pane_area.1,
            w: pane_area.2, h: pane_area.3,
        };
        let layouts = self.tab_bar.active_tab().pane_tree.get_layout(pane_rect);
        let (margin, cell_w, cell_h) = (self.margin, self.cell_w, self.cell_h);

        for (pane_id, rect) in &layouts {
            let new_cols = ((rect.w - margin * 2.0) / cell_w).max(1.0) as usize;
            let new_rows = ((rect.h - margin * 2.0) / cell_h).max(1.0) as usize;
            if let Some(pane) = self.panes.iter_mut().find(|p| p.id == *pane_id) {
                pane.cols = new_cols;
                pane.rows = new_rows;
                pane.grid.borrow_mut().resize(new_cols, new_rows);
                let _ = pane.pty_session.pty.resize(new_cols as u16, new_rows as u16);
            }
        }
    }

}
```

- [ ] **Step 6: Actualizar `keyboard.rs`: refactorizar para usar `PostKeyAction` y eliminar dependencia de `renderer/cell_w/cell_h`**

El `impl App` wrapper del Step 4 pasa `renderer`, `cell_w`, `cell_h` a `handle_keyboard`. En este step, se rompe esa dependencia introduciendo `PostKeyAction`.

**6a. Añadir el enum `PostKeyAction` al inicio de `keyboard.rs` (antes de `use` declarations):**

```rust
pub enum PostKeyAction {
    None,
    FontChange(f32),
}
```

**6b. Cambiar la firma de `handle_keyboard` para que devuelva `PostKeyAction` y elimine `renderer`, `cell_w`, `cell_h`:**

```rust
pub fn handle_keyboard(
    event: &KeyEvent,
    state: &mut AppState,
    tab_bar: &mut TabBar,
    panes: &mut Vec<Pane>,
    layout: &Layout,
    margin: f32,
    clipboard: &mut Option<arboard::Clipboard>,
    window: &Arc<Window>,
) -> PostKeyAction
```

**6c. Dentro del cuerpo de `handle_keyboard`, sustituir las 4 llamadas a `change_font_size` por `return PostKeyAction::FontChange(new_size)`:**

```rust
// Antes (Task 6):
Some(Action::FontIncrease) => {
    let new_size = (state.font_size + 1.0).min(32.0);
    change_font_size(state, renderer, panes, tab_bar, layout, margin, cell_w, cell_h, new_size);
}
// Después (Task 7 Step 6):
Some(Action::FontIncrease) => {
    return PostKeyAction::FontChange((state.font_size + 1.0).min(32.0));
}

Some(Action::FontDecrease) => {
    return PostKeyAction::FontChange((state.font_size - 1.0).max(6.0));
}

Some(Action::FontReset) => {
    return PostKeyAction::FontChange(state.config.font_size);
}

Some(Action::ReloadConfig) => {
    state.config.reload();
    return PostKeyAction::FontChange(state.config.font_size);
}
```

Añadir `PostKeyAction::None` al final del cuerpo de la función (antes del cierre `}`).

**6d. Reemplazar el `impl App` wrapper del Step 4 por la versión actualizada:**

```rust
impl App {
    pub(super) fn handle_keyboard(&mut self, event: winit::event::KeyEvent) {
        let action = handle_keyboard(
            &event,
            &mut self.state,
            &mut self.tab_bar,
            &mut self.panes,
            &self.layout,
            self.margin,
            &mut self.clipboard,
            &self.window,
        );
        if let PostKeyAction::FontChange(size) = action {
            self.change_font_size(size);
        }
    }
}
```

**6e. Eliminar de los `use` de `keyboard.rs` la referencia a `change_font_size` y a `luna_renderer::renderer::Renderer`** (ya no son necesarias).

*Nota: `renderer` ya no se pasa a `handle_keyboard` porque `change_font_size` ahora lo maneja vía `self`.*

- [ ] **Step 7: Reemplazar `main.rs` por la versión final**

```rust
// crates/Luna-app/src/main.rs
mod app;
mod input;
mod keyboard;
mod mouse;
mod pane_ops;
mod render;
mod search;
mod state;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (app, event_loop) = app::App::new()?;
    app.run(event_loop)
}
```

- [ ] **Step 8: Verificar compilación completa**

```bash
cargo build --workspace
```

Resultado esperado: compilación exitosa sin errores ni warnings nuevos.

- [ ] **Step 9: Correr los tests existentes**

```bash
cargo test --workspace
```

Resultado esperado: todos los tests pasan (hay tests en `Luna-terminal` y `Luna-ui`).

- [ ] **Step 10: Verificar que el binario ejecuta correctamente**

```bash
cargo run -p Luna-app
```

Resultado esperado: Luna abre su ventana, el terminal responde a input, tabs y splits funcionan igual que antes.

- [ ] **Step 11: Commit final**

```bash
git add crates/Luna-app/src/
git commit -m "refactor(Luna-app): introduce struct App, main.rs down to 10 lines"
```

---

## Criterio de éxito global

- `cargo build --workspace` limpio
- `cargo test --workspace` sin regresiones
- `main.rs` tiene ≤ 15 líneas
- Ningún archivo nuevo supera 250 líneas
- El binario se comporta idénticamente al anterior
