# Fase 2 — PTY y Parser VT

## T-011 · Detección de shell nativo por OS

**Archivo:** `crates/Luna-terminal/src/shell.rs`

```rust
pub struct ShellConfig {
    pub program: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
}

pub fn detect_shell() -> ShellConfig { ... }
```

- `#[cfg(target_os = "windows")]` → `COMSPEC` env var, fallback `C:\Windows\System32\cmd.exe`
- `#[cfg(not(target_os = "windows"))]` → `$SHELL` env var
  - macOS fallback: `/bin/zsh`
  - Linux fallback: `/bin/bash`
- Test: `test_detect_shell_returns_program` — verifica que devuelva path no vacío

## T-012 · Lanzar PTY con portable-pty

**Archivo:** `crates/Luna-terminal/src/pty.rs`

```rust
pub struct PtyHandle {
    pub master: Box<dyn MasterPty + Send>,
    pub writer: Option<Box<dyn Write + Send>>,
    pub child: Box<dyn Child + Send + Sync>,
    pub reader: Option<Box<dyn Read + Send>>,
}

pub struct PtySession {
    pub pty: PtyHandle,
    pub rx: mpsc::UnboundedReceiver<Vec<u8>>,
}
```

- `PtyHandle::spawn(cols, rows, shell)`:
  1. `native_pty_system().openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })`
  2. `CommandBuilder::new(&shell.program)` con args y env
  3. `slave.spawn_command(cmd)` → child process
  4. `master.try_clone_reader()` + `master.take_writer()` → I/O
- `pty.write(data)` → `writer.write_all(data)`
- `pty.resize(cols, rows)` → `master.resize(PtySize { ... })`
- Tests:
  - `test_pty_spawn`: spawn shell, verifica Ok
  - `test_pty_write_and_read`: escribe `echo HELLO_LUNA\n`, espera 200ms, lee output

## T-013 · Lectura asíncrona del PTY con tokio

**Archivo:** `crates/Luna-terminal/src/pty.rs` (dentro de `PtyHandle::start_reader`)

- `std::thread::spawn` (thread de OS, no tokio task) con loop de lectura:
  - Buffer 4096 bytes, `Read::read` bloqueante
  - `tx.send(data)` por `mpsc::unbounded_channel`
  - Maneja `WouldBlock` con sleep 1ms
  - EOF (Ok(0)) o error → break
- Devuelve `PtySession { pty, rx }` donde `pty.reader` se setea a `None`
- El thread de lectura se ejecuta en background, enviando bytes al channel

## T-014 · Parser VT100/ANSI

**Archivo:** `crates/Luna-terminal/src/parser.rs`

```rust
pub struct VteProcessor {
    grid: Rc<RefCell<Grid>>,
    fg: Color,
    bg: Color,
    flags: CellFlags,
}
```

- Implementa `vte::Perform` trait:
  - `print(c)`: setea celda en posición cursor con atributos actuales, avanza cursor
    - Si `INVERSE`: swap fg↔bg automáticamente
  - `execute(byte)`:
    - `\n` → `grid.new_line()`
    - `\r` → `grid.carriage_return()`
    - `\x08` (BS) → cursor col-1
    - `\t` → cursor a siguiente tabstop (cada 8)
    - `\x0c` (FF) → clear display + cursor a (0,0)
  - `csi_dispatch(params, action)`:
    - `A`/`B`/`C`/`D` — cursor up/down/forward/back con clamp
    - `H`/`f` — cursor position absolute (row, col)
    - `J` — erase display (0=end, 1=start, 2=all, 3=scrollback)
    - `K` — erase line (0=end, 1=start, 2=all)
    - `m` — SGR (delega a `handle_sgr`)
    - `s`/`u` — save/restore cursor (DECSC/DECRC)
    - `h`/`l` — no-op (modo SM/RM)
  - `esc_dispatch`:
    - `7`/`8` — save/restore cursor (alternativo a CSI s/u)
    - `c` — RIS (reset + clear completo)

- **SGR completo** en `handle_sgr`:
  - 0: reset, 1: bold, 3: italic, 4: underline, 5/6: blink, 7: inverse, 8: invisible
  - 22-28: remove respectivos flags
  - 30-37: fg 3-bit, 40-47: bg 3-bit
  - 38/48: xterm-256 (`38;5;N`) y true color (`38;2;R;G;B`)
  - 39/49: fg/bg default
  - 90-97: fg bright, 100-107: bg bright

- `get_param(params, idx)`: extrae parámetro de `vte::Params` con default 0
- Tests: 8 tests cubriendo print, newline, cursor movement, SGR (bold green, true color), clear screen, ESC save/restore

## T-015 · Grid de celdas

**Archivo:** `crates/Luna-terminal/src/grid.rs`

```rust
pub struct Grid {
    cells: Vec<CharCell>,
    cols: usize,
    rows: usize,
    cursor_col: usize,
    cursor_row: usize,
    scrollback: ScrollbackBuffer,
    scroll_offset: usize,
    saved_cursor_col: usize,
    saved_cursor_row: usize,
}
```

- `CharCell { c, fg, bg, flags, dirty }` con `Default` → carácter ' ', dirty=true
- `Color` enum: `Default`, `Indexed(u8)`, `Rgb(u8,u8,u8)` con métodos `fg_rgba()`, `bg_rgba()`
- `ansi_256_to_rgba(idx)`: mapeo de 256 colores ANSI a [f32; 4]
  - 0-15: 16 colores estándar (black, red, green, yellow, blue, magenta, cyan, white + bright)
  - 16-231: 6×6×6 cube (216 colores)
  - 232-255: escala de grises (24 tonos)
- `grid.set(col, row, cell)`, `grid.get(col, row) -> &CharCell`
- `grid.advance_cursor()`: col+1, wrap a fila siguiente, scroll si necesario
- `grid.new_line()`: col=0, row+1, scroll si necesario
- `shift_up(n)`: push línea superior al scrollback + shift rows up + empty last row
- `clear_region`, `clear_line`, `clear_line_from_start`
- `resize(new_cols, new_rows)`: copia contenido existente, fill con default
- `dirty_cells() -> Iterator<(col, row, &CharCell)>`: solo celdas con dirty=true
- `clear_dirty()`: reset all dirty flags
- Tests: 6 tests (new, set/get, cursor advance, cursor wrap, scroll up, ansi_256 colors, resize)

## T-016 · Scrollback buffer circular

**Archivo:** `crates/Luna-terminal/src/buffer.rs`

```rust
pub struct ScrollbackBuffer {
    lines: Vec<Option<Vec<CharCell>>>,
    capacity: usize,
    head: usize,
    count: usize,
}
```

- Buffer circular: push en `head`, overflow sobrescribe más antiguo
- `push(line)`: inserta línea, incrementa count hasta capacity
- `get_line(index) -> &[CharCell]`: resuelve índice circular
  - Mapea `start = (count < capacity) ? 0 : head`
  - `actual = (start + index) % capacity`
- `len() -> usize`: count actual (≤ capacity)
- Capacity default: 100.000 líneas
- Tests: 3 tests (push&get, overflow circular, large buffer > capacity mantiene límite)

### Datos clave portable-pty 0.8

- `native_pty_system()` para crear `PtySystem`
- `PtySize { rows, cols, pixel_width, pixel_height }` para dimensiones
- `openpty(size)` → `(MasterPty, SlavePty)`
- `CommandBuilder::new(program)` con `.args()`, `.env()`
- `SlavePty::spawn_command(cmd)` → `Box<dyn Child + Send + Sync>`
- `MasterPty::try_clone_reader()` → `Box<dyn Read + Send>`
- `MasterPty::take_writer()` → `Box<dyn Write + Send>`
- `MasterPty::resize(PtySize)` para resize

### Tests: 18 tests

```sh
cargo test -p Luna-terminal
# shell: 1, pty: 2, parser: 8, grid: 6, buffer: 3
```
