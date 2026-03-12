# DriveWipe Live USB

Bootable live USB image and PXE network boot environment for running DriveWipe without installing an OS. Wipe any drive — including the boot drive — in a purpose-built data sanitization environment.

## Overview

Based on Alpine Linux (minimal, ~200 MB target image). Boots directly into the DriveWipe TUI with full hardware access, including a custom kernel module for direct ATA/NVMe passthrough.

## Building

```bash
# Via xtask (preferred)
cargo xtask live-build

# Or directly
./scripts/build-live.sh
```

Requirements:
- Docker (for building the kernel module and assembling the rootfs)
- `syslinux` and `grub` (for BIOS/UEFI boot support)
- Root privileges (for loop mounting)

## Directory Structure

```
live/
  alpine-config/         # Alpine Linux base configuration
    init-script.sh       # Boot script: loads drivers, module, unfreezes, launches TUI
    packages.txt         # APK packages for the live image
  pxe/                   # PXE network boot infrastructure
    dnsmasq.conf         # DHCP + TFTP server configuration
    ipxe/boot.ipxe       # iPXE auto-boot script
    ipxe/menu.ipxe       # iPXE interactive menu
    README.md            # PXE setup guide
  syslinux.cfg           # BIOS boot configuration
  grub.cfg               # UEFI boot configuration
```

## Boot Flow

1. BIOS/UEFI loads bootloader (syslinux or GRUB)
2. Kernel boots with custom initramfs and `drivewipe.live=1`
3. Minimal Alpine userspace initializes
4. `init-script.sh` runs:
   - Sets hostname to `drivewipe-live`, creates `/etc/drivewipe-live` marker
   - Loads DriveWipe kernel module (`drivewipe.ko`) if available
   - Loads all storage drivers (AHCI, NVMe, USB, SCSI, RAID controllers)
   - Triggers device enumeration (`udevadm trigger && settle`)
   - Detects frozen drives, performs suspend/resume to unfreeze
   - Configures networking for PXE-booted systems
5. DriveWipe TUI launches in live mode (`drivewipe-tui --live`)
6. On exit, drops to a root shell

## Live TUI Features

When running in live mode, the TUI adds four additional screens:

| Screen | Key Actions |
|---|---|
| **Live Dashboard** | System overview, quick navigation (1/2/3) |
| **HPA/DCO Manager** | Detect (d), Remove HPA (r), Restore DCO (R), Freeze DCO (F) |
| **ATA Security Manager** | View security state, Unfreeze all drives (u) |
| **Kernel Module Status** | Module version, capabilities, Refresh (r) |

## Included Drivers

- AHCI (SATA controllers)
- NVMe
- USB storage (UAS + mass storage)
- SCSI
- virtio-blk (for VM testing)
- MegaRAID SAS, Adaptec AACRAID, MPT3SAS (enterprise RAID)

## Kernel Module

The custom kernel module (`kernel/drivewipe/`) provides:
- Direct ATA command passthrough (bypasses SCSI translation)
- Direct NVMe admin command passthrough
- HPA/DCO detection and removal
- DMA-coherent zero-copy I/O
- ATA security state querying

Falls back to SG_IO userspace commands when the module is unavailable.

## Available Artifacts

| Artifact | Filename | Description |
|---|---|---|
| **ISO Image** | `drivewipe-live-v1.2.0.iso` | Write to USB with Rufus or `dd`. |
| **PXE Network** | `drivewipe-live-v1.2.0-pxe.tar.gz` | Dissected artifacts for network booting. |

## PXE Network Boot

For wiping entire racks without USB drives, use the `drivewipe-live-v1.2.0-pxe.tar.gz` archive. This provides a turnkey boot environment:

1. **Extract** to your TFTP root (e.g., `/var/lib/tftpboot`).
2. **Configure** your DHCP/TFTP server using the included `dnsmasq.conf`.
3. **Boot** clients to the network (Supports BIOS and UEFI).

See [live/pxe/README.md](pxe/README.md) for full setup.
