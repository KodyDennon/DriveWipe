#!/usr/bin/env sh
#
# DriveWipe installer.
#
#   curl -fsSL https://raw.githubusercontent.com/KodyDennon/DriveWipe/main/install.sh | sh
#
# Or, from an extracted release archive:
#
#   ./install.sh
#
# Options:
#   --version X.Y.Z   install a specific version (default: latest)
#   --prefix DIR      install root (default: /usr/local, or ~/.local without root)
#   --uninstall       remove an existing installation
#   --no-verify       skip checksum verification (not recommended)
#
# POSIX sh — no bashisms — so it runs under dash, ash, and busybox.

set -eu

REPO="KodyDennon/DriveWipe"
VERSION=""
PREFIX=""
UNINSTALL=0
VERIFY=1

# ── Output ──────────────────────────────────────────────────────────────────

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    B=$(printf '\033[1m'); DIM=$(printf '\033[2m'); R=$(printf '\033[0m')
    GREEN=$(printf '\033[32m'); RED=$(printf '\033[31m'); YELLOW=$(printf '\033[33m')
else
    B=''; DIM=''; R=''; GREEN=''; RED=''; YELLOW=''
fi

say()  { printf '%s\n' "$*"; }
step() { printf '  %s->%s %s\n' "$DIM" "$R" "$*"; }
ok()   { printf '  %s+%s %s\n' "$GREEN" "$R" "$*"; }
warn() { printf '  %s!%s %s\n' "$YELLOW" "$R" "$*" >&2; }
die()  { printf '%serror:%s %s\n' "$RED" "$R" "$*" >&2; exit 1; }

need() {
    command -v "$1" >/dev/null 2>&1 || die "'$1' is required but not installed."
}

# ── Arguments ───────────────────────────────────────────────────────────────

while [ $# -gt 0 ]; do
    case "$1" in
        --version) VERSION="${2:?--version needs a value}"; shift 2 ;;
        --version=*) VERSION="${1#*=}"; shift ;;
        --prefix)  PREFIX="${2:?--prefix needs a value}"; shift 2 ;;
        --prefix=*) PREFIX="${1#*=}"; shift ;;
        --uninstall) UNINSTALL=1; shift ;;
        --no-verify) VERIFY=0; shift ;;
        -h|--help)
            # Printed inline rather than read back from $0, because under
            # `curl … | sh` there is no script file to read.
            cat <<'USAGE'
DriveWipe installer

Usage:
  curl -fsSL https://raw.githubusercontent.com/KodyDennon/DriveWipe/main/install.sh | sh
  ./install.sh [options]

Options:
  --version X.Y.Z   Install a specific version (default: latest)
  --prefix DIR      Install root (default: /usr/local, or ~/.local without root)
  --uninstall       Remove an existing installation
  --no-verify       Skip checksum verification (not recommended)
  -h, --help        Show this message

To pass options through a pipe, use `sh -s --`:
  curl -fsSL .../install.sh | sh -s -- --uninstall
USAGE
            exit 0
            ;;
        *) die "unknown option: $1 (try --help)" ;;
    esac
done

# ── Install location ────────────────────────────────────────────────────────
# Prefer a system-wide install, but fall back to the user's home rather than
# demanding sudo — DriveWipe still needs root to touch a disk, but it does not
# need root merely to be installed.

if [ -z "$PREFIX" ]; then
    if [ "$(id -u)" = "0" ]; then
        PREFIX="/usr/local"
    elif [ -w /usr/local/bin ]; then
        PREFIX="/usr/local"
    else
        PREFIX="$HOME/.local"
    fi
fi

BINDIR="$PREFIX/bin"

# ── Uninstall ───────────────────────────────────────────────────────────────

if [ "$UNINSTALL" = "1" ]; then
    say "${B}Uninstalling DriveWipe${R}"
    removed=0
    for f in drivewipe drivewipe-tui drivewipe-gui; do
        if [ -e "$BINDIR/$f" ] || [ -L "$BINDIR/$f" ]; then
            rm -f "$BINDIR/$f" && ok "removed $BINDIR/$f" && removed=1
        fi
    done
    for d in "$PREFIX/share/applications/drivewipe.desktop" \
             "$HOME/.local/share/applications/drivewipe.desktop"; do
        [ -e "$d" ] && rm -f "$d" && ok "removed $d" && removed=1
    done
    [ "$removed" = "0" ] && warn "nothing found under $PREFIX"
    say ""
    say "Configuration and reports were left in place:"
    say "  ${DIM}~/.config/drivewipe${R} and ${DIM}~/.local/share/drivewipe${R}"
    exit 0
fi

# ── Platform detection ──────────────────────────────────────────────────────

detect_platform() {
    os=$(uname -s)
    arch=$(uname -m)

    case "$os" in
        Linux)  os_name="Linux" ;;
        Darwin) os_name="macOS" ;;
        *) die "unsupported operating system: $os. Windows users should download the .zip from https://github.com/$REPO/releases" ;;
    esac

    case "$os_name:$arch" in
        Linux:x86_64|Linux:amd64)     PLATFORM="Linux-x64";           EXT="tar.gz" ;;
        Linux:aarch64|Linux:arm64)    PLATFORM="Linux-ARM64";         EXT="tar.gz" ;;
        macOS:x86_64)                 PLATFORM="macOS-Intel";         EXT="tar.gz" ;;
        macOS:arm64)                  PLATFORM="macOS-Apple-Silicon"; EXT="tar.gz" ;;
        *) die "unsupported architecture: $arch on $os_name" ;;
    esac
}

# ── Local archive install ───────────────────────────────────────────────────
# When run from inside an extracted release archive, install what is already
# here instead of downloading anything.

SCRIPT_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)

install_binary() {
    src="$1"
    mkdir -p "$BINDIR" || die "cannot create $BINDIR"

    install -m 0755 "$src" "$BINDIR/drivewipe" 2>/dev/null \
        || { cp "$src" "$BINDIR/drivewipe" && chmod 0755 "$BINDIR/drivewipe"; } \
        || die "cannot write to $BINDIR. Re-run with sudo, or pass --prefix \$HOME/.local"
    ok "installed $BINDIR/drivewipe"

    # DriveWipe picks its interface from the name it is invoked as, so these
    # symlinks are all that separate "open the TUI" from "open the GUI".
    for alias_name in drivewipe-tui drivewipe-gui; do
        ln -sf drivewipe "$BINDIR/$alias_name" && ok "linked $BINDIR/$alias_name"
    done
}

install_desktop_entry() {
    [ "$(uname -s)" = "Linux" ] || return 0

    if [ "$(id -u)" = "0" ]; then
        appdir="$PREFIX/share/applications"
    else
        appdir="$HOME/.local/share/applications"
    fi
    mkdir -p "$appdir" 2>/dev/null || return 0

    cat > "$appdir/drivewipe.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=DriveWipe
GenericName=Secure Drive Eraser
Comment=Securely sanitize drives to NIST SP 800-88 and DoD standards
Exec=$BINDIR/drivewipe --gui
Icon=drive-harddisk
Terminal=false
Categories=System;Security;Utility;
Keywords=wipe;erase;disk;drive;sanitize;secure;
DESKTOP
    ok "desktop entry at $appdir/drivewipe.desktop"
}

# ── Download and verify ─────────────────────────────────────────────────────

fetch() {
    url="$1"; out="$2"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$url" -o "$out"
    else
        wget -qO "$out" "$url"
    fi
}

resolve_version() {
    [ -n "$VERSION" ] && return 0
    step "resolving latest release"
    VERSION=$(fetch "https://api.github.com/repos/$REPO/releases/latest" /dev/stdout \
        | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -1)
    [ -n "$VERSION" ] || die "could not determine the latest version. Pass --version X.Y.Z"
}

verify_checksum() {
    archive="$1"; sums="$2"
    if [ "$VERIFY" = "0" ]; then
        warn "checksum verification skipped"
        return 0
    fi
    if command -v sha256sum >/dev/null 2>&1; then
        want=$(grep " $(basename "$archive")\$" "$sums" | awk '{print $1}')
        got=$(sha256sum "$archive" | awk '{print $1}')
    elif command -v shasum >/dev/null 2>&1; then
        want=$(grep " $(basename "$archive")\$" "$sums" | awk '{print $1}')
        got=$(shasum -a 256 "$archive" | awk '{print $1}')
    else
        warn "no sha256 tool found; skipping verification"
        return 0
    fi

    [ -n "$want" ] || die "no checksum published for $(basename "$archive") — refusing to install"
    [ "$want" = "$got" ] || die "checksum mismatch for $(basename "$archive")
  expected $want
  actual   $got
This archive does not match what was published. Do not use it."
    ok "checksum verified"
}

download_and_install() {
    need uname
    command -v curl >/dev/null 2>&1 || command -v wget >/dev/null 2>&1 \
        || die "either curl or wget is required."
    need tar

    detect_platform
    resolve_version

    tag="$VERSION"
    case "$tag" in v*) ;; *) tag="v$tag" ;; esac

    archive="DriveWipe-${tag}-${PLATFORM}.${EXT}"
    base="https://github.com/$REPO/releases/download/$tag"

    tmp=$(mktemp -d 2>/dev/null || mktemp -d -t drivewipe)
    trap 'rm -rf "$tmp"' EXIT INT TERM

    step "downloading $archive"
    fetch "$base/$archive" "$tmp/$archive" \
        || die "download failed. Check that $tag exists and has a build for $PLATFORM:
  https://github.com/$REPO/releases"

    if [ "$VERIFY" = "1" ]; then
        step "downloading checksums"
        if fetch "$base/SHA256SUMS.txt" "$tmp/SHA256SUMS.txt" 2>/dev/null; then
            verify_checksum "$tmp/$archive" "$tmp/SHA256SUMS.txt"
        else
            warn "SHA256SUMS.txt not published for $tag; cannot verify"
        fi
    fi

    step "extracting"
    tar xzf "$tmp/$archive" -C "$tmp" || die "could not extract $archive"

    bin=$(find "$tmp" -type f -name drivewipe -perm -u+x 2>/dev/null | head -1)
    [ -n "$bin" ] || die "the archive did not contain a drivewipe binary"

    install_binary "$bin"
    install_desktop_entry
}

# ── Main ────────────────────────────────────────────────────────────────────

say ""
say "${B}DriveWipe installer${R}"
say ""

if [ -f "$SCRIPT_DIR/drivewipe" ] && [ -x "$SCRIPT_DIR/drivewipe" ]; then
    step "installing from this directory"
    install_binary "$SCRIPT_DIR/drivewipe"
    install_desktop_entry
else
    download_and_install
fi

# ── PATH advice ─────────────────────────────────────────────────────────────

case ":$PATH:" in
    *":$BINDIR:"*) ;;
    *)
        say ""
        warn "$BINDIR is not on your PATH. Add it with:"
        say "      echo 'export PATH=\"$BINDIR:\$PATH\"' >> ~/.profile"
        ;;
esac

say ""
say "${B}Installed.${R} DriveWipe picks its interface from how you start it:"
say ""
say "  ${B}drivewipe${R}                     interactive terminal interface"
say "  ${B}drivewipe --gui${R}               desktop window"
say "  ${B}drivewipe list${R}                command line, for scripts"
say "  ${B}drivewipe --help${R}              all commands"
say ""
say "Wiping a drive needs root, so prefix those commands with ${B}sudo${R}."
say ""
