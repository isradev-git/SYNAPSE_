# Fase 12 — Sprint 1: Kitty Keyboard, Benchmarking, CI

Sprint que cubre R-022 (Kitty keyboard), R-008 (auto-copy), R-020 (benchmarks), y CI Windows.

---

## R-022 — Kitty Keyboard Protocol (Completado)

### Problema

Neovim moderno espera el protocolo de teclado de Kitty para diferenciar `Ctrl+[` de `Escape`, `Ctrl+I` de `Tab`, `Ctrl+M` de `Enter`. Sin esto, neovim no puede distinguir keys que producen los mismos bytes de control.

### Archivos tocados

| Archivo | Cambio |
|---------|--------|
| `crates/Luna-terminal/src/kitty.rs` | **Nuevo** — Struct `KittyKeyboard`, flags, push/pop, `encode_key_event()`, `encode_modifiers()`, constantes funcionales, 10 tests |
| `crates/Luna-terminal/src/lib.rs` | `pub mod kitty` |
| `crates/Luna-terminal/src/parser.rs` | Pre-scanner de CSI `?`/`=`/`>`/`<`u, `TerminalModes.kitty`, `pending_kitty_responses`, `drain_kitty_responses()`, 6 tests |
| `crates/Luna-app/src/input.rs` | `from_key_kitty()`, `encode_kitty_all()`, `encode_kitty_disambiguate()`, `named_to_kitty_keycode()` |
| `crates/Luna-app/src/keyboard.rs` | Key releases + repeats encoding cuando Kitty activo |
| `crates/Luna-app/src/render.rs` | Drenado de `pending_kitty_responses` al PTY |

### Decisiones de diseño

- **Pre-scan approach**: Las secuencias CSI Kitty se interceptan escaneando bytes antes del parser `vte`, porque `vte` no expone bytes marcadores privados (`>`, `=`, `<`) en `csi_dispatch`.
- **Parseo de parámetros**: `CSI = 1;2;4 u` → OR de todos los params como bitmask → flags=7. Si hay exactamente 2 params y el segundo es 1-3, se trata como mode.
- **Respuestas**: `VteProcessor.pending_kitty_responses: Vec<Vec<u8>>` → el render loop las drena y escribe al PTY. No necesita threading.
- **Flags por pane**: `kitty` vive en `TerminalModes` que es per-pane (via `Rc<RefCell<>>`).
- **Modos de encoding**:
  - Disambiguate (flag 1): `Ctrl+letter` → `CSI codepoint;5 u`. Enter/Tab/Backspace/Escape sin Ctrl → bytes legacy (especificado en spec).
  - Report events (flag 2): Key releases (`:3`) y repeats (`:2`) añadidos como event type.
  - Report all (flag 8): Todas las teclas como CSI escapes.
- **Modificadores**: `1 + bitmask` donde shift=1, alt=2, ctrl=4, super=8 (exactamente como especifica Kitty).

### Tests

```
kitty.rs:        test_kitty_keyboard_default, test_push_pop, test_set_flags,
                 test_pop_empty_stack, test_encode_modifiers, test_encode_key_event_press_no_mods,
                 test_encode_key_event_press_with_ctrl, test_encode_key_event_with_event_type,
                 test_encode_key_event_mods_and_event, test_reset
parser.rs:       test_kitty_query_flags_response, test_kitty_set_disambiguate,
                 test_kitty_set_multiple_flags, test_kitty_set_with_mode,
                 test_kitty_push_pop, test_kitty_normal_csi_u_still_works
```

### Referencia

https://sw.kovidgoyal.net/kitty/keyboard-protocol/

---

## R-008 — Auto-copy en doble/triple click (Completado)

### Problema

Doble click para seleccionar palabra y triple click para seleccionar línea ya funcionaban, pero no copiaban automáticamente al clipboard (comportamiento estándar en terminales Unix).

### Cambios

- `keyboard.rs:36`: `extract_selection()` → `pub(crate)` (era privada al módulo)
- `mouse.rs:394-419`: `App::handle_mouse_button()` — después de detectar `click_count >= 2`, extrae el texto seleccionado y lo copia al clipboard via `arboard`

### Tests

No se añadieron tests específicos (requiere GUI/clipboard mock). Pasa todos los tests existentes.

---

## CI — Windows runner (Completado)

### Problema

El CI solo corría en `ubuntu-22.04` y `macos-14`. Windows estaba diferido (R-017 scope).

### Cambio

`.github/workflows/ci.yml:20`: `[ubuntu-22.04, macos-14, windows-2022]`

Las deps de sistema Linux ya estaban gated con `if: runner.os == 'Linux'`. Windows GitHub runner tiene MSVC toolchain por defecto, no necesita deps extra.

---

## R-020 — Benchmarks y FPS logging (Completado en entorno WSL)

### Problema

`BENCHMARKS.md` tenía tabla vacía y metodología sin datos. Sin contador de frames en el binario.

### Cambios

| Archivo | Cambio |
|---------|--------|
| `app.rs:27-28` | Nuevos campos `frame_count`, `fps_last_print` en `App` |
| `render.rs:623-631` | FPS counter que imprime cada segundo via `tracing::info!(target: "luna::bench")` |
| `build/bench.sh` | **Nuevo** — script automatizado: startup time, RAM idle, RAM scrollback, FPS |
| `BENCHMARKS.md` | Tabla actualizada con binario 11MB, instrucciones detalladas, comparativa con competidores |

### Cómo medir FPS

```sh
RUST_LOG=luna::bench=info ./target/release/luna
# → "FPS: 60.0" cada segundo en terminal
```

### Cómo usar el script

```sh
./build/bench.sh release
```

### Pendiente (requiere GPU + display)

- Latencia input→render (typometer)
- FPS idle / output masivo
- RAM idle / scrollback
- Tiempo de arranque real

---

## Correcciones de CI

- `cargo fmt --all --all` ejecutado para alinear todo con `rustfmt`. Commit: `3620398`.

---

## Resumen de commits

| Hash | Fecha | Mensaje |
|------|-------|---------|
| `d23067b` | 2026-05-11 | feat: implement R-022 Kitty keyboard protocol |
| `3620398` | 2026-05-11 | style: cargo fmt all crates to fix CI |
| `d3cba0d` | 2026-05-11 | ci: add windows-2022 to test matrix |
| `dda81a1` | 2026-05-11 | perf: add FPS counter, bench script, update BENCHMARKS.md |

---

## Estado del ROADMAP

| Item | Estado |
|------|--------|
| R-022 (Kitty keyboard) | ✅ |
| R-008 (auto-copy doble/triple click) | ✅ |
| R-020 (benchmarks) | ✅ (~60% — requiere HW con GPU) |
| CI Windows | ✅ |
| R-011 (iconos app) | ⬜ Pendiente |
| R-024 (screenshots) | ⬜ Pendiente |
| R-018 (firma binarios) | ⬜ Pendiente (requiere certs) |
| R-019 (tests OS reales) | ⬜ Pendiente (requiere HW) |

**Tests totales**: 154, todos pasan. Clippy limpio.
