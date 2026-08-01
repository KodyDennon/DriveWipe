#!/usr/bin/env bash
#
# Builds a genuinely bootable DriveWipe Live ISO from an Alpine rootfs tarball.
#
# Replaces an earlier "Stage 4" that ran `dd if=/dev/zero of=drivewipe-live.img`
# and announced the result as a bootable image. It was 256 MB of zeros; writing
# it to a USB stick produced an unbootable blank drive.
#
# The whole rootfs is packed into the initramfs and boots entirely into RAM.
# That is the right shape for a wiping appliance: once booted, the USB stick can
# be removed, and no drive the operator might want to erase is holding the
# running system.
#
# Usage: scripts/build-iso.sh <rootfs.tar> <output.iso> [version]

set -euo pipefail

ROOTFS_TAR="${1:?usage: build-iso.sh <rootfs.tar> <output.iso> [version]}"
OUTPUT="${2:?usage: build-iso.sh <rootfs.tar> <output.iso> [version]}"
VERSION="${3:-dev}"

for t in xorriso mksquashfs cpio; do
    command -v "$t" >/dev/null || { echo "missing required tool: $t" >&2; exit 1; }
done
[ -f "$ROOTFS_TAR" ] || { echo "no rootfs tarball at $ROOTFS_TAR" >&2; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

ROOT="$WORK/root"
ISO="$WORK/iso"
mkdir -p "$ROOT" "$ISO/boot/syslinux"

echo "  -> unpacking rootfs"
tar xf "$ROOTFS_TAR" -C "$ROOT"

# The Docker stage stages everything under /drivewipe-live; flatten it if so.
if [ -d "$ROOT/drivewipe-live" ]; then
    mv "$ROOT/drivewipe-live" "$WORK/flat"
    rm -rf "$ROOT"
    mv "$WORK/flat" "$ROOT"
fi

# ── Kernel ──────────────────────────────────────────────────────────────────

KERNEL="$(find "$ROOT" -name 'vmlinuz*' -type f | head -1)"
[ -n "$KERNEL" ] || { echo "no kernel found in the rootfs" >&2; exit 1; }
cp "$KERNEL" "$ISO/boot/vmlinuz"
echo "  -> kernel: $(basename "$KERNEL") ($(du -h "$ISO/boot/vmlinuz" | cut -f1))"

# Kernel modules must travel with the initramfs or the live system cannot see
# SATA, NVMe or USB storage — which is every device it exists to wipe.
MODVER="$(ls "$ROOT/lib/modules" 2>/dev/null | head -1 || true)"
if [ -n "$MODVER" ]; then
    echo "  -> kernel modules: $MODVER"
else
    echo "  !! no /lib/modules in rootfs; storage drivers may be unavailable" >&2
fi

# The whole image is held in RAM, so firmware for hardware this tool never
# touches is a direct cost to the minimum machine it can run on. Alpine's
# linux-firmware is ~520 MB of GPU, wifi and high-end NIC blobs.
#
# This is a denylist rather than a keeplist on purpose: anything unrecognised
# survives, so a storage or RAID controller cannot be starved of firmware by a
# name that was not anticipated here.
if [ -d "$ROOT/lib/firmware" ]; then
    before="$(du -sm "$ROOT/lib/firmware" | cut -f1)"
    for fw in amdgpu nvidia i915 xe radeon cirrus mrvl mellanox qcom mediatek \
              ath9k_htc ath10k ath11k ath12k ath6k brcm rtw88 rtw89 rtlwifi \
              iwlwifi libertas ti-connectivity cypress netronome qed \
              liquidio myricom bnx2x dpaa2 nvidia-gpu intel/vsc intel/ipu \
              intel/ish nxp mtk-sof sof sof-tplg; do
        rm -rf "${ROOT:?}/lib/firmware/${fw:?}"
    done
    after="$(du -sm "$ROOT/lib/firmware" | cut -f1)"
    echo "  -> firmware pruned: ${before} MB -> ${after} MB (storage blobs kept)"
fi

# ── init ────────────────────────────────────────────────────────────────────
# The initramfs is the root filesystem, so /init is PID 1. It sets up the
# pseudo-filesystems, loads storage drivers, then hands the console to
# DriveWipe.

cat > "$ROOT/init" <<'INIT'
#!/bin/sh
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
export TERM=linux
export HOME=/root

mount -t proc     none /proc      2>/dev/null
mount -t sysfs    none /sys       2>/dev/null
mount -t devtmpfs none /dev       2>/dev/null || mount -t tmpfs none /dev
mount -t tmpfs    none /tmp       2>/dev/null
mount -t tmpfs    none /run       2>/dev/null
mkdir -p /dev/pts && mount -t devpts none /dev/pts 2>/dev/null

echo "DriveWipe Live" > /etc/hostname
hostname drivewipe-live 2>/dev/null
echo "DriveWipe Live Environment" > /etc/drivewipe-live

# Storage drivers. Most are modules in Alpine's linux-lts, and without them the
# tool boots to an empty drive list.
if [ -d "/lib/modules/$(uname -r)" ]; then
    depmod -a 2>/dev/null
    for m in sd_mod sr_mod ahci libahci ata_piix nvme nvme_core \
             usb_storage uas xhci_pci ehci_pci virtio_blk virtio_scsi \
             megaraid_sas mpt3sas aacraid; do
        modprobe "$m" 2>/dev/null
    done
fi

# Give the kernel a moment to enumerate before the drive list is drawn.
sleep 2

cat <<'BANNER'

  ____       _         __        ___
 |  _ \ _ __(_)_   ____\ \      / (_)_ __   ___
 | | | | '__| \ \ / / _ \ \ /\ / /| | '_ \ / _ \
 | |_| | |  | |\ V /  __/\ V  V / | | |_) |  __/
 |____/|_|  |_| \_/ \___| \_/\_/  |_| .__/ \___|
                                    |_|
  Live Environment — running entirely in RAM.
  The boot media can be removed.

BANNER

# Automated verification path. Booting with `drivewipe.selftest` on the kernel
# command line runs a non-interactive check and powers off, so CI can assert the
# image really boots and really sees disks, instead of a human squinting at a
# screenshot once.
if grep -q drivewipe.selftest /proc/cmdline 2>/dev/null; then
    echo "DRIVEWIPE_SELFTEST_BEGIN"
    echo "version: $(/usr/local/bin/drivewipe --version 2>&1)"
    echo "methods: $(/usr/local/bin/drivewipe methods --format ids 2>/dev/null | wc -l)"
    echo "--- drives ---"
    /usr/local/bin/drivewipe list 2>&1 | head -40
    echo "--- block devices seen by kernel ---"
    ls /sys/block 2>/dev/null | tr '\n' ' '
    echo
    echo "DRIVEWIPE_SELFTEST_END"
    poweroff -f 2>/dev/null || echo o > /proc/sysrq-trigger
    sleep 30
fi


if [ -x /usr/local/bin/drivewipe ]; then
    /usr/local/bin/drivewipe --tui
    echo
    echo "DriveWipe exited. Dropping to a shell."
    echo "  drivewipe --tui    restart the interface"
    echo "  poweroff           shut down"
else
    echo "ERROR: /usr/local/bin/drivewipe is missing from this image."
fi

exec /bin/sh
INIT
chmod +x "$ROOT/init"

# ── initramfs ───────────────────────────────────────────────────────────────

echo "  -> building initramfs"
COMPRESS="gzip -9"
EXT="gz"
if command -v zstd >/dev/null; then
    COMPRESS="zstd -19 -T0 -q"
    EXT="zst"
fi

( cd "$ROOT" && find . -print0 | cpio --null -o --format=newc --quiet ) \
    | $COMPRESS > "$ISO/boot/initramfs.$EXT"
echo "  -> initramfs: $(du -h "$ISO/boot/initramfs.$EXT" | cut -f1)"

# ── Bootloader ──────────────────────────────────────────────────────────────

SYSLINUX_DIRS="/usr/lib/ISOLINUX /usr/lib/syslinux/modules/bios /usr/share/syslinux"
find_syslinux() {
    for d in $SYSLINUX_DIRS; do
        [ -f "$d/$1" ] && { echo "$d/$1"; return 0; }
    done
    return 1
}

ISOLINUX_BIN="$(find_syslinux isolinux.bin)" \
    || { echo "isolinux.bin not found — install isolinux/syslinux" >&2; exit 1; }
cp "$ISOLINUX_BIN" "$ISO/boot/syslinux/"
for mod in ldlinux.c32 libcom32.c32 libutil.c32 menu.c32 vesamenu.c32; do
    p="$(find_syslinux "$mod")" && cp "$p" "$ISO/boot/syslinux/" || true
done

cat > "$ISO/boot/syslinux/isolinux.cfg" <<CFG
# Mirror the boot menu to serial as well as VGA, so a headless station shows
# the menu rather than appearing to hang.
SERIAL 0 115200
UI menu.c32
PROMPT 0
TIMEOUT 100
DEFAULT drivewipe

MENU TITLE DriveWipe Live ${VERSION}

LABEL drivewipe
    MENU LABEL DriveWipe (normal)
    LINUX /boot/vmlinuz
    INITRD /boot/initramfs.${EXT}
    # Both consoles: wiping stations are often headless and driven over serial.
    APPEND console=tty0 console=ttyS0,115200

LABEL safe
    MENU LABEL DriveWipe (safe mode - no ACPI, no SMP)
    LINUX /boot/vmlinuz
    INITRD /boot/initramfs.${EXT}
    APPEND acpi=off nosmp noapic console=tty0

LABEL serial
    MENU LABEL DriveWipe (serial console ttyS0)
    LINUX /boot/vmlinuz
    INITRD /boot/initramfs.${EXT}
    APPEND console=ttyS0,115200 console=tty0

LABEL selftest
    MENU LABEL Self-test (non-interactive, powers off)
    LINUX /boot/vmlinuz
    INITRD /boot/initramfs.${EXT}
    APPEND drivewipe.selftest console=tty0 console=ttyS0,115200
CFG

# ── ISO ─────────────────────────────────────────────────────────────────────

echo "  -> mastering ISO"
xorriso -as mkisofs \
    -o "$OUTPUT" \
    -V "DRIVEWIPE" \
    -J -r \
    -b boot/syslinux/isolinux.bin \
    -c boot/syslinux/boot.cat \
    -no-emul-boot -boot-load-size 4 -boot-info-table \
    -quiet \
    "$ISO"

# Make the same file work when written straight to a USB stick with dd.
if command -v isohybrid >/dev/null; then
    isohybrid "$OUTPUT" 2>/dev/null || true
fi

echo "  -> ISO: $OUTPUT ($(du -h "$OUTPUT" | cut -f1))"
