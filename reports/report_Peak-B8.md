# Report - Peak B8: network resilience

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **WP:** B8 (Phase B - CLI improvements).
- **Status:** Done, pending CI verification of this commit's head (local cargo blocked by the sandbox write fault).

## 0. What already existed (verified in source, not rebuilt)

`ApiClient` already had: 5 s connect / 15 s total timeouts, GET-only retries on transport errors and 502/503/504 (3 attempts, backoff), writes never retried, `Retry-After` parsing, and `api_fail` already mapped 401/404/412/422/429/0 to what/why/fix messages. B8 closes the remaining gaps below instead of duplicating any of that.

## 1. Failure modes, completed

- **429 is now retried for GETs**, waiting out the server's `Retry-After` (capped at 5 s so a hostile or broken server cannot park the CLI). Writes remain never-retried - retries stay strictly on idempotent calls.
- **5xx gets a real message**: new `api_fail` arm - "this is the server's problem, not your snippet's - reads were already retried; wait a moment and try again, and run `fg doctor` if it persists" (previously the catch-all printed a bare status/code with no fix).
- **Malformed responses are detected at the boundary**: `send` now reads the body as text and parses it; a success status with a non-JSON, non-empty body returns a `malformed_response` failure ("the server at ... returned a response that is not valid JSON") with its own `api_fail` arm, instead of a silent `Null` document confusing callers downstream. Error-status bodies keep the tolerant parse (their fallback message already existed).
- **Transport errors name their mode**: timeout ("request to ... timed out"), connect ("cannot connect to ... (connection refused or DNS failure)"), or the raw error otherwise - replacing the single "cannot reach" dump.
- **Server down / slow** were already covered by the connect/total timeouts plus the status-0 arm ("check the registry URL and your network; local commands keep working offline"); the mode naming above makes the slow case legible.

## 2. Offline cache: honest staleness

- `CachePointer` records `fetched_at` (ISO-8601 UTC) from Peak B8 on (`#[serde(default)]`, so pre-B8 pointers still load and read as empty). `cache_latest` surfaces it on `CachedSnippet`, and `fetch_offline` attaches it to the replayed `Fetched` - only when the pointer is the version being replayed, so a pinned older version never borrows the newer fetch time.
- The run card's cache line changes from "(from the local cache - offline replay)" to **"(from cache, fetched 2026-11-22 01:23 UTC - may be outdated)"**, or "(from cache - may be outdated)" for pre-B8 cache entries with no recorded time. The confirmation-card golden is unaffected (its fixture is not from cache).

## 3. EPIPE: pipelines end quietly (the B2 e2e race, closed)

Since B2's investigation, a closed output pipe (`fg diff ... | grep -q`) panicked the CLI with "failed printing to stdout" and, under `pipefail`, failed work whose content was correct. The crate forbids unsafe code, so no SIGPIPE reset; instead `main` installs a panic hook that stays silent **only** for the print-failure payload (every other panic keeps the stock hook verbatim), runs the CLI under `catch_unwind`, and maps exactly that payload to a quiet exit 0. Any other panic is resumed untouched - real bugs still look like bugs.

## 4. B7 clippy fix-forward (disclosed)

CI run 44971342984 for the B7 head failed the Rust Clippy step with two `unreadable_literal` errors - the days-from-civil constants in B7's age helper (`146097`, `719468`, now written `146_097` / `719_468`). Behaviour-identical; rides in this commit for the usual reason, disclosed here and in the B7 report's addendum.

## 5. Gates

- `cargo fmt --check` via local rustfmt: CLEAN.
- Clippy/test/build: via CI for this head (recorded in PROGRESS.md once verified). The retry/malformed paths are exercised by the client's existing unit tests plus CI's e2e against the real server; a deliberately misbehaving server (429/malformed drills) is not part of the suite and is recorded as such.

## 5. Next

B9 - `fg config`, release measurements, man regeneration, docs refresh; then the Phase B summary and tag.
