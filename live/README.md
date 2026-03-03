# DriveWipe Live USB

Bootable live USB image for running DriveWipe without installing an OS.

## Overview

Based on Alpine Linux (minimal, ~200MB target image). Boots into DriveWipe TUI automatically.

## Building

```bash
./scripts/build-live.sh
```

Requirements:
- Docker (for building the image)
- `syslinux` and `grub` (for BIOS/UEFI boot support)
- Root privileges (for loop mounting)

## Contents

- `alpine-config/` — Alpine Linux base configuration
- `initramfs/` — Custom initramfs with DriveWipe binaries
- `syslinux.cfg` — BIOS boot configuration
- `grub.cfg` — UEFI boot configuration

## Boot Flow

1. BIOS/UEFI loads bootloader
2. Kernel boots with custom initramfs
3. Minimal Alpine userspace initializes
4. DriveWipe TUI launches automatically in fullscreen
5. On exit, drops to a root shell

## Included Drivers

- AHCI (SATA controllers)
- NVMe
- USB storage (UAS + mass storage)
- SCSI
- virtio-blk (for VM testing)
