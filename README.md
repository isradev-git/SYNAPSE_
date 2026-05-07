# Luna

Terminal emulator multiplataforma de alto rendimiento, construida en Rust con GPU rendering via `wgpu`.

## Estado

En desarrollo. Fase 0: scaffolding.

## Stack

- Rust (edition 2021)
- winit + wgpu (windowing + GPU)
- cosmic-text (text shaping)
- portable-pty + vte (PTY + VT parser)
- tokio (async I/O)

## Cómo construir

```sh
cargo build --release
```

El binario se genera en `target/release/luna`.
