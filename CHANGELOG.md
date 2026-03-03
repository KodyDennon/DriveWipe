# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0] - 2026-03-02

### Added

#### Core Modules
- **Audit Log System** — Structured JSONL event logging for all operations (wipe, clone, partition, forensic, health, config changes) with timestamps, operator info, and device identification
- **Drive Health Monitoring** — SMART attribute parsing (ATA), NVMe health log parsing, drive health snapshots with save/load, pre/post-wipe health comparison with pass/fail verdicts, sequential read/write micro-benchmarks
- **Drive Profile Database** — Manufacturer-specific drive profiles (Samsung EVO/Pro, WD Blue, Seagate Barracuda, Crucial MX, Intel SSD, Kingston, generics for HDD/SSD/NVMe) with model regex matching, SLC cache size hints, over-provisioning ratios, recommended wipe methods, and performance characteristics
- **Drive Cloning** — Block-level sector-by-sector cloning with hash verification, partition-aware cloning with resize-to-fit, compression (flate2/zstd), AES-256 encryption, image format with chunked data and metadata headers
- **Partition Manager** — GPT and MBR partition table parsing with full CRC32 validation, partition CRUD operations (create, delete, resize, move) with overlap detection, bounds checking, and data preservation, filesystem detection via magic bytes (ext4, NTFS, FAT32, exFAT, XFS, Btrfs)
- **Forensic Toolkit** — Per-sector entropy calculation with heatmap data, file signature scanning (JPEG, PDF, DOCX, EXE, ZIP, PNG, MP3, SQLite, etc.), statistical random sector sampling with configurable confidence levels, HPA/DCO hidden area detection, formal forensic report generation with hash chains and chain-of-custody, DFXML export
- **Time Estimation** — EMA-smoothed throughput tracking with configurable alpha, multi-pass awareness with separate write/verify estimates, drive profile integration for SLC cache cliff prediction, confidence intervals (best/expected/worst), per-pass ETA breakdown, calibration period before first estimate, historical performance database with load/save
- **Sleep Prevention** — RAII `SleepGuard` pattern (acquires on creation, releases on drop) for Linux (D-Bus logind Inhibit), macOS (IOPMAssertionCreateWithName), and Windows (SetThreadExecutionState)
- **Desktop Notifications** — Cross-platform notification support for Linux (notify-rust/D-Bus), macOS (osascript), and Windows (toast notifications) with configurable urgency levels
- **Keyboard Lock Mode** — Configurable unlock key sequence with ring buffer matching, lock/unlock event emission to audit log, TUI integration that intercepts all input when locked
- **DriveWipe Secure Method** — 4 specialized variants: HDD (multi-pass patterns + verify), SATA SSD (overwrite + TRIM + overwrite + ATA Secure Erase + verify), NVMe (overwrite + deallocate + NVMe Format/Sanitize + overwrite + verify), USB (multi-pass overwrite + verify)

#### CLI
- `drivewipe health <device>` — Display SMART data, health snapshots, compare before/after
- `drivewipe profile <device>` — Show matched drive profile with recommendations
- `drivewipe clone <source> <target>` — Block or partition-aware cloning with `--compress`, `--encrypt`, `--mode` flags
- `drivewipe partition list|create|delete|resize|move` — Partition management subcommands
- `drivewipe forensic scan|report|compare` — Forensic analysis with JSON report output

#### TUI
- **Main Menu** — Central hub with all feature options (Secure Wipe, Drive Health, Drive Clone, Partition Manager, Forensic Analysis, Settings)
- **Health Screen** — SMART attribute display with color-coded health indicators
- **Clone Setup/Progress** — Source/target drive selection, mode picker, dual throughput display with background clone operations
- **Partition Screen** — Partition table display with partition info, live device reading
- **Forensic Screen** — Background forensic scanning with entropy and signature results
- **Settings Screen** — Toggle auto-reports, notifications, sleep prevention, auto health checks
- Background thread operations for clone, forensic, and partition reading

#### GUI (iced 0.13)
- Full graphical application with 9 screens and navigation
- Drive selection, method selection, confirmation, wipe progress screens
- Health, clone, partition, forensic, and settings screens
- Theme system with color constants for consistent styling (dark backgrounds, status colors, text hierarchy)
- Version display on main menu

#### Build System
- `scripts/build.sh` — Cross-platform build script with `--dev`, `--portable`, `--no-gui`, `--install` flags
- `scripts/build-live.sh` — Docker-based Alpine Linux bootable USB image builder
- `release.sh` (gitignored) — Local release automation: auto-detect platform, build, version bump, tag, push, upload to GitHub Releases via `gh` CLI

#### Bootable Live USB
- Alpine Linux-based minimal live image configuration
- Auto-launch DriveWipe TUI on boot
- UEFI (GRUB) and BIOS (SYSLINUX) boot support
- Pre-installed storage drivers (SATA, NVMe, USB, SCSI)
- udev device enumeration on startup

#### Documentation
- User guides: installation, quickstart, wipe methods, cloning, health monitoring, forensics, configuration reference, troubleshooting
- Developer docs: architecture overview, testing guide

#### Tests
- 267 tests across workspace (up from 130)
- Partition operations tests (create, overlap rejection, delete, resize, move, MBR limit, CRC validation)
- Profile matcher tests, audit logger tests, health snapshot tests
- Keyboard lock tests, time estimator tests
- MockDevice test infrastructure with configurable size and error injection

### Changed
- Wipe method registry now contains 21 methods (9 software + 8 firmware + 4 DriveWipe Secure)
- Progress event system expanded with 12 new variants for health, clone, partition, and forensic operations
- `DriveWipeConfig` expanded with fields for profiles, notifications, sleep prevention, audit logging, and performance history
- Error enum expanded with 16 new variants covering all new modules

### Fixed
- GPT partition table CRC32 validation fully implemented (was TODO)
- Partition CRUD operations fully implemented (were returning stub errors)
- CLI forensic report command generates actual `ForensicReport` with JSON output (was placeholder)
- TUI clone operation spawns background thread with real `clone_block`/`clone_partition_aware` calls (was "not yet implemented")
- TUI forensic operation spawns background `ForensicSession.execute()` (was showing CLI guidance)
- TUI partition screen reads actual partition table from device (was showing CLI guidance)
- All 17 compiler warnings resolved (unused fields, unused theme constants)

### Previous Changes (pre-1.0.0)

#### Fixed
- **Windows**: DoD wipe verification now works correctly on Windows. Fixed buffer alignment issues that caused verification to fail silently when using `FILE_FLAG_NO_BUFFERING`.
  - `PatternVerifier`, `ZeroVerifier`, and random pattern verification now use aligned buffers for reads, matching the alignment requirements of Windows direct I/O
  - Verification warnings and errors are now properly sent as `ProgressEvent` messages so they appear in the TUI log viewer
  - Users will now see detailed error messages (e.g., "Verification mismatch at offset 0x1234") instead of just "FAILED"
- **Windows TUI**: Drive capacity now displays correctly in the drive list. Fixed drive enumeration to open drives with `GENERIC_READ` access instead of zero access, which is required for `IOCTL_DISK_GET_LENGTH_INFO` to succeed.
- **Windows TUI**: Fixed "device disconnected" error when starting wipe. Windows device paths like `\\.\PhysicalDrive0` don't support `.exists()` check, so this validation is now skipped on Windows platforms.
- **Windows TUI**: Added Administrator reminder to confirmation dialog to help users avoid common privilege errors.
- **Windows Debugging**: Added comprehensive logging and debug file output to diagnose device opening failures. Debug log is written to `%TEMP%\drivewipe_debug.log`. The TUI now displays the debug log location in the log viewer. Error messages include specific Windows error codes.
- Test suite: Fixed clippy warnings for redundant imports and bool comparisons
- Cross-module visibility: Made `extract_windows_drive_number` visible to other modules via `pub(crate)`
- Windows I/O: Fixed clippy warning for `std::mem::forget` on `Copy` type by using `let _ = handle` pattern

#### Added (Initial Release)
- Core library (`drivewipe-core`) with full wipe engine
  - 9 software wipe methods: Zero, One, Random (AES-256-CTR), DoD 5220.22-M (3-pass), DoD ECE (7-pass), Gutmann (35-pass), HMG IS5 Baseline, HMG IS5 Enhanced, RCMP TSSIT OPS-II
  - 8 firmware wipe methods: ATA Secure Erase, ATA Enhanced Secure Erase, NVMe Format (User Data & Crypto), NVMe Sanitize (Block, Crypto, Overwrite), TCG Opal Crypto Erase
  - Custom user-defined wipe methods from config.toml
  - AES-256-CTR PRNG with hardware AES-NI acceleration
  - Linux/macOS/Windows raw device I/O with direct I/O and aligned buffers
  - Drive enumeration on all platforms
  - Boot drive detection
  - Pattern-based and zero-optimized read-back verification
  - Session resume from saved state
  - JSON and PDF report generation
  - Progress event system via crossbeam channels
  - Cooperative cancellation via `CancellationToken`
- CLI (`drivewipe`) with subcommands: list, wipe, verify, info, report, queue, resume
- TUI (`drivewipe-tui`) with full interactive interface
- Firmware wipe implementations (ATA Secure Erase, NVMe Format/Sanitize, TCG Opal)
- GitHub Actions CI across Linux/macOS/Windows
- Comprehensive test suite

### Known Limitations
- ATA Secure Erase is not supported on macOS (no reliable ATA passthrough)
- NVMe commands on macOS require `nvme-cli` (`brew install nvme-cli`)
- TCG Opal crypto erase is only supported on Linux (macOS/Windows planned)
- ATA security state detection not yet implemented on any platform
- GUI is functional but uses default iced theme styling (custom theme planned)
