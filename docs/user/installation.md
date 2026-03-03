# Installation

## Prerequisites

- **Rust 1.85+** (2024 edition) — install via [rustup](https://rustup.rs/)
- **Root/Administrator privileges** — required for raw device access on all platforms

### Platform-specific dependencies

**Linux:**
- `libudev-dev` (Debian/Ubuntu) or `systemd-devel` (Fedora/RHEL) for drive enumeration
- D-Bus for notifications and sleep prevention

**macOS:**
- Xcode Command Line Tools (`xcode-select --install`)
- Optional: `nvme-cli` via Homebrew for NVMe firmware commands

**Windows:**
- Visual Studio Build Tools with C++ workload
- Administrator privileges

## Building from Source

```bash
git clone https://github.com/your-org/drivewipe.git
cd drivewipe
cargo build --release
```

Binaries will be in `target/release/`:
- `drivewipe` — CLI
- `drivewipe-tui` — Terminal UI
- `drivewipe-gui` — Graphical UI

## Installing

```bash
# Install all binaries
cargo install --path crates/drivewipe-cli
cargo install --path crates/drivewipe-tui
cargo install --path crates/drivewipe-gui
```

## Verifying Installation

```bash
drivewipe --version
drivewipe-tui --version
drivewipe-gui --version
```

## Configuration

On first run, DriveWipe creates a default configuration at:
- Linux/macOS: `~/.config/drivewipe/config.toml`
- Windows: `%APPDATA%\drivewipe\config.toml`

See [config-reference.md](config-reference.md) for all options.
