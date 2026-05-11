#!/usr/bin/env bash
#
# unit_tests.sh — Run all unit tests, one crate at a time, with detailed output
#
# Arguments: $1 = PROJECT_ROOT, $2 = REPORTS_DIR
#

set -euo pipefail

PROJECT_ROOT="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
REPORTS_DIR="${2:-$(dirname "${BASH_SOURCE[0]}")/../reports}"
TEST_LOG="$REPORTS_DIR/unit_tests.log"
TIMESTAMP=$(date '+%Y%m%d_%H%M%S')

cd "$PROJECT_ROOT"

# ─── Pre-flight check ────────────────────────────────────────────────────────

if ! command -v cargo &> /dev/null; then
    echo "✗ cargo not found" >&2
    exit 1
fi

GREEN='\033[0;32m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

TOTAL_PASSED=0
TOTAL_FAILED=0

# ─── Header ──────────────────────────────────────────────────────────────────

{
    echo "========================================="
    echo " Unit Test Results — $TIMESTAMP"
    echo "========================================="
    echo ""
} > "$TEST_LOG"

# ─── Run tests per crate ─────────────────────────────────────────────────────

CRATES=(
    "Luna-terminal"
    "Luna-renderer"
    "Luna-ui"
    "Luna-config"
)

for crate in "${CRATES[@]}"; do
    printf "  Testing %s..." "$crate"

    TEST_OUTPUT=$(mktemp)

    if cargo test -p "$crate" --color always 2>&1 | tee "$TEST_OUTPUT"; then
        PASS_COUNT=$(grep -c 'test .* ... ok' "$TEST_OUTPUT" || true)
        TOTAL_PASSED=$((TOTAL_PASSED + (PASS_COUNT > 0 ? PASS_COUNT : 0)))
        printf " ${GREEN}✓${NC} (%d passed)\n" "$PASS_COUNT"

        echo "--- $crate ---" >> "$TEST_LOG"
        grep -E '^test .* ... ok$' "$TEST_OUTPUT" >> "$TEST_LOG" || true
        echo "" >> "$TEST_LOG"
    else
        FAIL_COUNT=$(grep -c 'test .* ... FAILED' "$TEST_OUTPUT" || true)
        TOTAL_FAILED=$((TOTAL_FAILED + FAIL_COUNT))
        printf " ${RED}✗${NC} (%d failed)\n" "$FAIL_COUNT"

        echo "--- $crate (FAILED) ---" >> "$TEST_LOG"
        grep -E '(FAILED|panicked)' "$TEST_OUTPUT" >> "$TEST_LOG" || true
        echo "" >> "$TEST_LOG"
    fi

    rm -f "$TEST_OUTPUT"
done

# ─── Summary ─────────────────────────────────────────────────────────────────

echo ""
echo "  Total passed: $TOTAL_PASSED"
echo "  Total failed: $TOTAL_FAILED"

{
    echo ""
    echo "========================================="
    echo " Total: $TOTAL_PASSED passed, $TOTAL_FAILED failed"
    echo "========================================="
} >> "$TEST_LOG"

if [ "$TOTAL_FAILED" -gt 0 ]; then
    echo "  ${RED}✗ Some tests failed${NC} — see $TEST_LOG"
    exit 1
else
    echo "  ${GREEN}✓ All tests passed${NC}"
    exit 0
fi
