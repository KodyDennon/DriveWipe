#!/usr/bin/env bash
#
# Boots the DriveWipe Live ISO in QEMU with virtual disks attached and asserts
# that it actually works.
#
# The previous "ISO" was 256 MB of zeros that nothing ever booted, and the
# release notes advertised it anyway. Nothing here is taken on trust: the image
# must carry a boot record, must boot, must start DriveWipe, and must enumerate
# the disks presented to it.
#
# Two checks, because they cover different failures:
#
#   1. The ISO carries an El Torito boot record and the isolinux payload, which
#      is what makes the *media* bootable.
#   2. The kernel and initramfs extracted from it boot to a working DriveWipe
#      that sees attached SATA and NVMe disks, which is what makes the *system*
#      work. These are booted directly so the self-test can be selected on the
#      kernel command line; `-append` is not available when booting `-cdrom`.
#
# Usage: scripts/test-iso.sh <image.iso>

set -euo pipefail

ISO="${1:?usage: test-iso.sh <image.iso>}"
[ -f "$ISO" ] || { echo "no ISO at $ISO" >&2; exit 1; }
command -v qemu-system-x86_64 >/dev/null || { echo "qemu-system-x86_64 required" >&2; exit 1; }
command -v xorriso >/dev/null || { echo "xorriso required" >&2; exit 1; }

PASS=0; FAIL=0
pass() { printf '  PASS  %s\n' "$1"; PASS=$((PASS+1)); }
fail() { printf '  FAIL  %s\n' "$1"; FAIL=$((FAIL+1)); }

WORK="$(mktemp -d)"
trap 'chmod -R u+w "$WORK" 2>/dev/null; rm -rf "$WORK"' EXIT

echo "Testing $(basename "$ISO") ($(du -h "$ISO" | cut -f1))"
echo

# ── 1. Bootable media ───────────────────────────────────────────────────────

if xorriso -indev "$ISO" -report_el_torito plain 2>&1 | grep -qi "El Torito"; then
    pass "ISO carries an El Torito boot record"
else
    fail "ISO has no boot record — it would not boot from USB or DVD"
fi

xorriso -osirrox on -indev "$ISO" -extract / "$WORK/iso" >/dev/null 2>&1 || true

[ -f "$WORK/iso/boot/syslinux/isolinux.bin" ] \
    && pass "isolinux bootloader present" \
    || fail "isolinux.bin missing from the image"

[ -f "$WORK/iso/boot/syslinux/isolinux.cfg" ] \
    && pass "boot menu configuration present" \
    || fail "isolinux.cfg missing"

KERNEL="$WORK/iso/boot/vmlinuz"
INITRD="$(ls "$WORK/iso/boot/"initramfs.* 2>/dev/null | head -1 || true)"

[ -f "$KERNEL" ] && pass "kernel present ($(du -h "$KERNEL" | cut -f1))" \
                 || { fail "no kernel in the image"; echo; echo "  $PASS passed, $((FAIL+1)) failed"; exit 1; }
[ -n "$INITRD" ] && pass "initramfs present ($(du -h "$INITRD" | cut -f1))" \
                 || { fail "no initramfs in the image"; echo; echo "  $PASS passed, $((FAIL+1)) failed"; exit 1; }

# ── 2. It actually runs ─────────────────────────────────────────────────────

# Two disks on different transports, so more than one storage driver has to
# come up for both to be visible.
truncate -s 512M "$WORK/sata.img"
truncate -s 256M "$WORK/nvme.img"

ACCEL=()
if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then ACCEL=(-enable-kvm); fi

echo
echo "  booting${ACCEL:+ with KVM}..."

timeout 420 qemu-system-x86_64 "${ACCEL[@]}" \
    -m 3072 -smp 2 \
    -kernel "$KERNEL" -initrd "$INITRD" \
    -append "drivewipe.selftest console=ttyS0,115200" \
    -drive file="$WORK/sata.img",format=raw,if=none,id=sata0 \
    -device ich9-ahci,id=ahci \
    -device ide-hd,drive=sata0,bus=ahci.0 \
    -drive file="$WORK/nvme.img",format=raw,if=none,id=nvme0 \
    -device nvme,drive=nvme0,serial=DWTEST0001 \
    -display none -serial "file:$WORK/serial.log" -no-reboot \
    </dev/null >/dev/null 2>&1 || true

LOG="$WORK/serial.log"
if [ ! -s "$LOG" ]; then
    fail "no serial output — the system did not boot"
    echo; echo "  $PASS passed, $FAIL failed"; exit 1
fi

grep -q "Run /init as init process" "$LOG" \
    && pass "kernel handed over to /init" \
    || fail "kernel never reached /init"

grep -q "Live Environment" "$LOG" \
    && pass "live init ran" \
    || fail "init did not reach its banner"

grep -qiE "ahci" "$LOG" \
    && pass "AHCI/SATA driver loaded" \
    || fail "no SATA driver — SATA disks would be invisible"

grep -qi "nvme" "$LOG" \
    && pass "NVMe driver loaded" \
    || fail "no NVMe driver — NVMe disks would be invisible"

if grep -q "DRIVEWIPE_SELFTEST_BEGIN" "$LOG"; then
    pass "self-test ran inside the live environment"

    grep -qE "^version: drivewipe [0-9]" "$LOG" \
        && pass "drivewipe executes in the image" \
        || fail "drivewipe did not report a version"

    n="$(sed -n 's/^methods: //p' "$LOG" | tr -dc '0-9\n' | head -1)"
    if [ -n "$n" ] && [ "$n" -ge 27 ]; then
        pass "all $n wipe methods registered"
    else
        fail "expected >=27 methods, saw '${n:-none}'"
    fi

    # The entire point of the image: it has to see the disks.
    if grep -qE "/dev/(sd[a-z]|nvme[0-9]n[0-9])" "$LOG"; then
        pass "DriveWipe enumerated the attached disks"
        grep -oE "/dev/(sd[a-z]|nvme[0-9]n[0-9])" "$LOG" | sort -u | sed 's/^/          /'
    else
        fail "DriveWipe listed no drives despite two being attached"
        sed -n '/--- drives ---/,/DRIVEWIPE_SELFTEST_END/p' "$LOG" | head -15 | sed 's/^/        /'
    fi
else
    fail "self-test never started"
    tail -25 "$LOG" | sed 's/^/        /'
fi

echo
echo "  $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
