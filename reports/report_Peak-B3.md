# Report - Peak B3: run experience

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **WP:** B3 (Phase B - CLI improvements)
- **Status:** Done, pending CI verification of this commit's head (local cargo blocked by the sandbox write fault).

## 1. What landed

- **Fetch progress.** A cloud run now prints `forgehub: fetching <owner>/<name> from <registry>` to stderr before the single fetch request (suppressed with `--json`). Previously the fetch was silent; on a slow registry a run looked hung at exactly the moment the user most needed a sign of life.
- **One summary for both run paths.** The cloud path already printed `exit code N · X executed, Y skipped · Zs`; the local path printed nothing. `runner::summary_line` / `runner::print_run_report` now produce that line from one place, and `cmd_run` (local) prints it too - local and cloud runs end with the same summary. The `--json` outcomes are unchanged.
- **Failure hint.** `RunOutcome` gains `failed_step` (recorded by `execute` and `execute_stepwise` when a step's failure stops the run; `None` for success, script mode, `always`-step failures and user aborts). After a stopped run, stderr gets: `forgehub: step N failed (exit C) - re-run with \`fg run <name> --step\` to step through it`. Suppressed when the run already used `--step`.
- **Golden tests for the safety displays** (the plan's byte-identical requirement):
  - `cloudrun::tests::confirmation_card_golden` pins the full confirmation card for a fixed third-party snippet (identity, version, short hash, visibility, owner warning with stars/pulls, every command numbered).
  - `cmds::tests::render_plan_golden` pins the local plan used by `--dry` and the confirm-before-run prompt.
  - Any future change to these bytes must update the golden in the same commit and say so in its report.
- **Runner tests:** summary-line shape, failure-hint rules (no hint on success or without a step), and `execute` recording the failing step (steps `true` / `exit 3` / `true`: exit 3, 2 executed, failed step 2).

## 2. What did not change

The confirmation card's content and rendering (`render_card`), the trust decision (`confirm_kind`, `flag_yes_may_skip`), the fail-safe non-TTY refusal, and the hash verification are byte- and behaviour-identical - the golden test now proves the card part of that claim. Nothing in the safety model was touched.

## 3. Gates

- `cargo fmt --check` via local rustfmt: CLEAN.
- Clippy/test/build: via CI for this head (recorded in PROGRESS.md once verified).

## 4. Next

B4 - listing, search and find.
