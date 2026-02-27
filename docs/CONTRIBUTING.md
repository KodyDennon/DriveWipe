# Contributing to DriveWipe

Contributions are welcome from all individuals and organizations.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/DriveWipe.git`
3. Create a feature branch: `git checkout -b feature/my-feature`
4. Make your changes
5. Run checks: `cargo fmt && cargo clippy && cargo test`
6. Commit and push
7. Open a Pull Request

## Development Setup

### Prerequisites

- Rust 1.85+ (install via [rustup](https://rustup.rs))
- For PDF report generation: the `pdf-report` feature requires fonts (LiberationSans or similar)
- Root/Administrator access for real device testing

### Build

```bash
# Build all crates
cargo build --workspace

# Build with PDF support
cargo build --workspace --features drivewipe-core/pdf-report

# Run tests
cargo test --workspace

# Run clippy
cargo clippy --workspace -- -D warnings

# Format code
cargo fmt --all
```

### Testing

- **Unit tests**: `cargo test --workspace`
- **Integration tests**: `cargo test --workspace -- --include-ignored` (some tests require temp files)
- **Real device tests**: `DRIVEWIPE_TEST_DEVICE=/dev/sdX cargo test --features real-device-tests` (DANGEROUS - only on disposable drives)

## Code Style

- Follow `rustfmt` defaults (see `rustfmt.toml`)
- Use `thiserror` for error types in the core crate
- Use `anyhow` for error handling in CLI/TUI crates
- Prefer `log` macros over `println!` for diagnostic output
- All public API items should have doc comments

## Architecture

The project is a Cargo workspace with four crates:

- **drivewipe-core**: Library with all business logic. No UI code.
- **drivewipe-cli**: CLI binary consuming the core API.
- **drivewipe-tui**: Terminal UI binary consuming the core API.
- **drivewipe-gui**: GUI binary (Phase 2, not yet implemented).

When adding features, put the logic in `drivewipe-core` and the presentation in the appropriate UI crate.

## Safety

This tool performs **irreversible data destruction**. When contributing:

- Never remove or weaken safety checks (boot drive detection, confirmation flows)
- All new wipe operations must go through the confirmation system
- Test with files, not real drives, unless you know what you're doing
- Document any platform-specific limitations

## License

By contributing, you agree that your contributions will be distributed under the project's license (see LICENSE.md).
