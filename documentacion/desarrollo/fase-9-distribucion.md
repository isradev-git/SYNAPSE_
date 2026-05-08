# Fase 9 — Distribución y Empaquetado

## Arquitectura

```
├── .github/
│   └── workflows/
│       ├── release.yml    ← cargo-dist generated: tag trigger → build all → GitHub Release
│       └── ci.yml         ← PR trigger: test + lint (fmt, clippy) on ubuntu/macos/windows
├── build/
│   ├── build-linux.sh     ← .deb + .rpm + AppImage
│   ├── build-mac.sh       ← .app bundle + .dmg
│   └── build-win.ps1      ← .exe + ZIP portable
├── dist-workspace.toml    ← cargo-dist config (targets, installers, CI)
└── crates/Luna-app/wix/
    └── main.wxs           ← WiX MSI installer for Windows (auto-generated)
```

---

## T-041 — Configurar cargo-dist

**Herramienta:** `dist` (cargo-dist v0.31.0) — binario `dist` en `~/.cargo/bin/`

### Inicialización

```sh
dist init --yes --hosting github
```

Genera:
- `[profile.dist]` en `Cargo.toml` (hereda de release, lto = "thin")
- `dist-workspace.toml` con configuración del proyecto
- `dist generate` crea `.github/workflows/release.yml` + `wix/main.wxs`

### Configuración final (`dist-workspace.toml`)

```toml
[dist]
cargo-dist-version = "0.31.0"
ci = "github"
installers = ["shell", "powershell", "homebrew", "msi"]
targets = [
    "aarch64-apple-darwin",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
]
hosting = "github"
```

### Plan de release

`dist plan` muestra lo que se construirá:

| Artifact | Target | Tipo |
|----------|--------|------|
| `Luna-app-*-apple-darwin.tar.xz` | x86_64 + aarch64 macOS | Binary tarball |
| `Luna-app-*-linux-gnu.tar.xz` | x86_64 + aarch64 Linux | Binary tarball |
| `Luna-app-*-windows-msvc.zip` | x86_64 Windows | ZIP |
| `Luna-app-installer.sh` | Linux/macOS | Shell installer |
| `Luna-app-installer.ps1` | Windows | PowerShell installer |
| `Luna-app.rb` | macOS/Linux | Homebrew formula |
| `Luna-app.msi` | Windows | WiX MSI |
| `source.tar.gz` | Source | Tarball |

### Ajustes en Cargo.toml

Añadidos a todos los crates: `repository.workspace`, `homepage.workspace`.  
Workspace package ahora incluye `homepage = "https://github.com/Luna/Luna"` y `description`.

---

## T-042 — Empaquetado macOS

**Script:** `build/build-mac.sh`

### Estructura del .app bundle

```
Luna.app/
└── Contents/
    ├── Info.plist
    ├── MacOS/
    │   └── Luna         ← binario compilado
    └── Resources/       ← iconos (placeholder)
```

### Info.plist

```xml
<key>CFBundleIdentifier</key>  <string>com.luna.app</string>
<key>CFBundleName</key>        <string>Luna</string>
<key>NSHighResolutionCapable</key>  <true/>
<key>LSMinimumSystemVersion</key>   <string>12.0</string>
```

### .dmg (opcional)

Si `create-dmg` está instalado (`brew install create-dmg`), el flag `--dmg` genera un `.dmg` con ícono de app y enlace a `/Applications`.

### Firma y notarización

No implementado (requiere Apple Developer account). Placeholder documentado en el script.

---

## T-043 — Empaquetado Windows

**Script:** `build/build-win.ps1`  
**MSI:** `crates/Luna-app/wix/main.wxs` (generado por cargo-dist)

### ZIP portable

`Compress-Archive` empaqueta `luna.exe` en un ZIP portable.

### MSI installer (WiX)

`main.wxs` define:
- Instalación en `%ProgramFiles%\Luna-app\bin\luna.exe`
- Añade al `PATH` del sistema automáticamente
- Página de bienvenida, personalización de features
- Upgrade code fijo para actualizaciones futuras
- GUIDs únicos generados por `cargo wix`

El MSI se construye en CI (GitHub Actions) — no localmente a menos que WiX Toolset esté instalado.

### Firma de código

No implementado (requiere Code Signing Certificate EV). Placeholder.

---

## T-044 — Empaquetado Linux

**Script:** `build/build-linux.sh`

### .deb (Debian/Ubuntu)

```
luna-0.1.0-amd64.deb
├── DEBIAN/control          ← nombre, versión, dependencias
├── usr/bin/luna             ← binario
└── usr/share/applications/
    └── luna.desktop         ← integración con menú del sistema
```

Dependencias: `libx11-6`, `libxkbcommon0`, `libwayland-client0`

### .rpm (Fedora/RHEL)

Generado con `rpmbuild` + archivo `.spec`.  
Estructura equivalente al `.deb`.

### AppImage

Usa `appimagetool` para crear un binario portable autocontenido.  
Estructura AppDir con `AppRun` wrapper + `.desktop` file.

### Construcción manual

```sh
# .deb + .rpm + AppImage
./build/build-linux.sh --release

# Solo .deb
./build/build-linux.sh --deb

# Solo AppImage
./build/build-linux.sh --appimage
```

---

## T-045 — CI/CD con GitHub Actions

### release.yml

**Trigger:** push de tag que coincida con `**[0-9]+.[0-9]+.[0-9]+*` (ej. `v0.1.0`)

**Jobs:**
1. `plan` — `dist plan` para calcular la matriz de builds
2. `build-local-artifacts` — matrix paralela: ubuntu, macos (x86_64 + aarch64 con cross), windows
3. `build-global-artifacts` — shell installers, Homebrew formula, checksums
4. `host` — `gh release create` con artifacts upload
5. `announce` — placeholder para Homebrew PR, etc.

### ci.yml

**Trigger:** push a `main`, pull request a `main`

**Jobs:**

| Job | Runs-on | Qué hace |
|-----|---------|----------|
| `test` | ubuntu-22.04, macos-14, windows-2022 | `cargo build --workspace`, `cargo test --workspace` |
| `lint` | ubuntu-22.04 | `cargo fmt --all -- --check`, `cargo clippy --workspace -- -D warnings` |

Cache de dependencias con `actions/cache@v4` para acelerar builds.

---

## Próxima fase: Fase 10 — Calidad y Conformidad

Tareas pendientes:
- T-046: Test suite de VT100 con vttest
- T-047: Benchmark de rendimiento (FPS, latencia, RAM)
- T-048: Test de compatibilidad por OS
- T-049: Revisión de UX y diseño visual
- T-050: Documentación final (README, CONFIGURATION, KEYBINDS, CHANGELOG)
