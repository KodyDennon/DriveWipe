# DriveWipe Overhaul Plan

> **Format**: What to do, not how to do it. Organized by priority phase. No time estimates.
>
> **Scope**: This plan covers the full transformation of DriveWipe from a secure wipe tool into a comprehensive drive management, forensics, and sanitization platform.

## Implementation Status Summary

> **Last verified: March 2026.** Status checked against actual codebase.

| # | Feature | Phase | Status |
|---|---|---|---|
| 1 | Drive health system | Phase 1 | ✅ Module exists (`health/` — 6 files: smart, nvme, benchmark, snapshot, diff) |
| 2 | Custom high-security wipe ("DriveWipe Secure") | Phase 1 | ✅ Implemented (4 variants in wipe registry) |
| 3 | Drive profile system | Phase 1 | ✅ Module exists (`profile/` — database, matcher, mod) |
| 4 | Time estimate overhaul | Phase 1 | ✅ Module exists (`time_estimate/` — EMA smoothing) |
| 5 | TUI overhaul | Phase 1 | 🔶 Partial — core screens done, new feature screens not yet |
| 6 | Unified audit log expansion | Phase 1 | ✅ Module exists (`audit/` — 2 files) |
| 7 | Sleep prevention | Phase 1 | ✅ Module exists (`sleep_inhibit/`) |
| 8 | Keyboard lock mode | Phase 1 | ✅ Module exists (`keyboard_lock/`) |
| 9 | Desktop notifications | Phase 1 | ✅ Module exists (`notify/`) |
| 10 | Crate structure evolution | Phase 1 | ✅ All modules created in `drivewipe-core` |
| 11 | Drive cloning | Phase 2 | ✅ Module exists (`clone/` — block, image, partition_aware) |
| 12 | Partition manager | Phase 2 | ✅ Module exists (`partition/` — gpt, mbr, ops, filesystem) |
| 13 | Forensic toolkit | Phase 2 | ✅ Module exists (`forensic/` — entropy, signatures, sampling, export, hidden) |
| 14 | Documentation overhaul | Phase 3 | 🔶 Partial — README overhauled, user guides not yet |
| 15 | Build system & installer | Phase 3 | ✅ CI workflows + xtask commands in place |
| 16 | GUI foundation (iced) | Phase 3 | 🔶 Scaffold only — `drivewipe-gui` crate exists |
| 17 | Bootable live environment | Phase 3 | ✅ Fully implemented (see LIVE-ENVIRONMENT-PLAN.md) |

---

## Phase 1 — Core Enhancements

Build on the existing wipe infrastructure. These items deepen what DriveWipe already does and lay groundwork that later phases depend on.

---

### 1.1 Drive Health System

Integrate comprehensive drive health monitoring into the core library. This underpins pre/post-wipe comparison (item 1.2) and forensic analysis (Phase 2).

- [ ] Implement SMART data retrieval for SATA/SAS drives (Linux, macOS, Windows)
- [ ] Implement NVMe health log retrieval (SMART/Health Information Log Page)
- [ ] Capture full SMART attribute set: read/write error rates, reallocated sectors, wear leveling count, power-on hours, temperature, pending sectors, uncorrectable errors
- [ ] Capture NVMe-specific health data: available spare, percentage used, data units read/written, media errors, critical warnings
- [ ] Implement error log retrieval (ATA error log, NVMe error log)
- [ ] Implement temperature history tracking during operations
- [ ] Build a quick sequential read/write performance benchmark (small, non-destructive for reads; writes only to already-allocated test regions or skipped for pre-wipe)
- [ ] Create a `DriveHealthSnapshot` data structure that captures all of the above at a point in time
- [ ] Implement pre-wipe health snapshot capture (automatically before any wipe operation)
- [ ] Implement post-wipe health snapshot capture (automatically after wipe completion)
- [ ] Build a health diff/comparison engine that highlights degradation between two snapshots
- [ ] Generate a pass/fail verdict based on configurable thresholds (new bad sectors, wear level increase, temperature spikes, performance degradation percentage)
- [ ] Include health data in wipe reports (JSON and PDF)
- [ ] Add a standalone `drivewipe health` CLI subcommand for on-demand health checks
- [ ] Add health display to the TUI (see 1.5)

---

### 1.2 Custom High-Security Wipe Method

Design and implement DriveWipe's own proprietary sanitization method that maximizes data destruction across all drive types, especially SSDs where software overwrites alone are insufficient.

- [ ] Design the "DriveWipe Secure" method specification that chains multiple techniques:
  - Full surface software overwrite (multiple passes with varied patterns)
  - TRIM/UNMAP all addressable blocks
  - Firmware sanitize command (if available for the drive)
  - Repeat the overwrite cycle post-TRIM to hit blocks the FTL has remapped
  - Final verification pass
- [ ] Create separate strategy variants for HDD, SATA SSD, NVMe SSD, and USB drives
- [ ] Clearly document what each step covers and what gaps remain per drive type
- [ ] Document known limitations honestly (over-provisioned areas, wear-leveled blocks, firmware-reserved regions)
- [ ] Research and implement vendor-specific sanitize commands for major flash controllers where publicly documented
- [ ] Investigate NAND-level access possibilities for common controllers (research-grade, best-effort)
- [ ] Register the method in the `WipeMethodRegistry` alongside existing methods

---

### 1.3 Drive Profile System

Build a community-extensible drive profile database that enables drive-specific behavior across all DriveWipe operations.

- [ ] Define a drive profile schema (TOML files) containing:
  - Manufacturer and model family patterns (regex matching)
  - Flash controller type (for SSDs/NVMe)
  - Known over-provisioning ratio
  - Supported firmware sanitize commands and quirks
  - Recommended wipe strategy for this drive family
  - Known vulnerabilities or limitations (e.g., "firmware erase does not clear OP area")
  - Performance characteristics (expected throughput ranges)
- [ ] Build an auto-detection system that matches a connected drive to a profile based on model string, firmware revision, and vendor ID
- [ ] Ship built-in profiles for major manufacturers:
  - Samsung (EVO, PRO, QVO families, PM series enterprise)
  - Western Digital / SanDisk (Blue, Black, Red, Green, enterprise)
  - Seagate / LaCie (Barracuda, IronWolf, Exos, FireCuda)
  - Intel / Solidigm
  - Crucial / Micron
  - Kingston
  - SK Hynix
  - Toshiba / Kioxia
- [ ] Implement a fallback "generic" profile for unknown drives
- [ ] Create a `profiles/` directory in the repo for community-contributed profiles
- [ ] Document the profile format and contribution process for community members
- [ ] Add a `drivewipe profile` CLI subcommand to show which profile matched a drive
- [ ] Integrate profile data into wipe method selection (auto-suggest the profile's recommended strategy)
- [ ] Integrate profile data into the TUI drive info screen
- [ ] Integrate profile data into reports

---

### 1.4 Time Estimate Overhaul

Replace the current throughput-based ETA with a significantly more accurate estimation system.

- [ ] Implement exponential moving average smoothing for throughput measurements
- [ ] Account for multi-pass methods properly (estimate remaining time across all remaining passes, not just current pass)
- [ ] Factor in verification pass time separately (verification reads are typically faster than writes)
- [ ] Store historical wipe performance data per drive model/family (from drive profiles and past wipes on this machine)
- [ ] Use historical data to calibrate initial estimates before real throughput data is available
- [ ] Account for known drive speed characteristics:
  - HDD outer vs. inner track speed degradation
  - SSD write cliff when SLC cache is exhausted
  - NVMe thermal throttling patterns
- [ ] Display confidence intervals or a range (best case / expected / worst case)
- [ ] Show per-pass ETA breakdown in addition to total remaining time
- [ ] Persist wipe performance history to disk for future calibration (in sessions directory)

---

### 1.5 TUI Overhaul

Research TUI framework alternatives and redesign the terminal interface to be a full-featured drive management toolkit.

#### Framework Research

- [ ] Evaluate the current ratatui setup against alternatives:
  - tui-realm (component-based architecture on top of ratatui)
  - cursive (ncurses-based, different rendering model)
  - ratatui with a custom component/widget system built on top
  - Any other promising Rust TUI frameworks
- [ ] Decide on framework direction: stick with ratatui (with or without custom component layer), switch frameworks, or build custom
- [ ] If building a custom component system: design reusable, composable widget primitives that all screens share (consistent borders, navigation, keybindings, scrolling behavior)

#### New Screens & Modes

- [ ] **Drive Info / Health Screen** — detailed SMART attributes, NVMe health log, partition table layout, firmware info, drive profile match, health status with color-coded indicators
- [ ] **Clone Workflow Screen** — source/target drive selection, clone mode picker (block vs. partition-aware), progress tracking with source read + target write throughput, sector-level progress visualization
- [ ] **Partition Manager Screen** — visual partition table layout (like a horizontal bar chart), select partitions to resize/move/create/delete, live preview of changes before applying
- [ ] **Forensic Analysis Screen** — entropy heatmap visualization, sector sampling results, file signature scan results, hidden area detection status, exportable report trigger
- [ ] **Settings / Config Editor Screen** — edit config.toml values from within the TUI, toggle auto-verify, set default methods, manage custom methods, set operator info
- [ ] **Health Comparison Screen** — side-by-side pre/post wipe health data, degradation highlights, pass/fail verdict display

#### Existing Screen Improvements

- [ ] Improve the wipe dashboard with data from the health system and drive profiles
- [ ] Add drive profile badge/indicator to drive selection list
- [ ] Show health warnings during drive selection (e.g., "drive has 12 reallocated sectors")
- [ ] Improve log viewer with filtering and search
- [ ] Add keyboard navigation documentation inline (not just the help overlay)
- [ ] Ensure all new features (clone, forensics, partitions, health) are fully accessible from the TUI main menu

---

### 1.6 Unified Audit Log Expansion

Extend the existing per-session audit log system to cover all new operations with typed event categories.

- [ ] Add event categories to the audit log: wipe events, clone events, partition events, forensic events, health events
- [ ] All new operations (clone, partition management, forensic scans, health checks) write to the same audit log infrastructure
- [ ] Support filtering log entries by event category when reviewing
- [ ] Include the event category in JSON report output for downstream processing
- [ ] Ensure all destructive operations (wipe, clone write, partition modify) are logged before execution begins (intent log) and after completion (result log)

---

### 1.7 Sleep Prevention (Keep-Alive)

Prevent the host machine from sleeping during long-running operations in both TUI and GUI.

- [ ] Implement sleep inhibitor on Linux via systemd-inhibit / D-Bus (org.freedesktop.login1.Manager.Inhibit)
- [ ] Implement sleep inhibitor on macOS via IOPMAssertionCreateWithName (kIOPMAssertionTypePreventUserIdleSystemSleep)
- [ ] Implement sleep inhibitor on Windows via SetThreadExecutionState (ES_CONTINUOUS | ES_SYSTEM_REQUIRED)
- [ ] Activate sleep inhibition when any long-running operation starts (wipe, clone, forensic scan, benchmark)
- [ ] Release sleep inhibition when all operations complete or are cancelled
- [ ] Show a "sleep prevented" indicator in the TUI and GUI status bar
- [ ] Make sleep prevention configurable (enable/disable in config.toml, default: enabled)

---

### 1.8 Kitty Cat Keyboard Mode

A keyboard lock mode that disables all keyboard input in the TUI and GUI, preventing accidental keypresses from interfering with running operations. Requires a specific key sequence to unlock.

- [ ] Define an unlock key sequence (configurable, default: a multi-key combo that's unlikely to be typed accidentally)
- [ ] When activated: all keyboard input is ignored except the unlock sequence
- [ ] Display a clear visual indicator that keyboard is locked (prominent banner or status bar message)
- [ ] Show a hint about the unlock sequence on screen
- [ ] Add a keybinding to activate keyboard lock mode from any screen
- [ ] Automatically offer to activate keyboard lock when a long-running operation begins
- [ ] Log keyboard lock/unlock events to the audit log
- [ ] Support keyboard lock in both TUI and GUI

---

### 1.9 Desktop Notification System

Replace the terminal bell with proper OS-native desktop notifications so operators know when long-running operations complete, even if the terminal is in the background.

- [ ] Implement Linux notifications via libnotify / D-Bus (freedesktop notification spec)
- [ ] Implement macOS notifications via Notification Center (NSUserNotification or UNUserNotification)
- [ ] Implement Windows notifications via toast notifications (Windows.UI.Notifications)
- [ ] Notify on: wipe completion (success or failure), clone completion, forensic scan completion, health check completion
- [ ] Include key info in the notification: drive name, operation type, outcome, duration
- [ ] Make notifications configurable (enable/disable in config.toml)
- [ ] Integrate with TUI and CLI (both should trigger notifications on operation completion)

---

### 1.10 Crate Structure Evolution

All new feature domains (cloning, partitions, forensics, health) will be added as new modules within `drivewipe-core` rather than separate crates, keeping the single-library architecture and simple dependency graph.

- [ ] Add `core::health` module (SMART, NVMe health, benchmarks, snapshots, diffs)
- [ ] Add `core::clone` module (block-level and partition-aware cloning, image I/O)
- [ ] Add `core::partition` module (GPT/MBR parsing and manipulation, filesystem-aware resize/move)
- [ ] Add `core::forensic` module (entropy analysis, file signatures, sector sampling, hidden area detection)
- [ ] Add `core::profile` module (drive profile schema, auto-detection, profile database)
- [ ] Add `core::notify` module (cross-platform desktop notifications)
- [ ] Ensure clean module boundaries with well-defined public APIs that TUI, CLI, and GUI all consume identically
- [ ] Keep wipe-specific code in existing modules — new modules do not duplicate existing functionality

---

## Phase 2 — New Feature Domains

Entirely new capabilities that expand DriveWipe beyond secure wiping.

---

### 2.1 Drive Cloning

Implement both raw block-level and partition-aware drive cloning with full support for asymmetric (different-sized) source and target drives.

#### Clone Safety

- [ ] Multi-step confirmation before writing to target drive (show target drive details, require typing device path)
- [ ] Refuse to clone onto the boot drive unless explicitly overridden
- [ ] Warn if target drive contains existing data/partitions (offer to abort or confirm overwrite)
- [ ] Validate source is readable before beginning write to target
- [ ] Log all clone operations to the unified audit log

#### Block-Level Clone Mode

- [ ] Implement raw sector-by-sector copy from source to target drive
- [ ] Handle asymmetric cloning: smaller source to larger target (pad remaining space), larger source to smaller target (with user acknowledgment of data loss beyond target capacity)
- [ ] Support cloning to/from image files (not just drive-to-drive)
- [ ] Support compressed clone images (gzip, zstd) for reduced storage footprint
- [ ] Support encrypted clone images (AES-256) for secure data transport and compliance
- [ ] Support combined compression + encryption on image files
- [ ] Implement bandwidth throttling option (avoid saturating I/O on production machines)
- [ ] Show real-time progress: read throughput, write throughput, sectors copied, ETA
- [ ] Implement hash verification (compute hash of source and target after clone to confirm integrity)
- [ ] Support resume after interruption (track last successfully written offset)

#### Partition-Aware Clone Mode

- [ ] Read and understand GPT, MBR, and hybrid partition tables from source drive
- [ ] Clone individual partitions rather than raw sectors
- [ ] Automatically resize the target partition table to fit the target drive
- [ ] Handle the "System Reserved blocks expansion" case: detect blocking partitions, relocate them, expand the main data partition to fill available space
- [ ] Preserve partition GUIDs, attributes, and flags during clone
- [ ] Support selective partition cloning (clone only specific partitions, skip others)
- [ ] Validate target drive capacity against source partition data (warn if target is too small for the actual data, even if partition table says otherwise)

#### Clone Integration

- [ ] Add `drivewipe clone` CLI subcommand with source, target, and mode arguments
- [ ] Add clone workflow to TUI (screen designed in 1.5)
- [ ] Include clone operations in audit logging
- [ ] Generate clone reports (source/target info, hash verification result, partition changes made)

---

### 2.2 General Partition Manager

Implement a full partition management system supporting all major partition table types and maximum filesystem coverage.

#### Partition Table Operations

- [ ] Read, parse, and write GPT partition tables
- [ ] Read, parse, and write MBR partition tables
- [ ] Handle hybrid MBR/GPT layouts
- [ ] Create new partition tables (GPT or MBR) on blank drives
- [ ] Create new partitions (specify type, size, alignment)
- [ ] Delete partitions
- [ ] Resize partitions (grow and shrink)
- [ ] Move partitions (relocate start sector without data loss)
- [ ] Change partition type GUIDs/IDs
- [ ] Enforce proper partition alignment (4K / 1 MiB alignment for optimal performance on modern drives)
- [ ] Detect and warn about misaligned partitions
- [ ] Preview all changes before applying (dry-run mode)

#### Filesystem Support

Filesystem-aware operations (resize, move with data preservation) for as many filesystems as possible:

- [ ] **NTFS** — resize, move, create (covers Windows systems)
- [ ] **ext4 / ext3 / ext2** — resize, move, create (covers most Linux systems)
- [ ] **FAT32 / exFAT** — resize, move, create (covers USB drives, EFI partitions)
- [ ] **XFS** — grow only (XFS cannot shrink), move, create
- [ ] **Btrfs** — resize, move, create
- [ ] **APFS** — metadata awareness, read-only (APFS resize is macOS-managed)
- [ ] **HFS+** — metadata awareness, basic resize support
- [ ] **ZFS** — metadata awareness (ZFS manages its own partitioning)
- [ ] **ReFS** — metadata awareness where possible
- [ ] **UFS** — metadata awareness (FreeBSD/legacy Unix)
- [ ] **Linux swap** — create, resize
- [ ] **Raw/unknown** — byte-level partition move/copy without filesystem understanding (safe fallback for any unrecognized filesystem)

#### Partition Safety

- [ ] Require confirmation before any destructive operation (delete, resize shrink, move)
- [ ] Show a clear preview of what will change before applying (current state vs. proposed state)
- [ ] Refuse to modify partitions on the boot drive unless explicitly overridden
- [ ] Warn if a partition is currently mounted and require unmount before modification
- [ ] Support undo/rollback for partition table changes (save backup of original table before applying)
- [ ] Log all partition operations to the unified audit log

#### Partition Manager Integration

- [ ] Add `drivewipe partition` CLI subcommand family (list, create, delete, resize, move)
- [ ] Add partition manager to TUI (screen designed in 1.5)

---

### 2.3 Forensic Drive Evaluation Toolkit

Implement a comprehensive forensic analysis system for pre-wipe and post-wipe drive evaluation.

#### Analysis Capabilities

- [ ] **Entropy analysis** — scan the full drive surface and generate a per-sector entropy score; high entropy = encrypted or random data, low entropy = zeros or patterns, medium entropy = likely contains real data
- [ ] **Entropy heatmap** — visual representation of entropy distribution across the drive surface (for TUI and reports)
- [ ] **Sector sampling** — statistical random sampling of sectors to estimate data remnant percentage without reading the entire drive
- [ ] **File signature scanning** — scan for known file headers/magic bytes (JPEG, PDF, DOCX, EXE, ZIP, etc.) to detect recoverable file fragments
- [ ] **Deleted file detection** — parse filesystem metadata (where readable) to identify files marked as deleted but not overwritten
- [ ] **Slack space analysis** — detect data in filesystem slack space (partial sectors at end of file allocations)
- [ ] **Hidden partition detection** — scan for partition tables, filesystem headers, and boot signatures outside of declared partition boundaries
- [ ] **HPA/DCO analysis** — detect and report on Host Protected Area and Device Configuration Overlay regions (existing detection expanded)
- [ ] **Firmware region awareness** — identify and report on regions that software cannot access (SSD over-provisioned area, NVMe controller-reserved space)

#### Forensic Reporting

- [ ] Generate operator-facing analysis results (displayed in TUI, included in standard reports)
- [ ] Generate formal forensic reports suitable for audit/legal/compliance:
  - Timestamps with timezone info
  - Hash chains for integrity verification
  - Sector sampling methodology description
  - Statistical confidence levels for data remnant estimates
  - Drive identification (serial, model, firmware, capacity)
  - Analyst/operator identification
  - Chain-of-custody fields (who ran the analysis, when, where)
- [ ] Export forensic reports as PDF and JSON
- [ ] Export in forensic interchange formats compatible with industry tools:
  - DFXML (Digital Forensics XML) for tool-agnostic data exchange
  - Hash sets compatible with NSRL / Autopsy / FTK
  - E01 (EnCase Evidence File) format for disk image export if applicable
  - Timeline output compatible with log2timeline/Plaso
- [ ] Include forensic analysis results in wipe completion reports (pre-wipe state and post-wipe verification)
- [ ] Log all forensic operations to the unified audit log

#### Forensic Integration

- [ ] Add `drivewipe forensic` CLI subcommand family (scan, report, compare)
- [ ] Add forensic analysis screen to TUI (designed in 1.5)
- [ ] Support running forensic analysis before a wipe (to document what was on the drive) and after a wipe (to verify destruction)
- [ ] Optionally auto-run a quick forensic scan as part of the wipe workflow (configurable)

---

## Phase 3 — Infrastructure & Polish

Polish, packaging, documentation, and the GUI foundation.

---

### 3.1 Documentation Overhaul

Complete rewrite and expansion of all documentation. User guides and API docs. This is a public repo and the docs should reflect that.

#### README Rewrite

- [ ] Rewrite the README to cover all new features (cloning, partitions, forensics, health, drive profiles)
- [ ] Add feature comparison table (DriveWipe vs. DBAN, nwipe, shred, etc.)
- [ ] Add screenshots/recordings of the TUI in action
- [ ] Add a clear "Why DriveWipe?" section
- [ ] Update the architecture diagram to include new crates/modules
- [ ] Add badges (build status, version, license, platforms)

#### User Documentation

- [ ] Installation guide for all platforms (Linux, macOS, Windows) including prerequisites
- [ ] Getting started / quickstart tutorial
- [ ] Wipe methods deep-dive: what each method does, when to use it, security guarantees and limitations
- [ ] Drive cloning guide with common scenarios (OS migration, drive upgrade, backup)
- [ ] Partition management guide
- [ ] Forensic analysis guide (what the results mean, how to interpret entropy data)
- [ ] Drive health guide (understanding SMART attributes, what the health report means)
- [ ] Drive profile system guide (how to use, how to contribute new profiles)
- [ ] Custom wipe method authoring guide
- [ ] Configuration reference (all config.toml options explained)
- [ ] CLI reference (all commands, all flags, with examples)
- [ ] TUI reference (all screens, all keybindings, navigation)
- [ ] Troubleshooting guide (common errors, platform-specific issues, FAQ)
- [ ] Safety and security model documentation (what DriveWipe guarantees, what it doesn't)

#### Developer Documentation

- [ ] Architecture overview (crate structure, trait design, data flow)
- [ ] Contributing guide update (cover new feature areas, profile contribution process)
- [ ] API reference (rustdoc for all public types and functions in drivewipe-core)
- [ ] Platform-specific implementation notes (Linux ioctls, macOS APIs, Windows APIs)
- [ ] Testing guide (how to run tests, how to test against real devices safely)

#### Documentation Tooling

- [ ] Choose and set up a documentation site generator (mdbook or similar)
- [ ] Configure doc generation as part of the build process
- [ ] Add documentation CI checks (broken links, formatting)

---

### 3.2 Build System & Installer

Make it trivially easy for anyone to build, install, and run DriveWipe on their machine after cloning the repo. Also produce distributable packages for releases.

#### Smart Build Script

- [ ] Create a build/setup script (or expand xtask) that:
  - Detects the host OS and architecture
  - Checks for Rust toolchain and installs it if missing (via rustup)
  - Checks for platform-specific dependencies and tells the user what to install
  - Builds optimized release binaries for the host platform
  - Places binaries in the appropriate system location (or offers to)
  - Generates a platform-native distributable:
    - `.deb` on Debian/Ubuntu
    - `.rpm` on Fedora/RHEL
    - `.pkg` or `.dmg` on macOS
    - `.msi` or portable `.zip` on Windows
  - Prints a clear "success, here's how to run it" message at the end
- [ ] Support a `--dev` flag for development builds (debug mode, skip packaging)
- [ ] Support a `--portable` flag that builds a self-contained directory with all binaries

#### GitHub Releases Pipeline

- [ ] Verify/update the existing GitHub Actions release workflow
- [ ] Build release binaries for all supported platforms (Linux x86_64/aarch64, macOS x86_64/aarch64, Windows x86_64)
- [ ] Produce distributable packages for each platform in release artifacts
- [ ] Generate checksums (SHA-256) for all release artifacts
- [ ] Auto-generate release notes from changelog

---

### 3.3 GUI Foundation (iced)

Build the foundational GUI application using iced. The goal is to get the architecture right and establish patterns that all future GUI features build on. The GUI covers the same features as the TUI but with a graphical interface.

#### Architecture

- [ ] Set up the iced application scaffold in `drivewipe-gui`
- [ ] Design the message/command pattern for communicating with `drivewipe-core` (async, non-blocking)
- [ ] Implement a core-to-GUI event bridge (same `ProgressEvent` channel the TUI uses)
- [ ] Design a modular screen/view system that mirrors the TUI screens but with graphical widgets
- [ ] Establish a consistent visual design language (colors, spacing, typography, component styles)

#### Core Screens

- [ ] Main menu / navigation
- [ ] Drive selection with drive info summary cards
- [ ] Wipe method selection with descriptions and recommendations
- [ ] Confirmation dialog with all safety warnings
- [ ] Wipe progress dashboard (progress bars, throughput graph, ETA, sector visualization)
- [ ] Drive health viewer
- [ ] Clone workflow
- [ ] Partition manager (visual partition layout)
- [ ] Forensic analysis viewer
- [ ] Settings/preferences
- [ ] Report viewer

#### Extensibility

- [ ] Design the GUI so new features can be added as new "pages" or "tabs" without restructuring
- [ ] Keep all business logic in `drivewipe-core` — the GUI is purely a presentation layer
- [ ] Ensure the GUI and TUI can coexist (same core API, different frontends)

---

### 3.4 Bootable Live Environment ✅

Create a bootable USB image that boots a minimal Linux environment straight into DriveWipe, enabling operators to wipe the boot drive (currently refused) and operate on machines without an installed OS.

> **Status**: Fully implemented. See [LIVE-ENVIRONMENT-PLAN.md](LIVE-ENVIRONMENT-PLAN.md) for details.

- [x] Select and configure a minimal Linux base (Alpine, Tiny Core, or custom initramfs)
- [x] Bundle DriveWipe TUI binary and all dependencies into the live image
- [x] Auto-launch the DriveWipe TUI on boot (no shell required for basic operation, shell accessible for advanced users)
- [x] Include necessary kernel modules for drive access (SATA, NVMe, USB mass storage, SCSI)
- [x] Include firmware blobs needed for common hardware (NVMe controllers, USB host controllers)
- [x] Support both UEFI and legacy BIOS boot
- [x] Support writing the image to USB drives (provide a tool or instructions)
- [x] Include network support (optional, for sending reports or notifications)
- [x] Keep the image as small as possible (target: under 200 MB)
- [x] Document the bootable image creation process for contributors who want to customize it
- [x] Add build automation for generating the bootable image (script or xtask command)

Additional items implemented beyond original plan:
- [x] Custom kernel module for direct ATA/NVMe passthrough, HPA/DCO manipulation, DMA I/O
- [x] PXE network boot infrastructure (dnsmasq, iPXE menu with Normal/Safe/Serial modes)
- [x] 4 new TUI screens with full interactive handlers (Live Dashboard, HPA/DCO Manager, ATA Security, Kernel Status)
- [x] `drivewipe-live` Rust crate with capabilities probing and SG_IO fallbacks
- [x] GitHub Actions live ISO build job with release categorization

---

## Testing Strategy

A dedicated section covering how all new features should be tested.

---

### Mock / Simulation Infrastructure

- [ ] Create a `MockDrive` implementation of `RawDeviceIo` that operates on in-memory buffers or temp files — used by all tests that need a "drive" without touching real hardware
- [ ] Create a `MockDriveEnumerator` that returns configurable fake `DriveInfo` structs for testing drive detection, profile matching, and UI rendering
- [ ] Build a `DriveSimulator` that can emulate drive behaviors: slow sectors, thermal throttling, SLC cache exhaustion, firmware command responses — for testing the time estimation system and wipe engine edge cases
- [ ] Create mock SMART data generators for testing the health system without real drives

### Per-Feature Testing Notes

- [ ] **Drive Health**: Test SMART parsing against known-good SMART data dumps from real drives. Verify diff engine catches degradation correctly. Test on mock data for all SMART attribute types.
- [ ] **Custom Wipe Method**: Verify each step of the multi-strategy chain executes in order. Test fallback behavior when firmware commands aren't available. Verify against `MockDrive` that the full surface is covered.
- [ ] **Drive Profiles**: Test auto-detection matching against a corpus of real model strings. Verify fallback to generic profile for unknown drives. Test profile loading and validation.
- [ ] **Time Estimates**: Feed historical throughput data into the estimator and verify predictions against known outcomes. Test multi-pass ETA calculation. Test confidence interval generation.
- [ ] **Drive Cloning**: Clone mock drives of different sizes (both directions). Verify byte-perfect copies via hash comparison. Test interrupt/resume during clone. Test partition-aware cloning with various partition table layouts.
- [ ] **Partition Manager**: Test GPT and MBR parsing against known partition table binary dumps. Verify resize calculations don't overlap partitions. Test move operations preserve data. Dry-run mode should never modify the mock drive.
- [ ] **Forensic Toolkit**: Test entropy calculation against buffers with known entropy (all zeros, all random, mixed). Test file signature scanning against buffers containing known magic bytes. Verify sector sampling statistics converge to known values with sufficient samples.
- [ ] **TUI**: Use `ratatui::backend::TestBackend` for render snapshot tests on all new screens. Test state machine transitions for all new workflows (clone, forensics, partition management). Test keyboard navigation paths.
- [ ] **GUI**: Test message handling and state management independently from rendering. Verify core-to-GUI event bridge delivers all event types correctly.
- [ ] **Build Script**: Test the build script on clean VMs/containers for each supported platform. Verify it detects missing dependencies correctly.
- [ ] **Bootable USB**: Test the live image boots successfully in QEMU/VirtualBox for both UEFI and BIOS modes. Verify DriveWipe TUI launches automatically. Test drive detection within the live environment.
- [ ] **Notifications**: Test notifications fire on all three platforms. Verify they display correct information. Test enable/disable config toggle.
- [ ] **Sleep Prevention**: Verify sleep inhibitor activates on operation start and releases on completion/cancellation across all platforms. Test that the system does not sleep during a mock long-running operation.
- [ ] **Kitty Cat Keyboard Mode**: Test that all keypresses are ignored when locked except the unlock sequence. Test that the unlock sequence correctly re-enables input. Test the visual lock indicator renders correctly.

### CI Considerations

- [ ] All mock-based tests run in CI on all platforms (Linux, macOS, Windows)
- [ ] Real device tests remain gated behind `--features real-device-tests` + `DRIVEWIPE_TEST_DEVICE` env var — never in CI
- [ ] Add integration test suite that runs full wipe/clone/forensic workflows against temp file "drives"
- [ ] Add documentation build and link checking to CI
- [ ] Add clippy and formatting checks for all new code

---

## Summary Checklist (Quick Reference)

| #  | Feature                                                              | Phase   | Status |
|----|----------------------------------------------------------------------|---------|--------|
| 1  | Drive health system (SMART, benchmarks, diff reports)                | Phase 1 | ✅ |
| 2  | Custom high-security wipe method ("DriveWipe Secure")                | Phase 1 | ✅ |
| 3  | Drive profile system (auto-detect, community-contributed)            | Phase 1 | ✅ |
| 4  | Time estimate full overhaul                                          | Phase 1 | ✅ |
| 5  | TUI overhaul (framework research + all new screens)                  | Phase 1 | 🔶 |
| 6  | Unified audit log expansion (typed events for all operations)        | Phase 1 | ✅ |
| 7  | Sleep prevention / keep-alive (OS-native)                            | Phase 1 | ✅ |
| 8  | Kitty Cat Keyboard Mode (keyboard lock)                              | Phase 1 | ✅ |
| 9  | Desktop notification system (OS-native)                              | Phase 1 | ✅ |
| 10 | Crate structure evolution (new core modules)                         | Phase 1 | ✅ |
| 11 | Drive cloning (block-level + partition-aware, compressed, encrypted) | Phase 2 | ✅ |
| 12 | General partition manager (GPT/MBR, max filesystem coverage)         | Phase 2 | ✅ |
| 13 | Forensic drive evaluation toolkit (full analysis + formal reports)   | Phase 2 | ✅ |
| 14 | Documentation overhaul (user guides + API docs + doc site)           | Phase 3 | 🔶 |
| 15 | Build system & installer (smart script + GitHub releases)            | Phase 3 | ✅ |
| 16 | GUI foundation (iced, modular, extensible)                           | Phase 3 | 🔶 |
| 17 | Bootable live USB environment                                        | Phase 3 | ✅ |
