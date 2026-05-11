#!/usr/bin/env bash
#
# build_check.sh — Compile all crates in debug and release modes
#
# Arguments: $1 = PROJECT_ROOT, $2 = REPORTS_DIR
#

set -euo pipefail

PROJECT_ROOT="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
REPORTS_DIR="${2:-$(dirname "${BASH_SOURCE[0]}")/../reports}"
BUILD_LOG="$REPORTS_DIR/build_check.log"

cd "$PROJECT_ROOT"

# ─── Pre-flight check ────────────────────────────────────────────────────────

if ! command -v cargo &> /dev/null; then
    echo "✗ cargo not found" >&2
    exit 1
fi

# ─── Debug build ─────────────────────────────────────────────────────────────

echo "  Building debug..."

if cargo build --workspace --color always 2>&1 | tee "$BUILD_LOG"; then
    echo "  ✓ Debug build passed"
else
    echo "  ✗ Debug build FAILED — see $BUILD_LOG"
    exit 1
fi

# ─── Check each crate individually ───────────────────────────────────────────

for crate in Luna-app Luna-terminal Luna-renderer Luna-ui Luna-config; do
    echo "  Building $crate..."

    if cargo build -p "$crate" --color always 2>&1; then
        echo "    ✓ $crate"
    else
        echo "    ✗ $crate FAILED"
        exit 1
    fi
done

# ─── Release build (optional, but checks LTO and optimizations) ──────────────

echo "  Building release (with optimizations)..."

if cargo build --release --workspace --color always 2>&1 | tee -a "$BUILD_LOG"; then
    echo "  ✓ Release build passed"
else
    echo "  ✗ Release build FAILED — see $BUILD_LOG"
    exit 1
fi

# ─── cargo check (faster validation) ─────────────────────────────────────────

echo "  Running cargo check..."

if cargo check --workspace --all-targets --color always 2>&1; then
    echo "  ✓ cargo check passed"
else
    echo "  ✗ cargo check FAILED"
    exit 1
fi

echo ""
echo "  ✓ All builds passed"

exit 0
