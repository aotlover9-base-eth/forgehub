# ADR 0005 - Web auth uses cookie sessions; the CLI uses bearer tokens

- **Status:** Accepted (Step 2, 2026-11-19)

## Context
The browser app and the CLI have opposite constraints. A browser needs
ambient, same-origin authentication that survives reloads and can be
revoked instantly; putting a long-lived bearer token in `localStorage`
exposes it to any XSS. A CLI has no cookie jar, runs in scripts and CI, and
needs revocable, scoped, non-interactive credentials.

## Decision
- **Web:** server-side sessions. A 256-bit random session id in an
  `HttpOnly`, `SameSite=Lax`, `Secure`-in-production cookie; only its
  SHA-256 is stored; rotation on login and password change; idle + absolute
  expiry; CSRF defence is Origin/Host checking **plus** a per-session
  `X-CSRF-Token` header (spec: openapi `cookieSession`).
- **CLI / API clients:** personal API tokens (`fgh_` + 256-bit random,
  stored hashed, scoped `read`/`write`, shown once), sent as
  `Authorization: Bearer`. Bearer requests carry no ambient authority, so
  they are CSRF-exempt. Tokens cannot mint or revoke tokens or change the
  password - those operations require a cookie session.
- The device-code flow (Step 8) is the bridge: a browser (cookie) approves,
  a CLI receives a token.

## Alternatives rejected
- **JWTs in the browser:** revocation and logout-all become client-side
  fiction without a server denylist, which reintroduces server state with
  worse ergonomics than sessions.
- **One token system for both:** either the browser stores a bearer token
  (XSS-exfiltratable) or the CLI manages cookies and CSRF (fragile in
  scripts). Neither is acceptable.
- **OAuth providers in v1:** explicitly a non-goal (roadmap §1.2, Future §10).

## Consequences
Two auth extractors exist server-side (`CurrentUser` from cookie or
bearer), and the authorisation test matrix (Step 9) must cover both against
every endpoint.
