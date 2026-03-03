#!/usr/bin/env bash
set -euo pipefail

# DriveWipe Local Release Script
# Auto-detects platform, builds release binaries, creates a version tag,
# and uploads to GitHub Releases via the `gh` CLI.
#
# Usage:
#   ./release.sh              # auto-detect bump, create new release
#   ./release.sh patch        # force patch bump (0.1.5 -> 0.1.6)
#   ./release.sh minor        # force minor bump (0.1.5 -> 0.2.0)
#   ./release.sh major        # force major bump (0.1.5 -> 1.0.0)
#   ./release.sh 1.0.0        # set exact version, create new release
#   ./release.sh --attach v1.0.0   # build for THIS platform and attach to existing release
#
# The --attach flag is for multi-platform releases:
#   1. Run `./release.sh 1.0.0` on your Mac to create the release
#   2. Run `./release.sh --attach v1.0.0` on your Windows desktop to add Windows binaries
#   3. Run `./release.sh --attach v1.0.0` on a Linux box to add Linux binaries
#
# Requirements: cargo, gh (GitHub CLI, authenticated), git

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

# ── Helpers ─────────────────────────────────────────────────────────────────

die() { echo "ERROR: $*" >&2; exit 1; }

check_tool() {
    command -v "$1" &>/dev/null || die "'$1' is required but not found. Install it first."
}

get_current_version() {
    grep '^version' crates/drivewipe-core/Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/'
}

bump_version() {
    local cur="$1" kind="$2"
    local major minor patch
    IFS='.' read -r major minor patch <<< "$cur"
    case "$kind" in
        major) echo "$((major + 1)).0.0" ;;
        minor) echo "${major}.$((minor + 1)).0" ;;
        patch) echo "${major}.${minor}.$((patch + 1))" ;;
        *)     echo "$kind" ;; # exact version passthrough
    esac
}

set_workspace_version() {
    local ver="$1"
    for toml in crates/*/Cargo.toml; do
        sed -i.bak "s/^version = \".*\"/version = \"${ver}\"/" "$toml"
        rm -f "${toml}.bak"
    done
}

auto_detect_bump() {
    local last_tag
    last_tag=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
    local range="${last_tag:+${last_tag}..HEAD}"
    range="${range:-HEAD}"

    if git log "$range" --pretty=%s | grep -qiE '^(feat!|BREAKING)'; then
        echo "major"
    elif git log "$range" --pretty=%s | grep -qiE '^feat'; then
        echo "minor"
    else
        echo "patch"
    fi
}

# ── Platform detection ──────────────────────────────────────────────────────

detect_platform() {
    local os arch target suffix archive

    case "$(uname -s)" in
        Linux*)  os="linux" ;;
        Darwin*) os="macos" ;;
        CYGWIN*|MINGW*|MSYS*) os="windows" ;;
        *) die "Unsupported OS: $(uname -s)" ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64)  arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *) die "Unsupported architecture: $(uname -m)" ;;
    esac

    case "${os}-${arch}" in
        linux-x86_64)   target="x86_64-unknown-linux-gnu" ;;
        linux-aarch64)  target="aarch64-unknown-linux-gnu" ;;
        macos-x86_64)   target="x86_64-apple-darwin" ;;
        macos-aarch64)  target="aarch64-apple-darwin" ;;
        windows-x86_64) target="x86_64-pc-windows-msvc" ;;
        windows-aarch64) target="aarch64-pc-windows-msvc" ;;
    esac

    suffix=""
    archive="tar.gz"
    if [ "$os" = "windows" ]; then
        suffix=".exe"
        archive="zip"
    fi

    echo "${os}|${arch}|${target}|${suffix}|${archive}"
}

# ── Build & Package (shared by both modes) ─────────────────────────────────

build_and_package() {
    local tag="$1"

    # Ensure target is installed
    if ! rustup target list --installed | grep -q "$TARGET"; then
        echo "Adding Rust target: $TARGET"
        rustup target add "$TARGET"
    fi

    echo "Building release binaries..."
    echo ""

    cargo build --release --target "$TARGET" --package drivewipe-cli
    echo "  [1/3] drivewipe-cli built"

    cargo build --release --target "$TARGET" --package drivewipe-tui
    echo "  [2/3] drivewipe-tui built"

    if cargo build --release --target "$TARGET" --package drivewipe-gui 2>&1; then
        echo "  [3/3] drivewipe-gui built"
    else
        echo "  [3/3] drivewipe-gui skipped (build failed — may need platform GUI libs)"
    fi
    echo ""

    # ── Package ─────────────────────────────────────────────────────────────

    DIST_DIR="target/dist"
    ARCHIVE_NAME="drivewipe-${tag}-${TARGET}"
    rm -rf "$DIST_DIR"
    mkdir -p "$DIST_DIR"

    cp "target/${TARGET}/release/drivewipe${SUFFIX}" "$DIST_DIR/"    || die "CLI binary not found at target/${TARGET}/release/drivewipe${SUFFIX}"
    cp "target/${TARGET}/release/drivewipe-tui${SUFFIX}" "$DIST_DIR/" || die "TUI binary not found at target/${TARGET}/release/drivewipe-tui${SUFFIX}"
    cp "target/${TARGET}/release/drivewipe-gui${SUFFIX}" "$DIST_DIR/" 2>/dev/null || true
    [ -f LICENSE.md ] && cp LICENSE.md "$DIST_DIR/"
    cp README.md "$DIST_DIR/"

    echo "Packaging: ${ARCHIVE_NAME}.${ARCHIVE}"

    ARCHIVE_PATH="${ROOT_DIR}/${ARCHIVE_NAME}.${ARCHIVE}"

    if [ "$ARCHIVE" = "zip" ]; then
        (cd "$DIST_DIR" && zip -q "${ARCHIVE_PATH}" ./* )
    else
        tar czf "${ARCHIVE_PATH}" -C "$DIST_DIR" .
    fi

    # Generate checksum
    CHECKSUM_FILE="${ROOT_DIR}/${ARCHIVE_NAME}.sha256"
    if command -v sha256sum &>/dev/null; then
        (cd "$(dirname "$ARCHIVE_PATH")" && sha256sum "$(basename "$ARCHIVE_PATH")" > "$CHECKSUM_FILE")
    else
        (cd "$(dirname "$ARCHIVE_PATH")" && shasum -a 256 "$(basename "$ARCHIVE_PATH")" > "$CHECKSUM_FILE")
    fi

    echo "Archive:  $ARCHIVE_PATH"
    echo "Checksum: $CHECKSUM_FILE"
    echo ""
}

# ── Main ────────────────────────────────────────────────────────────────────

echo "=== DriveWipe Release Builder ==="
echo ""

# Check tools
check_tool cargo
check_tool gh
check_tool git

# Parse --attach mode
ATTACH_MODE=false
ATTACH_TAG=""

if [ "${1:-}" = "--attach" ]; then
    ATTACH_MODE=true
    ATTACH_TAG="${2:-}"
    if [ -z "$ATTACH_TAG" ]; then
        die "Usage: ./release.sh --attach <tag>  (e.g. ./release.sh --attach v1.0.0)"
    fi
    # Normalize: ensure tag starts with 'v'
    if [[ "$ATTACH_TAG" != v* ]]; then
        ATTACH_TAG="v${ATTACH_TAG}"
    fi
fi

# Detect platform
IFS='|' read -r OS ARCH TARGET SUFFIX ARCHIVE <<< "$(detect_platform)"
echo "Platform:     $OS ($ARCH)"
echo "Rust target:  $TARGET"
echo ""

# ── Attach mode: build + upload to existing release ────────────────────────

if [ "$ATTACH_MODE" = true ]; then
    echo "Mode:         ATTACH to existing release"
    echo "Tag:          $ATTACH_TAG"
    echo ""

    # Verify the release exists
    if ! gh release view "$ATTACH_TAG" &>/dev/null; then
        die "Release '$ATTACH_TAG' not found on GitHub. Create it first with: ./release.sh <version>"
    fi

    # Build and package
    build_and_package "$ATTACH_TAG"

    # Upload artifacts to the existing release
    echo "Attaching to release $ATTACH_TAG..."
    gh release upload "$ATTACH_TAG" \
        "$ARCHIVE_PATH" \
        "$CHECKSUM_FILE" \
        --clobber

    echo ""
    echo "=== Attached ${TARGET} build to ${ATTACH_TAG} ==="
    echo ""
    echo "View at: $(gh release view "$ATTACH_TAG" --json url -q .url)"

    # Cleanup
    rm -rf "$DIST_DIR"
    exit 0
fi

# ── New release mode ───────────────────────────────────────────────────────

# Ensure clean working tree
if [ -n "$(git status --porcelain -- ':(exclude).claude')" ]; then
    die "Working tree is dirty. Commit or stash changes first."
fi

# Ensure we're on main
BRANCH=$(git branch --show-current)
if [ "$BRANCH" != "main" ]; then
    die "Must be on 'main' branch (currently on '$BRANCH')."
fi

echo "Mode:         NEW release"

# Determine version
CURRENT=$(get_current_version)
BUMP_TYPE="${1:-$(auto_detect_bump)}"

if [[ "$BUMP_TYPE" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    NEW_VERSION="$BUMP_TYPE"
else
    NEW_VERSION=$(bump_version "$CURRENT" "$BUMP_TYPE")
fi

TAG="v${NEW_VERSION}"

echo "Current version: $CURRENT"
echo "New version:     $NEW_VERSION ($BUMP_TYPE)"
echo "Tag:             $TAG"
echo ""

# Check tag doesn't already exist
if git rev-parse "$TAG" &>/dev/null; then
    die "Tag '$TAG' already exists. Use --attach to add builds: ./release.sh --attach $TAG"
fi

# Build and package
build_and_package "$TAG"

# ── Version bump & tag ──────────────────────────────────────────────────────

echo "Updating workspace version to $NEW_VERSION..."
set_workspace_version "$NEW_VERSION"

git add crates/*/Cargo.toml
git commit -m "chore(release): bump version to ${NEW_VERSION}"
git tag -a "$TAG" -m "Release ${TAG}"

echo "Pushing tag $TAG..."
git push origin main
git push origin "$TAG"

echo ""

# ── GitHub Release ──────────────────────────────────────────────────────────

echo "Creating GitHub release..."

RELEASE_NOTES="## DriveWipe ${TAG}

### Downloads
| Platform | Architecture | Archive |
|---|---|---|
| ${OS} | ${ARCH} | \`${ARCHIVE_NAME}.${ARCHIVE}\` |

*Run \`./release.sh --attach ${TAG}\` on other platforms to add their builds.*

### Contents
- \`drivewipe${SUFFIX}\` — CLI tool
- \`drivewipe-tui${SUFFIX}\` — Terminal UI"

# Add GUI line if it was built
if [ -f "$DIST_DIR/drivewipe-gui${SUFFIX}" ]; then
    RELEASE_NOTES="${RELEASE_NOTES}
- \`drivewipe-gui${SUFFIX}\` — Graphical UI"
fi

RELEASE_NOTES="${RELEASE_NOTES}

### Verify
\`\`\`
shasum -a 256 -c ${ARCHIVE_NAME}.sha256
\`\`\`
"

gh release create "$TAG" \
    "$ARCHIVE_PATH" \
    "$CHECKSUM_FILE" \
    --title "DriveWipe ${TAG}" \
    --notes "$RELEASE_NOTES"

echo ""
echo "=== Release $TAG published ==="
echo ""
echo "View at: $(gh release view "$TAG" --json url -q .url)"
echo ""
echo "To add builds from other machines:"
echo "  ./release.sh --attach $TAG"

# Cleanup dist
rm -rf "$DIST_DIR"
