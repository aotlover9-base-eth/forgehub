# Forgenesis analysis — what ForgeHub reuses, ports, redesigns or drops

- **Source studied:** https://github.com/aotlover9-base-eth/forgenesis
- **Revision studied:** `f69cdf2` ("Forgenesis: forge once, execute forever"), the only commit in the cloned history (`git log --oneline` shows one commit)
- **Cloned to:** `/tmp/forgenesis-ref` (throwaway, outside this repo, per Step 1)
- **Licence:** MIT, Copyright (c) 2026 Aotlover9 — the same author as ForgeHub. Reuse and porting are permitted; substantial ports in later steps must carry an attribution comment header (per `roadmap.md` Step 1 §4).
- **Size:** single Rust package, binary `fg`, ~7,512 lines of Rust in `src/` plus a 491-line README; its README claims 126 unit tests (verified: 126 `#[test]` attributes in `src/`).

This document is the reuse plan for Steps 3–5 and 18. No Forgenesis code was
copied in Step 1; porting happens in the steps named below.

## 1. Module map (`src/`)

| File | Lines | Responsibility (from its own module docs) |
|------|------:|--------------------------------------------|
| `main.rs` | 1,404 | clap CLI: all subcommands, command dispatch, most command implementations (`cmd_init`, `cmd_record`, `cmd_run`, `cmd_list`, `cmd_show`, `cmd_parameterize`, `cmd_import/export`, `cmd_delete/rename/clone/tag/search`, `cmd_stats`, `cmd_analyze`, `cmd_sync`, `cmd_install/plugins/update`, `cmd_backup/restore`, `cmd_ui`, `cmd_config`, `cmd_doctor`) |
| `lib.rs` | 18 | Crate root; re-exports the 14 modules below |
| `model.rs` | 624 | Workflow YAML document: `Workflow`, `StepSpec` (bare string / map / bool), `StepMap` (aliases `run`/`cmd`/`exec`), `ConditionSpec`/`Condition` (`exists`, `missing`, `which`, `shell`, `env`, `equals`), `RetrySpec`/`Retry` (`retry: 3` or `{times, delay}`), `ParamSpec`/`Param`, validation, `validate_name`, YAML round-trip |
| `runner.rs` | 1,003 | Execution engine: conditions, retries, timeouts, parallel dispatch, background/quiet steps, `Mode::{Normal, Dry, Step}`, `RunReport`/`StepResult`, exit behaviour |
| `record.rs` | 420 | `fg record`: interactive shell wrapping (bash `--rcfile`, zsh `ZDOTDIR`) with a per-command hook, `--simple` stdin capture, pure post-processing in `clean_captured` (drops noise, consecutive duplicates, recording-session commands; leading `cd` becomes workflow/step `cwd`) |
| `analyze.rs` | 587 | History mining: bash/zsh/fish history parsing, session grouping by time gaps, non-overlapping repeated-sequence counting, fragment/rotation suppression, `Candidate::suggested_name`, secret scrubbing before anything is written |
| `vars.rs` | 206 | `{{VAR}}` placeholders (`find_placeholders`, `substitute` — errors list **all** missing names at once, `substitute_lossy`), built-ins (`HOME`, `PWD`, `DIRNAME`, `DATE`, `TIME`, `TIMESTAMP`, `USER`), `KEY=value` parsing, `~`/`$VAR` path and env expansion |
| `config.rs` | 202 | `Paths` (`$FORGENESIS_HOME` or `~/.forgenesis`: config, workflows, plugins, backups, `history.db`), project `.forgenesis/` discovery, `Config` (shell, `confirm_before_run`, `parallel_limit`, theme, editor, `human_overhead_seconds`, history file, analyze thresholds, `ignore_commands`, `registry_url`), TOML load/save, shell detection |
| `store.rs` | 401 | Workflow storage: global registry plus per-project `.forgenesis/` shadowing, `Origin::{Global, Project}`, load/save/list/search plumbing |
| `db.rs` | 386 | rusqlite (bundled) history DB: WAL, `runs`/`steps` recording, `WorkflowStat`, `Totals`; stats arithmetic lives with `cmd_stats` in `main.rs` (saved = commands × `human_overhead_seconds` − actual runtime; interrupted runs excluded) |
| `parameterize.rs` | 395 | Literal mining and whole-word replacement into `{{VARS}}`: path segments, `--flag=value`/`KEY=value` values, repeated tokens; skips common words |
| `sync.rs` | 516 | Repo sharing through `.forgenesis/`: two-way newest-wins sync, byte-identical files reported unchanged (idempotent, no spurious git diffs), same-timestamp conflicts reported not guessed; also backup/restore helpers |
| `plugins.rs` | 482 | Workflow packs: `plugin.toml` manifests, built-in offline catalog (`react-dev`, `docker`, `rust`, `python`, `node`), install from registry URL or a local directory, installed-plugin listing/removal |
| `doctor.rs` | 286 | Health checks: install/home/config/shell, workflow parse checks, per-ecosystem tool checks (docker, node, python, rust, git, flutter, aws); `Level::{Ok, Warn, Fail}`; non-zero exit on real breakage |
| `tui.rs` | 431 | ratatui + crossterm browser: list + detail, filter state in a testable `App` struct separated from rendering, `Outcome::{Quit, Run, Dry}` (TUI tears down before running so output stays visible) |
| `term.rs` | 151 | Hand-rolled ANSI helpers (no colour crate on the hot path), `NO_COLOR`/`TERM=dumb`/non-TTY handling, duration parsing (`500ms`, `5s`, `2m`, `1h`) and human formatting |

Dependencies (its `Cargo.toml`): `anyhow`, `chrono`, `clap` (derive),
`crossterm`, `dirs`, `ratatui`, `regex`, `rusqlite` (bundled), `serde`,
`serde_yaml`, `shell-words`, `toml`. Release profile: `opt-level = "z"`, LTO,
`codegen-units = 1`, strip. Edition 2021.

## 2. Workflow YAML model — the compatibility baseline

A workflow is YAML with: `name`, `description`, `tags`, `cwd`, `shell`, `env`,
`variables` (aliases `vars`/`params`; each a bare default string or
`{description, default, required}`), `steps`, top-level `parallel`, default
`retry`, `always`, `created`, `source`.

A step is a bare string, a boolean (YAML reads unquoted `true`/`false` as a
command — accepted deliberately), or a map with `command` (aliases `run`,
`cmd`, `exec`), `parallel`, `name`, `if`, `wait`, `retry`, `cwd`, `env`,
`continue_on_error` (alias `ignore_errors`), `background`, `quiet`, `timeout`.

ForgeHub's "forge file" (roadmap Appendix A) is a superset: it keeps all of
the above for **steps mode** and adds **script mode** (`mode: script` +
`script:` body). Step 2's `docs/spec-snippet-format.md` must contain the
Forgenesis-compatibility table; Step 3 implements the conversion layer with
fixtures from this model.

## 3. Port as-is (logic and tests, adapted names/paths)

| Forgenesis piece | ForgeHub destination | Step |
|------------------|----------------------|------|
| `vars.rs`: placeholder finding, all-missing-at-once substitution, built-ins, `KEY=value` parsing, env/tilde expansion | `forge-core` variables module | 3 |
| `model.rs`: step/condition/retry/param untagged-spec normalisation | `forge-core` model (extended with script mode) | 3 |
| `term.rs`: duration parsing; colour gating (`NO_COLOR`, non-TTY) semantics | `forge-core` (durations) and `forge-cli` terminal module (colours, M3-mapped per agent.md §8.4) | 3–4 |
| `record.rs::clean_captured` post-processing and noise/duplicate rules | `forge-cli` record | 4 |
| `runner.rs`: condition evaluation, retry/timeout/parallel/background semantics, dry/step modes, run report | `forge-cli` runner (extended: script mode, confirm card, exit codes per spec) | 4 |
| `analyze.rs`: session grouping, non-overlapping counting, fragment/rotation suppression, suggested names | `forge-cli` analyze (plus pre-write secret scrubbing via `forge-core`) | 5 |
| `parameterize.rs`: literal ranking and whole-word replacement | `forge-cli` parameterize | 5 |
| `db.rs`: history schema and stats arithmetic (interrupted runs excluded) | `forge-cli` local `history.db` | 5 |
| `doctor.rs`: check structure and ecosystem checks | `forge-cli` doctor (plus ForgeHub cloud/alias/credential checks in Steps 5 and 11) | 5 |
| `tui.rs`: `App` state separated from rendering, filter logic and its tests | `forge-cli` TUI state (rendering redesigned: Local/Mine/Explore tabs, same confirm/trust path) | 18 |
| `sync.rs` backup/restore mechanics (timestamps, idempotence, no-guess conflicts) | `forge-cli` backup/restore (never credentials) | 5 |

## 4. Redesign (and why)

| Piece | Redesign | Why |
|-------|----------|-----|
| `model.rs` validation | Move to `forge-core`, make it strict and size-limited (64 KiB body, 20 tags, 280-char description, max steps), with line/column errors; Forgenesis is deliberately forgiving and has no size limits | CLI and server must agree byte-for-byte on validation and hashing (roadmap §2); a cloud store needs hard limits (agent.md §10.5) |
| Name validation | ForgeHub slug rule `^[a-z0-9][a-z0-9._-]{0,63}$`, no `..`, no leading dot, plus the reserved-word list; Forgenesis allows uppercase and validates mainly for filenames | Cloud addresses are `owner/name`; lowercase + reserved words prevent subcommand shadowing and typosquatting surface (Steps 2–3) |
| Hashing / canonical serialisation | New in `forge-core` (SHA-256 over a deterministic canonical form); Forgenesis has none | Trust, diff-on-change and pinning depend on it (Step 13) |
| Secret scanning | New shared scanner in `forge-core`, used by CLI `record`/`push` and by the server; Forgenesis drops credential-looking commands in `analyze`/`record` with local heuristics only | One scanner, three call sites, server must reject secrets on push (agent.md §10.6) |
| Execution confirmation | Forgenesis defaults `confirm_before_run = false`; ForgeHub always shows the confirm card and asks `[y/N]` for cloud snippets, typed `yes` for third-party, diff on hash change, fail-safe without TTY | Safety is the product (agent.md §10.1–10.2); local parity keeps `-y` semantics from the spec |
| Script execution | New: script mode runs via a 0700 temp file in a private temp dir (ADR in Step 2); Forgenesis only has steps, each in its own shell | Pasting a script on the web must preserve `cd`/`export` across lines |
| `main.rs` (1,404 lines, most commands inline) | Split into per-command modules in `forge-cli` | Agent.md §11: modules small and cohesive; cloud commands (Steps 11–13) would make a single file unmaintainable |
| `store.rs` project shadowing (`.forgenesis/` in a repo) | Not in ForgeHub v1 scope; replaced by cloud push/pull and `.meta` provenance sidecars (Step 12) | Roadmap's sharing model is the server, not repo directories; see §5 below |
| `plugins.rs` packs / marketplace | Not in ForgeHub v1 scope (non-goals / Future in roadmap §1.2, §10); public snippets, stars and forks (Step 10) are the v1 sharing mechanism | Locked scope (agent.md R10) |
| Config | `~/.forgehub/config.toml` with `registry_url`, colour mode and the Forgenesis keys that still apply (`shell`, `ignore_commands`, `human_overhead_seconds`, analyze thresholds, `parallel_limit`); `$FORGEHUB_HOME` replaces `$FORGENESIS_HOME` | Same shape, ForgeHub names; credentials live separately in `~/.config/forgehub/credentials.toml` (0600, Step 11), never in config or backups |
| Terminal colours | Forgenesis uses green/red/yellow/blue/magenta/cyan freely; ForgeHub maps to agent.md §8.4: one accent, dim secondary, red/amber/green only for error/warning/success, `NO_COLOR`/`--color`/ASCII fallback | Material You controlled-colour rule applies to the CLI too (R4) |

## 5. Dropped, with reasons

| Forgenesis feature | ForgeHub decision |
|--------------------|-------------------|
| `sync` through a repo `.forgenesis/` directory | Dropped for v1; cloud `push`/`pull` (Step 12) is the sharing path. Recorded here so Step 5's parity check has a documented reason, as its acceptance criteria require. |
| `install` / `plugins` / `update` of workflow packs and the built-in catalog | Dropped for v1; replaced by explore/search/fork (Steps 10, 12). `forgehub update --check` (Step 18) is a different feature: it checks the CLI's own version and never self-replaces. |
| `config` subcommand (`fg config --set …`) | Not in the roadmap's CLI reference (Appendix D); configuration is edited as `config.toml` in v1. Doctor reports config validity (Step 5). Noted as a candidate deferred idea. |
| Binary named `fg` | ForgeHub's binary is `forgehub`; `fg` is an alias (ADR 0001). |

## 6. Command mapping — Forgenesis → ForgeHub

| Forgenesis (`fg …`) | ForgeHub (`forgehub …` / `fg …`) | Step |
|----------------------|----------------------------------|------|
| `init` | `init` (creates `~/.forgehub`) | 4 |
| `record <name> [--simple] [--force] [-d desc]` | `record <name>` (bash/zsh live, `--simple` stdin); secret refusal via `forge-core`; `--push` convenience added | 4, 12 |
| `run <name> [KEY=value…] [--dry] [--step] [-k] [-q] [-y]` | `run <name|owner/name[@N]>` and shorthand `forgehub <name>`; same flags plus cloud resolution, confirm card and trust | 4, 13 |
| `list [--tag] [--long]` | `list` (+ `--json`) | 4 |
| `show <name>` | `show <name>` (+ `--json`) | 4 |
| `search <query> [--market]` | `search <query>` local; cloud discovery is `remote search` / web explore | 4, 12 |
| `tag <name> <tags…> [--remove]` | `tag <name> +a -b` (spec syntax, Appendix D) | 4 |
| `rename`, `clone`, `delete [-y]` | `rename`, `clone`, `delete` (confirmation) | 4–5 |
| `import <file>`, `export <name> [-o]` | `import <path\|dir\|--from-forgenesis>`, `export <name> [-o file]` | 5 |
| `parameterize <name> [--literal --as]` | `parameterize <name>` (preview + confirmation) | 5 |
| `analyze [--min-count] [--limit] [--dry] [-y] [--file]` | `analyze` (same flags; secret scrub before any write) | 5 |
| `stats [--limit] [--explain]` | `stats` (+ `--json`); same saved-time formula | 5 |
| `backup [-o]`, `restore [name] [--force]` | `backup`, `restore` (workflows + config, never credentials) | 5 |
| `doctor [topic]` | `doctor [topic]` (+ alias, credential and registry checks later) | 5, 11 |
| `ui` | `ui` (Local · Mine · Explore; same confirm/trust path) | 18 |
| `sync [--push\|--pull] [--dry-run]` | — dropped (§5); use `push` / `pull` | 12 |
| `install <pack>`, `plugins [--remove]`, `update` | — dropped (§5); `update --check` (CLI version only) | 18 |
| `config [--path] [--set]` | — dropped (§5); edit `config.toml` | — |
| — (no cloud in Forgenesis) | `login`, `logout`, `whoami`, `push`, `pull`, `status`, `diff`, `fork`, `remote …`, `trust …`, `cache …`, `alias install\|uninstall\|show`, `completions <shell>` | 5, 11–13, 18 |

## 7. Test notes

Forgenesis's tests are in-module unit tests (126) covering sequence mining,
YAML parsing, the runner, parameterization, sync and TUI state — all without
spawning a shell or a terminal. ForgeHub keeps that discipline (agent.md §11)
and adds `assert_cmd` integration tests with a temporary `FORGEHUB_HOME`
from Step 4, and server integration tests from Step 6.

## 8. Licence / attribution

Forgenesis is MIT (Copyright (c) 2026 Aotlover9), by the same author as
ForgeHub. Ported code keeps the MIT licence (this repo's `LICENSE` uses the
same copyright holder, found in the Forgenesis `LICENSE` and its git history
— the local `git config` in this environment was unset). Where a port is
substantial, the ForgeHub file will carry a header comment naming Forgenesis
as its source.
