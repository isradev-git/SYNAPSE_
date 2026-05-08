# Luna — Configuración

## Archivo de configuración

```
~/.config/Luna/config.toml      # Linux
~/Library/Application Support/Luna/config.toml  # macOS
%APPDATA%\Luna\config.toml      # Windows
```

Se crea automáticamente al primer arranque con valores por defecto.

## Opciones

```toml
# Tamaño de fuente en puntos (default: 14.0)
font_size = 14.0

# Tamaño inicial de la ventana en píxeles
window_width = 1280
window_height = 800

# Líneas de scrollback (default: 100_000)
scrollback_lines = 100000
```

### Keybinds personalizados

```toml
# Ejemplo: cambiar Ctrl+T → Ctrl+N para nueva tab
[[keybinds]]
key = "n"
ctrl = true
shift = false
alt = false
action = "new_tab"

# Ejemplo: añadir Ctrl+Shift+P como atajo de pegar alternativo
[[keybinds]]
key = "p"
ctrl = true
shift = true
alt = false
action = "paste"
```

Las entradas `[[keybinds]]` sobreescriben los defaults. Si una combinación ya existe, se reemplaza con el nuevo action.

## Ejemplo completo

```toml
font_size = 16.0
window_width = 1920
window_height = 1080
scrollback_lines = 200000

[[keybinds]]
key = "enter"
ctrl = true
action = "fullscreen"

[[keybinds]]
key = "n"
ctrl = true
shift = false
action = "new_tab"
```

## Recarga en caliente

Presionar `Ctrl+,` para recargar `config.toml` sin reiniciar Luna. Los cambios en keybinds y tamaño de fuente se aplican inmediatamente.
