# Installation

DriveWipe ships as a single `drivewipe` binary containing all three interfaces.

## Quick install (Linux and macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/KodyDennon/DriveWipe/main/install.sh | sh
```

The installer detects your platform and architecture, downloads the matching
release, **verifies it against the published SHA-256 checksum before installing
anything**, and places the binary in `/usr/local/bin` — or `~/.local/bin` if you
are not root. On Linux it also adds a desktop entry so DriveWipe appears in your
application menu.

Options:

| Flag | Effect |
|---|---|
| `--version X.Y.Z` | Install a specific release instead of the latest |
| `--prefix DIR` | Install under `DIR/bin` instead of the default |
| `--uninstall` | Remove the binary, symlinks and desktop entry |
| `--no-verify` | Skip checksum verification (not recommended) |

To uninstall:

```bash
curl -fsSL https://raw.githubusercontent.com/KodyDennon/DriveWipe/main/install.sh | sh -s -- --uninstall
```

Your configuration and reports under `~/.config/drivewipe` and
`~/.local/share/drivewipe` are left alone.

## Manual install

Download an archive for your platform from the
[Releases](https://github.com/KodyDennon/DriveWipe/releases) page — Linux, macOS
and Windows, x86_64 and ARM64 — then either run the bundled `install.sh` or copy
the `drivewipe` binary somewhere on your `PATH`.

Verify your download first:

```bash
sha256sum -c SHA256SUMS.txt
```

## Building from source

### Prerequisites

- **Rust 1.94+** (2024 edition) — install via [rustup](https://rustup.rs/)
- **Root/Administrator privileges** — to access raw devices, not to build

**Linux:** `libudev-dev` (Debian/Ubuntu) or `systemd-devel` (Fedora/RHEL) for
drive enumeration, and D-Bus for notifications and sleep prevention.

**macOS:** Xcode Command Line Tools (`xcode-select --install`). Optionally
`nvme-cli` via Homebrew for NVMe firmware commands.

**Windows:** Visual Studio Build Tools with the C++ workload.

### Build

```bash
git clone https://github.com/KodyDennon/DriveWipe.git
cd DriveWipe
cargo build --release
```

The binary lands at `target/release/drivewipe`.

For a server or container image with no desktop interface compiled in:

```bash
cargo build --release --package drivewipe-cli \
  --no-default-features --features pdf-report
```

### Install

```bash
cargo install --path crates/drivewipe-cli
```

## Verifying the installation

```bash
drivewipe --version
```

## Choosing an interface

The same binary serves all three; it decides from how you start it:

```bash
drivewipe            # terminal UI
drivewipe --gui      # desktop window
drivewipe list       # command line
```

The installer also creates `drivewipe-tui` and `drivewipe-gui` symlinks, which
open those interfaces directly.

## Configuration

On first run, DriveWipe creates a default configuration at:

- Linux/macOS: `~/.config/drivewipe/config.toml`
- Windows: `%APPDATA%\drivewipe\config.toml`

See [config-reference.md](config-reference.md) for all options.
