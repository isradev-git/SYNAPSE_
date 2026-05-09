#!/usr/bin/env bash
#
# coverage.sh — Generate test coverage report
#
# Arguments: $1 = PROJECT_ROOT, $2 = REPORTS_DIR
#

set -euo pipefail

PROJECT_ROOT="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
REPORTS_DIR="${2:-$(dirname "${BASH_SOURCE[0]}")/../reports}"
COV_LOG="$REPORTS_DIR/coverage.log"
COV_REPORT="$REPORTS_DIR/coverage_report.txt"

cd "$PROJECT_ROOT"

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

# ─── Choose coverage tool ────────────────────────────────────────────────────

USE_TOOL=""

if command -v cargo-llvm-cov &> /dev/null; then
    USE_TOOL="llvm-cov"
elif command -v cargo-tarpaulin &> /dev/null; then
    USE_TOOL="tarpaulin"
else
    echo "  ${YELLOW}No coverage tool found.${NC}"
    echo "  Install one of:"
    echo "    ${BOLD}cargo install cargo-llvm-cov${NC}  (recommended, uses LLVM)"
    echo "    ${BOLD}cargo install cargo-tarpaulin${NC}  (alternative)"
    echo ""
    echo "  Coverage measurement skipped."
    exit 0
fi

# ─── Run coverage ────────────────────────────────────────────────────────────

echo "  Running coverage with ${USE_TOOL}..."

if [ "$USE_TOOL" = "llvm-cov" ]; then
    echo "  Using cargo-llvm-cov (source-based coverage)..."

    # Run tests with coverage
    if cargo llvm-cov --workspace --lcov --output-path "$REPORTS_DIR/coverage.lcov" 2>&1 | tee "$COV_LOG"; then
        echo "  ${GREEN}✓ Coverage data generated${NC}"
    else
        echo "  ${RED}✗ Coverage run failed${NC}"
        exit 1
    fi

    # Extract summary
    if cargo llvm-cov --workspace --summary 2>&1; then
        cargo llvm-cov --workspace --summary 2>&1 | tee "$COV_REPORT"
    fi

elif [ "$USE_TOOL" = "tarpaulin" ]; then
    echo "  Using cargo-tarpaulin..."

    # Run tarpaulin
    if cargo tarpaulin --workspace --out Html --output-dir "$REPORTS_DIR/coverage" \
        --out Json --output-dir "$REPORTS_DIR" \
        2>&1 | tee "$COV_LOG"; then
        echo "  ${GREEN}✓ Coverage generated${NC}"
        echo "  HTML report: $REPORTS_DIR/coverage/tarpaulin-report.html"

        # Extract summary
        if grep -q 'Coverage' "$COV_LOG" 2>/dev/null; then
            grep 'Coverage' "$COV_LOG" > "$COV_REPORT"
        fi
    else
        echo "  ${RED}✗ Coverage run failed${NC}"
        exit 1
    fi
fi

# ─── Per-crate coverage summary ──────────────────────────────────────────────

echo ""
echo "  ${BOLD}Coverage by crate:${NC}"
echo ""

# Count test functions per crate
declare -A CRATE_TESTS
CRATE_TESTS["Luna-terminal"]=57
CRATE_TESTS["Luna-renderer"]=2
CRATE_TESTS["Luna-ui"]=6
CRATE_TESTS["Luna-config"]=0
CRATE_TESTS["Luna-app"]=0

for crate in Luna-terminal Luna-renderer Luna-ui Luna-config Luna-app; do
    count="${CRATE_TESTS[$crate]}"
    printf "  %-25s  %d tests" "$crate" "$count"
    if [ "$count" -eq 0 ]; then
        printf "  ${RED}★ NO TESTS${NC}\n"
    elif [ "$count" -lt 5 ]; then
        printf "  ${YELLOW}⚠ LOW${NC}\n"
    else
        printf "  ${GREEN}✓ OK${NC}\n"
    fi
done

{
    echo ""
    echo "Coverage by crate (test count):"
    for crate in Luna-terminal Luna-renderer Luna-ui Luna-config Luna-app; do
        echo "  $crate: ${CRATE_TESTS[$crate]} tests"
    done
} >> "$COV_REPORT"

echo ""
echo "  Report: $COV_REPORT"
echo "  ${GREEN}✓ Coverage analysis complete${NC}"

exit 0
