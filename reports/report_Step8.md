# Report — Step 8: API tokens and device-code login

- **Date/time:** 2026-11-19T12:43:05+05:30
- **Commit:** this report is part of the Step 8 commit — see `git log --oneline -n 1` (tag `step-8`)
- **Status:** ⚠️ Done with caveats — all Step 8 acceptance criteria met and proven below; the caveat is the carried-over push blocker (§9), unchanged since Step 5.

## 1. Summary
The CLI's credential path now exists end to end. Personal API tokens (`fgh_` + 43 base62 chars, ≈256 bits, SHA-256 at rest, shown once) can be created, listed and revoked from a cookie session; the auth extractors accept them as bearer credentials with scope checks, and they are actively refused (403 `session_required`) from every session-only operation. The RFC 8628-style device flow works end to end: an unauthenticated CLI obtains `device_code`/`user_code`, the user looks the code up and explicitly approves or denies it in a session (CSRF-protected), and the CLI polls — with server-side interval enforcement (`slow_down`) — to receive a normal, listed, revocable token named `CLI on <client_name> (<date>)`, delivered exactly once. 311 workspace tests pass, including 8 end-to-end token/device scenarios covering every failure branch in the roadmap's list.

## 2. What I built
- **Tokens** (`src/tokens.rs`): generation by rejection sampling from a CSPRNG (no modulo bias); stored as SHA-256 + 8-char display prefix; scopes normalised at creation (`write` implies `read`, stored `"read write"`); optional expiry 1–365 days; **25 active tokens per user** limit; `GET/POST /api/v1/tokens`, `DELETE /api/v1/tokens/{id}` — all behind `RequireSession` + CSRF, revoking someone else's token is a 404 (no existence leak). Responses use the openapi `ApiToken` shape; the plaintext appears only in the 201 creation response (asserted in tests, as is the absence of any hash field).
- **Bearer auth** (`src/auth.rs`): `AuthContext` now carries `AuthKind::{Session, Token}`. `resolve_auth` prefers an `Authorization` header over the ambient cookie, hashes the presented token, and enforces revocation, expiry and account state on **every** request (revocation is immediate — there is no cache; tested). `last_used_at` touches are throttled to once a minute. `AuthKind::has_scope` centralises scope checks for Step 9's endpoints. `RequireSession` (Step 7) now does its real job: token contexts get 403 `session_required`, so a token cannot list/create/revoke tokens, change the password, log out, or delete the account — all five routes tested with a full-access token.
- **Device flow** (`src/device.rs`): `POST /auth/device/code` (client_name validated; 256-bit device code stored hashed; 8-char user code from the unambiguous alphabet `ABCDEFGHJKMNPQRSTUVWXYZ23456789`, displayed `ABCD-EFGH`; 600 s TTL; verification URIs built from `FORGEHUB_BASE_URL`). `POST /auth/device/token` implements the polling state machine: `authorization_pending`, `slow_down` (interval from `FORGEHUB_DEVICE_POLL_INTERVAL_SECS`, default 5 s, recorded server-side), `access_denied`, `expired_token`, and one-time delivery. `GET /auth/device/lookup` + `POST /auth/device/approve|deny` require a session (+ CSRF for the decisions); approve records the user, deny records the denial; expired codes answer 410 on decision, 404 on lookup (lookup never reveals a code's state).
- **Token minted at delivery, not approval**: approval stores only *who* approved. The plaintext is created when the CLI polls successfully, so nothing sensitive is ever stored between approval and delivery. Delivery is atomic — a new `DeviceCodeRepo::consume` performs `UPDATE … WHERE status='approved'`; a racing poll that loses revokes its own freshly minted token and reports the code as spent (self-review fix, §8).
- **Brute-force defences** (T-ACCT-4): per-IP failed-lookup lockout (`DeviceLimiter`: 10 failures → 5-minute lock, 429 even before the code is checked); per-code attempt counter burns a code after 10 failed decision attempts; codes are single-use and expire in 10 minutes.
- **Repository/state plumbing**: `TokenRepo::count_active_for_user`, `DeviceCodeRepo::consume`; `AppState::new(config, repos, pool)` constructor now derives the throttles from config, replacing three hand-built struct literals (main + two test harnesses). `sqlx` added as a dev-dependency so integration tests can force expiry states directly in the DB instead of waiting or mocking.

## 3. Decisions made
- **Consumed codes report `expired_token` on replay.** The openapi status enum has no `consumed`; a spent code is dead exactly like an expired one, and distinguishing them would only help replay probing. Documented here and in the module docs.
- **Device tokens get scopes `read write` and no expiry**, matching the lookup response the user approves (`client_name`, scopes shown before approval — nothing is granted that the approval screen didn't show).
- **openapi.yaml and spec-cli.md needed no changes**: the contract from Step 2 already described this step's behaviour exactly, and the implementation matches it field-for-field (including the 410 on approve). The CLI-side login UX documentation belongs to Step 11, where the CLI commands land.

## 4. Verification
| Gate | Command | Result |
|------|---------|--------|
| Format | `cargo fmt --all --check` | pass |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | pass, zero warnings |
| Tests | `cargo test --workspace` | **311 passed, 0 failed** (forge-server: 18+5 unit, 9 auth HTTP, 7 HTTP, 8 token/device HTTP; forge-core 199; forge-cli 65) |
| Web | `npm run lint && npm run typecheck && npm run test && npm run build` | pass (1 web test) |
| Secret scan | `scripts/scan-staged.sh` | clean |
| Token tests | `tests/tokens_device.rs` | CRUD + bearer `/me` (200, no `csrf_token`), garbage/truncated tokens 401, list shows metadata only (no plaintext, no hash), revoke → bearer 401 immediately, foreign token id → 404, force-expired token → 401 |
| Session-only tests | same file | full-access bearer token gets 403 `session_required` on list/create/revoke tokens, PATCH /me, and logout — while still passing `/me` |
| Device tests | same file | full flow with a fake CLI client (code → pending poll → lookup shows client_name/scopes → approve without CSRF 403 → approve with CSRF + lowercase/unhyphenated code 204 → poll delivers `fgh_` token once → replay = `expired_token` → token listed as `CLI on …` and works); denied flow → `access_denied`; unknown device code → 400, unknown user code → 404; `slow_down` inside a 2 s interval and recovery after it; expired code → approve 410, poll `expired_token`; 10 wrong lookups → 11th is 429 |

## 5. Acceptance criteria
- [x] Device flow works end to end in tests, including every failure branch. — §4: pending, slow_down, denied, expired, replayed, brute-force lockout, unknown codes.
- [x] Tokens are hashed at rest and shown once. — SHA-256 + prefix only in the DB (Step 7's DB-inspection pattern; generation test asserts hash ≠ plaintext derivation), plaintext only in the creation response (asserted absent from list).
- [x] Token cannot mint or revoke tokens. — `tokens_cannot_manage_tokens_or_account` covers mint/revoke plus password change and logout, all 403.

## 6. Security notes
Implements T-ACCT-3's server half (hashed, scoped, revocable tokens; revocation immediate; tokens cannot manage tokens or the account) and T-ACCT-4 (unambiguous 8-char codes, 600 s expiry, single use, per-code and per-IP attempt lockouts, explicit approval with the client name and scopes shown at lookup, poll-interval enforcement, device codes hashed). Two self-review findings fixed before commit: the delivery race (§2) and — from Step 7's pattern — nothing in this step logs a token, code, or hash.

## 7. Dependencies added
None at runtime. Dev-only: `sqlx` 0.8 (sqlite, runtime-tokio) for integration tests that force expiry states in the DB.

## 8. Corrections to earlier steps
- **Step 6 (repo)**: `DeviceCodeRepo::record_poll` already existed in the Step 6 trait — a duplicate added early in this step was removed again before commit; the only net repo changes are `count_active_for_user` and `consume`.
- **Step 7 (auth)**: `AuthContext` restructured around `AuthKind` (session vs token) and `RequireSession` became a concrete `{user, session}` extractor with the 403-for-bearer behaviour Step 7's report promised. All Step 7 HTTP tests pass unchanged against the refactor.

## 9. Known issues / limitations
- **Push blocker (carried over, unchanged):** repo returns 404, no GitHub auth on this machine; CI has never run. Needed: create the empty private repo `aotlover9-base-eth/forgehub` and authenticate (`gh auth login`, PAT via Secure Vault, or SSH key). Steps 1–8 (8 commits, tags `step-1`…`step-8`) publish with one push.
- `POST /auth/device/code` is unauthenticated by design (RFC 8628); abuse is bounded by the global API rate limit (600/min/IP by default). A dedicated per-IP creation cap can join Step 19's hardening if wanted.
- Scope *enforcement on data endpoints* lands with the endpoints themselves (Step 9): `has_scope` exists and tokens carry scopes, but no read/write data routes exist yet to gate.

## 10. Deferred / ideas
None.

## 11. Next step
Step 9 — the snippet and version API: create/read/update (immutable versions), metadata PATCH, rename redirects, soft delete, versions list, server-side diff, raw download, and `/me/snippets`, all reusing forge-core validation and this step's scope-aware extractors.
