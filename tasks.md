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
- [ ] En `Luna-terminal/src/shell.rs`:
  - Detectar OS con `#[cfg(target_os = "...")]`
  - Windows: devolver `cmd.exe` (con opción de PowerShell)
  - macOS: leer `$SHELL`, fallback a `/bin/zsh`
  - Linux: leer `$SHELL`, fallback a `/bin/bash`
- [ ] Función `detect_shell() -> ShellConfig { program, args, env }`
- [ ] Test unitario: ejecuta en el OS actual y devuelve path válido

### T-012 · Lanzar PTY con portable-pty
- [ ] En `Luna-terminal/src/pty.rs`:
  - Crear `PtySystem` con `portable_pty::native_pty_system()`
  - Abrir par PTY master/slave con tamaño inicial (cols: 80, rows: 24)
  - Lanzar proceso de shell en el slave PTY
  - Exponer `PtyHandle { master, child_process }`
- [ ] Función `pty.write(bytes)` — enviar input al proceso
- [ ] Función `pty.resize(cols, rows)` — ajustar tamaño del PTY
- [ ] Test: lanzar shell, escribir `echo hello\n`, leer output

### T-013 · Lectura asíncrona del PTY con tokio
- [ ] En `Luna-terminal/src/pty.rs`:
  - Tarea tokio que lee bytes del master PTY en loop
  - Enviar bytes a un `tokio::sync::mpsc::channel`
  - La tarea principal recibe y procesa sin bloquear
- [ ] Manejar EOF (proceso terminado): notificar al UI
- [ ] Test: output de comandos llega en < 5ms desde ejecución

### T-014 · Implementar parser VT100/ANSI
- [ ] En `Luna-terminal/src/parser.rs`:
  - Implementar `vte::Perform` trait sobre struct `VteProcessor`
  - `print(c: char)`: escribir carácter en posición actual del grid
  - `execute(byte)`: manejar C0 controls (LF, CR, BS, HT, BEL)
  - `csi_dispatch(params, action)`: manejar secuencias CSI:
    - Movimiento de cursor: `CUU`, `CUD`, `CUF`, `CUB`, `CUP`
    - Borrado: `ED` (erase display), `EL` (erase line)
    - Atributos SGR: colores fg/bg, bold, italic, underline
    - Scroll: `SU`, `SD`
- [ ] Mantener estado: posición cursor (col, row), atributos actuales
- [ ] Test: parsear secuencia `\e[1;32mHello\e[0m` → bold verde

### T-015 · Implementar Grid de celdas
- [ ] En `Luna-terminal/src/grid.rs`:
  - `Grid { cells: Vec<CharCell>, cols: usize, rows: usize }`
  - `grid.set(col, row, cell)`
  - `grid.get(col, row) -> &CharCell`
  - `grid.scroll_up(n)` — mueve líneas al scrollback buffer
  - `grid.clear_region(top, bottom)` — borra región con celda vacía
  - `grid.resize(cols, rows)` — refleja redimensionado de ventana
- [ ] Marcar celdas como `dirty` al modificarse
- [ ] Test: grid de 80×24, escribir en varias posiciones, verificar dirty tracking

### T-016 · Implementar scrollback buffer
- [ ] En `Luna-terminal/src/buffer.rs`:
  - Buffer circular con límite configurable (default 100.000 líneas)
  - `buffer.push_line(line: Vec<CharCell>)`
  - `buffer.get_line(index) -> Option<&[CharCell]>`
  - `buffer.len() -> usize`
- [ ] Scroll offset: `buffer.scroll_offset` (cuántas líneas hacia arriba)
- [ ] Test: llenar con 200.000 líneas, verificar que las más antiguas se descartan

---

## FASE 3 — Rendering de Terminal Completo

### T-017 · Conectar Grid → Renderer
- [ ] En cada frame: iterar celdas del grid + scrollback visible
- [ ] Para cada celda sucia: obtener glifo del atlas (o rasterizar si nuevo)
- [ ] Construir vertex buffer de instancias para el draw call
- [ ] Solo actualizar instancias de celdas marcadas como `dirty`
- [ ] Limpiar flags dirty tras render
- [ ] Test: `ls -la` se muestra correctamente con colores

### T-018 · Shader de cursor animado
- [ ] En `Luna-renderer/src/shaders/cursor.wgsl`:
  - Uniform `cursor_time: f32` (tiempo en segundos)
  - Alpha = sin(time * π / blink_interval) para parpadeo suave
  - Color del cursor: `#ff3d94`
- [ ] Pasar `cursor_time` actualizado en cada frame
- [ ] Soportar estilos: block, beam, underline (configurable)
- [ ] Test: cursor parpadea a ~500ms sobre la posición correcta

### T-019 · Soporte de colores ANSI completo
- [ ] SGR: colores de 8 colores estándar (30-37, 40-47)
- [ ] SGR: colores bright (90-97, 100-107)
- [ ] SGR: xterm-256 (`38;5;N` y `48;5;N`)
- [ ] SGR: true color 24-bit (`38;2;R;G;B` y `48;2;R;G;B`)
- [ ] SGR: reset (0), bold (1), italic (3), underline (4), inverse (7)
- [ ] Test: ejecutar `echo $'\e[38;2;255;100;0mTrue color\e[0m'` y verificar color exacto

### T-020 · Resize de ventana → resize de PTY y grid
- [ ] Al recibir `WindowEvent::Resized`:
  - Calcular nuevas columnas y filas según tamaño de fuente
  - Llamar `pty.resize(cols, rows)`
  - Llamar `grid.resize(cols, rows)`
  - Reconfigurar surface wgpu
- [ ] Test: redimensionar ventana, ejecutar `tput cols` y `tput lines`, verificar valores correctos

---

## FASE 4 — Input de Usuario

### T-021 · Captura de input de teclado
- [ ] En `Luna-app/src/event.rs`:
  - Manejar `WindowEvent::KeyboardInput` de winit
  - Manejar `WindowEvent::ReceivedCharacter` (texto imprimible)
  - Convertir KeyCode → bytes para enviar al PTY
  - Manejar teclas especiales: Enter → `\r`, Backspace → `\x7f`, Escape → `\x1b`
  - Manejar teclas de función: F1-F12 → secuencias xterm
  - Manejar flechas: `\e[A/B/C/D`
  - Manejar Ctrl+tecla → byte de control (Ctrl+C → `\x03`)
- [ ] Test: escribir en la terminal, caracteres aparecen correctamente

### T-022 · Scroll con teclado y ratón
- [ ] `WindowEvent::MouseWheel`: scroll 3 líneas por tick
- [ ] PgUp / PgDn: scroll una pantalla completa
- [ ] Ctrl+Shift+PgUp/PgDn: scroll al inicio / final del buffer
- [ ] Actualizar `scroll_offset` del buffer, re-renderizar
- [ ] Test: ejecutar `man bash`, hacer scroll arriba y abajo

### T-023 · Selección de texto con ratón
- [ ] `MouseInput::Pressed` + `CursorMoved`: calcular celda de inicio
- [ ] Durante drag: calcular celda de fin, marcar rango como seleccionado
- [ ] Renderizar selección con color `#ff3d9466` (overlay sobre celdas)
- [ ] `MouseInput::Released`: fijar selección
- [ ] Doble click: seleccionar palabra completa
- [ ] Triple click: seleccionar línea completa
- [ ] Test: seleccionar texto, verificar highlight correcto

### T-024 · Copiar y pegar
- [ ] Ctrl+Shift+C: copiar texto seleccionado a clipboard (`arboard`)
- [ ] Ctrl+Shift+V: pegar clipboard como bytes al PTY
- [ ] Activar bracketed paste mode si el shell lo soporta (`\e[?2004h`)
- [ ] En bracketed paste: envolver con `\e[200~` ... `\e[201~`
- [ ] Test: copiar output de `ls`, pegar en prompt de otro panel

---

## FASE 5 — Sistema de Tabs

### T-025 · Estructura de datos de tabs
- [ ] En `Luna-ui/src/tab_bar.rs`:
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
- [ ] `tab_bar.new_tab() -> TabId`
- [ ] `tab_bar.close_tab(id)` (mínimo 1 tab siempre)
- [ ] `tab_bar.activate(id)`
- [ ] `tab_bar.set_title(id, title)`

### T-026 · Rendering de la barra de tabs
- [ ] Barra fija en parte superior, altura 32px
- [ ] Tab activa: fondo `#b5307e`, texto `#ffffff`, fuente bold
- [ ] Tab inactiva: fondo `#6a2a98`, texto `#cccccc`
- [ ] Hover: fondo `#ff3d9422`
- [ ] Botón `+` al final: nueva tab
- [ ] Botón `×` en cada tab: cerrar (visible en hover)
- [ ] Si hay muchas tabs: scroll horizontal de la barra
- [ ] Test visual: 5 tabs, activa resaltada correctamente

### T-027 · Interacción con tabs
- [ ] Click en tab → activarla
- [ ] Click en `+` → nueva tab
- [ ] Click en `×` → cerrar tab (confirmar si tiene proceso activo)
- [ ] Ctrl+T → nueva tab
- [ ] Ctrl+W → cerrar tab activa
- [ ] Ctrl+1..9 → ir a tab por número
- [ ] Ctrl+Tab → siguiente tab (circular)
- [ ] Ctrl+Shift+Tab → tab anterior (circular)
- [ ] Test: todos los atajos funcionan correctamente

### T-028 · Título dinámico de tab
- [ ] Manejar OSC 0 y OSC 2: `\e]0;título\a` → actualizar título de tab
- [ ] Si no hay OSC: mostrar el último componente del CWD
- [ ] Detectar CWD con OSC 7 (`file://host/path`) o leyendo `/proc/PID/cwd`
- [ ] Test: `cd /tmp && echo $PWD` → tab muestra "tmp"

---

## FASE 6 — Sistema de Split de Paneles

### T-029 · Estructura de árbol binario de paneles
- [ ] En `Luna-ui/src/splitter.rs`:
  ```rust
  pub enum PaneTree {
      Leaf(PaneId),
      Split { direction, ratio: f32, first: Box<PaneTree>, second: Box<PaneTree> }
  }
  ```
- [ ] `splitter.split(pane_id, direction) -> (PaneId, PaneId)`
- [ ] `splitter.close(pane_id)` → reemplaza nodo padre con el hermano
- [ ] `splitter.get_layout(tree, rect) -> Vec<(PaneId, Rect)>`
- [ ] Test unitario: árbol de 4 paneles, verificar rectángulos sin solapamiento

### T-030 · Rendering de múltiples paneles
- [ ] Usar `get_layout()` para calcular rect de cada panel
- [ ] Renderizar cada panel en su rect (clip rendering a ese rect)
- [ ] Divisor entre paneles: línea de 2px color `#6a2a98`
- [ ] Panel activo: borde de 1px `#ff3d94`
- [ ] Panel inactivo: borde de 1px `#3f1c6d`
- [ ] Test visual: 4 paneles con distintos colores de fondo, bordes correctos

### T-031 · Atajos de split
- [ ] Ctrl+Shift+D: split vertical (divide panel activo izq/der)
- [ ] Ctrl+Shift+E: split horizontal (divide panel activo arr/abajo)
- [ ] Ctrl+Shift+W: cerrar panel activo (mínimo 1 panel)
- [ ] Ctrl+Shift+↑↓←→: mover foco al panel adyacente en esa dirección
- [ ] Test: crear 6 paneles con splits, navegar entre ellos con teclado

### T-032 · Resize de paneles con ratón
- [ ] Detectar hover sobre divisor (hitbox de 6px)
- [ ] Cambiar cursor a `CursorIcon::EwResize` o `NsResize` según orientación
- [ ] Click + drag: actualizar `ratio` del nodo Split en tiempo real
- [ ] Clamp ratio: mínimo panel = 80px ancho y 60px alto
- [ ] Test: arrastrar divisor, paneles se redimensionan suavemente

### T-033 · Proceso PTY independiente por panel
- [ ] Cada `PaneId` tiene su propio `PtyHandle` con su proceso de shell
- [ ] CWD independiente por panel
- [ ] Al cerrar panel: `child_process.kill()` + drop de `PtyHandle`
- [ ] Al crear nuevo panel: heredar CWD del panel activo actual
- [ ] Test: `cd /tmp` en panel 1, abrir panel 2, verificar CWD diferente

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
| 2    | PTY y Parser VT                | T-011 a T-016  | 🔴 Crítica     | ⬜       |
| 3    | Rendering de terminal          | T-017 a T-020  | 🔴 Crítica     | ⬜       |
| 4    | Input de usuario               | T-021 a T-024  | 🔴 Crítica     | ⬜       |
| 5    | Sistema de Tabs                | T-025 a T-028  | 🟠 Alta        | ⬜       |
| 6    | Sistema de Split               | T-029 a T-033  | 🟠 Alta        | ⬜       |
| 7    | Búsqueda y productividad       | T-034 a T-036  | 🟡 Media       | ⬜       |
| 8    | Configuración                  | T-037 a T-040  | 🟡 Media       | ⬜       |
| 9    | Distribución y CI/CD           | T-041 a T-045  | 🟡 Media       | ⬜       |
| 10   | Calidad y conformidad          | T-046 a T-050  | 🟢 Baja        | ⬜       |

**Total: 50 tareas atómicas.**
Las fases 0-4 forman el MVP funcional: una terminal real corriendo en ventana con GPU rendering.
Las fases 5-6 añaden la diferenciación visual (splits y tabs).
Las fases 7-10 llevan el proyecto a nivel comercial.
