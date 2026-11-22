# Peak C8 - Usage stats the user owns

Commit: `peak C8: usage stats` (hash recorded in PROGRESS.md after push)
Date: 2026-11-22

## What shipped

The plan's three clauses, each kept literally:

- **`fg stats` stays local.** It already read only the on-machine
  history database (runs per snippet, last run, the saved-time
  formula); C8 changed nothing about it, and the CLI reference now
  says so in words: local stats never leave the machine.
- **Cloud runs are reported, best-effort, online only.** When a
  cloud run completes, the CLI posts one small report (`POST
  /me/runs`: ref, version, commands, duration, success, and the
  estimated saved time computed with the exact `fg stats` formula
  and the user's own configured overhead). Reporting shares the
  C7 reporting path (`report_run_outcome`): it never fails a run,
  never happens on the offline cache path, and local runs are
  never reported at all.
- **Cloud stats count only the user's own runs.** Reports are
  stored per runner (migration `0006_run_reports.sql`), and `GET
  /me/stats` aggregates only the caller's rows. Someone else
  running your snippet enters THEIR stats, never yours - proven
  by test. Aggregates: totals (runs, commands, time spent,
  failures, estimated saved) plus per-snippet rows (runs,
  commands, failures, last run), most runs first.

The web Library page gains a **"Your library"** summary card:
the four headline numbers, the top five workflows with last-run
dates, an explanation of what is counted, and an empty state
that points at `fg stats` for local runs.

Reports are validated hard (ref must be owner/name; commands,
duration and saved-time caps; version range) - a buggy or
hostile client cannot write nonsense into the table.

## Tests

- Server integration (`tests/run_stats.rs`): +2 tests -
  aggregation math across two refs with a failure, the
  own-runs-only rule (bob's run of anna's snippet counts for
  bob, not anna), and the validation matrix (bad refs,
  negative/huge numbers 400; anonymous 401; nothing recorded
  by rejected reports).
- CLI integration: +1 test in `tests/cloudrun.rs` - stats are
  empty before a run, and after one confirmed cloud run the
  API shows the run with the right ref, a positive saved-time
  estimate and a last-run timestamp.
- Web: +2 tests (totals + top workflows rendering incl. the
  failure line; the empty state explaining local vs cloud).

## Gates (run locally, this machine)

- `cargo fmt --all --check` clean; `cargo clippy --workspace
  --all-targets -- -D warnings` clean (run_fetched's reporting
  was extracted into `report_run_outcome` in-WP).
- `cargo test --workspace`: **528 passed, 0 failed** (525 at C7).
- Web: lint (check-tokens initially caught inline font-size
  literals in the new card; fixed to the typescale token),
  typecheck, **78 tests passed**, build - all green.
- OpenAPI gained `POST /me/runs`, `GET /me/stats` and the
  `WorkflowStat` schema; the CLI reference's stats section now
  documents the local/cloud split.

## Notes

- The saved-time estimate is computed once, by the CLI at run
  time (per-run, clamped at zero), so the web number can differ
  by small amounts from a fresh `fg stats` aggregate if some
  runs "lost" time. The formula and the user's own overhead
  setting are honoured; the card explains the estimate in the
  same words as `fg stats --explain`.
- Snippet fetches with `intent=run` (the pre-existing popularity
  counter) are a different thing and were left alone: they count
  downloads by anyone, not the owner's own usage.

## Also in this commit

- PROGRESS.md records C7's CI verification (run 44983374639,
  fully green, all 5 jobs) which completed after the C7 commit
  landed.
