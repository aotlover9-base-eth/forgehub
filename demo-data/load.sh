#!/bin/sh
# ForgeHub demo pack loader.
#
#   sh demo-data/load.sh              import locally + push (private) to your server
#   sh demo-data/load.sh --local-only import locally only
#
# What it does:
#   1. Imports every snippet in ./snippets into your local library
#      (~/.forgehub). Names that already exist locally are skipped.
#   2. If you are logged in (`forgehub whoami`), pushes each snippet
#      that is not already on your server. Pushes are PRIVATE by
#      default - nothing becomes public.
#
# Safe to re-run: both phases skip what is already there.
# Uses the `forgehub` binary explicitly (never the `fg` alias, which
# may belong to another tool on some machines).

set -u
cd "$(dirname "$0")"

LOCAL_ONLY=0
if [ "${1:-}" = "--local-only" ]; then LOCAL_ONLY=1; fi

FH=$(command -v forgehub || true)
if [ -z "$FH" ]; then
  echo "forgehub: not found on PATH."
  echo "  fix: cargo install --path crates/forge-cli   (from the repo root)"
  exit 1
fi

echo "==> Step 1/2: importing the pack into your local library"
tmp=$(mktemp)
# Note: `forgehub import` exits 1 when everything was skipped (nothing
# new) - for this loader a full skip is success, so judge by the
# summary line, not the exit code. Only its absence means real failure.
"$FH" import snippets >"$tmp" 2>&1
if grep -qE '^Imported [0-9]+ snippet' "$tmp"; then
  grep -E '^Imported ' "$tmp"
else
  cat "$tmp"
  rm -f "$tmp"
  exit 1
fi
rm -f "$tmp"

if [ "$LOCAL_ONLY" = "1" ]; then
  echo "==> --local-only: done. Snippets are in your local library."
  exit 0
fi

if ! "$FH" whoami >/dev/null 2>&1; then
  echo "==> Step 2/2: skipped (not logged in)."
  echo "    The snippets are in your local library. To also put them on"
  echo "    your server under your account, run:"
  echo "      forgehub login"
  echo "    then re-run this script."
  exit 0
fi

echo "==> Step 2/2: pushing new snippets to your server (private)"
pushed=0
skipped=0
failed=0
for f in snippets/*.yml; do
  [ -e "$f" ] || continue
  name=$(basename "$f" .yml)
  if "$FH" remote show "$name" >/dev/null 2>&1; then
    skipped=$((skipped + 1))
  elif "$FH" push "$name" >/dev/null 2>&1; then
    pushed=$((pushed + 1))
    echo "  pushed  $name"
  else
    failed=$((failed + 1))
    echo "  FAILED  $name  (try: forgehub push $name)"
  fi
done
echo "Done: $pushed pushed, $skipped already on the server, $failed failed."
echo "All pushed snippets are PRIVATE. Make one public with:"
echo "  forgehub push <name> --public"
