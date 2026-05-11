# Installers & Installation Guides — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Windows to CI, activate cargo-dist native installers (shell/powershell/msi), fix broken URLs, and create a complete installation guide for users.

**Architecture:** cargo-dist handles all artifact generation. We update `dist-workspace.toml` to enable three installer types and add the Windows target. The `release.yml` gets a WiX PATH step for Windows runners. Documentation lives in `INSTALL.md` (new) with `README.md` updated to reference it.

**Tech Stack:** cargo-dist 0.31.0, WiX v3 (pre-installed on `windows-2022` runners), GitHub Actions, Markdown.

---

## File Map

| Action | File |
|--------|------|
| Modify | `dist-workspace.toml` |
| Modify | `crates/Luna-app/wix/main.wxs` |
| Modify | `.github/workflows/release.yml` |
| Modify | `README.md` |
| Modify | `build/build-linux.sh` |
| Create | `INSTALL.md` |

---

## Task 1: Activate installers and add Windows target in dist-workspace.toml

**Files:**
- Modify: `dist-workspace.toml`

- [ ] **Step 1: Open dist-workspace.toml and verify current content**

Current content should be:
```toml
[workspace]
members = ["cargo:."]

[dist]
cargo-dist-version = "0.31.0"
ci = "github"
installers = []
targets = [
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
]
hosting = "github"
pr-run-mode = "plan"
allow-dirty = ["ci"]
```

- [ ] **Step 2: Replace the `[dist]` section with the updated version**

```toml
[workspace]
members = ["cargo:."]

[dist]
cargo-dist-version = "0.31.0"
ci = "github"
installers = ["shell", "powershell", "msi"]
targets = [
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
]
hosting = "github"
pr-run-mode = "plan"
allow-dirty = ["ci"]
```

- [ ] **Step 3: Commit**

```bash
git add dist-workspace.toml
git commit -m "feat(dist): add Windows target and shell/powershell/msi installers"
```

---

## Task 2: Fix wix/main.wxs — product name, URL, license

**Files:**
- Modify: `crates/Luna-app/wix/main.wxs`

The current file has three problems:
1. `Name='Luna-app'` should be `Name='Luna'`
2. `ARPHELPLINK` points to wrong URL `https://github.com/Luna/Luna`
3. License component is commented out — users should get a LICENSE sidecar

- [ ] **Step 1: Fix product Name (line 63)**

Find:
```xml
        Name='Luna-app'
```
Replace with:
```xml
        Name='Luna'
```

- [ ] **Step 2: Fix ARPHELPLINK URL (line 176)**

Find:
```xml
        <Property Id='ARPHELPLINK' Value='https://github.com/Luna/Luna'/>
```
Replace with:
```xml
        <Property Id='ARPHELPLINK' Value='https://github.com/isradev-git/luna'/>
```

- [ ] **Step 3: Uncomment the License component (lines 106–110 and 150)**

Find the commented-out License component block:
```xml
                    <!--
                    <Component Id='License' Guid='*'>
                        <File Id='LicenseFile' Name='ChangeMe' DiskId='1' Source='C:\Path\To\File' KeyPath='yes'/>
                    </Component>
                    -->
```
Replace with (path is relative to `crates/Luna-app/` where cargo-wix runs — two levels up reaches workspace root):
```xml
                    <Component Id='License' Guid='*'>
                        <File Id='LicenseFile' Name='LICENSE' DiskId='1' Source='..\..\LICENSE' KeyPath='yes'/>
                    </Component>
```

- [ ] **Step 4: Uncomment the License ComponentRef (line 150)**

Find:
```xml
            <!--<ComponentRef Id='License'/>-->
```
Replace with:
```xml
            <ComponentRef Id='License'/>
```

- [ ] **Step 5: Commit**

```bash
git add crates/Luna-app/wix/main.wxs
git commit -m "fix(wix): rename product to Luna, fix URL, include LICENSE in MSI"
```

---

## Task 3: Add WiX PATH step to release.yml for Windows runners

**Files:**
- Modify: `.github/workflows/release.yml`

Windows runners (`windows-2022`) have WiX v3 pre-installed but not always on PATH. cargo-dist's `msi` installer type uses `cargo-wix` internally, which needs `candle.exe` and `light.exe` from WiX v3 on PATH.

- [ ] **Step 1: Locate the insertion point**

In `build-local-artifacts` job, find the step that currently reads:
```yaml
      - name: Install system dependencies (Linux)
        if: runner.os == 'Linux'
        run: |
          sudo apt-get update
          sudo apt-get install -y libx11-dev libxkbcommon-dev libwayland-dev libxrandr-dev libxi-dev mold
```

- [ ] **Step 2: Add WiX PATH step immediately after that Linux step**

Insert this block after the Linux dependencies step:
```yaml
      - name: Add WiX Toolset to PATH (Windows)
        if: runner.os == 'Windows'
        shell: pwsh
        run: |
          $wixDir = (Get-ChildItem "C:\Program Files (x86)\WiX Toolset*" |
            Sort-Object Name | Select-Object -Last 1).FullName + "\bin"
          echo $wixDir | Out-File -FilePath $env:GITHUB_PATH -Encoding utf8 -Append
```

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add WiX Toolset to PATH for Windows MSI builds"
```

---

## Task 4: Fix README.md — URL, install section, badges, platforms

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add CI and release badges under the title**

Find the current header:
```markdown
# Luna

> Terminal emulator GPU-accelerated · Rust · wgpu · Multiplataforma
```
Replace with:
```markdown
# Luna

> Terminal emulator GPU-accelerated · Rust · wgpu · Multiplataforma

[![CI](https://github.com/isradev-git/luna/actions/workflows/ci.yml/badge.svg)](https://github.com/isradev-git/luna/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/isradev-git/luna)](https://github.com/isradev-git/luna/releases/latest)
```

- [ ] **Step 2: Update the plataformas line**

Find:
```markdown
- **Plataformas**: macOS (Metal) y Linux (X11/Wayland) — Windows en desarrollo futuro
```
Replace with:
```markdown
- **Plataformas**: macOS (Metal), Linux (X11/Wayland) y Windows (DirectX 12)
```

- [ ] **Step 3: Replace the "Instalación rápida" section**

Find the entire block:
```markdown
## Instalación rápida

```sh
# Desde source (macOS / Linux)
git clone https://github.com/Luna/Luna.git
cd Luna
cargo build --release -p Luna-app
./target/release/luna
```

> **Linux:** requiere dependencias del sistema:
> ```sh
> # Ubuntu/Debian
> sudo apt install libx11-dev libxkbcommon-dev libwayland-dev libxrandr-dev libxi-dev
> # Fedora
> sudo dnf install libX11-devel libxkbcommon-devel wayland-devel
> ```
```
Replace with:
```markdown
## Instalación rápida

```sh
# Linux / macOS
curl -fsSL https://github.com/isradev-git/luna/releases/latest/download/luna-installer.sh | sh

# Windows (PowerShell)
irm https://github.com/isradev-git/luna/releases/latest/download/luna-installer.ps1 | iex
```

También disponible como `.msi` (Windows) y tarballs en la [página de releases](https://github.com/isradev-git/luna/releases/latest).

Ver [INSTALL.md](INSTALL.md) para instrucciones detalladas por plataforma, compilar desde source, y configuración inicial.
```

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs(readme): fix repo URL, update install section, add CI badges, add Windows"
```

---

## Task 5: Fix HOMEPAGE URL in build-linux.sh

**Files:**
- Modify: `build/build-linux.sh`

- [ ] **Step 1: Fix the HOMEPAGE variable (line 48)**

Find:
```bash
HOMEPAGE="https://github.com/Luna/Luna"
```
Replace with:
```bash
HOMEPAGE="https://github.com/isradev-git/luna"
```

- [ ] **Step 2: Commit**

```bash
git add build/build-linux.sh
git commit -m "fix(build): correct repo URL in build-linux.sh"
```

---

## Task 6: Create INSTALL.md — full installation guide

**Files:**
- Create: `INSTALL.md`

- [ ] **Step 1: Create INSTALL.md with the following content**

```markdown
# Instalación de Luna

Luna es un emulador de terminal GPU-accelerated disponible para Linux, macOS y Windows.

## Contenido

- [Binarios precompilados](#binarios-precompilados-recomendado)
  - [Linux](#linux)
  - [macOS](#macos)
  - [Windows](#windows)
- [Compilar desde source](#compilar-desde-source)
- [Configuración inicial](#configuración-inicial)

---

## Binarios precompilados (recomendado)

### Linux

**Installer automático (x86_64):**
```sh
curl -fsSL https://github.com/isradev-git/luna/releases/latest/download/luna-installer.sh | sh
```
Instala el binario en `~/.cargo/bin/luna` y lo añade al PATH.

**Descarga manual:**
1. Ir a [Releases](https://github.com/isradev-git/luna/releases/latest)
2. Descargar `luna-x86_64-unknown-linux-gnu.tar.gz`
3. Extraer e instalar:
```sh
tar xzf luna-x86_64-unknown-linux-gnu.tar.gz
sudo mv luna /usr/local/bin/luna
luna
```

**Dependencias del sistema (requeridas):**
```sh
# Ubuntu / Debian
sudo apt install libx11-6 libxkbcommon0 libwayland-client0

# Fedora / RHEL
sudo dnf install libX11 libxkbcommon wayland-libs-client
```

---

### macOS

**Installer automático (Apple Silicon y Intel):**
```sh
curl -fsSL https://github.com/isradev-git/luna/releases/latest/download/luna-installer.sh | sh
```

**Descarga manual:**
1. Ir a [Releases](https://github.com/isradev-git/luna/releases/latest)
2. Descargar el tarball para tu arquitectura:
   - Apple Silicon (M1/M2/M3): `luna-aarch64-apple-darwin.tar.gz`
   - Intel: `luna-x86_64-apple-darwin.tar.gz`
3. Extraer e instalar:
```sh
tar xzf luna-*.tar.gz
sudo mv luna /usr/local/bin/luna
luna
```

> **Nota:** Si macOS muestra "desarrollador no verificado", ejecutar:
> ```sh
> xattr -dr com.apple.quarantine /usr/local/bin/luna
> ```

---

### Windows

**Installer PowerShell:**
```powershell
irm https://github.com/isradev-git/luna/releases/latest/download/luna-installer.ps1 | iex
```

**Instalador MSI (recomendado para entornos corporativos):**
1. Ir a [Releases](https://github.com/isradev-git/luna/releases/latest)
2. Descargar `luna-x86_64-pc-windows-msvc.msi`
3. Ejecutar el `.msi` y seguir el asistente de instalación
4. El instalador añade `luna` al PATH del sistema automáticamente

**Descarga manual (ZIP):**
1. Descargar `luna-x86_64-pc-windows-msvc.zip` desde Releases
2. Extraer `luna.exe` a una carpeta en tu PATH (por ejemplo `C:\tools\`)

---

## Compilar desde source

Requiere [Rust stable](https://rustup.rs/) (1.75+).

### Linux

```sh
# Instalar dependencias de desarrollo
sudo apt install libx11-dev libxkbcommon-dev libwayland-dev libxrandr-dev libxi-dev mold

# Clonar y compilar
git clone https://github.com/isradev-git/luna.git
cd luna
cargo build --release -p Luna-app
./target/release/luna
```

### macOS

```sh
# Xcode Command Line Tools (si no están instalados)
xcode-select --install

git clone https://github.com/isradev-git/luna.git
cd luna
cargo build --release -p Luna-app
./target/release/luna
```

### Windows

```powershell
# Requiere Visual Studio Build Tools o Visual Studio con componente C++
# https://visualstudio.microsoft.com/visual-cpp-build-tools/

git clone https://github.com/isradev-git/luna.git
cd luna
cargo build --release -p Luna-app
.\target\release\luna.exe
```

---

## Configuración inicial

Al primer arranque, Luna crea `~/.config/Luna/config.toml` (Linux/macOS) o `%APPDATA%\Luna\config.toml` (Windows) con valores por defecto.

### Ejemplo de config.toml

```toml
[font]
font_size = 14.0
font_family = "JetBrains Mono"
font_ligatures = false

[shell]
# shell_program = "/usr/bin/fish"   # descomenta para cambiar de shell

[cursor]
cursor_style = "block"   # block | beam | underline
cursor_blink = true
cursor_blink_ms = 500

[theme]
theme = "luna"   # luna | dracula | catppuccin-mocha | tokyo-night
```

Abre el archivo con `Ctrl+,` desde dentro de Luna.

Ver [CONFIGURATION.md](CONFIGURATION.md) para la referencia completa de opciones.
```

- [ ] **Step 2: Commit**

```bash
git add INSTALL.md
git commit -m "docs: add INSTALL.md with platform installation guides"
```

---

## Task 7: Tag and verify CI

- [ ] **Step 1: Verify all previous tasks are committed**

```bash
git log --oneline -8
git status
```
Expected: clean working tree, 6 new commits since the last release.

- [ ] **Step 2: Push commits to main**

```bash
git push origin main
```
Wait for the CI (`ci.yml`) to pass on ubuntu and macos before tagging.

- [ ] **Step 3: Create and push tag v0.1.0**

```bash
git tag v0.1.0
git push origin v0.1.0
```
This triggers `release.yml`.

- [ ] **Step 4: Monitor the release workflow**

Go to: `https://github.com/isradev-git/luna/actions`

Expected jobs to pass:
- `plan` (ubuntu-22.04)
- `build-local-artifacts (aarch64-apple-darwin)` — macOS arm64
- `build-local-artifacts (x86_64-apple-darwin)` — macOS Intel
- `build-local-artifacts (x86_64-unknown-linux-gnu)` — Linux
- `build-local-artifacts (x86_64-pc-windows-msvc)` — Windows (new)
- `build-global-artifacts` — checksums + shell/powershell scripts
- `host` — uploads to GitHub Release
- `announce`

- [ ] **Step 5: Verify GitHub Release artifacts**

Go to: `https://github.com/isradev-git/luna/releases/latest`

Confirm these files are present:
```
luna-aarch64-apple-darwin.tar.gz
luna-x86_64-apple-darwin.tar.gz
luna-x86_64-unknown-linux-gnu.tar.gz
luna-x86_64-pc-windows-msvc.zip
luna-installer.sh
luna-installer.ps1
luna-x86_64-pc-windows-msvc.msi
*.sha256 checksums
```

- [ ] **Step 6: Smoke-test the shell installer**

On a Linux/macOS machine (or CI runner):
```sh
curl -fsSL https://github.com/isradev-git/luna/releases/latest/download/luna-installer.sh | sh
```
Expected: downloads and installs `luna` binary, prints install path.

- [ ] **Step 7: If any CI job fails**

Common issues:
- **WiX not found on Windows runner**: The `Add WiX Toolset to PATH` step may need `choco install wixtoolset --yes` if the runner doesn't have it pre-installed. Add before the path step:
  ```yaml
  choco install wixtoolset --yes --no-progress
  ```
- **MSI build fails with license path error**: cargo-wix resolves relative paths from the crate directory. Verify the `Source` attribute is `..\..\LICENSE` (2 levels up from `crates/Luna-app/` to workspace root).
- **Windows target cross-compile**: Ensure `x86_64-pc-windows-msvc` is in `dist-workspace.toml` targets and the runner is `windows-2022`.
```

- [ ] **Step 8: Update ROADMAP.md to mark R-011 related items done if applicable**

No new ROADMAP items are covered — this work sits within R-017 (distribution, already marked done) as a quality improvement. No ROADMAP update needed.
