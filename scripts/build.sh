#!/usr/bin/env bash
set -euo pipefail

# DriveWipe Build Script
# Usage: ./scripts/build.sh [--dev] [--portable] [--no-gui]

DEV_MODE=false
PORTABLE=false
NO_GUI=false
INSTALL=false

for arg in "$@"; do
    case "$arg" in
        --dev)     DEV_MODE=true ;;
        --portable) PORTABLE=true ;;
        --no-gui)  NO_GUI=true ;;
        --install) INSTALL=true ;;
        --help|-h)
            echo "Usage: ./scripts/build.sh [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --dev       Build in debug mode (faster compilation)"
            echo "  --portable  Build with static linking where possible"
            echo "  --no-gui    Skip building the GUI (avoids iced dependency)"
            echo "  --install   Install binaries to cargo bin directory"
            echo "  --help      Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $arg"
            exit 1
            ;;
    esac
done

echo "=== DriveWipe Build ==="
echo ""

# Check Rust toolchain
if ! command -v cargo &>/dev/null; then
    echo "ERROR: Rust toolchain not found."
    echo "Install from https://rustup.rs/"
    exit 1
fi

RUST_VERSION=$(rustc --version | awk '{print $2}')
echo "Rust version: $RUST_VERSION"

# Check minimum version (1.85)
MIN_MAJOR=1
MIN_MINOR=85
MAJOR=$(echo "$RUST_VERSION" | cut -d. -f1)
MINOR=$(echo "$RUST_VERSION" | cut -d. -f2)
if [ "$MAJOR" -lt "$MIN_MAJOR" ] || ([ "$MAJOR" -eq "$MIN_MAJOR" ] && [ "$MINOR" -lt "$MIN_MINOR" ]); then
    echo "ERROR: Rust $MIN_MAJOR.$MIN_MINOR+ required (found $RUST_VERSION)"
    echo "Run: rustup update"
    exit 1
fi

# Detect OS and architecture
OS=$(uname -s)
ARCH=$(uname -m)
echo "Platform: $OS $ARCH"

# Build profile
if [ "$DEV_MODE" = true ]; then
    PROFILE=""
    PROFILE_NAME="debug"
    echo "Mode: Development (debug)"
else
    PROFILE="--release"
    PROFILE_NAME="release"
    echo "Mode: Release"
fi

echo ""

# Build CLI
echo "Building drivewipe (CLI)..."
cargo build $PROFILE --package drivewipe-cli
echo "  OK"

# Build TUI
echo "Building drivewipe-tui (Terminal UI)..."
cargo build $PROFILE --package drivewipe-tui
echo "  OK"

# Build GUI (optional)
if [ "$NO_GUI" = false ]; then
    echo "Building drivewipe-gui (Graphical UI)..."
    cargo build $PROFILE --package drivewipe-gui
    echo "  OK"
else
    echo "Skipping GUI build (--no-gui)"
fi

echo ""

# Show binary locations
echo "Binaries:"
echo "  CLI: target/$PROFILE_NAME/drivewipe"
echo "  TUI: target/$PROFILE_NAME/drivewipe-tui"
if [ "$NO_GUI" = false ]; then
    echo "  GUI: target/$PROFILE_NAME/drivewipe-gui"
fi

# Install
if [ "$INSTALL" = true ]; then
    echo ""
    echo "Installing..."
    cargo install --path crates/drivewipe-cli
    cargo install --path crates/drivewipe-tui
    if [ "$NO_GUI" = false ]; then
        cargo install --path crates/drivewipe-gui
    fi
    echo "Installed to $(dirname $(which cargo))"
fi

echo ""
echo "=== Build complete ==="
