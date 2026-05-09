#!/usr/bin/env bash
#
# vt_conformance.sh — Run VT100/xterm conformance tests and generate a report
#
# Arguments: $1 = PROJECT_ROOT, $2 = REPORTS_DIR
#

set -euo pipefail

PROJECT_ROOT="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
REPORTS_DIR="${2:-$(dirname "${BASH_SOURCE[0]}")/../reports}"
VT_LOG="$REPORTS_DIR/vt_conformance.log"
VT_REPORT="$REPORTS_DIR/vt_conformance_report.txt"

cd "$PROJECT_ROOT"

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# ─── Run VT100 parser tests ──────────────────────────────────────────────────

echo "  Running VT100/xterm parser tests..."

VT_OUTPUT=$(mktemp)

if cargo test -p Luna-terminal -- vt 2>&1 | tee "$VT_OUTPUT"; then
    echo "  ${GREEN}✓ VT parser tests passed${NC}"
else
    echo "  ${RED}✗ VT parser tests FAILED${NC}"
    cat "$VT_OUTPUT" >> "$VT_LOG"
    rm -f "$VT_OUTPUT"
    exit 1
fi

# Count tests
TEST_COUNT=$(grep -c '^test .* ... ok$' "$VT_OUTPUT" || echo "0")
cat "$VT_OUTPUT" >> "$VT_LOG"
rm -f "$VT_OUTPUT"

# ─── Generate VT conformance report ──────────────────────────────────────────

{
    echo "========================================="
    echo " VT100 / Xterm Conformance Report"
    echo " Generated: $(date '+%Y-%m-%d %H:%M:%S')"
    echo "========================================="
    echo ""
    echo "Parser tests executed: $TEST_COUNT"
    echo ""

    echo "┌───────────────────────┬──────────┐"
    echo "│ Feature               │ Status   │"
    echo "├───────────────────────┼──────────┤"

    declare -A FEATURES=(
        ["C0 controls (CR,LF,BS,TAB,FF)"]="test_c0_cr,test_c0_bs,test_c0_tab_multiple,test_c0_ff_form_feed"
        ["ISO 2022 charset"]="pending"
        ["CUU / CUD cursor"]="test_cuu_cursor_up,test_cud_cursor_down"
        ["CUF / CUB cursor"]="test_cuf_cursor_forward,test_cub_cursor_back"
        ["CUP cursor position"]="test_cursor_movement,test_cup_no_args"
        ["Cursor save/restore"]="test_esc_save_restore_cursor,test_csi_save_restore"
        ["ED erase display"]="test_ed_0_cursor_to_end,test_ed_1_start_to_cursor,test_ed_2_entire_display"
        ["EL erase line"]="test_el_0_cursor_to_end,test_el_1_start_to_cursor,test_el_2_entire_line"
        ["SGR 8-color"]="test_sgr_bold_green"
        ["SGR bright colors"]="test_sgr_bright_colors,test_sgr_bright_bg"
        ["SGR 256-color"]="test_sgr_256_color,test_sgr_256_bg"
        ["SGR true color (24-bit)"]="test_sgr_true_color"
        ["SGR attributes (bold)"]="test_sgr_bold_green"
        ["SGR attributes (italic)"]="test_sgr_italic"
        ["SGR attributes (underline)"]="test_sgr_underline"
        ["SGR attributes (blink)"]="test_sgr_blink"
        ["SGR attributes (inverse)"]="test_sgr_inverse"
        ["SGR reset"]="test_sgr_reset"
        ["RIS reset"]="test_ris_reset"
        ["Auto-wrap"]="test_auto_wrap"
        ["Line feed + scroll"]="test_line_feed_scroll"
        ["OSC title"]="test_osc_set_title,test_osc_set_title_osc2"
        ["OSC cwd"]="test_osc_set_cwd"
        ["UTF-8 multi-byte"]="test_multi_byte_utf8"
        ["Edge cases"]="test_empty_csi,test_cursor_up_clamped,test_cursor_down_clamped,test_cursor_fwd_clamped"
    )

    # Check each feature against the log
    for feature in "${!FEATURES[@]}"; do
        tests="${FEATURES[$feature]}"
        if [ "$tests" = "pending" ]; then
            printf "│ %-21s │ ${YELLOW}%-8s${NC} │\n" "$feature" "TODO"
        else
            # Check if all referenced tests passed
            all_ok=true
            IFS=',' read -ra TEST_ARRAY <<< "$tests"
            for test_name in "${TEST_ARRAY[@]}"; do
                if ! grep -q "test ${test_name} ... ok" "$VT_LOG" 2>/dev/null; then
                    all_ok=false
                    break
                fi
            done
            if [ "$all_ok" = true ]; then
                printf "│ %-21s │ ${GREEN}%-8s${NC} │\n" "$feature" "PASS"
            else
                printf "│ %-21s │ ${RED}%-8s${NC} │\n" "$feature" "FAIL"
            fi
        fi
    done

    echo "└───────────────────────┴──────────┘"
    echo ""

    # Count statuses
    PASS_COUNT=$(grep -c 'PASS' <<< "$(for f in "${!FEATURES[@]}"; do echo "${FEATURES[$f]}"; done)" || echo "0")
    echo "Summary: $(grep -c 'PASS' <<< "$(
        for f in "${!FEATURES[@]}"; do
            tests="${FEATURES[$f]}"
            if [ "$tests" != "pending" ]; then
                IFS=',' read -ra TEST_ARRAY <<< "$tests"
                for test_name in "${TEST_ARRAY[@]}"; do
                    if grep -q "test ${test_name} ... ok" "$VT_LOG" 2>/dev/null; then
                        echo "PASS"
                    fi
                done
            fi
        done
    )" || echo "0") test cases passed"
    echo ""

    # Known gaps
    echo "Known gaps:"
    echo "  - Mouse reporting (X10, SGR, URXVT): not yet tested"
    echo "  - Focus events: not yet tested"
    echo "  - Bracketed paste mode: not yet tested"
    echo "  - Kitty keyboard protocol: not yet implemented"
    echo "  - vttest interactive suite: not yet executed"
    echo ""

} > "$VT_REPORT"

cat "$VT_REPORT"

echo "  ${GREEN}✓ VT conformance report: $VT_REPORT${NC}"
echo "  ${CYAN}  $TEST_COUNT tests executed${NC}"

exit 0
