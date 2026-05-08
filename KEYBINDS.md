# Luna — Keybinds

Todos los atajos son personalizables via `~/.config/Luna/config.toml`.

## Tabs

| Atajo                  | Acción                        |
|------------------------|-------------------------------|
| Ctrl+T                 | Nueva tab                     |
| Ctrl+W                 | Cerrar tab activa             |
| Ctrl+Tab               | Siguiente tab                 |
| Ctrl+Shift+Tab         | Tab anterior                  |
| Ctrl+1 .. Ctrl+9       | Ir a tab N (1-indexado)       |

## Paneles (Splits)

| Atajo                  | Acción                        |
|------------------------|-------------------------------|
| Ctrl+Shift+D           | Split vertical (izq/der)      |
| Ctrl+Shift+E           | Split horizontal (arr/abajo)  |
| Ctrl+Shift+W           | Cerrar panel activo           |
| Ctrl+Shift+↑↓←→        | Mover foco a panel adyacente  |

## Búsqueda

| Atajo                  | Acción                        |
|------------------------|-------------------------------|
| Ctrl+Shift+F           | Buscar en buffer              |
| Ctrl+R                 | Búsqueda inversa en historial |
| Enter (en búsqueda)    | Siguiente match               |
| Shift+Enter            | Match anterior                |
| Escape (en búsqueda)   | Cerrar búsqueda               |

## Fuente

| Atajo                  | Acción                        |
|------------------------|-------------------------------|
| Ctrl+=                 | Aumentar tamaño de fuente     |
| Ctrl+-                 | Reducir tamaño de fuente      |
| Ctrl+0                 | Restaurar tamaño por defecto  |

## Pantalla

| Atajo                  | Acción                        |
|------------------------|-------------------------------|
| F11                    | Pantalla completa             |
| Ctrl+L                 | Limpiar pantalla              |

## Portapapeles

| Atajo                  | Acción                        |
|------------------------|-------------------------------|
| Ctrl+Shift+C           | Copiar selección              |
| Ctrl+Shift+V           | Pegar                         |

## Scroll

| Atajo                  | Acción                        |
|------------------------|-------------------------------|
| Shift+PgUp             | Scroll hacia arriba           |
| Shift+PgDn             | Scroll hacia abajo            |
| Ctrl+Shift+PgUp        | Ir al inicio del scrollback   |
| Ctrl+Shift+PgDn        | Ir al final del scrollback    |
| Mouse Wheel            | Scroll (línea o página)       |

## Configuración

| Atajo                  | Acción                        |
|------------------------|-------------------------------|
| Ctrl+,                 | Recargar configuración        |

## Personalización

Añadir a `~/.config/Luna/config.toml`:

```toml
[[keybinds]]
key = "n"
ctrl = true
shift = false
alt = false
action = "new_tab"
```

### Acciones disponibles

`search`, `history_search`, `clear_screen`, `new_tab`, `close_tab`, `next_tab`, `prev_tab`,
`tab_switch_1`..`tab_switch_9`, `split_vertical`, `split_horizontal`, `close_pane`,
`navigate_up`, `navigate_down`, `navigate_left`, `navigate_right`,
`font_increase`, `font_decrease`, `font_reset`, `fullscreen`, `copy`, `paste`, `reload_config`

### Nombres de teclas

- Caracteres: `"a"`, `"A"`, `"1"`, `"/"` (case-insensitive match)
- Nombradas: `"Tab"`, `"Escape"`, `"Enter"`/`"Return"`, `"Backspace"`, `"Delete"`, `"Insert"`, `"Home"`, `"End"`, `"PgUp"`, `"PgDn"`
- Flechas: `"Up"`/`"ArrowUp"`, `"Down"`/`"ArrowDown"`, `"Left"`/`"ArrowLeft"`, `"Right"`/`"ArrowRight"`
- Función: `"F1"`..`"F12"`
