# SYNAPSE_ — Plan UI/UX por fases

> Objetivo: sidebar vertical de tabs + tiling de panes estilo Hyprland + estética cyberpunk completa.
> Cada fase es un binario funcional. No saltar fases. Clippy clean en cada commit.

---

## Resumen de fases

| Fase | Nombre | Impacto | Tiempo est. |
|------|--------|---------|-------------|
| A | Sidebar vertical de tabs | 🔴 Crítico — cambia toda la geometría | 3-4 días |
| B | Tiling de panes mejorado (Hyprland-style) | 🔴 Crítico — UX de splits | 3-4 días |
| C | Tab bar redesign + indicadores | 🟡 Alto — primera impresión | 2-3 días |
| D | Status bar | 🟡 Alto — información contextual | 2 días |
| E | Postproc shaders (CRT + bloom) | 🔴 Crítico — identidad visual | 5-7 días |
| F | Splash boot + cursor extras | 🟢 Medio — polish | 2 días |
| G | Command palette | 🟡 Alto — productividad | 4-5 días |

---

## Fase A — Sidebar vertical de tabs

### Qué es
Mover la tab bar de horizontal-arriba a vertical-izquierda. Ancho fijo configurable (default 180px). Las tabs se apilan verticalmente con scroll si hay muchas.

### Por qué primero
Todo lo demás (status bar, pane layout, hit-tests de mouse) depende de la geometría del área de panes. Hay que redefinir `pane_area()` antes de construir encima.

### Archivos a tocar
- `crates/SYNAPSE_-ui/src/layout.rs` — campo nuevo `sidebar_width`, cambio de `pane_area()`
- `crates/SYNAPSE_-ui/src/theme.rs` — constante `SIDEBAR_WIDTH` en lugar de `TAB_BAR_HEIGHT`
- `crates/SYNAPSE_-app/src/render.rs` — reescribir `build_tab_bar_ui_rects` y `build_tab_bar_text`
- `crates/SYNAPSE_-app/src/pane_ops.rs` — hit-tests de click en sidebar (coordenada Y en lugar de X)
- `crates/SYNAPSE_-app/src/mouse.rs` — hover detection vertical

### Cambios concretos

**Layout:**
```rust
// ANTES
pub fn pane_area(&self) -> (f32, f32, f32, f32) {
    (0.0, self.tab_bar_height, self.window_width, self.window_height - self.tab_bar_height)
}

// DESPUÉS
pub fn pane_area(&self) -> (f32, f32, f32, f32) {
    let x = self.sidebar_width;
    let y = 0.0;
    let w = (self.window_width - self.sidebar_width).max(0.0);
    let h = self.window_height;
    (x, y, w, h)
}
```

**Estructura visual de la sidebar:**
```
┌─────────────────────────────────┐
│ [S]  SYNAPSE_           (180px) │  ← logo/título arriba
│─────────────────────────────────│
│ ● ~/dev/synapse         [×]     │  ← tab activa (borde izq neón 3px)
│   SSH: prod             [×]     │  ← tab inactiva
│   build                 [×]     │
│─────────────────────────────────│
│ [+] Nueva tab                   │  ← botón fijo al fondo
└─────────────────────────────────┘
```

**Cada tab en la sidebar:**
- Altura: 36px
- Tab activa: rectángulo con `tab_active_bg` + borde izquierdo 3px `#FF003C`
- Tab inactiva: fondo transparente, texto `tab_text_inactive`
- Hover: overlay `tab_hover_bg`
- Botón ×: aparece en hover, alineado a la derecha
- Texto: CWD truncado o título del proceso (máx 18 chars + ellipsis)

**Hit-test de click (pane_ops.rs):**
```rust
// Ahora el click en sidebar usa coordenada Y
// y está en [0, window_height], x en [0, sidebar_width]
fn handle_sidebar_click(y: f64, layout: &Layout, ...) {
    let tab_h = 36.0_f64;
    let header_h = 48.0_f64; // logo area
    let bottom_btn_h = 36.0_f64;
    
    // Botón "+" (fondo)
    if y >= (layout.window_height - bottom_btn_h) as f64 { /* new tab */ return; }
    
    // Tabs
    let rel_y = y - header_h;
    if rel_y < 0.0 { return; }
    let idx = (rel_y / tab_h).floor() as usize + scroll_offset;
    // activar tab idx...
}
```

**Scroll en sidebar:**  
El scroll pasa a ser vertical (rueda del ratón sobre la sidebar o botones ↑↓ si hay overflow).

### Config TOML nueva
```toml
[ui]
sidebar_width = 180       # px lógicos
sidebar_position = "left" # "left" | "right" (para futura opción)
```

### Tests a escribir/actualizar
- `test_pane_area_with_sidebar` — verifica x=sidebar_width, w=window_width-sidebar_width
- `test_sidebar_hit_test_tab_N` — click en Y correcto activa tab correcta
- `test_sidebar_scroll_offset` — tabs con overflow

### Verificación visual
- Un solo pane cubre todo el espacio a la derecha de la sidebar
- Redimensionar ventana mantiene proporciones correctas
- Click en cada tab la activa y cambia el pane mostrado

---

## Fase B — Tiling de panes estilo Hyprland

### Qué es
El `PaneTree` ya existe (árbol binario de splits). Lo que falta es:
1. **Splits automáticos inteligentes** — al crear un nuevo pane, el split ocurre en la dirección óptima (la mayor dimensión del pane activo), no siempre la misma
2. **Navegación por dirección** — moverse al pane de la izquierda/derecha/arriba/abajo de forma geométrica, no solo por orden de árbol
3. **Resize con teclado** — ajustar el ratio del split con keybinds
4. **Visual de focus claro** — borde activo más prominente, resto atenuado

### Splits automáticos (Ghostty/Hyprland style)
Cuando el usuario hace `Ctrl+Shift+D` o `Ctrl+Shift+E`:
- **Actual:** split siempre vertical o siempre horizontal según el keybind
- **Nuevo:** `Ctrl+Enter` → auto-split (divide el pane activo por su eje mayor)

```rust
pub fn auto_split_direction(rect: &PaneRect) -> SplitDirection {
    if rect.w >= rect.h {
        SplitDirection::Vertical   // pane ancho → partir verticalmente
    } else {
        SplitDirection::Horizontal // pane alto  → partir horizontalmente
    }
}
```

### Navegación geométrica
Actualmente `Alt+Arrows` navega por orden de árbol. Queremos navegación por proximidad física:

```rust
// Dado el rect del pane activo y todos los rects,
// encontrar el pane más cercano en la dirección pedida
pub fn find_pane_in_direction(
    active_rect: &PaneRect,
    direction: Direction, // Left/Right/Up/Down
    all_rects: &[(PaneId, PaneRect)],
) -> Option<PaneId> {
    // Filtrar panes en la dirección correcta
    // Ordenar por distancia al borde activo
    // Devolver el más cercano
}
```

**Lógica de dirección:**
- `Left`: panes donde `rect.x + rect.w <= active.x`, ordenar por `active.x - (rect.x + rect.w)` ASC
- `Right`: panes donde `rect.x >= active.x + active.w`, ordenar por `rect.x - (active.x + active.w)` ASC
- `Up`: panes donde `rect.y + rect.h <= active.y`
- `Down`: panes donde `rect.y >= active.y + active.h`
- Desempate: pane cuyo centro Y (o X) está más próximo al centro del pane activo

### Resize con teclado
```toml
# keybinds nuevos
{ key = "Left",  ctrl = true, shift = true, alt = true, action = "resize_pane_left" }
{ key = "Right", ctrl = true, shift = true, alt = true, action = "resize_pane_right" }
{ key = "Up",    ctrl = true, shift = true, alt = true, action = "resize_pane_up" }
{ key = "Down",  ctrl = true, shift = true, alt = true, action = "resize_pane_down" }
```

Cada press ajusta el ratio del split que contiene el pane activo en ±0.05.

### Visual de focus (Hyprland-style)
- Pane activo: borde 2px `#FF003C` con alpha pulse (si effects ON)
- Panes inactivos: borde 1px `#1A1A1A`, contenido con overlay transparente `rgba(0,0,0,0.15)` para "atenuar" el fondo
- Al cambiar focus: fade rápido 100ms (si effects ON)

**El overlay de atenuación** se implementa como un UIRect adicional sobre el área del pane inactivo:
```rust
if !is_active && pane_count > 1 {
    cached_ui_rects.push(UIRect {
        pos: [rect.x, rect.y],
        size: [rect.w, rect.h],
        color: [0.0, 0.0, 0.0, 0.12], // overlay sutil
    });
}
```

### Keybinds finales de splits
```
Ctrl+Enter          → auto-split (dirección inteligente)
Ctrl+Shift+D        → split vertical explícito
Ctrl+Shift+E        → split horizontal explícito  
Alt+←↑↓→           → navegar pane en dirección geométrica
Ctrl+Shift+Alt+←↑↓→ → resize pane ±5%
Ctrl+Shift+Z        → zoom/maximize pane activo (toggle)
Ctrl+Shift+W        → cerrar pane activo
```

### Tests
- `test_auto_split_direction_wide_pane` — rect 800×400 → Vertical
- `test_auto_split_direction_tall_pane` — rect 400×800 → Horizontal
- `test_find_pane_left` — devuelve el pane correcto a la izquierda
- `test_find_pane_right/up/down`
- `test_resize_ratio_clamp` — ratio nunca < 0.1 ni > 0.9

---

## Fase C — Sidebar polish + indicadores visuales

### Qué es
Con la sidebar funcional (Fase A), añadir el polish visual que la hace sentir cyberpunk.

### Elementos

**Tab activa — acento neón:**
```
│ ▌ ~/dev/synapse    [×] │
  ↑
  borde izq 3px #FF003C con glow (si effects ON: box-shadow equiv en UIRect)
```

El "glow" en UIRect: dos rectángulos extra con color `#FF003C` a alpha decreciente:
```rust
// borde principal
UIRect { pos: [0.0, tab_y], size: [3.0, tab_h], color: [1.0, 0.0, 0.24, 1.0] }
// glow exterior (más ancho, más transparente)
UIRect { pos: [0.0, tab_y], size: [6.0, tab_h], color: [1.0, 0.0, 0.24, 0.3] }
UIRect { pos: [0.0, tab_y], size: [10.0, tab_h], color: [1.0, 0.0, 0.24, 0.1] }
```

**Indicador de proceso activo:**
- Punto `●` de color antes del nombre: verde `#00CC44` si el proceso está vivo, rojo `#FF003C` si terminó
- Actualizarlo cuando el pane recibe `Event::Exit`

**Pane label overlay (Hyprland-style):**
Al cambiar de pane (por teclado o click), mostrar durante 600ms en la esquina superior izquierda del pane activo el ID:
```
┌─────────────────────────
│ P2                      ← label 600ms, fade-out
│
```
Implementar con un `Instant` en `AppState`:
```rust
pub pane_label_until: Option<std::time::Instant>,
pub pane_label_id: u32,
```
En `render.rs`, si `pane_label_until > Instant::now()`, emitir el texto en la posición `(content_x + 4, content_y + 4)`.

**Resize indicator:**
Al arrastrar un divider, mostrar en el centro del pane el texto `80×24` (cols×rows):
```rust
pub dragging_divider: bool,
```
En render, si `dragging_divider`, calcular cols/rows del pane activo y emitir texto centrado.

**Scroll indicator:**
Ya implementado (barra derecha en scrollback). Verificar que funciona con la nueva geometría de Fase A.

### Config TOML nueva
```toml
[ui]
show_pane_labels = true    # overlay P1/P2/... al cambiar foco
show_resize_indicator = true
sidebar_show_process_dot = true
```

---

## Fase D — Status bar

### Qué es
Barra horizontal de 18px en la parte inferior con información contextual. Toggle con `Ctrl+Shift+S`.

### Layout con sidebar + status bar
```
┌──────┬──────────────────────────────┐
│      │                              │
│ SIDE │         PANES                │
│  BAR │                              │
│      ├──────────────────────────────│
│      │ ~/dev  main  k8s:prod  14:22 │  ← status bar
└──────┴──────────────────────────────┘
```

`pane_area()` con status bar activa:
```rust
let h = self.window_height - if self.status_bar_visible { STATUS_BAR_HEIGHT } else { 0.0 };
```

### Contenido de la status bar

**Izquierda:**
- CWD del pane activo (truncado, máx 30 chars, `~/` en lugar de `/home/user/`)
- Branch git: leer `$GIT_BRANCH` o parsear `.git/HEAD` del CWD (async, sin bloquear)
- Contexto k8s: `$KUBECONFIG` actual (si existe)

**Centro:**
- `user@hostname`

**Derecha:**
- Hora `HH:MM:SS` (actualizar cada segundo junto al cursor blink timer)
- (Opcional futuro: CPU%, MEM%)

### Implementación

La status bar es puramente UIRect + texto en `render.rs`. No hay nuevo pipeline.

```rust
pub fn build_status_bar(
    layout: &Layout,
    state: &AppState,
    pane: Option<&Pane>,
) -> (Vec<UIRect>, Vec<(char, f32, f32, f32, [f32;4], [f32;4])>) {
    // fondo
    // texto izq: cwd + git branch
    // texto centro: user@host
    // texto derecha: hora
}
```

Git branch: leer en background thread, guardar en `AppState.git_branch: Option<String>`. Actualizar cuando cambia el pane activo o el CWD.

### Config TOML
```toml
[ui]
status_bar = true
status_bar_show_git = true
status_bar_show_k8s = true
status_bar_show_time = true
```

---

## Fase E — Postproc shaders (CRT + bloom)

### Qué es
El cambio visual más impactante. Pasar de render directo a surface a un pipeline de dos pasos:
1. Render → textura offscreen (HDR R16G16B16A16)
2. Postproc pass → scanlines + vignette + bloom → surface final

### Pipeline nuevo
```
Frame N:
  Pass 0 → offscreen_tex  [bg + cells + ui + cursor]
  Pass 1a → bloom: threshold + downsample + gauss H
  Pass 1b → bloom: gauss V + upsample additive
  Pass 2 → composite: offscreen + bloom + scanlines + vignette + chroma → surface
```

### Shaders a crear

**`postproc.wgsl`** — pass final:
```wgsl
// Scanlines: líneas horizontales periódicas con opacidad configurable
let scanline = sin(uv.y * screen_height * PI) * 0.5 + 0.5;
let scanline_intensity = mix(1.0, scanline, config.scanline_strength);

// Vignette: oscurecer esquinas
let vignette = 1.0 - length(uv - 0.5) * config.vignette_strength;

// Chromatic aberration: split RGB channels
let r = tex.sample(uv + vec2(config.chroma, 0.0)).r;
let g = tex.sample(uv).g;
let b = tex.sample(uv - vec2(config.chroma, 0.0)).b;

color = vec4(r, g, b, 1.0) * scanline_intensity * vignette;
```

**`bloom.wgsl`** — gauss de dos pasos:
```wgsl
// Pass H: convolución horizontal con kernel gauss
// Pass V: convolución vertical, suma al original
// Threshold: solo pixels > bloom_threshold contribuyen
```

### Refactor de `Renderer::draw_frame`
```rust
// ANTES: dibuja directo a surface texture
pub fn draw_frame(&mut self, cells: &[CellInstance], ui_rects: &[UIRect]) -> Result<()>

// DESPUÉS: dibuja a offscreen, luego postproc → surface
pub fn draw_frame_with_postproc(
    &mut self,
    cells: &[CellInstance],
    ui_rects: &[UIRect],
    effects: &EffectsConfig,
) -> Result<()>
```

### Texturas intermedias necesarias
```rust
// En Renderer::new o resize:
offscreen_tex: wgpu::Texture,         // mismo size que surface, R16G16B16A16
bloom_tex_a: wgpu::Texture,           // 1/4 size, para primer bloom pass
bloom_tex_b: wgpu::Texture,           // 1/4 size, para segundo bloom pass
postproc_uniform: wgpu::Buffer,       // config de efectos
```

### Uniform buffer de postproc
```rust
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
pub struct PostprocConfig {
    pub scanline_strength: f32,  // 0.0 = off, 0.3 = default
    pub scanline_freq: f32,      // líneas por pixel, default 2.0
    pub vignette_strength: f32,  // 0.0 = off, 0.4 = default
    pub bloom_threshold: f32,    // 0.7 = solo brillo alto
    pub bloom_intensity: f32,    // 1.0 = default
    pub chroma_strength: f32,    // 0.002 = sutil
    pub _pad: [f32; 2],
}
```

### Config TOML
```toml
[effects]
enabled = true
scanlines = { strength = 0.25, freq = 2.0 }
vignette = { strength = 0.35 }
bloom = { threshold = 0.7, intensity = 1.0, tint = "#FF003C" }
chroma = { strength = 0.002 }
pane_pulse = true
```

### Toggle en runtime
`Ctrl+Shift+E` → `EffectsToggle` action → flip `state.effects_enabled` → si OFF, postproc pass es un simple blit (sin coste).

### Performance target
- Todos efectos OFF: 0 overhead (blit directo)
- Todos efectos ON: 60fps @ 1440p en GPU media (M1/Intel Iris)

### Tests
- `test_postproc_config_serialization` — round-trip TOML
- `test_effects_toggle_action` — flip correcto del estado
- Smoke test visual: iniciar con `effects.enabled = true`, verificar que la terminal arranca sin panic

---

## Fase F — Splash boot + cursor extras

### Splash boot animation

Al arrancar (primera vez que se muestra un frame), mostrar 800ms de animación antes de conectar el PTY.

**Secuencia:**
1. Fondo negro
2. ASCII art `SYNAPSE_` aparece línea por línea con "glitch reveal" (caracteres aleatorios → carácter correcto)
3. Texto `INITIALIZING NEURAL LINK... [OK]` con progreso de chars aleatorios
4. Fade-out → pane normal

**Implementación:**
```rust
pub enum SplashState {
    None,
    Running { start: Instant, phase: SplashPhase },
    Done,
}
```

En el render loop, si `splash_state != Done`, renderizar la splash en lugar de los panes normales. Cualquier tecla → skip a `Done`.

**ASCII art de SYNAPSE_:**
```
 ███████╗██╗   ██╗███╗   ██╗ █████╗ ██████╗ ███████╗███████╗
 ██╔════╝╚██╗ ██╔╝████╗  ██║██╔══██╗██╔══██╗██╔════╝██╔════╝
 ███████╗ ╚████╔╝ ██╔██╗ ██║███████║██████╔╝███████╗█████╗  
 ╚════██║  ╚██╔╝  ██║╚██╗██║██╔══██║██╔═══╝ ╚════██║██╔══╝  
 ███████║   ██║   ██║ ╚████║██║  ██║██║     ███████║███████╗
 ╚══════╝   ╚═╝   ╚═╝  ╚═══╝╚═╝  ╚═╝╚═╝     ╚══════╝╚══════╝
                                                          _
```

### Cursor extras

**Hollow block en pane inactivo** (estándar terminal):
```rust
// En push_cursor_rect, si pane no es activo:
// en lugar de rect sólido, dibujar 4 UIRects de 1px formando el contorno
```

**`neon_underbar` style:**
```toml
cursor_style = "neon_underbar"
```
Underline de 2px + dos UIRects de glow debajo con alpha decreciente. Color `panel_active_border`.

### Config TOML
```toml
[ui]
splash_enabled = true        # false para desactivar
cursor_style = "neon_underbar"  # nuevo valor posible
```

---

## Fase G — Command palette

### Qué es
Overlay flotante (Ctrl+Shift+P) con búsqueda fuzzy sobre todas las acciones disponibles. La pieza de UX que desbloquea descubribilidad de todo lo demás.

### Visual
```
┌────────────────────────────────────────┐
│  🔍  buscar acciones, tabs, temas...   │  ← input
│────────────────────────────────────────│
│  ▶  Nueva tab                Ctrl+T    │
│     Split vertical           Ctrl+D    │
│     Tema: dracula                      │
│     ~/dev/synapse (tab 1)              │
│     Toggle effects           Ctrl+E    │
└────────────────────────────────────────┘
```

### Implementación

El overlay es puro UIRenderer + texto. No hay nuevo pipeline GPU.

**Estado:**
```rust
pub struct PaletteState {
    pub open: bool,
    pub query: String,
    pub results: Vec<PaletteItem>,
    pub selected: usize,
}

pub enum PaletteItem {
    Action { label: String, keybind: Option<String>, action: Action },
    Tab { label: String, index: usize },
    Theme { name: String },
}
```

**Apertura:** `Ctrl+Shift+P` → `state.palette.open = true`
**Navegación:** `↑↓` mueven `selected`, `Enter` ejecuta, `Esc` cierra
**Input de texto:** chars normales van a `palette.query`, `Backspace` los borra

**Fuzzy match:** implementación simple sin librería externa:
```rust
fn fuzzy_score(query: &str, candidate: &str) -> Option<usize> {
    // substring match simple por ahora
    // luego: nucleo-matcher si se añade la dep
    let q = query.to_lowercase();
    let c = candidate.to_lowercase();
    if c.contains(&q) { Some(c.len() - q.len()) } else { None }
}
```

**Render del overlay:**
```
fondo: rect centrado, 600px ancho, posición Y 20% desde arriba
color: theme.search_bar_bg (semi-transparente)
borde: 1px theme.panel_active_border
```

**Theme picker integrado:**
Escribir `theme ` en el query filtra solo temas. Seleccionar uno → `Action::SetTheme("dracula")` → aplicar en runtime sin restart.

### Keybinds nuevos
```toml
{ key = "P", ctrl = true, shift = true, action = "palette_open" }
```

### Tests
- `test_palette_fuzzy_match` — "nvt" matchea "nueva tab"
- `test_palette_navigation` — up/down mueven selected correctamente
- `test_palette_close_on_esc` — estado correcto al cerrar

---

## Checklist de cada fase

Antes de dar una fase por terminada:

- [ ] `cargo build -p SYNAPSE_-app` limpio
- [ ] `cargo test --workspace` — todos en verde
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` limpio
- [ ] `cargo fmt --all -- --check` limpio
- [ ] Smoke test visual: iniciar, hacer las interacciones nuevas, verificar que nada explota
- [ ] Tests nuevos escritos para la funcionalidad de la fase
- [ ] Config TOML documentada en comentarios del struct

---

## Orden de dependencias

```
Fase A (sidebar)
    └→ Fase B (tiling) — depende de la nueva geometría de pane_area
        └→ Fase C (polish sidebar) — depende de A y B estables
            └→ Fase D (status bar) — depende de la geometría final
                └→ Fase E (shaders) — independiente de A-D, puede ir en paralelo
                    └→ Fase F (splash/cursor) — depende de E (efectos)
                        └→ Fase G (palette) — depende de todo lo anterior
```

Fase E puede arrancarse en paralelo a B/C/D si hay capacidad, ya que solo toca el renderer y no el layout.

---

*Última actualización: 2026-05-22*
