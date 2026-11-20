# Step 22 Report - Docs, final audit, v0.1.0

## 1. Summary

ForgeHub v0.1.0 is finished: documentation rewritten to match the shipped behaviour (every load-bearing claim verified against the code or by running it), a full final audit executed from a clean clone, and the release candidate tagged. Per the run limits, **no GitHub Release was published and nothing was deployed** - §5 gives the exact commands for the user to do that deliberately.

## 2. Documentation

- **README.md** rewritten: what it is, the `fg ubuntu-setup` story, 60-second quickstarts (CLI + web), install paths, the safety model in plain words, the dashboard screenshot, doc links. Claims verified while writing: the record flow is a live capture shell (spec-cli §table), the alias rationale is the shell-builtin collision (FAQ), quickstart commands are the real ones.
- **docs/user-guide.md** - web (accounts, editor, versions, discovery, tokens, device login, sessions) and CLI (local library, cloud, the trust matrix, offline behaviour), with two inaccuracies caught and fixed during verification (`remote search` is the real command; the device poll endpoint and the raw route were checked against the router - `api.md` initially placed `/raw` outside `/api/v1`; it is inside, and the doc now says so).
- **docs/cli-reference.md** - **generated from the binary's own `--help`** for all 34 commands and their subcommands (986 lines), so it cannot drift from clap without a regeneration.
- **docs/api.md** - orientation over the normative `openapi.yaml`: auth model, error envelope and stable codes, If-Match/ETag semantics, cursor pagination, endpoint map.
- **docs/self-hosting.md** (entry point over deploy/operations), **docs/forgenesis-migration.md**, **docs/faq.md** (safety, stopping a run, why the alias, password reality, offline, telemetry, Windows), **CONTRIBUTING.md** (the project's ground rules as contributor gates), **CODE_OF_CONDUCT.md**.
- **docs/known-gaps.md** final pass: the Docker entry now states plainly that the image was *never* build-verified (Step 21 came and went without Docker; the release pipeline does not exercise it either), and a new section lists the deliberate v0.1.0 absences - organisations, OAuth, admin web UI, OS keyring, Windows depth, TUI single-page cloud tabs.

## 3. Final audit checklist (evidence in this report and the step's runs)

| Check | Result | Evidence |
|---|---|---|
| Clean clone builds; all gates pass | **PASS** | Fresh clone + this step's tree copied in: `cargo fmt --check` rc 0; clippy `-D warnings` 0 errors; `cargo test --workspace` **411 passed / 0 failed**; web lint (incl. M3 token check) clean, typecheck clean, vitest 45 passed, production build OK |
| E2E from the clean clone | **PASS** | `scripts/e2e.sh` - "E2E PASS: full story green" against the clone's own binaries |
| 22 commits, one per step, in order | **PASS** | `git log` - steps 1–21 present at audit start; this commit is step 22 |
| Tags `step-1`…`step-22` + `v0.1.0` | **PASS** | `step-1`…`step-21` verified in the clone; `step-22` and annotated `v0.1.0` created with this step and pushed |
| 22 reports complete | **PASS** | `reports/report_Step1..22.md` (Step 21's report is committed inside this step's commit - see §4) |
| No secrets in history | **PASS** | Pattern scan over all 21 pre-step revisions: every hit is the secret scanner's own synthetic test corpus, its regexes, or ADR 0007's prose; `.env` appears in zero commits; `scripts/scan-staged.sh` clean on this commit |
| No stray TODOs / stubs | **PASS** | Tree scan: no `TODO`/`FIXME`/`todo!()`/`unimplemented!()` in source (matches were mktemp templates and a test device code) |
| Dependency licences reviewed | **PASS** | `cargo deny check` - advisories ok, bans ok, **licenses ok**, sources ok; three unmaintained-only ignores documented in `deny.toml` + known-gaps |
| M3 token check | **PASS** | `check-tokens: clean` in lint (clone + CI) |
| Accessibility checks | **PASS** | Token contrast suite + axe (0 violations) in web tests; Lighthouse accessibility **100** (Step 21) |
| Performance budgets | **PASS** | Step 20 tables: read p95 2.89 ms, search p95 36.75 ms @10k; CLI `--version` 4.23 ms, local run 9.29 ms, binary 9.1 MiB (musl static 9.6 MB, Step 21) |
| CI green on the final commit | **See §4** | Steps 20/21 CI had two real failures (below); both fixes are in this commit and the final CI result is verified by head SHA after push |

## 4. Corrections to earlier steps

- **Step 20's CI Web job failed** (found during this step's CI verification, which is now always by head SHA): vitest's default include pattern collected the Playwright spec `web/e2e/app.spec.ts` and failed the file ("Playwright Test did not expect test() to be called here") while all 45 tests passed - my Step 20 gate reading took the test count and missed the file-level failure and exit code. Fix: vitest `include` is now scoped to `src/**`. Verified locally (7 files, 45 tests, clean exit) and in the clean clone.
- **Step 21's CI supply-chain job failed** on the `rustsec/audit-check` action's own check-run update call (`404 … checks/runs#update-a-check-run`) *after* the audit printed "No vulnerabilities were found" (0 vulnerabilities; the 5 warnings are the documented informational ones). Fix: the job now installs `cargo-audit` via the pinned `taiki-e/install-action` and runs the binary directly - its exit code is the gate; the job's `checks: write` permission was removed as no longer needed.
- **Step 21's report file** missed its own commit (a write/add race in one tool block) and is committed here, unchanged.

## 5. Release commands (for the user - nothing published by this step)

Pushing the `v0.1.0` tag (done in this step) triggers the release workflow, which builds all five targets and uploads the assets as a workflow artifact named `release-assets-v0.1.0` - it does **not** create a Release. To publish:

```sh
# 1. Wait for the "Release binaries" workflow for tag v0.1.0 to go green:
gh run list --workflow release.yml --limit 1
# 2. Download the built assets:
gh run download --name release-assets-v0.1.0 --dir /tmp/fh-release
# 3. Create the Release and upload the assets + checksums:
gh release create v0.1.0 --title "ForgeHub v0.1.0" --notes-file CHANGELOG.md \
  /tmp/fh-release/forgehub-v0.1.0-*.tar.gz /tmp/fh-release/forgehub-v0.1.0-*.zip \
  /tmp/fh-release/SHA256SUMS
# 4. Verify the page:
gh release view v0.1.0 --web
```

After that, `scripts/install.sh` works against the real release (its `latest` resolution reads the release tag), and the `packaging/` templates can be filled from the published `SHA256SUMS`.

## 6. Project retrospective

**What went well.** The spec-first order (Steps 1–2) paid for itself at every later step - the CLI/server hash contract, the trust matrix and the token rules were decided once and never relitigated. The gates discipline caught real defects while they were cheap: a live SQLite CVE (Step 19), an N+1 that tripled search latency (Step 20), terminal-escape holes in three display paths (Step 19), and a release-workflow action SHA that did not exist (Step 21, caught by verification against the API). Testing crossed boundaries early - the CLI integration rig drove the *real* server from Step 11 on, which is why the Step 20 E2E story passed on its first full run.

**Risks that remain.** (1) The Docker image has never been built - the largest unverified surface left. (2) No email system means no password recovery; a public instance will feel this first. (3) Search latency is linear in match count past ~50k public snippets. (4) Two CI failures (Steps 20/21) were only caught by head-SHA verification in the *following* step - the process lesson is recorded, but a red main can sit unnoticed between steps.

**Next 10 ideas, ordered by value** (from roadmap §10, re-ranked):
1. Build-verify the Docker image in CI (closes the top risk).
2. Email verification + password reset (unlocks public instances).
3. Snippet signing + `fg verify` (extends the trust model to provenance).
4. Opt-in sandboxed run (container/bubblewrap) for foreign snippets.
5. OS keyring for CLI credentials.
6. OAuth login (GitHub first).
7. Organisations and shared team libraries.
8. Typed parameter prompts (choices, never-stored secrets).
9. PostgreSQL backend behind the existing repo traits.
10. CLI self-update (checksum-verified, using the release pipeline).

## 7. Step status

All 22 steps complete. `PROGRESS.md` is fully ticked; tags `step-1`…`step-22` and `v0.1.0` are pushed.
