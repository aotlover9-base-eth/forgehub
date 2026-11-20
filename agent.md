# agent.md - Operating Manual for the Coding Agent

> **Project:** ForgeHub (working name) - a cloud home for shell command snippets, with a web app and a fast Rust CLI (`fg`).
> **Agent:** Muse, **high thinking** for the whole project.
> **Companion file:** `roadmap.md` (the plan, the 22 steps, the prompts, the specs).
> **This file** is the law. If a step prompt ever conflicts with this file, stop and flag it in the report instead of guessing.

---

## 1. Mission in one paragraph

Users create an account on a website and store **commands and small shell scripts** (text only - never file uploads, never binaries). A user installs a Rust CLI, aliases it to `fg`, logs in from the terminal, and can then run any stored snippet from anywhere:

```
fg ubuntu-setup
```

The CLI fetches the snippet from the cloud, **shows exactly what it will run**, asks `[y/N]`, and only then runs it. The CLI also keeps all the local features of the author's earlier tool, **Forgenesis** (`record`, local `run`, variables, `--dry`, `--step`, search, tags, stats, backup, TUI…), and adds git-like cloud commands (`login`, `push`, `pull`, `fork`). Reference for the local features: https://github.com/aotlover9-base-eth/forgenesis (Rust, MIT, by the same user - reuse and port it freely, do not reinvent it).

Speed is a feature. Safety is a feature. A snippet runner that executes remote text is a remote-code-execution surface by design, so **honest, loud safety UX is part of the product**.

---

## 2. Session start checklist (do this every time you begin or resume)

1. Read this file fully.
2. Read `roadmap.md` - at minimum: §Locked decisions, §Design language, the step you are about to run, and the appendices it references.
3. Read `PROGRESS.md` (the checklist you maintain) and the **latest** `reports/report_Step*.md`.
4. Run `git status` and `git log --oneline -n 10`. The working tree **must be clean** and the last commit must be the previous step's commit. If not, stop and reconcile before writing code (see §5.6).
5. Run the baseline gates (§9) so you know the starting state is green.
6. Only then plan the step.

If you lost context mid-step, re-do this checklist; `PROGRESS.md` + the last report + `git diff` tell you where you were.

---

## 3. Non-negotiable rules

| # | Rule |
|---|------|
| R1 | **Git is installed and authenticated.** After **every** step you create **exactly one commit** (a "git snapshot"). 22 steps = 22 commits. Details in §5. |
| R2 | After **every** step you write and **save** `reports/report_Step<N>.md` (e.g. `reports/report_Step7.md`) using the template in §6, and that file is **part of that step's commit**. |
| R3 | **Never** mark a step done until its acceptance criteria and the verification gates (§9) pass. Failing gates = no commit. |
| R4 | **Design language is strictly Material You (Material Design 3).** Controlled colour - see §8. No hardcoded colours, radii, or font sizes outside the token files. |
| R5 | **Security first** for anything that touches auth, tokens, snippet execution, or user content - see §10. |
| R6 | **Text only.** The product stores commands/scripts as text with a hard size limit. No file upload endpoint anywhere. |
| R7 | **No telemetry** in the CLI or server beyond what the roadmap explicitly lists. |
| R8 | **Never** commit secrets, `.env` files, tokens, private keys, or real credentials. Use `.env.example`. |
| R9 | **Never force-push, rewrite pushed history, or delete tags.** |
| R10 | **Stay in scope.** If you find something worth doing that is not in the current step, write it in the report under "Deferred / ideas" and move on. |
| R11 | **No fabricated claims.** If you did not run it, do not say it passed. Report real command output (trimmed). |
| R12 | Prefer **boring, well-maintained dependencies**. Every new dependency is justified in the report (what, why, licence, last-release recency). |

---

## 4. Locked technical decisions (summary - full table in roadmap.md)

- **Monorepo**, Cargo workspace + web app:
  ```
  /crates/forge-core     shared lib: snippet/workflow model, YAML parsing, variables, name rules, hashing, secret scan, danger lints
  /crates/forge-cli      the `forgehub` binary (aliased to `fg`)
  /crates/forge-server   axum API + static web hosting + SQLite (sqlx)
  /web                   React + TypeScript + Vite + @material/web (Material 3)
  /docs                  ADRs, specs, CLI reference, user guide
  /reports               report_Step1.md … report_Step22.md
  ```
- **Rust** stable, edition 2021+, `clap`, `serde`, `serde_yaml`/`toml`, `reqwest` (rustls, no OpenSSL), `tokio`, `ratatui`+`crossterm`, `rusqlite` (CLI local history), `sqlx` (server), `argon2`, `sha2`, `tracing`.
- **Database:** SQLite (WAL) for v1 with FTS5 for search, behind repository traits so Postgres is a later swap.
- **Auth:** email+password (argon2id) for web with cookie sessions; **device-code flow (RFC 8628 style)** for CLI login; hashed personal API tokens.
- **Binary name:** `forgehub`; the installer/`forgehub alias install` creates the `fg` alias (because `fg` is a shell builtin - see roadmap §Shell alias).
- If you believe a locked decision is wrong, **do not silently deviate**: write an ADR in `docs/adr/` explaining the problem and your proposal, finish the step with the locked decision, and raise it in the report.

---

## 5. Git protocol (the snapshot rule)

Git is installed and the user is already authenticated (HTTPS credential helper or SSH, whichever `git remote -v` shows).

### 5.1 Branching
- Work on `main`. No feature branches unless the roadmap says so.
- The remote is a **private** GitHub repo (Step 1 creates it with `gh repo create` if `gh` is available; otherwise `git remote add origin <url>` using the URL the user supplies).

### 5.2 One step = one commit
Sequence at the end of every step, in this exact order:

1. Verification gates pass (§9).
2. Write/refresh `reports/report_Step<N>.md`.
3. Tick the step in `PROGRESS.md`.
4. `git add -A` - then **review `git status` and `git diff --cached --stat`**; unstage anything that should not be committed (build output, secrets, large binaries).
5. Commit with the message format below.
6. `git tag step-<N>` (lightweight tag is fine).
7. `git push origin main --follow-tags`.
8. Confirm with `git log --oneline -n 3` that the commit exists and `git status` is clean.

### 5.3 Commit message format
```
step <N>: <imperative summary, ≤ 60 chars>

- <what changed, bullet 1>
- <what changed, bullet 2>
- <tests: what was added / what passes>

Report: reports/report_Step<N>.md
```
Add the attribution trailer lines the environment tells you to add, if any.

### 5.4 What a "snapshot" must contain
Code **and** tests **and** docs touched **and** the step report **and** the updated `PROGRESS.md`. A commit that has code but no report is invalid.

### 5.5 Mistakes in an earlier step
- Fix the problem **inside the current step's commit** and describe it in the current report under "Corrections to earlier steps". Do **not** create extra commits and do not rewrite history.
- Exception: if an earlier-step bug makes the build/tests red so the current step cannot be verified, fix that first (still inside the current step's commit) and say so.

### 5.6 If the repo is not in the expected state
- Uncommitted changes from a crashed session → inspect with `git diff`; if they clearly belong to the unfinished step, continue that step. If unclear, stash to `git stash push -u -m "recovery-<date>"`, tell the user, and continue from the last good commit.
- Never `git reset --hard` or `git clean -fd` without first making a backup branch (`git branch backup/<date>`) and noting it in the report.

### 5.7 Secrets guard
Before every commit, run `git diff --cached` through the repo's secret scan (Step 3 adds `scripts/scan-staged.sh`; before that, eyeball it). If anything looks like a key, token, or password, abort the commit.

---

## 6. Report protocol

After every step write **`reports/report_Step<N>.md`**, then commit it with the step. Use this template - all headings required, write "None" where empty:

```markdown
# Report - Step <N>: <title>

- **Date/time:** <ISO-8601 with timezone>
- **Commit:** <short sha> (tag `step-<N>`)
- **Status:** ✅ Done | ⚠️ Done with caveats | ❌ Blocked

## 1. Summary
<3–6 sentences: what now exists that did not before.>

## 2. What I built
<Bullets by area. Name real files/modules/endpoints/commands.>

## 3. Decisions made
<Each decision + why + alternatives rejected. Link ADRs.>

## 4. Verification
| Gate | Command | Result |
|------|---------|--------|
| Format | `cargo fmt --check` | pass |
| Lint | `cargo clippy ... -D warnings` | pass |
| Tests | `cargo test --workspace` | N passed, 0 failed |
| Web | `npm run lint && npm test && npm run build` | pass |
| Manual | <what you ran by hand> | <observed output, trimmed> |

## 5. Acceptance criteria
- [x] <copied from roadmap, each ticked only if truly verified>

## 6. Security notes
<Threats considered, mitigations added, anything left open.>

## 7. Dependencies added
<name - purpose - licence - why this one>

## 8. Corrections to earlier steps
<None, or what you fixed and why.>

## 9. Known issues / limitations
<Honest list.>

## 10. Deferred / ideas
<Out-of-scope things you noticed.>

## 11. Next step
<One paragraph: what Step N+1 needs to know from this step.>
```

Keep reports factual and compact (usually 1–2 screens). They are read by a human reviewer and by you on resume.

---

## 7. Step protocol (the loop)

For each step:

1. **Think (high thinking):** read the step, list unknowns, look at existing code, design before typing. For non-trivial choices write a short ADR (`docs/adr/NNNN-title.md`).
2. **Plan:** write a 5–12 line plan at the top of your working notes (not committed). Identify tests first.
3. **Implement in small increments.** Compile and test often. Prefer vertical slices that work end-to-end.
4. **Test:** unit tests for logic, integration tests for API/CLI behaviour, snapshot tests only where output is stable. Test failure paths and authorisation, not only the happy path.
5. **Verify:** run all gates (§9) and the step's acceptance criteria by hand.
6. **Self-review:** read your own `git diff` as a hostile reviewer: secrets, panics (`unwrap` in non-test code), missing authz, missing input limits, UI hardcoded colours, unclear errors.
7. **Report → PROGRESS.md → commit → tag → push** (§5, §6).
8. **Checkpoint?** If the roadmap marks this step with 🛑 **CHECKPOINT**, stop after pushing and print:
   `CHECKPOINT <k> reached after Step <N>. Reply CONTINUE to proceed to Step <N+1>.`
   Otherwise continue directly to the next step without asking.

### Autonomy
Work autonomously. Ask the user only when you are blocked by something you cannot decide or do yourself: missing credentials, spending money, an irreversible action, a licence conflict, a name collision, or a locked decision that appears impossible. When you must ask, ask one concise question, propose a default, and continue with unrelated work if any.

### Optional parallelism
If your environment supports sub-agents, you may parallelise **inside** a step (e.g. server tests vs. CLI tests), but one lead agent owns the working tree, the report, and the single commit. Never produce more than one commit per step.

---

## 8. Design language - Material You (Material Design 3), strictly

This applies to the **web app** fully, and to the **CLI/TUI** through the same semantic roles.

### 8.1 Principles
- Follow the official Material 3 guidance: colour roles, type scale, shape scale, elevation by tonal surface, state layers, motion, adaptive layout.
- **Controlled colour.** One seed colour → one generated tonal scheme. Surfaces and text are neutral. The accent (`primary`) is for actions, selection and focus only. `tertiary` is used sparingly (at most one small highlight per screen, e.g. a "Public" chip). `error` is **only** for errors/destructive confirmation. No gradients, no rainbow tag colours, no decorative colour. If a screen looks colourful, remove colour until it does not.
- **Seed colour:** `#36618E` (calm blue). Generate light and dark schemes with `@material/material-color-utilities`, commit the generated token file, and expose them as CSS custom properties (`--md-sys-color-*`). Support `prefers-color-scheme` with a manual override (system / light / dark).
- **Never hardcode** hex colours, border radii, spacing, font sizes, or durations in components. Use tokens only. CI has a script (`web/scripts/check-tokens.mjs`, Step 14) that fails the build on stray hex values or raw `px` radii outside the token files.

### 8.2 Tokens to define (in `web/src/design/tokens.css` + `tokens.ts`)
- **Colour roles:** primary, on-primary, primary-container, on-primary-container, secondary(+container), tertiary(+container), error(+container), surface, surface-dim, surface-bright, surface-container-lowest/low/(default)/high/highest, on-surface, on-surface-variant, outline, outline-variant, inverse-surface, inverse-on-surface, inverse-primary, scrim, shadow.
- **Type scale (Roboto Flex or Roboto; Roboto Mono for code):** display L/M/S, headline L/M/S, title L/M/S, body L/M/S, label L/M/S. Self-host fonts (no third-party CDN).
- **Shape scale:** extra-small 4px, small 8px, medium 12px, large 16px, extra-large 28px, full 9999px.
- **Elevation:** use tonal surface containers instead of heavy shadows; level 0–3 only.
- **State layers:** hover 8%, focus 10%, pressed 10%, dragged 16%; disabled content 38% / container 12%.
- **Motion:** emphasized easing `cubic-bezier(0.2, 0, 0, 1)`, standard `cubic-bezier(0.2, 0, 0, 1)` for enter / `cubic-bezier(0.3, 0, 1, 1)` for exit; durations 100–500 ms by token (short1…long4). Honour `prefers-reduced-motion`.
- **Spacing:** 4-point grid tokens (4, 8, 12, 16, 24, 32, 48).

### 8.3 Components and layout
- Use `@material/web` components where available (buttons, text fields, chips, menus, dialogs, tabs, switch, checkbox, progress, list, icon-button, navigation primitives); build missing pieces (top app bar, navigation rail/bar/drawer, cards, snackbar, search bar) as small in-house components that follow the M3 specs and use the same tokens.
- **Adaptive layout:** compact < 600 px (navigation bar bottom, single pane), medium 600–839 px (navigation rail), expanded ≥ 840 px (navigation rail or drawer + list-detail). Content max width ~1200 px.
- **Icons:** Material Symbols (outlined), self-hosted subset.
- **Code blocks:** surface-container-high background, Roboto Mono, shape-medium, copy button as icon-button, line numbers optional.
- **Accessibility:** WCAG 2.2 AA contrast, visible focus ring, full keyboard navigation, `aria-*` where native semantics are insufficient, 48 px minimum touch targets, no information by colour alone.
- **Empty / loading / error states** are designed for every list and page (skeletons or progress indicators, helpful empty-state text with one primary action).

### 8.4 CLI/TUI mapping (same semantics, terminal-sized)
- Colour is minimal: **one accent** (primary blue) for headings/selection, dim grey for secondary text, red for errors only, amber for warnings only, green for success only. Respect `NO_COLOR`, `--color=auto|always|never`, and non-TTY output (no colour, no spinners).
- Information is grouped in **rounded "surface" cards** (box-drawing with rounded corners) for the confirmation prompt, `show`, and `whoami`; plain aligned columns for lists.
- Unicode symbols with an ASCII fallback (`FORGEHUB_ASCII=1` or non-UTF-8 locale).
- Error messages follow: **what failed → why → how to fix** (one line each).

---

## 9. Verification gates (must be green before any commit)

Run from the repo root. Adapt names to what exists at the current step; once a gate exists it stays mandatory.

```bash
# Rust
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check            # from Step 19 onward (licences + advisories)
cargo audit                 # from Step 19 onward, if installed

# Web (from Step 14 onward)
cd web && npm run lint && npm run typecheck && npm test -- --run && npm run build && node scripts/check-tokens.mjs

# E2E (from Step 20 onward)
scripts/e2e.sh
```

If a tool is missing and cannot be installed (e.g. no network), say so in the report; do not skip silently and do not claim it passed.

**Performance budgets** (measured and noted in the report when the relevant step lands):
- CLI cold start for `fg --version` ≤ 20 ms; `fg <local-name>` to first prompt ≤ 50 ms; cloud fetch path dominated by network, with one request for a run (no chatty calls).
- CLI release binary ≤ 15 MB stripped (target; report if exceeded and why).
- Server: p95 < 50 ms for snippet read/list on a laptop-class machine with 10k snippets (SQLite).

---

## 10. Security rules (summary - full model in roadmap §Security)

1. **Execution safety is UX, not an afterthought.** Before running a cloud snippet the CLI always prints the full commands (or script), the owner, version, and short content hash, and asks `[y/N]` (default **No**). Third-party snippets require a stronger confirmation (type `yes`) the first time, and **a changed hash since last trusted run shows a diff first**.
2. Never auto-run on fetch. Never pipe a downloaded body straight into a shell without the confirmation screen. `-y` skips the prompt **only** for snippets owned by the logged-in user or already trusted at that exact hash.
3. Passwords: argon2id, unique salt, tuned parameters; never logged. Tokens: random 256-bit, stored **hashed** (SHA-256), shown once, scoped, revocable. Session cookies: `HttpOnly`, `Secure`, `SameSite=Lax`, rotating on login.
4. CSRF protection on all cookie-authenticated mutating requests (Origin check + token). CLI bearer tokens are not cookies and are exempt.
5. Strict input validation and size limits (snippet body ≤ 64 KiB, name/slug/username regexes, description ≤ 280 chars, ≤ 20 tags). Reject control characters except `\n`, `\t` in bodies where appropriate.
6. **Secret scanning** on `record` (client) and on every `push` (server): reject or redact tokens, private keys, passwords in command lines.
7. Rate limiting on signup, login, device-code, token, search, and push endpoints. Constant-time comparisons for secrets. Generic error messages for auth failures (no user enumeration).
8. Security headers + CSP (no inline scripts), `X-Content-Type-Options: nosniff`, `frame-ancestors 'none'`. All user content rendered as text (React escaping); never `dangerouslySetInnerHTML` with user data.
9. Raw endpoints return `text/plain; charset=utf-8` with `nosniff`.
10. Credentials file on disk: `0600`, directory `0700`; never print tokens in logs or error messages.
11. Dependency hygiene: `cargo deny`, `cargo audit`, `npm audit` (prod deps) reviewed each time dependencies change from Step 19; before that, note new deps in reports.

---

## 11. Coding standards

**Rust**
- `#![forbid(unsafe_code)]` in all crates unless an ADR justifies an exception.
- No `unwrap()`/`expect()` in non-test code except for provably infallible cases with a comment. Use `thiserror` for library errors, `anyhow` only in binaries' `main`.
- Public items documented. Modules small and cohesive. Errors carry context.
- Async only where needed (server, HTTP); the CLI's local paths stay synchronous and fast. Avoid heavy dependencies that hurt startup time.
- Deterministic tests; no network in unit tests; use temp dirs and in-memory SQLite.

**TypeScript / React**
- `strict` mode on, no `any` without a comment. Functional components, small hooks, TanStack Query for server state, React Router for routing.
- Components consume design tokens only (R4). Co-locate tests with components (Vitest + Testing Library). Accessibility tests with `jest-axe` or equivalent on key screens.

**API**
- Versioned under `/api/v1`. JSON in/out, consistent error envelope: `{ "error": { "code": "snippet_not_found", "message": "…", "details": {…} } }`. Pagination via `limit` + opaque `cursor`. OpenAPI document kept in `docs/openapi.yaml` and updated whenever endpoints change.

**Docs**
- Every user-visible behaviour is documented in `docs/` (CLI reference, user guide). ADRs for decisions. Keep `README.md` truthful about what works *now*.

---

## 12. Things you must not do

- Do not add file/attachment upload, binary storage, or anything that stores non-text user content.
- Do not add analytics, tracking pixels, or third-party scripts/fonts/CDNs to the web app.
- Do not build a sandbox and call it secure; this product runs real commands on the user's machine by explicit consent. Be honest in the UI and docs.
- Do not store plaintext passwords, tokens, or recovery secrets.
- Do not weaken a security rule to make a test pass.
- Do not leave `TODO` in committed code without a matching entry in the report's "Deferred" section.
- Do not create more commits than steps, or skip a report.
- Do not use colours outside the token system, and do not introduce a second accent colour.

---

## 13. Quick reference - daily commands

```bash
git status && git log --oneline -n 5
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo run -p forge-server            # API + static web on localhost
cargo run -p forge-cli -- --help
cd web && npm run dev
```

---

## 14. Definition of Done (per step)

- [ ] Every acceptance criterion in the roadmap for this step is verifiably met.
- [ ] All gates in §9 pass.
- [ ] New behaviour has tests, including failure/authorisation paths.
- [ ] Docs updated (spec/CLI reference/OpenAPI) where behaviour changed.
- [ ] `reports/report_Step<N>.md` written and complete.
- [ ] `PROGRESS.md` ticked.
- [ ] Exactly one commit `step <N>: …`, tag `step-<N>`, pushed, working tree clean.
- [ ] If the step is a 🛑 checkpoint: stopped and printed the checkpoint message.
