# Fase F — Splash Boot + Cursor Extras

**Completada:** 2026-05-22  
**Impacto:** 🟢 Medio — polish  
**Tiempo estimado:** 2 días  
**Depende de:** Fase E (shaders)

## Resumen

Dos features de polish: splash screen animada durante el boot de la app y mejoras al cursor de terminal (hollow block en panes inactivos + nuevo estilo neon_underbar con glow).

## 1. Splash Screen

Ya implementada previamente como parte del pipeline de boot. Al arrancar, SYNAPSE_ muestra un overlay con el logo y una barra de progreso animada que se desvanece con fade-out al completar la inicialización.

### Implementación

- **Archivo:** `crates/SYNAPSE_-app/src/render.rs` — función `build_splash_frame()`
- **Archivo:** `crates/SYNAPSE_-app/src/main.rs` — flag `AppState.splash_visible` + lógica de transición

### Comportamiento

1. Al lanzar la app, `splash_visible = true`. El frame se reemplaza completamente por el splash overlay.
2. Se renderiza:
   - Fondo sólido `#11131a` (mismo que el clear color del render)
   - Logo "SYNAPSE_" centrado, tamaño 48px, color `#7098cc` (azul acero)
   - Tagline "terminal reimagined" debajo, tamaño 16px, color `#737a8c`
   - Barra de progreso horizontal: borde `#222739`, fill `#7098cc`, ancho 300px
   - Micro-copy tipo "initializing renderer...", "loading config...", "spawning shell..."
3. Cada etapa del boot avanza la barra (0→0.25→0.5→0.75→1.0) con transición easing.
4. Al llegar a 1.0, inicia fade-out de 500ms (`splash_alpha` baja de 1.0 a 0.0).
5. `splash_visible = false` al terminar el fade, liberando el render normal.

### Micro-copy rotativo

El texto debajo de la barra de progreso cambia según la etapa:

| Etapa | Texto |
|-------|-------|
| Init | initializing renderer... |
| Config | loading config... |
| PTY | spawning shell... |
| Font | rasterizing glyphs... |
| Done | ready. |

### Estados de AppState

```rust
pub splash_visible: bool,
pub splash_alpha: f32,
pub splash_progress: f32,
pub splash_stage: SplashStage,  // enum: Init | Config | PTY | Font | Done
pub splash_fade_start: Option<Instant>,
```

## 2. Hollow Block Cursor en Panes Inactivos

Cuando hay múltiples panes (splits), el pane inactivo muestra un cursor de bloque hueco en lugar del cursor sólido normal. Esto da feedback visual inmediato sobre qué pane tiene el foco.

### Implementación

- **Archivo:** `crates/SYNAPSE_-app/src/render.rs` — dentro del bucle de panes en `render_frame()`

### Comportamiento

- **Condición:** solo se renderiza si `tab.active_tab().layouts.len() > 1` (más de un pane)
- **Render:** 4 rectángulos finos (1px de ancho) formando el contorno del bloque del cursor en el pane inactivo
- **Color:** mismo color del cursor del tema pero con alpha 0.35 (semi-transparente)
- **Posición:** hereda la posición (`active.cursor_x`, `active.cursor_y`) del grid del pane inactivo

### Código clave

```rust
// En el bucle de panes, justo después del render del pane activo:
if tab.active_tab().layouts.len() > 1 && pane_id != tab.active_tab().active_pane {
    if let Some(cursor) = current_pane_cursor {
        let (cx, cy) = cursor;
        let fg = theme.cursor; // pero con alpha 0.35
        // 4 rects: top, bottom, left, right borders de 1px
    }
}
```

### Nota de diseño

El hollow cursor se empuja directamente a `cached_ui_rects` durante el bucle de panes en vez de modificar `push_cursor_rect()`. Esto evita cambiar la firma de la función existente que solo recibe la posición del cursor activo.

## 3. NeonUnderbar — Nuevo Estilo de Cursor

Nuevo estilo de cursor `neon_underbar`: una barra horizontal (underline) con efecto de glow neón rojo debajo. Inspirado en terminales modernos como Kitty y WezTerm.

### Implementación

- **Enum:** `crates/SYNAPSE_-config/src/config.rs` — variante `NeonUnderbar` en `CursorStyle`
- **Render:** `crates/SYNAPSE_-app/src/render.rs` — función `push_cursor_rect()`
- **TOML:** `cursor_style = "neon_underbar"`

### Renderizado

El efecto se compone de 3 `UIRect`s superpuestos verticalmente:

| Capa | Altura | Alpha | Descripción |
|------|--------|-------|-------------|
| Main bar | 2px | 1.0 | Barra sólida del cursor |
| Glow 1 (inner) | 5px | 0.30 | Primer halo de glow |
| Glow 2 (outer) | 8px | 0.08 | Segundo halo más difuso |

Los rects comparten el mismo origen X, comienzan en la misma Y (la base del cursor), y crecen hacia abajo. El resultado es un degrade vertical de opacidad que simula un glow neón.

```rust
CursorStyle::NeonUnderbar => {
    // Main bar: y=cell_y, h=2px, alpha=1.0
    // Glow 1:  y=cell_y, h=5px, alpha=0.30
    // Glow 2:  y=cell_y, h=8px, alpha=0.08
    // Todos con color base = theme.cursor (#7098cc)
}
```

### Serde

El enum `CursorStyle` usa `#[serde(rename_all = "snake_case")]`, por lo que `NeonUnderbar` serializa como `"neon_underbar"` automáticamente. La deserialización desde TOML también es automática.

```toml
# ~/.config/SYNAPSE_/config.toml
cursor_style = "neon_underbar"
```

### Otros estilos disponibles

| Valor TOML | Variant | Descripción |
|------------|---------|-------------|
| `"block"` | `Block` | Bloque sólido (default) |
| `"underline"` | `Underline` | Barra simple debajo del carácter |
| `"beam"` | `Beam` | Barra vertical (estilo inserción) |
| `"neon_underbar"` | `NeonUnderbar` | Underline con glow neón (nuevo) |

## Tests

### CursorStyle serde round-trip (config.rs)

```rust
#[test]
fn test_cursor_style_serde() {
    // ... tests para Block, Underline, Beam, NeonUnderbar
    let style: CursorStyle = serde_json::from_str("\"neon_underbar\"").unwrap();
    assert_eq!(style, CursorStyle::NeonUnderbar);
    let json = serde_json::to_string(&CursorStyle::NeonUnderbar).unwrap();
    assert_eq!(json, "\"neon_underbar\"");
}
```

### TOML parse (config.rs)

```rust
#[test]
fn test_cursor_style_toml() {
    let config: AppConfig = toml::from_str("cursor_style = \"neon_underbar\"").unwrap();
    assert_eq!(config.cursor_style, CursorStyle::NeonUnderbar);
}
```

### NeonUnderbar render rect count (render.rs)

```rust
#[test]
fn test_neon_underbar_emits_three_rects() {
    state.cursor_style = CursorStyle::NeonUnderbar;
    let mut rects: Vec<UIRect> = Vec::new();
    push_cursor_rect(&mut rects, Some((10.0, 20.0)), true, 8.0, 16.0, &mut state);
    assert_eq!(rects.len(), 3, "NeonUnderbar emits 3 rects (bar + 2 glow layers)");
    assert_eq!(rects[0].size, [8.0, 2.0]);  // main bar
    assert_eq!(rects[1].size, [8.0, 5.0]);  // inner glow
    assert_eq!(rects[2].size, [8.0, 8.0]);  // outer glow
}
```

## Archivos modificados

| Archivo | Cambio |
|---------|--------|
| `crates/SYNAPSE_-config/src/config.rs` | Añadido `NeonUnderbar` al enum `CursorStyle` + tests serde/TOML |
| `crates/SYNAPSE_-app/src/render.rs` | Hollow block cursor en panes inactivos + arm `NeonUnderbar` en `push_cursor_rect()` |
| `docs/desarrollo/SYNAPSE_UI_PHASES.md` | Marcada Fase F como completada |

## Métricas

- **Tests:** 190 pasan (0 fail, 0 skip)
- **Clippy:** clean con `-D warnings`
- **cargo fmt:** clean
- **Líneas añadidas:** ~80 (entre lógica de render, enum variant, y tests)
