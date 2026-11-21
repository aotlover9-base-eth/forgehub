#!/usr/bin/env bash
# ForgeHub visual regression (Peak A8): like e2e-web.sh, but
# runs only the screenshot suite in web/e2e/visual against
# committed goldens in web/e2e/visual/__screenshots__.
#
# Usage:
#   scripts/visual-web.sh [--skip-build]           check
#   scripts/visual-web.sh --update [--skip-build]  regenerate goldens
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export PATH="$HOME/.cargo/bin:$PATH"

UPDATE=0
SKIP_BUILD=0
for arg in "$@"; do
    [ "$arg" = "--update" ] && UPDATE=1
    [ "$arg" = "--skip-build" ] && SKIP_BUILD=1
done

WORK="$(mktemp -d /tmp/forgehub-visual-web.XXXXXX)"
PORT="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
export E2E_BASE_URL="http://127.0.0.1:$PORT"
export VISUAL_TESTS=1
SERVER_PID=""
cleanup() {
    [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null || true
    rm -rf "$WORK"
}
trap cleanup EXIT

if [ "$SKIP_BUILD" = "0" ]; then
    echo "visual-web: building server + SPA"
    cargo build -p forge-server 2>&1 | tail -1
    (cd web && npm run build > /dev/null 2>&1) || { echo "visual-web: web build failed" >&2; exit 1; }
fi

FORGEHUB_BIND="127.0.0.1:$PORT" \
FORGEHUB_DATABASE_URL="sqlite://$WORK/e2e.db" \
FORGEHUB_WEB_DIR="$ROOT/web/dist" \
FORGEHUB_BASE_URL="$E2E_BASE_URL" \
FORGEHUB_LOGIN_BACKOFF_BASE_MS=60 \
    "$ROOT/target/debug/forge-server" > "$WORK/server.log" 2>&1 &
SERVER_PID=$!
for _ in $(seq 1 50); do
    curl -s -o /dev/null "$E2E_BASE_URL/api/v1/health" && break
    sleep 0.2
done

cd web
if [ "$UPDATE" = "1" ]; then
    npx playwright test e2e/visual --update-snapshots
    echo "VISUAL-WEB GOLDENS UPDATED"
else
    npx playwright test e2e/visual
    echo "VISUAL-WEB PASS on $E2E_BASE_URL"
fi
