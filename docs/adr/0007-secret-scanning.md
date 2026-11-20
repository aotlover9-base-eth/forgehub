# ADR 0007 - Secret scanning: one shared scanner, patterns + context, never echoing values

- **Status:** Accepted (Step 2, 2026-11-19)

## Context
Commands routinely contain credentials (`--password`, `TOKEN=…`, pasted API
keys, private-key blocks). Once a secret is in an immutable, possibly public
version history it cannot be reliably un-published, so detection must
happen **before** persistence, at every entry point: `fg record`, the CLI's
`push`, the server's create/update, and the web editor.

## Decision
One scanner in `forge-core` (Step 3), used by all four call sites, combining:
1. **Known-format patterns:** private-key blocks (`-----BEGIN … PRIVATE KEY-----`), AWS access key ids, GitHub/GitLab/Slack/Stripe-style token prefixes, generic bearer tokens.
2. **Assignment context:** `password=` / `--password` / `passwd` literals and `KEY=value` assignments whose key name suggests a secret (`token`, `secret`, `password`, `api_key`, …) with a non-placeholder value.
3. **High-entropy strings in assignment context only** (never free-floating - command lines are full of hashes and ids that are not secrets).

Findings carry `{kind, line}` and **never the matched value**. The scanner
offers a redaction function (replace the value with `{{VARIABLE}}` guidance)
for the recorder. Behaviour: client-side `record`/`push` **refuse**;
server-side create/update **rejects 422** with findings; the web editor
blocks Save. There is no "save anyway" override in v1 - a user who truly
needs a literal can split it through a variable, which is the documented
workaround.

Danger lints (sudo, `rm -rf /`, `curl|sh`, …) are a **separate**, warning-only
system in the same crate: heuristics about risk, never about secrecy, and
never blocking.

## Alternatives rejected
- **Server-only scanning:** `record` would already have written the secret
  to local disk, and offline workflows would carry it indefinitely.
- **Client-only scanning:** a hostile or old client could push secrets the
  server then stores and serves.
- **Entropy scanning everywhere:** false positives on hashes, UUIDs and
  base64 in legitimate commands would train users to ignore findings.
- **Third-party scanner service / phoning home:** violates the no-telemetry
  rule and sends the very secrets at issue to a third party.

## Consequences
The scanner ships with positive and negative corpora in Step 3 and is
fuzz/perf-tested in Step 19 (threat T-DOS-3). False-negative honesty: the
docs state the scanner catches known shapes, not all secrets.
