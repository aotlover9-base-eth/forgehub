#!/bin/sh
# ForgeHub CLI installer (build from source).
#
# ForgeHub publishes no prebuilt binaries, so this script builds
# the CLI with cargo. What it does, in order (nothing else):
#   1. Checks that git and cargo are available.
#   2. Uses the checkout it lives in, or - when run from
#      elsewhere - clones the repository into a temp directory.
#   3. Runs `cargo install --locked --path crates/forge-cli`
#      into ${FORGEHUB_INSTALL_DIR:-~/.local} (binary lands in
#      <dir>/bin/forgehub).
#   4. Offers to run `forgehub alias install` (adds the `fg`
#      alias); it never edits your shell rc files itself, and
#      the offer is skipped when stdin is not a terminal.
#
# It refuses to run as root unless FORGEHUB_ALLOW_ROOT=1.
# Read it before you run it - that is the ForgeHub way.
#
# Environment overrides:
#   FORGEHUB_INSTALL_DIR   cargo install root (default ~/.local)
#   FORGEHUB_REPO_URL      git URL to clone when not run from a
#                          checkout (default: the GitHub repo)
#   FORGEHUB_ALLOW_ROOT=1  allow running as root

set -eu

REPO_URL="${FORGEHUB_REPO_URL:-https://github.com/aotlover9-base-eth/forgehub.git}"
INSTALL_ROOT="${FORGEHUB_INSTALL_DIR:-$HOME/.local}"

say() { printf '%s\n' "$*"; }
die() { printf 'install.sh: %s\n' "$*" >&2; exit 1; }

if [ "$(id -u)" = "0" ] && [ "${FORGEHUB_ALLOW_ROOT:-0}" != "1" ]; then
    die "refusing to run as root (set FORGEHUB_ALLOW_ROOT=1 if you really mean it)"
fi
[ "$(uname -s)" = "Linux" ] || die "ForgeHub is Linux-only; use the web app on other systems"
command -v cargo >/dev/null 2>&1 || die "cargo not found - install Rust from https://rustup.rs first"

# Use the checkout this script lives in, if it is one.
script_dir="$(cd "$(dirname "$0")" 2>/dev/null && pwd || true)"
src=""
if [ -n "$script_dir" ] && [ -f "$script_dir/../crates/forge-cli/Cargo.toml" ]; then
    src="$(cd "$script_dir/.." && pwd)"
else
    command -v git >/dev/null 2>&1 || die "git not found (needed to clone $REPO_URL)"
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT INT TERM
    say "Cloning $REPO_URL ..."
    git clone --depth 1 "$REPO_URL" "$tmp/forgehub" || die "clone failed: $REPO_URL"
    src="$tmp/forgehub"
fi

say "ForgeHub installer"
say "  source  : $src"
say "  install : $INSTALL_ROOT/bin/forgehub"
say ""

cargo install --locked --path "$src/crates/forge-cli" --root "$INSTALL_ROOT" --force
say "Installed: $INSTALL_ROOT/bin/forgehub"
"$INSTALL_ROOT/bin/forgehub" --version || true

case ":$PATH:" in
    *":$INSTALL_ROOT/bin:"*) : ;;
    *) say "Note: $INSTALL_ROOT/bin is not on your PATH. Add it in your shell rc file:"
       say "  export PATH=\"$INSTALL_ROOT/bin:\$PATH\"" ;;
esac

if [ -t 0 ]; then
    printf 'Also install the fg alias? [y/N] '
    read -r answer || answer=""
    case "$answer" in
        y|Y|yes|YES) "$INSTALL_ROOT/bin/forgehub" alias install || say "Alias step failed; run \`forgehub alias install\` later." ;;
        *) say "Skipped. You can run \`forgehub alias install\` any time." ;;
    esac
else
    say "Non-interactive shell: run \`forgehub alias install\` to get the \`fg\` alias."
fi
say "Done. Try: forgehub --help"
