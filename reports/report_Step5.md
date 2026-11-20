# Report - Step 5: local CLI parity (vars, stats, backup, doctor, alias)

- **Date/time:** 2026-11-19T10:13:39+05:30
- **Commit:** this report is part of the Step 5 commit - see `git log --oneline -n 1` (tag `step-5`)
- **Status:** ⚠️ Done with caveats - all Step 5 acceptance criteria met and proven below; the caveat is the carried-over push blocker (§9). This step ends at **CHECKPOINT 1**.

## 1. Summary
The local CLI is now at parity with the Forgenesis feature set the analysis chose to keep, plus ForgeHub's additions. Added in this step: interactive variable prompting at run time, `parameterize`, `clone`, `import`/`export` (with the Forgenesis conversion path wired in), `analyze`, `stats`, `backup`/`restore` (self-contained ustar archives with SHA-256 manifest verification), `doctor`, `alias install|uninstall|show` (a real, working `fg` symlink), and `completions` for five shells. Every command supports `--json`. 235 workspace tests pass, and the full interactive flow (prompt → card → confirm → run) was exercised on a real terminal via `script`.

## 2. What I built
- **Variable prompting** (`cmds::resolve_or_prompt`): resolution now follows spec §7 exactly - arguments → defaults → built-ins → **prompt on a terminal** (each missing variable, with its declared description) → error listing all still-missing names with the copy-pasteable fix. The interactive flag is a parameter, so the prompt path is unit-tested with scripted input (answer resolves; empty answer stays missing; non-interactive lists all).
- **`parameterize`** (`parity.rs`): finds literals (URLs, multi-segment paths, quoted strings) appearing in ≥ 2 distinct commands; longest-literal wins when candidates overlap (a URL beats the path inside it - see §8). `--yes`/non-terminal applies all with suggested names (`URL`, `DIR`, `VALUE`, uniquified); on a terminal each candidate is confirmed with an editable name or `skip`. Applying declares the variable with the literal as default and rewrites every command/script body.
- **`clone`**: duplicates a snippet under a new validated name, records `source: cloned from local …`, refuses to overwrite.
- **`import`**: strict ForgeHub parse first; on failure, the forge-core Forgenesis conversion (notes printed for every dropped field and the name lowercasing); `--name` override and `--force` the only ways to change identity or overwrite. **`export`**: YAML to stdout or `--out` file.
- **`analyze`**: per-snippet report - content hash, command count, programs used, declared vs referenced variables, danger-lint warnings, secret findings (kinds + command numbers only). Exit 1 when secrets are present so it composes in scripts; warnings alone exit 0.
- **`stats`**: snippet/command totals, script vs steps counts, most complex snippet, top tags; full `--json`.
- **`backup`/`restore`** (`backup.rs`): a minimal ustar writer/reader in the CLI itself (no external `tar`, no archive dependency). Archives contain `config.yaml`, every snippet, and `manifest.json` with a SHA-256 per file. Restore **verifies every hash before writing anything**, refuses entries missing from the manifest, absolute paths and `..` traversal, and requires confirmation (or `-y`) since it overwrites; config is restored only with `--config`. Archive bytes are reproducible (mtime 0).
- **`doctor`**: home, config parse, shell availability, editor presence, every snippet file parses (broken ones named), temp-dir writability, git presence - `OK/WARN/FAIL` lines, exit 1 on any FAIL, `--json` form.
- **`alias`**: `install` creates the `fg` symlink next to the real binary (refusing to overwrite anything that doesn't already point at this binary), `show` reports state, `uninstall` removes only our own symlink. Proven end-to-end: the installed `fg` runs `--version`.
- **`completions`**: `clap_complete` generation for bash, zsh, fish, powershell, elvish; tested for bash/zsh/fish output containing the command tree.

## 3. Decisions made
- **Backup format is hand-rolled ustar.** The alternatives were shelling out to `tar` (absent on minimal Windows installs; Step 21 ships there) or adding a tar crate (fine, but the format is 512-byte headers and the manifest verification matters more than the container). The reader is deliberately stricter than GNU tar: traversal and unmanifested entries are hard errors.
- **Parameterize overlap rule**: candidates are sorted by occurrence count, then length, and any candidate that is a substring of a kept one is dropped. Without this, parameterizing a repeated URL also parameterizes the path inside it and corrupts both (this bug was caught by the Step 5 integration test before the rule existed - see §8).
- **`analyze` exit code**: 1 only for secret findings. Making lint warnings fail the command would punish exactly the honest-warning behaviour the lints exist for.
- **Restore confirmation** follows the same fail-safe rule as delete: no terminal + no `-y` = exit 2, nothing written.

## 4. Verification
| Gate | Command | Result |
|------|---------|--------|
| Format | `cargo fmt --all --check` | pass |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | pass, zero warnings |
| Tests | `cargo test --workspace` | **235 passed, 0 failed** (forge-core 196, forge-cli 19 unit + 11 Step 4 integration + 8 Step 5 integration, forge-server 1) |
| Web | `npm run lint && npm run typecheck && npm test -- --run && npm run build` | pass (1 web test) |
| Interactive smoke | release binary under `script -qec` (real TTY): `run greet` with required variable `WHO` | prompted `Value for WHO (who to greet):`, card showed `echo hello world`, `[y/N]` confirmed, step ran and printed `hello world` |
| Alias smoke | `alias install` → run `./target/release/fg --version` → `alias uninstall` | symlink created, `forgehub 0.1.0` printed through it, removed cleanly |
| Backup tests | roundtrip + corruption | restore after delete brings the snippet back; one flipped byte → hash mismatch, exit 1, nothing written |

The 6-line manual test script for this checkpoint is in the checkpoint message accompanying this report (init, record, run --dry, run, stats, alias install in a temp HOME).

## 5. Acceptance criteria
- [x] The local feature set is complete per `docs/forgenesis-analysis.md`, and `fg` works end-to-end locally. - Every "port as-is / redesign" row of the analysis is implemented (record, run, variables, conditions, stats, backup/restore, doctor, alias, completions, import/export, analyze); dropped features (repo sync, packs, the `config` subcommand) remain dropped with their documented reasons.
- [x] `forgehub --help` lists all local commands. - Verified in the Step 4 smoke and again here: 21 subcommands including all Step 5 additions.

## 6. Security notes
Restore is the security-sensitive addition: hash-verified-before-write, traversal-proof, manifest-bound, confirmation-gated. `analyze` never prints secret values. `parameterize` and `import` save through the same validated store path as everything else, so a parity command cannot write an invalid or unparseable snippet into the library (one test caught a case where the *input file itself* was invalid YAML - a command containing `: ` unquoted - and the error correctly pointed at line 3).

## 7. Dependencies added
CLI: `regex` 1 (parameterize candidate detection; already a forge-core dependency), `clap_complete` 4 (completions generation).

## 8. Corrections to earlier steps
- **Step 3 (forge-core)**: `Snippet::command_count()` counted steps-mode structure for script snippets too, returning 0 for a script body. Found by the Step 5 `stats` integration test (a 1-line script counted 0 commands). Fixed in forge-core to count body lines in script mode; all 196 core tests still pass.
- **Step 5-internal, caught by tests before commit**: parameterize's overlapping-candidate corruption (§3) - fixed with the substring-drop rule and covered by `parameterize_extracts_repeated_url`.

## 9. Known issues / limitations
- **Push blocker (carried over):** Steps 1–5 are committed and tagged locally (`step-1`…`step-5`); pushing still needs a GitHub remote - `gh` is not authenticated on this machine and no remote URL has been supplied. One user action (authenticate `gh`, or paste the private repo URL) unblocks all five pushes at once.
- `alias install` is Unix-only in this step (symlinks); on Windows it prints the copy-to-`fg.exe` instruction instead of failing silently. Windows packaging is Step 21.
- `parameterize` detects URLs, multi-segment paths and quoted strings; bare repeated words (e.g. a hostname without a scheme) are not candidates - deliberate, since word-frequency parameterizing produces noise variables.
- `backup` archives are uncompressed `.tar` (a snippet library is small text; compression is a Step 21 packaging concern if archives ever grow).

## 10. Deferred / ideas
- `fg alias install --rc` to append a shell alias line when the binary directory is not writable (doctor already detects the situation class).
- Backup encryption was considered and rejected for v1: archives are local files under the user's control, and the threat model does not claim protection from a local attacker with the user's files.

## 11. Next step
After CONTINUE: Step 6 stands up the server skeleton (axum + sqlx, config, logging, `/health`, Dockerfile, `docs/running.md`) - the start of the cloud half.

## Corrections (post-checkpoint review, 2026-11-19)

After CHECKPOINT 1 the user reviewed Step 5 against `roadmap.md` §5/§6 and the Step 5 prompt, and ordered the gaps fixed **inside this step's existing commit** (amended; the `step-5` tag moved; nothing had been pushed, so history stays one-commit-per-step). This section supersedes the parts of §§1–2 above that describe the first implementation. A full audit found the first implementation had drifted from the roadmap in six places; all are fixed and re-proven:

1. **`alias` rewritten to the roadmap §6 mechanism.** Was: a `fg` symlink next to the binary - insufficient, because the shell builtin `fg` shadows `$PATH` in interactive shells. Now: `alias install|uninstall|show [--shell bash|zsh|fish] [-y]` writes a clearly marked, idempotent block (`# >>> forgehub alias >>>` … `# <<< forgehub alias <<<`) containing `alias fg='forgehub'` and `alias jobs-fg='builtin fg'` into `~/.bashrc`/`~/.zshrc` (fish: `fg`/`jobs-fg` function files in the `funcsave` layout). The rc file is backed up (`<rc>.forgehub-backup-<timestamp>`) before every edit; editing asks on a terminal and refuses without one unless `-y` (exit 2). **Proof (real output, temp HOME, this machine):** after install, interactive bash reports `fg is aliased to 'forgehub'` and `fg --version` prints `forgehub 0.1.0`; interactive zsh reports `fg is an alias for forgehub` and also prints `forgehub 0.1.0`. After uninstall, both shells report `fg is a shell builtin` again (zsh: `no job control in this shell` when invoked). Tests cover idempotence (second install = no change, no second backup), backup contents, block-only removal with user rc content preserved, and the fish files.
2. **`analyze` replaced with the roadmap command.** Was: a per-snippet lint/secret report (not in the roadmap). Now: history mining ported from Forgenesis `analyze.rs` - bash/zsh/fish history parsing, session grouping by 30-minute gaps, non-overlapping sequence counting, fragment/rotation suppression, ranking, `--min-count/--limit/--max-len/--dry/-y/--file`. Secret scrubbing happens in the filter stage (Forgenesis marker screen **and** the forge-core scanner) before anything is printed or written; an integration test repeats a secret-shaped command 8 times and asserts it appears in neither the output nor any created snippet. The 16 Forgenesis unit tests were ported with the module.
3. **`stats` replaced with the roadmap command.** Was: library counts. Now: a SQLite `history.db` (rusqlite, bundled; schema ported from Forgenesis `db.rs`). `run` inserts a row at start and finishes it at completion, so an interrupted run stays unfinished - counted separately and excluded from all totals. Saved time = (commands × `human_overhead_seconds`, config, default 12) − actual runtime, never negative. Flags `--limit/--explain`; `--json` emits exactly the spec-cli §3 keys. The old library counts moved nowhere - `list` already covers the library.
4. **`import` completed.** Was: single file only. Now: a file, a directory (every `.yaml`/`.yml`, collisions reported and skipped, never silently overwritten), or `--from-forgenesis` (`$FORGENESIS_HOME/workflows` or `~/.forgenesis/workflows`), with tests for all three and for collision reporting.
5. **`parameterize` completed.** Added `--literal TEXT --as NAME` pairs (spec-cli §4), a printed preview of ranked candidates in every mode, and the fail-safe rule: no terminal + no `-y` = preview printed, nothing applied, exit 2.
6. **`backup`/`restore` defaults fixed.** Backups now land in `~/.forgehub/backups/forgehub-backup-<timestamp>.tar` by default (was: current directory), `backup [name]` backs up one snippet, and `restore` with no argument restores the newest archive; `--force` joins `-y` as a confirmation bypass. Archives still hold config + snippets only - no credentials exist in the archive path by construction.
7. **`doctor` completed.** Added the `[topic]` argument (check-name topics plus ecosystem topics rust/node/python/docker/go), the alias-presence, home-permissions, history.db and credential-permissions checks, and a registry check that reports "skipped" when no registry is configured. JSON now uses the spec-cli §3 keys (`checks[].name/level/detail`, `ok`) - the first implementation's `status`/`check` keys were wrong.
8. **`list`/`show` JSON aligned to spec-cli §3** (the keys were already documented there; the output now matches): `list` → `{"workflows": [{name, description, tags, mode, origin}]}`, plus `--tag` and `--long`; `show` → `{name, description, tags, mode, shell, variables, commands}`; human `show` is now the §4 card with numbered commands instead of raw YAML (`export` still prints YAML).
9. **`--dry` accepted everywhere `--dry-run` is** (clap alias on `run`; `analyze --dry` also accepts `--dry-run`).
10. **Completions are dynamic.** `completions <shell>` now prints the registration snippet for `clap_complete`'s env protocol (`unstable-dynamic` feature, v4.6.12), and name-taking commands complete local snippet names from the live library. Proof (real output, simulating the generated bash script's exact call): `forgehub run al` → `alpha`, `alpine`; `forgehub show b` → `beta`, served from a temp library.
11. **Config grew the Forgenesis keys** the corrected commands need: `human_overhead_seconds` (12), `analyze_min_count` (3), `analyze_max_len` (8), `ignore_commands` (Forgenesis noise list), `history_file`.
12. **Spec deviations documented in `docs/spec-cli.md`** per Step 5 prompt item 10: local state is `config.yaml` + `snippets/` (the Step 2 draft said `config.toml` + `workflows/`; Steps 4–5 shipped YAML - same key names, and renaming the directory/format now would break every existing library for a cosmetic gain), and the backup/completions rows now describe the shipped behaviour.

Gate results after the corrections: `cargo fmt --all --check` pass; `cargo clippy --workspace --all-targets -- -D warnings` pass, zero warnings; `cargo test --workspace` **262 passed, 0 failed** (forge-core 196, forge-cli 41 unit + 11 local + 13 parity integration, forge-server 1); web lint/typecheck/test/build pass. Remote push and CI status are recorded in §9's place below once attempted.

**Remote push attempt (2026-11-19, after the corrections above):** `git remote add origin https://github.com/aotlover9-base-eth/forgehub.git` succeeded. `git push origin main --follow-tags` failed with the real output: `fatal: could not read Username for 'https://github.com': No such device or address` (exit 128). Two facts verified at the same time: `gh auth status` → "You are not logged into any GitHub hosts", and `GET https://api.github.com/repos/aotlover9-base-eth/forgehub` → **404** (the repository does not exist yet, or is private and invisible without authentication). Consequently the remote has no tags to confirm yet, and **CI has not run** - there is nothing on GitHub to run it. Exactly what is needed from the user: (1) create the empty private repo `aotlover9-base-eth/forgehub` on GitHub (no README/.gitignore, so the push is not rejected), and (2) authenticate this machine - `gh auth login`, or a PAT (classic `repo` scope, or fine-grained Contents read/write on that repo) provided via the Secure Vault, or add this machine's SSH public key to the account and switch the remote to SSH. The moment both exist, one `git push origin main --follow-tags` publishes all five commits and tags `step-1`…`step-5`, and the CI workflow in `.github/workflows/` will run on the push.
