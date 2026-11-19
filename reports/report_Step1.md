# Report — Step 1: bootstrap monorepo, study forgenesis, CI skeleton

- **Date/time:** 2026-11-19T09:31:50+05:30
- **Commit:** this report is part of the Step 1 commit — see `git log --oneline -n 1` (tag `step-1`)
- **Status:** ⚠️ Done with caveats — everything that can be done locally is done and gated; the private GitHub repo creation and `git push` are blocked on GitHub authentication / a remote URL, which only the user can provide (see §9).

## 1. Summary
A clean ForgeHub monorepo now exists at `~/workspace/forgehub` on branch `main`: a Cargo workspace with `forge-core`, `forge-cli` (binary `forgehub`) and `forge-server` skeleton crates that compile, lint and test green; a Vite + React + TypeScript (strict) web app that renders "ForgeHub" and passes lint, typecheck, test and build; CI, configuration and documentation scaffolding; a full written analysis of Forgenesis with a port/redesign/drop plan and a command mapping; and ADR 0001 recording that the `forgehub` name is free on crates.io and npm and available under the author's GitHub account. The Rust toolchain was missing from this machine and was installed via rustup. The one acceptance item not met is the pushed private remote, because `gh` is not authenticated here.

## 2. What I built
- **Tool verification:** git 2.43.0; `gh` 2.102.0 (present, **not authenticated** — `gh auth status`: "You are not logged into any GitHub hosts"); node v24.22.1; npm 10.9.4; rustc/cargo **were missing** and were installed with rustup (stable, minimal + rustfmt + clippy): rustc 1.100.0, cargo 1.100.0.
- **Repo:** `git init -b main` in `~/workspace/forgehub`; `agent.md` and `roadmap.md` copied to the root from the user's uploaded files. Local (repo-only) git identity set from the Forgenesis history: `Aotlover9 <nikhilbhaiop77@gmail.com>` — no global config was changed.
- **Cargo workspace:** root `Cargo.toml` (resolver 2, workspace package 0.1.0 / edition 2021 / MIT, workspace clippy lints, release profile LTO + `codegen-units=1` + strip), `Cargo.lock`, `rust-toolchain.toml` (stable + rustfmt + clippy), `rustfmt.toml`.
- **Crates (each `#![forbid(unsafe_code)]`, each with a real test):** `crates/forge-core/src/lib.rs` (product/binary name constants + doctest), `crates/forge-cli/src/main.rs` (binary `forgehub`; `--version`/`--help`, unknown commands fail with what/why/fix and exit 2), `crates/forge-server/src/main.rs` (same skeleton shape). No feature code — Step 1 specifies a trivial skeleton only.
- **Web:** created with `npm create vite@latest web -- --template react-ts`; `src/App.tsx` renders the ForgeHub placeholder, `src/App.test.tsx` tests it; Vite dev proxy for `/api`; ESLint flat config, Prettier, Vitest + Testing Library + jsdom configured. Dependencies added: `@material/web`, `@tanstack/react-query`, `react-router-dom`. No design system yet (Step 14, per the roadmap).
- **Repo files:** `.gitignore` (Rust, Node, env, `*.db`, build output), `.editorconfig`, `.env.example` (secret-free), `LICENSE` (MIT, Copyright (c) 2026 Aotlover9 — found in the Forgenesis `LICENSE` and its git history; local git config was unset), `README.md` (honest: work in progress, Step 1 only), `PROGRESS.md` (all 22 steps, Step 1 ticked), `deny.toml` (draft), `docs/README.md`, `scripts/README.md`.
- **CI:** `.github/workflows/ci.yml` — Rust job (fmt, clippy `-D warnings`, `cargo test --workspace`, cargo cache) and Web job (node 20, `npm ci --legacy-peer-deps`, lint, typecheck, test, build). YAML validated by parsing (see §4). It has not run on GitHub because there is no remote yet.
- **Forgenesis study:** cloned https://github.com/aotlover9-base-eth/forgenesis to `/tmp/forgenesis-ref` (outside the repo), read its README (491 lines), `Cargo.toml`, and all 16 `src/` modules. `docs/forgenesis-analysis.md` contains the module map with line counts, the YAML model, port-as-is / redesign / drop tables with reasons, and the full Forgenesis → ForgeHub command mapping.
- **Name check:** live crates.io, npm and GitHub API queries (results in ADR 0001 and §3).

## 3. Decisions made
- **Keep the name ForgeHub / binary `forgehub` / alias `fg`** — ADR `docs/adr/0001-name-and-binary.md`. crates.io `forgehub` and `forge-hub` both return "crate does not exist"; npm `forgehub` and `forge-hub` both return "Not found"; `aotlover9-base-eth/forgehub` returns 404 (available). 94 unrelated GitHub repos use the name under other owners, which does not block a per-owner repository name. Alternative (rename) rejected as churn with no blocking collision.
- **TypeScript pinned to `~5.9.3` instead of the template's `~7.0.2`** — `typescript-eslint` explicitly refuses TS 7.0 ("typescript-eslint does not support TS 7.0"), so the mandated ESLint gate could not run at all on TS 7. Two approaches were tried: (1) `--legacy-peer-deps` install on TS 7 — install succeeded but `npm run lint` crashed at startup; (2) downgrade to TS 5.9.3 — lint and typecheck both pass. TS 5.9 is the boring, supported choice; revisit when typescript-eslint supports TS 7.
- **`@testing-library/dom` added as an explicit devDependency** — with `--legacy-peer-deps`, the peer was not installed and `@testing-library/react` had no `screen` export under typecheck.
- **`npm ci --legacy-peer-deps` in CI and in the README build instructions** — required by the `typescript-eslint` peer range against the template's other tooling; recorded here rather than hidden.
- **Workspace clippy pedantic lints are on** (`[workspace.lints.clippy]`) and the skeleton was fixed to satisfy them under `-D warnings` (doc backticks, `assert_ne!` for non-empty checks) rather than weakening the lints.
- **Git identity for this repo only** taken from the Forgenesis commit history (same author), because this machine's git config was empty. If the user wants a different name/email, it is one `git config` change before the first push.
- **Forgenesis `sync`, packs (`install`/`plugins`) and `config` subcommand are recorded as dropped for v1** in the analysis, with reasons (cloud push/pull and explore/fork are the v1 sharing model; Appendix D has no `config` command). This is exactly the documented-reason case Step 5's acceptance criteria allow.

## 4. Verification
| Gate | Command | Result |
|------|---------|--------|
| Format | `cargo fmt --all --check` | pass |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| Tests | `cargo test --workspace` | 4 passed, 0 failed (3 unit + 1 doctest) |
| Web lint | `cd web && npm run lint` | pass |
| Web typecheck | `cd web && npm run typecheck` | pass (`strict: true` set in `tsconfig.app.json`) |
| Web tests | `cd web && npm test -- --run` | 1 passed, 0 failed |
| Web build | `cd web && npm run build` | pass — `dist/index.html` 0.45 kB, JS 219.67 kB (gzip 68.67 kB) |
| CI syntax | `python3 -c "yaml.safe_load(open('.github/workflows/ci.yml'))"` | pass (`CI_YAML_OK`) |
| Manual | `cargo run -p forge-cli -- --version` | `forgehub 0.1.0` |
| Manual | `cargo run -p forge-server -- --version` | `forge-server 0.1.0` |
| Manual | name checks | crates.io / npm: names free; GitHub author repo: 404 (available) — §3 |

Trimmed test output:
```text
running 1 test  test tests::version_string_is_not_empty ... ok            (forge-cli)
running 1 test  test tests::product_and_binary_names_are_stable ... ok    (forge-core)
running 1 test  test tests::version_string_is_not_empty ... ok            (forge-server)
running 1 test  test crates/forge-core/src/lib.rs - (line 11) ... ok      (doctest)
```

## 5. Acceptance criteria
- [ ] Private repo exists with `origin` set; `main` pushed. — **NOT MET, blocked:** `gh` is present but not authenticated on this machine, and no remote URL has been supplied. Per the Step 1 prompt ("otherwise ask me for the remote URL once"), the user must either authenticate `gh` or provide the remote URL. The local commit and tag `step-1` are ready to push the moment a remote exists.
- [x] Workspace and web app build; CI file present and syntactically valid.
- [x] `docs/forgenesis-analysis.md` lists modules, reuse plan, and command mapping.
- [x] ADR 0001 records the name decision.
- [x] `PROGRESS.md` lists all 22 steps.

## 6. Security notes
- No secrets, `.env` files, tokens or credentials are in the repo; `.env.example` contains only a clearly-marked placeholder and `.gitignore` excludes `.env*` (except the example), keys and databases. The staged diff was eyeballed before committing (the `scripts/scan-staged.sh` scanner arrives in Step 3, per agent.md §5.7).
- `#![forbid(unsafe_code)]` in all three crates.
- No file-upload surface, no telemetry, no third-party web scripts/fonts were introduced. The web placeholder uses a system font stack only; the Material 3 self-hosted fonts/tokens arrive in Step 14.
- The CLI/server skeletons execute nothing and open no sockets.

## 7. Dependencies added
- Rust: **none** in Step 1 (workspace crates depend only on `forge-core` by path) — deliberately, so the skeleton's gates prove the toolchain, not a dependency tree.
- Web runtime: `@material/web` (^2.5.0, Apache-2.0 — official Material 3 web components, required by the locked stack), `@tanstack/react-query` (^5.108.0, MIT — server state, locked stack), `react-router-dom` (^7.18.6, MIT — routing, locked stack), `react`/`react-dom` ^19.3.1 (MIT, Vite template).
- Web dev: `vitest` ^5.1.1 (MIT), `@testing-library/react` ^16.3.3 / `@testing-library/dom` / `@testing-library/jest-dom` ^8.0.0 / `@testing-library/user-event` (MIT), `jsdom` ^31.0.1 (MIT), `eslint` ^10.15.0 (MIT) + `@eslint/js`, `typescript-eslint` ^8.73.0 (MIT), `eslint-plugin-react-hooks`, `eslint-plugin-react-refresh`, `prettier` ^3.10.1 (MIT), `typescript` ~5.9.3 (Apache-2.0, pinned — see §3), `vite` ^8.3.6 (MIT), `oxlint` (MIT, from the Vite template; ESLint is the configured `lint` script).

## 8. Corrections to earlier steps
None — this is Step 1.

## 9. Known issues / limitations
- **No remote, no push.** `origin` is unset and `main` is not pushed, solely because GitHub authentication is unavailable in this environment. This also means CI has never run on GitHub and the "private repo exists" acceptance item is open.
- CI action versions (`actions/checkout@v4`, `actions/setup-node@v4`, `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`) are tag-pinned, not SHA-pinned; SHA pinning is a Step 19 hardening item per the roadmap.
- `cargo deny check` / `cargo audit` are not run yet — agent.md §9 makes them mandatory from Step 19; `deny.toml` is a draft until then.
- The Vite template's `web/.oxlintrc.json` and `web/README.md` remain from scaffolding; oxlint is not wired into any script.

## 10. Deferred / ideas
- Re-check crates.io availability of `forgehub` immediately before any future crate publication (Step 21).
- Decide in Step 2 whether the dropped Forgenesis `config` subcommand should return as a thin `config.toml` editor (analysis §5).

## 11. Next step
Step 2 turns roadmap Appendices A–E into `docs/spec-snippet-format.md`, `docs/spec-data-model.md`, `docs/openapi.yaml`, `docs/spec-cli.md`, `docs/threat-model.md` and `docs/design-spec.md`, plus ADRs (SQLite vs Postgres, script vs steps, script execution mechanism, cookie vs token, hash/pinning, secret scanning). It needs from Step 1: the Forgenesis YAML model and compatibility facts in `docs/forgenesis-analysis.md` §2, the name/binary decision in ADR 0001, and a remote — Step 2's commit cannot be pushed either until the Step 1 remote blocker is resolved.
