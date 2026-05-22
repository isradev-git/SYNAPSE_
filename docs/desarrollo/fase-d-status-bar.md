# Fase D — Status bar (COMPLETADA)

> Fecha: 2026-05-22

## Resumen

Barra horizontal de 18px en la parte inferior con información contextual. Toggle con `Ctrl+Shift+S`. El área de panes se reduce cuando está visible.

## Archivos modificados

| Archivo | Cambio |
|---------|--------|
| `crates/SYNAPSE_-ui/src/theme.rs` | `STATUS_BAR_HEIGHT: f32 = 18.0` |
| `crates/SYNAPSE_-ui/src/lib.rs` | Exporta `STATUS_BAR_HEIGHT` |
| `crates/SYNAPSE_-ui/src/layout.rs` | `Layout.status_bar_visible: bool`; `pane_area()` resta 18px cuando activa |
| `crates/SYNAPSE_-config/src/config.rs` | 4 flags: `status_bar`, `status_bar_show_git`, `status_bar_show_k8s`, `status_bar_show_time` |
| `crates/SYNAPSE_-config/src/keybinds.rs` | `Action::ToggleStatusBar` + binding `Ctrl+Shift+S` |
| `crates/SYNAPSE_-app/src/state.rs` | 6 campos nuevos + helpers `spawn_git_branch_reader`, `read_k8s_context`, `read_user_host` |
| `crates/SYNAPSE_-app/src/render.rs` | `build_status_bar()`; polling git branch rx; `status_time_dirty`; integración en `render_frame` |
| `crates/SYNAPSE_-app/src/keyboard.rs` | `PostKeyAction::ToggleStatusBar`; handler en free fn + `AppCore::handle_keyboard` |
| `crates/SYNAPSE_-app/src/app.rs` | `layout.status_bar_visible = config.status_bar` en `initialize()` |
| `crates/SYNAPSE_-app/src/pane_ops.rs` | `layout.status_bar_visible` sync en `handle_scale_factor_change()` |

## Layout con status bar

```
┌──────┬──────────────────────────────┐
│      │                              │
│ SIDE │         PANES                │
│  BAR │                              │
│      ├──────────────────────────────│
│      │ ~/dev  main  user@host  14:22│
└──────┴──────────────────────────────┘
```

`pane_area()`:
```rust
let bar_h = if self.status_bar_visible { STATUS_BAR_HEIGHT } else { 0.0 };
let h = (self.window_height - bar_h).max(0.0);
```

## Contenido de la status bar

| Zona | Contenido |
|------|-----------|
| Izquierda | CWD (máx 30 chars, `~/` sustituye `/home/user`), rama git, contexto k8s |
| Centro | `user@hostname` |
| Derecha | `HH:MM:SS` UTC (actualiza cada segundo) |

## Git branch (async)

- `spawn_git_branch_reader(cwd)` — spawns thread, recorre dirs hacia arriba buscando `.git/HEAD`
- Resultado almacenado en `AppState.git_branch: Option<String>`
- Se relanza en `tab_changed` o `first_frame`
- Polling en `render_frame` via `state.git_branch_rx.as_ref().and_then(|rx| rx.try_recv().ok())`

## Clock dirty

- `cached_time_sec: u64` en AppState (UTC, segundos desde epoch)
- `status_time_dirty` en render_frame cuando el segundo cambia → fuerza rebuild de frame cache
- Sin `sleep` ni hilo extra — aprovecha el render loop existente

## Config TOML

```toml
status_bar = true
status_bar_show_git = true
status_bar_show_k8s = true
status_bar_show_time = true
```

## Tests

- `test_pane_area_with_status_bar` — h = 800 - 18 = 782 cuando visible
- `test_pane_area_status_bar_hidden` — h = 800 cuando oculta
- `test_config_status_bar_defaults` — 4 flags en `true` por defecto
- `test_toggle_status_bar_action_from_str` — `"toggle_status_bar"` parseable
- `test_toggle_status_bar_default_binding` — `Ctrl+Shift+S` registrado
- `test_status_bar_initial_state` — visible=true, branch=None, cached_time=0, user_host no vacío

## Checklist

- [x] `cargo build -p SYNAPSE_-app` limpio
- [x] `cargo test --workspace` — 197 pasan
- [x] `cargo clippy --workspace --all-targets -- -D warnings` limpio
- [x] `cargo fmt --all -- --check` limpio
