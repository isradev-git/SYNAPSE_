# Luna — Proyecto

> Terminal emulator multiplataforma de alto rendimiento, construida en Rust.
> Objetivo: producto comercial a nivel de Ghostty / WezTerm con identidad visual propia.

---

## Visión General

Luna es un emulador de terminal moderno escrito en Rust, con rendering GPU-accelerated,
soporte completo de VT100/ANSI/xterm-256color, sistema de paneles divididos, tabs, historial
persistente y autocompletado inteligente. Distribuido como binario nativo en Windows, macOS y Linux.

Prioridades de diseño, en orden:
1. **Corrección** — comportamiento idéntico al estándar de terminal (VT100/xterm)
2. **Rendimiento** — GPU rendering, latencia de input < 5ms, 60fps estables
3. **Experiencia** — UI visual de alta calidad, split de paneles, tabs, shortcuts
4. **Comercialización** — distribución sencilla, actualizaciones, potencial freemium

---

## Competidores de Referencia

| Terminal   | Lenguaje | Lo que aprendemos de ellos               |
|------------|----------|------------------------------------------|
| Ghostty    | Zig      | Máxima performance nativa, diseño limpio |
| Warp       | Rust     | UX moderna, modelo de negocio freemium   |
| WezTerm    | Rust     | Feature-complete, multiplataforma real   |
| Alacritty  | Rust     | Renderer GPU minimalista de referencia   |

---

## Stack Tecnológico

| Componente          | Crate / Tecnología          | Justificación                                    |
|---------------------|-----------------------------|--------------------------------------------------|
| Lenguaje            | Rust (stable, 2021 edition) | Performance, seguridad de memoria, ecosistema    |
| Windowing           | `winit`                     | Estándar cross-platform para ventanas en Rust    |
| GPU Rendering       | `wgpu`                      | WebGPU abstraction: Metal / Vulkan / DX12 / GL   |
| Shaping de texto    | `cosmic-text`               | Unicode, ligaduras, RTL, fuentes variables        |
| PTY                 | `portable-pty`              | PTY cross-platform (mismo que WezTerm)           |
| Parser VT/ANSI      | `vte`                       | Parser VT100/xterm battle-tested                 |
| Async runtime       | `tokio`                     | I/O asíncrono para lectura de PTY                |
| Serialización       | `serde` + `toml`            | Config en formato TOML (como Cargo)              |
| CLI args            | `clap`                      | Argumentos de línea de comandos                  |
| Clipboard           | `arboard`                   | Clipboard cross-platform                         |
| Logging             | `tracing`                   | Logs estructurados para debug y producción       |
| Snapshot testing    | `insta`                     | Snapshot testing para el renderer                |
| Distribución        | `cargo-dist`                | GitHub Releases automatizado, multiplataforma    |

---

## Paleta de Colores

```
#ff3d94  — Rosa neón        → cursor activo, resaltado de selección, prompt activo
#b5307e  — Rosa profundo    → borde de panel activo, tab activa
#6a2a98  — Violeta          → borde de panel inactivo, separadores, tab bar bg
#3f1c6d  — Violeta oscuro   → fondo de paneles inactivos, hover de elementos
#210b4b  — Casi negro       → fondo principal de ventana y paneles activos
```

Colores auxiliares:
```
#ffffff   — Texto principal del buffer
#cccccc   — Texto secundario (historial dimmed)
#ff3d9466 — Selección de texto (rosa con 40% alpha)
#ff3d9422 — Hover sobre elementos UI
```

Colores ANSI estándar (integrados en el tema):
```
Black:   #1a0a35    Red:     #ff4466    Green:   #44ff88    Yellow:  #ffcc44
Blue:    #6644ff    Magenta: #ff44cc    Cyan:    #44ccff    White:   #ffffff
+ variantes bright para cada uno
```

---

## Arquitectura del Sistema

```
Luna/
├── Cargo.toml                     ← Workspace root
├── Cargo.lock
├── .cargo/config.toml             ← Target configs, optimizaciones release
│
├── crates/
│   ├── Luna-app/             ← Binario principal
│   │   └── src/
│   │       ├── main.rs            ← Entry point: init, event loop con render + input
│   │       ├── app.rs             ← AppState (modifiers, selection)
│   │       └── input.rs           ← InputAction enum + mapeo key → acción
│   │
│   ├── Luna-terminal/        ← Lógica de terminal (PTY, VT parser, buffer)
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── pty.rs             ← Abstracción de PTY (portable-pty wrapper)
│   │       ├── parser.rs          ← VT100/ANSI parser (wrapper sobre `vte`)
│   │       ├── grid.rs            ← Grid de celdas (CharCell con atributos)
│   │       ├── buffer.rs          ← Buffer circular de scrollback
│   │       └── shell.rs           ← Detección de shell por OS
│   │
│   ├── Luna-renderer/        ← GPU rendering con wgpu
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── renderer.rs        ← Pipeline principal de wgpu
│   │       ├── atlas.rs           ← Texture atlas de glifos
│   │       ├── text.rs            ← Rasterización con cosmic-text
│   │       ├── cell.rs            ← Render de celdas individuales
│   │       └── shaders/
│   │           ├── cell.wgsl      ← Shader de celdas de texto
│   │           └── cursor.wgsl    ← Shader de cursor animado
│   │
│   ├── Luna-ui/              ← Sistema de paneles, tabs, layout
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── tab_bar.rs         ← Sistema de tabs
│   │       ├── pane.rs            ← Panel individual
│   │       ├── splitter.rs        ← Árbol binario de splits
│   │       ├── layout.rs          ← Cálculo de rectángulos
│   │       └── theme.rs           ← Colores, fuentes, estilos
│   │
│   └── Luna-config/          ← Configuración de usuario
│       └── src/
│           ├── lib.rs
│           ├── config.rs          ← Struct Config + defaults
│           └── keybinds.rs        ← Mapa de atajos personalizable
│
├── assets/
│   ├── fonts/
│   │   ├── JetBrainsMono-Regular.ttf
│   │   ├── JetBrainsMono-Bold.ttf
│   │   └── JetBrainsMono-Italic.ttf
│   └── icons/
│       ├── Luna.ico          ← Windows
│       ├── Luna.icns         ← macOS
│       └── Luna.png          ← Linux (512×512)
│
├── build/
│   ├── build-win.ps1
│   ├── build-mac.sh
│   └── build-linux.sh
│
└── dist/                          ← Output de builds (gitignored)
```

---

## Arquitectura de Datos Clave

### CharCell — Celda del grid de terminal

```rust
pub struct CharCell {
    pub c: char,           // Carácter Unicode
    pub fg: Color,         // Color de foreground (RGBA u8×4)
    pub bg: Color,         // Color de background (RGBA u8×4)
    pub flags: CellFlags,  // Atributos visuales
}

bitflags! {
    pub struct CellFlags: u8 {
        const BOLD      = 0b00000001;
        const ITALIC    = 0b00000010;
        const UNDERLINE = 0b00000100;
        const BLINK     = 0b00001000;
        const INVERSE   = 0b00010000;
        const INVISIBLE = 0b00100000;
    }
}
```

### PaneTree — Árbol binario de paneles

```rust
pub enum PaneTree {
    Leaf(PaneId),
    Split {
        direction: SplitDirection,
        ratio: f32,           // 0.0..1.0
        first: Box<PaneTree>,
        second: Box<PaneTree>,
    }
}

pub enum SplitDirection { Horizontal, Vertical }
```

---

## Pipeline de Rendering

```
PTY output (bytes raw)
        ↓
   VTE parser
        ↓
CharCell updates en Grid  ←── dirty tracking por celda
        ↓
Glyph rasterization (cosmic-text → bitmap por glifo)
        ↓
Texture atlas upload (wgpu, solo glifos nuevos)
        ↓
Instanced draw call (una instancia por celda visible)
        ↓
Present → swap chain → pantalla
```

El renderer usa **instanced rendering**: un único draw call por frame que envía
todas las celdas como instancias GPU, con posición, UV de atlas y color como
vertex attributes. Mínimo de overhead de CPU.

---

## Detección de Shell por OS

```
Windows → cmd.exe  (con soporte de PowerShell como alternativa)
macOS   → $SHELL   → zsh > bash > sh
Linux   → $SHELL   → bash > zsh > sh
```

Configurable en `config.toml` con `[shell] program = "/usr/bin/fish"`.

---

## Atajos de Teclado

| Atajo                  | Acción                          |
|------------------------|---------------------------------|
| Ctrl+T                 | Nueva tab                       |
| Ctrl+W                 | Cerrar tab activa               |
| Ctrl+1..9              | Ir a tab N                      |
| Ctrl+Tab               | Siguiente tab                   |
| Ctrl+Shift+Tab         | Tab anterior                    |
| Ctrl+Shift+D           | Split vertical                  |
| Ctrl+Shift+E           | Split horizontal                |
| Ctrl+Shift+W           | Cerrar panel activo             |
| Ctrl+Shift+↑↓←→        | Mover foco entre paneles        |
| Ctrl+Shift+Alt+↑↓←→    | Redimensionar panel activo      |
| Ctrl+Shift+C           | Copiar selección                |
| Ctrl+Shift+V           | Pegar                           |
| Ctrl+C                 | Interrumpir proceso (SIGINT)    |
| Ctrl+L                 | Limpiar pantalla                |
| Ctrl++                 | Aumentar tamaño de fuente       |
| Ctrl+-                 | Reducir tamaño de fuente        |
| Ctrl+0                 | Tamaño de fuente por defecto    |
| Ctrl+Shift+F           | Buscar en buffer                |
| Ctrl+R                 | Búsqueda en historial           |
| F11                    | Fullscreen toggle               |
| Ctrl+,                 | Abrir config en editor          |

Todos los atajos son reconfigurables en `config.toml`.

---

## Configuración de Usuario

**Ruta:**
- Linux/macOS: `~/.config/Luna/config.toml`
- Windows: `%APPDATA%\Luna\config.toml`

```toml
[font]
family = "JetBrains Mono"
size = 14.0
ligatures = true

[window]
opacity = 1.0
decorations = "full"          # full | minimal | none
startup_size = [1280, 800]

[scrollback]
lines = 100000

[shell]
program = ""                  # vacío = autodetectar según OS
args = []

[theme]
name = "Luna-default"    # extensible con temas custom en themes/

[cursor]
style = "block"               # block | beam | underline
blink = true
blink_interval_ms = 500

[keybinds]
# "ctrl+shift+t" = "new_tab"
# "ctrl+shift+d" = "split_vertical"
```

---

## Distribución y Comercialización

### Canales de distribución
- **GitHub Releases** — binarios para los 3 OS (via `cargo-dist`)
- **Homebrew** — `brew install Luna` (macOS/Linux)
- **winget / Scoop** — Windows
- **Flatpak / .deb / .rpm** — Linux
- **Microsoft Store / Mac App Store** — fase posterior

### Modelo de negocio (propuesta)
- **Free**: terminal completa, funcionalidad core
- **Pro**: temas premium, sync de configuración en la nube, soporte prioritario
- **Team**: gestión centralizada de configuración para equipos, SSO

### Firma de binarios
- macOS: Apple Developer Certificate + notarización obligatoria
- Windows: Code Signing Certificate EV (evitar SmartScreen)
- Linux: GPG signing de releases

---

## Métricas de Calidad Objetivo

| Métrica                         | Objetivo       |
|---------------------------------|----------------|
| Latencia input → render         | < 5ms          |
| FPS en uso normal               | 60fps estables |
| FPS con output masivo           | ≥ 30fps        |
| Tiempo de arranque              | < 200ms        |
| Uso de RAM en idle              | < 50MB         |
| Líneas de scrollback soportadas | 100.000        |
| Conformidad VT/xterm            | vttest passing |

---

## Conformidad con Estándares de Terminal

Implementación objetivo:
- VT100 / VT220 / VT320
- xterm-256color
- True color (24-bit, `COLORTERM=truecolor`)
- Bracketed paste mode
- Mouse reporting (X10, SGR, URXVT)
- Focus events
- OSC (títulos de ventana, clipboard, colores dinámicos)
- Kitty keyboard protocol (extensión moderna)
