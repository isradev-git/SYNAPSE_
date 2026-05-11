#!/usr/bin/env bash
#
# toolchain_check.sh — Verify Rust toolchain is installed and meets requirements
#
# Arguments: $1 = PROJECT_ROOT, $2 = REPORTS_DIR
#

set -euo pipefail

PROJECT_ROOT="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
REPORTS_DIR="${2:-$(dirname "${BASH_SOURCE[0]}")/../reports}"

cd "$PROJECT_ROOT"

# ─── Check rustc ─────────────────────────────────────────────────────────────

if ! command -v rustc &> /dev/null; then
    echo "✗ rustc not found on PATH"
    exit 1
fi

RUSTC_VERSION=$(rustc --version)
echo "  rustc: $RUSTC_VERSION"

# ─── Check cargo ─────────────────────────────────────────────────────────────

if ! command -v cargo &> /dev/null; then
    echo "✗ cargo not found on PATH"
    exit 1
fi

CARGO_VERSION=$(cargo --version)
echo "  cargo: $CARGO_VERSION"

# ─── Check toolchain ─────────────────────────────────────────────────────────

TOOLCHAIN=$(rustup default 2>/dev/null || echo "unknown")
echo "  toolchain: $TOOLCHAIN"

# ─── Check edition support ───────────────────────────────────────────────────

# Luna uses edition 2021, which requires Rust >= 1.56
MIN_RUST_VERSION="1.56.0"
if ! rustc --version | grep -qE '[0-9]+\.[0-9]+\.[0-9]+'; then
    echo "✗ Cannot parse rustc version"
    exit 1
fi

# ─── Check for cargo fmt ─────────────────────────────────────────────────────

if rustup component list 2>/dev/null | grep -q 'rustfmt.*installed'; then
    echo "  rustfmt: installed"
else
    echo "  rustfmt: NOT installed (run: rustup component add rustfmt)"
fi

# ─── Check for clippy ────────────────────────────────────────────────────────

if rustup component list 2>/dev/null | grep -q 'clippy.*installed'; then
    echo "  clippy: installed"
else
    echo "  clippy: NOT installed (run: rustup component add clippy)"
fi

# ─── Check cross-compilation targets ─────────────────────────────────────────

echo ""
echo "  Cross-compilation targets:"
if rustup target list 2>/dev/null | grep -q 'aarch64-apple-darwin (installed)'; then
    echo "    ✓ aarch64-apple-darwin"
else
    echo "    ✗ aarch64-apple-darwin (not installed)"
fi

echo ""
echo "  ✓ Toolchain OK"

exit 0
