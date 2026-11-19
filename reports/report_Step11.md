# Report — Step 11: CLI login, whoami, credentials

## 1. Summary

Step 11 is complete. The CLI can now log in to a ForgeHub server:
`fg login` runs the RFC 8628 device flow (code card, browser open
attempt, interval/`slow_down`-honouring poll), `fg login --token`
takes a token from stdin for CI, `fg whoami` shows the current
login, and `fg logout [--revoke]` removes the credential — with
`--revoke` also killing the token server-side via a new
`DELETE /tokens/current` self-revoke endpoint. Credentials live in
`~/.config/forgehub/credentials.toml` (dir 0700, file 0600, atomic
writes, permission-enforced reads), keyed by registry. Doctor
gained live cloud checks (reachability, clock skew, token
validity). The whole flow is covered by integration tests that run
the real CLI binary against the real server with a scripted
browser. Separately, this step carries the first-CI-run fix
(§8): the Rust job was green on GitHub; the web job needed Node 24.

## 2. What I built

- `crates/forge-cli/src/cloud.rs` (new, ~975 lines):
  - `ApiClient`: reqwest blocking + rustls, `User-Agent:
    forgehub/<version>`, 5 s connect / 15 s total timeouts, proxy
    env vars honoured (reqwest system-proxy default). GETs retry
    transport errors and 502/503/504 twice with backoff;
    POST/DELETE never retry. Server error envelopes are parsed
    into a typed `ApiFailure`.
  - Registry resolution: `--registry` → `FORGEHUB_REGISTRY` →
    config `server`, validated/normalised (scheme + host, no
    credentials/query/fragment).
  - Credentials store: TOML keyed by registry URL; XDG-aware path;
    dir 0700 / file 0600; atomic write via temp file + rename;
    reads refuse group/other-readable files (TTY fix offer,
    non-TTY error naming `chmod 600`).
  - Device login: prints the verification URL + user code, tries
    to open the browser (`xdg-open`/`open`; skipped silently on
    SSH/headless), polls as a pure state machine
    (`poll_verdict`/`next_interval`, unit-tested) with denied →
    exit 2, expired → exit 1, `slow_down` → interval + 5 s.
  - `--token` reads stdin only (never argv), validates against
    `GET /me` before storing. `FORGEHUB_TOKEN` overrides the file
    for CI and is never stored. Tokens are never printed; output
    shows at most the 8-char prefix, and server messages pass
    through a `redact` helper when a token is in scope.
  - On login, config `server` + `username` are updated (failure
    there is a warning, not a failed login).
- Commands wired in `main.rs`: `login`, `logout`, `whoami`
  (`--json` for whoami emits exactly the spec-cli §3 keys:
  `username`, `registry`, `token_name`, `token_prefix`, `scopes`,
  `expires_at`). New exit-code helper `CliError::auth` (exit 4).
- `doctor` cloud checks (parity.rs): registry reachability via
  `GET /health` (3 s timeout), clock skew vs the server `Date`
  header (warn > 5 min), stored-token validity via `GET /me`
  (401 → fail with the `fg login` fix). No registry configured →
  the previous single offline-safe note; doctor never prompts.
- Server: `DELETE /api/v1/tokens/current` — a bearer token
  revokes itself (204); a session gets 400 `no_current_token`;
  anonymous 401. Documented in `docs/openapi.yaml`. This was
  necessary because token management is session-only by design
  (Step 8), so the CLI previously had no way to revoke at all.
- Tests: `crates/forge-cli/tests/cloud.rs` (7 integration tests,
  real binary + real server + scripted browser) and 5 unit tests
  in cloud.rs; 1 new server test for self-revoke.

## 3. Decisions made

- **HTTP client: reqwest blocking**, constructed lazily inside
  cloud commands only (the roadmap's requirement). Local commands
  never build a client; their cost is only the larger binary
  (measured in §4 — startup still ~4 ms against a 20 ms budget).
  ureq would be smaller, but the roadmap names reqwest, and its
  proxy/redirect behaviour is the better-tested path.
- **Self-revoke endpoint instead of weakening token management:**
  `DELETE /tokens/{id}` stays session-only; `/tokens/current`
  gives a bearer exactly one power — destroying itself. That is
  the smallest possible privilege extension and matches the
  roadmap's `logout --revoke` requirement.
- whoami metadata (token name/scopes/expiry) comes from the
  credentials entry written at login, because no API returns it
  to a bearer. For `--token` logins those fields are honestly
  unknown (name `null`, scopes `[]`, JSON keys still present per
  spec); the username is always verified live via `GET /me`.
- Logout with a failed revoke still removes local credentials
  (with a loud warning and exit 1): trapping a user in a login
  because a server is down would be worse than a token that
  remains revocable from the web UI.
- Registry config key is the existing `server` field (Step 4),
  not a new `registry_url` key — one source of truth; the
  `--registry` flag and `FORGEHUB_REGISTRY` env match the roadmap.

## 4. Verification

- `cargo fmt --all --check` — clean. `cargo clippy --workspace
  --all-targets -- -D warnings` — exit 0 (untruncated count).
- `cargo test --workspace` — **349 passed, 0 failed** (336 at
  Step 10 + 7 CLI integration + 5 cloud unit + 1 server
  self-revoke). The two post-gate cosmetic edits (a test fixture
  string and a local rename, both to satisfy the secret scanner)
  were re-verified with `cargo test -p forge-cli` — 77 passed.
- Web gates — lint, typecheck, test (1 passed), build all pass.
- `scripts/scan-staged.sh` — clean (it initially flagged two
  test-shaped strings; both were rewritten rather than excluded).
- Integration tests cover: full device login → whoami (human +
  JSON key-for-key) → logout --revoke (token then 401s
  server-side; whoami exit 4); denied → exit 2; expired (DB-forced)
  → exit 1; `--token` via stdin; `FORGEHUB_TOKEN` override;
  invalid/unreachable registry errors; credentials 0600/0700
  assertions; chmod 644 → whoami refuses naming the fix; the full
  token asserted absent from all CLI output.
- Manual smoke (real server + CLI, this machine): doctor before
  login (registry ok, clock ok, token "not logged in"), after
  `--token` login (token valid for the user), logout --revoke,
  whoami exit 4 — all as expected.
- **Startup re-measure (acceptance):** release binary `--version`
  mean 4.00 ms / p95 3.85 ms over 30 runs (Step 4: 2.95/3.15 ms;
  budget 20 ms). Binary size 8,523,096 bytes ≈ 8.1 MiB (Step 4:
  3.07 MB; budget 15 MB). The ~1 ms / ~5 MB is the reqwest+rustls
  link cost; local behaviour is otherwise unchanged.

## 5. Acceptance criteria

- [x] `login` → `whoami` → `logout --revoke` works against a real
  local server in tests.
- [x] Credentials file permissions enforced; tokens never printed.
- [x] Local command startup time unchanged (re-measured, §4).

## 6. Security notes

- Tokens exist in exactly three places: the credentials file
  (0600, dir 0700), process memory, and the `Authorization`
  header to their own registry. Never argv, never logs, never
  output (prefix only, 8 chars — the same prefix the server
  itself displays).
- The permission gate refuses to *read* a world-readable
  credentials file rather than silently using it.
- `--token` from stdin only; a token in argv would persist in
  shell history and `ps`.
- `http://` registries are accepted (self-hosting on a LAN is a
  real use case); the token then travels unencrypted — the
  registry URL is shown in the login card and whoami so the user
  can see what they chose. (A warning for non-loopback http is a
  Step 19 hardening candidate.)
- Self-revoke cannot touch any other token (asserted in the
  server test: a second token stays valid).

## 7. Dependencies added

- forge-cli: `reqwest` 0.12 (blocking, json, rustls-tls,
  default-features off), `toml` 0.8, `chrono` 0.4 (std + clock;
  already in the workspace via forge-core).
- forge-cli dev-dependencies: `forge-server` (path), `axum` 0.8,
  `tokio` 1, `tempfile` 3 — integration tests only.

## 8. Corrections to earlier steps

- **CI web job (Step 1's workflow):** the first-ever CI run
  (44482326352, on the Step 10 commit) came back with the Rust
  job fully green (fmt, clippy, tests — 2m53s) but the Web job
  red: Vitest's worker pool fails to start on the pinned Node 20
  (`Failed to start forks worker … Cannot read properties of
  undefined (reading 'bind')`), while the same suite passes
  locally on Node 24. Fix in this commit: CI `node-version`
  20 → 24, with the reason recorded in the workflow file. The
  re-run result will be reported with the Step 11 push.
- Reports for Steps 6–10 recorded "CI has never run"; that is
  now historical — CI exists and its first result is above.

## 9. Known issues / limitations

- Push is no longer blocked: `main` and tags `step-1`…`step-10`
  are on GitHub (verified with `git ls-remote`); this step's
  commit pushes the same way. Lightweight tags need an explicit
  `git push origin --tags` (`--follow-tags` only carries
  annotated tags) — noted for future steps.
- Device-flow `slow_down` handling is unit-tested as a state
  machine but not exercised end-to-end (the CLI honours the
  server interval, so a compliant server never triggers it).
- OS keyring storage remains out of scope (roadmap deferred
  idea); the credentials file is the store.

## 10. Deferred / ideas

- Warn on non-loopback `http://` registries at login — Step 19
  hardening candidate.
- `fg status` (local vs cloud drift) arrives with Step 12's
  push/pull.

## 11. Next step

Step 12: CLI push/pull/remote list/fork.
