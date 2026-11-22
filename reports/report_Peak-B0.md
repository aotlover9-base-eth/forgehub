# Report - Peak B0: CLI audit and baseline

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **WP:** B0 (Phase B - CLI improvements)
- **Status:** Done. Deliverable: `docs/cli/PEAK-AUDIT.md` (new). No product code changed in this WP.

## 1. What was done

- Captured `--help` for the top level and every subcommand family from the built binary; read the clap definitions in `crates/forge-cli/src/main.rs` and the output/error code (`exit.rs`, `cmds.rs`, `cloud.rs`, `cloudrun.rs`, `sync.rs`, `trust.rs`, `tui.rs`) at HEAD.
- Attempted the plan's prescribed live pass (temp `FORGEHUB_HOME` + temp HOME + local server). The server cannot start and the CLI's own writes are denied by the sandbox write fault (PROGRESS.md), so the live pass covered read-only surfaces only: full help surface, `--version`, `list`/`show` on an empty home, and the error text of a denied `init`/`import` (which became finding F12). Server-backed flows were audited from code plus the CI e2e suite. This is recorded in the audit's method section - nothing was claimed as run that was not.
- Re-measured the section 9 budgets that the environment allows (table in the audit, section 1): `fg --version` 16 ms wall on a debug build against a 20 ms release budget; the run-to-prompt timing, the stripped release size, and a fresh server p95 are owed at B9 with the reasons stated.
- Spot-checked `docs/cli-reference.md` (986 lines) and `docs/spec-cli.md` against the binary: current for the audited surface, one real mismatch found (F3: spec-cli lists a `run --version` flag that clap does not define; version selection is the `name@version` syntax).

## 2. Headline findings (full list: audit F1-F14)

- Help is dominated by the four repeated global flags; the top level is a flat 33-command list; no help topics (B2).
- `search` has no filter flags at all; `list` lacks `--sort`/`--visibility`/`--since`/`--limit` (B4, plus new `fg find`).
- No shared output module and no `--color` flag; the section 8.4 colour contract is unimplemented outside the TUI (B1).
- Raw OS errors escape the what/why/fix error standard in store/IO paths (B1; observed live).
- Missing commands the plan calls for: `fg new`, `fg lint`, cloud `fg edit`, `fg config`, `trust show`, `pull --all` (B5, B9, B7, B6).
- Two findings have no owning WP in the plan (F5 cache path/prune, F10 doctor depth); the audit assigns F5 opportunistically to B7 and leaves F10 recorded for the final report rather than dropping it.

## 3. Gates

Docs-only WP; no code changed, so the section 9 code gates are not applicable to this commit. CI for this commit's head runs the full suite against the unchanged code; per the WP-0b lesson its result is eyeballed before B1 starts. (The Phase A summary head `764a8af` had run `44965874894` in progress at this WP's close-out; the A8 head `3bb18b0` is green in run `44965399856`.)

## 4. Next

B1 - output design system for the terminal.
