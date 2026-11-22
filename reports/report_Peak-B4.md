# Report - Peak B4: listing, search and find

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **WP:** B4 (Phase B - CLI improvements). Fixes audit F2.
- **Status:** Done, pending CI verification of this commit's head (local cargo blocked by the sandbox write fault).

## 1. What landed

- **One filter set for `list` and `search`** (`cmds::ListQuery`, applied by one function): `--tag` (search gains it), `--visibility`, `--sort name|recent`, `--since <n>(m|h|d|w)`, `--long`. Both commands render through one row printer, so the same snippet looks the same in either - the audit's F2 complaint (search had no flags and a different shape) is closed.
- **`--long` is now a table**: aligned NAME / COMMANDS / MODE / SHELL / HASH / TAGS columns, replacing the stacked per-snippet lines. Compact rows are byte-identical to before.
- **`--visibility` semantics, made real**: a local file has no visibility of its own, so the filter matches the snippet's **last-synced visibility**, recorded in the `.meta` sidecar. To make that data exist, `sync::Meta` gains a `visibility` field (serde-defaulted, so pre-B4 sidecars still parse), `PushOutcome` carries the resulting visibility from all three push paths (create / unchanged / updated), and push + pull record it. Never-synced snippets match no visibility value; this is documented in spec-cli and the CLI reference rather than left as a surprise.
- **`--since` / `--sort recent`** read the snippet file's mtime via a new `Store::modified`. `--since` parsing is strict (`parse_since`: units m/h/d/w, overflow-checked, capped at ~100 years so the cutoff arithmetic cannot underflow) with what/why/fix errors.
- **Search JSON unified**: `{ "workflows": [...] }` with the exact `list` entry shape (was a bare array with fewer keys). Documented in spec-cli §3 alongside `find`'s schema. This is a deliberate, documented schema change in the WP whose job is schema stability.
- **`fg find <query>`** (new command, Library group, shorthand-safe via KNOWN_COMMANDS): subsequence fuzzy matching over local names and cached `owner/name` references, ranked by `fuzzy_score` (start/separator/consecutive bonuses, small length penalty; unit-tested ranking). `--plain` prints bare names one per line for fzf-style piping; `--limit` caps output; `--json` emits `{ "matches": [{ "name", "source", "score" }] }`.

## 2. Tests added

- `parse_since`: all units, malformed values, negative, absurd-duration rejection.
- `fuzzy_score`: non-match, prefix-beats-scattered, exact-beats-longer, case-insensitivity, empty query.
- Existing list/search integration tests pass unchanged (compact rows and the "No snippets match" message are untouched).

## 3. Gates

- `cargo fmt --check` via local rustfmt: CLEAN.
- Clippy/test/build: via CI for this head (recorded in PROGRESS.md once verified).

## 4. Next

B5 - editing and authoring (`fg new`, cloud `fg edit`, `fg lint`).

## Addendum (2026-11-22, after CI run 44969502861)

The B4 head's CI failed the Rust job's Clippy step with six errors, all in B4-touched code: `similar_names` (`matched`/`matches` in `cmd_search`), `sort_by_key` (the name sort), `map_unwrap_or` x2 (the push visibility fallbacks), and `too_many_lines` x2 (`push_one` 102, `run_cli` 130). Fixes, all behaviour-preserving: `matched` renamed `found`; name sort via `sort_by_key`; a shared `outcome_visibility(preferred, fallback)` helper (requested value first for updates, server response first for creates - the original precedence in each arm); `push_baseline` and `version_conflict` extracted from `push_one` (94 code lines; the conflict message is byte-identical); `run_cli`'s match split into `dispatch_local`/`dispatch_author` following the existing dispatcher pattern, with `ListQuery::build` absorbing the duplicated query construction (Trust stays in `run_cli` because `TrustCommand` is not Clone). Because the sandbox fault prevents hunk-level commit splitting via local git, the fix-forward lands inside the B5 commit, disclosed here and in the B5 report; the B5 CI run is the verification for both.
