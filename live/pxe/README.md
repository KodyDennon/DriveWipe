# DriveWipe PXE Boot Infrastructure

## Overview

This directory contains everything needed to set up a PXE boot server for
DriveWipe Live. Clients network-boot directly into the DriveWipe TUI, with no
USB drives required.

## Directory Structure

```
live/pxe/
├── dnsmasq.conf          # DHCP + TFTP server configuration
├── ipxe/
│   ├── boot.ipxe         # iPXE chainload script (auto-boot)
│   └── menu.ipxe         # iPXE boot menu with options
└── README.md             # This file
```

## Prerequisites

- **dnsmasq** — DHCP + TFTP server (`apt install dnsmasq` / `dnf install dnsmasq`)
- **iPXE** — Network boot firmware (optional, for HTTP boot; TFTP works without it)
- **DriveWipe Live artifacts** — Built via `scripts/build-live.sh`:
  - `vmlinuz` — Linux kernel
  - `initramfs.img` — DriveWipe Live initramfs
  - `drivewipe-live.iso` — Full ISO (optional, for hybrid boot)

## Quick Start

### 1. Build live artifacts

```bash
# From the repository root:
./scripts/build-live.sh

# Or via xtask:
cargo xtask live-build
```

### 2. Copy boot files to TFTP root

```bash
sudo mkdir -p /var/lib/tftpboot/drivewipe
sudo cp output/vmlinuz /var/lib/tftpboot/drivewipe/
sudo cp output/initramfs.img /var/lib/tftpboot/drivewipe/
sudo cp live/pxe/ipxe/boot.ipxe /var/lib/tftpboot/drivewipe/
sudo cp live/pxe/ipxe/menu.ipxe /var/lib/tftpboot/drivewipe/
```

### 3. Configure and start dnsmasq

```bash
# Edit the configuration for your network interface:
sudo cp live/pxe/dnsmasq.conf /etc/dnsmasq.d/drivewipe.conf
sudo vi /etc/dnsmasq.d/drivewipe.conf  # adjust interface, DHCP range

# Restart dnsmasq:
sudo systemctl restart dnsmasq
```

### 4. Boot a client

Set the client machine to PXE boot (usually F12 or network boot in BIOS/UEFI).
The iPXE menu will appear, and the client will auto-boot into DriveWipe Live
after a 10-second timeout.

## Testing with QEMU

```bash
# Test PXE boot locally without hardware:
qemu-system-x86_64 \
    -m 2G \
    -boot n \
    -device virtio-net-pci,netdev=net0 \
    -netdev user,id=net0,tftp=/var/lib/tftpboot,bootfile=drivewipe/boot.ipxe
```

## Network Requirements

- PXE server and clients must be on the same Layer 2 network (same VLAN/subnet)
- No other DHCP servers should be running on the same network segment
- If using an existing DHCP server, configure it to point `next-server` and
  `filename` to the PXE server instead of running dnsmasq DHCP

## Security Considerations

- The PXE boot process is **unencrypted** over TFTP — only use on trusted networks
- DriveWipe Live runs with root privileges for direct drive access
- Consider using HTTPS boot (iPXE supports it) in production environments
