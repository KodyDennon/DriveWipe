# DriveWipe Live — Maximum Power Live Environment

## Implementation Status

> **All 6 phases are complete.** This plan has been fully implemented.

| Phase | Description | Status |
|---|---|---|
| 1 | Foundation (`drivewipe-live` crate + HPA/DCO via SG_IO) | ✅ Done |
| 2 | TUI Live Mode (4 screens, handlers, live probing) | ✅ Done |
| 3 | Kernel Module (`kernel/drivewipe/` — 9 C source files) | ✅ Done |
| 4 | Live Image Upgrade (`init-script.sh`, boot configs, builder) | ✅ Done |
| 5 | PXE Boot (`live/pxe/` — dnsmasq, iPXE, README) | ✅ Done |
| 6 | Build Integration & Polish (xtask `live-build`, CI, versioning) | ✅ Done |

**Remaining work:** `forensic/hidden.rs` contains data types only — live HPA/DCO detection is wired through the TUI's `refresh_drives()` and action handlers, not through the forensic session path.

---

## Context

DriveWipe currently runs as a normal application on top of an OS. When booted into a Linux live environment, it can be **drastically more powerful** — direct kernel integration, hidden area manipulation, ATA frozen state bypass, zero-copy DMA I/O, and PXE network boot for wiping entire racks. The existing `live/` directory has a basic Alpine setup; this plan upgrades it into a serious data sanitization platform.

**Scope**: Both new hardware capabilities in Rust code AND upgraded live image infrastructure with custom kernel module, PXE boot, and advanced TUI features.

---

## New Directory Structure

```
DriveWipe/
  kernel/drivewipe/                    # NEW — Custom Linux kernel module
    Makefile
    Kbuild
    drivewipe_main.c                   # Module init, /dev/drivewipe char device
    drivewipe_ata.c                    # Direct ATA passthrough via libata (bypasses SCSI)
    drivewipe_nvme.c                   # Direct NVMe admin passthrough
    drivewipe_hpa.c                    # HPA/DCO detect + remove via ATA commands
    drivewipe_dma.c                    # DMA-coherent buffer management, zero-copy I/O
    drivewipe_ioctl.h                  # Shared ioctl API (kernel ↔ userspace)
    drivewipe_internal.h               # Internal kernel headers

  crates/drivewipe-live/               # NEW — Live environment Rust crate
    Cargo.toml
    src/
      lib.rs
      detect.rs                        # Live environment detection
      kernel_module.rs                 # /dev/drivewipe ioctl interface
      hpa.rs                           # HPA detect + remove (kernel module + SG_IO fallback)
      dco.rs                           # DCO detect + restore (kernel module + SG_IO fallback)
      ata_security.rs                  # ATA security state query
      unfreeze.rs                      # Suspend/resume cycle to unfreeze drives
      dma_io.rs                        # Zero-copy DMA I/O via kernel module
      capabilities.rs                  # LiveCapabilities feature gating

  crates/drivewipe-tui/src/ui/
    live_dashboard.rs                  # NEW — System overview in live mode
    hpa_dco_screen.rs                  # NEW — HPA/DCO detection and removal UI
    ata_security_screen.rs             # NEW — ATA security state / unfreeze UI
    kernel_status_screen.rs            # NEW — Kernel module status display

  live/pxe/                            # NEW — PXE boot infrastructure
    dnsmasq.conf                       # DHCP + TFTP server config
    ipxe/boot.ipxe                     # iPXE boot menu script
    ipxe/drivewipe.ipxe                # DriveWipe-specific iPXE chain
    README.md                          # PXE server setup guide

  scripts/
    build-kernel-module.sh             # NEW — Standalone kernel module build
    setup-pxe-server.sh                # NEW — Automated PXE server setup
```

### Files Modified

- `Cargo.toml` — Add `drivewipe-live` to workspace members
- `crates/drivewipe-core/src/error.rs` — New error variants (HPA/DCO/kernel module)
- `crates/drivewipe-core/src/types.rs` — Enhanced `HiddenAreaInfo` with native/factory LBA fields
- `crates/drivewipe-core/src/drive/linux.rs` — Populate `hidden_areas` and `ata_security` with real data
- `crates/drivewipe-core/src/forensic/hidden.rs` — Wire up real HPA/DCO detection
- `crates/drivewipe-tui/Cargo.toml` — Add optional `drivewipe-live` dep behind `live` feature
- `crates/drivewipe-tui/src/app.rs` — Add live env state, new `AppScreen` variants
- `crates/drivewipe-tui/src/ui/mod.rs` — Dispatch new live screens
- `crates/drivewipe-tui/src/ui/main_menu.rs` — Conditional live mode menu items
- `live/alpine-config/init-script.sh` — Kernel module loading, driver loading, unfreeze logic
- `live/alpine-config/packages.txt` — Additional storage/network packages
- `live/grub.cfg` — Add `drivewipe.live=1` cmdline, verbose/debug entries
- `live/syslinux.cfg` — Add `drivewipe.live=1` cmdline
- `scripts/build-live.sh` — Kernel module build stage, PXE artifact generation
- `crates/xtask/src/main.rs` — Add `BuildLive`, `BuildKernelModule`, `BuildPxe` commands

---

## 1. Kernel Module (`kernel/drivewipe/`)

### ioctl API (`drivewipe_ioctl.h`)

Shared header between kernel and userspace. Character device at `/dev/drivewipe`. Requires `CAP_SYS_RAWIO`.

| ioctl | Struct | Purpose |
|---|---|---|
| `DW_IOC_ATA_CMD` | `dw_ata_cmd` | Raw ATA command passthrough (bypasses SCSI translation) |
| `DW_IOC_NVME_CMD` | `dw_nvme_cmd` | Raw NVMe admin command passthrough |
| `DW_IOC_HPA_DETECT` | `dw_hpa_info` | READ NATIVE MAX ADDRESS, compare with current max |
| `DW_IOC_HPA_REMOVE` | `dw_hpa_info` | SET MAX ADDRESS to native max (permanent) |
| `DW_IOC_DCO_DETECT` | `dw_dco_info` | DEVICE CONFIGURATION IDENTIFY (0xB1/0xC2) |
| `DW_IOC_DCO_RESTORE` | `dw_dco_info` | DEVICE CONFIGURATION RESTORE (0xB1/0xC3) |
| `DW_IOC_DCO_FREEZE` | `dw_dco_info` | DEVICE CONFIGURATION FREEZE LOCK (0xB1/0xC5) |
| `DW_IOC_DMA_IO` | `dw_dma_request` | Zero-copy DMA read/write |
| `DW_IOC_ATA_SEC_STATE` | `dw_ata_security_state` | Query ATA security state (frozen/locked/enabled) |
| `DW_IOC_MODULE_INFO` | `dw_module_info` | Query module version + capabilities bitmask |

### Key structs

```c
struct dw_ata_cmd {
    __u8  command, feature, device, protocol;
    __u16 sector_count;
    __u64 lba;
    __u32 data_len;
    __u64 data_ptr;        // userspace buffer
    __u32 timeout_ms;
    __u8  status, error;   // output
    __u32 result_len;      // output
};

struct dw_hpa_info {
    char  device[64];
    __u64 current_max_lba;
    __u64 native_max_lba;
    __u8  hpa_present;
    __u64 hpa_sectors;
};

struct dw_dco_info {
    char  device[64];
    __u8  dco_present;
    __u64 dco_real_max_lba;
    __u64 dco_current_max;
    __u8  dco_features[512]; // raw DCO IDENTIFY data
};

struct dw_ata_security_state {
    char  device[64];
    __u8  supported, enabled, locked, frozen;
    __u8  count_expired, enhanced_erase_supported;
    __u16 erase_time_normal, erase_time_enhanced; // minutes
};

struct dw_module_info {
    __u32 version_major, version_minor, version_patch;
    __u32 capabilities;    // DW_CAP_* bitmask
};
```

### Module source files

- **`drivewipe_main.c`** — `misc_register` for `/dev/drivewipe`, ioctl dispatcher, `CAP_SYS_RAWIO` enforcement
- **`drivewipe_ata.c`** — Direct ATA via libata's `ata_exec_internal()` (bypasses SCSI translation layer entirely). Fallback to SG_IO if libata internal access unavailable
- **`drivewipe_nvme.c`** — Direct NVMe via `nvme_submit_sync_cmd()` (bypasses NVMe driver command filtering)
- **`drivewipe_hpa.c`** — READ NATIVE MAX ADDRESS (0xF8/0x27 for 28/48-bit), SET MAX ADDRESS (0xF9/0x37), DCO IDENTIFY/RESTORE/FREEZE (0xB1 with features 0xC2/0xC3/0xC5)
- **`drivewipe_dma.c`** — `dma_alloc_coherent()` buffers for zero-copy I/O

Safety: all ioctls validate with `access_ok()` + `copy_from_user()`/`copy_to_user()`. ATA/NVMe commands validated against allowlist. DCO RESTORE requires magic confirmation value. Module is GPL (required for libata/NVMe kernel symbols).

---

## 2. Rust Crate: `crates/drivewipe-live/`

### Dependencies
```toml
[dependencies]
drivewipe-core = { path = "../drivewipe-core" }
thiserror.workspace = true
log.workspace = true
serde.workspace = true
serde_json.workspace = true
nix.workspace = true
libc.workspace = true
```

### Modules

**`detect.rs`** — Live environment detection:
- File marker: `/etc/drivewipe-live`
- Kernel cmdline: `drivewipe.live=1` in `/proc/cmdline`
- Hostname: `drivewipe-live`
- Kernel module: `/dev/drivewipe` exists, `DW_IOC_MODULE_INFO` succeeds
- PXE: `BOOTIF=` or `ip=dhcp` in `/proc/cmdline`

**`kernel_module.rs`** — `/dev/drivewipe` fd wrapper with typed ioctl methods. Pattern matches existing `libc::ioctl()` usage in `ata.rs` and `nvme.rs`.

**`hpa.rs`** — HPA detection and removal:
1. Try kernel module: `DW_IOC_HPA_DETECT` / `DW_IOC_HPA_REMOVE`
2. Fallback: SG_IO with ATA_16 CDB for READ NATIVE MAX ADDRESS (reuses `SgIoHdr` pattern from `crates/drivewipe-core/src/wipe/firmware/ata.rs:186-295`)
3. Compare native max LBA vs IDENTIFY DEVICE words 60-61 (28-bit) / 100-103 (48-bit)

**`dco.rs`** — DCO detection and removal:
1. Try kernel module: `DW_IOC_DCO_DETECT` / `DW_IOC_DCO_RESTORE` / `DW_IOC_DCO_FREEZE`
2. Fallback: SG_IO with ATA_16 for DEVICE CONFIGURATION commands (0xB1)
3. Parse 512-byte DCO IDENTIFY response for factory capacity and restricted features

**`ata_security.rs`** — Query security state from IDENTIFY DEVICE words 82, 85, 89, 128. Report frozen/locked/enabled status with estimated erase times.

**`unfreeze.rs`** — Write `"mem"` to `/sys/power/state` to trigger suspend/resume cycle. The BIOS freezes drives during boot; after resume, drives reset to unfrozen state.

**`dma_io.rs`** — Zero-copy I/O path via `DW_IOC_DMA_IO`. Implements `RawDeviceIo` trait alternative for maximum throughput.

**`capabilities.rs`** — `LiveCapabilities` struct probes what's actually available and gates features accordingly.

---

## 3. Core Crate Changes

### `error.rs` — New variants
```rust
HpaError(String),
DcoError(String),
HiddenAreaRemovalFailed { reason: String },
DcoFrozen,
KernelModuleNotLoaded(String),
KernelModuleError(String),
LiveEnvironmentRequired(String),
```

### `types.rs` — Enhanced `HiddenAreaInfo`
Add `hpa_native_max_lba`, `hpa_current_max_lba`, `dco_factory_max_lba`, `dco_features_restricted: Vec<String>`.

### `drive/linux.rs` — Enhanced enumeration
When drivewipe-live is available, populate `hidden_areas` and `ata_security` with real data instead of `::default()`.

### `forensic/hidden.rs` — Real detection
Wire `ForensicSession::execute()` to call live crate's HPA/DCO detection (currently sets `hidden_areas: None`).

---

## 4. TUI Live Mode

### Feature flag
```toml
# crates/drivewipe-tui/Cargo.toml
[features]
live = ["dep:drivewipe-live"]

[dependencies]
drivewipe-live = { path = "../drivewipe-live", optional = true }
```

### New `AppScreen` variants
`LiveDashboard`, `HpaDcoManager`, `AtaSecurityManager`, `KernelModuleStatus`

### Conditional main menu
When live mode detected, show additional items: HPA/DCO Manager, ATA Security, Live Dashboard. Live mode gets amber accent color and "DRIVEWIPE LIVE" branding.

### New screens

- **`live_dashboard.rs`** — Kernel/module version, CPU/RAM, all drives with HPA/DCO indicators, network info, hwmon temps
- **`hpa_dco_screen.rs`** — Drive table (current vs native capacity, HPA/DCO status). Actions: Detect, Remove HPA, Restore DCO, Freeze DCO. Big red confirmation before removal. Before/after capacity display
- **`ata_security_screen.rs`** — SATA drives with security state. Frozen drives highlighted red with Unfreeze action. Erase time estimates from IDENTIFY DEVICE
- **`kernel_status_screen.rs`** — Module version, capabilities bitmask, command stats, recent dmesg errors

### `--live` CLI flag
Explicit live mode activation (auto-detected otherwise).

---

## 5. PXE Boot

### `live/pxe/dnsmasq.conf`
DHCP (192.168.100.50-150) + TFTP + iPXE chainloading. Isolated network interface.

### `live/pxe/ipxe/boot.ipxe`
Menu: Boot DriveWipe Live / Verbose / Debug Shell / iPXE Shell / Reboot. HTTP kernel+initramfs transfer (faster than TFTP).

### `scripts/setup-pxe-server.sh`
Automated setup: install dnsmasq, create TFTP directory, copy kernel+initramfs+iPXE, start service.

### `live/pxe/README.md`
Network requirements, server setup on Ubuntu/Alpine/CentOS, BIOS/UEFI config, scaling to hundreds of machines, security/network isolation.

---

## 6. Live Image Builder Updates

### `scripts/build-live.sh` — New Docker stages
```
Stage 1: Build Rust binaries (musl static, --features live)
Stage 2: Build kernel module (alpine-sdk + linux-lts-dev)
Stage 3: Assemble rootfs (binaries + module + configs)
Stage 4: Create bootable image + PXE artifacts
```

### `live/alpine-config/init-script.sh` — Upgraded
1. Set hostname, create `/etc/drivewipe-live` marker
2. `insmod drivewipe.ko`
3. Load all storage drivers (ahci, nvme, usb_storage, uas, mpt3sas, megaraid_sas, aacraid, virtio)
4. `udevadm trigger && udevadm settle`
5. Auto-detect frozen drives, suspend/resume to unfreeze if any found
6. Network config for PXE-booted systems
7. Launch `drivewipe-tui --live`

### Boot configs
`grub.cfg` and `syslinux.cfg` updated with `drivewipe.live=1` kernel cmdline parameter. Three boot options: normal, verbose, debug shell.

---

## 7. xtask Commands

Add to `crates/xtask/src/main.rs`:
- `BuildLive` — wraps `scripts/build-live.sh`
- `BuildKernelModule` — runs `make -C kernel/drivewipe`
- `BuildPxe` — generates PXE server directory at `target/pxe-server/`

---

## Implementation Phases

### Phase 1: Foundation (drivewipe-live crate + HPA/DCO via SG_IO)
Create crate skeleton, implement `detect.rs`, `hpa.rs`, `dco.rs`, `ata_security.rs` with SG_IO fallback paths. Wire into core's `forensic/hidden.rs` and `drive/linux.rs`. Add error variants and type enhancements. Unit tests for parsing logic.

### Phase 2: TUI Live Mode
Add `live` feature flag, implement live env detection on startup, add 4 new screens, modify main menu for conditional items, add visual indicators.

### Phase 3: Kernel Module
Create `kernel/drivewipe/` with all C source files. Implement char device, ATA passthrough via libata, NVMe passthrough, HPA/DCO commands, DMA buffers. Add `kernel_module.rs` to drivewipe-live. Wire kernel module as primary path with SG_IO fallback.

### Phase 4: Live Image Upgrade
Update init script, packages, boot configs. Add kernel module build stage to `build-live.sh`. Test full boot-to-TUI flow in QEMU.

### Phase 5: PXE Boot
Create dnsmasq config, iPXE scripts, setup script, documentation. Update builder to generate PXE artifacts.

### Phase 6: Build Integration & Polish
Add xtask commands. Implement `unfreeze.rs` and `dma_io.rs`. End-to-end testing. Documentation.

---

## Verification

```bash
# Build the live crate
cargo build --package drivewipe-live

# Build TUI with live features
cargo build --package drivewipe-tui --features live

# Build kernel module (requires linux headers)
make -C kernel/drivewipe

# Build the live USB image
./scripts/build-live.sh

# Test in QEMU
qemu-system-x86_64 -m 2G -drive file=drivewipe-live.img,format=raw \
  -drive file=test-disk.qcow2,if=none,id=d1 -device ahci,id=ahci \
  -device ide-hd,drive=d1,bus=ahci.0

# PXE test in QEMU
qemu-system-x86_64 -m 2G -boot n \
  -device e1000,netdev=net0 \
  -netdev user,id=net0,tftp=target/pxe-server,bootfile=pxelinux.0

# Unit tests
cargo test --package drivewipe-live
```

## Key Reusable Code

- `crates/drivewipe-core/src/wipe/firmware/ata.rs:186-295` — `SgIoHdr` struct and `sg_io_ata16` for SG_IO fallback
- `crates/drivewipe-core/src/types.rs` — `HiddenAreaInfo`, `DriveInfo` data model
- `crates/drivewipe-tui/src/ui/main_menu.rs` — Pattern for menu items and screen dispatch
- `crates/drivewipe-tui/src/app.rs` — `AppScreen` enum pattern for adding new screens
- `crates/drivewipe-core/src/forensic/hidden.rs` — Existing `HiddenAreaResult` struct to populate
