# SYNAPSE_ — Desarrollo Principal

Estado actual: **v0.2.0** | Phase 1 ✅ | Phase 2 ✅ | Phase 3 ✅ | Phase 4 ✅

Marca cada paso con `[x]` al completarlo.

---

## Phase 1 — Foundation ✅ COMPLETA

- [x] Eliminar crate SYNAPSE_-terminal
- [x] Integrar `alacritty_terminal` (VT parsing, grid, scrollback)
- [x] Reemplazar cosmic-text/swash → `fontdue`
- [x] PTY reader thread → Processor::advance() → Term
- [x] dirty AtomicBool para frame rebuild
- [x] Atlas 2048×2048 RGBA con reset al 90%
- [x] Instanced rendering (64 bytes/cell, single draw call)
- [x] Tabs: Ctrl+T / Ctrl+W / Ctrl+1-9
- [x] Splits V/H: Ctrl+Shift+D / Ctrl+Shift+E
- [x] Navegación panes: Alt+arrows
- [x] Search: Ctrl+F
- [x] Temas: synapse_, dracula, catppuccin-mocha, tokyo-night
- [x] Hot-reload config: Ctrl+,
- [x] Cursor block/beam/underline con blink
- [x] xterm256 color palette completa
- [x] Named ANSI colors (NamedColor enum)
- [x] Resize PTY + Term al redimensionar ventana

---

## Phase 2 — UI Rework ✅ COMPLETA

### 2.1 Scrollback
- [x] Implementar scroll visual del buffer de alacritty_terminal
- [x] `InputAction::ScrollUp(n)` → desplazar viewport N líneas arriba
- [x] `InputAction::ScrollDown(n)` → desplazar viewport N líneas abajo
- [x] `InputAction::ScrollToTop` → ir al inicio del scrollback
- [x] `InputAction::ScrollToBottom` → volver al bottom (modo normal)
- [x] Guardar scroll_offset por pane
- [x] Leer desde `term.grid()` con offset aplicado (no solo display_iter)
- [x] Scroll con rueda del ratón (ya llega `handle_scroll` en app.rs)
- [x] Indicador visual de posición en scrollback (opcional)

### 2.2 Detección de salida de pane
- [x] Conectar eventos `Event::Exit` de alacritty_terminal EventProxy
- [x] Procesar `event_rx` en render loop para detectar pane muerto
- [x] Poblar `exited_panes` en `render_frame()` (hoy siempre vacío)
- [x] Cerrar pane automáticamente al morir el proceso
- [x] Si muere el único pane del único tab → abrir nuevo pane fresco
- [x] Si muere pane en tab con múltiples panes → colapsar árbol
- [x] Si muere el único pane de un tab (con otros tabs) → cerrar tab

### 2.3 Selección y Copy
- [x] Implementar `extract_selection()` en `keyboard.rs` (hoy stub que devuelve "")
- [x] Rastrear inicio/fin de selección en el grid de alacritty_terminal
- [x] Renderizar highlight de selección (bg rect sobre celdas seleccionadas)
- [x] Mouse click → inicio de selección
- [x] Mouse drag → extender selección
- [x] `Action::Copy` (Ctrl+Shift+C) → extraer texto del grid y copiar al clipboard
- [x] `InputAction::Copy` (Ctrl+C cuando hay selección) → copiar
- [x] Double-click → seleccionar palabra
- [x] Triple-click → seleccionar línea

### 2.4 OSC Title / CWD tracking
- [x] Conectar handler para OSC 0 y OSC 2 (título de ventana/tab)
- [x] Implementar `EventListener::send_event` para `Event::Title(String)`
- [x] Actualizar `pane.title` desde el evento en el render loop
- [ ] Conectar OSC 7 para CWD tracking (`file://host/path`) — alacritty_terminal 0.24 no expone evento CwdChanged; requiere hook custom (BLOQUEADO)
- [ ] Actualizar `pane.cwd` desde OSC 7 (BLOQUEADO, depende de OSC 7)
- [x] Verificar que `build_tab_bar_text()` muestra el título correcto

### 2.5 History Search (Ctrl+R)
- [x] Implementar `Action::HistorySearch` en `keyboard.rs` (hoy stub vacío)
- [x] Construir índice de historial desde scrollback del grid de alacritty
- [x] UI: barra inferior "reverse-i-search" (ya existe el render en `render.rs:609`)
- [x] Ctrl+R → abrir/ciclar al siguiente match
- [x] Esc → cancelar y restaurar línea actual
- [x] Enter → aceptar match y enviarlo al PTY

### 2.6 Deuda técnica
- [x] Consolidar `TermSize` duplicado (`app.rs:24` y `pane_ops.rs:16`) en un solo lugar
- [x] PTY reader: parsear bytes fuera del lock (patrón alacritty: staging queue)
- [x] Limpiar comentarios "Phase 1 stub" una vez implementados
- [x] `cargo clippy --workspace --all-targets -- -D warnings` limpio
- [x] Cobertura de tests ≥80 tests workspace

---

## Phase 3 — Autosuggestions (SYNAPSE_-suggest) ✅ COMPLETA

### 3.1 Nuevo crate
- [x] Crear `crates/SYNAPSE_-suggest/` con `Cargo.toml`
- [x] Añadir al workspace `Cargo.toml`
- [x] Añadir dependencia en `SYNAPSE_-app`

### 3.2 Carga de historial
- [x] Leer `~/.zsh_history` (formato extendido: `: timestamp:elapsed;command`)
- [x] Leer `~/.bash_history`
- [x] Leer `~/.local/share/fish/fish_history`
- [x] Deduplicar y limpiar líneas corruptas (skip silencioso)
- [x] Construir `Vec<String>` en memoria

### 3.3 Prefix Trie
- [x] Implementar trie de prefijos en Rust puro (`trie.rs`)
- [x] O(m) lookup donde m = longitud del prefijo
- [x] Construido una vez al startup, inmutable durante sesión
- [x] Target memoria: <20MB para historial típico
- [x] Test: prefix "git p" → "git push origin main" (historial semilla)
- [x] Test: historial vacío → sin sugerencia, sin panic
- [x] Test: línea corrupta → skip, resto carga correctamente

### 3.4 Runtime
- [x] `Suggester::query(prefix: &str) -> Option<&str>`
- [x] Integrar en `handle_keyboard()` para cualquier char tipado
- [x] Tab o → → aceptar sugerencia completa → `pane.write_to_pty()`
- [x] Shift+→ → aceptar siguiente palabra de la sugerencia
- [x] Esc / ↑ / ↓ → descartar ghost text
- [x] Cualquier otro char → re-query trie con nuevo prefijo

### 3.5 Ghost text rendering
- [x] Renderizar ghost text en misma línea que cursor, después de la posición cursor
- [x] Color: `fg` con 40% alpha (gris fantasma)
- [x] Renderizar en UI layer (draw call 3), encima de cell layer
- [x] NO enviado al PTY hasta ser aceptado
- [x] Test: Tab accept → string correcto producido
- [x] Test: Shift+→ → única palabra aceptada

---

## Phase 4 — Polish + Image Protocol ✅ COMPLETA

### 4.1 Kitty Image Protocol
- [x] Implementar parser de secuencias APC (`\x1b_G...ST` y `\x1b_G...\x07`)
- [x] Decodificar payload base64 → bytes de imagen
- [x] Soportar formato PNG, raw RGBA (f=32) y RGB (f=24)
- [x] `ImageStore` con acumulación de chunks (m=1 → continuar, m=0 → flush)
- [x] Pre-scan PTY bytes antes del VTE processor → canal `apc_rx` por pane
- [x] Drainear `apc_rx` cada frame y procesar con `image_store.process()`
- [x] Soporte para `a=T` (transmit), `a=p` (put), `a=d` (delete)
- [x] Tests: extract_apc (ST + BEL terminators), parse_apc, image_store CRUD, scan_kkp

### 4.2 Ligatures (opt-in)
- [x] `font_ligatures = true/false` en config (campo ya existía)
- [x] Shaping con `rustybuzz` (pure-Rust HarfBuzz port) → `ShapedGlyph { glyph_id, x_advance, cluster }`
- [x] Rasterizar por glyph_id con `fontdue::rasterize_indexed(glyph_id, px)`
- [x] `ShapedGlyphKey { glyph_id, font_size_bits, bold, italic }` como cache key en atlas
- [x] `build_ligature_instances()` en renderer: agrupa runs de misma fila/estilo → shape → render
- [x] `draw_frame_with_options(cells, ui_rects, bg_rects, ligatures: bool)`
- [x] Activado vía `state.config.font_ligatures` en `render_frame()`

### 4.3 Scrollback performance
- [x] Per-pane dirty tracking: solo rebuild cuando pane del tab activo está dirty
- [x] Background panes drenan su dirty flag sin triggear rebuild
- [x] Cells procesadas limitadas al viewport visible (ya existente en render loop)

### 4.4 Smooth resize
- [x] Cache clearing en `handle_resize()`: limpia `cached_cell_data`, `cached_ui_rects`, `cached_bg_rects`
- [x] Primer frame post-resize siempre rebuilt desde cero → sin artifacts

### 4.5 Kitty Keyboard Protocol
- [x] `kitty_flags: Arc<AtomicU8>` + `kitty_active: Arc<AtomicBool>` por pane (shared reader↔main)
- [x] PTY reader pre-scan: detecta `\x1b[?u` (query) → responde `\x1b[?1u` inmediatamente
- [x] KKP push (`\x1b[=Nu`): guarda flags previos en `kitty_flags_stack`, activa con flags N
- [x] KKP pop (`\x1b[Nu`): restaura flags previos del stack
- [x] Canal `kkp_rx: mpsc::Receiver<KkpCommand>` drenado en `poll_events()`
- [x] `from_key_kitty()` en `input.rs`: CSI u con codepoints correctos (Escape=27, Enter=13, ArrowLeft=57350…)
- [x] Modifier encoding: `(shift | alt<<1 | ctrl<<2) + 1`
- [x] Event type: press=1, release=3; releases solo cuando `flags & 2 != 0`
- [x] Fall-through a legacy `from_key()` para casos no manejados

---

## Performance (validar en cada phase)

- [ ] Input→render latency: <5ms
- [ ] FPS estable: 60fps
- [ ] FPS bajo carga pesada: ≥30fps
- [ ] Startup: <200ms
- [ ] RAM idle: <50MB

---

## Tests — Estado actual (105 tests, todos pasan)

| Crate | Tests |
|---|---|
| SYNAPSE_-app (image_protocol + KKP) | 13 |
| SYNAPSE_-config | 29 |
| SYNAPSE_-renderer | 8 |
| SYNAPSE_-suggest | 9 |
| SYNAPSE_-ui | 49 |
| **Total** | **108** |

- [x] SYNAPSE_-renderer: atlas no-overlap, rasterize ASCII, draw_frame sin panic
- [x] SYNAPSE_-ui: PaneTree split/close/layout, TabBar CRUD
- [x] SYNAPSE_-config: round-trip TOML, valores por defecto, cursor styles
- [x] SYNAPSE_-suggest: trie prefix match, empty history, corrupted line, accept
- [x] Workspace total: ≥80 tests pasando (**108 actualmente**)
- [x] `cargo clippy --workspace --all-targets -- -D warnings` limpio

---

## Notas de arquitectura (no olvidar)

- Surface UNORM (no sRGB) — wgpu haría doble gamma con sRGB
- `line_h = font_size * 1.2` para baseline — usar font_size por glyph, NO cell_h global
- Uploads al atlas requieren alineación de 256 bytes por fila
- PTY reader: 1KiB chunks para limitar tiempo de lock
- `pty_master` separado de `pty_writer` — necesario para resize
- `pty_writer` wrapped en `Arc<Mutex<>>` — compartido entre reader thread (KKP responses) y main thread
- APC pre-scan antes de alacritty VTE processor — alacritty ignora secuencias APC
- `kitty_flags_stack: Vec<u8>` por pane — implementa push/pop semántico del KKP spec
- Ligatures: rustybuzz shape → fontdue rasterize_indexed (NO rasterize por char, por glyph_id)
- `draw_frame_with_options(ligatures: bool)` — `draw_frame` es wrapper con `ligatures=false`

---

## Pendiente / Fase siguiente

- [ ] OSC 7 CWD tracking (bloqueado por alacritty_terminal 0.24 API)
- [x] Custom themes vía TOML (`~/.config/SYNAPSE_/themes/`) — ansi_colors sobreescribible, selection/search_highlight/search_current wireados
- [ ] Renderizar imágenes Kitty en pantalla (ImageStore poblado pero sin draw call aún)
- [ ] Benchmark formal scrollback >50k líneas con medición FPS
- [ ] Validación performance targets (latency/FPS/RAM)
