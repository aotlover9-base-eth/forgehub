# Report - Peak B7: trust and cache surfaces

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **WP:** B7 (Phase B - CLI improvements). Fixes audit F4 and F5 (opportunistic, as PROGRESS.md planned).
- **Status:** Done, pending CI verification of this commit's head (local cargo blocked by the sandbox write fault).

## 1. Trust list: real columns

`fg trust list` now prints an aligned table - SNIPPET (owner/name), PINNED (short hash), VERSION, TRUSTED (date pinned), RUNS, CHANGED-SINCE - instead of the old one-line prose. Changed-since compares the pin against the newest **cached** copy (offline by design; trust surfaces never need the network): `changed (vN cached)`, `unchanged`, or `unknown (not cached)`. `--json` rows gain `changed_since` and `cached_version`.

## 2. `fg trust show <owner/name>` (new)

The pin in full (version, full hash, trusted-since, last run, run count) plus, when the newest cached copy has changed, the **complete diff vs the pinned body** rendered through B6's `render_diff` gutter. Honest edges: no cached copy -> says so and how to get one; changed but the pinned body was evicted -> says a diff cannot be shown rather than inventing one. `--json` carries the record plus `changed_since`, `cached_version` and the raw unified `diff`. A bare name (no owner) fails with the what/fix shape; an unknown pin is exit 3.

## 3. Typed-yes prompt context

The strongest confirmation now carries its context (plan B7): `Run alice/deploy on this machine? alice published it about 2 months ago; it has 4 versions.` - owner name, snippet age (from the API's `created_at`, carried on `Fetched`; empty for cache replays, where the age clause is omitted rather than guessed), and the version count (the linear history's length). Age text comes from a small days-from-civil conversion with a future-date guard. The card itself and the golden bytes are untouched; the yes/no prompt for own snippets is unchanged.

## 4. Cache: `cache path` + `cache prune` (audit F5)

- `fg cache path` prints the cache directory (JSON: `{ "path" }`).
- `fg cache prune --max-size <size>` parses sizes (`500`, `500KB`, `50MB`, `2GB`; binary units, overflow-checked, unit-tested) and evicts LRU via the existing `evict_lru`, reporting before/after sizes and the eviction count. This gives the previously internal-only eviction a user surface alongside `cache list`/`cache clear`.

## 5. B6 clippy fix-forward (disclosed)

CI run 44970723239 for the B6 head failed the Rust Clippy step with eight errors, all in B6 code: `if_not_else` (cmd_lint), `doc_markdown` (the `DiffView` mention in render_diff's doc), `bool_to_int_with_if` x2 (pull --all exit codes, now `i32::from(!errors.is_empty())`), `fn_params_excessive_bools` + `too_many_lines` 140 (cmd_pull_all - now a `PullAllArgs` struct following the `CloudRunArgs` allow precedent, with candidate enumeration, the dry-run plan print and the execute loop extracted; 71 lines), `too_many_lines` 102 (cmd_push - the dry-run report extracted to `push_dry_run`), and `too_many_lines` 105 (dispatch_cloud - the pull arm extracted to `dispatch_pull`). All behaviour-preserving; they ride in this commit for the usual reason (the fault prevents hunk-level splitting), disclosed here and in the B6 report's addendum.

## 6. Gates

- `cargo fmt --check` via local rustfmt: CLEAN.
- Clippy/test/build: via CI for this head (recorded in PROGRESS.md once verified). `Fetched` gained a field; all construction sites (API fetch, offline verify path, test literals) were updated.

## 6. Next

B8 - network failure modes, retries for idempotent calls, the offline-cache staleness note, and the EPIPE mitigation found in B2.

## Addendum (2026-11-22, after CI run 44971342984)

The B7 head's CI failed the Rust Clippy step with two `unreadable_literal` errors (the age helper's civil-date constants). Fixed in the B8 commit (disclosed in its report, section 4); the B8 CI run is the verification for both.
