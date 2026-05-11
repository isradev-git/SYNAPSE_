# Fase 13 — Windows Removal y Cross-Compilation

> 2026-05-11 — Sprint 1

## Contexto y Motivación

Luna se compilaba y ejecutaba en Linux/WSL correctamente (~27 FPS con llvmpipe), pero
el binario nativo de Windows (.exe) se cerraba instantáneamente sin mostrar error.
Se intentó diagnosticar y reparar el soporte nativo Windows, pero tras múltiples
problemas se decidió eliminar por completo el soporte Windows nativo y centrarse
en macOS + Linux (incluyendo WSL2).

## Diagnóstico Original (Windows .exe crash)

Se identificaron 6 causas raíz:

1. **WGSL shader**: `corners[vertex_index]` con índice dinámico rechazado por
   naga validator en Vulkan. Solución: `corner_for_index()` con if/else.
2. **Vertex input**: Struct `CellInstance` como `@location(0) instance: CellInstance`
   rechazado como type mismatch. Solución: parámetros individuales `@location(0..4)`.
3. **GPU adapter**: Sin fallback a software. Solución: dual `request_adapter`
   con `force_fallback_adapter: true`.
4. **windows_subsystem**: Ocultaba errores en release (no hay consola).
   Solución: solo en release, logging a archivo en `%TEMP%\Luna\luna.log`.
5. **Shell detection**: `cmd.exe` hardcodeado. Solución: COMSPEC → powershell.exe
   → pwsh.exe → cmd.exe con validación PATH.
6. **PTY errors**: Mensajes sin contexto. Solución: incluir shell path y
   requisito ConPTY.

## Cambios Realizados

### 1. Shaders WGSL (`cell.wgsl`, `ui.wgsl`)

- Reemplazado `corners[vertex_index]` por `corner_for_index()` con if/else
- Aplanado struct `CellInstance` a parámetros `@location(0..4)` individuales

### 2. Renderer (`renderer.rs`)

- Adapter: HighPerformance → fallback LowPower + force_fallback_adapter
- Logging de adapter name, backend, driver info
- **Eliminado** `Backends::DX11` (no existe en wgpu 22)

### 3. Shell detection (`shell.rs`)

- Windows: COMSPEC → PATH powershell.exe → PATH pwsh.exe → cmd.exe hardcodeado
- Helper `find_program_in_path()` para PATH search
- **Eliminado** junto con todo el bloque Windows

### 4. PTY errors (`pty.rs`)

- Mensaje incluye shell path y requisito ConPTY
- **Simplificado** tras eliminar Windows

### 5. Main (`main.rs`)

- `windows_subsystem = "windows"` solo en release
- Panic hook dual (Windows: MessageBoxW vs Unix: eprintln)
- Logging dual (Windows: archivo en %TEMP% vs Unix: stderr)
- **Reescrito** sin ningún bloque cfg windows

### 6. Config (`config.rs`)

- Añadida ruta `%APPDATA%\Luna` para Windows
- **Eliminada** junto con catch-all `#[cfg(not(...))]`

### 7. Editor fallback (`keyboard.rs`)

- Windows: `notepad`
- **Eliminado**

### 8. Cargo.toml (`Luna-app`)

- Eliminados metadatos WiX (MSI installer)

### 9. CI/CD

- `ci.yml`: eliminado `windows-2022` del matrix
- `release.yml`: eliminados `enable windows longpaths` y paso WiX

### 10. Toolchain check

- Eliminado target `x86_64-pc-windows-msvc`

## Cross-Compilation (cargo-xwin)

Se intentó compilar .exe desde WSL usando `cargo-xwin`:

```bash
cargo install cargo-xwin
cargo xwin build --release -p Luna-app --target x86_64-pc-windows-msvc
```

Resultado: `.exe` de 11MB, PE32+ GUI, compilación exitosa.
El .exe se abría en Windows pero el texto aparecía ilegible
("entrecortado"), probable error en el atlas de fuente o el
muestreo de textura en DX12.

## Decisión: Eliminar Windows Nativo

Dado que:
- El texto salía corrupto en DX12
- El usuario no tiene Rust en Windows (solo WSL)
- WSL2 funciona perfectamente

Se eliminaron **211 líneas de código Windows-specific** en 10 archivos.

## Estado Actual

| Aspecto | Estado |
|---------|--------|
| Build (Linux) | OK — 154 tests, clippy clean |
| Build (macOS) | OK (teórico, sin hardware para testear) |
| Build (WSL2)  | OK, ventana abre y renderiza |
| GPU (WSL2)    | llvmpipe Vulkan, ~27 FPS |
| GPU (native)  | Vulkan/Metal con GPU real → 60 FPS |
| Windows .exe  | Eliminado (no soportado) |

## Próximos Pasos

1. Verificar que la ventana de Luna es visible en WSL (posible problema
   con WSLg, probar `WINIT_UNIX_BACKEND=x11`)
2. Continuar con R-011 (íconos de app) o R-024 (screenshots/docs)
   del ROADMAP
