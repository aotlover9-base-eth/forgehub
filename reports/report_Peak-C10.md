# Peak C10 - forgehub self-check and the CHANGELOG

Commit: `peak C10: self-check and changelog`
Plan: Phase C, C10 - "`forgehub self-check` (verifies
the installed binary matches the release checksum when
online; offline it explains how) and the CHANGELOG
entries for everything user-visible in this phase."

## self-check (crates/forge-cli/src/selfcheck.rs)

An honest verifier, built around what the releases
actually publish:

- Releases ship `SHA256SUMS` over the release ARCHIVES
  (the two Linux tarballs), not over the extracted
  binary - so a binary cannot be matched against them
  directly, and the command says so instead of pretending
  otherwise.
- Always printed: version, binary path, the binary's own
  SHA-256 (comparable across installs), the release asset
  name for this platform, and the published archive hash
  when the sums were fetched.
- `--archive <file>` verifies a downloaded archive end
  to end against the published entry: match exits 0,
  mismatch (or no entry) exits 1 with a do-not-install
  warning.
- Sums source: `--sums <file>` for offline verification,
  else HTTPS from the release (5 s connect / 10 s total,
  the CLI's usual timeouts). `FORGEHUB_SUMS_URL` overrides
  the URL for mirrors and tests.
- When the sums cannot be fetched - offline, or the
  private repository answering 404 to anonymous HTTPS -
  the command explains the manual flow (`gh release
  download v<version>` + `sha256sum -c SHA256SUMS`) and
  still exits 0; a requested archive verification that
  cannot complete exits 1.
- `--json` emits version, binary path and hash, asset
  name, published hash, sums note and the archive verdict.

Live run on this machine (debug build): version 0.1.6,
binary sha256 printed, the private repo's 404 classified
correctly as "no published SHA256SUMS found", manual
steps printed, exit 0.

## Tests

- Unit (3): asset names for the two published platforms
  and none for Windows/macOS; SHA256SUMS parsing
  (binary markers, path prefixes, uppercase hashes, junk
  lines); file hashing against the known sha256("abc")
  vector.
- Integration (4, tests/self_check.rs, real binary):
  forced-offline run (loopback port 1) reports the hash
  and explains, exit 0; archive + local sums match exits
  0; mismatch exits 1 with MISMATCH; --json shape.

## CHANGELOG and docs

- CHANGELOG `[Unreleased]` now carries the Peak phase:
  Added - C1 collections, C2 typed parameters, C3
  history/rollback, C4 public pages, C5 import helpers,
  C6 share links, C7 activity log, C8 library stats, C10
  self-check; Changed - the Phase A Material You rebuild,
  the Phase B CLI output/help work, C9 i18n groundwork.
- docs/cli-reference.md: self-check section with the
  design note. Man pages regenerated
  (`forgehub --gen-man docs/man`): only the top page
  changed plus the new forgehub-self-check.1 - the rest
  were already current.

## Gates (this machine)

- cargo fmt --all --check clean; clippy --workspace
  --all-targets -D warnings clean (4 selfcheck lints
  fixed in-WP: doc backticks, map_or_else, if-let,
  semicolon).
- **cargo test --workspace: 535 passed, 0 failed**
  (528 at C8 + 7 new).
- No web or server changes in this WP.
