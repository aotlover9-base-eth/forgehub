#!/usr/bin/env bash
# ForgeHub end-to-end test (Step 20).
#
# Builds the real binaries, starts the real server on an
# ephemeral port with a throwaway database, and drives the
# whole product story with the real CLI + API:
#
#   signup -> device-flow login (scripted approval) -> import
#   -> push -> web-side edit (API PUT) -> status/diff -> pull
#   -> second user: fork, typed-yes run, trusted run, changed
#   -> hash diff + decline -> admin hide/unhide -> token
#   revoke -> server restart persistence -> DB backup/restore
#   drill -> account delete.
#
# Fails loudly (exit 1, server log tail) on the first mismatch.
# Usage: scripts/e2e.sh [--skip-build] [--skip-web]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export PATH="$HOME/.cargo/bin:$PATH"

SKIP_BUILD=0
SKIP_WEB=0
for arg in "$@"; do
    case "$arg" in
        --skip-build) SKIP_BUILD=1 ;;
        --skip-web) SKIP_WEB=1 ;;
        *) echo "unknown arg: $arg" >&2; exit 2 ;;
    esac
done

WORK="$(mktemp -d /tmp/forgehub-e2e.XXXXXX)"
PORT="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
BASE="http://127.0.0.1:$PORT"
SERVER_LOG="$WORK/server.log"
SERVER_PID=""

cleanup() {
    if [ -n "$SERVER_PID" ]; then kill "$SERVER_PID" 2>/dev/null || true; fi
    if [ "${E2E_KEEP:-0}" = "1" ]; then
        echo "e2e: keeping work dir $WORK"
    else
        rm -rf "$WORK"
    fi
}
trap cleanup EXIT

fail() {
    echo "E2E FAIL: $*" >&2
    echo "--- server log (tail) ---" >&2
    tail -20 "$SERVER_LOG" >&2 || true
    exit 1
}

step() { echo "e2e: $*"; }

start_server() {
    local db="$1"
    FORGEHUB_BIND="127.0.0.1:$PORT" \
    FORGEHUB_DATABASE_URL="sqlite://$db" \
    FORGEHUB_WEB_DIR="$ROOT/web/dist" \
    FORGEHUB_BASE_URL="$BASE" \
    FORGEHUB_LOGIN_BACKOFF_BASE_MS=60 \
        "$ROOT/target/debug/forge-server" >>"$SERVER_LOG" 2>&1 &
    SERVER_PID=$!
    for _ in $(seq 1 50); do
        if curl -s -o /dev/null "$BASE/api/v1/health"; then return 0; fi
        sleep 0.2
    done
    fail "server did not become healthy"
}

stop_server() {
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
    SERVER_PID=""
}

# fg <home> <args...> — run the CLI against an isolated HOME.
fg() {
    local home="$1"; shift
    env -i PATH="$PATH" HOME="$home" FORGEHUB_HOME="$home/.forgehub" \
        XDG_CONFIG_HOME="$home/.config" FORGEHUB_REGISTRY="$BASE" \
        "$ROOT/target/debug/forgehub" "$@"
}

# fg_pty <home> <input-line> <args...> — same, but under a PTY
# (scripts/ptyrun.py): the trust prompts refuse piped stdin by
# design, so interactive confirmations need a terminal.
fg_pty() {
    local home="$1"; local input="$2"; shift 2
    python3 "$ROOT/scripts/ptyrun.py" "$input" \
        env -i PATH="$PATH" HOME="$home" FORGEHUB_HOME="$home/.forgehub" \
        XDG_CONFIG_HOME="$home/.config" FORGEHUB_REGISTRY="$BASE" \
        "$ROOT/target/debug/forgehub" "$@"
}

api() { curl -s "$@"; }

csrf_of() { # <cookie-jar>
    api -b "$1" "$BASE/api/v1/me" | python3 -c 'import sys,json;print(json.load(sys.stdin)["csrf_token"])'
}

signup() { # <username> <jar>
    api -c "$2" -H 'Content-Type: application/json' \
        -d "{\"username\":\"$1\",\"email\":\"$1@example.com\",\"password\":\"a long passphrase\"}" \
        "$BASE/api/v1/auth/signup" > /dev/null
}

# ---------------------------------------------------------------- build
if [ "$SKIP_BUILD" = "0" ]; then
    step "building CLI + server (debug)"
    cargo build -p forge-cli -p forge-server 2>&1 | tail -1
    if [ "$SKIP_WEB" = "0" ]; then
        step "building web"
        (cd web && npm run build > /dev/null 2>&1) || fail "web build failed"
    fi
fi
[ -f "$ROOT/web/dist/index.html" ] || mkdir -p "$ROOT/web/dist" && touch "$ROOT/web/dist/index.html"

ALICE="$WORK/alice"; BOB="$WORK/bob"
mkdir -p "$ALICE" "$BOB"

step "starting server on $BASE (db $WORK/db.sqlite)"
start_server "$WORK/db.sqlite"

# ---------------------------------------------------------------- alice: signup + device login
step "alice signs up (API)"
signup alicee2e "$WORK/alice.jar"
ALICE_CSRF="$(csrf_of "$WORK/alice.jar")"

step "alice logs in the CLI via the device flow (scripted approval)"
fg "$ALICE" login > "$WORK/login.out" 2>&1 &
LOGIN_PID=$!
USER_CODE=""
for _ in $(seq 1 50); do
    USER_CODE="$(grep -oE 'code: +[A-Z0-9-]+' "$WORK/login.out" 2>/dev/null | awk '{print $2}' || true)"
    [ -n "$USER_CODE" ] && break
    sleep 0.2
done
[ -n "$USER_CODE" ] || fail "CLI did not print a device code"
api -b "$WORK/alice.jar" -H "X-CSRF-Token: $ALICE_CSRF" -H 'Content-Type: application/json' \
    -d "{\"user_code\":\"$USER_CODE\"}" "$BASE/api/v1/auth/device/approve" > /dev/null
wait "$LOGIN_PID" || { cat "$WORK/login.out"; fail "device login failed"; }
fg "$ALICE" whoami | grep -q "alicee2e" || fail "whoami does not show alicee2e"

# ---------------------------------------------------------------- alice: import + push
step "alice imports a snippet and pushes it public"
cat > "$WORK/ubuntu-setup.yaml" <<'YAML'
name: ubuntu-setup
description: E2E demo setup
mode: script
shell: bash
script: |
  echo e2e-hello-v1
YAML
fg "$ALICE" import "$WORK/ubuntu-setup.yaml" > /dev/null
fg "$ALICE" push ubuntu-setup --public | grep -q "ubuntu-setup" || fail "push output unexpected"
api "$BASE/api/v1/snippets/alicee2e/ubuntu-setup" | grep -q '"version":1' || fail "pushed snippet is not v1 via API"

# ---------------------------------------------------------------- web-side edit -> v2, status/diff/pull
step "alice edits the snippet through the API (the 'web' change) -> v2"
ETAG_V1="$(api -b "$WORK/alice.jar" "$BASE/api/v1/snippets/alicee2e/ubuntu-setup" | python3 -c 'import sys,json;print(json.load(sys.stdin)["version"])')"
api -b "$WORK/alice.jar" -H "X-CSRF-Token: $ALICE_CSRF" -H 'Content-Type: application/json' \
    -H "If-Match: \"$ETAG_V1\"" -X PUT \
    -d '{"name":"ubuntu-setup","body":"name: ubuntu-setup\ndescription: E2E demo setup\nmode: script\nshell: bash\nscript: |\n  echo e2e-hello-v2\n","visibility":"public"}' \
    "$BASE/api/v1/snippets/alicee2e/ubuntu-setup" | grep -q '"version":2' || fail "API PUT did not create v2"
fg "$ALICE" status ubuntu-setup | grep -q "behind" || fail "status should say behind after remote edit"
fg "$ALICE" diff ubuntu-setup | grep -q "e2e-hello-v2" || fail "diff should show the v2 line"
fg "$ALICE" pull ubuntu-setup --force > /dev/null
fg "$ALICE" status ubuntu-setup | grep -q "unchanged" || fail "status should say unchanged after pull"

# ---------------------------------------------------------------- bob: token login, fork, trust flow
step "bob signs up and gets a CLI token via the API"
signup bobe2e "$WORK/bob.jar"
BOB_CSRF="$(csrf_of "$WORK/bob.jar")"
BOB_TOKEN="$(api -b "$WORK/bob.jar" -H "X-CSRF-Token: $BOB_CSRF" -H 'Content-Type: application/json' \
    -d '{"name":"e2e","scopes":["read","write"]}' "$BASE/api/v1/tokens" \
    | python3 -c 'import sys,json;print(json.load(sys.stdin)["token"])')"
[ -n "$BOB_TOKEN" ] || fail "token creation returned no token"
printf '%s' "$BOB_TOKEN" | fg "$BOB" login --token > /dev/null
fg "$BOB" whoami | grep -q "bobe2e" || fail "bob whoami failed"

step "bob forks alice's snippet"
fg "$BOB" fork alicee2e/ubuntu-setup | grep -q "ubuntu-setup" || fail "fork failed"

step "bob runs alice's snippet: first run needs a typed yes"
fg_pty "$BOB" "yes" run alicee2e/ubuntu-setup | grep -q "e2e-hello-v2" || fail "typed-yes run did not execute v2"
step "bob runs it again: trusted hash, plain y is enough"
fg_pty "$BOB" "y" run alicee2e/ubuntu-setup | grep -q "e2e-hello-v2" || fail "trusted run did not execute"

step "alice changes the snippet (v3); bob declines the changed-hash prompt"
api -b "$WORK/alice.jar" -H "X-CSRF-Token: $ALICE_CSRF" -H 'Content-Type: application/json' \
    -H "If-Match: \"$(api "$BASE/api/v1/snippets/alicee2e/ubuntu-setup" | python3 -c 'import sys,json;print(json.load(sys.stdin)["version"])')\"" -X PUT \
    -d '{"name":"ubuntu-setup","body":"name: ubuntu-setup\ndescription: E2E demo setup\nmode: script\nshell: bash\nscript: |\n  echo e2e-hello-v3\n","visibility":"public"}' \
    "$BASE/api/v1/snippets/alicee2e/ubuntu-setup" | grep -q '"version":3' || fail "API PUT did not create v3"
set +e
fg_pty "$BOB" "n" run alicee2e/ubuntu-setup > "$WORK/declined.out" 2>&1
DECLINE_RC=$?
set -e
[ "$DECLINE_RC" = "2" ] || fail "declining a changed snippet should exit 2, got $DECLINE_RC"
grep -qx "e2e-hello-v3" "$WORK/declined.out" && fail "declined snippet still executed"
step "bob accepts the change after seeing the diff"
fg_pty "$BOB" "yes" run alicee2e/ubuntu-setup | grep -q "e2e-hello-v3" || fail "changed-hash run did not execute v3"

# ---------------------------------------------------------------- admin hide/unhide
step "admin hides the snippet: it 404s for bob and leaves explore"
FORGEHUB_DATABASE_URL="sqlite://$WORK/db.sqlite" "$ROOT/target/debug/forge-server" admin hide-snippet alicee2e/ubuntu-setup > /dev/null
CODE="$(api -o /dev/null -w '%{http_code}' "$BASE/api/v1/snippets/alicee2e/ubuntu-setup")"
[ "$CODE" = "404" ] || fail "hidden snippet should 404, got $CODE"
FORGEHUB_DATABASE_URL="sqlite://$WORK/db.sqlite" "$ROOT/target/debug/forge-server" admin unhide-snippet alicee2e/ubuntu-setup > /dev/null
CODE="$(api -o /dev/null -w '%{http_code}' "$BASE/api/v1/snippets/alicee2e/ubuntu-setup")"
[ "$CODE" = "200" ] || fail "unhidden snippet should be back, got $CODE"

# ---------------------------------------------------------------- token revoke
step "bob's token is revoked; the CLI loses access (exit 4)"
TOKEN_ID="$(api -b "$WORK/bob.jar" "$BASE/api/v1/tokens" | python3 -c 'import sys,json;print(json.load(sys.stdin)["tokens"][0]["id"])')"
api -b "$WORK/bob.jar" -H "X-CSRF-Token: $BOB_CSRF" -X DELETE "$BASE/api/v1/tokens/$TOKEN_ID" > /dev/null
set +e
fg "$BOB" whoami > /dev/null 2>&1
WHOAMI_RC=$?
set -e
[ "$WHOAMI_RC" = "4" ] || fail "revoked token should make whoami exit 4, got $WHOAMI_RC"

# ---------------------------------------------------------------- restart persistence
step "server restart: data and sessions survive"
stop_server
start_server "$WORK/db.sqlite"
api "$BASE/api/v1/snippets/alicee2e/ubuntu-setup" | grep -q '"version":3' || fail "snippet lost across restart"

# ---------------------------------------------------------------- backup / restore drill
step "database backup + restore drill"
"$ROOT/scripts/backup-db.sh" "$WORK/db.sqlite" "$WORK/backup.sqlite" > /dev/null
stop_server
mv "$WORK/db.sqlite" "$WORK/db.orig.sqlite"
cp "$WORK/backup.sqlite" "$WORK/db.sqlite"
start_server "$WORK/db.sqlite"
api "$BASE/api/v1/snippets/alicee2e/ubuntu-setup" | grep -q '"version":3' || fail "restored DB does not serve the snippet"

# ---------------------------------------------------------------- account delete
step "bob deletes his account; login then fails"
api -b "$WORK/bob.jar" -H "X-CSRF-Token: $BOB_CSRF" -H 'Content-Type: application/json' -X DELETE \
    -d '{"username":"bobe2e","password":"a long passphrase"}' "$BASE/api/v1/me" > /dev/null
CODE="$(api -o /dev/null -w '%{http_code}' -H 'Content-Type: application/json' \
    -d '{"identifier":"bobe2e","password":"a long passphrase"}' "$BASE/api/v1/auth/login")"
[ "$CODE" = "401" ] || fail "deleted account should not log in, got $CODE"

stop_server
echo "E2E PASS: full story green on $BASE (work dir $WORK)"
