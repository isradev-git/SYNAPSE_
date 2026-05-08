# Luna — Benchmarks

## Metodología

Las métricas se miden en hardware de referencia (Intel i7-12700H, 32GB RAM, SSD NVMe, Linux Ubuntu 22.04, GPU integrada Intel Iris Xe) con el binario de release (`cargo build --release`).

## Métricas Objetivo

| Métrica                         | Objetivo       | Medición                |
|---------------------------------|----------------|-------------------------|
| Latencia input → render         | < 5ms          | Timestamp de tecla → frame presentado |
| FPS en uso normal               | 60fps estables | `cat /dev/urandom` por 60s, promedio |
| FPS con output masivo           | ≥ 30fps        | `find /` por 30s, promedio |
| Tiempo de arranque              | < 200ms        | `time luna -e exit` |
| Uso de RAM en idle              | < 50MB         | `ps aux \| grep luna` tras 30s idle |
| Uso de RAM con 100.000 líneas  | < 500MB        | `yes A` por 60s, medir RSS |

## Medición de FPS

```sh
# Output masivo
timeout 60 cat /dev/urandom | ./target/release/luna

# FPS se puede ver en logs: RUST_LOG=info ./target/release/luna
```

## Medición de latencia

1. Ejecutar Luna con `RUST_LOG=trace`
2. Escribir una tecla y medir el tiempo hasta que el frame se presenta
3. La latencia incluye: input → PTY → parser → rasterizer → GPU → present

## Medición de RAM

```sh
# En idle
./target/release/luna &
PID=$!
sleep 30
ps -o rss= -p $PID  # RSS en KB

# Con scrollback lleno
timeout 60 yes A | ./target/release/luna &
PID=$!
sleep 60
ps -o rss= -p $PID
```

## Comparación con competidores

| Terminal   | FPS idle | FPS output masivo | RAM idle | Arranque |
|------------|----------|-------------------|----------|----------|
| Luna       | (TBD)    | (TBD)             | (TBD)    | (TBD)    |
| Alacritty  | ~120     | ~40               | ~35MB    | ~150ms   |
| Kitty      | ~120     | ~50               | ~40MB    | ~180ms   |
| WezTerm    | ~90      | ~30               | ~45MB    | ~250ms   |

> Actualizar con mediciones reales al alcanzar MVP estable.
