#!/usr/bin/env bash
# build-linux.sh — Build and package SYNAPSE_ for Linux
#
# Produces:
#   - synapse_ binary in target/release/
#   - .deb package (Debian/Ubuntu)
#   - .rpm package (Fedora/RHEL)
#   - AppImage (universal)
#
# Requirements:
#   - Rust toolchain (rustup, cargo, rustc)
#   - dpkg-deb (for .deb) — apt install dpkg-dev
#   - rpmbuild (for .rpm) — apt install rpm
#   - appimagetool (for AppImage) — download from GitHub
#   - linuxdeploy (for AppImage) — optional
#
# Usage:
#   ./build/build-linux.sh              # dev build
#   ./build/build-linux.sh --release     # release build + all packages
#   ./build/build-linux.sh --deb         # release + deb only
#   ./build/build-linux.sh --appimage    # release + AppImage only

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
PROFILE="debug"
BUILD_ALL=false
BUILD_DEB=false
BUILD_RPM=false
BUILD_APPIMAGE=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --release) PROFILE="release"; shift ;;
        --deb) BUILD_DEB=true; PROFILE="release"; shift ;;
        --rpm) BUILD_RPM=true; PROFILE="release"; shift ;;
        --appimage) BUILD_APPIMAGE=true; PROFILE="release"; shift ;;
        --all|--release) BUILD_ALL=true; PROFILE="release"; shift ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

cd "$PROJECT_DIR"

APP_NAME="synapse_"
VERSION="0.2.0"
MAINTAINER="SYNAPSE_ Team"
DESCRIPTION="A modern GPU-accelerated terminal emulator"
HOMEPAGE="https://github.com/isradev-git/synapse_"

# Build
echo "[1/4] Building $APP_NAME ($PROFILE)..."
if [ "$PROFILE" = "release" ]; then
    cargo build --release -p SYNAPSE_-app
else
    cargo build -p SYNAPSE_-app
fi

BIN_DIR="target/$PROFILE"
BINARY="$BIN_DIR/$APP_NAME"

if [ ! -f "$BINARY" ]; then
    echo "Error: binary not found at $BINARY"
    exit 1
fi

strip "$BINARY" 2>/dev/null || true

# .deb package
if [ "$BUILD_DEB" = true ] || [ "$BUILD_ALL" = true ]; then
    echo "[2/4] Building .deb package..."
    DEB_DIR="$BIN_DIR/deb-package"
    DEB_NAME="$APP_NAME-$VERSION-amd64.deb"

    rm -rf "$DEB_DIR"
    mkdir -p "$DEB_DIR/DEBIAN"
    mkdir -p "$DEB_DIR/usr/bin"
    mkdir -p "$DEB_DIR/usr/share/applications"
    mkdir -p "$DEB_DIR/usr/share/icons/hicolor/256x256/apps"

    cp "$BINARY" "$DEB_DIR/usr/bin/$APP_NAME"

    # Create .desktop file
    cat > "$DEB_DIR/usr/share/applications/$APP_NAME.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=SYNAPSE_
Comment=$DESCRIPTION
Exec=$APP_NAME
Terminal=false
Categories=System;TerminalEmulator;
Keywords=terminal;shell;console;
DESKTOP

    # Create control file
    cat > "$DEB_DIR/DEBIAN/control" <<CONTROL
Package: $APP_NAME
Version: $VERSION
Section: utils
Priority: optional
Architecture: amd64
Maintainer: $MAINTAINER
Depends: libx11-6, libxkbcommon0, libwayland-client0
Description: $DESCRIPTION
 SYNAPSE_ is a modern GPU-accelerated terminal emulator
 written in Rust with wgpu rendering, split panes,
 tabs, and full VT100/xterm support.
CONTROL

    dpkg-deb --build "$DEB_DIR" "$BIN_DIR/$DEB_NAME"
    echo "  .deb: $BIN_DIR/$DEB_NAME"
fi

# .rpm package
if [ "$BUILD_RPM" = true ] || [ "$BUILD_ALL" = true ]; then
    echo "[3/4] Building .rpm package..."
    RPM_ROOT="$BIN_DIR/rpm-buildroot"

    rm -rf "$RPM_ROOT"
    mkdir -p "$RPM_ROOT/usr/bin"
    mkdir -p "$RPM_ROOT/usr/share/applications"

    cp "$BINARY" "$RPM_ROOT/usr/bin/$APP_NAME"
    cp "$DEB_DIR/usr/share/applications/$APP_NAME.desktop" \
       "$RPM_ROOT/usr/share/applications/$APP_NAME.desktop" 2>/dev/null || true

    # Create spec file
    cat > "$BIN_DIR/synapse_.spec" <<SPEC
Name:        $APP_NAME
Version:     $VERSION
Release:     1%{?dist}
Summary:     $DESCRIPTION
License:     MIT
URL:         $HOMEPAGE

%description
SYNAPSE_ is a modern GPU-accelerated terminal emulator written in Rust
with wgpu rendering, split panes, tabs, and full VT100/xterm support.

%files
/usr/bin/$APP_NAME
/usr/share/applications/$APP_NAME.desktop
SPEC

    if command -v rpmbuild &> /dev/null; then
        rpmbuild -bb --buildroot="$RPM_ROOT" "$BIN_DIR/synapse_.spec" \
            --define "_rpmdir $BIN_DIR" 2>&1 | tail -5
        echo "  .rpm in $BIN_DIR/"
    else
        echo "  Warning: rpmbuild not found. Skipping .rpm."
        echo "  Install with: sudo apt install rpm"
    fi
fi

# AppImage
if [ "$BUILD_APPIMAGE" = true ] || [ "$BUILD_ALL" = true ]; then
    echo "[4/4] Building AppImage..."
    APPDIR="$BIN_DIR/$APP_NAME.AppDir"

    rm -rf "$APPDIR"
    mkdir -p "$APPDIR/usr/bin"
    mkdir -p "$APPDIR/usr/share/icons/hicolor/256x256/apps"

    cp "$BINARY" "$APPDIR/usr/bin/$APP_NAME"

    # Create wrapper script (AppRun)
    cat > "$APPDIR/AppRun" <<APPRUN
#!/bin/bash
HERE="\$(dirname "\$(readlink -f "\${0}")")"
exec "\$HERE/usr/bin/$APP_NAME" "\$@"
APPRUN
    chmod +x "$APPDIR/AppRun"

    # Create .desktop file for AppImage
    cat > "$APPDIR/$APP_NAME.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=SYNAPSE_
Comment=$DESCRIPTION
Exec=$APP_NAME
Terminal=false
Categories=System;TerminalEmulator;
DESKTOP

    if command -v appimagetool &> /dev/null; then
        ARCH=x86_64 appimagetool "$APPDIR" "$BIN_DIR/$APP_NAME-$VERSION-x86_64.AppImage"
        echo "  AppImage: $BIN_DIR/$APP_NAME-$VERSION-x86_64.AppImage"
    else
        echo "  Warning: appimagetool not found. Skipping AppImage."
        echo "  Download from: https://github.com/AppImage/AppImageKit/releases"
    fi
fi

echo ""
echo "Build complete: $BINARY"
