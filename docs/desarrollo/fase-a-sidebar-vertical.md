# Fase A — Sidebar vertical de tabs (COMPLETADA)

> Fecha: 2026-05-22

## Resumen

La tab bar se movió de horizontal-superior a vertical-izquierda. El área de panes ahora comienza a la derecha de la sidebar.

## Archivos modificados

| Archivo | Cambio |
|---------|--------|
| `crates/SYNAPSE_-ui/src/theme.rs` | +4 constantes: `SIDEBAR_DEFAULT_WIDTH` (180), `SIDEBAR_TAB_HEIGHT` (36), `SIDEBAR_HEADER_HEIGHT` (48), `SIDEBAR_SCROLL_BTN_H` (20) |
| `crates/SYNAPSE_-ui/src/layout.rs` | `pane_area()` → `(sidebar_width, 0, w-sidebar_width, h)`, métodos `tab_visible_range()` + `tab_y()` reescritos para eje Y, `sidebar_visible_height()` añadido |
| `crates/SYNAPSE_-ui/src/lib.rs` | Exporta nuevas constantes |
| `crates/SYNAPSE_-config/src/config.rs` | Campo `sidebar_width: f32` (default 180) en `Config` |
| `crates/SYNAPSE_-app/src/render.rs` | `build_tab_bar_ui_rects()` y `build_tab_bar_text()` reescritos: sidebar con header, scroll ↑↓, tabs de 36px, borde neón activo 3px, botón "+" fijo abajo |
| `crates/SYNAPSE_-app/src/pane_ops.rs` | `handle_tab_click()` reescrito para hit-test en Y, scroll vertical de tabs |
| `crates/SYNAPSE_-app/src/mouse.rs` | Hover detection + click boundaries cambiados a `cursor_x < sidebar_width`, scroll en sidebar scrollea tabs |
| `crates/SYNAPSE_-app/src/app.rs` | `initialize()` + `handle_scale_factor_change()` actualizados para `sidebar_width` |

## Estructura visual

```
┌──────────────────────┐
│ SYNAPSE_      (48px) │ ← header con logo
│──────────────────────│
│ [▲]           (20px) │ ← scroll up (si overflow)
│ ● ~/dev/synap  [×](36px)│ ← tab activa (borde izq neón)
│   SSH: prod    [×](36px)│ ← tab inactiva
│   build        [×](36px)│
│ [▼]           (20px) │ ← scroll down
│──────────────────────│
│ [+] New tab    (36px) │ ← botón fijo al fondo
└──────────────────────┘
```

## Config TOML

```toml
sidebar_width = 180    # px lógicos, configurable
```

## Tests

- `test_pane_area_with_sidebar` — x=180, w=1100 @ 1280x800
- `test_sidebar_visible_height` — 800 - 48 - 36 = 716px
- `test_tab_y_no_scroll_btn` — tab 0 @ y=48
- `test_tab_visible_range_overflow` — 40 tabs activan scroll
- `test_pane_area_zero_sidebar_width` — sidebar_width=0 → x=0
- `test_pane_area_window_smaller_than_sidebar` — clamp a 0

## Checklist

- [x] `cargo build -p SYNAPSE_-app` limpio
- [x] `cargo test --workspace` — 168 pasan
- [x] `cargo clippy --workspace --all-targets -- -D warnings` limpio
- [x] `cargo fmt --all -- --check` limpio
