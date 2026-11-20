# Report - Step 4: local CLI core (init, record, run, manage)

- **Date/time:** 2026-11-19T10:06:42+05:30
- **Commit:** this report is part of the Step 4 commit - see `git log --oneline -n 1` (tag `step-4`)
- **Status:** ⚠️ Done with caveats - all Step 4 acceptance criteria met and proven below; caveats are the carried-over push blocker (§9) and two documented scope notes (§9).

## 1. Summary
`forgehub` (alias `fg`) now works fully offline. `crates/forge-cli` implements storage under `$FORGEHUB_HOME`, `init`, the shell-session recorder with secret refusal, the execution engine (steps mode with conditions/retries/timeouts/background/parallel/always, script mode via ADR 0004's private temp file), `run` plus the `fg <name>` shorthand behind the confirmation card, and `list`/`show`/`search`/`tag`/`rename`/`delete`/`edit` - all with the spec-cli exit codes and `--json` shapes. 24 new CLI tests (13 unit + 11 end-to-end against the real binary) pass; startup timings are well inside budget (below).

## 2. What I built
- `src/store.rs` - `Store` rooted at `$FORGEHUB_HOME` or `~/.forgehub`: `snippets/` (one YAML file per snippet, written via temp file + rename), `tmp/`, `config.yaml` (`default_shell`, `editor`, `confirm_before_run` default true, `username`/`server` reserved for Step 11). Snippet names are validated by forge-core before becoming file names, so a name cannot escape the directory. Listing skips (with a stderr warning) a file that fails to parse instead of breaking the whole library.
- `src/exit.rs` - `CliError` carrying spec-cli exit codes (1 failure, 2 refused, 3 not found; 4 arrives with auth in Step 11) and the what/why/fix message shape.
- `src/prompt.rs` - `[y/N]` and typed-`yes` prompts over an injectable `BufRead` (unit-tested: EOF/empty/anything-but-yes means no). Prompts go to stderr so stdout stays machine-clean.
- `src/record.rs` - spawns the configured shell with `-i` and `HISTFILE` pointed at a private temp file, reads the history back, cleans it (blanks, `exit`, consecutive dupes), secret-scans every line: findings print kind + command number only (never values); without a terminal the recording is refused (exit 2, nothing saved); on a terminal the user may save the **redacted** version instead. Name/description/tags via flags or prompt; overwrite requires `--force`.
- `src/runner.rs` - substitution everywhere (commands, cwd, env, condition fields); condition evaluation (`exists`/`missing` resolved against the step cwd, `which` on PATH, `env`, `equals`, `shell`); retries with delay; timeouts via poll-and-kill (exit 124); background steps detached; parallel steps on threads (step fails if any branch fails); `continue_on_error`; `wait`; `always` steps run after success and failure, and an `always` failure flips a successful run to failure. Script mode writes the substituted body to a `0700` file in a fresh `0700` dir under the store `tmp/`, runs `<shell> <file>` with inherited stdio, and removes both via a drop guard - verified by test (no `run-*` leftovers) and by a `read`-from-stdin script body that works because stdin is the terminal, not the script (the ADR 0004 property).
- `src/cmds.rs` - the commands. `run` resolves local names (exit 3 with up-to-3 similar-name suggestions), resolves variables via forge-core (a missing-variables error lists **all** of them with the copy-pasteable fix), prints the confirmation card (name, source, mode, shell, short hash, every substituted command, `always` steps, danger-lint warnings, all display-escaped per spec-cli §8) and refuses with exit 2 when confirmation is on but stdin is not a TTY. `--dry-run` prints the same plan and runs nothing. `delete` confirms the same way. `edit` opens `$EDITOR` (config → `$EDITOR` → `$VISUAL` → `vi`), re-validates, restores the original on a broken edit, and honours an in-editor rename by moving the file. `tag` folds/dedupes via forge-core.
- `src/main.rs` - clap CLI with global `--json`/`-y`, and the shorthand rewrite: a first word that is not a known command or flag becomes `run <name>` (subcommands always win, per spec-cli §5).

## 3. Decisions made
- **Recorder transport is `HISTFILE` + interactive shell**, the simplest mechanism that works across bash/sh (and zsh, which also honours `HISTFILE` for interactive sessions). A pty-based recorder would capture more exotic sessions but adds a large dependency and failure surface for Step 4; the history approach is also what makes the recorder trivially testable with a fake shell script, which the integration tests exploit.
- **Timeout returns 124** (the GNU `timeout` convention), documented in the runner; spec-cli does not mandate a value.
- **`run --json` prints the result object after execution**; child output still goes to stdout (inherent to running real commands). The plan variant is available via `--dry-run --json` with no child output. Noted here because it is the one place `--json` stdout is not exclusively JSON.
- **Exit code 4 (auth)** is not yet constructed by any code path; the `CliError::auth` constructor was removed rather than left as dead code and will be added by Step 11 with its first caller. The typed-`yes` prompt is implemented and unit-tested now because Step 12's trust flow is its first caller (marked with an `allow(dead_code)` and a comment saying so).

## 4. Verification
| Gate | Command | Result |
|------|---------|--------|
| Format | `cargo fmt --all --check` | pass |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | pass, zero warnings |
| Tests | `cargo test --workspace` | **222 passed, 0 failed** (forge-core 196, forge-cli 13 unit + 11 integration, forge-server 1) |
| Web | `npm run lint && npm run typecheck && npm test -- --run && npm run build` | pass (1 web test) |
| `--version` timing | release binary, 30 runs | **mean 2.95 ms, p95 3.15 ms** (budget ≤ 20 ms) |
| Run-to-prompt timing | release binary, `run demo --dry-run`, 30 runs | **mean 6.28 ms, p95 8.11 ms** (budget ≤ 50 ms) |
| Binary size | `ls -la target/release/forgehub` | **3.07 MB** (budget ≤ 15 MB) |
| Manual smoke | temp HOME: `init`, write `ubuntu-setup`, `list`, TTY run via `script -qec` answering `y` | card printed with hash `72830e2d`, both steps executed; `--help` lists all 10 commands |

Integration tests cover, against the real binary: init idempotence + JSON; record via fake shell → list/show/dry-run/run (real output asserted), shorthand `fg demo -y`; record secret refusal (exit 2, no file, secret value absent from stderr); missing name (exit 3 + suggestion); missing variable (exit 1, all names listed) then supplied via `KEY=value`; failing step exit-code propagation (7) with `always` output; condition hold/skip both directions; script mode state persistence + temp cleanup; tag add/remove with folding; search hit/miss; rename; edit append + broken-edit restore; delete refusal/`-y`/not-found; retry-until-success via a counter file; timeout kill (124).

## 5. Acceptance criteria
- [x] A user can init, record two commands, and run them with `fg <name>` - all offline. - Proven by `record_with_fake_shell_then_run` and the manual smoke (§4).
- [x] Startup timings measured and within budget. - `--version` 2.95 ms mean (≤ 20 ms); local run to plan 6.28 ms mean (≤ 50 ms); binary 3.07 MB (≤ 15 MB).

## 6. Security notes
Nothing runs without the confirmation card (or explicit `-y`, or the user's own `confirm_before_run: false` config); non-TTY + confirmation on = exit 2, never a silent run. The recorder can never persist a detected secret without an explicit redacted-save consent, and refusals never echo the secret value. Card content is display-escaped. Script temp files are `0700` and always cleaned up. Snippet names cannot traverse out of the snippets directory (forge-core validation before path construction).

## 7. Dependencies added
CLI: `clap` 4 (derive), `anyhow` 1, `serde` 1, `serde_yaml` 0.9, `serde_json` 1. (forge-core's set is unchanged from Step 3.)

## 8. Corrections to earlier steps
- None to Steps 1–3 files. One Step 4-internal correction during development: `referenced_variables` already covered conditions (Step 3 fix), and the runner relies on it; no change needed.

## 9. Known issues / limitations
- **Push blocker (carried over):** Steps 1–4 committed and tagged locally; pushing still needs a GitHub remote (no `gh` auth, no URL supplied).
- `record` depends on the shell writing `HISTFILE` for interactive sessions; a shell configured with history disabled records nothing and the CLI reports "nothing was recorded" rather than guessing. zsh users with `HISTFILE` unset in their rc may need `fg record` from bash - the shell is configurable via `default_shell` in `config.yaml`.
- Cloud name resolution (`owner/name`, own-cloud fallback), the trust store, and login are Steps 11–13 as scheduled; `run` in this step resolves local names only and says so in its help text.
- Background steps are detached with inherited stdio; their output can interleave with later steps (documented behaviour, same as Forgenesis).

## 10. Deferred / ideas
- A `--shell` flag on `record` for one-off shell choice (config covers the persistent case).
- Recording could offer to parameterise repeated literals inline; that is Step 5's `parameterize` command instead, by design.

## 11. Next step
Step 5 completes local parity: variable prompting at run time, `parameterize`, `clone`, `import`/`export`, `analyze`, `stats`, `backup`/`restore`, `doctor`, `alias install|uninstall|show`, shell completions, and full `--json` coverage - then CHECKPOINT 1.
