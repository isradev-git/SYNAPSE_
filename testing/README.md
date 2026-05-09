# Luna Test Suite

> Infraestructura completa de testing para el proyecto Luna — terminal emulator GPU-accelerated.

---

## Estructura

```
testing/
├── README.md                    ← Este documento
├── run_all.sh                   ← Orquestrador principal (ejecuta todas las fases)
├── scripts/
│   ├── toolchain_check.sh       ← Verifica toolchain de Rust
│   ├── build_check.sh           ← Compilación debug + release + cargo check
│   ├── unit_tests.sh            ← Tests unitarios por crate con estadísticas
│   ├── lint_check.sh            ← cargo fmt + clippy + auditoría de stubs
│   ├── vt_conformance.sh        ← Tabla de conformidad VT100/xterm
│   ├── integration_test.sh      ← Escenarios de integración cross-crate
│   ├── dependency_audit.sh      ← cargo-deny, cargo-outdated, cargo-audit
│   ├── coverage.sh              ← Cobertura con llvm-cov o tarpaulin
│   └── bench_quick.sh           ← Benchmarks (startup, binary size, compile time)
└── reports/                     ← Resultados de ejecuciones (gitignored)
    └── .gitkeep
```

## Uso rápido

```sh
# Ejecutar todas las fases
./testing/run_all.sh

# Solo una fase específica
./testing/run_all.sh --phase 3        # Solo unit tests

# Modo rápido (salta benchmark y cobertura)
./testing/run_all.sh --quick

# Salida verbosa
./testing/run_all.sh --verbose

# Ejecutar un script individual
./testing/scripts/unit_tests.sh
```

## Requisitos previos

### Obligatorio
- **Rust toolchain** (`rustc` + `cargo`, instalado via [rustup](https://rustup.rs))
- **rustfmt** y **clippy** (`rustup component add rustfmt clippy`)

### Opcional pero recomendado
```sh
# Auditoría de dependencias
cargo install cargo-deny cargo-audit cargo-outdated

# Cobertura de código
cargo install cargo-llvm-cov     # Recomendado (LLVM source-based)
# o
cargo install cargo-tarpaulin    # Alternativa

# Hot-reload de tests durante desarrollo
cargo install cargo-watch
```

## Fases de la test suite

### Fase 1 — Toolchain Check
Verifica que Rust está instalado, versión mínima, componentes (fmt, clippy) y targets de cross-compilación.

**Objetivo:** `rustc` ≥ 1.56.0 (edition 2021), `cargo` presente.

### Fase 2 — Build Check
Compila todos los crates en modo debug y release, y ejecuta `cargo check --all-targets`.

| Crate | Tipo | Descripción |
|-------|------|-------------|
| `Luna-app` | bin | Entry point, event loop |
| `Luna-terminal` | lib | PTY, parser VT100, grid, buffer |
| `Luna-renderer` | lib | wgpu, atlas, text shaping, cell rendering |
| `Luna-ui` | lib | Tabs, panes, layouts, splitters |
| `Luna-config` | lib | TOML config, keybinds |

### Fase 3 — Unit Tests
Ejecuta tests por crate con conteo de passed/failed.

| Crate | Tests actuales | Cobertura |
|-------|---------------|-----------|
| `Luna-terminal` | 73 | parser (44), grid (9), buffer (3), pty (2), shell (1) + nuevos |
| `Luna-renderer` | 2 | atlas (1), text (1) |
| `Luna-ui` | 33 | splitter (6), tab_bar (14), layout (13) |
| `Luna-config` | 26 | config (8), keybinds (18) |

### Fase 4 — Lint Check
- `cargo fmt --check` — verifica formato consistente
- `cargo clippy --workspace --all-targets` — warnings y errores de estilo
- Escaneo de `todo!()` / `unimplemented!()` para detectar regresiones
- Verificación de permisos de archivos

### Fase 5 — VT100/Xterm Conformance
Ejecuta los 44 tests de parser VT100 y genera un reporte tabular de conformidad con el estándar:

- C0 controls (CR, LF, BS, TAB, FF)
- CSI cursor (CUU, CUD, CUF, CUB, CUP)
- CSI erase (ED, EL)
- SGR (8-color, bright, 256-color, true color)
- SGR attributes (bold, italic, underline, blink, inverse)
- OSC (title, CWD)
- Edge cases (UTF-8, clamp, wrap)

### Fase 6 — Integration Tests
Escenarios que cruzan múltiples crates:

1. PTY round-trip (spawn + write + read)
2. VT sequence pipeline (parse → grid → buffer)
3. Config round-trip (load → save → reload)
4. Keybind lookup (defaults + overrides)
5. PaneTree operations (split + layout + close)
6. Grid scrollback overflow (100K+ lines)
7. Dependency tree verification
8. Documentation generation

### Fase 7 — Dependency Audit
- **cargo-deny**: advisory + license + bans (si está instalado)
- **cargo-audit**: vulnerabilidades RustSec
- **cargo-outdated**: dependencias desactualizadas
- **cargo tree**: visualización del árbol de dependencias

### Fase 8 — Coverage
Genera reporte de cobertura de código con `cargo-llvm-cov` (preferido) o `cargo-tarpaulin`.

Indica qué crates tienen baja cobertura para priorizar esfuerzo de testing.

### Fase 9 — Benchmarks
Mediciones rápidas de rendimiento:

| Métrica | Target | Medición |
|---------|--------|----------|
| Startup time | < 200ms | Media de 5 ejecuciones |
| Binary size | N/A | `stat` del binario release |
| Compile time | N/A | Build desde clean |
| Test suite time | N/A | `cargo test --workspace` |

## Resultados

Los reports se guardan en `testing/reports/` con timestamp:

```
reports/
├── build_check.log
├── clippy_check.log
├── fmt_check.log
├── unit_tests.log
├── vt_conformance.log
├── vt_conformance_report.txt
├── integration_test.log
├── dependency_audit.log
├── coverage.log
├── coverage_report.txt
├── coverage.lcov
├── bench_quick.log
├── bench_quick_report.txt
└── test-run_20260509_170000.txt
```

Añadir `testing/reports/*.log` y `testing/reports/*.lcov` al `.gitignore`.

## Desarrollo — hot-reload de tests

```sh
# Re-ejecutar tests al modificar código
cargo watch -x test

# Solo un crate específico
cargo watch -x "test -p Luna-terminal"

# Con output visible
cargo watch -x "test -p Luna-terminal -- --nocapture"
```

## Añadir nuevos tests

### Test unitario (dentro del crate)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mi_feature() {
        let result = mi_funcion();
        assert_eq!(result, expected);
    }
}
```

### Test de integración (cross-crate)
Crear `crates/<crate>/tests/<nombre>.rs`:
```rust
use luna_terminal::grid::Grid;
use luna_terminal::parser::VteProcessor;

#[test]
fn test_cross_crate_scenario() {
    let mut grid = Grid::new(80, 24);
    let mut proc = VteProcessor::new();
    // ...
}
```

## CI/CD

La test suite está diseñada para correr en CI. El workflow `.github/workflows/ci.yml` ejecuta:

```yaml
- cargo build --workspace
- cargo test --workspace
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets -- -D warnings
```

Para replicar exactamente el comportamiento de CI localmente:

```sh
./testing/run_all.sh --quick --verbose
```

## Historial de tests añadidos

| Fecha | Archivo | Tests añadidos |
|-------|---------|---------------|
| 2026-05-09 | `Luna-config/src/config.rs` | 8 tests (defaults, serde, save/load, override) |
| 2026-05-09 | `Luna-config/src/keybinds.rs` | 18 tests (lookup, overrides, named keys, navigation) |
| 2026-05-09 | `Luna-ui/src/tab_bar.rs` | 14 tests (crud, circular nav, titles, id increment) |
| 2026-05-09 | `Luna-ui/src/layout.rs` | 13 tests (pane area, tab width, visible range, scroll) |
