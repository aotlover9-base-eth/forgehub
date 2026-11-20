# Step 13 Report - Run from cloud with safety UX

## 1. Summary
The headline feature works: `fg ubuntu-setup` (and
`fg run owner/name[@version]`) fetches a cloud snippet in one
request, verifies its hash locally, shows the full confirmation
card, applies the trust model (typed `yes` for other people's
snippets, diff + re-confirmation on any hash change), and only
then executes - identically to a local run. Non-interactive
invocations fail safe. This was CHECKPOINT 2 in the original
plan; per the user's override for this run it did not stop -
see §10, "Checkpoint review".

## 2. What was built
- **`crates/forge-cli/src/cloudrun.rs`** (new): reference
  parsing (`name`, `owner/name`, `@N` pins); one-request fetch
  (`?intent=run`) → forge-core parse + validate → **local hash
  recomputation compared against the server hash (mismatch =
  hard error, nothing runs)**; the confirmation card (owner,
  version, short hash, visibility, mode, lint warnings, EVERY
  command numbered - script bodies now split per line - long
  lines wrapped at 100 cols, never truncated, all text through
  `escape_display`); the pure trust decision
  (`confirm_kind`/`flag_yes_may_skip`, unit-tested matrix);
  changed-hash diffs from the cached baseline; execution via
  the same `runner::execute` as local runs, with history
  recording and an `exit code <n> · duration` summary;
  `--dry` (card or JSON), `--step`, `--no-cache`, `--offline`.
- **`crates/forge-cli/src/trust.rs`** (new): `trust.db`
  (SQLite) recording (owner, name, hash, version, runs,
  timestamps); the run cache under `<home>/cache/` (immutable
  version bodies + a latest pointer; cache loads re-verify the
  body hash and fail closed). `fg trust list|revoke`,
  `fg trust <owner/name@hash|@version>` (fetch, card, confirm
  under the same rules, record without running; a requested
  hash that doesn't match the fetched content is refused).
- **Resolution in `cmds::cmd_run`**: explicit `owner/name` or
  `@N` → cloud; bare name → local first (shadowing rule), then
  the caller's own cloud snippet, then a not-found error whose
  suggestions are **edit-distance ≤ 2** local names (spec-cli
  §5) plus remote search hits - never auto-run.
- **`runner::execute_stepwise`** (`--step`): per-step
  `[y/N/q]` prompting for steps mode (conditions honoured,
  `always` steps unprompted); `prompt::ask_step` treats any
  unrecognised input as skip (fail safe).
- **Fix in `cmds::show_commands`**: script-mode bodies are
  split into lines for display (previously the whole body was
  one "command" and embedded newlines rendered as `�` in
  cards) - improves `show`, pull previews and remote show too.

## 3. Trust model as implemented
| Case | Prompt | `-y` skips? |
|---|---|---|
| Own snippet | `[y/N]` | yes |
| Other's, first time | typed `yes` | **no** |
| Other's, exact hash trusted | `[y/N]` | yes |
| Any, hash changed since approval | `[y/N]` (own) / typed `yes` (other's) + diff shown | **no** |
| Non-TTY, any unskippable case | card printed, exit 2 | - |

## 4. Tests
- 6 integration tests (`tests/cloudrun.rs`) against the real
  server: own cloud run with `-y` + fail-safe without (marker
  file proves execution/non-execution); third-party typed-yes
  requirement (trust seeded directly in `trust.db` because a
  subprocess has no TTY - the interactive path itself is
  unit-tested via the decision matrix and `ask_typed_yes`'s
  own tests from Step 5); changed hash → exit 2 with "CHANGED"
  and the new line visible in the diff; version pin (`@1`
  card shows v1) + offline replay + never-fetched offline
  error; malicious display fixture (raw ESC/CR/bidi in an
  API-created public snippet - the card contains no raw ESC,
  CR or U+202E and shows `\e` visibly); local shadowing +
  typo suggestion ("helo" → "hello", exit 3).
- Unit tests: trust DB lifecycle, reference parsing, the
  trust-decision matrix, edit distance, line wrapping.
- **Workspace total: 373 passed, 0 failed** (was 361).

## 5. Manual smoke (real output, this machine)
Server on 127.0.0.1:18098; user `demo` signed up via curl; a
public `ubuntu-setup` created through the API; CLI logged in
with `--token`. Then:
```text
$ fg ubuntu-setup --dry
About to run `ubuntu-setup` (cloud: demo/ubuntu-setup)
  Update packages and install basics
  version 1 · sha256:88b0a5c5 · public · mode: script
  Warnings:
    line 1: runs with sudo (administrator privileges)
  Commands:
   1. sudo apt-get update && sudo apt-get install -y curl git
Dry run: nothing was executed.            (exit 0)

$ fg ubuntu-setup < /dev/null             (no TTY, no -y)
…card printed to stderr…
forgehub: not confirmed
  what: `demo/ubuntu-setup` was not run
  why: there is no terminal to confirm on
  fix: run it once interactively to approve this exact content,
       or pass --yes (-y) for your own unchanged snippets
                                          (exit 2 - fail safe)
```

## 6. Gates (agent.md §9) - real output
```text
$ cargo fmt --all --check            → clean
$ cargo clippy --workspace --all-targets -- -D warnings → 0 errors
$ cargo test --workspace             → TOTAL: 373 passed, 0 failed
$ web: lint ✓ · typecheck ✓ · test 1 passed ✓ · build ✓
$ scripts/scan-staged.sh             → clean (at commit time)
```

## 7. Acceptance check (roadmap Step 13)
- [x] `fg ubuntu-setup` shows the card, asks, runs only on yes.
- [x] Third-party snippets need typed `yes`; changed hash
  shows a diff and re-confirms; `-y` bypasses neither.
- [x] Non-TTY fails safe (card + exit 2).
- [x] Hash verified locally; mismatch is a hard error.
- [x] Pins, offline replay, suggestions, shadowing - tested.

## 8. Known gaps / risks
- The interactive typed-`yes` path cannot be exercised by a
  subprocess test (no TTY); it is covered by the pure decision
  matrix + prompt unit tests, and the fail-safe direction is
  covered end-to-end. A PTY-driven test is a Step 20 candidate.
- Hash-mismatch is covered at the `verify_body` level by
  construction (single code path for fetch/cache/API test
  paths) but has no dedicated e2e with a lying server.
- `--offline` with a bare name needs the username from a
  stored credential or config; documented in the error text.

## 9. Corrections to earlier steps
- `show_commands` script-line splitting (see §2) - a display
  bug from Step 4, fixed here where it mattered most.
- Step 12's CI: green (verified after the fact - Web + Rust
  jobs passed on commit `270e15a`).

## 10. Checkpoint review (CHECKPOINT 2 - per user override, not a stop)

**What you should manually test** (5 minutes):
```bash
# 1. Start the server (dev build is fine)
FORGEHUB_DATABASE_URL="sqlite:///tmp/fh.db" \
FORGEHUB_BIND=127.0.0.1:8080 FORGEHUB_BASE_URL=http://127.0.0.1:8080 \
  cargo run -p forge-server
# 2. In another shell: sign up + make a token (web UI lands in Step 15;
#    use curl for now - see the Step 11 report for the exact calls),
#    create a public snippet named ubuntu-setup via POST /api/v1/snippets.
# 3. fg login --registry http://127.0.0.1:8080
# 4. fg ubuntu-setup --dry          → card + sudo warning, nothing runs
# 5. fg ubuntu-setup                → [y/N] prompt (it is yours)
# 6. From a second account: fg run <you>/ubuntu-setup
#    → "not yours" card; only the typed word yes runs it
# 7. Edit the snippet on the server, run again → CHANGED + diff, re-confirm
# 8. fg trust list · fg trust revoke <you>/ubuntu-setup
```

**What is solid:** the whole trust matrix is implemented as a
pure function with a unit-tested truth table; hash parity is
verified on every fetch and every cache load; fail-safe
behaviour is proven by subprocess tests with marker files;
the malicious-display fixture passes with real control
characters.

**What is risky:** the interactive prompts (typed `yes`,
`[y/N]`, `--step`) are only unit-tested - no PTY end-to-end
test exists yet; the changed-hash diff needs the old body in
the cache (if the cache was cleared, you get the CHANGED
notice without a diff - safe, but less informative).

**What I would fix first:** a PTY-driven integration test for
the interactive confirmations (Step 20 E2E pass), then
diff-baseline recovery by fetching the trusted version from
the server when the cache lacks it.

## 11. Next step
Step 14 - web design system (Material You tokens/components).
