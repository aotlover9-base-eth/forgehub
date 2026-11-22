# Report - Peak B6: sync and status quality

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **WP:** B6 (Phase B - CLI improvements). Fixes audit F6.
- **Status:** Done, pending CI verification of this commit's head (local cargo blocked by the sandbox write fault). Also carries the B5 clippy fix-forward (see section 4).

## 1. Status: exact next command per row

- `SyncState::next_command(owner, name)` maps each state to the exact command that resolves it: ahead/local-only -> `fg push <name>`, behind/remote-only -> `fg pull <owner>/<name>`, diverged -> `fg diff <name>`, unchanged -> none.
- `fg status` human rows append `  next: <command>` when there is one; `--json` rows gain a `"next"` field (string or null). Unit-tested for all six states.

## 2. push/pull --all + dry-run + overwrite confirmation

- **`fg pull --all`** (new): enumerates tracked snippets (via sidecars) plus remote-only snippets of mine - the same set `fg status` shows - then **fetches and classifies everything before changing anything**: new / update (local untouched since sync) / risky (local carries unsynced changes) / unchanged. Risky copies are listed up front and need **one shared confirmation** (or `-y`/`--force`); declining skips only the risky ones. Exit 1 when any fetch failed, with the errors itemised. `--json` reports pulled/unchanged/skipped_risky/errors.
- **`fg pull --dry-run`** (single and --all): prints the classification plan and the counts, changes nothing.
- **`fg push --dry-run`** (single and --all): `push_plan` runs the local validate + secret scan (`checked_local`, so a dry run still refuses secrets) and a read-only GET, reporting create/update/visibility/unchanged with the resulting version. `--as` is honoured for single pushes; `--all` prints one row per snippet.
- The single-pull core was factored into `fetch_validated` + `save_pulled`, shared by pull, pull --all, pull --dry-run and (already) the B5 cloud edit's building blocks - one validation path for untrusted remote text.

## 3. Diff gutter parity with the web DiffView

- New `sync::render_diff(unified, style)`: the web component's discipline in the terminal - the +/- sign moves into a **gutter column**, additions painted success, deletions error (never colour alone: the gutter symbol differs too, and the deletion gutter is U+2212 in Unicode mode exactly like the web's), hunk headers accented, context aligned at the gutter column. Every line is **display-escaped before painting** (Step 19 T-EXEC-3 unchanged).
- Applied in `fg diff` (live style) and in the run confirmation card's changed-since-pinning diff (PLAIN style, since the card is a style-free renderer by design). The card's security test (hostile ANSI/bidi diff line) still passes by construction: escaping happens inside render_diff before any gutter is added.
- Unit tests pin the gutter output in ASCII and Unicode modes.

## 4. B5 clippy fix-forward (disclosed)

CI run 44970331789 for the B5 head failed the Rust Clippy step with two errors, both in B4/B5 code: `sort_by_key` for the recent sort (now `sort_by_key(Reverse(modified))`) and `bool_to_int_with_if` in `cmd_lint`'s exit-code computation (now `i32::from(!lints.is_empty())`). Both behaviour-preserving; they ride in this commit for the same reason as the B4 fixes (the fault prevents hunk-level splitting), and this section is the disclosure. The B5 report carries a matching addendum.

## 5. Gates

- `cargo fmt --check` via local rustfmt: CLEAN.
- Clippy/test/build: via CI for this head (recorded in PROGRESS.md once verified). Live sync runs against a dev server remain blocked by the sandbox fault and are owed with the B9 verification pass.

## 6. Next

B7 - trust and cache surfaces.

## Addendum (2026-11-22, after CI run 44970723239)

The B6 head's CI failed the Rust Clippy step with eight errors, all in B6 code (if_not_else, doc_markdown, bool_to_int_with_if x2, excessive bools + too_many_lines on cmd_pull_all, too_many_lines on cmd_push and dispatch_cloud). All fixed behaviour-preservingly in the B7 commit - the full list is in the B7 report, section 5; the B7 CI run is the verification for both.
