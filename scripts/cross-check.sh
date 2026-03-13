#!/bin/bash
set -euo pipefail

# DriveWipe Cross-Platform Pre-Push Check
# Locally validates what CI checks across macOS, Linux, and Windows
# so you don't burn GitHub Actions minutes on failures.
#
# Requirements: cargo, rustfmt, clippy, zig, cargo-zigbuild
#   brew install zig
#   cargo install cargo-zigbuild
#
# Usage:
#   ./scripts/cross-check.sh          # full check (all platforms)
#   ./scripts/cross-check.sh --quick  # fast check (native clippy + fmt only)
#   ./scripts/cross-check.sh --linux  # native + linux only
#   ./scripts/cross-check.sh --windows # native + windows only

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

FAILED=0
SKIPPED=0

pass() { echo -e "  ${GREEN}PASS${NC}  $1"; }
fail() { echo -e "  ${RED}FAIL${NC}  $1"; FAILED=$((FAILED + 1)); }
skip() { echo -e "  ${YELLOW}SKIP${NC}  $1"; SKIPPED=$((SKIPPED + 1)); }
step() { echo -e "\n${CYAN}${BOLD}[$1]${NC} $2"; }

MODE="${1:-full}"

# ── Preflight ─────────────────────────────────────────────────────────────

step 0 "Preflight checks"

if ! command -v cargo &>/dev/null; then
    fail "cargo not found"
    exit 1
fi
pass "cargo"

if ! cargo fmt --version &>/dev/null; then
    skip "rustfmt not installed (cargo fmt)"
else
    pass "rustfmt"
fi

if ! cargo clippy --version &>/dev/null; then
    skip "clippy not installed"
else
    pass "clippy"
fi

if [[ "$MODE" != "--quick" ]]; then
    if ! command -v zig &>/dev/null; then
        echo -e "  ${YELLOW}WARN${NC}  zig not found — install with: brew install zig"
        echo -e "  ${YELLOW}WARN${NC}  Skipping cross-platform checks"
        MODE="--quick"
    elif ! command -v cargo-zigbuild &>/dev/null; then
        echo -e "  ${YELLOW}WARN${NC}  cargo-zigbuild not found — install with: cargo install cargo-zigbuild"
        echo -e "  ${YELLOW}WARN${NC}  Skipping cross-platform checks"
        MODE="--quick"
    else
        pass "zig + cargo-zigbuild"
    fi
fi

# ── Step 1: Format ────────────────────────────────────────────────────────

step 1 "Formatting (cargo fmt --check)"

if cargo fmt --all -- --check 2>/dev/null; then
    pass "All files formatted correctly"
else
    fail "Formatting issues found — run: cargo fmt --all"
fi

# ── Step 2: Clippy (native) ──────────────────────────────────────────────

step 2 "Clippy — native ($(rustc -vV | grep host | cut -d' ' -f2))"

if cargo clippy --workspace -- -D warnings 2>&1; then
    pass "Clippy clean"
else
    fail "Clippy warnings/errors on native target"
fi

# ── Step 3: Tests ─────────────────────────────────────────────────────────

step 3 "Tests (cargo test)"

if cargo test --workspace 2>&1; then
    pass "All tests pass"
else
    fail "Test failures"
fi

# ── Step 4: Cross-platform compilation ────────────────────────────────────

if [[ "$MODE" == "--quick" ]]; then
    skip "Cross-platform checks (--quick mode)"
else
    TARGETS=()
    case "$MODE" in
        --linux)   TARGETS=("x86_64-unknown-linux-gnu") ;;
        --windows) TARGETS=("x86_64-pc-windows-gnu") ;;
        *)         TARGETS=("x86_64-unknown-linux-gnu" "x86_64-pc-windows-gnu") ;;
    esac

    for target in "${TARGETS[@]}"; do
        step 4 "Cross-compile check — ${target}"

        # Ensure target is installed
        if ! rustup target list --installed | grep -q "$target"; then
            echo "  Installing target ${target}..."
            rustup target add "$target"
        fi

        if cargo zigbuild --workspace --target "$target" 2>&1; then
            pass "Compiles for ${target}"
        else
            fail "Compilation failed for ${target}"
        fi
    done
fi

# ── Summary ───────────────────────────────────────────────────────────────

echo ""
echo -e "${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
if [[ $FAILED -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}  All checks passed!${NC} Safe to push."
else
    echo -e "${RED}${BOLD}  ${FAILED} check(s) failed.${NC} Fix before pushing."
fi
if [[ $SKIPPED -gt 0 ]]; then
    echo -e "  ${YELLOW}(${SKIPPED} skipped)${NC}"
fi
echo -e "${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

exit $FAILED
