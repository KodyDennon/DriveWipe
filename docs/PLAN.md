# DriveWipe — Implementation Plan

## Context

DriveWipe is a cross-platform (macOS, Linux, Windows) secure data sanitization tool written in Rust. It implements military/corporate-grade drive wiping compliant with modern standards (NIST SP 800-88 Rev. 2, IEEE 2883:2022) including software overwrite for HDDs, firmware-level commands for SSDs, and cryptographic erasure for self-encrypting drives.

The tool provides three interfaces: a CLI for scripting/config, a TUI for advanced interactive use, and a GUI (Phase 2, pure Rust via egui/iced) for simpler automated use. It supports parallel multi-drive wipe, full read-back verification, audit logging, and PDF/JSON report generation.

**Key decisions from user:**
- All platforms (macOS, Linux, Windows)
- Full erasure suite (software + firmware + crypto erase)
- Multi-step safety confirmation before any destructive operation
- Refuse to wipe boot drive (plan for bootable USB image later)
- Resume capability after interruption with auto-restart option
- Drive queue: pre-built batch mode + live queue (add drives during active wipe)
- Per-drive method assignment in batches (auto-suggest best method per drive type)
- Auto-generate JSON report after every wipe (PDF on request only)
- `--force` flag for scripted use with extra safeguards (requires explicit --device, --method, --yes-i-know-what-im-doing)
- Warn on SSDs with software overwrite (recommend firmware erase)
- Warn on USB-attached drives (firmware commands may fail)
- Full audit logging (per-second summaries during wipe)
- State/logs saved to `~/.local/share/drivewipe/sessions/`
- Standalone only (no network/fleet features, but extensible architecture)
- 100% AI-coded — no constraints on complexity

**Custom License:** Free for personal, hobby, and commercial use. Government or organizations in intelligence, forensics, and security-related fields are expected to support or contribute to development. Open contribution.

---

## Phase 1: Core Engine + CLI + TUI

### Workspace Structure

```
DriveWipe/
  Cargo.toml                    # workspace root
  LICENSE.md
  README.md
  rustfmt.toml
  .github/workflows/ci.yml

  crates/
    drivewipe-core/             # library crate — all logic, no UI
      src/
        lib.rs
        error.rs                # DriveWipeError (thiserror)
        config.rs               # ~/.config/drivewipe/config.toml parsing
        types.rs                # DriveInfo, Transport, DriveType, WipeResult, etc.
        session.rs              # WipeSession + WipeEngine orchestrator

        drive/
          mod.rs                # DriveEnumerator trait
          info.rs               # DriveInfo construction, HPA/DCO, SMART
          linux.rs              # sysfs + udev + ioctl enumeration
          macos.rs              # IOKit + DiskArbitration
          windows.rs            # SetupDi + WMI + DeviceIoControl

        io/
          mod.rs                # RawDeviceIo trait + aligned buffer alloc
          linux.rs              # O_DIRECT, /dev/sdX, /dev/nvmeXnY
          macos.rs              # F_NOCACHE, /dev/rdiskN
          windows.rs            # FILE_FLAG_NO_BUFFERING, \\.\PhysicalDriveN

        wipe/
          mod.rs                # WipeMethod trait + WipeMethodRegistry + WipeEngine
          patterns.rs           # PatternGenerator trait + ZeroFill, OneFill, RandomFill, etc.
          software.rs           # All software overwrite methods (DoD, Gutmann, HMG, etc.)
          custom.rs             # User-defined custom wipe method from config
          crypto_erase.rs       # TCG Opal crypto erase
          firmware/
            mod.rs              # FirmwareWipe trait + dispatch
            ata.rs              # ATA Secure Erase (Normal + Enhanced)
            nvme.rs             # NVMe Format (SES=1,2) + NVMe Sanitize

        verify/
          mod.rs                # Verifier trait
          pattern_verify.rs     # Read-back comparison against expected pattern
          zero_verify.rs        # Optimized all-zeros check

        progress/
          mod.rs                # ProgressChannel, CancellationToken
          events.rs             # ProgressEvent enum (SessionStarted, BlockWritten, etc.)

        report/
          mod.rs                # ReportGenerator trait
          data.rs               # WipeReport struct (serde)
          json.rs               # JSON report generator
          pdf.rs                # PDF certificate generator (genpdf)

        crypto/
          mod.rs
          aes_ctr_rng.rs        # AES-256-CTR PRNG with AES-NI acceleration

        platform/
          mod.rs
          privilege.rs          # Root/admin check + hint messages

        resume/
          mod.rs                # WipeState persistence + resume logic
          state.rs              # Serializable progress state per session

    drivewipe-cli/              # binary crate — command-line interface
      src/
        main.rs                 # clap App setup, subcommand dispatch
        commands/
          mod.rs
          list.rs               # drivewipe list
          wipe.rs               # drivewipe wipe --method <m> --device <d>
          verify.rs             # drivewipe verify --device <d>
          info.rs               # drivewipe info --device <d>
          report.rs             # drivewipe report --input <log> --format <pdf|json>
          queue.rs              # drivewipe queue (batch add/start/status/cancel)
          resume.rs             # drivewipe resume (list/resume interrupted sessions)
        display.rs              # Table formatting for terminal output
        confirm.rs              # Multi-step confirmation (type serial + countdown)
        progress.rs             # indicatif progress bars with throughput + ETA

    drivewipe-tui/              # binary crate — terminal UI
      src/
        main.rs                 # crossterm alternate screen + raw mode setup
        app.rs                  # App state machine (DriveSelection -> MethodSelect -> Confirm -> Wiping -> Done)
        event.rs                # Input + progress + tick event loop
        ui/
          mod.rs                # Top-level layout
          drive_list.rs         # Drive table with checkbox selection
          method_select.rs      # Method picker with descriptions
          wipe_dashboard.rs     # Multi-drive progress (gauges, throughput, ETA)
          info_panel.rs         # Detailed drive info popup
          log_viewer.rs         # Scrollable timestamped log
          confirm_dialog.rs     # Modal requiring "YES" typed input
          help.rs               # Keyboard shortcut overlay
        widgets/
          mod.rs
          throughput_sparkline.rs  # Live throughput graph (60 samples)
          progress_gauge.rs     # Per-drive custom gauge

    drivewipe-gui/              # Phase 2 — stub only for now
      Cargo.toml
      src/
        main.rs
```

### Core Traits

**`WipeMethod`** — Defines a multi-pass erasure strategy:
- `name()`, `id()`, `description()`, `pass_count()`
- `pattern_for_pass(pass) -> Box<dyn PatternGenerator>`
- `includes_verification() -> bool`

**`PatternGenerator`** — Produces fill data:
- `fill(buf: &mut [u8])` — fills buffer with pattern bytes
- Implementations: `ZeroFill`, `OneFill`, `ConstantFill(u8)`, `RandomFill(AesCtrRng)`, `RepeatingPattern(Vec<u8>)`

**`RawDeviceIo`** — Direct disk I/O:
- `open(path)`, `write_at(offset, buf)`, `read_at(offset, buf)`, `capacity()`, `block_size()`, `sync()`, `close()`
- Platform implementations: Linux (O_DIRECT), macOS (F_NOCACHE), Windows (NO_BUFFERING)

**`DriveEnumerator`** — Drive detection:
- `enumerate() -> Vec<DriveInfo>`, `inspect(path) -> DriveInfo`
- `detect_hidden_areas(path) -> HiddenAreaInfo`, `ata_security_state(path)`

**`FirmwareWipe`** — Firmware-level erase:
- `is_supported(drive) -> bool`, `execute(path, progress_tx) -> Result`
- Implementations: `AtaSecureErase`, `NvmeFormatErase`, `NvmeSanitize`

**`WipeEngine`** — Orchestrator:
- `execute(session, progress_tx, cancel_token) -> WipeReport`
- Core loop: for each pass, fill 1 MiB aligned buffer with pattern, write to device, send progress events
- Checks cancel token between blocks, saves state for resume

**`ProgressEvent`** — Channel-based event system (crossbeam-channel):
- `SessionStarted`, `PassStarted`, `BlockWritten`, `PassCompleted`, `VerificationStarted/Progress/Completed`, `FirmwareEraseStarted/Progress/Completed`, `Error`, `Interrupted`, `Completed`
- All frontends (CLI, TUI, GUI) receive these same events

### Wipe Methods Implemented

| ID | Name | Passes | Notes |
|---|---|---|---|
| `zero` | Zero Fill | 1 | 0x00 everywhere |
| `one` | One Fill | 1 | 0xFF everywhere |
| `random` | Random Fill | 1 | AES-256-CTR PRNG |
| `dod-short` | DoD 5220.22-M | 3 | 0x00, 0xFF, random + verify |
| `dod-ece` | DoD 5220.22-M ECE | 7 | 3-pass + random + 3-pass |
| `gutmann` | Gutmann | 35 | Legacy — 35 specific patterns |
| `hmg-baseline` | HMG IS5 Baseline | 1 | 0x00 + verify |
| `hmg-enhanced` | HMG IS5 Enhanced | 3 | 0x00, 0xFF, random + verify |
| `rcmp` | RCMP TSSIT OPS-II | 7 | Alternating 0/1 x6 + random |
| `custom` | Custom | N | User-defined in config.toml |
| `ata-erase` | ATA Secure Erase | firmware | Normal mode |
| `ata-erase-enhanced` | ATA Enhanced Erase | firmware | Includes remapped sectors |
| `nvme-format-user` | NVMe Format (User Data) | firmware | SES=1 |
| `nvme-format-crypto` | NVMe Format (Crypto) | firmware | SES=2, near-instant |
| `nvme-sanitize-block` | NVMe Sanitize (Block) | firmware | Block erase |
| `nvme-sanitize-crypto` | NVMe Sanitize (Crypto) | firmware | Destroy encryption key |
| `nvme-sanitize-overwrite` | NVMe Sanitize (Overwrite) | firmware | Pattern overwrite |
| `tcg-opal` | TCG Opal Crypto Erase | firmware | SED key destruction |

### Safety Features

1. **Boot drive detection** — Refuse to wipe the drive the OS is running from
2. **Multi-step confirmation** — Show drive serial/model/capacity, require typing device path or "YES I UNDERSTAND", 3-second countdown with abort
3. **SSD software wipe warning** — Detect SSD, warn that software overwrite is unreliable due to wear leveling, recommend firmware erase, allow override
4. **USB drive warning** — Detect USB-attached drives, warn that firmware commands may fail through USB bridges
5. **Frozen ATA security warning** — Detect frozen state, suggest suspend/resume to unfreeze
6. **HPA/DCO detection** — Check for hidden areas, warn that they won't be reached by software overwrite, recommend ATA Enhanced Erase
7. **Root/admin check** — Verify elevated privileges on startup, provide platform-specific instructions if not elevated
8. **Ctrl+C handling** — Graceful interruption via CancellationToken, save state for resume

### Queue System

- **Batch mode**: Select multiple drives, assign method per drive (or one method for all), start the batch. Runs sequentially or in parallel based on `--parallel` flag or TUI toggle.
- **Live queue**: While drives are actively wiping, user can add more drives to the queue from the TUI or CLI. New drives start when a parallel slot opens up (configurable max concurrent drives, default: number of physical drives detected).
- **Auto-suggest**: When a drive is added to the queue, auto-suggest the best wipe method based on drive type (firmware erase for SSDs, software overwrite for HDDs). User can override.
- **Per-drive method**: Each queued drive has its own method, verification flag, and report config. The TUI shows a per-row method dropdown.

### Force Mode (Scripted/Automated Use)

- `--force` flag skips interactive confirmation BUT requires:
  - Explicit `--device <path>` (no "pick from list")
  - Explicit `--method <id>` (no default)
  - Must also pass `--yes-i-know-what-im-doing` (long flag, hard to typo)
- Logs a `WARN` level message: "Running in force mode — all confirmation bypassed"
- All other safety checks still apply (boot drive refusal, privilege check, SSD warnings logged but not interactive)

### Resume System

- State file: `~/.local/share/drivewipe/sessions/<uuid>.state`
- Contains: session UUID, device path, device serial, method ID, current pass, bytes written, timestamp
- On startup with `--auto-resume`: scan for incomplete `.state` files, match by device serial (not path, which can change), offer to resume
- State saved every 10 seconds during active wipe
- Cleaned up on successful completion

### Audit Logging

- Log file: `~/.local/share/drivewipe/sessions/<uuid>.log`
- Per-second entries during active wipe: `[timestamp] pass=2/3 written=45.2GiB/500GiB throughput=312MiB/s`
- All events logged: start, each pass start/end, errors, verification result, completion
- Survives crashes — flush after each entry

### Report Generation

- **JSON**: Auto-generated after every wipe to `~/.local/share/drivewipe/sessions/<uuid>.report.json`. Always available as an audit trail with zero user effort.
- **PDF**: "Data Sanitization Certificate" — generated on request via `drivewipe report --format pdf --input <json>` or `--report-pdf <path>` flag during wipe. Contains drive info, method, pass details table, verification result, timestamps, operator name, hostname, session UUID.
- CLI `drivewipe report` subcommand can also regenerate PDF from any saved JSON report after the fact.

### Key Dependencies

| Crate | Purpose |
|---|---|
| `thiserror` | Error types |
| `serde` + `serde_json` + `toml` | Serialization, config |
| `clap` (derive) | CLI argument parsing |
| `ratatui` + `crossterm` | TUI framework |
| `indicatif` | CLI progress bars |
| `crossbeam-channel` | Progress event channels |
| `aes` + `ctr` + `cipher` | AES-256-CTR PRNG |
| `rand` | CSPRNG seeding |
| `nix` | Linux/macOS ioctl, raw I/O |
| `windows` | Windows API bindings |
| `genpdf` | PDF report generation (behind `pdf-report` feature) |
| `zeroize` | Secure memory zeroing |
| `uuid` + `chrono` | Session IDs, timestamps |
| `ctrlc` | Signal handling |
| `dialoguer` | CLI interactive prompts |
| `log` + `env_logger` | Logging |

### Build Order (dependency-driven)

```
Step 1: Workspace skeleton, error types, shared types, config, privilege checks    [DONE]
   |
Step 2: AES-256-CTR PRNG + PatternGenerator implementations                       [DONE]
   |
Step 3: RawDeviceIo trait + platform implementations (Linux, macOS, Windows)       [DONE]
   |
Step 4: DriveEnumerator trait + platform implementations                           [DONE]
   |
Step 5: WipeMethod implementations + WipeEngine + ProgressEvent + resume state     [DONE]
   |
Step 6: Verification (read-back) + Report generation (JSON + PDF)                  [DONE]
   |
   +--- Step 7: CLI (all subcommands, confirmation flow, progress bars)            [DONE]
   |
   +--- Step 8: TUI (state machine, all screens, multi-drive dashboard)            [DONE]
   |
Step 9: Firmware wipe commands (ATA Secure Erase, NVMe Format/Sanitize, TCG Opal) [DONE]
   |
Step 10: Live environment (Kernel module + builders + PXE infrastructure)          [DONE]
```

### Verification Plan

1. **Unit tests**: All pattern generators, wipe method configurations, error types, config parsing, report serialization round-trips
2. **Integration tests**: Wipe a 10 MiB temp file with each method, verify contents match expected patterns, test cancellation mid-wipe, test parallel multi-file wipe, test resume from saved state
3. **CLI tests**: `assert_cmd` + `predicates` for all subcommands with `--dry-run` flag
4. **TUI tests**: `ratatui::backend::TestBackend` render snapshots, state machine transition tests
5. **Real device tests**: Gated behind `--features real-device-tests` + `DRIVEWIPE_TEST_DEVICE` env var, never in CI
6. **CI**: GitHub Actions matrix — `[ubuntu-latest, macos-latest, windows-latest]` x `[stable, nightly]` — build, test, clippy, fmt

### Platform Limitations (documented clearly)

- **macOS**: ATA passthrough is very limited. Firmware erase commands may require Linux boot media. NVMe access uses private IONVMeFamily API or falls back to `nvme-cli` if installed. Recommend Linux for firmware operations.
- **Windows**: Requires Administrator. ATA passthrough via `IOCTL_ATA_PASS_THROUGH`. NVMe via `IOCTL_STORAGE_PROTOCOL_COMMAND`.
- **Linux**: Best platform support across the board. Full ioctl access for ATA, NVMe, and TCG Opal (via `sed-opal` kernel driver).

---

## Phase 2: GUI (Future)

- Pure Rust GUI using egui or iced
- Simplified workflow: detect drives -> pick method -> confirm -> wipe -> report
- Same core library, different presentation layer
