# Architecture

## Workspace Structure

```
DriveWipe/
├── crates/
│   ├── drivewipe-core/     # All business logic (library crate)
│   ├── drivewipe-cli/      # CLI binary (thin wrapper)
│   ├── drivewipe-tui/      # TUI binary (ratatui)
│   ├── drivewipe-gui/      # GUI binary (iced)
│   ├── drivewipe-live/     # Live environment (HPA/DCO, kernel module, ATA security)
│   └── xtask/              # Build automation
├── kernel/                 # Custom Linux kernel module for ATA/NVMe passthrough
├── live/                   # Live ISO boot configs (grub, syslinux, PXE, Alpine)
├── docs/                   # Documentation
└── scripts/                # Build, release, and cross-check scripts
```

## Core Library (`drivewipe-core`)

All business logic lives in the core crate. The CLI, TUI, and GUI are thin presentation layers.

### Key Traits

- **`RawDeviceIo`** — Platform-specific raw device I/O (Linux O_DIRECT, macOS F_NOCACHE, Windows FILE_FLAG_NO_BUFFERING)
- **`WipeMethod`** — Wipe method interface (name, passes, pattern generation, verification)
- **`PatternGenerator`** — Generate fill patterns for wipe passes
- **`FirmwareWipe`** — Firmware-level erase commands (ATA, NVMe, TCG Opal)
- **`DriveEnumerator`** — Platform-specific drive discovery
- **`Verifier`** — Read-back verification engine
- **`ReportGenerator`** — JSON/PDF report output

### Core Patterns

**Progress Events:** All long-running operations communicate via `ProgressEvent` enum sent over `crossbeam_channel::Sender`. This decouples core logic from UI.

**CancellationToken:** Cooperative cancellation across threads. Shared between the session executor and the UI layer.

**RAII Guards:** Sleep prevention (`SleepGuard`) and keyboard lock use RAII — acquire on creation, release on drop.

**WipeSession:** Central orchestrator that coordinates:
1. Audit logging
2. Profile lookup
3. Pre-wipe health snapshot
4. Sleep guard activation
5. Time estimator initialization
6. Pattern generation and write
7. Verification
8. Post-wipe health comparison
9. Report generation
10. Notification on completion

### Module Map

| Module | Responsibility |
|---|---|
| `audit` | JSONL audit trail with date rotation |
| `clone` | Block/partition-aware cloning, image I/O |
| `config` | TOML config loading |
| `crypto` | AES-256-CTR PRNG for pattern generation; AES-256-CTR stream cipher for image encryption with SHA-256 KDF |
| `drive` | Enumeration, DriveInfo, boot detection |
| `forensic` | Entropy, signatures, sampling, hidden areas, reports |
| `health` | SMART/NVMe parsing, snapshots, diffs, benchmarks |
| `io` | RawDeviceIo implementations per platform |
| `keyboard_lock` | Ring buffer sequence detection |
| `notify` | Cross-platform desktop notifications |
| `partition` | GPT/MBR parsing, CRUD ops, filesystem detection |
| `pattern` | Pattern generation (zero, one, random, constant, repeating) |
| `profile` | Drive profiles with regex matching |
| `progress` | ProgressEvent enum (26+ variants) |
| `report` | JSON and PDF output |
| `resume` | Crash-safe state persistence |
| `session` | WipeSession orchestrator |
| `sleep_inhibit` | Platform sleep prevention |
| `time_estimate` | EMA-smoothed estimation with confidence intervals |
| `verify` | Read-back verification |
| `wipe` | Method registry, software/firmware/DriveWipe Secure |

## TUI (`drivewipe-tui`)

Built on ratatui 0.30. Uses an `AppScreen` enum state machine with 19+ states (including live environment screens). The main event loop handles terminal events (key/mouse/resize) and progress events from core. Supports interactive partition CRUD (create/delete) and forensic scan execution.

## GUI (`drivewipe-gui`)

Built on iced 0.14. Uses a `Screen` enum for navigation, `Message` enum for all events, and delegates to screen view functions. Supports wipe execution, clone execution with progress, forensic scanning, health checks, and partition viewing.

## CLI (`drivewipe-cli`)

Uses clap for argument parsing with subcommands: `list`, `info`, `wipe`, `verify`, `report`, `resume`, `queue`, `health`, `profile`, `clone`, `partition`, `forensic`.
