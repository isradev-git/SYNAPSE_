# Instalación de Luna

Luna es un emulador de terminal GPU-accelerated para Linux, macOS y Windows.

## Contenido

- [macOS](#macos)
- [Linux](#linux)
- [Windows](#windows)
- [Compilar desde source](#compilar-desde-source)
- [Configuración inicial](#configuración-inicial)

---

## macOS

Luna tiene binarios para **Apple Silicon (M1/M2/M3/M4)** e **Intel** — el proceso es idéntico en ambos.

### Opción A — Installer automático (recomendado)

Abre Terminal y ejecuta:

```sh
curl -fsSL https://github.com/isradev-git/luna/releases/latest/download/Luna-app-installer.sh | sh
```

El script detecta tu arquitectura, descarga el binario correcto y lo instala en `~/.cargo/bin/luna`.
Como se ejecuta vía `curl | sh`, **macOS no le añade el flag de cuarentena** y Gatekeeper no interviene.

Asegúrate de que `~/.cargo/bin` está en tu PATH. Si no lo está, añade al final de `~/.zshrc`:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
```

### Opción B — Descarga manual desde Releases

1. Ve a [github.com/isradev-git/luna/releases/latest](https://github.com/isradev-git/luna/releases/latest)
2. Descarga el archivo para tu chip:
   - **Apple Silicon**: `Luna-app-aarch64-apple-darwin.tar.xz`
   - **Intel**: `Luna-app-x86_64-apple-darwin.tar.xz`
3. En Terminal, extrae e instala:

```sh
# Apple Silicon
tar xJf Luna-app-aarch64-apple-darwin.tar.xz
sudo install -m755 Luna-app-aarch64-apple-darwin/luna /usr/local/bin/luna

# Intel
tar xJf Luna-app-x86_64-apple-darwin.tar.xz
sudo install -m755 Luna-app-x86_64-apple-darwin/luna /usr/local/bin/luna
```

> **Si usaste el navegador para descargar** (Safari, Chrome…), macOS marca el archivo con un flag de cuarentena. Quítalo antes de ejecutar:
> ```sh
> xattr -dr com.apple.quarantine /usr/local/bin/luna
> ```
> Si descargaste con curl en Terminal, esto no es necesario.

### Opción C — Compilar desde source

Sin cuarentena, sin advertencias. Ver [Compilar desde source → macOS](#macos-1).

---

## Linux

El binario precompilado es un ejecutable `x86_64` enlazado dinámicamente contra **glibc**. Funciona en cualquier distribución moderna x86_64 con las dependencias de runtime instaladas.

> **Alpine Linux y NixOS:** el binario glibc no es compatible. Usa la opción de [compilar desde source](#linux-1).

### Opción A — Installer automático

```sh
curl -fsSL https://github.com/isradev-git/luna/releases/latest/download/Luna-app-installer.sh | sh
```

Instala en `~/.cargo/bin/luna`. Añade `~/.cargo/bin` al PATH si no lo tienes:

```sh
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc && source ~/.bashrc
```

### Opción B — Descarga manual

```sh
curl -LO https://github.com/isradev-git/luna/releases/latest/download/Luna-app-x86_64-unknown-linux-gnu.tar.xz
tar xJf Luna-app-x86_64-unknown-linux-gnu.tar.xz
sudo install -m755 Luna-app-x86_64-unknown-linux-gnu/luna /usr/local/bin/luna
luna
```

### Dependencias de runtime

Necesarias para ejecutar el binario precompilado. Instálalas si Luna falla al arrancar.

```sh
# Ubuntu / Debian / Linux Mint / Pop!_OS
sudo apt install libx11-6 libxkbcommon0 libwayland-client0 libxrandr2 libxi6

# Fedora / RHEL / Rocky Linux / AlmaLinux
sudo dnf install libX11 libxkbcommon wayland-libs-client libXrandr libXi

# Arch Linux / Manjaro / EndeavourOS
sudo pacman -S libx11 libxkbcommon wayland libxrandr libxi

# openSUSE Tumbleweed / Leap
sudo zypper install libX11-6 libxkbcommon0 libwayland-client0 libXrandr2 libXi6
```

Luna usa GPU vía Vulkan. En la mayoría de sistemas el driver ya incluye el loader de Vulkan. Si Luna no arranca con error de GPU:

```sh
# Ubuntu / Debian
sudo apt install libvulkan1

# Fedora
sudo dnf install vulkan-loader

# Arch
sudo pacman -S vulkan-icd-loader
```

---

## Windows

**Installer PowerShell:**
```powershell
irm https://github.com/isradev-git/luna/releases/latest/download/Luna-app-installer.ps1 | iex
```

**Instalador MSI** (recomendado para entornos corporativos):
1. Descargar `Luna-app-x86_64-pc-windows-msvc.msi` desde [Releases](https://github.com/isradev-git/luna/releases/latest)
2. Ejecutar el `.msi` y seguir el asistente
3. El instalador añade `luna` al PATH del sistema automáticamente

**ZIP manual:**
1. Descargar `Luna-app-x86_64-pc-windows-msvc.zip` desde Releases
2. Extraer `luna.exe` a una carpeta en tu PATH (por ejemplo `C:\tools\`)

> **SmartScreen:** como el binario aún no tiene certificado de firma, Windows puede mostrar una advertencia la primera vez. Haz clic en "Más información" → "Ejecutar de todas formas".

---

## Compilar desde source

Requiere [Rust stable](https://rustup.rs/) 1.75+.

### macOS

No requiere dependencias adicionales. Xcode Command Line Tools es suficiente.

```sh
# Instalar Xcode CLT si no lo tienes
xcode-select --install

git clone https://github.com/isradev-git/luna.git
cd luna
cargo build --release -p Luna-app
./target/release/luna
```

El binario compilado localmente no tiene flag de cuarentena — Gatekeeper no interviene.

### Linux

Instala las dependencias de desarrollo antes de compilar:

```sh
# Ubuntu / Debian / Linux Mint / Pop!_OS
sudo apt install libx11-dev libxkbcommon-dev libwayland-dev libxrandr-dev libxi-dev mold

# Fedora / RHEL / Rocky Linux / AlmaLinux
sudo dnf install libX11-devel libxkbcommon-devel wayland-devel libXrandr-devel libXi-devel

# Arch Linux / Manjaro / EndeavourOS
sudo pacman -S libx11 libxkbcommon wayland libxrandr libxi mold

# openSUSE Tumbleweed / Leap
sudo zypper install libX11-devel libxkbcommon-devel wayland-devel libXrandr-devel libXi-devel

# Alpine Linux
sudo apk add libx11-dev libxkbcommon-dev wayland-dev libxrandr-dev libxi-dev

# NixOS
nix-shell -p libX11 libxkbcommon wayland libXrandr libXi
```

```sh
git clone https://github.com/isradev-git/luna.git
cd luna
cargo build --release -p Luna-app
./target/release/luna
```

### Windows

Requiere [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) con el componente "Desarrollo para el escritorio con C++".

```powershell
git clone https://github.com/isradev-git/luna.git
cd luna
cargo build --release -p Luna-app
.\target\release\luna.exe
```

---

## Configuración inicial

Al primer arranque Luna crea el archivo de configuración con valores por defecto:

- **Linux / macOS**: `~/.config/Luna/config.toml`
- **Windows**: `%APPDATA%\Luna\config.toml`

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
