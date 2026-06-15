# SYNAPSE_ Benchmarks

Performance targets, how to measure them, and the numbers we can capture in CI
(headless). Frame-timing and FPS require a real display and GPU, so reproduce those
on your own hardware with the built-in profiler (`F12`).

> Measured: Linux x86_64, release build (`cargo build --release`), v1.0.0.
> GPU-dependent rows are marked **measure locally** — they cannot be captured headless.

---

## Targets

| Metric | Target (v1.0) | Stretch |
|--------|---------------|---------|
| Input → render latency | < 5 ms | < 3 ms |
| Steady FPS | 60 | 144 (effects off) |
| FPS under heavy output | ≥ 30 | ≥ 60 |
| Cold startup to first frame | < 200 ms | < 120 ms |
| Idle RAM | < 50 MB | < 40 MB |

---

## Measured (headless / CI)

| Metric | Value | Notes |
|--------|-------|-------|
| Release binary size | ~18 MB | Unstripped, x86_64. `strip` removes ≈30%. |
| CLI startup (config load + parse) | ~20 ms | `synapse_ --version`; excludes window/GPU init. |
| CLI resident memory | ~5 MB | Pre-GPU path; the grid/atlas/GPU buffers are allocated lazily on first frame. |
| Source size | ~26.2k LOC | 5 crates. |
| Test suite | 353 tests | `cargo test --workspace`. |

---

## Measure locally (needs a display + GPU)

### Frame timing / FPS / latency
Press **`F12`** in a running SYNAPSE_ window to toggle the on-screen profiler. It
reports per-frame CPU/GPU time, frame count, and the active draw-call breakdown.

```sh
cargo run --release -p SYNAPSE_-app
# then F12
```

### Throughput (heavy output)
```sh
# Large continuous output — watch the profiler frame time stay under 16.6 ms (60 FPS).
yes "$(python3 -c 'print("x"*200)')" | head -n 2000000
time cat a-very-large-file.txt
```

### Latency micro-pattern
Type in a shell with the profiler open; input-to-present is the gap between the
keystroke and the next presented frame. For rigorous numbers use an external
high-speed camera or a tool like [`typometer`](https://github.com/pavelfatin/typometer).

### Standard corpora
- [`vtebench`](https://github.com/alacritty/vtebench) — `vtebench -b alt-screen-random-write`.
- [`alacritty/vtebench` cursor & scrolling suites] for cross-terminal comparison.

---

## Methodology notes
- Build with `--release`; the debug build is **not** representative (no LTO, debug assertions).
- Effects (`[effects]`) cost GPU time; benchmark with them both on and off.
- On Raspberry Pi / GLES adapters effects are auto-disabled and limits are downleveled —
  expect lower ceilings than desktop Vulkan/Metal.
- Disable `freeze_background_tabs` only when measuring multi-tab rendering; it is on by default.

*Contributions of measured numbers on macOS and Raspberry Pi 5 are welcome —
open a PR adding a row with your hardware and the profiler output.*
