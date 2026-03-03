# Contributing to DriveWipe

Contributions are welcome from all individuals and organizations.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/KodyDennon/DriveWipe.git`
3. Create a feature branch: `git checkout -b feature/my-feature`
4. Make your changes
5. Run checks: `cargo fmt && cargo clippy --workspace --all-features -- -D warnings && cargo test --workspace`
6. Commit using [scoped conventional commits](#commit-messages)
7. Push and open a Pull Request

## Development Setup

### Prerequisites

- Rust 1.85+ (install via [rustup](https://rustup.rs))
- For PDF report generation: the `pdf-report` feature requires fonts (LiberationSans or similar)
- For live environment: Docker (for building the live image and kernel module)
- Root/Administrator access for real device testing

### Build

```bash
# Build all crates
cargo build --workspace

# Build with PDF support
cargo build --workspace --features drivewipe-core/pdf-report

# Build TUI with live features (Linux only)
cargo build --package drivewipe-tui --features live

# Build the live USB image
cargo xtask live-build

# Run tests
cargo test --workspace

# Run clippy
cargo clippy --workspace --all-features -- -D warnings

# Format code
cargo fmt --all
```

### Testing

- **Unit tests**: `cargo test --workspace`
- Current version: **v1.1.2**
- **Integration tests**: `cargo test --workspace -- --include-ignored` (some tests require temp files)
- **Live crate tests**: `cargo test -p drivewipe-live`
- **Real device tests**: `DRIVEWIPE_TEST_DEVICE=/dev/sdX cargo test --features real-device-tests` (DANGEROUS — only on disposable drives)

## Commit Messages

Use **scoped conventional commits** so the automated versioning system bumps the correct crate:

```
feat(core): add NVMe secure erase support    → Minor bump for drivewipe-core
fix(cli): resolve progress bar flicker       → Patch bump for drivewipe-cli
feat(tui): new dashboard view                → Minor bump for drivewipe-tui
feat(live): add DCO freeze support           → Minor bump for drivewipe-live
fix(gui): correct window resize              → Patch bump for drivewipe-gui
```

See [DEVELOPMENT.md](DEVELOPMENT.md) for the full versioning contract.

## Code Style

- Follow `rustfmt` defaults (see `rustfmt.toml`)
- Use `thiserror` for error types in the core crate
- Use `anyhow` for error handling in CLI/TUI crates
- Prefer `log` macros over `println!` for diagnostic output
- All public API items should have doc comments
- Use `#[cfg(feature = "...")]` gating for optional features

## Architecture

The project is a Cargo workspace with six crates:

| Crate | Purpose |
|---|---|
| **drivewipe-core** | Library with all business logic. No UI code. |
| **drivewipe-cli** | CLI binary consuming the core API. |
| **drivewipe-tui** | Terminal UI binary consuming the core API. |
| **drivewipe-gui** | GUI binary (iced framework). |
| **drivewipe-live** | Live environment hardware access (HPA/DCO, ATA security, kernel module). |
| **xtask** | Build automation tasks (bump, release, live-build). |

When adding features, put the logic in `drivewipe-core` and the presentation in the appropriate UI crate. Live environment hardware features go in `drivewipe-live`.

### Cross-Platform Development

All platform-specific code uses `#[cfg(target_os = "...")]` gating. When adding platform-specific features:

- Put Linux, macOS, and Windows implementations in separate `#[cfg]` blocks or modules
- Ensure the code compiles cleanly on all platforms (even if a feature returns `PlatformNotSupported` on some)
- Keep platform-specific constants inside `#[cfg]`-gated modules to avoid dead code warnings
- Test with `cargo build` on your development platform; CI will verify all three platforms
- The `drivewipe-live` crate is Linux-only — use `#[cfg(target_os = "linux")]` appropriately

### Drive Profiles

Community-contributed drive profiles live in `profiles/`. To add a new profile:

1. Create a TOML file in `profiles/` (see existing profiles for format)
2. Include manufacturer, model regex, controller type, and recommended wipe strategy
3. Test with `cargo test -p drivewipe-core` to verify profile loading and matching
4. Submit a PR with real-world model strings you've tested against

## Safety

This tool performs **irreversible data destruction**. When contributing:

- Never remove or weaken safety checks (boot drive detection, confirmation flows)
- All new wipe operations must go through the confirmation system
- Test with files, not real drives, unless you know what you're doing
- Document any platform-specific limitations
- Destructive live operations (HPA removal, DCO restore) must use the confirmation flow

## License

By contributing, you agree that your contributions will be distributed under the project's license (see LICENSE.md).
