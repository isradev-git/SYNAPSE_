# Changelog

Los cambios significativos se documentan aquí siguiendo el formato [Keep a Changelog](https://keepachangelog.com).

## [0.1.0] — Sin publicar

### R-025 · Ctrl+, abre config en $EDITOR
- `Ctrl+,` ahora recarga config Y envía `$EDITOR <config_path>\r` al PTY activo
- Detecta `$EDITOR` → `$VISUAL` → fallback OS (`open` macOS, `notepad` Windows, `xdg-open` Linux)

### R-016 · vttest — Conformidad VT100/VT220
- `DECSTBM` (`CSI r`): scroll region configurable; cursor homes al activar
- `SU`/`SD` (`CSI S`/`T`): scroll hacia arriba/abajo dentro de la región
- `IL`/`DL` (`CSI L`/`M`): insertar/eliminar líneas respetando la región de scroll
- `ICH`/`DCH`/`ECH` (`CSI @`/`P`/`X`): insertar/eliminar/borrar caracteres en línea
- `CHA`/`VPA`/`CNL`/`CPL` (`CSI G`/`d`/`E`/`F`): secuencias de movimiento de cursor faltantes
- DEC Special Graphics (`ESC(0`/`ESC(B`, SO/SI): caracteres de línea (┌┐└┘┼─│ etc.)
- `DECCKM` (`?1h/l`): modo application cursor; arrows envían `\eOA`–`D`
- `RIS` (`ESC c`): reset completo de región, gráficos DEC y todos los modos
- 10 tests nuevos en `parser.rs`; `COMPATIBILITY.md` actualizado

### R-015 · Viewport culling en render
- `Grid::visible_cells_bounded(max_rows, max_cols)`: solo itera celdas dentro del viewport del pane
- Elimina procesamiento de celdas fuera de pantalla incluso con scrollback activo
- 3 tests nuevos en `grid.rs`

### R-014 · Atlas de glifos: LRU eviction
- `AtlasEntry.last_frame`: timestamp LRU por glifo
- `begin_frame()`: reset diferido al frame siguiente cuando el atlas se llena (evita corrupción)
- `tracing::warn!` cuando el atlas supera 90% de ocupación

### R-013 · Scroll horizontal de tab bar
- `Layout::tab_visible_range()`: calcula ventana visible de tabs según offset
- Botones `<` / `>` (20px) aparecen automáticamente con overflow
- `tab_scroll_offset` en `AppState`; auto-scroll al cambiar de tab con teclado

### R-012 · Tab hover effect
- `AppState.hover_tab`: índice de tab bajo cursor
- Overlay `#ff3d9422` en tabs inactivas al pasar el ratón

### R-011 · Iconos de aplicación
- Pendiente (requiere assets de diseño)

### R-010 · Config struct completa
- `Config` expandido: `font_family`, `font_ligatures`, `shell_program`, `shell_args`,
  `cursor_style`, `cursor_blink`, `cursor_blink_ms`
- `CursorStyle` enum (Block/Beam/Underline) exportado desde `luna_config`
- Todos los campos con `#[serde(default)]` — retrocompatible

### R-009 · Título de tab: CWD y truncado
- Prioridad: OSC title → `Path::file_name(cwd)` → `"Tab N"`
- Truncación con `…` aplicada a todas las fuentes de título

### R-008 · Selección doble y triple click
- Doble click: expande selección hasta separador (whitespace/símbolo)
- Triple click: selecciona línea completa
- `last_click_time` + `click_count` en `AppState` (ventana 400ms)

### R-007 · Cerrar tab con botón ×
- `×` renderizado en los últimos 14px de cada tab
- Click en zona close → cierra esa tab (no la activa); mata PTY de todos sus panes

### R-006 · Focus events
- `?1004h/l` activa/desactiva `modes.focus_events`
- `WindowEvent::Focused` → envía `\e[I`/`\e[O` al PTY activo

### R-005 · Bracketed paste mode
- `bracketed_paste` defaults `true` en panes nuevos
- Ambos handlers de paste leen el modo antes de envolver con `\e[200~`/`\e[201~`

### R-004 · Mouse reporting (X10 / SGR / button-motion)
- `MouseReportMode` enum: None, X10, ButtonMotion, AnyMotion
- `TerminalModes` struct per-pane compartida vía `Rc<RefCell<>>`
- Clicks y scroll reportados en X10 o SGR según modo; Shift bypass para selección manual

### R-003 · PTY EOF — proceso muerto notifica al UI
- Canal `mpsc` tipado como `Option<Vec<u8>>` — `None` = EOF
- `handle_pane_exit`: cierra pane/tab; si era la última tab, spawn nuevo shell

### R-002 · Dirty tracking
- `grid.clear_dirty()` llamado al final de cada frame de render

### R-001 · Cursor animado
- Parpadeo a 500ms (`cursor_blink_on` toggle en `App`)
- Solo se renderiza cursor del pane activo y cuando no está scrolled out
- Estilos: `block`, `beam` (1.5px vertical), `underline` (2px horizontal)
- `cursor_style`, `cursor_blink`, `cursor_blink_ms` leídos de config

---

### Añadido (Fase 0-5)
- Terminal GPU-accelerated con wgpu (Vulkan/Metal/DirectX 12)
- PTY nativo con shell detection (bash, zsh, fish, cmd.exe)
- Parser VT100/xterm completo (C0, CSI, SGR, OSC, ESC)
- Grid de celdas con scrollback (100.000 líneas)
- Texture atlas de glifos con cosmic-text + JetBrains Mono
- Instanced rendering con un solo draw call por frame
- Input de teclado completo (teclas especiales, modificadores, Ctrl+key)
- Scroll con ratón y teclado
- Selección de texto con ratón
- Copiar y pegar (clipboard via arboard, bracketed paste)
- Colores ANSI: 8-color, bright, 256-color, true color (24-bit)
- SGR: bold, italic, underline, blink, inverse, invisible

### Añadido (Fase 5-6)
- Sistema de tabs con barra superior
- Ctrl+T / Ctrl+W / Ctrl+Tab / Ctrl+1..9
- Títulos dinámicos de tabs (OSC 0/2)
- Sistema de splits (árbol binario de paneles)
- Ctrl+Shift+D/E para split vertical/horizontal
- Redimensionado de paneles con ratón (drag de divisores)
- Navegación entre paneles con Ctrl+Shift+↑↓←→
- PTY independiente por panel (CWD heredado)
- Shader de UI para divisores y bordes

### Añadido (Fase 7-8)
- Búsqueda en buffer (Ctrl+Shift+F) con resaltado y navegación
- Búsqueda inversa en historial (Ctrl+R)
- Limpiar pantalla (Ctrl+L)
- Sistema de configuración TOML (~/.config/Luna/config.toml)
- Keybinds personalizables (30 atajos por defecto)
- Ajuste de fuente en runtime (Ctrl+= / Ctrl+- / Ctrl+0)
- Pantalla completa (F11)
- Recarga de configuración en caliente (Ctrl+,)

### Añadido (Fase 9)
- Empaquetado de distribución (cargo-dist)
- CI/CD con GitHub Actions (release.yml + ci.yml)
- Empaquetado Linux (.deb, .rpm, AppImage)
- Empaquetado macOS (.app, .dmg)
- Empaquetado Windows (.exe, ZIP, MSI via WiX)

### Añadido (Fase 10)
- Suite de tests de conformidad VT100/xterm (30+ tests)
- Documentación completa (README, CONFIGURATION, KEYBINDS, COMPATIBILITY)
- Benchmarks iniciales
- Fix: scroll de grid corregido (new_line → shift_up)
