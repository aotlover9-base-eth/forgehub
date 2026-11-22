# Peak Phase B - CLI audit and baseline (B0)

- **Date:** 2026-11-22
- **Method:** every subcommand's `--help` captured from the built binary; `crates/forge-cli/src/main.rs` (clap definitions), `exit.rs`, `cmds.rs`, `cloud.rs`, `cloudrun.rs`, `sync.rs`, `trust.rs`, `tui.rs` read at HEAD; `docs/cli-reference.md` and `docs/spec-cli.md` spot-checked against the binary.
- **Environment caveat:** the audit binary is the local debug build (2026-11-21, pre-Peak source). Its help surface matches the clap definitions at HEAD (Peak touched no CLI code). Live write flows (`init`, `import`, a real `run`) could not be exercised: the sandbox write fault (see PROGRESS.md) denies the CLI's own file writes with EPERM - itself an audit data point (F12). Server-backed flows were audited from code and the CI e2e suite, not re-run locally; anything not run says so.

## 1. Baseline against the agent.md section 9 budgets

| Budget | Limit | Measured at B0 | Verdict |
|--------|-------|----------------|---------|
| `fg --version` cold start | <= 20 ms | 16 ms wall, **debug** build, warm page cache | Within budget even unoptimised; release re-measure owed at B9 (no release build possible during the write fault) |
| `fg <local-name>` to first prompt | <= 50 ms | Not run - the fault blocks the store/history writes a run performs | Owed; CI e2e covers run correctness, not this timing |
| Release binary, stripped | <= 15 MB | Not re-measured (no release build during the fault) | Owed at B9 |
| Server p95 read/list at 10k | < 50 ms | Unchanged since Step 20: ~2-4 ms reads/lists, ~37 ms search (docs/known-gaps.md) | Within budget; Phase C re-checks |

Startup composition (code): clap parse, `Store::open` (reads config + scans `snippets/`), then dispatch. No network on local paths. The 16 ms debug figure says the budget has real headroom; B1-B9 must not add startup work (no config migration checks, no registry probes) to local commands.

## 2. What is already good (do not regress)

- **Help examples everywhere:** every subcommand carries an `after_help` Examples block (verified in `main.rs` and the captured helps).
- **Error shape exists:** `exit::CliError` carries what/why/fix plus a stable exit code; codes 0/1/2/3/4/130 are documented in spec-cli section 2 and match `exit.rs`.
- **`--json` is global** and spec-cli documents per-command schemas; several commands have parse-tests.
- **Dynamic completion exists:** local snippet names complete for name-taking arguments via `ArgValueCompleter` (Step-era), and `fg completions <shell>` emits scripts for bash/zsh/fish/powershell/elvish.
- **`list` already has `--tag` and `--long`**; `pull` is complete (`--as`, `--version`, `--force`); `push --all` exists; `remote list --stars` exists; `record --push` exists; the `record` finish word is `end` (post-release fix).
- **Colour discipline:** non-TUI output is plain text (no ANSI), so piped output is clean by construction; the TUI honours `NO_COLOR` and `TERM=dumb`.

## 3. Friction list (each item names its fixing WP)

- **F1 - Help is dominated by globals (B2).** The four global flags (`--json`, `-y`, `-v`, `--offline`) print in full in every subcommand's help; for `edit`, `delete`, `show` they are most of the help. The top-level help is a flat list of 33 commands with no grouping. There are no help topics (`fg help trust` gives clap's subcommand help, not the trust model explainer).
- **F2 - `search` has no filter flags at all (B4).** Beyond globals, `search` accepts only the query: no `--tag`, `--visibility`, `--sort`, `--since`, `--long`, `--limit`. `list` has `--tag`/`--long` but no `--sort`, `--visibility`, `--since`, `--limit`. The two commands' output shapes differ for the same data.
- **F3 - Spec/help mismatch on `run --version` (B1).** spec-cli's command table lists `run ... [--version N]`, but clap defines no such flag; version selection exists only as the `name@version` target syntax. Align the spec (the syntax covers the need) and say so in B1's sweep.
- **F4 - `trust list` is thin; no `trust show` (B7).** `trust` has `list`/`revoke`/`add`; the list does not surface version, approved-at or last-run columns, and there is no way to inspect one entry's detail without running something.
- **F5 - `cache` is list/clear only (no plan WP owns this).** No `cache path` (users cannot find the cache directory without reading docs) and no `cache prune --max-size` (the LRU cap exists via config `cache_max_mb`, but there is no one-shot prune or a human explanation of an eviction). Small and safe; lands opportunistically with B7 (same local-store theme) or is recorded as a deferral in the final report.
- **F6 - Sync discoverability (B6).** `status` prints states but no next-command hints; `remote` has no `run` (running a cloud snippet is `fg run owner/name`, undiscoverable from `remote` help); `push`/`pull` results do not print the follow-up command (`fg status`, `fg run <name>`).
- **F7 - Flag declaration order leaks into help (B2, cosmetic).** `push --help` lists flags in derive order (`--all`, `--json`, `--public`, `-y`, `--unlisted`, `-v`, `--offline`, `--private`, `-m`) - the visibility trio is split apart. Group or order deliberately.
- **F8 - Authoring gaps (B5).** There is no `fg new` (authoring starts from `record` or a hand-written YAML file); `fg edit` is local-only (no download-edit-push flow for cloud snippets); there is no `fg lint` (the core lints and secret scan run inside other commands but cannot be invoked standalone with their own exit codes). `edit`'s save-time validation rollback is the only guard.
- **F9 - Completion gaps (B2).** Name completion exists for local names; `owner/name` targets do not complete from the local cache, flag values do not complete (`--tag` offers nothing), and the generated scripts are not regression-tested per shell.
- **F10 - `doctor` is env-only (no plan WP owns this).** It checks the local environment and library health but not registry reachability, client/server version skew, config lint (unknown keys are silently ignored today - B9 validates values on `config set`, but a doctor lint is not in the plan), or cache/trust store health. Recorded here for the final report; not silently dropped.
- **F11 - Config is invisible (B9).** No `fg config` surface at all: users hand-edit `config.yaml`; unknown keys pass silently; there is no `config list/get/set/path` and no first-run discoverability of `FORGEHUB_HOME`/`FORGEHUB_REGISTRY` precedence.
- **F12 - Raw OS errors escape the error standard (B1).** Observed live during this audit: `fg init` in a write-denied directory prints `writing /tmp/b0home/config.yaml: Operation not permitted (os error 1)` - accurate, but no why/fix and no stable shape. B1's sweep converts store/IO error surfaces to the `CliError` what/why/fix standard without touching exit codes.
- **F13 - No `--color` flag and no shared output module (B1).** agent.md section 8.4 and the plan's B1 require the full contract: `NO_COLOR`, `--color=auto|always|never`, non-TTY plain output, ASCII fallback, one accent for headings/selection, red/amber/green reserved. Today non-TUI output never colours (so pipes are clean but terminals get no accent either), there is no `--color` flag, and each command formats ad hoc. B1 builds the one output module with the full contract and tests each mode.
- **F14 - Man pages and cli-reference drift risk (B2).** `docs/cli-reference.md` (986 lines) spot-checks current for the audited surface (`--offline`, trust, cache, remote all present), but every B2/B4-B9 behaviour change must update it and `docs/man/` in the same commit - the plan already requires this for B2; this audit extends the rule to all Phase B WPs.

- **F15 - The `--json` error envelope was not implemented (found in B1, fixed in B1).** spec-cli section 3 promises errors with `--json` as `{ "error": { "code", "message" } }` on stderr; the top-level error path printed the plain chain regardless of `--json`. B1 implements it in `exit::render_error` with tests.

## 4. The fix map

| WP | Fixes |
|----|-------|
| B1 | F3, F12, F13 (output module, colour contract, error standard sweep, exit-code/JSON test matrix) |
| B2 | F1, F7, F9, F14 (grouped help, `fg help <topic>`, cache-based `owner/name` completion, docs in-commit) |
| B3 | Run experience per plan (fetch progress line, elapsed + final summary, failure hint, golden test pinning the safety card) |
| B4 | F2 (one filter set for `list`/`search`, documented `--json` schemas, new `fg find`) |
| B5 | F8 (new `fg new`, cloud `fg edit`, new `fg lint`) |
| B6 | F6 (status table with next commands, `pull --all`, diff gutter parity) |
| B7 | F4 (trust list columns + `trust show`; typed-yes card gains owner/age/version-count per plan); F5 opportunistically |
| B8 | Failure-mode behaviour per plan (down/slow/429/5xx/malformed, idempotent-only retries, offline cache timestamp note). F10 (doctor) is unassigned by the plan - see the final report |
| B9 | F11 (`fg config get|set|list` with validation; + the owed release measurements from section 1) |

## 5. Docs updated by this WP

- This file (new): `docs/cli/PEAK-AUDIT.md`.
- `docs/cli-reference.md`: verified spot-current; no changes needed at B0 (changes ride with the WPs that change behaviour, per F14).
