# Fase 10 — Calidad y Conformidad

## T-046 — Test suite VT100/xterm

### Tests añadidos (30+ nuevos en `parser.rs`)

**C0 Controls:**
- `test_c0_cr` — Carriage return
- `test_c0_bs` — Backspace  
- `test_c0_tab` / `test_c0_tab_multiple` — Tabulación (cada 8 columnas)
- `test_c0_ff_form_feed` — Form feed limpia pantalla y reposiciona cursor

**CSI Cursor Movement:**
- `test_cuu_cursor_up` — CUU con parámetro
- `test_cud_cursor_down` — CUD con parámetro
- `test_cuf_cursor_forward` — CUF con parámetro
- `test_cub_cursor_back` — CUB con parámetro
- `test_cup_no_args` — CUP sin args va a (1,1)

**CSI Erase:**
- `test_ed_0_cursor_to_end` — ED=0 borra de cursor a fin de pantalla
- `test_ed_1_start_to_cursor` — ED=1 borra de inicio a cursor
- `test_ed_2_entire_display` — ED=2 borra toda la pantalla
- `test_el_0_cursor_to_end` — EL=0 borra de cursor a fin de línea
- `test_el_1_start_to_cursor` — EL=1 borra de inicio a cursor
- `test_el_2_entire_line` — EL=2 borra línea completa

**SGR Attributes:**
- `test_sgr_reset` — SGR 0 resetea todos los atributos
- `test_sgr_bright_colors` / `test_sgr_bright_bg` — Colores bright (90-97, 100-107)
- `test_sgr_underline` / `test_sgr_italic` / `test_sgr_blink` / `test_sgr_inverse`
- `test_sgr_256_color` / `test_sgr_256_bg` — xterm-256
- `test_sgr_attribute_off` — Desactivar atributos (22, 23, 24)

**Otros:**
- `test_ris_reset` — ESC c resetea estado y pantalla
- `test_csi_save_restore` — CSI s/u
- `test_auto_wrap` — Auto-wrap al llegar al borde derecho
- `test_line_feed_scroll` — Scroll al hacer LF en última fila
- `test_osc_set_title` / `test_osc_set_title_osc2` / `test_osc_set_cwd`
- `test_empty_csi` — CSI sin params
- `test_multi_byte_utf8` — Caracteres multibyte (ñ)
- `test_cursor_up/down/fwd_clamped` — Movimiento clampado a bordes

### Bug fix: `new_line()` scroll

```rust
// Antes (bug):
if self.cursor_row >= self.rows {
    self.scroll_up(1);  // ─ solo ajusta viewport offset, NO mueve datos
}

// Ahora (correcto):
if self.cursor_row >= self.rows {
    self.shift_up(1);   // ─ desplaza datos al scrollback y limpia última fila
}
```

`scroll_up()` solo cambia `scroll_offset` (para navegación del usuario por el scrollback). `shift_up()` es la función que realmente mueve datos de las celdas al buffer de scrollback.

### Resultado

- Tests pasando: 67 (eran 29 antes de Fase 10)
- Parser: de 8 tests → 39 tests

---

## T-047 — Benchmarks

Archivo: `BENCHMARKS.md`

Documenta metodología para medir:
- FPS (idle y output masivo)
- Latencia input→render
- RAM (idle y con scrollback lleno)
- Tiempo de arranque

Tabla comparativa con Alacritty, Kitty y WezTerm (valores pendientes de medición real).

---

## T-048 — Compatibilidad por OS

Archivo: `COMPATIBILITY.md`

- Tabla por distro Linux (Ubuntu, Fedora, Arch, Debian)
- Dependencias de sistema documentadas
- macOS 12+ y Windows 10+
- Tabla de conformidad VT/xterm: 11 features ✅, 5 pendientes
- Shell detection por OS

---

## T-049 — UX y Diseño Visual

- Paleta de colores completa documentada
- Contraste texto principal (#ffffff sobre #210b4b = ratio ~15:1, supera WCAG AAA)
- Colores de UI validados contra la paleta identidad
- Cursor animado pendiente de implementación (shader existe en `cursor.wgsl`)

---

## T-050 — Documentación Final

| Archivo               | Contenido                                                |
|-----------------------|----------------------------------------------------------|
| `README.md`           | Instalación, stack, features, comandos, docs             |
| `CONFIGURATION.md`    | Opciones TOML, keybinds personalizados, ejemplo completo |
| `KEYBINDS.md`         | Tabla de 30 atajos, nombres de teclas, personalización   |
| `COMPATIBILITY.md`    | OS soportados, conformidad VT, shells                    |
| `BENCHMARKS.md`       | Metodología de medición, métricas objetivo                |
| `CONTRIBUTING.md`     | Setup, estructura, convenciones, flujo de PR             |
| `CHANGELOG.md`        | Historial por fase, formato keepachangelog               |
| `LICENSE`             | MIT                                                      |

---

## Resumen final del proyecto

| Fase | Estado | Tests |
|------|--------|-------|
| 0 — Scaffolding      | ✅ | — |
| 1 — Ventana + wgpu   | ✅ | — |
| 2 — PTY + VT Parser  | ✅ | 21 |
| 3 — Rendering        | ✅ | — |
| 4 — Input            | ✅ | — |
| 5 — Tabs             | ✅ | 6 |
| 6 — Splits           | ✅ | — |
| 7 — Búsqueda         | ✅ | — |
| 8 — Configuración    | ✅ | — |
| 9 — Distribución     | ✅ | — |
| 10 — Calidad         | ✅ | 67 |

**50 tareas completadas, 67 tests unitarios pasando.**
