# Peak C6 - Shared read-only collection links

Commit: `peak C6: shared collection links` (hash recorded in PROGRESS.md after push)
Date: 2026-11-22

## What shipped

An owner can mint a read-only share link for any of their collections -
including a private one - and anyone holding the link can read the
collection's current snippets on a server-rendered page, with no
account. The design is **ADR 0011** (written before the code):

- **Built like an API token.** 43 random base62 characters behind an
  `fgl_` prefix; only the SHA-256 hash and an 8-character prefix are
  stored (migration `0004_collection_share_links.sql`). The plaintext
  URL is shown exactly once, at mint time, in the web UI and by
  `fg collection share`.
- **Always expiring, instantly revocable.** Expiry is 1, 7 or 30 days
  (7 default) - there is no forever option. Revocation takes effect on
  the next request. Expired, revoked and unknown tokens all render the
  same 404 page; the endpoint is not an oracle. At most 10 unrevoked
  links per collection; deleting a collection cascades its links away.
- **View-only grant.** The page at `/shared/{token}` shows members in
  order with their current code and states plainly that running needs
  an account and normal access. It carries `noindex`, inherits the
  global CSP, escapes every interpolated value, and works without
  JavaScript. Admin-hidden and soft-deleted snippets are excluded -
  an admin action outranks an owner's share.
- **Live membership.** The page reads the collection as it is now:
  removing a snippet removes it from every outstanding link (proven by
  test), adding one shares it. The web UI states this in plain words
  next to the mint button, so the owner consents to the live semantics.

Surfaces: `POST/GET /collections/{owner}/{name}/share-links` and
`DELETE .../share-links/{id}` (owner-only, existence-masked: other
users get 404, anonymous 401, read-only tokens 403 on mint); CLI
`fg collection share [--days N]`, `fg collection shares`,
`fg collection unshare <id>`; a Share links card on the web collection
page (owner only) with the one-time URL and copy button.

This is the one deliberate exception to C1's "a collection never
widens access" invariant, and ADR 0011 scopes it: only through an
explicit, expiring, revocable link the owner created.

## Tests

- Server unit: +2 (link activity rules - expiry/revocation edges;
  token generation shape, uniqueness, hash storage).
- Server integration (`tests/share_links.rs`): +2 tests covering the
  full lifecycle (mint, list shows prefix and never the token,
  anonymous page shows a private snippet's code under the CSP,
  unknown/malformed tokens 404, live membership after item removal,
  revoke kills the page instantly, double-revoke 404, list reflects
  revocation) and the authorisation matrix (other user 404 on mint/
  list/revoke, anonymous 401, bad expiry 400, read-only token 403).
- CLI: +1 integration test (`share_link_commands_round_trip`) driving
  share/shares/unshare against the real server, including fetching
  the anonymous page and its death after revocation.
- Web: +1 test (mint shows the URL once, list renders prefixes,
  revoke calls through).

## Gates (run locally, this machine)

- `cargo fmt --all --check` clean; `cargo clippy --workspace
  --all-targets -- -D warnings` clean (three findings fixed in-WP).
- `cargo test --workspace`: **522 passed, 0 failed** (517 at C5).
- Web: lint, typecheck (verified by exit code), **75 tests passed**,
  build, check-tokens all green.
- OpenAPI gained the three endpoints and the `ShareLink` schema; the
  collection man pages were regenerated (3 new pages) and the CLI
  reference updated.

## Also in this commit

- PROGRESS.md records C5's CI verification (run 44980899065, fully
  green, all 5 jobs) which completed after the C5 commit landed.
