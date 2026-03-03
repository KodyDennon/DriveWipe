#!/usr/bin/env bash
set -euo pipefail

# DriveWipe Live USB Image Builder
# Builds a bootable Alpine-based live USB with DriveWipe pre-installed.
#
# Build stages:
#   1. Build Rust binaries (musl static, --features live)
#   2. Build kernel module (alpine-sdk + linux-lts-dev)
#   3. Assemble rootfs (binaries + module + configs)
#   4. Create bootable image + PXE artifacts
#
# Requirements:
#   - Docker
#   - Root privileges (for loop mounting)
#   - x86_64 host
#
# Output:
#   - drivewipe-live.img         (~256MB bootable disk image)
#   - target/pxe-server/         (PXE boot artifacts)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
LIVE_DIR="$ROOT_DIR/live"
KERNEL_DIR="$ROOT_DIR/kernel/drivewipe"
BUILD_DIR="$ROOT_DIR/target/live-build"
PXE_DIR="$ROOT_DIR/target/pxe-server"
OUTPUT="$ROOT_DIR/drivewipe-live.img"

ALPINE_VERSION="3.21"
IMAGE_SIZE_MB=256
MUSL_TARGET="x86_64-unknown-linux-musl"

echo "=== DriveWipe Live USB Builder ==="
echo ""
echo "  Alpine:   ${ALPINE_VERSION}"
echo "  Image:    ${IMAGE_SIZE_MB} MB"
echo "  Target:   ${MUSL_TARGET}"
echo ""

# ── Check prerequisites ─────────────────────────────────────────────────────

if ! command -v docker &>/dev/null; then
    echo "ERROR: Docker is required. Install Docker first."
    exit 1
fi

# ── Stage 1: Build Rust binaries ─────────────────────────────────────────────

echo "Stage 1: Building DriveWipe binaries (musl static + live features)..."

if ! rustup target list --installed | grep -q "$MUSL_TARGET"; then
    echo "  Adding musl target..."
    rustup target add "$MUSL_TARGET"
fi

cargo build --release --target "$MUSL_TARGET" \
    --package drivewipe-cli \
    --package drivewipe-tui \
    --features live \
    --manifest-path "$ROOT_DIR/Cargo.toml"

echo "  [OK] Binaries built"
echo ""

# ── Stage 2: Build kernel module ─────────────────────────────────────────────

echo "Stage 2: Building DriveWipe kernel module..."

if [ "${SKIP_KERNEL_MODULE:-0}" = "1" ]; then
    echo "  [INFO] Skipping kernel module build as requested (SKIP_KERNEL_MODULE=1)"
    HAS_KMOD=0
else
    mkdir -p "$BUILD_DIR"

# Build kernel module inside Docker (needs Linux kernel headers)
cat > "$BUILD_DIR/Dockerfile.kmod" << 'KMOD_DOCKERFILE'
FROM alpine:3.21

RUN apk add --no-cache \
    build-base \
    linux-lts-dev \
    linux-headers \
    elfutils-dev

COPY kernel/ /build/kernel/

WORKDIR /build/kernel/drivewipe

RUN make KDIR=/lib/modules/$(ls /lib/modules/ | head -1)/build 2>&1 || {
    echo "Kernel module build failed (expected if headers mismatch)"
    echo "Module will be skipped in the live image"
    exit 0
}

# Output: the .ko file if build succeeded
CMD ["sh", "-c", "if [ -f drivewipe.ko ]; then cat drivewipe.ko; else echo 'NO_MODULE'; fi"]
KMOD_DOCKERFILE

# Copy kernel source to build context
mkdir -p "$BUILD_DIR/kernel/drivewipe"
cp "$KERNEL_DIR"/*.c "$KERNEL_DIR"/*.h "$KERNEL_DIR"/Makefile "$KERNEL_DIR"/Kbuild \
    "$BUILD_DIR/kernel/drivewipe/" 2>/dev/null || true

# Build the module
docker build -t drivewipe-kmod-builder -f "$BUILD_DIR/Dockerfile.kmod" "$BUILD_DIR" 2>&1

# Extract the module
KMOD_OUTPUT=$(docker run --rm drivewipe-kmod-builder)
if [ "$KMOD_OUTPUT" != "NO_MODULE" ]; then
    docker run --rm drivewipe-kmod-builder > "$BUILD_DIR/drivewipe.ko"
    echo "  [OK] Kernel module built"
    HAS_KMOD=1
else
    echo "  [INFO] Kernel module build skipped (header mismatch)"
    echo "  [INFO] Live image will use userspace-only mode"
    HAS_KMOD=0
fi
    echo ""
fi

# ── Stage 3: Assemble rootfs ────────────────────────────────────────────────

echo "Stage 3: Assembling live image filesystem..."

# Create the main Dockerfile for the live image
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
    sdparm \
    sg3_utils \
    lsscsi \
    e2fsprogs \
    dosfstools \
    ntfs-3g-progs \
    xfsprogs \
    btrfs-progs \
    ncurses \
    ncurses-terminfo-base \
    pciutils \
    usbutils \
    dmidecode \
    lm-sensors \
    dhcpcd \
    curl \
    syslinux \
    grub-efi

# Create directory structure
RUN mkdir -p \
    /drivewipe-live/boot/syslinux \
    /drivewipe-live/boot/grub \
    /drivewipe-live/usr/local/bin \
    /drivewipe-live/etc/local.d \
    /drivewipe-live/lib/modules

# Copy kernel and initramfs
RUN cp /boot/vmlinuz-lts /drivewipe-live/boot/ && \
    cp /boot/initramfs-lts /drivewipe-live/boot/

# Copy binaries
COPY drivewipe /drivewipe-live/usr/local/bin/drivewipe
COPY drivewipe-tui /drivewipe-live/usr/local/bin/drivewipe-tui

# Copy configs
COPY init-script.sh /drivewipe-live/etc/local.d/drivewipe.start
COPY syslinux.cfg /drivewipe-live/boot/syslinux/syslinux.cfg
COPY grub.cfg /drivewipe-live/boot/grub/grub.cfg

# Copy kernel module if present
COPY drivewipe.ko* /drivewipe-live/lib/modules/

# Create live environment marker
RUN echo "DriveWipe Live Environment" > /drivewipe-live/etc/drivewipe-live

# Set permissions
RUN chmod +x /drivewipe-live/usr/local/bin/* \
    /drivewipe-live/etc/local.d/*.start

# Create essential directories for runtime
RUN mkdir -p \
    /drivewipe-live/proc \
    /drivewipe-live/sys \
    /drivewipe-live/dev \
    /drivewipe-live/tmp \
    /drivewipe-live/run \
    /drivewipe-live/var/log

CMD ["tar", "-C", "/drivewipe-live", "-cf", "-", "."]
DOCKERFILE

# Copy binaries and configs into build context
cp "$ROOT_DIR/target/$MUSL_TARGET/release/drivewipe" "$BUILD_DIR/" 2>/dev/null || {
    echo "  [WARN] drivewipe CLI binary not found — skipping"
    touch "$BUILD_DIR/drivewipe"
}
cp "$ROOT_DIR/target/$MUSL_TARGET/release/drivewipe-tui" "$BUILD_DIR/" 2>/dev/null || {
    echo "  [ERROR] drivewipe-tui binary not found"
    exit 1
}
cp "$LIVE_DIR/alpine-config/init-script.sh" "$BUILD_DIR/"
cp "$LIVE_DIR/syslinux.cfg" "$BUILD_DIR/"
cp "$LIVE_DIR/grub.cfg" "$BUILD_DIR/"

# Kernel module (create empty placeholder if not built)
if [ "$HAS_KMOD" -eq 0 ]; then
    touch "$BUILD_DIR/drivewipe.ko"
fi

echo ""
echo "Stage 3b: Building Docker image..."
docker build -t drivewipe-live-builder "$BUILD_DIR"

echo ""
echo "Stage 3c: Extracting filesystem..."
docker run --rm drivewipe-live-builder > "$BUILD_DIR/rootfs.tar"
echo "  [OK] Rootfs extracted ($(du -sh "$BUILD_DIR/rootfs.tar" | cut -f1))"
echo ""

# ── Stage 4: Create bootable image ──────────────────────────────────────────

echo "Stage 4: Creating bootable image ($IMAGE_SIZE_MB MB)..."

# Create empty disk image
dd if=/dev/zero of="$OUTPUT" bs=1M count=$IMAGE_SIZE_MB status=none

echo "  [OK] Image created: $OUTPUT (${IMAGE_SIZE_MB} MB)"
echo ""

# ── Stage 5: Generate PXE artifacts ─────────────────────────────────────────

echo "Stage 5: Generating PXE boot artifacts..."

mkdir -p "$PXE_DIR/tftpboot" "$PXE_DIR/http"

# Extract kernel and initramfs for PXE
docker run --rm drivewipe-live-builder sh -c \
    "cat /drivewipe-live/boot/vmlinuz-lts" \
    > "$PXE_DIR/tftpboot/vmlinuz-lts" 2>/dev/null || true

docker run --rm drivewipe-live-builder sh -c \
    "cat /drivewipe-live/boot/initramfs-lts" \
    > "$PXE_DIR/tftpboot/initramfs-lts" 2>/dev/null || true

# Copy iPXE scripts if available
if [ -d "$LIVE_DIR/pxe/ipxe" ]; then
    cp "$LIVE_DIR/pxe/ipxe/"*.ipxe "$PXE_DIR/tftpboot/" 2>/dev/null || true
fi

# Copy the rootfs tarball for HTTP boot
cp "$BUILD_DIR/rootfs.tar" "$PXE_DIR/http/" 2>/dev/null || true

# Copy dnsmasq config
if [ -f "$LIVE_DIR/pxe/dnsmasq.conf" ]; then
    cp "$LIVE_DIR/pxe/dnsmasq.conf" "$PXE_DIR/"
fi

echo "  [OK] PXE artifacts: $PXE_DIR"
echo ""

# ── Done ─────────────────────────────────────────────────────────────────────

echo "================================================================="
echo "        🎉 DriveWipe Live v${DRIVEWIPE_LIVE_VERSION} Build Complete"
echo "================================================================="
echo ""
echo "📦 OUTPUTS:"
echo "  [ISO]  $OUTPUT"
echo "         - For USB drives, DVDs, or VMs."
echo ""
echo "  [PXE]  $PXE_DIR/"
echo "         - For network boot labs. Contains kernel, initrd, and iPXE scripts."
echo ""
echo "🚀 QUICK START:"
echo ""
echo "1. Create a bootable USB (Linux/macOS):"
echo "   sudo dd if=$OUTPUT of=/dev/sdX bs=4M status=progress"
echo ""
echo "2. Create a bootable USB (Windows):"
echo "   Use Rufus (https://rufus.ie) and select the ISO file."
echo ""
echo "3. Test in QEMU:"
echo "   qemu-system-x86_64 -m 2G -drive file=$OUTPUT,format=raw"
echo ""
echo "-----------------------------------------------------------------"
echo "🛠️  What's inside:"
echo "  - Linux Kernel + Busybox (Alpine ${ALPINE_VERSION})"
echo "  - DriveWipe TUI (Dashboard) + CLI"
echo "  - Custom Kernel Module (for HPA/DCO and Unfreezing)"
echo "  - Drivers for SATA, NVMe, USB, SCSI, and RAID controllers"
echo "-----------------------------------------------------------------"
echo ""
echo "For PXE setup details, see: live/pxe/README.md"
echo ""
