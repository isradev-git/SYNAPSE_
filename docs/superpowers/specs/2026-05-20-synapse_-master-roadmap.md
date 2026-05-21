# SYNAPSE_ — Master Roadmap

> **Objetivo:** Convertir SYNAPSE_ en la terminal más completa, rápida y visualmente impactante del mercado, fusionando rendimiento GPU de primer nivel con una estética cyberpunk / Blade Runner / hacker movie llevada al máximo.

**Versión actual:** v0.2.0 (Fase 10 — MVP estable)
**Estado del repo:** 10.236 LOC, 4 crates + suggest, 120 tests, clippy clean.
**Targets activos:** macOS (Metal), Linux (X11/Wayland). Windows diferido.

---

## 1. Diagnóstico — Estado actual

### 1.1 Lo que YA funciona
| Área | Capacidad |
|------|-----------|
| **Render** | wgpu 22 instanced, atlas glyphs fontdue, 5 underline styles, image protocol (parcial), UI bg/fg passes |
| **VT** | alacritty_terminal 0.24 (xterm-256color, SGR full, OSC, true color, mouse reporting básico) |
| **Layout** | Tabs + árbol binario de splits, drag de divisores, tab scroll |
| **Input** | Keybinds TOML personalizables (30 actions), mouse selection, double/triple click, copy-on-release |
| **Búsqueda** | Buffer search Ctrl+Shift+F + history search Ctrl+R |
| **Autosuggest** | synapse_suggest crate — trie + builtins + historia + ghost text |
| **Config** | TOML hot-reload Ctrl+, themes builtin (4) + TOML custom |
| **Otros** | Link detection (OSC 8 + heurística), scrollback 100K, fuente dinámica, fullscreen, splash inicial |

### 1.2 Crates y arquitectura

```
SYNAPSE_-app         binary, winit, render loop                (~4.700 LOC)
  ├─ app.rs          App + AppCore + event handlers
  ├─ render.rs       cell→instance pipeline                    (1.424 LOC — el más caliente)
  ├─ pane_ops.rs     create/split/close panes
  ├─ keyboard.rs     keymap + actions
  ├─ input.rs        clipboard, font, fullscreen
  ├─ mouse.rs        click, drag, select, URL hover
  ├─ search.rs       buffer & history search engines
  ├─ state.rs        AppState, SearchState, SuggestState
  └─ image_protocol.rs Sixel/Kitty/iTerm2 (parcial)
SYNAPSE_-renderer    wgpu pipelines + atlas                    (~2.000 LOC)
  ├─ renderer.rs     surface, device, draw_frame
  ├─ cell.rs         CellRenderer + cell.wgsl
  ├─ ui.rs           UIRenderer + ui.wgsl
  ├─ underline.rs    UnderlineRenderer + underline.wgsl (NEW)
  ├─ image.rs        ImageRenderer + image.wgsl
  ├─ text.rs         shaping (rustybuzz disponible, fontdue raster)
  └─ atlas.rs        TextureAtlas 2048² RGBA
SYNAPSE_-ui          layout, panes, tab bar, theme             (~1.300 LOC)
SYNAPSE_-config      Config + Keybinds + Theme TOML            (~1.340 LOC)
SYNAPSE_-suggest     trie + history + builtins                 (~740 LOC)
```

### 1.3 Gaps identificados
Carencias frente a Ghostty / WezTerm / Kitty / Warp / Tabby:

| Categoría | Gap |
|-----------|-----|
| **VT completeness** | Sin OSC 133 (semantic prompts), sin OSC 52 (clipboard), sin OSC 1337 iTerm2 completo, sin DECRQM/DECSCUSR todos, bracketed paste parcial |
| **Multiplex** | Sin sesiones persistentes (tmux-like), sin broadcast input, sin copy-mode estilo vim |
| **Imagen** | image_protocol.rs existe pero no totalmente wired al ImageRenderer; falta Kitty completo, Sixel optimizado, iTerm2 OSC 1337 |
| **Shaders** | No hay efectos GPU (CRT, bloom, scanlines, chroma, glitch, matrix bg) — la oportunidad cyberpunk está sin explotar |
| **Fonts** | rustybuzz disponible pero ligaduras OFF; sin fallback chain; sin emoji color (COLRv1/CBDT); sin font features (calt, ss01..ss20) |
| **UX** | Sin command palette (Cmd+P), sin quake mode, sin notifications (OSC 9), sin bell handling, sin pane zoom, sin theme picker en runtime |
| **Sesiones** | Sin save/restore layout, sin "remember last tabs" |
| **CLI / IPC** | Sin `synapse_ -e cmd`, sin daemon/IPC socket para abrir desde otra ventana |
| **Plataformas** | Sin Wayland CSD, sin window blur/transparency, sin Mica/acrylic |
| **Performance** | Glyph atlas no se LRU-evict; no hay damage tracking real (re-render todo si dirty); no warm-render para tabs background |
| **A11y** | Sin screen-reader, sin alto contraste, sin reduce-motion |
| **i18n** | Sin BiDi (RTL), CJK ancho probablemente parcial |
| **Telemetría dev** | Sin overlay perf ingame (frame time, dirty count) |

---

## 2. Visión — Qué será SYNAPSE_ v1.0

> **"La terminal que parece una pantalla de Blade Runner 2049 sin sacrificar 60fps."**

**Tres pilares no negociables:**

1. **Velocidad bruta** — input→pixel <3ms, 144Hz capable, 0-alloc render path.
2. **VT compliance total** — pasa todos los xterm/vttest, soporta Kitty/Sixel/iTerm2 imágenes, OSC 133 jumping.
3. **Estética cyberpunk sin compromisos** — shader postproc opcional (CRT/bloom/glitch), animaciones, fondo matrix rain, neon borders pulsantes, fonts custom.

---

## 3. Roadmap por fases

Cada fase produce un binario funcional y testeable. **No** saltar fases.

### Fase 11 — VT compliance + multiplex básico (1-2 semanas)

**Objetivo:** Pasar Neovim, tmux, htop, btop, lazygit sin glitches.

- [ ] **11.1** OSC 52 — set/query clipboard remoto (común en SSH+vim).
- [ ] **11.2** OSC 133 — marks semánticos prompt/output/command. Habilita "jump to next prompt" (Ctrl+Up/Down).
- [ ] **11.3** OSC 7 — track CWD por pane (parsear pwd-spawn integration). Permite splits que heredan dir.
- [ ] **11.4** OSC 9 / OSC 777 — desktop notifications (libnotify Linux, NSUserNotification macOS).
- [ ] **11.5** Bell handling — `\a` → flash visual (border red pulse) + sonido opcional + notif si window unfocused.
- [ ] **11.6** Bracketed paste completo (`\e[200~ … \e[201~`).
- [ ] **11.7** DECSCUSR todos los modos (0-6) + DECRQM responder a queries.
- [ ] **11.8** Copy mode (vim-keys, prefix Ctrl+Shift+Space): hjkl, w/b/e, /search, y yank, V line, v char.
- [ ] **11.9** Pane zoom / maximize (toggle Ctrl+Shift+Z) — pane activa ocupa toda la ventana sin destruir layout.
- [ ] **11.10** Broadcast input — Ctrl+Shift+B: envía teclado a todos los panes de la tab.

**Test:** Vim macros, tmux nested, lazygit + bat preview, fzf, neovim spell check todo OK.

---

### Fase 12 — Imágenes y media (1-2 semanas)

**Objetivo:** `kitten icat`, `imgcat`, `chafa --kitty/sixel` funcionan nativamente. `nvim image.nvim` muestra PNGs inline.

- [ ] **12.1** Auditar `image_protocol.rs` actual — detectar qué falta vs Kitty Graphics Protocol spec.
- [ ] **12.2** Kitty graphics protocol completo:
  - [ ] Transmission (a=t,T,p,d,q + chunked m=1).
  - [ ] Placement con z-index, virtual cursor positioning, unicode placeholders.
  - [ ] Delete commands (a=d, d=A/C/I/N…).
- [ ] **12.3** iTerm2 OSC 1337 (`File=inline=1:…base64…`) — útil para shells donde Kitty no está activo.
- [ ] **12.4** Sixel — parser + decoder, render como bitmap a ImageRenderer.
- [ ] **12.5** Animated GIF / APNG — frame loop en ImageStore.
- [ ] **12.6** Image cache LRU (max 256MB RAM, evict por LRU+TTL).

**Test:** `kitten icat assets/logo.png`, `chafa --format=sixel video.mp4`, `nvim` con image.nvim.

---

### Fase 13 — Shaders cyberpunk (postproc) ⚡ **CORE AESTÉTICO**

**Objetivo:** SYNAPSE_ se ve único. Cada efecto OFF por default (zero perf cost), configurable en TOML, toggle runtime.

- [ ] **13.1** Postproc pipeline — añadir intermediate texture (offscreen) + 2º pass full-quad. Refactor `draw_frame` para escribir a texture en vez de directo a surface.
- [ ] **13.2** **CRT scanlines shader** (`shaders/crt.wgsl`):
  - Horizontal scanlines (intensity, freq, alpha).
  - Vertical RGB mask (subpixel-style).
  - Slight barrel distortion (configurable curve).
  - Vignette (radial dark gradient).
- [ ] **13.3** **Bloom / glow** (2-pass gaussian blur):
  - Threshold (sólo glyphs sobre brightness X).
  - Tint config (default: rojo cyber #FF003C glow).
  - Sigma + samples ajustables.
- [ ] **13.4** **Chromatic aberration** — RGB channel split en ejes radiales (más fuerte en bordes).
- [ ] **13.5** **Glitch / datamosh** — random horizontal shifts triggers: pane focus change, error, manual trigger.
- [ ] **13.6** **Phosphor decay** — text fade-in slow (15-50ms) tipo CRT lento. Útil para output rápido.
- [ ] **13.7** **Background matrix rain** — opcional, behind cell layer, configurable: glifos katakana japoneses + ASCII, color cyber, speed.
- [ ] **13.8** **Hex grid background** — alternativa más sutil, mesh hexagonal animado con pulse.
- [ ] **13.9** **Neon pane border pulse** — animar `panel_active_border` con sin-wave alpha 0.6→1.0 cada 2s.
- [ ] **13.10** **Cursor trail** — N frames anteriores del cursor con alpha decay → estela.
- [ ] **13.11** Config TOML:
  ```toml
  [effects]
  enabled = true
  scanlines = { intensity = 0.3, freq = 2.0 }
  bloom = { threshold = 0.7, sigma = 4.0, tint = "#FF003C" }
  chroma = { strength = 0.002 }
  matrix_bg = { enabled = false, color = "#00FF55", density = 0.3 }
  pane_pulse = true
  cursor_trail = 4
  ```
- [ ] **13.12** Keybind toggle `effects_toggle` (Ctrl+Shift+E).

**Performance:** todos los efectos OFF → 0 overhead. Todos los efectos ON → debe mantener 60fps@1440p en GPU media (Intel Iris / M1 base).

---

### Fase 14 — Command Palette & Quake Mode (1 semana)

- [ ] **14.1** **Command Palette** (Ctrl+Shift+P):
  - Overlay flotante con UIRenderer (rounded rect + blur backdrop si shader disponible).
  - Fuzzy search (nucleo-matcher) sobre acciones, tabs, panes, history, themes.
  - Preview en vivo (highlight tab/pane mientras navegas).
  - Acciones extensibles vía TOML plugins.
- [ ] **14.2** **Quake mode** (dropdown desde top de pantalla):
  - Flag `--quake` o keybind global (registrar shortcut OS-level si posible).
  - Slide-down animation (200ms ease-out).
  - Auto-hide on focus loss configurable.
- [ ] **14.3** **Theme picker en runtime** — abierto desde palette o Ctrl+Shift+T: lista de themes con preview live (1s para confirmar / Esc cancela).

---

### Fase 15 — Sesiones, layouts y CLI/IPC (1 semana)

- [ ] **15.1** **Save/restore layout** — al cerrar, serializa tabs+splits+CWD por pane a `~/.cache/SYNAPSE_/session.json`. Flag `--restore` al iniciar.
- [ ] **15.2** **Named workspaces** — `synapse_ --workspace dev` carga layout específico.
- [ ] **15.3** **CLI argument parsing** (clap está en deps):
  - `synapse_ -e <cmd>` ejecuta y cierra al exit.
  - `synapse_ --new-tab <cmd>` envía a daemon si corriendo.
  - `synapse_ --new-window`.
  - `synapse_ --theme synapse_` override puntual.
- [ ] **15.4** **IPC daemon socket** (`~/.cache/SYNAPSE_/ipc.sock`):
  - Subcomandos: `synapse_ list`, `synapse_ kill <tab>`, `synapse_ send <pane> <cmd>`.
  - Útil para integración Neovim/scripts.

---

### Fase 16 — Fonts y typography pro (3-5 días)

- [ ] **16.1** **Ligatures ON por default** — wirear rustybuzz al pipeline de glyphs. Probar con FiraCode / JetBrains Mono.
- [ ] **16.2** **Font fallback chain** — config:
  ```toml
  font_family = ["JetBrains Mono", "Symbols Nerd Font", "Noto Color Emoji"]
  ```
  Si glyph no existe en primary → buscar en next. Cache LRU por codepoint.
- [ ] **16.3** **Emoji color** — soporte COLRv1 (Noto Color Emoji nuevo formato) + CBDT/CBLC fallback.
- [ ] **16.4** **OpenType features** — config `font_features = ["ss01", "calt", "zero"]`.
- [ ] **16.5** **Variable fonts** — soportar wght/wdth axes si fuente lo expone.
- [ ] **16.6** **Bold/italic real** — actualmente probable que use mismo glyph; cargar style="Bold Italic" cuando SGR.

---

### Fase 17 — Estética cyberpunk fina (3-5 días)

> Cosas que NO son shaders pero suman al feel.

- [ ] **17.1** **Splash boot animation** — al arrancar, 800ms de "decode":
  - Línea por línea de ASCII art SYNAPSE_ con glitch reveal.
  - Texto "INITIALIZING NEURAL LINK… [OK]" con random hex chars.
  - Skippable con cualquier tecla.
- [ ] **17.2** **Tab bar redesign**:
  - Tab activa: línea inferior de 2px neón rojo con glow.
  - Hover: alpha pulse.
  - Tab close button: × rojo que se rota 90° con animación al hover.
  - Background: gradiente sutil (top más oscuro).
- [ ] **17.3** **Status bar opcional** (bottom 18px):
  - LEFT: CWD truncado, branch git, k8s context si aplica.
  - CENTER: hostname con "@" estilo `user@host`.
  - RIGHT: CPU%, MEM%, hora HH:MM:SS, latencia ping si SSH.
  - Toggle Ctrl+Shift+S.
  - Render con UIRenderer (zero cost si OFF).
- [ ] **17.4** **Pane label overlay** — al hacer focus/split, mostrar 600ms el ID del pane (P1, P2…) en esquina sup-izq con fade-out.
- [ ] **17.5** **Resize indicators** — al arrastrar splitter, mostrar dims (cols×rows) en centro del pane.
- [ ] **17.6** **Scroll indicator** — barra vertical fina en borde derecho, alpha-fade tras 1.5s sin scroll.
- [ ] **17.7** **ASCII logo SYNAPSE_** custom en welcome (primera vez sin args).
- [ ] **17.8** **Cursor styles extras**:
  - "Hollow block" cuando pane no-focused (estándar terminal).
  - Opción `cursor_style = "neon_underbar"` — underbar con glow rojo.
- [ ] **17.9** **Selection visual mejorado** — bg con scanline diagonal sutil + neon border 1px.

---

### Fase 18 — Performance hardening (3-5 días)

- [ ] **18.1** **Damage tracking real** — alacritty_terminal expone `damage()` API; sólo re-collect cells dirty en lugar de todo el grid.
- [ ] **18.2** **Atlas LRU eviction** — cuando atlas hits 90%, evict glyphs por uso reciente en lugar de reset total.
- [ ] **18.3** **Background tab freeze** — panes no visibles no se rendrizan; sólo se actualiza VT state.
- [ ] **18.4** **Frame pacing** — implementar VRR-aware (`SurfaceConfiguration::present_mode`). Config: `vsync = "auto" | "off" | "mailbox"`.
- [ ] **18.5** **Profiler overlay** (debug build): F12 toggle:
  - Frame time (16ms target line).
  - Cells uploaded, glyphs in atlas %, draw calls.
  - PTY bytes/s.
- [ ] **18.6** **Benchmark suite** vs alacritty/wezterm — `cat huge.log`, `vim` scroll, `cmatrix` 1440p.

---

### Fase 19 — Quality of life (paralelo, dispersar en fases)

- [ ] Multi-window (varias instancias del proceso, comparte daemon).
- [ ] Mouse reporting xterm protocol completo (1006, 1015).
- [ ] Hyperlinks OSC 8 click → open browser (xdg-open / open).
- [ ] Drag&drop archivos al pane → pega path.
- [ ] URL detection mejorado (regex completo IPv6, ports).
- [ ] File detection — paths absolutos clickables → abre `$EDITOR file:line`.
- [ ] Search regex toggle (Ctrl+/ dentro de search).
- [ ] History persistente cross-pane (sqlite o RON file).
- [ ] Recording mode — exporta a `.cast` (asciinema format).
- [ ] Snapshot mode — F11: PNG export del pane activo.

---

### Fase 20 — A11y, i18n, robustez (1 semana)

- [ ] BiDi básico (Hebrew/Arabic) — unicode-bidi crate.
- [ ] CJK width tables — UAX#11 wcwidth correcto, ambiguos configurables.
- [ ] Screen reader hook (AT-SPI Linux, NSAccessibility macOS) — exponer texto del pane activo.
- [ ] High contrast theme (negro/blanco puro).
- [ ] `reduce_motion` config → desactiva todas animaciones.
- [ ] Crash reporter — backtrace a `~/.cache/SYNAPSE_/crash-<ts>.log`.

---

## 4. Detalles técnicos clave por fase

### 4.1 Pipeline postproc (Fase 13)

```
Frame N:
  pass 0  → offscreen tex (R16G16B16A16) [bg + cells + ui + cursor + underline]
  pass 1a → bloom: threshold + downsample 4× + gaussian H
  pass 1b → bloom: gaussian V + upsample additive
  pass 2  → composite: offscreen + bloom + chroma + scanlines + vignette → surface
```

Requiere:
- 2 nuevos pipelines: `postproc.wgsl`, `bloom.wgsl`.
- 3 textures intermediates (offscreen + 2 bloom mip levels).
- Refactor `Renderer::draw_frame` para aceptar `target: &TextureView`.

### 4.2 Damage tracking (Fase 18.1)

`alacritty_terminal::Term::damage()` devuelve `TermDamage { lines, full }`. Cambiar `render::collect_cells` para:
- Si `full` → flujo actual.
- Si `lines` → reusar `cached_cell_data`, sólo regenerar rows en damage set.

Reduce cells re-uploaded en ≥80% en uso típico.

### 4.3 IPC daemon (Fase 15.4)

- Socket Unix `/tmp/SYNAPSE_-$UID.sock` (Linux/macOS).
- Protocolo: JSON-line, comandos `{"cmd": "new_tab", "args": {…}}`.
- Detección: si proceso ya corre y `--single-instance`, envía cmd y exit.

### 4.4 Shaders configurables (Fase 13)

Cada shader es un `#[repr(C)] struct` uniform buffer. WGSL preprocessor simple (string replace) para `#define` flags. Pipeline cache por hash de config → no recompila si user no cambia params.

---

## 5. Prioridades — qué hacer YA

**Sprint 1 (esta semana)** — `feat/postproc-pipeline`:
1. Postproc refactor (offscreen target).
2. CRT scanlines + vignette shader.
3. Bloom 2-pass.
4. Pane pulse animation.
5. Effects toggle keybind + config TOML.

**Sprint 2** — `feat/vt-compliance`:
6. OSC 133 marks + jump command (palette).
7. OSC 52 clipboard.
8. Bell visual + audio + notif.
9. Bracketed paste fix.

**Sprint 3** — `feat/command-palette`:
10. Command palette overlay UI.
11. Quake mode dropdown.
12. Theme picker runtime.

**Sprint 4** — `feat/images-kitty`:
13. Auditoría image_protocol.
14. Kitty graphics completo.
15. Sixel parser.

**Sprint 5** — `feat/cyber-polish`:
16. Splash boot animation.
17. Status bar.
18. Pane label overlay.
19. Selection visual upgrade.
20. Scroll indicator.

---

## 6. Cómo seguir este roadmap

Por cada fase:

1. **Crear spec dedicada** — `docs/superpowers/specs/2026-MM-DD-<fase>-design.md` usando skill `superpowers:brainstorming` si la fase es no-trivial.
2. **Crear plan dedicado** — `docs/superpowers/plans/2026-MM-DD-<fase>.md` usando skill `superpowers:writing-plans`.
3. **Ejecutar con subagents** — skill `superpowers:subagent-driven-development`, una tarea por subagent + dual review (spec compliance + code quality).
4. **Branch por fase** — `feat/fase-NN-<slug>`.
5. **Tests obligatorios** — cada tarea TDD: test failing → impl → green → commit.
6. **Clippy clean** — `cargo clippy --workspace --all-targets -- -D warnings` en cada commit.
7. **Smoke test visual** — para fases UI/render, screenshot/grabación + verificación manual.

---

## 7. Métricas de éxito v1.0

| Métrica | Target |
|---------|--------|
| Input→render latency | <3ms p99 (actual: ~5ms) |
| FPS 1440p efectos OFF | 144 sostenido |
| FPS 1440p TODOS efectos ON | 60 sostenido |
| Startup (cold) | <120ms (actual: <200ms) |
| RAM idle 1 pane | <40MB (actual: <50MB) |
| Tests workspace | 250+ (actual: 120) |
| VT compliance vttest | 100% sin glitches |
| Apps probadas sin bug | nvim+lazygit+tmux+btop+yazi+helix |

---

## 8. Anti-features (NO hacer)

- **Plugins JS/TS** — añade complejidad y RAM. SYNAPSE_ es nativo, todo en TOML.
- **AI integrada in-terminal** — fuera del scope. Que el shell lo haga.
- **Web/HTML embebido** — no es Electron. Sin webview.
- **Servidor remoto propio** — usar SSH/mosh estándar.
- **Tema "claro" oficial** — SYNAPSE_ es cyber. Custom TOML pueden, no shipearemos uno light de fábrica.

---

## 9. Inspiración / benchmarks de referencia

| Terminal | Qué copiar | Qué mejorar |
|----------|-----------|-------------|
| Alacritty | Velocidad pura, simpleza | Su UI es nula → SYNAPSE_ aporta tabs/splits/effects |
| WezTerm | Multiplex, command palette | Configuración Lua compleja → TOML simple |
| Kitty | Graphics protocol, ligaduras | Estética genérica → SYNAPSE_ ES cyber |
| Ghostty | Polish, performance | OSS reciente, sin shaders postproc |
| Warp | UX moderna (blocks, AI) | Closed source, telemetría, RAM hambre |
| Hyper | Estética customizable | Electron lento → SYNAPSE_ nativo |
| Rio | Sugar nativo Rust | Aún beta → SYNAPSE_ apunta a producción |

---

**Última actualización:** 2026-05-20
**Owner:** isradev-git
**Próximo paso:** brainstorming Fase 13 (postproc shaders) → spec → plan → exec.
