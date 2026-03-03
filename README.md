# DriveWipe

**Cross-platform secure data sanitization and drive management tool** — NIST SP 800-88 Rev. 2 / IEEE 2883:2022 compliant.

DriveWipe provides military/corporate-grade drive wiping with software overwrite for HDDs, firmware-level commands for SSDs, and cryptographic erasure for self-encrypting drives. Beyond sanitization, it includes drive health monitoring, forensic analysis, drive cloning, and partition management — all in a single tool with CLI, TUI, and GUI interfaces.

## Features

### Secure Data Sanitization
- **9 software wipe methods** — Zero/One/Random fill, DoD 5220.22-M (3 & 7 pass), Gutmann (35 pass), HMG IS5 Baseline & Enhanced, RCMP TSSIT OPS-II, plus custom user-defined methods
- **8 firmware wipe methods** — ATA Secure Erase (normal & enhanced), NVMe Format/Sanitize (5 modes), TCG Opal crypto erase
- **4 DriveWipe Secure methods** — Intelligent multi-phase sanitization tailored for HDD, SATA SSD, NVMe, and USB drives
- **Full read-back verification** after wipe
- **Resume capability** — auto-save state every 10 seconds, resume after interruption
- **Multi-drive parallel wipe** with live queue (add drives during active wipe)

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
- **Alignment enforcement** — 4K/1MiB boundary alignment
- **Dry-run/preview mode** — preview changes before applying

### Forensic Analysis
- **Entropy analysis** — per-sector entropy calculation and heatmap generation
- **File signature scanning** — detect JPEG, PDF, DOCX, EXE, ZIP, and more
- **Statistical sampling** — random sector sampling with confidence levels
- **Hidden area detection** — HPA/DCO scanning
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

### Three Interfaces
- **CLI** (`drivewipe`) — full-featured command-line for scripting and automation
- **TUI** (`drivewipe-tui`) — interactive terminal UI with ratatui
- **GUI** (`drivewipe-gui`) — graphical desktop application with iced

## Quick Start

### Prerequisites

- Rust 1.85+ (2024 edition)
- Root/Administrator privileges (required for raw device access)

### Build

```bash
cargo build --release
```

### Install

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

# Launch the GUI
sudo drivewipe-gui
```

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

## Architecture

```
DriveWipe/
  crates/
    drivewipe-core/    # Library — all logic, no UI
    drivewipe-cli/     # CLI binary (drivewipe)
    drivewipe-tui/     # TUI binary (drivewipe-tui)
    drivewipe-gui/     # GUI binary (drivewipe-gui)
    xtask/             # Build automation tasks
  profiles/            # Drive profile TOML files
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

## Session Data

All session data is stored in `~/.local/share/drivewipe/sessions/`:

- `<uuid>.state` — Resume checkpoint (saved every 10 seconds during wipe)
- `<uuid>.log` — Audit log with per-second entries
- `<uuid>.report.json` — Auto-generated JSON report after completion

## Safety

DriveWipe includes multiple safety mechanisms:

1. **Boot drive detection** — Refuses to wipe the drive the OS is running from
2. **Multi-step confirmation** — Shows drive details, requires typing device path, 3-second countdown
3. **SSD software wipe warning** — Recommends firmware erase for SSDs (wear leveling makes software overwrite unreliable)
4. **USB bridge warning** — Firmware commands may fail through USB adapters
5. **ATA frozen warning** — Detects frozen security state, suggests suspend/resume
6. **HPA/DCO detection** — Warns about hidden areas unreachable by software overwrite
7. **Ctrl+C handling** — Graceful interruption with state save for resume
8. **Keyboard lock mode** — Prevents accidental keystrokes during operations
9. **Sleep prevention** — Keeps system awake during long operations
10. **Pre-wipe health check** — Optional automatic SMART check before wiping

## Platform Support

| Feature | Linux | macOS | Windows |
|---|---|---|---|
| Drive enumeration | Full (sysfs) | Full (diskutil) | Full (DeviceIoControl) |
| Raw device I/O | Full (O_DIRECT) | Full (F_NOCACHE) | Full (FILE_FLAG_NO_BUFFERING) |
| Boot drive detection | Full (/proc/mounts) | Full (/sbin/mount) | Full (IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS) |
| Software wipe methods | Full | Full | Full |
| ATA Secure Erase | Full (SG_IO + ATA_16 CDB) | Not supported | Full (IOCTL_ATA_PASS_THROUGH) |
| NVMe Format/Sanitize | Full (NVME_IOCTL_ADMIN_CMD) | Via nvme-cli | Full (IOCTL_STORAGE_PROTOCOL_COMMAND) |
| TCG Opal crypto erase | Full (sed-opal ioctls) | Not supported | Not yet (future) |
| SMART health | Full (ATA/NVMe ioctl) | Full (IOKit) | Full (IOCTL_STORAGE_QUERY_PROPERTY) |
| Sleep prevention | Full (D-Bus logind) | Full (IOPMAssertion) | Full (SetThreadExecutionState) |
| Desktop notifications | Full (D-Bus freedesktop) | Full (osascript) | Full (toast) |

### Linux

Best platform support. All software and firmware wipe methods fully operational. ATA Secure Erase uses SCSI ATA_16 CDB via the SG_IO ioctl. NVMe commands use the kernel's admin command ioctl. TCG Opal uses the kernel's `sed-opal` driver. Sleep prevention via D-Bus logind inhibit. Requires root privileges.

### macOS

Software wipe methods fully operational. ATA Secure Erase is not supported (macOS lacks a reliable ATA passthrough). NVMe commands require `nvme-cli` (install with `brew install nvme-cli`). TCG Opal is not supported (no kernel SED driver). Sleep prevention via IOPMAssertionCreateWithName. Requires root privileges.

**Gatekeeper:** If macOS shows *"drivewipe-gui can't be opened because Apple cannot check it for malicious software"*, remove the quarantine attribute:

```bash
xattr -dr com.apple.quarantine drivewipe-gui
```

### Windows

Full support for software wipe methods, drive enumeration, and device I/O using `CreateFileW` with `FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH`. ATA Secure Erase uses `IOCTL_ATA_PASS_THROUGH` with `ATA_PASS_THROUGH_EX`. NVMe commands use `IOCTL_STORAGE_PROTOCOL_COMMAND`. Sleep prevention via SetThreadExecutionState. Requires Administrator privileges.

## Testing

```bash
# Run all tests
cargo test --workspace

# Run with verbose output
cargo test --workspace -- --nocapture

# Run specific test suite
cargo test -p drivewipe-core --test integration_tests
```

## License

Free for personal, hobby, and commercial use. Government, intelligence, forensics, and security organizations are expected to support or contribute to development. See [LICENSE.md](LICENSE.md) for full terms.

## Contributing

Contributions welcome! See [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) for guidelines.
