#!/usr/bin/env bash
#
# Exercises the wipe engine against a real block device using a loopback file.
#
# Everything in the test suite runs against an in-memory mock, which never
# touches O_DIRECT, real block-size alignment, BLKDISCARD, or the kernel's
# block layer. This closes that gap without risking a physical drive.
#
# Needs root for losetup. It only ever touches the loop device it creates.
#
#   sudo ./scripts/test-device.sh                  # default methods
#   sudo ./scripts/test-device.sh --size 512       # 512 MiB image
#   sudo ./scripts/test-device.sh --method gutmann # one specific method

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY="${DRIVEWIPE_BIN:-$ROOT_DIR/target/release/drivewipe}"
SIZE_MB=128
METHODS=(zero nist-800-88-clear dod-short afssi-5020 navso-p-5239-26)

while [ $# -gt 0 ]; do
    case "$1" in
        --size)   SIZE_MB="$2"; shift 2 ;;
        --method) METHODS=("$2"); shift 2 ;;
        --bin)    BINARY="$2"; shift 2 ;;
        -h|--help) sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "unknown option: $1" >&2; exit 1 ;;
    esac
done

PASS=0; FAIL=0
pass() { printf '  \033[32mPASS\033[0m  %s\n' "$1"; PASS=$((PASS+1)); }
fail() { printf '  \033[31mFAIL\033[0m  %s\n' "$1"; FAIL=$((FAIL+1)); }
info() { printf '  ----  %s\n' "$1"; }

[ "$(id -u)" = "0" ] || { echo "must run as root (losetup needs it): sudo $0" >&2; exit 1; }
[ -x "$BINARY" ] || { echo "no binary at $BINARY — cargo build --release" >&2; exit 1; }
command -v losetup >/dev/null || { echo "losetup not found" >&2; exit 1; }

WORK="$(mktemp -d)"
LOOP=""
cleanup() {
    [ -n "$LOOP" ] && losetup -d "$LOOP" 2>/dev/null || true
    rm -rf "$WORK"
}
trap cleanup EXIT INT TERM

IMG="$WORK/disk.img"
dd if=/dev/urandom of="$IMG" bs=1M count="$SIZE_MB" status=none
LOOP="$(losetup -fP --show "$IMG")"

# Refuse to continue unless we really got a loop device, so a bug here can
# never point the wipe at something else.
case "$LOOP" in
    /dev/loop*) ;;
    *) echo "refusing to continue: '$LOOP' is not a loop device" >&2; exit 1 ;;
esac

echo
echo "Device: $LOOP  (${SIZE_MB} MiB, backed by $IMG)"
echo "Binary: $BINARY"
echo

# Confirm the device starts as random data, so a wipe genuinely changes it.
before="$(dd if="$LOOP" bs=4096 count=1 status=none | sha256sum | cut -d' ' -f1)"
zeros="$(head -c 4096 /dev/zero | sha256sum | cut -d' ' -f1)"
[ "$before" != "$zeros" ] && pass "device starts as non-zero data" \
                          || fail "device did not start as random data"

for method in "${METHODS[@]}"; do
    echo
    info "method: $method"

    if "$BINARY" wipe --device "$LOOP" --method "$method" \
        --force --yes-i-know-what-im-doing > "$WORK/$method.log" 2>&1; then
        pass "$method completed"
    else
        fail "$method failed (see below)"
        tail -20 "$WORK/$method.log" | sed 's/^/        /'
        continue
    fi

    # Verification is mandatory for these standards, so the run must say so.
    if grep -qE "Verification:[[:space:]]*PASSED" "$WORK/$method.log"; then
        pass "$method reported verification PASSED"
    else
        case "$method" in
            zero) info "$method does not mandate verification" ;;
            *)    fail "$method did not report a passing verification" ;;
        esac
    fi

    # The engine must have actually changed the surface.
    after="$(dd if="$LOOP" bs=4096 count=1 status=none | sha256sum | cut -d' ' -f1)"
    [ "$after" != "$before" ] && pass "$method changed the device contents" \
                             || fail "$method left the first block untouched"

    # A zero-terminated method must leave zeros; confirm with an independent read.
    case "$method" in
        zero|nist-800-88-clear)
            if [ "$after" = "$zeros" ]; then
                pass "$method left the surface zeroed"
            else
                fail "$method should have zeroed the surface"
            fi
            ;;
    esac

    # Restore random data so the next method starts from a known non-zero state.
    dd if=/dev/urandom of="$LOOP" bs=1M count="$SIZE_MB" status=none conv=fsync 2>/dev/null || true
    before="$(dd if="$LOOP" bs=4096 count=1 status=none | sha256sum | cut -d' ' -f1)"
done

# ── Corruption detection ────────────────────────────────────────────────────
# Wipe, then flip a byte and confirm `verify` notices. This is the property the
# whole verification rework rests on.
echo
info "verification catches a modified sector"
"$BINARY" wipe --device "$LOOP" --method zero --force --yes-i-know-what-im-doing \
    >/dev/null 2>&1 || true
printf '\xff' | dd of="$LOOP" bs=1 seek=$((7 * 1024 * 1024)) conv=notrunc status=none

if "$BINARY" verify --device "$LOOP" --pattern zero >"$WORK/verify.log" 2>&1; then
    fail "verify passed a device with a modified byte"
elif grep -q "Verification failed at offset" "$WORK/verify.log"; then
    pass "verify identified the modified byte"
else
    # Exiting non-zero is not proof of detection. Before the O_DIRECT
    # alignment fix, verify failed with EINVAL and this looked like a pass.
    fail "verify failed, but not by detecting the modification:"
    sed 's/^/        /' "$WORK/verify.log" | head -6
fi

echo
echo "  $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
