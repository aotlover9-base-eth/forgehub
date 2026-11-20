# Known gaps

Honest list of things ForgeHub deliberately does not do yet, so
nobody has to guess. Each gap names the step or condition that
closes it.

## Account recovery: password reset exists (2026-11-20); email is still unverified

**Password reset landed as a post-v0.1.0 increment
(2026-11-20).** `POST /api/v1/auth/password-reset/request`
mails a single-use link (token stored only as a SHA-256 hash,
one live link per account, one-hour expiry, throttled per IP
and per address, enumeration-safe 204 responses);
`POST /api/v1/auth/password-reset/confirm` sets the new
password and revokes every session and API token on the
account. Delivery is SMTP via `FORGEHUB_SMTP_URL`
(`FORGEHUB_MAIL_FROM` sets the sender). Two honest caveats:

- **Without SMTP configured, no mail leaves the server.** In
  production the request is accepted and logged as
  undeliverable; the operator fallback remains replacing the
  `password_hash` row directly. In dev (`FORGEHUB_ENV=dev`,
  no SMTP), reset links are written to the server log
  instead — the console mailer — so local testing needs no
  mail setup.
- **Email addresses are still not verified at signup.**
  They are format-checked only, so an account can be
  registered with an address the user does not control; a
  reset link for such an account goes to whoever controls
  that address, which is the correct behaviour, but signup
  itself proves nothing about ownership.

## Rate limiting is per-process (Step 6)

Login throttling and API rate limits live in the memory of one
server process. A multi-node deployment needs a shared store;
single-node deployments (the v1 target) are exact.

## Docker image build-verified 2026-11-19; CI builds and smokes it since 2026-11-20

The first real `docker compose up --build` on a user machine
(2026-11-19) built the image and started Caddy fine — and
caught a real bug: `docker-compose.yml` carried a literal
`<redacted>` placeholder as `FORGEHUB_DATABASE_URL` (a
redaction artifact introduced when the file was written in
Step 21; the development machine has no Docker, so nothing
had ever run it). The server therefore could not open its
database (SQLite code 14). Fixed the same day in a
post-v0.1.0 commit, together with the same placeholder in
`docs/deploy.md`. CI now has a `docker` job that builds the
image, boots it and demands a healthy response plus a
working signup, so this class of breakage fails the build
instead of reaching a user.

## Features deliberately absent in v0.1.0

- **Organisations and teams.** Snippets belong to individual
  users; there is no shared ownership, no org namespaces, no
  roles. Forking is the collaboration primitive.
- **OAuth / third-party login.** Username + password only; no
  "Sign in with …" providers.
- **Admin web UI.** Moderation is the operator CLI
  (`forge-server admin …`) by design — no privileged web
  surface exists to attack.
- **OS keyring storage for CLI credentials.** Tokens live in
  `~/.config/forgehub/credentials.toml` (directory 0700, file
  0600, reads refuse looser permissions). Keyring integration
  is a post-v1 hardening option.
- **Windows depth.** The CLI builds and runs on Windows for
  local library management, but executing snippets assumes a
  POSIX shell and the `fg` alias is Unix-shell only.
- **TUI cloud tabs paginate fully** (closed 2026-11-20).
  The terminal UI follows `next_cursor` through Mine and
  Explore (with a 50-page safety cap and a repeated-cursor
  stop), like `remote list` always did.

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
