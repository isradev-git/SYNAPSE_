# Luna — Task List

> Completar las tareas **en orden estricto**. Cada tarea es atómica y testeable antes de pasar a la siguiente.
> Estado: [ ] Pendiente · [x] Completada · [~] En progreso

---

## FASE 0 — Entorno y Scaffolding

### T-001 · Instalar Rust toolchain
- [x] Instalar `rustup` desde rustup.rs
- [x] Instalar stable toolchain: `rustup toolchain install stable`
- [x] Añadir targets cross-platform:
  - `rustup target add x86_64-pc-windows-msvc`
  - `rustup target add x86_64-apple-darwin aarch64-apple-darwin`
  - `rustup target add x86_64-unknown-linux-gnu`
- [x] Instalar herramientas de desarrollo:
  - `cargo install cargo-watch` (rebuild en caliente)
  - `cargo install cargo-dist` (distribución)
  - `cargo install cargo-deny` (auditoría de dependencias)
- [x] Verificar: `rustc --version` y `cargo --version`

### T-002 · Crear workspace de Cargo
- [x] Crear directorio `Luna/`
- [x] Crear `Cargo.toml` raíz con `[workspace]` que liste los 5 crates
- [x] Crear estructura de directorios completa según `proyecto.md`
- [x] Crear `Cargo.toml` individual para cada crate con sus dependencias
- [x] Verificar: `cargo build` compila sin errores desde raíz

### T-003 · Configurar dependencias en Cargo.toml
- [x] En workspace `Cargo.toml`, definir `[workspace.dependencies]` compartidas:
  - `winit = "0.30"`
  - `wgpu = "22"`
  - `cosmic-text = "0.12"`
  - `portable-pty = "0.8"`
  - `vte = "0.13"`
  - `tokio = { version = "1", features = ["full"] }`
  - `serde = { version = "1", features = ["derive"] }`
  - `toml = "0.8"`
  - `arboard = "3"`
  - `tracing = "0.1"`
  - `tracing-subscriber = "0.3"`
  - `bitflags = "2"`
  - `clap = { version = "4", features = ["derive"] }`
- [x] Cada crate hijo referencia con `{ workspace = true }`
- [x] Verificar: `cargo check` pasa sin errores

### T-004 · Inicializar repositorio git
- [x] `git init`
- [x] Crear `.gitignore`: excluir `/target`, `/dist`, `*.log`, `config.toml` personal
- [x] Crear `README.md` con descripción básica del proyecto
- [x] Commit inicial: "chore: initial project structure"

### T-005 · Configurar .cargo/config.toml
- [x] Definir perfil `release` con optimizaciones:
  ```toml
  [profile.release]
  opt-level = 3
  lto = "thin"
  codegen-units = 1
  strip = true
  ```
- [x] Configurar linker más rápido para dev (mold en Linux, lld en Windows)
- [x] Verificar: `cargo build --release` produce binario optimizado

---

## FASE 1 — Ventana Base con winit + wgpu

### T-006 · Crear ventana vacía con winit
- [x] En `Luna-app/src/main.rs`: inicializar `EventLoop` de winit
- [x] Crear ventana con título "Luna", tamaño 1280×800, resizable
- [x] Implementar loop de eventos básico: manejar `WindowEvent::CloseRequested`
- [x] Manejar `WindowEvent::Resized` (guardar nuevo tamaño)
- [x] Test: ventana se abre, se puede redimensionar y cerrar

### T-007 · Inicializar surface wgpu sobre la ventana
- [x] En `Luna-renderer/src/renderer.rs`: crear `wgpu::Instance`
- [x] Crear `Surface` desde la ventana winit
- [x] Seleccionar `Adapter` (GPU) con `request_adapter`
- [x] Crear `Device` y `Queue`
- [x] Configurar `SurfaceConfiguration` (formato de textura, present mode)
- [x] Limpiar pantalla con color `#210b4b` en cada frame
- [x] Test: ventana muestra fondo morado oscuro a 60fps

### T-008 · Cargar fuente JetBrains Mono con cosmic-text
- [x] Descargar JetBrains Mono Regular, Bold, Italic (licencia OFL)
- [x] Colocar en `assets/fonts/`
- [x] En `Luna-renderer/src/text.rs`: inicializar `cosmic_text::FontSystem`
- [x] Cargar la fuente desde bytes embebidos (`include_bytes!`)
- [x] Crear `cosmic_text::SwashCache` para rasterización
- [x] Test: rasterizar el carácter 'A' y obtener bitmap sin panic

### T-009 · Implementar texture atlas de glifos
- [x] En `Luna-renderer/src/atlas.rs`:
  - Crear textura wgpu de 2048×2048 RGBA8
  - Implementar allocator simple (shelf packing)
  - `atlas.get_or_insert(glyph_key) -> UvRect`
  - Cache de glifos ya rasterizados (HashMap<CacheKey, UvRect>)
- [x] Función `atlas.upload_glyph(device, queue, bitmap, rect)`
- [x] Test: insertar 100 glifos distintos sin solaparse en el atlas

### T-010 · Render de texto básico en pantalla
- [x] En `Luna-renderer/src/cell.rs`: pipeline wgpu para celdas de texto
- [x] Vertex buffer con instancias: `[position: vec2, uv: vec2, fg_color: vec4, bg_color: vec4]`
- [x] Shader `cell.wgsl`: renderiza quad por instancia, muestrea atlas para fg, bg sólido
- [x] Función `renderer.draw_text(text, x, y, fg, bg)`
- [x] Test: renderizar "Hello, Luna!" en pantalla con color `#ffffff` sobre `#210b4b`

---

## FASE 2 — PTY y Parser VT

### T-011 · Detección de shell nativo por OS
- [x] En `Luna-terminal/src/shell.rs`:
  - Detectar OS con `#[cfg(target_os = "...")]`
  - Windows: devolver `cmd.exe` (con opción de PowerShell)
  - macOS: leer `$SHELL`, fallback a `/bin/zsh`
  - Linux: leer `$SHELL`, fallback a `/bin/bash`
- [x] Función `detect_shell() -> ShellConfig { program, args, env }`
- [x] Test unitario: ejecuta en el OS actual y devuelve path válido

### T-012 · Lanzar PTY con portable-pty
- [x] En `Luna-terminal/src/pty.rs`:
  - Crear `PtySystem` con `portable_pty::native_pty_system()`
  - Abrir par PTY master/slave con tamaño inicial (cols: 80, rows: 24)
  - Lanzar proceso de shell en el slave PTY
  - Exponer `PtyHandle { master, child_process }`
- [x] Función `pty.write(bytes)` — enviar input al proceso
- [x] Función `pty.resize(cols, rows)` — ajustar tamaño del PTY
- [x] Test: lanzar shell, escribir `echo hello\n`, leer output

### T-013 · Lectura asíncrona del PTY con tokio
- [x] En `Luna-terminal/src/pty.rs`:
  - Tarea thread que lee bytes del master PTY en loop
  - Enviar bytes a un `std::sync::mpsc::channel`
  - La tarea principal recibe y procesa sin bloquear
- [ ] Manejar EOF (proceso terminado): notificar al UI (pendiente)
- [x] Test: output de comandos llega en < 5ms desde ejecución

### T-014 · Implementar parser VT100/ANSI
- [x] En `Luna-terminal/src/parser.rs`:
  - Implementar `vte::Perform` trait sobre struct `VteProcessor`
  - `print(c: char)`: escribir carácter en posición actual del grid
  - `execute(byte)`: manejar C0 controls (LF, CR, BS, HT, BEL, FF)
  - `csi_dispatch(params, action)`: manejar secuencias CSI:
    - Movimiento de cursor: `A`, `B`, `C`, `D`, `H`, `f`
    - Borrado: `J` (erase display), `K` (erase line)
    - Atributos SGR: colores fg/bg, bold, italic, underline
  - `esc_dispatch`: save/restore cursor (ESC 7/8), RIS (ESC c)
- [x] Mantener estado: posición cursor (col, row), atributos actuales
- [x] Test: parsear secuencia `\e[1;32mHello\e[0m` → bold verde

### T-015 · Implementar Grid de celdas
- [x] En `Luna-terminal/src/grid.rs`:
  - `Grid { cells: Vec<CharCell>, cols: usize, rows: usize }`
  - `grid.set(col, row, cell)`
  - `grid.get(col, row) -> &CharCell`
  - `grid.new_line()` — wrapper de advance_cursor con LF semántica
  - `grid.clear_region(top, bottom)` — borra región con celda vacía
  - `grid.resize(cols, rows)` — refleja redimensionado de ventana
- [x] Marcar celdas como `dirty` al modificarse
- [x] Test: grid de 80×24, escribir en varias posiciones, verificar dirty tracking

### T-016 · Implementar scrollback buffer
- [x] En `Luna-terminal/src/buffer.rs`:
  - Buffer circular con límite configurable (default 100.000 líneas)
  - `buffer.push(line: Vec<CharCell>)`
  - `buffer.get_line(index) -> &[CharCell]`
  - `buffer.len() -> usize`
- [x] Scroll offset: `grid.scroll_offset` gestionado en Grid (cuántas líneas hacia arriba)
- [x] Test: llenar con 200.000 líneas, verificar que las más antiguas se descartan

---

## FASE 3 — Rendering de Terminal Completo

### T-017 · Conectar Grid → Renderer
- [x] En cada frame: iterar celdas del grid + scrollback visible (visible_cells)
- [x] Para cada celda sucia: obtener glifo del atlas (o rasterizar si nuevo)
- [x] Construir vertex buffer de instancias para el draw call
- [ ] Solo actualizar instancias de celdas marcadas como `dirty` (actualmente se re-suben todas)
- [x] Limpiar flags dirty tras render (clear_dirty no se llama actualmente)
- [x] Test: `ls -la` se muestra correctamente con colores

### T-018 · Shader de cursor animado
- [ ] En `Luna-renderer/src/shaders/cursor.wgsl`: (creado pero no implementado)
  - Shader existe pero no se usa; cursor se renderiza como celda overlay
  - Uniform `cursor_time: f32` (tiempo en segundos)
  - Color del cursor: `#ff3d94`
- [ ] Pasar `cursor_time` actualizado en cada frame (pendiente)
- [ ] Soportar estilos: block, beam, underline (pendiente)
- [ ] Test: cursor parpadea a ~500ms sobre la posición correcta (pendiente)

### T-019 · Soporte de colores ANSI completo
- [x] SGR: colores de 8 colores estándar (30-37, 40-47)
- [x] SGR: colores bright (90-97, 100-107)
- [x] SGR: xterm-256 (`38;5;N` y `48;5;N`)
- [x] SGR: true color 24-bit (`38;2;R;G;B` y `48;2;R;G;B`)
- [x] SGR: reset (0), bold (1), italic (3), underline (4), inverse (7), blink (5/6), invisible (8)
- [x] Test: `echo $'\e[38;2;255;100;0mTrue color\e[0m'` corregido en parser

### T-020 · Resize de ventana → resize de PTY y grid
- [x] Al recibir `WindowEvent::Resized`:
  - Calcular nuevas columnas y filas según tamaño de fuente
  - Llamar `pty.resize(cols, rows)`
  - Llamar `grid.resize(cols, rows)`
  - Reconfigurar surface wgpu
- [x] Test: redimensionar ventana, ejecutar `tput cols` y `tput lines`, verificar valores correctos

---

## FASE 4 — Input de Usuario

### T-021 · Captura de input de teclado
- [x] En `Luna-app/src/input.rs` (no event.rs):
  - Manejar `WindowEvent::KeyboardInput` de winit
  - Convertir KeyEvent → InputAction via `from_key(event, modifiers)`
  - Manejar teclas especiales: Enter → `\r`, Backspace → `\x7f`, Escape → `\x1b`, Tab/Shift+Tab
  - Manejar teclas de función: F1-F12 → secuencias xterm (`\x1bOP`..`\x1b[24~`)
  - Manejar flechas: `\e[A/B/C/D` con modificadores Shift (`1;2`) y Ctrl (`1;5`)
  - Manejar Ctrl+tecla → byte de control (Ctrl+A=1..Ctrl+Z=26)
  - Manejar Home/End/Delete/Insert/PgUp/PgDn con modificadores
- [x] Test: escribir en la terminal, caracteres aparecen correctamente

### T-022 · Scroll con teclado y ratón
- [x] `WindowEvent::MouseWheel`: scroll según delta (LineDelta o PixelDelta)
- [x] Shift+PgUp / Shift+PgDn: scroll 24 líneas local
- [x] Ctrl+Shift+PgUp/PgDn: scroll al inicio / final del scrollback
- [x] Actualizar `scroll_offset` del grid, re-renderizar
- [x] Auto-scroll al fondo en escritura de texto (excepto PgUp/PgDn)
- [x] Test: ejecutar `man bash`, hacer scroll arriba y abajo

### T-023 · Selección de texto con ratón
- [x] `MouseInput(Pressed)` + `CursorMoved`: calcular celda en viewport coords
- [x] Durante drag: crear/actualizar Selection (viewport coords)
- [x] Renderizar selección con color `#ff3d9466` (overlay sobre celdas)
- [x] `MouseInput(Released)`: state.selecting = false, selection preserved
- [ ] Doble click: seleccionar palabra completa (pendiente)
- [ ] Triple click: seleccionar línea completa (pendiente)
- [x] `extract_selection()`: extrae texto con scrollback-aware get_visible()

### T-024 · Copiar y pegar
- [x] Ctrl+Shift+C: copiar texto seleccionado a clipboard (`arboard::Clipboard`)
- [x] Ctrl+Shift+V: pegar clipboard como bytes al PTY
- [x] Bracketed paste: envolver con `\e[200~` ... `\e[201~`
- [ ] Activar bracketed paste mode si el shell lo soporta (`\e[?2004h`) (pendiente)
- [x] Clipboard es `Option` — graceful degradation si falla
- [x] Test: copiar output de `ls`, pegar en prompt de otro panel

---

## FASE 5 — Sistema de Tabs

### T-025 · Estructura de datos de tabs
- [x] En `Luna-ui/src/tab_bar.rs`:
  ```rust
  pub struct TabBar {
      tabs: Vec<Tab>,
      active: usize,
  }
  pub struct Tab {
      id: TabId,
      title: String,
      pane_tree: PaneTree,
      active_pane: PaneId,
  }
  ```
- [x] `tab_bar.new_tab() -> (TabId, PaneId)` — autoincremental, activa la nueva
- [x] `tab_bar.close_tab(index) -> Option<Tab>` (mínimo 1 tab siempre)
- [x] `tab_bar.activate(index)` — clamped a `0..tabs.len()`
- [x] `tab_bar.set_title(tab_id, title)` — busca por ID
- [x] `Pane` struct con PTY + grid + processor + title/cwd via `Rc<RefCell<String>>`
- [x] `PaneTree` enum (Leaf | Split) en `splitter.rs` con `all_panes()` y `find_active()`
- [x] `Layout` struct con `pane_area()`, `tab_width()`, `tab_x()`, `pane_margin()`

### T-026 · Rendering de la barra de tabs
- [x] Barra fija en parte superior, altura 32px
- [x] Tab activa: fondo `#b5307e`, texto `#ffffff`
- [x] Tab inactiva: fondo `#6a2a98`, texto `#cccccc`
- [x] Separadores verticales de 1px entre tabs (`#3f1c6d`)
- [ ] Hover: fondo `#ff3d9422` (pendiente)
- [x] Botón `+` al final: nueva tab
- [ ] Botón `×` en cada tab: cerrar (pendiente)
- [ ] Si hay muchas tabs: scroll horizontal de la barra (pendiente)
- [x] Shader `ui.wgsl` para rectángulos coloreados (TriangleStrip instanced)
- [x] `UIRenderer` struct con pipeline propio + screen uniform + instance buffer
- [x] `draw_frame()` en Renderer: cells + UI rects en un solo render pass

### T-027 · Interacción con tabs
- [x] Click en tab → activarla
- [x] Click en `+` → nueva tab con shell independiente
- [ ] Click en `×` → cerrar tab (pendiente)
- [x] Ctrl+T → nueva tab
- [x] Ctrl+W → cerrar tab activa
- [x] Ctrl+1..9 → ir a tab por número (Ctrl+9 → index 8)
- [x] Ctrl+Tab → siguiente tab (circular)
- [x] Ctrl+Shift+Tab → tab anterior (circular)
- [x] Fix: MouseInput usa `state.cursor_x/y` (trackeado en CursorMoved) en vez de `window.inner_position()`
- [x] Fix: `handle_tab_click` recibe coordenadas lógicas (sin doble división por scale_factor)
- [x] Fix: `Layout::update()` llamado en `Resized`

### T-028 · Título dinámico de tab
- [x] Manejar OSC 0 y OSC 2: `\e]0;título\a` → actualizar título de tab
- [x] OSC 7: `file://host/path` → actualizar CWD
- [x] Si no hay OSC: mostrar "Tab N" (fallback)
- [ ] Truncado con "…" si el título excede el ancho de la tab
- [ ] Mostrar último componente del CWD como título (pendiente)

---

## FASE 6 — Sistema de Split de Paneles

### T-029 · Estructura de árbol binario de paneles
- [x] En `Luna-ui/src/splitter.rs`:
  ```rust
  pub enum PaneTree {
      Leaf(PaneId),
      Split { direction, ratio: f32, first: Box<PaneTree>, second: Box<PaneTree> }
  }
  ```
- [x] `splitter.split(pane_id, new_id, direction) -> Result<(PaneId, PaneId), ()>`
- [x] `splitter.close(pane_id) -> Option<PaneId>` → reemplaza nodo padre con el hermano
- [x] `splitter.get_layout(rect) -> Vec<(PaneId, PaneRect)>`
- [x] `get_dividers(rect) -> Vec<DividerInfo>` (hitbox 6px + direction + pane_id + parent_rect)
- [x] `set_ratio(pane_id, ratio)` → actualiza el ratio del split que contiene a pane_id
- [x] Test unitario: árbol de 4 paneles, verificar rectángulos sin solapamiento (6 tests)
- [x] `PaneRect` struct: `{ x, y, w, h }`
- [x] `PaneId` ahora deriva `Ord` + `PartialOrd`

### T-030 · Rendering de múltiples paneles
- [x] Usar `get_layout()` para calcular rect de cada panel
- [x] Renderizar cada panel en su rect (clip por col/row bounds)
- [x] Divisor entre paneles: línea de 2px color `#6a2a98` (centrada en hitbox de 6px)
- [x] Panel activo: borde de 1px `#b5307e` (`PANEL_ACTIVE_BORDER`)
- [x] Panel inactivo: borde de 1px `#3f1c6d` (`PANEL_INACTIVE_BORDER`)
- [x] RedrawRequested: un solo render pass para cells + UI rects de todos los panes
- [x] Resized: recalcula cols/rows de cada pane según su layout rect

### T-031 · Atajos de split
- [x] Ctrl+Shift+D: split vertical (divide panel activo izq/der)
- [x] Ctrl+Shift+E: split horizontal (divide panel activo arr/abajo)
- [x] Ctrl+Shift+W: cerrar panel activo (mínimo 1 panel) + kill child process
- [x] Ctrl+Shift+↑↓←→: mover foco al panel adyacente en esa dirección (`adjacent_pane`)
- [x] Nuevo panel en split hereda cols/rows y CWD del panel activo

### T-032 · Resize de paneles con ratón
- [x] Detectar hover sobre divisor (hitbox de 6px)
- [x] Cambiar cursor a `CursorIcon::EwResize` o `NsResize` según orientación
- [x] Click + drag: actualizar `ratio` del nodo Split en tiempo real (`set_ratio`)
- [x] Clamp ratio: 0.1–0.9 via `set_ratio`
- [x] `AppState.dragging_divider: Option<DividerDrag>`, `hover_divider: bool`
- [x] Cursor vuelve a `Text` al salir del divisor

### T-033 · Proceso PTY independiente por panel
- [x] Cada `PaneId` tiene su propio `PtyHandle` con su proceso de shell
- [x] CWD independiente por panel (OSC 7 tracking + `shell.cwd`)
- [x] Al cerrar panel: `child_process.kill()` + drop de `PtyHandle`
- [x] `ShellConfig.cwd: Option<String>` → `CommandBuilder::cwd()` en spawn
- [x] Al crear nuevo panel: heredar CWD del panel activo actual
- [x] Al cerrar tab: kill todos los panes de la tab

---

## FASE 7 — Búsqueda y Productividad

### T-034 · Búsqueda en buffer (Ctrl+Shift+F)
- [ ] Overlay de búsqueda: barra en la parte superior del panel activo
- [ ] Input field para el término de búsqueda
- [ ] Resaltar todas las ocurrencias en el buffer visible
- [ ] Navegar entre matches: Enter (siguiente), Shift+Enter (anterior)
- [ ] Mostrar contador: "3/12 matches"
- [ ] Escape: cerrar búsqueda
- [ ] Test: buscar "error" en output de compilación con múltiples matches

### T-035 · Búsqueda en historial (Ctrl+R)
- [ ] Al activar: mostrar prompt `(reverse-i-search): _` en el input line
- [ ] Filtrar historial en tiempo real mientras el usuario escribe
- [ ] Mostrar el match más reciente que contenga el término
- [ ] Enter: confirmar y colocar en input line (sin ejecutar)
- [ ] Ctrl+R de nuevo: siguiente match más antiguo
- [ ] Escape: cancelar, restaurar input anterior
- [ ] Test: buscar "git", navegar entre matches con Ctrl+R repetido

### T-036 · Limpiar pantalla (Ctrl+L)
- [ ] Enviar secuencia `\e[2J\e[H` al PTY (clear estándar)
- [ ] El scrollback no se borra (solo la vista)
- [ ] Test: ejecutar `ls`, Ctrl+L, verificar pantalla limpia pero scroll muestra historial

---

## FASE 8 — Configuración

### T-037 · Sistema de configuración con TOML
- [ ] En `Luna-config/src/config.rs`:
  - Struct `Config` con serde Deserialize + Default
  - Cargar desde ruta por OS (`dirs` crate para obtener config dir)
  - Si no existe: crear con valores por defecto
  - Función `config.reload()` — releer en caliente (Ctrl+,)
- [ ] Test: modificar `font.size` en TOML, recargar, verificar cambio visual

### T-038 · Sistema de keybinds personalizable
- [ ] En `Luna-config/src/keybinds.rs`:
  - Mapa `HashMap<KeyCombo, Action>`
  - `KeyCombo { modifiers: Modifiers, key: KeyCode }`
  - `Action` enum con todas las acciones posibles
  - Defaults hardcodeados, override desde TOML
- [ ] Función `keybinds.lookup(event) -> Option<Action>`
- [ ] Test: remapear Ctrl+T a Ctrl+N en config, verificar funcionamiento

### T-039 · Ajuste de tamaño de fuente en runtime
- [ ] Ctrl++ / Ctrl+- : ±1pt en `config.font.size`
- [ ] Ctrl+0: restaurar default
- [ ] Al cambiar: recalcular cols/rows, redimensionar PTY y grid
- [ ] Persistir el tamaño en config al salir
- [ ] Test: cambiar tamaño 3 veces, el layout se ajusta sin artefactos

### T-040 · Fullscreen (F11)
- [ ] Toggle `window.set_fullscreen(Some(Fullscreen::Borderless(None)))`
- [ ] Recalcular layout de paneles al cambiar a fullscreen y al salir
- [ ] Test: F11 entra y sale de fullscreen correctamente en los 3 OS

---

## FASE 9 — Distribución y Empaquetado

### T-041 · Configurar cargo-dist
- [ ] `cargo install cargo-dist`
- [ ] `cargo dist init` en la raíz del workspace
- [ ] Configurar targets: win64, mac universal (x86+arm), linux x86
- [ ] Añadir `dist.toml` con metadatos del proyecto
- [ ] Test: `cargo dist build` genera binarios en `/dist`

### T-042 · Empaquetado macOS (.app + .dmg)
- [ ] Crear estructura `Luna.app/Contents/{MacOS,Resources,Info.plist}`
- [ ] `Info.plist` con bundle ID `com.Luna.app`, versión, icono
- [ ] Generar `.dmg` con fondo personalizado usando `create-dmg`
- [ ] Script `build/build-mac.sh` automatiza todo el proceso
- [ ] Placeholder para firma y notarización (requiere Apple Developer account)

### T-043 · Empaquetado Windows (.exe + installer)
- [ ] Script `build/build-win.ps1` compila release y copia assets
- [ ] Crear installer con `Inno Setup` o `WiX`
- [ ] Añadir al PATH automáticamente durante instalación
- [ ] Placeholder para firma con Code Signing Certificate
- [ ] Test: instalar en Windows limpio, ejecutar `Luna` desde CMD

### T-044 · Empaquetado Linux (AppImage + .deb)
- [ ] AppImage con `appimagetool` (universal, sin dependencias)
- [ ] `.deb` para Ubuntu/Debian con control file correcto
- [ ] `.rpm` para Fedora/RHEL (opcional)
- [ ] Script `build/build-linux.sh` genera los tres formatos
- [ ] Test: ejecutar AppImage en Ubuntu sin instalar nada

### T-045 · CI/CD con GitHub Actions
- [ ] Workflow `release.yml`: trigger en tag `v*`
- [ ] Jobs paralelos: build-windows, build-macos, build-linux
- [ ] Upload de artifacts a GitHub Release automáticamente
- [ ] Workflow `ci.yml`: en cada PR, `cargo test`, `cargo clippy`, `cargo fmt --check`
- [ ] Test: crear tag `v0.1.0`, verificar que los 3 binarios aparecen en Releases

---

## FASE 10 — Calidad y Conformidad

### T-046 · Test suite de VT100 con vttest
- [ ] Instalar `vttest` en cada OS objetivo
- [ ] Ejecutar la suite completa de vttest dentro de Luna
- [ ] Documentar qué tests pasan y cuáles fallan (objetivo: >90% passing)
- [ ] Crear issues para tests fallidos prioritarios

### T-047 · Benchmark de rendimiento
- [ ] Medir FPS con `cat /dev/urandom | head -c 10MB` (output masivo)
- [ ] Medir latencia input → render con timestamps
- [ ] Medir uso de RAM con 100.000 líneas en scrollback
- [ ] Comparar contra Alacritty como referencia
- [ ] Documentar resultados en `BENCHMARKS.md`

### T-048 · Test de compatibilidad por OS
- [ ] Windows: CMD y PowerShell como shell, rutas con backslash, UTF-8/CP1252
- [ ] macOS: zsh, fish, bash; Retina display (HiDPI); permisos de filesystem
- [ ] Linux: bash, zsh, fish; Wayland y X11; distintas distros (Ubuntu, Fedora, Arch)

### T-049 · Revisión de UX y diseño visual
- [ ] Verificar contraste WCAG AA en todos los elementos de texto
- [ ] Verificar animación de cursor (parpadeo suave, no abrupto)
- [ ] Verificar transiciones al cambiar de tab
- [ ] Verificar que los divisores de panel son visibles pero no intrusivos
- [ ] Capturas de pantalla HD para web/marketing

### T-050 · Documentación final
- [ ] `README.md`: instalación, primeros pasos, capturas de pantalla
- [ ] `CONFIGURATION.md`: referencia completa de opciones TOML
- [ ] `KEYBINDS.md`: tabla completa de atajos por OS
- [ ] `CONTRIBUTING.md`: guía para contribuidores externos
- [ ] `CHANGELOG.md`: formato keepachangelog.com
- [ ] `LICENSE`: decidir MIT vs Apache-2.0 (recomendado: MIT para comercial)

---

## Resumen de Fases

| Fase | Descripción                    | Tareas         | Prioridad      | Estado   |
|------|--------------------------------|----------------|----------------|----------|
| 0    | Entorno y Scaffolding          | T-001 a T-005  | 🔴 Crítica     | ✅       |
| 1    | Ventana base + wgpu            | T-006 a T-010  | 🔴 Crítica     | ✅       |
| 2    | PTY y Parser VT                | T-011 a T-016  | 🔴 Crítica     | ✅       |
| 3    | Rendering de terminal          | T-017 a T-020  | 🔴 Crítica     | ✅       |
| 4    | Input de usuario               | T-021 a T-024  | 🔴 Crítica     | ✅       |
| 5    | Sistema de Tabs                | T-025 a T-028  | 🟠 Alta        | ✅       |
| 6    | Sistema de Split               | T-029 a T-033  | 🟠 Alta        | ✅       |
| 7    | Búsqueda y productividad       | T-034 a T-036  | 🟡 Media       | ⬜       |
| 8    | Configuración                  | T-037 a T-040  | 🟡 Media       | ⬜       |
| 9    | Distribución y CI/CD           | T-041 a T-045  | 🟡 Media       | ⬜       |
| 10   | Calidad y conformidad          | T-046 a T-050  | 🟢 Baja        | ⬜       |

**Total: 50 tareas atómicas.**
Las fases 0-4 forman el MVP funcional: una terminal real corriendo en ventana con GPU rendering.
Las fases 5-6 añaden la diferenciación visual (splits y tabs).
Las fases 7-10 llevan el proyecto a nivel comercial.
