# Luna — Roadmap a Producción

Checklist ordenado por prioridad. Cada ítem es atómico y verificable antes de pasar al siguiente.
Estado: `[ ]` Pendiente · `[x]` Completado · `[~]` En progreso

---

## BLOQUE 1 — Terminal Funcional (Crítico)

### R-001 · Cursor animado ✓
**Problema:** `cursor.wgsl` existe pero nunca se usa. Cursor actual = celda overlay estática sin parpadeo.

- [x] Implementar parpadeo a 500ms (toggle `cursor_blink_on` en `App`)
- [x] Solo renderizar cursor para pane activo y cuando no scrolled out
- [x] Color: `#ff3d94` cursor bg, dark fg
- [x] Soportar 3 estilos: `block` (relleno), `beam` (1.5px vertical UIRect), `underline` (2px horizontal UIRect)
- [x] Configurable via `cursor_style` en `config.toml` — block|beam|underline
- [x] `cursor_blink` y `cursor_blink_ms` leídos de config en vez de constante hardcodeada
- [ ] Activar shader `cursor.wgsl` en el pipeline de render (requiere refactor renderer — diferido)

**Verificar:** cursor parpadea, posición correcta tras scroll y split. `cursor_style = "beam"` muestra cursor vertical.

---

### R-002 · Dirty tracking — fix rendimiento ✓
**Problema:** `clear_dirty()` nunca se llama. Todas las celdas se re-suben al GPU cada frame.

- [x] Llamar `grid.clear_dirty()` al final de cada frame de render (en `render.rs`)
- [ ] En `render.rs`: solo construir instancias para celdas con `dirty == true` — requiere buffer
      persistente en renderer + merge parcial. Diferido a R-014 (renderer refactor).
- [ ] Verificar que resize marca todo dirty

**Verificar:** `cargo flamegraph` o htop muestra CPU reducida en idle.

---

### R-003 · PTY EOF — proceso muerto notifica al UI ✓
**Problema:** cuando el shell hace `exit` o crashea, la tab queda zombie indefinidamente.

- [x] En el reader thread de `pty.rs`: detectar `Ok(0)` (EOF) y enviar `None` por el canal
- [x] Canal tipado como `Option<Vec<u8>>` — `None` = EOF
- [x] En `render.rs`: al recibir `None`, llamar `handle_pane_exit`
- [x] Si era el último pane de la tab: cerrar la tab
- [x] Si era la última tab: spawn nuevo shell en vez de crash

**Verificar:** escribir `exit` en el shell → tab se cierra sola.

---

### R-004 · Mouse reporting (X10 / SGR / button motion) ✓
**Problema:** vim, htop, fzf, tmux usan mouse reporting. Sin esto, el ratón no funciona en apps TUI.

- [x] `MouseReportMode` enum en `luna_terminal` (None, X10, ButtonMotion, AnyMotion)
- [x] `TerminalModes` struct per-pane con `mouse_report`, `mouse_sgr`, `focus_events`, `bracketed_paste`
- [x] `parser.rs`: CSI `?1000/1002/1003/1006h/l` activan/desactivan modos
- [x] `mouse.rs`: intercept clicks cuando `mode != None && !shift_held`, encode X10 o SGR, write PTY
- [x] `mouse.rs`: scroll reporting (btn 64/65) cuando mouse reporting activo
- [x] Coordenadas 1-indexed relativas al pane activo

**Verificar:** `vim` responde a clicks. `htop` puede hacer click en procesos.

---

### R-005 · Bracketed paste mode — activar al inicio ✓
**Problema:** el wrap `\e[200~`/`\e[201~` está implementado pero `\e[?2004h` nunca se envía. Zsh y fish lo esperan.

- [x] `TerminalModes.bracketed_paste` defaults `true` en cada pane nuevo
- [x] `parser.rs`: `?2004h/l` toggle `modes.bracketed_paste`
- [x] `keyboard.rs`: ambos handlers de paste leen `pane.modes.borrow().bracketed_paste` antes de wrap

**Verificar:** pegar texto multilinea en zsh/fish sin que se ejecute cada línea.

---

## BLOQUE 2 — UX Básica (Importante)

### R-006 · Focus events ✓
**Problema:** vim, neovim y otros editores usan `FocusIn`/`FocusOut` para cambiar comportamiento del cursor y estado.

- [x] `focus_events` en `TerminalModes` struct (shared via Rc<RefCell<>>)
- [x] En `parser.rs`: `?1004h/l` activa/desactiva `modes.focus_events`
- [x] En `app.rs`: `WindowEvent::Focused(bool)` → `handle_focus()` → envía `\e[I`/`\e[O` al PTY activo
- [x] Solo envía si el pane tiene `focus_events == true` (no contaminación si app no lo pidió)

**Verificar:** neovim cambia el estilo del cursor al enfocar/desenfocar ventana.

---

### R-007 · Cerrar tab con botón × ✓
**Problema:** click en `×` no implementado. T-027 pendiente. Solo se puede cerrar con `Ctrl+W`.

- [x] `build_tab_bar_text` renderiza `×` en los últimos 14px de cada tab (color semitransparente)
- [x] `handle_tab_click` refactorizado para tomar `&Layout` en vez de `window_width: f64`
- [x] Click en zona `close_start..tab_end` → cierra esa tab específica (no la activa)
- [x] Mata PTY de todos los panes de la tab cerrada (`panes.retain`)
- [x] Solo funciona con >1 tab (no se puede cerrar la última)
- [x] Botón `+` ahora usa `layout.tab_x` y `layout.window_height` para pane dims correctos

**Verificar:** click en `×` cierra tab correcta, no la activa.

---

### R-008 · Selección doble y triple click ✓
**Problema:** T-023 pendiente. Sin selección por palabra o línea completa.

- [x] `AppState` tiene `last_click_time: Instant` y `click_count: u8`
- [x] `handle_mouse_button`: detecta doble/triple click (< 400ms entre presses)
- [x] Doble click: expande desde posición clickeada izq/der hasta separador (whitespace + símbolos)
- [x] Triple click: selecciona col 0 → cols-1 de la fila
- [x] Click simple limpia selección previa
- [ ] Auto-copiar al clipboard en selección por doble/triple click (Unix behavior — pendiente)

**Verificar:** doble click selecciona palabra, triple click selecciona línea.

---

### R-009 · Título de tab: truncado y CWD ✓
**Problema:** títulos largos desbordan. Sin fallback a CWD cuando no hay OSC.

- [x] `Tab` struct tiene campo `cwd: String` (sincroniado en `render_frame` junto con `title`)
- [x] Prioridad: OSC title → `Path::file_name(tab.cwd)` → `"Tab N"`
- [x] Truncación con `…` aplicada uniforme a todas las fuentes de título
- [x] Tab default title vacío (antes era "Luna") para activar el fallback correctamente

**Verificar:** tab con título largo muestra `…`. Nueva tab muestra nombre del directorio actual.

---

### R-010 · Config struct completa ✓
**Problema:** `Config` tiene 4 campos. `proyecto.md` especifica secciones `[font]`, `[shell]`, `[window]`, `[cursor]`, `[theme]` inexistentes.

- [x] Config expandido con: `font_family`, `font_ligatures`, `shell_program`, `shell_args`, `cursor_style`, `cursor_blink`, `cursor_blink_ms`
- [x] Todos los campos con `#[serde(default)]` — retrocompatible con configs anteriores
- [x] `CursorStyle` enum (Block/Beam/Underline) exportado de `luna_config`
- [x] `shell_program` / `shell_args` usados en `create_pane_full` → NewTab, SplitVertical, SplitHorizontal
- [x] `cursor_blink` y `cursor_blink_ms` leídos en `render()` — reemplaza constante hardcodeada
- [x] `cursor_style` leído en render loop para decidir block/beam/underline
- [ ] `window_opacity` (futura transparencia) — diferido hasta soporte wgpu surface alpha

**Verificar:** `config.toml` con `shell_program = "/usr/bin/fish"` lanza fish al abrir nueva tab.

---

### R-011 · Iconos de aplicación
**Problema:** `assets/icons/` no existe. Sin iconos, el empaquetado produce app sin icono en macOS/Windows.

- [ ] Crear directorio `assets/icons/`
- [ ] Diseñar icono Luna: luna creciente morada sobre fondo `#210b4b`, estilo minimalista
- [ ] Exportar en formatos requeridos:
  - `Luna.png` — 512×512, fondo transparente (Linux)
  - `Luna.ico` — multi-tamaño (16, 32, 48, 256px) (Windows)
  - `Luna.icns` — multi-tamaño (16→1024px) (macOS)
- [ ] Referenciar en `build/build-mac.sh` (`CFBundleIconFile` en `Info.plist`)
- [ ] Referenciar en `wix/main.wxs` (propiedad `Icon` del instalador)
- [ ] En `app.rs`: `window.set_window_icon(...)` para icono en barra de tareas

**Verificar:** app tiene icono en Dock (macOS), taskbar (Windows), file manager (Linux).

---

### R-012 · Tab hover effect ✓
**Problema:** T-026: color `#ff3d9422` en hover especificado pero nunca aplicado.

- [x] `AppState.hover_tab: Option<usize>` — índice de tab bajo cursor
- [x] `handle_cursor_moved`: calcula tab hovered via `layout.tab_width` cuando `cursor_y < TAB_BAR_HEIGHT`
- [x] `build_tab_bar_ui_rects`: overlay `TAB_HOVER_BG` (#ff3d9422) en tab con hover (solo tabs inactivas)
- [x] `hover_tab = None` cuando cursor sale de tab bar

**Verificar:** mover el ratón sobre tabs cambia el color de fondo sutilmente.

---

### R-013 · Scroll horizontal de tab bar ✓
**Problema:** con muchas tabs el contenido desborda sin scroll.

- [x] En `Layout`: `tab_visible_range(tab_count, offset)` → `(start, end, show_left, show_right)`
- [x] En `Layout`: `scrolled_tab_width(vis_count, show_left, show_right)` y `pub const SCROLL_BTN_W = 20.0`
- [x] En `AppState`: `tab_scroll_offset: usize` (primer tab visible)
- [x] En `render.rs`: `build_tab_bar_ui_rects` y `build_tab_bar_text` solo renderizan tabs `[start, end)`
- [x] En `render.rs`: botones `<` y `>` (20px cada uno) aparecen solo cuando hay overflow
- [x] En `pane_ops.rs`: `handle_tab_click` detecta clicks en `<`/`>` y ajusta `scroll_offset`
- [x] En `mouse.rs`: hover detection ajustado para tabs con offset
- [x] NextTab / PrevTab / TabSwitch1-9 / NewTab / CloseTab: llaman `ensure_tab_visible` para auto-scroll

**Verificar:** con 15 tabs, se pueden navegar todas con los botones `<`/`>` o con Ctrl+Tab.

---

## BLOQUE 3 — Rendimiento y Corrección

### R-014 · Atlas de glifos: LRU eviction ✓
**Problema:** atlas 2048×2048 fijo. Si se llena, `get_or_insert` falla silenciosamente.

- [x] `AtlasEntry { uv: UvRect, last_frame: u64 }` — LRU timestamp per glyph
- [x] `TextureAtlas.frame: u64` — counter incremented in `begin_frame()`
- [x] `get_or_insert` actualiza `entry.last_frame = self.frame` en cache hit
- [x] `allocate` sets `needs_reset = true` cuando `y_offset + height > ATLAS_SIZE`
- [x] `begin_frame()` llamado al inicio de `draw_frame()`: si `needs_reset`, limpia cache + reset allocator; logs evicted count
- [x] `tracing::warn!` cuando `fill_fraction() >= 0.9` — una vez por ciclo de llenado
- [x] Reset deferred al frame siguiente (no mid-frame) — evita corrupción de instancias ya construidas

**Verificar:** ejecutar `cat` de un archivo con muchos caracteres Unicode distintos sin panic ni corrupción visual.

---

### R-015 · Viewport culling en render ✓
**Problema:** celdas fuera del viewport visible se envían al GPU. Con scrollback activo, hay trabajo innecesario.

- [x] `Grid::visible_cells_bounded(max_rows, max_cols)` — pre-alloca Vec con capacidad exacta, itera solo filas/columnas dentro del viewport del pane
- [x] `Grid::visible_cells()` delega a `visible_cells_bounded(rows, cols)` — sin cambio de API externa
- [x] En `render.rs`: sustituida llamada a `visible_cells()` por `visible_cells_bounded(pane_rows, pane_cols)`
- [x] Eliminado el check `col >= pane_cols || vrow >= pane_rows` (ya innecesario)
- [x] 3 tests nuevos en `grid.rs`: `bounded_no_scroll`, `bounded_with_scrollback`, `bounded_count`

**Verificar:** con 100k líneas de scrollback, FPS no degrada al hacer scroll.

---

### R-016 · vttest — validación de conformidad VT ✓
**Problema:** T-046: tests unitarios escritos pero vttest interactivo nunca ejecutado dentro de Luna.

- [x] DECSTBM (`CSI r`) — scroll region; cursor homes on set; only region scrolls
- [x] SU/SD (`CSI S/T`) — scroll up/down within region
- [x] IL/DL (`CSI L/M`) — insert/delete lines within region
- [x] ICH/DCH/ECH (`CSI @/P/X`) — insert/delete/erase characters on line
- [x] CHA/VPA/CNL/CPL (`CSI G/d/E/F`) — missing cursor motion sequences
- [x] DEC Special Graphics (`ESC(0`/`ESC(B`, SO/SI) — line drawing chars (┌┐└┘┼─│ etc.)
- [x] DECCKM (`?1h/l`) — application cursor mode; arrows send `\eOA`–`D`
- [x] Scroll region respects boundaries: rows outside region unaffected by IL/DL/shifts
- [x] RIS (`ESC c`) resets scroll region, DEC graphics, all modes
- [x] 10 new tests in `parser.rs`, `COMPATIBILITY.md` updated

**Verificar:** vttest tests 1, 2, 3, 11 pasan sin fallos visibles.

---

## BLOQUE 4 — Distribución

### R-017 · Repositorio GitHub real + CI activo
**Problema:** workflows escritos pero sin repo real. T-045 pendiente.

- [ ] Crear repositorio en GitHub (público o privado)
- [ ] Push inicial del código
- [ ] Verificar que `ci.yml` pasa en los 3 runners (ubuntu, macos, windows)
- [ ] Crear primer tag `v0.1.0` y verificar que `release.yml` produce binarios
- [ ] Subir binarios a GitHub Releases

**Verificar:** GitHub Actions verde en los 3 OS. Release con 5 binarios descargables.

---

### R-018 · Firma de binarios
**Problema:** sin firma, macOS muestra "desarrollador no verificado" y Windows SmartScreen bloquea.

**macOS:**
- [ ] Obtener Apple Developer Certificate (99$/año)
- [ ] En `build/build-mac.sh`: añadir `codesign --deep --sign "Developer ID Application: ..."` tras crear `.app`
- [ ] Añadir paso de notarización: `xcrun notarytool submit` + `xcrun stapler staple`
- [ ] Documentar en `CONTRIBUTING.md` requisitos para builds firmados

**Windows:**
- [ ] Obtener Code Signing Certificate EV (DigiCert / Sectigo, ~300$/año)
- [ ] En `build/build-win.ps1`: añadir `signtool sign` tras compilar
- [ ] Verificar que SmartScreen no bloquea el instalador

**Linux:**
- [ ] Crear clave GPG para el proyecto
- [ ] En `release.yml`: firmar `.deb`, `.rpm`, AppImage con GPG
- [ ] Publicar clave pública en keyserver

**Verificar:** instalar en macOS real sin warnings. Instalar en Windows real sin SmartScreen.

---

### R-019 · Tests en OS reales
**Problema:** T-042/043/044: sin validación en hardware real.

- [ ] macOS (Apple Silicon y/o Intel): Metal renderer, fuentes del sistema, PTY, resize
- [ ] Windows 10/11: DX12 renderer, cmd.exe y PowerShell, instalador WiX, PATH
- [ ] Ubuntu 22.04 LTS: X11 y Wayland, paquetes .deb y AppImage
- [ ] Para cada fallo: fix + regresión test

**Verificar:** app arranca, muestra terminal funcional, splits/tabs funcionan, en los 3 OS.

---

### R-020 · Benchmarks reales
**Problema:** T-047: `BENCHMARKS.md` tiene metodología pero tabla completamente vacía.

- [ ] Compilar `--release` en máquina de referencia (documentar specs)
- [ ] Medir latencia input→render con `typometer` o similar
- [ ] Medir FPS con output masivo: `cat /dev/urandom | head -c 10MB | strings`
- [ ] Medir RAM idle con `ps aux` o Activity Monitor
- [ ] Medir tiempo de arranque: `time luna &`
- [ ] Rellenar tabla comparativa vs Alacritty en misma máquina
- [ ] Documentar resultados en `BENCHMARKS.md`

**Verificar:** métricas dentro de targets: <5ms latencia, 60fps idle, <50MB RAM, <200ms arranque.

---

## BLOQUE 5 — Polish y Diferenciación

### R-021 · Ligaduras de fuente
**Problema:** `proyecto.md` especifica `ligatures = true`. JetBrains Mono tiene ligaduras (`->`, `=>`, `!=`, etc.). No implementado.

- [ ] En `config.rs`: añadir `font_ligatures: bool` (default `false`) — ya cubierto en R-010
- [ ] En `text.rs` / shaping: configurar `cosmic-text` para activar/desactivar ligaduras según config
- [ ] Verificar que `->` renderiza como ligadura cuando está activo

**Verificar:** con `font_ligatures = true`, `=>` aparece como símbolo único en el código.

---

### R-022 · Kitty keyboard protocol
**Problema:** neovim moderno espera este protocolo para diferenciar `Ctrl+[` de `Escape`, entre otros.

- [ ] En `parser.rs`: manejar `\e[?u` query y `\e[=<flags>u` activación
- [ ] En `keyboard.rs`: cuando el protocolo esté activo, usar encoding extendido:
  - Enviar key releases además de presses
  - Diferenciar `Ctrl+I` de `Tab`, `Ctrl+M` de `Enter`
  - Enviar modificadores en campos separados
- [ ] Referencia: https://sw.kovidgoyal.net/kitty/keyboard-protocol/

**Verificar:** neovim detecta el protocolo y reporta soporte. `Ctrl+[` y `Escape` distinguibles.

---

### R-023 · Sistema de temas
**Problema:** paleta hardcodeada en `theme.rs`. Sin carga de temas externos.

- [ ] En `config.rs`: añadir `theme: String` (default `"Luna"`)
- [ ] En `Luna-config`: añadir módulo `themes.rs` con struct `Theme { bg, fg, cursor, selection, tab_active, ... }`
- [ ] Leer tema desde `~/.config/Luna/themes/<name>.toml` si existe
- [ ] Fallback al tema Luna hardcodeado si no se encuentra
- [ ] Pasar `Theme` al renderer en vez de constantes de `theme.rs`
- [ ] Incluir 2-3 temas extra: `Dracula`, `Catppuccin-Mocha`, `Tokyo-Night`

**Verificar:** `theme = "Dracula"` en config cambia toda la paleta de colores.

---

### R-024 · Screenshots para README y distribución
**Problema:** T-049: README sin imágenes. Producto no se ve profesional en GitHub.

- [ ] Capturar screenshots HD (2x) en macOS:
  - Terminal en uso normal con colores ANSI
  - Split 2×2 con diferentes procesos
  - Búsqueda activa con matches resaltados
  - Tab bar con múltiples tabs
- [ ] Añadir a `assets/screenshots/`
- [ ] Actualizar `README.md` con imagen hero y galería
- [ ] Crear GIF/video corto mostrando splits + tabs en acción

**Verificar:** README en GitHub muestra imágenes. Primera impresión visual de calidad.

---

### R-025 · Ctrl+, abre editor de config
**Problema:** `proyecto.md` dice "abrir config en editor". Código: recarga silenciosa. Discrepancia.

- [ ] En `keyboard.rs` / acción `ReloadConfig`: además de recargar, abrir el archivo en editor:
  - Detectar `$EDITOR` o `$VISUAL`
  - Fallback: `xdg-open` (Linux), `open` (macOS), `notepad` (Windows)
  - Enviar el comando como input al PTY activo: `$EDITOR ~/.config/Luna/config.toml\r`
- [ ] O bien: mantener solo recarga silenciosa y documentar correctamente (decisión de diseño)

**Verificar:** `Ctrl+,` abre el config en `$EDITOR` dentro de la terminal.

---

## Resumen de bloques

| Bloque | Descripción | Ítems | Para qué sirve |
|--------|-------------|-------|----------------|
| 1 | Terminal funcional | R-001 a R-005 | Usable en el día a día |
| 2 | UX básica | R-006 a R-013 | A nivel de competidores |
| 3 | Rendimiento y corrección | R-014 a R-016 | Uso prolongado sin degradación |
| 4 | Distribución | R-017 a R-020 | Usuarios reales pueden instalarlo |
| 5 | Polish y diferenciación | R-021 a R-025 | Producto de calidad comercial |

**Total: 25 ítems, ~80 sub-tareas atómicas.**

Orden de ejecución recomendado: R-001 → R-002 → R-003 → R-004 → R-005 → R-006 → R-007 → R-008 → R-009 → R-010 → R-011 → R-012 → R-013 → R-014 → R-015 → R-016 → R-017 → R-018 → R-019 → R-020 → R-021 → R-022 → R-023 → R-024 → R-025
