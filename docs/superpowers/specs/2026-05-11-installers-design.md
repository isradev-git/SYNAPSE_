# Design: Installers & Installation Guides

**Date:** 2026-05-11
**Status:** Approved

## Goal

Produce signed, downloadable installers for Linux, macOS, and Windows via GitHub Actions CI, and provide clear installation guides so users can install Luna without building from source.

## Approach

Use cargo-dist's native installer generation (`shell`, `powershell`, `msi`) rather than custom scripts. cargo-dist already manages the release matrix; we extend it to include Windows and activate the installer types.

---

## Section 1 — Distribution config

### `dist-workspace.toml`

Add Windows target and activate all three installer types:

```toml
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

### `crates/Luna-app/wix/main.wxs`

Three fixes:
1. `Name='Luna-app'` → `Name='Luna'`
2. `ARPHELPLINK` URL → `https://github.com/isradev-git/luna`
3. Uncomment the `License` component and point it to `../../../LICENSE`

---

## Section 2 — CI changes

### `release.yml`

Add a conditional step in `build-local-artifacts` to install WiX Toolset on Windows runners. cargo-dist detects WiX in PATH and builds the `.msi` automatically.

```yaml
- name: Install WiX Toolset (Windows)
  if: runner.os == 'Windows'
  run: |
    choco install wixtoolset --yes
    echo "C:\Program Files (x86)\WiX Toolset v3.14\bin" >> $GITHUB_PATH
```

No other changes to `release.yml` — cargo-dist regenerates the build matrix automatically when the new Windows target is present in `dist-workspace.toml`.

---

## Section 3 — Documentation

### New: `INSTALL.md`

Sections:
- **Binarios precompilados** (recommended) — one-liner per platform using the cargo-dist-generated installer URLs
- **Descarga manual** — direct links to GitHub Releases page for users who prefer not to pipe to shell
- **Compilar desde source** — per-OS requirements + `cargo build` command
- **Configuración inicial** — where `config.toml` lives, how to set shell, font, theme

### Updated: `README.md`

- Fix repo URL: `https://github.com/Luna/Luna.git` → `https://github.com/isradev-git/luna`
- Replace "Instalación rápida" section with one-liner per platform + link to `INSTALL.md`
- Add CI badge and latest-release badge
- Remove "Windows en desarrollo futuro" — Windows is now supported

### Updated: build scripts

- `build/build-linux.sh`: fix `HOMEPAGE` variable
- `build/build-mac.sh`: no URL issues (doesn't embed URL)

---

## Section 4 — Release process

1. Apply all changes above
2. Push to `main`
3. Tag `v0.1.0` — triggers `release.yml`
4. Verify all 4 CI jobs pass (Linux, macOS arm64, macOS x86_64, Windows)
5. Confirm GitHub Release contains:
   - 3 platform tarballs/zips
   - `luna-installer.sh` (Linux + macOS)
   - `luna-installer.ps1` (Windows)
   - `luna-x86_64-windows.msi`
   - SHA256 checksums

---

## Artifacts expected after release

| File | Platform |
|------|----------|
| `luna-aarch64-apple-darwin.tar.gz` | macOS Apple Silicon |
| `luna-x86_64-apple-darwin.tar.gz` | macOS Intel |
| `luna-x86_64-unknown-linux-gnu.tar.gz` | Linux x86_64 |
| `luna-x86_64-pc-windows-msvc.zip` | Windows x86_64 |
| `luna-installer.sh` | Linux + macOS |
| `luna-installer.ps1` | Windows |
| `luna-x86_64-pc-windows-msvc.msi` | Windows MSI |
| `*.sha256` | All platforms |

---

## Out of scope

- Code signing (Apple Developer cert, GPG) — R-018 in ROADMAP, requires paid cert
- `.deb` / `.rpm` / AppImage in CI — complex runners, available via `build-linux.sh` locally
- macOS `.dmg` in CI — available via `build-mac.sh` locally
- Homebrew formula — future work
