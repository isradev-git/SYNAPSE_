# Mejoras Definitivas — SYNAPSE_ Ultraterminal v1

> Plan maestro para alcanzar la terminal más completa del mercado. Cada feature es atómica, trackeable con [ ], y ordenada por prioridad. Empezar por la M-001 y avanzar en orden.

**Versión:** 1.0 | **Fecha:** 2026-05-22  
**Basado en:** Competitive analysis vs. Kitty, WezTerm, Warp, Ghostty, iTerm2  
**Estado actual:** Fases 0-13 completadas + UI Phases A-G completadas | 205 tests | v0.2.0

---

## Metodología

- Cada mejora tiene: ID, prioridad (P0-P3), esfuerzo estimado, archivos a tocar, y checklist de subtareas.
- Las mejoras son independientes entre sí (salvo cuando se indica dependencia explícita).
- Al completar una mejora, marcar `[x]` y commitear con prefijo `feat(M-XXX):`.
- Mantener clippy clean + tests pasando + cargo fmt después de cada una.

---

## M-001: Ligaduras ON por defecto

**Prioridad:** P0 | **Esfuerzo:** 5 min | **Dependencias:** ninguna

El engine de ligaduras (rustybuzz + HarfBuzz) ya está implementado en `text.rs:306-347` y `renderer.rs:354-428`. Solo está deshabilitado por defecto.

- [ ] Cambiar `font_ligatures: false` → `font_ligatures: true` en `crates/SYNAPSE_-config/src/config.rs:119` (o donde esté el `Default`)
- [ ] Verificar que `cargo test -p SYNAPSE_-config` pasa
- [ ] Build release y probar visualmente con JetBrains Mono o Fira Code

**Archivos:** `crates/SYNAPSE_-config/src/config.rs`

---

## M-002: Pane Zoom (Ctrl+Shift+Z)

**Prioridad:** P0 | **Esfuerzo:** 1 día | **Dependencias:** ninguna

Maximizar un pane a pantalla completa y restaurarlo. Similar a WezTerm/iTerm2.

- [ ] Añadir `Zoom` al enum `Action` en `keybinds.rs`
- [ ] Añadir default keybind `Ctrl+Shift+Z` → `"zoom_pane"` en `default_entries()`
- [ ] Añadir campo `zoomed_pane: Option<PaneId>` en `AppState`
- [ ] En `keyboard.rs`, handler `Action::Zoom`:
  - Si `zoomed_pane == None`: guardar pane activo y layout actual, crear layout temporal con un solo pane full-window
  - Si `zoomed_pane == Some`: restaurar layout original
- [ ] En `render.rs`, si hay zoom activo, el layout para ese pane ignora el PaneTree y ocupa toda el área de panel
- [ ] Test: zoom in, escribir en terminal, zoom out, verificar que nada se pierde
- [ ] Test unitario: zoom toggle cambia `zoomed_pane`

**Archivos:** `crates/SYNAPSE_-config/src/keybinds.rs`, `crates/SYNAPSE_-app/src/state.rs`, `crates/SYNAPSE_-app/src/keyboard.rs`, `crates/SYNAPSE_-app/src/render.rs`

---

## M-003: Broadcast Input (Ctrl+Shift+B)

**Prioridad:** P0 | **Esfuerzo:** 2 horas | **Dependencias:** ninguna

Enviar la misma entrada a todos los panes de la tab activa. Indispensable para operar múltiples servidores SSH a la vez.

- [ ] Añadir `ToggleBroadcast` al enum `Action` en `keybinds.rs`
- [ ] Añadir default `Ctrl+Shift+B` → `"toggle_broadcast"` en `default_entries()`
- [ ] Añadir campo `broadcasting: bool` en `AppState`
- [ ] En `keyboard.rs`, handler `Action::ToggleBroadcast`: flip `broadcasting`
- [ ] En `pane_ops.rs`, función `write_to_pty()` existente: si `state.broadcasting`, iterar todos los `PaneId` de la tab activa y escribir a cada uno
- [ ] Indicador visual en status bar / tab: "[BROADCAST]" o icono cuando activo
- [ ] Test: activar broadcast, escribir `echo hola`, verificar que todos los panes reciben el comando

**Archivos:** `crates/SYNAPSE_-config/src/keybinds.rs`, `crates/SYNAPSE_-app/src/state.rs`, `crates/SYNAPSE_-app/src/keyboard.rs`, `crates/SYNAPSE_-app/src/pane_ops.rs`, `crates/SYNAPSE_-app/src/render.rs`

---

## M-004: Drag & Drop de archivos

**Prioridad:** P0 | **Esfuerzo:** 2 horas | **Dependencias:** ninguna

Arrastrar un archivo desde el explorador y que pegue su path absoluto en el pane activo.

- [ ] En `main.rs` event loop, añadir match para `WindowEvent::DroppedFile(path)`
- [ ] Obtener path absoluto canonizado con `std::fs::canonicalize(path)`
- [ ] Escribir path + espacio en el PTY del pane activo via `write_to_pty()`
- [ ] Si hay múltiples archivos, escribir paths separados por espacio
- [ ] Test manual: arrastrar un archivo .rs desde el explorador → debe aparecer el path en la terminal

**Archivos:** `crates/SYNAPSE_-app/src/main.rs`, `crates/SYNAPSE_-app/src/pane_ops.rs`

---

## M-005: CLI args (-e, --new-tab, --help)

**Prioridad:** P0 | **Esfuerzo:** 1 día | **Dependencias:** ninguna

`clap` está en `[workspace.dependencies]` pero sin usar. Dar soporte a argumentos de línea de comandos.

- [ ] Añadir `use clap::Parser` en `main.rs`
- [ ] Struct `Cli`:
  ```rust
  #[derive(Parser)]
  #[command(name = "synapse_", version, about)]
  struct Cli {
      /// Ejecutar comando y salir
      #[arg(short = 'e')]
      command: Option<String>,
      /// Directorio de trabajo inicial
      #[arg(short = 'd', long)]
      working_directory: Option<String>,
      /// Nuevo tab con comando
      #[arg(long)]
      new_tab: Option<String>,
      /// Mantener terminal viva tras ejecutar -e
      #[arg(long)]
      hold: bool,
  }
  ```
- [ ] Si `-e cmd`: spawn PTY con `[cmd]` en vez de `[$SHELL]`, esperar a que termine, salir (o mantener si `--hold`)
- [ ] Si `--new-tab cmd`: abrir terminal normalmente + nuevo tab ejecutando `cmd`
- [ ] `-d path`: cambiar CWD del primer pane a ese path
- [ ] Respeta `SHELL` env var para el comando de shell base
- [ ] Test: `./synapse_ -e "echo hola"` debe imprimir "hola" y salir
- [ ] Test: `./synapse_ --new-tab "htop"` debe abrir con un tab extra corriendo htop

**Archivos:** `crates/SYNAPSE_-app/src/main.rs`, `crates/SYNAPSE_-app/src/pane_ops.rs`

---

## M-006: Search con regex toggle

**Prioridad:** P0 | **Esfuerzo:** 2 horas | **Dependencias:** ninguna

El search actual (`search.rs`) usa substring matching. Añadir toggle para regex.

- [ ] Añadir campo `regex_mode: bool` en `SearchState` (`search.rs`)
- [ ] En `search.rs:do_search()`, si `regex_mode`, compilar pattern con `regex::Regex::new(query)` (la crate `regex` no está en deps — añadir a `Cargo.toml` del crate app, o usar `regex-automata` si ya está en el árbol de deps de tracing/log)
- [ ] Keybind: `Alt+R` dentro de la search bar togglea `regex_mode`
- [ ] Indicador visual en la search bar: "[regex]" o "/re/"
- [ ] Fallback graceful: si el regex es inválido, mostrar "invalid regex" en lugar de crashear
- [ ] Test unitario: `regex_search("foo|bar")` matchea "foo" y "bar"

**Archivos:** `crates/SYNAPSE_-app/src/search.rs`, `crates/SYNAPSE_-app/Cargo.toml`

---

## M-007: File/Path detection clickable

**Prioridad:** P1 | **Esfuerzo:** 1 día | **Dependencias:** M-006 (infra regex, se puede reusar)

Detectar paths como `/home/user/project/src/main.rs:42` o `./lib/mod.ts:10` y hacerlos Ctrl+click. Reutiliza la infraestructura OSC 8 existente.

- [ ] En `render.rs`, junto a la detección de OSC 8 hyperlinks, añadir regex scan:
  - Path absoluto: `([/~][a-zA-Z0-9._\-/]+)(?::(\d+))?(?::(\d+))?`
  - Path relativo: `(\.{1,2}/[a-zA-Z0-9._\-/]+)(?::(\d+))?(?::(\d+))?`
- [ ] Construir `cached_url_spans` con estos matches como `HoverUrl` con `open_url` modificado
- [ ] En `mouse.rs`, al hacer Ctrl+click en un path:
  - Si `path:line:col`, abrir con `$EDITOR +line path` o `code -g path:line`
  - Si solo path, abrir con `xdg-open path`
- [ ] Config entry `clickable_paths: bool` (default true)
- [ ] Test: ejecutar `gcc main.c` que produce error, hacer Ctrl+click en `main.c:42:10` → debe abrir editor en esa línea

**Archivos:** `crates/SYNAPSE_-app/src/render.rs`, `crates/SYNAPSE_-app/src/mouse.rs`, `crates/SYNAPSE_-config/src/config.rs`

---

## M-008: Sixel decoder

**Prioridad:** P1 | **Esfuerzo:** 3-5 días | **Dependencias:** ninguna (ImageStore pipeline ya existe)

Sixel es el formato de imágenes más usado en terminales (gnuplot, lsix, img2sixel). La pipeline de imágenes existe en `image_protocol.rs` + `renderer/image.rs`. Solo falta el decoder.

- [ ] Añadir crate `sixel-rs` o `libsixel` binding, o implementar decoder mínimo:
  - Detectar `ESC P q` ... `ESC \` como inicio/fin de secuencia Sixel
  - Parsear: color registers (#P), repeat (!N char), raster attributes ("pan;pad;w;h), new line ($), next line (-)
  - Decodificar a bitmap RGBA (w × h × 4)
  - Subir como `StoredImage` a `ImageStore` con ID auto-asignado
  - Colocar como `ImagePlacement` en cursor actual
- [ ] Integrar en el PTY reader thread (similar a como KKP/KKP pre-scan en `image_protocol.rs`)
- [ ] Soportar Sixel scrolling: cuando llega al fondo del viewport, hacer scroll
- [ ] Config entry `sixel_enabled: bool` (default true)
- [ ] Test: `printf '\ePq#0;2;100;100;100#1;2;100;50;50#2;2;100;100;0#1~~@@#2~~@@#0$$$$$\e\\'` debe dibujar algo visible

**Archivos:** `crates/SYNAPSE_-app/src/image_protocol.rs`, nuevo `crates/SYNAPSE_-app/src/sixel.rs`, `crates/SYNAPSE_-config/src/config.rs`

---

## M-009: iTerm2 OSC 1337 inline images

**Prioridad:** P1 | **Esfuerzo:** 2 días | **Dependencias:** ninguna (misma pipeline que Sixel)

Soporte para `OSC 1337 ; File=inline=1 ; size=N ; name=base64encoded : BASE64` y `File=inline=1 : BASE64`.

- [ ] Detectar `ESC ] 1337 ; File=inline=1` en el PTY reader thread
- [ ] Parsear atributos: `size`, `width`, `height`, `preserveAspectRatio`, `name`
- [ ] Decodificar base64 → bytes
- [ ] Detectar formato (PNG header `\x89PNG`, JPEG `\xff\xd8`, GIF `GIF87a`/`GIF89a`, BMP `BM`)
- [ ] Decodificar a RGBA usando `image` crate (si no está en deps, añadir)
- [ ] Subir a `ImageStore` con ID auto-asignado
- [ ] Colocar como `ImagePlacement` en cursor actual
- [ ] Manejar transmisión chunked (si los datos llegan en múltiples lecturas del PTY)
- [ ] Config entry: `iterm2_images: bool` (default true)
- [ ] Test: `printf '\e]1337;File=inline=1;width=3;preserveAspectRatio=1:<base64 de un PNG 3x3>\a'`

**Archivos:** `crates/SYNAPSE_-app/src/image_protocol.rs`, `crates/SYNAPSE_-config/src/config.rs`

---

## M-010: Font fallback chain

**Prioridad:** P1 | **Esfuerzo:** 3-5 días | **Dependencias:** M-001 (para no romper ligaduras)

Cuando un glifo no está en la font principal, buscar en una cadena de fallback (ej: `["JetBrains Mono", "Noto Color Emoji", "DejaVu Sans"]`).

- [ ] Cambiar `font_family: String` → `font_family: Vec<String>` en config
- [ ] En `TextAtlas::new()`, cargar múltiples `Font` de fontdue + `Face` de rustybuzz
- [ ] En `atlas.rs:get_or_insert(codepoint)`: si la font principal no tiene el glyph, iterar fallbacks
- [ ] Cache por (font_index, codepoint) para no penalizar cada lookup
- [ ] En `text.rs:shape_run()`, si la font principal no tiene un glyph en la secuencia de ligadura, saltar a la siguiente font y re-shapear
- [ ] Config: `font_family: ["JetBrains Mono", "Noto Color Emoji"]` en TOML
- [ ] Test: mostrar emoji (U+1F600) y caracteres CJK (U+4E2D) en la misma línea con JetBrains Mono como font principal

**Archivos:** `crates/SYNAPSE_-config/src/config.rs`, `crates/SYNAPSE_-renderer/src/atlas.rs`, `crates/SYNAPSE_-renderer/src/text.rs`

---

## M-011: Sesiones save/restore

**Prioridad:** P1 | **Esfuerzo:** 2 días | **Dependencias:** M-005 (para CLI `--restore`)

Guardar y restaurar el estado completo de tabs, panes, y CWDs entre sesiones.

- [ ] Struct `Session` (serializable con serde):
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
- [ ] `save_session(state)`: serializar TabBar + layout + CWDs a `~/.cache/SYNAPSE_/session.json`
- [ ] `restore_session()`: deserializar y recrear tabs+panes. Los PTYs se recrean frescos (no se puede restaurar el estado del terminal en sí — eso requeriría multiplex).
- [ ] Autosave en `window.on_close()` o `Ctrl+Q`
- [ ] CLI: `synapse_ --restore` carga la última sesión
- [ ] CLI: `synapse_ --restore mysession` carga sesión nombrada
- [ ] Config: `restore_session: bool`, `session_save_interval_secs: u64` (autosave periódico)
- [ ] Tests: serializar→deserializar round-trip

**Archivos:** nuevo `crates/SYNAPSE_-app/src/session.rs`, `crates/SYNAPSE_-app/src/main.rs`, `crates/SYNAPSE_-app/src/state.rs`, `crates/SYNAPSE_-config/src/config.rs`

---

## M-012: Quake Mode (dropdown terminal)

**Prioridad:** P1 | **Esfuerzo:** 3-4 días | **Dependencias:** M-005 (CLI args)

Terminal estilo dropdown estilo Quake/Guake/Yakuake: se oculta/muestra con una hotkey global.

- [ ] En `main.rs`, detectar flag `--quake`
- [ ] Si quake mode: crear window con `with_visible(false)`, `with_decorations(false)`, `with_always_on_top(true)`
- [ ] Posicionar window: ancho = screen width, alto = 40-50% screen height, y = 0 (top edge)
- [ ] Animación slide-down al mostrarse: timer que incrementa y_position cada frame hasta destino
- [ ] Animación slide-up al ocultarse: decrementa y_position hasta que window está fuera de pantalla → `set_visible(false)`
- [ ] Toggle con `Ctrl+Space` o tecla configurable
- [ ] Si la window pierde foco, auto-ocultar (configurable)
- [ ] Hotkey global: registrar `Ctrl+Space` o configurable a nivel de sistema (esto requiere DBus en Linux o `global-hotkey` crate)
- [ ] Config: sección `[quake]` con `enabled`, `height_percent`, `animation_ms`, `hide_on_focus_lost`, `hotkey`
- [ ] Test: correr `./synapse_ --quake`, presionar toggle, verificar slide animation

**Archivos:** `crates/SYNAPSE_-app/src/main.rs`, nuevo `crates/SYNAPSE_-app/src/quake.rs`, `crates/SYNAPSE_-config/src/config.rs`

---

## M-013: Atlas LRU eviction

**Prioridad:** P1 | **Esfuerzo:** 2-3 días | **Dependencias:** M-010 (fallback fonts aumentan presión en atlas)

Cuando el atlas está >90% lleno y se necesita espacio para un glifo nuevo, desalojar los glifos menos usados.

- [ ] En `atlas.rs`, añadir `last_used: Instant` a cada entry del atlas (o un contador de generación)
- [ ] En `get_or_insert()`, si el atlas está lleno, encontrar el entry con `last_used` más antiguo y:
  - Mover glifos adyacentes para compactar
  - Marcar la región como libre
  - Si no se puede compactar, limpiar todo el atlas (full eviction) y comenzar de nuevo
- [ ] Alternativa más simple: doble atlas. Cuando uno se llena, swapear al otro y limpiar el viejo.
- [ ] Métrica: loguear `atlas_utilization_percent` cada vez que se evicciona
- [ ] Test: renderizar 5000 caracteres Unicode distintos, verificar que el atlas se evictiona sin crash

**Archivos:** `crates/SYNAPSE_-renderer/src/atlas.rs`

---

## M-014: Background tab freeze

**Prioridad:** P1 | **Esfuerzo:** 2 días | **Dependencias:** ninguna

Pausar la lectura del PTY en tabs que no son la activa, ahorrando CPU y batería.

- [ ] Añadir campo `frozen: bool` a `Tab`
- [ ] En `pane_ops.rs`, el reader thread de cada pane chequea `pane.tab().frozen` y hace `thread::sleep(Duration::from_millis(100))` en vez de leer
- [ ] En `keyboard.rs`, al cambiar de tab (Ctrl+Tab), descongelar la nueva tab y congelar la anterior
- [ ] Al volver a una tab congelada, el PTY reader debe procesar todo lo acumulado en el buffer del kernel (no se pierde nada, solo se pausa el consumo)
- [ ] Opcional: flag `unfreeze_on_bell` — si una tab congelada emite bell (\x07), descongelarla para notificar
- [ ] Config: `freeze_background_tabs: bool` (default true)
- [ ] Test: abrir 3 tabs, cambiar entre ellas, verificar que solo la activa avanza contenido

**Archivos:** `crates/SYNAPSE_-app/src/pane_ops.rs`, `crates/SYNAPSE_-app/src/keyboard.rs`, `crates/SYNAPSE_-ui/src/pane.rs`, `crates/SYNAPSE_-config/src/config.rs`

---

## M-015: Real damage tracking

**Prioridad:** P1 | **Esfuerzo:** 2-3 días | **Dependencias:** ninguna

En vez de iterar todo el grid cada frame, usar `alacritty_terminal::Term::damage()` para solo reconstruir las celdas que cambiaron.

- [ ] `alacritty_terminal::Term::damage()` ya expone `TermDamage { lines: Range<Line>, full: bool }`
- [ ] En `render.rs`, antes de construir `cell_data`, llamar `term.damage()` para cada pane visible
- [ ] Si `full == false` y `lines` es un rango finito, solo iterar y reconstruir esas líneas del grid
- [ ] Mantener un `Vec<CellInstance>` persistente (no reasignarlo cada frame), y solo reemplazar las instancias de líneas dañadas
- [ ] La GPU subida de vertex buffer también puede ser parcial (subir solo el rango modificado)
- [ ] Test de rendimiento: correr `yes` por 5 segundos, medir tiempo de `build_cell_data()` antes y después
- [ ] Debería reducir el tiempo de construcción de cell data en >90% durante output masivo

**Archivos:** `crates/SYNAPSE_-app/src/render.rs`, `crates/SYNAPSE_-renderer/src/renderer.rs`

---

## M-016: Selection visual mejorado (highlight v2)

**Prioridad:** P1 | **Esfuerzo:** 1 día | **Dependencias:** ninguna

El text selection actual funciona pero es básico. Mejorar la estética.

- [ ] Cambiar el color de highlight de `selection_bg` a un color semi-transparente del tema (`selection_bg` ya existe en Theme?)
- [ ] Añadir animación de borde en la selección: pulso sutil de 1px del color de acento
- [ ] Doble-click: seleccionar palabra bajo cursor (delimitadores: espacios, `(){}[]<>'"/\|,;:=`)
- [ ] Triple-click: seleccionar línea completa
- [ ] Shift+click: extender selección desde el punto de inicio hasta el click
- [ ] Alt+click: selección rectangular (block selection)
- [ ] Opcional: highlight de todas las ocurrencias de la palabra seleccionada (como VS Code)
- [ ] Test: doble-click en una palabra → seleccionada; triple-click → línea entera

**Archivos:** `crates/SYNAPSE_-app/src/render.rs`, `crates/SYNAPSE_-app/src/mouse.rs`, `crates/SYNAPSE_-app/src/state.rs`

---

## M-017: Workspaces

**Prioridad:** P2 | **Esfuerzo:** 3-5 días | **Dependencias:** M-011 (sesiones)

Named workspaces con conjuntos de tabs. Switch rápido entre "dev", "ssh", "logs", etc.

- [ ] Struct `Workspace`:
  ```rust
  struct Workspace {
      name: String,
      tabs: Vec<Tab>,
      active_tab: usize,
  }
  ```
- [ ] Campo `workspaces: HashMap<String, Workspace>` + `active_workspace: String` en `AppState`
- [ ] `Action::WorkspaceSwitch` — muestra lista en palette o rota con `Ctrl+Shift+N`
- [ ] `Action::WorkspaceNew` — crea workspace con nombre dado (input en palette)
- [ ] `Action::WorkspaceRename` — renombrar workspace actual
- [ ] `Action::WorkspaceDelete` — eliminar workspace (mata todos sus PTYs)
- [ ] Visual: nombre del workspace en la status bar (ej: `[dev] ~/proyecto/`)
- [ ] Session save/restore incluye workspaces
- [ ] Test: crear workspace "dev" con 3 tabs, crear "ssh" con 1 tab, switchear entre ellos

**Archivos:** `crates/SYNAPSE_-app/src/state.rs`, `crates/SYNAPSE_-app/src/keyboard.rs`, `crates/SYNAPSE_-app/src/render.rs`, `crates/SYNAPSE_-config/src/keybinds.rs`

---

## M-018: Profiler overlay (F12)

**Prioridad:** P2 | **Esfuerzo:** 2 días | **Dependencias:** M-015 (damage tracking métricas)

Overlay de debugging con métricas de rendimiento en tiempo real.

- [ ] Añadir `Action::ToggleProfiler` + keybind `F12`
- [ ] Campo `profiler_active: bool` en `AppState`
- [ ] En `main.rs` event loop, acumular métricas:
  - `frame_time_ms`: tiempo entre `RedrawRequested` y `present()`
  - `cell_count`: número de celdas dibujadas este frame
  - `draw_calls`: número de draw calls
  - `pty_bytes_per_sec`: bytes leídos del PTY en el último segundo (ventana deslizante)
  - `fps`: media móvil de los últimos 60 frames
  - `atlas_used_percent`: porcentaje del atlas ocupado
  - `frame_cache_hit_rate`: % de frames que usaron el caché
- [ ] Renderizar overlay como texto en la esquina superior derecha del panel activo, con fondo semi-transparente
- [ ] Usar `draw_text()` del font engine existente para renderizar las líneas de texto del profiler
- [ ] Test: abrir profiler, correr `find /`, verificar métricas en tiempo real

**Archivos:** `crates/SYNAPSE_-app/src/main.rs`, `crates/SYNAPSE_-app/src/state.rs`, `crates/SYNAPSE_-app/src/render.rs`, `crates/SYNAPSE_-config/src/keybinds.rs`

---

## M-019: History persistente cross-pane

**Prioridad:** P2 | **Esfuerzo:** 2-3 días | **Dependencias:** M-005 (CLI)

Persistir historial de comandos deduplicado entre sesiones, similar a `atuin` pero integrado en la terminal.

- [ ] Interceptar OSC 133 A (prompt start) y C (command exit) para delimitar comandos
- [ ] Al recibir OSC 133 C (command finished), extraer la línea de comando del grid
- [ ] Almacenar en `~/.cache/SYNAPSE_/history.json` como `Vec<{cmd, cwd, timestamp, exit_code}>`
- [ ] Deduplicar: si el comando ya existe, moverlo al frente (MRU order)
- [ ] Integrar con `crates/SYNAPSE_-suggest/src/`: cargar el historial persistente en el frequency trie al iniciar
- [ ] Búsqueda cross-session: `Ctrl+R` busca en historial persistente, no solo en scrollback actual
- [ ] Config: `persistent_history: bool` (default true), `history_max_entries: usize` (default 10000)
- [ ] Test: ejecutar comandos, cerrar SYNAPSE_, reabrir, verificar que `Ctrl+R` los encuentra

**Archivos:** nuevo `crates/SYNAPSE_-app/src/history.rs`, `crates/SYNAPSE_-app/src/pane_ops.rs`, `crates/SYNAPSE_-suggest/src/`, `crates/SYNAPSE_-config/src/config.rs`

---

## M-020: Plugin system TOML (keybind → shell command)

**Prioridad:** P2 | **Esfuerzo:** 3-5 días | **Dependencias:** ninguna

Extensibilidad vía TOML: definir nuevos comandos de palette y keybinds que ejecutan shell commands con expansión de variables.

- [ ] Sección `[plugins]` en config:
  ```toml
  [[plugins.commands]]
  name = "Open Lazygit"
  keybind = "Ctrl+Shift+G"
  command = "lazygit"
  cwd = "$CURRENT_PANE_CWD"
  split = "horizontal"  # or "vertical", "tab", "overlay"

  [[plugins.commands]]
  name = "Format JSON"
  keybind = "Ctrl+Shift+J"
  command = "python3 -m json.tool $SELECTED_TEXT"
  replace_selection = true
  ```
- [ ] Variables: `$CURRENT_PANE_CWD`, `$SELECTED_TEXT`, `$CURRENT_FILE` (del OSC 7), `$CLIPBOARD`
- [ ] Ejecución:
  - `split = "horizontal"`: split pane + ejecutar comando en nuevo pane
  - `split = "overlay"`: ventana flotante que ejecuta el comando y muestra stdout/stderr
  - `replace_selection = true`: reemplazar texto seleccionado con stdout del comando
- [ ] Integrar en command palette (`build_palette_items` añade plugins)
- [ ] Validación de config al cargar: keybinds duplicados = warning
- [ ] Test: definir un plugin "echo hello", ejecutarlo desde palette, verificar stdout

**Archivos:** `crates/SYNAPSE_-config/src/config.rs`, `crates/SYNAPSE_-app/src/keyboard.rs`, `crates/SYNAPSE_-app/src/palette.rs`

---

## M-021: Recording asciinema (.cast)

**Prioridad:** P2 | **Esfuerzo:** 1-2 semanas | **Dependencias:** ninguna

Exportar sesión a formato `.cast` de asciinema para compartir grabaciones de terminal.

- [ ] `Action::ToggleRecording` + keybind `Ctrl+Shift+R` (o configurable)
- [ ] Campo `recording: Option<RecordingState>` en `AppState`:
  ```rust
  struct RecordingState {
      start_time: Instant,
      events: Vec<Vec<u8>>,  // raw PTY output chunks
  }
  ```
- [ ] En el PTY reader thread, si `recording.is_some()`, copiar cada chunk de salida a `events`
- [ ] `.cast` format v2:
  - Header: `{"version": 2, "width": W, "height": H, "timestamp": T, "env": {"SHELL": "...", "TERM": "..."}}`
  - Eventos: `[ts, "o", "output data\n"]`
- [ ] `Action::StopRecording`: escribir `~/.cache/SYNAPSE_/recordings/synapse_YYYY-MM-DD_HH-MM-SS.cast`
- [ ] Notificación al terminar: "Recording saved to ..."
- [ ] Test: grabar 5 segundos, verificar que el `.cast` se reproduce con `asciinema play`

**Archivos:** nuevo `crates/SYNAPSE_-app/src/record.rs`, `crates/SYNAPSE_-app/src/pane_ops.rs`, `crates/SYNAPSE_-app/src/state.rs`, `crates/SYNAPSE_-config/src/keybinds.rs`

---

## M-022: Background image por pane

**Prioridad:** P2 | **Esfuerzo:** 1-2 días | **Dependencias:** M-008 o M-009 (para cargar imagen)

Fondo de imagen personalizado por pane (wallpaper).

- [ ] Config por pane o global: `background_image: Option<String>` (path a PNG/JPG)
- [ ] En `render.rs`, antes de dibujar las celdas de un pane, dibujar la imagen de fondo escalada al tamaño del pane
- [ ] Opacidad configurable: `background_opacity: f32` (0.0-1.0)
- [ ] La imagen se carga en `ImageStore` al inicio y se renderiza con el pipeline de imágenes existente (como `ImagePlacement` con z=-1)
- [ ] Opciones: `contain`, `cover`, `tile`, `stretch`
- [ ] Test: configurar `background_image = "/home/user/wallpaper.png"`, verificar que se ve detrás del texto

**Archivos:** `crates/SYNAPSE_-config/src/config.rs`, `crates/SYNAPSE_-app/src/render.rs`, `crates/SYNAPSE_-app/src/image_protocol.rs`

---

## M-023: Multiplex daemon (tmux Rust nativo)

**Prioridad:** P3 | **Esfuerzo:** 2-3 semanas | **Dependencias:** M-005, M-011

El verdadero diferenciador: servidor de multiplexación que persiste sesiones más allá de la ventana.

- [ ] Arquitectura:
  ```
  synapse_daemon (background process)
    ├── Unix socket: ~/.cache/SYNAPSE_/synapse.sock
    ├── PTY pool: HashMap<SessionId, Vec<Pane>>
    ├── Protocolo: JSON-line sobre Unix socket
    └── Cada "ventana" de synapse_ es un cliente que se conecta al daemon
  ```
- [ ] Comandos del protocolo:
  - `attach <session_id>` — conectar ventana a sesión existente
  - `detach` — desconectar sin matar sesión
  - `list` — listar sesiones activas
  - `kill <session_id>` — matar sesión
  - `new <name>` — crear sesión nueva
- [ ] Modo headless: `synapse_ daemon start` lanza el daemon
- [ ] Cliente: `synapse_ attach <session_id>` conecta una ventana al daemon
- [ ] El daemon forwardea output del PTY a todos los clientes conectados, y forwardea input de cualquier cliente al PTY
- [ ] Single-instance: si ya hay una ventana conectada, `synapse_` envía comando al daemon en vez de abrir nueva (configurable)
- [ ] Formato de frame: grid cells + cursor position serializado (no raw PTY bytes, porque cada cliente puede tener viewport distinto)
- [ ] Scrollback compartido (o independiente por cliente)
- [ ] Test: `synapse_ daemon start`, `synapse_ attach`, `Ctrl+Alt+D` detach, `synapse_ attach` reconecta

**Archivos:** nuevo `crates/SYNAPSE_-daemon/` (crate workspace member), `crates/SYNAPSE_-app/src/client.rs`, `crates/SYNAPSE_-app/src/main.rs`

---

## M-024: IPC daemon + CLI commands

**Prioridad:** P3 | **Esfuerzo:** 1-2 semanas | **Dependencias:** M-023 (el daemon es el backend natural)

Comandos CLI para controlar SYNAPSE_ desde scripts:

```bash
synapse_ list               # listar sesiones activas
synapse_ send "ls -la"      # enviar comando a sesión activa
synapse_ send -s dev "vim"  # enviar a sesión "dev"
synapse_ kill dev            # matar sesión
synapse_ new dev             # crear sesión nueva
synapse_ attach dev          # conectar a sesión existente
synapse_ capture             # tomar screenshot de la sesión actual (PNG)
```

- [ ] Sobre el mismo Unix socket del daemon (M-023)
- [ ] `synapse_ send` inyecta texto en el PTY de la sesión activa
- [ ] `synapse_ capture` renderiza un frame PNG usando el renderer (sin necesidad de ventana)
- [ ] Salida JSON para `list` (consumible por scripts / waybar / polybar)
- [ ] Test: `synapse_ list` desde otro terminal muestra sesiones

**Archivos:** mismo que M-023

---

## M-025: Wayland CSD + transparencia

**Prioridad:** P3 | **Esfuerzo:** 3-5 días | **Dependencias:** ninguna

Client-side decorations en Wayland y soporte de transparencia de ventana.

- [ ] En `main.rs`, al crear window, detectar si es Wayland (`WINIT_UNIX_BACKEND=wayland`)
- [ ] Si Wayland: `WindowAttributes::default().with_decorations(false)` + dibujar barra de título custom (minimal, 24px) con botones close/minimize/maximize
- [ ] Transparencia: `with_transparent(true)` en WindowAttributes
- [ ] Config: `window_opacity: f32` (0.0-1.0), `window_blur: bool` (si el compositor lo soporta)
- [ ] La transparencia requiere que el clear color del renderer use alpha < 1.0
- [ ] Test: abrir en sway/hyprland, verificar que se ve el wallpaper a través de la terminal

**Archivos:** `crates/SYNAPSE_-app/src/main.rs`, `crates/SYNAPSE_-renderer/src/renderer.rs`, `crates/SYNAPSE_-config/src/config.rs`

---

## M-026: Color emoji (COLRv1/CBDT)

**Prioridad:** P3 | **Esfuerzo:** 5-7 días | **Dependencias:** M-010 (font fallback)

Renderizar emojis en color usando COLRv1 o tablas CBDT/CBLC.

- [ ] fontdue no soporta COLRv1 ni CBDT. Necesitamos alternativa:
  - Opción A: crate `skrifa` (Google Fonts) para parsear COLRv1 + pintar en GPU
  - Opción B: crate `ttf-parser` + `colrv1` feature (más ligero pero menos maduro)
  - Opción C: crate `emoji` + `fontdb` + `tiny-skia` para rasterizar emoji → RGBA → atlas
- [ ] En `atlas.rs:get_or_insert(codepoint)`, si es emoji (rango Unicode U+1F300-U+1FAFF, U+2600-U+27BF, etc.):
  - Si la font fallback tiene tabla COLR/CBDT, rasterizar en color
  - Si no, rasterizar en grayscale como glifo normal
- [ ] Integrar con font fallback chain: `"Noto Color Emoji"` como última en la cadena
- [ ] Test: `echo "🚀🔥💻🎯🦀"` debe mostrar emojis a color

**Archivos:** `crates/SYNAPSE_-renderer/src/atlas.rs`, `crates/SYNAPSE_-renderer/src/text.rs`, `Cargo.toml`

---

## M-027: High contrast theme + reduce_motion

**Prioridad:** P3 | **Esfuerzo:** 2 horas | **Dependencias:** ninguna

Modo de accesibilidad básico.

- [ ] Añadir theme built-in `high-contrast`: `bg=#000000`, `fg=#FFFFFF`, `accent=#FFFF00`, resto en blanco/negro puro
- [ ] Config: `reduce_motion: bool` — si true, deshabilita todas las animaciones (splash, cursor blink, slide animations, pane pulse, resize indicators)
- [ ] En cada lugar donde hay animación, chequear `config.reduce_motion` y saltar
- [ ] Test: activar reduce_motion, verificar que la UI es instantánea sin animaciones

**Archivos:** `crates/SYNAPSE_-config/src/themes.rs`, `crates/SYNAPSE_-config/src/config.rs`, `crates/SYNAPSE_-app/src/render.rs`

---

## M-028: Scrollbar / minimap

**Prioridad:** P3 | **Esfuerzo:** 1-2 días | **Dependencias:** M-016 (selection v2 para interacción)

Barra de scroll vertical o minimapa lateral para navegar el scrollback.

- [ ] Config: `scrollbar: "none" | "thin" | "minimap"` (default "thin")
- [ ] Modo "thin": barra de 4px en el borde derecho, altura proporcional al viewport/scrollback
- [ ] Modo "minimap": columna estrecha (20-30px) con representación reducida del buffer (1px por línea, color basado en densidad de texto)
- [ ] Click en scrollbar: saltar a esa posición del scrollback
- [ ] Drag en scrollbar: scroll continuo
- [ ] Interacción solo con mouse, no interfiere con selección de texto
- [ ] Test: generar 500 líneas de output, verificar que la scrollbar aparece y es interactiva

**Archivos:** `crates/SYNAPSE_-app/src/render.rs`, `crates/SYNAPSE_-app/src/mouse.rs`, `crates/SYNAPSE_-config/src/config.rs`

---

## M-029: Pane watermark / badge

**Prioridad:** P3 | **Esfuerzo:** 1 día | **Dependencias:** M-022 (background image, misma zona)

Texto semi-transparente en el fondo del pane mostrando el nombre del pane/CWD/tab.

- [ ] Config: `pane_badge: bool` (default false)
- [ ] Si activo: renderizar texto del CWD o título del pane en grande (font 48px+), centrado, con opacidad 0.05, detrás de las celdas
- [ ] Formato configurable: `pane_badge_format: "{cwd}"` o `"{title}"` o `"{user}@{host}"`
- [ ] Se actualiza cuando cambia el CWD (OSC 7) o el título (OSC 0/2)
- [ ] Test: activar badge, verificar que se ve el path en el fondo del pane

**Archivos:** `crates/SYNAPSE_-app/src/render.rs`, `crates/SYNAPSE_-config/src/config.rs`

---

## M-030: Shell integration scripts

**Prioridad:** P3 | **Esfuerzo:** 2-3 días | **Dependencias:** M-018 (profiler), M-019 (history)

Scripts oficiales de integración para zsh, bash, y fish que envíen OSC sequences para features avanzados.

- [ ] Script `synapse-integration.zsh` / `.bash` / `.fish` que se sourcea desde `.zshrc`
- [ ] Funcionalidades:
  - `preexec()` hook: envía OSC 133 A (prompt start) + metadata del comando
  - `precmd()` hook: envía OSC 133 C (command exit) + exit code + timestamp
  - Cambio de directorio: envía OSC 7 file://host/path
  - Notificaciones de comandos largos: si un comando dura >30s, envía OSC 9/777 para notificar
  - Variable `SYNAPSE_INSIDE` para detección (similar a `KITTY_INSTALLATION_DIR`)
- [ ] Empaquetar scripts en `~/.config/SYNAPSE_/shell/` al primer arranque
- [ ] Comando `synapse_ setup` o `synapse_ init` que añade el source automáticamente a `.zshrc`
- [ ] Test: sourcear el script en zsh, `cd /tmp`, verificar que la status bar muestra `/tmp`

**Archivos:** nuevos `assets/shell/synapse-integration.zsh`, `assets/shell/synapse-integration.bash`, `assets/shell/synapse-integration.fish`, `crates/SYNAPSE_-app/src/pane_ops.rs`

---

## Resumen por esfuerzo

| ID | Feature | Esfuerzo | Prioridad |
|----|---------|----------|-----------|
| M-001 | Ligaduras ON por defecto | 5 min | P0 |
| M-002 | Pane Zoom | 1 día | P0 |
| M-003 | Broadcast Input | 2h | P0 |
| M-004 | Drag & Drop | 2h | P0 |
| M-005 | CLI args | 1 día | P0 |
| M-006 | Search regex | 2h | P0 |
| M-007 | Path detection clickable | 1 día | P1 |
| M-008 | Sixel decoder | 3-5d | P1 |
| M-009 | iTerm2 OSC 1337 | 2d | P1 |
| M-010 | Font fallback | 3-5d | P1 |
| M-011 | Sesiones save/restore | 2d | P1 |
| M-012 | Quake Mode | 3-4d | P1 |
| M-013 | Atlas LRU | 2-3d | P1 |
| M-014 | Background tab freeze | 2d | P1 |
| M-015 | Damage tracking real | 2-3d | P1 |
| M-016 | Selection v2 | 1d | P1 |
| M-017 | Workspaces | 3-5d | P2 |
| M-018 | Profiler overlay | 2d | P2 |
| M-019 | History persistente | 2-3d | P2 |
| M-020 | Plugin system TOML | 3-5d | P2 |
| M-021 | Recording .cast | 1-2w | P2 |
| M-022 | Background image | 1-2d | P2 |
| M-023 | Multiplex daemon | 2-3w | P3 |
| M-024 | IPC CLI | 1-2w | P3 |
| M-025 | Wayland CSD + transparencia | 3-5d | P3 |
| M-026 | Color emoji | 5-7d | P3 |
| M-027 | High contrast + reduce_motion | 2h | P3 |
| M-028 | Scrollbar / minimap | 1-2d | P3 |
| M-029 | Pane badge | 1d | P3 |
| M-030 | Shell integration scripts | 2-3d | P3 |

---

## Progreso global

- [ ] **Fase 1: P0 (6 mejoras, ~3 días)** — Ligaduras, Pane Zoom, Broadcast, Drag & Drop, CLI args, Search regex
- [ ] **Fase 2: P1 (10 mejoras, ~15-25 días)** — Path detection, Sixel, iTerm2 images, Font fallback, Sesiones, Quake, Atlas LRU, Tab freeze, Damage tracking, Selection v2
- [ ] **Fase 3: P2 (6 mejoras, ~12-20 días)** — Workspaces, Profiler, History, Plugins, Recording, Background images
- [ ] **Fase 4: P3 (8 mejoras, ~20-35 días)** — Multiplex, IPC, Wayland CSD, Emoji color, A11y, Scrollbar, Badge, Shell integration

---

*"La terminal más completa del mercado: open source, GPU-accelerated, Rust puro, sin telemetría."*
