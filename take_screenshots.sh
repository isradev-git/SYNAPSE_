#!/usr/bin/env bash
# Run this from your terminal (iTerm2 / Terminal.app) — NOT via Claude Code
# It launches SYNAPSE_, captures 4 screenshots, then kills the app.
set -e

PROJ="$(cd "$(dirname "$0")" && pwd)"
SHOTS="$PROJ/assets/screenshots"
mkdir -p "$SHOTS"

echo "Building..."
cargo build -p SYNAPSE_-app -q

echo "Launching SYNAPSE_..."
cargo run -p SYNAPSE_-app > /tmp/synapse_shot.log 2>&1 &
APP_PID=$!

cleanup() { kill "$APP_PID" 2>/dev/null; }
trap cleanup EXIT

sleep 5

activate() {
  osascript -e 'tell application "System Events"
    set p to first application process whose name contains "synapse_"
    set frontmost of p to true
  end tell' 2>/dev/null
}

activate
sleep 0.5

# 1 — Main terminal (cyberpunk theme)
screencapture -x "$SHOTS/main.png"
echo "✓ main.png"
sleep 0.5

# 2 — Split panes: Ctrl+Shift+D
osascript -e 'tell application "System Events" to keystroke "d" using {control down, shift down}'
sleep 1.5
screencapture -x "$SHOTS/split-panes.png"
echo "✓ split-panes.png"
sleep 0.5

# 3 — Search bar + regex active: Ctrl+Shift+F, type "error", Ctrl+/
osascript -e 'tell application "System Events" to keystroke "f" using {control down, shift down}'
sleep 0.8
osascript -e 'tell application "System Events" to keystroke "error"'
sleep 0.4
osascript -e 'tell application "System Events" to key code 44 using {control down}'
sleep 0.4
screencapture -x "$SHOTS/search-regex.png"
echo "✓ search-regex.png"

# Close search
osascript -e 'tell application "System Events" to key code 53'
sleep 0.4

# 4 — Command palette: Ctrl+Shift+P, type "split"
osascript -e 'tell application "System Events" to keystroke "p" using {control down, shift down}'
sleep 0.8
osascript -e 'tell application "System Events" to keystroke "split"'
sleep 0.4
screencapture -x "$SHOTS/command-palette.png"
echo "✓ command-palette.png"

echo ""
echo "Done. Screenshots in assets/screenshots/"
ls -lh "$SHOTS/"
