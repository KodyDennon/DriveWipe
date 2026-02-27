# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial implementation of DriveWipe
- Core library (`drivewipe-core`) with full wipe engine
  - 9 software wipe methods (Zero, One, Random, DoD 5220.22-M, DoD ECE, Gutmann, HMG IS5, RCMP)
  - 8 firmware wipe method stubs (ATA Secure Erase, NVMe Format/Sanitize, TCG Opal)
  - Custom user-defined wipe methods from config
  - AES-256-CTR PRNG with hardware acceleration
  - Cross-platform raw device I/O (Linux O_DIRECT, macOS F_NOCACHE, Windows NO_BUFFERING)
  - Cross-platform drive enumeration (Linux sysfs, macOS diskutil, Windows stub)
  - Pattern-based and zero-optimized read-back verification
  - Session resume from saved state
  - JSON and PDF report generation
  - Progress event system via crossbeam channels
  - Boot drive detection and refusal
  - Multi-step safety confirmation system
- CLI (`drivewipe`) with subcommands: list, wipe, verify, info, report, queue, resume
  - Force mode for scripted use (`--force --yes-i-know-what-im-doing`)
  - indicatif progress bars with throughput and ETA
  - Interactive multi-step confirmation with countdown
- TUI (`drivewipe-tui`) with full interactive interface
  - Drive selection with checkbox table
  - Method picker with auto-suggestion
  - Multi-drive wipe dashboard with progress gauges and throughput sparkline
  - Scrollable log viewer
  - Keyboard-driven navigation
- GUI (`drivewipe-gui`) stub for Phase 2
- GitHub Actions CI (build, test, clippy, fmt across Linux/macOS/Windows)
- Full documentation (README, PLAN, CONTRIBUTING, SECURITY, CODE_OF_CONDUCT)
