#!/bin/sh
# ForgeHub CLI installer.
#
# What this script does, in order (nothing else):
#   1. Detects your OS and CPU architecture.
#   2. Downloads forgehub-<version>-<target>.tar.gz and the
#      SHA256SUMS file from the ForgeHub GitHub releases.
#   3. VERIFIES the archive's SHA-256 checksum. If it does not
#      match, the script stops and installs nothing.
#   4. Extracts the archive in a temporary directory and
#      installs the single `forgehub` binary into
#      ${FORGEHUB_INSTALL_DIR:-~/.local/bin} (mode 0755).
#   5. Offers to run `forgehub alias install` (which adds the
#      `fg` alias); it never edits your shell rc files itself,
#      and the offer is skipped when stdin is not a terminal.
#
# It refuses to run as root unless FORGEHUB_ALLOW_ROOT=1.
# Read it before you run it — that is the ForgeHub way.
#
# Environment overrides:
#   FORGEHUB_VERSION       release tag (default: latest)
#   FORGEHUB_INSTALL_DIR   install directory (default ~/.local/bin)
#   FORGEHUB_RELEASE_BASE  base URL of the release assets
#                          (default: the GitHub releases URL;
#                          point it at a local mirror to test)
#   FORGEHUB_ALLOW_ROOT=1  allow running as root

set -eu

REPO="aotlover9-base-eth/forgehub"
VERSION="${FORGEHUB_VERSION:-latest}"
INSTALL_DIR="${FORGEHUB_INSTALL_DIR:-$HOME/.local/bin}"
if [ "$VERSION" = "latest" ]; then
    BASE="${FORGEHUB_RELEASE_BASE:-https://github.com/$REPO/releases/latest/download}"
else
    BASE="${FORGEHUB_RELEASE_BASE:-https://github.com/$REPO/releases/download/$VERSION}"
fi

say() { printf '%s\n' "$*"; }
die() { printf 'install.sh: %s\n' "$*" >&2; exit 1; }

# --- root check ------------------------------------------------
if [ "$(id -u)" = "0" ] && [ "${FORGEHUB_ALLOW_ROOT:-0}" != "1" ]; then
    die "refusing to run as root (set FORGEHUB_ALLOW_ROOT=1 if you really mean it)"
fi

# --- detect platform --------------------------------------------
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
    Linux)  os_part="unknown-linux-musl" ;;
    Darwin) die "macOS binaries are not published (ForgeHub ships Linux and Windows builds); build from source instead: cargo install --path crates/forge-cli" ;;
    *) die "unsupported OS: $os (Linux is supported here; Windows uses the .zip asset)" ;;
esac
case "$arch" in
    x86_64|amd64)   arch_part="x86_64" ;;
    aarch64|arm64)  arch_part="aarch64" ;;
    *) die "unsupported architecture: $arch" ;;
esac
target="${arch_part}-${os_part}"

# Release assets are named forgehub-<tag>-<target>.tar.gz.
# For "latest", resolve the current tag via the GitHub API
# (one extra HTTPS call; mirrors should pin FORGEHUB_VERSION).
if [ "$VERSION" = "latest" ]; then
    api_url="https://api.github.com/repos/$REPO/releases/latest"
    if command -v curl >/dev/null 2>&1; then
        tag_json="$(curl -fsSL "$api_url" || true)"
    elif command -v wget >/dev/null 2>&1; then
        tag_json="$(wget -qO- "$api_url" || true)"
    else
        tag_json=""
    fi
    VERSION="$(printf '%s' "$tag_json" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
    [ -n "$VERSION" ] || die "could not resolve the latest release tag; set FORGEHUB_VERSION explicitly"
    BASE="${FORGEHUB_RELEASE_BASE:-https://github.com/$REPO/releases/download/$VERSION}"
fi
asset="forgehub-${VERSION}-${target}.tar.gz"

say "ForgeHub installer"
say "  platform : $os / $arch ($target)"
say "  download : $BASE/$asset"
say "  install  : $INSTALL_DIR/forgehub"
say ""

# --- tools -------------------------------------------------------
if command -v curl >/dev/null 2>&1; then
    fetch() { curl -fsSL "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then
    fetch() { wget -q "$1" -O "$2"; }
else
    die "need curl or wget to download the release"
fi
if command -v sha256sum >/dev/null 2>&1; then
    checksum() { sha256sum "$1" | awk '{print $1}'; }
elif command -v shasum >/dev/null 2>&1; then
    checksum() { shasum -a 256 "$1" | awk '{print $1}'; }
else
    die "need sha256sum or shasum to verify the download"
fi

# --- download + verify -------------------------------------------
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

say "Downloading $asset ..."
fetch "$BASE/$asset" "$tmp/$asset" || die "download failed: $BASE/$asset"
say "Downloading SHA256SUMS ..."
fetch "$BASE/SHA256SUMS" "$tmp/SHA256SUMS" || die "download failed: $BASE/SHA256SUMS"

expected="$(awk -v a="$asset" '$2 == a {print $1}' "$tmp/SHA256SUMS")"
[ -n "$expected" ] || die "no checksum for $asset in SHA256SUMS"
actual="$(checksum "$tmp/$asset")"
if [ "$actual" != "$expected" ]; then
    die "CHECKSUM MISMATCH for $asset (expected $expected, got $actual) — refusing to install"
fi
say "Checksum verified: $actual"

# --- install ------------------------------------------------------
mkdir -p "$tmp/extract"
tar -xzf "$tmp/$asset" -C "$tmp/extract"
binary="$(find "$tmp/extract" -name forgehub -type f | head -n 1)"
[ -n "$binary" ] || die "archive did not contain a forgehub binary"
mkdir -p "$INSTALL_DIR"
install -m 0755 "$binary" "$INSTALL_DIR/forgehub"
say "Installed: $INSTALL_DIR/forgehub"
"$INSTALL_DIR/forgehub" --version || true

case ":$PATH:" in
    *":$INSTALL_DIR:"*) : ;;
    *) say "Note: $INSTALL_DIR is not on your PATH. Add it in your shell rc file:"
       say "  export PATH=\"$INSTALL_DIR:\$PATH\"" ;;
esac

# --- offer the fg alias (never edits rc files itself) -------------
if [ -t 0 ]; then
    printf 'Also install the fg alias? [y/N] '
    read -r answer || answer=""
    case "$answer" in
        y|Y|yes|YES) "$INSTALL_DIR/forgehub" alias install || say "Alias step failed; run \`forgehub alias install\` later." ;;
        *) say "Skipped. You can run \`forgehub alias install\` any time." ;;
    esac
else
    say "Non-interactive shell: run \`forgehub alias install\` to get the \`fg\` alias."
fi
say "Done. Try: forgehub --help"
