# Fase 7 — Búsqueda y Productividad

## Arquitectura general

```
main.rs (event loop)
    │
    ├── AppState
    │   ├── search: SearchState          ─ Ctrl+Shift+F (buffer search)
    │   │   ├── active, term, cursor_pos
    │   │   ├── matches: Vec<SearchMatch>  ─ (col, global_row) por cada ocurrencia
    │   │   └── current_match: usize       ─ navegación Enter / Shift+Enter
    │   │
    │   └── history_search: HistorySearchState ─ Ctrl+R (reverse-i-search)
    │       ├── active, term
    │       ├── history: Vec<String>         ─ líneas únicas del buffer (más reciente primero)
    │       ├── matches: Vec<usize>          ─ índices de líneas que contienen el término
    │       └── current_match: usize
    │
    └── redraw:
         ├── match_set: HashSet<(col, row)> ─ posiciones de celdas matcheadas
         ├── highlight: SEARCH_HIGHLIGHT (all) / SEARCH_CURRENT (current)
         ├── search_bar: overlay superior (28px) con input + contador
         └── history_bar: overlay inferior (28px) con prompt + match text
```

Dos modos de búsqueda independientes con UI overlay similar. Cada uno intercepta el teclado mientras está activo, bloqueando toda entrada al PTY.

---

## T-034 · Búsqueda en buffer (Ctrl+Shift+F)

**Archivos modificados:**
- `crates/Luna-ui/src/theme.rs` — colores `SEARCH_*`
- `crates/Luna-app/src/app.rs` — structs `SearchState`, `SearchMatch`
- `crates/Luna-terminal/src/grid.rs` — método `all_lines()` para iterar todo el contenido
- `crates/Luna-app/src/main.rs` — lógica de búsqueda, renderizado y manejo de teclado

### Flujo de activación

1. Usuario presiona `Ctrl+Shift+F` → `state.search.toggle()`
2. Se llama `update_search_matches()` que:
   - Lee todas las líneas del grid + scrollback via `grid.all_lines()`
   - Busca el término (case-insensitive) en cada línea
   - Almacena resultados en `state.search.matches: Vec<SearchMatch>`
3. En cada frame se construye un `HashSet<(col, row)>` con las celdas matcheadas
4. El render loop consulta el match set para decidir el color de fondo de cada celda

### Manejo de teclado en modo search

| Tecla | Acción |
|-------|--------|
| Escape | Cerrar búsqueda |
| Enter | Siguiente match |
| Shift+Enter | Match anterior |
| Backspace | Borrar carácter |
| Delete | Borrar hacia adelante |
| ← → | Mover cursor en el input |
| Home / End | Inicio/final del input |
| Caracteres regulares | Insertar en el término |
| Ctrl+Shift+F | Cerrar búsqueda (toggle off) |

### Renderizado

- **Search bar:** overlay de 28px en la parte superior del `pane_area`
  - Fondo: `SEARCH_BAR_BG` (gris oscuro semitransparente)
  - Texto `Search: <término>|` alineado a la izquierda
  - Contador `N/M matches` alineado a la derecha
  - Cursor `|` en la posición actual de edición

- **Match highlighting:**
  - `SEARCH_HIGHLIGHT` (dorado 35% alpha) — todos los matches
  - `SEARCH_CURRENT` (naranja 55% alpha) — match actual (navegado)
  - Prioridad de color: selección > current match > match > bg normal
  - Los matches se calculan sobre líneas globales (scrollback + grid)
  - Conversión `vrow → global_row` usando `scroll_offset` y `scrollback_len`

### Funciones nuevas en Grid

```rust
pub fn scrollback_len(&self) -> usize           // total líneas en scrollback
pub fn set_scroll_offset(&mut self, offset)     // setear posición de scroll
pub fn all_lines(&self) -> Vec<Vec<char>>       // scrollback + grid como líneas de chars
```

### Scroll al match

Al navegar con Enter/Shift+Enter, se llama `scroll_to_current_match()`:
- Si el match está en scrollback → ajusta `scroll_offset` para centrarlo
- Si el match está en el grid visible → `scroll_to_bottom()`

---

## T-035 · Búsqueda en historial (Ctrl+R)

**Archivos modificados:**
- `crates/Luna-app/src/app.rs` — struct `HistorySearchState`
- `crates/Luna-app/src/main.rs` — handler `handle_history_search_input()`

### Flujo de activación

1. `Ctrl+R` activa `history_search` (solo si no hay otro modo de búsqueda activo)
2. `build_history()` escanea `grid.all_lines()` de más reciente a más antiguo:
   - Colecta líneas no vacías y con len > 1
   - Deduplica (HashSet de strings)
   - Almacena en `state.history_search.history: Vec<String>`
3. `update_filter()` filtra las líneas que contienen el término (case-insensitive)
4. Muestra el match más reciente en el prompt overlay

### Manejo de teclado en modo history search

| Tecla | Acción |
|-------|--------|
| Escape | Cancelar, no escribir nada al PTY |
| Enter | Confirmar: escribe el texto del match al PTY (sin `\r`) |
| Backspace | Borrar último carácter del término |
| Ctrl+R | Siguiente match más antiguo (ciclo) |
| Caracteres regulares | Añadir al término de búsqueda |

### Prompt overlay

- Barra de 28px en la parte **inferior** del `pane_area`
- Formato: `(reverse-i-search)\`término': texto_match`
- Contador `N/M` a la derecha
- El texto del match se trunca si excede el ancho disponible

### Escritura al PTY

Al confirmar con Enter, se escribe el texto del match al PTY como bytes sin `\r` al final. Esto coloca el texto en la línea de input del shell sin ejecutarlo.

---

## T-036 · Limpiar pantalla (Ctrl+L)

**Archivos modificados:**
- `crates/Luna-app/src/main.rs` — handler inline en el KeyboardInput

### Implementación

1. Intercepta `Ctrl+L` antes que `InputAction::from_key`
2. Ejecuta localmente sobre el grid del panel activo:
   - `grid.clear_region(0, rows - 1)` — limpia todas las celdas visibles
   - `grid.set_cursor(0, 0)` — reposiciona el cursor
3. También envía `\x0c` al PTY (`pty.write(b"\x0c")`) para shells con readline (bash/zsh) que necesitan redibujar
4. El scrollback **no** se borra (solo la vista activa del grid)

### Consideración

El parser VT (`VteProcessor::execute()`) ya maneja `\x0c` (form feed) como clear display + cursor home. La implementación de T-036 es un clear *local* que no depende de que el shell ecocancie la secuencia de vuelta.

---

## Colores nuevos (theme.rs)

```rust
pub const SEARCH_BAR_HEIGHT: f32 = 28.0;
pub const SEARCH_BAR_BG: [f32; 4]       = [0.08, 0.08, 0.12, 0.97];  // casi negro
pub const SEARCH_HIGHLIGHT: [f32; 4]     = [1.0, 0.84, 0.0, 0.35];    // dorado
pub const SEARCH_CURRENT: [f32; 4]       = [1.0, 0.45, 0.0, 0.55];    // naranja
pub const SEARCH_TEXT: [f32; 4]          = [0.85, 0.85, 0.85, 1.0];   // gris claro
pub const SEARCH_TEXT_DIM: [f32; 4]      = [0.5, 0.5, 0.5, 1.0];     // gris tenue
```

---

## Tests

No se añadieron tests unitarios específicos para esta fase. La búsqueda depende del estado visual (grid + scrollback) y la interacción con winit, lo que la hace más adecuada para tests de integración/UI en fases futuras.

Los 29 tests existentes (21 terminal + 6 ui + 2 renderer) siguen pasando sin regresiones.

---

## Próxima fase: Fase 8 — Configuración

Tareas pendientes:
- T-037: Sistema de configuración TOML
- T-038: Keybinds personalizables
- T-039: Ajuste de tamaño de fuente en runtime
- T-040: Fullscreen (F11)
