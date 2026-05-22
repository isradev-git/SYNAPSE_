# Fase E — Postproc shaders (COMPLETADA)

> Fecha: 2026-05-22

## Resumen

Pipeline de postprocesamiento GPU con efectos cyberpunk configurables. El renderer escribe a una textura offscreen que luego pasa por un pipeline de postproc (bloom H → composite final) antes de presentarse en pantalla. Todos los efectos son configurables via `[effects]` en config.toml y se activan/desactivan con `Ctrl+Shift+E`.

## Archivos modificados

| Archivo | Cambio |
|---------|--------|
| `crates/SYNAPSE_-config/src/effects.rs` | `EffectsConfig` + sub-configs (`ScanlinesConfig`, `BloomConfig`, `ChromaConfig`, `MatrixBgConfig`) + `parse_hex_color()` |
| `crates/SYNAPSE_-config/src/config.rs` | Campo `effects: EffectsConfig` en `Config` |
| `crates/SYNAPSE_-config/src/lib.rs` | Exporta `effects` module + `EffectsConfig` |
| `crates/SYNAPSE_-config/src/keybinds.rs` | `EffectsToggle` action + `Ctrl+Shift+E` default binding |
| `crates/SYNAPSE_-renderer/src/postproc.rs` | `PostProcRenderer` — offscreen texture, bloom H pass, composite pass, `PostProcUniform`, `BloomUniform` |
| `crates/SYNAPSE_-renderer/src/shaders/postproc.wgsl` | Shader composite: scanlines, vignette, barrel distortion, bloom V composite, chroma aberration, glitch, matrix rain, hex grid |
| `crates/SYNAPSE_-renderer/src/shaders/bloom_h.wgsl` | Bloom horizontal pass — threshold + 13-tap gaussian blur |
| `crates/SYNAPSE_-renderer/src/lib.rs` | `pub mod postproc` |
| `crates/SYNAPSE_-renderer/src/renderer.rs` | Campo `postproc`, `effects_enabled`, `effects_config`, `start_time`, `glitch_timer`; render a offscreen + postproc → surface; `build_postproc_uniform()`; `set_effects_enabled/config` |
| `crates/SYNAPSE_-renderer/Cargo.toml` | Dependencia `SYNAPSE_-config` |
| `crates/SYNAPSE_-app/src/state.rs` | `effects_enabled: bool` + `cursor_trail` |
| `crates/SYNAPSE_-app/src/render.rs` | Neon border pulse (si `pane_pulse`), cursor trail rendering |
| `crates/SYNAPSE_-app/src/app.rs` | `renderer.set_effects_config/enabled` en init; handler de `EffectsToggle` |
| `crates/SYNAPSE_-app/src/keyboard.rs` | `PostKeyAction::EffectsToggle` → `renderer.set_effects_enabled` |

## Pipeline de render

```
Frame N:
  Pass 0 → offscreen_tex  [bg + cells + ui + underline + images + cursor]
  Pass 1a → bloom: threshold + downsample + gaussian H (bloom_h.wgsl)
  Pass 2 → composite: offscreen + bloom V + chroma + scanlines + vignette + glitch + matrix → surface
```

Todos los efectos OFF → 0 overhead. El pipeline offscreen→postproc siempre corre, pero el shader hace passthrough cuando no hay efectos activos.

## Efectos implementados

| Efecto | Bitmask | Descripción |
|--------|---------|-------------|
| Scanlines | `EFFECT_SCANLINES` (1) | Líneas horizontales periódicas + vignette + barrel distortion sutil |
| Bloom | `EFFECT_BLOOM` (2) | Threshold + gauss H/V 2-pass + tint configurable |
| Chroma | `EFFECT_CHROMA` (4) | Separación RGB radial desde el centro |
| Glitch | `EFFECT_GLITCH` (8) | Shifts horizontales aleatorios por bloque |
| Matrix BG | `EFFECT_MATRIX_BG` (16) | Lluvia de caracteres estilo Matrix |
| Hex Grid | `EFFECT_HEX_GRID` (32) | Malla hexagonal animada con pulso sutil |

## Config TOML

```toml
[effects]
enabled = true

[effects.scanlines]
intensity = 0.3
freq = 2.0

[effects.bloom]
threshold = 0.7
sigma = 4.0
tint = "#FF003C"

[effects.chroma]
strength = 0.002

[effects.matrix_bg]
enabled = false
color = "#00FF55"
density = 0.3

hex_grid = false
pane_pulse = false
cursor_trail = 0
```

## Test

- `test_postproc_uniform_size` — 80 bytes
- `test_bloom_uniform_size` — 16 bytes
- `test_effect_bitmasks_unique` — sin colisiones
- `test_effects_config_defaults` — valores por defecto
- `test_effects_config_toml_round_trip` — serialización ida y vuelta
- `test_effects_partial_toml` — parseo parcial
- `test_parse_hex_color` — 5 casos (válido, #00FF55, inválido, parcial)
- `test_effects_toggle_action` — parseo from_str
- `test_effects_toggle_default_binding` — Ctrl+Shift+E registrado
- `test_cursor_trail_push/no_duplicate/max_zero` — trail de cursor

## Checklist

- [x] `cargo build -p SYNAPSE_-app` limpio
- [x] `cargo test --workspace` — 190 pasan
- [x] `cargo clippy --workspace --all-targets -- -D warnings` limpio
- [x] `cargo fmt --all -- --check` limpio
