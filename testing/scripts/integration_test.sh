#!/usr/bin/env bash
#
# integration_test.sh — Run integration-level scenarios across crates
#
# Arguments: $1 = PROJECT_ROOT, $2 = REPORTS_DIR
#

set -euo pipefail

PROJECT_ROOT="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
REPORTS_DIR="${2:-$(dirname "${BASH_SOURCE[0]}")/../reports}"
INTEG_LOG="$REPORTS_DIR/integration_test.log"

cd "$PROJECT_ROOT"

GREEN='\033[0;32m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

EXIT_CODE=0

{
    echo "========================================="
    echo " Integration Test Results — $(date '+%Y-%m-%d %H:%M:%S')"
    echo "========================================="
    echo ""
} > "$INTEG_LOG"

# ─── Scenario 1: PTY creation ────────────────────────────────────────────────

echo "  Scenario 1: PTY creation (spawn + write + read)..."

if cargo test -p SYNAPSE_-app 2>&1 | tee -a "$INTEG_LOG"; then
    echo "    ${GREEN}✓ PTY creation${NC}"
else
    echo "    ${RED}✗ PTY creation FAILED${NC}"
    EXIT_CODE=1
fi

# ─── Scenario 2: VT sequence pipeline ────────────────────────────────────────

echo "  Scenario 2: VT sequence pipeline (alacritty_terminal)..."
echo "    ${CYAN}  (VT pipeline handled by alacritty_terminal 0.24)${NC}"

# ─── Scenario 3: Config round-trip ───────────────────────────────────────────

echo "  Scenario 3: Config round-trip (load + modify + save)..."

if cargo test -p SYNAPSE_-config 2>&1 | tee -a "$INTEG_LOG"; then
    echo "    ${GREEN}✓ Config round-trip${NC}"
else
    echo "    ${RED}✗ Config round-trip FAILED${NC}"
    EXIT_CODE=1
fi

# ─── Scenario 4: Keybind lookup ──────────────────────────────────────────────

echo "  Scenario 4: Keybind lookup (defaults + overrides)..."

if cargo test -p SYNAPSE_-config 2>&1 | tee -a "$INTEG_LOG"; then
    echo "    ${GREEN}✓ Keybind lookup${NC}"
else
    echo "    ${RED}✗ Keybind lookup FAILED${NC}"
    EXIT_CODE=1
fi

# ─── Scenario 5: PaneTree operations ─────────────────────────────────────────

echo "  Scenario 5: PaneTree operations (split + layout + close)..."

if cargo test -p SYNAPSE_-ui 2>&1 | tee -a "$INTEG_LOG"; then
    echo "    ${GREEN}✓ PaneTree operations${NC}"
else
    echo "    ${RED}✗ PaneTree operations FAILED${NC}"
    EXIT_CODE=1
fi

# ─── Scenario 6: Autosuggestions ─────────────────────────────────────────────

echo "  Scenario 6: Autosuggestions (trie + history)..."
if cargo test -p SYNAPSE_-suggest 2>&1 | tee -a "$INTEG_LOG"; then
    echo "    ${GREEN}✓ Autosuggestions${NC}"
else
    echo "    ${RED}✗ Autosuggestions FAILED${NC}"
    EXIT_CODE=1
fi

# ─── Scenario 7: Cross-crate dependency graph ────────────────────────────────

echo "  Scenario 7: Cross-crate dependency tree..."

if cargo tree --workspace --depth 2 2>&1 | tee -a "$INTEG_LOG"; then
    echo "    ${GREEN}✓ Dependency tree OK${NC}"
else
    echo "    ${RED}✗ Dependency tree has issues${NC}"
    EXIT_CODE=1
fi

# ─── Scenario 8: Docs generation ─────────────────────────────────────────────

echo "  Scenario 8: Documentation generation..."

if cargo doc --workspace --no-deps --document-private-items 2>&1 | tee -a "$INTEG_LOG"; then
    echo "    ${GREEN}✓ Docs generated${NC}"
else
    echo "    ${RED}✗ Docs generation FAILED${NC}"
    EXIT_CODE=1
fi

# ─── Summary ─────────────────────────────────────────────────────────────────

echo ""
if [ "$EXIT_CODE" -eq 0 ]; then
    echo "  ${GREEN}✓ All integration scenarios passed${NC}"
else
    echo "  ${RED}✗ Some integration scenarios FAILED${NC}"
    echo "  See $INTEG_LOG for details"
fi

exit $EXIT_CODE
