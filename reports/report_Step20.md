# Step 20 Report — End-to-end tests and performance pass

## 1. Summary

The whole system was proven together, end to end, with real binaries and a real browser — and the performance pass found and fixed the one real bottleneck: search and list pages were hydrating every snippet with ~5 separate database queries (an N+1 pattern). After bulk hydration, every agent.md budget is met at the budgeted scale with wide margins.

## 2. End-to-end evidence

- **`scripts/e2e.sh`** (new, runs in CI as its own job): builds the real CLI + server, starts the server on an ephemeral port with a throwaway DB, and drives the full story — API signup → device-flow login with scripted approval → import → push → "web" edit via API PUT → status (`behind`) → diff → pull → `unchanged`; second user: token login → fork → **typed-`yes` run** → trusted run → changed-hash **decline (exit 2, nothing executed)** → accept after diff → admin hide (404) / unhide → token revoke (CLI exit 4) → server restart persistence → **DB backup/restore drill** → account delete (login then 401). Interactive confirmations run under a PTY helper (`scripts/ptyrun.py`) because the CLI correctly refuses piped stdin for third-party runs — the fail-safe itself was re-verified on the way (a piped `yes` is refused with a what/why/fix message). Result: **E2E PASS**.
- **Browser E2E** (`web/e2e/app.spec.ts`, Playwright + Chromium headless shell, `scripts/e2e-web.sh`, also in CI): signup → dashboard → create `ubuntu-setup` → view → edit → versions → select two → Compare → diff visible → Settings → Tokens → reveal-once flow (Copy unlocks "I've copied it") → device page unknown-code error. Result: **passed (11 s)**. Dashboard screenshots at the three width classes are committed in `docs/screenshots/` (34–65 KB each).
- **One-request cloud run**: a counting-middleware integration test proves `fg run` fetches its snippet with exactly **one** HTTP GET.
- **Pagination**: `remote list` and `status` now follow `next_cursor` past the first 100 items (integration test creates 105 snippets and counts them) — closing the Step 12 known gap.
- **Resilience** (`tests/resilience.rs`): with the database write-locked by a second connection, reads keep working (WAL), writes fail after the busy timeout as a generic `500 {"error":{"code":"internal_error","message":"Internal server error"}}` (no storage internals leaked), and the same write succeeds once the lock clears. Server-restart persistence is covered by `e2e.sh`.

## 3. Performance (release build, this shared-CPU VM)

Server, seeded 10,000 public snippets (the budgeted scale):

| Endpoint | p50 | p95 | Budget |
|---|---|---|---|
| Snippet read | 0.94 ms | **2.89 ms** | p95 < 50 ms ✓ |
| Search `q=docker` (matches everything) | 16.80 ms | **36.75 ms** | p95 < 50 ms ✓ |
| Explore list | 2.34 ms | **3.89 ms** | p95 < 50 ms ✓ |
| Snippet create (push) | 3.07 ms | 5.37 ms | — |
| Conditional read | — | 304 on `If-None-Match` ✓ | — |

Before the fix, the same run measured search p95 **82.66 ms** and explore p95 **40.72 ms**. The fix: new bulk repository reads (`latest_versions`, `tags_for`, `starred_among`, `by_ids` for snippets and users) and a `HydratedPage` renderer in discovery, plus a parts-based `snippet_json_parts` — a page of 20 now costs ~6 queries instead of ~120. All discovery/snippets behaviour tests pass unchanged.

Scale check, 10,000 users + 50,000 snippets (bulk-SQL seed): read p50 0.93 / p95 **1.56** / p99 2.03 ms; search (all-matching term) p95 94.38 ms — over 50 ms at 5× the budgeted scale, because FTS5 bm25 must score every match; recorded in `docs/known-gaps.md` with the follow-up options.

CLI (release): `--version` mean **4.23 ms** (budget 20), local `run --dry-run` mean **9.29 ms** / p95 18.49 (budget 50), binary **9,490,808 bytes ≈ 9.1 MiB** (budget 15 MB).

Web bundle: shell chunk 687.30 → **670.92 kB** (gzip 176.53) after removing four dead `@material/web` registrations (divider, both progress indicators, secondary-tab — registered but never used); route chunks 2.4–11.9 kB. The remainder is the design system the shell genuinely uses; see the updated known-gaps entry.

## 4. Backup/restore

`scripts/backup-db.sh` (SQLite online backup API via python3; integrity-checked, cron-friendly) + `docs/operations.md` (backup, restore drill, abuse/retention ops, failure behaviour). The drill is not just documented — `e2e.sh` performs it on every run (live backup → replace DB → restart → data served).

## 5. Tests and gates (real output)

- `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` — **0 errors**.
- `cargo test --workspace` — **411 passed, 0 failed** (Step 19: 408; +3: pagination, one-request run, DB-lock resilience).
- Perf suite (ignored by default): both scenarios pass in release, including the budget assertions.
- Web: lint clean, typecheck clean, **45 tests passed**, build succeeds; Playwright suite green.
- `scripts/e2e.sh` — PASS.

## 6. Corrections to earlier steps

- Step 10/17 performance debt corrected: the per-snippet hydration in search/explore/profile/star lists (introduced across Steps 10 and 17) is replaced by bulk hydration; response shapes are byte-identical (same JSON builders).
- Step 12 known gap closed: CLI list pagination (see §2). `docs/known-gaps.md` updated (pagination entry removed; bundle + 50k-search entries rewritten with measured facts).

## 7. Checkpoint review (CHECKPOINT 4 — per the run override, no stop)

**What to test manually (short script)**
1. Run `bash scripts/e2e.sh` and `bash scripts/e2e-web.sh` — both should print PASS.
2. Start the server (`FORGEHUB_WEB_DIR=web/dist cargo run -p forge-server`), open it in a browser, sign up, create a public snippet, and open it in a private window — the safety banner and Run-it card should show.
3. From a second terminal profile: `fg login`, `fg run <you>/<snippet>` — type `yes`, run it; edit the snippet on the web, run again — you should get the diff + re-confirmation.
4. `fg trust list`, star/fork from the web UI, and try approving a device code at `/device` from the CLI login flow.

**What is solid**
- The trust/confirmation model end to end (CLI + API + browser all exercised against the real server, including the decline path and the non-TTY fail-safe).
- Performance at and beyond the budgeted scale for reads/lists; search inside budget at 10k.
- Security posture (Step 19 verification + this step's locked-DB and one-request evidence); supply-chain gates green in CI (Step 19 CI run, including `npm audit`, was fully green).

**What is risky**
- Search latency grows linearly with match count (94 ms p95 at 50k with an all-matching term) — fine for v1 scale, the first thing to revisit if a public instance grows.
- The Dockerfile has still never been built (no Docker on this machine); Step 21 must treat it as unverified until the release pipeline exercises it.
- Interactive PTY paths are covered by `e2e.sh`'s PTY helper for the main flows; exotic terminal sizes/terminfo quirks in the TUI remain only state-machine-tested.

**What I would fix first**
1. Exercise the Dockerfile + write the release pipeline (Step 21's job — it is the last unverified surface).
2. A real-browser pass on a *physical* phone width for the device-approval flow (the Playwright pass covers layout at 400 px, not touch behaviour).
3. Search: add the 50k all-match case to the perf watch list before any public launch.

## 8. Next step

Step 21 — packaging, installer, release pipeline **preparation only** (per the run limits: no publishing, no deploying, no paid accounts, no GitHub Release without asking).
