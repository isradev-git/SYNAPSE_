# Fase 8 — Configuración

## Arquitectura general

```
Luna-config/
    ├── config.rs     ─ Config { font_size, window_width, window_height, scrollback_lines }
    │                   ─ load() / save() / reload() / config_dir()
    └── keybinds.rs   ─ Keybinds { entries: Vec<KeyBindEntry> }
                        ─ Action enum (28 variantes)
                        ─ KeyBindEntry { key, ctrl, shift, alt, action }
                        ─ lookup(key, modifiers) → Option<Action>
                        ─ from_config(overrides) para override TOML

main.rs
    ├── startup: Config::load() → window dimensions, font_size inicial
    ├── AppState { config, keybinds, font_size, fullscreen }
    └── KeyboardInput:
         ├── search/history-search input → delegado
         ├── keybinds.lookup() → dispatch central de shortcuts
         └── fallback → InputAction::from_key (terminal input)
```

El sistema de keybinds reemplaza los matches hardcodeados de fases anteriores por un dispatch centralizado. Todas las acciones (tabs, splits, búsqueda, fuente, pantalla completa, copiar/pegar) pasan por `state.keybinds.lookup()`.

---

## T-037 · Configuración TOML

**Archivos:**
- `crates/Luna-config/src/config.rs` — struct `Config` completo
- `crates/Luna-config/Cargo.toml` — añadido `winit` dependency

### Struct Config

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_font_size")]
    pub font_size: f32,           // 14.0
    #[serde(default = "default_window_width")]
    pub window_width: u32,        // 1280
    #[serde(default = "default_window_height")]
    pub window_height: u32,       // 800
    #[serde(default = "default_scrollback_lines")]
    pub scrollback_lines: usize,  // 100_000
}
```

### Métodos

| Método | Comportamiento |
|--------|---------------|
| `Config::load()` | Lee de `~/.config/Luna/config.toml`. Si no existe, crea con defaults y guarda |
| `Config::save()` | Escribe `config.toml` en el directorio de configuración |
| `Config::reload()` | Relee el archivo TOML, sobreescribe los campos |
| `Config::config_path()` | Devuelve `Option<PathBuf>` según OS |
| `Default` trait | Implementado con valores por defecto |

### Rutas de configuración por OS

| OS | Ruta |
|----|------|
| Linux | `$XDG_CONFIG_HOME/Luna/config.toml` o `~/.config/Luna/config.toml` |
| macOS | `~/Library/Application Support/Luna/config.toml` |
| Windows | `%APPDATA%\Luna\config.toml` |

No se usa crate externo (`dirs`) — implementación manual con `std::env::var` y `#[cfg]`.

---

## T-038 · Sistema de keybinds

**Archivos:**
- `crates/Luna-config/src/keybinds.rs` — `Action`, `KeyCombo`, `KeyBindEntry`, `Keybinds`

### Action enum (28 variantes)

```rust
pub enum Action {
    Search,          // Ctrl+Shift+F
    HistorySearch,   // Ctrl+R
    ClearScreen,     // Ctrl+L
    NewTab,          // Ctrl+T
    CloseTab,        // Ctrl+W
    NextTab,         // Ctrl+Tab
    PrevTab,         // Ctrl+Shift+Tab
    TabSwitch1..9,   // Ctrl+1..9
    SplitVertical,   // Ctrl+Shift+D
    SplitHorizontal, // Ctrl+Shift+E
    ClosePane,       // Ctrl+Shift+W
    NavigateUp/Down/Left/Right,  // Ctrl+Shift+Arrows
    FontIncrease,    // Ctrl+=
    FontDecrease,    // Ctrl+-
    FontReset,       // Ctrl+0
    Fullscreen,      // F11
    Copy,            // Ctrl+Shift+C
    Paste,           // Ctrl+Shift+V
    ReloadConfig,    // Ctrl+,
}
```

### KeyBindEntry (serializable)

```rust
pub struct KeyBindEntry {
    pub key: String,       // "f", "Tab", "F11", "Up", etc.
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub action: String,    // "search", "new_tab", etc.
}
```

### Keybinds struct

- `new()` → construye con `default_entries()` (30 bindings hardcodeados)
- `from_config(overrides)` → aplica overrides TOML sobre los defaults
- `lookup(key, modifiers)` → busca el primer entry que coincida con los modifiers y `key_matches()`

### `key_matches()` — matching de teclas

| Tipo de key | Matcheo |
|-------------|---------|
| `Key::Character(c)` | `c.as_str().eq_ignore_ascii_case(key_str)` |
| `Key::Named(named)` | Mapeo manual: `NamedKey::Tab` ↔ `"Tab"`, `NamedKey::F11` ↔ `"F11"`, etc. |

Soporta alias: `"Enter"`/`"Return"`, `"Up"`/`"ArrowUp"`, etc.

### Migración de shortcuts

Todos los shortcuts que antes tenían `if ctrl && shift { match key { ... } }` ahora usan `state.keybinds.lookup()`. Los handlers `handle_split_keyboard()` y `handle_tab_keyboard()` han sido eliminados. Las enums `SplitAction` y `TabAction` también se eliminaron.

---

## T-039 · Ajuste de tamaño de fuente

**Archivos:**
- `crates/Luna-app/src/main.rs` — `change_font_size()` helper
- `crates/Luna-app/src/app.rs` — `AppState.font_size: f32`

### Shortcuts

| Combinación | Acción |
|-------------|--------|
| Ctrl+= | Incrementar 1pt (máx 32pt) |
| Ctrl+- | Decrementar 1pt (mín 6pt) |
| Ctrl+0 | Reset al default de config |

### `change_font_size()` — proceso completo

1. Actualiza `state.font_size` y `state.config.font_size`
2. Guarda config via `state.config.save()`
3. Recalcula `cell_w`, `cell_h` via `renderer.cell_metrics(new_size)` (rasteriza 'W' con nuevo tamaño)
4. Itera todos los paneles del tab activo, recalcula cols/rows según sus layout rects
5. Redimensiona grid y PTY de cada panel

### Variables de métricas de celda

- Antes: `let (cell_w, cell_h) = renderer.cell_metrics(font_size);` (inmutables)
- Ahora: `let (mut cell_w, mut cell_h) = renderer.cell_metrics(initial_font_size);` (mutables)
- Se actualizan en cada cambio de fuente

### Render section

El `RedrawRequested` handler ahora captura `let font_size = state.font_size;` al inicio, en vez de usar una constante hardcodeada.

---

## T-040 · Pantalla completa (F11)

**Archivos:**
- `crates/Luna-app/src/main.rs` — handler inline en keybind dispatch
- `crates/Luna-app/src/app.rs` — `AppState.fullscreen: bool`

### Implementación

```rust
Some(Action::Fullscreen) => {
    state.fullscreen = !state.fullscreen;
    if state.fullscreen {
        window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
    } else {
        window.set_fullscreen(None);
    }
}
```

Usa `Fullscreen::Borderless(None)` (sin monitor específico). Al cambiar a fullscreen o volver, winit emite `WindowEvent::Resized`, que dispara el recálculo de layout existente.

---

## Cambios en AppState

```rust
pub struct AppState {
    pub config: Config,          // nuevo
    pub keybinds: Keybinds,      // nuevo
    pub font_size: f32,          // nuevo — tamaño actual de fuente
    pub fullscreen: bool,        // nuevo — estado de pantalla completa
    // ... campos existentes
}
```

`AppState::new(config, keybinds, font_size)` recibe configuración pre-cargada.

---

## Tests

No se añadieron tests unitarios específicos. El sistema de configuración es inherentemente dependiente del filesystem del OS. Los 29 tests existentes siguen pasando.

---

## Próxima fase: Fase 9 — Distribución y CI/CD

Tareas pendientes:
- T-041: Configurar cargo-dist
- T-042: Empaquetado macOS (.app + .dmg)
- T-043: Empaquetado Windows (.exe + installer)
- T-044: Empaquetado Linux (AppImage + .deb)
- T-045: CI/CD con GitHub Actions
