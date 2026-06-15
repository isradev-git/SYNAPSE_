# SYNAPSE_ — Gaps y Pendientes

> Auditado contra código real para v1.0.0 (2026-06-15).
> 353 tests passing · 5 crates · build limpio · clippy limpio · ~26k LOC.
> Targets nativos: macOS · Linux · Raspberry Pi 4/5. **Windows: no soportado.**
>
> **v1.0.0 cierra:** Kitty graphics completo (chunked/o=z/t=f), BiDi/RTL (unicode-bidi +
> Arabic joining), app icon procedural, latencia de respuestas DSR (síncronas),
> logs silenciosos, docs (INSTALL/CONFIGURATION/COMPATIBILITY/BENCHMARKS).

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

### ~~P-001 · `window_blur`~~ ✅ IMPLEMENTADO
- macOS: `NSVisualEffectView` via `objc2` → `platform_macos.rs`
- Linux X11: `_KDE_NET_WM_BLUR_BEHIND_REGION` via `x11-dl` → `platform_linux.rs`
- Linux Wayland: `window.set_blur(true)` via winit → delegado a `org_kde_kwin_blur_manager` (KDE/KWin). Sin deps extra (winit ya lleva `wayland-protocols-plasma` internamente).

---

## 2. Features con estructura pero sin UI/wiring completo

### ~~P-002 · Workspace rename~~ ✅ IMPLEMENTADO
- `WsRenameState` en `state.rs`, F2 keybind, render en status bar, `workspace.rs::rename()`

### ~~P-003 · dead_code WorkspaceManager~~ ✅ RESUELTO
- `active_cell_caches()`, `active_tab_bar_mut()`, `load_tab_bars()` eliminados. `#[allow(dead_code)]` en `Workspace` struct eliminado.

---

## 3. Features que la competencia tiene y SYNAPSE_ no

### ~~P-004 · IPC daemon socket~~ ✅ IMPLEMENTADO
- `ipc.rs`: UnixSocket `/tmp/synapse_$UID.sock`, JSON-line, comandos `list`/`kill`/`send`/`new-tab`/`new-window`

### ~~P-005 · SSH Profiles~~ ✅ IMPLEMENTADO
- `SshProfile` en config, `open_ssh_profile()` en keyboard.rs, palette command

### ~~P-006 · Variables de entorno por perfil/tab~~ ✅ IMPLEMENTADO
- `TabProfile { name, shell, cwd, env }` en config. `create_pane_full_env()` inyecta env vars al PTY spawn. Perfil visible en command palette como "Profile: nombre".

### ~~P-007 · Auto-update~~ ✅ IMPLEMENTADO
- `update.rs`: consulta GitHub Releases API. `--check-update` CLI flag. `check_updates_on_startup` en config.

### ~~P-008 · Multi-window~~ ✅ IMPLEMENTADO
- IPC `NewWindow` delega a instancia existente. `--new-window` flag abre en instancia activa.

---

## 4. VT / compatibilidad pendiente

### ~~P-009 · DECRQM~~ ✅ IMPLEMENTADO
- `scan_decrqm()` en `image_protocol.rs`, `decrqm_pm()` en `pane_ops.rs`. tmux/vim obtienen respuesta correcta.

### ~~P-010 · BiDi / RTL~~ ✅ IMPLEMENTADO (v1.0)
- `unicode-bidi` reorder visual (UAX #9) en `renderer::render_bidi_run`, gated por `text::contains_rtl` (LTR mantiene fast path).
- Shaping direccional (`shape_run_dir`) → joining árabe vía rustybuzz `guess_segment_properties`.
- Requiere fuente RTL en `font_family` (JetBrains Mono no trae glifos árabes/hebreos). Selección usa orden lógico.

### ~~P-011 · Kitty graphics protocol — completitud~~ ✅ IMPLEMENTADO (v1.0)
- Chunked transmission (`m=1`) ahora preserva id + acción en chunks siguientes (icat muestra imágenes).
- Compresión `o=z` (zlib vía flate2), medios `t=f`/`t=t` (file/temp). Query ya respondía en reader thread.
- Pendiente menor: `t=s` (shared memory), `U=1` (unicode placeholders).

---

## 5. Performance gaps

### ~~P-012 · Atlas warm entre sesiones~~ ✅ IMPLEMENTADO
- `atlas.rs`: `save_warm_cache()` / `load_and_warm()`. Path: `~/.cache/SYNAPSE_/glyph_atlas.bin`. Guarda en graceful shutdown, carga en startup.

---

## 6. Assets y documentación faltante

| Item | Estado | Nota |
|------|--------|------|
| App icon (`assets/icon.png` + `@512`) | ✅ Hecho | Procedural en runtime + `--export-icon`. Falta `.icns`/`.ico` empaquetado (docs en INSTALL). |
| `docs/BENCHMARKS.md` | ✅ Hecho | Targets + mediciones headless; FPS/latencia = profiler F12 en hardware. |
| `assets/screenshots/` | ❌ Pendiente | Requiere display; opcional post-1.0. |
| `CHANGELOG.md` | ✅ Existe | Actualizado a [1.0.0]. |
| `CONFIGURATION.md` | ✅ Hecho | Todas las opciones de `config.toml`. |
| `INSTALL.md` | ✅ Hecho | Build macOS/Linux/Raspberry Pi. |
| `COMPATIBILITY.md` | ✅ Hecho | Matriz de plataformas + escapes + gráficos. |

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
