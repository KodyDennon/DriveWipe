#!/bin/sh
# DriveWipe Live USB init script
# Runs after Alpine base init, launches DriveWipe TUI
#
# This script:
# 1. Sets up the environment and hostname
# 2. Creates the live environment marker file
# 3. Loads the DriveWipe kernel module
# 4. Loads all storage drivers (HBA, NVMe, USB, RAID)
# 5. Triggers udev enumeration and waits for devices to settle
# 6. Auto-detects frozen SATA drives and unfreezes via suspend/resume
# 7. Configures networking for PXE-booted systems
# 8. Launches the DriveWipe TUI in live mode

set -e

# ── Environment ──────────────────────────────────────────────────────────────

export TERM=linux
export PATH="/usr/local/bin:/usr/bin:/bin:/sbin:/usr/sbin"

# Set hostname
hostname drivewipe-live
echo "drivewipe-live" > /etc/hostname

# Create live environment marker
echo "DriveWipe Live Environment" > /etc/drivewipe-live

# ── Display banner ───────────────────────────────────────────────────────────

clear
echo "=================================================================="
echo "    ____       _            __        ___"
echo "   / __ \\_____(_)   _____  / |       / (_)___  ___"
echo "  / / / / ___/ / | / / _ \\/ /| | /| / / / __ \\/ _ \\"
echo " / /_/ / /  / /| |/ /  __/ ___ |/ |/ / / /_/ /  __/"
echo "/_____/_/  /_/ |___/\\___/_/  |_|__/|_/_/ .___/\\___/"
echo "                                      /_/"
echo "  LIVE — Secure Drive Management Environment"
echo "=================================================================="
echo ""

# ── Mount essential filesystems ──────────────────────────────────────────────

mount -t proc proc /proc 2>/dev/null || true
mount -t sysfs sysfs /sys 2>/dev/null || true
mount -t devtmpfs devtmpfs /dev 2>/dev/null || true
mount -t tmpfs tmpfs /tmp 2>/dev/null || true

# ── Load DriveWipe kernel module ─────────────────────────────────────────────

echo "[*] Loading DriveWipe kernel module..."
if [ -f /lib/modules/$(uname -r)/extra/drivewipe.ko ]; then
    insmod /lib/modules/$(uname -r)/extra/drivewipe.ko 2>/dev/null
    if [ -c /dev/drivewipe ]; then
        echo "    [OK] /dev/drivewipe is ready"
    else
        echo "    [WARN] Module loaded but /dev/drivewipe not created"
    fi
elif [ -f /lib/modules/drivewipe.ko ]; then
    insmod /lib/modules/drivewipe.ko 2>/dev/null
    if [ -c /dev/drivewipe ]; then
        echo "    [OK] /dev/drivewipe is ready"
    else
        echo "    [WARN] Module loaded but /dev/drivewipe not created"
    fi
else
    echo "    [INFO] Kernel module not found — using userspace fallback"
fi

# ── Load storage drivers ────────────────────────────────────────────────────

echo "[*] Loading storage drivers..."

# Core SATA/AHCI
for mod in ahci libahci libata sd_mod sr_mod; do
    modprobe "$mod" 2>/dev/null || true
done

# NVMe
for mod in nvme nvme_core; do
    modprobe "$mod" 2>/dev/null || true
done

# USB storage
for mod in usb_storage uas xhci_hcd xhci_pci ehci_hcd ehci_pci uhci_hcd ohci_hcd ohci_pci; do
    modprobe "$mod" 2>/dev/null || true
done

# SCSI subsystem
for mod in scsi_mod sg scsi_transport_sas; do
    modprobe "$mod" 2>/dev/null || true
done

# Hardware RAID controllers
for mod in mpt3sas megaraid_sas aacraid hpsa mpi3mr smartpqi; do
    modprobe "$mod" 2>/dev/null || true
done

# Virtio (for QEMU/KVM testing)
for mod in virtio_pci virtio_blk virtio_scsi virtio_net; do
    modprobe "$mod" 2>/dev/null || true
done

echo "    [OK] Storage drivers loaded"

# ── Enumerate devices ────────────────────────────────────────────────────────

echo "[*] Enumerating devices..."
udevadm trigger --type=subsystems --action=add 2>/dev/null || true
udevadm trigger --type=devices --action=add 2>/dev/null || true
udevadm settle --timeout=15 2>/dev/null || true

# Wait a moment for late-arriving devices
sleep 2

# Count detected drives
SATA_COUNT=$(ls /sys/block/sd* 2>/dev/null | wc -l || echo 0)
NVME_COUNT=$(ls /sys/block/nvme* 2>/dev/null | wc -l || echo 0)
TOTAL_DRIVES=$((SATA_COUNT + NVME_COUNT))
echo "    [OK] Found ${TOTAL_DRIVES} drives (${SATA_COUNT} SATA/SAS, ${NVME_COUNT} NVMe)"

# ── Auto-unfreeze frozen SATA drives ────────────────────────────────────────

# BIOS freezes ATA security on SATA drives during POST.
# A suspend/resume cycle resets the ATA security state to unfrozen,
# allowing security erase and other ATA security commands.

FROZEN_COUNT=0
for dev in /dev/sd?; do
    [ -b "$dev" ] || continue
    if hdparm -I "$dev" 2>/dev/null | grep -q "frozen"; then
        FROZEN_COUNT=$((FROZEN_COUNT + 1))
    fi
done

if [ "$FROZEN_COUNT" -gt 0 ]; then
    echo "[*] Detected ${FROZEN_COUNT} frozen SATA drive(s)"
    echo "    Performing suspend/resume cycle to unfreeze..."

    # Sync filesystems before suspend
    sync

    # Suspend to RAM — on resume, SATA drives reset to unfrozen state
    echo mem > /sys/power/state 2>/dev/null || {
        echo "    [WARN] Suspend failed — drives may remain frozen"
        echo "    [INFO] Some BIOS configurations prevent suspend from working"
    }

    # Wait for devices to re-enumerate after resume
    sleep 3
    udevadm trigger 2>/dev/null || true
    udevadm settle --timeout=10 2>/dev/null || true

    # Verify unfreeze
    STILL_FROZEN=0
    for dev in /dev/sd?; do
        [ -b "$dev" ] || continue
        if hdparm -I "$dev" 2>/dev/null | grep -q "frozen"; then
            STILL_FROZEN=$((STILL_FROZEN + 1))
        fi
    done

    if [ "$STILL_FROZEN" -eq 0 ]; then
        echo "    [OK] All drives unfrozen successfully"
    else
        echo "    [WARN] ${STILL_FROZEN} drive(s) still frozen"
        echo "    [INFO] Manual unfreeze available in TUI: ATA Security screen"
    fi
else
    if [ "$SATA_COUNT" -gt 0 ]; then
        echo "[*] No frozen SATA drives detected — skipping unfreeze"
    fi
fi

# ── Network configuration ───────────────────────────────────────────────────

# Configure networking if PXE-booted or network interfaces are available
echo "[*] Configuring network..."

# Check if PXE-booted (BOOTIF= or ip=dhcp in kernel cmdline)
PXE_BOOT=0
if grep -q "BOOTIF=" /proc/cmdline 2>/dev/null; then
    PXE_BOOT=1
fi
if grep -q "ip=dhcp" /proc/cmdline 2>/dev/null; then
    PXE_BOOT=1
fi

if [ "$PXE_BOOT" -eq 1 ]; then
    echo "    [INFO] PXE boot detected"
    # Network should already be configured by the kernel ip= parameter
    # Just ensure the interface is up
    for iface in /sys/class/net/eth* /sys/class/net/en*; do
        [ -d "$iface" ] || continue
        IFNAME=$(basename "$iface")
        ip link set "$IFNAME" up 2>/dev/null || true
    done
else
    # Try to get DHCP on the first available interface
    for iface in /sys/class/net/eth* /sys/class/net/en*; do
        [ -d "$iface" ] || continue
        IFNAME=$(basename "$iface")
        [ "$IFNAME" = "lo" ] && continue
        ip link set "$IFNAME" up 2>/dev/null || true
        udhcpc -i "$IFNAME" -n -q -t 3 2>/dev/null && {
            echo "    [OK] Network configured on $IFNAME"
            break
        } || true
    done
fi

# ── System information ──────────────────────────────────────────────────────

echo ""
echo "System summary:"
echo "  Kernel:     $(uname -r)"
echo "  CPU:        $(grep -c ^processor /proc/cpuinfo 2>/dev/null || echo '?') cores"
echo "  RAM:        $(awk '/MemTotal/ {printf "%.0f MB", $2/1024}' /proc/meminfo 2>/dev/null || echo '?')"
echo "  Drives:     ${TOTAL_DRIVES} detected"
if [ -c /dev/drivewipe ]; then
    echo "  Module:     loaded (/dev/drivewipe)"
else
    echo "  Module:     not loaded (userspace mode)"
fi
echo ""

# ── Launch DriveWipe TUI ─────────────────────────────────────────────────────

echo "[*] Starting DriveWipe TUI..."
sleep 1

if [ -x /usr/local/bin/drivewipe-tui ]; then
    /usr/local/bin/drivewipe-tui --live
else
    echo ""
    echo "ERROR: DriveWipe TUI binary not found at /usr/local/bin/drivewipe-tui"
    echo "Dropping to shell for manual recovery."
fi

# ── Post-TUI shell ──────────────────────────────────────────────────────────

echo ""
echo "=================================================================="
echo "  DriveWipe TUI exited."
echo ""
echo "  Commands:"
echo "    drivewipe-tui --live    Restart the TUI"
echo "    drivewipe               CLI interface"
echo "    hdparm -I /dev/sdX      Check drive identity"
echo "    nvme list               List NVMe devices"
echo "    poweroff                Shut down"
echo "    reboot                  Restart"
echo "=================================================================="
echo ""
exec /bin/sh
