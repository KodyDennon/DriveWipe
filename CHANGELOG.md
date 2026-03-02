# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- **Windows**: DoD wipe verification now works correctly on Windows. Fixed buffer alignment issues that caused verification to fail silently when using `FILE_FLAG_NO_BUFFERING`.
  - `PatternVerifier`, `ZeroVerifier`, and random pattern verification now use aligned buffers for reads, matching the alignment requirements of Windows direct I/O
  - Verification warnings and errors are now properly sent as `ProgressEvent` messages so they appear in the TUI log viewer
  - Users will now see detailed error messages (e.g., "Verification mismatch at offset 0x1234") instead of just "FAILED"
- **Windows TUI**: Drive capacity now displays correctly in the drive list. Fixed drive enumeration to open drives with `GENERIC_READ` access instead of zero access, which is required for `IOCTL_DISK_GET_LENGTH_INFO` to succeed.
- **Windows TUI**: Fixed "device disconnected" error when starting wipe. Windows device paths like `\\.\PhysicalDrive0` don't support `.exists()` check, so this validation is now skipped on Windows platforms.
- **Windows TUI**: Added Administrator reminder to confirmation dialog to help users avoid common privilege errors.
- **Windows Debugging**: Added comprehensive logging and debug file output to diagnose device opening failures. Debug log is written to `%TEMP%\drivewipe_debug.log` (typically `C:\Users\<username>\AppData\Local\Temp\drivewipe_debug.log`). The TUI now displays the debug log location in the log viewer. Error messages include specific Windows error codes and are written to stderr, the debug file, and the TUI log viewer for complete troubleshooting visibility.
- Test suite: Fixed clippy warnings for redundant imports and bool comparisons
- Cross-module visibility: Made `extract_windows_drive_number` visible to other modules via `pub(crate)`
- Windows I/O: Fixed clippy warning for `std::mem::forget` on `Copy` type by using `let _ = handle` pattern

### Added

- Core library (`drivewipe-core`) with full wipe engine
  - 9 software wipe methods: Zero, One, Random (AES-256-CTR), DoD 5220.22-M (3-pass), DoD ECE (7-pass), Gutmann (35-pass), HMG IS5 Baseline, HMG IS5 Enhanced, RCMP TSSIT OPS-II
  - 8 firmware wipe methods: ATA Secure Erase, ATA Enhanced Secure Erase, NVMe Format (User Data & Crypto), NVMe Sanitize (Block, Crypto, Overwrite), TCG Opal Crypto Erase
  - Custom user-defined wipe methods from config.toml
  - AES-256-CTR PRNG with hardware AES-NI acceleration
  - Method registry with software + firmware method lookup
  - Linux raw device I/O (`O_DIRECT | O_SYNC | O_NOFOLLOW`, `BLKSSZGET` ioctl, block device validation)
  - macOS raw device I/O (`F_NOCACHE`, `DKIOCGETBLOCKSIZE` ioctl, `/dev/rdisk` path validation)
  - Windows raw device I/O (`FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH`, `OVERLAPPED` read/write, `DeviceIoControl` for capacity/geometry)
  - Page-aligned buffer allocation for direct I/O
  - Linux drive enumeration via sysfs (`/sys/block/`)
  - macOS drive enumeration via `diskutil` plist parsing
  - Windows drive enumeration via `DeviceIoControl` (`IOCTL_STORAGE_QUERY_PROPERTY`, `IOCTL_DISK_GET_DRIVE_GEOMETRY_EX`, SSD detection via seek penalty)
  - Boot drive detection (Linux: `/proc/mounts`, macOS: `/sbin/mount`, Windows: `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS` on `C:\`)
  - Pattern-based and zero-optimized read-back verification
  - Session resume from saved state (JSON persistence with device serial matching)
  - JSON report generation (auto after every wipe)
  - PDF report generation ("Data Sanitization Certificate" via `genpdf`)
  - Progress event system via crossbeam channels
  - Cooperative cancellation via `CancellationToken` (Arc<AtomicBool>)
  - Multi-step safety confirmation system
- CLI (`drivewipe`) with subcommands: list, wipe, verify, info, report, queue, resume
  - Force mode for scripted use (`--force --yes-i-know-what-im-doing`)
  - indicatif progress bars with throughput and ETA
  - Interactive multi-step confirmation with countdown
  - Parallel multi-drive queue with per-drive method assignment
- TUI (`drivewipe-tui`) with full interactive interface
  - Drive selection with checkbox table
  - Method picker with auto-suggestion per drive type
  - Multi-drive wipe dashboard with progress gauges and throughput sparkline
  - Scrollable log viewer
  - Keyboard-driven navigation
  - Auto JSON report generation on wipe completion
- GUI (`drivewipe-gui`) — Phase 2 placeholder (prints message and exits)
- Comprehensive test suite (130 tests)
  - Unit tests for all types, config, errors, patterns, PRNG, aligned buffers
  - Integration tests with MockDevice for wipe sessions, verification, cancellation
  - Registry tests for all 17 methods
  - Report serialization round-trip tests
- GitHub Actions CI (build, test, clippy, fmt, docs, security audit across Linux/macOS/Windows)
- Documentation: README, PLAN, CONTRIBUTING, SECURITY, CODE_OF_CONDUCT, CHANGELOG
- WipeSession firmware dispatch — `execute_firmware()` on `WipeMethod` trait skips software write loop for firmware methods
- Firmware wipe implementations (fully cross-platform where hardware allows):
  - ATA Secure Erase: Linux (`SG_IO` + `ATA_16` CDB), Windows (`IOCTL_ATA_PASS_THROUGH`)
  - NVMe Format/Sanitize: Linux (`NVME_IOCTL_ADMIN_CMD`), macOS (shells to `nvme-cli`), Windows (`IOCTL_STORAGE_PROTOCOL_COMMAND`)
  - TCG Opal crypto erase: Linux (`sed-opal` kernel ioctls)

### Known Limitations

- ATA Secure Erase is not supported on macOS (no reliable ATA passthrough)
- NVMe commands on macOS require `nvme-cli` (`brew install nvme-cli`)
- TCG Opal crypto erase is only supported on Linux (macOS/Windows planned)
- ATA security state detection not yet implemented on any platform
- SMART health querying not yet implemented on any platform
- HPA/DCO hidden area detection returns defaults on all platforms
