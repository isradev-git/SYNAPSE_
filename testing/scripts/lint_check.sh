#!/usr/bin/env bash
#
# lint_check.sh — Run cargo fmt and cargo clippy on the workspace
#
# Arguments: $1 = PROJECT_ROOT, $2 = REPORTS_DIR
#

set -euo pipefail

PROJECT_ROOT="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
REPORTS_DIR="${2:-$(dirname "${BASH_SOURCE[0]}")/../reports}"
FMT_LOG="$REPORTS_DIR/fmt_check.log"
CLIPPY_LOG="$REPORTS_DIR/clippy_check.log"

cd "$PROJECT_ROOT"

GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

EXIT_CODE=0

# ─── cargo fmt ───────────────────────────────────────────────────────────────

echo "  Running cargo fmt --check..."

if cargo fmt --all -- --check 2>&1 | tee "$FMT_LOG"; then
    echo "  ${GREEN}✓ cargo fmt passed${NC}"
else
    echo "  ${RED}✗ cargo fmt found formatting issues${NC}"
    echo "  Fix with: cargo fmt --all"
    EXIT_CODE=1
fi

# ─── cargo clippy ────────────────────────────────────────────────────────────

echo "  Running cargo clippy..."

if cargo clippy --workspace --all-targets --no-deps --color always 2>&1 | tee "$CLIPPY_LOG"; then
    echo "  ${GREEN}✓ cargo clippy passed (no warnings)${NC}"
else
    # Check if it's just warnings or actual errors
    if grep -q '^error' "$CLIPPY_LOG" 2>/dev/null; then
        echo "  ${RED}✗ cargo clippy found errors${NC}"
        EXIT_CODE=1
    else
        echo "  ${RED}⚠  cargo clippy found warnings${NC}"
        echo "  See $CLIPPY_LOG for details"
        # Don't fail on warnings alone
    fi
fi

# ─── Check for deprecated APIs ───────────────────────────────────────────────

echo "  Scanning for deprecated usage..."

RUST_DEPRECATED_COUNT=0

# Check for leftover todo!() or unimplemented!() calls (excluding tests and comments)
# The codebase currently has 0, but this catches regressions
while IFS= read -r line; do
    RUST_DEPRECATED_COUNT=$((RUST_DEPRECATED_COUNT + 1))
done < <(
    grep -rn --include='*.rs' -E '\b(todo!\(|unimplemented!\()' \
        "$PROJECT_ROOT/crates" \
        2>/dev/null || true
)

if [ "$RUST_DEPRECATED_COUNT" -gt 0 ]; then
    echo "  ${RED}⚠  Found $RUST_DEPRECATED_COUNT todo!() or unimplemented!() calls${NC}"
else
    echo "  ${GREEN}✓ No todo!() or unimplemented!() found${NC}"
fi

# ─── Check file permissions ──────────────────────────────────────────────────

echo "  Checking file permissions..."

BAD_PERMS=$(find "$PROJECT_ROOT/crates" -name '*.rs' -perm +111 2>/dev/null || true)
if [ -n "$BAD_PERMS" ]; then
    echo "  ${RED}✗ Some .rs files have execute permission${NC}"
    EXIT_CODE=1
else
    echo "  ${GREEN}✓ File permissions OK${NC}"
fi

exit $EXIT_CODE
