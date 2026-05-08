#!/usr/bin/env bash
# build-mac.sh — Build and package Luna for macOS
#
# Produces:
#   - Luna.app bundle in target/release/
#   - Luna.dmg disk image
#
# Requirements:
#   - Apple Developer tools (Xcode or Command Line Tools)
#   - cargo, rustc from rustup
#   - create-dmg (brew install create-dmg) for .dmg generation
#
# Usage:
#   ./build/build-mac.sh              # dev build
#   ./build/build-mac.sh --release    # release build
#   ./build/build-mac.sh --dmg        # release + dmg

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
PROFILE="debug"
CREATE_DMG=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --release) PROFILE="release"; shift ;;
        --dmg) CREATE_DMG=true; shift ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

cd "$PROJECT_DIR"

APP_NAME="Luna"
BUNDLE_ID="com.luna.app"
VERSION="0.1.0"

# Build universal binary (x86_64 + arm64)
echo "[1/4] Building $APP_NAME ($PROFILE)..."
if [ "$PROFILE" = "release" ]; then
    cargo build --release -p Luna-app
else
    cargo build -p Luna-app
fi

BIN_DIR="target/$PROFILE"
BINARY="$BIN_DIR/luna"

if [ ! -f "$BINARY" ]; then
    echo "Error: binary not found at $BINARY"
    exit 1
fi

# Create .app bundle structure
echo "[2/4] Creating $APP_NAME.app bundle..."
APP_DIR="$BIN_DIR/$APP_NAME.app"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

# Copy binary
cp "$BINARY" "$APP_DIR/Contents/MacOS/$APP_NAME"

# Create Info.plist
cat > "$APP_DIR/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>
    <key>CFBundleExecutable</key>
    <string>$APP_NAME</string>
    <key>CFBundleIdentifier</key>
    <string>$BUNDLE_ID</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundleDisplayName</key>
    <string>$APP_NAME</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>LSMinimumSystemVersion</key>
    <string>12.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
</dict>
</plist>
PLIST

echo "[3/4] $APP_NAME.app created at $APP_DIR"

# Create .dmg
if [ "$CREATE_DMG" = true ]; then
    echo "[4/4] Creating .dmg..."
    if ! command -v create-dmg &> /dev/null; then
        echo "Warning: create-dmg not found. Install with: brew install create-dmg"
        echo "Skipping .dmg creation."
        exit 0
    fi
    DMG_PATH="$BIN_DIR/$APP_NAME-$VERSION.dmg"
    rm -f "$DMG_PATH"
    create-dmg \
        --volname "$APP_NAME" \
        --volicon "" \
        --window-pos 200 120 \
        --window-size 600 400 \
        --icon-size 100 \
        --icon "$APP_NAME.app" 175 190 \
        --hide-extension "$APP_NAME.app" \
        --app-drop-link 425 190 \
        "$DMG_PATH" \
        "$APP_DIR"
    echo "DMG created at $DMG_PATH"
else
    echo "[4/4] Skipping .dmg (use --dmg to create)."
fi
