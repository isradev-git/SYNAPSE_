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

# Tema de colores: "luna" | "dracula" | "catppuccin-mocha" | "tokyo-night"
# También acepta nombre de archivo en ~/.config/Luna/themes/<nombre>.toml
theme = "luna"
```

### Temas incluidos

| Nombre | Descripción |
|--------|-------------|
| `luna` | Paleta morada original (por defecto) |
| `dracula` | Tema oscuro Dracula |
| `catppuccin-mocha` | Catppuccin Mocha |
| `tokyo-night` | Tokyo Night |

### Temas personalizados

Crear `~/.config/Luna/themes/mi-tema.toml` con cualquier subconjunto de colores en hex:

```toml
[colors]
bg = "#1e1e1e"
fg = "#d4d4d4"
cursor = "#569cd6"
tab_active_bg = "#007acc"
tab_inactive_bg = "#252526"
panel_active_border = "#007acc"
```

Cualquier campo omitido hereda del tema base especificado en `config.toml`.
Activar con `theme = "mi-tema"` en `config.toml`.

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
