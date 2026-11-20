# Changelog

All notable changes to ForgeHub are documented here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.1] - 2026-11-20

A polish-and-recovery release: the Observe redesign, the
most-requested missing account feature, and a batch of fixes
found by real use on a real machine.

### Added

- **Password reset**: emailed single-use links (hashed
  tokens, one live link per account, one-hour expiry,
  enumeration-safe responses); a successful reset revokes
  every session and API token. Delivery via SMTP
  (`FORGEHUB_SMTP_URL`); dev servers log the link instead.
- **In-app Home page** (`/home`): greeting, recent snippets,
  popular-this-week and a run-anywhere card — separate from
  the public landing page, which the nav's Home used to
  bounce signed-in users to. Login and signup land there.
- **OS keyring credential storage** for the CLI
  (`FORGEHUB_CREDENTIAL_STORE=auto|file|keyring`): Keychain
  on macOS, Credential Manager on Windows, secret-service
  on Linux via the opt-in `linux-keyring` build feature;
  the credentials file remains the fallback everywhere.
- **Demo data pack**: 105 ready-to-import snippets
  (`demo-data/`) with an idempotent loader script.
- **CI**: a Docker job that builds the image and smoke-tests
  the boot (health + a real signup), closing the gap that
  let the v0.1.0 compose bug ship.

### Changed

- **Observe theme**: the landing page and the whole
  signed-in app are now a dark, monochrome glass design
  (Instrument Serif display type, translucent panels over a
  smoke backdrop) replacing the Material You 3 light/dark
  schemes; the app is dark-only and the theme toggle is
  gone. Page headings use the serif inside the app too.
- **Motion**: landing entrance choreography, per-navigation
  page transitions, card hover lift and button press
  feedback — all silenced by `prefers-reduced-motion`.
- **TUI**: the Mine and Explore cloud tabs paginate through
  the full list instead of showing only the first page.
- README rewritten table-first, with screenshots of the
  themed app.

### Fixed

- `docker-compose.yml` shipped a literal placeholder as
  `FORGEHUB_DATABASE_URL`, crash-looping the server on
  first real use (SQLite code 14); the same placeholder in
  `docs/deploy.md` and a missing `FORGEHUB_DOMAIN` in
  `.env.example` are fixed too.
- Typing in web form fields could fire the shell's global
  keyboard shortcuts (an email starting with "n" bounced
  signup to the login page); detection now uses the
  composed event path.
- The landing page redirected signed-in users to the
  Library and the nav lit Home on unrelated pages; both
  navigation bugs are fixed.
- The CLI now trusts the OS certificate store (rustls
  native roots), so `fg login` works against private/local
  CAs such as Caddy's on `https://localhost`.
- Mobile layout: the landing nav no longer overflows at
  phone widths, and the compact bottom bar is pinned to
  the viewport instead of sinking below long pages.

## [0.1.0] - 2026-11-19

The first release: a cloud home for shell command snippets —
web app, server, and the `forgehub` CLI (alias `fg`).

### Added

- **CLI (local)**: record/run/list/show/search/tag/rename/delete/
  edit snippets, parameterize, clone, import/export (including
  Forgenesis import), analyze shell history into snippets, run
  stats, backup/restore, doctor, shell alias (`fg`) and dynamic
  completions, an interactive TUI (`fg ui`), an LRU run cache,
  and `fg update --check`.
- **CLI (cloud)**: device-code login (RFC 8628 style), scoped
  API tokens, push/pull with optimistic concurrency, status and
  diff against the server, fork, remote list/show/delete,
  visibility, search, star — and running cloud snippets with
  the trust model: full command display, typed `yes` for other
  people's snippets, hash-pinned trust, changed-hash diff and
  re-confirmation, fail-safe without a terminal.
- **Server**: axum + SQLite (WAL, FTS5) API implementing
  `docs/openapi.yaml` — accounts with argon2id passwords and
  hashed sessions, API tokens and device login, snippet CRUD
  with immutable versions, full-text search, explore feeds,
  profiles, stars, forks, reports, and an operator admin CLI.
- **Web**: Material You 3 app (seed `#36618E`) — auth, dashboard,
  snippet editor with live validation and secret blocking,
  snippet pages with version diffing, explore/search with match
  highlighting, profiles, settings (profile, security, sessions,
  tokens), and the device-approval page.
- **Safety & supply chain**: secret scanning at record/push/
  save, terminal-escape neutralisation at every display site,
  CSP/security headers, per-IP/account throttling, `cargo deny`
  + `cargo audit` + `npm audit` in CI, actions pinned by SHA.
- **Packaging**: multi-stage Dockerfile, docker-compose with
  Caddy, checksum-verifying installer, release workflow building
  Linux/macOS/Windows binaries with SHA-256 checksums and build
  provenance attestations.

[Unreleased]: https://github.com/aotlover9-base-eth/forgehub/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/aotlover9-base-eth/forgehub/releases/tag/v0.1.0
