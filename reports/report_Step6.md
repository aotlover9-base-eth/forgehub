# Report — Step 6: server foundation

- **Date/time:** 2026-11-19T12:12:29+05:30
- **Commit:** this report is part of the Step 6 commit — see `git log --oneline -n 1` (tag `step-6`)
- **Status:** ⚠️ Done with caveats — all Step 6 acceptance criteria met and proven below; the caveat is the carried-over push blocker (§9), unchanged since Step 5's corrections.

## 1. Summary
`forge-server` is now a real HTTP server instead of the Step 1 skeleton. It loads validated configuration from the environment, opens SQLite (WAL, foreign keys on), runs the full spec-data-model schema as migration `0001_init.sql`, and serves the API skeleton: `GET /api/v1/health`, `GET /api/v1/version`, the openapi error envelope for everything else, request ids, the security-header set with the CSP defined in exactly one constant, a 256 KiB body limit, a 30 s timeout, gzip/brotli compression, and token-bucket rate limiting — plus static hosting of the web build with SPA fallback and immutable caching for hashed assets. All database access sits behind repository traits (`UserRepo`, `SessionRepo`, `TokenRepo`, `DeviceCodeRepo`, `SnippetRepo`, `ReportRepo`) with SQLite implementations, so handlers written from Step 7 on never touch SQL. 283 workspace tests pass, and the real binary was run and probed by hand (§4).

## 2. What I built
- **Config** (`src/config.rs`): every `FORGEHUB_*` variable from the roadmap prompt — `ENV` (dev|prod), `BIND` (default `127.0.0.1:8080`), `BASE_URL`, `DATABASE_URL`, `SECRET_KEY`, `ALLOW_SIGNUP` (default true), `WEB_DIR` (default `web/dist`), body limit (default 256 KiB), rate limit (default 600/min), request timeout (default 30 s) — parsed and validated with named errors. In prod a missing or < 32-byte secret refuses startup; in dev it warns. `Config::summary()` prints the whole configuration for the startup log with the secret shown only as set/unset + length; a test asserts the value itself never appears. `.env.example` documents every variable.
- **Logging** (`src/main.rs`): `tracing` with an `EnvFilter` (`RUST_LOG`); pretty in dev, JSON in prod. Each request runs inside a span carrying its request id. Graceful shutdown on Ctrl+C and SIGTERM (drain, then exit) — proven in §4.
- **Database** (`src/db.rs`, `migrations/0001_init.sql`): sqlx pool (max 8) with WAL, `synchronous = NORMAL`, foreign keys ON, 5 s busy timeout; `sqlx::migrate!` at startup. The migration creates the complete spec-data-model schema: users, sessions, api_tokens, device_codes, snippets, snippet_versions (immutable — no update path anywhere), snippet_tags, stars, renames, reports, all spec indexes, and the contentless FTS5 `snippets_fts` (`unicode61 remove_diacritics 2`). Timestamps are application-supplied UTC ISO-8601 text per the spec conventions.
- **Repositories** (`src/repo.rs`): the six traits above, one `SqliteStore` implementing all of them, a `Repos` bundle of `Arc<dyn Trait>`. Snippet writes are transactional: `create_with_version` and `add_version` write snippet + version + tags + FTS in one transaction; `star`/`unstar` maintain `star_count` in the same transaction and are idempotent; unique violations map to `RepoError::Conflict`, missing rows to `RepoError::NotFound`. FTS maintenance implements the spec §3 rule (public + latest version + not hidden + not deleted) in exactly one place — see §8 for the contentless-delete subtlety.
- **HTTP** (`src/http.rs`, `src/error.rs`, `src/ratelimit.rs`): `ApiError` renders the openapi envelope `{"error": {"code", "message", "details"?}}`; sqlx errors become a generic 500 (cause logged server-side only). Health runs a real `SELECT 1` and returns 503 with the envelope when the DB is down. Version returns `{version, git_sha}` (`FORGEHUB_GIT_SHA`, else `dev`). Unknown `/api/v1/*` paths return the envelope, never HTML. Middleware: `x-request-id` (uuid v4) on every response; security headers on every response (CSP in the single constant `http::CSP`: `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'`, plus nosniff, `Referrer-Policy: strict-origin-when-cross-origin`, `Permissions-Policy: camera=(), microphone=(), geolocation=()`, `X-Frame-Options: DENY`); rate limiting by SHA-256 hash of the `Authorization` header, else client IP, with `RateLimit-Limit/Remaining` headers and `Retry-After` on 429. CORS is deliberately absent (same-origin only). Static serving via `ServeDir` with SPA fallback; `/assets/*-<hash>.*` gets `public, max-age=31536000, immutable`, everything else `no-cache`; when the web build is absent the server logs a warning and serves API only. The API layer stack (body limit, timeout with a 408 envelope on elapse, rate limit) is one shared function used by both the production router and the tests; compression sits one level up so it covers static files too.
- **Docs & packaging**: `docs/running.md` (config table, dev workflow with the Vite proxy, production, Docker, backup note) and a multi-stage `Dockerfile` (node web build → rust server build → slim Debian runtime, non-root user, `/data` volume, healthcheck). Docker is not installed on this machine, so the image itself was not built here — the Dockerfile is written against the documented commands the web/Rust builds already prove locally; flagged honestly rather than claimed.

## 3. Decisions made
- **Dynamic sqlx queries only** (no `query!` macros): the build never needs a live `DATABASE_URL`, which keeps CI and the Docker build hermetic. The cost — no compile-time SQL checking — is paid back with repository tests against real migrated databases.
- **Compression at the app level, not the API level**: tower-http's default predicate skips bodies under 32 bytes, so the 25-byte health JSON is (correctly) not gzipped; the version response and the 219 KB web bundle are — both proven on the wire (§4).
- **Timeout handling**: tower's `TimeoutLayer` fails with a boxed error that axum's `Router::layer` cannot accept directly; it is paired with a `HandleErrorLayer` inside a `ServiceBuilder` (408 `request_timeout` envelope on elapse, generic 500 otherwise) so the composed layer is Infallible-error as axum requires.
- **Rate-limit keys never hold raw credentials**: the bucket key is `auth:<sha256 of the Authorization header>` or `ip:<addr>`; a test asserts the raw token never appears in a key.

## 4. Verification
| Gate | Command | Result |
|------|---------|--------|
| Format | `cargo fmt --all --check` | pass |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | pass, zero warnings |
| Tests | `cargo test --workspace` | **283 passed, 0 failed** (forge-core 199 incl. 3 new corpus tests, forge-cli 65, forge-server 12 unit + 7 HTTP integration) |
| Web | `npm run lint && npm run typecheck && npm run test && npm run build` | pass (1 web test) |
| Secret scan | `scripts/scan-staged.sh` on the staged diff | clean (after the §8 scanner fix) |
| Real server | `forge-server` with `FORGEHUB_BIND=127.0.0.1:18099`, temp DB | startup log shows the secret-free summary (`secret_key=not set`), the dev weak-secret warning, "database ready and migrations applied", the API-only warning, "listening on http://127.0.0.1:18099" |
| Endpoints | `curl` against the running server | `/api/v1/health` → `{"status":"ok","db":"ok"}`; `/api/v1/version` → `{"version":"0.1.0","git_sha":"dev"}`; `/api/v1/zzz` → 404 `{"error":{"code":"not_found","message":"No such API route"}}` |
| Headers | `curl -sI /api/v1/health` | CSP exactly as specified, `x-content-type-options: nosniff`, `x-frame-options: DENY`, `referrer-policy`, `ratelimit-limit: 600`, `x-request-id: <uuid>` |
| Prod refusal | `FORGEHUB_ENV=prod` without a secret | `Error: FORGEHUB_SECRET_KEY must be set to at least 32 bytes in production (got 0)` — refuses to start |
| Database | python sqlite3 on the server's DB file | all 10 spec tables + FTS shadow tables present, `_sqlx_migrations` shows version 1 applied, `PRAGMA journal_mode` = `wal` |
| Static + SPA | server with `FORGEHUB_WEB_DIR=web/dist` (real Vite build) | `/` serves index.html (`cache-control: no-cache`), `/some/deep/route` → 200 (SPA fallback), `/assets/index-CSLqDigW.js` → `cache-control: public, max-age=31536000, immutable` |
| Compression | `curl -H "Accept-Encoding: gzip"` | `content-encoding: gzip` on both the JS bundle and `/api/v1/version` |
| Shutdown | `kill -TERM` | log: "shutdown signal received; draining connections" → "forge-server stopped" |
| HTTP tests | `tests/http.rs` (real server on an ephemeral port, reqwest + one raw-socket test) | health, version, envelope 404, all security headers + request id, 429 with `Retry-After` at limit 3/min, 413 for a 2 KiB body against a 1 KiB limit (through the exact production layer stack around a test echo route), gzip on the wire |

## 5. Acceptance criteria
- [x] `cargo run -p forge-server` starts, migrates, serves `/api/v1/health`. — Proven by hand (§4) and by the HTTP test suite.
- [x] Security headers and body limit verified by tests. — `security_headers_and_request_id_are_present`, `body_limit_rejects_oversized_payloads`; rate limit, envelope, gzip likewise.
- [x] Repository traits isolate SQL from handlers. — Handlers (health/version) use the pool only for the health `SELECT 1`; all entity access is behind the six traits with a single SQLite implementation.

## 6. Security notes
The startup summary can never leak the secret (tested). Rate-limit keys are hashes, never raw tokens. Error responses never contain SQL or internal detail (sqlx errors map to a generic 500; the cause stays in the server log). The FTS index only ever contains public snippets' latest versions — enforced transactionally in the repository, tested in both directions (publish → indexed; new version → re-indexed; privatise → removed). The body limit, timeout, and rate limit are on the API from the first step that exposes it, not retrofitted later.

## 7. Dependencies added
Server: `axum` 0.8, `sqlx` 0.8 (runtime-tokio, sqlite, migrate, macros — dynamic queries only), `tokio` 1 (full), `tower` 0.5 (timeout, util), `tower-http` 0.6 (compression gzip+br, fs), `tracing` 0.1 + `tracing-subscriber` 0.3 (env-filter, json), `serde`/`serde_json`, `anyhow` 1, `thiserror` 2, `async-trait` 0.1, `chrono` 0.4 (clock only), `uuid` 1 (v4), `sha2` 0.10 (rate-limit keys). Dev: `reqwest` 0.12 (json, gzip), `tempfile` 3.

## 8. Corrections to earlier steps
- **Step 3 (forge-core secret scanner) — two false-positive classes fixed, found by dogfooding `scan-staged.sh` on this step's diff.** (a) *Code-shaped assignment values*: `secret_key = get("FORGEHUB_SECRET_KEY")`, `bucket.tokens = f(x, y)` and `let ip_key = key_for(…)` were flagged `secret_assignment` although the values are expressions, not literals; the assignment rule now skips values containing `(`, `)`, `,` or `::` — the same exemption `PasswordLiteral` already had. (b) *Substring name matching*: `tokenize = 'unicode61 …'` (the FTS5 tokenizer clause in this step's migration) was flagged because `is_secret_named` used `contains("token")`; `token`/`auth` now must be whole name segments (`GITHUB_TOKEN`, `AUTH_TOKEN` still detected; `tokenize`, `tokens`, `author` clean). Three corpus tests added (`code_shaped_assignments_are_clean`, `token_substring_names_are_clean`, `segmented_secret_names_still_detected`); all pre-existing corpus positives still pass, so detection was narrowed, not weakened.
- **Step 6-internal, caught by tests before commit**: the first FTS sync used `DELETE FROM snippets_fts WHERE rowid = …`, which SQLite rejects on contentless tables ("cannot DELETE from contentless fts5 table"). The sync now captures the pre-mutation snapshot and removes the old row with the FTS5 `delete` command carrying the originally indexed values; a test proves a superseded version's body stops matching.

## 9. Known issues / limitations
- **Push blocker (carried over, unchanged):** `origin` is set, but the repo `aotlover9-base-eth/forgehub` returns 404 and this machine has no GitHub authentication, so `git push origin main --follow-tags` fails with `fatal: could not read Username for 'https://github.com': No such device or address`. CI has never run. Exactly what is needed: (1) create the empty private repo on GitHub (no README/.gitignore), and (2) authenticate this machine — `gh auth login`, a PAT via the Secure Vault, or an SSH key + SSH remote. Steps 1–6 (6 commits, tags `step-1`…`step-6`) publish with one push once both exist.
- **Dockerfile not build-tested**: no Docker on this machine (noted in §2). It will be exercised in Step 21's release pipeline work at the latest.
- Rate limiting is in-memory per process; a multi-node deployment needs a shared store (recorded in `docs/running.md` and the threat model).
- No authentication exists yet — that is Step 7 (web sessions) and Step 8 (tokens/device codes). The repository layer for both already exists and is tested.

## 10. Deferred / ideas
None.

## 11. Next step
Step 7 — web account auth and sessions: signup/login/logout with argon2id, cookie sessions (hashed ids, CSRF), the `/api/v1/auth/*` and `/api/v1/me` endpoints from openapi.yaml, and account deletion.
