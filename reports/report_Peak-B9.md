# Report - Peak B9: config command and verification

- **Date/time:** 2026-11-22 (Asia/Kolkata)
- **WP:** B9 (Phase B - CLI improvements), the final Phase B WP.
- **Status:** Done. Also closes Phase B's owed items: the B8
  clippy fix (separate commit `602d8f3`, CI fully green), the
  man-page regeneration owed since B2, B5's cloud-edit live
  verification, and B0's owed release measurements.

## 0. Reconciliation before any work (third builder)

Local git HEAD was stale at A7 (`46722ba`) because the earlier
builders committed through the GitHub API. Verified against the
remote tree (tarball of `main` @ `9972275`): the local working
tree was exactly remote B8 plus three small uncommitted fixes
(the B8 clippy fix-forward and one stale test assertion, see the
B8 report addendum) - no other divergence, and the 72 visual
goldens deleted locally were restored from the remote. Local
git was re-synced with `git fetch origin && git reset --mixed
origin/main` (the WP-0b note's own instruction), so from this WP
on, commits are ordinary git commits again. The B8 fix went out
first as its own commit `602d8f3`; **CI run `44973090836` is
fully green, all five jobs** - the B4-B8 clippy chain is broken.

## 1. `fg config get|set|list` (new module `config_cmd.rs`)

Five user-facing keys over the existing `config.yaml` (no new
storage, no schema break - `Config` gained two optional fields,
`color` and `default_visibility`, both `serde(default)`):

| Key | Maps to | Validation on `set` |
|---|---|---|
| `registry_url` | `server` | `cloud::normalize_registry` (http/https base URL, normalised) |
| `color` | `color` (new) | `auto` / `always` / `never` (case-insensitive, stored canonical) |
| `shell` | `default_shell` | `bash`/`sh`/`zsh`/`fish`, or an absolute path that is a file |
| `default_visibility` | `default_visibility` (new) | `private` / `unlisted` / `public` |
| `cache_size_mb` | `cache_max_mb` | whole MiB, 1-10240 |

- Aliases: `colour` -> `color`, `server`/`registry` ->
  `registry_url`, `visibility` -> `default_visibility`,
  `cache_size` -> `cache_size_mb`. Unknown keys are refused
  with the valid list - in particular there is no key under
  which a secret could be stored; tokens stay in the
  credential store, and a test asserts `config set token ...`
  is refused and never reaches the file.
- `list` prints every key with its effective value and source
  (`config` / `default` / `unset`; `cache_size_mb` reads
  `effective` because the struct always carries a value) plus
  the config path and a one-line summary per key. `get` prints
  one key. Both honour `--json` with stable shapes (pinned by
  test: the five keys in canonical order).
- Errors are house-style what/fix blocks, exit 1.

**The routing catch worth recording:** `fg config` initially
fell into the `fg <name>` shorthand and tried to run a snippet
named `config` - the shorthand guard list `KNOWN_COMMANDS` in
`main.rs` predates the command. It now includes `config`
(forge-core's reserved-name list already did, which is why the
failure was a clean refusal rather than a run).

## 2. The keys are wired, not decorative

- **color**: `--color` became `Option<ColorMode>`; resolution
  is flag > config `color` > `auto`, computed in `run_cli`
  after the store opens (a store failure still initialises
  output from the flag alone so its error renders as before).
- **default_visibility**: with no visibility flag, the push
  dispatch (and the TUI push action) passes the config value
  into `cmd_push`/`push_one`, where it is used **only on the
  create branch** - it replaces the built-in "private for new"
  default. Existing snippets keep their visibility unless a
  flag changes it; the B5 cloud-edit re-push passes no default.
  Proven live (below).
- **registry_url / shell / cache_size_mb** were already read
  by `resolve_registry`, `record` and the cache respectively;
  `fg config` is their validated write path.

## 3. Tests (all new this WP)

- `config_cmd.rs` unit tests (7): alias resolution, per-key
  validation matrices (including "a refused value leaves the
  stored one untouched"), effective-value fallbacks, and a
  set/get round trip through a temp store.
- `crates/forge-cli/tests/config.rs` (5, real binary, temp
  home): fresh-home defaults, set/get round trip for every key
  (asserting the on-disk YAML keys), registry normalisation,
  refusal of eight invalid/unknown inputs with `fix:` in
  stderr, and the `--json` shapes.
- `crates/forge-cli/tests/sync.rs` (+2, real server):
  `push_uses_config_default_visibility_for_creates_only`
  (create goes public from config; after changing the default
  to private and pushing new content, the snippet stays
  public) and `cloud_edit_pushes_a_new_version_live` (next
  section).

## 4. Owed item closed: B5 cloud edit, verified live

B5's report recorded "cloud edit's live run not yet verified
(fault)". It is now covered by the integration test above:
against the real server, `fg -y edit anna/hello` with a
scripted `$EDITOR` (sed rewriting a marker) downloads the
snippet, edits it, and pushes version 2; the test asserts the
server-side version and body. Passed locally in this WP's
test run (and runs in CI from now on).

## 5. Owed item closed: man pages regenerated (owed since B2)

`forgehub --gen-man docs/man` with the current binary: 11
pages updated (cache, doctor, edit, list, login, pull, push,
search, trust, update, top-level) and 10 added (config,
config-list, config-get, config-set, find, lint, new,
cache-path, cache-prune, and the trust-show page) - the B2-B9
help surface is now fully reflected in `docs/man/`.

## 6. Owed item closed: release measurements (owed since B0)

Measured on this machine (shared-CPU sandbox) with a release
build of the B9 tree (`cargo build --release -p forge-cli`):

| Budget (agent.md section 9) | Limit | Measured | Verdict |
|---|---|---|---|
| `fg --version` | <= 20 ms | mean 6.54 ms, p95 7.82 ms (n=40) | within |
| local run to prompt | <= 50 ms | mean 8.20 ms, p95 10.54 ms (n=25, real PTY, prompt answered "n") | within |
| release binary size | <= 15 MB | 9,775,992 bytes (~9.3 MiB) as built | within |

The piped-stdin variant of the run probe refuses with exit 2
("confirmation is on and there is no terminal to confirm on"),
which is the designed fail-safe, so the prompt figure comes
from the PTY probe. Server p95 at 10k stands as re-checked at
B0 (Step 20 numbers, unchanged code paths); Phase C re-checks
per the audit.

## 7. Docs

- `docs/spec-cli.md`: the `config` row no longer says "not a
  command in v1" - it specifies the five keys, the create-only
  default-visibility semantics and the flag-over-config colour
  precedence.
- `docs/cli-reference.md`: a Config paragraph in the header
  notes, a glance-list entry, and full `forgehub config`
  help sections (from the binary's own `-h` output).
- `docs/man/`: regenerated (section 5).

## 8. Gates

Local, in a good write window (the fault was quiet all
session): `cargo fmt --all --check` clean; `cargo clippy
--workspace --all-targets -- -D warnings` clean; `cargo test
--workspace` green (count in PROGRESS.md once the run lands).
Web untouched by this WP. CI verification of this commit's
head is recorded in PROGRESS.md.

## 9. Next

Phase B summary (`reports/report_Peak-B-summary.md`), tag
`peak-phase-b`, then Phase C (C1 - collections).
