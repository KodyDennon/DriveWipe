# DriveWipe

**Cross-platform secure data sanitization and drive management tool** — NIST SP 800-88 Rev. 2 / IEEE 2883:2022 compliant.

DriveWipe provides military/corporate-grade drive wiping with software overwrite for HDDs, firmware-level commands for SSDs, and cryptographic erasure for self-encrypting drives. Beyond sanitization, it includes drive health monitoring, forensic analysis, drive cloning, partition management, and a bootable live environment for data sanitization labs — all in a single tool with CLI, TUI, and GUI interfaces.

---

## Why DriveWipe?

- **21 wipe methods** — software, firmware, and hybrid — covering every drive type
- **Live environment** — boot from USB or PXE network to wipe any drive, including the boot drive
- **Direct hardware access** — custom kernel module for ATA/NVMe passthrough, HPA/DCO manipulation, and DMA I/O
- **Full forensic toolkit** — entropy analysis, signature scanning, and formal chain-of-custody reports
- **Cross-platform** — native Linux, macOS, and Windows support with platform-specific optimizations
- **Three interfaces** — CLI for automation, TUI for interactive use, GUI for simplicity

---

## Features

### Secure Data Sanitization
- **9 software wipe methods** — Zero/One/Random fill, DoD 5220.22-M (3 & 7 pass), Gutmann (35 pass), HMG IS5 Baseline & Enhanced, RCMP TSSIT OPS-II, plus custom user-defined methods
- **8 firmware wipe methods** — ATA Secure Erase (normal & enhanced), NVMe Format/Sanitize (5 modes), TCG Opal crypto erase
- **4 DriveWipe Secure methods** — Intelligent multi-phase sanitization tailored for HDD, SATA SSD, NVMe, and USB drives
- **Full read-back verification** after wipe
- **Resume capability** — auto-save state every 10 seconds, resume after interruption
- **Multi-drive parallel wipe** with live queue (add drives during active wipe)

### Live Environment
- **Bootable USB/ISO** — Alpine Linux-based live image (~200 MB) boots directly into DriveWipe TUI
- **PXE network boot** — wipe entire racks without USB drives; iPXE menu with Normal, Safe Mode, and Serial Console options
- **Custom kernel module** — direct ATA/NVMe command passthrough bypassing SCSI translation
- **HPA/DCO detection & removal** — find and remove hidden areas that software overwrites can't reach
- **Drive unfreezing** — automatic suspend/resume cycle to unfreeze ATA security-frozen drives
- **DMA I/O** — zero-copy DMA-coherent buffer I/O for maximum throughput

### Drive Health Monitoring
- **SMART data parsing** — ATA and NVMe health attributes
- **Pre/post wipe health snapshots** — automatic comparison with pass/fail verdict
- **Sequential read/write benchmarks** — performance baseline measurements
- **Temperature monitoring** — real-time temperature tracking during operations

### Drive Cloning
- **Block-level cloning** — sector-by-sector copy between drives
- **Partition-aware cloning** — resize partitions to fit target drive
- **Image I/O** — create and restore compressed, encrypted drive images
- **Compression** — flate2 and zstd support for image files
- **Encryption** — AES-256 encryption for image files

### Partition Management
- **GPT and MBR support** — full parsing and writing with CRC validation
- **Create, delete, resize, move** partitions with data preservation
- **Filesystem detection** — NTFS, ext4, FAT32, exFAT, XFS, Btrfs
- **Alignment enforcement** — 4K/1 MiB boundary alignment
- **Dry-run/preview mode** — preview changes before applying

### Forensic Analysis
- **Entropy analysis** — per-sector entropy calculation and heatmap generation
- **File signature scanning** — detect JPEG, PDF, DOCX, EXE, ZIP, and more
- **Statistical sampling** — random sector sampling with confidence levels
- **Hidden area detection** — HPA/DCO scanning (enhanced in live mode with real ATA probing)
- **Formal forensic reports** — timestamps, hash chains, chain-of-custody
- **Export formats** — DFXML, NSRL-compatible hash sets

### Drive Profiles
- **Manufacturer-specific profiles** — Samsung, WD, Seagate, Crucial, Intel, Kingston, and more
- **Automatic drive matching** — regex-based model detection
- **Optimized recommendations** — suggested wipe method per drive type
- **SLC cache and quirk awareness** — profile-driven performance tuning

### Infrastructure
- **Audit logging** — comprehensive JSONL audit trail for all operations
- **Report generation** — JSON and PDF certificate output
- **Desktop notifications** — cross-platform alerts on operation completion
- **Sleep prevention** — RAII-based system sleep inhibition during operations
- **Keyboard lock** — configurable unlock sequence prevents accidental input
- **Intelligent time estimates** — EMA-smoothed throughput with confidence intervals
- **Automated versioning** — git-commit-driven version bumps with LOC safety triggers

### Three Interfaces
- **CLI** (`drivewipe`) — full-featured command-line for scripting and automation
- **TUI** (`drivewipe-tui`) — interactive terminal UI with ratatui
- **GUI** (`drivewipe-gui`) — graphical desktop application with iced

---

## Quick Start

### Prerequisites

- Rust 1.85+ (2024 edition)
- Root/Administrator privileges (required for raw device access)

### Build

```bash
# Build all desktop binaries
cargo build --release

# Build TUI with live environment features (Linux only)
cargo build --release --package drivewipe-tui --features live

# Build the live USB image (requires Docker)
cargo xtask live-build
```

### Install

Download the pre-built binaries for your platform from the [Releases](https://github.com/KodyDennon/DriveWipe/releases) page. Archives are provided for **Linux**, **macOS**, and **Windows** (x86_64 and ARM64).

*Note: DriveWipe requires **Administrative / Root** privileges to access raw disks.*

To build from source:

```bash
cargo install --path crates/drivewipe-cli
cargo install --path crates/drivewipe-tui
cargo install --path crates/drivewipe-gui
```

### Usage

```bash
# List detected drives
sudo drivewipe list

# Show detailed drive info
sudo drivewipe info --device /dev/sda

# Wipe a drive (interactive confirmation)
sudo drivewipe wipe --device /dev/sda --method dod-short

# Wipe with verification
sudo drivewipe wipe --device /dev/sda --method dod-short --verify true

# Force mode (scripted use)
sudo drivewipe wipe --device /dev/sda --method zero --force --yes-i-know-what-im-doing

# Verify a wiped drive
sudo drivewipe verify --device /dev/sda --pattern zero

# Generate PDF report from JSON
drivewipe report --input session.report.json --format pdf --output certificate.pdf

# Resume interrupted sessions
sudo drivewipe resume --list
sudo drivewipe resume --auto

# Queue multiple drives
sudo drivewipe queue add --device /dev/sda --method dod-short
sudo drivewipe queue add --device /dev/sdb --method zero
sudo drivewipe queue start --parallel 2

# Drive health check
sudo drivewipe health /dev/sda

# Drive profile lookup
sudo drivewipe profile /dev/sda

# Clone a drive
sudo drivewipe clone /dev/sda /dev/sdb --mode block

# Partition management
sudo drivewipe partition list /dev/sda

# Forensic analysis
sudo drivewipe forensic scan /dev/sda

# Launch the TUI
sudo drivewipe-tui

# Launch the TUI in live mode (auto-detected in live environment)
sudo drivewipe-tui --live

# Launch the GUI
sudo drivewipe-gui
```

---

## Wipe Methods

| ID | Name | Passes | Type |
|---|---|---|---|
| `zero` | Zero Fill | 1 | Software |
| `one` | One Fill | 1 | Software |
| `random` | Random Fill (AES-256-CTR) | 1 | Software |
| `dod-short` | DoD 5220.22-M | 3 | Software |
| `dod-ece` | DoD 5220.22-M ECE | 7 | Software |
| `gutmann` | Gutmann | 35 | Software |
| `hmg-baseline` | HMG IS5 Baseline | 1 | Software |
| `hmg-enhanced` | HMG IS5 Enhanced | 3 | Software |
| `rcmp` | RCMP TSSIT OPS-II | 7 | Software |
| `ata-erase` | ATA Secure Erase | firmware | Firmware |
| `ata-erase-enhanced` | ATA Enhanced Secure Erase | firmware | Firmware |
| `nvme-format-user` | NVMe Format (User Data Erase) | firmware | Firmware |
| `nvme-format-crypto` | NVMe Format (Cryptographic Erase) | firmware | Firmware |
| `nvme-sanitize-block` | NVMe Sanitize (Block Erase) | firmware | Firmware |
| `nvme-sanitize-crypto` | NVMe Sanitize (Cryptographic Erase) | firmware | Firmware |
| `nvme-sanitize-overwrite` | NVMe Sanitize (Overwrite) | firmware | Firmware |
| `tcg-opal` | TCG Opal Crypto Erase | firmware | Firmware |
| `drivewipe-secure-hdd` | DriveWipe Secure (HDD) | 4 | Hybrid |
| `drivewipe-secure-sata-ssd` | DriveWipe Secure (SATA SSD) | 4 | Hybrid |
| `drivewipe-secure-nvme` | DriveWipe Secure (NVMe) | 4 | Hybrid |
| `drivewipe-secure-usb` | DriveWipe Secure (USB) | 4 | Hybrid |

### DriveWipe Secure Methods

The DriveWipe Secure methods combine software overwrite with firmware commands for maximum assurance:

- **HDD**: Multi-pass pattern writes followed by full verification
- **SATA SSD**: Overwrite + TRIM + overwrite + ATA Secure Erase (if available) + verify
- **NVMe**: Overwrite + deallocate + NVMe Format/Sanitize (if available) + overwrite + verify
- **USB**: Multi-pass overwrite + verify (limited by USB controller capabilities)

---

## Architecture

```
DriveWipe/
  crates/
    drivewipe-core/    # Library — all logic, no UI
    drivewipe-cli/     # CLI binary (drivewipe)
    drivewipe-tui/     # TUI binary (drivewipe-tui)
    drivewipe-gui/     # GUI binary (drivewipe-gui)
    drivewipe-live/    # Live environment — HPA/DCO, ATA security, DMA I/O
    xtask/             # Build automation (bump, release, live-build)
  kernel/drivewipe/    # Custom Linux kernel module (/dev/drivewipe)
  live/
    alpine-config/     # Live USB boot configuration and init scripts
    pxe/               # PXE network boot infrastructure
  profiles/            # Drive profile TOML files
  scripts/             # Build scripts (DRIVEWIPE_LIVE_VERSION=1.1.2 ./scripts/build-live.sh)
  docs/                # Documentation
```

### Core Modules

| Module | Purpose |
|---|---|
| `audit` | JSONL audit logging with date-based rotation |
| `clone` | Block-level and partition-aware drive cloning |
| `config` | TOML configuration loading and management |
| `crypto` | AES-256-CTR pattern generation |
| `drive` | Drive enumeration and info gathering |
| `forensic` | Entropy analysis, signature scanning, forensic reports |
| `health` | SMART/NVMe health monitoring and benchmarks |
| `io` | Platform-specific raw device I/O (`RawDeviceIo` trait) |
| `keyboard_lock` | Input lock with configurable unlock sequence |
| `notify` | Cross-platform desktop notifications |
| `partition` | GPT/MBR parsing, partition CRUD operations |
| `pattern` | Wipe pattern generation |
| `profile` | Drive manufacturer profiles and matching |
| `progress` | `ProgressEvent` channel-based progress system |
| `report` | JSON and PDF report generation |
| `resume` | Crash-safe session state persistence |
| `session` | `WipeSession` orchestrator |
| `sleep_inhibit` | RAII system sleep prevention |
| `time_estimate` | EMA-smoothed time estimation with confidence intervals |
| `verify` | Read-back verification engine |
| `wipe` | Wipe method registry and implementations |

### Live Environment Crate (`drivewipe-live`)

| Module | Purpose |
|---|---|
| `capabilities` | Probe and report live environment capabilities |
| `detect` | Live environment detection (cmdline, markers, module) |
| `kernel_module` | `/dev/drivewipe` ioctl wrapper with typed methods |
| `hpa` | HPA detection & removal (kernel module + SG_IO fallback) |
| `dco` | DCO detection, restore & freeze (kernel module + SG_IO) |
| `ata_security` | ATA security state querying from IDENTIFY DEVICE |
| `unfreeze` | Suspend/resume cycle to unfreeze drives |
| `dma_io` | Zero-copy DMA I/O via kernel module |

### Kernel Module (`kernel/drivewipe/`)

| ioctl | Purpose |
|---|---|
| `DW_IOC_ATA_CMD` | Raw ATA command passthrough (bypasses SCSI) |
| `DW_IOC_NVME_CMD` | Raw NVMe admin command passthrough |
| `DW_IOC_HPA_DETECT` / `REMOVE` | READ NATIVE MAX ADDRESS / SET MAX ADDRESS |
| `DW_IOC_DCO_DETECT` / `RESTORE` / `FREEZE` | Device Configuration Overlay commands |
| `DW_IOC_DMA_IO` | Zero-copy DMA read/write |
| `DW_IOC_ATA_SEC_STATE` | Query ATA security state |
| `DW_IOC_MODULE_INFO` | Module version + capabilities bitmask |

---

## Live Environment

### USB Boot

```bash
# Build the live image
cargo xtask live-build

# Write to USB (replace v1.1.8 and /dev/sdX with your version and device)
sudo dd if=drivewipe-live-v1.1.8.iso of=/dev/sdX bs=4M status=progress
```

### PXE Network Boot

DriveWipe Live can be network-booted for wiping entire racks. The PXE artifact (`drivewipe-live-v1.1.8-pxe.tar.gz`) contains everything needed to seed a TFTP/HTTP server.

```bash
# Extract PXE artifacts from a built image
tar -xzvf DriveWipe-v1.1.8-Live-PXE.tar.gz -C /var/lib/tftpboot/

# Configure dnsmasq with the included config
sudo cp /var/lib/tftpboot/dnsmasq.conf /etc/dnsmasq.d/drivewipe.conf
sudo systemctl restart dnsmasq
```

See [`live/pxe/README.md`](live/pxe/README.md) for full PXE setup instructions and QEMU testing.

### Live TUI Features

When running in the live environment, the TUI adds:

- **Live Dashboard** — system overview with kernel module status, CPU/RAM, drive summary
- **HPA/DCO Manager** — detect, remove HPA, restore DCO, freeze DCO with confirmation for destructive actions
- **ATA Security Manager** — view frozen/locked/enabled state, one-click unfreeze via suspend/resume
- **Kernel Module Status** — module version, capabilities bitmask, feature availability

---

## Configuration

Default config location: `~/.config/drivewipe/config.toml`

```toml
default_method = "dod-short"
parallel_drives = 2
auto_verify = true
auto_report_json = true
log_level = "info"
operator_name = "John Doe"
state_save_interval_secs = 10
notifications_enabled = true
sleep_prevention_enabled = true
auto_health_pre_wipe = true
keyboard_lock_sequence = "unlock"
profiles_dir = "~/.config/drivewipe/profiles"
audit_dir = "~/.local/share/drivewipe/audit"
performance_history_dir = "~/.local/share/drivewipe/performance"

[[custom_methods]]
id = "my-method"
name = "My Custom Method"
description = "Custom 2-pass wipe"
verify_after = true

[[custom_methods.passes]]
pattern_type = "random"

[[custom_methods.passes]]
pattern_type = "zero"
```

---

## Safety

DriveWipe includes multiple safety mechanisms:

1.  **Boot drive detection** — refuses to wipe the drive the OS is running from
2.  **Multi-step confirmation** — shows drive details, requires typing device path, 3-second countdown
3.  **SSD software wipe warning** — recommends firmware erase for SSDs (wear leveling makes software overwrite unreliable)
4.  **USB bridge warning** — firmware commands may fail through USB adapters
5.  **ATA frozen warning** — detects frozen security state, suggests suspend/resume (automatic in live mode)
6.  **HPA/DCO detection** — warns about hidden areas unreachable by software overwrite; live mode can remove them
7.  **Ctrl+C handling** — graceful interruption with state save for resume
8.  **Keyboard lock mode** — prevents accidental keystrokes during operations
9.  **Sleep prevention** — keeps system awake during long operations
10. **Pre-wipe health check** — optional automatic SMART check before wiping

---

## Platform Support

| Feature | Linux | macOS | Windows |
|---|---|---|---|
| Drive enumeration | Full (sysfs) | Full (diskutil) | Full (DeviceIoControl) |
| Raw device I/O | Full (O_DIRECT) | Full (F_NOCACHE) | Full (FILE_FLAG_NO_BUFFERING) |
| Boot drive detection | Full (/proc/mounts) | Full (/sbin/mount) | Full (IOCTL_VOLUME_GET_...) |
| Software wipe methods | Full | Full | Full |
| ATA Secure Erase | Full (SG_IO) | Not supported | Full (IOCTL_ATA_PASS_THROUGH) |
| NVMe Format/Sanitize | Full (nvme ioctl) | Via nvme-cli | Full (IOCTL_STORAGE_...) |
| TCG Opal crypto erase | Full (sed-opal) | Not supported | Not yet |
| SMART health | Full | Full (IOKit) | Full |
| Sleep prevention | Full (D-Bus) | Full (IOPMAssertion) | Full (SetThreadExecutionState) |
| Desktop notifications | Full (D-Bus) | Full (osascript) | Full (toast) |
| **Live environment** | **Full** | N/A | N/A |
| **Kernel module** | **Full** | N/A | N/A |
| **PXE boot** | **Full** | N/A | N/A |

---

## Development

### Automated Versioning

DriveWipe uses a **"Safety First"** automated versioning system. See [DEVELOPMENT.md](DEVELOPMENT.md) for details.

All six crates (`drivewipe-core`, `drivewipe-cli`, `drivewipe-tui`, `drivewipe-gui`, `drivewipe-live`, `xtask`) are versioned independently based on scoped commit messages and LOC thresholds.

### xtask Commands

```bash
cargo xtask bump          # Automated version bumps based on git history
cargo xtask release       # Interactive release wizard
cargo xtask live-build    # Build the live USB image
DRIVEWIPE_LIVE_VERSION=1.1.2 ./scripts/build-live.sh
```

### Testing

```bash
# Run all tests
cargo test --workspace

# Run with verbose output
cargo test --workspace -- --nocapture

# Run live crate tests
cargo test -p drivewipe-live

# Run specific test suite
cargo test -p drivewipe-core --test integration_tests
```

---

## License

Free for personal, hobby, and commercial use. Government, intelligence, forensics, and security organizations are expected to support or contribute to development. See [LICENSE.md](LICENSE.md) for full terms.

## Contributing

Contributions welcome! See [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) for guidelines.
