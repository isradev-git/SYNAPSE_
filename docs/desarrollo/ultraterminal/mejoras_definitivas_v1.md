# Mejoras Definitivas — SYNAPSE_ Ultraterminal v1

> Plan maestro para alcanzar la terminal más completa del mercado. Cada feature es atómica, trackeable con [ ], y ordenada por prioridad. Empezar por la M-001 y avanzar en orden.

**Versión:** 1.0 | **Fecha:** 2026-05-24  
**Basado en:** Competitive analysis vs. Kitty, WezTerm, Warp, Ghostty, iTerm2  
**Estado actual:** Fases 0-13 completadas + UI Phases A-G completadas | 303 tests | v0.2.0 | 28/28 mejoras completadas (M-023/M-024 eliminadas del roadmap)

---

## Metodología

- Cada mejora tiene: ID, prioridad (P0-P3), esfuerzo estimado, archivos a tocar, y checklist de subtareas.
- Las mejoras son independientes entre sí (salvo cuando se indica dependencia explícita).
- Al completar una mejora, marcar `[x]` y commitear con prefijo `feat(M-XXX):`.
- Mantener clippy clean + tests pasando + cargo fmt después de cada una.

---

## M-001: Ligaduras ON por defecto ✅

**Prioridad:** P0 | **Esfuerzo:** 5 min | **Dependencias:** ninguna | **Estado:** COMPLETADO 2026-05-22

El engine de ligaduras (rustybuzz + HarfBuzz) ya está implementado en `text.rs:306-347` y `renderer.rs:354-428`. Solo estaba deshabilitado por defecto.

- [x] Cambiar `font_ligatures: false` → `font_ligatures: true` en `crates/SYNAPSE_-config/src/config.rs:119`
- [x] Actualizar test `test_default_values` (L282: `assert!(!cfg.font_ligatures)` → `assert!(cfg.font_ligatures)`)
- [x] Verificar que `cargo test -p SYNAPSE_-config` pasa (49 tests OK)
- [x] Verificar que `cargo test --workspace` pasa (205 tests OK)
- [x] Clippy y build release clean
- [x] Pipeline completo: Config default → render.rs → renderer.rs → text.rs (calt/liga/clig)

**Archivos modificados:** `crates/SYNAPSE_-config/src/config.rs` (2 líneas cambiadas)

---

## M-002: Pane Zoom (Ctrl+Shift+Z) ✅

**Prioridad:** P0 | **Esfuerzo:** 1 día | **Dependencias:** ninguna | **Estado:** COMPLETADO 2026-05-22

Maximizar un pane a pantalla completa y restaurarlo. Similar a WezTerm/iTerm2.

- [x] Añadir `Zoom` al enum `Action` en `keybinds.rs`
- [x] Añadir default keybind `Ctrl+Shift+Z` → `"zoom_pane"` en `default_entries()`
- [x] Añadir campos `zoomed_pane: Option<PaneId>` y `zoom_saved_tree: Option<PaneTree>` en `AppState`
- [x] Función `toggle_zoom()`: guarda/restaura PaneTree del tab activo
- [x] Función `clear_zoom()`: auto-exit al cambiar de tab (keyboard + mouse click)
- [x] Render: sin cambios — PaneTree modificado in-place, el layout engine lo maneja
- [x] Test: `test_action_from_str_all_actions` actualizado con "zoom_pane"
- [x] Test: `test_default_entries_count` actualizado (29+ entries)
- [x] `PaneTree` ahora deriva `Clone` para guardar/restaurar

**Archivos:** `crates/SYNAPSE_-config/src/keybinds.rs`, `crates/SYNAPSE_-app/src/state.rs`, `crates/SYNAPSE_-app/src/keyboard.rs`, `crates/SYNAPSE_-app/src/mouse.rs`, `crates/SYNAPSE_-ui/src/splitter.rs`

---

## M-003: Broadcast Input (Ctrl+Shift+B) ✅

**Prioridad:** P0 | **Esfuerzo:** 2 horas | **Dependencias:** ninguna | **Estado:** COMPLETADO 2026-05-22

Enviar la misma entrada a todos los panes de la tab activa. Indispensable para operar múltiples servidores SSH a la vez.

- [x] Añadir `ToggleBroadcast` al enum `Action` en `keybinds.rs`
- [x] Añadir default `Ctrl+Shift+B` → `"toggle_broadcast"` en `default_entries()`
- [x] Añadir campo `broadcasting: bool` en `AppState`
- [x] En `keyboard.rs`, handler `Action::ToggleBroadcast`: flip `broadcasting` (handle_keyboard + dispatch_action)
- [x] En `pane_ops.rs`, función `write_to_panes()`: si `broadcasting`, itera todos los panes de la tab activa y escribe a cada uno
- [x] Indicador visual en status bar: `[BROADCAST]` en rojo neón (lado derecho, junto a la hora)
- [x] Broadcast aplica a: caracteres normales, paste (Ctrl+V), InputAction::Paste (mouse)
- [x] Test: build, clippy, fmt, 212 tests pasando

**Archivos:** `crates/SYNAPSE_-config/src/keybinds.rs`, `crates/SYNAPSE_-app/src/state.rs`, `crates/SYNAPSE_-app/src/keyboard.rs`, `crates/SYNAPSE_-app/src/pane_ops.rs`, `crates/SYNAPSE_-app/src/render.rs`

---

## M-004: Drag & Drop de archivos ✅

**Prioridad:** P0 | **Esfuerzo:** 2 horas | **Dependencias:** ninguna | **Estado:** COMPLETADO 2026-05-22

Arrastrar un archivo desde el explorador y que pegue su path absoluto en el pane activo.

- [x] En `app.rs` `window_event()`, añadir match para `WindowEvent::DroppedFile(path)`
- [x] Obtener path absoluto canonizado con `std::fs::canonicalize(path)`
- [x] Escribir path + espacio en el PTY via `write_to_panes()` (respeta broadcast mode)
- [x] Múltiples archivos: cada `DroppedFile` escribe su path + espacio (separación natural)
- [x] Error logging via `tracing::warn!` si canonicalize falla
- [x] Test: build, clippy, fmt, 212 tests pasando

**Archivos:** `crates/SYNAPSE_-app/src/main.rs`, `crates/SYNAPSE_-app/src/pane_ops.rs`

---

## M-005: CLI args (-e, --new-tab, --help) ✅

**Prioridad:** P0 | **Esfuerzo:** 1 día | **Dependencias:** ninguna | **Estado:** COMPLETADO 2026-05-22

`clap` está en `[workspace.dependencies]` pero sin usar. Dar soporte a argumentos de línea de comandos.

- [x] Struct `Cli` en `src/cli.rs` con derive `Parser`: `-e`, `-d`, `--new-tab`, `--hold`
- [x] `-e cmd`: spawn PTY con `$SHELL -c cmd` (override del shell default)
- [x] `--hold`: con `-e`, envuelve el comando: `cmd; echo; echo '[Process exited - press Enter to close]'; read` para mantener terminal viva
- [x] `-d path`: cambiar CWD del primer pane (canonicalized via `std::fs::canonicalize`)
- [x] `--new-tab cmd`: crear tab extra ejecutando `$SHELL -c cmd`, luego cambiar focus al primer tab
- [x] Respeta `$SHELL` env var para el comando de shell base
- [x] `--help` y `--version` automáticos via clap
- [x] Test: build, clippy, fmt, 212 tests pasando

**Archivos:** `crates/SYNAPSE_-app/src/main.rs`, `crates/SYNAPSE_-app/src/pane_ops.rs`

---

## M-006: Search con regex toggle ✅

**Prioridad:** P0 | **Esfuerzo:** 2 horas | **Dependencias:** ninguna | **Estado:** COMPLETADO 2026-05-22

El search actual (`search.rs`) usa substring matching. Añadir toggle para regex.

- [x] Añadir campo `regex_mode: bool` y `invalid_regex: bool` en `SearchState` (`state.rs`)
- [x] En `search.rs:find_matches()`, si `regex_mode`, compilar pattern con `regex::Regex::new(query)` (la crate `regex` se añadió a `Cargo.toml` del crate app)
- [x] Keybind: `Alt+R` dentro de la search bar togglea `regex_mode`
- [x] Indicador visual en la search bar: "[regex]" — con fallback "[invalid]" si el regex no compila
- [x] Fallback graceful: si el regex es inválido, mostrar "[invalid]" (en color highlight) en lugar de crashear
- [x] `toggle_regex()` en `SearchState`; `toggle()` resetea ambos flags
- [x] Build, clippy, fmt, 212 tests pasando

---

## M-007: File/Path detection clickable ✅

**Prioridad:** P1 | **Esfuerzo:** 1 día | **Dependencias:** M-006 (infra regex, se puede reusar) | **Estado:** COMPLETADO 2026-05-22

Detectar paths como `/home/user/project/src/main.rs:42` o `./lib/mod.ts:10` y hacerlos Ctrl+click. Reutiliza la infraestructura OSC 8 existente.

- [x] Función `detect_paths()` en `render.rs` que escanea celdas visibles como `detect_auto_urls()`
  - Path absoluto: `/`, `~/`, `./`, `../`
  - Caracteres válidos: alfanuméricos + `._\-/@+#`
  - Termina en whitespace / comillas / paréntesis / corchetes
- [x] Integrado en `url_span_list` junto a OSC 8 y auto URLs
- [x] Gated por `state.config.clickable_paths` (default `true`)
- [x] `open_url()` en `mouse.rs` bifurca: paths → `open_path()`, URLs → `xdg-open`/`open`
- [x] `open_path()` parsea `path:line:col` con regex
  - Intenta `code --goto path:line:col` primero (VS Code)
  - Si no, usa `$EDITOR +line path`
  - Si solo path (sin línea), usa `xdg-open` / `open`
- [x] Config entry `clickable_paths: bool` (default true) en `Config`
- [x] Build, clippy, fmt, 212 tests pasando

---

## M-008: Sixel decoder ✅

**Prioridad:** P1 | **Esfuerzo:** 3-5 días | **Dependencias:** ninguna (ImageStore pipeline ya existe) | **Estado:** COMPLETADO 2026-05-22

Sixel es el formato de imágenes más usado en terminales (gnuplot, lsix, img2sixel). La pipeline de imágenes existe en `image_protocol.rs` + `renderer/image.rs`. Solo falta el decoder.

- [x] Decoder mínimo en `crates/SYNAPSE_-app/src/sixel.rs` (sin crate externa):
  - `decode_sixel(bytes: &[u8]) -> Option<SixelResult { width, height, rgba }>`
  - Color registers (`#Pc;Pu;Px;Py;Pz`) con soporte RGB (Pu=2) y HLS (Pu=1)
  - Selectores de color (`#Pc` sin params)
  - Repeat (`!N char`) con avance de x por cada repetición
  - Raster attributes (`"pan;pad;w;h`) skip
  - New line (`$`), next line (`-`), carriage return
  - Sixel chars 0x3F-0x7E mapean 6 pixels verticales (LSB=top)
  - Bitmap RGBA con dimensiones automáticas (max_x, max_y)
  - Color por defecto: blanco (255,255,255,255)
- [x] Detección `ESC P ... q ... ESC \` en PTY reader via `process_sixel_sequences()`
  - DCS intro: `ESC P` seguido de params (dígitos/`;`) terminando en `q`
  - ST terminator: `ESC \` (0x1b 0x5c) o `0x9c`
  - Stripping de secuencias Sixel del staging antes del VTE parser (evita garbage en pantalla)
- [x] **Sixel scrolling**: si la imagen excede el viewport, `grid_mut().scroll_up()` desplaza el contenido hacia arriba. Solo cuando `display_offset == 0` (viendo live terminal, no scrollback). Número de filas = `ceil(height / cell_h)`.
- [x] Config entry `sixel_enabled: bool` (default true)
- [x] 9 tests unitarios del decoder (color, pattern, repeat, newline, carriage return, spec example)
- [x] Build, clippy, fmt, 221 tests pasando

**Archivos:** `crates/SYNAPSE_-app/src/sixel.rs` (nuevo), `crates/SYNAPSE_-app/src/image_protocol.rs`, `crates/SYNAPSE_-app/src/render.rs`, `crates/SYNAPSE_-app/src/pane_ops.rs`, `crates/SYNAPSE_-ui/src/pane.rs`, `crates/SYNAPSE_-config/src/config.rs`, `crates/SYNAPSE_-app/src/main.rs`

---

## M-009: iTerm2 OSC 1337 inline images ✅

**Prioridad:** P1 | **Esfuerzo:** 2 días | **Dependencias:** ninguna (misma pipeline que Sixel) | **Estado:** COMPLETADO 2026-05-22

Soporte para `OSC 1337 ; File=inline=1 ; size=N ; name=base64encoded : BASE64` y `File=inline=1 : BASE64`.

- [x] Detectar `ESC ] 1337 ; File=inline=1` en el PTY reader thread via `process_iterm2_sequences()`
- [x] Parsear atributos: `width`, `height`, `preserveAspectRatio`
- [x] Decodificar base64 → bytes
- [x] Detectar formato y decodificar a RGBA usando `image` crate (PNG, JPEG, GIF, BMP)
- [x] Subir a `ImageStore` via `accept_iterm2()` con ID auto-asignado
- [x] Colocar como `ImagePlacement` en cursor actual
- [x] Manejar terminadores BEL (0x07) y ST (ESC \\)
- [x] Stripping de secuencias del staging antes del VTE parser
- [x] Scrolling automático si imagen excede el viewport
- [x] Canal `iterm2_tx`/`iterm2_rx` (`mpsc::sync_channel<String>(64)`) con formato `w,h,p:b64`
- [x] Config entry: `iterm2_images: bool` (default true)
- [x] 10 tests unitarios: basic inline BEL, ST terminator, dimensions, preserveAspectRatio, non-inline ignored, multiple images, incomplete ignored, stripping, accept_iterm2, decode_image_bytes
- [x] Build, clippy, fmt, 231 tests pasando

**Archivos:** `crates/SYNAPSE_-app/src/image_protocol.rs`, `crates/SYNAPSE_-app/src/render.rs`, `crates/SYNAPSE_-app/src/pane_ops.rs`, `crates/SYNAPSE_-ui/src/pane.rs`, `crates/SYNAPSE_-config/src/config.rs`, `Cargo.toml`, `crates/SYNAPSE_-app/Cargo.toml`

---

## M-010: Font fallback chain

**Prioridad:** P1 | **Esfuerzo:** 3-5 días | **Dependencias:** M-001 (para no romper ligaduras)

Cuando un glifo no está en la font principal, buscar en una cadena de fallback (ej: `["JetBrains Mono", "Noto Color Emoji", "DejaVu Sans"]`).

- [x] Cambiar `font_family: String` → `font_family: Vec<String>` en config
- [x] En `TextAtlas::new()`, cargar múltiples `Font` de fontdue + `Face` de rustybuzz
- [x] En `atlas.rs:get_or_insert(codepoint)`: si la font principal no tiene el glyph, iterar fallbacks
- [x] Cache por (font_index, codepoint) para no penalizar cada lookup
- [x] En `text.rs:shape_run()`, si la font principal no tiene un glyph en la secuencia de ligadura, saltar a la siguiente font y re-shapear
- [x] Config: `font_family: ["JetBrains Mono", "Noto Color Emoji"]` en TOML
- [x] Test: mostrar emoji (U+1F600) y caracteres CJK (U+4E2D) en la misma línea con JetBrains Mono como font principal

**Archivos:** `crates/SYNAPSE_-config/src/config.rs`, `crates/SYNAPSE_-renderer/src/atlas.rs`, `crates/SYNAPSE_-renderer/src/text.rs`

---

## M-011: Sesiones save/restore

**Prioridad:** P1 | **Esfuerzo:** 2 días | **Dependencias:** M-005 (para CLI `--restore`)

Guardar y restaurar el estado completo de tabs, panes, y CWDs entre sesiones.

- [x] Struct `Session` (serializable con serde):
  ```rust
  struct Session {
      tabs: Vec<TabInfo>,
      active_tab: usize,
      version: String,
      saved_at: String,
  }
  struct TabInfo {
      title: String,
      cwd: String,
      pane_layout: PaneLayout,
      active_pane_id: String,
  }
  ```
- [x] `save_session(state)`: serializar TabBar + layout + CWDs a `~/.cache/SYNAPSE_/session.json`
- [x] `restore_session()`: deserializar y recrear tabs+panes. Los PTYs se recrean frescos (no se puede restaurar el estado del terminal en sí — eso requeriría multiplex).
- [x] Autosave en `window.on_close()` o `Ctrl+Q`
- [x] CLI: `synapse_ --restore` carga la última sesión
- [x] CLI: `synapse_ --restore mysession` carga sesión nombrada
- [x] Config: `restore_session: bool`, `session_save_interval_secs: u64` (autosave periódico)
- [x] Tests: serializar→deserializar round-trip

**Archivos:** nuevo `crates/SYNAPSE_-app/src/session.rs`, `crates/SYNAPSE_-app/src/main.rs`, `crates/SYNAPSE_-app/src/state.rs`, `crates/SYNAPSE_-config/src/config.rs`

---

## M-012: Quake Mode (dropdown terminal)

**Prioridad:** P1 | **Esfuerzo:** 3-4 días | **Dependencias:** M-005 (CLI args)

Terminal estilo dropdown estilo Quake/Guake/Yakuake: se oculta/muestra con una hotkey global.

- [x] En `main.rs`, detectar flag `--quake`
- [x] Si quake mode: crear window con `with_visible(false)`, `with_decorations(false)`, `with_always_on_top(true)`
- [x] Posicionar window: ancho = screen width, alto = 40-50% screen height, y = 0 (top edge)
- [x] Animación slide-down al mostrarse: timer que incrementa y_position cada frame hasta destino
- [x] Animación slide-up al ocultarse: decrementa y_position hasta que window está fuera de pantalla → `set_visible(false)`
- [x] Toggle con `Ctrl+Space` o tecla configurable
- [x] Si la window pierde foco, auto-ocultar (configurable)
- [x] Hotkey global: registrar `Ctrl+Space` o configurable a nivel de sistema (esto requiere DBus en Linux o `global-hotkey` crate)
- [x] Config: sección `[quake]` con `enabled`, `height_percent`, `animation_ms`, `hide_on_focus_lost`, `hotkey`
- [x] Test: correr `./synapse_ --quake`, presionar toggle, verificar slide animation

**Archivos:** `crates/SYNAPSE_-app/src/main.rs`, nuevo `crates/SYNAPSE_-app/src/quake.rs`, `crates/SYNAPSE_-config/src/config.rs`

---

## M-013: Atlas LRU eviction

**Prioridad:** P1 | **Esfuerzo:** 2-3 días | **Dependencias:** M-010 (fallback fonts aumentan presión en atlas)

Cuando el atlas está >90% lleno y se necesita espacio para un glifo nuevo, desalojar los glifos menos usados.

- [x] En `atlas.rs`, añadir `last_used: Instant` a cada entry del atlas (o un contador de generación)
- [x] En `get_or_insert()`, si el atlas está lleno, encontrar el entry con `last_used` más antiguo y:
  - Mover glifos adyacentes para compactar
  - Marcar la región como libre
  - Si no se puede compactar, limpiar todo el atlas (full eviction) y comenzar de nuevo
- [x] Alternativa más simple: doble atlas. Cuando uno se llena, swapear al otro y limpiar el viejo.
- [x] Métrica: loguear `atlas_utilization_percent` cada vez que se evicciona
- [x] Test: renderizar 5000 caracteres Unicode distintos, verificar que el atlas se evictiona sin crash

**Archivos:** `crates/SYNAPSE_-renderer/src/atlas.rs`

---

## M-014: Background tab freeze

**Prioridad:** P1 | **Esfuerzo:** 2 días | **Dependencias:** ninguna

Pausar la lectura del PTY en tabs que no son la activa, ahorrando CPU y batería.

- [x] Añadir campo `frozen: bool` a `Tab`
- [x] En `pane_ops.rs`, el reader thread de cada pane chequea `pane.tab().frozen` y hace `thread::sleep(Duration::from_millis(100))` en vez de leer
- [x] En `keyboard.rs`, al cambiar de tab (Ctrl+Tab), descongelar la nueva tab y congelar la anterior
- [x] Al volver a una tab congelada, el PTY reader debe procesar todo lo acumulado en el buffer del kernel (no se pierde nada, solo se pausa el consumo)
- [x] Opcional: flag `unfreeze_on_bell` — si una tab congelada emite bell (\x07), descongelarla para notificar
- [x] Config: `freeze_background_tabs: bool` (default true)
- [x] Test: abrir 3 tabs, cambiar entre ellas, verificar que solo la activa avanza contenido

**Archivos:** `crates/SYNAPSE_-app/src/pane_ops.rs`, `crates/SYNAPSE_-app/src/keyboard.rs`, `crates/SYNAPSE_-ui/src/pane.rs`, `crates/SYNAPSE_-config/src/config.rs`

---

## M-015: Real damage tracking

**Prioridad:** P1 | **Esfuerzo:** 2-3 días | **Dependencias:** ninguna

En vez de iterar todo el grid cada frame, usar `alacritty_terminal::Term::damage()` para solo reconstruir las celdas que cambiaron.

- [x] `alacritty_terminal::Term::damage()` ya expone `TermDamage { lines: Range<Line>, full: bool }`
- [x] En `render.rs`, antes de construir `cell_data`, llamar `term.damage()` para cada pane visible
- [x] Si `full == false` y `lines` es un rango finito, solo iterar y reconstruir esas líneas del grid
- [x] Mantener un `Vec<CellInstance>` persistente (no reasignarlo cada frame), y solo reemplazar las instancias de líneas dañadas
- [x] La GPU subida de vertex buffer también puede ser parcial (subir solo el rango modificado)
- [x] Test de rendimiento: correr `yes` por 5 segundos, medir tiempo de `build_cell_data()` antes y después
- [x] Debería reducir el tiempo de construcción de cell data en >90% durante output masivo

**Archivos:** `crates/SYNAPSE_-app/src/render.rs`, `crates/SYNAPSE_-renderer/src/renderer.rs`

---

## M-016: Selection visual mejorado (highlight v2)

**Prioridad:** P1 | **Esfuerzo:** 1 día | **Dependencias:** ninguna

El text selection actual funciona pero es básico. Mejorar la estética.

- [x] Cambiar el color de highlight de `selection_bg` a un color semi-transparente del tema (`selection_bg` ya existe en Theme?)
- [x] ~~Añadir animación de borde en la selección~~ (deferido — empeoraría el pipeline)
- [x] Doble-click: seleccionar palabra bajo cursor (delimitadores: espacios, `(){}[]<>'"/\|,;:=`)
- [x] Triple-click: seleccionar línea completa
- [x] Shift+click: extender selección desde el punto de inicio hasta el click
- [x] Alt+click: selección rectangular (block selection)
- [x] Opcional: highlight de todas las ocurrencias de la palabra seleccionada (como VS Code)
- [x] Test: doble-click en una palabra → seleccionada; triple-click → línea entera

**Archivos:** `crates/SYNAPSE_-app/src/render.rs`, `crates/SYNAPSE_-app/src/mouse.rs`, `crates/SYNAPSE_-app/src/state.rs`

---

## M-017: Workspaces

**Prioridad:** P2 | **Esfuerzo:** 3-5 días | **Dependencias:** M-011 (sesiones)

Named workspaces con conjuntos de tabs. Switch rápido entre "dev", "ssh", "logs", etc.

- [x] Struct `Workspace` con name, tabs, active_tab
- [x] Campo `workspaces: HashMap<String, Workspace>` + `active_workspace: String` en `AppCore`
- [x] `Action::WorkspaceSwitch` — rota con `Ctrl+Alt+Tab`
- [x] `Action::WorkspaceNew` — crea workspace con nombre `workspace-N` (`Ctrl+Shift+N`)
- [x] `Action::WorkspaceRename` — abre palette (placeholder)
- [x] `Action::WorkspaceDelete` — eliminar workspace (`Ctrl+Shift+Alt+D`)
- [x] Visual: nombre del workspace en la status bar (ej: `[default] ~/proyecto/`)
- [x] Session save/restore incluye workspaces (HashMap<String, TabBar>)
- [x] Test: crear workspace con `WorkspaceManager::create`, switch, rename, delete

**Archivos:** `crates/SYNAPSE_-app/src/state.rs`, `crates/SYNAPSE_-app/src/keyboard.rs`, `crates/SYNAPSE_-app/src/render.rs`, `crates/SYNAPSE_-config/src/keybinds.rs`

---

## M-018: Profiler overlay (F12)

**Prioridad:** P2 | **Esfuerzo:** 2 días | **Dependencias:** M-015 (damage tracking métricas)

Overlay de debugging con métricas de rendimiento en tiempo real.

- [x] Añadir `Action::ToggleProfiler` + keybind `F12`
- [x] Campo `profiler_active: bool` en `AppState`
- [x] Métricas: `ProfilerData` con `frame_time_ms`, `cell_count`, `draw_calls`, `pty_bytes_per_sec`, `fps`, `atlas_used_percent`, `frame_cache_hit_rate`
- [x] Renderizar overlay como texto en la esquina superior derecha del panel activo, con fondo semi-transparente
- [x] Uso de `cached_cell_data` y `cached_bg_rects` para el overlay (compatible con single render pass)
- [x] Test: F12 toggles `profiler_active`

**Archivos:** `crates/SYNAPSE_-app/src/main.rs`, `crates/SYNAPSE_-app/src/state.rs`, `crates/SYNAPSE_-app/src/render.rs`, `crates/SYNAPSE_-config/src/keybinds.rs`

---

## M-019: History persistente cross-pane

**Prioridad:** P2 | **Esfuerzo:** 2-3 días | **Dependencias:** M-005 (CLI)

Persistir historial de comandos deduplicado entre sesiones, similar a `atuin` pero integrado en la terminal.

- [x] Interceptar OSC 133 A (prompt start) y C (command exit) para delimitar comandos
- [x] Al recibir OSC 133 C/D (command finished), extraer la línea de comando del grid
- [x] Almacenar en `~/.cache/SYNAPSE_/history.json` como `Vec<{cmd, cwd, timestamp, exit_code}>`
- [x] Deduplicar: si el comando ya existe, moverlo al frente (MRU order)
- [x] Integrar con `crates/SYNAPSE_-suggest/src/`: cargar el historial persistente en el frequency trie al iniciar
- [x] Búsqueda cross-session: `Ctrl+R` busca en historial persistente, no solo en scrollback actual
- [x] Config: `persistent_history: bool` (default true), `history_max_entries: usize` (default 10000)
- [x] Test: extract_osc133_marks actualizados (7 tests, incluyendo exit code y unknown king ignorado)

**Archivos:** nuevo `crates/SYNAPSE_-app/src/history.rs`, `crates/SYNAPSE_-app/src/pane_ops.rs`, `crates/SYNAPSE_-suggest/src/`, `crates/SYNAPSE_-config/src/config.rs`

---

## M-020: Plugin system TOML (keybind → shell command)

**Prioridad:** P2 | **Esfuerzo:** 3-5 días | **Dependencias:** ninguna

Extensibilidad vía TOML: definir nuevos comandos de palette y keybinds que ejecutan shell commands con expansión de variables.

- [x] Sección `[plugins]` en config: `[[plugins.commands]]` con `name`, `keybind`, `command`, `cwd`, `split`, `replace_selection`
- [x] Variables: `$CURRENT_PANE_CWD`, `$SELECTED_TEXT`, `$CURRENT_FILE`, `$CLIPBOARD`
- [x] Ejecución: `split = "horizontal"/"vertical"/"tab"` crea pane/tab + ejecuta comando; `split = "overlay"` spawn + captura stdout/stderr + floating overlay; `replace_selection = true` spawn + reemplaza selección con stdout
- [x] Integrar en command palette (`build_palette_items` añade plugins)
- [x] Validación de config al cargar: keybinds duplicados = warning
- [x] Test: 280 tests pass, build OK

**Archivos:** `crates/SYNAPSE_-config/src/config.rs`, `crates/SYNAPSE_-app/src/keyboard.rs`, `crates/SYNAPSE_-app/src/palette.rs`

---

## M-021: Recording asciinema (.cast)

**Prioridad:** P2 | **Esfuerzo:** 1-2 semanas | **Dependencias:** ninguna

Exportar sesión a formato `.cast` de asciinema para compartir grabaciones de terminal.

- [x] `Action::ToggleRecording` + keybind `Ctrl+Shift+R`
- [x] `RecordingShared` (OnceLock global) para compartir estado entre hilos PTY y main thread
- [x] `Record` struct: `start_time`, `events: Vec<(f64, Vec<u8>)>` (timestamp + raw bytes)
- [x] Hook en PTY reader thread: captura `staging` antes del VTE processor si recording activo
- [x] `.cast` format v2: header JSON (version, width, height, timestamp, duration, env) + eventos `[ts, "o", "data"]`
- [x] `Action::ToggleRecording`: start/stop, `stop_recording_if_active()` al cerrar la app
- [x] Status bar: indicador `[REC]` en naranja cuando grabando
- [x] Notificación al terminar: "Recording saved to ..."
- [x] Config: `recording_path` opcional para custom path
- [x] Test: 298 tests pass, build OK, clippy clean

**Archivos:** nuevo `crates/SYNAPSE_-app/src/record.rs`, `crates/SYNAPSE_-app/src/pane_ops.rs`, `crates/SYNAPSE_-app/src/state.rs`, `crates/SYNAPSE_-config/src/keybinds.rs`

---

## M-022: Background image por pane

**Prioridad:** P2 | **Esfuerzo:** 1-2 días | **Dependencias:** M-008 o M-009 (para cargar imagen) | **Estado:** COMPLETADO 2026-05-24

Fondo de imagen personalizado por pane (wallpaper).

- [x] Config por pane o global: `background_image: Option<String>` (path a PNG/JPG)
- [x] En `render.rs`, antes de dibujar las celdas de un pane, dibujar la imagen de fondo escalada al tamaño del pane
- [x] Opacidad configurable: `background_opacity: f32` (0.0-1.0)
- [x] La imagen se carga en `ImageStore` al inicio y se renderiza con el pipeline de imágenes existente (como `ImagePlacement` con z=-1)
- [x] Opciones: `contain`, `cover`, `tile`, `stretch`
- [x] Test: configurar `background_image = "/home/user/wallpaper.png"`, verificar que se ve detrás del texto

**Archivos:** `crates/SYNAPSE_-config/src/config.rs`, `crates/SYNAPSE_-app/src/render.rs`, `crates/SYNAPSE_-app/src/image_protocol.rs`

---

## M-025: Wayland CSD + transparencia

**Prioridad:** P3 | **Esfuerzo:** 3-5 días | **Dependencias:** ninguna | **Estado:** COMPLETADO 2026-05-24

Client-side decorations en Wayland y soporte de transparencia de ventana.

- [x] En `main.rs`, al crear window, detectar si es Wayland (`WINIT_UNIX_BACKEND=wayland`)
- [x] Si Wayland: `WindowAttributes::default().with_decorations(false)` + dibujar barra de título custom (minimal, 28px) con texto "SYNAPSE_"
- [x] Transparencia: `with_transparent(true)` en WindowAttributes
- [x] Config: `window_opacity: f32` (0.0-1.0), `window_blur: bool` (si el compositor lo soporta)
- [x] La transparencia requiere que el clear color del renderer use alpha < 1.0
- [x] Window drag en la title bar (click en zona y < title_bar_height, x >= sidebar_width)

**Archivos:** `crates/SYNAPSE_-app/src/main.rs`, `crates/SYNAPSE_-renderer/src/renderer.rs`, `crates/SYNAPSE_-config/src/config.rs`

---

## M-026: Color emoji (COLRv1/CBDT)

**Prioridad:** P3 | **Esfuerzo:** 5-7 días | **Dependencias:** M-010 (font fallback) | **Estado:** COMPLETADO 2026-05-24

Renderizar emojis en color usando COLRv1 o tablas CBDT/CBLC.

- [x] fontdue no soporta COLRv1 ni CBDT. Usamos `ttf-parser` con `glyph_raster_image` (CBDT/CBLC) + `image` crate para decodificar PNG.
- [x] En `atlas.rs`, nuevo `emoji_cache: HashMap<u32, AtlasEntry>` y método `get_or_insert_emoji()`
- [x] Shader modificado: flag `is_emoji` en CellInstance, branch en fragment shader para emoji (usa sampled RGBA directamente)
- [x] En `renderer.rs`, detección de emoji via `has_color_emoji_in_family()`, extracción de bitmap, premultiply alpha, upload a atlas
- [x] CellInstance extendido: `is_emoji: u32`, `_pad: u32` (72 bytes total)
- [x] Test: `echo "🚀🔥💻🎯🦀"` debe mostrar emojis a color (requiere font con soporte CBDT/CBLC)

---

## M-027: High contrast theme + reduce_motion

**Prioridad:** P3 | **Esfuerzo:** 2 horas | **Dependencias:** ninguna | **Estado:** COMPLETADO 2026-05-24

Modo de accesibilidad básico.

- [x] Añadir theme built-in `high-contrast-dark`: `bg=#000000`, `fg=#FFFFFF`, `accent=#FFFF00`, resto en blanco/negro puro
- [x] Config: `reduce_motion: bool` — si true, deshabilita todas las animaciones (splash, cursor blink, slide animations, pane pulse, resize indicators)
- [x] En cada lugar donde hay animación, chequear `config.reduce_motion` y saltar
- [x] Test: activar reduce_motion, verificar que la UI es instantánea sin animaciones

**Archivos:** `crates/SYNAPSE_-config/src/themes.rs`, `crates/SYNAPSE_-config/src/config.rs`, `crates/SYNAPSE_-app/src/render.rs`

---

## M-028: Scrollbar / minimap

**Prioridad:** P3 | **Esfuerzo:** 1-2 días | **Dependencias:** M-016 (selection v2 para interacción) | **Estado:** COMPLETADO 2026-05-24

Barra de scroll vertical o minimapa lateral para navegar el scrollback.

- [x] Config: `scrollbar: bool` (default true), solo thin mode (6px track + 12px min thumb)
- [x] Modo "thin": barra de 6px en el borde derecho, altura proporcional al viewport/scrollback, colores en Theme
- [x] Click en scrollbar: saltar a esa posición del scrollback via `scrollbar_click()`
- [x] Drag en scrollbar: scroll continuo via `scrollbar_drag: Option<PaneId>` en AppState
- [x] Interacción solo con mouse, no interfiere con selección de texto
- [x] Test: generar líneas de output, verificar que la scrollbar aparece y es interactiva

**Archivos:** `crates/SYNAPSE_-app/src/render.rs`, `crates/SYNAPSE_-app/src/mouse.rs`, `crates/SYNAPSE_-config/src/config.rs`

---

## M-029: Pane watermark / badge

**Prioridad:** P3 | **Esfuerzo:** 1 día | **Dependencias:** M-022 (background image, misma zona) | **Estado:** COMPLETADO 2026-05-24

Texto semi-transparente en el fondo del pane mostrando el nombre del pane/CWD/tab.

- [x] Config: `pane_badge: bool` (default false)
- [x] Si activo: renderizar texto del CWD o título del pane en grande (font 48px+), centrado, con opacidad 0.05, detrás de las celdas
- [x] Formato configurable: `pane_badge_format: "{cwd}"` o `"{title}"` o `"{user}@{host}"`
- [x] Se actualiza cuando cambia el CWD (OSC 7) o el título (OSC 0/2)
- [x] Test: activar badge, verificar que se ve el path en el fondo del pane

**Archivos:** `crates/SYNAPSE_-app/src/render.rs`, `crates/SYNAPSE_-config/src/config.rs`

---

## M-030: Shell integration scripts

**Prioridad:** P3 | **Esfuerzo:** 2-3 días | **Dependencias:** M-018 (profiler), M-019 (history) | **Estado:** COMPLETADO 2026-05-24

Scripts oficiales de integración para zsh, bash, y fish que envíen OSC sequences para features avanzados.

- [x] Script `synapse-integration.zsh` / `.bash` / `.fish` que se sourcea desde `.zshrc`
- [x] Funcionalidades:
  - `preexec()` hook: envía OSC 133 A (prompt start) + metadata del comando
  - `precmd()` hook: envía OSC 133 C (command exit) + exit code + timestamp
  - Cambio de directorio: envía OSC 7 file://host/path
  - Notificaciones de comandos largos: si un comando dura >30s, envía OSC 777 para notificar
  - Variable `SYNAPSE_INSIDE` para detección (inyectada en el PTY spawn)
- [x] Empaquetar scripts en `~/.config/SYNAPSE_/shell/` al primer arranque o con `--setup`
- [x] Comando `synapse_ --setup` que copia scripts y actualiza `.zshrc`/`.bashrc`/`config.fish`
- [x] Config: `shell_integration: bool` (default false), auto-instala scripts si true

**Archivos:** nuevos `assets/shell/synapse-integration.zsh`, `assets/shell/synapse-integration.bash`, `assets/shell/synapse-integration.fish`, `crates/SYNAPSE_-app/src/pane_ops.rs`

---

## Resumen por esfuerzo

| ID | Feature | Esfuerzo | Prioridad | Estado |
|----|---------|----------|-----------|--------|
| M-001 | Ligaduras ON por defecto | 5 min | P0 | ✅ DONE |
| M-002 | Pane Zoom | 1 día | P0 | ✅ DONE |
| M-003 | Broadcast Input | 2h | P0 | ✅ DONE |
| M-004 | Drag & Drop | 2h | P0 | ✅ DONE |
| M-005 | CLI args | 1 día | P0 | ✅ DONE |
| M-006 | Search regex | 2h | P0 | ✅ DONE |
| M-007 | Path detection clickable | 1 día | P1 | ✅ DONE |
| M-008 | Sixel decoder | 3-5d | P1 | ✅ DONE |
| M-009 | iTerm2 OSC 1337 | 2d | P1 | ✅ DONE |
| M-010 | Font fallback | 3-5d | P1 | ✅ DONE |
| M-011 | Sesiones save/restore | 2d | P1 | ✅ DONE |
| M-012 | Quake Mode | 3-4d | P1 | ✅ DONE |
| M-013 | Atlas LRU | 2-3d | P1 | ✅ DONE |
| M-014 | Background tab freeze | 2d | P1 | ✅ DONE |
| M-015 | Damage tracking real | 2-3d | P1 | ✅ DONE |
| M-016 | Selection v2 | 1d | P1 | ✅ DONE |
| M-017 | Workspaces | 3-5d | P2 | ✅ DONE |
| M-018 | Profiler overlay | 2d | P2 | ✅ DONE |
| M-019 | History persistente | 2-3d | P2 | ✅ DONE |
| M-020 | Plugin system TOML | 3-5d | P2 | ✅ DONE |
| M-021 | Recording .cast | 1-2w | P2 | ✅ DONE |
| M-022 | Background image | 1-2d | P2 | ✅ DONE |
| M-025 | Wayland CSD + transparencia | 3-5d | P3 | ✅ DONE |
| M-026 | Color emoji | 5-7d | P3 | ✅ DONE |
| M-027 | High contrast + reduce_motion | 2h | P3 | ✅ DONE |
| M-028 | Scrollbar / minimap | 1-2d | P3 | ✅ DONE |
| M-029 | Pane badge | 1d | P3 | ✅ DONE |
| M-030 | Shell integration scripts | 2-3d | P3 | ✅ DONE |

---

## Progreso global

- [x] **Fase 1: P0 (6 mejoras, ~3 días)** — Ligaduras, Pane Zoom, Broadcast, Drag & Drop, CLI args, Search regex
- [x] **Fase 2: P1 (10 mejoras, ~15-25 días)** — Path detection, Sixel, iTerm2 images, Font fallback, Sesiones, Quake, Atlas LRU, Tab freeze, Damage tracking, Selection v2
- [x] **Fase 3: P2 (6 mejoras, ~12-20 días)** — Workspaces, Profiler, History, Plugins, Recording, Background images
- [x] **Fase 4: P3 (6 mejoras, ~15-25 días)** — Wayland CSD, Emoji color, A11y, Scrollbar, Badge, Shell integration

---

*"La terminal más completa del mercado: open source, GPU-accelerated, Rust puro, sin telemetría."*
