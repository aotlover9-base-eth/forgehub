# API

The normative contract is **[openapi.yaml](openapi.yaml)**
(OpenAPI 3.1) - this page is the orientation, not a second
source of truth.

## Shape

- Base path: `/api/v1` on the server origin
  (e.g. `https://hub.example.com/api/v1`).
- Health: `GET /api/v1/health` → `{"status":"ok","db":"ok"}`
  (503 with the error envelope when the DB is down).
- Version: `GET /api/v1/version` → `{"version":"0.1.0","git_sha":"…"}`.
- Raw snippet body: `GET /api/v1/snippets/{owner}/{name}/raw`
  (`text/plain; charset=utf-8`).

## Authentication

Two mechanisms, one identity:

1. **Session cookie** (web): `forgehub_session`, HttpOnly,
   set by `POST /auth/signup` / `POST /auth/login`. State-
   changing requests need the session's CSRF token in the
   `X-CSRF-Token` header (returned by `GET /me`) and pass an
   Origin check. Session-only endpoints (token management,
   device approval, session list) reject bearer tokens with
   `403 session_required`.
2. **Bearer token** (CLI/scripts):
   `Authorization: Bearer fgh_…`. Tokens have scopes - `read`,
   or `read write` (write implies read) - optional expiry, are
   shown once at creation, and are stored SHA-256-hashed.
   `DELETE /tokens/current` revokes the token used to call it.

When both are present, the `Authorization` header wins.

**Password reset** (2026-11-20): `POST /auth/password-reset/request`
with `{ "email": "…" }` always answers `204` - registered or
not - and mails a single-use link (one live link per account,
one-hour expiry, token stored SHA-256-hashed, throttled per IP
and per address). `POST /auth/password-reset/confirm` with
`{ "token": "…", "new_password": "…" }` answers `204` on
success and one generic `400 invalid_token` for an unknown,
used or expired token; a successful reset revokes every
session and API token on the account. Mail delivery needs
`FORGEHUB_SMTP_URL` (see `docs/running.md`); without it, dev
servers log the link and production servers log the request
as undeliverable.

## Errors

Every error is the same envelope:

```json
{ "error": { "code": "version_conflict", "message": "…", "details": { "latest_version": 3 } } }
```

Stable codes include `validation`, `unauthorized`,
`session_required`, `not_found` / `snippet_not_found`,
`username_taken`, `name_taken`, `version_conflict` (412),
`secret_detected` (422, with `details.findings` = kind + line
only), `body_too_large` (413), `rate_limited` (429, with
`Retry-After`), and `internal_error` (500, never with storage
internals).

## Concurrency and caching

- `PUT /snippets/{owner}/{name}` **requires** `If-Match`
  carrying the latest **version number**; a mismatch is
  `412 version_conflict` (checked inside the write
  transaction, so races cannot slip through).
- `GET` responses carry an `ETag` of the content hash;
  `If-None-Match` yields `304`.
- Lists paginate with opaque cursors: pass `limit` (≤ 100)
  and follow `next_cursor` until it is null.

## Endpoint map

| Area | Endpoints |
|---|---|
| Auth | `POST /auth/signup|login|logout|logout-all`, `GET/PATCH/DELETE /me`, `GET /me/sessions`, `DELETE /me/sessions/{ref}` |
| Device flow | `POST /auth/device/code`, `GET /auth/device/lookup`, `POST /auth/device/approve|deny`, `POST /auth/device/token` (poll) |
| Tokens | `GET/POST /tokens`, `DELETE /tokens/{id}`, `DELETE /tokens/current` |
| Snippets | `POST /snippets`, `GET/PUT/PATCH/DELETE /snippets/{owner}/{name}`, `GET /snippets/{owner}/{name}/versions[/{n}]`, `GET …/diff?from&to`, `POST /snippets/validate`, `GET /me/snippets` |
| Discovery | `GET /search`, `GET /explore`, `GET /tags`, `GET /users/{username}`, `PUT/DELETE /snippets/{o}/{n}/star`, `POST /snippets/{o}/{n}/fork`, `POST /snippets/{o}/{n}/report` |
| Collections | `POST /collections`, `GET /me/collections`, `GET/PATCH/DELETE /collections/{owner}/{name}`, `PUT/DELETE /collections/{o}/{n}/items/{so}/{sn}`, `POST /collections/{o}/{n}/reorder` |
| Public pages (not JSON) | `GET /s/{owner}/{name}` - server-rendered HTML for public snippets only (Peak C4); everything else is the same 404 |

Rate limits: a per-IP/token request budget (default 600/min)
plus a dedicated login throttle with exponential backoff and
a device-lookup lockout. Abuse handling for operators is in
`docs/operations.md`.
