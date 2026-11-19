#!/usr/bin/env bash
# ForgeHub browser E2E (Step 20): builds the SPA, starts the
# real server with a throwaway DB, and runs the Playwright
# suite in web/e2e against it.
#
# One-time setup:  cd web && npx playwright install chromium
# Usage: scripts/e2e-web.sh [--skip-build]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export PATH="$HOME/.cargo/bin:$PATH"

SKIP_BUILD=0
[ "${1:-}" = "--skip-build" ] && SKIP_BUILD=1

WORK="$(mktemp -d /tmp/forgehub-e2e-web.XXXXXX)"
PORT="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
export E2E_BASE_URL="http://127.0.0.1:$PORT"
SERVER_PID=""
cleanup() {
    [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null || true
    [ "${E2E_KEEP:-0}" = "1" ] || rm -rf "$WORK"
}
trap cleanup EXIT

if [ "$SKIP_BUILD" = "0" ]; then
    echo "e2e-web: building server + SPA"
    cargo build -p forge-server 2>&1 | tail -1
    (cd web && npm run build > /dev/null 2>&1) || { echo "e2e-web: web build failed" >&2; exit 1; }
fi

FORGEHUB_BIND="127.0.0.1:$PORT" \
FORGEHUB_DATABASE_URL="sqlite://$WORK/db.sqlite" \
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
npx playwright test
echo "E2E-WEB PASS on $E2E_BASE_URL"
