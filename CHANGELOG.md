# Changelog

All notable changes to DriveWipe will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.3.0] - 2026-03-13

### Added
- **Clone Image Encryption** — AES-256-CTR stream encryption for `.dwc` image files with SHA-256 iterated key derivation, per-chunk incrementing nonces, and password-based encrypt/decrypt workflow.
- **Partition-Aware Cloning** — Intelligent clone mode that parses GPT/MBR tables, copies partition table headers and each partition individually, skips unallocated space, and warns when partitions exceed target capacity.
- **Bandwidth Throttling** — Configurable rate limiting (`bandwidth_limit_bps`) for block, image, and partition-aware clone operations to prevent I/O saturation.
- **Forensic Hidden Area Detection** — Partition gap analysis that detects unallocated regions between partitions, scans for data remnants in gaps, identifies hidden/diagnostic MBR partitions, and reports HPA/DCO status.
- **GUI Forensic Execution** — Fully wired forensic scanning in the iced GUI with entropy stats, signature hits, sampling results, and hidden area findings displayed in real time.
- **GUI Clone Execution** — End-to-end clone workflow in the GUI with drive selection, start button, live progress bar, throughput display, and completion status.
- **TUI Partition CRUD** — Interactive partition management in the TUI: `d` to delete partitions, `n` to create partitions in the largest unallocated gap with 1 MiB alignment.
- **Enriched DFXML Export** — Forensic exports now include hidden area analysis, entropy statistics, and statistical sampling data alongside signature hits.

### Changed
- **Dependencies** — Updated console 0.16, iced 0.14, toml 1.0, quick-xml 0.38, nix 0.31, rand 0.10, toml_edit 0.23, upload-artifact v7, download-artifact v8.
- **Forensic Reports** — Conclusions now include hidden partition findings, unallocated gap data remnants, and HPA/DCO detection status.
- **Cross-Platform Tooling** — Added `scripts/cross-check.sh` for local Linux/Windows cross-compilation validation using `cargo-zigbuild`.

### Fixed
- **Clippy Compliance** — Resolved all clippy lints including `io_other_error`, `manual_div_ceil`, `missing_transmute_annotations`, and `redundant_closure_call`.
- **iced 0.14 Migration** — Fixed checkbox API, Pixels type (u16→f32), application builder, and stream channel typing for iced 0.14 compatibility.
- **rand 0.10 Migration** — Updated trait imports from `Rng::random` to `RngExt::random` and `RngExt::random_range`.

## [1.2.0] - 2026-03-11

### Added
- **Partition Persistence** — Native GPT/MBR partition table writing with automatic CRC32 recalculation and protective MBR generation.
- **Image-Based Cloning** — New cloning engine supporting backup to and restore from `.dwc` compressed image files using Zstd or Gzip.
- **GUI Engine Integration** — Fully wired the `iced` graphical interface to the core library, enabling real-time wipe progress, async health checks, and partition visualization.
- **Partition CLI/TUI** — Added `create`, `delete`, and `resize` subcommands to the CLI and interactive triggers to the TUI Partition Manager.
- **Memory Safety Core** — Introduced `DeviceWrapper` to safely handle fat-pointer trait objects across async/thread boundaries, resolving critical `E0606` casting errors.

### Changed
- **Async Test Suite** — Migrated the entire test suite (112+ tests) to `tokio::test` for full async compatibility.
- **Dependency Refactor** — Resolved cyclic dependencies between `core` and `live` crates by decoupling forensic orchestration.

### Fixed
- **Build Quality** — Resolved all compiler warnings and lint issues across the entire workspace.
- **CLI/TUI Stability** — Fixed borrow-after-move errors in partition management commands.

## [1.1.13] - 2026-03-03

### Fixed
- **Code Style** — Applied `cargo fmt` to resolve CI formatting failures.
- **Maintenance** — Re-synchronized workspace versions after v1.1.12 CI failure.

## [1.1.12] - 2026-03-03

### Fixed
- **Maintenance Update** — Synchronized workspace versions and updated changelog for consistency across all crates.

## [1.1.11] - 2026-03-02

### Fixed
- **Live Environment** — Removed missing `sdparm` package in Alpine 3.21 to resolve build failures.

## [1.1.10] - 2026-03-02

### Fixed
- **Build Tooling** — Ensured build directory exists in `build-live.sh` before writing artifacts.

## [1.1.9] - 2026-03-02

### Fixed
- **CI Stability** — Applied exhaustive ioctl type casts to resolve platform-specific compilation errors on Linux runners.

## [1.1.8] - 2026-03-02

### Fixed
- **Cross-Compilation** — Fixed `linux-musl` target builds by resolving dependency conflicts in `drivewipe-core`.

## [1.1.7] - 2026-03-02

### Fixed
- **Style Consistency** — Applied `cargo fmt` canonical import ordering across the entire workspace.

## [1.1.6] - 2026-03-02

### Fixed
- **Windows Safety** — Removed unused `mut` from `privilege.rs` `TOKEN_PRIVILEGES` variables to satisfy strict clippy lints.

## [1.1.5] - 2026-03-02

### Fixed
- **Production Windows I/O** — Removed debug logging from production builds and resolved all remaining clippy errors for Windows targets.

## [1.1.4] - 2026-03-02

### Fixed
- **Feature Gating** — Correctly gated `LIVE_MENU_ITEMS` and fixed release CI `--all-features` issues.

## [1.1.3] - 2026-03-02

### Fixed
- **Comprehensive Cross-Platform Guards** — Applied `#[cfg(all(feature = "live", target_os = "linux"))]` to all live feature usages in `app.rs`, `ui/mod.rs`, and `ui/main_menu.rs`, permanently preventing compile failures on macOS and Windows.
- **CI Fix** — Removed `--all-features` from `ci.yml` checks and clippy so the `live` feature is never activated on non-Linux CI runners.

## [1.1.2] - 2026-03-02

### Fixed
- **Universal Compilation** — Decoupled the `live` feature from standard macOS/Windows builds, fixing the `drivewipe-live` dependency resolution error.
- **CI Modernization** — Optimized GitHub Actions to use platform-aware feature flagging instead of a blanket `--all-features` check.

## [1.1.1] - 2026-03-02

### Fixed
- **Windows/macOS Compilation** — Gated Unix-specific `drivewipe-live` modules to fix workspace compilation on non-Linux platforms.
- **CI Dependencies** — Added missing system libraries to GitHub Actions for GUI builds.
- **Artifact Synchronization** — Fixed path mismatches in release automation for ISO and PXE assets.

## [1.1.0] - 2026-03-02

DriveWipe 1.1.0 is a major release that transforms the project from a wipe-only tool into a comprehensive drive management, forensics, and sanitization platform. This release adds 10 new core modules, expands the CLI with 5 new subcommands, adds 7 new TUI screens, delivers a full graphical interface, and includes build tooling for bootable live USB images.

### Added

- **Audit Log System** — Structured JSONL event logging for all operations with timestamps, operator identification, and device tracking. Covers wipe, clone, partition, forensic, health, and configuration events with configurable severity levels.
- **Drive Health Monitoring** — Full SMART attribute parsing for ATA drives and NVMe health log parsing. Includes drive health snapshots with save/load support, pre/post-wipe health comparison with automated pass/fail verdicts, and sequential read/write micro-benchmarks.
- **Drive Profile Database** — Manufacturer-specific profiles for Samsung EVO/Pro, WD Blue, Seagate Barracuda, Crucial MX, Intel SSD, Kingston, plus generic profiles for HDD, SSD, and NVMe. Profiles include model regex matching, SLC cache size hints, over-provisioning ratios, recommended wipe methods, and performance characteristics loaded from TOML files.
- **Drive Cloning** — Block-level sector-by-sector cloning with hash verification, partition-aware cloning with automatic resize-to-fit, optional compression (flate2/zstd), and AES-256 encryption. Image format uses chunked data with metadata headers for resume support.
- **Partition Manager** — GPT and MBR partition table parsing with full CRC32 validation. Supports create, delete, resize, and move operations with overlap detection, bounds checking, and data preservation. Includes filesystem detection via magic bytes for ext4, NTFS, FAT32, exFAT, XFS, and Btrfs.
- **Forensic Toolkit** — Per-sector entropy calculation with heatmap data generation, file signature scanning for common formats (JPEG, PDF, DOCX, EXE, ZIP, PNG, MP3, SQLite, etc.), statistical random sector sampling with configurable confidence levels, and HPA/DCO hidden area detection. Generates formal forensic reports with hash chains and chain-of-custody metadata, plus DFXML export.
- **Time Estimation Engine** — EMA-smoothed throughput tracking, multi-pass awareness with separate write/verify estimates, drive profile integration for SLC cache cliff prediction, confidence intervals (best/expected/worst), per-pass ETA breakdown, calibration period, and historical performance database with per-device load/save.
- **Sleep Prevention** — RAII `SleepGuard` that prevents system sleep during long-running operations. Supports Linux (D-Bus logind Inhibit), macOS (IOPMAssertionCreateWithName), and Windows (SetThreadExecutionState).
- **Desktop Notifications** — Cross-platform notification support via notify-rust for Linux (D-Bus freedesktop notifications), macOS (osascript), and Windows (toast notifications). Fires on operation completion with configurable urgency levels.
- **Keyboard Lock Mode** — Prevents accidental interruption during critical operations. Uses a configurable unlock key sequence with ring buffer matching. Emits lock/unlock events to the audit log.
- **DriveWipe Secure Wipe Method** — 4 specialized variants optimized per storage type:
  - **HDD**: Multi-pass pattern writes with verification
  - **SATA SSD**: Overwrite + TRIM + overwrite + ATA Secure Erase (if supported) + verify
  - **NVMe**: Overwrite + deallocate + NVMe Format/Sanitize (if supported) + overwrite + verify
  - **USB**: Multi-pass overwrite + verify (limited by USB controller throughput)
- **CLI Subcommands**:
  - `drivewipe health <device>` — Display SMART data, save/load health snapshots, compare before/after
  - `drivewipe profile <device>` — Show matched drive profile with recommendations
  - `drivewipe clone <source> <target>` — Block or partition-aware cloning with `--compress`, `--encrypt`, `--mode` flags
  - `drivewipe partition list|create|delete|resize|move` — Full partition management
  - `drivewipe forensic scan|report|compare` — Forensic analysis with JSON report output
- **TUI Screens**:
  - Main Menu — Central navigation hub for all features
  - Drive Health — SMART attribute table with color-coded health indicators
  - Clone Setup & Progress — Source/target selection, mode picker, real-time throughput display with background clone operations
  - Partition Manager — Live partition table reading and display
  - Forensic Analysis — Background scanning with entropy and signature results
  - Settings — Toggle auto-reports, notifications, sleep prevention, and auto health checks
- **GUI Application** (iced 0.13) — Full graphical interface with 9 screens (drive select, method select, confirmation, wipe progress, health, clone, partition, forensic, settings), themed with consistent color system, version display, and responsive layouts.
- **Build System**:
  - `scripts/build.sh` — Cross-platform build script with `--dev`, `--portable`, `--no-gui`, and `--install` flags
  - `scripts/build-live.sh` — Docker-based Alpine Linux bootable USB image builder with UEFI (GRUB) and BIOS (SYSLINUX) boot support
  - `release.sh` (gitignored, local-only) — Automated release script that detects platform, builds release binaries, bumps version, creates git tag, and uploads to GitHub Releases. Supports `--attach <tag>` to add platform-specific builds to an existing release without creating a new one.
- **Bootable Live USB** — Alpine Linux-based minimal live image with auto-launch TUI, pre-installed storage drivers (SATA, NVMe, USB, SCSI), udev device enumeration, and target image size under 256MB.
- **Documentation** — User guides for installation, quickstart, wipe methods, cloning, health monitoring, forensics, configuration reference, and troubleshooting. Developer docs for architecture and testing.
- **Tests** — 267 tests across workspace including partition CRUD operations, GPT CRC32 validation, profile matching, audit logging, health snapshots, keyboard lock, time estimation, and MockDevice test infrastructure.

### Changed

- Wipe method registry expanded from 17 to 21 methods (9 software + 8 firmware + 4 DriveWipe Secure).
- Progress event system expanded with 12 new variants for health, clone, partition, and forensic operations.
- `DriveWipeConfig` expanded with configuration for profiles directory, notification preferences, sleep prevention, keyboard lock sequence, auto health checks, audit directory, and performance history.
- Error types expanded with 16 new variants covering all new modules.

### Fixed

- GPT partition table CRC32 validation now fully implemented (was previously a TODO stub).
- All compiler warnings resolved across the entire workspace (0 warnings).

### Known Limitations

- ATA Secure Erase is not supported on macOS (no reliable ATA passthrough).
- NVMe commands on macOS require `nvme-cli` (`brew install nvme-cli`).
- TCG Opal crypto erase is only supported on Linux.
- GUI uses default iced theme styling; custom dark theme is planned.
- Live USB image builder requires Docker and root privileges.

---

## [0.1.5] - 2026-02-28

### Added

- Comprehensive implementation plan and archived completed development phases.

### Fixed

- Stabilized throughput display with EMA smoothing and longer measurement windows to prevent erratic readings.

### Changed

- Major performance optimizations across I/O, pattern generation, and TUI rendering.
- Professional security-focused TUI redesign with modern layout.

---

## [0.1.4] - 2026-02-27

### Added

- Complete TUI overhaul with modern design, sparkline throughput chart, scrollable log viewer, and keyboard-driven navigation.
- Debug logging infrastructure with `eprintln` converted to `log::debug` to prevent TUI interference.

### Fixed

- Windows: Set disk OFFLINE and use synchronous I/O to resolve write failures.
- Windows: Add `SeManageVolumePrivilege`, `SeBackupPrivilege`, `SeRestorePrivilege` for raw disk I/O.
- Windows: Remove unnecessary volume dismount/lock, use zero sharing mode for exclusive physical drive access.
- Windows: Add `WRITE_DAC`, `READ_CONTROL`, `SYNCHRONIZE` access rights.
- Windows: Enable `FSCTL_LOCK_VOLUME` and `FSCTL_ALLOW_EXTENDED_DASD_IO` for Windows 11 compatibility.
- Windows: Add delay after dismount and use exclusive write access.
- Windows: Filter key events to `Press`-only to prevent double input from `Press`+`Release`.

---

## [0.1.3] - 2026-02-26

### Added

- Administrator reminder in Windows TUI confirmation dialog.
- Comprehensive Windows debugging with file-based debug log at `%TEMP%\drivewipe_debug.log`.
- Device unmounting before raw I/O on all platforms.
- Improved boot drive detection and config fallbacks.

### Fixed

- Windows TUI: "device disconnected" error on wipe start (Windows device paths don't support `.exists()` check).
- Windows: DoD wipe verification buffer alignment issues with `FILE_FLAG_NO_BUFFERING`.
- Windows TUI: Drive capacity now displays correctly (fixed `IOCTL_DISK_GET_LENGTH_INFO` access mode).
- Verification warnings and errors now properly sent as `ProgressEvent` messages.

---

## [0.1.2] - 2026-02-25

### Added

- All firmware wipe implementations fully cross-platform:
  - ATA Secure Erase: Linux (`SG_IO` + `ATA_16` CDB), Windows (`IOCTL_ATA_PASS_THROUGH`)
  - NVMe Format/Sanitize: Linux (`NVME_IOCTL_ADMIN_CMD`), macOS (`nvme-cli` shell-out), Windows (`IOCTL_STORAGE_PROTOCOL_COMMAND`)
  - TCG Opal crypto erase: Linux (`sed-opal` kernel ioctls)
- Windows platform support for drive enumeration, raw device I/O, and all wipe methods.
- GitHub Actions release workflow for 6 platform targets (Linux/macOS/Windows x86_64/ARM64).
- `WipeSession` firmware dispatch — `execute_firmware()` on `WipeMethod` trait skips software write loop for firmware methods.

### Fixed

- Windows-only clippy lints and cross-platform compilation errors.
- Unix-only `extract_base_device` gated behind `#[cfg(unix)]`.
- Test path separators for Windows CI.

---

## [0.1.1] - 2026-02-24

### Added

- Safety First automated versioning system (`xtask bump`).
- Comprehensive audit fixes and expanded test suite.

### Fixed

- Documentation link issues and doc comment formatting.

---

## [0.1.0] - 2026-02-23

Initial release of DriveWipe.

### Added

- **Core library** (`drivewipe-core`):
  - 9 software wipe methods: Zero Fill, One Fill, Random (AES-256-CTR), DoD 5220.22-M (3-pass), DoD 5220.22-M ECE (7-pass), Gutmann (35-pass), HMG IS5 Baseline, HMG IS5 Enhanced, RCMP TSSIT OPS-II
  - 8 firmware wipe methods: ATA Secure Erase, ATA Enhanced Secure Erase, NVMe Format (User Data Erase & Cryptographic Erase), NVMe Sanitize (Block Erase, Crypto Erase, Overwrite), TCG Opal Crypto Erase
  - Custom user-defined wipe methods via `config.toml`
  - AES-256-CTR cryptographic PRNG with hardware AES-NI acceleration
  - Method registry with software + firmware method lookup
  - Linux raw device I/O (`O_DIRECT | O_SYNC | O_NOFOLLOW`, `BLKSSZGET` ioctl)
  - macOS raw device I/O (`F_NOCACHE`, `DKIOCGETBLOCKSIZE` ioctl, `/dev/rdisk` paths)
  - Page-aligned buffer allocation for direct I/O
  - Linux drive enumeration via sysfs (`/sys/block/`)
  - macOS drive enumeration via `diskutil` plist parsing
  - Boot drive detection (Linux: `/proc/mounts`, macOS: `/sbin/mount`)
  - Pattern-based and zero-optimized read-back verification
  - Session resume from saved state (JSON persistence with device serial matching)
  - JSON report generation (automatic after every wipe)
  - PDF report generation ("Data Sanitization Certificate" via `genpdf`)
  - Progress event system via crossbeam channels
  - Cooperative cancellation via `CancellationToken`
  - Multi-step safety confirmation system
- **CLI** (`drivewipe`): Subcommands for list, wipe, verify, info, report, queue, resume. Force mode for scripted use. Interactive confirmation with countdown.
- **TUI** (`drivewipe-tui`): Drive selection table, method picker with per-drive suggestions, multi-drive wipe dashboard with progress gauges, scrollable log viewer.
- **Test suite**: 130 tests covering types, config, errors, patterns, PRNG, aligned buffers, wipe sessions, verification, cancellation, method registry, and report serialization.
- GitHub Actions CI (build, test, clippy, fmt, docs, security audit) across Linux, macOS, and Windows.

[Unreleased]: https://github.com/KodyDennon/DriveWipe/compare/v1.3.0...HEAD
[1.3.0]: https://github.com/KodyDennon/DriveWipe/compare/v1.2.0...v1.3.0
[1.2.0]: https://github.com/KodyDennon/DriveWipe/compare/v1.1.13...v1.2.0
[1.1.13]: https://github.com/KodyDennon/DriveWipe/compare/v1.1.12...v1.1.13
[1.1.12]: https://github.com/KodyDennon/DriveWipe/compare/v1.1.11...v1.1.12
[1.1.11]: https://github.com/KodyDennon/DriveWipe/compare/v1.1.10...v1.1.11
[1.1.10]: https://github.com/KodyDennon/DriveWipe/compare/v1.1.9...v1.1.10
[1.1.9]: https://github.com/KodyDennon/DriveWipe/compare/v1.1.8...v1.1.9
[1.1.8]: https://github.com/KodyDennon/DriveWipe/compare/v1.1.7...v1.1.8
[1.1.7]: https://github.com/KodyDennon/DriveWipe/compare/v1.1.6...v1.1.7
[1.1.6]: https://github.com/KodyDennon/DriveWipe/compare/v1.1.5...v1.1.6
[1.1.5]: https://github.com/KodyDennon/DriveWipe/compare/v1.1.4...v1.1.5
[1.1.4]: https://github.com/KodyDennon/DriveWipe/compare/v1.1.3...v1.1.4
[1.1.3]: https://github.com/KodyDennon/DriveWipe/compare/v1.1.2...v1.1.3
[1.1.2]: https://github.com/KodyDennon/DriveWipe/compare/v1.1.1...v1.1.2
[1.1.1]: https://github.com/KodyDennon/DriveWipe/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/KodyDennon/DriveWipe/compare/v0.1.5...v1.1.0
[0.1.5]: https://github.com/KodyDennon/DriveWipe/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/KodyDennon/DriveWipe/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/KodyDennon/DriveWipe/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/KodyDennon/DriveWipe/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/KodyDennon/DriveWipe/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/KodyDennon/DriveWipe/releases/tag/v0.1.0
