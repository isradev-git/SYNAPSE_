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
