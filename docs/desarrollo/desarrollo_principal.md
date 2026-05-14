# Luna — Desarrollo Principal

Estado actual: **v0.2.0** | Phase 1 completa ✅

Marca cada paso con `[x]` al completarlo.

---

## Phase 1 — Foundation ✅ COMPLETA

- [x] Eliminar crate Luna-terminal
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
- [x] Temas: luna, dracula, catppuccin-mocha, tokyo-night
- [x] Hot-reload config: Ctrl+,
- [x] Cursor block/beam/underline con blink
- [x] xterm256 color palette completa
- [x] Named ANSI colors (NamedColor enum)
- [x] Resize PTY + Term al redimensionar ventana

---

## Phase 2 — UI Rework

### 2.1 Scrollback
- [x] Implementar scroll visual del buffer de alacritty_terminal
- [x] `InputAction::ScrollUp(n)` → desplazar viewport N líneas arriba
- [x] `InputAction::ScrollDown(n)` → desplazar viewport N líneas abajo
- [x] `InputAction::ScrollToTop` → ir al inicio del scrollback
- [x] `InputAction::ScrollToBottom` → volver al bottom (modo normal)
- [x] Guardar scroll_offset por pane
- [x] Leer desde `term.grid()` con offset aplicado (no solo display_iter)
- [x] Scroll con rueda del ratón (ya llega `handle_scroll` en app.rs)
- [ ] Indicador visual de posición en scrollback (opcional)

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
- [ ] `InputAction::Copy` (Ctrl+C cuando hay selección) → copiar
- [x] Double-click → seleccionar palabra
- [x] Triple-click → seleccionar línea

### 2.4 OSC Title / CWD tracking
- [x] Conectar handler para OSC 0 y OSC 2 (título de ventana/tab)
- [x] Implementar `EventListener::send_event` para `Event::Title(String)`
- [x] Actualizar `pane.title` desde el evento en el render loop
- [ ] Conectar OSC 7 para CWD tracking (`file://host/path`) — alacritty_terminal 0.24 no expone evento CwdChanged; requiere hook custom
- [ ] Actualizar `pane.cwd` desde OSC 7
- [ ] Verificar que `build_tab_bar_text()` muestra el título correcto

### 2.5 History Search (Ctrl+R)
- [x] Implementar `Action::HistorySearch` en `keyboard.rs` (hoy stub vacío)
- [x] Construir índice de historial desde scrollback del grid de alacritty
- [x] UI: barra inferior "reverse-i-search" (ya existe el render en `render.rs:609`)
- [x] Ctrl+R → abrir/ciclar al siguiente match
- [x] Esc → cancelar y restaurar línea actual
- [x] Enter → aceptar match y enviarlo al PTY

### 2.6 Deuda técnica
- [x] Consolidar `TermSize` duplicado (`app.rs:24` y `pane_ops.rs:16`) en un solo lugar
- [ ] PTY reader: parsear bytes fuera del lock (patrón alacritty: staging queue)
- [x] Limpiar comentarios "Phase 1 stub" una vez implementados
- [x] `cargo clippy --workspace --all-targets -- -D warnings` limpio
- [ ] Cobertura de tests ≥80 tests workspace

---

## Phase 3 — Autosuggestions (luna-suggest)

### 3.1 Nuevo crate
- [ ] Crear `crates/luna-suggest/` con `Cargo.toml`
- [ ] Añadir al workspace `Cargo.toml`
- [ ] Añadir dependencia en `Luna-app`

### 3.2 Carga de historial
- [ ] Leer `~/.zsh_history` (formato extendido: `: timestamp:elapsed;command`)
- [ ] Leer `~/.bash_history`
- [ ] Leer `~/.local/share/fish/fish_history`
- [ ] Deduplicar y limpiar líneas corruptas (skip silencioso)
- [ ] Construir `Vec<String>` en memoria

### 3.3 Prefix Trie
- [ ] Implementar trie de prefijos en Rust puro
- [ ] O(m) lookup donde m = longitud del prefijo
- [ ] Construido una vez al startup, inmutable durante sesión
- [ ] Target memoria: <20MB para historial típico
- [ ] Test: prefix "git p" → "git push origin main" (historial semilla)
- [ ] Test: historial vacío → sin sugerencia, sin panic
- [ ] Test: línea corrupta → skip, resto carga correctamente

### 3.4 Runtime
- [ ] `Suggester::on_key(prefix: &str) -> Option<String>`
- [ ] Integrar en `handle_keyboard()` para cualquier char tipado
- [ ] Tab o → → aceptar sugerencia completa → `pane.write_to_pty()`
- [ ] Shift+→ → aceptar siguiente palabra de la sugerencia
- [ ] Esc / ↑ / ↓ → descartar ghost text
- [ ] Cualquier otro char → re-query trie con nuevo prefijo

### 3.5 Ghost text rendering
- [ ] Renderizar ghost text en misma línea que cursor, después de la posición cursor
- [ ] Color: `fg` con 40% alpha (gris fantasma)
- [ ] Renderizar en UI layer (draw call 3), encima de cell layer
- [ ] NO enviado al PTY hasta ser aceptado
- [ ] Test: Tab accept → string correcto producido
- [ ] Test: Shift+→ → única palabra aceptada

---

## Phase 4 — Polish + Image Protocol

### 4.1 Kitty Image Protocol
- [ ] Implementar parser de secuencias APC (`\x1b_G...ST`)
- [ ] Decodificar payload base64 → bytes de imagen
- [ ] Soportar formato PNG y raw RGBA
- [ ] Integrar imagen en atlas o textura separada
- [ ] Renderizar en cell layer con UV coords correctas
- [ ] Soporte para `a=T` (transmit), `a=p` (put), `a=d` (delete)
- [ ] Verificar con: `kitty +kitten icat imagen.png`

### 4.2 Ligatures (opt-in)
- [ ] Añadir `font_ligatures = true/false` en config (ya existe el campo)
- [ ] Implementar shaping de ligatures con fontdue o alternativa
- [ ] Solo activar cuando `config.font_ligatures = true`
- [ ] Test: `->`, `=>`, `!=`, `>=` con JetBrains Mono

### 4.3 Scrollback performance
- [ ] Benchmark: scrollback >50k líneas, medir FPS
- [ ] Implementar dirty-region tracking (no rebuild todo el frame)
- [ ] Limitar cells procesadas al viewport visible únicamente

### 4.4 Theme polish
- [ ] Soporte para themes custom via TOML (`~/.config/Luna/themes/`)
- [ ] Verificar colores con cada tema (dracula, catppuccin, tokyo-night)
- [ ] Smooth resize sin flicker

### 4.5 Kitty Keyboard Protocol
- [ ] Implementar `kitty_flags` y `kitty_active` (hoy stubs en `keyboard.rs:76`)
- [ ] Detectar soporte en el terminal que recibe
- [ ] Enviar secuencias CSI u para teclas modificadas

---

## Performance (validar en cada phase)

- [ ] Input→render latency: <5ms
- [ ] FPS estable: 60fps
- [ ] FPS bajo carga pesada: ≥30fps
- [ ] Startup: <200ms
- [ ] RAM idle: <50MB

---

## Tests objetivo

- [ ] Luna-renderer: atlas no-overlap, rasterize ASCII, draw_frame sin panic
- [ ] Luna-ui: PaneTree split/close/layout, TabBar CRUD
- [ ] Luna-config: round-trip TOML, valores por defecto, cursor styles
- [ ] luna-suggest: trie prefix match, empty history, corrupted line, accept
- [ ] Workspace total: ≥80 tests pasando

---

## Notas de arquitectura (no olvidar)

- Surface UNORM (no sRGB) — wgpu haría doble gamma con sRGB
- `line_h = font_size * 1.2` para baseline — usar font_size por glyph, NO cell_h global
- Uploads al atlas requieren alineación de 256 bytes por fila
- PTY reader: 1KiB chunks para limitar tiempo de lock
- `pty_master` separado de `pty_writer` — necesario para resize
