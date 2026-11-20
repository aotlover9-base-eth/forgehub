# ForgeHub - Project Roadmap

> **What it is:** a cloud home for shell command snippets (web app) + a very fast Rust CLI (`fg`) that records, stores, pulls, previews and runs them.
> **Working name:** ForgeHub (binary `forgehub`, alias `fg`). Step 1 checks the name is usable; if not, rename via one find-and-replace and record it in an ADR.
> **Builder:** Muse, high thinking. **Rules of engagement:** `agent.md` (read it first, every session).
> **Steps:** 22 · **Commits:** 22 (one per step, tagged `step-1 … step-22`) · **Reports:** `reports/report_Step1.md … report_Step22.md`
> **Design language:** strictly Material You (Material Design 3), controlled colour.

---

## 0. How to use this file

1. Put `agent.md` and `roadmap.md` in an empty folder that will become the repo root. Git is installed and authenticated.
2. Start Muse and paste **Kickoff Prompt (§12)** once.
3. Then paste each **Step prompt** in order (Step 1, 2, …). Each step ends with: verification → report → one commit → tag → push.
4. At each 🛑 **CHECKPOINT** Muse stops and waits for you to say `CONTINUE`. Review the last reports, try the thing, then continue.
5. Every prompt assumes `agent.md` is the law. Prompts only add step-specific detail.

---

## 1. Vision

> Save a command once. Run it anywhere with `fg <name>`.

A person pastes `sudo apt update && sudo apt -y upgrade && sudo apt -y install curl wget tar htop tmux ca-certificates` into the website, names it `ubuntu-setup`, and saves. On a fresh machine they install the CLI, run `fg login`, then `fg ubuntu-setup`. The CLI shows the exact command, asks `[y/N]`, and runs it.

The same CLI also works offline like Forgenesis: `fg record xyz` captures what you type into a named workflow, `fg xyz` runs it. `fg push xyz` publishes it to your cloud account like `git push`.

### 1.1 Goals
- Accounts, login, and a personal library of snippets (private by default).
- Public and unlisted sharing so others can run `fg alice/ubuntu-setup`.
- Versioned, immutable snippet history (every push is a new version) with diff view.
- Rust CLI that is **fast**, keeps all Forgenesis local features, and adds cloud commands and device-code login.
- Honest, strong safety UX around executing remote text.
- A web UI in strict Material 3 that is calm, accessible and adaptive (phone → desktop).

### 1.2 Non-goals (v1)
- No file uploads, binaries, attachments, or large artifacts. **Text only, ≤ 64 KiB per version.**
- No sandboxing or "safe execution" claims. Commands run on the user's machine with their permissions, after explicit consent.
- No teams/orgs, comments, billing, email notifications, or OAuth providers (listed under Future).
- No telemetry.

### 1.3 Interpretation of the idea (assumptions made - change them before Step 2 if wrong)
- "Store codes but not files" → store **commands/scripts as text**; no file hosting.
- Snippets are **either** a plain **script** (what a person pastes on the web: any multi-line shell text, runs as one script) **or** a list of **steps** (what `fg record` produces; each step runs in its own shell, like Forgenesis).
- Names are lowercase; cloud address is `owner/name`; `fg name` resolves **local first, then your own cloud snippets**, and `fg owner/name` fetches someone else's.
- The CLI binary is `forgehub`; `fg` is an alias installed by `forgehub alias install` (because `fg` is a shell job-control builtin, see §6).

---

## 2. Locked decisions

| Area | Decision | Reason |
|------|----------|--------|
| Repo | Monorepo, private GitHub repo, `main` branch | Simple, one history, 22 commits |
| CLI | Rust stable, `clap`, `serde`, `reqwest` (rustls), `ratatui`/`crossterm`, `rusqlite` (local history) | Speed, single static-ish binary |
| Shared lib | `forge-core` crate used by CLI **and** server | One definition of snippet format, name rules, hashing, secret scan |
| Server | Rust, `axum`, `tokio`, `sqlx` (SQLite WAL + FTS5), `tower-http` | Same language, fast, one deployable binary |
| Web | React + TypeScript + Vite + `@material/web` + TanStack Query + React Router | Official Material 3 web components |
| Hosting model | One container: server serves `/api/*` and the built web app | Cheap and simple to self-host |
| Auth (web) | Email + password (argon2id), cookie sessions | Familiar; no external dependency |
| Auth (CLI) | Device-code flow → personal API token (hashed in DB) | Like `gh auth login`; no password in the terminal |
| Snippet format | YAML "forge file" (superset-compatible with Forgenesis workflows) | Reuse, import/export |
| Versions | Immutable, integer per snippet; content-hashed (SHA-256) | Trust, diff, pinning |
| Visibility | `private` (default), `unlisted`, `public` | Sharing control |
| Search | SQLite FTS5 over name, description, tags, owner, body | No extra service |
| Design | Material 3, seed `#36618E`, tonal surfaces, controlled colour | User requirement |
| Telemetry | None | Trust |

---

## 3. Architecture

```
            ┌─────────────────────────┐            ┌────────────────────────────┐
            │  Browser (React + M3)   │            │  Terminal: fg (forgehub)   │
            │  create / edit / explore│            │  record · run · push · pull│
            └───────────┬─────────────┘            └─────────────┬──────────────┘
                        │  cookie session                         │  Bearer token (fgh_…)
                        ▼                                         ▼
                 ┌──────────────────────────────────────────────────────┐
                 │  forge-server (axum)  /api/v1/*  +  static web files │
                 │  auth · snippets · versions · search · tokens · limits│
                 └───────────────┬──────────────────────────────────────┘
                                 │ sqlx
                          ┌──────▼──────┐
                          │ SQLite (WAL)│  users · sessions · tokens · snippets
                          │ + FTS5      │  versions · stars · device_codes
                          └─────────────┘

   forge-core (shared): snippet model · YAML parse/validate · variables · name rules
                        SHA-256 hashing · secret scanner · danger lints · size limits

   CLI local state:  ~/.forgehub/{config.toml, workflows/, cache/, trust.db, history.db, backups/}
   CLI credentials:  ~/.config/forgehub/credentials.toml  (0600)   [or OS keyring, optional]
```

---

## 4. Design language - Material You (M3), strictly

The full rules are in `agent.md` §8. Summary of what reviewers will check:

- **Colour:** one seed (`#36618E`) → generated light/dark schemes → CSS custom properties. Neutral surfaces; `primary` only for actions/selection/focus; `tertiary` rare; `error` only for errors. **No gradients. No multi-colour tags.** Tag chips are neutral (outlined) - never colourful.
- **Typography:** M3 type scale, Roboto Flex/Roboto + Roboto Mono for code, self-hosted.
- **Shape:** M3 shape scale (4/8/12/16/28/full). Buttons are full-round, cards `medium`/`large`, dialogs `extra-large`.
- **Elevation:** tonal surface containers; shadows only at levels 1–3 for menus/dialogs.
- **Components:** top app bar (small/medium), navigation rail (≥600 px) / navigation bar (<600 px), filled/tonal/outlined/text buttons, FAB (new snippet), filled text fields, chips (assist/filter), cards (outlined/filled), lists, tabs, dialogs, snackbars, menus, search bar, switch, progress.
- **Adaptive:** compact / medium / expanded layouts; list–detail on expanded.
- **States:** hover/focus/pressed state layers; visible focus ring; reduced motion respected.
- **Screens designed for all states:** loading (skeleton/progress), empty (explanation + one action), error (retry), offline.
- **CLI:** same semantic roles in terminal form - one accent, red/amber/green only for error/warning/success, rounded "surface" cards for the confirm prompt, `NO_COLOR` respected.

**CLI confirm card (reference mock):**

```
╭─ alice/ubuntu-setup · v3 · sha256:9f2c1a7e ───────────────────────────╮
│ Update an Ubuntu machine and install the basics                       │
│ owner: you · visibility: private · mode: script · shell: bash         │
╰───────────────────────────────────────────────────────────────────────╯
   1 │ sudo apt update && sudo apt -y upgrade && sudo apt -y install curl wget tar htop tmux ca-certificates

  ! uses sudo (will ask for your password)
Run this on this machine? [y/N]
```

---

## 5. Git snapshot & report protocol (summary)

Full detail in `agent.md` §5–§6.

- Git is installed and authenticated. **After every step: exactly one commit**, message `step N: <summary>`, tag `step-N`, `git push origin main --follow-tags`.
- After every step: write and save `reports/report_StepN.md` (template in `agent.md` §6) **inside that step's commit**; tick `PROGRESS.md`.
- No commit if gates fail. No history rewriting.

### 5.1 Commit table

| Step | Commit message | Report file |
|------|----------------|-------------|
| 1 | `step 1: bootstrap monorepo, study forgenesis, CI skeleton` | `reports/report_Step1.md` |
| 2 | `step 2: specs, ADRs, threat model, API contract` | `reports/report_Step2.md` |
| 3 | `step 3: forge-core shared library` | `reports/report_Step3.md` |
| 4 | `step 4: local CLI core (init, record, run, manage)` | `reports/report_Step4.md` |
| 5 | `step 5: local CLI parity (vars, stats, backup, doctor, alias)` | `reports/report_Step5.md` |
| 6 | `step 6: server foundation` | `reports/report_Step6.md` |
| 7 | `step 7: web account auth and sessions` | `reports/report_Step7.md` |
| 8 | `step 8: API tokens and device-code login` | `reports/report_Step8.md` |
| 9 | `step 9: snippet and version API` | `reports/report_Step9.md` |
| 10 | `step 10: search, tags, stars, forks, profiles` | `reports/report_Step10.md` |
| 11 | `step 11: CLI login, whoami, credentials` | `reports/report_Step11.md` |
| 12 | `step 12: CLI push, pull, remote list, fork` | `reports/report_Step12.md` |
| 13 | `step 13: run from cloud with safety UX` | `reports/report_Step13.md` |
| 14 | `step 14: web design system (Material 3)` | `reports/report_Step14.md` |
| 15 | `step 15: web auth screens and dashboard` | `reports/report_Step15.md` |
| 16 | `step 16: web snippet editor and detail pages` | `reports/report_Step16.md` |
| 17 | `step 17: web explore, profiles, settings, device approval` | `reports/report_Step17.md` |
| 18 | `step 18: CLI TUI, offline mode, completions, polish` | `reports/report_Step18.md` |
| 19 | `step 19: security hardening pass` | `reports/report_Step19.md` |
| 20 | `step 20: end-to-end tests and performance pass` | `reports/report_Step20.md` |
| 21 | `step 21: packaging, installer, release pipeline` | `reports/report_Step21.md` |
| 22 | `step 22: docs, final audit, v0.1.0` | `reports/report_Step22.md` |

### 5.2 Checkpoints (Muse stops and waits for `CONTINUE`)

| Checkpoint | After step | What you should review |
|-----------|-----------|------------------------|
| 🛑 1 | Step 5 | The local CLI works like Forgenesis (try `fg record`, `fg run`, `--dry`) |
| 🛑 2 | Step 13 | Full CLI ↔ server loop: login, push, `fg ubuntu-setup` from the cloud, safety prompts |
| 🛑 3 | Step 17 | The whole web UI: Material 3 look, colour restraint, responsive layouts |
| 🛑 4 | Step 20 | Before packaging/release: test results, performance numbers, security notes |

---

## 6. Shell alias note (`fg` is a builtin)

In bash/zsh, `fg` is job control and builtins beat `$PATH` binaries, so a binary called `fg` is not reachable interactively. Forgenesis solved this with an alias. ForgeHub does the same, automatically:

- Binary name: `forgehub`.
- `forgehub alias install [--shell bash|zsh|fish]` appends a clearly marked block to the shell rc file: `alias fg='forgehub'` (aliases are expanded before builtins in interactive shells). It also offers `alias jobs-fg='builtin fg'` so job control is not lost. Idempotent, with `alias uninstall`.
- Non-interactive scripts should call `forgehub` directly.
- fish: use a function wrapper `function fg; forgehub $argv; end` (`funcsave`).
- The installer (Step 21) asks before touching the rc file.

---

## 7. Steps

Each step lists **Goal**, **Why**, the **Prompt for Muse**, **Acceptance criteria**, and the **Commit**. The prompt is paste-ready. Every prompt implicitly ends with: *"Finish per agent.md: gates → report → PROGRESS.md → one commit → tag → push."*

---

### Step 1 - Bootstrap, study Forgenesis, CI skeleton

**Goal:** a clean, private GitHub repo with the monorepo skeleton, tooling, CI, docs scaffolding, and a written analysis of what to reuse from Forgenesis.
**Why:** everything later depends on a sound layout and on not rewriting working code.

**Prompt for Muse**

```text
STEP 1 - Bootstrap. Follow agent.md (read it fully, including the Step protocol).

1. Verify tools: git, gh (optional), rustc/cargo (stable), node (>=20) and npm. Print versions. If something is missing and cannot be installed, record it in the report.
2. Initialise the repo in the current folder (git init if needed, branch main). Put agent.md and roadmap.md at the root (they already exist). If `gh` is authenticated, create a PRIVATE repo named `forgehub` under the authenticated account and set `origin`; otherwise ask me for the remote URL once.
3. Name check: search crates.io, npm and GitHub for `forgehub` / `forge-hub` collisions (use the tools you have; do not guess). If `forgehub` is taken on crates.io for a binary crate it does not matter for the repo, but record the findings and pick the final binary name (default `forgehub`). Write ADR 0001.
4. Study https://github.com/aotlover9-base-eth/forgenesis: clone it into a throwaway folder OUTSIDE this repo (e.g. /tmp/forgenesis-ref). Read its README, Cargo.toml, src layout, workflow YAML model, variable resolution, recorder (`fg record`), runner (`fg run`), history/stats SQLite code, sync, packs, TUI, and tests. Produce docs/forgenesis-analysis.md: module map, what to port as-is, what to redesign (and why), licence notes (it is MIT, by the same author), and a list of its CLI commands with the ForgeHub equivalent for each. Do NOT copy its history; port code in later steps with attribution in a comment header where substantial.
5. Create the skeleton:
   - Cargo workspace: crates/forge-core (lib), crates/forge-cli (bin `forgehub`), crates/forge-server (bin). Each compiles with a trivial main/test. `#![forbid(unsafe_code)]`.
   - web/: Vite + React + TypeScript (strict). Install @material/web and @tanstack/react-query and react-router-dom. A placeholder App that renders "ForgeHub". Vitest + Testing Library + ESLint + Prettier configured. Do not design anything yet.
   - docs/ (adr/, README), reports/, scripts/, .gitignore (Rust, Node, env files, *.db, build output), .editorconfig, LICENSE (MIT, ask me for the copyright name only if you cannot find it in git config), README.md (honest: "work in progress"), .env.example, rust-toolchain.toml (stable), rustfmt.toml, deny.toml (draft), PROGRESS.md (checklist of all 22 steps, Step 1 ticked on completion).
   - .github/workflows/ci.yml: on push/PR run fmt, clippy -D warnings, cargo test --workspace, and web lint/typecheck/test/build. Cache cargo and npm.
6. Make all gates in agent.md section 9 that exist so far pass locally.
7. Write reports/report_Step1.md (template in agent.md), commit "step 1: bootstrap monorepo, study forgenesis, CI skeleton", tag step-1, push.
```

**Acceptance**
- [ ] Private repo exists with `origin` set; `main` pushed.
- [ ] Workspace and web app build; CI file present and syntactically valid.
- [ ] `docs/forgenesis-analysis.md` lists modules, reuse plan, and command mapping.
- [ ] ADR 0001 records the name decision.
- [ ] `PROGRESS.md` lists all 22 steps.

**Commit:** `step 1: bootstrap monorepo, study forgenesis, CI skeleton`

---

### Step 2 - Specs, ADRs, threat model, API contract

**Goal:** turn the appendices of this roadmap into precise, versioned specs that the code will be built against.
**Why:** a runner for remote text needs the format, the rules and the threats written down before code exists.

**Prompt for Muse**

```text
STEP 2 - Specifications. Follow agent.md.

Using roadmap.md Appendices A–E as the starting point, write and refine these documents under docs/. Where the appendix is ambiguous or you see a flaw, fix it in the doc and explain the change in an ADR.

1. docs/spec-snippet-format.md - the "forge file" YAML: fields, types, limits, both modes (`script` and `steps`), variables and their resolution order, env, cwd, conditions, retry/timeout, `always` cleanup, canonical serialisation rules used for hashing (must be deterministic: define key order, newline normalisation to \n, trailing whitespace policy, UTF-8 only), name/slug/username regexes, reserved words, size limits, a Forgenesis-compatibility table (what imports cleanly, what is converted, what is rejected). Include 8+ worked examples including the Ubuntu setup one-liner.
2. docs/spec-data-model.md - tables, columns, indexes, constraints, FTS5 design, migration policy. Include an ER diagram (mermaid).
3. docs/openapi.yaml - OpenAPI 3.1 for every endpoint in Appendix B plus request/response schemas, the error envelope, pagination, auth schemes (cookie session, bearer token), rate-limit headers. Must validate (use an OpenAPI linter if available offline, otherwise explain how you checked).
4. docs/spec-cli.md - every command, flag, exit code, output format (human and `--json`), name resolution algorithm for `fg <name>` including precedence (subcommand > local > own cloud > error with suggestions), reserved-word list, offline behaviour, and the exact confirm-prompt UX (see roadmap section 4 mock), trust model, diff-on-change behaviour.
5. docs/threat-model.md - STRIDE-style table for: account takeover, token theft, malicious public snippet (the main one), typosquatting of names, snippet swapping after trust (TOCTOU), XSS via user content, CSRF, enumeration, abuse/spam, DoS via large bodies/regex, supply-chain of the CLI installer. For each: mitigation and which step implements it.
6. docs/design-spec.md - expand roadmap section 4 into concrete page inventory, navigation map, per-screen layouts for compact/medium/expanded, token list, component list, empty/loading/error states, content design (microcopy tone: calm, direct, no jokes in error states).
7. ADRs for: SQLite vs Postgres, script vs steps modes, how scripts are executed locally (temp file 0700 vs `bash -s` stdin; note `read`/TTY gotchas), session cookie vs token for web, hash/pinning design, secret scanning approach.

No code in this step except trivial schema files if needed. Cross-check the docs against each other (names, endpoints, limits) and list any inconsistencies you resolved in the report.
```

**Acceptance**
- [ ] The seven documents exist, are mutually consistent, and cite exact limits.
- [ ] OpenAPI file validates (or the validation method is documented).
- [ ] Threat model maps each threat to a step number.

**Commit:** `step 2: specs, ADRs, threat model, API contract`

---

### Step 3 - `forge-core` shared library

**Goal:** the shared crate that defines what a snippet is.
**Why:** CLI and server must agree byte-for-byte on parsing, hashing and validation.

**Prompt for Muse**

```text
STEP 3 - forge-core. Follow agent.md and docs/spec-snippet-format.md.

Implement crates/forge-core (no network, no filesystem side effects except in explicitly named modules; fast to compile and start):
1. model: Snippet/Workflow types (name, description, tags, mode, shell, variables, env, cwd, steps | script, always), serde (YAML). Both `script` and `steps` modes. Steps accept bare strings or maps like Forgenesis (command aliases run/cmd/exec, name, if, wait, retry, cwd, env, continue_on_error, background, quiet, timeout, parallel).
2. parse + validate: strict errors with line/column and helpful messages; limits (64 KiB body, 20 tags, 280-char description, max steps); reject invalid UTF-8 and disallowed control chars.
3. names: validators and normalisers for snippet slugs, usernames, tags; reserved-word list shared with the CLI.
4. canonical serialisation + SHA-256 content hash + short-hash display (8 hex) exactly as spec'd; property tests proving determinism and that semantically-identical inputs hash equally.
5. variables: resolution order (CLI args > declared defaults > built-ins HOME PWD DIRNAME DATE TIME TIMESTAMP USER), whole-word `{{var}}`/`$VAR`-style substitution exactly as spec'd (port from Forgenesis where it matches), missing-variable reporting that lists ALL missing names at once.
6. secret scanner: detect private-key blocks, AWS access keys, GitHub/GitLab/Slack/Stripe-style tokens, bearer tokens, `password=`/`--password` literals, high-entropy strings in assignment context. API returns findings with line numbers and a redaction function. Tunable, low false-positive, with a corpus test (positive and negative samples).
7. danger lints (warnings only, never blocking): rm -rf on / or ~ or $HOME, dd to block devices, mkfs, fork bomb, chmod -R 777 /, curl|sh and wget|sh, writes to /dev/sd*, `sudo` usage, `eval` of downloaded text. Output structured warnings with line numbers. Document that lints are heuristics, not security.
8. import from Forgenesis workflow YAML (conversion layer + tests using fixtures copied from the Forgenesis repo tests).
9. scripts/scan-staged.sh that runs the secret scanner over `git diff --cached` (via a small `forge-core` example or a tiny bin target) and is used by agent.md section 5.7 from now on.
10. Tests: >= 100 unit tests, property tests (proptest) for hashing/parsing, fuzz-style tests for the parser on garbage input (must never panic). Benchmarks (criterion or simple timing) for parse+hash of a 64 KiB snippet; record numbers in the report.

Public API must be small and documented (rustdoc with examples that are doctests).
```

**Acceptance**
- [ ] Both modes parse/validate; hashes are deterministic and spec-conformant.
- [ ] Parser never panics on fuzz/garbage tests.
- [ ] Secret scanner and lints have positive and negative corpora.
- [ ] `scripts/scan-staged.sh` works and is used for the step's own commit.

**Commit:** `step 3: forge-core shared library`

---

### Step 4 - Local CLI core

**Goal:** `forgehub` works fully offline for the basics: init, record, run, list, show, search, tag, rename, delete.
**Why:** it must be a good local tool before it is a good cloud tool.

**Prompt for Muse**

```text
STEP 4 - Local CLI core. Follow agent.md, docs/spec-cli.md, and port from Forgenesis per docs/forgenesis-analysis.md.

Implement in crates/forge-cli (binary `forgehub`), reading local state from $FORGEHUB_HOME or ~/.forgehub:
1. `init` - create home dir, config.toml (shell, ignore_commands, registry_url, color), workflows/. Idempotent.
2. `record <name>` - subshell with a clear `● rec` prompt marker (bash and zsh; `--simple` reads commands from stdin for other shells), capture until `exit`, drop noise (ls, cd, clear, …) and consecutive duplicates, convert a leading `cd` into `cwd`, run the secret scanner on the captured commands and REFUSE to save secrets (offer redaction via a variable instead). Save as steps-mode YAML. Validate the name via forge-core.
3. `run <name> [KEY=value…]` and the shorthand `forgehub <name>` (resolution: subcommand > local workflow; cloud resolution arrives in Step 13 - leave a clean seam). Flags: --dry, --step (confirm each step), --keep-going/-k, --quiet/-q, -y. Both modes: steps (each in its own shell, honouring if/wait/retry/timeout/env/cwd/background/always) and script (write to a 0700 temp file in a private temp dir, run, delete - per ADR). Exit codes: 0 success, 1 failure, 130 on interrupt. Print the confirm card from docs/spec-cli.md BEFORE running and ask [y/N] unless -y (local workflows you authored may skip confirmation only with -y; default is to ask).
4. `list`, `show <name>`, `search <query>`, `tag <name> +a -b`, `rename`, `delete` (with confirmation), `edit <name>` ($EDITOR, re-validate on save). `--json` on list/show.
5. Terminal UX per agent.md section 8.4: rounded surface cards, one accent colour, NO_COLOR/--color/non-TTY handling, ASCII fallback, what/why/fix errors.
6. Performance: `forgehub --version` <= 20 ms, local run start to prompt <= 50 ms. Measure with hyperfine if available (else a simple loop) and record numbers. Avoid async runtime and heavy init on local paths.
7. Tests: integration tests with assert_cmd + temp HOME (record via --simple, run, dry-run, failure exit codes, step confirmation, name validation, secret refusal). Snapshot only the confirm card layout.

Do not implement variables prompting, stats, backup, doctor, alias, import/export yet (Step 5).
```

**Acceptance**
- [ ] `forgehub record demo` → `forgehub demo --dry` → `forgehub demo` works end to end.
- [ ] Script and steps modes both run; temp script is removed after run.
- [ ] Secrets are refused at record time.
- [ ] Startup numbers recorded and within budget (or deviation explained).

**Commit:** `step 4: local CLI core (init, record, run, manage)`

---

### Step 5 - Local CLI parity (Forgenesis feature set) 🛑 CHECKPOINT 1

**Goal:** everything else Forgenesis did locally, plus the alias installer.
**Why:** the user wants "all these features as well".

**Prompt for Muse**

```text
STEP 5 - Local parity. Follow agent.md, docs/spec-cli.md, docs/forgenesis-analysis.md.

Add to forge-cli:
1. Variables: interactive prompting for missing variables on a TTY; on non-TTY list ALL missing names at once and exit 1. `parameterize <name>` ranks repeated literals and converts them to variables (whole-word replacement), with a preview and confirmation.
2. `clone <name> <new>` (local copy), `import <path|dir|--from-forgenesis>` (YAML files, a directory, or ~/.forgenesis/workflows with lowercase normalisation and collision handling), `export <name> [-o file]`.
3. `analyze` - mine bash/zsh/fish history for repeated sequences (port from Forgenesis; flags --min-count --limit --dry -y --file), grouped into sessions by time gaps, non-overlapping counts, fragment/rotation suppression, secret scrubbing BEFORE anything is written.
4. `stats` - local SQLite history.db: runs, durations, estimated time saved = (commands x seconds) - actual runtime, interrupted runs excluded. Config keys human_overhead_seconds etc.
5. `backup` / `restore` of workflows + config (never credentials) into ~/.forgehub/backups with timestamps; restore asks for confirmation and never overwrites without it.
6. `doctor [topic]` - checks home dir perms, config validity, shell detection, alias presence, rustc/other optional toolchains only if relevant, registry reachability (skipped offline), credential file perms; non-zero exit on real breakage.
7. `alias install|uninstall|show [--shell bash|zsh|fish]` per roadmap section 6: idempotent marked block, backup of the rc file before editing, asks before writing unless -y, also defines `jobs-fg`. Test against temp rc files.
8. Shell completions: `completions bash|zsh|fish|powershell` via clap_complete (dynamic completion of local workflow names for run/show/etc.).
9. `--json` output for list/show/stats/doctor. Stable key names documented in docs/spec-cli.md.
10. Update docs/spec-cli.md where behaviour differs from the spec and say why in the report.
11. Tests as before; add tests for alias idempotence, import collisions, analyze on fixture histories, stats arithmetic, restore safety.

When finished, STOP at CHECKPOINT 1: print the checkpoint message from agent.md section 7 and include in your final message a 6-line manual test script I can run (init, record, run --dry, run, stats, alias install in a temp HOME).
```

**Acceptance**
- [ ] All Forgenesis local features have a ForgeHub equivalent (see mapping table in `docs/forgenesis-analysis.md`) or a documented reason they were dropped.
- [ ] Alias installer is idempotent and reversible; rc files are backed up.
- [ ] No credentials in backups. `analyze` never persists secrets.
- [ ] 🛑 Muse stops and prints the checkpoint message.

**Commit:** `step 5: local CLI parity (vars, stats, backup, doctor, alias)`

---

### Step 6 - Server foundation

**Goal:** a running axum server with config, database, migrations, logging, errors and test harness.
**Why:** every feature after this sits on it.

**Prompt for Muse**

```text
STEP 6 - Server foundation. Follow agent.md, docs/openapi.yaml, docs/spec-data-model.md.

In crates/forge-server:
1. Config from env with validation and a printed (secret-free) summary at start: FORGEHUB_BIND, FORGEHUB_BASE_URL, FORGEHUB_DATABASE_URL (sqlite path), FORGEHUB_SECRET_KEY (>= 32 bytes, required in production), FORGEHUB_ENV=dev|prod, FORGEHUB_ALLOW_SIGNUP=true|false, limits. Update .env.example.
2. SQLite via sqlx with WAL, foreign keys ON, busy timeout, connection pool sizing; migrations in crates/forge-server/migrations from docs/spec-data-model.md (users, sessions, api_tokens, device_codes, snippets, snippet_versions, stars, snippet_tags, FTS5 tables + triggers). Migrations run at start. Repository traits (UserRepo, SnippetRepo, ...) with SQLite impls so Postgres can come later.
3. HTTP skeleton: `/api/v1/health` (db check), `/api/v1/version`, consistent error envelope and `IntoResponse` mapping, request IDs, structured tracing (JSON in prod, pretty in dev), graceful shutdown, request body limit (default 256 KiB), timeouts, gzip/br compression, CORS disabled by default (same-origin app), static file serving for ../web/dist with SPA fallback and cache headers (hashed assets immutable).
4. Security headers middleware (CSP without inline scripts, nosniff, frame-ancestors none, referrer-policy, permissions-policy) - define the CSP in one place and test it.
5. Rate-limiting middleware (per-IP and per-token key, in-memory token bucket with configurable limits, `RateLimit-*` and `Retry-After` headers), applied per route group later.
6. Test harness: spin up the app on an ephemeral port with an in-memory/temp SQLite, helper client, factories. Tests for health, error envelope, headers, body limits, migrations idempotence.
7. `docs/running.md` - how to run server + web in dev (proxy /api from Vite) and prod.
```

**Acceptance**
- [ ] `cargo run -p forge-server` starts, migrates, serves `/api/v1/health`.
- [ ] Security headers and body limit verified by tests.
- [ ] Repository traits isolate SQL from handlers.

**Commit:** `step 6: server foundation`

---

### Step 7 - Web account auth and sessions

**Goal:** sign up, log in, log out, session cookies, current user endpoint.
**Why:** accounts are the root of ownership.

**Prompt for Muse**

```text
STEP 7 - Account auth. Follow agent.md section 10 and docs/threat-model.md.

Implement server endpoints from docs/openapi.yaml (auth group):
1. POST /api/v1/auth/signup (username, email, password) - validate username regex/reserved names, email format, password >= 10 chars with a small common-password denylist; argon2id (parameters documented, tuned to ~50-100 ms on the dev machine, stored in the PHC string); unique username and email (case-insensitive); respect FORGEHUB_ALLOW_SIGNUP; generic errors that do not reveal whether an email exists where avoidable (document the unavoidable username-exists case).
2. POST /api/v1/auth/login - constant-time-ish behaviour (hash a dummy when user not found), generic failure message, per-IP and per-account throttling with exponential backoff, session rotation.
3. POST /api/v1/auth/logout and /logout-all; GET /api/v1/me; PATCH /api/v1/me (display name, bio ≤ 280, change password requiring current password, which revokes other sessions).
4. Sessions: random 256-bit id in an HttpOnly, Secure (not in dev), SameSite=Lax cookie; only a SHA-256 hash stored; idle and absolute expiry (config); `last_used_at` updates throttled.
5. CSRF: mutating cookie-authenticated requests require a matching Origin/Host and a per-session CSRF token (header `X-CSRF-Token` returned from /me). Bearer-token requests are exempt (tokens arrive in Step 8).
6. Auth extractors/middleware: `CurrentUser` (cookie or bearer), `RequireUser`, with 401/403 envelopes.
7. Tests: signup/login happy path, duplicate user, weak password, wrong password, throttling, session expiry, logout-all, CSRF failure, cookie flags, password change revokes sessions, SQL injection attempts in fields, very long inputs.

No email sending exists yet: do NOT implement password reset or email verification; document the gap in docs/known-gaps.md (account recovery) so it is visible.
```

**Acceptance**
- [ ] Passwords never logged or returned; sessions stored hashed.
- [ ] CSRF and cookie flags covered by tests.
- [ ] Known gaps documented honestly.

**Commit:** `step 7: web account auth and sessions`

---

### Step 8 - API tokens and device-code login

**Goal:** personal tokens plus the device-code flow the CLI will use.
**Why:** CLI login without typing a password in a terminal, like `gh auth login`.

**Prompt for Muse**

```text
STEP 8 - Tokens and device flow. Follow agent.md and docs/openapi.yaml.

1. API tokens: format `fgh_` + 32 random bytes base62; store SHA-256 hash + 8-char prefix for display; scopes `read` and `write` (write implies read; `push` creates/updates snippets); optional expiry; `last_used_at`; revoke. Endpoints: GET/POST /api/v1/tokens, DELETE /api/v1/tokens/{id}. Token plaintext is shown ONLY at creation. Limit tokens per user.
2. Bearer auth in the extractor; tokens cannot create/revoke tokens or change password (cookie session only), tested.
3. Device-code flow (RFC 8628 spirit): POST /api/v1/auth/device/code {client_name} -> {device_code (high entropy, stored hashed), user_code (8 chars from an unambiguous alphabet, displayed as ABCD-EFGH), verification_uri, verification_uri_complete, expires_in 600, interval 5}; POST /api/v1/auth/device/token {device_code} -> pending | slow_down | access_denied | expired | {access_token, token_type}; browser side: GET /api/v1/auth/device/lookup?user_code=… and POST /api/v1/auth/device/approve|deny (cookie session + CSRF, shows client_name and requested scope, requires explicit approval). Limits: user_code attempts per IP and per code (lock after N wrong tries), single use, short expiry, polling-rate enforcement.
4. Approved device tokens are normal tokens named "CLI on <client_name> (<date>)" visible and revocable in settings.
5. Tests: full flow with a fake CLI client; expired, denied, replayed, brute-force lockout, slow_down, token scope enforcement, revoked token rejected immediately, token hash never returned.
6. Update openapi.yaml and docs/spec-cli.md (login UX) if anything changed.
```

**Acceptance**
- [ ] Device flow works end to end in tests, including every failure branch.
- [ ] Tokens are hashed at rest and shown once.
- [ ] Token cannot mint or revoke tokens.

**Commit:** `step 8: API tokens and device-code login`

---

### Step 9 - Snippet and version API

**Goal:** create, read, update (new version), delete snippets with correct authorisation.
**Why:** the core data of the product.

**Prompt for Muse**

```text
STEP 9 - Snippets API. Follow agent.md, docs/spec-snippet-format.md, docs/openapi.yaml. Reuse forge-core for ALL validation/hashing.

Endpoints:
1. POST /api/v1/snippets - create {name, description, visibility, tags[], mode, body} where body is the forge-file YAML OR plain script text (server wraps/normalises per spec). Slug unique per owner. Creates version 1.
2. GET /api/v1/snippets/{owner}/{name}[?version=N] - latest or specific. Public/unlisted readable without auth; private only by owner (return 404, not 403, to avoid leaking existence). Response includes: metadata, version, content hash, body, parsed summary (mode, steps count, variables, lint warnings, uses_sudo).
3. PUT /api/v1/snippets/{owner}/{name} - owner only; creates a NEW immutable version (reject if identical hash to latest, return 200 with `unchanged: true`); optional `message`. PATCH for metadata only (description, visibility, tags) without a new version. Renaming allowed with a redirect record so old `owner/name` resolves for a grace period (document policy).
4. DELETE - owner only, soft delete with 30-day retention then purge job; recreate-name rules documented.
5. GET /api/v1/snippets/{owner}/{name}/versions and /versions/{n}; GET /api/v1/snippets/{owner}/{name}/diff?from=a&to=b (unified diff computed server-side, size-limited); GET .../raw returns text/plain nosniff of the latest body.
6. GET /api/v1/me/snippets (paginated, filter by visibility/tag, sort by updated).
7. Server-side secret scanning on every create/update: reject with 422 and findings (line numbers, kinds, NOT the secret value). Danger lints returned as warnings, never blocking.
8. Limits: body <= 64 KiB, per-user snippet count (default 500), versions per snippet (default 200), rate limits on writes. 
9. Authorisation tests are the priority: every endpoint x {anonymous, other user, owner, read-only token, write token} x {private, unlisted, public}. Also: name collisions, case folding, reserved names, huge bodies, invalid YAML, concurrent updates (optimistic concurrency via `If-Match: <version>`), soft-delete behaviour, redirect after rename.
10. Update openapi.yaml; keep it in sync with the implementation (add a test that compares route list to the spec if feasible).
```

**Acceptance**
- [ ] Private snippets never leak (404 for non-owners) - tested across all endpoints.
- [ ] Every write creates an immutable version with a stable hash identical to what the CLI will compute.
- [ ] Secret scanning rejects; lints warn.

**Commit:** `step 9: snippet and version API`

---

### Step 10 - Search, tags, stars, forks, profiles

**Goal:** discovery and social basics.
**Why:** a "GitHub for snippets" needs finding and sharing, but it must stay simple and abuse-resistant.

**Prompt for Muse**

```text
STEP 10 - Discovery. Follow agent.md and the specs.

1. GET /api/v1/search?q=&tag=&owner=&sort=relevance|stars|updated&cursor= over PUBLIC snippets only, using FTS5 (name, description, tags, owner, body) with BM25 ranking, prefix matching, snippet highlights returned as plain text ranges (not HTML). Sanitise FTS query syntax (users must not be able to inject FTS operators that crash or slow queries); length limits; rate limits.
2. GET /api/v1/tags (popular public tags), GET /api/v1/explore (recent, popular this week by stars - computed cheaply, cached for 60 s).
3. Stars: PUT/DELETE /api/v1/snippets/{owner}/{name}/star (public/unlisted only; one per user; counter maintained transactionally), GET /api/v1/me/stars.
4. Forks: POST /api/v1/snippets/{owner}/{name}/fork {name?} - copies the latest version into the caller's account as private, records `forked_from` + version + hash; shows "forked from" and fork count; fork of private snippet only by owner.
5. Profiles: GET /api/v1/users/{username} (display name, bio, join date, public snippet count) and /users/{username}/snippets (public only, paginated). No email exposure ever.
6. Pull counter: increment `pull_count` on anonymous or non-owner fetch of PUBLIC snippets via the CLI path (a `?intent=run` query param set by the CLI, rate-limited, deduplicated per IP+snippet per hour); it is a public popularity number, not user tracking - no per-user pull history is stored.
7. Anti-abuse: report endpoint POST /api/v1/snippets/{owner}/{name}/report {reason} (stored for admin review, rate-limited); a minimal admin CLI subcommand in forge-server (`forge-server admin hide-snippet|ban-user|list-reports`) - no admin web UI in v1. Hidden snippets return 404 publicly.
8. Tests: search relevance fixtures, FTS injection attempts, pagination stability, star/fork authorisation, private-content never in search/explore/profile, pull-count dedup, report flow.
9. Performance: seed 10k snippets in a test/bench; record p95 for search and snippet read against the budget in agent.md section 9.
```

**Acceptance**
- [ ] Search never returns private/unlisted/hidden snippets.
- [ ] FTS operator injection is neutralised and tested.
- [ ] p95 numbers recorded.

**Commit:** `step 10: search, tags, stars, forks, profiles`

---

### Step 11 - CLI login, whoami, credentials

**Goal:** `fg login` through the browser; credentials stored safely.
**Why:** first piece of the cloud side in the CLI.

**Prompt for Muse**

```text
STEP 11 - CLI auth. Follow agent.md, docs/spec-cli.md, device flow in docs/openapi.yaml.

In forge-cli add an HTTP client module (reqwest, rustls, blocking or a tiny single-thread runtime used ONLY for cloud commands so local commands keep their startup speed; document the choice). User-Agent `forgehub/<version>`; timeouts; retries with backoff for idempotent calls only; proxy env vars respected; `--registry <url>` / config `registry_url` / `FORGEHUB_REGISTRY`.

Commands:
1. `login [--registry URL] [--token]` - default: device flow: request code, print the verification URL and the user code in a surface card, try to open the browser (`xdg-open`/`open`, skip silently when headless/SSH), poll respecting `interval` and `slow_down`, handle denied/expired with clear messages, store the token. `--token` reads a token from stdin (for CI), never from argv.
2. `logout [--revoke]` - delete local credentials; with --revoke also call the API to revoke the token.
3. `whoami` - username, registry, token name/prefix, scopes, expiry (surface card; `--json`).
4. Credential storage: ~/.config/forgehub/credentials.toml (XDG aware), dir 0700, file 0600, atomic writes, refuse to read if permissions are too open (warn + fix offer). Keyed by registry URL so several registries work. Optional OS keyring is OUT of scope for now (note as a deferred idea).
5. Never print tokens; redact in --verbose logs; `FORGEHUB_TOKEN` env var override for CI.
6. `doctor` gains cloud checks: registry reachable, token valid, clock skew warning.
7. Tests: spin up the real forge-server (test harness from Step 6) in integration tests and drive the full login via a scripted "browser" approving the code; denied, expired, slow_down, bad registry URL, credential permission handling, `--token` via stdin.
```

**Acceptance**
- [ ] `login` → `whoami` → `logout --revoke` works against a real local server in tests.
- [ ] Credentials file permissions enforced; tokens never printed.
- [ ] Local command startup time unchanged (re-measured).

**Commit:** `step 11: CLI login, whoami, credentials`

---

### Step 12 - CLI push, pull, remote list, fork

**Goal:** git-like cloud commands for the user's own snippets.
**Why:** "record it and push it like git".

**Prompt for Muse**

```text
STEP 12 - CLI cloud sync. Follow agent.md and docs/spec-cli.md.

Commands (all `--json` capable):
1. `push <name> [--public|--unlisted|--private] [-m message] [--as remote-name]` - validate locally with forge-core, secret-scan locally (refuse, show findings), POST create or PUT new version (`If-Match` for optimistic concurrency), print result: `owner/name v3 sha256:xxxxxxxx` and the web URL. If the remote latest hash equals the local hash say "up to date". Support `push --all` with a summary table. Visibility defaults to private unless the remote already exists (keep current visibility).
2. `pull <owner/name|name> [--as local-name] [--version N] [--force]` - fetch, show the confirm-style preview (it is untrusted text), validate, save as a local workflow; never overwrite an existing local workflow without --force or confirmation; record provenance (remote, version, hash) in a sidecar `.meta` so `status` can compare.
3. `status [name]` - compare local vs cloud (ahead/behind/diverged/unchanged by hash and version), like `git status`. `diff <name>` shows a unified diff local vs cloud latest.
4. `remote list [--mine|--stars]`, `remote show <owner/name>`, `remote delete <name>` (confirmation, shows what will be deleted), `remote visibility <name> public|unlisted|private` (confirmation when going public, with a reminder to review for sensitive content), `remote search <query>`.
5. `fork <owner/name> [--as name]` -> server fork then optional pull.
6. `record` gains `--push` convenience: after saving, ask "Push to cloud now? [y/N]" (only when logged in).
7. Error UX: not logged in -> tell the exact command (`forgehub login`); 401 expired token; 404 vs private; 409 conflict (someone pushed from another machine) -> suggest `pull`/`diff`; 422 secret findings; 429 with Retry-After.
8. Integration tests against the real test server: round trip push/pull equality (hash identical on both sides - this is critical), conflict (409), visibility changes, delete, fork, offline error messages, `--all`.
```

**Acceptance**
- [ ] Hash computed by CLI equals hash stored by server for the same content (tested).
- [ ] Optimistic concurrency conflicts are reported and recoverable.
- [ ] No local data is overwritten without confirmation or `--force`.

**Commit:** `step 12: CLI push, pull, remote list, fork`

---

### Step 13 - Run from cloud with safety UX 🛑 CHECKPOINT 2

**Goal:** the headline feature: `fg ubuntu-setup` fetches, shows, asks, runs.
**Why:** this is the product. It is also the riskiest part, so safety UX is implemented here with great care.

**Prompt for Muse**

```text
STEP 13 - Cloud run. Follow agent.md section 10, docs/threat-model.md (malicious public snippet, TOCTOU, typosquatting), docs/spec-cli.md.

Implement name resolution and execution:
1. `forgehub <name>` and `forgehub run <name|owner/name[@version]>` resolution order: built-in subcommand > local workflow > logged-in user's own cloud snippet (`me/<name>`) > explicit `owner/name`. If nothing matches, print "not found" plus up to 3 suggestions (similar local names; `remote search` top hits) - never auto-run a suggestion. `forgehub run <name>` bypasses the subcommand shadowing problem.
2. Fetch with ONE request (`GET /snippets/{owner}/{name}?intent=run`), cache the immutable version+hash in ~/.forgehub/cache for offline replay and diff. Validate with forge-core; re-compute the hash locally and compare with the server's (mismatch = hard error).
3. Confirm card (see roadmap section 4 mock): name, owner, version, short hash, visibility, mode, description, EVERY command/line numbered (long lines wrapped, never truncated), warnings from lints (sudo, rm -rf, curl|sh, etc.), variables that will be prompted. Prompt `Run this on this machine? [y/N]` (default No). 
4. Trust model (~/.forgehub/trust.db, SQLite): 
   - own snippets (owner == logged-in user): `[y/N]`; `-y` allowed.
   - someone else's: first time requires typing `yes` in full; shows owner, stars, age, pull count if available, and "this snippet is not yours" in warning colour; `-y` does NOT skip this unless that exact (owner,name,hash) has been trusted before via `forgehub trust owner/name@hash` or an earlier explicit `yes`.
   - hash changed since last trusted run: show a unified DIFF of old vs new, a clear "this snippet changed since you last ran it" notice, and require confirmation again; `-y` does not bypass.
   - `forgehub trust list|revoke`.
5. Flags: --dry (print only, exit 0), --step, --no-cache, --offline (use cache only), --version/@N pinning, --json for dry output, -y as above. Variables resolved/prompted as in Step 5. Never log the body to disk except the cache.
6. Execution identical to local run (steps/script modes), with the same exit codes; add `exit code <n>` and duration summary line; non-interactive stdin without -y and without prior trust must FAIL SAFE (print the card, exit 2, do not run).
7. Defences: refuse bodies that fail validation; refuse if the snippet contains terminal control/escape sequences in displayed text (render them visibly escaped so a snippet cannot hide commands with ANSI tricks or carriage returns - test with a malicious fixture that tries `\r`, ANSI clear-line, and Unicode bidi overrides); wrap very long single lines instead of truncating.
8. Tests (priority): full e2e against test server for own private snippet, others' public snippet, changed-hash diff flow, pin to version, offline replay, name shadowing, typo suggestion, malicious-display fixtures, non-TTY fail-safe, hash mismatch. Plus a manual script in the report: save `ubuntu-setup` on the web API, `forgehub ubuntu-setup --dry`.

When finished, STOP at CHECKPOINT 2 and print: `CHECKPOINT 2 reached after Step 13. Reply CONTINUE to proceed to Step 14.` Include a short manual test script for me (start server, signup via curl, login, push, run --dry).
```

**Acceptance**
- [ ] `fg ubuntu-setup` shows the card, asks `[y/N]`, runs only on yes.
- [ ] Third-party snippets need typed `yes`; changed hash shows a diff and re-confirms.
- [ ] Hidden-text tricks (ANSI/CR/bidi) are visibly escaped in the card.
- [ ] Non-interactive run without trust fails safe.

**Commit:** `step 13: run from cloud with safety UX`

---

### Step 14 - Web design system (Material 3)

**Goal:** tokens, theme, components and the app shell - before any real screens.
**Why:** design consistency and colour restraint are enforced from one place.

**Prompt for Muse**

```text
STEP 14 - Web design system. Follow agent.md section 8 STRICTLY and docs/design-spec.md. Read the official Material 3 guidance (use web search/fetch if you have it) and verify the current state of @material/web (it is the official M3 web component library; if its status or API differs from what docs assume, record it in an ADR and proceed with the best option that is still true Material 3).

1. Tokens: generate light/dark colour schemes from seed #36618E with @material/material-color-utilities; commit web/src/design/tokens.css (CSS custom properties: --md-sys-color-*, typescale, shape, elevation, motion, spacing, state-layer opacities) and tokens.ts. Map them onto @material/web's --md-* theming properties so library components pick them up. Theme switcher (system/light/dark) persisted in localStorage with try/catch, applied before first paint to avoid flash.
2. Fonts: self-host Roboto Flex (or Roboto) and Roboto Mono, subsetted, with font-display swap. Material Symbols Outlined self-hosted subset with the icons we use.
3. Component library in web/src/design/components: AppShell (top app bar + navigation rail/bar/drawer, adaptive by width class), PageHeader, Card (filled/outlined/elevated), ListItem, Chip (neutral outlined for tags), Button wrappers, TextField wrappers with error text, CodeBlock (Roboto Mono, copy icon-button, optional line numbers, shows long lines wrapped), EmptyState, ErrorState, Skeleton, Snackbar/ToastProvider, ConfirmDialog, Menu, Tabs, SearchBar, Fab, Badge-like "Public/Private" status chip (tonal, using secondary-container; tertiary only for "Public").
4. Layout system: width classes compact/medium/expanded with a useWidthClass hook; list-detail pane; max content width.
5. A hidden dev route `/__design` (only in dev builds) showing every component and state in light and dark for review and for screenshot tests.
6. web/scripts/check-tokens.mjs: fail on hex colours, raw rgb()/hsl(), px border-radius, or font-size literals outside design/tokens*.css. Wire into `npm run lint`.
7. Accessibility: focus rings, contrast verified programmatically for key role pairs (on-surface/surface, on-primary/primary, on-secondary-container/secondary-container, error pairs) in both schemes - write a test that computes WCAG contrast and fails below AA.
8. Tests: component tests (Testing Library) + jest-axe on the design page; visual snapshots only if cheap and stable (Playwright optional - if you add it, note browser install needs).
9. Colour restraint review: list in the report every place accent/tertiary/error is used and justify each. If a component uses more than neutral + primary, fix it.
```

**Acceptance**
- [ ] No hardcoded colours/radii/fonts outside the token files (script enforces it).
- [ ] Light/dark schemes pass AA contrast tests.
- [ ] `/__design` shows every component, adaptive shell works at 3 widths.

**Commit:** `step 14: web design system (Material 3)`

---

### Step 15 - Web auth screens and dashboard

**Goal:** sign up, log in, and "my snippets".
**Why:** first real user-visible flows.

**Prompt for Muse**

```text
STEP 15 - Auth screens + dashboard. Follow agent.md section 8 and docs/design-spec.md.

1. API client layer (typed from docs/openapi.yaml; generate types if a generator is available offline, otherwise hand-write and keep in sync), TanStack Query setup, CSRF token handling from /me, error envelope -> UI messages, 401 handling that redirects to login with return URL.
2. Screens: /login, /signup (live validation hints matching server rules, show/hide password, password guidance without forcing weird complexity), logout, /  (landing for logged-out: one calm hero with what it is, the `fg ubuntu-setup` example in a CodeBlock, primary "Create account" + text "Explore"), /dashboard (logged-in home): my snippets list with name, description, visibility chip, updated time, version, tags (neutral chips), per-item actions menu (copy run command `fg name`, copy raw URL, change visibility, delete with ConfirmDialog), filters (all/private/unlisted/public), sort, empty state with a single "New snippet" action and a short "install the CLI" card.
3. Copy-to-clipboard with Snackbar feedback; the run command copied is `fg <name>` for own snippets and `fg <owner>/<name>` otherwise.
4. Responsive behaviour per design-spec; keyboard shortcuts minimal (`/` focuses search later; `n` new snippet) with discoverable help.
5. Tests: component and integration tests with a mocked API (MSW) covering success, validation errors, server errors, rate limit message, session expiry redirect; axe checks on each screen.
6. Manual QA at 360, 768 and 1280 px in light and dark: describe what you checked; add screenshots to docs/screenshots/ only if you can capture them reliably (PNG, small).
```

**Acceptance**
- [ ] A new user can sign up, log in and see an empty dashboard with a clear next action.
- [ ] Layout adapts to compact/medium/expanded; axe finds no serious issues.
- [ ] Colour usage stays within the restraint rules.

**Commit:** `step 15: web auth screens and dashboard`

---

### Step 16 - Web snippet editor and detail pages

**Goal:** the heart of the web UI: create/edit a snippet and view it with versions and diff.
**Why:** this is where "paste your command, name it `ubuntu-setup`, save" happens.

**Prompt for Muse**

```text
STEP 16 - Snippet editor + detail. Follow agent.md section 8 and docs/design-spec.md.

1. /new and /{owner}/{name}/edit: a calm single-column form on compact, two-pane (editor + live preview) on expanded:
   - Name (live slug validation, availability check debounced), description, visibility (segmented/radio with plain-language explanations; default Private), tags (chip input, neutral chips, limit 20).
   - Body editor: a monospace textarea (Roboto Mono, no heavy third-party editor unless it is tiny and justified in an ADR) with line numbers optional, tab handling that does not trap keyboard focus (Esc then Tab leaves), soft-wrap toggle. Two modes via tabs: "Script" (paste commands; default) and "Steps (YAML)".
   - Live panel using forge-core compiled to WASM IF that is practical (wasm-bindgen/wasm-pack available offline); otherwise call a server validate endpoint (POST /api/v1/snippets/validate) debounced. Show: validation errors with line numbers, detected variables, secret findings (blocking, with line numbers, never echoing the secret), danger-lint warnings (non-blocking, amber, plain language), "uses sudo", step/line count, size meter against 64 KiB.
   - Save as new version with an optional message; unsaved-changes guard; optimistic concurrency conflict dialog (someone saved from the CLI meanwhile) offering "view their version / overwrite / copy mine".
2. /{owner}/{name} detail page: header (owner avatar-initial, name, visibility chip, stars, forks, pull count), description, tags, the "Run it" card: `fg owner/name` (or `fg name` for your own) with copy button, and the install-the-CLI one-liner link; CodeBlock with the body (line numbers, wrapped long lines, copy), warnings summary; tabs: Code · Versions · (Forks later). Versions tab: list with message, date, short hash; select two to view a unified diff (rendered as text with M3 surfaces; additions/deletions distinguished by background tone + a +/- gutter symbol, NOT by green/red alone - keep colour controlled and accessible).
3. Actions (owner): edit, change visibility, delete (ConfirmDialog requiring the name), star/unstar (others), fork, report (dialog with reason, rate-limit message), "Raw" link.
4. Safety copy on third-party public snippets: a neutral info banner "Review commands before you run them. `fg` will ask you to confirm." - informative, not scary.
5. Render ALL user text as text (no HTML injection); add tests with hostile content (HTML tags, ANSI escapes, bidi overrides, giant lines) to prove it is inert/visible.
6. Tests: editor validation flows, save/conflict, version diff rendering, private snippet 404 page, hostile content, axe, keyboard-only walkthrough.
```

**Acceptance**
- [ ] Creating `ubuntu-setup` from the web with the example command works; `fg ubuntu-setup` (dev server) then shows it.
- [ ] Secrets are blocked in the UI with line-numbered findings; lint warnings are non-blocking.
- [ ] Diff view is readable without relying on red/green alone.
- [ ] Hostile content renders inert.

**Commit:** `step 16: web snippet editor and detail pages`

---

### Step 17 - Web explore, profiles, settings, device approval 🛑 CHECKPOINT 3

**Goal:** finish the web surface: discover, profiles, account settings, token management, and the `/device` page for CLI login.
**Why:** completes the loop and lets you review the entire UI.

**Prompt for Muse**

```text
STEP 17 - Remaining web screens. Follow agent.md section 8 and docs/design-spec.md.

1. /explore: search bar (Material search), filter chips (neutral), sort, results list (compact cards: name, owner, description, tags, stars, updated), highlights from the API rendered safely, infinite scroll or "Load more" with stable cursors, empty and no-results states, popular tags row.
2. /{username}: profile (display name, bio, join date, public snippets list). Never shows email.
3. /settings with tabs: Profile (display name, bio), Security (change password with current password, active sessions list with revoke + "sign out everywhere"), Tokens (list name/prefix/scopes/last used/expiry; create dialog with scopes and expiry, plaintext shown ONCE in a CodeBlock with copy and an explicit "I've copied it" step; revoke with confirm), CLI (install instructions, `forgehub login` explanation), Danger zone (delete account: type username to confirm; implement the server endpoint `DELETE /api/v1/me` if not present - it must soft-delete then purge snippets/tokens/sessions and be tested).
4. /device: the CLI login approval page - enter or confirm user code (prefilled from `verification_uri_complete`), shows the client name and "This will let the CLI read and write your snippets", Approve / Deny, success and error states, works when not logged in (redirect to login then back). Clear, calm, accessible; it is a security-critical page: no auto-approve, no framing (frame-ancestors none), code input with strong brute-force rate-limit messaging.
5. Global: 404 page, error boundary page, offline banner, theme switcher in settings and top bar menu, footer links (docs, source, privacy note: "no tracking, no third-party scripts").
6. Performance: route-level code splitting, check bundle size (report numbers), no layout shift on load, fonts preloaded.
7. Tests: as before + the device approval flow end to end with MSW, token reveal-once behaviour, account deletion confirmation.
8. Run a full Material 3 + colour-restraint review across ALL screens at 3 widths and both themes; list findings and fixes in the report with before/after notes.

When finished, STOP at CHECKPOINT 3: print `CHECKPOINT 3 reached after Step 17. Reply CONTINUE to proceed to Step 18.` and give me a 10-line click-through script of the web UI.
```

**Acceptance**
- [ ] `forgehub login` → browser `/device` approve → CLI logged in (manually verified with dev server).
- [ ] Token plaintext appears once only.
- [ ] Account deletion works and is tested.
- [ ] 🛑 Muse stops and prints the checkpoint message.

**Commit:** `step 17: web explore, profiles, settings, device approval`

---

### Step 18 - CLI TUI, offline mode, completions, polish

**Goal:** make the CLI delightful and robust: TUI browser, offline behaviour, polish.
**Why:** Forgenesis had `fg ui`; the cloud adds a second tab.

**Prompt for Muse**

```text
STEP 18 - CLI polish. Follow agent.md section 8.4 and docs/spec-cli.md.

1. `forgehub ui` (ratatui + crossterm): tabs Local · Mine (cloud) · Explore; list with fuzzy filter (`/`), detail pane with the same numbered command view as the confirm card, keys: j/k or arrows, Enter/r run (goes through the SAME confirm/trust path as the CLI - no shortcuts), d dry-run, p push, l pull, f fork, s star, e edit, ? help, q quit. Terminal restored on panic/Ctrl-C. Colour: one accent, dim secondary, nothing else beyond error/warn/success. Works at 60x20 minimum; respects NO_COLOR and ASCII fallback. State logic separated from rendering and unit-tested (no terminal needed).
2. Offline mode: `--offline` flag and automatic graceful degradation (cache for previously fetched snippets with a clear "from cache, may be outdated" line; push/pull fail with a helpful message). Cache management: `forgehub cache list|clear`, size cap and LRU eviction in config.
3. `forgehub update --check` (query GitHub releases or the registry's /version for latest; DOES NOT self-replace; prints how to upgrade). Pin behaviour in docs.
4. Output polish: consistent spacing, error messages (what/why/fix), progress indicator only on TTY, `--json` consistency review across ALL commands (stable schema, documented), exit codes table in docs/spec-cli.md, `--verbose`/`-v` with redaction.
5. Man page and `--help` quality: examples in every subcommand help (clap `after_help`), generate a man page via clap_mangen into docs/.
6. Final measurement of startup and local run times; fix regressions. Re-measure binary size (strip, LTO, codegen-units=1, opt-level choices documented).
7. Tests: TUI state machine tests, offline/caching tests, update check against a fake server, help-text snapshot tests for the top-level help.
```

**Acceptance**
- [ ] TUI runs through the same safety path as the CLI (tested).
- [ ] Offline mode behaves predictably with clear messaging.
- [ ] Startup and size budgets met or deviations justified.

**Commit:** `step 18: CLI TUI, offline mode, completions, polish`

---

### Step 19 - Security hardening pass

**Goal:** an adversarial review and fixes, driven by the threat model.
**Why:** this product executes text from the internet - it deserves a dedicated step.

**Prompt for Muse**

```text
STEP 19 - Security hardening. Follow agent.md section 10 and docs/threat-model.md. Act as an attacker first, defender second.

1. Walk every row of docs/threat-model.md; for each, write the attack, try it against the running system (local only), and record result and fix. Update the table with "verified in step 19" evidence.
2. Auth: session fixation, cookie flags in prod mode, CSRF bypass attempts (missing/mismatched Origin, token reuse), login throttling bypass via header spoofing (trusted-proxy config: X-Forwarded-For only from configured proxies), timing differences on login, token enumeration, device-code brute force and phishing risk (the approval page must clearly show what is being approved).
3. Input: oversized bodies, deeply nested YAML (billion-laughs/anchors/aliases - forbid or cap aliases and depth in the parser), pathological regex/FTS queries, unicode edge cases (homoglyph usernames/names - decide and implement a policy: ASCII-only names), Unicode bidi/control chars in descriptions, null bytes.
4. Output: XSS attempts across every field (names, descriptions, tags, bodies, diffs, search highlights, error messages), CSP report-only then enforced, check headers with a script, verify `raw` endpoints cannot be framed or sniffed.
5. CLI: terminal escape injection via snippet text (done in Step 13 - re-verify in `ui`, `show`, `diff`, `pull`, `remote show`), symlink/permission issues in ~/.forgehub, temp-script handling (mode 0700 dir, O_EXCL, deleted on all exits including signals), credential file permission checks, no secrets in logs/crash output, update-check cannot be abused to run code.
6. Supply chain: `cargo deny` (licences + advisories + bans) configured and green; `cargo audit`; `npm audit --omit=dev`; pin GitHub Actions to commit SHAs; `Cargo.lock` and `package-lock.json` committed; minimal permissions in CI; fuzz targets (cargo-fuzz if available, else proptest-based) for the YAML parser, secret scanner and diff endpoint.
7. Rate limit and abuse review: signup/login/device/search/push/report limits tuned and documented; per-user quotas; admin commands tested (hide snippet, ban user, list reports).
8. Write docs/security.md (how to report vulnerabilities, supported versions, user-facing safety guidance for running snippets) and a SECURITY.md at the repo root.
9. Fix everything you find in this step (inside this step's single commit); anything you cannot fix goes to docs/known-gaps.md with severity and plan.
```

**Acceptance**
- [ ] Every threat-model row has evidence of an attempted attack and a result.
- [ ] `cargo deny`, `cargo audit` and `npm audit --omit=dev` are green or have documented exceptions.
- [ ] Parser resists alias/depth bombs; terminal-injection tests pass everywhere text is displayed.
- [ ] `SECURITY.md` and `docs/security.md` exist.

**Commit:** `step 19: security hardening pass`

---

### Step 20 - End-to-end tests and performance pass 🛑 CHECKPOINT 4

**Goal:** prove the whole system works together and meets the performance budgets.
**Why:** confidence before packaging.

**Prompt for Muse**

```text
STEP 20 - E2E + performance. Follow agent.md section 9.

1. scripts/e2e.sh: builds everything, starts the server on an ephemeral port with a temp DB, then drives the real binaries: signup (API), device-flow login (scripted approval via API with the session cookie), record via --simple, push, change on "web" (API PUT), pull, status/diff, fork, public snippet visible from a second HOME as another user with the typed-yes trust flow, changed-hash diff flow, hidden-snippet behaviour, token revoke, account delete. Fails loudly with logs on any mismatch. Runs in CI as a separate job.
2. Web E2E with Playwright if browsers can be installed (otherwise document and use jsdom-level integration tests): signup -> create `ubuntu-setup` -> view -> edit -> versions/diff -> settings token flow -> device approval. Capture screenshots at 3 widths for the report (small PNGs in docs/screenshots/ only if stable).
3. Load/perf: seed 10k users/50k snippets; measure p50/p95/p99 for read, search, list, push with a simple load tool (oha/wrk/k6/hey if available, else a small Rust bench client); verify budgets in agent.md section 9; fix hot spots (indexes, query plans via EXPLAIN QUERY PLAN, connection pool, caching headers/ETag + 304 for snippet reads with `If-None-Match`).
4. CLI perf: re-measure startup/local-run/cloud-run overhead; confirm the cloud run path uses exactly one HTTP request (assert in a test with a counting proxy/mock).
5. Backup and restore drill for the SQLite DB (online backup via `VACUUM INTO`/backup API), written up in docs/operations.md with a cron-friendly script scripts/backup-db.sh and a restore test.
6. Resilience: server restart during requests, DB locked scenarios, disk-full error handling (graceful 5xx without leaking details), CLI behaviour on flaky network (timeouts, retries only on idempotent calls).
7. Collect all numbers in the report as tables.

When finished, STOP at CHECKPOINT 4: print `CHECKPOINT 4 reached after Step 20. Reply CONTINUE to proceed to Step 21.` and include a short "release readiness" assessment: what is solid, what is risky, what you would fix first.
```

**Acceptance**
- [ ] `scripts/e2e.sh` passes and runs in CI.
- [ ] Performance budgets met or deviations explained with a plan.
- [ ] Backup/restore drill documented and tested.
- [ ] 🛑 Muse stops with a release-readiness assessment.

**Commit:** `step 20: end-to-end tests and performance pass`

---

### Step 21 - Packaging, installer, release pipeline

**Goal:** make it installable and deployable by someone who is not you.
**Why:** a CLI nobody can install is not a product.

**Prompt for Muse**

```text
STEP 21 - Packaging. Follow agent.md. Do NOT create paid accounts, buy domains, or deploy to production without asking me; prepare everything so deployment is one command.

1. Server: multi-stage Dockerfile (builds web, builds Rust with musl or distroless-compatible target, non-root user, read-only FS friendly, healthcheck), docker-compose.yml (server + volume for SQLite + optional Caddy reverse proxy with automatic HTTPS and sensible headers/trusted-proxy config), `.env.example` complete, docs/deploy.md covering: single VPS with Docker + Caddy, Fly.io example (fly.toml, volume), backups, upgrades, migrations, secrets, first-run admin steps, FORGEHUB_ALLOW_SIGNUP guidance.
2. CLI releases: GitHub Actions release workflow on tag `v*`: build release binaries for x86_64-unknown-linux-musl, aarch64-unknown-linux-musl, x86_64-apple-darwin, aarch64-apple-darwin, x86_64-pc-windows-msvc (CLI works on Windows without alias features that need bash; document limits), strip+LTO, SHA-256 checksums file, and (if feasible) keyless signatures with cosign/GitHub attestations; upload as release assets. Use cargo-dist or hand-written matrix - justify in an ADR. Pin actions by SHA.
3. Installer: scripts/install.sh (POSIX sh): detects OS/arch, downloads the correct asset over HTTPS, VERIFIES the checksum, installs to ~/.local/bin (or a chosen dir), refuses to run as root unless told, never edits shell rc files without asking, then offers `forgehub alias install`. Print exactly what it will do before doing it (the project preaches review-before-run, so the installer must be reviewable: short, commented, no obfuscation). shellcheck-clean. Also `cargo install --path crates/forge-cli` documented, and a Homebrew formula + AUR PKGBUILD + .deb (cargo-deb) as templates in packaging/ (not published).
4. Web: production build served by the server with correct caching; verify the Docker image serves the SPA and API on one port; Lighthouse (or equivalent) run for accessibility/performance with numbers in the report.
5. Versioning: workspace version 0.1.0, CHANGELOG.md (Keep a Changelog), `forgehub --version` includes git sha and build date, `/api/v1/version` matches.
6. Dry-run the release process locally (build the Docker image, run the container, run the e2e script against it, run the installer against a locally served fake release directory).
```

**Acceptance**
- [ ] `docker compose up` yields a working site; e2e script passes against the container.
- [ ] Installer verifies checksums and is shellcheck-clean.
- [ ] Release workflow is valid and pinned; nothing deployed or published without approval.

**Commit:** `step 21: packaging, installer, release pipeline`

---

### Step 22 - Docs, final audit, v0.1.0

**Goal:** documentation that matches reality, a final audit, and a tagged release candidate.
**Why:** finishing is a feature.

**Prompt for Muse**

```text
STEP 22 - Final docs and audit. Follow agent.md.

1. README.md: what it is, 60-second quickstart (web + CLI), the `fg ubuntu-setup` story, install, safety model in plain words, screenshots (if available), links to docs. Everything stated must be true NOW - verify each claim by running it.
2. docs/user-guide.md (web: accounts, creating/sharing snippets, versions, tokens, device login; CLI: record/run/push/pull/fork/trust/offline), docs/cli-reference.md (generated from clap where possible, plus hand-written examples), docs/api.md (from OpenAPI), docs/self-hosting.md (from deploy.md), docs/forgenesis-migration.md (`forgehub import --from-forgenesis`), docs/faq.md (is it safe? how do I stop a snippet from running? why does `fg` need an alias?), docs/known-gaps.md (account recovery/email, orgs, OAuth, admin UI, keyring, Windows limits, etc.), CONTRIBUTING.md, CODE_OF_CONDUCT.md.
3. Final audit checklist (write it in the report with PASS/FAIL and evidence): all 22 steps' acceptance criteria re-verified from a CLEAN clone (git clone into a temp dir, build, run all gates, run e2e), 22 commits + tags step-1..step-22 exist in order, 22 reports exist and are complete, no secrets in history (run the scanner over the entire history), no stray TODOs, dependency licences reviewed, M3 token check passes, a11y checks pass, budgets met.
4. Tag `v0.1.0` (annotated) in addition to `step-22`; do NOT publish a GitHub Release or deploy; tell me the exact commands to do it.
5. Final report: a "project retrospective" section (what went well, risks, next 10 ideas ordered by value) - the Future list in roadmap section 10 is a starting point.
```

**Acceptance**
- [ ] A clean clone builds and passes every gate and the e2e script.
- [ ] Docs verified against behaviour; no claim without evidence.
- [ ] 22 commits, 22 reports, tags `step-1…step-22` and `v0.1.0`.

**Commit:** `step 22: docs, final audit, v0.1.0`

---

## 8. Appendices (canonical starting specs - Step 2 refines them)

### Appendix A - Snippet format ("forge file")

Two modes. Everything is UTF-8, LF newlines, ≤ 64 KiB.

**Script mode** (what you paste on the web - default)

```yaml
name: ubuntu-setup
description: Update an Ubuntu machine and install the basics
tags: [ubuntu, setup]
mode: script
shell: bash            # bash | sh | zsh | fish - executable must exist locally
script: |
  sudo apt update && sudo apt -y upgrade && sudo apt -y install curl wget tar htop tmux ca-certificates
```

The web "Script" tab accepts raw pasted text; the server stores it as the `script:` value with defaults. A script can have multiple lines, `if` blocks, functions and heredocs; state (cd/export) persists across lines because it is one shell invocation.

**Steps mode** (what `record` produces; Forgenesis-compatible)

```yaml
name: deploy-site
description: Build and publish the static site
mode: steps
variables:
  env: { default: staging, description: "Target environment" }
cwd: ~/projects/site
steps:
  - npm ci
  - { name: build, command: "npm run build -- --mode {{env}}", retry: 2, timeout: 5m }
  - { command: "rsync -a dist/ server:/var/www/{{env}}/", if: { which: rsync } }
always:
  - echo "done at $(date)"
```

Each step runs in its own shell (a lone `cd` has no effect; recording converts a leading `cd` into `cwd`). Step fields: `command` (aliases `run`, `cmd`, `exec`), `name`, `if` (exists/missing/which/env/equals/shell), `wait`, `retry`, `cwd`, `env`, `continue_on_error`, `background`, `quiet`, `timeout`, `parallel`.

**Names**
- Snippet name: `^[a-z0-9][a-z0-9._-]{0,63}$` (lowercase; no `..`; no leading dot).
- Username: `^[a-z0-9][a-z0-9-]{1,38}$`, no consecutive hyphens, ASCII only.
- Tags: `^[a-z0-9][a-z0-9-]{0,29}$`, ≤ 20 per snippet.
- Reserved words (cannot be snippet names): every CLI subcommand (`run init record login logout whoami push pull fork status diff list show search tag rename clone delete edit import export analyze stats backup restore doctor alias completions ui cache update trust remote help version`), plus `me`, `api`, `admin`, `settings`, `explore`, `new`, `device`, `login`, `signup`, `static`, `assets`.

**Variables** resolve: CLI `KEY=value` → declared default → built-ins (`HOME PWD DIRNAME DATE TIME TIMESTAMP USER`) → prompt (TTY) → error listing all missing at once.

**Hash:** `sha256` over the canonical serialisation (fixed key order, LF, trimmed trailing whitespace on structural lines only, script body preserved byte-exact apart from LF normalisation and a single trailing newline); displayed as first 8 hex chars; full hash in API. Step 2 finalises the exact rules.

### Appendix B - API surface (`/api/v1`)

| Group | Method + path | Auth | Notes |
|-------|---------------|------|-------|
| Meta | `GET /health`, `GET /version` | none | |
| Auth | `POST /auth/signup`, `/auth/login`, `/auth/logout`, `/auth/logout-all` | none/session | rate-limited |
| Auth | `GET /me`, `PATCH /me`, `DELETE /me` | session (token for GET) | CSRF on mutating |
| Auth | `GET /me/snippets`, `GET /me/stars` | session/token | |
| Tokens | `GET /tokens`, `POST /tokens`, `DELETE /tokens/{id}` | session only | plaintext once |
| Device | `POST /auth/device/code`, `POST /auth/device/token` | none | polling |
| Device | `GET /auth/device/lookup`, `POST /auth/device/approve`, `POST /auth/device/deny` | session | |
| Snippets | `POST /snippets/validate` | optional | no persistence |
| Snippets | `POST /snippets` | write | create v1 |
| Snippets | `GET /snippets/{owner}/{name}` (`?version=`, `?intent=run`) | by visibility | 404 for private non-owner |
| Snippets | `PUT /snippets/{owner}/{name}` | write + owner | new version, `If-Match` |
| Snippets | `PATCH /snippets/{owner}/{name}` | write + owner | metadata only |
| Snippets | `DELETE /snippets/{owner}/{name}` | write + owner | soft delete |
| Snippets | `GET .../versions`, `.../versions/{n}`, `.../diff?from=&to=`, `.../raw` | by visibility | |
| Social | `PUT/DELETE .../star`, `POST .../fork`, `POST .../report` | session/token | |
| Discovery | `GET /search`, `/explore`, `/tags` | none | public only |
| Users | `GET /users/{username}`, `/users/{username}/snippets` | none | public only |

Error envelope: `{ "error": { "code": "…", "message": "…", "details": {} } }`. Pagination: `?limit=&cursor=`. Rate-limit headers on every response.

### Appendix C - Data model (sketch)

```
users(id, username UNIQUE NOCASE, email UNIQUE NOCASE, password_hash, display_name, bio, created_at, disabled_at, deleted_at)
sessions(id_hash PK, user_id FK, csrf_token, created_at, last_used_at, expires_at, ip_hint, ua_hint)
api_tokens(id, user_id FK, name, prefix, token_hash UNIQUE, scopes, created_at, last_used_at, expires_at, revoked_at)
device_codes(id, device_code_hash, user_code, client_name, status, user_id NULL, created_at, expires_at, last_polled_at, attempts)
snippets(id, owner_id FK, name, description, visibility, latest_version, star_count, fork_count, pull_count, forked_from_snippet_id NULL, hidden_at NULL, deleted_at NULL, created_at, updated_at, UNIQUE(owner_id, name))
snippet_versions(id, snippet_id FK, version, mode, body, hash, message, created_at, UNIQUE(snippet_id, version))
snippet_tags(snippet_id, tag)            -- index on tag
stars(user_id, snippet_id, created_at)   -- PK(user_id, snippet_id)
renames(owner_id, old_name, snippet_id, expires_at)
reports(id, snippet_id, reporter_id, reason, status, created_at)
snippets_fts  -- FTS5 over name, description, tags, owner, body of PUBLIC latest versions (maintained by triggers/app code)
```

### Appendix D - CLI command reference (target)

```
forgehub init | record <name> | run <name|owner/name[@N]> [K=V…] | <name> (shorthand)
forgehub list | show <name> | search <q> | tag <name> +a -b | rename | delete | edit | clone | import | export
forgehub analyze | parameterize <name> | stats | backup | restore | doctor | alias install|uninstall|show | completions <shell>
forgehub login | logout | whoami | push <name> | pull <owner/name> | status | diff | fork | remote list|show|delete|visibility|search
forgehub trust list|revoke | cache list|clear | ui | update --check
Global flags: --registry URL  --json  --color auto|always|never  -v/--verbose  -q/--quiet  -y/--yes  --offline
Run flags: --dry  --step  --keep-going/-k  --no-cache  --version N
Exit codes: 0 ok · 1 failure · 2 refused/not confirmed (fail-safe) · 3 not found · 4 auth required · 130 interrupted
```

### Appendix E - Security checklist (carried into the threat model)

- Execution: explicit confirmation, full visible commands, short hash, owner, version; typed `yes` for third-party; diff on change; fail-safe without TTY; escape/bidi/CR neutralisation in display; no auto-run; `-y` limited.
- Accounts: argon2id; throttling; generic errors; session rotation/expiry; CSRF; token hashing/scoping/revocation; device-code brute-force limits.
- Content: text only; size limits; secret scan (client + server); lints are warnings; XSS-proof rendering; CSP; nosniff; no framing.
- Operations: rate limits; quotas; admin hide/ban; backups; no telemetry; supply-chain checks; signed/checksummed releases; installer verifies checksums.

---

## 9. Definition of Done for the whole project

- [ ] 22 steps complete; 22 commits `step N: …`; tags `step-1…step-22`; `v0.1.0` tag.
- [ ] 22 reports in `reports/`.
- [ ] CLI: all Forgenesis local features + cloud login/push/pull/fork/run with safety UX; startup budgets met.
- [ ] Web: Material 3, controlled colour, adaptive, accessible (AA), no hardcoded design values.
- [ ] Security hardening done; threat model has evidence; known gaps documented honestly.
- [ ] Clean-clone build + all gates + e2e script green; Docker image runs.

---

## 10. Future ideas (not in v1)

Email verification and password reset · OAuth login (GitHub/Google) · organisations and shared team libraries · collections/playlists of snippets · parameter prompts with typed inputs (choices, secrets that are never stored) · snippet signing (author keys) and `fg verify` · OS keyring credentials · PostgreSQL backend · admin web UI · comments and discussions · web-based "dry-run explain" using static analysis · browser extension "Run with fg" button · `fg run` in a container/bubblewrap sandbox as an opt-in · import from shell history on the web · public snippet SEO pages · self-update for the CLI · Windows PowerShell mode.

---

## 11. Troubleshooting for the builder

- **`fg: no such job`** → alias not active; run `forgehub alias install`, restart the shell, or call `forgehub` directly.
- **Gates failing in CI but not locally** → pin toolchain via `rust-toolchain.toml`; ensure lockfiles committed.
- **Muse loses context** → paste: *"Resume per agent.md §2: read PROGRESS.md and the latest report, check git status, continue the current step."*
- **A step is too large for one session** → Muse may split work internally, but must still produce exactly one commit and one report for the step.
- **Locked decision seems wrong** → Muse writes an ADR and raises it in the report; it does not silently deviate.

---

## 12. Kickoff Prompt (paste once, before Step 1)

```text
You are Muse, working on ForgeHub with HIGH THINKING for the entire project.

Read agent.md and roadmap.md completely before doing anything. They are the source of truth.
Rules you must follow for the whole project:
- Git is installed and authenticated. After EVERY step make exactly ONE commit "step N: <summary>", tag step-N, push. There are 22 steps = 22 commits.
- After EVERY step write and save reports/report_StepN.md (template in agent.md section 6) and include it in that step's commit; tick PROGRESS.md.
- Design language is strictly Material You (Material Design 3) with a controlled, restrained colour palette (one seed #36618E, neutral surfaces, accent only for actions/selection/focus). No hardcoded colours, radii or fonts outside the token files.
- Rust for the CLI and server; React + TypeScript + @material/web for the web; text-only snippets (no file uploads).
- Safety UX for running remote text is a core feature (confirmation, visible commands, hashes, diff on change, fail-safe without a TTY).
- Do not claim anything passed unless you ran it. Do not skip gates. Stop at the four CHECKPOINTS and wait for me to say CONTINUE.

Acknowledge in 5 lines: the product, the 22-step plan, the commit/report rule, the design rule, and the checkpoints. Then wait for me to paste the Step 1 prompt.
```
