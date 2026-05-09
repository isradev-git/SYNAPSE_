#!/usr/bin/env bash
#
# dependency_audit.sh — Audit dependencies for security and freshness
#
# Arguments: $1 = PROJECT_ROOT, $2 = REPORTS_DIR
#

set -euo pipefail

PROJECT_ROOT="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
REPORTS_DIR="${2:-$(dirname "${BASH_SOURCE[0]}")/../reports}"
AUDIT_LOG="$REPORTS_DIR/dependency_audit.log"

cd "$PROJECT_ROOT"

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

EXIT_CODE=0

{
    echo "========================================="
    echo " Dependency Audit — $(date '+%Y-%m-%d %H:%M:%S')"
    echo "========================================="
    echo ""
} > "$AUDIT_LOG"

# ─── cargo deny (advisory, licenses, bans) ───────────────────────────────────

echo "  Checking security advisories..."

if command -v cargo-deny &> /dev/null; then
    if cargo deny check advisories 2>&1 | tee -a "$AUDIT_LOG"; then
        echo "  ${GREEN}✓ No security advisories${NC}"
    else
        echo "  ${RED}✗ Security advisories found${NC}"
        EXIT_CODE=1
    fi

    echo "  Checking licenses..."
    if cargo deny check licenses 2>&1 | tee -a "$AUDIT_LOG"; then
        echo "  ${GREEN}✓ License check passed${NC}"
    else
        echo "  ${YELLOW}⚠  License issues found${NC}"
    fi
else
    echo "  ${YELLOW}⊘ cargo-deny not installed (run: cargo install cargo-deny)${NC}"
fi

# ─── cargo outdated ──────────────────────────────────────────────────────────

echo "  Checking for outdated dependencies..."

if command -v cargo-outdated &> /dev/null; then
    OUTDATED=$(cargo outdated --workspace --root-deps-only 2>&1 || true)
    OUTDATED_COUNT=$(echo "$OUTDATED" | grep -cE '^\w+\s+\d+' || true)

    echo "$OUTDATED" >> "$AUDIT_LOG"

    if [ "${OUTDATED_COUNT:-0}" -gt 0 ]; then
        echo "  ${YELLOW}⚠  $OUTDATED_COUNT outdated dependencies${NC}"
        echo "$OUTDATED"
    else
        echo "  ${GREEN}✓ All dependencies up to date${NC}"
    fi
else
    echo "  ${YELLOW}⊘ cargo-outdated not installed (run: cargo install cargo-outdated)${NC}"
fi

# ─── cargo audit (RustSec) ───────────────────────────────────────────────────

echo "  Running cargo audit (RustSec)..."
if command -v cargo-audit &> /dev/null; then
    if cargo audit 2>&1 | tee -a "$AUDIT_LOG"; then
        echo "  ${GREEN}✓ No RustSec vulnerabilities${NC}"
    else
        echo "  ${RED}✗ RustSec vulnerabilities found${NC}"
        EXIT_CODE=1
    fi
elif command -v cargo-deny &> /dev/null; then
    # cargo-deny advisories already covers this
    echo "  ${GREEN}✓ (covered by cargo-deny advisories)${NC}"
else
    echo "  ${YELLOW}⊘ cargo-audit not installed (run: cargo install cargo-audit)${NC}"
fi

# ─── Dependency tree summary ─────────────────────────────────────────────────

echo "  Dependency tree:"
echo "  Total crates: $(cargo tree --workspace 2>/dev/null | wc -l | tr -d ' ') lines in tree"

# Count unique dependencies
DEPS_COUNT=$(cargo tree --workspace --edges normal --depth 0 2>/dev/null | grep -cE '^\w+' || echo "0")
echo "  Direct workspace deps: $DEPS_COUNT"

{
    echo ""
    echo "  Dependency tree:"
    cargo tree --workspace 2>/dev/null || true
} >> "$AUDIT_LOG"

echo ""
echo "  Audit report: $AUDIT_LOG"

exit $EXIT_CODE
