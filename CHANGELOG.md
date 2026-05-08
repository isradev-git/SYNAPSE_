# Changelog

Los cambios significativos se documentan aquí siguiendo el formato [Keep a Changelog](https://keepachangelog.com).

## [0.1.0] — Sin publicar

### Añadido (Fase 0-5)
- Terminal GPU-accelerated con wgpu (Vulkan/Metal/DirectX 12)
- PTY nativo con shell detection (bash, zsh, fish, cmd.exe)
- Parser VT100/xterm completo (C0, CSI, SGR, OSC, ESC)
- Grid de celdas con scrollback (100.000 líneas)
- Texture atlas de glifos con cosmic-text + JetBrains Mono
- Instanced rendering con un solo draw call por frame
- Input de teclado completo (teclas especiales, modificadores, Ctrl+key)
- Scroll con ratón y teclado
- Selección de texto con ratón
- Copiar y pegar (clipboard via arboard, bracketed paste)
- Colores ANSI: 8-color, bright, 256-color, true color (24-bit)
- SGR: bold, italic, underline, blink, inverse, invisible

### Añadido (Fase 5-6)
- Sistema de tabs con barra superior
- Ctrl+T / Ctrl+W / Ctrl+Tab / Ctrl+1..9
- Títulos dinámicos de tabs (OSC 0/2)
- Sistema de splits (árbol binario de paneles)
- Ctrl+Shift+D/E para split vertical/horizontal
- Redimensionado de paneles con ratón (drag de divisores)
- Navegación entre paneles con Ctrl+Shift+↑↓←→
- PTY independiente por panel (CWD heredado)
- Shader de UI para divisores y bordes

### Añadido (Fase 7-8)
- Búsqueda en buffer (Ctrl+Shift+F) con resaltado y navegación
- Búsqueda inversa en historial (Ctrl+R)
- Limpiar pantalla (Ctrl+L)
- Sistema de configuración TOML (~/.config/Luna/config.toml)
- Keybinds personalizables (30 atajos por defecto)
- Ajuste de fuente en runtime (Ctrl+= / Ctrl+- / Ctrl+0)
- Pantalla completa (F11)
- Recarga de configuración en caliente (Ctrl+,)

### Añadido (Fase 9)
- Empaquetado de distribución (cargo-dist)
- CI/CD con GitHub Actions (release.yml + ci.yml)
- Empaquetado Linux (.deb, .rpm, AppImage)
- Empaquetado macOS (.app, .dmg)
- Empaquetado Windows (.exe, ZIP, MSI via WiX)

### Añadido (Fase 10)
- Suite de tests de conformidad VT100/xterm (30+ tests)
- Documentación completa (README, CONFIGURATION, KEYBINDS, COMPATIBILITY)
- Benchmarks iniciales
- Fix: scroll de grid corregido (new_line → shift_up)
