# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Core library (`drivewipe-core`) with full wipe engine
  - 9 software wipe methods: Zero, One, Random (AES-256-CTR), DoD 5220.22-M (3-pass), DoD ECE (7-pass), Gutmann (35-pass), HMG IS5 Baseline, HMG IS5 Enhanced, RCMP TSSIT OPS-II
  - 8 firmware wipe method stubs (ATA Secure Erase, ATA Enhanced, NVMe Format x2, NVMe Sanitize x3, TCG Opal) — return `PlatformNotSupported`
  - Custom user-defined wipe methods from config.toml
  - AES-256-CTR PRNG with hardware AES-NI acceleration
  - Method registry with software + firmware method lookup
  - Linux raw device I/O (`O_DIRECT | O_SYNC | O_NOFOLLOW`, `BLKSSZGET` ioctl, block device validation)
  - macOS raw device I/O (`F_NOCACHE`, `DKIOCGETBLOCKSIZE` ioctl, `/dev/rdisk` path validation)
  - Windows raw device I/O stub — compiles but returns `PlatformNotSupported`
  - Page-aligned buffer allocation for direct I/O
  - Linux drive enumeration via sysfs (`/sys/block/`)
  - macOS drive enumeration via `diskutil` plist parsing
  - Windows drive enumeration stub — returns empty list
  - Boot drive detection (Linux: `/proc/mounts`, macOS: `/sbin/mount`, Windows: stub)
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

### Known Limitations

- All 8 firmware wipe methods are stubs (require platform-specific ATA/NVMe/SED ioctl)
- Windows I/O and drive enumeration are stubs
- Windows boot drive detection always returns false
- ATA security state detection not yet implemented on any platform
- SMART health querying not yet implemented on any platform
- HPA/DCO hidden area detection returns defaults on all platforms
