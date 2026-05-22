# Fase G — Command Palette

**Completada:** 2026-05-22  
**Impacto:** 🟡 Alto — productividad  
**Tiempo estimado:** 4-5 días  
**Depende de:** Fase F (splash/cursor)

## Resumen

Overlay flotante accesible con `Ctrl+Shift+P` con búsqueda fuzzy sobre todas las acciones, tabs, y temas. Desbloquea la descubribilidad de todas las funcionalidades sin memorizar keybinds.

## Visual

```
┌──────────────────────────────────────────────┐
│  🔍  busqueda en tiempo real...              │ ← input
│──────────────────────────────────────────────│
│     New Tab                        Ctrl+T    │
│  ▶  Close Tab                      Ctrl+W    │ ← seleccionado
│     Split Vertical                 Ctrl+... │
│     Switch to: ~/dev/synapse                  │ ← tab
│     Theme: dracula                            │ ← theme
└──────────────────────────────────────────────┘
```

- Overlay centrado horizontalmente, 600px ancho (mín panel_area - 40px)
- Altura: 40px input + 8 items máximo × 22px + padding
- Fondo: `search_bar_bg`, borde: `panel_active_border` (1px)
- Item seleccionado: highlight `search_highlight`
- Keybinds en texto dim a la derecha de cada action

## Implementación

### Archivo nuevo: `crates/SYNAPSE_-app/src/palette.rs`

#### `PaletteState`

```rust
pub struct PaletteState {
    pub active: bool,
    pub query: String,
    pub results: Vec<PaletteItem>,
    pub selected: usize,
    pub pending_action: Option<Action>,
    pub pending_theme_reload: bool,
}
```

El patrón `pending_action`/`pending_theme_reload` permite que el palette ejecute acciones que requieren acceso a `panes`, `layout`, `window`, etc. sin tener que pasar esos parámetros al módulo de palette. Cuando el usuario presiona Enter en un item Action, se guarda en `pending_action`. El `keyboard.rs` lo chequea al salir del palette gate y lo despacha con `dispatch_action()`.

#### `PaletteItem`

```rust
pub enum PaletteItem {
    Action { label: String, keybind: Option<String>, action: Action },
    Tab { label: String, index: usize },
    Theme { name: String },
}
```

#### `build_palette_items(tab_bar)` — 31 actions + tabs dinámicas + 4 temas

Las 31 acciones cubren todo el enum `Action` (excepto variantes de navegación específica como `TabSwitch1-9`). Las tabs se generan dinámicamente desde `tab_bar.tabs`. Los 4 temas built-in se listan explícitamente.

#### `fuzzy_score(query, candidate)` — subsequence match

Algoritmo simple: recorre `candidate` buscando los caracteres de `query` en orden. Si todos coinciden, devuelve `Some(-first_match_idx)`. Los matches más tempranos rankean mejor.

#### `handle_palette_input(key, event, state, tab_bar)`

| Tecla | Acción |
|-------|--------|
| `Esc` | Cierra el palette |
| `Enter` | Ejecuta item seleccionado, cierra palette |
| `↑` / `↓` | Navega selección |
| `Backspace` | Borra último char del query |
| `Delete` | Borra primer char del query |
| Chars normales | Añade al query, re-filtra resultados |

### Modificaciones en `keyboard.rs`

- **Gate de palette**: insertado entre `history_search` y el `keybind lookup`. Si `state.palette.active`, todas las teclas van a `handle_palette_input()` y retornan.
- **`Action::PaletteOpen`**: manejado en el match de acciones, llama `state.palette.toggle(tab_bar)`.
- **`dispatch_action()`**: función nueva que replica la lógica de dispatch del `handle_keyboard` match block para ejecutar acciones del palette. Se llama cuando `pending_action` está `Some` y el palette acaba de cerrarse.

### Modificaciones en `render.rs`

- **`ui_active`**: añadido `|| state.palette.active` para invalidar el frame cache.
- **Palette overlay**: renderizado en el bloque de reconstrucción de frame, después del history search bar:
  - `UIRect` de fondo + 4 `UIRect`s de borde (1px cada lado)
  - Emoji 🔍 + texto del query + cursor `|`
  - Divider horizontal entre input y resultados
  - Items de resultados (label + keybind hint) con highlight en el seleccionado
  - Mensaje "No matching results" si el query no matchea nada

### Otros archivos modificados

| Archivo | Cambio |
|---------|--------|
| `crates/SYNAPSE_-config/src/keybinds.rs` | Añadido `PaletteOpen` a `Action`, `"palette_open"` a `from_str`, default `Ctrl+Shift+P` |
| `crates/SYNAPSE_-app/src/main.rs` | `mod palette;` |
| `crates/SYNAPSE_-app/src/state.rs` | Campo `palette: PaletteState` en `AppState` |

## Tests (15 nuevos en palette.rs)

| Test | Descripción |
|------|-------------|
| `test_palette_state_new` | Estado inicial vacío, inactive |
| `test_palette_toggle_opens` | Abrir carga items |
| `test_palette_toggle_closes` | Cerrar limpia y desactiva |
| `test_take_pending_action` | take consume el Option |
| `test_take_pending_theme_reload` | take consume el bool |
| `test_fuzzy_score_exact_match` | "new" → "New Tab" |
| `test_fuzzy_score_case_insensitive` | "CLOSE" → "Close Tab" |
| `test_fuzzy_score_subsequence` | "sv" → "Split Vertical" |
| `test_fuzzy_score_no_match` | "zzz" no matchea nada |
| `test_fuzzy_score_empty_query` | Query vacío matchea todo |
| `test_do_fuzzy_filter_returns_all_on_empty_query` | Todos los items con query "" |
| `test_do_fuzzy_filter_specific_action` | "split vertical" → Split Vertical |
| `test_do_fuzzy_filter_no_results` | "zzz nonexistent" → 0 resultados |
| `test_do_fuzzy_filter_theme_results` | "dracula" → Theme |
| `test_palette_item_clone` | Clone preserva campos |

## Métricas

- **Tests totales:** 205 (69+49+12+12+63)
- **Tests nuevos:** 30 (15 palette + 1 keybinds ampliado de 27 a 28)
- **Clippy:** clean
- **cargo fmt:** clean
