#!/usr/bin/env bash
# SYNAPSE_ — benchmark script
# Usage: ./build/bench.sh [release|debug]
#
# Measures: startup time, RAM idle, RAM scrollback, FPS normal, FPS heavy output

set -euo pipefail

BIN="${1:-release}"
case "$BIN" in
    release) BIN_PATH="./target/release/synapse_" ;;
    debug)   BIN_PATH="./target/debug/synapse_" ;;
    *)       echo "Usage: $0 [release|debug]"; exit 1 ;;
esac

if [ ! -f "$BIN_PATH" ]; then
    echo "Binary not found: $BIN_PATH"
    echo "Run: cargo build --release"
    exit 1
fi

BIN_SIZE=$(du -h "$BIN_PATH" | cut -f1)
echo "=== SYNAPSE_ Benchmarks ==="
echo "Binary: $BIN_PATH ($BIN_SIZE)"
echo ""

# ── Startup time ────────────────────────────────────────────────────────
echo "--- Startup time ---"
for i in 1 2 3; do
    START=$(date +%s%N)
    $BIN_PATH &
    PID=$!
    # Wait for window to appear (poll xdotool or wmctrl)
    for _ in $(seq 1 100); do
        if command -v wmctrl &>/dev/null && wmctrl -l 2>/dev/null | grep -q "SYNAPSE_"; then
            break
        fi
        sleep 0.01
    done
    ELAPSED=$(( ($(date +%s%N) - START) / 1000000 ))
    echo "  Run $i: ${ELAPSED}ms"
    kill $PID 2>/dev/null || true
    sleep 1
done
echo ""

# ── RAM idle ─────────────────────────────────────────────────────────────
echo "--- RAM idle ---"
$BIN_PATH &
PID=$!
sleep 5
RSS_KB=$(ps -o rss= -p $PID 2>/dev/null || echo "0")
RSS_MB=$(echo "scale=1; $RSS_KB / 1024" | bc 2>/dev/null || echo "0")
echo "  RSS after 5s idle: ${RSS_MB}MB"
kill $PID 2>/dev/null || true
sleep 1
echo ""

# ── RAM with scrollback ──────────────────────────────────────────────────
echo "--- RAM with scrollback ---"
$BIN_PATH &
PID=$!
sleep 2
# Send massive output to fill scrollback
if command -v xdotool &>/dev/null; then
    WID=$(xdotool search --name "SYNAPSE_" 2>/dev/null | head -1)
    if [ -n "$WID" ]; then
        for _ in $(seq 1 10); do
            xdotool type --window "$WID" "yes 'scrollback fill test line' | head -n 10000" 2>/dev/null || true
            xdotool key --window "$WID" Return 2>/dev/null || true
            sleep 2
        done
        sleep 5
        RSS_KB=$(ps -o rss= -p $PID 2>/dev/null || echo "0")
        RSS_MB=$(echo "scale=1; $RSS_KB / 1024" | bc 2>/dev/null || echo "0")
        echo "  RSS with 100k lines: ${RSS_MB}MB"
    fi
fi
kill $PID 2>/dev/null || true
echo ""

# ── FPS idle / heavy ─────────────────────────────────────────────────────
echo "--- FPS ---"
echo "  Run with: RUST_LOG=synapse_::bench=info $BIN_PATH"
echo "  Then cat a large file to measure FPS under load."
echo ""

echo "=== Done ==="
echo "Update BENCHMARKS.md with collected results."
