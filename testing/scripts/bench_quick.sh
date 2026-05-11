#!/usr/bin/env bash
#
# bench_quick.sh — Quick performance benchmarks
#
# Arguments: $1 = PROJECT_ROOT, $2 = REPORTS_DIR
#

set -euo pipefail

PROJECT_ROOT="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
REPORTS_DIR="${2:-$(dirname "${BASH_SOURCE[0]}")/../reports}"
BENCH_LOG="$REPORTS_DIR/bench_quick.log"
BENCH_REPORT="$REPORTS_DIR/bench_quick_report.txt"

cd "$PROJECT_ROOT"

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# ─── Check if release binary exists ──────────────────────────────────────────

RELEASE_BIN="$PROJECT_ROOT/target/release/Luna-app"
if [ ! -f "$RELEASE_BIN" ]; then
    echo "  Building release binary first..."
    cargo build --release -p Luna-app 2>&1
    if [ ! -f "$RELEASE_BIN" ]; then
        echo "  ${RED}✗ Release binary not found — build may have failed${NC}"
        exit 1
    fi
fi

{
    echo "========================================="
    echo " Quick Benchmark Results"
    echo " Generated: $(date '+%Y-%m-%d %H:%M:%S')"
    echo " Platform: $(uname -s) $(uname -m)"
    echo " Binary: $RELEASE_BIN"
    echo "========================================="
    echo ""
} > "$BENCH_REPORT"

# ─── Startup time ────────────────────────────────────────────────────────────

echo "  Measuring startup time..."

# Run 5 times and average (Uses GNU date or perl fallback)
START_COUNT=5
TOTAL_START=0

for i in $(seq 1 $START_COUNT); do
    if date +'%s%N' &>/dev/null 2>&1; then
        # macOS-compatible: use perl for microsecond timing
        START_NS=$(perl -MTime::HiRes=time -e 'printf "%d", time*1000' 2>/dev/null || echo 0)
        timeout 3 "$RELEASE_BIN" --help 2>/dev/null || true
        END_NS=$(perl -MTime::HiRes=time -e 'printf "%d", time*1000' 2>/dev/null || echo 0)
        ELAPSED=$((END_NS - START_NS))
        TOTAL_START=$((TOTAL_START + (ELAPSED > 0 ? ELAPSED : 0)))
    else
        # Fallback to seconds-based timing
        START_S=$(date +%s 2>/dev/null || echo 0)
        timeout 3 "$RELEASE_BIN" --help 2>/dev/null || true
        END_S=$(date +%s 2>/dev/null || echo 0)
        ELAPSED=$((END_S - START_S))
        TOTAL_START=$((TOTAL_START + (ELAPSED * 1000)))
    fi
done

AVG_START=$((TOTAL_START / START_COUNT))
echo "  ${CYAN}  Average startup: ${AVG_START}ms (target: <200ms)${NC}"

{
    echo "Startup time: ${AVG_START}ms (average of $START_COUNT runs, target: <200ms)"
    echo ""
} >> "$BENCH_REPORT"

# ─── Binary size ─────────────────────────────────────────────────────────────

echo "  Checking binary size..."

BINARY_SIZE=$(stat -f%z "$RELEASE_BIN" 2>/dev/null || stat -c%s "$RELEASE_BIN" 2>/dev/null || echo 0)
BINARY_SIZE_MB=$(echo "scale=2; $BINARY_SIZE / 1048576" | bc 2>/dev/null || echo "N/A")
echo "  ${CYAN}  Binary size: ${BINARY_SIZE_MB}MB${NC}"
echo "Binary size: ${BINARY_SIZE_MB}MB" >> "$BENCH_REPORT"

# ─── Compile time ────────────────────────────────────────────────────────────

echo "  Measuring clean compile time..."

CLEAN_START=$(date +%s 2>/dev/null)
cargo clean 2>/dev/null || true
cargo build --release -p Luna-app 2>&1 | tail -5
CLEAN_END=$(date +%s 2>/dev/null)
COMPILE_TIME=$((CLEAN_END - CLEAN_START))
echo "  ${CYAN}  Clean release build: ${COMPILE_TIME}s${NC}"
echo "Clean build time: ${COMPILE_TIME}s" >> "$BENCH_REPORT"

# ─── Test execution time ─────────────────────────────────────────────────────

echo "  Measuring test execution time..."

TEST_START=$(date +%s 2>/dev/null)
cargo test --workspace 2>&1 | tail -5
TEST_END=$(date +%s 2>/dev/null)
TEST_TIME=$((TEST_END - TEST_START))
echo "  ${CYAN}  Full test suite: ${TEST_TIME}s${NC}"
echo "Test suite time: ${TEST_TIME}s" >> "$BENCH_REPORT"

# ─── Summary ─────────────────────────────────────────────────────────────────

echo ""
echo "  ${GREEN}✓ Benchmarks complete${NC}"
echo "  Report: $BENCH_REPORT"

cat "$BENCH_REPORT"

exit 0
