# Security policy

## Reporting a vulnerability

Please report suspected vulnerabilities **privately** to the maintainer
(GitHub user `aotlover9-base-eth`) - for example via a private GitHub
security advisory on this repository ("Security" tab → "Report a
vulnerability") - rather than opening a public issue.

Please include:

- a description of the issue and its impact,
- the exact steps or proof-of-concept needed to reproduce it,
- the version (CLI `--version` output or server `/api/v1/version`) you tested.

You can expect an acknowledgement within a few days. Because ForgeHub runs
other people's shell commands by design, reports about the confirmation and
trust model (anything that lets a snippet run, or appear different, without
the user's informed consent) are treated as the highest severity.

## Supported versions

Only the latest `0.1.x` release receives security fixes. There is no LTS.

## Scope notes

- ForgeHub is **not a sandbox**: a snippet you confirm runs with your full
  user privileges. The safety model is visibility and consent - see
  `docs/security.md` for the user-facing guidance.
- The server operator can read the database, including private snippet
  bodies (they are not end-to-end encrypted).
- Account recovery does not exist in v1 (no email is sent): a lost password
  means a lost account.
