# ForgeHub security

This document is the user- and operator-facing companion to
`docs/threat-model.md` (the engineering threat model, re-verified with
evidence in Step 19 — see the verification log at the bottom of that file).

## The one thing to understand

**ForgeHub is not a sandbox.** When you run a snippet — yours or someone
else's — its commands execute in your shell, with your permissions, on your
machine. ForgeHub's job is to make sure you always know *exactly* what will
run and *whose* it is before anything executes, and that nothing runs
silently or changes underneath you. It cannot make a command you approved
safe.

## What protects you when you run other people's snippets

- **Full display, always.** The confirmation card shows the owner, the
  version, the content hash, and *every* command, numbered. Text is
  escaped for display (ANSI escapes, carriage returns, bidi overrides and
  invisible characters are neutralised), so a command cannot hide behind
  terminal tricks.
- **Typed `yes` for strangers.** Running someone else's snippet for the
  first time requires typing `yes` — a reflexive Enter is not enough, and
  `-y` never skips this.
- **Trust is pinned to content.** If a snippet you ran before changes, the
  next run shows a diff against the version you last ran and asks again.
  `-y` never bypasses a change.
- **Danger warnings.** Commands involving `sudo`, recursive deletes,
  `curl | sh`, disk tools and similar are flagged on the card before you
  decide.
- **Pinning.** `fg owner/name@3` runs exactly version 3.
- **No auto-run, ever.** Search results and suggestions are never executed
  automatically.

Practical guidance: prefer snippets from people you know; read every line
of a first run (especially anything with `sudo`, `curl`, or `rm`); use
`--dry-run` to preview; check `fg trust list` occasionally and revoke trust
you no longer recognise.

## Your account and tokens

- Passwords are stored with argon2id; login is throttled per IP and per
  account, and failure messages never reveal whether an account exists.
- Sessions are 256-bit random ids, stored hashed, in `HttpOnly` cookies
  (`Secure` in production, `SameSite=Lax`), rotated at login and password
  change. You can review and revoke sessions under Settings → Security.
- API/device tokens are 256-bit random, shown **once**, stored hashed, and
  scoped (`read` or `read write`). A token can never change your password
  or manage other tokens. Revocation is immediate.
- The CLI stores its token in `~/.config/forgehub/credentials.toml`
  (directory `0700`, file `0600`) and refuses to read a credential file
  with loose permissions.
- **Use `https://` registries.** The CLI warns at login when a registry is
  plain `http://` to a non-loopback host, because your token would cross
  the network unencrypted. `fg doctor` flags the same thing.
- Device login: only approve a code that *your own terminal* just showed
  you. The approval page names the client and what it will be able to do;
  ForgeHub staff will never ask you to approve a code.

## Privacy

- No telemetry, no analytics, no third-party scripts or fonts in the web
  app. Pull counts are aggregate counters (deduplicated per IP per hour);
  there is no per-user run history on the server.
- Private snippets are confidential against other users (they return 404,
  indistinguishable from missing, on every endpoint), but they are **not**
  end-to-end encrypted: the server operator can read the database.

## For operators

- Set `FORGEHUB_ENV=prod` and a ≥ 32-byte `FORGEHUB_SECRET_KEY`; terminate
  TLS in front of the server (e.g. Caddy) so `Secure` cookies and HSTS
  apply. The app sets CSP (no inline scripts), `nosniff`,
  `X-Frame-Options: DENY`, a restrictive `Permissions-Policy` and
  `Referrer-Policy` on every response.
- Client IPs for throttling and rate limiting come from the socket, never
  from `X-Forwarded-For`; if you front the server with a proxy, all users
  behind it share one throttle identity — size limits accordingly.
- Abuse handling: `forge-server admin list-reports`,
  `admin hide-snippet <owner>/<name>`, `admin ban-user <username>`
  (banning revokes the user's sessions and tokens immediately).
  `FORGEHUB_ALLOW_SIGNUP=false` closes registration.
- Supply chain: `cargo deny check`, `cargo audit` and
  `npm audit --omit=dev` run in CI on every push; GitHub Actions are
  pinned to commit SHAs.

## Reporting

See `SECURITY.md` at the repository root.
