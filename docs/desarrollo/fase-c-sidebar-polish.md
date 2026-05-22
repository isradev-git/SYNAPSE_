# Fase C — Sidebar polish + indicadores (COMPLETADA)

> Fecha: 2026-05-22

## Resumen

Indicadores visuales en la sidebar y overlays de contexto en panes: dot de proceso vivo/muerto, glow neón en tab activa, overlay de etiqueta de pane (600ms) al navegar, e indicador de dimensiones al redimensionar dividers.

## Archivos modificados

| Archivo | Cambio |
|---------|--------|
| `crates/SYNAPSE_-ui/src/tab_bar.rs` | `Tab.alive: bool` — indica si el proceso del pane sigue vivo; `Tab::new()` inicializa a `true` |
| `crates/SYNAPSE_-app/src/state.rs` | `AppState.pane_label_until: Option<Instant>` + `pane_label_id: u32` — timer del overlay de etiqueta |
| `crates/SYNAPSE_-config/src/config.rs` | Tres flags: `show_pane_labels`, `show_resize_indicator`, `sidebar_show_process_dot` (todos `true` por defecto) |
| `crates/SYNAPSE_-app/src/render.rs` | Glow neón en tab activa; dot `●` verde/rojo; overlay `P1`/`P2`; indicador `cols×rows`; `label_active`/`resize_active` en `needs_cell_rebuild` |
| `crates/SYNAPSE_-app/src/keyboard.rs` | Set `pane_label_until`/`pane_label_id` tras `Navigate` exitoso |

## Glow neón en tab activa

Dos `UIRect` extra empujados **antes** del borde sólido (orden importa para alpha blending):

```rust
// glow externo — 10px ancho, α 0.1
UIRect { pos: [0.0, y], size: [10.0, tab_h], color: [1.0, 0.0, 0.24, 0.1] }
// glow interno — 6px ancho, α 0.3
UIRect { pos: [0.0, y], size: [6.0,  tab_h], color: [1.0, 0.0, 0.24, 0.3] }
// borde sólido — 3px
UIRect { pos: [0.0, y], size: [3.0,  tab_h], color: theme.panel_active_border }
```

Solo renderizado si `effects_enabled == true`.

## Dot de proceso

Carácter `●` antes del título del tab:

| Estado | Color |
|--------|-------|
| `alive = true` | `#00CC44` verde `[0.0, 0.8, 0.267, 1.0]` |
| `alive = false` | `#FF003C` rojo `[1.0, 0.0, 0.235, 1.0]` |

El título se desplaza `char_w * 1.5` a la derecha cuando el dot está activo.

## Overlay de etiqueta de pane

Al ejecutar `Navigate` con éxito:
- `pane_label_until = Instant::now() + 600ms`
- `pane_label_id = next.0 as u32`

En `render_frame`, si `label_active`:
- Localiza el pane en `layouts` por `PaneId`
- Renderiza `P{pos+1}` (ej. `P1`, `P2`) en top-left del pane (margin + 4px offset)
- Font size 14px, color `theme.fg`, fondo transparente

## Indicador de redimensionado

Mientras `dragging_divider.is_some()` y `show_resize_indicator`:
- Calcula `cols = content_w / cell_w`, `rows = content_h / cell_h` del pane activo
- Texto: `"{cols}×{rows}"` (× = U+00D7)
- Centrado en el pane, pill oscuro `[0,0,0,0.7]` de fondo, 6px padding H / 4px V
- Font size 12px

## Config TOML

```toml
show_pane_labels = true        # overlay P1/P2 al navegar (600ms)
show_resize_indicator = true   # cols×rows al arrastrar divider
sidebar_show_process_dot = true # ● verde/rojo por tab
```

## Estructura visual

```
Sidebar:
┌─────────────────────┐
│ ●● ~/dev/synap  [×] │ ← dot verde + glow neón izquierdo
│    SSH: prod    [×] │ ← tab inactiva (sin glow)
└─────────────────────┘

Pane tras Navigate:
┌──────────────────────┐
│P2                    │ ← overlay 600ms, top-left
│                      │
└──────────────────────┘

Pane durante resize:
┌──────────────────────┐
│                      │
│       80×24          │ ← pill centrado mientras arrastra
│                      │
└──────────────────────┘
```

## Tests

- `test_tab_alive_default_true` — `Tab::new()` → `alive == true`
- `test_pane_label_initial_none` — `AppState::new()` → `pane_label_until == None`
- `test_config_ui_flags_default` — tres flags en `true` por defecto

## Checklist

- [x] `cargo build -p SYNAPSE_-app` limpio
- [x] `cargo test --workspace` — 183 pasan
- [x] `cargo clippy --workspace --all-targets -- -D warnings` limpio
- [x] `cargo fmt --all -- --check` limpio
