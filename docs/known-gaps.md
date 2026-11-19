# Known gaps

Honest list of things ForgeHub deliberately does not do yet, so
nobody has to guess. Each gap names the step or condition that
closes it.

## Account recovery (Step 7)

**There is no password reset and no email verification.** The
server sends no email at all in v1 — there is no SMTP or provider
integration — so:

- If a user forgets their password, they cannot recover the
  account self-service. The only recovery path is an operator
  with database access replacing the `password_hash` for the
  user row (argon2id PHC format) and deleting the user's rows
  from `sessions`.
- Email addresses are format-checked at signup but never
  verified, so an account can be registered with an address the
  user does not control. Nothing in v1 sends mail to the address,
  so the impact is limited to the account itself.

Closing this gap needs a mail-sending decision (provider or SMTP)
and a reset-token flow; it is recorded here rather than silently
absent, and the threat model's account-takeover row (T-ACCT-1)
should be re-read when it lands.

## Rate limiting is per-process (Step 6)

Login throttling and API rate limits live in the memory of one
server process. A multi-node deployment needs a shared store;
single-node deployments (the v1 target) are exact.

## Docker image not built in CI yet (Step 6)

The `Dockerfile` is maintained with the code but was not
build-tested on the dev machine (no Docker available). Step 21's
release pipeline is the latest point where it must be exercised.

## Unmaintained-but-sound dependencies (Step 19)

`cargo deny check` is green with three documented advisory
ignores in `deny.toml`, all "unmaintained" (not vulnerability)
advisories:

- **chrono** (RUSTSEC-2026-0543) — used only to format/parse
  UTC timestamps. Post-v1: migrate to `time` or `jiff` and drop
  the ignore.
- **unsafe-libyaml via serde_yaml 0.9** (RUSTSEC-2024-0449) —
  the YAML stack implements the canonical snippet format; it
  only ever parses size-capped (≤ 64 KiB), pre-scanned input
  (anchors/aliases/tags rejected, depth ≤ 16). Post-v1:
  evaluate a maintained YAML implementation against the
  byte-identical hash contract before switching.
- **paste** (RUSTSEC-2024-0436) — a compile-time proc-macro
  pulled in by ratatui; no runtime exposure. Resolves itself
  when ratatui drops it.

## Web bundle size (Step 17)

The web shell chunk is ~671 kB (177 kB gzip) after Step 20
removed four dead `@material/web` registrations. The rest is
the design system itself (buttons, fields, menus, dialogs are
used by the shell and nearly every page) plus React, the
router and TanStack Query; route chunks are 2–12 kB. Splitting
element registration per route could shave more, at real
complexity cost; it is a post-v1 optimisation, not a defect.

## Search latency at very large scale (Step 20)

At the budgeted scale (10k snippets) search p95 is ~37 ms,
inside the 50 ms budget. At 50k snippets with a term that
matches *every* snippet, p95 rises to ~94 ms: FTS5 must score
every match for bm25 ranking, and that scan is linear in the
number of matches. Reads and lists stay at ~2–4 ms p95 at
50k. If a deployment grows past ~50k public snippets, the
fixes are a covering index on the FTS auxiliary data or
precomputed popularity ordering; neither is needed at v1
scale.
