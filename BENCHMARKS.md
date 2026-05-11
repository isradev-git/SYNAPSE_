# Luna — Benchmarks

## Metodología

Las métricas se miden en hardware de referencia (Intel i7-12700H, 32GB RAM, SSD NVMe, Linux Ubuntu 22.04, GPU integrada Intel Iris Xe) con el binario de release (`cargo build --release`).

## Cómo medir

```sh
# 1. Compilar release
cargo build --release

# 2. Ejecutar script de benchmark automatizado
chmod +x build/bench.sh
./build/bench.sh release

# 3. Medir FPS manualmente
RUST_LOG=luna::bench=info ./target/release/luna
# → imprime "FPS: 60.0" cada segundo en stderr

# 4. Medir FPS bajo carga
RUST_LOG=luna::bench=info ./target/release/luna &
sleep 2
cat /dev/urandom | head -c 50MB > /tmp/bigfile
cat /tmp/bigfile | timeout 30 ./target/release/luna 2>fps.log
grep "FPS:" fps.log

# 5. Medir RAM
./target/release/luna &
PID=$!
sleep 30
ps -o rss= -p $PID | awk '{print $1/1024 " MB"}'
kill $PID
```

## Métricas Objetivo

| Métrica                         | Objetivo       | Medición           |
|---------------------------------|----------------|--------------------|
| Latencia input → render         | < 5ms          | TBD (typometer)    |
| FPS en uso normal               | 60fps estables | TBD                |
| FPS con output masivo           | ≥ 30fps        | TBD                |
| Tiempo de arranque              | < 200ms        | TBD                |
| Uso de RAM en idle              | < 50MB         | TBD                |
| Uso de RAM con 100.000 líneas  | < 500MB        | TBD                |
| Tamaño de binario release       | —              | 11 MB              |

## Medición de FPS

El binario incluye un contador de frames. Con `RUST_LOG=luna::bench=info` imprime el FPS actual cada segundo.

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
ps -o rss= -p $PID | awk '{print $1/1024 " MB"}'

# Con scrollback lleno
./target/release/luna &
PID=$!
sleep 2
# Rellenar scrollback con yes
for _ in $(seq 1 10); do
    xdotool type "yes 'benchmark fill' | head -n 10000"
    xdotool key Return
    sleep 2
done
sleep 5
ps -o rss= -p $PID | awk '{print $1/1024 " MB"}'
kill $PID
```

## Comparación con competidores

| Terminal   | FPS idle | FPS output masivo | RAM idle | Arranque | Binario |
|------------|----------|-------------------|----------|----------|---------|
| Luna       | TBD      | TBD               | TBD      | TBD      | 11 MB   |
| Alacritty  | ~120     | ~40               | ~35MB    | ~150ms   | 5 MB    |
| Kitty      | ~120     | ~50               | ~40MB    | ~180ms   | 15 MB   |
| WezTerm    | ~90      | ~30               | ~45MB    | ~250ms   | 25 MB   |

> **Actualizar** con mediciones reales en máquina con GPU y display.
> Los benchmarks se ejecutan con `./build/bench.sh release`.
