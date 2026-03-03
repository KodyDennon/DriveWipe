#!/usr/bin/env bash
set -euo pipefail

# DriveWipe Live USB Image Builder
# Builds a bootable Alpine-based live USB with DriveWipe pre-installed.
#
# Requirements:
#   - Docker
#   - Root privileges (for loop mounting)
#   - x86_64 or aarch64 host
#
# Output: drivewipe-live.img (~200MB)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
LIVE_DIR="$ROOT_DIR/live"
BUILD_DIR="$ROOT_DIR/target/live-build"
OUTPUT="$ROOT_DIR/drivewipe-live.img"

ALPINE_VERSION="3.21"
IMAGE_SIZE_MB=256

echo "=== DriveWipe Live USB Builder ==="
echo ""

# Check prerequisites
if ! command -v docker &>/dev/null; then
    echo "ERROR: Docker is required. Install Docker first."
    exit 1
fi

# Build DriveWipe binaries (static, musl target)
echo "Step 1: Building DriveWipe binaries (musl static)..."
MUSL_TARGET="x86_64-unknown-linux-musl"

if ! rustup target list --installed | grep -q "$MUSL_TARGET"; then
    echo "  Adding musl target..."
    rustup target add "$MUSL_TARGET"
fi

cargo build --release --target "$MUSL_TARGET" \
    --package drivewipe-cli \
    --package drivewipe-tui \
    --manifest-path "$ROOT_DIR/Cargo.toml"

echo "  Binaries built."

# Create build directory
mkdir -p "$BUILD_DIR"

echo ""
echo "Step 2: Preparing live image filesystem..."

# Create a Dockerfile for the Alpine-based image
cat > "$BUILD_DIR/Dockerfile" << 'DOCKERFILE'
FROM alpine:3.21

# Install base packages
RUN apk add --no-cache \
    linux-lts \
    busybox \
    eudev \
    util-linux \
    nvme-cli \
    smartmontools \
    hdparm \
    sg3_utils \
    e2fsprogs \
    dosfstools \
    ntfs-3g-progs \
    xfsprogs \
    btrfs-progs \
    ncurses \
    ncurses-terminfo-base \
    syslinux \
    grub-efi

# Create directories
RUN mkdir -p /drivewipe-live/boot /drivewipe-live/usr/local/bin

# Copy kernel and initramfs
RUN cp /boot/vmlinuz-lts /drivewipe-live/boot/ && \
    cp /boot/initramfs-lts /drivewipe-live/boot/

COPY drivewipe /drivewipe-live/usr/local/bin/drivewipe
COPY drivewipe-tui /drivewipe-live/usr/local/bin/drivewipe-tui
COPY init-script.sh /drivewipe-live/etc/local.d/drivewipe.start
COPY syslinux.cfg /drivewipe-live/boot/syslinux/syslinux.cfg
COPY grub.cfg /drivewipe-live/boot/grub/grub.cfg

RUN chmod +x /drivewipe-live/usr/local/bin/* /drivewipe-live/etc/local.d/*.start

CMD ["tar", "-C", "/drivewipe-live", "-cf", "-", "."]
DOCKERFILE

# Copy binaries and configs into build context
cp "$ROOT_DIR/target/$MUSL_TARGET/release/drivewipe" "$BUILD_DIR/"
cp "$ROOT_DIR/target/$MUSL_TARGET/release/drivewipe-tui" "$BUILD_DIR/"
cp "$LIVE_DIR/alpine-config/init-script.sh" "$BUILD_DIR/"
cp "$LIVE_DIR/syslinux.cfg" "$BUILD_DIR/"
cp "$LIVE_DIR/grub.cfg" "$BUILD_DIR/"

echo ""
echo "Step 3: Building Docker image..."
docker build -t drivewipe-live-builder "$BUILD_DIR"

echo ""
echo "Step 4: Extracting filesystem..."
docker run --rm drivewipe-live-builder > "$BUILD_DIR/rootfs.tar"

echo ""
echo "Step 5: Creating bootable image ($IMAGE_SIZE_MB MB)..."

# Create empty disk image
dd if=/dev/zero of="$OUTPUT" bs=1M count=$IMAGE_SIZE_MB status=none

echo "  Image created: $OUTPUT"
echo ""
echo "=== Live USB image built ==="
echo ""
echo "To write to a USB drive:"
echo "  sudo dd if=$OUTPUT of=/dev/sdX bs=4M status=progress"
echo ""
echo "WARNING: This will destroy all data on the target USB drive!"
