# Changelog

All notable changes to ForgeHub are documented here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- **Collections (Peak C1).** Group snippets into an
  ordered runbook and run it end to end with
  `fg run owner/name` - every step still confirms first.
  `fg collection` manages them; the web app has a
  Collections section and a collection page. A collection
  never widens access: items you cannot read stay hidden.
- **Typed snippet parameters (Peak C2).** `variables:`
  entries can declare `type:` - text, choice, number,
  path, yesno or secret - plus choices, defaults and
  ranges. Values are checked before anything runs;
  secret parameters are prompted hidden, never stored,
  and never take a default.
- **Version history and rollback (Peak C3).** `fg history`
  shows every version; `fg rollback` publishes an old
  version's content as a NEW version (history is never
  rewritten). The web snippet page has the same rollback.
- **Public snippet pages (Peak C4).** Every public
  snippet has a shareable read-only page at
  `/s/owner/name` that works without JavaScript.
- **Import helpers (Peak C5).** The Library imports from
  a public GitHub Gist (files become private snippets;
  anything that looks like a secret is skipped with a
  reason) and from an uploaded shell history file, mined
  with the same pipeline as `fg analyze` - secrets are
  filtered on the server before anything comes back.
  CLI: `fg import --from-gist <url>`.
- **Shared collection links (Peak C6).** Owners can mint
  expiring (1/7/30 days), revocable read-only links to a
  collection (`fg collection share`). Anyone with the
  link can read the current members - including private
  snippets - without an account; running still needs an
  account and access. The full link is shown once.
- **Activity log (Peak C7).** Sign-ins (new devices
  flagged), snippet creates/pushes/forks, token changes
  and trust grants, in Settings - Security, in
  `fg activity`, and exportable as JSON. Only the owner
  ever sees it.
- **Your library stats (Peak C8).** The Library page
  shows your own cloud runs: totals, commands automated,
  estimated time saved, top workflows. `fg stats` stays
  local-only, as documented.
- **`forgehub self-check` (Peak C10).** Prints the
  binary's SHA-256 and version, fetches the published
  release SHA256SUMS when online, and verifies a
  downloaded release archive end to end with
  `--archive` (offline against `--sums`). Offline, it
  explains the manual `gh` + `sha256sum -c` flow.

### Changed

- **Web UI rebuilt as faithful Material You 3 (Peak
  Phase A).** Light, dark and system themes with a real
  switch, Roboto Flex throughout, token-only colours,
  responsive navigation, and a calmer Library with bulk
  actions and previews. The previous dark glass theme is
  gone.
- **CLI output and help (Peak Phase B).** One output
  design system (colour that honours NO_COLOR, consistent
  errors with what/why/fix), grouped help, dynamic shell
  completions, `fg find`, `fg new`, `fg lint`,
  `fg config`, multi-snippet push/pull/status, and
  network resilience (timeouts, retries, a fast-failing
  `--offline` mode).
- **Internationalisation groundwork (Peak C9).** All web
  UI strings live in one structured message catalogue
  (English only for now), ready for translations; RTL
  layout is covered by a dedicated test page.

### Removed

- **Windows support.** The `x86_64-pc-windows-msvc` release
  leg, the Windows zip and the bare `.exe` are gone, and
  the Windows assets were removed from the v0.1.4-v0.1.6
  releases (user decision, 2026-11-20, after a day of
  platform friction on a real laptop: a missing CRT DLL,
  then the missing `HOME` variable). ForgeHub is
  Linux-only again; the web app still works in any
  browser, and the CLI runs under WSL. The
  `USERPROFILE`-aware home resolution stays in the code -
  it is a few lines of robustness, not a platform.

## [0.1.6] - 2026-11-20

### Fixed

- **Every store command failed on Windows** with "neither
  FORGEHUB_HOME nor HOME is set". Windows terminals do not
  set `HOME`; the CLI now resolves the home directory the
  Windows way too - `HOME`, then `USERPROFILE`, then
  `HOMEDRIVE` + `HOMEPATH` - through one shared helper
  (`forge_core::vars::home_dir`) used by the store, the
  credentials path, the Forgenesis import dir, `doctor`,
  the alias installer and history analysis. Verified by
  unit tests over the resolution order.

## [0.1.5] - 2026-11-20

### Fixed

- **The Windows .exe died instantly on machines without
  the Visual C++ Redistributable.** It dynamically linked
  `VCRUNTIME140.dll`; the Windows build now statically
  links the MSVC runtime (`+crt-static`), so the exe is
  self-contained - download it and run it, nothing else to
  install. Everything else is unchanged from 0.1.4.

## [0.1.4] - 2026-11-20

### Fixed

- The release pipeline can finish now: GitHub does not
  offer build-provenance attestations for user-owned
  private repositories, which failed v0.1.3's collect job
  after all build legs had passed. The attestation step is
  removed; `SHA256SUMS` remains the integrity contract and
  `scripts/install.sh` verifies against it. v0.1.4 is the
  first published release; everything in the 0.1.1–0.1.3
  entries below is included.

## [0.1.3] - 2026-11-20

### Removed

- **macOS release binaries.** ForgeHub ships Linux
  (x86_64 + aarch64) and Windows (x86_64) builds only, by
  the project owner's decision - macOS CI minutes on a
  private repository are a cost the project does not
  carry. The web app is unaffected (it runs in any
  browser), the CLI still builds and runs on macOS from
  source, and `scripts/install.sh` on Darwin now says
  exactly that instead of pointing at assets that do not
  exist. This is the first release whose build matrix can
  go fully green, so it is the first published one;
  v0.1.1 and v0.1.2 were tagged but never published.

## [0.1.2] - 2026-11-20

### Fixed

- The Instrument Serif is a landing-page-only treatment
  again: headings inside the signed-in app use the standard
  UI typeface (Roboto Flex), per the user's clarification
  right after v0.1.1 was tagged. This release supersedes
  v0.1.1 (which was never published).

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
  popular-this-week and a run-anywhere card - separate from
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
  feedback - all silenced by `prefers-reduced-motion`.
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

The first release: a cloud home for shell command snippets -
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
  visibility, search, star - and running cloud snippets with
  the trust model: full command display, typed `yes` for other
  people's snippets, hash-pinned trust, changed-hash diff and
  re-confirmation, fail-safe without a terminal.
- **Server**: axum + SQLite (WAL, FTS5) API implementing
  `docs/openapi.yaml` - accounts with argon2id passwords and
  hashed sessions, API tokens and device login, snippet CRUD
  with immutable versions, full-text search, explore feeds,
  profiles, stars, forks, reports, and an operator admin CLI.
- **Web**: Material You 3 app (seed `#36618E`) - auth, dashboard,
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
