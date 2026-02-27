# DriveWipe

**Cross-platform secure data sanitization tool** — NIST SP 800-88 Rev. 2 / IEEE 2883:2022 compliant.

DriveWipe provides military/corporate-grade drive wiping with software overwrite for HDDs, firmware-level commands for SSDs, and cryptographic erasure for self-encrypting drives. It supports parallel multi-drive operations, full read-back verification, audit logging, and certificate generation.

## Features

- **17 wipe methods** — Zero/One/Random fill, DoD 5220.22-M (3 & 7 pass), Gutmann (35 pass), HMG IS5, RCMP TSSIT OPS-II, ATA Secure Erase, NVMe Format/Sanitize, TCG Opal crypto erase, and custom user-defined methods
- **Three interfaces** — CLI for scripting, TUI for interactive use, GUI (Phase 2)
- **Cross-platform** — macOS, Linux, Windows
- **Multi-drive parallel wipe** with live queue (add drives during active wipe)
- **Full read-back verification** after wipe
- **Resume capability** — auto-save state every 10 seconds, resume after interruption
- **Audit logging** — per-second throughput logs, survives crashes
- **Report generation** — auto JSON after every wipe, PDF certificates on request
- **Safety first** — boot drive refusal, multi-step confirmation, SSD/USB warnings, frozen ATA detection

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

# Launch the TUI
sudo drivewipe-tui
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

## Architecture

```
DriveWipe/
  crates/
    drivewipe-core/    # Library — all logic, no UI
    drivewipe-cli/     # CLI binary (drivewipe)
    drivewipe-tui/     # TUI binary (drivewipe-tui)
    drivewipe-gui/     # GUI binary (Phase 2)
```

The core library (`drivewipe-core`) contains all business logic: drive enumeration, I/O, wipe methods, verification, progress tracking, resume state, and report generation. The CLI and TUI are thin wrappers consuming the core API.

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

## Platform Notes

- **Linux** — Best platform support. Full ioctl access for ATA, NVMe, and TCG Opal.
- **macOS** — ATA passthrough is limited. NVMe uses private IONVMeFamily API. Recommend Linux for firmware operations.
- **Windows** — Requires Administrator. ATA via `IOCTL_ATA_PASS_THROUGH`, NVMe via `IOCTL_STORAGE_PROTOCOL_COMMAND`.

## License

Free for personal, hobby, and commercial use. Government, intelligence, forensics, and security organizations are expected to support or contribute to development. See [LICENSE.md](LICENSE.md) for full terms.

## Contributing

Contributions welcome! See [CONTRIBUTING.md](docs/CONTRIBUTING.md) for guidelines.
