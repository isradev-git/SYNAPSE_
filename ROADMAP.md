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
- [ ] Soportar 3 estilos: `block` (relleno), `beam` (1px vertical), `underline` (1px horizontal)
- [ ] Activar shader `cursor.wgsl` en el pipeline de render (pendiente refactor renderer)

**Verificar:** cursor parpadea, posición correcta tras scroll y split.

---

### R-002 · Dirty tracking — fix rendimiento ✓
**Problema:** `clear_dirty()` nunca se llama. Todas las celdas se re-suben al GPU cada frame.

- [x] Llamar `grid.clear_dirty()` al final de cada frame de render (en `render.rs`)
- [ ] En `render.rs`: solo construir instancias para celdas con `dirty == true` (optimización pendiente)
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

### R-006 · Focus events
**Problema:** vim, neovim y otros editores usan `FocusIn`/`FocusOut` para cambiar comportamiento del cursor y estado.

- [ ] En `AppState`: añadir `focus_events_enabled: bool`
- [ ] En `parser.rs`: manejar `\e[?1004h/l` (activar/desactivar focus events)
- [ ] En `app.rs`: manejar `WindowEvent::Focused(true/false)` de winit
- [ ] Al recibir foco: enviar `\e[I` al PTY activo (si `focus_events_enabled`)
- [ ] Al perder foco: enviar `\e[O` al PTY activo

**Verificar:** neovim cambia el estilo del cursor al enfocar/desenfocar ventana.

---

### R-007 · Cerrar tab con botón ×
**Problema:** click en `×` no implementado. T-027 pendiente. Solo se puede cerrar con `Ctrl+W`.

- [ ] En `render.rs` / `build_tab_bar_ui_rects`: renderizar botón `×` en cada tab (últimos 16px del ancho)
- [ ] Renderizar texto `×` sobre el rect del botón
- [ ] En `mouse.rs` / `handle_tab_click`: detectar click en zona `×` de cada tab
- [ ] Al detectar: llamar lógica de cierre equivalente a `Ctrl+W` para esa tab específica (no solo la activa)
- [ ] Matar PTY de todos los panes de la tab cerrada

**Verificar:** click en `×` cierra tab correcta, no la activa.

---

### R-008 · Selección doble y triple click
**Problema:** T-023 pendiente. Sin selección por palabra o línea completa.

- [ ] En `AppState`: añadir `last_click_time: Instant` y `click_count: u8`
- [ ] En `mouse.rs` / `handle_mouse_button`: detectar doble/triple click (< 400ms entre clicks)
- [ ] Doble click: calcular bounds de palabra en la celda clickeada (split en whitespace/símbolos)
  - Expandir izquierda y derecha desde la posición hasta encontrar separador
  - Actualizar `state.selection` con start/end de la palabra
- [ ] Triple click: seleccionar línea completa (col 0 → cols-1 de la fila clickeada)
- [ ] Auto-copiar al clipboard en selección por doble/triple click (comportamiento Unix estándar)

**Verificar:** doble click selecciona palabra, triple click selecciona línea.

---

### R-009 · Título de tab: truncado y CWD
**Problema:** títulos largos desbordan. Sin fallback a CWD cuando no hay OSC.

- [ ] En `render.rs` al renderizar texto de tab: calcular max chars según ancho de tab
- [ ] Si `title.len() > max_chars`: truncar a `max_chars - 1` y añadir `…`
- [ ] En `tab_bar.rs` / título visible: si `title.is_empty()`, usar último componente de `pane.cwd()`
  - `Path::new(cwd).file_name()` → string de directorio
  - Fallback final: `"Tab N"`
- [ ] Actualizar en cada frame o solo cuando cambia OSC/CWD

**Verificar:** tab con título largo muestra `…`. Nueva tab muestra nombre del directorio actual.

---

### R-010 · Config struct completa
**Problema:** `Config` tiene 4 campos. `proyecto.md` especifica secciones `[font]`, `[shell]`, `[window]`, `[cursor]`, `[theme]` inexistentes.

- [ ] Expandir `Config` en `config.rs`:
  ```toml
  font_size = 14.0          # ya existe
  font_family = "JetBrains Mono"
  font_ligatures = false
  window_width = 1280       # ya existe
  window_height = 800       # ya existe
  window_opacity = 1.0
  scrollback_lines = 100000 # ya existe
  shell_program = ""        # vacío = autodetectar
  shell_args = []
  cursor_style = "block"    # block | beam | underline
  cursor_blink = true
  cursor_blink_ms = 500
  ```
- [ ] Mantener compatibilidad: todos los campos con `#[serde(default)]`
- [ ] En `pane_ops.rs` / `create_pane`: usar `config.shell_program` si no está vacío (override `detect_shell()`)
- [ ] En `app.rs`: pasar `cursor_style`/`cursor_blink` a `AppState` para que R-001 los lea
- [ ] En `renderer.rs`: leer `window_opacity` para futura transparencia

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

### R-012 · Tab hover effect
**Problema:** T-026: color `#ff3d9422` en hover especificado pero nunca aplicado.

- [ ] En `AppState`: añadir `hover_tab: Option<usize>` (índice de tab bajo cursor)
- [ ] En `mouse.rs` / `handle_cursor_moved`: calcular qué tab está bajo el cursor (si el cursor está en la tab bar)
- [ ] En `render.rs` / `build_tab_bar_ui_rects`: aplicar color `#ff3d9422` como overlay en la tab con hover
- [ ] Limpiar `hover_tab` cuando el cursor sale de la tab bar

**Verificar:** mover el ratón sobre tabs cambia el color de fondo sutilmente.

---

### R-013 · Scroll horizontal de tab bar
**Problema:** con muchas tabs el contenido desborda sin scroll.

- [ ] En `Layout`: calcular si las tabs exceden el ancho de ventana
- [ ] En `AppState`: añadir `tab_scroll_offset: usize` (primer tab visible)
- [ ] En `render.rs`: solo renderizar tabs desde `tab_scroll_offset` hasta que llenen el ancho
- [ ] En `mouse.rs`: botones `<` y `>` en los extremos de la tab bar para scrollear
- [ ] Ctrl+Tab / Ctrl+Shift+Tab: auto-scroll si la tab activa queda fuera de vista

**Verificar:** con 15 tabs, se pueden navegar todas con los botones o con Ctrl+Tab.

---

## BLOQUE 3 — Rendimiento y Corrección

### R-014 · Atlas de glifos: LRU eviction
**Problema:** atlas 2048×2048 fijo. Si se llena, `get_or_insert` falla silenciosamente.

- [ ] En `atlas.rs`: trackear últimos accesos con un contador de frame por glifo
- [ ] Al necesitar insertar y no haber espacio: evict el glifo menos recientemente usado
- [ ] O bien: implementar resize dinámico del atlas (duplicar a 4096×4096 si se necesita)
- [ ] Loggear con `tracing::warn!` cuando el atlas esté al 90% de capacidad

**Verificar:** ejecutar `cat` de un archivo con muchos caracteres Unicode distintos sin panic ni corrupción visual.

---

### R-015 · Viewport culling en render
**Problema:** celdas fuera del viewport visible se envían al GPU. Con scrollback activo, hay trabajo innecesario.

- [ ] En `render.rs`: al iterar celdas, solo incluir filas visibles en pantalla
- [ ] Calcular `visible_start_row` y `visible_end_row` según `scroll_offset` y alto del pane
- [ ] No instanciar celdas fuera de ese rango

**Verificar:** con 100k líneas de scrollback, FPS no degrada al hacer scroll.

---

### R-016 · vttest — validación de conformidad VT
**Problema:** T-046: tests unitarios escritos pero vttest interactivo nunca ejecutado dentro de Luna.

- [ ] Descargar/compilar `vttest` (disponible en la mayoría de repos de Linux/macOS)
- [ ] Ejecutar `vttest` dentro de Luna, pasar todas las suites básicas:
  - Test 1: cursor movement
  - Test 2: screen features
  - Test 3: character sets
  - Test 11: VT100 special keys
- [ ] Para cada fallo: identificar la secuencia CSI/ESC correspondiente, fix en `parser.rs`
- [ ] Documentar en `COMPATIBILITY.md` qué suites pasan

**Verificar:** vttest tests 1, 2, 11 pasan sin fallos visibles.

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
