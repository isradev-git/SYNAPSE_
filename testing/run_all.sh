#!/usr/bin/env bash
#
# run_all.sh — Luna Project Test Suite Orchestrator
# ==================================================
# Runs all testing phases in order. Each phase can also be run independently.
# Exit code is 0 only if ALL phases pass.
#
# Usage:
#   ./testing/run_all.sh            # Run all phases
#   ./testing/run_all.sh --phase 1  # Run only phase 1 (build check)
#   ./testing/run_all.sh --quick    # Skip slow phases (bench, coverage)
#   ./testing/run_all.sh --verbose  # Show full output
#   ./testing/run_all.sh --help     # Show this help
#
# Phases:
#   1. Toolchain check       — verify Rust is installed
#   2. Build check           — cargo build --workspace
#   3. Unit tests            — cargo test --workspace
#   4. Lint check            — cargo fmt + cargo clippy
#   5. VT100 conformance     — run VT100/xterm parser tests
#   6. Integration tests     — PTY round-trip, config, layout
#   7. Dependency audit      — cargo deny check / cargo outdated
#   8. Coverage              — cargo tarpaulin / cargo llvm-cov
#   9. Benchmarks            — performance metrics
#

set -euo pipefail

# ─── Configuration ───────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
REPORTS_DIR="$SCRIPT_DIR/reports"
TIMESTAMP=$(date '+%Y%m%d_%H%M%S')
REPORT_FILE="$REPORTS_DIR/test-run_$TIMESTAMP.txt"
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0
ALL_START_TIME=$(date +%s)

# ─── Argument parsing ────────────────────────────────────────────────────────

PHASE_FILTER=""
QUICK_MODE=false
VERBOSE=false

for arg in "$@"; do
    case $arg in
        --phase|-p)
            PHASE_FILTER="$2"
            shift 2
            ;;
        --phase=*|-p=*)
            PHASE_FILTER="${arg#*=}"
            shift
            ;;
        --quick|-q)
            QUICK_MODE=true
            shift
            ;;
        --verbose|-v)
            VERBOSE=true
            shift
            ;;
        --help|-h)
            head -23 "$0" | tail -20
            exit 0
            ;;
        *)
            echo "Unknown argument: $arg"
            echo "Use --help for usage"
            exit 1
            ;;
    esac
done

# ─── Helpers ─────────────────────────────────────────────────────────────────

log_phase() {
    local phase="$1" name="$2"
    printf "\n${BOLD}${CYAN}══════════════════════════════════════════════════════════════${NC}\n"
    printf "${BOLD}${CYAN}  PHASE $phase — $name${NC}\n"
    printf "${BOLD}${CYAN}══════════════════════════════════════════════════════════════${NC}\n\n"
}

log_pass() {
    PASS_COUNT=$((PASS_COUNT + 1))
    printf "  ${GREEN}✓ PASS${NC}  $1\n"
    echo "  PASS  $1" >> "$REPORT_FILE"
}

log_fail() {
    FAIL_COUNT=$((FAIL_COUNT + 1))
    printf "  ${RED}✗ FAIL${NC}  $1\n"
    echo "  FAIL  $1" >> "$REPORT_FILE"
}

log_skip() {
    SKIP_COUNT=$((SKIP_COUNT + 1))
    printf "  ${YELLOW}⊘ SKIP${NC}  $1\n"
    echo "  SKIP  $1" >> "$REPORT_FILE"
}

run_script() {
    local script="$1"
    local name="$2"
    local script_path="$SCRIPT_DIR/scripts/$script"
    local result=0

    if [ ! -f "$script_path" ]; then
        log_fail "$name (script not found: $script)"
        return 1
    fi

    if [ "$VERBOSE" = true ]; then
        # Run with full output. Pipefail ensures we capture the script exit code.
        # We use a temp file to capture the exit code reliably.
        local exit_file
        exit_file=$(mktemp)
        (
            bash "$script_path" "$PROJECT_ROOT" "$REPORTS_DIR" 2>&1
            echo $? > "$exit_file"
        ) | tail -20 >> "$REPORT_FILE"
        result=$(cat "$exit_file")
        rm -f "$exit_file"
    else
        bash "$script_path" "$PROJECT_ROOT" "$REPORTS_DIR" > /dev/null 2>&1
        result=$?
    fi

    if [ "$result" -eq 0 ]; then
        log_pass "$name"
        return 0
    else
        log_fail "$name"
        return 1
    fi
}

should_run_phase() {
    local phase="$1"
    if [ -z "$PHASE_FILTER" ]; then
        return 0
    fi
    if [ "$PHASE_FILTER" = "$phase" ]; then
        return 0
    fi
    if [ "$QUICK_MODE" = true ] && [ "$phase" -ge 7 ]; then
        log_skip "Phase $phase (skipped in quick mode)"
        return 1
    fi
    log_skip "Phase $phase (filtered out by --phase $PHASE_FILTER)"
    return 1
}

# ─── Init ────────────────────────────────────────────────────────────────────

mkdir -p "$REPORTS_DIR"

{
    echo "========================================="
    echo " Luna Test Suite — Run $TIMESTAMP"
    echo " Project: $PROJECT_ROOT"
    echo " Platform: $(uname -s) $(uname -m)"
    echo "========================================="
    echo ""
} | tee "$REPORT_FILE"

# ─── Phase 1: Toolchain Check ────────────────────────────────────────────────

TOOLCHAIN_OK=false

if should_run_phase 1; then
    log_phase "1" "Toolchain Check"

    if run_script "toolchain_check.sh" "Rust toolchain"; then
        TOOLCHAIN_OK=true
    else
        echo ""
        echo "  ${RED}CRITICAL: Rust toolchain not found.${NC}"
        echo "  Install via: ${BOLD}curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh${NC}"
        echo "  Then re-run: ${BOLD}./testing/run_all.sh${NC}"
        echo ""
    fi
fi

# If toolchain is missing, we can't run phases 2-9
if [ "$TOOLCHAIN_OK" = false ]; then
    if [ -n "$PHASE_FILTER" ] && [ "$PHASE_FILTER" -ne 1 ]; then
        log_phase "$PHASE_FILTER" "(forced — but toolchain is missing)"
    fi
    echo "  Stopping: Rust toolchain required for remaining phases."
    log_skip "Phases 2-9 (no Rust toolchain)"
    echo ""
    echo "Summary:"
    printf "  ${GREEN}Passed:  $PASS_COUNT${NC}\n"
    printf "  ${RED}Failed:  $FAIL_COUNT${NC}\n"
    printf "  ${YELLOW}Skipped: $SKIP_COUNT${NC}\n"
    exit 1
fi

# ─── Phase 2: Build Check ────────────────────────────────────────────────────

if should_run_phase 2; then
    log_phase "2" "Build Check"
    run_script "build_check.sh" "cargo build --workspace"
fi

# ─── Phase 3: Unit Tests ─────────────────────────────────────────────────────

if should_run_phase 3; then
    log_phase "3" "Unit Tests"
    run_script "unit_tests.sh" "cargo test --workspace"
fi

# ─── Phase 4: Lint Check ─────────────────────────────────────────────────────

if should_run_phase 4; then
    log_phase "4" "Lint Check"
    run_script "lint_check.sh" "cargo fmt + clippy"
fi

# ─── Phase 5: VT100 Conformance ──────────────────────────────────────────────

if should_run_phase 5; then
    log_phase "5" "VT100/Xterm Conformance"
    run_script "vt_conformance.sh" "VT100 parser tests"
fi

# ─── Phase 6: Integration Tests ──────────────────────────────────────────────

if should_run_phase 6; then
    log_phase "6" "Integration Tests"
    run_script "integration_test.sh" "Integration scenarios"
fi

# ─── Phase 7: Dependency Audit ───────────────────────────────────────────────

if should_run_phase 7; then
    log_phase "7" "Dependency Audit"
    run_script "dependency_audit.sh" "cargo deny + outdated"
fi

# ─── Phase 8: Coverage ───────────────────────────────────────────────────────

if should_run_phase 8; then
    log_phase "8" "Test Coverage"
    run_script "coverage.sh" "code coverage"
fi

# ─── Phase 9: Benchmarks ─────────────────────────────────────────────────────

if should_run_phase 9; then
    log_phase "9" "Benchmarks"
    run_script "bench_quick.sh" "performance benchmarks"
fi

# ─── Summary ─────────────────────────────────────────────────────────────────

ALL_END_TIME=$(date +%s)
DURATION=$((ALL_END_TIME - ALL_START_TIME))

printf "\n${BOLD}${CYAN}══════════════════════════════════════════════════════════════${NC}\n"
printf "${BOLD}${CYAN}  TEST SUITE COMPLETE — %s${NC}\n" "$(date '+%H:%M:%S')"
printf "${BOLD}${CYAN}══════════════════════════════════════════════════════════════${NC}\n\n"

printf "  ${GREEN}✓ Passed:  %d${NC}\n" "$PASS_COUNT"
printf "  ${RED}✗ Failed:  %d${NC}\n" "$FAIL_COUNT"
printf "  ${YELLOW}⊘ Skipped: %d${NC}\n" "$SKIP_COUNT"
printf "  Duration:  %d seconds\n\n" "$DURATION"

{
    echo ""
    echo "========================================="
    echo " Final Summary"
    echo "========================================="
    echo " Passed:  $PASS_COUNT"
    echo " Failed:  $FAIL_COUNT"
    echo " Skipped: $SKIP_COUNT"
    echo " Duration: ${DURATION}s"
    echo ""
} >> "$REPORT_FILE"

if [ "$FAIL_COUNT" -gt 0 ]; then
    printf "  ${RED}✗ Test suite FAILED with %d error(s)${NC}\n\n" "$FAIL_COUNT"
    echo "See full report: $REPORT_FILE"
    exit 1
else
    printf "  ${GREEN}✓ All tests PASSED${NC}\n\n"
    echo "See full report: $REPORT_FILE"
    exit 0
fi
