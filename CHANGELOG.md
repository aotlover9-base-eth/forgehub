# Changelog

All notable changes to ForgeHub are documented here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
