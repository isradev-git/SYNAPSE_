# Fase B — Tiling estilo Hyprland (COMPLETADA)

> Fecha: 2026-05-22

## Resumen

Tiling automático de panes con split inteligente, redimensionado por teclado y overlay de oscurecimiento en panes inactivos. La navegación geométrica entre panes ya existía (`adjacent_pane()`), no requirió cambios.

## Archivos modificados

| Archivo | Cambio |
|---------|--------|
| `crates/SYNAPSE_-ui/src/splitter.rs` | `auto_split_direction(rect)` — split por eje mayor; `PaneTree::adjust_ratio()` — resize ±5%, innermost wins, clamped [0.1, 0.9] |
| `crates/SYNAPSE_-config/src/keybinds.rs` | `Action::AutoSplit`, `ResizePaneLeft`, `ResizePaneRight`, `ResizePaneUp`, `ResizePaneDown` — variantes nuevas + bindings por defecto |
| `crates/SYNAPSE_-app/src/keyboard.rs` | Handlers para `AutoSplit` (llama `auto_split_direction` + `split()`) y `ResizePaneLeft/Right/Up/Down` (llama `adjust_ratio()`) |
| `crates/SYNAPSE_-app/src/render.rs` | Overlay dim (`[0,0,0,0.12]`) en todos los panes excepto el activo |

## Keybindings

| Acción | Keybinding |
|--------|-----------|
| `AutoSplit` | `Ctrl+Enter` |
| `ResizePaneLeft` | `Ctrl+Shift+Alt+Left` |
| `ResizePaneRight` | `Ctrl+Shift+Alt+Right` |
| `ResizePaneUp` | `Ctrl+Shift+Alt+Up` |
| `ResizePaneDown` | `Ctrl+Shift+Alt+Down` |

## Lógica de split automático

```
auto_split_direction(rect):
  if rect.w >= rect.h → SplitDirection::Horizontal  (divide verticalmente)
  else                → SplitDirection::Vertical     (divide horizontalmente)
```

## Lógica de resize

```
adjust_ratio(pane_id, split_dir, delta):
  1. Buscar split más interno que contenga pane_id y coincida en dirección
  2. Aplicar delta (±0.05) al ratio
  3. Clamp a [0.1, 0.9]
  4. Si no hay split coincidente en esa dirección → no-op
```

## Estructura visual

```
┌──────────┬──────────┐
│          │          │
│  Pane 1  │  Pane 2  │ ← Ctrl+Enter sobre Pane 1 → split horizontal
│ (activo) │  (dim)   │   Pane 2 oscurecido α 0.12
│          │          │
└──────────┴──────────┘
  ↑ Ctrl+Shift+Alt+Right → ratio 0.5→0.55
```

## Tests

- `test_auto_split_direction_wide_pane` — w>h → Horizontal
- `test_auto_split_direction_tall_pane` — h>w → Vertical
- `test_auto_split_direction_square_pane` — w==h → Horizontal
- `test_adjust_ratio_second_branch_grows` — delta positivo
- `test_adjust_ratio_clamp_min` — ratio no baja de 0.1
- `test_adjust_ratio_clamp_max` — ratio no sube de 0.9
- `test_adjust_ratio_wrong_direction_no_op` — dirección no coincidente → sin cambio
- `test_adjust_ratio_nested_innermost_wins` — split más interno tiene prioridad
- `test_action_from_str_all_actions` — nuevas variantes parseables
- `test_resize_pane_actions_from_str` — variantes resize parseables
- `test_resize_pane_default_bindings` — bindings registrados
- `test_auto_split_action_from_str` — `AutoSplit` parseable
- `test_auto_split_default_binding` — `Ctrl+Enter` registrado

## Checklist

- [x] `cargo build -p SYNAPSE_-app` limpio
- [x] `cargo test --workspace` — 176 pasan
- [x] `cargo clippy --workspace --all-targets -- -D warnings` limpio
- [x] `cargo fmt --all -- --check` limpio
