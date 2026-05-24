# SYNAPSE_ — Gaps y Pendientes

> Auditado contra código real post-v0.2.0 (2026-05-24).
> 303 tests passing · 5 crates · build limpio · 21k LOC.

---

## Estado real v0.2.0 — qué está implementado

Antes de los gaps, lista de lo que realmente funciona para evitar reimplementar:

| Feature | Archivo(s) | Estado |
|---------|-----------|--------|
| GPU postproc (scanlines, bloom, chroma, glitch, matrix rain) | `postproc.wgsl`, `bloom_h.wgsl`, `postproc.rs`, `effects.rs` | ✅ Completo |
| OSC 52 remote clipboard | `render.rs:2791`, `pane.rs:17` | ✅ Completo |
| OSC 133 semantic prompts | `pane_ops.rs:64`, `render.rs:1091` | ✅ Completo |
| OSC 9/777 desktop notifications | `pane_ops.rs:131`, `render.rs:2617` | ✅ Completo |
| Bell visual + notificación unfocused | `render.rs:2771` | ✅ Completo |
| Copy mode (vim keys hjkl/v/V/y) | `render.rs`, commits | ✅ Completo |
| Command palette | `palette.rs` — Action/Tab/Theme | ✅ Completo |
| Quake mode (slide animation) | `quake.rs`, `QuakeConfig` | ✅ Completo |
| Session save/restore | `session.rs` → `~/.cache/SYNAPSE_/session.json` | ✅ Completo |
| Font ligatures | `renderer.rs:424` via rustybuzz | ✅ Completo |
| Font fallback chain | `text.rs`, array config | ✅ Completo |
| Color emoji CBDT/CBLC | `cell.wgsl` is_emoji path, ttf-parser | ✅ Completo |
| Sixel decoder | `sixel.rs` (native Rust) | ✅ Completo |
| iTerm2 OSC 1337 | `image_protocol.rs:519` | ✅ Completo |
| Kitty graphics protocol | `image_protocol.rs` | ✅ Parcial (placement + delete) |
| Broadcast input | `render.rs:1084` ToggleBroadcast | ✅ Completo |
| Recording asciinema .cast | `record.rs` | ✅ Completo |
| Profiler overlay F12 | `render.rs:2879` + `state.rs:ProfilerData` | ✅ Completo |
| Pane zoom | `render.rs` Ctrl+Shift+Z | ✅ Completo |
| Workspaces (named) | `workspace.rs` | ✅ Estructura presente |
| Shell integration zsh/bash/fish | `assets/shell/` + `setup.rs` | ✅ Completo |
| Plugin system TOML | `keyboard.rs`, `PluginCommand` | ✅ Completo |
| Drag & drop archivos | `app.rs:169` DroppedFile | ✅ Completo |
| Persistent history cross-session | `history.rs` | ✅ Completo |
| Scrollbar per-pane | `render.rs` | ✅ Completo |
| Pane badge | `render.rs` | ✅ Completo |
| Background image per pane | `render.rs`, `BackgroundMode` | ✅ Completo |
| High contrast theme | `themes.rs` `high-contrast-dark` | ✅ Completo |
| reduce_motion | `config.rs` | ✅ Completo |
| Wayland CSD | `app.rs` | ✅ Completo |
| Window transparency | `window_opacity` config | ✅ Completo |
| CLI args (-e, -d, --quake, --restore, --setup) | `cli.rs` | ✅ Completo |

---

## 1. Stubs sin implementación de backend

### P-001 · `window_blur` — config stub, cero OS backend
**Archivo:** `crates/SYNAPSE_-config/src/config.rs:234`
**Estado:** Campo en `Config`, default `false`. Ninguna llamada en `app.rs`, `render.rs` ni ningún otro archivo de app.
**Necesita:**
- macOS: `objc2` + `NSVisualEffectView` con material `.hudWindow`/`.underWindowBackground`
- Linux X11: `_NET_WM_BLUR_BEHIND` atom via xcb (KDE/Picom)
- Linux Wayland: `zwlr_layer_shell` o extensión compositor (KDE, GNOME con blur plugin)
**Impacto:** Diferenciador visual clave. Warp/iTerm2 lo tienen. Es el feature más pedido en terminales modernas.
**Esfuerzo:** Medio (macOS ~200 LOC, Linux ~150 LOC).

---

## 2. Features con estructura pero sin UI/wiring completo

### P-002 · Workspace rename — UI de entrada de texto faltante
**Estado:** `workspace.rs:142` tiene `rename()` con `#[allow(dead_code)]`. Lógica completa. Sin inline text input en status bar para dispararlo.
**Fix:** Inline text input en status bar (similar al search bar existente).
**Esfuerzo:** Bajo (~80 LOC, reusar SearchBarState pattern).

### P-003 · `active_cell_caches` / `active_tab_bar_mut` — métodos dead_code en WorkspaceManager
**Estado:** `workspace.rs:57,62` — métodos públicos con `#[allow(dead_code)]`. Navegación rápida entre workspaces puede no estar completamente wired en todos los edge cases.
**Fix:** Auditar si `switch_workspace` funciona correctamente con el TabBar renderer.

---

## 3. Features que la competencia tiene y SYNAPSE_ no

### P-004 · IPC daemon socket
**Estado:** Cero evidencia en codebase. `cli.rs` tiene `--new-tab` como arg pero sin socket backend.
**Necesita:** Unix socket `/tmp/SYNAPSE_-$UID.sock`, protocolo JSON-line, subcomandos `list`/`kill`/`send`/`new-tab`.
**Impacto:** Esencial para integración con Neovim, scripts de shell, launchers.
**Esfuerzo:** Medio (~300 LOC).

### P-005 · SSH Profiles
**Estado:** No implementado.
**Diseño:** `[[ssh_profile]]` en TOML → abre tab/pane con `ssh user@host -i key -p port`.
**Esfuerzo:** Medio (~150 LOC config + spawning).

### P-006 · Variables de entorno por perfil/tab
**Estado:** No implementado. PTY spawn usa env del proceso padre.
**Diseño:** `[[tab_profile]]` con `env = { "API_KEY" = "..." }`, pasado a PTY spawn.

### P-007 · Auto-update / `--check-update`
**Estado:** No implementado.
**Opción mínima:** Consultar GitHub Releases API, mostrar notificación, sin auto-install.
**Esfuerzo:** Bajo (~80 LOC, `reqwest` + tokio ya en deps potenciales).

### P-008 · Multi-window con daemon compartido
**Estado:** No implementado. Cada instancia es independiente.
**Bloqueado por:** P-004 (IPC socket).

---

## 4. VT / compatibilidad pendiente

### P-009 · DECRQM — responder a queries de modo terminal
**Estado:** `CSI ? Ps $ p` enviado por tmux/vim no recibe respuesta (silencio actual).
**Fix:** Tabla de modos + respuesta `CSI ? Ps ; Pm $ y`.
**Esfuerzo:** Bajo (~60 LOC en VTE handler).

### P-010 · BiDi / RTL
**Estado:** No implementado. Texto árabe/hebreo se renderiza incorrectamente (LTR forzado).
**Necesita:** `unicode-bidi` crate, reorder visual antes de shaping.
**Esfuerzo:** Alto (interacción compleja con rustybuzz shaping).

### P-011 · Kitty graphics protocol — completitud
**Estado:** Placement + delete implementados. Chunked transmission (`m=1`), virtual cursor positioning, unicode placeholders — sin verificar.
**Fix:** Auditar contra spec oficial Kitty, correr `kitten icat`.

---

## 5. Performance gaps

### P-012 · Atlas warm entre sesiones
**Estado:** Atlas se vacía en restart. Glyphs más frecuentes se re-renderizan al arrancar.
**Fix:** Serializar atlas a disco (`.cache/SYNAPSE_/glyph_atlas.bin`), cargar en startup.
**Esfuerzo:** Medio. Bloquea ganar ~50ms de startup en uso real.

---

## 6. Assets y documentación faltante

| Item | Estado | Nota |
|------|--------|------|
| App icons `.icns` / `.png 512px` | ❌ No existen | `assets/` solo tiene `fonts/` y `shell/` |
| `docs/BENCHMARKS.md` | ❌ No existe | Necesita vtebench + mediciones reales |
| `assets/screenshots/` | ❌ No existe | README sin imágenes del producto |
| `CHANGELOG.md` | ❌ No existe | Linked desde README — enlace roto |
| `CONFIGURATION.md` | ❌ No existe | Linked desde README — enlace roto (eliminado en README rewrite) |
| `INSTALL.md` | ❌ No existe | Linked desde README — enlace roto (eliminado en README rewrite) |
| `COMPATIBILITY.md` | ❌ No existe | Linked desde README — enlace roto (eliminado en README rewrite) |

---

## Prioridad de implementación recomendada

### Sprint 1 — Completar la experiencia (alto ROI)

| ID | Feature | Esfuerzo | Impacto |
|----|---------|----------|---------|
| P-001 | `window_blur` macOS + Linux | Medio | Alto — diferenciador visual |
| P-002 | Workspace rename UI | Bajo | Medio — lógica ya existe |
| P-004 | IPC daemon socket | Medio | Alto — habilita `--new-tab`, nvim |
| App icons | Diseño + build.rs embed | Medio | Alto — distribución |

### Sprint 2 — Features faltantes vs competencia

| ID | Feature | Esfuerzo | Impacto |
|----|---------|----------|---------|
| P-005 | SSH Profiles | Medio | Alto |
| P-009 | DECRQM responder queries | Bajo | Medio |
| P-011 | Kitty graphics completitud | Bajo-Medio | Medio |
| P-007 | Auto-update check | Bajo | Bajo-Medio |

### Sprint 3 — Distribución y visibilidad

| Item | Esfuerzo | Impacto |
|------|----------|---------|
| BENCHMARKS.md con mediciones reales | Bajo | Alto (credibilidad) |
| Screenshots en README | Bajo | Alto (conversión) |
| CHANGELOG.md | Bajo | Medio |
| P-012 Atlas warm entre sesiones | Medio | Medio (startup) |

### Sprint 4 — Compliance y a11y avanzada

| ID | Feature | Esfuerzo | Impacto |
|----|---------|----------|---------|
| P-010 | BiDi/RTL | Alto | Bajo-Medio |
| P-006 | ENV vars por perfil | Medio | Medio |
| P-008 | Multi-window | Alto (dep P-004) | Alto |

---

## Estado por crate

| Crate | Tests | `#[allow(dead_code)]` | Gaps reales |
|-------|-------|----------------------|-------------|
| `SYNAPSE_-app` | 131 | `workspace.rs`, `state.rs (ProfilerData)`, `history.rs`, `palette.rs (plugins)`, `input.rs`, `overlay.rs`, `image_protocol.rs` | P-001, P-002, P-004 |
| `SYNAPSE_-renderer` | 65 | Ninguno | P-012 |
| `SYNAPSE_-ui` | 22 | Ninguno | — |
| `SYNAPSE_-config` | 19 | Ninguno | P-001 (stub) |
| `SYNAPSE_-suggest` | 66 | Ninguno | — |

---

*Última revisión: 2026-05-24*
