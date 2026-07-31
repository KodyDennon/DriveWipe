#!/usr/bin/env bash
#
# Exercises install.sh end to end against a local HTTP server standing in for
# GitHub Releases, so the download path is covered rather than only the
# install-from-extracted-archive path.
#
# The download path shipped broken once: `tar -C` was applied before the archive
# path was resolved, so extraction failed for everyone using the documented
# `curl | sh` flow while the local-archive path kept working.
#
# Usage: scripts/test-install.sh [path-to-drivewipe-binary]

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INSTALL_SH="$ROOT_DIR/install.sh"
BINARY="${1:-$ROOT_DIR/target/debug/drivewipe}"

PASS=0
FAIL=0
pass() { printf '  PASS  %s\n' "$1"; PASS=$((PASS + 1)); }
fail() { printf '  FAIL  %s\n' "$1"; FAIL=$((FAIL + 1)); }

[ -x "$BINARY" ] || { echo "no drivewipe binary at $BINARY; build it first" >&2; exit 1; }
command -v python3 >/dev/null || { echo "python3 required" >&2; exit 1; }

WORK="$(mktemp -d)"
SERVER_PID=""
cleanup() {
    [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null || true
    rm -rf "$WORK"
}
trap cleanup EXIT

# ── Build a fake release ────────────────────────────────────────────────────

TAG="v9.9.9"
case "$(uname -s):$(uname -m)" in
    Linux:x86_64)  PLATFORM="Linux-x64" ;;
    Linux:aarch64) PLATFORM="Linux-ARM64" ;;
    Darwin:x86_64) PLATFORM="macOS-Intel" ;;
    Darwin:arm64)  PLATFORM="macOS-Apple-Silicon" ;;
    *) echo "unsupported test platform" >&2; exit 1 ;;
esac
ARCHIVE="DriveWipe-${TAG}-${PLATFORM}.tar.gz"

mkdir -p "$WORK/serve/$TAG" "$WORK/stage"
cp "$BINARY" "$WORK/stage/drivewipe"
cp "$INSTALL_SH" "$WORK/stage/install.sh"
echo "license" > "$WORK/stage/LICENSE.md"
tar czf "$WORK/serve/$TAG/$ARCHIVE" -C "$WORK/stage" .

( cd "$WORK/serve/$TAG" && sha256sum "$ARCHIVE" > SHA256SUMS.txt )
printf '{"tag_name": "%s"}\n' "$TAG" > "$WORK/serve/latest.json"

# `python3 -m http.server 0` buffers its banner, so the chosen port cannot be
# read back reliably. Bind explicitly and report the port before serving.
cat > "$WORK/serve.py" <<'PYSERVER'
import http.server, os, socketserver, sys

os.chdir(sys.argv[1])

class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *a):
        pass

with socketserver.TCPServer(("127.0.0.1", 0), Quiet) as httpd:
    with open(sys.argv[2], "w") as f:
        f.write(str(httpd.server_address[1]))
        f.flush()
    httpd.serve_forever()
PYSERVER

python3 "$WORK/serve.py" "$WORK/serve" "$WORK/port" &
SERVER_PID=$!

for _ in $(seq 1 50); do
    [ -s "$WORK/port" ] && break
    sleep 0.2
done
PORT="$(cat "$WORK/port" 2>/dev/null || true)"
[ -n "${PORT:-}" ] || { echo "local server did not start"; exit 1; }

export DRIVEWIPE_BASE_URL="http://127.0.0.1:$PORT"
export DRIVEWIPE_API_URL="http://127.0.0.1:$PORT/latest.json"

echo "Testing install.sh against http://127.0.0.1:$PORT"

# ── Download path ───────────────────────────────────────────────────────────

PREFIX="$WORK/prefix"
if HOME="$WORK/home" sh "$INSTALL_SH" --prefix "$PREFIX" >"$WORK/out.log" 2>&1; then
    pass "download install succeeded"
else
    fail "download install failed"
    cat "$WORK/out.log"
fi

grep -q "checksum verified" "$WORK/out.log" \
    && pass "checksum was verified" \
    || fail "checksum was not verified"

[ -x "$PREFIX/bin/drivewipe" ] \
    && pass "binary installed" \
    || fail "binary missing at $PREFIX/bin/drivewipe"

for alias_name in drivewipe-tui drivewipe-gui; do
    [ -L "$PREFIX/bin/$alias_name" ] \
        && pass "$alias_name symlink created" \
        || fail "$alias_name symlink missing"
done

"$PREFIX/bin/drivewipe" --version >/dev/null 2>&1 \
    && pass "installed binary runs" \
    || fail "installed binary does not run"

# ── Version pinning ─────────────────────────────────────────────────────────

if HOME="$WORK/home" sh "$INSTALL_SH" --prefix "$WORK/pinned" --version "$TAG" >/dev/null 2>&1; then
    pass "--version installs a pinned release"
else
    fail "--version failed"
fi

# ── Tamper detection ────────────────────────────────────────────────────────
# The whole point of publishing checksums is that a modified archive is
# rejected, so prove it is.

printf 'tampered' >> "$WORK/serve/$TAG/$ARCHIVE"
if HOME="$WORK/home" sh "$INSTALL_SH" --prefix "$WORK/bad" >"$WORK/tamper.log" 2>&1; then
    fail "a tampered archive was installed anyway"
else
    grep -qi "checksum mismatch" "$WORK/tamper.log" \
        && pass "tampered archive rejected on checksum mismatch" \
        || fail "install failed but not with a checksum error"
fi

# ── Uninstall ───────────────────────────────────────────────────────────────

HOME="$WORK/home" sh "$INSTALL_SH" --prefix "$PREFIX" --uninstall >/dev/null 2>&1
if [ ! -e "$PREFIX/bin/drivewipe" ] && [ ! -e "$PREFIX/bin/drivewipe-tui" ]; then
    pass "uninstall removed the binary and symlinks"
else
    fail "uninstall left files behind"
fi

echo
echo "  $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
